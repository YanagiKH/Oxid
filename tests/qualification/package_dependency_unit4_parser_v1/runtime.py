"""Live successor parser runtime orchestration; import executes no target.

The producer uses genuine frozen helpers and unchanged public semantic drivers.
Transported JSON cannot replace a live runtime handle, BuildResult, supervisor
permit, RuntimeInputs or ProbeRecord. Global CI qualification remains separate.
"""
import ast
import builtins
import contextlib
import contextvars
import copy
import hashlib
import json
from pathlib import Path
import re
import sys
import time
import types
import uuid
import weakref
import threading
import os

_API=None
_LIVE=weakref.WeakKeyDictionary()
_ACTIVE=contextvars.ContextVar('parser_runtime_writer',default=None)
_CURRENT=[]
PROFILES=('debug','release')
ROLES=(False,True)

class Rejected(ValueError):pass
class NotReady(RuntimeError):pass

def need(ok,message):
    if not ok:raise Rejected(message)

def same(actual,wanted,message):need(actual==wanted,message)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def serial(value):return (json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()

def bind_consumer(api):
    global _API
    need(_API is None,'runtime consumer is already bound')
    _API=api

class RuntimeSession:
    __slots__=('__weakref__',)
    def __new__(cls,*args,**kwargs):raise TypeError('RuntimeSession is issued only from four actual live builds')
    def document(self):return copy.deepcopy(_live(self)['report'])

def _live(handle):
    need(type(handle) is RuntimeSession and handle in _LIVE,'live parser RuntimeSession required')
    state=_LIVE[handle];need(not state['failed'],'runtime failure retained; admission remains closed')
    need(state['thread']==threading.get_ident() and state['pid']==os.getpid(),'original runtime thread/process required')
    return state

def _function(raw,name):
    tree=ast.parse(raw)
    node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name==name)
    return b''.join(raw.splitlines(keepends=True)[node.lineno-1:node.end_lineno])+b'\n'

def derive_functions(raw):
    """Exact source derivations: only named admission/provenance seams change."""
    original_pin=_API._manifest()['retained_portable']['sha256']
    same(sha(raw),original_pin,'retained parser source bytes')
    output={};proofs={}
    for name in ('phase_control_binding','phase_scope','phase_evidence','verify_passivity','project_enum_observations'):
        original=_function(raw,name);current=original;edits=[]
        if name=='phase_scope':
            edits=[("root / 'session.json'","root / CHECKPOINT"),
                   ("session['authority_sha256']","session['authority']['sha256']"),
                   ("'portable-collection.json'","'package-source-collection.json'"),
                   ("'portable-passivity.json'","'package-source-passivity.json'")]
        elif name=='phase_control_binding':
            marker="    binding = a['current']['lexer_phase']['controls']\n"
            prefix=original.decode().split(marker)[0]
            edits=[(prefix,"def phase_control_binding(a):\n    package_phase_authority(a)\n")]
        elif name=='verify_passivity':
            marker='    report = load(artifact(envelope["report"], result / "report.json"))\n'
            prefix=original.decode().split(marker)[0]
            replacement=('def verify_passivity(root, session_path, profile, a, nonces):\n'
                '    root, session_path = Path(root).absolute(), Path(session_path).absolute()\n'
                '    envelope, builds, session = package_passivity_admission(root, session_path, profile, a)\n'
                '    out = root / ("passivity-" + profile); result = out / "result"\n')
            edits=[(prefix,replacement)]
        elif name=='phase_evidence':
            before=("        build_invocation_path = root / (('build-control-' if control else 'build-') + profile) / 'invocation.json'\n"
                "        build_invocation_identity = (identity(build_invocation_path) if identities is None\n"
                "                                     else identities[str(build_invocation_path)])\n"
                "        build_invocation = load(resolve(build_invocation_identity))\n"
                "        same(build_invocation['exit_code'], 0, 'successful actual build before phase controls')\n"
                "        prepared_ns = max(prepared_ns, build_invocation['finished_ns'])\n")
            edits=[(before,"        prepared_ns = max(prepared_ns, package_build_finished_ns(profile, control))\n")]
        seams=[]
        for before,after in edits:
            before,after=before.encode(),after.encode()
            same(current.count(before),1,'unique exact provenance seam: '+name)
            start=current.index(before);end=start+len(before)
            seams.append(dict(replaced_sha256=sha(before),replacement_sha256=sha(after),
                unchanged_prefix_sha256=sha(current[:start]),unchanged_suffix_sha256=sha(current[end:])))
            current=current[:start]+after+current[end:]
        output[name]=current
        proofs[name]=dict(original_sha256=sha(original),derived_sha256=sha(current),seams=seams)
    return output,proofs

def _derived(state):
    raw=_API._record_at(_API._manifest()['retained_portable'])
    sources,proof=derive_functions(raw)
    same(proof,_API._manifest()['runtime_derivations'],'exact runtime provenance derivation')
    old=state['old'];facade=state['facade'];module=types.ModuleType('package_parser_runtime_derivations')
    module.__file__=str(Path(__file__).absolute())
    module.__dict__.update({name:getattr(old,name) for name in
        ('require','same','artifact','regular','safe_relative','load','read','write','identity','sha','phase_module','u8_policy_module','phase_native_identity','verify_map','Rejected')})
    module.__dict__.update(Path=Path,re=re,copy=copy,REPOSITORY=_API.REPOSITORY,CHECKPOINT=_API.CHECKPOINT,AUTHORITY_SHA=_API.AUTHORITY_SHA,
        PHASE_SCOPES=old.PHASE_SCOPES,PHASE_PROFILES=old.PHASE_PROFILES,PHASE_ROLES=old.PHASE_ROLES,
        ENUM_AST_SCHEMA_ADAPTER=copy.deepcopy(old.ENUM_AST_SCHEMA_ADAPTER),verify_build=facade.verify_build,
        package_phase_authority=lambda a:_phase_authority(state,a),
        package_passivity_admission=lambda root,path,profile,a:_passivity_admission(state,root,path,profile,a),
        package_build_finished_ns=lambda profile,control:_build(state,profile,control)[1]['helper_probe']['completed_ns'],
        comparator=lambda:(state['comparator'],state['comparator_proof']))
    # Construct the new module with all explicit callbacks before compiling it.
    # No global on any retained module is replaced or temporarily patched.
    for name,raw in sources.items():exec(compile(raw,str(Path(__file__).absolute())+':'+name,'exec'),module.__dict__)
    return module

def _comparator(state):
    old=state['old'];root=_API.REPOSITORY/_API.FROZEN_PATH/'frozen/comparator'
    raw=_API.file_bytes(root/'comparator.py');derived,proof=_API.derive_comparator(raw)
    decoder=types.ModuleType('package_parser_exact_debug_decoder');decoder.__file__=str(root/'debug_decoder.py')
    exec(compile(_API.file_bytes(root/'debug_decoder.py'),decoder.__file__,'exec'),decoder.__dict__)
    original_import=builtins.__import__
    def isolated_import(name,globals=None,locals=None,fromlist=(),level=0):
        if name=='debug_decoder' and level==0:return decoder
        return original_import(name,globals,locals,fromlist,level)
    module=types.ModuleType('package_source_derived_comparator');module.__file__=str(Path(__file__).absolute())
    module.__dict__.update(__builtins__={**vars(builtins),'__import__':isolated_import},
        package_source_admission=lambda path,contract,approval:_admit_execution(state,path,contract,approval))
    exec(compile(derived,module.__file__+':comparator','exec'),module.__dict__)
    return module,proof

def _build(state,profile,control):
    need(profile in PROFILES and type(control) is bool,'closed runtime build role/profile')
    result=state['builds'][(profile,control)]
    return _API.verify_build(result) if state.get('full_read',False) else _API._selected_build(result)

def _phase_authority(state,a):
    same(a,state['recipe'],'exact admitted successor recipe')
    _selected_session(state)
    original=_API.load(_API._record_at(_API._manifest()['retained_authority']))
    same(a['current']['lexer_phase'],original['lexer_phase'],'unchanged phase semantics and layout authority')

def _selected_session(state):
    need(not state['failed'] and state['thread']==threading.get_ident() and state['pid']==os.getpid(),'live original runtime session owner')
    with _API._preparation_api().checked_scope(state['invocation']):
        same(_API.identity(state['session_path']),state['session_identity'],'selected actual checkpoint')
        same(_API.identity(state['root']/'shared-context.json'),state['session']['shared_context'],'selected actual shared context')
        same(state['session']['host'],state['old'].measured_host(),'selected actual session host')
        _API._tick(state['invocation'])
    return copy.deepcopy(state['session']),state['root']

class _Facade:
    def __init__(self,state):self.state=state;self.REPOSITORY=_API.REPOSITORY
    def __getattr__(self,name):
        need(name in ('require','same','artifact','load','identity','read','write','minimal_env'),'closed public control API')
        return getattr(self.state['old'],name)
    def session_at(self,path,a):
        same(Path(path).absolute(),self.state['session_path'],'runtime exact checkpoint')
        same(a,self.state['recipe'],'runtime exact recipe')
        return _selected_session(self.state)
    def verify_build(self,root,path,profile,a,control=False):
        self.session_at(path,a);same(Path(root),self.state['root'],'runtime exact root')
        return _build(self.state,profile,control)
    def fresh(self,path):
        need(_ACTIVE.get() is self.state,'producer-owned active phase required')
        need(Path(path)==self.state['root']/'u8-policy-controls','fixed u8 control root')
        return self.state['old'].fresh(path)
    def invoke(self,argv,cwd,env,out,host):
        need(_ACTIVE.get() is self.state and self.state['active']=='u8','u8 producer callback only')
        return _invoke(self.state,'parser-u8',argv,cwd,env,out,120,8*1024**2,accepted=(0,))
    def phase_scope(self,a,path,scope,profile,emit=False):
        self.session_at(path,a)
        return self.state['derived'].phase_scope(a,path,scope,profile,emit=emit)
    def phase_control_binding(self,a):return self.state['derived'].phase_control_binding(a)


def create_runtime(a,build_results,contract_root,runtime_inputs):
    with _API._read_scope(a),_API._bounded_phase(a._context.invocation(),600):
        result=_create_runtime(a,build_results,contract_root,runtime_inputs)
        _API._tick(a._context.invocation())
        return result

def _create_runtime(a,build_results,contract_root,runtime_inputs):
    """Issue local runtime authority from all four already completed builds."""
    old,recipe,document=_API._admit(a)
    need(type(build_results) in (tuple,list) and len(build_results)==4,'four live builds required')
    builds={};machine=None;session_path=None;finished=[];invocation=a._context.invocation()
    for result in build_results:
        native,outer=_API.verify_build(result);producer=_API._result(result)
        finished.append(outer['helper_probe']['completed_ns'])
        need(producer['a'] is a and producer['invocation'] is invocation,'same original source/invocation')
        if machine is None:machine=producer['machine'];session_path=producer['session_path']
        need(producer['machine'] is machine and producer['session_path']==session_path,'same machine/checkpoint')
        key=(native['profile'],native['control']);need(key not in builds,'duplicate live build');builds[key]=result
    same(set(builds),{(p,c) for p in PROFILES for c in ROLES},'complete exact four-build set')
    need(len({_API.verify_build(v)[0]['binary']['sha256'] for v in builds.values()})==4,'four distinct actual binaries')
    session,root=_API.session_at(session_path,a)
    need(not any(s['root']==root for s in _CURRENT),'one runtime producer per session')
    _API._machine_api().verify_parser_runtime(runtime_inputs,machine,invocation)
    state=dict(a=a,old=old,recipe=recipe,document=document,builds=builds,machine=machine,invocation=invocation,
        runtime_inputs=runtime_inputs,session_path=session_path,session=session,session_identity=_API.identity(session_path),root=root,thread=threading.get_ident(),pid=os.getpid(),full_read=False,
        contract_root=Path(contract_root).absolute(),stages={},processes={},operation_preservation_errors=[],failed=False,active=None,
        runtime_roster=set(),runtime_dirs=set(),sealed={},sealed_dirs=set(),built_after_ns=max(finished),report={'schema':'oxid-package-source-parser-runtime-v1',
        'status':'NotReady','execution_qualified':False,'ordering':'four actual builds before all parser runtime stages',
        'stages':[]})
    state['facade']=_Facade(state);state['comparator'],state['comparator_proof']=_comparator(state)
    state['contract']=state['comparator'].load_contract(state['contract_root'])
    _contract_closure(state)
    helpers=_helper_probes(builds)
    state['permit']=_API._preparation_api().register_parser_runtime(invocation,session_path,helpers,state['contract_root'])
    state['derived']=_derived(state)
    state['runtime_roster'],state['runtime_dirs']=_full_roster(state)
    handle=object.__new__(RuntimeSession);_LIVE[handle]=state;state['handle']=handle;_CURRENT.append(state)
    _readmit(state)
    return handle


def _helper_probes(builds):
    return [_API._result(builds[(p,c)])['helper'] for c in ROLES for p in PROFILES]


def _contract_closure(state):
    root=state['contract_root'];raw=_API.file_bytes(root/'package-freeze.json')
    same(sha(raw),state['recipe']['freeze_sha256'],'fixed frozen corpus')
    manifest=_API.load(raw);rows=manifest['files'];state['old'].verify_map(root,rows)
    _API._close_tree(root,[r['path'] for r in rows]+['package-freeze.json'])
    _API._tick(state['invocation'])


def _full_roster(state):
    names=set()
    for profile in PROFILES:
        collect='collect-'+profile+'/'
        names.update(collect+x for x in ('driver.stdout','driver.stderr','invocation.json','package-source-collection.json'))
        names.update(collect+'result/'+x for x in ('execution-manifest.json','observations.jsonl','test-roster.stdout','test-roster.stderr'))
        for case in state['contract']['cases']:
            names.update(collect+'result/'+case['id']+'/'+x for x in
                ('request.json','raw.json','stdout.txt','stderr.txt','source.ox','receipt.json','lexer-phase.json'))
        passive='passivity-'+profile+'/'
        names.update(passive+x for x in ('driver.stdout','driver.stderr','invocation.json','package-source-passivity.json','result/report.json'))
        for name,_ in state['recipe']['recipe']['passivity']['cases']:
            names.add(passive+'result/'+name+'.ox')
            for suffix in ('-control','-instrumented'):
                names.update(passive+'result/'+name+suffix+'/'+x for x in ('raw.json','stdout.txt','stderr.txt','lexer-phase.json'))
        for scope in ('collection','passivity','u8_controls'):names.add('lexer-phase/'+scope+'-'+profile+'.json')
    u8=state['old'].u8_policy_module(state['recipe'])
    for profile in PROFILES:
        for control in ROLES:
            for name in u8.CASES:
                prefix='u8-policy-controls/'+profile+('-control-' if control else '-observer-')+name+'/'
                names.update(prefix+x for x in ('source.ox','driver.stdout','driver.stderr','invocation.json','raw.json','lexer-phase.json'))
        prefix='u8-policy-controls/'+profile+'-enabled-public/'
        names.update(prefix+x for x in ('driver.stdout','driver.stderr','invocation.json'))
    names.add('u8-policy-controls/receipt.json')
    driver=state['old'].phase_module(state['recipe'],controls=True)
    for probe,profile,role in driver.roster():
        prefix=driver.process_dir(dict(probe=probe,profile=profile,role=role))+'/'
        names.update(prefix+x for x in ('request.json','stdout','stderr','receipt.json'))
    names.update(('lexer-phase-controls/probe-index.json','package-source-comparison.json'))
    dirs={p.as_posix() for n in names for p in Path(n).parents if p!=Path('.')}
    return names,dirs


def _phase_files(state):
    name=state['active'];all_names=state['runtime_roster']
    if name is None:return set()
    if name.startswith('passivity-'):
        return {n for n in all_names if n.startswith(name+'/') or n=='lexer-phase/'+name+'.json'}
    if name.startswith('collect-'):
        profile=name[len('collect-'):]
        return {n for n in all_names if n.startswith(name+'/') or n=='lexer-phase/collection-'+profile+'.json'}
    if name=='u8':return {n for n in all_names if n.startswith('u8-policy-controls/') or n.startswith('lexer-phase/u8_controls-')}
    if name=='phase':return {n for n in all_names if n.startswith('lexer-phase-controls/')}
    if name=='compare':return {'package-source-comparison.json'}
    raise Rejected('unknown runtime phase')

def closure(root):
    """Exact phase-appropriate extension; no premature files or empty dirs."""
    candidates=[s for s in _CURRENT if s['root']==Path(root)]
    if not candidates:return [],[]
    same(len(candidates),1,'single runtime producer for session')
    state=candidates[0];need(not state['failed'],'failed runtime tree cannot be readmitted')
    active=_ACTIVE.get() is state
    allowed=set(state['sealed']) | (_phase_files(state) if active else set())
    allowed_dirs={p.as_posix() for n in allowed for p in Path(n).parents if p!=Path('.')}
    names=[];dirs=[]
    tops={Path(n).parts[0] for n in state['runtime_roster']}
    for top in sorted(tops):
        first=state['root']/top
        if not first.exists() and not first.is_symlink():continue
        paths=[first,*first.rglob('*')] if first.is_dir() and not first.is_symlink() else [first]
        for path in paths:
            name=path.relative_to(state['root']).as_posix()
            _API._tick(state['invocation']);need(not path.is_symlink(),'runtime symlink')
            if path.is_dir():
                need(name in allowed_dirs,'unexpected or premature runtime directory');dirs.append(name)
            else:
                need(name in allowed and path.is_file() and path.stat().st_size<=128*1024**2,'unexpected or unbounded runtime file')
                names.append(name)
                if name in state['sealed']:same(_API.identity(path),state['sealed'][name],'sealed runtime file drift')
    need(set(state['sealed'])<=set(names),'missing sealed runtime member')
    if not active:
        same(set(names),set(state['sealed']),'unsealed runtime evidence')
        same(set(dirs),state['sealed_dirs'],'runtime directory drift')
    return names,dirs

def _seal(state):
    names,dirs=closure(state['root'])
    same(set(names),set(state['sealed'])|_phase_files(state),'complete exact current-phase roster')
    state['sealed']={name:_API.identity(state['root']/name) for name in names}
    state['sealed_dirs']=set(dirs)
    expected=set()
    for phase in (*state['stages'],state['active']):
        expected.update(_process_roster(state,phase))
    same(set(state['processes']),expected,'complete independently prescribed actual process roster')


def _process_roster(state,phase):
    root=state['root']
    if phase.startswith(('passivity-','collect-')):return {str(root/phase)}
    if phase=='compare':return set()
    if phase=='u8':
        cases=state['old'].u8_policy_module(state['recipe']).CASES
        return {str(root/'u8-policy-controls'/(p+('-control-' if c else '-observer-')+name)) for p in PROFILES for c in ROLES for name in cases}|{str(root/'u8-policy-controls'/(p+'-enabled-public')) for p in PROFILES}
    if phase=='phase':
        driver=state['old'].phase_module(state['recipe'],controls=True)
        return {str(root/driver.process_dir(dict(probe=n,profile=p,role=r))) for n,p,r in driver.roster()}
    raise Rejected('unknown process roster phase')


def _readmit(state):
    with _API._bounded_phase(state['invocation'],600),_API._read_scope(state['a']):
        need(not state['full_read'],'nonrecursive full parser readmission')
        state['full_read']=True
        try:
            result=_readmit_impl(state);_API._tick(state['invocation']);return result
        except BaseException:
            state['failed']=True;_API._poison_authority(state['a']);raise
        finally:state['full_read']=False

def _readmit_impl(state):
    need(not state['failed'],'runtime producer previously failed')
    _API._admit(state['a']);_API.session_at(state['session_path'],state['a'])
    _API._machine_api().verify_parser_runtime(state['runtime_inputs'],state['machine'],state['invocation'])
    for profile in PROFILES:
        for control in ROLES:_build(state,profile,control)
    _contract_closure(state)
    if state['active'] is None:
        expected=set().union(*(_process_roster(state,name) for name in state['stages']))
        same(set(state['processes']),expected,'exact readmitted actual process roster')
    for process in state['processes'].values():_verify_process(state,process)
    return True

@contextlib.contextmanager
def _phase(state,name,seconds):
    _readmit(state);need(state['active'] is None and name not in state['stages'],'fresh serial runtime phase')
    state['active']=name;token=_ACTIVE.set(state)
    try:
        with _API._bounded_phase(state['invocation'],seconds):yield
        _seal(state);state['stages'][name]=True
        state['report']['stages'].append(name)
    except BaseException as error:
        try:_failure_evidence(state,name,error)
        except BaseException as failure:error.add_note('Runtime failure evidence incomplete: '+repr(failure)[:1000])
        state['failed']=True;_API._poison_authority(state['a'])
        error.add_note('Parser runtime phase failed; live admission closed: '+name)
        raise
    finally:
        state['active']=None;_ACTIVE.reset(token)
    _readmit(state)


def _failure_evidence(state,name,error):
    attempts=[];started=time.time_ns()
    checks=(('source',lambda:_API._admit(state['a'])),
        ('copied-source',lambda:_API._verify_views(state['root'],state['old'],state['recipe'],state['document'],verify_outputs=False)),
        ('runtime-machine',lambda:_API._machine_api().verify_parser_runtime(state['runtime_inputs'],state['machine'],state['invocation'])),
        ('four-builds',lambda:[_build(state,p,c) for p in PROFILES for c in ROLES]),
        ('issued-processes',lambda:[_verify_process(state,p) for p in state['processes'].values()]))
    try:
        with _API._bounded_phase(state['invocation'],60):
            for label,check in checks:
                try:
                    _API._tick(state['invocation']);check();_API._tick(state['invocation'])
                    attempts.append(dict(name=label,status='preserved'))
                except BaseException as failure:attempts.append(dict(name=label,status='unverified',error=repr(failure)[:1000]))
    except BaseException as failure:
        attempts.extend(dict(name=label,status='unverified',error=repr(failure)[:1000]) for label,_ in checks if label not in {x['name'] for x in attempts})
    receipt=dict(schema='oxid-package-source-parser-runtime-failure-v1',status='NotReady',execution_qualified=False,
        phase=name,original_error=repr(error)[:2000],started_ns=started,completed_ns=time.time_ns(),
        invocation_nonce=state['invocation'].document()['nonce'],preservation=attempts,
        issued_processes=[p['document'] for p in state['processes'].values()],post_readback_failures=list(state.get('operation_preservation_errors',[])))
    raw=serial(receipt);need(len(raw)<=1024**2,'bounded terminal runtime evidence')
    with (state['root']/'package-source-runtime-failure.json').open('xb') as f:f.write(raw)


def _verify_process(state,process):
    with _API._preparation_api().checked_scope(state['invocation']):
        return _verify_process_body(state,process)

def _verify_process_body(state,process):
    probe=process['probe'];d=_API._preparation_api().verify_probe(probe,state['invocation'],
        argv=process['argv'],cwd=process['cwd'],environment=process['environment'],
        timeout_seconds=process['timeout'],stream_limit_bytes=process['limit'],
        operation_kind=process['kind'],runtime_permit=state['permit'])
    same(d,process['document'],'actual retained runtime process')
    need(type(d['status']) is int and d['status'] in process['accepted'],'exact admitted runtime exit')
    need(d['cleanup_complete'] and d['cleanup_error'] is None and not d['failure'] and not d['timed_out'] and not d['stream_limit_exceeded'],'complete bounded runtime cleanup')
    same(d['invocation_nonce'],state['invocation'].document()['nonce'],'runtime invocation nonce')
    need(state['built_after_ns']<=d['started_ns']<=d['completed_ns'],'runtime follows all four builds')
    if 'projection' in process:
        row=process['projection'];same(_API.identity(row['path']),row,'runtime process projection drift')
        receipt=_API.load(_API.file_bytes(row['path']))
        same(receipt,process['receipt'],'exact actual runtime process projection')
        for key,data in (('stdout',probe.stdout()),('stderr',probe.stderr())):
            same(_API.identity(receipt[key]['path']),receipt[key],'runtime copied stream identity')
            same(_API.file_bytes(receipt[key]['path']),data,'runtime copied actual stream bytes')
    return d


def _operation_selection(state,kind,argv):
    if kind in ('parser-collect','parser-passivity'):
        semantic='collection' if kind=='parser-collect' else 'passivity'
        for profile in PROFILES:
            if argv==_helper_args(state,semantic,profile)[1]:
                return [(profile,False)] if semantic=='collection' else [(profile,False),(profile,True)]
        raise Rejected('exact fixed parser helper route required')
    need(kind in ('parser-u8','parser-phase'),'closed parser selected operation')
    matches=[key for key,value in state['builds'].items() if value.document()['binary']['path']==argv[0]]
    same(len(matches),1,'one original actual binary route');profile,control=matches[0]
    if kind=='parser-phase':
        tail=['frontend::parser::unit4_observer::lexer_phase::phase_controls','--exact','--ignored','--nocapture','--test-threads=1']
        same(argv[1:],tail,'exact phase selected route')
    else:
        observer=['--exact','frontend::parser::unit4_observer::observe_request','--ignored','--nocapture']
        public=['--exact','frontend::parser::u8_syntax_tests::u8_named_conversion_has_a_real_child_and_exact_trivia_origins_on_every_mode','--nocapture']
        need(argv[1:]==observer or (control and argv[1:]==public),'exact u8 selected route')
    return [(profile,control)]

def _selected_operation(state,kind,argv):
    need(state['thread']==threading.get_ident() and state['pid']==os.getpid(),'original runtime read owner')
    with _API._preparation_api().checked_scope(state['invocation']):
        _API._machine_api().verify_parser_runtime_selected(state['runtime_inputs'],state['machine'],state['invocation'])
        for key in _operation_selection(state,kind,argv):
            _API._selected_build(state['builds'][key]);_API._selected_source(state['builds'][key])
        _API._tick(state['invocation'])

def _preserve_after_failure(state,kind,argv,error):
    try:_selected_operation(state,kind,argv)
    except BaseException as preservation:
        message='Selected parser post-operation preservation failed: '+repr(preservation)[:1000]
        state.setdefault('operation_preservation_errors',[]).append(message)
        error.add_note(message)

def _invoke(state,kind,argv,cwd,env,out,timeout,limit,accepted,project=True):
    need(_ACTIVE.get() is state,'active runtime producer required')
    same(Path(cwd),state['root'],'runtime cwd');same(state['old'].measured_host(),state['session']['host'],'runtime actual host')
    _selected_operation(state,kind,argv)
    try:
        probe=_API._preparation_api().owned_probe(state['invocation'],argv,str(cwd),env,
            timeout_seconds=timeout,stream_limit_bytes=limit,operation_kind=kind,runtime_permit=state['permit'])
    except BaseException as error:
        _preserve_after_failure(state,kind,argv,error)
        raise
    doc=probe.document();record=dict(probe=probe,document=doc,argv=list(argv),cwd=str(cwd),environment=dict(env),
        timeout=timeout,limit=limit,kind=kind,accepted=tuple(accepted))
    key=str(Path(out));need(key not in state['processes'],'unique actual process directory')
    state['processes'][key]=record
    try:_verify_process(state,record)
    except BaseException as error:
        _preserve_after_failure(state,kind,argv,error)
        raise
    else:_selected_operation(state,kind,argv)
    # A new-schema direct projection of an actual owned process, not an old
    # invoke() receipt. Retained explicit control APIs consume its common fields.
    if not project:return doc
    out=Path(out)
    (out/'driver.stdout').write_bytes(probe.stdout());(out/'driver.stderr').write_bytes(probe.stderr())
    receipt=dict(schema='oxid-package-source-parser-process-v1',argv=list(argv),cwd=str(cwd),environment=dict(env),
        started_ns=doc['started_ns'],finished_ns=doc['completed_ns'],exit_code=doc['status'],
        stdout=_API.identity(out/'driver.stdout'),stderr=_API.identity(out/'driver.stderr'),host=state['session']['host'],
        owned_process=doc)
    (out/'invocation.json').write_bytes(serial(receipt))
    record['projection']=_API.identity(out/'invocation.json');record['receipt']=copy.deepcopy(receipt)
    return receipt

def _helper_args(state,kind,profile):
    root=state['root'];path=str(state['session_path']);out=root/(('collect-' if kind=='collection' else 'passivity-')+profile)
    if kind=='collection':
        argv=[sys.executable,'-B',str(root/'helpers/run.py'),'--build-receipt',str(root/('build-'+profile)/'result/build-receipt.json'),
            '--contract-dir',str(state['contract_root']),'--checkpoint',path,'--output',str(out/'result')]
    else:
        argv=[sys.executable,'-B',str(root/'helpers/passivity.py'),'--instrumented',str(root/('build-'+profile)/'result/build-receipt.json'),
            '--control',str(root/('build-control-'+profile)/'result/build-receipt.json'),'--checkpoint',path,'--output',str(out/'result')]
    return out,argv

def _stage_admission(state,kind,profile):
    need(profile in PROFILES,'exact runtime profile')
    phase=('collect-' if kind=='collection' else 'passivity-')+profile
    need(phase in state['stages'] or (_ACTIVE.get() is state and state['active']==phase),'actual issued runtime stage')
    out,argv=_helper_args(state,kind,profile)
    process=state['processes'].get(str(out));need(process is not None,'original helper ProbeRecord required')
    _verify_process(state,process);same(process['argv'],argv,'fixed genuine helper command')
    same(process['environment'],state['old'].minimal_env(state['root']/'home'),'fixed clean helper environment')
    envelope=_API.load(_API.file_bytes(out/('package-source-'+kind+'.json')))
    same(envelope['schema'],'oxid-package-source-parser-'+kind+'-v1','distinct runtime envelope schema')
    same(envelope['profile'],profile,'runtime envelope profile')
    same(envelope['session'],_API.identity(state['session_path']),'runtime checkpoint')
    same(envelope['invocation'],process['projection'],'actual new-schema process projection')
    same(envelope['owned_process'],process['document'],'actual original supervisor process')
    key,filename=('manifest','execution-manifest.json') if kind=='collection' else ('report','report.json')
    same(envelope[key],_API.identity(out/'result'/filename),'actual native helper receipt')
    summary=(_API.load(process['probe'].stdout()))
    wanted=(dict(status='collected',cases=248,observations=319,manifest=str(out/'result'/filename)) if kind=='collection' else dict(status='pass',pairs=6,report=str(out/'result'/filename)))
    same(summary,wanted,'actual frozen helper stdout summary');same(process['probe'].stderr(),b'','actual frozen helper stderr')
    if kind=='collection':same(envelope['contract_dir'],str(state['contract_root']),'exact contract root')
    return envelope

def _passivity_admission(state,root,path,profile,a):
    same(Path(root),state['root'],'passivity root');same(Path(path),state['session_path'],'passivity checkpoint')
    same(a,state['recipe'],'passivity admitted recipe')
    envelope=_stage_admission(state,'passivity',profile)
    return envelope,[_build(state,profile,c) for c in ROLES],state['session']

def passivity(handle,profile):
    state=_live(handle);need(profile in PROFILES,'fixed passivity profile')
    expected='passivity-'+profile
    _order(state,expected)
    with _phase(state,expected,1200):
        out,argv=_helper_args(state,'passivity',profile);state['old'].fresh(out)
        _invoke(state,'parser-passivity',argv,state['root'],state['old'].minimal_env(state['root']/'home'),out,900,8*1024**2,(0,))
        envelope=dict(schema='oxid-package-source-parser-passivity-v1',session=_API.identity(state['session_path']),profile=profile,
            invocation=_API.identity(out/'invocation.json'),owned_process=state['processes'][str(out)]['document'],
            report=_API.identity(out/'result/report.json'))
        (out/'package-source-passivity.json').write_bytes(serial(envelope))
        state['derived'].phase_scope(state['recipe'],state['session_path'],'passivity',profile,emit=True)
        state['derived'].verify_passivity(state['root'],state['session_path'],profile,state['recipe'],set())
    return _API.identity(out/'package-source-passivity.json')

def collect(handle,profile):
    state=_live(handle);need(profile in PROFILES,'fixed collection profile');expected='collect-'+profile
    _order(state,expected)
    with _phase(state,expected,2100):
        out,argv=_helper_args(state,'collection',profile);state['old'].fresh(out)
        _invoke(state,'parser-collect',argv,state['root'],state['old'].minimal_env(state['root']/'home'),out,1800,8*1024**2,(0,))
        envelope=dict(schema='oxid-package-source-parser-collection-v1',session=_API.identity(state['session_path']),profile=profile,
            invocation=_API.identity(out/'invocation.json'),owned_process=state['processes'][str(out)]['document'],
            manifest=_API.identity(out/'result/execution-manifest.json'),contract_dir=str(state['contract_root']))
        (out/'package-source-collection.json').write_bytes(serial(envelope))
        state['derived'].phase_scope(state['recipe'],state['session_path'],'collection',profile,emit=True)
        # Re-run the unchanged frozen raw normalization and roster suffix now.
        actual,rows,_=state['comparator'].execution_manifest(out/'result/execution-manifest.json',state['contract'],
            {'runtime':handle,'nonces':set()})
        same(actual,profile,'actual collected profile');same(len(rows),319,'complete collected modes')
    return _API.identity(out/'package-source-collection.json')

def _admit_execution(state,path,contract,approval):
    need(type(approval) is dict and set(approval)=={'runtime','nonces'} and approval['runtime'] is state['handle']
        and type(approval['nonces']) is set,'explicit live comparator approval')
    same(contract,state['contract'],'exact frozen contract')
    manifest=_API.load(_API.file_bytes(path));profile=manifest['profile'];envelope=_stage_admission(state,'collection',profile)
    root=state['root'];out=root/('collect-'+profile)
    same(Path(path),out/'result/execution-manifest.json','actual collection manifest path')
    same(manifest['schema'],'oxid-unit4-parser-execution-v1','genuine frozen collection schema')
    same(manifest['status'],'collected','complete original collection')
    same(manifest['contract_decoded_sha256'],state['recipe']['contract_sha256'],'original execution contract')
    same(manifest['package_freeze_sha256'],state['recipe']['freeze_sha256'],'original package freeze')
    state['old'].artifact(manifest['authority_checkpoint'],state['session_path'])
    build,build_outer=_build(state,profile,False)
    same(manifest['build_receipt'],build_outer['receipt'],'actual original build receipt')
    host=manifest['host_runtime'];state['old'].check_host(host,state['recipe'])
    same(host,state['session']['host'],'actual collection/session host')
    same(host,state['old'].measured_host(),'actual local collection host')
    for key,name in (('driver','run.py'),('normalizer','parse_debug.py')):
        state['old'].artifact(manifest[key],root/'helpers'/name)
    same(manifest['case_count'],248,'full actual case count');same(manifest['observation_count'],319,'full actual mode count')
    same(manifest['requested_case_ids'],[c['id'] for c in contract['cases']],'full fixed corpus without subsets')
    same([r['case_id'] for r in manifest['case_receipts']],manifest['requested_case_ids'],'full actual case order')
    inventory={'execution-manifest.json','observations.jsonl','test-roster.stdout','test-roster.stderr'}
    for row in manifest['case_receipts']:
        case=row['case_id'];state['old'].safe_relative(case);need('/' not in case,'single-component case path')
        nonce=row['execution_id'];need(nonce not in approval['nonces'],'cross-profile/case repeated execution nonce')
        approval['nonces'].add(nonce);directory=out/'result'/case
        for key,name in (('request','request.json'),('raw','raw.json'),('stdout','stdout.txt'),('stderr','stderr.txt')):
            state['old'].artifact(row[key],directory/name)
        request=state['old'].load(state['old'].artifact(row['request']))
        state['old'].artifact(request['source'],directory/'source.ox')
        same(_API.load(_API.file_bytes(directory/'receipt.json')),row,'actual on-disk case receipt')
        inventory.update(case+'/'+n for n in ('request.json','raw.json','stdout.txt','stderr.txt','source.ox','receipt.json','lexer-phase.json'))
    _API._close_tree(out/'result',sorted(inventory))
    same(manifest['observations']['path'],str(out/'result/observations.jsonl'),'actual normalized output path')
    state['derived'].phase_scope(state['recipe'],state['session_path'],'collection',profile)
    return manifest,host,build,profile


def execute_u8_controls(handle):
    state=_live(handle);_order(state,'u8')
    with _phase(state,'u8',3000):
        module=state['old'].u8_policy_module(state['recipe'])
        module.execute(state['facade'],state['session_path'],state['recipe'])
        binaries={(p,c):_build(state,p,c)[0]['binary'] for p in PROFILES for c in ROLES}
        receipt=_API.load(_API.file_bytes(state['root']/'u8-policy-controls/receipt.json'))
        module.verify(state['facade'],receipt,state['old'].artifact,binaries,state['session'])
        module.phase_receipts(state['facade'],state['session_path'],state['recipe'])
    return _API.identity(state['root']/'u8-policy-controls/receipt.json')


def execute_phase_controls(handle):
    state=_live(handle);_order(state,'phase')
    with _phase(state,'phase',2400):
        driver=state['old'].phase_module(state['recipe'],controls=True)
        binding=state['derived'].phase_control_binding(state['recipe'])
        same(binding['driver'],state['recipe']['current']['lexer_phase']['controls']['driver'],'retained original phase driver')
        state['old'].verify_map(_API.REPOSITORY,[binding['driver']])
        session_sha=_API.identity(state['session_path'])['sha256'];binaries={}
        after=max(_build(state,p,c)[1]['helper_probe']['completed_ns'] for p in PROFILES for c in ROLES)
        for profile in PROFILES:
            for control in ROLES:
                role='not_observed' if control else 'observed';binary=_build(state,profile,control)[0]['binary']
                binaries[(profile,role)]=state['old'].phase_native_identity(binary,root=state['root'])
        same(len({r['sha256'] for r in binaries.values()}),4,'four actual distinct phase binaries')
        approved=binding['approved_layouts']
        layouts={b['sha256']:approved[p+'-'+r] for (p,r),b in binaries.items()}
        requests=[]
        for probe,profile,role in driver.roster():
            request=dict(schema=driver.REQUEST_SCHEMA,nonce=uuid.uuid4().hex,probe=probe,profile=profile,role=role,
                session_sha256=session_sha,authority_sha256=_API.AUTHORITY_SHA,driver_sha256=binding['driver']['sha256'],
                binary=binaries[(profile,role)],entrypoint=driver.ENTRYPOINT,expected=driver.expected(probe,profile,role))
            driver.validate_request(driver.canonical_bytes(request));requests.append(request)
        driver.validate_catalogue(requests,session_sha256=session_sha,authority_sha256=_API.AUTHORITY_SHA,
            driver_sha256=binding['driver']['sha256'],binaries=binaries)
        root=state['root'];state['old'].fresh(root/driver.ROOT_NAME);receipts=[]
        for request in requests:
            _API._tick(state['invocation']);directory=driver.process_dir(request);work=root/directory;work.mkdir()
            request_identity=driver.write_new(root,directory+'/request.json',driver.canonical_bytes(request),driver.REQUEST_LIMIT)
            argv,cwd,env=driver.inputs(root,request);supplied=driver.input_hashes(argv,cwd,env)
            _invoke(state,'parser-phase',argv,cwd,env,work,driver.TIMEOUT_SECONDS,driver.STREAM_LIMIT,
                (request['expected']['exit_code'],),project=False)
            process=state['processes'][str(work)];doc=process['document'];probe=process['probe']
            stdout,stderr=probe.stdout(),probe.stderr()
            stdout_identity=driver.write_new(root,directory+'/stdout',stdout,driver.STREAM_LIMIT)
            stderr_identity=driver.write_new(root,directory+'/stderr',stderr,driver.STREAM_LIMIT)
            status,boundary,count,layout,failure=driver.validate_probe_output(request,stdout,stderr,doc['status'],
                approved_layout=layouts[request['binary']['sha256']])
            same(supplied,driver.input_hashes(*driver.inputs(root,request)),'actual phase inputs preserved')
            receipt=dict(schema=driver.RECEIPT_SCHEMA,**{k:request[k] for k in ('nonce','probe','profile','role')},
                request=request_identity,status=status,exit_code=doc['status'],boundary=boundary,
                started_ns=doc['started_ns'],finished_ns=doc['completed_ns'],timed_out=doc['timed_out'],
                stream_limit_exceeded=doc['stream_limit_exceeded'],**supplied,stdout=stdout_identity,stderr=stderr_identity,
                marker_count=count,layout=layout,failure=failure)
            raw=driver.canonical_bytes(receipt)
            driver.validate_receipt(raw,request,stdout,stderr,root,approved_layout=layouts[request['binary']['sha256']],prepared_ns=after)
            need(status!='failed','actual complete phase probe failed')
            receipts.append(driver.write_new(root,directory+'/receipt.json',raw,driver.RECEIPT_LIMIT))
            after=doc['completed_ns']
        index=dict(schema=driver.INDEX_SCHEMA,session_sha256=session_sha,authority_sha256=_API.AUTHORITY_SHA,
            driver_sha256=binding['driver']['sha256'],counts=dict(driver.COUNTS),receipts=receipts)
        driver.write_new(root,driver.ROOT_NAME+'/probe-index.json',driver.canonical_bytes(index),driver.INDEX_LIMIT)
        state['derived'].phase_evidence(state['recipe'],state['session_path'])
    return _API.identity(state['root']/'lexer-phase-controls/probe-index.json')


def compare(handle):
    state=_live(handle);_order(state,'compare')
    with _phase(state,'compare',600):
        c=state['comparator'];a=state['recipe'];root=state['root'];contract=c.load_contract(state['contract_root'])
        effective,effective_receipt=c.admit_contract_amendment(contract,state['contract_root'],state['old'].effective_authority(a))
        same(effective_receipt['effective_contract_identity'],a['effective_contract_identity'],'unchanged historical coordinate amendment')
        same(effective_receipt['effective_parser_document_canonical_sha256'],a['effective_document_canonical_sha256'],'unchanged effective document')
        approval={'runtime':handle,'nonces':set()}
        passive=[state['derived'].verify_passivity(root,state['session_path'],p,a,approval['nonces']) for p in PROFILES]
        rows=[];identities={}
        for profile in PROFILES:
            actual,observed,bound=c.execution_manifest(root/('collect-'+profile)/'result/execution-manifest.json',contract,approval)
            same(actual,profile,'complete comparison profile order');rows.extend(observed);identities[profile]=bound
        same(len(rows),638,'all unchanged parser observations')
        need(identities['debug']['binary_sha256']!=identities['release']['binary_sha256'],'actual separate profile executables')
        unadapted=c.compare_effective_rows(effective,effective_receipt,rows,identities)
        projected,projection=state['derived'].project_enum_observations(a,rows)
        frozen=c.compare_effective_rows(effective,effective_receipt,projected,identities)
        current,current_receipt=state['old'].current_parser_contract(a,effective,effective_receipt)
        current_result=c.compare_rows(current,projected,identities);current_result.update(current_receipt)
        u8=state['old'].u8_policy_module(a);control_path=root/'u8-policy-controls/receipt.json'
        controls=_API.load(_API.file_bytes(control_path));binaries={(p,q):_build(state,p,q)[0]['binary'] for p in PROFILES for q in ROLES}
        u8.verify(state['facade'],controls,state['old'].artifact,binaries,state['session'])
        same(controls['session'],_API.identity(state['session_path']),'u8 exact successor checkpoint')
        same(controls['closed_policy'],a['current']['u8_closed_policy'],'unchanged closed policy')
        u8.phase_receipts(state['facade'],state['session_path'],a)
        phase=state['derived'].phase_evidence(a,state['session_path'])
        result=dict(schema='oxid-package-source-parser-comparison-v1',status='NotReady',execution_qualified=False,
            local_semantics_status=current_result['status'],session=_API.identity(state['session_path']),
            source_manifest=state['document']['current_source_manifest'],authority_sha256=_API.AUTHORITY_SHA,
            comparator_derivation=state['comparator_proof'],runtime_derivations=_API._manifest()['runtime_derivations'],
            ordinary_passivity=passive,unadapted_frozen_comparison=unadapted,
            unadapted_frozen_comparison_canonical_sha256=c.sha(c.canonical(unadapted)),
            frozen_comparison=frozen,frozen_comparison_canonical_sha256=c.sha(c.canonical(frozen)),
            current_comparison=current_result,current_comparison_canonical_sha256=c.sha(c.canonical(current_result)),
            enum_structural_projection=projection,u8_policy_controls=_API.identity(control_path),
            lexer_phase_envelopes=phase['envelopes'],lexer_phase_controls=phase['probe_index'],
            evidence_scope='fresh local execution; four-host public coverage and exact-head CI join remain separately mandatory')
        (root/'package-source-comparison.json').write_bytes(serial(result))
        state['report']['comparison']=_API.identity(root/'package-source-comparison.json')
        state['report']['local_semantics_status']=current_result['status']
    return copy.deepcopy(result)

SEQUENCE=('passivity-debug','collect-debug','passivity-release','collect-release','u8','phase','compare')
def _order(state,wanted):
    same(state['report']['stages'],list(SEQUENCE[:len(state['stages'])]),'exact new runtime sequence')
    same(SEQUENCE[len(state['stages'])] if len(state['stages'])<len(SEQUENCE) else None,wanted,'exact next runtime stage')

def complete(handle):
    state=_live(handle);need(not state['stages'],'complete producer requires fresh runtime handle')
    for profile in PROFILES:passivity(handle,profile);collect(handle,profile)
    execute_u8_controls(handle);execute_phase_controls(handle)
    return compare(handle)

def verify_runtime(handle):
    state=_live(handle);need(state['active'] is None,'partial runtime cannot be admitted')
    same(state['report']['stages'],list(SEQUENCE),'all parser runtime stages required')
    _readmit(state)
    state['derived'].phase_evidence(state['recipe'],state['session_path'])
    need(state['report']['local_semantics_status']=='pass','current local comparison failed')
    return copy.deepcopy(state['report'])
