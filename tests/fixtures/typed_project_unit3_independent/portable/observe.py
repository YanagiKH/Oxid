#!/usr/bin/env python3
"""Guarded portable entrance to the byte-identical reviewed source-only observer."""
from common import *
import argparse,subprocess

def main():
    p=argparse.ArgumentParser()
    for flag in ('python','prepared-manifest','materialization','build-receipt','case','profile','output'):p.add_argument('--'+flag,required=True)
    p.add_argument('--probe-timeout',type=float,default=30);p.add_argument('--controller-timeout',type=float,default=240)
    a=p.parse_args();require(a.profile in ('debug','release'),'invalid profile');require(0<a.probe_timeout<=30 and 0<a.controller_timeout<=300,'bounded observation timeouts required')
    prepared_path=pathlib.Path(a.prepared_manifest).resolve();prepared=read_json(prepared_path);require(prepared['schema']==SCHEMA+'-prepared-source','wrong prepared manifest')
    adapter=prepared_path.parent/relative(prepared['adapter_directory']);check_manifest(adapter,prepared['adapter_files'],exact=True)
    build=read_json(a.build_receipt);require(build.get('portable_schema')==SCHEMA+'-build'and build['status']=='built'and build['profile']==a.profile,'wrong portable build receipt');verify(prepared_path,build['prepared_manifest']);verify(build['binary']['path'],build['binary'])
    material_path=pathlib.Path(a.materialization).resolve();material=read_json(material_path);require(material['schema']==SCHEMA+'-materialized'and material['status']=='materialized','wrong materialization receipt')
    source_root=material_path.parent;require(str(source_root)==material['source_root'],'materialization moved/rebound');check_manifest(source_root,material['members']);requests=source_root/'requests.jsonl';verify(requests,material['requests'])
    require({p.relative_to(source_root).as_posix()for p in source_root.rglob('*.ox')if p.is_file()}=={x['path']for x in material['members']},'materialized source membership changed')
    python=pathlib.Path(a.python).resolve();require(python.is_file()and os.access(python,os.X_OK),'Python tool missing')
    # Check effective assertions in the selected interpreter before invoking the
    # legacy frozen script, which intentionally has not been edited/retargeted.
    probe=[str(python),'-B','-c','import sys; print(int(__debug__), sys.flags.optimize); sys.exit(0 if __debug__ and sys.flags.optimize == 0 else 72)']
    output=pathlib.Path(a.output).resolve();require(not output.exists(),'fresh observation directory required')
    receipt_path=output.with_name(output.name+'.portable.json');require(not receipt_path.exists(),'portable receipt already exists')
    command=[str(python),'-B',str(adapter/'run.py'),'--requests',str(requests),'--case',a.case,'--build-receipt',str(pathlib.Path(a.build_receipt).resolve()),'--profile',a.profile,'--output',str(output)]
    wrapper_receipt={'schema':SCHEMA+'-observe','assertion_mode':assertion_mode(),'selected_python':bound_file(python),'selected_assertion_probe':{'argv':probe},'argv':command,'exit_code':None,'prepared_manifest':bound_file(prepared_path),'materialization':bound_file(material_path),'build_receipt':bound_file(a.build_receipt),'frozen_controller':bound_file(adapter/'run.py'),'frozen_normalizer':bound_file(adapter/'parse_debug.py'),'case':a.case,'profile':a.profile,'status':'portable-observer-failure','timeouts':{'probe':a.probe_timeout,'controller':a.controller_timeout}}
    environment=safe_env();stage='probe'
    try:
        proof=run_bounded(probe,env=environment,timeout=a.probe_timeout)
        wrapper_receipt['selected_assertion_probe'].update(exit_code=proof.returncode,stdout=proof.stdout,stderr=proof.stderr)
        require(proof.returncode==0 and proof.stdout.strip()=='1 0','selected Python assertions disabled')
        stage='controller';result=run_bounded(command,env=environment,timeout=a.controller_timeout)
        wrapper_receipt.update(exit_code=result.returncode,stdout=result.stdout,stderr=result.stderr)
        require(result.returncode==0,'controller returned nonzero')
        require(output.is_dir()and(output/'receipt.json').is_file(),'controller omitted observation receipt')
        inner=read_json(output/'receipt.json');wrapper_receipt['observation_receipt']=bound_file(output/'receipt.json');require(inner['status']=='observed','inner observation failed')
        wrapper_receipt['status']='observed'
    except Exception as error:
        wrapper_receipt.update(failed_stage=stage,error=f'{type(error).__name__}: {error}')
        if isinstance(error,subprocess.TimeoutExpired):wrapper_receipt.update(timeout_stdout=error.output,timeout_stderr=error.stderr)
    if(output/'receipt.json').is_file():wrapper_receipt['observation_receipt']=bound_file(output/'receipt.json')
    # The inner controller rejects an existing output. Failed pre-worker checks
    # create no output; a wrapper failure after worker admission gets its own
    # sibling receipt without ever replacing the inner receipt.
    receipt_path.parent.mkdir(parents=True,exist_ok=True);write_json(receipt_path,wrapper_receipt)
    print(json.dumps({'status':wrapper_receipt['status'],'receipt':str(receipt_path)}));return 0 if wrapper_receipt['status']=='observed'else 1
if __name__=='__main__':sys.exit(main())
