#!/usr/bin/env python3
from pathlib import Path
import subprocess,sys
root=Path(__file__).resolve().parents[1]
cmds=[[sys.executable,str(root/'tools/validate_spec.py')],[sys.executable,str(root/'tools/generate_document_fragments.py'),'--check']]
cmds.append([sys.executable, str(root/'tools/generate_rust_domain.py'), '--check'])
log=[]
for cmd in cmds:
    r=subprocess.run(cmd,capture_output=True,text=True)
    log.append(r.stdout)
    if r.stderr: log.append(r.stderr)
    if r.returncode:
        report=''.join(log)+f'\nMAIA SPEC GUARD: FAIL ({r.returncode})\n'
        (root/'generated/spec_guard_report.txt').write_text(report,encoding='utf-8',newline='\n')
        print(report,end='')
        sys.exit(r.returncode)
log.append('MAIA SPEC GUARD: PASS\n')
report=''.join(log)
(root/'generated/spec_guard_report.txt').write_text(report,encoding='utf-8',newline='\n')
print(report,end='')
