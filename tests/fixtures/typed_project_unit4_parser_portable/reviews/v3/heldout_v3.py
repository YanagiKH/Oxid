#!/usr/bin/env python3
"""Independent final v3 boundary controls. Synthetic fixtures never qualify a candidate."""
import contextlib,copy,json,os,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT/'snapshot'))
import portable as p
from synthetic_receipts import build_fixture
from test_passivity import append_passivity_fixture
A=p.authority(); records=[]
HIST=Path(os.environ['UNIT4_PORTABLE_HISTORICAL_REPO']); CURRENT=Path(os.environ['UNIT4_PORTABLE_CHECKOUT'])
TOOLCHAIN=Path(os.environ['UNIT4_PORTABLE_TOOLCHAIN'])
@contextlib.contextmanager
def poisoned(env):
    saved=os.environ.copy()
    try: os.environ.update(env); yield
    finally: os.environ.clear();os.environ.update(saved)
def check(name,condition,detail=None):
    records.append({'name':name,'status':'pass' if condition else 'fail','detail':detail}); assert condition,name
def reject(name,fn):
    try: fn()
    except (ValueError,OSError,KeyError,TypeError,IndexError) as e:check(name,True,str(e));return
    check(name,False,'invalid input admitted')
# Oracle uses absolute system Git and a clean environment independently of p.git.
clean={'PATH':'/usr/bin:/bin','HOME':'/nonexistent/independent-git-home','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null','GIT_NO_REPLACE_OBJECTS':'1'}
def truth(path,*args):return subprocess.check_output(['/usr/bin/git','-C',str(path),*args],env=clean,text=True,timeout=30).strip()
for target,other in ((CURRENT,HIST),(HIST,CURRENT)):
    want={k:truth(target,'rev-parse',arg) for k,arg in (('head','HEAD'),('tree','HEAD^{tree}'))}
    for label,env in [('directory',{'GIT_DIR':truth(other,'rev-parse','--absolute-git-dir')}),('config',{'GIT_CONFIG_COUNT':'1','GIT_CONFIG_KEY_0':'core.worktree','GIT_CONFIG_VALUE_0':str(other),'GIT_WORK_TREE':str(other),'GIT_INDEX_FILE':'/nonexistent/foreign-index','GIT_OBJECT_DIRECTORY':'/nonexistent/foreign-object','GIT_CONFIG_PARAMETERS':'malformed','PATH':'/nonexistent/foreign-bin'})]:
        with poisoned(env):got=p.verify_checkout(target,A)
        check('git_'+label+'_'+target.name,{k:got[k] for k in want}==want,{'expected':want,'actual':{k:got[k] for k in want}})
reject('nested_worktree_directory_rejected',lambda:p.git(CURRENT/'src','rev-parse','HEAD'))
# Reuse only the original review-owned exact selected-file clone; the original
# counterexample scripts are still present, making this a true regression input.
clone=Path('/workspace/scratch/8266abf56995/portable-parser-review-selected-toolchain-plus-extra')
p.verify_toolchain(clone,A)
view=p.create_executable_view(ROOT/'narrow-rust-bin',clone,A)
reject('no_narrow_view_rejected',lambda:p.minimal_env(ROOT,ROOT/'cache',clone))
env=p.minimal_env(ROOT,ROOT/'cache',clone,view)
resolved={name:subprocess.check_output(['which',name],env=env,text=True,timeout=5).strip() for name in ('rustc','cargo','which','as')}
check('old_shadow_scripts_excluded',resolved=={'rustc':str(view/'rustc'),'cargo':str(view/'cargo'),'which':'/usr/bin/which','as':'/usr/bin/as'},resolved)
check('original_selected_sysroot_retained',p.verify_sysroot(view,clone,env)==str(clone))
(view/'as').symlink_to(clone/'bin/as')
reject('native_tool_in_view_rejected',lambda:p.minimal_env(ROOT,ROOT/'cache',clone,view));(view/'as').unlink()
(view/'cargo').unlink();(view/'cargo').symlink_to(clone/'bin/which')
reject('view_cargo_target_substitution_rejected',lambda:p.minimal_env(ROOT,ROOT/'cache',clone,view));(view/'cargo').unlink();(view/'cargo').symlink_to(clone/'bin/cargo')
# Unmocked independent host and passivity probes using a real freshly prepared
# source session and the reviewed suite's explicitly synthetic artifact bodies.
f=build_fixture(ROOT/'synthetic-session');root=f['root'];sp=f['session_path'];mp=f['manifest_path']
append_passivity_fixture(f)
paths=[sp,root/'prepare/invocation.json',root/'build-debug/invocation.json',root/'build-debug/portable-build.json',root/'collect-debug/invocation.json',root/'collect-debug/portable-collection.json',mp]
paths.extend(x for x in (root/'passivity-debug').rglob('*') if x.is_file())
original={x:x.read_bytes() for x in paths}
def restore():
    for path,raw in original.items():path.write_bytes(raw)
def edit(path,fn):
    value=p.read(path);fn(value);p.write(path,value)
def refresh_invocations():
    for stage,bound in [('build-debug','portable-build.json'),('collect-debug','portable-collection.json')]:
        edit(root/stage/bound,lambda v,stage=stage:v.__setitem__('invocation',p.identity(root/stage/'invocation.json')))
    edit(root/'collect-debug/portable-collection.json',lambda v:v.__setitem__('manifest',p.identity(mp)))
def admit():return f['c'].execution_manifest(mp,{'cases':[f['case']]},{'authority':A,'session_path':sp,'nonces':set()})
check('one_case_synthetic_admission',len(admit()[1])==1)
for path in (root/'build-debug/invocation.json',root/'collect-debug/invocation.json'):
    edit(path,lambda v:v['host']['uname'].__setitem__('release','SYNTHETIC-COHERENT-DIFFERENT'))
edit(mp,lambda v:v['host_runtime']['uname'].__setitem__('release','SYNTHETIC-COHERENT-DIFFERENT'))
refresh_invocations();reject('coherent_build_collection_host_change_rejected',admit);restore()
edit(root/'prepare/invocation.json',lambda v:v['host']['uname'].__setitem__('release','SYNTHETIC-DIFFERENT-PREPARE'))
edit(sp,lambda v:v.__setitem__('prepare_invocation',p.identity(root/'prepare/invocation.json')))
reject('rehashed_prepare_host_change_rejected',lambda:p.session_at(sp,A));restore()
pass_report=root/'passivity-debug/result/report.json';pass_bound=root/'passivity-debug/portable-passivity.json'
def passive(nonces=None):return p.verify_passivity(root,sp,'debug',A,set() if nonces is None else nonces)
def rebind_passivity():
    report=p.read(pass_report)
    for row in report['results']:
        for receipt in row['receipts']:
            for key in ('raw','stdout','stderr'):receipt[key]=p.identity(receipt[key]['path'])
    p.write(pass_report,report);edit(pass_bound,lambda v:v.__setitem__('report',p.identity(pass_report)))
check('six_synthetic_ordinary_pairs',passive()['pairs']==6)
raw=root/'passivity-debug/result/original-control/raw.json'
edit(raw,lambda v:v['observations'][0].__setitem__('nodes_admitted',1));rebind_passivity()
reject('successful_node_ledger_difference_rejected',passive);restore()
edit(raw,lambda v:v['observations'].reverse());rebind_passivity()
reject('swapped_mode_order_rejected',passive);restore()
edit(raw,lambda v:v['observations'][0].__setitem__('executed',False));rebind_passivity()
reject('unexecuted_ordinary_mode_rejected',passive);restore()
edit(pass_report,lambda v:v.__setitem__('control',v['instrumented']));edit(pass_bound,lambda v:v.__setitem__('report',p.identity(pass_report)))
reject('instrumented_receipt_as_control_rejected',passive);restore()
edit(pass_bound,lambda v:v.__setitem__('profile','release'))
reject('passivity_profile_relabel_rejected',passive);restore()
all_nonces={p.read(root/'passivity-debug/result'/(name+suffix)/'raw.json')['nonce'] for name,_ in A['recipe']['passivity']['cases'] for suffix in ('-instrumented','-control')}
reject('passivity_cross_gate_nonce_reuse_rejected',lambda:passive(all_nonces));restore()
# Check relative user input without resolving away symlink ancestry.
cwd=Path.cwd()
try:
    os.chdir(ROOT)
    check('relative_session_passivity_now_accepted',p.verify_passivity('synthetic-session','synthetic-session/session.json','debug',A,set())['pairs']==6)
    check('relative_session_admission_now_accepted',p.session_at('synthetic-session/session.json',A)[1]==root)
    (ROOT/'linked-session').symlink_to(root,target_is_directory=True)
    reject('relative_symlink_session_rejected',lambda:p.session_at('linked-session/session.json',A))
    reject('relative_symlink_passivity_rejected',lambda:p.verify_passivity('synthetic-session','linked-session/session.json','debug',A,set()))
finally:
    os.chdir(cwd)
# Test the real build preamble through its boundary, intercepting only the exact
# rustc version subprocess. No authority/session/toolchain admission is mocked.
from unittest import mock
real_output=p.subprocess.check_output
probes=[]
def timeout_version(argv,*args,**kwargs):
    if argv==[str(TOOLCHAIN/'bin/rustc'),'--version','--verbose']:
        probes.append({'argv':argv,'timeout':kwargs.get('timeout')})
        raise subprocess.TimeoutExpired(argv,kwargs.get('timeout'))
    return real_output(argv,*args,**kwargs)
with mock.patch.object(p.subprocess,'check_output',side_effect=timeout_version):
    try:p.build(sp,'release',TOOLCHAIN,Path(os.environ['UNIT4_PORTABLE_CARGO_CACHE']))
    except subprocess.TimeoutExpired:
        check('actual_build_preamble_has_15s_probe_timeout',probes==[{'argv':[str(TOOLCHAIN/'bin/rustc'),'--version','--verbose'],'timeout':15}])
    else:check('actual_build_preamble_has_15s_probe_timeout',False)
check('timed_out_probe_creates_no_build_output',not (root/'build-release').exists())
# Use packaged authority and independent source-byte coordinates; never compare
# against candidate outputs or derive expectations from observations.
import base64,re
base=f['c'].load_contract(f['CONTRACT']);before=f['c'].canonical(base)
authority=p.effective_authority(A)
effective,receipt=f['c'].admit_contract_amendment(base,f['CONTRACT'],authority)
check('base_contract_unchanged_after_amendment',f['c'].canonical(base)==before)
check('full_effective_document_exact_identity',f['c'].sha(f['c'].canonical(effective))=='c2d4f8db28f7815b3e17ca13fec9a7da70f0923dd052656edb339bc0cd7740cd')
case=base['cases'][246];text=base64.b64decode(case['source']['base64'],validate=True)
anchors=list(re.finditer(rb'\bfn\s+(pub)\b',text));assert len(anchors)==1
start,end=anchors[0].span(1);line_start=text.rfind(b'\n',0,start)+1
expected={'start':start,'end':end,'column':start-line_start+1,'end_column':end-line_start+1}
actual={k:effective['cases'][246]['expected']['first_diagnostic_projection']['primary'][k] for k in expected}
check('effective_coordinates_match_independent_source_anchor',actual==expected,{'expected':expected,'actual':actual})
def differences(x,y,path=''):
    if type(x) is not type(y):return [path]
    if isinstance(x,dict):
        if x.keys()!=y.keys():return [path]
        return [leaf for key in sorted(x) for leaf in differences(x[key],y[key],path+'/'+key)]
    if isinstance(x,list):
        if len(x)!=len(y):return [path]
        return [leaf for index,(a,b) in enumerate(zip(x,y)) for leaf in differences(a,b,path+'/'+str(index))]
    return [] if x==y else [path]
prefix='/cases/246/expected/first_diagnostic_projection/primary/'
check('only_exact_four_primary_leaves_change',differences(base,effective)==[prefix+k for k in sorted(expected)],{'changed_leaves':differences(base,effective)})
reject('already_applied_effective_base_rejected',lambda:f['c'].admit_contract_amendment(effective,f['CONTRACT'],authority))
reject('truncated_base_rejected',lambda:f['c'].admit_contract_amendment({'cases':base['cases']},f['CONTRACT'],authority))
reject('summary_only_effective_document_rejected',lambda:f['c'].compare_effective_rows({'effective_contract_identity':receipt['effective_contract_identity']},receipt,[],{}))
wrong=copy.deepcopy(receipt);wrong['ordered_amendment_sha256']*=2
reject('duplicate_amendment_receipt_rejected',lambda:f['c'].compare_effective_rows(effective,wrong,[],{}))
result=f['c'].compare_effective_rows(effective,receipt,[],{})
check('zero_rows_fail_effective638',result['status']=='fail' and result['expected_observations']==638)
check('original_raw_identity_remains_separate',result['execution_contract']['contract_decoded_sha256']==A['contract_sha256'] and result['execution_contract']['package_freeze_sha256']==A['freeze_sha256'] and result['effective_parser_document_canonical_sha256']!=A['contract_sha256'])
# Check all frozen v8b bodies and the retained raw suffix.

import ast
source=(p.HERE/'frozen/comparator/comparator.py').read_text();begin=source.index(p.PREFIX_START,source.index('def execution_manifest(path, contract, approval):\n'));end=source.index(p.PREFIX_END,begin)
derived=source[:begin]+'    manifest, host, build, profile = portable_admission(path, contract, approval)\n'+source[end:]
old=ast.parse(source);new=ast.parse(derived)
old_map={x.name:ast.dump(x,include_attributes=False) for x in old.body if isinstance(x,ast.FunctionDef) and x.name!='execution_manifest'}
new_map={x.name:ast.dump(x,include_attributes=False) for x in new.body if isinstance(x,ast.FunctionDef) and x.name!='execution_manifest'}
check('non_admission_functions_unchanged',old_map==new_map,{'functions':len(old_map)})
old_fn=next(x for x in old.body if isinstance(x,ast.FunctionDef) and x.name=='execution_manifest');new_fn=next(x for x in new.body if isinstance(x,ast.FunctionDef) and x.name=='execution_manifest')
start=next(i for i,x in enumerate(old_fn.body) if isinstance(x,ast.Assign) and any(isinstance(y,ast.Name) and y.id=='cases' for y in x.targets))
check('raw_suffix_unchanged',[ast.dump(x,include_attributes=False) for x in old_fn.body[start:]]==[ast.dump(x,include_attributes=False) for x in new_fn.body[1:]],{'statements':len(new_fn.body)-1})
p.write(ROOT/'heldout-report.json',{'schema':'oxid-portable-v3-independent-heldout-controls-v1','status':'pass' if all(x['status']=='pass' for x in records) else 'fail','scope':'source/recipe admission and synthetic ordinary passivity only','target_adapter_sha256':p.sha((p.HERE/'portable.py').read_bytes()),'records':records,'compiler_build_executions':0,'candidate_executions':0,'native_and_rust_identity_probes':True})
print(json.dumps({'status':'pass','controls':len(records)},indent=2))
