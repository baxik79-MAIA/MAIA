"""Internal M5 staging RC: exact-byte packaging/copy validation, never product update."""
from __future__ import annotations
import argparse,hashlib,io,json,os,re,stat,subprocess,sys,time,zipfile
from pathlib import Path,PurePosixPath,PureWindowsPath

SOURCE_SHA='8991d407f0790af77560e7d7792d0b9ff0bd6be830db040464235a355adfe145'
SOURCE_MANIFEST_SHA='62bed8019a1c105e0732d73c075d736b9630311d746880b65ad054d69d34bdb5'
M4_BATCH_SHA='dcd768dec25d1d971c074e93cd9b79ab4338ff1da6b0e9ff8f0160bfb22e1d80'
M4_EXPORT_SHA='7db6f5e98dec33d70c12d4562d9720f356df3722b3d260838097c43f08a6e940'
M5_HELPER_TEST_COUNT=23
FLAGS=('native_image_chain_proven','native_run_ready','apply_permitted','durability_proven')
PROFILE='UNSIGNED_DEV_STAGING_RC'
MAX_FILE=2*1024*1024;MAX_TOTAL=16*1024*1024;MAX_MEMBERS=96
SECRET=re.compile(rb'(?:sk-(?:ant-|proj-)?[A-Za-z0-9_-]{20,}|AIza[A-Za-z0-9_-]{30,}|github_pat_[A-Za-z0-9_]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----)')
class Refusal(RuntimeError):pass
def need(ok,message):
 if not ok:raise Refusal(message)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def canon(value):return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=True,allow_nan=False).encode('ascii')+b'\n'
def pairs(items):
 out={}
 for key,value in items:need(key not in out,'duplicate JSON key');out[key]=value
 return out
def parse(raw):return json.loads(raw.decode('utf-8'),object_pairs_hook=pairs,parse_constant=lambda x:(_ for _ in ()).throw(Refusal('nonfinite JSON')))
def safe(name):
 need(type(name)is str and name and not name.startswith('/')and '\\'not in name and ':'not in name,'archive path')
 need(all(x not in ('','.','..')and not x.endswith((' ','.'))and not any(ord(c)<32 or c in '<>"|?*'for c in x)for x in name.split('/')),'archive path component')
 p=PurePosixPath(name)
 reserved={'CON','PRN','AUX','NUL'}|{x+str(i)for x in ('COM','LPT')for i in range(1,10)}
 need(all(re.fullmatch('[A-Za-z0-9_.-]+',x)and x.split('.')[0].upper()not in reserved for x in p.parts),'nonportable/reserved member')
 need(not any(x.casefold()in ('.git','.venv','venv','node_modules','__pycache__')for x in p.parts),'forbidden directory')
 need(p.suffix.casefold()not in ('.pyc','.key','.pem','.pfx','.sqlite','.db')and not p.name.casefold().endswith(('-wal','-shm'))and p.name.casefold()!='.env','private or mutable payload')
 return name
def ordinary(path,directory=False):
 info=path.lstat();need(not stat.S_ISLNK(info.st_mode)and not getattr(info,'st_file_attributes',0)&0x400,'symlink/reparse')
 need(stat.S_ISDIR(info.st_mode)if directory else stat.S_ISREG(info.st_mode)and info.st_nlink==1,'ordinary type/link')
 return info
def entry(path):
 try:path.lstat();return True
 except FileNotFoundError:return False
def parents(path):
 for p in reversed((path,*path.parents)):ordinary(p,True)
def read(path,bound=MAX_FILE):
 path=Path(path).absolute();parents(path.parent);a=ordinary(path);need(a.st_size<=bound,'file size bound')
 with path.open('rb')as f:
  b=f.read(bound+1)
 z=ordinary(path);need((a.st_dev,a.st_ino,a.st_size,a.st_mtime_ns,a.st_ctime_ns)==(z.st_dev,z.st_ino,z.st_size,z.st_mtime_ns,z.st_ctime_ns)and len(b)==z.st_size,'file changed')
 return b
def write(path,raw):
 parents(path.parent)
 with path.open('xb')as f:f.write(raw);f.flush();os.fsync(f.fileno())
 need(read(path,max(MAX_FILE,len(raw)))==raw,'write readback')
def scan(name,raw):
 safe(name);need(not SECRET.search(raw),'credential pattern:'+name)
 try:raw.decode('utf-8')
 except UnicodeDecodeError:raise Refusal('nontext payload:'+name)
def source_files(root):
 root=Path(root).absolute();parents(root);raw=read(root/'SOURCE_MANIFEST.json');need(sha(raw)==SOURCE_MANIFEST_SHA,'source manifest pin')
 m=parse(raw);need(set(m)=={'schema','source_sha256','files'}and m['schema']==1 and m['source_sha256']==SOURCE_SHA and len(m['files'])==43,'source manifest shape')
 actual={}
 for p in root.rglob('*'):
  ordinary(p,p.is_dir())
  if p.is_file():actual[p.relative_to(root).as_posix()]=read(p)
 need(set(actual)==set(m['files'])|{'SOURCE_MANIFEST.json'},'source inventory')
 for n,pin in m['files'].items():need(len(actual[safe(n)])==pin['bytes']and sha(actual[n])==pin['sha256'],'source file pin:'+n)
 need(sha(canon({n:v['sha256']for n,v in m['files'].items()}).rstrip(b'\n'))==SOURCE_SHA,'source digest')
 return actual
def accepted_m4(raw):
 p=parse(raw);need(p.get('status')=='M4_SCOPED_QUALIFICATION_PUBLISHED'and p.get('source_sha256')==SOURCE_SHA,'accepted M4 publication required')
 need(all(p.get(k)is True for k in ('source_verified','originals_read_back','production_unchanged','milestone_m4_accepted'))and all(p.get(k)is False for k in FLAGS),'M4 publication scope')
 need(p.get('acceptance_scope')=='ISOLATED_COPIED_PS_CONTROLLER_WORKER_AND_MONITOR_UI_STAGING_ONLY','M4 scope')
 need(p.get('batch_result_sha256')==M4_BATCH_SHA and p.get('raw_export_sha256')==M4_EXPORT_SHA,'qualified M4 batch/export binding')
 return p
def build(source,publication,output,supplement_root=None):
 output=Path(output).absolute();parents(output.parent);need(not entry(output),'fresh artifact required')
 files={'payload/'+n:b for n,b in source_files(source).items()};pub=read(publication);accepted_m4(pub)
 extras=Path(supplement_root or Path(__file__).parent)
 for name in ('m5_rc.py','test_m5_rc.py','INSTALL_MIGRATE_RETURN.md','LIMITATIONS.md','CI_RECIPE.md'):
  local=extras/name if name.endswith('.py')or (extras/name).is_file()else extras.parent/'docs'/name
  files[('tools/'if name.endswith('.py')else 'docs/')+name]=read(local)
 files['evidence/M4_PUBLICATION_RESULT.json']=pub
 metadata={'schema':1,'profile':PROFILE,'release_candidate':'MAIA-M5-DEV-STAGING-RC1','source_sha256':SOURCE_SHA,'source_manifest_sha256':SOURCE_MANIFEST_SHA,'m4_publication_sha256':sha(pub),'signature_status':'UNSIGNED_DEVELOPMENT_INPUT_NOT_DEPLOYMENT_ARTIFACT','installed_update_permitted':False,'deployment_distribution_permitted':False,'schema_migration_supported':False,'effect_execution_authority':False,'native_m4_evidence_reused_by_exact_source_identity':True,**{k:False for k in FLAGS}}
 files['RC_METADATA.json']=canon(metadata)
 for n,b in files.items():scan(n,b)
 need(len(files)<MAX_MEMBERS and sum(map(len,files.values()))<MAX_TOTAL,'artifact bound')
 files['RC_MANIFEST.json']=canon({'schema':1,'profile':PROFILE,'files':{n:{'bytes':len(b),'sha256':sha(b)}for n,b in sorted(files.items())}})
 stream=io.BytesIO()
 with zipfile.ZipFile(stream,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9)as z:
  for n,b in sorted(files.items()):
   info=zipfile.ZipInfo(n,(1980,1,1,0,0,0));info.create_system=3;info.external_attr=0o100644<<16;info.compress_type=zipfile.ZIP_DEFLATED;z.writestr(info,b,compresslevel=9)
 raw=stream.getvalue();verify_bytes(raw,expected_m4_publication=pub);write(output,raw)
 return {'status':'INTERNAL_STAGING_RC_BUILT','path':str(output),'bytes':len(raw),'sha256':sha(raw),'members':len(files),'profile':PROFILE,'signed':False}
def verify_bytes(raw,expected_m4_publication=None):
 need(len(raw)<MAX_TOTAL,'archive byte bound')
 with zipfile.ZipFile(io.BytesIO(raw))as z:
  entries=z.infolist();names=[safe(e.filename)for e in entries]
  need(1<=len(entries)<=MAX_MEMBERS and len(names)==len(set(names)),'entry count/duplicate')
  need(len({n.casefold()for n in names})==len(names),'case-alias members')
  lower={n.casefold()for n in names};need(not any('/'.join(n.split('/')[:i]).casefold()in lower for n in names for i in range(1,len(n.split('/')))),'file/directory alias')
  need(all(not e.is_dir()and stat.S_IFMT(e.external_attr>>16)in (0,stat.S_IFREG)and not e.flag_bits&1 and e.file_size<=MAX_FILE for e in entries),'entry type/encryption/size')
  need(sum(e.file_size for e in entries)<MAX_TOTAL,'expanded byte bound');files={n:z.read(n)for n in names}
 for n,b in files.items():scan(n,b)
 need('RC_MANIFEST.json'in files and 'RC_METADATA.json'in files,'RC descriptors missing');manifest=parse(files['RC_MANIFEST.json']);need(set(manifest)=={'schema','profile','files'}and manifest['schema']==1 and manifest['profile']==PROFILE,'RC manifest schema')
 need(set(manifest['files'])==set(files)-{'RC_MANIFEST.json'},'RC payload set')
 for n,p in manifest['files'].items():need(set(p)=={'bytes','sha256'}and type(p['bytes'])is int and len(files[n])==p['bytes']and sha(files[n])==p['sha256'],'RC payload hash:'+n)
 m=parse(files['RC_METADATA.json']);need(m.get('profile')==PROFILE and m.get('source_sha256')==SOURCE_SHA and m.get('source_manifest_sha256')==SOURCE_MANIFEST_SHA,'RC source/profile')
 need(m.get('signature_status')=='UNSIGNED_DEVELOPMENT_INPUT_NOT_DEPLOYMENT_ARTIFACT'and all(m.get(k)is False for k in (*FLAGS,'installed_update_permitted','deployment_distribution_permitted','schema_migration_supported','effect_execution_authority')),'RC authority escalation')
 pub=files.get('evidence/M4_PUBLICATION_RESULT.json');need(pub is not None and sha(pub)==m['m4_publication_sha256'],'RC M4 receipt pin');accepted_m4(pub)
 if expected_m4_publication is not None:need(type(expected_m4_publication)is bytes and pub==expected_m4_publication,'embedded M4 publication differs from prechecked bytes')
 sm=files.get('payload/SOURCE_MANIFEST.json');need(sm is not None and sha(sm)==SOURCE_MANIFEST_SHA,'RC source manifest');s=parse(sm)
 need(s.get('source_sha256')==SOURCE_SHA and len(s.get('files',{}))==43,'RC source map')
 need({n for n in files if n.startswith('payload/')}=={'payload/'+n for n in s['files']}|{'payload/SOURCE_MANIFEST.json'},'RC source inventory')
 fixed={'tools/m5_rc.py','tools/test_m5_rc.py','docs/INSTALL_MIGRATE_RETURN.md','docs/LIMITATIONS.md','docs/CI_RECIPE.md','evidence/M4_PUBLICATION_RESULT.json','RC_METADATA.json','RC_MANIFEST.json'}
 need(set(files)=={'payload/'+n for n in s['files']}|{'payload/SOURCE_MANIFEST.json'}|fixed,'exact RC member list')
 for n,p in s['files'].items():b=files['payload/'+safe(n)];need(len(b)==p['bytes']and sha(b)==p['sha256'],'RC source bytes:'+n)
 return files
def tree_bytes(root):
 parents(root);out={};directories=set()
 for p in root.rglob('*'):
  ordinary(p,p.is_dir())
  if p.is_file():out[p.relative_to(root).as_posix()]=read(p)
  else:directories.add(p.relative_to(root).as_posix())
 expected={'/'.join(n.split('/')[:i])for n in out for i in range(1,len(n.split('/')))}
 need(directories==expected,'foreign/empty candidate directory')
 return out
def windows_scope(work,report):
 w,r=PureWindowsPath(str(work)),PureWindowsPath(str(report));need('..'not in w.parts and '..'not in r.parts,'M5 scope dotdot')
 need(w==PureWindowsPath('C:/MAIA/worktrees/M5_RC_20261109')and r.parent==PureWindowsPath('C:/MAIA/reports/M5_RC_20261109'),'fixed Windows M5 scope');safe(r.name)
def fixed_scope(work,report):
 need('..'not in Path(work).parts and '..'not in Path(report).parts,'scope dotdot')
 if os.name=='nt':windows_scope(work,report)
 work=Path(work).absolute();report=Path(report).absolute();parents(work);parents(report.parent)
 need(work!=report and not work.is_relative_to(report)and not report.is_relative_to(work),'work/report overlap')
 return work,report
def preflight(archive,work,report,mode='fresh-staging-copy',store='none'):
 work,report=fixed_scope(work,report);need(mode=='fresh-staging-copy'and store=='none','production/schema/store migration unsupported')
 need(not entry(work/'candidate')and not entry(report),'fresh staging target/report required');raw=read(archive,MAX_TOTAL);files=verify_bytes(raw)
 return work,report,raw,files
def stage(archive,work,report):
 work,report,raw,files=preflight(archive,work,report);report.mkdir();target=work/'candidate';result={'status':'COPY_INCOMPLETE_NO_REPLAY','profile':PROFILE,'archive_sha256':sha(raw),'archive_bytes':len(raw),'target':str(target),'report_root':str(report),'production_modified':False,'schema_modified':False,'effect_execution_authority':False,**{k:False for k in FLAGS}}
 write(report/'ATTEMPT.json',canon({'operation':'FRESH_STAGING_COPY','no_replay':True,'archive_sha256':sha(raw)}))
 try:
  write(report/'PREIMAGE.json',canon({'target':str(target),'target_existed':False,'production_update':False,'schema_migration':False}))
  write(report/'RC_BACKUP.zip',raw)
  need(read(archive,MAX_TOTAL)==raw,'archive drift before copy');target.mkdir()
  for n,b in sorted(files.items()):p=target/n;p.parent.mkdir(parents=True,exist_ok=True);parents(p.parent);write(p,b)
  need(tree_bytes(target)==files,'fresh-copy readback');need(read(report/'RC_BACKUP.zip',MAX_TOTAL)==raw and read(archive,MAX_TOTAL)==raw,'backup/archive drift')
  result.update(status='FRESH_STAGING_COPY_VERIFIED',copied_members=len(files),postimage={n:{'bytes':len(b),'sha256':sha(b)}for n,b in sorted(files.items())},backup_sha256=sha(raw))
 except BaseException as exc:result['error']=type(exc).__name__+':'+str(exc)
 write(report/'COPY_RESULT.json',canon(result));need(result['status']=='FRESH_STAGING_COPY_VERIFIED','copy incomplete; preserve namespace')
 return result
def copied_snapshot(work,report):
 copy_raw=read(report/'COPY_RESULT.json');copy=parse(copy_raw);raw=read(report/'RC_BACKUP.zip',MAX_TOTAL);files=verify_bytes(raw)
 need(copy.get('status')=='FRESH_STAGING_COPY_VERIFIED'and copy.get('profile')==PROFILE and copy.get('target')==str(work/'candidate')and copy.get('report_root')==str(report),'known copy receipt binding')
 need(type(copy.get('archive_bytes'))is int and copy['archive_bytes']==len(raw)and sha(raw)==copy.get('archive_sha256')==copy.get('backup_sha256'),'copy/archive/backup identity')
 need(copy.get('postimage')=={n:{'bytes':len(b),'sha256':sha(b)}for n,b in sorted(files.items())},'copy postimage binding')
 need(all(copy.get(k)is False for k in (*FLAGS,'production_modified','schema_modified','effect_execution_authority')),'copy scope')
 need(tree_bytes(work/'candidate')==files,'foreign/changed candidate: refuse operation');return copy_raw,raw,files
def return_candidate(work,report):
 work,report=fixed_scope(work,report);parents(report);copy_raw,raw,files=copied_snapshot(work,report)
 target=work/'candidate';retired=work/'retired_candidate';need(not entry(retired),'fresh retired target required')
 write(report/'RETURN_ATTEMPT.json',canon({'operation':'RETIRE_OWNED_COPY','copy_result_sha256':sha(copy_raw),'no_replay':True}));result={'status':'RETURN_INCOMPLETE_NO_REPLAY','production_modified':False,'schema_modified':False,'candidate_preserved':True}
 try:
  need(tree_bytes(target)==files,'return pre-rename drift');target.rename(retired);need(not target.exists()and tree_bytes(retired)==files,'return byte preservation')
  result.update(status='OWNED_CANDIDATE_RETIRED_BYTE_PRESERVED',retired_path=str(retired),archive_sha256=sha(raw),copy_result_sha256=sha(copy_raw),restored_target_absence=True)
 except BaseException as exc:result['error']=type(exc).__name__+':'+str(exc)
 write(report/'RETURN_RESULT.json',canon(result));need(result['status']=='OWNED_CANDIDATE_RETIRED_BYTE_PRESERVED','return incomplete; preserve namespace');return result
def runtime_record(path,label):
 p=Path(path);need(p.is_absolute(),'absolute runtime path');parents(p.parent);before=ordinary(p);need(before.st_size<256*1024*1024,'runtime bound');h=hashlib.sha256()
 with p.open('rb')as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 after=ordinary(p);need((before.st_dev,before.st_ino,before.st_size,before.st_mtime_ns)==(after.st_dev,after.st_ino,after.st_size,after.st_mtime_ns),'runtime file changed');digest=h.hexdigest()
 if os.name=='nt':
  fixed={'python':('C:/Users/PS/AppData/Local/Programs/Python/Python313/python.exe','85b71d8c6ec1905935f74be0c9869aae198d00e98f39df699ec66f9c5a84cecd'),'node':('C:/Program Files/nodejs/node.exe','3602f2bb1a10f2cbab4c36886218a33c1ab3db87290e73b033c46c77147d0237')}
  expected=fixed[label];need(str(p).casefold()==str(Path(expected[0])).casefold()and digest==expected[1],'fixed Windows runtime path/hash')
 return {'path':str(p),'bytes':after.st_size,'sha256':digest,'scope':'FILE_BYTES_ONLY_NOT_EXECUTED_IMAGE_PROOF'}
def suite_semantics(label,code,stdout,stderr):
 stdout=stdout.replace(b'\r\n',b'\n');stderr=stderr.replace(b'\r\n',b'\n')
 if code!=0:return {'passed':False,'reason':'nonzero exit','tests':None}
 if label=='UI_CONTROLLED_DOM':
  count=re.search(rb'^# tests (\d+)\s*$',stdout,re.M);passed=bool(count and int(count[1])==1 and re.search(rb'^# pass 1\s*$',stdout,re.M)and all(re.search(rb'^# '+k+rb' 0\s*$',stdout,re.M)for k in (b'fail',b'cancelled',b'skipped',b'todo')))
  return {'passed':passed,'tests':int(count[1])if count else None,'scope':'CONTROLLED_DOM_NOT_BROWSER_RENDER'}
 found=re.search(rb'\nRan (\d+) tests? in [^\n]+\n\nOK\s*\Z',stderr)
 expected={'M5_HELPERS':M5_HELPER_TEST_COUNT,'M3_PURE':64,'M4_PURE':98}[label]
 return {'passed':bool(found and int(found[1])==expected),'tests':int(found[1])if found else None,'expected_tests':expected,'zero_skips_required':True}
def validate(work,report,python_path,node_path=None):
 work,report=fixed_scope(work,report);parents(report);copy_raw,raw,files=copied_snapshot(work,report);target=work/'candidate'
 runtimes={'python':runtime_record(python_path,'python')}
 if node_path:runtimes['node']=runtime_record(node_path,'node')
 write(report/'VALIDATION_ATTEMPT.json',canon({'no_replay':True,'archive_sha256':sha(raw),'scope':'PURE_CONTROLS_ONLY_NO_M4_EFFECT_REPLAY','runtime_files':runtimes,'hard_budget_seconds':150}))
 commands=[('M5_HELPERS',[python_path,'-I','-S','-B','-m','unittest','discover','-s','tools','-p','test_m5*.py','-v']),('M3_PURE',[python_path,'-I','-S','-B','-m','unittest','discover','-s','payload/baseline_m3','-p','test_m3*.py','-v']),('M4_PURE',[python_path,'-I','-S','-B','-m','unittest','discover','-s','payload','-p','test_m4*.py','-v'])]
 if node_path:commands.append(('UI_CONTROLLED_DOM',[node_path,'--test','--test-reporter=tap','payload/test_m4_ui.js']))
 env={k:v for k,v in os.environ.items()if k.upper()in ('SYSTEMROOT','WINDIR','TEMP','TMP')};env['PYTHONDONTWRITEBYTECODE']='1';results=[];started=time.monotonic();error=None;unchanged=False
 try:
  for label,command in commands:
   remaining=150-(time.monotonic()-started);need(remaining>0,'validation total deadline')
   cp=subprocess.run(command,cwd=target,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=min(60,remaining),check=False)
   write(report/(label+'.stdout.log'),cp.stdout);write(report/(label+'.stderr.log'),cp.stderr);sem=suite_semantics(label,cp.returncode,cp.stdout,cp.stderr);results.append({'suite':label,'command':command,'exit_code':cp.returncode,'stdout_sha256':sha(cp.stdout),'stderr_sha256':sha(cp.stderr),'semantic':sem})
   if not sem['passed']:break
  need(tree_bytes(target)==files,'fresh copy changed by controls');unchanged=True
  need(all(runtime_record(v['path'],k)==v for k,v in runtimes.items()),'runtime drift across controls')
 except BaseException as exc:error=type(exc).__name__+':'+str(exc)
 out={'status':'FRESH_RC_PURE_CONTROLS_PASS'if not error and unchanged and len(results)==len(commands)and all(r['semantic']['passed']for r in results)else 'FRESH_RC_PURE_CONTROLS_FAILED','profile':PROFILE,'platform':sys.platform,'python_version':sys.version,'runtime_files':runtimes,'archive_sha256':sha(raw),'results':results,'candidate_bytes_unchanged':unchanged,'error':error,'elapsed_seconds':time.monotonic()-started,'remote_ci_job_observed':False,'m4_native_effect_batch_replayed':False,'production_modified':False,'child_custody_scope':'FINITE_KNOWN_PURE_TEST_CHILD_ONLY_NO_NATIVE_TREE_QUALIFICATION',**{k:False for k in FLAGS}};write(report/'VALIDATION_RESULT.json',canon(out));need(out['status']=='FRESH_RC_PURE_CONTROLS_PASS','fresh RC controls failed');return out
def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);s=p.add_subparsers(dest='command',required=True)
 b=s.add_parser('build');b.add_argument('--source',required=True);b.add_argument('--m4-publication',required=True);b.add_argument('--output',required=True)
 v=s.add_parser('verify');v.add_argument('--archive',required=True)
 for c in ('preflight','stage','return','validate'):
  q=s.add_parser(c);q.add_argument('--work-root',required=True);q.add_argument('--report-root',required=True)
  if c in ('preflight','stage'):q.add_argument('--archive',required=True)
  if c=='preflight':q.add_argument('--mode',default='fresh-staging-copy');q.add_argument('--store',default='none')
  if c=='validate':q.add_argument('--python',required=True);q.add_argument('--node')
 a=p.parse_args(argv)
 try:
  if a.command=='build':r=build(a.source,a.m4_publication,a.output)
  elif a.command=='verify':raw=read(a.archive,MAX_TOTAL);f=verify_bytes(raw);r={'status':'INTERNAL_RC_VERIFIED','sha256':sha(raw),'bytes':len(raw),'members':len(f),'signature_verified':False,'profile':PROFILE}
  elif a.command=='preflight':w,rp,raw,f=preflight(a.archive,a.work_root,a.report_root,a.mode,a.store);r={'status':'FRESH_STAGING_PREFLIGHT_PASS','sha256':sha(raw),'members':len(f),'target_absent':True,'production_update_permitted':False,'schema_migration_supported':False}
  elif a.command=='stage':r=stage(a.archive,a.work_root,a.report_root)
  elif a.command=='return':r=return_candidate(a.work_root,a.report_root)
  else:r=validate(a.work_root,a.report_root,a.python,a.node)
  print(json.dumps(r));return 0
 except Exception as exc:print(json.dumps({'status':'REFUSED_OR_INCOMPLETE_PRESERVE_NAMESPACE','error':type(exc).__name__+':'+str(exc),'production_update_permitted':False}));return 2
if __name__=='__main__':raise SystemExit(main())
