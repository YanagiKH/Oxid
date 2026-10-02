#!/usr/bin/env python3
"""Build one explicit reviewed source overlay/profile with bounded concurrency."""
import argparse,hashlib,json,os,pathlib,subprocess,sys
HERE=pathlib.Path(__file__).resolve().parent
def identity(path):
    data=path.read_bytes();return {'path':str(path),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
def main():
    p=argparse.ArgumentParser();p.add_argument('--manifest',type=pathlib.Path,required=True);p.add_argument('--profile',choices=('debug','release'),required=True);a=p.parse_args()
    m=a.manifest.resolve();manifest=json.loads(m.read_text());source=pathlib.Path(manifest['source'])
    for item in manifest['files']:
        got=identity(source/item['path']);assert got['bytes']==item['bytes']and got['sha256']==item['sha256'],item['path']
    toolchain=HERE.parent/'toolchains/rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin'
    rustc=toolchain/'rustc';cargo=toolchain/'cargo';rustdoc=toolchain/'rustdoc'
    env=os.environ.copy();env.update(CARGO_HOME=str(HERE.parent/'toolchains/cargo'),RUSTUP_HOME=str(HERE.parent/'toolchains/rustup'),RUSTUP_TOOLCHAIN='1.99.0',RUSTC=str(rustc),RUSTDOC=str(rustdoc),CARGO_INCREMENTAL='0',CARGO_BUILD_JOBS='2',CARGO_TARGET_DIR=str(HERE/'target'),CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
    version=subprocess.run([str(rustc),'--version','--verbose'],env=env,text=True,capture_output=True,check=True).stdout;assert 'rustc 1.99.0 'in version
    name=f"build-{manifest['revision']}-{a.profile}";stdout=HERE/(name+'.stdout.jsonl');stderr=HERE/(name+'.stderr')
    assert not stdout.exists()and not stderr.exists(),'preserve prior build receipts'
    command=[str(cargo),'test','--offline','--no-run','--bin','oxid','--jobs','2','--message-format=json']+(['--release']if a.profile=='release'else[])
    with stdout.open('w')as out,stderr.open('w')as err:
        result=subprocess.run(command,cwd=source,env=env,stdout=out,stderr=err)
    binaries=[]
    for line in stdout.read_text().splitlines():
        try:row=json.loads(line)
        except json.JSONDecodeError:continue
        if row.get('reason')=='compiler-artifact'and row.get('executable')and row.get('profile',{}).get('test'):binaries.append(pathlib.Path(row['executable']))
    receipt={'schema':1,'profile':a.profile,'status':'built'if result.returncode==0 and len(binaries)==1 else'build-failure','exit_code':result.returncode,'argv':command,'cwd':str(source),'rustc':version,'toolchain_binary':identity(rustc),'overlay_manifest':identity(m),'environment':{k:env[k]for k in ('CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG')},'stdout':identity(stdout),'stderr':identity(stderr)}
    if receipt['status']=='built':receipt['binary']=identity(binaries[0])
    (HERE/(name+'.json')).write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt))
    return 0 if receipt['status']=='built'else 1
if __name__=='__main__':sys.exit(main())
