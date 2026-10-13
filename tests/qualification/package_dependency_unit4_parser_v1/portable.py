"""Distinct package-source parser consumer. Global qualification remains NotReady.

Installed inputs have fixed paths. No original module globals are patched. The
retained source-independent build verifier remains the full Cargo/tool/artifact
predicate; a new checkpoint and role maps are admitted before calling it.
"""
from __future__ import annotations
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys
import time
import os
import stat
import types
import weakref
import contextlib
import contextvars
import threading

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
AUTHORITY_SHA = 'e52b9c0f41aef4565a0ce2381e0750837222edd3a6a14d829c70a7577e334911'
MACHINE_MANIFEST_SHA256 = '1661e0efc94e43410acf74d011f98cb52be76cac5f5904cb658e410e0bbec03a'
SHARED_MANIFEST_SHA256 = 'fa910b15e2008451da83f1cebc6452ff3617b34e1c5a04be85dfe558be8e93fa'
OLD_PATH = 'tests/qualification/unit4_parser_current/portable.py'
SOURCE_DIR = 'tests/qualification/package_dependency_unit4_source_v1/'
CONTEXT_PATH = 'tests/qualification/package_dependency_unit4_current/context.py'
FROZEN_PATH = 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/'
SESSION_SCHEMA = 'oxid-package-source-parser-session-v1'
CHECKPOINT = 'package-source-session.json'

class Rejected(ValueError):
    pass

class NotReady(RuntimeError):
    pass

def require(ok, message):
    if not ok:
        raise Rejected(message)

def same(got, wanted, message):
    require(got == wanted, message)

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def serial(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False)+'\n').encode()

def load(raw):
    def pairs(items):
        value = {}
        for k, v in items:
            require(k not in value, 'duplicate JSON key')
            value[k] = v
        return value
    return json.loads(raw, object_pairs_hook=pairs,
        parse_constant=lambda _: (_ for _ in ()).throw(Rejected('nonfinite JSON')))

def file_bytes(path):
    path = Path(path).absolute()
    require(path.is_file() and not any(p.is_symlink() for p in (path, *path.parents)), 'regular file required')
    return path.read_bytes()

def identity(path):
    path = Path(path).absolute(); raw = file_bytes(path)
    return dict(path=str(path), bytes=len(raw), sha256=sha(raw))

def _record_at(row, root=REPOSITORY):
    require(set(row) == {'path','bytes','sha256'}, 'closed identity')
    name = row['path']; path = Path(name)
    require(not path.is_absolute() and path.as_posix() == name and '..' not in path.parts, 'relative installed identity')
    raw = file_bytes(root/path)
    same(dict(path=name, bytes=len(raw), sha256=sha(raw)), row, 'installed identity changed: '+name)
    return raw

def _manifest():
    raw = file_bytes(HERE/'authority.json')
    same(sha(raw), AUTHORITY_SHA, 'exact parser consumer authority')
    manifest=load(raw)
    same(sorted(p.name for p in HERE.iterdir()),manifest['initial_closed_roster'],'closed parser package')
    for name in manifest['initial_closed_roster']:
        file_bytes(HERE/name)
    return manifest

def _module(name, path, raw):
    # Separate instance, fixed actual __file__; never touch retained globals.
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module

def _canonical_owner(directory, manifest_pin, schema, files, entry, key):
    require(isinstance(manifest_pin,str),'consumer dependency awaits exact review pin')
    raw=file_bytes(directory/'package-manifest.json')
    same(sha(raw),manifest_pin,'exact consumer dependency package')
    manifest=load(raw)
    same(manifest['schema'],schema,'consumer dependency schema')
    same([r['path'] for r in manifest['files']],sorted(files),'closed dependency roster')
    same(sorted(p.name for p in directory.iterdir()),sorted((*files,'package-manifest.json')),'dependency directory closure')
    bodies={}
    for row in manifest['files']:
        bodies[row['path']]=_record_at(row,directory)
    path=directory/entry;digest=sha(bodies[entry]);module=sys.modules.get(key)
    if module is not None:
        same(getattr(module,'__file__',None),str(path),'canonical dependency owner path')
        same(getattr(module,'__source_sha256__',None),digest,'canonical dependency owner bytes')
        return module
    module=types.ModuleType(key)
    module.__file__,module.__source_sha256__=str(path),digest
    sys.modules[key]=module
    try:
        exec(compile(bodies[entry],str(path),'exec'),module.__dict__)
    except BaseException:
        if sys.modules.get(key) is module:del sys.modules[key]
        raise
    return module

def _preparation_api():
    return _canonical_owner(REPOSITORY/Path(CONTEXT_PATH).parent,SHARED_MANIFEST_SHA256,
        'oxid-package-dependency-unit4-current-package-v1',
        ('context.py','preparation.py','prepare_worker.py','test_context.py','test_preparation.py'),
        'preparation.py','_oxid_package_dependency_unit4_preparation_v1')

def _context_api():
    return _preparation_api().context_api()

def _machine_api():
    return _canonical_owner(HERE.parent/'package_dependency_unit4_machine_v1',MACHINE_MANIFEST_SHA256,
        'oxid-package-dependency-unit4-machine-package-v1',('machine.py','test_machine.py'),
        'machine.py','_oxid_package_dependency_unit4_machine_v1')

class AdmittedAuthority:
    __slots__ = ('_context', '_document', '_recipe', '_old', '_seal')
    def __init__(self, context, document, recipe, old, seal):
        require(seal is _AUTHORITY_TOKEN, 'use authority(context)')
        object.__setattr__(self,'_context',context)
        object.__setattr__(self,'_document',serial(document))
        object.__setattr__(self,'_recipe',serial(recipe))
        object.__setattr__(self,'_old',old)
        object.__setattr__(self,'_seal',seal)
    def __setattr__(self,*args):
        raise TypeError('immutable parser authority')
    def document(self):
        return load(self._document)
    def recipe(self):
        return load(self._recipe)

_AUTHORITY_TOKEN = object()

def authority(context):
    with _bounded_phase(context.invocation(),120):
        return _authority(context)

def _authority(context):
    c = _context_api(); c.verify_context(context)
    document = context.document(); manifest = _manifest()
    same(document['status'], 'NotReady', 'source-only context')
    same(document['execution_qualified'], False, 'no inherited qualification')
    oldraw = _record_at(manifest['retained_portable'])
    same(manifest['retained_portable']['path'], OLD_PATH, 'fixed old API')
    old = _module('package_parser_retained_build_primitives', REPOSITORY/OLD_PATH, oldraw)
    historical = load(_record_at(manifest['historical_authority']))
    active = load(_record_at(manifest['retained_authority']))
    successor = load(_record_at(manifest['source_authority']))
    candidate_raw = _record_at(manifest['candidate_manifest'])
    same(document['current_source_manifest'], successor['current_source_manifest'], 'exact execution source')
    same(document['source_checkpoint'], successor['source_checkpoint'], 'separate source checkpoint')
    same(document['predecessor_source_manifest'], successor['predecessor_source_manifest'], 'retained source authority')
    old.verify_map(REPOSITORY/FROZEN_PATH, historical['package_files'])
    old.verify_map(REPOSITORY/FROZEN_PATH/'frozen/helpers', historical['helper_files'], exact=True)
    for row in manifest['retained_inputs']:
        _record_at(row)
    original = copy.deepcopy(active)
    for role, field in (('parser_observed','current_derived_files'), ('parser_control','current_control_derived_files')):
        same(document['views'][role]['files'], successor['maps'][field], 'exact parser role map')
        same(len(document['views'][role]['files']), 542, 'complete parser role count')
    for field, rows in successor['maps'].items():
        require(field in ('current_base_files','current_derived_files','current_control_derived_files'), 'closed map projection')
        active[field] = copy.deepcopy(rows)
    active['current_candidate_source_manifest_sha256'] = sha(candidate_raw)
    active['current_source_manifest'] = copy.deepcopy(successor['current_source_manifest'])
    active['reviewed_source_head'] = successor['source_checkpoint']['head']
    active['source_only_tree'] = successor['source_checkpoint']['tree']
    changed = {k for k in active if active[k] != original[k]}
    same(changed, set(manifest['projection_fields']), 'exact changed recipe fields')
    recipe = dict(historical, current=active)
    old.validate_current_dependencies(active, load(_record_at(successor['current_source_manifest'])), historical)
    return AdmittedAuthority(context, document, recipe, old, _AUTHORITY_TOKEN)

_READ_OWNER=contextvars.ContextVar('parser_read_owner',default=None)
_POISONED=set()

def _read_owner(a=None):
    owner=_READ_OWNER.get()
    if owner is None:return None
    require(owner['active'] and owner['thread']==threading.get_ident() and owner['pid']==os.getpid(),'expired or foreign parser read lifetime')
    require(a is None or owner['authority'] is a,'cross-authority parser read lifetime')
    require(owner['authority'] not in _POISONED,'poisoned parser authority')
    current=owner['authority'];same((current._document,current._recipe),owner['authority_bytes'],'read-lifetime authority metadata drift')
    require(current._context is owner['context'] and current._context.invocation() is owner['invocation'],'read-lifetime original context association')
    require(_preparation_api() is owner['preparation'] and _machine_api() is owner['machine_api'],'canonical parser owners changed')
    owner['preparation'].check_deadline(owner['invocation'])
    return owner

@contextlib.contextmanager
def _read_scope(a):
    """Call-local read-only reuse; must finish before any owned launch."""
    require(type(a) is AdmittedAuthority and a._seal is _AUTHORITY_TOKEN and a not in _POISONED,'live unpoisoned parser authority')
    prior=_read_owner(a)
    if prior is not None:
        yield
        _read_owner(a)
        return
    invocation=a._context.invocation();preparation=_preparation_api()
    owner=dict(authority=a,invocation=invocation,preparation=preparation,machine_api=_machine_api(),
        active=True,thread=threading.get_ident(),pid=os.getpid(),authority_bytes=(a._document,a._recipe),context=a._context,admitted=None,sessions={},machine=None,machine_document=None,toolchain=None,builds={})
    token=None
    try:
        with preparation.checked_scope(invocation):
            token=_READ_OWNER.set(owner)
            yield
            _read_owner(a)
            _read_exit(owner)
            preparation.full_verify_invocation(invocation)
    except BaseException:
        _POISONED.add(a)
        raise
    finally:
        owner['active']=False;owner['admitted']=None;owner['sessions'].clear();owner['builds'].clear();owner['machine_document']=None
        if token is not None:_READ_OWNER.reset(token)

def _read_exit(owner):
    with _bounded_phase(owner['invocation'],300):
        return _read_exit_body(owner)

def _read_exit_body(owner):
    """Full cached-domain exit checks; checkout-only verification is insufficient."""
    a=owner['authority'];invocation=owner['invocation']
    if owner['admitted'] is not None:
        _context_api().verify_context(a._context)
        old,recipe,document=owner['admitted']
        for key,(session,root) in owner['sessions'].items():
            same(identity(key),owner['session_identity'],'read-exit checkpoint body')
            same(identity(root/'shared-context.json'),session['shared_context'],'read-exit shared context body')
            same(session['host'],old.measured_host(),'read-exit actual host')
            _verify_views(root,old,recipe,document)
        if owner['toolchain'] is not None:
            with _bounded_phase(invocation,120):old.verify_toolchain(owner['toolchain'],recipe)
    if owner['machine_document'] is not None:
        owner['machine_api'].verify_machine(owner['machine'],invocation)
        same(owner['machine'].document(),owner['machine_document'],'read-exit machine carrier')
    _read_owner(a)

def _machine_document(machine):
    owner=_read_owner()
    if owner is not None and owner['machine'] is machine and owner['machine_document'] is not None:
        return copy.deepcopy(owner['machine_document'])
    return machine.document()

def _admit(a):
    require(type(a) is AdmittedAuthority and a._seal is _AUTHORITY_TOKEN, 'authenticated parser authority required')
    require(a not in _POISONED,'poisoned parser authority')
    owner=_read_owner(a)
    if owner is not None and owner['admitted'] is not None:
        old,recipe,document=owner['admitted'];return old,copy.deepcopy(recipe),copy.deepcopy(document)
    fresh = authority(a._context)
    same(fresh._document, a._document, 'context changed')
    same(fresh._recipe, a._recipe, 'recipe changed')
    value=(fresh._old,fresh.recipe(),fresh.document())
    if owner is not None:owner['admitted']=value
    return value[0],copy.deepcopy(value[1]),copy.deepcopy(value[2])

def plan_session(a):
    old, recipe, document = _admit(a)
    root = Path(document['invocation']['run_root'])/'parser'
    return dict(schema=SESSION_SCHEMA, status='NotReady', execution_qualified=False,
        root=str(root), checkpoint=str(root/CHECKPOINT), invocation=document['invocation'],
        checkout=document['checkout'], source_checkpoint=document['source_checkpoint'],
        current_source_manifest=document['current_source_manifest'],
        source_maps={r:document['views'][r]['files'] for r in ('parser_observed','parser_control')},
        profiles=recipe['recipe']['profiles'], runtime_admission='NotReady')

def _overlay(source, recipe, control):
    active = recipe['current']
    return dict(schema='oxid-package-source-parser-overlay-v1', source=str(source), control=control,
        candidate_source_manifest_sha256=active['current_candidate_source_manifest_sha256'],
        current_source_manifest_sha256=active['current_source_manifest']['sha256'],
        observer_source_sha256=recipe['helper_manifest_sha256'],
        observer_files=recipe['helper_files'],
        files=active['current_control_derived_files' if control else 'current_derived_files'])

def _close_tree(root,files,directories=(),links=None):
    links={} if links is None else links
    root=Path(root).absolute()
    require(root.is_dir() and not any(p.is_symlink() for p in (root,*root.parents)),'safe exact root')
    wanted_files=set(files); require(len(wanted_files)==len(files),'duplicate closure member')
    wanted_dirs=set(directories)
    for name in (*wanted_files,*wanted_dirs):
        path=Path(name)
        require(not path.is_absolute() and path.as_posix()==name and '..' not in path.parts,'safe closure path')
        wanted_dirs.update(p.as_posix() for p in path.parents if p!=Path('.'))
    found_files=set(); found_dirs=set()
    for path in root.rglob('*'):
        name=path.relative_to(root).as_posix()
        if path.is_symlink():
            require(name in links and os.readlink(path)==links[name],'unapproved link in exact tree')
            require(path.resolve(strict=True)==Path(links[name]),'executable view route drift')
            found_files.add(name);continue
        if path.is_file():found_files.add(name)
        elif path.is_dir():found_dirs.add(name)
        else:raise Rejected('special member in exact tree')
    same(found_files,wanted_files,'exact tree file roster')
    same(found_dirs,wanted_dirs,'exact tree directory roster')

def _verify_views(root, old, recipe, document, prepared=True, verify_outputs=True):
    closure=[]
    for control, role, dirname in ((False,'parser_observed','source'), (True,'parser_control','control-source')):
        source = root/dirname; view = document['views'][role]
        # Preparation metadata has its own named copied directory, never an
        # extra compiler member or a wildcard exception to source closure.
        old.verify_map(source, view['files'], exact=True, extras=('overlay-manifest.json','observer-source-manifest.json'))
        same(load(file_bytes(source/'overlay-manifest.json')), _overlay(source,recipe,control), 'new overlay binding')
        same(sha(file_bytes(source/'observer-source-manifest.json')), recipe['helper_manifest_sha256'], 'retained helper manifest')
        old.verify_map(root/'preparation-metadata'/role, view['metadata'], exact=True)
        old.no_cargo_configs(source)
        _close_tree(source,[r['path'] for r in view['files']]+['overlay-manifest.json','observer-source-manifest.json'])
        _close_tree(root/'preparation-metadata'/role,[r['path'] for r in view['metadata']])
        closure.extend(dirname+'/'+r['path'] for r in view['files'])
        closure.extend(dirname+'/'+name for name in ('overlay-manifest.json','observer-source-manifest.json'))
        closure.extend('preparation-metadata/'+role+'/'+r['path'] for r in view['metadata'])
    old.verify_map(root/'helpers', recipe['helper_files'], exact=True)
    _close_tree(root/'helpers',[r['path'] for r in recipe['helper_files']])
    _close_tree(root/'home',[])
    old.no_cargo_configs(root)
    for ancestor in (root,*root.parents):
        require(not (ancestor/'.cargo').is_symlink(),'ancestor Cargo directory symlink')
        for name in ('config','config.toml'):
            require(not (ancestor/'.cargo'/name).is_symlink(),'ancestor Cargo config symlink')
    if not verify_outputs:return
    closure.extend('helpers/'+r['path'] for r in recipe['helper_files'])
    if prepared:closure.extend((CHECKPOINT,'shared-context.json'))
    directories=['home','preparation-metadata/parser_observed','preparation-metadata/parser_control'];links={}
    for output,result in tuple(_OUTPUTS.items()):
        output=Path(output)
        if output.parent!=root:continue
        require(result is not None,'incomplete parser build retained; session cannot admit further work')
        state=_result(result)
        same(_snapshot_output(output,state['invocation'],state['tools']),state['output_snapshot'],'build output drift')
        prefix=output.name+'/'
        closure.extend(prefix+name for name in state['output_snapshot']['files'])
        directories.extend([output.name,*[prefix+name for name in state['output_snapshot']['directories']]])
        links.update({prefix+name:target for name,target in state['output_snapshot']['links'].items()})
    if _RUNTIME is not None:
        names,dirs=_runtime_api().closure(root);closure.extend(names);directories.extend(dirs)
    _close_tree(root,closure,directories,links)

def prepare(a):
    require(type(a) is AdmittedAuthority,'authenticated parser authority required')
    with _bounded_phase(a._context.invocation(),300):
        return _prepare(a)

def _prepare(a):
    """Materialize a distinct session from authenticated prepared role bytes.

    This function is implementation only in this release; source controls do
    not call it. No old invocation or receipt is relabelled as a new process.
    """
    with _read_scope(a):old, recipe, document = _admit(a); plan = plan_session(a)
    invocation=a._context.invocation();_tick(invocation)
    root = old.fresh(plan['root']); (root/'home').mkdir()
    for control, role, dirname in ((False,'parser_observed','source'), (True,'parser_control','control-source')):
        view = document['views'][role]; source = root/dirname; source.mkdir()
        for row in view['files']:
            _tick(invocation)
            target = source/row['path']; target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(Path(view['path'])/row['path'], target)
        metadata = root/'preparation-metadata'/role; metadata.mkdir(parents=True)
        for row in view['metadata']:
            _tick(invocation)
            target = metadata/row['path']; target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(Path(view['path'])/row['path'], target)
        (source/'overlay-manifest.json').write_bytes(serial(_overlay(source,recipe,control)))
        helper_raw=json.dumps(recipe['helper_files'],sort_keys=True,separators=(',',':')).encode()
        same(sha(helper_raw),recipe['helper_manifest_sha256'],'exact retained helper manifest')
        (source/'observer-source-manifest.json').write_bytes(helper_raw)
    (root/'helpers').mkdir()
    for row in recipe['helper_files']:
        _tick(invocation)
        target=root/'helpers'/row['path']; target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(REPOSITORY/FROZEN_PATH/'frozen/helpers'/row['path'],target)
    host = old.measured_host(); old.check_host(host,recipe)
    _verify_views(root,old,recipe,document,prepared=False); _admit(a)
    (root/'shared-context.json').write_bytes(serial(document))
    session = dict(plan, shared_context=identity(root/'shared-context.json'), context_sha256=sha(serial(document)), authority=identity(HERE/'authority.json'), adapter=identity(Path(__file__)),
        context=document, host=host, prepared_ns=time.time_ns(),
        preparation=document['preparation'],
        overlay=identity(root/'source/overlay-manifest.json'),
        control_overlay=identity(root/'control-source/overlay-manifest.json'))
    (root/CHECKPOINT).write_bytes(serial(session))
    session_at(root/CHECKPOINT,a)
    return identity(root/CHECKPOINT)

def session_at(path,a):
    require(type(a) is AdmittedAuthority,'authenticated parser authority required')
    with _bounded_phase(a._context.invocation(),300),_read_scope(a):
        owner=_read_owner(a);key=str(Path(path).absolute())
        if key in owner['sessions']:
            session,root=owner['sessions'][key]
            same(identity(path),owner['session_identity'],'checked session body drift')
            same(identity(root/'shared-context.json'),session['shared_context'],'checked shared context drift')
            same(session['host'],a._old.measured_host(),'checked session actual host')
            return copy.deepcopy(session),root
        session,root=_session_at(path,a);owner['sessions'][key]=(copy.deepcopy(session),root);owner['session_identity']=identity(path)
        return session,root

def _session_at(path,a):
    old,recipe,document = _admit(a); plan=plan_session(a)
    path=Path(path).absolute(); root=Path(plan['root'])
    same(path,root/CHECKPOINT,'distinct exact parser checkpoint')
    session=load(file_bytes(path))
    same(set(session),set(plan)|{'authority','adapter','context','host','prepared_ns','preparation','overlay','control_overlay','shared_context','context_sha256'},'closed successor session')
    for key,value in plan.items():
        same(session[key],value,'session field '+key)
    same(session['context'],document,'trusted current context')
    same(session['context_sha256'],sha(serial(document)),'canonical shared context digest')
    same(session['shared_context'],identity(root/'shared-context.json'),'shared context checkpoint')
    owner=_read_owner(a)
    if owner is not None and owner['admitted'] is not None:
        same(load(file_bytes(root/'shared-context.json')),document,'serialized checkpoint equals this lifetime freshly admitted live context')
    else:
        restored=_context_api().load_context(root/'shared-context.json',a._context.invocation())
        same(restored.document(),document,'independently readmitted shared checkpoint')
    same(session['preparation'],document['preparation'],'original preparation binding')
    same(session['authority'],identity(HERE/'authority.json'),'consumer authority identity')
    same(session['adapter'],identity(Path(__file__)),'consumer adapter identity')
    require(type(session['prepared_ns']) is int and document['invocation']['started_ns'] <= session['prepared_ns'] <= time.time_ns(),'session timing')
    require(re.fullmatch('[0-9a-f]{32}',document['invocation']['nonce']) is not None,'fresh producer nonce')
    old.check_host(session['host'],recipe)
    same(session['host'],old.measured_host(),'exact current host snapshot')
    for key,dirname in (('overlay','source'),('control_overlay','control-source')):
        same(session[key],identity(root/dirname/'overlay-manifest.json'),'fresh overlay identity')
    _verify_views(root,old,recipe,document)
    return session,root

def plan_build(session_path,profile,a,control=False):
    old,recipe,_ = _admit(a); session,root=session_at(session_path,a)
    require(type(control) is bool and profile in recipe['recipe']['profiles'],'build role/profile')
    source=root/('control-source' if control else 'source')
    out=root/(('build-control-' if control else 'build-')+profile)
    require(not out.exists() and not out.is_symlink(),'fresh build output required')
    return dict(schema='oxid-package-source-parser-build-plan-v1',status='NotReady',
        session=identity(session_path),source=str(source),output=str(out),control=control,profile=profile,
        helper_argv=[sys.executable,'-B',str(root/'helpers/build.py'),'--overlay',str(source/'overlay-manifest.json'),'--profile',profile,'--output',str(out/'result')],
        cargo_argv=[str(out/'rust-bin/cargo'),*recipe['recipe']['cargo_tail'],*(['--release'] if profile=='release' else [])],
        recipe=recipe['recipe'],compiler_files=recipe['compiler_files'],dependency_files=old.dependency_files(recipe),
        required_live_machine='producer-issued ToolAdmission',operation_kind='parser-helper')

BUILD_TIMEOUT_SECONDS=1800
BUILD_STREAM_LIMIT=8*1024**2
PROBE_TIMEOUT_SECONDS=30
_RESULTS=weakref.WeakKeyDictionary()
_OUTPUTS={}
_PHASE_DEADLINE=contextvars.ContextVar('parser_phase_deadline',default=None)

@contextlib.contextmanager
def _bounded_phase(invocation,seconds):
    previous=_PHASE_DEADLINE.get();deadline=time.monotonic()+seconds
    token=_PHASE_DEADLINE.set(min(previous,deadline) if previous is not None else deadline)
    try:
        _tick(invocation)
        yield
        _tick(invocation)
    finally:
        _PHASE_DEADLINE.reset(token)

class BuildResult:
    """Only build() can issue a live result; JSON is never execution authority."""
    __slots__=('__weakref__',)
    def __new__(cls,*args,**kwargs):
        raise TypeError('BuildResult is issued only by the parser build producer')
    def document(self):
        return load(_result(self)['raw'])

def _result(result):
    require(type(result) is BuildResult and result in _RESULTS,'live parser BuildResult required')
    return _RESULTS[result]

def _tick(invocation):
    deadline=_PHASE_DEADLINE.get()
    require(deadline is None or time.monotonic()<deadline,'bounded parser phase deadline expired')
    owner=_READ_OWNER.get()
    if owner is not None:
        require(owner['active'] and owner['invocation'] is invocation and owner['thread']==threading.get_ident() and owner['pid']==os.getpid(),'deadline owner lifetime')
        owner['preparation'].check_deadline(invocation)
    else:_preparation_api().check_deadline(invocation)

def _stat_record(path):
    value=path.stat()
    require(stat.S_ISREG(value.st_mode),'regular output body')
    return {k:getattr(value,k) for k in ('st_dev','st_ino','st_mode','st_size','st_mtime_ns','st_ctime_ns')}

def _output_file(path,invocation):
    require(not path.is_symlink(),'regular output file')
    before=_stat_record(path);require(before['st_size']<=1024**3,'bounded build output file')
    digest=hashlib.sha256();size=0
    with path.open('rb') as stream:
        while True:
            _tick(invocation);raw=stream.read(1024**2)
            if not raw:break
            size+=len(raw);require(size<=1024**3,'build output grew beyond bound');digest.update(raw)
    same(_stat_record(path),before,'output changed during capture')
    same(size,before['st_size'],'output byte count changed')
    return dict(bytes=size,sha256=digest.hexdigest(),stat=before)

def _snapshot_output(root,invocation,tools):
    require(root.is_dir() and not any(p.is_symlink() for p in (root,*root.parents)),'owned output directory')
    files={};directories=[];links={};total=0;count=0
    wanted_links={'rust-bin/'+name:tools[name] for name in ('cargo','rustc')}
    for path in root.rglob('*'):
        _tick(invocation);count+=1;require(count<=200000,'bounded output member roster')
        name=path.relative_to(root).as_posix()
        if path.is_symlink():
            require(name in wanted_links,'unexpected build output symlink')
            same(os.readlink(path),wanted_links[name],'exact executable view link')
            same(str(path.resolve(strict=True)),wanted_links[name],'executable view resolution')
            links[name]=wanted_links[name];files[name]={'link':wanted_links[name]}
        elif path.is_dir():directories.append(name)
        else:
            row=_output_file(path,invocation);files[name]=row;total+=row['bytes']
            require(total<=6*1024**3,'bounded complete build output')
    same(links,wanted_links,'complete narrow executable view')
    return dict(files=files,directories=sorted(directories),links=links)

def _cache_rows(machine,recipe,old):
    doc=machine.document();snapshot=doc['snapshot'];cache=Path(snapshot['environment']['CARGO_HOME'])
    records={row['path']:dict(row) for row in old.dependency_files(recipe)}
    for package in snapshot['dependencies']:
        for key in ('index','index_config'):
            row=package[key];name=Path(row['path']).relative_to(cache).as_posix()
            expected={k:row[k] for k in ('bytes','sha256')};expected['path']=name
            if name in records:same(records[name],expected,'machine/cache row association')
            records[name]=expected
    return cache,[records[name] for name in sorted(records)]

def _verify_cache(root,rows,old,invocation):
    with _bounded_phase(invocation,120):
        return _verify_cache_members(root,rows,old)

def _verify_cache_members(root,rows,old):
    old.verify_map(root,rows)
    for sub in ('registry/src','registry/cache','registry/index'):
        prefix=sub+'/'
        _close_tree(root/sub,[r['path'][len(prefix):] for r in rows if r['path'].startswith(prefix)])
    for name in ('config','config.toml','credentials','credentials.toml'):
        require(not os.path.lexists(root/name),'private Cargo configuration/credentials forbidden')

def _environment(machine,role,profile,out):
    env=machine.environment(role,profile,out/'result/target')
    env.update(HOME=str(out/'home'),CARGO_HOME=str(out/'cargo-home'),
               PATH=str(out/'rust-bin')+':/usr/bin:/bin',PYTHONDONTWRITEBYTECODE='1',PYTHONNOUSERSITE='1',PYTHONOPTIMIZE='0')
    return env

def _verify_probe(probe,invocation,argv,cwd,env,timeout,kind):
    doc=_preparation_api().verify_probe(probe,invocation,argv=argv,cwd=str(cwd),environment=env,
        timeout_seconds=timeout,stream_limit_bytes=BUILD_STREAM_LIMIT,operation_kind=kind)
    require(type(doc['status']) is int and doc['status']==0,'successful owned process required')
    require(type(doc['pid']) is int and doc['pid']>0,'actual owned child PID')
    require(type(doc['started_ns']) is int and type(doc['completed_ns']) is int,'actual integer process timestamps')
    require(doc['cleanup_complete'] and doc['cleanup_error'] is None and doc['failure'] is None
        and not doc['timed_out'] and not doc['stream_limit_exceeded'],'owned bounded process completion')
    same(doc['invocation_nonce'],invocation.document()['nonce'],'owned process invocation')
    require(invocation.document()['started_ns']<=doc['started_ns']<=doc['completed_ns']<=time.time_ns(),'actual process window')
    return doc

def _machine_bind(machine,invocation,recipe,old,out=None):
    owner=_read_owner()
    if owner is None or owner['machine_document'] is None:
        _machine_api().verify_machine(machine,invocation)
        document=machine.document()
        if owner is not None:owner['machine']=machine;owner['machine_document']=copy.deepcopy(document)
    else:
        require(owner['machine'] is machine and owner['invocation'] is invocation,'same full-checkpoint machine owner')
        document=copy.deepcopy(owner['machine_document'])
    snapshot=document['snapshot']
    same(snapshot['host']['name'],'Linux x86_64','parser original host policy')
    same(document['rustc_verbose'],recipe['recipe']['rustc_verbose'],'retained exact compiler version')
    original=snapshot['rust']['parser_original_authority']
    require(original is not None,'actual parser toolchain authority required')
    same(original['compiler_files'],recipe['compiler_files'],'retained original compiler bytes')
    same(original['recipe'],recipe['recipe'],'retained original build recipe')
    tools={name:machine.tool(name) for name in ('cargo','rustc','which')}
    same(str(Path(sys.executable).resolve()),snapshot['host']['python']['resolved'],'helper interpreter route')
    same(identity(Path(sys.executable).resolve()),{k:snapshot['host']['python']['body'][k] for k in ('path','bytes','sha256')},'helper interpreter body')
    same(str(Path(shutil.which('which',path='/usr/bin:/bin') or '').resolve()),tools['which'],'fixed helper discovery executable')
    toolchain=Path(snapshot['rust']['root'])
    if owner is None or owner['toolchain'] is None:
        with _bounded_phase(invocation,120):old.verify_toolchain(toolchain,recipe)
        if owner is not None:owner['toolchain']=toolchain
    else:same(owner['toolchain'],toolchain,'same read-lifetime compiler root')
    same(tools['cargo'],str(toolchain/'bin/cargo'),'original Cargo path')
    same(tools['rustc'],str(toolchain/'bin/rustc'),'original rustc path')
    if out is not None:old.verify_executable_view(out/'rust-bin',toolchain)
    if out is not None:
        same(list((out/'home').iterdir()),[],'empty private build home')
    return tools,toolchain

def build(session_path,profile,a,machine,control=False):
    """Run the actual frozen helper under the producer-owned bounded tree.

    This implementation is dormant during source-only validation. Its returned
    live result keeps actual supervisor objects; files alone cannot recreate it.
    """
    with _read_scope(a):
        old,recipe,_=_admit(a);session,root=session_at(session_path,a)
        plan=plan_build(session_path,profile,a,control);invocation=a._context.invocation()
        tools,toolchain=_machine_bind(machine,invocation,recipe,old)
    out=Path(plan['output']);require(not os.path.lexists(out),'fresh build directory')
    _tick(invocation);out.mkdir();_OUTPUTS[str(out)]=None
    sysroot=helper=None;cache_rows=[]
    try:
        (out/'home').mkdir()
        with _bounded_phase(invocation,120):old.create_executable_view(out/'rust-bin',toolchain,recipe)
        _preparation_api().register_executable_view(invocation,out/'rust-bin',toolchain)
        cache,cache_rows=_cache_rows(machine,recipe,old)
        with _bounded_phase(invocation,120):
            old.cargo_cache(cache,out/'cargo-home',recipe);_verify_cache(out/'cargo-home',cache_rows,old,invocation)
        role='parser_control' if control else 'parser_observed'
        env=_environment(machine,role,profile,out);api=_preparation_api()
        sysroot_argv=[str(out/'rust-bin/rustc'),'--print','sysroot']
        sysroot=api.owned_probe(invocation,sysroot_argv,str(root),env,
            timeout_seconds=PROBE_TIMEOUT_SECONDS,stream_limit_bytes=BUILD_STREAM_LIMIT)
        sd=_verify_probe(sysroot,invocation,sysroot_argv,root,env,PROBE_TIMEOUT_SECONDS,'probe')
        same(sysroot.stdout(),(str(toolchain)+'\n').encode(),'actual narrow-view sysroot')
        same(sysroot.stderr(),b'','sysroot stderr')
        _machine_bind(machine,invocation,recipe,old,out);_admit(a)
        helper=api.owned_probe(invocation,plan['helper_argv'],str(root),env,
            timeout_seconds=BUILD_TIMEOUT_SECONDS,stream_limit_bytes=BUILD_STREAM_LIMIT,operation_kind='parser-helper')
        hd=_verify_probe(helper,invocation,plan['helper_argv'],root,env,BUILD_TIMEOUT_SECONDS,'parser-helper')
        require(session['prepared_ns']<=sd['started_ns']<=sd['completed_ns']<=hd['started_ns'],'session/sysroot/helper order')
        _machine_bind(machine,invocation,recipe,old,out);_verify_cache(out/'cargo-home',cache_rows,old,invocation);_admit(a)
        for name in ('stdout.jsonl','stderr.txt','build-receipt.json'):
            require((out/'result'/name).stat().st_size<=BUILD_STREAM_LIMIT,'bounded helper evidence')
        receipt_path=out/'result/build-receipt.json';receipt=load(file_bytes(receipt_path))
        same(load(helper.stdout()),receipt,'actual helper stdout/receipt equality');same(helper.stderr(),b'','helper stderr')
        # This is solely an explicit window projection from the actual owned
        # process, not an old invocation record or a fabricated helper receipt.
        window={'started_ns':hd['started_ns'],'finished_ns':hd['completed_ns']}
        require(Path(receipt['binary']['path']).stat().st_size<=256*1024**2,'bounded parser executable')
        with _bounded_phase(invocation,120):old.verify_cargo(receipt,root,out,profile,toolchain,window,recipe,control=control)
        for previous in tuple(_OUTPUTS.values()):
            if previous is not None and _result(previous)['invocation'] is invocation:
                require(previous.document()['binary']['sha256']!=receipt['binary']['sha256'],'distinct actual parser binaries required')
        envelope=dict(schema='oxid-package-source-parser-build-v1',status='built',execution_qualified=False,
            session=identity(session_path),authority=identity(HERE/'authority.json'),adapter=identity(Path(__file__)),
            invocation=invocation.document(),profile=profile,control=control,source=str(root/('control-source' if control else 'source')),
            output=str(out),machine_sha256=sha(serial(machine.document())),environment=env,
            sysroot_probe=sd,helper_probe=hd,receipt=identity(receipt_path),binary=identity(receipt['binary']['path']),
            compiler_source_sha256=recipe['current']['current_source_manifest']['sha256'],
            candidate_source_sha256=recipe['current']['current_candidate_source_manifest_sha256'])
        (out/'package-source-build.json').write_bytes(serial(envelope))
        result=object.__new__(BuildResult)
        state=dict(raw=serial(envelope),a=a,machine=machine,invocation=invocation,session_path=Path(session_path).absolute(),
            root=root,out=out,profile=profile,control=control,helper=helper,sysroot=sysroot,tools=tools,cache_rows=cache_rows,
            checkpoint=identity(out/'package-source-build.json'))
        state['output_snapshot']=_snapshot_output(out,invocation,tools)
        selected_paths=[state['session_path'],root/'shared-context.json',*[root/'helpers'/row['path'] for row in recipe['helper_files']]]
        state['selected_stats']={str(p):_stat_record(p) for p in selected_paths}
        _RESULTS[result]=state;_OUTPUTS[str(out)]=result
        verify_build(result)
        return result
    except BaseException as error:
        _OUTPUTS[str(out)]=None
        try:_failure_receipt(error,out,a,machine,invocation,recipe,old,cache_rows,sysroot,helper)
        except BaseException as diagnostic_error:
            error.add_note('Parser failure evidence could not be completed: '+repr(diagnostic_error)[:1000])
        raise

def _failure_receipt(error,out,a,machine,invocation,recipe,old,cache_rows,sysroot,helper):
    started=time.time_ns();attempts=[]
    checks=(('source-authorities',lambda:_admit(a)),
            ('copied-source',lambda:_verify_views(out.parent,old,recipe,a.document(),verify_outputs=False)),
            ('machine',lambda:_machine_bind(machine,invocation,recipe,old)),
            ('private-cache',lambda:_verify_cache(out/'cargo-home',cache_rows,old,invocation)))
    # The global budget may already be exhausted or the supervisor may have
    # failed closed. Record that fact per check; never promote missing proof.
    with_context=_bounded_phase(invocation,60)
    try:
        with with_context:
            for name,check in checks:
                try:
                    _tick(invocation);check();_tick(invocation)
                    attempts.append(dict(name=name,status='preserved'))
                except BaseException as failure:
                    attempts.append(dict(name=name,status='unverified',error=repr(failure)[:1000]))
    except BaseException as failure:
        for name,_ in checks:
            if name not in {x['name'] for x in attempts}:
                attempts.append(dict(name=name,status='unverified',error=repr(failure)[:1000]))
    receipt=dict(schema='oxid-package-source-parser-build-failure-v1',status='NotReady',execution_qualified=False,
        original_error=repr(error)[:2000],started_ns=started,completed_ns=time.time_ns(),
        invocation_nonce=invocation.document()['nonce'],output=str(out),preservation=attempts,sysroot_record_issued=sysroot is not None,helper_record_issued=helper is not None)
    target=out/'package-source-build-failure.json'
    require(not os.path.lexists(target) and out.is_dir() and not any(p.is_symlink() for p in (out,*out.parents)),'fresh safe terminal failure receipt')
    raw=serial(receipt);require(len(raw)<=16384,'bounded failure receipt')
    with target.open('xb') as stream:stream.write(raw)

def verify_build(result):
    state=_result(result)
    with _bounded_phase(state['invocation'],300),_read_scope(state['a']):
        owner=_read_owner(state['a'])
        if result in owner['builds']:return _selected_build(result)
        value=_verify_build(result);owner['builds'][result]=copy.deepcopy(value)
        return value

def _verify_build(result):
    """Re-admit only the live producer result, preserving the native receipt."""
    state=_result(result);a=state['a'];old,recipe,_=_admit(a)
    envelope=result.document();session,root=session_at(state['session_path'],a)
    same(root,state['root'],'build/session root');out=state['out'];invocation=state['invocation']
    require(a._context.invocation() is invocation,'same original producer invocation')
    tools,toolchain=_machine_bind(state['machine'],invocation,recipe,old,out)
    same(tools,state['tools'],'build tool association')
    same(envelope['machine_sha256'],sha(serial(_machine_document(state['machine']))),'original machine capture')
    same(envelope['session'],identity(state['session_path']),'original session checkpoint')
    same(envelope['authority'],identity(HERE/'authority.json'),'build authority')
    same(envelope['adapter'],identity(Path(__file__)),'build consumer')
    same(state['checkpoint'],identity(out/'package-source-build.json'),'new envelope bytes')
    same(load(file_bytes(out/'package-source-build.json')),envelope,'new envelope exact content')
    profile,control=state['profile'],state['control'];role='parser_control' if control else 'parser_observed'
    env=_environment(state['machine'],role,profile,out);same(envelope['environment'],env,'actual machine-derived build environment')
    sd=_verify_probe(state['sysroot'],invocation,[str(out/'rust-bin/rustc'),'--print','sysroot'],root,env,PROBE_TIMEOUT_SECONDS,'probe')
    same(sd,envelope['sysroot_probe'],'original sysroot process')
    same(state['sysroot'].stdout(),(str(toolchain)+'\n').encode(),'actual narrow-view sysroot');same(state['sysroot'].stderr(),b'','sysroot stderr')
    source=root/('control-source' if control else 'source')
    argv=[sys.executable,'-B',str(root/'helpers/build.py'),'--overlay',str(source/'overlay-manifest.json'),'--profile',profile,'--output',str(out/'result')]
    hd=_verify_probe(state['helper'],invocation,argv,root,env,BUILD_TIMEOUT_SECONDS,'parser-helper')
    same(hd,envelope['helper_probe'],'original actual helper process')
    require(session['prepared_ns']<=sd['started_ns']<=sd['completed_ns']<=hd['started_ns'],'fresh session/process ordering')
    _verify_cache(out/'cargo-home',state['cache_rows'],old,invocation)
    for name in ('stdout.jsonl','stderr.txt','build-receipt.json'):
        require((out/'result'/name).stat().st_size<=BUILD_STREAM_LIMIT,'bounded helper evidence')
    receipt=load(old.artifact(envelope['receipt'],out/'result/build-receipt.json'))
    same(load(state['helper'].stdout()),receipt,'original actual helper stdout');same(state['helper'].stderr(),b'','helper stderr')
    window={'started_ns':hd['started_ns'],'finished_ns':hd['completed_ns']}
    same(identity(receipt['binary']['path']),envelope['binary'],'new emitted binary identity')
    with _bounded_phase(invocation,120):old.verify_cargo(receipt,root,out,profile,toolchain,window,recipe,control=control)
    same(_snapshot_output(out,invocation,tools),state['output_snapshot'],'complete generated output preservation')
    _machine_bind(state['machine'],invocation,recipe,old,out);_admit(a)
    return receipt,envelope

def _poison_authority(a):
    _POISONED.add(a)

def _selected_build(result):
    """Read exact live build evidence; full admission remains a boundary rule."""
    state=_result(result);a=state['a'];require(a not in _POISONED,'poisoned parser authority')
    invocation=state['invocation'];require(a._context.invocation() is invocation,'selected original invocation')
    with _preparation_api().checked_scope(invocation):
        envelope=result.document();root=state['root'];out=state['out'];old=a._old;recipe=a.recipe()
        for actual,wanted,label in ((identity(HERE/'authority.json'),envelope['authority'],'authority'),
            (identity(Path(__file__)),envelope['adapter'],'consumer'),(identity(state['session_path']),envelope['session'],'checkpoint'),
            (identity(out/'package-source-build.json'),state['checkpoint'],'build envelope')):same(actual,wanted,'selected '+label)
        for name,expected in state['selected_stats'].items():same(_stat_record(Path(name)),expected,'selected helper/checkpoint stat')
        session=load(file_bytes(state['session_path']));same(identity(root/'shared-context.json'),session['shared_context'],'selected shared checkpoint')
        same(session['host'],old.measured_host(),'selected actual host');same(session['context'],a.document(),'selected retained context')
        old.verify_map(root/'helpers',recipe['helper_files'],exact=True);_close_tree(root/'helpers',[r['path'] for r in recipe['helper_files']])
        receipt=load(old.artifact(envelope['receipt'],out/'result/build-receipt.json'))
        same(receipt,load(state['helper'].stdout()),'selected genuine helper receipt');same(state['helper'].stderr(),b'','selected helper stderr')
        same(state['helper'].document(),envelope['helper_probe'],'selected original helper record')
        same(state['sysroot'].document(),envelope['sysroot_probe'],'selected original sysroot record')
        same(identity(receipt['binary']['path']),envelope['binary'],'selected actual binary')
        for path in (Path(receipt['binary']['path']),out/'result/build-receipt.json',out/'package-source-build.json'):
            name=path.relative_to(out).as_posix();same(_output_file(path,invocation),state['output_snapshot']['files'][name],'selected output body/stat')
        old.verify_executable_view(out/'rust-bin',Path(state['tools']['rustc']).parent.parent)
        for name in ('cargo','rustc'):
            same(str((out/'rust-bin'/name).resolve(strict=True)),state['tools'][name],'selected compiler route')
        _tick(invocation)
        return copy.deepcopy(receipt),copy.deepcopy(envelope)

def _selected_source(result):
    state=_result(result);a=state['a'];require(a not in _POISONED,'poisoned parser authority')
    with _preparation_api().checked_scope(state['invocation']):
        document=a.document();recipe=a.recipe();old=a._old
        role='parser_control' if state['control'] else 'parser_observed';dirname='control-source' if state['control'] else 'source'
        source=state['root']/dirname;view=document['views'][role]
        old.verify_map(source,view['files'],exact=True,extras=('overlay-manifest.json','observer-source-manifest.json'))
        same(load(file_bytes(source/'overlay-manifest.json')),_overlay(source,recipe,state['control']),'selected exact source overlay')
        same(sha(file_bytes(source/'observer-source-manifest.json')),recipe['helper_manifest_sha256'],'selected helper source manifest')
        old.no_cargo_configs(source);_close_tree(source,[r['path'] for r in view['files']]+['overlay-manifest.json','observer-source-manifest.json'])
        _tick(state['invocation'])

def derive_comparator(raw):
    """Pure exact derivation, not comparator construction or runtime admission."""
    pins=_manifest()['comparator_derivation']
    same(sha(raw),pins['original_sha256'],'frozen comparator source')
    source=raw.decode(); function=source.index('def execution_manifest(path, contract, approval):\n')
    start=source.index('    manifest = read_json(path)\n',function)
    end=source.index('    cases = {c["id"]: c for c in contract["cases"]}\n',start)
    replacement='    manifest, host, build, profile = package_source_admission(path, contract, approval)\n'
    derived=(source[:start]+replacement+source[end:]).encode()
    proof=dict(original_sha256=sha(raw),derived_sha256=sha(derived),replaced_region_sha256=sha(source[start:end].encode()),replacement_sha256=sha(replacement.encode()),unchanged_prefix_sha256=sha(source[:start].encode()),unchanged_suffix_sha256=sha(source[end:].encode()),semantic_predicate_handlers_unchanged=22)
    same(proof,pins,'exact comparator derivation; all semantic bytes unchanged')
    return derived,dict(schema='oxid-package-source-parser-comparator-derivation-v1',**proof)

_RUNTIME=None

def _runtime_api():
    global _RUNTIME
    manifest=_manifest();row=manifest['runtime_helper']
    same(row['path'],'runtime.py','fixed runtime source path')
    raw=_record_at(row,HERE)
    if _RUNTIME is None:
        module=_module('package_source_parser_runtime',HERE/'runtime.py',raw)
        names=('AUTHORITY_SHA','CHECKPOINT','FROZEN_PATH','REPOSITORY','_admit','_bounded_phase',
            '_close_tree','_machine_api','_manifest','_preparation_api','_record_at','_result','_tick',
            '_verify_views','_read_scope','_selected_build','_selected_source','_poison_authority','derive_comparator','file_bytes','identity','load','session_at','verify_build')
        module.bind_consumer(types.SimpleNamespace(**{name:globals()[name] for name in names}))
        _RUNTIME=module
    return _RUNTIME

def create_runtime(a,build_results,contract_root,runtime_inputs):
    return _runtime_api().create_runtime(a,build_results,contract_root,runtime_inputs)

def passivity(runtime,profile):return _runtime_api().passivity(runtime,profile)
def collect(runtime,profile):return _runtime_api().collect(runtime,profile)
def execute_u8_controls(runtime):return _runtime_api().execute_u8_controls(runtime)
def execute_phase_controls(runtime):return _runtime_api().execute_phase_controls(runtime)
def compare(runtime):return _runtime_api().compare(runtime)
def complete_runtime(runtime):return _runtime_api().complete(runtime)
def verify_runtime(runtime):return _runtime_api().verify_runtime(runtime)

# The old unbound signature cannot grant access to runtime internals.
def _not_ready(*args,**kwargs):
    raise NotReady('Use the successor live RuntimeSession producer API; unbound legacy admission is unavailable')
verify_passivity = admit_execution = comparator = _not_ready
phase_control_binding = phase_scope = phase_evidence = project_enum_observations = _not_ready
u8_policy_module = phase_module = _not_ready
