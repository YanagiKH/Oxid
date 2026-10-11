"""Producer-owned source preparation and finite process records.

Records are live invocation capabilities, not portable attestations. All runtime
and CI qualification remains separate. No subprocess is launched during import.
"""
import hashlib,json,os,queue,re,shutil,signal,stat,sys,threading,time,types
from pathlib import Path
from contextlib import contextmanager
SELF='tests/qualification/package_dependency_unit4_current'
KEY='_oxid_package_dependency_unit4_preparation_v1'
CONTEXT_KEY='_oxid_package_dependency_unit4_context_v1'
FILES={'context.py','preparation.py','prepare_worker.py','test_context.py','test_preparation.py'}
TREE_PATH='tests/qualification/unit4_public_v3/process_tree.py'
TREE_SHA='6b53900e18b33a26c5949bf8905f742e9abe5b67e74748585bbb24b6e4091454'
TRAP_SOURCE='tests/qualification/unit4_public_v3/no_tool_trap.rs'
TRAP_SOURCE_SHA='6493bd391eb3d8647dd3c33f27b75ff614024070942715bc20acc9732632bda2'
PARSER_BUILD_SHA='4a81dcd5a2252f7dbd5045075e23f54d7f08844a1617abab4180ab7181c3d793'
_PROBES={};_RECORDS={};_RUNS={};_MODULES={};_VIEWS={};_TOKEN=object();_DEADLINE=None
def remaining_seconds():
 return max(0.0,_DEADLINE-time.monotonic()) if _DEADLINE is not None else 36000.0
def tick():
 if _DEADLINE is not None:need(time.monotonic()<_DEADLINE,'preparation total deadline')
 _public_output_tick_active()
class Rejected(ValueError):pass
class Cancelled(RuntimeError):pass
def need(value,message):
 if not value:raise Rejected(message)
def sha(raw):
 h=hashlib.sha256()
 for offset in range(0,len(raw),1024**2):tick();h.update(raw[offset:offset+1024**2])
 tick();return h.hexdigest()
def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':'),allow_nan=False).encode()
def serial(value):return (json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()
def decode(raw):
 def pairs(items):
  d={}
  for k,v in items:need(k not in d,'duplicate JSON key');d[k]=v
  return d
 return json.loads(raw,object_pairs_hook=pairs,parse_constant=lambda _:need(False,'nonfinite JSON'))
def real(path,exists=True):
 p=Path(path);need(p.is_absolute() and p.resolve(strict=exists)==p and not any(x.is_symlink() for x in (p,*p.parents)),'canonical path required');return p
def body(path,limit=128*1024**2):
 tick();p=real(path);s=p.lstat();need(stat.S_ISREG(s.st_mode) and s.st_size<=limit,'regular bounded body');chunks=[];total=0
 with p.open('rb') as stream:
  while True:
   tick();chunk=stream.read(1024**2)
   if not chunk:break
   total+=len(chunk);need(total<=s.st_size,'growing bounded input');chunks.append(chunk)
 raw=b''.join(chunks);need(len(raw)==s.st_size,'body changed');return raw
def identity(path):
 p=real(path);raw=body(p);return {'path':str(p),'bytes':len(raw),'sha256':sha(raw)}
def _manifest():
 root=real(Path(__file__).parent);m=decode(body(root/'package-manifest.json'));need(m['schema']=='oxid-package-dependency-unit4-current-package-v1','shared manifest schema')
 rows={r['path']:r for r in m['files']};need(len(rows)==len(m['files']) and set(rows)==FILES,'shared five-helper roster')
 need({p.name for p in root.iterdir()}==FILES|{'package-manifest.json'},'closed shared directory')
 for n,r in rows.items():need(set(r)=={'path','bytes','sha256'} and dict(identity(root/n),path=n)==r,'shared body pin')
 need(getattr(sys.modules.get(KEY),'__dict__',None) is globals(),'canonical executing producer required')
 need(getattr(sys.modules.get(KEY),'__file__',None)==str(root/'preparation.py') and getattr(sys.modules.get(KEY),'__source_sha256__',None)==rows['preparation.py']['sha256'],'canonical producer owner required')
 return root,rows
def _module(path,name,digest):
 raw=body(path);need(sha(raw)==digest,'module body pin');old=sys.modules.get(name)
 if old is not None:
  need(_MODULES.get(name) is old,'unissued cached helper module')
  need(getattr(old,'__file__',None)==str(path) and getattr(old,'__source_sha256__',None)==digest,'occupied canonical module identity');return old
 m=types.ModuleType(name);m.__file__=str(path);m.__source_sha256__=digest;sys.modules[name]=m
 try:exec(compile(raw,str(path),'exec'),m.__dict__)
 except BaseException:
  if sys.modules.get(name) is m:del sys.modules[name]
  raise
 _MODULES[name]=m
 return m
_CHECKED=[];_LAUNCH=None
class _CheckedScope:
 __slots__=('invocation','thread','context','producer','active')
 def __init__(self,invocation,context,token=None):
  need(token is _TOKEN,'issued checked scope required')
  for name,value in {'invocation':invocation,'thread':threading.get_ident(),'context':context,'producer':sys.modules.get(KEY),'active':True}.items():object.__setattr__(self,name,value)
 def __setattr__(self,*_):raise TypeError('immutable checked scope')

def _active_owner(invocation=None):
 value=_CHECKED[-1] if _CHECKED else _LAUNCH
 if value is None:return None
 need(value.active and value.thread==threading.get_ident(),'expired or cross-thread checked owner')
 need(invocation is None or value.invocation is invocation,'cross-invocation checked owner')
 need(sys.modules.get(KEY) is value.producer and getattr(value.producer,'__dict__',None) is globals() and sys.modules.get(CONTEXT_KEY) is value.context,'canonical checked owner drift')
 check_deadline(value.invocation)
 return value

def context_api():
 active=_active_owner()
 if active is not None:return active.context
 root,rows=_manifest();return _module(root/'context.py',CONTEXT_KEY,rows['context.py']['sha256'])
def verify_invocation(invocation):
 active=_active_owner(invocation)
 return active.context._invocation_association(invocation) if active is not None else context_api()._invocation(invocation)

def full_verify_invocation(invocation):
 """Explicit whole-checkout readmission, even inside a checked read lifetime."""
 _manifest();context=context_api();value=context._invocation(invocation);_verify_view_rows(list(_VIEWS.get(invocation,{}).values()),_force_full=True);return value

def cancel_invocation(invocation):
 """Poison only the original live invocation without starting preservation IO."""
 context=_MODULES.get(CONTEXT_KEY);need(context is not None and sys.modules.get(CONTEXT_KEY) is context,'canonical cancellation context required')
 context._invocation_association(invocation);need(invocation in _RUNS,'active cancellation invocation required');_RUNS[invocation]['failed']=True

def _poison(invocation):
 if invocation in _RUNS:_RUNS[invocation]['failed']=True

@contextmanager
def checked_scope(invocation):
 """A live read-only owner lifetime; no body hash survives this scope."""
 scope=None
 try:
  need(_LAUNCH is None,'read scope during owned launch')
  need(invocation in _RUNS,'checked scope requires active producer invocation')
  context=context_api();context._invocation_association(invocation);check_deadline(invocation)
  need(not _CHECKED or _CHECKED[-1].invocation is invocation,'cross-invocation nested scope')
  need(len(_CHECKED)<16,'bounded checked scope nesting')
  _manifest();scope=_CheckedScope(invocation,context,_TOKEN);_CHECKED.append(scope)
  yield
  _active_owner(invocation);_manifest();context._invocation_association(invocation)
 except BaseException:
  _poison(invocation);raise
 finally:
  if scope is not None:
   if not _CHECKED or _CHECKED[-1] is not scope:_poison(invocation);need(False,'checked scope stack corruption')
   _CHECKED.pop();object.__setattr__(scope,'active',False)

def _worker():
 root,rows=_manifest();return _module(root/'prepare_worker.py','_oxid_package_dependency_unit4_prepare_worker_v1',rows['prepare_worker.py']['sha256'])
def _copy(value):return decode(canonical(value))
class ProbeRecord:
 __slots__=('_document',)
 def __init__(self,value,token=None):
  need(token is _TOKEN,'producer-issued probe required');object.__setattr__(self,'_document',canonical(value))
 def __setattr__(self,*_):raise TypeError('immutable probe record')
 def document(self):return decode(self._document)
 def stdout(self):return _probe_stream(self,'stdout')
 def stderr(self):return _probe_stream(self,'stderr')
class PreparationRecord:
 __slots__=('_document',)
 def __init__(self,value,token=None):
  need(token is _TOKEN,'producer-issued preparation required');object.__setattr__(self,'_document',canonical(value))
 def __setattr__(self,*_):raise TypeError('immutable preparation record')
 def document(self):return decode(self._document)
def _probe_stream(record,name):
 need(record in _PROBES,'unregistered probe');r=record.document()[name];need(identity(r['path'])==r,'probe stream drift');return body(r['path'])
def _run_state(invocation):
 inv,captured=verify_invocation(invocation)
 if invocation not in _RUNS:
  root=real(inv['run_root'],False);need(not root.exists(),'invocation root already exists');root.mkdir();(root/'operations').mkdir();(root/'home').mkdir();(root/'tmp').mkdir()
  _RUNS[invocation]={'deadline':time.monotonic()+36000,'next':0,'root':root,'failed':False}
 state=_RUNS[invocation];need(not state['failed'],'invocation previously failed');return inv,captured,state

def check_deadline(invocation):
 if invocation not in _RUNS:_run_state(invocation)
 state=_RUNS[invocation];need(not state['failed'] and time.monotonic()<state['deadline'],'invocation failed or deadline expired');tick();return True

def _sample_linux(group):
 found=[]
 if not sys.platform.startswith('linux'):return found
 for p in Path('/proc').iterdir():
  if not p.name.isdigit():continue
  try:
   raw=(p/'stat').read_text();f=raw[raw.rfind(')')+2:].split();pgid=int(f[2]);rss=int(f[21])*os.sysconf('SC_PAGE_SIZE')
  except (FileNotFoundError,ProcessLookupError,PermissionError):continue
  if pgid==group:found.append({'pid':int(p.name),'rss':rss,'state':f[0]})
 return found

NATIVE_SOURCE_SHA='2303197eeff30da4a4c552f1131de64cd871f1fe0ff6ea3afe60010db68645b4'
NATIVE_MEMBERS=frozenset(('program.ll','runtime.c','program.o','runtime.o','executable'))
# LLVM 19.1.7 CompilerInstance/Path/FileOutputBuffer: fixed output stems only.
NATIVE_TEMP_RE=re.compile(r'(?:(program|runtime)-[0-9a-f]{8}\.o\.tmp|(executable)\.tmp[0-9a-f]{7})')
NATIVE_TEMP_COUNTS={'program':1,'runtime':1,'executable':2}
def _native_temp_family(name):
 match=NATIVE_TEMP_RE.fullmatch(name)
 return (match.group(1) or match.group(2)) if match else None
def _native_record_temp(path,registration):
 family=_native_temp_family(Path(path).name)
 if family is None:return
 names=registration['temporary_members'][family];name=Path(path).name
 if name not in names:names.append(name)
 need(len(names)<=NATIVE_TEMP_COUNTS[family],'excess distinct native temporary members')
def _directory_identity(path):
 p=real(path);s=p.lstat();need(stat.S_ISDIR(s.st_mode),'native output parent directory');return {'path':str(p),'device':s.st_dev,'inode':s.st_ino,'mode':s.st_mode}

def _native_plan(invocation,kind,argv,cwd):
 if kind!='public-runtime' or not sys.platform.startswith('linux') or len(argv)<2 or argv[1]!='compile':return None
 # Invalid/early-rejection CLI tuples never receive a temporary-path exception.
 if argv.count('--output')!=1 or argv.count('--backend')!=1:return None
 oi=argv.index('--output');bi=argv.index('--backend')
 if oi+1>=len(argv) or bi+1>=len(argv) or argv[bi+1]!='llvm':return None
 output=Path(argv[oi+1]);output=output if output.is_absolute() else Path(cwd)/output
 if os.path.lexists(output) or not output.parent.is_dir():return None
 parent=real(output.parent);inv,captured=verify_invocation(invocation);root=Path(inv['run_root'])
 need(root in parent.parents,'native workspace parent outside owned invocation')
 source=Path(captured['checkout']['path'])/'src/frontend/native.rs';row=identity(source);need(row['sha256']==NATIVE_SOURCE_SHA,'fixed native workspace lifecycle source')
 for p in parent.iterdir():check_deadline(invocation);need(not p.name.startswith('.oxid-native-'),'pre-existing native workspace prefix')
 return {'parent':_directory_identity(parent),'source':row,'compiler':identity(argv[0]),'output':str(output),'invocation_nonce':inv['nonce']}

def _native_register(plan,pid):
 if plan is None:return None
 need(type(pid) is int and pid>0,'actual native compiler PID')
 return dict(plan,pid=pid,workspace=str(Path(plan['parent']['path'])/('.oxid-native-'+str(pid)+'-0')),tolerated_disappearances=[],temporary_members={name:[] for name in NATIVE_TEMP_COUNTS})

def _native_known(path,registration):
 if registration is None:return False
 p=Path(path);workspace=Path(registration['workspace'])
 return p==workspace or p.parent==workspace and (p.name in NATIVE_MEMBERS or _native_temp_family(p.name) is not None)

def _native_enoent(path,registration):
 need(_native_known(path,registration),'vanished stable or unknown native output')
 _native_record_temp(path,registration)
 value=str(path)
 if value not in registration['tolerated_disappearances']:registration['tolerated_disappearances'].append(value)
 need(len(registration['tolerated_disappearances'])<=10,'finite native disappearance roster')


def _native_observed(path,st,registration):
 if registration is None:return
 p=Path(path);parent=Path(registration['parent']['path']);workspace=Path(registration['workspace'])
 if p.parent==parent and p.name.startswith('.oxid-native-'):need(p==workspace,'unregistered native workspace')
 if p==workspace:need(stat.S_ISDIR(st.st_mode) and stat.S_IMODE(st.st_mode)==0o700,'exact private native workspace directory')
 elif workspace in p.parents:
  need(p.parent==workspace and (p.name in NATIVE_MEMBERS or _native_temp_family(p.name) is not None) and stat.S_ISREG(st.st_mode),'unknown or special native workspace member')
  _native_record_temp(p,registration)

def _scan_disappearance(path,root,registration,transient_roots):
 if _native_known(path,registration):_native_enoent(path,registration);return
 need(Path(root)/'tmp' in Path(path).parents or any(p in Path(path).parents for p in transient_roots),'vanished stable output')

def _output_entries(root,registration,deadline,transient_roots=()):
 if registration is None:
  yield from root.rglob('*');return
 pending=[Path(root)]
 while pending:
  need(time.monotonic()<deadline,'output traversal deadline');directory=pending.pop()
  try:
   with os.scandir(directory) as entries:
    for entry in entries:
     need(time.monotonic()<deadline,'output traversal deadline');p=Path(entry.path)
     try:s=entry.stat(follow_symlinks=False)
     except FileNotFoundError:_scan_disappearance(p,root,registration,transient_roots);continue
     _native_observed(p,s,registration);yield p
     if stat.S_ISDIR(s.st_mode):pending.append(p)
  except FileNotFoundError:_scan_disappearance(directory,root,registration,transient_roots)

def _native_terminal(registration):
 if registration is None:return
 tick()
 parent=Path(registration['parent']['path']);need(_directory_identity(parent)==registration['parent'],'native output parent identity changed')
 need(not os.path.lexists(registration['workspace']),'native workspace residue after cleanup')
 for p in parent.iterdir():tick();need(not p.name.startswith('.oxid-native-'),'unexpected native workspace residue')
 need(identity(registration['source']['path'])==registration['source'],'native lifecycle source drift')

def _operation_process_cap(operation_kind,preparation,native_plan):
 # The private plan exists only after the live public tuple/environment policy
 # and fixed native source/output authority have been checked by _owned_probe.
 if not preparation and operation_kind=='public-runtime' and native_plan is not None:
  need(native_plan['source']['sha256']==NATIVE_SOURCE_SHA,'fixed public native process-chain source')
  return 5
 return 16 if not preparation and operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build','ci-production-build') else 4

def _supervise(tree_class,argv,cwd,environment,timeout,limit,*,preparation=False,output_root=None,operation_kind='probe',transient_roots=(),allowed_links=None,native_plan=None,output_monitor=None,production_invocation=None):
 """Owned tree, bounded streams and cleanup; cancellation always enters finally."""
 started=time.time_ns();deadline=time.monotonic()+timeout;tree=None;readers=[];streams={'stdout':bytearray(),'stderr':bytearray()};events=queue.Queue(maxsize=8);closed=set();exceeded=False;timed_out=False;cleanup=None;status=None;pid=None;failure=None
 if output_monitor is not None:output_monitor['operation']['deadline']=deadline
 native_registration=None;old_handlers={};peaks={'processes':0,'rss_bytes':0,'output_bytes':None if output_monitor is not None else 0}
 compile_operation=operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build','ci-production-build')
 policy={'processes':_operation_process_cap(operation_kind,preparation,native_plan),'rss_bytes':(2 if preparation or not compile_operation else 8)*1024**3,'output_bytes':128*1024**2 if preparation else 6*1024**3,'file_bytes':64*1024**2 if preparation else 2*1024**3,'process_memory_enforced_on':'linux','all_hosts':['wall','stream','output_file','output_total','owned_tree_cleanup'],'transient_roots':[str(p) for p in transient_roots]}
 if output_monitor is not None:policy['output_measurement']='sampled ledger; see public_output_ledger, no continuous output peak'
 def cancel(*_):raise Cancelled('qualification cancelled')
 def reader(pipe,name):
  try:
   while True:
    chunk=os.read(pipe.fileno(),65536);events.put((name,chunk))
    if not chunk:break
  except OSError as e:events.put(('error',repr(e)));events.put((name,b''))
  finally:pipe.close()
 try:
  if threading.current_thread() is threading.main_thread():
   for sig in (signal.SIGINT,signal.SIGTERM):old_handlers[sig]=signal.signal(sig,cancel)
  if production_invocation is not None:production_tick(production_invocation)
  tree=tree_class(argv,cwd,environment,new_session=True);pid=tree.child.pid;native_registration=_native_register(native_plan,pid);_public_output_operation_registration(output_monitor,native_registration)
  for name in streams:
   thread=threading.Thread(target=reader,args=(getattr(tree.child,name),name),daemon=True);thread.start();readers.append(thread)
  while len(closed)!=2 or tree.child.poll() is None:
   if production_invocation is not None:production_tick(production_invocation)
   if time.monotonic()>=deadline:timed_out=True;break
   if sys.platform.startswith('linux'):
    members=_sample_linux(pid);rss=sum(r['rss'] for r in members);peaks['processes']=max(peaks['processes'],len(members));peaks['rss_bytes']=max(peaks['rss_bytes'],rss);need(len(members)<=policy['processes'] and rss<=policy['rss_bytes'],'operation process/RSS bound')
   if output_monitor is not None:public_output_tick(output_monitor['invocation'],deadline=deadline)
   elif output_root is not None:
    total=0
    roots=(output_root,) if production_invocation is None else _production_output_roots(production_invocation,output_root)
    for p in (item for monitored in roots for item in _output_entries(monitored,native_registration,deadline,transient_roots)):
     if time.monotonic()>=deadline:timed_out=True;break
     try:s=p.lstat()
     except FileNotFoundError:
      if _native_known(p,native_registration):_native_enoent(p,native_registration)
      else:need(output_root/'tmp' in p.parents or any(root in p.parents for root in transient_roots),'vanished stable output')
      continue
     _native_observed(p,s,native_registration)
     if stat.S_ISLNK(s.st_mode):
      need(allowed_links is not None and str(p) in allowed_links,'unregistered output link');_verify_link(allowed_links[str(p)],hash_body=False);continue
     need(stat.S_ISREG(s.st_mode) or stat.S_ISDIR(s.st_mode),'special output member')
     if stat.S_ISREG(s.st_mode):
      file_limit=limit if operation_kind=='parser-helper' and p in (Path(argv[-1])/'stdout.jsonl',Path(argv[-1])/'stderr.txt') else policy['file_bytes']
      need(s.st_size<=file_limit,'output file bound');total+=s.st_size
    peaks['output_bytes']=max(peaks['output_bytes'],total);need(total<=policy['output_bytes'],'operation output byte bound')
    if timed_out:break
   try:name,data=events.get(timeout=.05)
   except queue.Empty:continue
   need(name!='error','stream reader failed: '+str(data))
   if not data:closed.add(name);continue
   room=limit-len(streams[name]);streams[name].extend(data[:room])
   if len(data)>room:exceeded=True;break
  if not timed_out and not exceeded:status=tree.child.wait(timeout=max(.01,deadline-time.monotonic()))
 except BaseException as error:failure=error
 finally:
  if tree is not None:
   try:
    tree.close()
    try:status=tree.child.wait(timeout=5)
    except Exception:tree.child.kill();status=tree.child.wait(timeout=2)
   except BaseException as error:cleanup=repr(error)
  for sig,handler in old_handlers.items():signal.signal(sig,handler)
  # Drain queued bytes without waiting on inherited pipes beyond cleanup grace.
  until=time.monotonic()+.5
  while any(t.is_alive() for t in readers) and time.monotonic()<until:
   try:name,data=events.get(timeout=.02)
   except queue.Empty:continue
   if name in streams and data:
    room=limit-len(streams[name]);streams[name].extend(data[:room]);exceeded|=len(data)>room
  for t in readers:t.join(timeout=.01)
  if any(t.is_alive() for t in readers):cleanup=cleanup or 'reader cleanup incomplete'
  if sys.platform.startswith('linux') and pid is not None and any(r['state']!='Z' for r in _sample_linux(pid)):cleanup=cleanup or 'owned process group remains'
 try:
  _native_terminal(native_registration)
  if failure is None and cleanup is None and not timed_out and not exceeded:_public_output_operation_terminal(output_monitor,deadline=deadline)
 except BaseException as error:failure=failure or error
 output_preservation=_public_output_failure_audit(output_monitor,deadline=deadline) if failure is not None or cleanup is not None or timed_out or exceeded else None
 result={'public_output_ledger':_public_output_report(output_monitor),'public_output_failure_preservation':output_preservation,'native_workspace':native_registration,'pid':pid,'started_ns':started,'completed_ns':time.time_ns(),'status':status,'timed_out':timed_out,'stream_limit_exceeded':exceeded,'cleanup_complete':cleanup is None,'cleanup_error':cleanup,'failure':repr(failure) if failure else None,'observed_peaks':peaks,'operation_policy':policy,'preparation_resource_policy':({'wall_seconds':timeout,'stream_bytes':limit,'output_bytes':128*1024**2,'file_bytes':64*1024**2,'linux_group_processes':4,'linux_group_rss_bytes':2*1024**3,'linux_child_cpu_seconds':120,'linux_child_address_space_bytes':1024**3} if preparation else None)}
 return result,{n:bytes(v) for n,v in streams.items()}

def _target_stat(path):
 s=Path(path).lstat();need(stat.S_ISREG(s.st_mode),'regular executable target');return {n:getattr(s,n) for n in ('st_dev','st_ino','st_mode','st_size','st_mtime_ns','st_ctime_ns')}

def _verify_link(row,hash_body=True):
 path=Path(row['path']);need(path.is_symlink() and os.readlink(path)==row.get('link_text',row['target']) and str(path.resolve(strict=True))==row['target'],'registered executable link drift')
 need(_target_stat(row['target'])==row['target_stat'],'registered target stat drift')
 if hash_body:need(identity(row['target'])==row['target_identity'],'registered executable body drift')

def _verify_view_rows(rows,_force_full=False,_stat_owner=None):
 if _stat_owner is not None and not _force_full:
  invocation,permit=_stat_owner;_runtime_permit_owner(permit,invocation,'public');need(_active_owner(invocation) is not None,'live public view owner');need(canonical(rows)==canonical(list(_VIEWS.get(invocation,{}).values())),'original public executable view roster')
 replay=globals().get('_PUBLIC_REPLAY')
 if replay is not None and not _force_full:
  _replay_owner(replay['permit'],replay['invocation']);need(canonical(rows)==replay['views_raw'],'original replay executable view roster')
 groups={};targets={}
 for r in rows:
  tick();_verify_link(r,hash_body=False);target=r['target']
  if target in targets:need(targets[target]==(r['target_identity'],r['target_stat']),'shared executable target authority differs')
  else:
   if _force_full or (replay is None and _stat_owner is None):need(identity(target)==r['target_identity'],'registered executable body drift')
   elif replay is not None:need(replay['view_bodies'].get(target)==r['target_identity'],'original replay executable target body')
   targets[target]=(r['target_identity'],r['target_stat'])
  groups.setdefault(Path(r['path']).parent,set()).add(Path(r['path']).name)
 for root,names in groups.items():real(root);need(names=={'cargo','rustc'} and {p.name for p in root.iterdir()}==names,'closed registered executable view')
 for target,(_,expected) in targets.items():tick();need(_target_stat(target)==expected,'registered executable changed during view verification')
 return {r['path']:r for r in rows}

def register_executable_view(invocation,view,toolchain):
 inv,captured,state=_run_state(invocation);need(captured['scope'] in ('parser','combined'),'Linux parser view scope required')
 view=real(view);toolchain=real(toolchain);expected={state['root']/'parser'/((('build-control-' if control else 'build-')+profile))/'rust-bin' for control in (False,True) for profile in ('debug','release')};need(view in expected,'fixed parser executable view location')
 authority_path=Path(captured['checkout']['path'])/'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json';raw=body(authority_path);need(sha(raw)=='02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0','fixed compiler pair authority')
 authority=decode(raw);wanted={r['path']:r for r in authority['compiler_files']};need({p.name for p in view.iterdir()}=={'cargo','rustc'},'exact two executable links');rows=[]
 for name in ('cargo','rustc'):
  target=real(toolchain/'bin'/name);r=identity(target);need(dict(r,path='bin/'+name)==wanted['bin/'+name],'fixed original compiler body')
  item={'path':str(view/name),'target':str(target),'target_identity':r,'target_stat':_target_stat(target)};_verify_link(item);rows.append(item)
 registered=_VIEWS.setdefault(invocation,{})
 for r in rows:need(r['path'] not in registered or registered[r['path']]==r,'executable view registration changed');registered[r['path']]=r
 return _copy(rows)

class RuntimePermit:
 __slots__=('_document',)
 def __init__(self,value,token=None):
  need(token is _TOKEN,'producer-issued runtime permit required');object.__setattr__(self,'_document',canonical(value))
 def __setattr__(self,*_):raise TypeError('immutable runtime permit')
 def document(self):return decode(self._document)

_RUNTIME={}
def _reverify_issued_probe(probe,invocation):
 need(probe in _PROBES and _PROBES[probe]['invocation'] is invocation,'same invocation issued build probe required')
 d=probe.document()
 return verify_probe(probe,invocation,**{k:d[k] for k in ('argv','cwd','environment','timeout_seconds','stream_limit_bytes','operation_kind')})

def register_parser_runtime(invocation,checkpoint,helper_probes,contract_root):
 inv,captured,state=_run_state(invocation);check_deadline(invocation)
 need(captured['scope'] in ('parser','combined'),'parser runtime scope')
 need(type(helper_probes) in (list,tuple) and len(helper_probes)==4 and len(set(helper_probes))==4,'four distinct actual helper probes')
 root=state['root']/'parser';checkpoint=real(checkpoint);need(checkpoint==root/'package-source-session.json','fixed parser runtime checkpoint')
 cp=decode(body(checkpoint));need(cp['invocation']==inv and cp['root']==str(root) and cp['status']=='NotReady' and cp['execution_qualified'] is False,'actual successor checkpoint context')
 binaries={};pins=[identity(checkpoint)];commands=[]
 for control in (False,True):
  for profile in ('debug','release'):
   probe=helper_probes[len(commands)];d=_reverify_issued_probe(probe,invocation)
   out=root/(('build-control-' if control else 'build-')+profile)/'result'
   expected=[sys.executable,'-B',str(root/'helpers/build.py'),'--overlay',str(root/('control-source' if control else 'source')/'overlay-manifest.json'),'--profile',profile,'--output',str(out)]
   need(d['operation_kind']=='parser-helper' and d['argv']==expected and d['status']==0 and d['cwd']==str(root),'actual fixed parser helper build')
   receipt_path=out/'build-receipt.json';receipt=decode(body(receipt_path,8*1024**2));need(decode(probe.stdout())==receipt and probe.stderr()==b'','actual helper receipt equality')
   binary=real(receipt['binary']['path']);need(out/'target' in binary.parents,'parser binary in actual helper target')
   row=identity(binary);need(row['sha256']==receipt['binary']['sha256'],'helper binary body binding')
   need(cp['prepared_ns']<=d['started_ns']<=d['completed_ns'],'build follows actual preparation')
   key=('control-' if control else 'observed-')+profile;binaries[key]=row;pins.extend([identity(receipt_path),row]);commands.append(probe)
 need(len({r['sha256'] for r in binaries.values()})==4,'four distinct parser binaries')
 authority=Path(captured['checkout']['path'])/'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json';raw=body(authority);need(sha(raw)=='02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0','fixed parser runtime helper authority');pins.append(identity(authority))
 for r in decode(raw)['helper_files']:
  row=identity(root/'helpers'/r['path']);need(dict(row,path=r['path'])==r,'fixed parser runtime helper body');pins.append(row)
 contract=real(contract_root);need(contract.is_dir(),'contract directory required');contract_files=[]
 for p in sorted(contract.rglob('*')):
  check_deadline(invocation);s=p.lstat();need(not p.is_symlink() and (stat.S_ISREG(s.st_mode) or stat.S_ISDIR(s.st_mode)),'regular contract closure')
  if stat.S_ISREG(s.st_mode):contract_files.append(identity(p))
 need(0<len(contract_files)<=1024 and sum(r['bytes'] for r in contract_files)<=64*1024**2,'bounded runtime contract closure');pins.extend(contract_files)
 value={'kind':'parser','invocation_nonce':inv['nonce'],'root':str(root),'checkpoint':str(checkpoint),'authority_sha256':cp['authority']['sha256'],'binaries':binaries,'contract_root':str(contract),'body_pins':pins}
 permit=RuntimePermit(value,_TOKEN);_RUNTIME[permit]={'invocation':invocation,'probes':tuple(commands),'contract_files':tuple(r['path'] for r in contract_files)};return permit

def register_public_runtime(invocation,build_probes,metadata_probes):
 inv,captured,state=_run_state(invocation);check_deadline(invocation)
 need(captured['scope'] in ('public','combined'),'public runtime scope')
 need(type(build_probes) in (list,tuple) and len(build_probes)==4 and len(set(build_probes))==4,'four distinct public build probes')
 need(type(metadata_probes) in (list,tuple) and len(metadata_probes)==2 and len(set(metadata_probes))==2,'two actual metadata probes')
 pins=[];binaries={};probes=[]
 for i,role in enumerate(('ordinary','lifecycle')):
  source=state['root']/'source-preparation/public'/role;target=state['root']/'targets'/('public-'+role)
  metadata=metadata_probes[i];md=_reverify_issued_probe(metadata,invocation)
  need(md['operation_kind']=='probe' and md['status']==0 and md['argv']==['cargo','metadata','--format-version','1','--no-deps','--locked','--offline'] and md['cwd']==str(source),'actual public metadata probe')
  value=decode(metadata.stdout());need(metadata.stderr()==b'' and value['version']==1 and value['resolve'] is None and len(value['packages'])==1,'exact public package metadata')
  package=value['packages'][0];need(package['id']=='path+'+source.as_uri()+'#oxid@0.9.0' and package['manifest_path']==str(source/'Cargo.toml') and value['target_directory']==str(target),'path qualified current package')
  need(any(t['name']=='oxid' and t['kind']==['bin'] and t['src_path']==str(source/'src/cli.rs') for t in package['targets']),'actual current CLI entrypoint');probes.append(metadata)
  for j,profile in enumerate(('debug','release')):
   probe=build_probes[2*i+j];d=_reverify_issued_probe(probe,invocation)
   need(d['operation_kind']=='public-build' and d['status']==0 and d['argv']==['cargo','build','--bin','oxid','--locked','--offline']+(['--release'] if profile=='release' else []) and d['cwd']==str(source),'actual full public build')
   need(d['environment'].get('CARGO_TARGET_DIR')==str(target) and md['completed_ns']<=d['started_ns'],'current target and metadata/build order')
   name='oxid.exe' if os.name=='nt' else 'oxid';emitted=target/profile/name;copied=state['root']/'public-build'/role/('oxid-'+profile+('.exe' if os.name=='nt' else ''))
   actual=identity(emitted);copy=identity(copied);need((actual['bytes'],actual['sha256'])==(copy['bytes'],copy['sha256']),'copied actual public binary')
   need(d['started_ns']<=emitted.stat().st_mtime_ns<=d['completed_ns'],'fresh emitted public binary window')
   binaries[role+'-'+profile]=copy;pins.extend([actual,copy]);probes.append(probe)
 value={'kind':'public','invocation_nonce':inv['nonce'],'root':str(state['root']),'binaries':binaries,'body_pins':pins}
 permit=RuntimePermit(value,_TOKEN);_RUNTIME[permit]={'invocation':invocation,'probes':tuple(probes)};return permit

_PUBLIC_CONTRACTS={};_GUARD_LINKS={};_NATIVE={}
def register_public_contracts(invocation,runtime_permit,contract_root,path_map,amendment_root):
 d=_runtime_permit(runtime_permit,invocation,'public');need(runtime_permit not in _PUBLIC_CONTRACTS,'contracts already registered')
 inv,captured=verify_invocation(invocation);repo=Path(captured['checkout']['path']);path=repo/'tests/qualification/unit4_public_v3/contracts.py'
 module=_module(path,'_oxid_package_dependency_unit4_public_contract_policy_v1','170515201950aa6018aa176e9d02720d7fdca5f9e52fae123a3a01c71d8a6ac9')
 root=real(contract_root);amendment=real(amendment_root);need(type(path_map) is dict and 0<len(path_map)<=5000,'bounded explicit contract path map')
 paths=set()
 for base in (root,amendment):
  for p in base.rglob('*'):
   check_deadline(invocation);s=p.lstat();need(not p.is_symlink() and (stat.S_ISDIR(s.st_mode) or stat.S_ISREG(s.st_mode)),'regular public contract closure')
   if stat.S_ISREG(s.st_mode):paths.add(p);need(len(paths)<=20000,'public contract file bound')
 for name,value in path_map.items():need(type(name) is str and type(value) is str,'explicit contract relocation');paths.add(real(value))
 need(len(paths)<=20000,'public contract file bound');rows=[identity(p) for p in sorted(paths)];need(sum(r['bytes'] for r in rows)<=256*1024**2,'public contract byte bound')
 contracts=module.Contracts(root,path_map=path_map,amendment_root=amendment);check_deadline(invocation)
 rosters={section:contracts.roster(section) for section in ('public','original','predecessors','lifecycle','guards')}
 _PUBLIC_CONTRACTS[runtime_permit]={'contracts':contracts,'rosters':rosters,'pins':rows,'helper':identity(path),'root':root};return runtime_permit

def _public_case(permit,invocation,runtime_case):
 return _public_case_verified(_runtime_permit(permit,invocation,'public'),permit,runtime_case)

def _public_case_verified(d,permit,runtime_case):
 # Private pure derivation from the live permit just authenticated by the caller.
 need(permit in _PUBLIC_CONTRACTS,'authenticated public contracts required');c=_PUBLIC_CONTRACTS[permit]
 need(identity(c['helper']['path'])==c['helper'],'public contract helper changed')
 need(type(runtime_case) is dict and set(runtime_case)=={'section','index','variant'},'closed public tuple identity')
 section,index,variant=(runtime_case[k] for k in ('section','index','variant'))
 need(section in c['rosters'] and type(index) is int and 0<=index<len(c['rosters'][section]) and variant in ('ordinary','observer'),'finite public tuple')
 row=c['rosters'][section][index];need(row['scope']=='execute','only actual-host required tuple can execute')
 root=Path(d['root'])/'public-runtime/collection';role='lifecycle' if variant=='observer' else 'ordinary';binary=d['binaries'][role+'-'+row['profile']]['path']
 if section=='guards':
  base=root/'guards'/('%03d'%index);fixture_root=base/'fixtures';fixture=fixture_root/row['case']['source_case_id'];output=base/'occupied-output' if row['group']=='occupied' else fixture/'out.bin'
  args=[x.replace('${FIXTURE_ROOT}',str(fixture_root)).replace('${OUTPUT}',str(output)) for x in row['argv_template'][1:]];evidence=base/variant
 else:
  evidence=root/section/('%05d'%index)/variant;fixture=evidence/'fixture';args=row['argv_template'][1:]
 return d,row,[binary]+args,fixture,evidence

def register_public_guard(invocation,runtime_permit,runtime_case):
 d,row,argv,fixture,evidence=_public_case(runtime_permit,invocation,runtime_case)
 need(runtime_case['section']=='guards' and row['group']=='occupied' and row['case']['output_kind']=='symlink','exact occupied symlink tuple')
 base=evidence.parent;link=base/'occupied-output';target=base/'preserved-target'
 need(body(target)==b'Oxid Unit3 no-clobber sentinel\x00\xff\n','exact guard sentinel')
 item={'path':str(link),'target':str(target),'link_text':'preserved-target','target_identity':identity(target),'target_stat':_target_stat(target)};_verify_link(item)
 links=_GUARD_LINKS.setdefault(invocation,{})
 need(str(link) not in links or links[str(link)]==item,'guard link registration changed');links[str(link)]=item;return _copy(item)

def register_public_native(invocation,compile_probe,*,runtime_permit):
 d=_runtime_permit(runtime_permit,invocation,'public');need(compile_probe in _PROBES,'actual compile probe required');owner=_PROBES[compile_probe]
 need(owner['invocation'] is invocation and owner.get('runtime_permit') is runtime_permit,'native compile owner')
 doc=compile_probe.document();case=doc['runtime_case']
 verified=verify_probe(compile_probe,invocation,**{k:doc[k] for k in ('argv','cwd','environment','timeout_seconds','stream_limit_bytes','operation_kind')},runtime_permit=runtime_permit,runtime_case=case)
 need(verified['status']==0 and doc['operation_kind']=='public-runtime' and doc['argv'][1]=='compile','successful actual compile required')
 _,row,argv,fixture,evidence=_public_case(runtime_permit,invocation,case);original=fixture/'out.bin';copied=evidence/'native-source-free/program'
 original_row=identity(original);copy=identity(copied);need((copy['bytes'],copy['sha256'])==(original_row['bytes'],original_row['sha256']),'source-free native copy equality')
 raw=body(copied);need(len(raw)>=64 and raw[:7]==b'\x7fELF\x02\x01\x01' and int.from_bytes(raw[16:18],'little') in (2,3) and int.from_bytes(raw[18:20],'little')==62,'native x86_64 ELF')
 need(doc['started_ns']<=original.stat().st_mtime_ns<=doc['completed_ns'],'native emitted window')
 permit=RuntimePermit({'kind':'native','invocation_nonce':d['invocation_nonce'],'runtime_case':case,'body_pins':[original_row,copy],'binary':copy},_TOKEN)
 _NATIVE[permit]={'invocation':invocation,'runtime_permit':runtime_permit,'compile_probe':compile_probe};return permit

_PUBLIC_TOOLS={}
def register_public_tools(invocation,runtime_permit,setup_probes):
 d=_runtime_permit(runtime_permit,invocation,'public');need(runtime_permit not in _PUBLIC_TOOLS,'runtime tools already registered')
 need(type(setup_probes) in (list,tuple) and len(setup_probes) in (3,4) and len(set(setup_probes))==len(setup_probes),'closed actual runtime setup probes')
 root=Path(d['root'])/'public-runtime/tools/main';docs=[_reverify_issued_probe(p,invocation) for p in setup_probes]
 need(all(x['cwd']==str(root) for x in docs),'same main runtime purpose required')
 env=dict(docs[0]['environment'])
 allowed={'HOME','CARGO_HOME','RUSTUP_HOME','RUSTUP_TOOLCHAIN','PYTHONDONTWRITEBYTECODE','PYTHONNOUSERSITE','PYTHONOPTIMIZE','CARGO_INCREMENTAL','CARGO_BUILD_JOBS','CARGO_TERM_COLOR','TEMP','TMP','TMPDIR','VSLANG','PATH','LANG','LC_ALL','SDKROOT','DEVELOPER_DIR','RUSTC','CC','CXX','AR','UNIT4_ADAPTER_ROOT','UNIT4_REAL_LLVM','LD_LIBRARY_PATH','SystemRoot','SYSTEMROOT','WINDIR','COMSPEC','PATHEXT','USERPROFILE','ProgramFiles','ProgramFiles(x86)','ProgramW6432','VSINSTALLDIR','VCINSTALLDIR','VCToolsInstallDir','VSCMD_ARG_TGT_ARCH','VSCMD_ARG_HOST_ARCH','LIB','INCLUDE','LIBPATH'}
 allowed.update('CARGO_TARGET_'+target+'_LINKER' for target in ('X86_64_UNKNOWN_LINUX_GNU','X86_64_APPLE_DARWIN','AARCH64_APPLE_DARWIN','X86_64_PC_WINDOWS_MSVC'))
 need(set(env)<=allowed,'closed producer runtime environment keys')
 need(not any(k in env for k in ('LD_PRELOAD','DYLD_INSERT_LIBRARIES','PYTHONPATH','UNIT4_TOOL_LOG','UNIT4_TOOL_EVIDENCE','UNIT4_RACE_OUTPUT','UNIT4_LIFECYCLE_LOG','OXID_LLVM_BIN')),'unsafe runtime base override')
 inv,captured=verify_invocation(invocation);helper_root=Path(captured['checkout']['path'])/'tests/qualification/unit4_public_v3';need(env.get('UNIT4_ADAPTER_ROOT')==str(helper_root),'fixed runtime adapter root')
 pins=[];names=('clang','opt','ld.lld')
 if sys.platform.startswith('linux'):
  need(len(docs)==3 and all(x['operation_kind']=='runtime-tool-version' and x['status']==0 and x['environment']==env for x in docs),'Linux main requires three genuine LLVM version probes')
  wrappers=root/'tool-wrappers';native=real(env['UNIT4_REAL_LLVM']);real(env['LD_LIBRARY_PATH'])
  for name,probe,doc in zip(names,setup_probes,docs):
   need(doc['argv']==[str(native/name),'--version'] and probe.stderr()==b'' and re.search(rb'(?<![0-9])19\.1\.7(?=$|[ \r\n(])',probe.stdout()),'fixed actual LLVM tool version')
   row=identity(wrappers/name);need(row['sha256']=='60382c17f7c51217d2e53354a423666aa644ed678fcce21b7d1507abfd7f16ed','fixed actual tool wrapper');pins.append(row)
 else:
  need(len(docs)==4 and docs[0]['operation_kind']=='runtime-trap-build' and docs[0]['status']==0 and 'UNIT4_REAL_LLVM' not in env and 'LD_LIBRARY_PATH' not in env,'non-native actual trap setup')
  wrappers=root/'no-tool-traps';env['OXID_LLVM_BIN']=str(wrappers);binary=identity(root/('no-tool-trap.exe' if os.name=='nt' else 'no-tool-trap'));pins.append(binary)
  for name,probe,doc in zip(names,setup_probes[1:],docs[1:]):
   expected=dict(env,UNIT4_TOOL_LOG=str(root/(name+'-trap-control.jsonl')))
   need(doc['operation_kind']=='runtime-trap-control' and doc['status']==121 and doc['argv']==[str(wrappers/name),'--version'] and doc['environment']==expected and probe.stdout()==b'' and probe.stderr()==b'unexpected native tool invocation\n','exact actual trap control')
   row=identity(wrappers/name);need((row['bytes'],row['sha256'])==(binary['bytes'],binary['sha256']),'actual compiled trap copy');pins.append(row)
   if os.name=='nt':
    alias=identity(wrappers/(name+'.exe'));need((alias['bytes'],alias['sha256'])==(binary['bytes'],binary['sha256']),'Windows actual trap alias');pins.append(alias)
 env['OXID_LLVM_BIN']=str(wrappers);need({p.name for p in wrappers.iterdir()}=={Path(r['path']).name for r in pins if Path(r['path']).parent==wrappers},'closed runtime wrapper roster')
 _PUBLIC_TOOLS[runtime_permit]={'environment':env,'pins':pins,'probes':tuple(setup_probes)};return runtime_permit

def _public_environment(permit,invocation,kind,case):
 selected=None if kind=='generator' else _public_case(permit,invocation,case)
 return _public_environment_verified(permit,invocation,kind,case,selected)

def _public_environment_verified(permit,invocation,kind,case,selected):
 need(permit in _PUBLIC_TOOLS,'actual runtime tool setup required');tools=_PUBLIC_TOOLS[permit]
 for row in tools['pins']:check_deadline(invocation);need(identity(row['path'])==row,'runtime wrapper body drift')
 env=dict(tools['environment'])
 if kind=='generator':return env
 _,row,argv,fixture,evidence=selected
 env.update(UNIT4_TOOL_LOG=str(evidence/'tools.jsonl'),UNIT4_TOOL_EVIDENCE=str(evidence))
 if case['variant']=='observer':env['UNIT4_LIFECYCLE_LOG']=str(evidence/'lifecycle.jsonl')
 if case['section']=='guards' and row['group']=='race':env['UNIT4_RACE_OUTPUT']=str(fixture/'out.bin')
 return env

def _public_runtime_recipe(permit,native_permit,invocation,kind,case,argv,cwd,environment,timeout,limit):
 d=_runtime_permit(permit,invocation,'public') if kind=='generator' else _runtime_permit_owner(permit,invocation,'public')
 if _PUBLIC_REPLAY is not None:_replay_owner(permit,invocation)
 selected=None if kind=='generator' else _public_case_verified(d,permit,case)
 if selected is not None:_public_selected_pair(d,case,selected,invocation)
 need(environment==_public_environment_verified(permit,invocation,kind,case,selected),'exact producer runtime environment');need(timeout==(60 if kind=='generator' else 120) and limit==8*1024**2,'fixed public runtime limits')
 if kind=='generator':
  need(case is None and native_permit is None and permit in _PUBLIC_CONTRACTS,'generator contract context');c=_PUBLIC_CONTRACTS[permit]['contracts'];root=Path(d['root'])/'public-runtime/original-generator';path=root/'generate_fixtures.py'
  matches=[r for r in c.verified if r.get('authority_path','').endswith('/typed_project_unit1_compatibility/generate_fixtures.py')];need(len(matches)==1,'exact retained Unit1 generator authority');generator=matches[0];need(identity(path)['sha256']==generator['sha256'] and identity(path)['bytes']==generator['bytes'],'retained original generator body')
  need(argv==[sys.executable,str(path)] and cwd==root,'exact original generator command');return identity(path)
 _,row,expected,fixture,evidence=selected
 if kind=='public-runtime':need(native_permit is None and argv==expected and cwd==fixture,'exact frozen public compiler tuple');return None
 need(kind=='public-native' and type(native_permit) is RuntimePermit and native_permit in _NATIVE,'issued native permit required');owner=_NATIVE[native_permit]
 need(owner['invocation'] is invocation and owner['runtime_permit'] is permit,'same invocation native permit');n=native_permit.document();need(n['runtime_case']==case,'native compile tuple association')
 for pin in n['body_pins']:need(identity(pin['path'])==pin,'native compiled body drift')
 need(argv==[n['binary']['path']] and cwd==evidence/'native-source-free','exact source-free native argv/cwd');return None

_HOSTED={}
def register_public_hosted(invocation,runtime_permit,ledger,output,uid,gid,setpriv):
 d=_runtime_permit(runtime_permit,invocation,'public');inv,captured=verify_invocation(invocation)
 need(sys.platform.startswith('linux') and type(uid) is int and uid>0 and type(gid) is int and gid>0,'Linux nonroot hosted identity')
 import pwd
 account=pwd.getpwuid(uid);need(account.pw_gid==gid,'existing hosted account identity')
 root=Path(d['root'])/'public-runtime/hosted';ledger=real(ledger);output=real(output)
 need(ledger==root/'transport.json' and output==root/'observations','fixed hosted ledger/output')
 row=identity(ledger);value=decode(body(ledger));need(value['schema']=='package-dependency-unit4-public-hosted-transport-v1' and value['invocation']==inv and value['root']==str(root) and value['output']==str(output),'hosted actual invocation')
 need(value['status']=='NotReady' and value['execution_qualified'] is False and value['qualification_claim'] is False and value['source_manifest_sha256']=='f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5','hosted successor source distinction')
 need(value['identity']=={'effective_uid':uid,'effective_gid':gid,'supplementary_groups':[]},'exact hosted account ledger')
 repo=Path(captured['checkout']['path']);relative='tests/qualification/package_dependency_unit4_public_v1';manifest=decode(context_api()._git(repo,'show',captured['checkout']['head']+':'+relative+'/package-manifest.json'))
 names={'binding.py','build.py','test_binding.py','test_build.py','runtime.py','test_runtime.py','hosted.py','test_hosted.py'}
 need(manifest['schema']=='oxid-package-dependency-unit4-public-package-v1' and {r['path'] for r in manifest['files']}==names and len(manifest['files'])==len(names),'committed hosted helper package roster')
 need(decode(body(repo/relative/'package-manifest.json'))==manifest,'working hosted package versus captured commit')
 entries=value['entries'];need(type(entries) is list and 0<len(entries)<=20000,'bounded hosted transport entries');by_logical={r['logical_path']:r for r in entries};need(len(by_logical)==len(entries),'duplicate hosted transport input')
 pins=[row];actual_files=set();total=0
 for r in entries:
  check_deadline(invocation);need(set(r)=={'logical_path','physical_path','bytes','sha256','mode','roles'},'closed hosted entry')
  logical=real(r['logical_path']);physical=real(r['physical_path']);need(physical==root/'readonly'/str(logical).lstrip('/'),'exact hosted namespace mapping')
  expected={'bytes':r['bytes'],'sha256':r['sha256']};a=identity(logical);b=identity(physical);need({k:a[k] for k in expected}==expected and {k:b[k] for k in expected}==expected,'hosted actual copied input bytes')
  need(r['mode'] in (0o444,0o555) and stat.S_IMODE(physical.stat().st_mode)==r['mode'],'hosted readonly input mode');pins.extend([a,b]);actual_files.add(physical);total+=r['bytes']
 need(total<=512*1024**2,'hosted transport byte cap')
 found=set();found_dirs=set();expected_dirs=set()
 for path in actual_files:
  expected_dirs.update(p for p in path.parents if root/'readonly' in p.parents)
 for p in (root/'readonly').rglob('*'):
  check_deadline(invocation);s=p.lstat();need(not p.is_symlink() and (stat.S_ISREG(s.st_mode) or stat.S_ISDIR(s.st_mode)),'regular hosted readonly closure')
  if stat.S_ISREG(s.st_mode):found.add(p)
  else:found_dirs.add(p);need(stat.S_IMODE(s.st_mode)==0o555,'readonly hosted directory mode')
 need(found==actual_files and found_dirs==expected_dirs,'closed hosted readonly file/directory roster')
 for r in manifest['files']:
  path=repo/relative/r['path'];need(dict(identity(path),path=r['path'])==r and str(path) in by_logical,'committed hosted helper body and copied closure')
 for key,binary in d['binaries'].items():
  role,profile=key.rsplit('-',1);need(value['cfgs'][role]['binaries'][profile]['path']==binary['path'] and value['cfgs'][role]['binaries'][profile]['sha256']==binary['sha256'],'hosted actual new binary')
 controller=value['controller'];setpriv=real(setpriv);need(setpriv.name=='setpriv' and controller['setpriv']==identity(setpriv),'actual hosted setpriv body')
 python=real(controller['python']['path']);need(controller['python']==identity(python) and python==Path(sys.executable).resolve(),'actual hosted Python body')
 helper=repo/relative/'hosted.py';need(controller['helper']==str(helper),'fixed hosted helper location');copied_helper=by_logical[str(helper)]['physical_path']
 argv=[str(setpriv),'--reuid='+str(uid),'--regid='+str(gid),'--clear-groups','--no-new-privs',str(python),'-B',copied_helper,'worker','--ledger',str(ledger),'--ledger-sha256',row['sha256'],'--out',str(output),'--expected-uid',str(uid),'--expected-gid',str(gid)]
 permit=RuntimePermit({'kind':'hosted','invocation_nonce':inv['nonce'],'body_pins':pins+[identity(setpriv),identity(python)],'argv':argv,'cwd':str(output)},_TOKEN)
 _HOSTED[permit]={'invocation':invocation,'runtime_permit':runtime_permit};return permit

def _hosted_runtime_recipe(permit,invocation,argv,cwd,environment,timeout,limit):
 need(type(permit) is RuntimePermit and permit in _HOSTED and _HOSTED[permit]['invocation'] is invocation,'issued hosted permit required');d=permit.document()
 _runtime_permit(_HOSTED[permit]['runtime_permit'],invocation,'public')
 need(environment=={'HOME':d['cwd'],'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','PYTHONDONTWRITEBYTECODE':'1','PYTHONNOUSERSITE':'1'},'exact minimal hosted environment')
 need(argv==d['argv'] and str(cwd)==d['cwd'] and timeout==600 and limit==8*1024**2,'exact hosted worker command/bounds')
 for r in d['body_pins']:check_deadline(invocation);need(identity(r['path'])==r,'hosted input drift')
 return identity(argv[7])

def _runtime_permit_owner(permit,invocation,kind):
 need(type(permit) is RuntimePermit and permit in _RUNTIME and _RUNTIME[permit]['invocation'] is invocation,'live same-invocation runtime permit required')
 d=permit.document();need(d['kind']==kind,'runtime permit domain');return d

_PUBLIC_REPLAY=None
_PUBLIC_INPUTS=[]

@contextmanager
def public_inputs(invocation,permit):
 """Read-only public input phase; inactive build tools retain route/stat checks.

 Actual build-tool bodies are reread by full machine and replay boundaries.
 LLVM/runtime bodies and all probe evidence remain fresh reads in this phase.
 """
 with checked_scope(invocation):
  _runtime_permit_owner(permit,invocation,'public');need(permit in _PUBLIC_TOOLS,'registered public setup required')
  state=(invocation,permit,threading.get_ident(),tuple(_PUBLIC_TOOLS[permit]['probes']))
  need(not _PUBLIC_INPUTS or _PUBLIC_INPUTS[-1][:3]==state[:3],'mixed public input phase')
  _PUBLIC_INPUTS.append(state)
  try:
   yield
   need(_PUBLIC_INPUTS[-1] is state and threading.get_ident()==state[2],'public input owner drift')
   _runtime_permit_owner(permit,invocation,'public')
  finally:
   need(_PUBLIC_INPUTS and _PUBLIC_INPUTS[-1] is state,'public input phase stack drift');_PUBLIC_INPUTS.pop()

def _public_probe_view_owner(record,invocation,kind,permit):
 if kind in ('public-runtime','public-native'):
  if _active_owner(invocation) is None:return None
  _runtime_permit_owner(permit,invocation,'public');need(_PROBES[record]['runtime_permit'] is permit,'original public probe permit');return (invocation,permit)
 if _PUBLIC_INPUTS:
  inv,issued,thread,probes=_PUBLIC_INPUTS[-1]
  need(inv is invocation and thread==threading.get_ident(),'public input invocation/thread')
  if any(record is p for p in probes):
   _runtime_permit_owner(issued,invocation,'public');return (invocation,issued)
 return None

def _runtime_permit_full(permit,invocation,kind):
 d=_runtime_permit_owner(permit,invocation,kind)
 for r in d['body_pins']:check_deadline(invocation);need(identity(r['path'])==r,'runtime input body drift')
 return d

def _replay_owner(permit,invocation):
 state=_PUBLIC_REPLAY
 if state is None:return None
 need(state['permit'] is permit and state['invocation'] is invocation and state['thread']==threading.get_ident(),'same issued public replay owner required')
 _active_owner(invocation);need(canonical(permit.document())==state['raw'],'immutable replay permit changed')
 return state

def _runtime_permit(permit,invocation,kind):
 if _PUBLIC_REPLAY is not None and kind=='public':
  _replay_owner(permit,invocation);return _runtime_permit_owner(permit,invocation,kind)
 return _runtime_permit_full(permit,invocation,kind)

@contextmanager
def public_replay(invocation,permit):
 """Read-only evidence replay; full compiler bodies reread at both boundaries."""
 global _PUBLIC_REPLAY
 need(_PUBLIC_REPLAY is None,'public replay cannot overlap')
 with checked_scope(invocation):
  d=_runtime_permit_full(permit,invocation,'public');views=_copy(list(_VIEWS.get(invocation,{}).values()));_verify_view_rows(views)
  state={'views_raw':canonical(views),'view_bodies':{r['target']:r['target_identity'] for r in views},'permit':permit,'invocation':invocation,'thread':threading.get_ident(),'raw':canonical(d),'bodies':{r['path']:r for r in d['body_pins']}}
  need(len(state['bodies'])==len(d['body_pins']),'closed public replay bodies')
  _PUBLIC_REPLAY=state
  try:
   yield
   _replay_owner(permit,invocation)
   need(_runtime_permit_full(permit,invocation,'public')==d,'public replay boundary drift');_verify_view_rows(views,_force_full=True)
  except BaseException:
   _poison(invocation);raise
  finally:_PUBLIC_REPLAY=None

def _probe_body_identity(row,record,invocation):
 if _PUBLIC_REPLAY is not None:
  state=_replay_owner(_PROBES[record]['runtime_permit'],invocation)
  if state['bodies'].get(row['path'])==row:return row
 return identity(row['path'])

def _public_selected_pair(d,case,selected,invocation):
 _,row,argv,_,_=selected;role='lifecycle' if case['variant']=='observer' else 'ordinary';profile=row['profile']
 need(profile in ('debug','release') and (case['variant']=='ordinary' or case['section'] in ('lifecycle','guards')),'exact selected role/profile')
 copy=d['binaries'][role+'-'+profile];name='oxid.exe' if os.name=='nt' else 'oxid'
 emitted=str(Path(d['root'])/'targets'/('public-'+role)/profile/name)
 pins={r['path']:r for r in d['body_pins']};need(len(pins)==len(d['body_pins']) and pins.get(copy['path'])==copy and argv[0]==copy['path'],'original selected copied binary authority')
 need(emitted in pins and (pins[emitted]['bytes'],pins[emitted]['sha256'])==(copy['bytes'],copy['sha256']),'original selected emitted pair authority')
 for path in (emitted,copy['path']):
  check_deadline(invocation)
  if _PUBLIC_REPLAY is not None:
   state=_replay_owner(_PUBLIC_REPLAY['permit'],invocation);need(state['bodies'].get(path)==pins[path],'original replay selected pair')
  else:need(identity(path)==pins[path],'selected public compiled body drift')

def _parser_runtime_recipe(permit,invocation,kind,argv,cwd,environment,timeout,limit):
 d=_runtime_permit(permit,invocation,'parser');root=Path(d['root']);need(cwd==root,'fixed parser runtime cwd')
 env={'HOME':str(root/'home'),'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','PYTHONDONTWRITEBYTECODE':'1','PYTHONNOUSERSITE':'1'}
 if kind in ('parser-collect','parser-passivity'):
  need(environment==env and limit==8*1024**2,'exact parser helper runtime environment/streams')
  for profile in ('debug','release'):
   if kind=='parser-collect':
    expected=[sys.executable,'-B',str(root/'helpers/run.py'),'--build-receipt',str(root/('build-'+profile)/'result/build-receipt.json'),'--contract-dir',d['contract_root'],'--checkpoint',d['checkpoint'],'--output',str(root/('collect-'+profile)/'result')];bound=1800
   else:
    expected=[sys.executable,'-B',str(root/'helpers/passivity.py'),'--instrumented',str(root/('build-'+profile)/'result/build-receipt.json'),'--control',str(root/('build-control-'+profile)/'result/build-receipt.json'),'--checkpoint',d['checkpoint'],'--output',str(root/('passivity-'+profile)/'result')];bound=900
   if argv==expected:need(timeout==bound,'exact parser helper time bound');return identity(argv[2])
  need(False,'exact full parser helper recipe')
 binaries={r['path']:key for key,r in d['binaries'].items()};need(argv[0] in binaries,'registered parser binary required');key=binaries[argv[0]];control=key.startswith('control-');profile=key.split('-',1)[1]
 if kind=='parser-u8':
  need(timeout==120 and limit==8*1024**2,'u8 runtime bounds')
  observer='frontend::parser::unit4_observer::observe_request';public='frontend::parser::u8_syntax_tests::u8_named_conversion_has_a_real_child_and_exact_trivia_origins_on_every_mode'
  if argv==[argv[0],'--exact',public,'--nocapture']:need(control and environment==env,'control-only public u8 test');return None
  need(argv==[argv[0],'--exact',observer,'--ignored','--nocapture'],'exact u8 observer command')
  case=environment.get('UNIT4_CASE_ID');need(case in ('narrow','widen','trivia','numeric'),'closed u8 case')
  directory=root/'u8-policy-controls'/(profile+('-control-' if control else '-observer-')+case);nonce=environment.get('UNIT4_NONCE');need(type(nonce) is str and re.fullmatch('[0-9a-f]{32}',nonce),'u8 nonce')
  env.update(UNIT4_SOURCE=str(directory/'source.ox'),UNIT4_RAW_OUTPUT=str(directory/'raw.json'),UNIT4_DISPLAY_PATH='u8-policy.ox',UNIT4_NONCE=nonce,UNIT4_CASE_ID=case,UNIT4_CONTROL=str(int(control)),UNIT4_ORIGINAL='1',UNIT4_REJECT_KIND='',UNIT4_REJECT_OCCURRENCE='0',UNIT4_RESERVE_FAIL_AT='0',UNIT4_DIRECT='0',UNIT4_TOKEN_LIMIT='200000',UNIT4_NODE_LIMIT='200000')
  need(environment==env,'exact u8 environment')
  inv,captured=verify_invocation(invocation);path=Path(captured['checkout']['path'])/'tests/qualification/unit4_parser_current/u8_policy_controls.py';policy=_module(path,'_oxid_package_dependency_unit4_u8_runtime_policy_v1','90807a872ef4b042abfc6f29d785a903c54fa00ae7dc364d26fe07dc6baa7b95')
  need(body(directory/'source.ox')==policy.CASES[case].encode(),'fixed original u8 case body');return identity(directory/'source.ox')
 need(kind=='parser-phase' and timeout==30 and limit==4096,'phase runtime bounds')
 inv,captured=verify_invocation(invocation);driver_path=Path(captured['checkout']['path'])/'tests/qualification/unit4_parser_current/lexer_phase_controls.py';driver=_module(driver_path,'_oxid_package_dependency_unit4_phase_policy_v1','cf8667d190c9b50c2f69b623783a33c8c931f795e359a43980ef2486ad44d8c9')
 role='not_observed' if control else 'observed';probe=environment.get('UNIT4_PHASE_PROBE');need(any(row[0]==probe and role in row[1] for row in driver.CATALOGUE),'closed public phase case')
 path=root/'lexer-phase-controls'/(profile+'-'+role+'-'+probe)/'request.json';request=driver.decode(body(path,2048),2048)
 need(request['profile']==profile and request['role']==role and request['probe']==probe and request['session_sha256']==identity(d['checkpoint'])['sha256'],'phase session association')
 need(request['authority_sha256']==d['authority_sha256'] and request['driver_sha256']=='cf8667d190c9b50c2f69b623783a33c8c931f795e359a43980ef2486ad44d8c9','phase exact authority/driver association')
 need(request['binary']['sha256']==d['binaries'][key]['sha256'],'phase actual binary association');expected=driver.inputs(root,request)
 need((argv,str(cwd),environment)==expected,'unchanged phase request inputs');return identity(path)

def _operation_recipe(operation_kind,argv,route,run_root):
 need(Path(route).is_absolute() and '..' not in Path(route).parts,'normalized executable route required')
 need(operation_kind in ('probe','public-build','parser-build','parser-helper','runtime-tool-version','runtime-trap-build','runtime-trap-control','parser-collect','parser-passivity','parser-u8','parser-phase','public-runtime','public-native','generator','public-hosted-worker'),'fixed operation kind')
 if operation_kind=='runtime-tool-version':need(len(argv)==2 and argv[1]=='--version' and Path(argv[0]).is_absolute() and Path(argv[0]).name in ('clang','opt','ld.lld','clang.exe','opt.exe','ld.lld.exe'),'exact LLVM version recipe')
 if operation_kind=='runtime-trap-build':need(len(argv)==7 and Path(argv[0]).is_absolute() and argv[1:4]==['--edition=2021','-C','opt-level=0'] and argv[5]=='-o','exact trap build recipe')
 if operation_kind=='runtime-trap-control':need(len(argv)==2 and argv[1]=='--version' and Path(argv[0]).name in ('clang','opt','ld.lld','clang.exe','opt.exe','ld.lld.exe'),'exact trap control recipe')
 if operation_kind=='public-build':need(argv in [['cargo','build','--bin','oxid','--locked','--offline']+suffix for suffix in ([],['--release'])],'exact public build recipe')
 if operation_kind=='parser-helper':
  root=Path(run_root)/'parser';need(len(argv)==9 and argv[:3]==[sys.executable,'-B',str(root/'helpers/build.py')] and argv[3]=='--overlay' and argv[5]=='--profile' and argv[7]=='--output' and argv[6] in ('debug','release'),'exact parser helper argv')
  control=argv[4]==str(root/'control-source/overlay-manifest.json');need(argv[4]==str(root/('control-source' if control else 'source')/'overlay-manifest.json') and argv[8]==str(root/(('build-control-' if control else 'build-')+argv[6])/'result'),'exact parser helper source/output')
 if operation_kind=='parser-build':
  tail=['test','--offline','--locked','--no-run','--bin','oxid','--jobs','2','--target','x86_64-unknown-linux-gnu','--message-format=json'];need(argv[1:] in (tail,tail+['--release']) and run_root in route.parents,'exact parser build recipe')
 return True

def _runtime_setup_recipe(kind,argv,cwd,environment,root,repo,timeout,limit):
 """Finite setup recipes; tool semantic admission belongs to RuntimeTools."""
 if kind not in ('runtime-tool-version','runtime-trap-build','runtime-trap-control'):return None
 roots={root/'public-runtime/tools'/role for role in ('main','capability')}
 need(cwd in roots and limit==8*1024**2,'fixed runtime setup cwd/streams')
 if kind=='runtime-tool-version':
  need(cwd==root/'public-runtime/tools/main' and timeout==30,'LLVM version limits');return None
 if kind=='runtime-trap-build':
  source=repo/TRAP_SOURCE;row=identity(source);need(row['sha256']==TRAP_SOURCE_SHA and argv[4]==str(source),'fixed original trap source')
  need(argv[6]==str(cwd/('no-tool-trap.exe' if os.name=='nt' else 'no-tool-trap')) and timeout==120,'fixed trap build output/limits');return row
 tool=Path(argv[0]).name;plain=tool.removesuffix('.exe')
 need(argv[0]==str(cwd/'no-tool-traps'/tool) and timeout==10,'fixed trap control executable/limits')
 need(environment.get('UNIT4_TOOL_LOG')==str(cwd/(plain+'-trap-control.jsonl')) and environment.get('OXID_LLVM_BIN')==str(cwd/'no-tool-traps'),'fixed trap control environment');return None

def _transient_target(operation_kind,argv,cwd,environment,run_root):
 root=Path(run_root);cwd=Path(cwd)
 if operation_kind=='parser-helper':return Path(argv[-1])/'target'
 if operation_kind=='public-build':
  roles={root/'source-preparation/public'/role:role for role in ('ordinary','lifecycle')};need(cwd in roles,'public build exact prepared source cwd');expected=root/'targets'/('public-'+roles[cwd])
 elif operation_kind=='parser-build':
  need(cwd in (root/'parser/source',root/'parser/control-source'),'parser build exact source cwd');profile='release' if argv[-1]=='--release' else 'debug';expected=root/'parser-targets'/('control-' if cwd.name=='control-source' else 'observed-')/profile
 else:return None
 need(environment.get('CARGO_TARGET_DIR')==str(expected),'fixed owned Cargo target required');return expected

def _emergency_terminal(directory,result,streams,error,state):
 # No source/tool rehash after the deadline. This failure-only path preserves
 # already observed process facts and explicitly records unverifiable evidence.
 state['failed']=True
 failure={'schema':'oxid-failed-terminal-capture-v1','success':False,'outcome':result,
          'evidence_error':repr(error),'captured_stream_lengths':{n:len(v) for n,v in streams.items()},
          'preservation_errors':[]}
 state['failed_terminal']=failure
 until=time.monotonic()+2
 for name,raw in streams.items():
  path=directory/name
  try:
   if time.monotonic()>=until:raise TimeoutError('emergency capture deadline')
   if path.exists() or path.is_symlink():continue
   with path.open('xb') as f:f.write(raw)
  except BaseException as e:failure['preservation_errors'].append({'path':str(path),'error':repr(e)})
 try:
  if time.monotonic()>=until:raise TimeoutError('emergency capture deadline')
  real(directory);raw=serial(failure);need(len(raw)<=2*1024**2,'bounded emergency metadata')
  with (directory/'failed-outcome.json').open('xb') as f:f.write(raw)
 except BaseException as e:failure['preservation_errors'].append({'path':str(directory/'failed-outcome.json'),'error':repr(e)})
 summary={k:result.get(k) for k in ('pid','status','started_ns','completed_ns','cleanup_complete','cleanup_error')}
 raise Rejected('terminal evidence failure; outcome='+json.dumps(summary,sort_keys=True)+'; preservation='+repr(failure['preservation_errors'])) from error

def _retain_terminal(directory,result,streams,state):
 try:
  for name,raw in streams.items():
   path=directory/name
   with path.open('xb') as f:f.write(raw)
   result[name]=identity(path)
  with (directory/'result.json').open('xb') as f:f.write(serial(result))
  identities={'result':identity(directory/'result.json'),'command':identity(directory/'command.json')}
  need(result['failure'] is None and result['cleanup_complete'] and not result['timed_out'] and not result['stream_limit_exceeded'],'operation did not complete within policy')
  return identities
 except BaseException as error:_emergency_terminal(directory,result,streams,error,state)

def owned_probe(invocation,argv,cwd,environment,timeout_seconds=30,stream_limit_bytes=8388608,operation_kind='probe',runtime_permit=None,native_permit=None,runtime_case=None):
 global _LAUNCH
 need(not _CHECKED and _LAUNCH is None,'owned launch outside read scope required')
 args=(invocation,argv,cwd,environment,timeout_seconds,stream_limit_bytes,operation_kind,runtime_permit,native_permit,runtime_case)
 if operation_kind not in ('public-runtime','public-native'):return _owned_probe(*args)
 # Only already-issued public permits get the measured association-only path.
 need(type(runtime_permit) is RuntimePermit and runtime_permit in _RUNTIME and _RUNTIME[runtime_permit]['invocation'] is invocation,'original public launch permit required')
 need(runtime_permit.document()['kind']=='public' and invocation in _RUNS,'active public invocation required')
 owner=None
 try:
  context=context_api();context._invocation_association(invocation);check_deadline(invocation);_manifest()
  owner=_CheckedScope(invocation,context,_TOKEN);_LAUNCH=owner
  result=_owned_probe(*args)
  _active_owner(invocation);_manifest();context._invocation_association(invocation)
  return result
 except BaseException:
  _poison(invocation);raise
 finally:
  if owner is not None:object.__setattr__(owner,'active',False)
  _LAUNCH=None

def _owned_probe(invocation,argv,cwd,environment,timeout_seconds=30,stream_limit_bytes=8388608,operation_kind='probe',runtime_permit=None,native_permit=None,runtime_case=None):
 inv,captured,state=_run_state(invocation);need(type(argv) is list and argv and all(type(x) is str and '\0' not in x for x in argv),'explicit argv');need(type(environment) is dict and all(type(k) is str and type(v) is str for k,v in environment.items()),'explicit environment')
 need(type(timeout_seconds) in (int,float) and 0<timeout_seconds<=1800 and type(stream_limit_bytes) is int and 0<stream_limit_bytes<=8*1024**2,'finite probe limits');need(time.monotonic()+timeout_seconds<=state['deadline'],'invocation deadline');tick()
 if _DEADLINE is not None:need(time.monotonic()+timeout_seconds<=_DEADLINE,'operation exceeds preparation deadline')
 route=Path(argv[0]) if Path(argv[0]).is_absolute() else Path(shutil.which(argv[0],path=environment.get('PATH','')) or '')
 need(route.is_absolute(),'executable unavailable in explicit PATH');executable=route.resolve(strict=True);real(executable);executable_before=identity(executable);cwd=real(cwd);repo=Path(captured['checkout']['path']);tree_path=repo/TREE_PATH;tree=_module(tree_path,'_oxid_package_dependency_unit4_process_tree_v1',TREE_SHA);tree_before=identity(tree_path)
 _operation_recipe(operation_kind,argv,route,state['root'])
 setup_source=_runtime_setup_recipe(operation_kind,argv,cwd,environment,state['root'],repo,timeout_seconds,stream_limit_bytes)
 helper=setup_source
 if operation_kind in ('parser-collect','parser-passivity','parser-u8','parser-phase'):
  helper=_parser_runtime_recipe(runtime_permit,invocation,operation_kind,argv,cwd,environment,timeout_seconds,stream_limit_bytes)
 elif operation_kind=='public-hosted-worker':
  need(native_permit is None and runtime_case is None,'hosted outer operation has no tuple');helper=_hosted_runtime_recipe(runtime_permit,invocation,argv,cwd,environment,timeout_seconds,stream_limit_bytes)
 elif operation_kind in ('public-runtime','public-native','generator'):
  helper=_public_runtime_recipe(runtime_permit,native_permit,invocation,operation_kind,runtime_case,argv,cwd,environment,timeout_seconds,stream_limit_bytes)
 else:need(runtime_permit is None and native_permit is None and runtime_case is None,'unexpected runtime permit')
 if operation_kind=='parser-helper':
  need(cwd==state['root']/'parser','parser helper cwd');helper=identity(argv[2]);need(helper['sha256']==PARSER_BUILD_SHA,'frozen parser build helper pin')
 transient=[];target=_transient_target(operation_kind,argv,cwd,environment,state['root'])
 if target is not None:transient=[real(target,False)]
 state['next']+=1;directory=state['root']/'operations'/('%04d'%state['next']);output_monitor=_public_output_operation_begin(invocation,runtime_permit,operation_kind,runtime_case,directory);directory.mkdir()
 command={'argv':argv,'cwd':str(cwd),'environment':environment,'timeout_seconds':timeout_seconds,'stream_limit_bytes':stream_limit_bytes,'operation_kind':operation_kind}
 (directory/'command.json').write_bytes(serial(command));is_prepare=argv==state.get('preparation_argv')
 view_rows=list(_VIEWS.get(invocation,{}).values());allowed_links=_verify_view_rows(view_rows,_stat_owner=(invocation,runtime_permit) if operation_kind in ('public-runtime','public-native') else None)
 for link,row in _GUARD_LINKS.get(invocation,{}).items():_verify_link(row);allowed_links[link]=row
 native_plan=_native_plan(invocation,operation_kind,argv,cwd)
 outcome,streams=_supervise(tree.ProcessTree,argv,str(cwd),environment,timeout_seconds,stream_limit_bytes,preparation=is_prepare,output_root=state['root'],operation_kind=operation_kind,transient_roots=transient,allowed_links=allowed_links,native_plan=native_plan,output_monitor=output_monitor)
 result=dict(command,schema='oxid-owned-probe-v1',invocation_nonce=inv['nonce'],**outcome)
 result['runtime_case']=_copy(runtime_case);result['runtime_permit_sha256']=sha(canonical(runtime_permit.document())) if runtime_permit is not None else None;result['executable_views']=view_rows;result['helper']=helper;result['executable']=executable_before;result['executable_route']={'path':str(route),'target':str(executable)};result['process_tree']=tree_before
 try:
  _verify_view_rows(view_rows,_stat_owner=(invocation,runtime_permit) if operation_kind in ('public-runtime','public-native') else None)
  for row in _GUARD_LINKS.get(invocation,{}).values():_verify_link(row)
  if helper is not None:need(identity(helper['path'])==helper,'parser helper drift')
  need(route.resolve(strict=True)==executable,'probe executable route drift');need(identity(executable)==executable_before,'probe executable drift');need(identity(tree_path)==tree_before,'process helper drift')
 except BaseException as error:
  result['failure']=result['failure'] or repr(error)
  result['public_output_failure_preservation']=_public_output_failure_audit(output_monitor,deadline=state['deadline'])
  _emergency_terminal(directory,result,streams,error,state)
 try:
  evidence=_retain_terminal(directory,result,streams,state);_public_output_operation_seal(output_monitor)
 except BaseException:
  _public_output_failure_audit(output_monitor,deadline=state['deadline']);raise
 record=ProbeRecord(result,_TOKEN);_PROBES[record]={'invocation':invocation,'directory':directory,'runtime_permit':runtime_permit,'native_permit':native_permit,**evidence};return record

def verify_probe(record,invocation,*,argv,cwd,environment,timeout_seconds,stream_limit_bytes,operation_kind='probe',runtime_permit=None,native_permit=None,runtime_case=None):
 verify_invocation(invocation);need(record in _PROBES and _PROBES[record]['invocation'] is invocation,'live producer probe required');owner=_PROBES[record];d=record.document()
 need(owner.get('runtime_permit') is runtime_permit,'original runtime permit required')
 need(owner.get('native_permit') is native_permit and d.get('runtime_case')==runtime_case,'original runtime tuple/native permit required')
 if operation_kind in ('parser-collect','parser-passivity','parser-u8','parser-phase'):_parser_runtime_recipe(runtime_permit,invocation,operation_kind,argv,Path(cwd),environment,timeout_seconds,stream_limit_bytes)
 elif operation_kind=='public-hosted-worker':_hosted_runtime_recipe(runtime_permit,invocation,argv,Path(cwd),environment,timeout_seconds,stream_limit_bytes)
 elif operation_kind in ('public-runtime','public-native','generator'):_public_runtime_recipe(runtime_permit,native_permit,invocation,operation_kind,runtime_case,argv,Path(cwd),environment,timeout_seconds,stream_limit_bytes)
 for key,value in {'argv':argv,'cwd':str(cwd),'environment':environment,'timeout_seconds':timeout_seconds,'stream_limit_bytes':stream_limit_bytes,'operation_kind':operation_kind}.items():need(d[key]==value,'probe command/context mismatch')
 need(d['cleanup_complete'] and d['cleanup_error'] is None and not d['failure'] and not d['timed_out'] and not d['stream_limit_exceeded'],'probe bounded completion required')
 need({p.name for p in owner['directory'].iterdir()}=={'command.json','stdout','stderr','result.json'},'closed operation evidence')
 for row in (owner['result'],owner['command'],d['executable'],d['process_tree']):need(_probe_body_identity(row,record,invocation)==row,'probe evidence/tool drift')
 need(str(Path(d['executable_route']['path']).resolve(strict=True))==d['executable_route']['target'],'probe route changed');
 _verify_view_rows(d.get('executable_views',[]),_stat_owner=_public_probe_view_owner(record,invocation,operation_kind,runtime_permit))
 for row in _GUARD_LINKS.get(invocation,{}).values():_verify_link(row)
 if d.get('helper') is not None:need(identity(d['helper']['path'])==d['helper'],'parser helper changed')
 _native_terminal(d.get('native_workspace'))
 _probe_stream(record,'stdout');_probe_stream(record,'stderr');return d

def _snapshot_files(paths,roots):
 result={};closures=[]
 for root in sorted(set(map(Path,roots))):
  root=real(root);names=[];dirs=[]
  for p in root.rglob('*'):
   tick()
   mode=p.lstat().st_mode;need(stat.S_ISREG(mode) or stat.S_ISDIR(mode),'special preserved member')
   n=p.relative_to(root).as_posix()
   if stat.S_ISDIR(mode):dirs.append(n)
   else:names.append(n);result[str(p)]=identity(p)
  closures.append({'path':str(root),'files':sorted(names),'directories':sorted(dirs)})
 for p in paths:result[str(real(p))]=identity(p)
 need(len(result)<=5000 and sum(r['bytes'] for r in result.values())<=256*1024**2,'finite preparation input closure')
 return sorted(result.values(),key=lambda r:r['path']),closures

def _verify_inputs(config):
 for r in config['input_files']:need(identity(r['path'])==r,'preserved preparation input drift')
 for r in config['input_routes']:need(str(Path(r['path']).resolve(strict=True))==r['target'],'tool route drift')
 for n in config['absent_paths']:need(not Path(n).exists() and not Path(n).is_symlink(),'unexpected preparation configuration')
 for r in config['closed_roots']:
  root=real(r['path']);files=[];dirs=[]
  for p in root.rglob('*'):
   tick()
   mode=p.lstat().st_mode;need(stat.S_ISREG(mode) or stat.S_ISDIR(mode),'special preserved member');(files if stat.S_ISREG(mode) else dirs).append(p.relative_to(root).as_posix())
  need(sorted(files)==r['files'] and sorted(dirs)==r['directories'],'preserved exact closure drift')
 return True

def _git_identity(invocation,repo,git,environment):
 result={}
 for key,args in (('head',['rev-parse','HEAD']),('tree',['rev-parse','HEAD^{tree}']),('index',['ls-files','--stage','-z'])):
  argv=[str(git),'--no-optional-locks','-c','core.hooksPath='+os.devnull,'-c','core.fsmonitor=false','-C',str(repo),*args]
  r=owned_probe(invocation,argv,repo,environment,10,1024**2);d=verify_probe(r,invocation,argv=argv,cwd=repo,environment=environment,timeout_seconds=10,stream_limit_bytes=1024**2)
  need(d['status']==0 and r.stderr()==b'','Git identity readback failure');raw=r.stdout();result[key]=sha(raw) if key=='index' else raw.decode().strip()
 return {'path':str(repo),'identity':result}

def _historical_selected(invocation,repo,git,environment,authority):
 argv=[str(git),'--no-optional-locks','-C',str(repo),'ls-files','--stage','-z'];r=owned_probe(invocation,argv,repo,environment,10,1024**2);need(r.document()['status']==0 and not r.stderr(),'historical index readback')
 records=[x for x in r.stdout().split(b'\0') if x];need(len(records)==800,'complete historical index');names=[];selected=[]
 for raw in records:
  metadata,name=raw.decode().split('\t',1);need(metadata.split()[-1]=='0' and '..' not in Path(name).parts and not name.startswith('/'),'historical index shape');names.append(name)
  if name.startswith(('tests/fixtures/','docs/','spec/','examples/','apps/','packages/','.github/')):continue
  p=repo/name
  if p.is_file():real(p);selected.append(name)
 need(len(set(names))==800 and selected==[r['path'] for r in authority['original_files']],'exact physical historical selection');return selected

def _config(invocation,scope,original_repo,historical_repo):
 inv,captured,state=_run_state(invocation);worker=_worker();repo=Path(captured['checkout']['path']);need(scope in worker.SCOPES,'exact preparation scope')
 need(captured.get('scope',scope)==scope,'captured scope mismatch');host=worker.measured_host();need((host['os'],host['architecture']) in worker.REQUIRED_HOSTS,'qualified host')
 if scope!='public':need((host['os'],host['architecture'])==('linux','x86_64'),'parser host applicability')
 python=Path(sys.executable).resolve(strict=True);need(str(python)==sys.executable,'canonical Python invocation required')
 tools=[python];routes=[]
 git_route=shutil.which('git');need(git_route is not None,'Git unavailable');git=Path(git_route).resolve(strict=True);tools.append(git)
 if str(git)!=git_route:routes.append({'path':git_route,'target':str(git)})
 env={'PATH':str(git.parent)+os.pathsep+os.defpath,'HOME':str(state['root']/'home'),'LANG':'C.UTF-8','LC_ALL':'C.UTF-8','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':os.devnull,'GIT_NO_REPLACE_OBJECTS':'1','GIT_TERMINAL_PROMPT':'0','PYTHONDONTWRITEBYTECODE':'1'}
 if os.name=='nt':
  for k in ('SYSTEMROOT','WINDIR'):need(k in os.environ,'Windows system context');env[k]=os.environ[k]
 repositories=[_git_identity(invocation,repo,git,env)]
 stage=repo/worker.STAGE1;paths=[stage/n for n in worker.STAGE1_PINS];roots=[repo/SELF,stage,repo/'src',repo/'native',repo/'tests/qualification/package_dependency_current',repo/'tests/fixtures/package_dependency_source_v1']
 source=context_api()._stage(repo)[1]['source'].capture(repo)
 paths += [repo/n for n in source['current_inputs']]
 # The retained closure is preserved at its original installed locations.
 common=['tests/qualification/lexer_reservation_current','tests/fixtures/typed_project_source_binding','tests/fixtures/typed_project_source_binding_byte_storage_v1']
 roots += [repo/n for n in common]
 paths += [repo/TREE_PATH]
 public=context_api()._stage(repo)[1]['public_maps'];parser=context_api()._stage(repo)[1]['parser_maps']
 paths += [repo/n for n in public.PINS]
 if scope!='parser':roots.append(repo/'tests/qualification/unit4_public_v3')
 absent=[];selected=[];original=history=None
 if scope!='public':
  original=real(original_repo);history=real(historical_repo);need(len({repo,original,history})==3,'distinct source repositories')
  repositories += [_git_identity(invocation,r,git,env) for r in (original,history)]
  need(repositories[1]['identity']['head']==worker.ORIGINAL_HEAD and repositories[1]['identity']['tree']==worker.ORIGINAL_TREE and repositories[2]['identity']['head']==worker.HISTORICAL_HEAD,'genuine predecessor Git identities')
  paths += [original/n for n in parser.PINS]+[original/n for n in source['predecessor_inputs']]
  paths += [original/n for n in worker.CORE_PINS if n.startswith((worker.PARSER,worker.FROZEN))]
  roots += [original/n for n in common]+[original/'src',original/'native',original/worker.PARSER,original/worker.PUBLIC,original/(worker.FROZEN+'frozen/helpers')]
  historical=decode(body(original/(worker.FROZEN+'authority.json')));selected=_historical_selected(invocation,history,git,env,historical);paths += [history/n for n in selected]
  absent.append(str(history/'.git/refs/replace'))
  for retained_repo in (original,history):
   gitdir=real(retained_repo/'.git');need(gitdir.is_dir(),'physical retained Git directory required')
   paths += [gitdir/n for n in ('config','index','HEAD')]
   for name in ('packed-refs','refs'):
    member=gitdir/name
    if member.exists():
     if member.is_dir():roots.append(member)
     else:paths.append(member)
    else:absent.append(str(member))
  portable_raw=body(original/(worker.PARSER+'portable.py'));need(sha(portable_raw)==worker.CORE_PINS[worker.PARSER+'portable.py'],'retained portable executable pin');portable=types.ModuleType('_source_preparation_metadata');portable.__file__=str(original/(worker.PARSER+'portable.py'));exec(compile(portable_raw,portable.__file__,'exec'),portable.__dict__)
  child_env=portable.minimal_env(state['root']/'source-preparation/predecessor/home')
  # This pure retained function resolves and hashes Python/which/as; no probes.
  child_tools=portable.process_tools(child_env);child_host=portable.measured_host()
  for tool in child_tools.values():tools.append(Path(tool['binary']['path']))
  for name in ('git','which','as'):
   route=shutil.which(name,path=child_env['PATH']);need(route is not None,'historical tool route missing');target=Path(route).resolve(strict=True);tools.append(target)
   if route!=str(target):routes.append({'path':route,'target':str(target)})
  config_file=Path('/etc/gitconfig')
  if config_file.exists():paths.append(config_file)
  else:absent.append(str(config_file))
 else:child_tools={};child_env={};child_host={}
 paths += tools
 output=state['root']/'source-preparation'
 for p in (output,*output.parents):
  for n in ('config','config.toml'):absent.append(str(p/'.cargo'/n))
 files,closures=_snapshot_files(paths,roots)
 config={'scope':scope,'python':str(python),'current_repo':str(repo),'stage1_root':str(repo),'original_repo':str(original) if original else None,'historical_repo':str(history) if history else None,'output':str(output),'run_root':str(state['root']),'input_files':files,'input_routes':sorted({r['path']:r for r in routes}.values(),key=lambda r:r['path']),'repositories':repositories,'closed_roots':closures,'absent_paths':sorted(set(absent)),'historical_selected':selected}
 _verify_inputs(config)
 return config,{'host':host,'git':str(git),'git_environment':env,'child_environment':child_env,'child_tools':child_tools,'child_host':child_host}

# Limits are local process resource limits, never network/host isolation claims.
_LIMIT_LAUNCHER="import resource,runpy,sys;resource.setrlimit(resource.RLIMIT_CPU,(120,120));resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824));resource.setrlimit(resource.RLIMIT_FSIZE,(67108864,67108864));p=sys.argv.pop(1);sys.argv[0]=p;runpy.run_path(p,run_name='__main__')"

def produce(invocation,scope,*,original_repo=None,historical_repo=None):
 """Capture expected inputs before one actual worker; return admitted live context."""
 global _DEADLINE
 started=time.monotonic();prior_deadline=_DEADLINE;config=plan=probe=None;failure=None;preserved=False
 inv,captured,state=_run_state(invocation);need(not state.get('preparation_started'),'preparation is single-attempt per invocation');state['preparation_started']=True;_DEADLINE=started+300
 try:
  config,expected=_config(invocation,scope,original_repo,historical_repo);worker=_worker();plan=worker.plan(config,tick);need(time.monotonic()-started<120,'preparation capture budget')
  config_path=state['root']/'preparation-config.json'
  with config_path.open('xb') as f:f.write(serial(config))
  worker_path=Path(__file__).with_name('prepare_worker.py')
  argv=[sys.executable,'-B',str(worker_path),'--config',str(config_path)]
  if sys.platform.startswith('linux'):argv=[sys.executable,'-B','-c',_LIMIT_LAUNCHER,str(worker_path),'--config',str(config_path)]
  state['preparation_argv']=argv;env={'PATH':expected['git_environment']['PATH'],'HOME':str(state['root']/'home'),'TMPDIR':str(state['root']/'tmp'),'TEMP':str(state['root']/'tmp'),'TMP':str(state['root']/'tmp'),'PYTHONDONTWRITEBYTECODE':'1','LANG':'C.UTF-8','LC_ALL':'C.UTF-8'}
  if os.name=='nt':env.update({k:os.environ[k] for k in ('SYSTEMROOT','WINDIR')})
  probe=owned_probe(invocation,argv,state['root'],env,180,8*1024**2);operation=verify_probe(probe,invocation,argv=argv,cwd=state['root'],environment=env,timeout_seconds=180,stream_limit_bytes=8*1024**2);need(operation['status']==0 and not probe.stderr(),'preparation worker failed')
  need(time.monotonic()-started<300,'preparation total deadline');report=worker.verify_output(config,plan,tick)
  _verify_inputs(config);after=[_git_identity(invocation,Path(r['path']),Path(expected['git']),expected['git_environment']) for r in config['repositories']];need(after==config['repositories'],'preparation Git poststate drift')
  if scope!='public':need(_historical_selected(invocation,Path(config['historical_repo']),Path(expected['git']),expected['git_environment'],decode(body(Path(config['original_repo'])/(worker.FROZEN+'authority.json'))))==config['historical_selected'],'historical selection drift')
  _windows(config,expected,operation,report);need(not any((state['root']/'tmp').iterdir()),'temporary preparation residue');need(time.monotonic()-started<300,'preparation terminal deadline');preserved=True
  document={'schema':'oxid-live-preparation-record-v1','scope':scope,'invocation':inv,'config_sha256':sha(canonical(config)),'plan_sha256':sha(canonical(plan)),'operation':operation,'expected':expected,'repositories_before':config['repositories'],'repositories_after':after,'report':identity(Path(config['output'])/'report.json'),'status':'NotReady','execution_qualified':False}
  record=PreparationRecord(document,_TOKEN);_RECORDS[record]={'invocation':invocation,'config':_copy(config),'plan':_copy(plan),'probe':probe,'config_file':identity(config_path)}
  api=context_api();token=api._register_producer(sys.modules[KEY]);api._record_preparation(invocation,record,token);return api.admit_prepared(invocation,config['output'])
 except BaseException as error:failure=repr(error);state['failed']=True;raise
 finally:
  preservation_error=None
  if config is not None and not preserved:
   try:
    if time.monotonic()-started>=300:raise TimeoutError('preservation deadline exhausted')
    _verify_inputs(config);preserved=True
   except BaseException as error:preservation_error=repr(error)
  _DEADLINE=prior_deadline
  with (state['root']/'preparation-status.json').open('xb') as f:f.write(serial({'success':failure is None,'failure':failure,'inputs_preserved':preserved,'preservation_scope':'sealed body/route/configuration/physical closure','repository_poststate_checked':failure is None and preserved,'preservation_error':preservation_error,'last_failed_operation':state.get('failed_terminal'),'status':'NotReady','execution_qualified':False}))

def _windows(config,expected,operation,report):
 start,finish=operation['started_ns'],operation['completed_ns'];need(type(start) is int and start<=finish,'actual worker window')
 if config['scope']=='public':need(start<=report['genuine_public_prepared_ns']<=finish,'public admission outside actual worker');return
 session=decode(body(Path(config['output'])/'predecessor/session.json'));need(start<=session['prepared_ns']<=finish,'old preparation outside actual worker')
 for name in ('prepare','prepare-control'):
  r=decode(body(Path(config['output'])/'predecessor'/name/'invocation.json'));need(start<=r['started_ns']<=r['finished_ns']<=session['prepared_ns'],'historical child outside actual worker')
  need(r['environment']==expected['child_environment'] and r['tools']==expected['child_tools'] and r['host']==expected['child_host'],'historical child differs from producer-captured context')

def verify_record(record,invocation):
 verify_invocation(invocation);need(record in _RECORDS and _RECORDS[record]['invocation'] is invocation,'live preparation producer required');stored=_RECORDS[record];d=record.document();c=stored['config'];p=stored['plan'];op=d['operation']
 verify_probe(stored['probe'],invocation,argv=op['argv'],cwd=op['cwd'],environment=op['environment'],timeout_seconds=op['timeout_seconds'],stream_limit_bytes=op['stream_limit_bytes']);need(op==stored['probe'].document() and op['status']==0,'actual successful preparation record')
 need(identity(stored['config_file']['path'])==stored['config_file'] and identity(d['report']['path'])==d['report'],'preparation evidence changed');_verify_inputs(c)
 need(sha(canonical(c))==d['config_sha256'] and sha(canonical(p))==d['plan_sha256'],'producer record association')
 _windows(c,d['expected'],op,decode(body(d['report']['path'])))
 return {'config':_copy(c),'plan':_copy(p),'provenance':d}


# Registered public output ledger; sampled prior-output preservation.
_OUTPUT_MONITORS = {}
_OUT_SECTIONS = ('public', 'original', 'predecessors', 'lifecycle', 'guards')
_OUT_TOTAL = 6 * 1024 ** 3
_OUT_FILE = 2 * 1024 ** 3
_OUT_DUE = 3.0  # Measured early start; the hard completed-audit gap remains 5s.
_OUT_STAT = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
_OUT_STABLE_DIR = ('st_dev', 'st_ino', 'st_mode')


def _out_raw_deadline(m, extra=None):
    deadline = m['run']['deadline']
    if m.get('operation') is not None and m['operation'].get('deadline') is not None:
        deadline = min(deadline, m['operation']['deadline'])
    if globals().get('_DEADLINE') is not None:
        deadline = min(deadline, _DEADLINE)
    if extra is not None:
        deadline = min(deadline, extra)
    need((not m['run']['failed'] or m.get('failure_read', False)) and time.monotonic() < deadline,
         'public output monitor deadline or poisoned invocation')
    return deadline


def _out_get(invocation):
    need(invocation in _OUTPUT_MONITORS, 'unissued public output monitor')
    m = _OUTPUT_MONITORS[invocation]
    need(m['thread'] == threading.get_ident() and m['invocation'] is invocation,
         'cross-thread or cross-invocation output monitor')
    _out_raw_deadline(m)
    return m


def _out_auth(invocation, permit):
    m = _out_get(invocation)
    need(m['permit'] is permit and not m['closed'], 'wrong or closed output permit')
    # Public transition entry authenticates the live producer capability. Inner
    # samples operate only on this private, thread-bound owner, not new inputs.
    d = _runtime_permit_owner(permit, invocation, 'public')
    need(Path(d['root']) == m['root'], 'output permit root drift')
    return m


def _out_stat(s):
    return {key: getattr(s, key) for key in _OUT_STAT}


def _out_under(path, root):
    name, prefix = os.fspath(path), os.fspath(root)
    return name == prefix or name.startswith(prefix.rstrip(os.sep) + os.sep)


def _out_live_roots(m):
    roots = [m['root'] / 'tmp']
    if m['case'] is not None:
        roots.append(m['case']['root'])
    if m['operation'] is not None:
        roots.append(m['operation']['root'])
    need(all(not _out_under(a, b) and not _out_under(b, a)
             for i, a in enumerate(roots) for b in roots[i + 1:]),
         'overlapping public output roots')
    return roots


def _out_structure(m):
    result = {m['root'], m['root'] / 'operations', m['root'] / 'tmp', m['collection']}
    for section in _OUT_SECTIONS:
        result.add(m['collection'] / section)
    return {str(path) for path in result}


def _out_allowed_links(m):
    links = dict(_VIEWS.get(m['invocation'], {}))
    links.update(_GUARD_LINKS.get(m['invocation'], {}))
    return links


def _out_check_ancestors(m, root):
    for path in root.parents:
        if path != m['root'] and m['root'] not in path.parents:
            break
        old = m['sealed'].get(str(path))
        need(old is not None and old['kind'] == 'directory', 'unregistered output ancestor')
        now = _out_stat(path.lstat())
        need(all(now[key] == old['stat'][key] for key in _OUT_STABLE_DIR),
             'public output ancestor identity/type/mode drift')


def _out_existing_roots(m):
    roots = []
    tmp = m['root'] / 'tmp'
    need(os.path.lexists(tmp), 'fixed output tmp root disappeared')
    roots.append(tmp)
    for key in ('case', 'operation'):
        active = m[key]
        if active is None:
            continue
        exists = os.path.lexists(active['root'])
        need(exists or not active['seen'], 'active stable output root disappeared')
        if exists:
            active['seen'] = True
            _out_check_ancestors(m, active['root'])
            roots.append(active['root'])
    return roots


def _out_hash(path, before, m, deadline):
    path = Path(path)
    flags = os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0)
    fd = os.open(path, flags)
    digest, total = hashlib.sha256(), 0
    with os.fdopen(fd, 'rb') as stream:
        need(_out_stat(os.fstat(stream.fileno())) == before, 'output opened identity changed')
        while True:
            _out_raw_deadline(m, deadline)
            # Sealing can span multiple seconds. Metadata audits never hash, so
            # this due-only callback cannot recurse back into body sealing.
            if not m['auditing'] and m['last_audit'] is not None and time.monotonic() >= m['last_audit'] + _OUT_DUE:
                additions = () if m['transition'] is None else m['transition']['additions']
                _out_audit(m, additions=additions, deadline=min(deadline, m['last_audit'] + 5))
            chunk = stream.read(1024 * 1024)
            if not chunk:
                break
            total += len(chunk)
            need(total <= before['st_size'] and total <= _OUT_FILE, 'growing output body')
            digest.update(chunk)
        need(_out_stat(os.fstat(stream.fileno())) == before, 'output changed during body read')
    need(total == before['st_size'] and _out_stat(path.lstat()) == before,
         'output body/path changed while sealing')
    return digest.hexdigest()


def _out_snapshot(m, roots, *, deadline, hash_bodies=False, registration=None):
    """No-follow bounded walk; roots include their own metadata."""
    rows, total, links = {}, 0, _out_allowed_links(m)
    pending = []
    native_parent = str(registration['parent']['path']) if registration is not None else None
    native_workspace = str(registration['workspace']) if registration is not None else None

    def observe(path, s):
        nonlocal total
        _out_raw_deadline(m, deadline)
        # Equivalent canonical component-prefix prefilter; native helper's
        # Path.parents grammar is evaluated only inside its exact possible scope.
        if registration is not None:
            name = path
            if (name == native_workspace or name.startswith(native_workspace + os.sep) or
                    os.path.dirname(name) == native_parent and os.path.basename(name).startswith('.oxid-native-')):
                _native_observed(Path(path), s, registration)
        row = {'stat': _out_stat(s)}
        if stat.S_ISDIR(s.st_mode):
            row['kind'] = 'directory'
            rows[path] = row
            pending.append((path, s))
        elif stat.S_ISREG(s.st_mode):
            need(0 <= s.st_size <= _OUT_FILE, 'public output per-file bound')
            row['kind'] = 'file'
            if hash_bodies:
                row['sha256'] = _out_hash(path, row['stat'], m, deadline)
            rows[path] = row
            total += s.st_size
            need(total <= _OUT_TOTAL, 'public output aggregate bound')
        elif stat.S_ISLNK(s.st_mode):
            need(path in links, 'unregistered public output link')
            link = links[path]
            _verify_link(link, hash_body=hash_bodies)
            row.update(kind='link', link_text=os.readlink(path), target=link['target'],
                       target_stat=dict(link['target_stat']))
            rows[path] = row
        else:
            need(False, 'special public output member')

    for path in roots:
        _out_raw_deadline(m, deadline)
        path = os.fspath(path)
        try:
            observe(path, os.lstat(path))
        except FileNotFoundError:
            _scan_disappearance(path, m['root'], registration, ())
    while pending:
        _out_raw_deadline(m, deadline)
        directory, expected = pending.pop()
        fd = None
        try:
            if os.name == 'posix':
                fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
                current = os.fstat(fd)
                need((current.st_dev, current.st_ino, current.st_mode) ==
                     (expected.st_dev, expected.st_ino, expected.st_mode), 'output directory identity changed')
                source = fd
            else:
                current = os.lstat(directory)
                need((current.st_dev, current.st_ino, current.st_mode) ==
                     (expected.st_dev, expected.st_ino, expected.st_mode), 'output directory identity changed')
                source = directory
            with os.scandir(source) as entries:
                for entry in entries:
                    _out_raw_deadline(m, deadline)
                    path = directory + os.sep + entry.name
                    try:
                        s = entry.stat(follow_symlinks=False)
                    except FileNotFoundError:
                        _scan_disappearance(path, m['root'], registration, ())
                        continue
                    observe(path, s)
        except FileNotFoundError:
            _scan_disappearance(directory, m['root'], registration, ())
            rows.pop(directory, None)
        finally:
            if fd is not None:
                os.close(fd)
    return rows, total


def _out_without_digest(row):
    return {key: value for key, value in row.items() if key != 'sha256'}


def _out_same(path, old, new, structural):
    if path in structural and old['kind'] == new['kind'] == 'directory':
        return all(old['stat'][key] == new['stat'][key] for key in _OUT_STABLE_DIR)
    if old['kind'] != new['kind'] or old['stat'] != new['stat']:
        return False
    return old['kind'] != 'link' or all(old[key] == new[key] for key in ('link_text', 'target', 'target_stat'))


def _out_compare(m, actual, *, additions=()):
    live, structural = [str(path) for path in _out_live_roots(m)], _out_structure(m)
    additions = {str(path) for path in additions}
    for path, old in m['sealed'].items():
        need(path in actual and _out_same(path, old, actual[path], structural),
             'sealed public output metadata or membership drift')
    for path in actual:
        need(path in m['sealed'] or path in additions or
             any(_out_under(path, root) for root in live),
             'unregistered output outside current public case/operation')


def _out_bytes(rows):
    return sum(row['stat']['st_size'] for row in rows.values() if row['kind'] == 'file')


def _out_audit(m, *, additions=(), deadline=None):
    need(not m['auditing'], 'overlapping output audit')
    m['auditing'] = True
    started = time.monotonic()
    try:
        deadline = _out_raw_deadline(m, deadline)
        _out_existing_roots(m)
        registration = None if m['operation'] is None else m['operation']['registration']
        rows, total = _out_snapshot(m, [m['root']], deadline=deadline, registration=registration)
        _out_compare(m, rows, additions=additions)
        completed = time.monotonic()
        need(completed < deadline, 'output audit completed after deadline')
        previous = m['last_audit']
        if previous is not None and (m['case'] is not None or m['operation'] is not None):
            need(completed - previous <= 5, 'public full-output audit cadence missed')
        m['last_audit'] = completed
        m['full_peak'] = max(m['full_peak'], total)
        m['audits'].append({'started': started, 'completed': completed, 'bytes': total})
        return rows
    except BaseException:
        m['run']['failed'] = True
        raise
    finally:
        m['auditing'] = False


def public_output_start(invocation, runtime_permit):
    """After generator/setup, before the first section or case is created."""
    inv, captured, run = _run_state(invocation)
    need(invocation not in _OUTPUT_MONITORS, 'public monitor already started')
    need(not any(not old['closed'] and not old['run']['failed'] and
                 old['thread'] == threading.get_ident() for old in _OUTPUT_MONITORS.values()),
         'overlapping active public output invocation')
    d = _runtime_permit(runtime_permit, invocation, 'public')
    need(runtime_permit in _PUBLIC_CONTRACTS and Path(d['root']) == run['root'],
         'authenticated public output initial state')
    m = {'invocation': invocation, 'permit': runtime_permit, 'run': run,
         'root': run['root'], 'collection': run['root'] / 'public-runtime/collection',
         'thread': threading.get_ident(), 'case': None, 'operation': None,
         'transition': None, 'section': None, 'finished_sections': [], 'finished_cases': set(),
         'case_order': (), 'case_next': 0,
         'sealed': {}, 'sealed_bytes': 0, 'last_audit': None, 'auditing': False,
         'audits': [], 'full_peak': 0, 'accounted_peak': 0, 'closed': False}
    _OUTPUT_MONITORS[invocation] = m
    try:
        rows, total = _out_snapshot(m, [m['root']], deadline=_out_raw_deadline(m))
        tmp = str(m['root'] / 'tmp')
        need(tmp in rows and rows[tmp]['kind'] == 'directory', 'fixed output tmp root')
        m['sealed'] = {path: row for path, row in rows.items() if path == tmp or not _out_under(path, tmp)}
        m['sealed_bytes'] = _out_bytes(m['sealed'])
        m['last_audit'] = time.monotonic()
        m['full_peak'] = total
        m['audits'].append({'started': m['last_audit'], 'completed': m['last_audit'], 'bytes': total})
    except BaseException:
        run['failed'] = True
        raise


def public_output_tick(invocation, *, deadline=None):
    """Bounded producer checkpoint; no process launch and no recursive ticks."""
    if invocation not in _OUTPUT_MONITORS:
        return None
    m = _out_get(invocation)
    if m['closed']:
        return None
    need(not m['auditing'], 'recursive public output checkpoint')
    deadline = _out_raw_deadline(m, deadline)
    try:
        additions = {} if m['transition'] is None else m['transition']['additions']
        if time.monotonic() >= m['last_audit'] + _OUT_DUE:
            _out_audit(m, additions=additions, deadline=min(deadline, m['last_audit'] + 5))
        roots = _out_existing_roots(m)
        roots += [path for path in additions if os.path.lexists(path) and
                  not any(parent in additions for parent in path.parents)]
        registration = None if m['operation'] is None else m['operation']['registration']
        observed, active = _out_snapshot(m, roots, deadline=deadline, registration=registration)
        live = _out_live_roots(m)
        string_additions = {str(path): kind for path, kind in additions.items()}
        need(all(path in string_additions or any(_out_under(path, root) for root in live)
                 for path in observed), 'unexpected transition output member')
        need(all(path not in observed or observed[path]['kind'] == kind
                 for path, kind in string_additions.items()), 'wrong transition output type')
        total = m['sealed_bytes'] + active
        need(total <= _OUT_TOTAL, 'public active-accounted output aggregate bound')
        m['accounted_peak'] = max(m['accounted_peak'], total)
        return {'active_accounted_bytes': total, 'sealed_bytes': m['sealed_bytes'],
                'full_observed_peak_bytes': m['full_peak'], 'last_full_audit': m['last_audit']}
    except BaseException:
        m['run']['failed'] = True
        raise


def _public_output_tick_active():
    """Due-only hook for canonical preparation.tick/check_deadline callers.

    Ordinary public runtime checkpoints still sample their live roots explicitly.
    This hook covers long source/contract/tool verification between those calls.
    """
    active = [m for m in _OUTPUT_MONITORS.values() if not m['closed'] and
              not m['run']['failed'] and m['thread'] == threading.get_ident()]
    need(len(active) <= 1, 'multiple active public output invocation owners')
    if active:
        m = active[0]
        _out_raw_deadline(m)
        if not m['auditing'] and time.monotonic() >= m['last_audit'] + _OUT_DUE:
            public_output_tick(m['invocation'])


def _out_promote(m, rows):
    need(not any(path in m['sealed'] for path in rows), 'output entry sealed twice')
    candidate = m['sealed_bytes'] + _out_bytes(rows)
    need(candidate <= _OUT_TOTAL, 'sealed output aggregate bound')
    m['sealed'].update(rows)
    m['sealed_bytes'] = candidate


@contextmanager
def public_case_scope(invocation, runtime_permit, section, index):
    m = _out_auth(invocation, runtime_permit)
    try:
        need(m['case'] is None and m['operation'] is None and m['transition'] is None,
             'nested or overlapping public output scope')
        need(section == m['section'], 'case outside opened section')
        need(m['case_next'] < len(m['case_order']) and index == m['case_order'][m['case_next']],
             'out-of-order public case index')
        case = {'section': section, 'index': index, 'variant': 'ordinary'}
        d = _runtime_permit_owner(runtime_permit, invocation, 'public')
        _, row, argv, fixture, evidence = _public_case_verified(d, runtime_permit, case)
        root = evidence.parent
        need((section, index) not in m['finished_cases'] and not os.path.lexists(root),
             'replayed or pre-existing public case root')
        public_output_tick(invocation)
        m['case'] = {'section': section, 'index': index, 'root': root, 'variants': set(),
                     'operations': [], 'seen': False}
        yield
        need(m['operation'] is None, 'public case still has owned operation')
        # The authenticated consumer performs exact per-row semantic/source and
        # evidence-roster validation immediately before this context exits.
        need((root / 'receipt.json').is_file(), 'public case final receipt missing')
        public_output_tick(invocation)
        rows, _ = _out_snapshot(m, [root], deadline=_out_raw_deadline(m), hash_bodies=True)
        public_output_tick(invocation)
        _out_check_ancestors(m, root)
        need(not any(os.path.basename(path).startswith('.oxid-native-') for path in rows),
             'native workspace residue in finalized public case')
        _out_promote(m, rows)
        m['finished_cases'].add((section, index))
        m['case_next'] += 1
        m['case'] = None
    except BaseException:
        m['run']['failed'] = True
        raise


@contextmanager
def public_output_boundary(invocation, runtime_permit, phase, section=None):
    m = _out_auth(invocation, runtime_permit)
    try:
        need(m['case'] is None and m['operation'] is None and m['transition'] is None,
             'output transition while case/child/transition active')
        _out_audit(m)
        if phase == 'section-open':
            need(m['section'] is None and len(m['finished_sections']) < len(_OUT_SECTIONS) and
                 section == _OUT_SECTIONS[len(m['finished_sections'])], 'out-of-order section open')
            parent = m['collection'] / section
            additions = {parent: 'directory', parent / 'roster.json': 'file'}
        elif phase == 'section-close':
            need(section == m['section'], 'wrong section close')
            rows = _PUBLIC_CONTRACTS[runtime_permit]['rosters'][section]
            expected = {(section, i) for i, row in enumerate(rows) if row['scope'] == 'execute'}
            need({key for key in m['finished_cases'] if key[0] == section} == expected,
                 'section output closure missing local case')
            names = ['observations.jsonl.gz', 'comparison.json']
            if section == 'predecessors':
                names.append('historical-comparison.json')
            additions = {m['collection'] / section / name: 'file' for name in names}
        elif phase == 'collection-close':
            need(section is None and m['section'] is None and
                 m['finished_sections'] == list(_OUT_SECTIONS), 'incomplete collection close')
            additions = {m['collection'].parent / 'collection.json': 'file'}
        else:
            need(False, 'unknown finite public output transition')
        need(not any(os.path.lexists(path) for path in additions), 'pre-existing output transition member')
        m['transition'] = {'phase': phase, 'additions': additions}
        yield
        actual = _out_audit(m, additions=additions)
        need(all(str(path) in actual and actual[str(path)]['kind'] == kind for path, kind in additions.items()),
             'missing or wrong output transition member')
        sealed = {}
        for path, kind in additions.items():
            row = actual[str(path)]
            if kind == 'file':
                row = dict(row, sha256=_out_hash(path, row['stat'], m, _out_raw_deadline(m)))
            sealed[str(path)] = row
        _out_promote(m, sealed)
        m['transition'] = None
        if phase == 'section-open':
            m['section'] = section
            m['case_order'] = tuple(i for i, row in enumerate(_PUBLIC_CONTRACTS[runtime_permit]['rosters'][section])
                                    if row['scope'] == 'execute')
            m['case_next'] = 0
        elif phase == 'section-close':
            m['section'] = None
            m['finished_sections'].append(section)
        else:
            m['closed'] = True
    except BaseException:
        m['run']['failed'] = True
        raise


def _public_output_operation_begin(invocation, runtime_permit, kind, case, directory):
    """Private hook before operation directory/command creation, not public API."""
    if kind not in ('public-runtime', 'public-native') or invocation not in _OUTPUT_MONITORS:
        return None
    m = _out_get(invocation)
    active = m['case']
    need(m['permit'] is runtime_permit and active is not None and m['operation'] is None and
         case['section'] == active['section'] and case['index'] == active['index'],
         'public process outside exact active case')
    need(case['variant'] == 'ordinary' or
         case['variant'] == 'observer' and case['section'] in ('lifecycle', 'guards'),
         'unexpected public case variant')
    key = kind, case['variant']
    need(key not in active['operations'], 'repeated compiler/native operation in public case')
    if kind == 'public-runtime':
        need(case['variant'] == 'ordinary' and not active['operations'] or
             case['variant'] == 'observer' and ('public-runtime', 'ordinary') in active['operations'],
             'public compiler variant order')
    else:
        need(('public-runtime', case['variant']) in active['operations'] and
             not (case['variant'] == 'ordinary' and ('public-runtime', 'observer') in active['operations']),
             'native execution outside current compiled variant')
    expected = m['root'] / 'operations' / ('%04d' % m['run']['next'])
    need(Path(directory) == expected and not os.path.lexists(expected), 'exact fresh current operation root')
    public_output_tick(invocation)
    m['operation'] = {'root': expected, 'kind': kind, 'case': dict(case),
                      'registration': None, 'seen': False}
    active['variants'].add(case['variant'])
    active['operations'].append(key)
    return m


def _public_output_operation_registration(m, registration):
    if m is not None:
        need(m['operation'] is not None, 'native registration without public operation')
        m['operation']['registration'] = registration


def _public_output_operation_terminal(m, *, deadline):
    """Successful owned cleanup: active scan and due full audit; case stays live.

    Full-root terminal/failure auditing applies at collection/failure barriers,
    not unconditionally at each successful child return.
    """
    if m is not None:
        public_output_tick(m['invocation'], deadline=deadline)
        m['operation'].pop('deadline',None)


def _public_output_operation_seal(m):
    """After successful _retain_terminal, before returning the issued probe."""
    if m is None:
        return
    try:
        root = m['operation']['root']
        _out_check_ancestors(m, root)
        rows, _ = _out_snapshot(m, [root], deadline=_out_raw_deadline(m), hash_bodies=True)
        need(set(rows) == {str(root)} | {str(root / name) for name in ('command.json', 'stdout', 'stderr', 'result.json')}
             and rows[str(root)]['kind'] == 'directory' and
             all(row['kind'] == 'file' for path, row in rows.items() if path != str(root)),
             'exact terminal operation four-file closure')
        public_output_tick(m['invocation'])
        _out_promote(m, rows)
        m['operation'] = None
    except BaseException:
        m['run']['failed'] = True
        raise


def _public_output_failure_audit(m, *, deadline):
    """Private failure-only hook AFTER owned-tree cleanup, before emergency writes.

    Read-only, bounded, never promotes rows and never makes a failure successful.
    The caller retains its original exception/status and this verification flag.
    """
    if m is None:
        return None
    need(not m.get('failure_read', False), 'nested failure output audit')
    m['failure_read'] = True
    try:
        _out_audit(m, deadline=deadline)
        result = {'verified': True, 'last_full_audit': m['last_audit']}
    except BaseException as error:
        result = {'verified': False, 'error': repr(error)[:2048]}
    finally:
        m['failure_read'] = False
        m['run']['failed'] = True
    m['failure_audit'] = result
    return dict(result)


def public_output_failure(invocation, runtime_permit):
    """Retain one bounded failure audit after the caller's owned cleanup."""
    m = _OUTPUT_MONITORS.get(invocation)
    if m is None:
        return {'verified': False, 'error': 'public output monitor not started'}
    need(m['invocation'] is invocation and m['permit'] is runtime_permit and
         m['thread'] == threading.get_ident(), 'original public failure monitor required')
    _runtime_permit_owner(runtime_permit, invocation, 'public')
    if 'failure_audit' not in m:
        _public_output_failure_audit(m, deadline=m['run']['deadline'])
    return dict(m['failure_audit'])


def _public_output_report(m):
    if m is None:
        return None
    audits = m['audits']
    gaps = [right['completed'] - left['completed'] for left, right in zip(audits, audits[1:])]
    return {'schema': 'public-output-sampled-ledger-v1',
            'full_observed_peak_bytes': m['full_peak'],
            'active_accounted_peak_bytes': m['accounted_peak'],
            'sealed_bytes': m['sealed_bytes'], 'full_audit_count': len(audits),
            'last_full_audit': m['last_audit'], 'max_completed_audit_gap': max(gaps, default=0),
            'continuous_peak_claim': False, 'body_cache_authority': False,
            'invocation_poisoned': m['run']['failed']}


# BEGIN EXPLICIT PRODUCTION SUCCESSOR
"""Production-only ownership branch of the retained complete supervisor.

The six changes above only select the fixed production compile resource class,
check its original invocation, and include its separately owned evidence root in
the existing output traversal. The process loop/cleanup/terminal algorithms are
not reimplemented. Imported Unit4 entrypoints below fail before owner lookup.
"""
import contextvars as _production_contextvars
import weakref as _production_weakref

PRODUCTION_KEY = '_oxid_package_dependency_production_process_v1'
SOURCE_OWNER_KEY = '_oxid_package_dependency_ci_source_v1'
BASELINE = {'path': 'tests/qualification/package_dependency_unit4_current/preparation.py',
            'bytes': 117696, 'sha256': '7c475867d93065b123f5c6f4acd12a7c622efb0c8392c0f7748d0637fa11e6e6'}
PRODUCTION_EXECUTION_REVIEWED = False
PRODUCTION_RECIPES = frozenset(('quality-and-repository:release-production',
    'native-scalar-preview:release-production', 'cross-platform:linux-x86_64:release-production',
    'cross-platform:windows-x86_64:release-production', 'cross-platform:macos-x86_64:release-production',
    'cross-platform:macos-aarch64:release-production'))
PRODUCTION_OPERATIONS = frozenset(('rustup-which-cargo', 'rustup-which-rustc',
    'rustc-version', 'cargo-version', 'rustc-sysroot', 'native-cc-version',
    'native-cxx-version', 'native-ar-version', 'metadata-before', 'metadata-after', 'build'))
NEUTRAL_REUSE = ('need', 'sha', 'canonical', 'serial', 'decode', 'real', 'body', 'identity',
    '_module', '_sample_linux', '_operation_process_cap', '_output_entries', '_supervise',
    '_retain_terminal', '_emergency_terminal')
UNIT4_PUBLIC_ENTRIES = ('context_api', 'verify_invocation', 'full_verify_invocation',
    'cancel_invocation', 'checked_scope', 'ProbeRecord', 'PreparationRecord', 'check_deadline',
    'register_executable_view', 'RuntimePermit', 'register_parser_runtime', 'register_public_runtime',
    'register_public_contracts', 'register_public_guard', 'register_public_native',
    'register_public_tools', 'register_public_hosted', 'public_inputs', 'public_replay',
    'owned_probe', 'verify_probe', 'produce', 'verify_record', 'public_output_start',
    'public_output_tick', 'public_case_scope', 'public_output_boundary', 'public_output_failure')

def _unit4_api_unavailable(*args, **kwargs):
    raise Rejected('Unit4 API is unavailable in the distinct production owner')

for _unit4_entry in UNIT4_PUBLIC_ENTRIES:
    globals()[_unit4_entry] = _unit4_api_unavailable

_PRODUCTION_INVOCATIONS = _production_weakref.WeakKeyDictionary()
_PRODUCTION_OPERATIONS = _production_weakref.WeakKeyDictionary()
_PRODUCTION_USED = set()
_PRODUCTION_ACTIVE = _production_contextvars.ContextVar('production_process_invocation', default=None)


def _production_source():
    owner = globals().get('_SOURCE_OWNER')
    need(type(owner) is types.ModuleType and sys.modules.get(SOURCE_OWNER_KEY) is owner,
         'authenticated original production source owner required')
    module = sys.modules.get(PRODUCTION_KEY)
    need(type(module) is types.ModuleType and module.__dict__ is globals(),
         'original canonical production process module required')
    owner.owner()
    need(owner.owns_production_owner(module), 'unissued production process module')
    return owner


class ProductionInvocation:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise Rejected('ProductionInvocation is issued only by begin_production')
    def document(self):
        return decode(_production_entry(self)['raw'])


class ProductionOperationRecord:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise Rejected('ProductionOperationRecord is issued only by owned_production_operation')
    def document(self):
        _production_source()
        need(type(self) is ProductionOperationRecord and self in _PRODUCTION_OPERATIONS,
             'original production operation record required')
        return decode(_PRODUCTION_OPERATIONS[self]['raw'])
    def stdout(self):
        return _production_stream(self, 'stdout')
    def stderr(self):
        return _production_stream(self, 'stderr')


def _production_entry(invocation):
    need(type(invocation) is ProductionInvocation and invocation in _PRODUCTION_INVOCATIONS,
         'original production invocation required')
    _production_source()
    return _PRODUCTION_INVOCATIONS[invocation]


def production_context(invocation):
    return _production_entry(invocation)['context']


def production_tick(invocation):
    # Association/deadline only inside bounded reads and the existing process
    # loop. Whole-source readmissions bracket every effect and final issuance.
    need(type(invocation) is ProductionInvocation and invocation in _PRODUCTION_INVOCATIONS,
         'original production invocation checkpoint required')
    state = _PRODUCTION_INVOCATIONS[invocation]
    need(state['thread'] == threading.get_ident(), 'cross-thread production invocation read/effect')
    if state['cancelled']:
        raise Cancelled('production invocation cancelled')
    need(not state['failed'] and time.monotonic() < state['deadline'],
         'production invocation failed or deadline expired')
    owner = globals().get('_SOURCE_OWNER')
    need(sys.modules.get(SOURCE_OWNER_KEY) is owner and
         getattr(sys.modules.get(PRODUCTION_KEY), '__dict__', None) is globals(),
         'production checkpoint owner drift')
    owner.tick()


def tick():
    invocation = _PRODUCTION_ACTIVE.get()
    if invocation is not None:
        production_tick(invocation)


def remaining_seconds():
    invocation = _PRODUCTION_ACTIVE.get()
    if invocation is None:
        return 36000.0
    production_tick(invocation)
    return max(0.0, _PRODUCTION_INVOCATIONS[invocation]['deadline'] - time.monotonic())


@contextmanager
def _production_scope(invocation):
    need(_PRODUCTION_ACTIVE.get() in (None, invocation), 'cross-invocation production read scope')
    token = _PRODUCTION_ACTIVE.set(invocation)
    try:
        production_tick(invocation)
        yield
        production_tick(invocation)
    finally:
        _PRODUCTION_ACTIVE.reset(token)


def _production_directory(path):
    path = real(path)
    value = path.lstat()
    need(stat.S_ISDIR(value.st_mode), 'production parent is not a directory')
    return {'dev': value.st_dev, 'ino': value.st_ino, 'mode': value.st_mode,
            'uid': value.st_uid, 'gid': value.st_gid}


def _production_recipe(source, context, recipe_id):
    need(type(recipe_id) is str and recipe_id in PRODUCTION_RECIPES, 'fixed production build recipe required')
    doc = source.verify_job(context)
    recipe = source.registry(context)['build_recipes'].get(recipe_id)
    need(type(recipe) is dict and recipe.get('id') == recipe_id and
         doc['job_recipe_id'] in recipe['job_recipe_ids'], 'production job/recipe mismatch')
    need(recipe['role'] == 'production' and recipe['profile'] == 'release' and
         recipe['argv'] == ['cargo', 'build', '--release', '--locked'] and
         recipe['projection_id'] == 'current-all376', 'unchanged original production recipe required')
    need(recipe['manifest_path'] == 'Cargo.toml' and recipe['package_name'] == 'oxid' and
         recipe['target_kind'] == 'bin' and recipe['target_name'] == 'oxid' and
         recipe['target_path'] == 'src/cli.rs', 'fixed production package/target required')
    suffix = '.exe' if doc['host'] == 'windows-x86_64' else ''
    need(recipe['output_relative'] == 'target/release/oxid' + suffix and
         recipe['artifact_key'] == recipe_id.replace(':', '-') and
         re.fullmatch('[a-z0-9_-]+', recipe['artifact_key']), 'fixed production output/evidence key')
    route = source.route(context, recipe['original_id'])
    original = route.get('original_record', {}).get('original', route.get('original', {}))
    need(original.get('command') == 'cargo build --release --locked', 'original production command drift')
    projection = source.projection(context, 'current-all376')
    need(projection['files'] == doc['source_files'] and len(projection['files']) == 376,
         'complete original current-all376 source required')
    return doc, recipe


def _production_paths(doc, recipe):
    runner = doc['runner_temp']
    need(type(runner) is dict and set(runner) == {'path', 'identity'}, 'admitted runner temporary root required')
    parent = real(runner['path'])
    need(_production_directory(parent) == runner['identity'], 'runner temporary root identity drift')
    repo = real(doc['repository_path'])
    need(parent != repo and parent not in repo.parents and repo not in parent.parents,
         'production evidence root must be external to checkout')
    root = real(parent / ('package-ci-production-' + doc['nonce'] + '-' + recipe['artifact_key']), False)
    selected = real(repo / recipe['output_relative'], False)
    return repo, parent, root, selected


def begin_production(context, fixed_recipe_id):
    source = _production_source()
    doc, recipe = _production_recipe(source, context, fixed_recipe_id)
    need(doc['host'] != 'windows-x86_64', 'NotReady: Windows descendant terminal containment is unreviewed')
    source.require_operational(context, recipe)
    need(PRODUCTION_EXECUTION_REVIEWED is True,
         'NotReady: production owner source implementation awaits independent execution review')
    source.load_sibling('artifacts')._policies(recipe)
    repo, parent, root, selected = _production_paths(doc, recipe)
    need(not os.path.lexists(root), 'production evidence root must be fresh')
    need(not os.path.lexists(selected), 'selected executable preexists; cache decision is required')
    key = (context, fixed_recipe_id)
    need(key not in _PRODUCTION_USED, 'source context/production recipe was already consumed')
    invocation = object.__new__(ProductionInvocation)
    started = time.time_ns()
    document = {'schema': 'oxid-production-invocation-v1', 'source': doc, 'recipe': recipe,
                'nonce': doc['nonce'], 'started_ns': started, 'evidence_root': str(root),
                'cargo_network_policy': 'original-online-command; no network-isolation claim'}
    state = {'raw': canonical(document), 'context': context, 'root': root, 'repo': repo,
             'parent_identity': _production_directory(parent),
             'deadline': time.monotonic() + 36000, 'thread': threading.get_ident(), 'next': 0, 'failed': False,
             'cancelled': False, 'active': None, 'operations': {}, 'created': False}
    _PRODUCTION_USED.add(key)
    _PRODUCTION_INVOCATIONS[invocation] = state
    try:
        root.mkdir()
        (root / 'operations').mkdir()
        state['created'] = True
        state['root_identity'] = _production_directory(root)
        state['operations_identity'] = _production_directory(root / 'operations')
        verify_production(invocation)
    except BaseException:
        state['failed'] = True
        raise
    return invocation


def verify_production(invocation):
    state = _production_entry(invocation)
    production_tick(invocation)
    source = _production_source()
    original = decode(state['raw'])
    doc, recipe = _production_recipe(source, state['context'], original['recipe']['id'])
    need(doc == original['source'] and recipe == original['recipe'], 'production source/recipe drift')
    repo, parent, root, selected = _production_paths(doc, recipe)
    need(repo == state['repo'] and root == state['root'] and str(root) == original['evidence_root'] and
         _production_directory(parent) == state['parent_identity'], 'production root association drift')
    need(state['created'] and _production_directory(root) == state['root_identity'] and
         _production_directory(root / 'operations') == state['operations_identity'],
         'production evidence directory identity drift')
    return original


def cancel_production(invocation):
    state = _production_entry(invocation)
    state['cancelled'] = True
    state['failed'] = True


def _production_output_roots(invocation, selected_root):
    state = _production_entry(invocation)
    target = real(state['repo'] / 'target', False)
    evidence = real(state['root'])
    need(selected_root in (target, evidence), 'production supervisor output root mismatch')
    need(_production_directory(evidence) == state['root_identity'], 'production output root changed')
    return (target, evidence)


def _production_descriptor(invocation, operation_id):
    need(type(operation_id) is str and operation_id in PRODUCTION_OPERATIONS,
         'fixed production operation ID required')
    source = _production_source()
    machine = source.load_production_owner('machine')
    value = machine.production_operation_inputs(invocation, operation_id)
    need(type(value) is dict and set(value) == {'argv', 'cwd', 'environment', 'timeout_seconds',
                                             'stream_limit_bytes', 'kind'}, 'closed owner operation descriptor')
    need(type(value['argv']) is list and value['argv'] and
         all(type(x) is str and '\0' not in x for x in value['argv']), 'fixed string argv required')
    need(type(value['environment']) is dict and all(type(k) is str and type(v) is str and
         '\0' not in k and '\0' not in v for k, v in value['environment'].items()), 'fixed environment required')
    need(type(value['timeout_seconds']) is int and 0 < value['timeout_seconds'] <= 1800 and
         type(value['stream_limit_bytes']) is int and 0 < value['stream_limit_bytes'] <= 8 * 1024 ** 2,
         'fixed bounded production operation resources')
    document = invocation.document()
    need(value['cwd'] == document['source']['repository_path'] and
         'CARGO_TARGET_DIR' not in value['environment'], 'original production cwd/target environment required')
    if operation_id == 'build':
        need(value['kind'] == 'ci-production-build' and value['argv'] == ['cargo', 'build', '--release', '--locked'],
             'unchanged production build vector required')
    elif operation_id.startswith('metadata-'):
        need(value['kind'] == 'ci-production-metadata' and
             value['argv'] == ['cargo', 'metadata', '--format-version', '1', '--locked', '--no-deps'],
             'fixed separate production metadata probe required')
    else:
        need(value['kind'] == 'ci-production-probe', 'fixed production discovery/version probe required')
    return _copy(value)


def _production_phase(invocation, operation_id):
    state = _production_entry(invocation)
    if operation_id not in ('metadata-before', 'build', 'metadata-after'):
        need(not any(name in state['operations'] for name in ('metadata-before', 'build', 'metadata-after')),
             'tool discovery/version probe after production build phase')
        return
    source = _production_source()
    source.load_production_owner('machine').verify_production_for_operation(invocation, operation_id)
    if operation_id == 'metadata-before':
        need('build' not in state['operations'] and 'metadata-after' not in state['operations'],
             'production metadata-before must precede build')
        return
    prior = state['operations'].get('metadata-before')
    need(prior is not None and prior['record'] is not None, 'original successful metadata-before is required')
    verify_production_operation(prior['record'], invocation, 'metadata-before')
    document = invocation.document()
    source.load_sibling('artifacts')._metadata_exact(prior['record'].stdout(), state['repo'],
        document['recipe'], document['source']['source_files'])
    if operation_id == 'metadata-after':
        build = state['operations'].get('build')
        need(build is not None and build['record'] is not None, 'original successful build before metadata-after')
        verify_production_operation(build['record'], invocation, 'build')


def _production_stream(record, name):
    need(type(record) is ProductionOperationRecord and record in _PRODUCTION_OPERATIONS,
         'original production operation stream required')
    owner = _PRODUCTION_OPERATIONS[record]
    with _production_scope(owner['invocation']):
        row = decode(owner['raw'])[name]
        need(identity(row['path']) == row, 'production raw stream drift')
        return body(row['path'])


def _owned_production_operation(invocation, operation_id):
    document = verify_production(invocation)
    state = _production_entry(invocation)
    need(PRODUCTION_EXECUTION_REVIEWED is True, 'NotReady: production execution source review remains held')
    need(document['source']['host'] != 'windows-x86_64', 'NotReady: Windows descendant terminal containment')
    need(state['active'] is None and operation_id not in state['operations'],
         'production operation is active or was already attempted')
    descriptor = _production_descriptor(invocation, operation_id)
    _production_phase(invocation, operation_id)
    need(time.monotonic() + descriptor['timeout_seconds'] <= state['deadline'], 'production operation deadline')
    recipe = document['recipe']
    selected = real(state['repo'] / recipe['output_relative'], False)
    if operation_id in ('metadata-before', 'build'):
        need(not os.path.lexists(selected), 'selected executable preexists; no cache adoption or deletion')
    with _production_scope(invocation):
        argv, env = descriptor['argv'], descriptor['environment']
        route = Path(argv[0]) if Path(argv[0]).is_absolute() else Path(shutil.which(argv[0], path=env.get('PATH', '')) or '')
        need(route.is_absolute(), 'production executable unavailable in original PATH')
        executable = real(route.resolve(strict=True))
        executable_before = identity(executable)
        tree_path = state['repo'] / TREE_PATH
        tree = _module(tree_path, '_oxid_package_dependency_production_tree_v1', TREE_SHA)
        tree_before = identity(tree_path)
        state['next'] += 1
        directory = state['root'] / 'operations' / ('%04d' % state['next'])
        state['operations'][operation_id] = {'directory': directory, 'record': None, 'descriptor': canonical(descriptor)}
        directory.mkdir()
        command = dict(descriptor, operation_id=operation_id, invocation_nonce=document['nonce'])
        (directory / 'command.json').write_bytes(serial(command))
        state['active'] = operation_id
        outcome = streams = None
        issued_record = None
        try:
            # Recheck after all preparation IO, immediately before owned launch.
            verify_production(invocation)
            if operation_id in ('metadata-before', 'build'):
                need(not os.path.lexists(selected), 'selected executable appeared immediately before launch')
            outcome, streams = _supervise(tree.ProcessTree, argv, descriptor['cwd'], env,
                descriptor['timeout_seconds'], descriptor['stream_limit_bytes'],
                output_root=state['repo'] / 'target', operation_kind=descriptor['kind'],
                transient_roots=(state['repo'] / 'target',), production_invocation=invocation)
            result = dict(command, schema='oxid-production-owned-operation-v1', **outcome)
            result.update(executable=executable_before, executable_route={'path': str(route), 'target': str(executable)},
                          process_tree=tree_before, cargo_offline=False, network_isolated=False)
            if type(result['status']) is not int or result['status'] != 0:
                result['failure'] = result['failure'] or 'production child returned nonzero status: ' + repr(result['status'])
            try:
                verify_production(invocation)
                if operation_id in ('metadata-before', 'build', 'metadata-after'):
                    _production_source().load_production_owner('machine').verify_production_for_operation(invocation, operation_id)
                need(route.resolve(strict=True) == executable and identity(executable) == executable_before,
                     'production executable route/body drift')
                need(identity(tree_path) == tree_before, 'production process helper drift')
            except BaseException as error:
                result['failure'] = result['failure'] or repr(error)
                _emergency_terminal(directory, result, streams, error, state)
            evidence = _retain_terminal(directory, result, streams, state)
            record = object.__new__(ProductionOperationRecord)
            issued_record = record
            _PRODUCTION_OPERATIONS[record] = {'invocation': invocation, 'operation_id': operation_id,
                'directory': directory, 'raw': canonical(result), 'descriptor': canonical(descriptor), **evidence}
            state['operations'][operation_id]['record'] = record
            verify_production_operation(record, invocation, operation_id)
            return record
        except BaseException:
            if issued_record is not None:
                _PRODUCTION_OPERATIONS.pop(issued_record, None)
                state['operations'][operation_id]['record'] = None
            state['failed'] = True
            raise
        finally:
            state['active'] = None


def verify_production_operation(record, invocation, operation_id):
    need(type(record) is ProductionOperationRecord and record in _PRODUCTION_OPERATIONS,
         'original production operation required; Unit4/data are inert')
    owned = _PRODUCTION_OPERATIONS[record]
    need(owned['invocation'] is invocation and owned['operation_id'] == operation_id,
         'production record belongs to another invocation/operation')
    document = verify_production(invocation)
    state = _production_entry(invocation)
    descriptor = _production_descriptor(invocation, operation_id)
    need(owned['descriptor'] == canonical(descriptor) and
         state['operations'][operation_id]['record'] is record, 'production operation owner association drift')
    result = decode(owned['raw'])
    for name, value in descriptor.items():
        need(result[name] == value, 'production recorded command/limits drift')
    need(result['operation_id'] == operation_id and result['invocation_nonce'] == document['nonce'],
         'production operation source association drift')
    need(type(result['pid']) is int and result['pid'] > 0 and type(result['status']) is int and
         result['status'] == 0 and document['started_ns'] <= result['started_ns'] <= result['completed_ns'] <= time.time_ns(),
         'actual production process window/status required')
    need(result['cleanup_complete'] is True and result['cleanup_error'] is None and result['failure'] is None and
         result['timed_out'] is False and result['stream_limit_exceeded'] is False, 'complete bounded production operation required')
    need({p.name for p in owned['directory'].iterdir()} == {'command.json', 'stdout', 'stderr', 'result.json'},
         'closed four-file production terminal evidence required')
    with _production_scope(invocation):
        for row in (owned['command'], owned['result'], result['executable'], result['process_tree']):
            need(identity(row['path']) == row, 'production operation/tool evidence drift')
        need(str(Path(result['executable_route']['path']).resolve(strict=True)) == result['executable_route']['target'],
             'production executable resolution drift')
        need(decode(body(owned['directory'] / 'result.json')) == result, 'production terminal body differs from original owner')
        for name in ('stdout', 'stderr'):
            raw = _production_stream(record, name)
            need(len(raw) <= descriptor['stream_limit_bytes'] and len(raw) == result[name]['bytes'] and
                 sha(raw) == result[name]['sha256'], 'production raw stream identity/bound')
    return result


def retain_production_failure(invocation, error):
    """Bounded failure-only summary; owned operation evidence is never replaced."""
    state = _production_entry(invocation)
    need(isinstance(error, BaseException), 'actual production failure exception required')
    state['failed'] = True
    if 'attempt_failure' in state:
        return _copy(state['attempt_failure'])
    original = decode(state['raw'])
    value = {'schema': 'oxid-production-failed-attempt-v1', 'success': False,
             'invocation_nonce': original['nonce'], 'recipe_id': original['recipe']['id'],
             'error_type': type(error).__name__, 'error': str(error)[:8192],
             'completed_ns': time.time_ns(), 'source_readback': 'not reverified after failure',
             'operations': [{'id': name, 'evidence_directory': str(row['directory']),
                             'completed_record': row['record'] is not None}
                            for name, row in state['operations'].items()],
             'terminal_failure': state.get('failed_terminal'), 'preservation_errors': []}
    path = state['root'] / 'failed-attempt.json'
    try:
        need(_production_directory(state['root']) == state['root_identity'], 'failure evidence root drift')
        raw = serial(value)
        need(len(raw) <= 2 * 1024 ** 2, 'bounded failure summary')
        with path.open('xb') as stream:
            stream.write(raw)
    except BaseException as preservation_error:
        value['preservation_errors'].append(type(preservation_error).__name__ + ': ' + str(preservation_error)[:2048])
    state['attempt_failure'] = value
    return _copy(value)



def owned_production_operation(invocation, operation_id):
    """One fixed original operation, including prelaunch-failure ownership."""
    need(type(invocation) is ProductionInvocation and invocation in _PRODUCTION_INVOCATIONS,
         'original production invocation required before operation')
    state = _PRODUCTION_INVOCATIONS[invocation]
    try:
        return _owned_production_operation(invocation, operation_id)
    except BaseException as error:
        # The enclosing read-scope terminal checkpoint can also fail after the
        # inner launch block returned. Revoke that provisional operation too.
        operation = state['operations'].get(operation_id) if type(operation_id) is str else None
        if operation is not None and operation.get('record') is not None:
            _PRODUCTION_OPERATIONS.pop(operation['record'], None)
            operation['record'] = None
        state['failed'] = True
        if state.get('created') and PRODUCTION_EXECUTION_REVIEWED is True:
            try:
                retain_production_failure(invocation, error)
            except BaseException:
                pass
        raise
