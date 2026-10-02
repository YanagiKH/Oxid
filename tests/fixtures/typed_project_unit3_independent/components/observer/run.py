#!/usr/bin/env python3
"""One source-only request -> passive actual receipt. Never reads expectations."""
import argparse,gzip,hashlib,json,os,pathlib,re,secrets,subprocess,sys,time
from parse_debug import parse,canonical,checked_raw,validate_raw,trace
HERE=pathlib.Path(__file__).resolve().parent
ALLOWED={'id','entry','source_root','source_files','mode','operations','profiles','fuel_budgets','native'}
def sha(data):return hashlib.sha256(data).hexdigest()
def file_identity(path):
    data=path.read_bytes();return {'path':str(path),'bytes':len(data),'sha256':sha(data)}
def relative(value):
    assert isinstance(value,str)and value and not any(c in value for c in '\r\n\0'),value
    p=pathlib.PurePosixPath(value)
    assert not p.is_absolute()and all(v not in ('','..','.')for v in value.split('/')),value
    return pathlib.Path(*p.parts)
def validate(request,request_path,profile):
    assert set(request)<=ALLOWED and ALLOWED-{'native','fuel_budgets'}<=set(request),'unknown or missing source request key'
    assert isinstance(request['id'],str)and re.fullmatch(r'[A-Za-z0-9_-]+',request['id'])
    assert request['mode']in ('private-project-candidate','original-single-file')
    assert isinstance(request['profiles'],list)and request['profiles']and all(v in ('debug','release')for v in request['profiles'])
    assert profile in request['profiles']
    ops=request['operations'];assert isinstance(ops,list)and ops and len(ops)==len(set(ops))and all(x in ('check','run','native')for x in ops)
    fuel=request.get('fuel_budgets',[]);assert isinstance(fuel,list)and len(fuel)==len(set(fuel))and all(type(x)is int and 0<=x<=1_000_000 for x in fuel)
    assert len(fuel)<=100,'request fuel count ceiling'
    native=request.get('native')
    if native is not None:
        assert isinstance(native,dict)and set(native)<= {'opt_level','source_free','admission_only'},'unknown native control'
        assert native.get('opt_level')==0 and type(native.get('opt_level'))is int
        assert sum(native.get(k)is True for k in ('source_free','admission_only'))==1
        assert all(type(native[k])is bool for k in ('source_free','admission_only')if k in native)
        assert not any(native.get(k)is False for k in ('source_free','admission_only')),'false native control is not an alternate request'
    root=(request_path.parent/relative(request['source_root'])).resolve()
    assert root.is_dir()
    files=request['source_files'];assert isinstance(files,list)and 0<len(files)<=32
    seen=set();identities=[]
    for item in files:
        assert isinstance(item,dict)and set(item)=={'path','bytes','sha256'},'unknown source file key'
        rel=relative(item['path']);assert str(rel)not in seen;seen.add(str(rel))
        path=(root/rel).resolve();assert path.is_relative_to(root)and path.is_file()
        assert type(item['bytes'])is int and 0<=item['bytes']<=1_048_576
        assert isinstance(item['sha256'],str)and re.fullmatch('[0-9a-f]{64}',item['sha256'])
        got=file_identity(path);assert got['bytes']==item['bytes']and got['sha256']==item['sha256'],('source transport mismatch',item['path'])
        identities.append({'relative_path':str(rel),**got})
    assert {str(p.relative_to(root))for p in root.rglob('*.ox')if p.is_file()}==seen,'source directory contains undeclared Oxid files'
    entry=relative(request['entry']);assert str(entry)in seen
    return root,root/entry,list(ops)+(['native']if native is not None and 'native'not in ops else []),identities

def normalize(out,operations=None,budgets=None):
    def get(name):
        path=out/name
        return parse(path.read_text())if path.exists()else None
    loaded=get('loaded.debug');checked=get('checked.debug');raw_pre=get('raw-before-audit.debug')
    load_failure=get('load-failure.debug');check_failure=get('check-failure.debug')
    assert (loaded is None)!=(load_failure is None),'missing or contradictory actual loader outcome'
    if loaded is not None:assert (checked is None)!=(check_failure is None),'missing or contradictory actual check outcome'
    else:assert checked is None and check_failure is None
    actual={'schema':1,'loaded':canonical(loaded),'index':canonical(get('index.debug')),'namespace_work':canonical(get('namespace-work.debug')),'audit':{'visits':canonical(get('audit-visits.debug')),'usage':canonical(get('audit-usage.debug'))},'reference':{},'native':[]}
    if raw_pre is not None:
        route={'Program':'scalar','RawOwnedProgram':'owned'}[raw_pre['tag']]
        raw_pre=canonical(raw_pre,raw=True);validate_raw(raw_pre,route)
        actual.update(route=route,raw_before_audit=raw_pre)
    if checked is not None:
        route,entry,raw=checked_raw(checked);actual.update(route=route,entry=entry,raw=raw)
        after=get('checked-after-consumers.debug')
        assert after is not None and checked==after,'immutable wrapper changed across consumers'
        actual['checked_immutable']=True
        assert raw_pre==raw,'checked witness raw differs from actual pre-audit raw'
    for path in sorted(out.glob('reference-*.trace.debug')):
        name=path.name.removesuffix('.trace.debug')
        actual['reference'][name]={'outcome':json.loads((out/(name+'.json')).read_text()),'trace':trace(parse(path.read_text()))}
    for path in sorted(out.glob('native-*.ll')):actual['native'].append({'kind':'llvm-emission-only',**file_identity(path)})
    for path in sorted(out.glob('native-*.diagnostic.json')):actual['native'].append({'kind':'native-admission-diagnostic','name':path.name,'diagnostic':json.loads(path.read_text())})
    diag=out/'diagnostics.jsonl'
    if diag.exists():actual['diagnostics']=[json.loads(x)for x in diag.read_text().splitlines()]
    if checked is None:assert actual.get('diagnostics'),'missing actual denial diagnostics'
    if checked is not None and operations is not None:
        names=['default']+[f'fuel-{n}'for n in (budgets or [])]
        expected_reference={f'reference-{n}'for n in names}if 'run'in operations else set()
        assert set(actual['reference'])==expected_reference,'missing/unrequested reference outcomes'
        if 'native'in operations:
            for name in names:
                paths=[out/f'native-{name}.ll',out/f'native-{name}.diagnostic.json']
                assert sum(p.exists()for p in paths)==1,('missing/ambiguous native outcome',name)
        else:assert not actual['native'],'unrequested native output'
    # Keep both passes separate. Never derive/fill missing visits from raw.
    visits=actual['audit']['visits']
    if visits is not None:
        actual['audit']['count_visits']=[v for v in visits if v['pass']=='count']
        actual['audit']['validate_visits']=[v for v in visits if v['pass']=='validate']
        assert len(actual['audit']['count_visits'])+len(actual['audit']['validate_visits'])==len(visits)
    return actual

def main():
    p=argparse.ArgumentParser();p.add_argument('--requests',type=pathlib.Path,required=True);p.add_argument('--case',required=True);p.add_argument('--build-receipt',type=pathlib.Path,required=True);p.add_argument('--profile',choices=('debug','release'),required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--retain-text',action='store_true');a=p.parse_args()
    request_path=a.requests.resolve();matches=[]
    for line in request_path.read_text().splitlines():
        value=json.loads(line)
        if value.get('id')==a.case:matches.append(value)
    assert len(matches)==1,'case must identify one source request'
    request=matches[0];root,entry,operations,source_ids=validate(request,request_path,a.profile)
    build=json.loads(a.build_receipt.read_text());assert build['profile']==a.profile and build['status']=='built'
    binary=pathlib.Path(build['binary']['path']);assert file_identity(binary)==build['binary'],'test binary identity changed'
    manifest=pathlib.Path(build['overlay_manifest']['path']);assert file_identity(manifest)==build['overlay_manifest'],'overlay manifest identity changed'
    out=a.output.resolve();assert not out.exists(),'refuse to overwrite evidence';out.mkdir(parents=True)
    nonce=secrets.token_hex(32);request_sha=sha(json.dumps(request,sort_keys=True,separators=(',',':')).encode())
    receipt={'schema':1,'case':a.case,'profile':a.profile,'nonce':nonce,'source_request_sha256':request_sha,'source_request':request,'request_file':file_identity(request_path),'sources':source_ids,'build_receipt':file_identity(a.build_receipt.resolve()),'observer_controller':file_identity(pathlib.Path(__file__).resolve()),'normalizer':file_identity(HERE/'parse_debug.py'),'status':'started'}
    env=os.environ.copy();env.update(OXID_UNIT3_ENTRY=str(entry),OXID_UNIT3_OUTPUT=str(out),OXID_UNIT3_MODE=request['mode'],OXID_UNIT3_OPERATIONS=','.join(operations),OXID_UNIT3_FUEL=','.join(map(str,request.get('fuel_budgets',[]))),OXID_UNIT3_NONCE=nonce,OXID_UNIT3_REQUEST_SHA256=request_sha)
    command=[str(binary),'frontend::unit3_observer::observe_request','--exact','--ignored','--nocapture','--test-threads=1'];receipt['argv']=command
    start=time.monotonic()
    exit_code=None
    try:
      roster_command=[str(binary),'--list','--ignored'];receipt['roster_argv']=roster_command
      roster=subprocess.run(roster_command,env=env,text=True,capture_output=True,timeout=30)
      (out/'roster.stdout').write_text(roster.stdout);(out/'roster.stderr').write_text(roster.stderr)
      assert roster.returncode==0 and roster.stdout.splitlines().count('frontend::unit3_observer::observe_request: test')==1,'exact ignored observer test absent/duplicated in roster'
      with (out/'adapter.stdout').open('wb')as stdout,(out/'adapter.stderr').open('wb')as stderr:
        exit_code=subprocess.run(command,env=env,stdout=stdout,stderr=stderr,timeout=180).returncode
    except subprocess.TimeoutExpired:
      receipt.update(status='observer-failure',error='observer execution timed out')
    except Exception as error:
      receipt.update(status='observer-failure',error=f'{type(error).__name__}: {error}')
    receipt.update(exit_code=exit_code,seconds=time.monotonic()-start)
    if exit_code!=0:
        receipt['status']='observer-failure'
        stderr=out/'adapter.stderr';receipt['observation_limit']=stderr.exists()and'OBSERVATION_LIMIT'in stderr.read_text()
    else:
      try:
        stdout=(out/'adapter.stdout').read_text()
        assert re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;',stdout),'observer exact test did not run once'
        assert 'frontend::unit3_observer::observe_request ... ok'in stdout,'missing exact observer test success'
        start_marker=json.loads((out/'started.json').read_text());complete=json.loads((out/'completion.json').read_text())
        assert start_marker=={'nonce':nonce,'source_request_sha256':request_sha,'stage':'started'},'wrong/stale start marker'
        assert set(complete)=={'nonce','source_request_sha256','stage'}and complete['nonce']==nonce and complete['source_request_sha256']==request_sha,'wrong/stale completion marker'
        assert complete['stage']in ('load-denied','check-denied','checked'),'unknown completion stage'
        actual=normalize(out,operations,request.get('fuel_budgets',[]));payload=json.dumps(actual,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
        stage='checked'if actual.get('checked_immutable')else('check-denied'if actual['loaded']is not None else'load-denied')
        assert complete['stage']==stage,'completion marker/artifact contradiction'
        assert len(payload)<=256*1024*1024,'OBSERVATION_LIMIT: normalized bytes'
        (out/'observations.json').write_bytes(payload);receipt['status']='observed'
      except Exception as error:
        receipt.update(status='observer-failure',error=f'{type(error).__name__}: {error}',observation_limit='OBSERVATION_LIMIT'in str(error))
    artifacts=[]
    for path in sorted(out.iterdir()):
        if not path.is_file():continue
        identity=file_identity(path)
        if not a.retain_text and path.suffix in ('.debug','.json','.jsonl','.stdout','.stderr'):
            compressed=path.with_name(path.name+'.gz');compressed.write_bytes(gzip.compress(path.read_bytes(),mtime=0));identity['compressed']=file_identity(compressed);path.unlink()
        artifacts.append(identity)
    receipt['artifacts']=artifacts
    (out/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({'case':a.case,'status':receipt['status'],'output':str(out),'exit_code':exit_code}))
    return 0 if receipt['status']=='observed'else 1
if __name__=='__main__':sys.exit(main())
