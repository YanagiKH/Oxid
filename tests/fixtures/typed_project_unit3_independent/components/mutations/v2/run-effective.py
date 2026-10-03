#!/usr/bin/env python3
"""Run one frozen source-only mutation through the reviewed observer controller.

Expected outcomes are deliberately not imported. A separate reviewer-owned
comparator consumes the resulting mutation envelope and source observations.
"""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,sys,time
if not __debug__:
    raise RuntimeError('mutation observer requires Python assertions; -O is forbidden')
if 'PYTHONOPTIMIZE' in os.environ:
    raise RuntimeError('inherited PYTHONOPTIMIZE is forbidden')
HERE=pathlib.Path(__file__).resolve().parent
ORACLES=HERE.parent/'typed-project-unit3-oracles'
OBSERVER=HERE.parent/'typed-project-unit3-observer'
REQUEST_SHA='628d08d0500d9b98acac658ad3c75ac311272cd493913b55c2e2598a48240bd8'
PINS={
    ORACLES/'mutation-requests.json':'228f51304751f2a08d962d95aecbe066fdfa813fdf10b46b144f9cf7ee51846b',
    ORACLES/'supplements/mutation-applicability-v1/supplement-manifest.json':'a01be26791455c85790abcee37f05373da9a0fe7f77aaae3fbb635378cebb987',
    ORACLES/'pre-execution-manifest.json':'b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba',
    ORACLES/'requests.jsonl':'f405bf46d8945027cd1d79d3c125692b4c00b8fe57b97c0d55c68b83b9795113',
    OBSERVER/'run.py':'ff51288e0bf90733eafe4690c02feacac06e61ab0608642423fd700e7fca42fc',
    OBSERVER/'parse_debug.py':'638fb39b814f637bf21902738edb35bbe775956d2c00a0e307957668e38c4264',
    HERE/'mutation-manifest-v2.json':'d6bc5a6bc88a51be9d109faaebe9eb5471e3977ea56cfb04cc8397eff219c874',
    HERE/'mutation-overlay-v2.patch':'c3a9ddc7d95fce4ea25b07ac9de2fdbdcefb43e052dcaeb1042bcf59e87536f9',
}
RUSTC=HERE.parent/'toolchains/rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/bin/rustc'
RUSTC_SHA='f3834d26669b03f6855fa54bd2381443123e860167bc0c7a8dd137e5cf6e5e4f'
def identity(path):
    data=path.read_bytes();return {'path':str(path.resolve()),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
def encoded(value):return (json.dumps(value,indent=2,sort_keys=True)+'\n').encode()
def verify_identity(info):assert identity(pathlib.Path(info['path']))==info,'changed bound artifact'
def verify_pins():
    for path,expected in PINS.items():assert identity(path)['sha256']==expected,('changed reviewed input',str(path))
def verify_source(manifest):
    root=pathlib.Path(manifest['source']);expected=set()
    for entry in manifest['files']:
        path=root/entry['path'];got=identity(path);assert got['sha256']==entry['sha256']and got['bytes']==entry['bytes'],entry['path'];expected.add(entry['path'])
    actual={str(path.relative_to(root))for path in root.rglob('*')if path.is_file()}
    assert actual==expected,'changed mutation source inventory'
def debug_tools():
    spec=importlib.util.spec_from_file_location('mutation_debug_parser',OBSERVER/'parse_debug.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module

def main():
    p=argparse.ArgumentParser();p.add_argument('--id',required=True);p.add_argument('--profile',choices=('debug','release'),required=True);p.add_argument('--build-receipt',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--baseline',type=pathlib.Path)
    a=p.parse_args();assert a.id=='raw-wrong_scalar_result','effective request adapter is restricted to its reviewed ID';verify_pins();reqfile=ORACLES/'supplements/mutation-applicability-v1/requests.json';req_identity=identity(reqfile);assert req_identity['sha256']==REQUEST_SHA
    payload=json.loads(reqfile.read_text());original_payload=json.loads((ORACLES/'mutation-requests.json').read_text());
    original_request=next(r for r in original_payload['mutations']if r['id']==a.id);rows=payload['mutations'];assert payload['count']==len(rows)==114 and len({r['id']for r in rows})==114
    matching=[r for r in rows if r['id']==a.id];assert len(matching)==1,'unknown mutation ID';request=matching[0];assert set(request)=={'id','positive_control','mutation'}
    assert request['mutation']['kind']!='driver-boundary','driver IDs use separately reviewed attributed receipts'
    source_requests=ORACLES/'requests.jsonl';source_request_identity=identity(source_requests);sources=[json.loads(l)for l in source_requests.read_text().splitlines()];controls=[r for r in sources if r['id']==request['positive_control']];assert len(controls)==1;source=controls[0]
    build_path=a.build_receipt.resolve();build_identity=identity(build_path);build=json.loads(build_path.read_text());assert build['status']=='built'and build['exit_code']==0 and build['profile']==a.profile
    verify_identity(build['binary']);verify_identity(build['overlay_manifest']);verify_identity(build['toolchain_binary']);verify_identity(build['stdout']);verify_identity(build['stderr'])
    assert build['toolchain_binary']['path']==str(RUSTC)and build['toolchain_binary']['sha256']==RUSTC_SHA
    assert 'rustc 1.99.0 'in build['rustc']
    assert build['overlay_manifest']==identity(HERE/'mutation-manifest-v2.json')
    assert build['environment']['CARGO_INCREMENTAL']=='0'and build['environment']['CARGO_BUILD_JOBS']=='2'
    cargo_rows=[json.loads(line)for line in pathlib.Path(build['stdout']['path']).read_text().splitlines()if line.startswith('{')]
    assert any(row.get('reason')=='build-finished'and row.get('success')is True for row in cargo_rows),'missing successful Cargo terminal'
    artifacts=[row for row in cargo_rows if row.get('reason')=='compiler-artifact'and row.get('executable')and row.get('profile',{}).get('test')]
    assert len(artifacts)==1 and artifacts[0]['executable']==build['binary']['path'],'unexpected compiler artifact roster'
    artifact=artifacts[0];profile=artifact['profile'];is_debug=a.profile=='debug'
    assert profile['opt_level']==('0'if is_debug else'3')and profile['debug_assertions']is is_debug and profile['overflow_checks']is is_debug,'Cargo profile differs from requested profile'
    source_root=HERE/'source-v2'
    assert artifact['manifest_path']==str(source_root/'Cargo.toml')and artifact['target']['src_path']==str(source_root/'src/cli.rs'),'wrong Cargo source target'
    assert artifact['target']['kind']==['bin']and artifact['target']['name']=='oxid'and artifact['features']==[]
    assert build['cwd']==str(source_root),'wrong build source directory'
    expected_argv=[str(RUSTC.with_name('cargo')),'test','--offline','--no-run','--bin','oxid','--jobs','2','--message-format=json']+([]if is_debug else['--release'])
    assert build['argv']==expected_argv,'build command does not match requested profile'
    assert pathlib.Path(build['binary']['path']).is_relative_to(HERE/'target'/a.profile),'binary outside profile target'
    manifest=json.loads(pathlib.Path(build['overlay_manifest']['path']).read_text());assert manifest['core_manifest_sha256']=='53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
    assert manifest['mutation_patch_sha256']==PINS[HERE/'mutation-overlay-v2.patch']
    assert manifest['observer_patch_sha256']=='62a2b6382f7684c5aaf03c355580f2e8d582c6c2688d7bab970f0b50f9c0ed54'
    verify_source(manifest)
    out=a.output.resolve();assert not out.exists(),'preserve prior results';out.mkdir(parents=True)
    env=os.environ.copy();env={k:v for k,v in env.items()if not k.startswith('OXID_UNIT3_MUTATION_')};env['PYTHONDONTWRITEBYTECODE']='1'
    observer_identity=identity(OBSERVER/'run.py');parser_identity=identity(OBSERVER/'parse_debug.py')
    receipt={'schema':1,'mutation_id':a.id,'profile':a.profile,'request':request,'original_request':original_request,'original_mutation_requests':identity(ORACLES/'mutation-requests.json'),'mutation_requests':req_identity,'source_requests':source_request_identity,'build_receipt':build_identity,'compiler_overlay':build['overlay_manifest'],'binary':build['binary'],'observer_controller':observer_identity,'debug_parser':parser_identity,'mutation_controller':identity(pathlib.Path(__file__)),'pinned_inputs':[identity(path)for path in PINS],'status':'started','commands':[]}
    (out/'invocation.json').write_bytes(encoded(receipt))
    def run(label,target,mutate):
        current=dict(env)
        if mutate:
            m=request['mutation'];current.update(OXID_UNIT3_MUTATION_ID=a.id,OXID_UNIT3_MUTATION_KIND=m['kind'],OXID_UNIT3_MUTATION_CATEGORY=m.get('category',''),OXID_UNIT3_MUTATION_OCCURRENCE=str(m.get('occurrence',0)))
        argv=[sys.executable,'-B',str(OBSERVER/'run.py'),'--requests',str(source_requests),'--case',source['id'],'--build-receipt',str(build_path),'--profile',a.profile,'--output',str(target),'--retain-text']
        start=time.monotonic()
        command={'label':label,'argv':argv,'exit_status':None}
        try:
            with(out/(label+'.stdout')).open('wb')as stdout,(out/(label+'.stderr')).open('wb')as stderr:
                r=subprocess.run(argv,env=current,stdout=stdout,stderr=stderr,timeout=240)
            command['exit_status']=r.returncode
        except Exception as error:
            command['execution_error']=repr(error)
            raise
        finally:
            command['seconds']=time.monotonic()-start
            for stream in ('stdout','stderr'):
                path=out/(label+'.'+stream)
                if path.exists():command[stream]=identity(path)
            receipt['commands'].append(command)
            (out/'invocation.json').write_bytes(encoded(receipt))
        assert r.returncode==0,('observer failure',label)
        child=json.loads((target/'receipt.json').read_text());assert child['status']=='observed'and child['case']==source['id']and child['profile']==a.profile
        assert child['build_receipt']==build_identity and child['source_request']==source
        return child
    try:
        baseline=a.baseline.resolve()if a.baseline else out/'baseline'
        if a.baseline:
            child=json.loads((baseline/'receipt.json').read_text());assert child['status']=='observed'and child['build_receipt']==build_identity and child['source_request']==source and child['profile']==a.profile
            assert not (baseline/'mutation-applied.debug').exists(),'baseline contains a mutation'
            for info in child['artifacts']:verify_identity(info)
        else:run('baseline',baseline,False)
        receipt['baseline_receipt']=identity(baseline/'receipt.json');receipt['baseline_observations']=identity(baseline/'observations.json')
        mutant=out/'mutant';run('mutant',mutant,True)
        pd=debug_tools()
        applied=pd.canonical(pd.parse((mutant/'mutation-applied.debug').read_text()))
        assert isinstance(applied,list)and len(applied)==4
        spec,path,before,after=applied;assert spec['id']==a.id and spec['kind']==request['mutation']['kind'];assert isinstance(path,str)and path
        before=pd.canonical(pd.parse(before));after=pd.canonical(pd.parse(after))
        artifacts={}
        for name in ['mutation-before-raw.debug','mutation-after-raw.debug','mutation-entry-agreement.debug','raw-verifier-start.debug','raw-verifier-failure.debug','generic-probe-input-original.debug','generic-probe-input-clone.debug','generic-probe-result.debug','active-constructor-map.debug']:
            file=mutant/name
            if file.exists():artifacts[name]={'identity':identity(file),'value':pd.canonical(pd.parse(file.read_text()))}
        original=mutant/'generic-probe-input-original.debug';clone=mutant/'generic-probe-input-clone.debug'
        if original.exists()or clone.exists():
            assert original.exists()and clone.exists()and original.read_bytes()==clone.read_bytes(),'raw probe clone changed structure'
            assert (mutant/'mutation-after-raw.debug').read_bytes()==original.read_bytes(),'raw probe did not use exact mutated raw'
        receipt.update(status='observed',mutation_applied={'path':path,'before':before,'after':after},mutation_artifacts=artifacts,mutant_receipt=identity(mutant/'receipt.json'),mutant_observations=identity(mutant/'observations.json'))
    except Exception as error:
        receipt.update(status='adapter-failure',error=repr(error))
    finally:
        failures=[]
        for info in [req_identity,source_request_identity,build_identity,observer_identity,parser_identity,receipt['mutation_controller'],build['binary'],build['overlay_manifest'],build['toolchain_binary'],build['stdout'],build['stderr']]:
            try:verify_identity(info)
            except Exception as error:failures.append(repr(error))
        for check in (verify_pins,lambda:verify_source(manifest)):
            try:check()
            except Exception as error:failures.append(repr(error))
        if failures:receipt.update(status='adapter-failure',post_run_identity_failures=failures)
        (out/'receipt.json').write_bytes(encoded(receipt))
    print(json.dumps({'id':a.id,'profile':a.profile,'status':receipt['status'],'output':str(out)}))
    return 0 if receipt['status']=='observed'else 1
if __name__=='__main__':sys.exit(main())
