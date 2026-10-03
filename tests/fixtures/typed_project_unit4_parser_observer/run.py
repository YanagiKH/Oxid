#!/usr/bin/env python3
"""Collect real observations; does not evaluate or invent expected outcomes."""
import argparse,base64,gzip,hashlib,json,os,pathlib,platform,re,struct,subprocess,uuid
import sys
if sys.flags.optimize or not __debug__:
    raise SystemExit("OPTIMIZED_PYTHON_UNSUPPORTED: parser observer requires enabled admission checks")
from parse_debug import parse
HERE=pathlib.Path(__file__).resolve().parent
FREEZE='7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027'
CONTRACT='b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc'
ENTRY='frontend::parser::unit4_observer::observe_request'
LIMIT=134217728

def digest(b):return hashlib.sha256(b).hexdigest()
def identity(p):
    p=pathlib.Path(p);b=p.read_bytes();return {'path':str(p.resolve()),'bytes':len(b),'sha256':digest(b)}
def unique(pairs):
    d={}
    for k,v in pairs:
        assert k not in d,('duplicate JSON key',k);d[k]=v
    return d
def read_json(p):return json.loads(p.read_text(),object_pairs_hook=unique)
def verified(identity_row):
    actual=identity(identity_row['path']);assert actual['bytes']==identity_row['bytes']and actual['sha256']==identity_row['sha256'],identity_row['path'];return pathlib.Path(identity_row['path'])
def b64(s):return base64.b64encode(s.encode('utf8')).decode('ascii')
def runtime_host():
    observed=platform.uname()
    system={'Linux':'linux','Darwin':'macos','Windows':'windows'}.get(observed.system,observed.system.lower())
    machine=observed.machine.lower()
    architecture={'amd64':'x86_64','x86_64':'x86_64','arm64':'aarch64','aarch64':'aarch64'}.get(machine,machine)
    return {'os':system,'architecture':architecture,'python_pointer_width':struct.calcsize('P')*8,'uname':{'system':observed.system,'release':observed.release,'version':observed.version,'machine':observed.machine},'python_executable':sys.executable,'python_version':sys.version}
def normalize(raw,case,receipt,nonce,host):
    assert raw['case_id']==case['id'] and raw['nonce']==nonce and raw['schema']=='oxid-unit4-parser-raw-v1'
    actual_source=raw['source_utf8'].encode('utf8');assert digest(actual_source)==case['source']['sha256']and len(actual_source)==case['source']['bytes']and raw['display_path']==case['source']['path'],'executed source mismatch'
    modes=['ProjectCandidate']+(['OwnedCandidate']if 'relation_to_original'in case['expected']else[])
    assert len(raw['observations'])==len(modes)
    rows=[]
    for index,(mode,row)in enumerate(zip(modes,raw['observations'])):
        assert row['mode']==mode and row['mode_execution_index']==index and row['executed']is True
        process_os=row.pop('runtime_os');process_arch=row.pop('runtime_architecture');process_width=row.pop('pointer_width')
        assert process_os==host['os']and process_arch==host['architecture']and process_width==host['python_pointer_width'],'compiled process ABI and measured host differ'
        binding={'contract_decoded_sha256':CONTRACT,'package_freeze_sha256':FREEZE,'case_id':case['id'],'source_sha256':case['source']['sha256'],'source_bytes':case['source']['bytes'],'display_path':case['source']['path'],'candidate_source_manifest_sha256':receipt['candidate_source_manifest_sha256'],'observer_source_sha256':receipt['observer_source_sha256'],'binary_sha256':receipt['binary']['sha256'],'profile':receipt['profile'],'actual_runtime_os':host['os'],'actual_runtime_architecture':host['architecture'],'actual_pointer_width':process_width,'rust_compiler_target':receipt['target'],'mode':row.pop('mode'),'seam':case.get('seam',{}),'execution_id':nonce,'source_generation':row.pop('source_generation'),'mode_execution_index':row.pop('mode_execution_index')}
        jsons=row.pop('json_diagnostic_renderings');assert [json.loads(s,object_pairs_hook=unique)for s in jsons]==row['diagnostics']
        row['json_diagnostic_bytes_base64']=[b64(s)for s in jsons];row['human_diagnostic_bytes_base64']=b64(row.pop('human_diagnostic_rendering'))
        if row['ast']is not None:row['ast']['canonical']=parse(row['ast'].pop('canonical_debug'))
        rows.append({'schema':'oxid-unit4-parser-observation-v1','binding':binding,**row})
    assert len({r['binding']['source_generation']for r in rows})==1
    return rows

def main():
    ap=argparse.ArgumentParser();ap.add_argument('--build-receipt',type=pathlib.Path,required=True);ap.add_argument('--contract-dir',type=pathlib.Path,required=True);ap.add_argument('--checkpoint',type=pathlib.Path,required=True);ap.add_argument('--output',type=pathlib.Path,required=True);ap.add_argument('--case',action='append');a=ap.parse_args()
    assert a.checkpoint.is_file(),'durable authority checkpoint required'
    # The coordinator supplies the already verified checkpoint; bind its exact bytes.
    receipt=read_json(a.build_receipt);assert receipt['status']=='built'and receipt['exit_code']==0
    binary=verified(receipt['binary']);verified(receipt['overlay_manifest']);verified(receipt['rustc'])
    assert digest((a.contract_dir/'package-freeze.json').read_bytes())==FREEZE
    freeze=read_json(a.contract_dir/'package-freeze.json')
    for item in freeze['files']:
        data=(a.contract_dir/item['path']).read_bytes();assert len(data)==item['bytes']and digest(data)==item['sha256'],item['path']
    decoded=gzip.decompress((a.contract_dir/'parser-contract-new-v1.json.gz').read_bytes());assert digest(decoded)==CONTRACT
    contract=json.loads(decoded,object_pairs_hook=unique);cases=contract['cases'];assert len(cases)==contract['case_count']==248
    if a.case:
        assert len(set(a.case))==len(a.case),'duplicate requested case'
        assert set(a.case)<={c['id']for c in cases},'unknown requested case';cases=[c for c in cases if c['id']in a.case]
    host=runtime_host()
    actual_tuple={'os':host['os'],'architecture':host['architecture'],'pointer_width':host['python_pointer_width'],'rust_target':receipt['target']}
    if actual_tuple not in contract['required_platforms']:
        raise RuntimeError('UNSUPPORTED_PLATFORM: '+json.dumps(actual_tuple,sort_keys=True))
    out=a.output.resolve();assert not out.exists(),out;out.mkdir(parents=True)
    roster=subprocess.run([str(binary),'--list'],capture_output=True,text=True);(out/'test-roster.stdout').write_text(roster.stdout);(out/'test-roster.stderr').write_text(roster.stderr)
    assert roster.returncode==0 and roster.stdout.splitlines().count(ENTRY+': test')==1,'exact observer absent/duplicate'
    manifest={'schema':'oxid-unit4-parser-execution-v1','host_runtime':host,'status':'running','contract_decoded_sha256':CONTRACT,'package_freeze_sha256':FREEZE,'build_receipt':identity(a.build_receipt),'authority_checkpoint':identity(a.checkpoint),'driver':identity(pathlib.Path(__file__)),'normalizer':identity(HERE/'parse_debug.py'),'requested_case_ids':[c['id']for c in cases],'profile':receipt['profile'],'case_receipts':[]}
    (out/'execution-manifest.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n')
    observations=out/'observations.jsonl'
    with observations.open('w')as observations_out:
        for case in cases:
            case_dir=out/case['id'];case_dir.mkdir();source=base64.b64decode(case['source']['base64'],validate=True);assert len(source)==case['source']['bytes']and digest(source)==case['source']['sha256'];(case_dir/'source.ox').write_bytes(source)
            nonce=uuid.uuid4().hex;seam=case.get('seam',{});reject=seam.get('reject_node_admission',{});segment=seam.get('segment_span',[0,0])
            env=os.environ.copy();env.update({
                'UNIT4_SOURCE':str(case_dir/'source.ox'),'UNIT4_DISPLAY_PATH':case['source']['path'],'UNIT4_NODE_LIMIT':str(seam.get('node_limit',case['limits']['nodes'])),'UNIT4_TOKEN_LIMIT':str(case['limits']['non_eof_tokens']),'UNIT4_RESERVE_FAIL_AT':str(seam.get('reserve_fail_at')or 0),'UNIT4_REJECT_KIND':reject.get('kind',''),'UNIT4_REJECT_OCCURRENCE':str(reject.get('occurrence',0)),'UNIT4_DIRECT':str(int(seam.get('direct_operation')=='path_segment')),'UNIT4_SEGMENT_START':str(segment[0]),'UNIT4_SEGMENT_END':str(segment[1]),'UNIT4_ORIGINAL':str(int('relation_to_original'in case['expected'])),'UNIT4_CONTROL':str(int(receipt['control'])),'UNIT4_NONCE':nonce,'UNIT4_CASE_ID':case['id'],'UNIT4_RAW_OUTPUT':str(case_dir/'raw.json')})
            request={'case_id':case['id'],'source':identity(case_dir/'source.ox'),'display_path':case['source']['path'],'limits':case['limits'],'seam':seam,'original_mode_requested':env['UNIT4_ORIGINAL']=='1','environment':{k:v for k,v in env.items()if k.startswith('UNIT4_')}}
            (case_dir/'request.json').write_text(json.dumps(request,sort_keys=True,indent=2)+'\n')
            cmd=[str(binary),ENTRY,'--exact','--ignored','--nocapture','--test-threads=1']
            with (case_dir/'stdout.txt').open('w')as stdout,(case_dir/'stderr.txt').open('w')as stderr:r=subprocess.run(cmd,env=env,stdout=stdout,stderr=stderr,timeout=120)
            case_receipt={'case_id':case['id'],'execution_id':nonce,'argv':cmd,'exit_code':r.returncode,'request':identity(case_dir/'request.json'),'stdout':identity(case_dir/'stdout.txt'),'stderr':identity(case_dir/'stderr.txt'),'status':'executed'if r.returncode==0 else'observer-failure'}
            manifest['case_receipts'].append(case_receipt)
            try:
                assert r.returncode==0,'observer executable failed'
                stdout=(case_dir/'stdout.txt').read_text();assert stdout.count('UNIT4_EXECUTED '+nonce)==1 and re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;',stdout),'zero/unconfirmed observer execution'
                assert (case_dir/'raw.json').stat().st_size<=LIMIT
                assert identity(case_dir/'source.ox')==request['source'],'source changed during execution'
                case_receipt['raw']=identity(case_dir/'raw.json');raw=read_json(case_dir/'raw.json');rows=normalize(raw,case,receipt,nonce,host)
                for row in rows:observations_out.write(json.dumps(row,sort_keys=True,separators=(',',':'),ensure_ascii=False)+'\n')
                case_receipt['observations']=len(rows)
            except Exception as error:
                case_receipt['status']='observer-failure';case_receipt['error']=str(error);manifest['status']='failed';(out/'execution-manifest.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n');raise
            (case_dir/'receipt.json').write_text(json.dumps(case_receipt,sort_keys=True,indent=2)+'\n')
    manifest['status']='collected';manifest['observations']=identity(observations);manifest['case_count']=len(cases);manifest['observation_count']=sum(r['observations']for r in manifest['case_receipts']);(out/'execution-manifest.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n');print(json.dumps({'status':'collected','cases':len(cases),'observations':manifest['observation_count'],'manifest':str(out/'execution-manifest.json')}))
if __name__=='__main__':main()
