#!/usr/bin/env python3
"""Source-only native/driver invoker. This program never reads expected data."""
from pathlib import Path
import argparse,hashlib,json,os,platform,shutil,subprocess,time
R=Path(__file__).resolve().parent; ROOT=R.parent
RUST=ROOT/'toolchains/rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin/rustc'
LLVM=ROOT/'toolchains/llvm19/usr/lib/llvm-19/bin'
ENV={**os.environ,'CARGO_HOME':str(ROOT/'toolchains/cargo'),'RUSTUP_HOME':str(ROOT/'toolchains/rustup'),'CARGO_INCREMENTAL':'0','CARGO_BUILD_JOBS':'2','CARGO_MANIFEST_DIR':str(R/'source-v2'),'OXID_LLVM_BIN':str(LLVM),'LD_LIBRARY_PATH':str(ROOT/'toolchains/llvm19/usr/lib/x86_64-linux-gnu')}
MINIMAL={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C'}
RECORDED_ENV_KEYS=('CARGO_HOME','RUSTUP_HOME','CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_MANIFEST_DIR','OXID_LLVM_BIN','LD_LIBRARY_PATH','OXID_UNIT3_NATIVE_RECEIPT','OXID_UNIT3_NATIVE_FIXTURE_ROOT','OXID_UNIT3_NATIVE_FUEL','OXID_UNIT3_PUBLIC','OXID_UNIT3_SPY_LOG')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,obj):path.write_text(json.dumps(obj,indent=2)+'\n')
def append(path,obj):
 with path.open('a')as f:f.write(json.dumps(obj)+'\n')
def execute(argv,cwd,env,where,timeout):
 started=time.time(); row={'argv':[str(v)for v in argv],'cwd':str(cwd),'timeout_seconds':timeout}
 try:
  p=subprocess.run(argv,cwd=cwd,env=env,capture_output=True,timeout=timeout);out,err=p.stdout,p.stderr;row['status']=p.returncode
 except subprocess.TimeoutExpired as e:out,err=e.stdout or b'',e.stderr or b'';row.update(status=None,qualification_failure='timeout')
 except OSError as e:out,err=b'',str(e).encode();row.update(status=None,qualification_failure='spawn-error')
 row['seconds']=time.time()-started
 for name,data in [('stdout',out),('stderr',err)]:
  (where/name).write_bytes(data);row[name]=data.decode('utf-8',errors='replace');row[name+'_sha256']=hashlib.sha256(data).hexdigest()
 write(where/'process.json',row);return row

def bindings():
 return {name:sha(R/name)for name in ['overlay-manifest-v2.json','main.rs','run.py','request-input-manifest.json']} | {'core_manifest':sha(ROOT/'typed-project-unit3-evidence/source-inputs-core-v1.json'),'rustc':sha(RUST),'runtime':sha(R/'source-v2/native/typed_preview.c')}

def verify_binary(profile):
 receipt=json.loads((R/('build-'+profile)/'verified-build.json').read_text())
 assert receipt['profile']==profile and receipt['status']==0
 assert receipt['bindings']==bindings(),'build identity changed'
 assert receipt['binary_sha256']==sha(R/('adapter-'+profile)),'binary identity changed'
 return receipt

def verify_requests():
 manifest=json.loads((R/'request-input-manifest.json').read_text())
 assert sha(ROOT/'typed-project-unit3-oracles/supplements/native-diagnostics-v1/supplement-manifest.json')==manifest['native_supplement_sha256']
 for name,digest in manifest['request_files'].items():assert sha(R/name)==digest,name
 original_path=ROOT/'typed-project-unit3-oracles/requests.jsonl'
 assert sha(original_path)=='f405bf46d8945027cd1d79d3c125692b4c00b8fe57b97c0d55c68b83b9795113'
 original=[json.loads(l)for l in original_path.read_text().splitlines()];by_id={x['id']:x for x in original}
 native=json.loads((R/'native-requests.json').read_text())
 assert native==[x for x in original if x.get('native')]
 driver=json.loads((R/'driver-requests.proposed.json').read_text())
 assert [x['id']for x in driver]==manifest['driver_ids']
 for x in driver:
  expected=dict(by_id[x['id']],driver_operations=['check','run','compile'],driver_formats=['text','json'])
  assert x==expected,'driver input changed or unexpected keys'
 fuel=json.loads((R/'fuel-requests.frozen.json').read_text())
 safe=ROOT/'typed-project-unit3-oracles/supplements/native-diagnostics-v1/fuel-requests.jsonl'
 assert sha(safe)=='dc80bda11a7e23731a0f82a5b0adcf249538a4bad8ff00fa82da6ad61aa5894b'
 assert fuel==[json.loads(l)for l in safe.read_text().splitlines()]
 for x in fuel:
  allowed={**by_id[x['id']],'fuel_budgets':x['fuel_budgets']};assert x==allowed
  assert all(type(n)is int and 0<=n<=1000000 for n in x['fuel_budgets'])
 clobber=json.loads((R/'no-clobber-requests.json').read_text())
 assert clobber==manifest['no_clobber_controls']
 assert len(clobber['requests'])==10
 return manifest

def verify_inputs():
 verify_requests()
 manifest=ROOT/'typed-project-unit3-evidence/source-inputs-core-v1.json'
 assert sha(manifest)=='53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
 for x in json.loads(manifest.read_text())['files']:assert sha(ROOT/'oxid-typed-project-execution'/x['path'])==x['sha256']
 overlay=json.loads((R/'overlay-manifest-v2.json').read_text())
 assert sha(R/'overlay-v2.patch')==overlay['overlay_sha256'];assert sha(R/'main.rs')==overlay['main_sha256']
 for x in overlay['files']:assert sha(R/'source-v2'/x['path'])==x['sha256']
 assert sha(ROOT/'typed-project-unit3-oracles/pre-execution-manifest.json')=='b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba'

def build(profiles):
 verify_inputs(); version_root=R/'tool-versions';version_root.mkdir(exist_ok=False);versions=[]
 for name,path,extra in [('rustc',RUST,['--version','--verbose']),('clang',LLVM/'clang',['--version']),('opt',LLVM/'opt',['--version']),('lld',LLVM/'ld.lld',['--version'])]:
  loc=version_root/name;loc.mkdir();row=execute([path,*extra],R,ENV,loc,20);row.update(name=name,binary_sha256=sha(path));versions.append(row)
 write(R/'toolchain.json',{'tools':versions,'host':platform.uname()._asdict(),'runtime_sha256':sha(R/'source-v2/native/typed_preview.c')})
 assert all(x['status']==0 for x in versions)
 for profile in profiles:
  loc=R/('build-'+profile);loc.mkdir();binary=R/('adapter-'+profile)
  argv=[RUST,'--cfg','test','--edition=2021',R/'main.rs','-o',binary,'-C','debuginfo=0','-C','codegen-units=2','-C','opt-level='+('0'if profile=='debug'else'3'),'-C','debug-assertions='+('yes'if profile=='debug'else'no'),'-C','overflow-checks='+('yes'if profile=='debug'else'no')]
  row=execute(argv,R,ENV,loc,180);row['profile']=profile;row['bindings']=bindings();row['environment_overrides']={k:ENV[k]for k in RECORDED_ENV_KEYS if k in ENV}
  if binary.exists():row['binary_sha256']=sha(binary)
  append(R/'build-attempts.jsonl',row)
  if row['status']!=0:raise SystemExit('build failed; preserved attempt')
  write(loc/'verified-build.json',row)

def fixture_input(request):
 root=R/'fixtures'/request['id'];entry=root/request['entry']
 for sf in request['source_files']:assert sha(root/sf['path'])==sf['sha256']
 return root,entry

def invocation(request,profile,group,operation,fmt='json',fuel=None,noclobber=None,spy=False):
 build_receipt=verify_binary(profile)
 root,entry=fixture_input(request)
 key=request['id']+'--'+operation+'--'+fmt+(f'--fuel-{fuel}'if fuel is not None else'')+(f'--{noclobber}'if noclobber else'')
 loc=R/'receipts'/group/profile/key;loc.mkdir(parents=True,exist_ok=False)
 elf=loc/'program';output=elf
 if noclobber:
  target=loc/'preserved-target';target.write_bytes(b'Oxid Unit3 no-clobber sentinel\x00\xff\n');output=loc/'occupied-output'
  if noclobber=='file':output.write_bytes(target.read_bytes())
  else:output.symlink_to(target.name)
  before={'target_sha256':sha(target),'output_sha256':sha(output),'is_symlink':output.is_symlink(),'readlink':os.readlink(output)if output.is_symlink()else None}
 else:before=None
 argv=[R/('adapter-'+profile),'--edition=typed-preview','--message-format='+fmt,operation,str(entry)]
 if operation=='compile':argv.extend(['--backend=llvm','--target=x86_64-unknown-linux-gnu','--output',str(output)])
 env={**ENV,'OXID_UNIT3_NATIVE_RECEIPT':str(loc),'OXID_UNIT3_NATIVE_FIXTURE_ROOT':str(root)}
 env.pop('OXID_UNIT3_NATIVE_FUEL',None);env.pop('OXID_UNIT3_PUBLIC',None)
 if fuel is not None:env['OXID_UNIT3_NATIVE_FUEL']=str(fuel)
 if spy:env.update(OXID_LLVM_BIN=str(R/'spy-bin'),OXID_UNIT3_SPY_LOG=str(loc/'spy-calls'))
 record={'schema':1,'id':request['id'],'profile':profile,'group':group,'operation':operation,'format':fmt,'fuel':fuel,'fixture_root':str(root),'entry':str(entry),'output':str(output)if operation=='compile'else None,'source_files':request['source_files'],'binary_sha256':sha(R/('adapter-'+profile)),'verified_build_sha256':sha(R/('build-'+profile)/'verified-build.json'),'core_manifest_sha256':sha(ROOT/'typed-project-unit3-evidence/source-inputs-core-v1.json'),'overlay_manifest_sha256':sha(R/'overlay-manifest-v2.json'),'input_request_sha256':hashlib.sha256(json.dumps(request,sort_keys=True).encode()).hexdigest(),'environment_overrides':{k:env[k]for k in RECORDED_ENV_KEYS if k in env},'noclobber_before':before}
 write(loc/'invocation.json',record)
 record['compiler']=execute(argv,R,env,loc,60)
 hidden=root.with_name(root.name+'.unavailable')
 if hidden.exists():
  os.rename(hidden,root);record['qualification_failure']='adapter failed to restore fixture; runner restored';record['runner_restored_source']=True
 record['source_restored']=root.exists()and all(sha(root/sf['path'])==sf['sha256']for sf in request['source_files'])
 record['tool_receipt_exists']=(loc/'tools.jsonl').exists()
 record['tools']=[json.loads(x)for x in (loc/'tools.jsonl').read_text().splitlines()]if record['tool_receipt_exists']else None
 record['source_state']=[json.loads(x)for x in (loc/'source-state.jsonl').read_text().splitlines()]if(loc/'source-state.jsonl').exists()else None
 record['spy_calls']=(loc/'spy-calls').read_text()if(loc/'spy-calls').exists()else''
 if(loc/'actual.ll').exists():record['ir_sha256']=sha(loc/'actual.ll')
 if noclobber:
  record['noclobber_after']={'target_sha256':sha(target),'output_sha256':sha(output),'is_symlink':output.is_symlink(),'readlink':os.readlink(output)if output.is_symlink()else None}
 if not noclobber and operation=='compile'and elf.exists():
  record['artifact_sha256']=sha(elf);record['artifact_magic_hex']=elf.read_bytes()[:16].hex()
  if elf.read_bytes()[:4]==b'\x7fELF'and record['compiler']['status']==0:
   isolated=loc/'isolated';isolated.mkdir();shutil.copy2(elf,isolated/'program')
   os.rename(root,hidden)
   try:
    files=sorted(x.name for x in isolated.iterdir());absent=all(not(root/sf['path']).exists()for sf in request['source_files']);assert files==['program']and absent
    run_receipt=loc/'execution';run_receipt.mkdir();run=execute([isolated/'program'],isolated,MINIMAL,run_receipt,30)
    run.update(source_unavailable=True,cwd_before_files=files,environment=MINIMAL,elf_sha256=sha(isolated/'program'));record['execution']=run
   finally:os.rename(hidden,root)
 record['source_restored_after_execution']=root.exists()and all(sha(root/sf['path'])==sf['sha256']for sf in request['source_files'])
 write(loc/'receipt.json',record);append(R/'run-receipts.jsonl',{'path':str(loc/'receipt.json'),'id':request['id'],'profile':profile,'group':group,'operation':operation,'format':fmt,'fuel':fuel,'status':record['compiler']['status'],'elf_execution':bool(record.get('execution'))})
 print(group,profile,key,'status',record['compiler']['status'],'ELF',record.get('execution',{}).get('status'),flush=True)
 return record

def main():
 p=argparse.ArgumentParser();p.add_argument('stage',choices=['build','native','fuel','driver','noclobber']);p.add_argument('--profile',choices=['debug','release','both'],default='both');a=p.parse_args();profiles=['debug','release']if a.profile=='both'else[a.profile]
 if a.stage=='build':return build(profiles)
 verify_inputs()
 native=json.loads((R/'native-requests.json').read_text());driver=json.loads((R/'driver-requests.proposed.json').read_text())
 all_requests={x['id']:x for x in native+driver}
 for profile in profiles:
  if a.stage=='native':
   for x in native:invocation(x,profile,'native-default','compile')
  if a.stage=='driver':
   for x in driver:
    for operation in ['check','run','compile']:
     for fmt in ['text','json']:invocation(x,profile,'driver',operation,fmt)
  if a.stage=='fuel':
   for x in json.loads((R/'fuel-requests.frozen.json').read_text()):
    for fuel in x['fuel_budgets']:
     invocation(x,profile,'native-fuel','compile',fuel=fuel)
     invocation(x,profile,'reference-fuel','run','text',fuel=fuel)
  if a.stage=='noclobber':
   for control in json.loads((R/'no-clobber-requests.json').read_text())['requests']:
    kind='file'if control['output_kind']=='regular'else'symlink'
    invocation(all_requests[control['id']],profile,'no-clobber','compile',control['format'],noclobber=kind,spy=True)
if __name__=='__main__':main()
