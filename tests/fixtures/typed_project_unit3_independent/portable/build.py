#!/usr/bin/env python3
"""Portable explicit-tool observer build; no PATH/default toolchain assumption."""
from common import *
import argparse,subprocess,time,tomllib

def main():
    p=argparse.ArgumentParser()
    for flag in ('prepared-manifest','output','target-dir','rustc','cargo','rustdoc','cargo-home','cc','cxx','ar'):p.add_argument('--'+flag,required=True)
    p.add_argument('--profile',choices=('debug','release'),required=True);p.add_argument('--jobs',type=int,default=2)
    p.add_argument('--version-timeout',type=float,default=30);p.add_argument('--build-timeout',type=float,default=600);a=p.parse_args();require(1<=a.jobs<=2,'at most2 build jobs');require(0<a.version_timeout<=30 and 0<a.build_timeout<=900,'bounded tool timeouts required')
    manifest_path=pathlib.Path(a.prepared_manifest).resolve();prepared=read_json(manifest_path);require(prepared['schema']==SCHEMA+'-prepared-source'and prepared['status']=='prepared','wrong prepared source')
    source=manifest_path.parent/relative(prepared['source_directory']);adapter=manifest_path.parent/relative(prepared['adapter_directory']);check_manifest(source,prepared['files'],exact=True);check_manifest(adapter,prepared['adapter_files'],exact=True)
    tools={name:pathlib.Path(getattr(a,name)).resolve()for name in ('rustc','cargo','rustdoc','cc','cxx','ar')}
    for path in tools.values():require(path.is_file()and os.access(path,os.X_OK),'tool missing/not executable')
    target=pathlib.Path(a.target_dir).resolve();require(not target.is_relative_to(source),'target must be separate from immutable source');out=fresh(a.output)
    env=safe_env()
    removed=[]
    for key in list(env):
        if key in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CFLAGS','CXXFLAGS','ARFLAGS','CARGO_BUILD_RUSTC','HOST_CC','HOST_CXX','HOST_AR','TARGET_CC','TARGET_CXX','TARGET_AR')or key.startswith(('CARGO_PROFILE_','CARGO_TARGET_','CFLAGS_','CXXFLAGS_','CC_','CXX_','AR_')):
            removed.append(key);env.pop(key)
    env.update({'RUSTC':str(tools['rustc']),'RUSTDOC':str(tools['rustdoc']),'CC':str(tools['cc']),'CXX':str(tools['cxx']),'AR':str(tools['ar']),'CARGO_HOME':str(pathlib.Path(a.cargo_home).resolve()),'CARGO_INCREMENTAL':'0','CARGO_BUILD_JOBS':str(a.jobs),'CARGO_TARGET_DIR':str(target),'CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0'})
    env.update({'CARGO_ENCODED_RUSTFLAGS':'','RUSTC_WRAPPER':'','RUSTC_WORKSPACE_WRAPPER':''})
    for profile in ('DEV','TEST','RELEASE'):
        env.update({f'CARGO_PROFILE_{profile}_OPT_LEVEL':'3'if profile=='RELEASE'else'0',f'CARGO_PROFILE_{profile}_DEBUG':'0',f'CARGO_PROFILE_{profile}_DEBUG_ASSERTIONS':'false'if profile=='RELEASE'else'true',f'CARGO_PROFILE_{profile}_OVERFLOW_CHECKS':'false'if profile=='RELEASE'else'true'})
    command=[str(tools['cargo']),'test','--offline','--no-run','--bin','oxid','--jobs',str(a.jobs),'--message-format=json']+(['--release']if a.profile=='release'else[])
    start=time.monotonic()
    receipt={'schema':1,'portable_schema':SCHEMA+'-build','assertion_mode':assertion_mode(),'profile':a.profile,'status':'build-failure','exit_code':None,'argv':command,'cwd':str(source),'toolchain':{k:bound_file(v)for k,v in tools.items()},'overlay_manifest':bound_file(manifest_path),'prepared_manifest':bound_file(manifest_path),'frozen_observer_root':str(adapter),'environment':{k:env[k]for k in ('CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','PYTHONOPTIMIZE')+tuple(f'CARGO_PROFILE_{p}_{s}'for p in ('DEV','TEST','RELEASE')for s in ('OPT_LEVEL','DEBUG','DEBUG_ASSERTIONS','OVERFLOW_CHECKS'))},'removed_build_override_names':sorted(removed),'timeouts':{'version':a.version_timeout,'build':a.build_timeout},'version_argv':[str(tools['rustc']),'--version','--verbose']}
    stage='version'
    try:
        probe=run_bounded(receipt['version_argv'],env=env,timeout=a.version_timeout)
        (out/'version.stdout').write_text(probe.stdout);(out/'version.stderr').write_text(probe.stderr);receipt['version_exit_code']=probe.returncode
        require(probe.returncode==0 and probe.stdout.startswith('rustc 1.99.0 '),'exact successful Rust1.99.0 probe required');receipt['rustc']=probe.stdout
        stage='cargo'
        with(out/'compiler.stdout.jsonl').open('w')as stdout,(out/'compiler.stderr').open('w')as stderr:
            result=run_bounded(command,cwd=source,env=env,stdout=stdout,stderr=stderr,timeout=a.build_timeout)
        receipt['exit_code']=result.returncode;require(result.returncode==0,'Cargo returned nonzero')
        require((out/'compiler.stdout.jsonl').stat().st_size<=64*1024*1024,'Cargo JSON log limit exceeded')
        rows=[json.loads(line)for line in(out/'compiler.stdout.jsonl').read_text().splitlines()]
        finished=[row for row in rows if row.get('reason')=='build-finished'];require(len(finished)==1 and finished[0].get('success')is True and rows[-1]==finished[0],'missing/duplicate/unsuccessful Cargo terminal')
        artifacts=[row for row in rows if row.get('reason')=='compiler-artifact'and row.get('executable')and row.get('profile',{}).get('test')is True];require(len(artifacts)==1,'exactly one test executable required')
        artifact=artifacts[0];package=tomllib.loads((source/'Cargo.toml').read_text())['package'];expected_package=f"path+{source.as_uri()}#{package['name']}@{package['version']}"
        require(artifact['package_id']==expected_package,'artifact package/source mismatch')
        target_info=artifact['target'];require(target_info['name']=='oxid'and target_info['kind']==['bin']and target_info['crate_types']==['bin']and pathlib.Path(target_info['src_path']).resolve()==source/'src/cli.rs'and target_info.get('test')is True,'artifact target/source mismatch')
        expected_profile={'opt_level':'0'if a.profile=='debug'else'3','debuginfo':0,'debug_assertions':a.profile=='debug','overflow_checks':a.profile=='debug','test':True}
        require(artifact['profile']==expected_profile and artifact.get('features')==[],'artifact profile/features mismatch')
        binary=pathlib.Path(artifact['executable']).resolve();require(binary.parent==target/('debug'if a.profile=='debug'else'release')/'deps'and binary.is_file()and os.access(binary,os.X_OK),'artifact executable location/access mismatch')
        check_manifest(source,prepared['files'],exact=True)
        receipt.update(status='built',binary=bound_file(binary),cargo_terminal=finished[0],cargo_artifact=artifact)
    except Exception as error:
        receipt.update(error=f'{type(error).__name__}: {error}',failed_stage=stage)
        if isinstance(error,subprocess.TimeoutExpired):
            for stream,value in (('stdout',error.output),('stderr',error.stderr)):
                if value is not None:(out/f'{stage}.timeout.{stream}').write_text(value)
    receipt['seconds']=time.monotonic()-start
    for name in ('version.stdout','version.stderr','compiler.stdout.jsonl','compiler.stderr','version.timeout.stdout','version.timeout.stderr','cargo.timeout.stdout','cargo.timeout.stderr'):
        if(out/name).is_file():receipt[name]=bound_file(out/name)
    write_json(out/'build.json',receipt);print(json.dumps({'status':receipt['status'],'receipt':str(out/'build.json')}));return 0 if receipt['status']=='built'else 1
if __name__=='__main__':sys.exit(main())
