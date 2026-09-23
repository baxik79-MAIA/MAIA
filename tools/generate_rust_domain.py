#!/usr/bin/env python3
"""Generate the OS-neutral domain declarations. No Cargo or network required."""
import argparse
import json
from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = Path('core/domain/src/generated/contracts.rs')
SOURCES = ('domain.yaml', 'approval.yaml', 'modes.yaml', 'states.yaml', 'action_binding.yaml', 'execution.yaml', 'audit.yaml', 'reasoning.yaml', 'outcomes.yaml', 'commercial.yaml', 'metrics.yaml')


def load(root=ROOT):
    return {name: yaml.safe_load((root / 'spec' / name).read_text(encoding='utf-8'))
            for name in SOURCES}


def variant(value):
    return ''.join(part.capitalize() for part in value.split('_'))


def render(data):
    domain = data['domain.yaml']
    out = ['// GENERATED / DO NOT EDIT. Run python tools/generate_rust_domain.py.',
           '// Canonical inputs: ' + ', '.join('spec/' + n for n in SOURCES),
           '// Audit contract snapshot: ' + json.dumps(data['audit.yaml'], sort_keys=True, separators=(',', ':')),
           'use crate::*;', '']

    def resolve(ref):
        file, path = ref.split('#')
        value = data[file.removeprefix('spec/')]
        for key in path.split('.'):
            value = value[key]
        return value

    def enum(name, values):
        out.append(f'canonical_enum!({name} {{')
        out.extend(f'    {variant(v)} => "{v}",' for v in values)
        out.append('});')

    for name, meta in domain['primitives'].items():
        if 'enum_source' in meta:
            enum(name, resolve(meta['enum_source']))
            if 'transitions_source' in meta:
                out.append(f'canonical_fsm!({name} {{')
                for src, dests in resolve(meta['transitions_source']).items():
                    for dst in dests:
                        out.append(f'    {variant(src)} => {variant(dst)},')
                out.append('});')
        elif meta.get('format') == 'uuid':
            out.append(f'string_type!({name}, validate_uuid_v7);')
        elif meta.get('min_length') == 1:
            out.append(f'string_type!({name}, validate_nonempty);')
        elif meta['wire_type'] == 'integer':
            out.append(f'integer_type!({name}, u{meta["bits"]}, {meta["minimum"]});')
        elif name == 'Utf8String':
            out.append('pub type Utf8String = String;')
        else:
            validator = {'ActionType': 'validate_action_type', 'Timestamp': 'validate_timestamp',
                         'Sha256Hex': 'validate_sha256', 'CurrencyCode': 'validate_currency'}[name]
            out.append(f'string_type!({name}, {validator});')
    certainty = domain['value_objects']['OutcomeCertainty']
    enum('OutcomeCertainty', certainty['values'])
    out.extend(['impl RunState {', '    pub const fn outcome_certainty(self) -> OutcomeCertainty {',
                '        match self {'])
    for state, value in certainty['state_coupling'].items():
        out.append(f'            Self::{variant(state)} => OutcomeCertainty::{variant(value)},')
    out.extend(['        }', '    }', '}'])
    # Structural closed-set predicates, sourced from canonical declarations.
    for name, method, values in [
        ('RiskClass', 'permits_policy_routing', data['action_binding.yaml']['connector_selection']['policy_routed']['allowed_risks']),
        ('RunState', 'is_in_flight', data['states.yaml']['pause_semantics']['in_flight_states']),
    ]:
        patterns = ' | '.join('Self::' + variant(value) for value in values)
        out.extend([f'impl {name} {{', f'    pub const fn {method}(self) -> bool {{',
                    f'        matches!(self, {patterns})', '    }', '}'])
    out.extend(['impl PolicyDecision {', '    pub const fn required_assurance(self) -> Option<ApprovalAssurance> {', '        match self {'])
    required = data['approval.yaml']['assurance']['required_by_decision']
    for decision in data['approval.yaml']['policy_decision']['values']:
        value = f'Some(ApprovalAssurance::{variant(required[decision])})' if decision in required else 'None'
        out.append(f'            Self::{variant(decision)} => {value},')
    out.extend(['        }', '    }', '}'])
    out.extend(['impl PolicyDecision {', '    pub const fn permits_record_state(self, state: ApprovalState) -> bool {', '        match self {'])
    compatibility = data['approval.yaml']['record']['snapshot_state_compatibility']
    for decision in data['approval.yaml']['policy_decision']['values']:
        patterns = ' | '.join('ApprovalState::' + variant(s) for s in compatibility[decision])
        expression = f'matches!(state, {patterns})' if patterns else 'false'
        out.append(f'            Self::{variant(decision)} => {expression},')
    out.extend(['        }', '    }', '}'])
    contracts = {**domain['value_objects'], **domain.get('artifact_contracts', {}), **domain.get('outcome_contracts', {}), **domain['execution_contracts']}
    known = set(domain['primitives']) | set(contracts)
    for name, meta in contracts.items():
        if 'fields' not in meta:
            continue
        out.append(f'domain_struct!({name} {{')
        for field, spec in meta['fields'].items():
            ty = spec['type']
            base = ty[5:-1] if ty.startswith('list<') and ty.endswith('>') else ty
            if base not in known:
                raise ValueError(f'Unknown field type: {ty}')
            if ty.startswith('list<'):
                ty = f'Vec<{base}>'
            if not spec['required'] or spec['nullable']:
                ty = f'Option<{ty}>'
            out.append(f'    {field}: {ty},')
        out.append('});')
    return '\n'.join(out) + '\n'


def check(root, content):
    path = root / OUTPUT
    return path.exists() and path.read_text(encoding='utf-8') == content


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    content = render(load())
    if args.check:
        if not check(ROOT, content):
            print(f'GENERATED RUST DRIFT: {OUTPUT.as_posix()}')
            return 1
        print('GENERATED RUST CHECK OK: 1 contract')
    else:
        path = ROOT / OUTPUT
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding='utf-8', newline='\n')
        print('GENERATED RUST WRITTEN: 1 contract')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
