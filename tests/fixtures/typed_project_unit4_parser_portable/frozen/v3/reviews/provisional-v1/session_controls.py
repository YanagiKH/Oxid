#!/usr/bin/env python3
"""Bounded synthetic admission integration controls, never candidate evidence."""
import copy,json,os,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT/'snapshot'))
import portable as p
A=p.authority()
TOOLCHAIN=Path('/workspace/scratch/8266abf56995/toolchains/rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu')
CACHE=Path('/workspace/scratch/8266abf56995/toolchains/cargo-home')
CONTRACT=Path('/workspace/shared/oxid-reset-recovery-20261003/contract-transport-independent-review/materialized/workspace/shared/oxid-reset-recovery-20261003/new-contracts/parser')
root=ROOT/'synthetic-session'
p.prepare('/workspace/scratch/8266abf56995/oxid-parser-unit4-adapter','/workspace/scratch/8266abf56995/oxid-recovery-main',root)
session_path=root/'session.json';session=p.read(session_path)
c,proof=p.comparator()
sys.path.insert(0,str(ROOT/'snapshot/frozen/comparator'))
import test_mutations as tm
case=next(x for x in c.load_contract(CONTRACT)['cases'] if x['id']=='ledger-empty')
results=[]
def save(path,value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_bytes(value if isinstance(value,bytes) else (json.dumps(value,sort_keys=True)+'\n').encode())
    return p.identity(path)
def record(name,expected,observed,detail):
    results.append({'name':name,'expected':expected,'observed':observed,'status':'pass' if expected==observed else 'fail','detail':detail})

profile='debug';out=root/'build-debug';out.mkdir();(out/'home').mkdir();p.cargo_cache(CACHE,out/'cargo-home',A)
started=time.time_ns();result=out/'result';binary=result/'target'/A['recipe']['target']/profile/'deps/oxid-abcdef'
raw=bytearray(64);raw[:6]=b'\x7fELF\x02\x01';raw[18:20]=(62).to_bytes(2,'little');binary_id=save(binary,bytes(raw));finished=time.time_ns()
cargo={'reason':'compiler-artifact','package_id':'path+'+(root/'source').as_uri()+'#oxid@0.9.0','manifest_path':str(root/'source/Cargo.toml'),'target':{'kind':['bin'],'crate_types':['bin'],'name':'oxid','src_path':str(root/'source/src/cli.rs'),'edition':'2021','doc':True,'doctest':False,'test':True},'profile':{'opt_level':'0','debuginfo':0,'debug_assertions':True,'overflow_checks':True,'test':True},'features':[],'filenames':[str(binary)],'executable':str(binary),'fresh':False}
build={'schema':'oxid-unit4-parser-build-v1','status':'built','exit_code':0,'control':False,'profile':profile,'target':A['recipe']['target'],'argv':[str(TOOLCHAIN/'bin/cargo'),*A['recipe']['cargo_tail']],'environment':A['recipe']['build_environment'],'cwd':str(root/'source'),'rustc_version':A['recipe']['rustc_verbose'],'rustc':p.identity(TOOLCHAIN/'bin/rustc'),'overlay_manifest':p.identity(root/'source/overlay-manifest.json'),'candidate_source_manifest_sha256':A['candidate_source_manifest_sha256'],'observer_source_sha256':A['helper_manifest_sha256'],'stdout':save(result/'stdout.jsonl',(json.dumps(cargo)+'\n{"reason":"build-finished","success":true}\n').encode()),'stderr':save(result/'stderr.txt',b''),'binary':binary_id}
env=p.minimal_env(out/'home',out/'cargo-home',TOOLCHAIN)
build_host=copy.deepcopy(session['host']);build_host['uname']['release']='SYNTHETIC-DIFFERENT-BUILD-KERNEL'
build_host['uname']['version']='SYNTHETIC-DIFFERENT-BUILD-HOST'
invocation={'argv':[sys.executable,'-B',str(root/'helpers/build.py'),'--overlay',str(root/'source/overlay-manifest.json'),'--profile',profile,'--output',str(result)],'cwd':str(root),'environment':env,'started_ns':started,'finished_ns':finished,'exit_code':0,'stdout':save(out/'driver.stdout',b'SYNTHETIC build envelope, no compilation\n'),'stderr':save(out/'driver.stderr',b''),'host':build_host,'tools':p.process_tools(env)}
portable_build={'schema':'oxid-unit4-portable-parser-build-v1','session':p.identity(session_path),'profile':profile,'toolchain':str(TOOLCHAIN),'invocation':save(out/'invocation.json',invocation),'receipt':save(result/'build-receipt.json',build)}
save(out/'portable-build.json',portable_build)
p.verify_build(root,session_path,profile,A)

out=root/'collect-debug';result=out/'result';case_dir=result/case['id'];nonce='1'*32
source_id=save(case_dir/'source.ox',b'')
case_env={'UNIT4_NONCE':nonce,'UNIT4_CASE_ID':case['id'],'UNIT4_NODE_LIMIT':'100000','UNIT4_TOKEN_LIMIT':'100000','UNIT4_RESERVE_FAIL_AT':'0','UNIT4_CONTROL':'0','UNIT4_SOURCE':source_id['path'],'UNIT4_DISPLAY_PATH':case['source']['path'],'UNIT4_ORIGINAL':'0','UNIT4_REJECT_KIND':'','UNIT4_REJECT_OCCURRENCE':'0','UNIT4_DIRECT':'0','UNIT4_SEGMENT_START':'0','UNIT4_SEGMENT_END':'0','UNIT4_RAW_OUTPUT':str(case_dir/'raw.json')}
request={'case_id':case['id'],'source':source_id,'display_path':case['source']['path'],'limits':case['limits'],'seam':{},'original_mode_requested':False,'environment':case_env}
row=tm.make(case)
raw_row={k:copy.deepcopy(v) for k,v in row.items() if k not in {'schema','binding','json_diagnostic_bytes_base64','human_diagnostic_bytes_base64'}}
raw_row.update(mode='ProjectCandidate',mode_execution_index=0,source_generation=1,runtime_os='linux',runtime_architecture='x86_64',pointer_width=64,json_diagnostic_renderings=[],human_diagnostic_rendering='')
raw_row['ast'].pop('canonical')
raw_row['ast']['canonical_debug']='Program { tokens: [Token { kind: Eof, span: Span { file: SourceFileId(0), start: 0, end: 0 } }], functions: [], expressions: [], records: [], items: [], modules: [], paths: [], path_segments: [], imports: [], source: SourceProvenance { text_len: 0, file: SourceFileId(0), .. }, project_syntax: false }'
raw={'schema':'oxid-unit4-parser-raw-v1','case_id':case['id'],'nonce':nonce,'source_utf8':'','display_path':case['source']['path'],'observations':[raw_row]}
host=copy.deepcopy(session['host'])
normalized=c.normalize_raw(copy.deepcopy(raw),case,build,nonce,host)
receipt={'case_id':case['id'],'execution_id':nonce,'status':'executed','exit_code':0,'argv':[str(binary),'frontend::parser::unit4_observer::observe_request','--exact','--ignored','--nocapture','--test-threads=1'],'stdout':save(case_dir/'stdout.txt',('UNIT4_EXECUTED '+nonce+'\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n').encode()),'stderr':save(case_dir/'stderr.txt',b''),'request':save(case_dir/'request.json',request),'raw':save(case_dir/'raw.json',raw),'observations':1}
save(case_dir/'receipt.json',receipt)
save(result/'test-roster.stdout',b'SYNTHETIC roster\n');save(result/'test-roster.stderr',b'')
manifest={'schema':'oxid-unit4-parser-execution-v1','status':'collected','contract_decoded_sha256':c.CONTRACT_SHA,'package_freeze_sha256':c.FREEZE_SHA,'host_runtime':host,'authority_checkpoint':p.identity(session_path),'build_receipt':portable_build['receipt'],'profile':profile,'driver':p.identity(root/'helpers/run.py'),'normalizer':p.identity(root/'helpers/parse_debug.py'),'requested_case_ids':[case['id']],'case_count':1,'case_receipts':[receipt],'observation_count':1,'observations':save(result/'observations.jsonl',(json.dumps(normalized[0])+'\n').encode())}
manifest_path=result/'execution-manifest.json'
env=p.minimal_env(root/'home');now=time.time_ns()
collect_invocation={'argv':[sys.executable,'-B',str(root/'helpers/run.py'),'--build-receipt',str(root/'build-debug/result/build-receipt.json'),'--contract-dir',str(CONTRACT),'--checkpoint',str(session_path),'--output',str(result)],'cwd':str(root),'environment':env,'started_ns':now,'finished_ns':now,'exit_code':0,'stdout':save(out/'driver.stdout',b'SYNTHETIC collection envelope, no candidate execution\n'),'stderr':save(out/'driver.stderr',b''),'host':host,'tools':p.process_tools(env)}
collection={'schema':'oxid-unit4-portable-parser-collection-v1','session':p.identity(session_path),'profile':profile,'contract_dir':str(CONTRACT),'invocation':save(out/'invocation.json',collect_invocation),'manifest':save(manifest_path,manifest)}
save(out/'portable-collection.json',collection)
def admit(fixture_contract=None,nonces=None):
    return c.execution_manifest(manifest_path,fixture_contract or {'cases':[case]},{'authority':A,'session_path':session_path,'nonces':set() if nonces is None else nonces})
profile_got,observations,bindings=admit()
record('build_collection_host_consistency',True,build_host==host,{'admission_result':'accepted mismatched build/collection uname','returned_profile':profile_got,'observations':len(observations),'build_uname':build_host['uname'],'collection_uname':host['uname'],'scope':'single synthetic ledger-empty case; no full-corpus qualification claim'})
# Stronger direct negative controls retain the real authority/session/source,
# exact receipt paths, synthetic artifact identity and downstream raw checks.
try:
    admit(c.load_contract(CONTRACT))
    rejected=False
except ValueError:
    rejected=True
record('full_contract_rejects_one_case',True,rejected,{'scope':'full exact frozen contract against one synthetic ledger-empty case'})
try:
    admit(nonces={nonce})
    rejected=False
except ValueError:
    rejected=True
record('previous_profile_nonce_reuse_rejected',True,rejected,{'scope':'shared nonce admission set contains the same case execution id'})
extra=result/'unreported-evidence';extra.write_bytes(b'SYNTHETIC extra')
try:
    admit()
    rejected=False
except ValueError:
    rejected=True
record('unreported_collection_file_rejected',True,rejected,{'scope':'unexpected file in fresh result inventory'})
extra.unlink()
# No data deletion beyond this review-created single temporary control file.
report={'schema':'oxid-portable-independent-session-controls-v1','scope':'real source preparation plus synthetic one-case admission; no Rust build or candidate execution','records':results,'real_source_materializations':1,'compiler_executions':0,'candidate_executions':0,'target_implementation_sha256':p.sha((ROOT/'snapshot/portable.py').read_bytes())}
p.write(ROOT/'session-report.json',report);print(json.dumps(report,indent=2))
raise SystemExit(any(x['status']=='fail' for x in results))
