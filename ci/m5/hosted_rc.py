"""Finite hosted CI adapter for the unchanged internal RC; never Labtop admission."""
from pathlib import Path
import argparse, base64, datetime, hashlib, json, os, shutil, stat, subprocess, sys, time, types

HERE = Path(__file__).resolve().parent
FLAGS = ('native_image_chain_proven', 'native_run_ready', 'apply_permitted', 'durability_proven')
ARTIFACT_SHA = '572fa19a023a2cf4ab7c4005ee63916b4ffb816332f97dc1e4be9c98157a9244'
HELPER_SHA = '54a3d8a95a105ca86ca80916b9c2e573c9a4d3cef2297abe82cc02ee2b394339'
PUBLICATION_SHA = '8f24d81ae864729b78c30c5b50b6ad562224c04baa1339626cd62ad9208c25c9'
sha = lambda b: hashlib.sha256(b).hexdigest()


def need(ok, reason):
    if not ok:
        raise RuntimeError(reason)


def read(path, bound=32 * 1024 * 1024, os_runtime_links=False):
    path = Path(path).absolute()
    for p in (path.parent, *path.parent.parents):
        i = p.lstat()
        need(stat.S_ISDIR(i.st_mode) and not p.is_symlink() and not getattr(i, 'st_file_attributes', 0) & 1024, 'ordinary input ancestor')
    a = path.lstat()
    need(stat.S_ISREG(a.st_mode) and (a.st_nlink >= 1 if os_runtime_links else a.st_nlink == 1) and not path.is_symlink() and not getattr(a, 'st_file_attributes', 0) & 1024 and a.st_size <= bound, 'ordinary input file')
    raw = path.read_bytes()
    b = path.lstat()
    need((a.st_dev, a.st_ino, a.st_size, a.st_mtime_ns) == (b.st_dev, b.st_ino, b.st_size, b.st_mtime_ns) and len(raw) == a.st_size, 'input drift')
    return raw


def save(path, raw):
    with path.open('xb') as stream:
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())
    need(read(path) == raw, 'report readback')


def encoded(value):
    return (json.dumps(value, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode()


def runtime(path, os_runtime_links=False):
    path = Path(path).resolve(strict=True)
    raw = read(path, 256 * 1024 * 1024, os_runtime_links)
    return {'path': str(path), 'bytes': len(raw), 'sha256': sha(raw), 'links': path.stat().st_nlink, 'scope': 'FILE_BYTES_NOT_LABTOP_PINS_OR_EXECUTED_IMAGE_PROOF'}


def inputs(manifest_sha):
    raw = read(HERE / 'INPUT_MANIFEST.json')
    need(sha(raw) == manifest_sha, 'workflow input manifest SHA')
    manifest = json.loads(raw)
    need(manifest['schema'] == 1 and set(manifest['files']) == {'hosted_rc.py', 'm5_rc.py', 'M4_PUBLICATION_RESULT.json', 'MAIA-M5-DEV-STAGING-RC1.zip'}, 'exact input inventory')
    buffers = {}
    for name, pin in manifest['files'].items():
        b = read(HERE / name)
        need(len(b) == pin['bytes'] and sha(b) == pin['sha256'], 'exact input bytes: ' + name)
        buffers[name] = b
    need(sha(buffers['m5_rc.py']) == HELPER_SHA and sha(buffers['M4_PUBLICATION_RESULT.json']) == PUBLICATION_SHA, 'reviewed helper / accepted publication')
    archive = buffers['MAIA-M5-DEV-STAGING-RC1.zip']
    need(len(archive) == 244307 and sha(archive) == ARTIFACT_SHA, 'same actual Windows-built RC')
    tool = types.ModuleType('_unchanged_m5_rc_hosted_verification')
    tool.__file__ = str(HERE / 'm5_rc.py')
    exec(compile(buffers['m5_rc.py'], tool.__file__, 'exec'), tool.__dict__)
    files = tool.verify_bytes(archive, expected_m4_publication=buffers['M4_PUBLICATION_RESULT.json'])
    need(len(files) == 52 and files['tools/m5_rc.py'] == buffers['m5_rc.py'], '52-member artifact / trusted helper join')
    return tool, files, manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest-sha256', required=True)
    parser.add_argument('--preflight-only', action='store_true')
    parser.add_argument('--local-validation', action='store_true')
    parser.add_argument('--local-root')
    args = parser.parse_args()
    tool, files, manifest = inputs(args.manifest_sha256)
    if args.preflight_only:
        print(json.dumps({'status': 'HOSTED_CI_INPUT_PREFLIGHT_PASS_NO_EXECUTION', 'artifact_sha256': ARTIFACT_SHA, 'members': len(files), 'hosted_ci_job_observed': False}))
        return 0
    local = args.local_validation
    if local:
        need(os.name != 'nt' and args.local_root and not os.environ.get('GITHUB_ACTIONS'), 'explicit local Linux adapter-validation mode')
        root = Path(args.local_root).absolute()
        need(not root.exists() and not root.is_symlink() and root.parent.is_dir(), 'fresh local validation root')
        root.mkdir()
        need(sys.version_info >= (3, 12) and sys.maxsize > 2**32, 'local CPython3.12+ x64')
    else:
        need(not args.local_root and os.environ.get('GITHUB_ACTIONS') == 'true' and os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted', 'GitHub-hosted runner only')
        need(os.environ.get('GITHUB_REPOSITORY') == 'baxik79-MAIA/MAIA', 'canonical CI repository')
        need(sys.version_info[:3] == (3, 13, 15) and sys.maxsize > 2**32, 'CPython 3.13.15 x64')
    python = str(Path(sys.executable).resolve(strict=True))
    node = str(Path(shutil.which('node')).resolve(strict=True))
    node_version = subprocess.run([node, '--version'], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10, check=True)
    need((local or node_version.stdout.strip() == b'v24.19.0') and not node_version.stderr, 'Node 24.19.0 in hosted mode')
    out = root / 'ci-evidence' if local else Path(os.environ['GITHUB_WORKSPACE']) / 'ci-evidence'
    need(not out.exists() and not out.is_symlink(), 'fresh report directory; no replay')
    out.mkdir()
    result = {'status': 'RC_PURE_CONTROLS_INCOMPLETE_NO_REPLAY', 'scope': 'LOCAL_ADAPTER_VALIDATION_NOT_HOSTED_CI' if local else 'SAME_UNSIGNED_RC_HOSTED_PORTABLE_CONTROLS_ONLY', 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'repository': os.environ.get('GITHUB_REPOSITORY') if not local else None, 'commit': os.environ.get('GITHUB_SHA') if not local else None, 'run_id': os.environ.get('GITHUB_RUN_ID') if not local else None, 'run_attempt': os.environ.get('GITHUB_RUN_ATTEMPT') if not local else None, 'job': os.environ.get('GITHUB_JOB') if not local else None, 'workflow_ref': os.environ.get('GITHUB_WORKFLOW_REF') if not local else None, 'runner_os': os.environ.get('RUNNER_OS') if not local else sys.platform, 'runner_arch': os.environ.get('RUNNER_ARCH'), 'runner_image': {'os': os.environ.get('ImageOS'), 'version': os.environ.get('ImageVersion')}, 'artifact_sha256': ARTIFACT_SHA, 'artifact_bytes': 244307, 'members': 52, 'helper_sha256': HELPER_SHA, 'input_manifest_sha256': args.manifest_sha256, 'python_version': sys.version, 'node_version': node_version.stdout.decode().strip(), 'hosted_ci_job_observed': not local, 'workflow_acceptance_pending_coordinator_reconciliation': True, 'full_m5_milestone_accepted': False, 'release_signature_created': False, 'installed_update_performed': False, 'schema_migration_performed': False, 'm4_native_effect_batch_replayed': False, 'store_claims': 0, 'effect_workers_created': 0, **{k: False for k in FLAGS}}
    started = time.monotonic()
    report = None
    try:
        if local:
            work, report = root / 'WORK', root / 'REPORT'
            work.mkdir()
        elif os.name == 'nt':
            root = Path('C:/MAIA')
            need(not root.exists() and not root.is_symlink(), 'fresh hosted Windows namespace')
            work = root / 'worktrees/M5_RC_20261109'
            report = root / 'reports/M5_RC_20261109/HOSTED_CI'
            work.mkdir(parents=True)
            report.parent.mkdir(parents=True)
        else:
            root = Path(os.environ['RUNNER_TEMP']) / 'maia-m5-hosted-rc'
            need(not root.exists() and not root.is_symlink(), 'fresh hosted Linux namespace')
            root.mkdir()
            work, report = root / 'WORK', root / 'REPORT'
            work.mkdir()
        runtimes = {'python': runtime(python), 'node': runtime(node)}
        result['runtime_files'] = runtimes
        save(out / 'ATTEMPT.json', encoded({'artifact_sha256': ARTIFACT_SHA, 'pure_controls_only': True, 'no_replay': True}))
        tool.preflight(HERE / 'MAIA-M5-DEV-STAGING-RC1.zip', work, report)
        result['copy'] = tool.stage(HERE / 'MAIA-M5-DEV-STAGING-RC1.zip', work, report)
        candidate = work / 'candidate'
        if os.name == 'nt':
            ps = str(Path(os.environ['SYSTEMROOT']) / 'System32/WindowsPowerShell/v1.0/powershell.exe')
            ps_before = runtime(ps, True)
            source = str(candidate / 'payload')
            ast = "$ErrorActionPreference='Stop';$r='" + source.replace("'", "''") + "';$all=@(Get-ChildItem -LiteralPath $r -Recurse -File|Where-Object{$_.Extension -in @('.ps1','.psm1')});foreach($p in $all){$t=$null;$e=$null;[void][Management.Automation.Language.Parser]::ParseFile($p.FullName,[ref]$t,[ref]$e);if($e.Count){throw ($p.FullName+':'+($e|Out-String))}};[pscustomobject]@{status='PS_AST_PASS';files=$all.Count}|ConvertTo-Json -Compress"
            qa = []
            result['windows_memory_qa'] = qa
            for label, args_ps in [('PS_AST', ['-EncodedCommand', base64.b64encode(ast.encode('utf-16-le')).decode()]), ('PS_MEMORY_WIRE', ['-File', str(candidate / 'payload/tests/Test-M4Wire.ps1'), '-SourceRoot', source])]:
                command = [ps, '-NoLogo', '-NoProfile', '-NonInteractive', *args_ps]
                try:
                    cp = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=False)
                except subprocess.TimeoutExpired as exc:
                    save(out / (label + '.stdout.log'), exc.stdout or b'')
                    save(out / (label + '.stderr.log'), exc.stderr or b'')
                    raise
                save(out / (label + '.stdout.log'), cp.stdout)
                save(out / (label + '.stderr.log'), cp.stderr)
                need(cp.returncode == 0 and not cp.stderr, 'Windows memory QA child failed')
                body = json.loads(cp.stdout.decode('utf-8-sig'))
                need(body == {'status': 'PS_AST_PASS', 'files': 5} if label == 'PS_AST' else body.get('qualified') is True and body.get('count') == 19 and len(set(body.get('passed', []))) == 19, 'Windows memory QA exact count')
                qa.append({'suite': label, 'exit_code': cp.returncode, 'stdout_sha256': sha(cp.stdout), 'stderr_sha256': sha(cp.stderr), 'result': body})
            need(runtime(ps, True) == ps_before, 'PowerShell runtime byte drift')
            result['powershell_runtime'] = ps_before
            result['windows_memory_qa_scope'] = 'AST5_WIRE19_MEMORY_ONLY_NOT_LABTOP_CAPSULE_POLICY_OR_NATIVE_PROOF'
        commands = [('M5_HELPERS', [python, '-I', '-S', '-B', '-m', 'unittest', 'discover', '-s', 'tools', '-p', 'test_m5*.py', '-v']), ('M3_PURE', [python, '-I', '-S', '-B', '-m', 'unittest', 'discover', '-s', 'payload/baseline_m3', '-p', 'test_m3*.py', '-v']), ('M4_PURE', [python, '-I', '-S', '-B', '-m', 'unittest', 'discover', '-s', 'payload', '-p', 'test_m4*.py', '-v']), ('UI_CONTROLLED_DOM', [node, '--test', '--test-reporter=tap', 'payload/test_m4_ui.js'])]
        env = {k: v for k, v in os.environ.items() if k.upper() in ('SYSTEMROOT', 'WINDIR', 'TEMP', 'TMP')}
        env['PYTHONDONTWRITEBYTECODE'] = '1'
        rows = []
        result['results'] = rows
        for label, command in commands:
            remaining = 210 - (time.monotonic() - started)
            need(remaining > 0, 'finite pure-control deadline')
            try:
                cp = subprocess.run(command, cwd=candidate, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=min(60, remaining), check=False)
            except subprocess.TimeoutExpired as exc:
                save(out / (label + '.stdout.log'), exc.stdout or b'')
                save(out / (label + '.stderr.log'), exc.stderr or b'')
                raise
            save(out / (label + '.stdout.log'), cp.stdout)
            save(out / (label + '.stderr.log'), cp.stderr)
            semantic = tool.suite_semantics(label, cp.returncode, cp.stdout, cp.stderr)
            rows.append({'suite': label, 'command': command, 'exit_code': cp.returncode, 'stdout_sha256': sha(cp.stdout), 'stderr_sha256': sha(cp.stderr), 'semantic': semantic})
            need(semantic['passed'] is True, 'exact pure-control suite failed: ' + label)
        need([row['semantic']['tests'] for row in rows] == [23, 64, 98, 1], '186 controls / no skipped substitute')
        need(tool.tree_bytes(candidate) == files, 'candidate unchanged after controls')
        need({k: runtime(v['path']) for k, v in runtimes.items()} == runtimes, 'hosted runtime byte drift')
        result['candidate_bytes_unchanged'] = True
        result['return'] = tool.return_candidate(work, report)
        need(tool.tree_bytes(work / 'retired_candidate') == files and not (work / 'candidate').exists(), 'retired postimage / absence')
        need(read(report / 'RC_BACKUP.zip') == read(HERE / 'MAIA-M5-DEV-STAGING-RC1.zip'), 'retained exact backup')
        result['status'] = 'LOCAL_ADAPTER_186_PURE_CONTROLS_AND_RETURN_PASS' if local else 'HOSTED_RC_186_PURE_CONTROLS_AND_RETURN_PASS'
        result['no_replay'] = True
    except BaseException as exc:
        result['error'] = type(exc).__name__ + ':' + str(exc)
        raise
    finally:
        if report is not None:
            for name in ('ATTEMPT.json', 'PREIMAGE.json', 'COPY_RESULT.json', 'RETURN_ATTEMPT.json', 'RETURN_RESULT.json'):
                if (report / name).is_file():
                    save(out / ('STAGING_' + name), read(report / name))
        result['finished_utc'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        result['elapsed_seconds'] = time.monotonic() - started
        save(out / 'RESULT.json', encoded(result))
        save(out / 'INPUT_MANIFEST.json', read(HERE / 'INPUT_MANIFEST.json'))
        print(json.dumps({k: result.get(k) for k in ('status', 'artifact_sha256', 'commit', 'run_id', 'runner_os', 'error')}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
