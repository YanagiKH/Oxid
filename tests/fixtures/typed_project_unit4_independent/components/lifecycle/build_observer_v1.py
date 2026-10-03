#!/usr/bin/env python3
import hashlib,json,os,shutil,subprocess,time
from pathlib import Path
OUT=Path(__file__).resolve().parent; ROOT=OUT.parent
sha=lambda b:hashlib.sha256(b).hexdigest()
manifest=OUT/'observer-source-manifest-v1.json'
source=OUT/'observer-source-v1'
env={**os.environ,'CARGO_HOME':str(ROOT/'toolchains/cargo'),'RUSTUP_HOME':str(ROOT/'toolchains/rustup'),
 'RUSTUP_TOOLCHAIN':'1.99.0','CARGO_INCREMENTAL':'0','CARGO_BUILD_JOBS':'2','CARGO_TARGET_DIR':str(OUT/'target-v1')}
env['PATH']=str(ROOT/'toolchains/cargo/bin')+':'+env['PATH']
def identity():
 for f in json.loads(manifest.read_text())['files']:
  b=(source/f['path']).read_bytes();assert len(b)==f['bytes'] and sha(b)==f['sha256'],f['path']
identity();bins=[]
(OUT/'binaries-v1').mkdir(exist_ok=False)
for profile in ('debug','release'):
 argv=['cargo','build','--bin','oxid','--locked','--offline']+(['--release'] if profile=='release' else [])
 start=time.time_ns();p=subprocess.run(argv,cwd=source,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 for name,b in [('stdout',p.stdout),('stderr',p.stderr)]:(OUT/f'observer-build-{profile}-v1.{name}').write_bytes(b)
 receipt={'argv':argv,'cwd':str(source),'started_ns':start,'completed_ns':time.time_ns(),'status':p.returncode,
  'source_manifest_sha256':sha(manifest.read_bytes()),'environment':{k:env[k] for k in ('CARGO_HOME','RUSTUP_HOME','RUSTUP_TOOLCHAIN','CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_TARGET_DIR')},
  'rustc':subprocess.check_output(['rustc','-Vv'],env=env).decode(),'stdout_sha256':sha(p.stdout),'stderr_sha256':sha(p.stderr)}
 (OUT/f'observer-build-{profile}-v1.json').write_text(json.dumps(receipt,indent=2)+'\n')
 print(profile,'build status',p.returncode,flush=True);assert p.returncode==0,p.stderr.decode()
 binary=OUT/'binaries-v1'/('oxid-'+profile);shutil.copy2(OUT/'target-v1'/profile/'oxid',binary)
 bins.append({'profile':profile,'path':str(binary),'bytes':binary.stat().st_size,'sha256':sha(binary.read_bytes())})
 identity()
(OUT/'binaries-v1.json').write_text(json.dumps({'observer_manifest_sha256':sha(manifest.read_bytes()),'binaries':bins},indent=2)+'\n')
print(json.dumps(bins,indent=2),flush=True)
