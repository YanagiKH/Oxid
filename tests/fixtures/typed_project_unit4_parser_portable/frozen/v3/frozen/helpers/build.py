#!/usr/bin/env python3
"""Build the exact overlay with the official sourced toolchain and bounded jobs."""
import argparse,hashlib,json,os,pathlib,subprocess
import sys
if sys.flags.optimize or not __debug__:
    raise SystemExit("OPTIMIZED_PYTHON_UNSUPPORTED: parser observer requires enabled admission checks")

def identity(p):
    p=pathlib.Path(p);b=p.read_bytes();return {'path':str(p.resolve()),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--overlay',type=pathlib.Path,required=True);ap.add_argument('--profile',choices=['debug','release'],required=True);ap.add_argument('--output',type=pathlib.Path,required=True);a=ap.parse_args()
    manifest=json.loads(a.overlay.read_text());source=pathlib.Path(manifest['source']);out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
    for row in manifest['files']:
        got=identity(source/row['path']);assert got['sha256']==row['sha256'] and got['bytes']==row['bytes'],row['path']
    rustc=pathlib.Path(subprocess.check_output(['which','rustc'],text=True).strip());cargo=pathlib.Path(subprocess.check_output(['which','cargo'],text=True).strip())
    version=subprocess.check_output([str(rustc),'--version','--verbose'],text=True);assert 'rustc 1.99.0 ' in version
    target='x86_64-unknown-linux-gnu'
    env=os.environ.copy();env.update(CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_TARGET_DIR=str(out/'target'))
    cmd=[str(cargo),'test','--offline','--locked','--no-run','--bin','oxid','--jobs','2','--target',target,'--message-format=json']+(['--release']if a.profile=='release'else[])
    with (out/'stdout.jsonl').open('w')as stdout,(out/'stderr.txt').open('w')as stderr:r=subprocess.run(cmd,cwd=source,env=env,stdout=stdout,stderr=stderr)
    bins=[]
    for line in (out/'stdout.jsonl').read_text().splitlines():
        row=json.loads(line)
        if row.get('reason')=='compiler-artifact' and row.get('executable') and row['profile']['test']:bins.append(pathlib.Path(row['executable']))
    receipt={'schema':'oxid-unit4-parser-build-v1','status':'built'if r.returncode==0 and len(bins)==1 else'build-failure','exit_code':r.returncode,'profile':a.profile,'target':target,'argv':cmd,'cwd':str(source),'rustc_version':version,'rustc':identity(rustc),'overlay_manifest':identity(a.overlay),'observer_source_sha256':manifest['observer_source_sha256'],'candidate_source_manifest_sha256':manifest['candidate_source_manifest_sha256'],'control':manifest['control'],'stdout':identity(out/'stdout.jsonl'),'stderr':identity(out/'stderr.txt'),'environment':{k:env[k]for k in ['CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']}}
    if receipt['status']=='built':receipt['binary']=identity(bins[0])
    (out/'build-receipt.json').write_text(json.dumps(receipt,sort_keys=True,indent=2)+'\n');print(json.dumps(receipt));return 0 if receipt['status']=='built'else 1
if __name__=='__main__':raise SystemExit(main())
