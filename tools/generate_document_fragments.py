#!/usr/bin/env python3
from pathlib import Path
import argparse, yaml, json, sys
ROOT=Path(__file__).resolve().parents[1]
SPEC=ROOT/'spec'; GEN=ROOT/'generated'
GEN.mkdir(exist_ok=True)
def load(n): return yaml.safe_load((SPEC/n).read_text(encoding='utf-8'))
def rec(title,fields):
    s=f'**{title}**\n\n'
    for k,v in fields: s+=f'- **{k}:** {v}\n\n'
    return s
outs={}
st=load('states.yaml')
s=''
for n,m in st['machines'].items(): s+=rec(n,[('Stany',', '.join(m['states'])),('Przejścia',json.dumps(m['transitions'],ensure_ascii=False,sort_keys=True)),('Semantyka',m.get('semantics','domain state machine'))])
outs['states.md']=s
ap=load('approval.yaml'); s=''
for r,d in ap['risk_classes'].items(): s+=rec(r,[('Znaczenie',d),('Domyślna bramka',ap['default_gate'][r])])
for g,m in ap['gate_to_approval_state'].items(): s+=rec('gate:'+g,[(k,v) for k,v in m.items()])
s+=rec('mutation_policy', list(ap['concurrency']['mutation_policy'].items()))
s+='## Execution authorization contracts\n\n```yaml\n'+yaml.safe_dump({k:ap[k] for k in ('policy_decision','assurance','record','binding_validity','execution_authorization','concurrency','recipient_boundary','dynamic_risk_classifiers')},sort_keys=False,allow_unicode=True)+'```\n'
outs['approval.md']=s
domain=load('domain.yaml')
s='# GENERATED - do not edit\n\n'
for section in ('primitives', 'value_objects', 'execution_contracts'):
    s+=f'## {section}\n\n```yaml\n'+yaml.safe_dump(domain[section], sort_keys=False, allow_unicode=True)+'```\n\n'
outs['execution_domain.md']=s
for filename in ('action_binding.yaml', 'execution.yaml'):
    outs[filename.replace('.yaml', '.md')] = '# GENERATED - do not edit\n\n```yaml\n' + yaml.safe_dump(load(filename), sort_keys=False, allow_unicode=True) + '```\n'
outs['audit.md'] = '# GENERATED - do not edit\n\n```yaml\n' + yaml.safe_dump(load('audit.yaml'), sort_keys=False, allow_unicode=True) + '```\n'

# Compact canonical registry for humans/agents. This file is generated from the
# same YAML as the detailed fragments and must never become a hand-maintained
# second source of truth.
s='# GENERATED - do not edit\n\n## Risk classes\n\n'
for r,d in ap['risk_classes'].items():
    s+=f'- `{r}`: {d}\n'
s+='\n## Default gates\n\n'
for r,g in ap['default_gate'].items():
    s+=f'- `{r}` -> `{g}`\n'
s+='\n## Gate to ApprovalState mapping\n\n'
for g,m in ap['gate_to_approval_state'].items():
    s+=f'- `{g}`: {json.dumps(m, ensure_ascii=False, sort_keys=True)}\n'
s+='\n## State machines\n\n'
for n,m in st['machines'].items():
    s+=f'- `{n}`: '+', '.join(f'`{x}`' for x in m['states'])+'\n'
s+='\n## Pause semantics\n\n'
ps=st.get('pause_semantics',{})
for k,v in ps.items():
    s+=f'- **{k}:** {json.dumps(v, ensure_ascii=False, sort_keys=True) if isinstance(v,(dict,list,bool)) else v}\n'
outs['canonical_registry.md']=s
t=load('testing.yaml'); s=''
for suite,items in t['suites'].items(): s+=rec(suite,[('Zakres',', '.join(items))])
s+='### Golden vectors\n\n'+''.join(f'- {x}\n' for x in t['golden_vectors'])
outs['testing.md']=s
ac=load('acceptance.yaml'); s=''
for stage,items in ac['releases'].items():
    s+=f'### {stage}\n\n'
    for x in items: s+=rec(x['id'],[('Wymaganie',x['requirement'])])
outs['acceptance.md']=s
s=''
for f in sorted(SPEC.glob('*.yaml')): s+=f'## A.{f.name}\n\n```yaml\n{f.read_text(encoding="utf-8").rstrip()}\n```\n\n'
outs['spec_appendix.md']=s

p=argparse.ArgumentParser(); p.add_argument('--check',action='store_true'); args=p.parse_args()
errors=[]
for name,content in outs.items():
    path=GEN/name
    content=content.rstrip()+'\n'
    if args.check:
        if not path.exists() or path.read_text(encoding='utf-8')!=content: errors.append(name)
    else: path.write_text(content,encoding='utf-8',newline='\n')
if errors:
    print('GENERATED DOC DRIFT:',', '.join(errors)); sys.exit(1)
print(('GENERATED DOC CHECK OK' if args.check else 'GENERATED DOCS WRITTEN')+f': {len(outs)} fragments')
