"""Producer-owned source preparation and finite process records.

Records are live invocation capabilities, not portable attestations. All runtime
and CI qualification remains separate. No subprocess is launched during import.
"""
import hashlib,json,math,os,queue,re,shutil,signal,stat,sys,threading,time,types
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

def _sample_linux(group, *, deadline=None):
 found=[]
 def check():
  if deadline is not None:need(time.monotonic()<deadline,'operation process sample deadline')
 check()
 if not sys.platform.startswith('linux'):return found
 for p in Path('/proc').iterdir():
  check()
  if not p.name.isdigit():continue
  try:
   raw=(p/'stat').read_text();check();f=raw[raw.rfind(')')+2:].split();pgid=int(f[2]);rss=int(f[21])*os.sysconf('SC_PAGE_SIZE')
  except (FileNotFoundError,ProcessLookupError,PermissionError):continue
  if pgid==group:found.append({'pid':int(p.name),'rss':rss,'state':f[0]})
 check()
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
 return 16 if not preparation and operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build') else 4

def _supervise(tree_class,argv,cwd,environment,timeout,limit,*,preparation=False,output_root=None,operation_kind='probe',transient_roots=(),allowed_links=None,native_plan=None,output_monitor=None):
 """Owned tree, bounded streams and cleanup; cancellation always enters finally."""
 started=time.time_ns();deadline=time.monotonic()+timeout;tree=None;readers=[];streams={'stdout':bytearray(),'stderr':bytearray()};events=queue.Queue(maxsize=8);closed=set();exceeded=False;timed_out=False;cleanup=None;status=None;pid=None;failure=None
 if output_monitor is not None:output_monitor['operation']['deadline']=deadline
 native_registration=None;owned_service=None;old_handlers={};peaks={'processes':0,'rss_bytes':0,'output_bytes':None if output_monitor is not None else 0}
 compile_operation=operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build')
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
 def accept_event(name,data):
  nonlocal exceeded
  need(name!='error','stream reader failed: '+str(data))
  need(name in streams,'unexpected stream event')
  if not data:closed.add(name);return
  room=limit-len(streams[name]);streams[name].extend(data[:room])
  if len(data)>room:exceeded=True;need(False,'operation stream byte bound')
 def service_fast(effective_deadline):
  # Owner-created nonrecursive closure, never supplied by a public caller.
  nonlocal timed_out
  def stage(name):
   attempt=None if output_monitor is None else output_monitor.get('current_attempt')
   if attempt is not None:attempt['fast_phase']=name
  stage('wall')
  now=time.monotonic()
  if now>=deadline:timed_out=True;need(False,'operation wall deadline')
  need(now<effective_deadline,'output service audit deadline')
  if sys.platform.startswith('linux'):
   stage('process-rss')
   try:members=_sample_linux(pid,deadline=min(deadline,effective_deadline))
   except BaseException:
    if time.monotonic()>=deadline:timed_out=True
    raise
   rss=sum(r['rss'] for r in members);peaks['processes']=max(peaks['processes'],len(members));peaks['rss_bytes']=max(peaks['rss_bytes'],rss)
   need(len(members)<=policy['processes'] and rss<=policy['rss_bytes'],'operation process/RSS bound')
  stage('streams')
  if output_monitor is not None:
   stats=_out_stats(output_monitor);stats['queue_peak_events']=max(stats['queue_peak_events'],events.qsize())
  for _ in range(8):
   try:name,data=events.get_nowait()
   except queue.Empty:break
   accept_event(name,data)
  if output_monitor is not None:
   stats['stream_peak_bytes']=max(stats['stream_peak_bytes'],sum(map(len,streams.values())))
  stage('poll')
  tree.child.poll()
  stage('final-wall')
  now=time.monotonic()
  if now>=deadline:timed_out=True;need(False,'operation wall deadline')
  need(now<effective_deadline,'output service audit deadline')
 try:
  if threading.current_thread() is threading.main_thread():
   for sig in (signal.SIGINT,signal.SIGTERM):old_handlers[sig]=signal.signal(sig,cancel)
  tree=tree_class(argv,cwd,environment,new_session=True);pid=tree.child.pid;native_registration=_native_register(native_plan,pid);_public_output_operation_registration(output_monitor,native_registration)
  for name in streams:
   thread=threading.Thread(target=reader,args=(getattr(tree.child,name),name),daemon=True);thread.start();readers.append(thread)
  if output_monitor is not None:
   need(_OUTPUT_MONITORS.get(output_monitor['invocation']) is output_monitor and
        output_monitor['operation'] is not None and output_monitor['thread']==threading.get_ident() and
        output_monitor.get('owned_service') is None and id(output_monitor) not in _OWNED_OUTPUT_SERVICES,
        'overlapping or foreign owned output service')
   owned_service={'monitor':output_monitor,'operation':output_monitor['operation'],
                  'invocation':output_monitor['invocation'],'thread':threading.get_ident(),
                  'pid':pid,'deadline':deadline,'active':True,'service':service_fast}
   _OWNED_OUTPUT_SERVICES[id(output_monitor)]=owned_service
   output_monitor['owned_service']=owned_service
   output_monitor['last_fast']=output_monitor['last_live']=time.monotonic()
  while len(closed)!=2 or tree.child.poll() is None:
   if time.monotonic()>=deadline:timed_out=True;break
   if output_monitor is not None:_out_service_fast(output_monitor,deadline,force=True)
   else:service_fast(deadline)
   if output_monitor is not None:public_output_tick(output_monitor['invocation'],deadline=deadline)
   elif output_root is not None:
    total=0
    for p in _output_entries(output_root,native_registration,deadline,transient_roots):
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
   if len(closed)==2 and tree.child.poll() is not None:break
   try:name,data=events.get(timeout=.05)
   except queue.Empty:continue
   accept_event(name,data)
  if not timed_out and not exceeded:status=tree.child.wait(timeout=max(.01,deadline-time.monotonic()))
 except BaseException as error:
  failure=error
  if time.monotonic()>=deadline:timed_out=True
 finally:
  # Detach before any close/wait/drain/terminal preservation work.
  if owned_service is not None:
   owned_service['active']=False
   if output_monitor.get('owned_service') is owned_service:output_monitor['owned_service']=None
   else:failure=failure or Rejected('owned output service detach identity drift')
   if _OWNED_OUTPUT_SERVICES.get(id(output_monitor)) is owned_service:del _OWNED_OUTPUT_SERVICES[id(output_monitor)]
   else:failure=failure or Rejected('owned output service registry drift')
  if tree is not None:
   try:tree.close()
   except BaseException as error:cleanup=cleanup or repr(error)
   try:
    try:status=tree.child.wait(timeout=5)
    except Exception:tree.child.kill();status=tree.child.wait(timeout=2)
   except BaseException as error:cleanup=cleanup or repr(error)
  for sig,handler in old_handlers.items():
   try:signal.signal(sig,handler)
   except BaseException as error:cleanup=cleanup or repr(error)
  # Drain queued bytes and reader errors/EOF without exceeding cleanup grace.
  until=time.monotonic()+.5
  while (any(t.is_alive() for t in readers) or not events.empty()) and time.monotonic()<until:
   try:name,data=events.get(timeout=.02)
   except queue.Empty:continue
   try:accept_event(name,data)
   except BaseException as error:failure=failure or error
  for t in readers:
   try:t.join(timeout=.01)
   except BaseException as error:cleanup=cleanup or repr(error)
  if any(t.is_alive() for t in readers):cleanup=cleanup or 'reader cleanup incomplete'
  try:
   if sys.platform.startswith('linux') and pid is not None and any(r['state']!='Z' for r in _sample_linux(pid)):cleanup=cleanup or 'owned process group remains'
  except BaseException as error:cleanup=cleanup or repr(error)
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
_OUT_DUE = 3.0
_OUT_START_LIMIT = 5.0  # New idle/start contract, NOT a completed-detection limit.
_OUT_EXECUTION_SECONDS = 14.0  # Explicit reviewed production contract; raw operation/run deadlines always take precedence.
_OUT_SERVICE_DUE = 0.05  # Scheduling target, not a proven hard service latency.
_OWNED_OUTPUT_SERVICES = {}  # Issued only by the actual _supervise lifetime.
_OUT_STAT = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
_OUT_STABLE_DIR = ('st_dev', 'st_ino', 'st_mode')


def _out_budget():
    value = _OUT_EXECUTION_SECONDS
    need(type(value) in (int, float) and math.isfinite(value) and value > 0,
         'public output audit execution policy not released')
    return float(value)


def _out_stats(m):
    # Constant-size aggregates; successful operation receipts never copy history.
    return m.setdefault('audit_stats', {'attempts': 0, 'failures': 0, 'successes': 0,
        'max_duration': 0.0, 'max_start_lag': 0.0, 'max_completed_gap': 0.0,
        'fast_count': 0, 'fast_seconds': 0.0, 'fast_max_duration': 0.0, 'fast_max_gap': 0.0, 'fast_max_start_gap': 0.0,
        'live_count': 0, 'live_seconds': 0.0, 'live_max_duration': 0.0, 'live_max_gap': 0.0, 'live_max_start_gap': 0.0,
        'live_max_entries': 0, 'live_max_bytes': 0, 'queue_peak_events': 0, 'stream_peak_bytes': 0})


def _out_stage(m, phase, path=None):
    attempt = m.get('current_attempt')
    if attempt is None:
        return
    if m.get('sampling_live'):
        attempt['nested_phase'] = 'live-' + phase
        if path is not None:
            attempt['nested_path'] = os.fspath(path)
        return
    now = time.monotonic()
    if phase != attempt['phase']:
        old = attempt['phase']
        attempt['phase_seconds'][old] = attempt['phase_seconds'].get(old, 0.0) + now - attempt['phase_started']
        attempt['phase'], attempt['phase_started'] = phase, now
    if path is not None:
        attempt['path'] = os.fspath(path)


def _out_count(m, kind):
    attempt = m.get('current_attempt')
    if attempt is not None:
        if m.get('sampling_live'):
            attempt.setdefault('nested_counts', {'entries': 0, 'sealed': 0, 'membership': 0})[kind] += 1
        else:
            attempt['counts'][kind] += 1


def _out_attempt_document(m, attempt, ended):
    row = {key: value for key, value in attempt.items() if key not in ('path', 'nested_path', 'phase_started')}
    row['phase_seconds'] = dict(attempt['phase_seconds'])
    row['phase_seconds'][attempt['phase']] = row['phase_seconds'].get(attempt['phase'], 0.0) + ended - attempt['phase_started']
    row['counts'] = dict(attempt['counts'])
    row['service'] = dict(attempt['service'])
    row['completed'], row['duration'] = ended, ended - attempt['started']
    for name in ('path', 'nested_path'):
        path = attempt.get(name)
        if path is not None:
            relative = os.path.relpath(path, m['root'])
            row[name] = relative[:256]
            row[name + '_sha256'] = hashlib.sha256(os.fsencode(relative)).hexdigest()
    return row


def _out_service_fast(m, deadline, *, force=False):
    owner = m.get('owned_service')
    if owner is None:
        return
    need(_OWNED_OUTPUT_SERVICES.get(id(m)) is owner and owner['monitor'] is m and
         owner['operation'] is m['operation'] and owner['invocation'] is m['invocation'] and
         owner['thread'] == threading.get_ident() == m['thread'] and owner['active'],
         'foreign or expired owned output service')
    need(not m.get('servicing_fast'), 'recursive owned output service')
    now = time.monotonic()
    previous = m.get('last_fast')
    if not force and previous is not None and now < previous + _OUT_SERVICE_DUE:
        return
    m['servicing_fast'] = True
    try:
        attempt = m.get('current_attempt')
        if attempt is not None:
            attempt['fast_service_active'] = True
        owner['service'](min(deadline, owner['deadline']))
        if attempt is not None:
            attempt.pop('fast_service_active', None)
            attempt.pop('fast_phase', None)
    finally:
        primary = sys.exc_info()[1]
        try:
            ended = time.monotonic()
            stats = _out_stats(m)
            stats['fast_count'] += 1
            stats['fast_seconds'] += ended - now
            attempt = m.get('current_attempt')
            if attempt is not None:
                attempt['service']['fast_count'] += 1
                attempt['service']['fast_seconds'] += ended - now
            stats['fast_max_duration'] = max(stats['fast_max_duration'], ended - now)
            if previous is not None:
                stats['fast_max_gap'] = max(stats['fast_max_gap'], ended - previous)
                stats['fast_max_start_gap'] = max(stats['fast_max_start_gap'], now - previous)
            m['last_fast'] = ended
        except BaseException as error:
            m.setdefault('diagnostic_errors', []).append(repr(error)[:512])
            del m['diagnostic_errors'][:-4]
            if primary is None:
                raise
        finally:
            m['servicing_fast'] = False


def _out_checkpoint(m, deadline, *, phase, path=None):
    _out_stage(m, phase, path)
    deadline = _out_raw_deadline(m, deadline)
    _out_service_fast(m, deadline)
    if m.get('owned_service') is not None and not m.get('sampling_live'):
        previous = m.get('last_live')
        if previous is None or time.monotonic() >= previous + _OUT_SERVICE_DUE:
            _out_sample_live(m, deadline=deadline)
    _out_raw_deadline(m, deadline)
    return deadline


def _out_raw_deadline(m, extra=None):
    deadline = m['run']['deadline']
    need(type(deadline) in (int, float) and math.isfinite(deadline), 'finite output run deadline')
    if m.get('operation') is not None and m['operation'].get('deadline') is not None:
        deadline = min(deadline, m['operation']['deadline'])
    if globals().get('_DEADLINE') is not None:
        deadline = min(deadline, _DEADLINE)
    if extra is not None:
        need(type(extra) in (int, float) and math.isfinite(extra), 'finite output extra deadline')
        deadline = min(deadline, extra)
    need((not m['run']['failed'] or m.get('failure_read', False)) and time.monotonic() < deadline,
         'public output monitor deadline or poisoned invocation')
    return deadline


def _out_get(invocation):
    need(invocation in _OUTPUT_MONITORS, 'unissued public output monitor')
    m = _OUTPUT_MONITORS[invocation]
    need(m['thread'] == threading.get_ident() and m['invocation'] is invocation,
         'cross-thread or cross-invocation output monitor')
    if m.get('diagnostic') is not None:
        _diagnostic_association(invocation, m['permit'], m['diagnostic'])
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
    need(stat.S_ISDIR(tmp.lstat().st_mode), 'fixed output tmp root type drift')
    roots.append(tmp)
    for key in ('case', 'operation'):
        active = m[key]
        if active is None:
            continue
        exists = os.path.lexists(active['root'])
        need(exists or not active['seen'], 'active stable output root disappeared')
        if exists:
            need(stat.S_ISDIR(active['root'].lstat().st_mode), 'active stable output root type drift')
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
                _out_audit(m, additions=additions, deadline=deadline, reason='body-seal-due')
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
        _out_checkpoint(m, deadline, phase='snapshot')
        _out_stage(m, 'snapshot', path)
        _out_count(m, 'entries')
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
        _out_checkpoint(m, deadline, phase='snapshot', path=path)

    for path in roots:
        path = os.fspath(path)
        _out_checkpoint(m, deadline, phase='snapshot', path=path)
        try:
            observe(path, os.lstat(path))
        except FileNotFoundError:
            _scan_disappearance(path, m['root'], registration, ())
    while pending:
        _out_checkpoint(m, deadline, phase='snapshot')
        directory, expected = pending.pop()
        fd = None
        first_error = None
        _out_stage(m, 'snapshot', directory)
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
                    _out_checkpoint(m, deadline, phase='snapshot')
                    path = directory + os.sep + entry.name
                    _out_stage(m, 'snapshot', path)
                    try:
                        s = entry.stat(follow_symlinks=False)
                    except FileNotFoundError:
                        _scan_disappearance(path, m['root'], registration, ())
                        continue
                    observe(path, s)
        except FileNotFoundError:
            _scan_disappearance(directory, m['root'], registration, ())
            rows.pop(directory, None)
        except BaseException as error:
            first_error = error
            raise
        finally:
            if fd is not None:
                active_error = sys.exc_info()[1]
                try:
                    os.close(fd)
                except BaseException as error:
                    if first_error is None and active_error is None:
                        raise
                    attempt = m.get('current_attempt')
                    if attempt is not None:
                        attempt.setdefault('close_error', repr(error)[:512])
    return rows, total


def _out_without_digest(row):
    return {key: value for key, value in row.items() if key != 'sha256'}


def _out_same(path, old, new, structural):
    if path in structural and old['kind'] == new['kind'] == 'directory':
        return all(old['stat'][key] == new['stat'][key] for key in _OUT_STABLE_DIR)
    if old['kind'] != new['kind'] or old['stat'] != new['stat']:
        return False
    return old['kind'] != 'link' or all(old[key] == new[key] for key in ('link_text', 'target', 'target_stat'))


def _out_collection_parent(m, actual, additions):
    """One authenticated collection-close parent, never a general ancestor waiver."""
    transition = m['transition']
    if transition is None or transition['phase'] != 'collection-close':
        return None
    parent = m['collection'].parent
    expected = {parent / 'collection.json': 'file'}
    need(transition['additions'] == expected and set(additions) == {str(p) for p in expected},
         'exact collection-close addition authority')
    authority = transition['collection_parent']
    path = str(parent)
    need(authority['path'] == path and m['sealed'].get(path) == authority['before'],
         'original collection-close parent authority')
    now = parent.lstat()
    need(stat.S_ISDIR(now.st_mode) and
         all(getattr(now, key) == value for key, value in authority['identity'].items()),
         'collection-close parent identity/type/mode/owner drift')
    need(path in actual and actual[path]['kind'] == 'directory' and
         actual[path]['stat'] == _out_stat(now), 'collection-close parent changed after snapshot')
    return path


def _out_compare(m, actual, *, additions=(), deadline):
    live, structural = [str(path) for path in _out_live_roots(m)], _out_structure(m)
    additions = {str(path) for path in additions}
    parent = _out_collection_parent(m, actual, additions)
    diagnostic_parent = _out_diagnostic_parent(m, actual, additions)
    need(parent is None or diagnostic_parent is None, 'mixed output terminal authorities')
    parent = parent if parent is not None else diagnostic_parent
    if parent is not None:
        structural.add(parent)
    for path, old in m['sealed'].items():
        _out_checkpoint(m, deadline, phase='sealed', path=path)
        _out_count(m, 'sealed')
        need(path in actual and _out_same(path, old, actual[path], structural),
             'sealed public output metadata or membership drift')
        _out_checkpoint(m, deadline, phase='sealed', path=path)
    for path in actual:
        _out_checkpoint(m, deadline, phase='membership', path=path)
        _out_count(m, 'membership')
        need(path in m['sealed'] or path in additions or
             any(_out_under(path, root) for root in live),
             'unregistered output outside current public case/operation')
        _out_checkpoint(m, deadline, phase='membership', path=path)


def _out_bytes(rows):
    return sum(row['stat']['st_size'] for row in rows.values() if row['kind'] == 'file')


def _out_audit(m, *, additions=(), deadline=None, reason='boundary', initial=False):
    need(not m['auditing'], 'overlapping output audit')
    m['auditing'] = True
    started = time.monotonic()
    stats = _out_stats(m)
    stats['attempts'] += 1
    attempt = {'number': stats['attempts'], 'reason': reason, 'started': started,
               'deadline': None, 'previous_completed': m['last_audit'],
               'phase': 'start', 'phase_started': started, 'phase_seconds': {},
               'counts': {'entries': 0, 'sealed': 0, 'membership': 0},
               'service': {'fast_count': 0, 'fast_seconds': 0.0, 'live_count': 0, 'live_seconds': 0.0},
               'success': False}
    m['current_attempt'] = attempt
    first_error = None
    try:
        budget = m['execution_seconds']
        need(type(budget) in (int, float) and math.isfinite(budget) and budget > 0,
             'finite released output audit execution policy required')
        need(math.isfinite(started), 'invalid output audit start clock')
        attempt['execution_deadline'] = started + budget
        raw_deadline = _out_raw_deadline(m, deadline)
        attempt['raw_deadline'] = raw_deadline
        deadline = _out_raw_deadline(m, min(started + budget, raw_deadline))
        attempt['deadline'] = deadline
        previous = m['last_audit']
        if previous is not None:
            need(math.isfinite(previous) and math.isfinite(started) and previous <= started,
                 'invalid output audit clock')
            lag = started - previous
            attempt['start_lag'] = lag
            stats['max_start_lag'] = max(stats['max_start_lag'], lag)
            if not m.get('failure_read'):
                need(lag <= _OUT_START_LIMIT, 'public full-output audit start cadence missed')
        _out_checkpoint(m, deadline, phase='roots')
        if not initial:
            _out_existing_roots(m)
        else:
            need(previous is None and not m['sealed'], 'initial output audit replay')
        _out_checkpoint(m, deadline, phase='roots')
        registration = None if m['operation'] is None else m['operation']['registration']
        rows, total = _out_snapshot(m, [m['root']], deadline=deadline, registration=registration)
        if initial:
            tmp = str(m['root'] / 'tmp')
            need(tmp in rows and rows[tmp]['kind'] == 'directory', 'fixed output tmp root')
        else:
            _out_compare(m, rows, additions=additions, deadline=deadline)
        _out_checkpoint(m, deadline, phase='final')
        completed = time.monotonic()
        need(completed < deadline, 'output audit completed after deadline')
        attempt['success'], attempt['bytes'] = True, total
        return rows
    except BaseException as error:
        first_error = error
        stats['failures'] += 1
        attempt['error'] = repr(error)[:2048]
        m['run']['failed'] = True
        raise
    finally:
        try:
            ended = time.monotonic()
            document = _out_attempt_document(m, attempt, ended)
            final = time.monotonic()
            document['phase_seconds'][attempt['phase']] += final - ended
            document['completed'], document['duration'] = final, final - started
            if attempt['success']:
                need(final < deadline, 'output audit diagnostics completed after deadline')
            stats['max_duration'] = max(stats['max_duration'], final - started)
            if not attempt['success'] and 'first_failed_attempt' not in m:
                m['first_failed_attempt'] = document
            m['last_attempt'] = document
            m['audits'].append(document)
            del m['audits'][:-16]
            # Commit only after successful bounded diagnostics. Preservation can
            # inspect a stale healthy cadence, but can never repair its anchor.
            if attempt['success'] and not m.get('failure_read'):
                m['last_audit'] = final
                m['full_peak'] = max(m['full_peak'], total)
                stats['successes'] += 1
                if previous is not None:
                    stats['max_completed_gap'] = max(stats['max_completed_gap'], final - previous)
                m['last_success'] = {'started': started, 'completed': final, 'bytes': total,
                                     'entries': attempt['counts']['entries'], 'duration': final - started}
        except BaseException as diagnostic_error:
            m['run']['failed'] = True
            diagnostic = repr(diagnostic_error)[:512]
            m.setdefault('diagnostic_errors', []).append(diagnostic)
            del m['diagnostic_errors'][:-4]
            if first_error is None:
                stats['failures'] += 1
            fallback = {'number': attempt['number'], 'started': started,
                        'deadline': attempt['deadline'], 'phase': attempt['phase'],
                        'counts': dict(attempt['counts']), 'success': False,
                        'error': attempt.get('error', diagnostic), 'diagnostic_error': diagnostic}
            m.setdefault('first_failed_attempt', fallback)
            m['last_attempt'] = fallback
            if first_error is None:
                raise
        finally:
            m['current_attempt'] = None
            m['auditing'] = False


def public_output_start(invocation, runtime_permit):
    need(invocation not in _PUBLIC_DIAGNOSTIC_INVOCATIONS, 'diagnostic invocation cannot start full collection')
    return _public_output_start(invocation, runtime_permit)


def _public_output_start(invocation, runtime_permit, diagnostic=None):
    """After generator/setup, before the first section or case is created."""
    if diagnostic is None:
        need(invocation not in _PUBLIC_DIAGNOSTIC_INVOCATIONS,
             'diagnostic invocation cannot use shared full start')
    else:
        owner = _diagnostic_owner(invocation, runtime_permit, diagnostic)
        need(owner['started'] and owner['terminal'] is None,
             'issued started diagnostic policy required')
    budget = _out_budget()
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
         'audits': [], 'full_peak': 0, 'accounted_peak': 0, 'closed': False,
         'execution_seconds': budget, 'sampling_live': False, 'servicing_fast': False,
         'owned_service': None, 'diagnostic': diagnostic}
    _OUTPUT_MONITORS[invocation] = m
    try:
        rows = _out_audit(m, reason='initial', initial=True)
        tmp = str(m['root'] / 'tmp')
        m['sealed'] = {path: row for path, row in rows.items() if path == tmp or not _out_under(path, tmp)}
        m['sealed_bytes'] = _out_bytes(m['sealed'])
    except BaseException:
        run['failed'] = True
        raise


def _out_sample_live(m, *, deadline):
    need(not m.get('sampling_live'), 'recursive current-root output sample')
    m['sampling_live'] = True
    started, previous = time.monotonic(), m.get('last_live')
    try:
        deadline = _out_raw_deadline(m, min(deadline, started + m['execution_seconds']))
        additions = {} if m['transition'] is None else m['transition']['additions']
        roots = _out_existing_roots(m)
        roots += [path for path in additions if os.path.lexists(path) and
                  not any(parent in additions for parent in path.parents)]
        registration = None if m['operation'] is None else m['operation']['registration']
        observed, active = _out_snapshot(m, roots, deadline=deadline, registration=registration)
        live = _out_live_roots(m)
        string_additions = {str(path): kind for path, kind in additions.items()}
        for path in observed:
            _out_checkpoint(m, deadline, phase='live', path=path)
            need(path in string_additions or any(_out_under(path, root) for root in live),
                 'unexpected transition output member')
        for path, kind in string_additions.items():
            _out_checkpoint(m, deadline, phase='live', path=path)
            need(path not in observed or observed[path]['kind'] == kind, 'wrong transition output type')
        total = m['sealed_bytes'] + active
        need(total <= _OUT_TOTAL, 'public active-accounted output aggregate bound')
        m['accounted_peak'] = max(m['accounted_peak'], total)
        stats = _out_stats(m)
        stats['live_max_entries'] = max(stats['live_max_entries'], len(observed))
        stats['live_max_bytes'] = max(stats['live_max_bytes'], active)
        attempt = m.get('current_attempt')
        if attempt is not None:
            attempt.pop('nested_phase', None)
            attempt.pop('nested_path', None)
        return {'active_accounted_bytes': total, 'sealed_bytes': m['sealed_bytes'],
                'full_observed_peak_bytes': m['full_peak'], 'last_full_audit': m['last_audit']}
    finally:
        primary = sys.exc_info()[1]
        try:
            ended = time.monotonic()
            stats = _out_stats(m)
            stats['live_count'] += 1
            stats['live_seconds'] += ended - started
            attempt = m.get('current_attempt')
            if attempt is not None:
                attempt['service']['live_count'] += 1
                attempt['service']['live_seconds'] += ended - started
            stats['live_max_duration'] = max(stats['live_max_duration'], ended - started)
            if previous is not None:
                stats['live_max_gap'] = max(stats['live_max_gap'], ended - previous)
                stats['live_max_start_gap'] = max(stats['live_max_start_gap'], started - previous)
            m['last_live'] = ended
        except BaseException as error:
            m.setdefault('diagnostic_errors', []).append(repr(error)[:512])
            del m['diagnostic_errors'][:-4]
            if primary is None:
                raise
        finally:
            m['sampling_live'] = False


def public_output_tick(invocation, *, deadline=None):
    """Bounded producer checkpoint; no process launch and no recursive ticks."""
    if invocation not in _OUTPUT_MONITORS:
        return None
    m = _out_get(invocation)
    if m['closed']:
        return None
    need(not m['auditing'] and not m.get('sampling_live'), 'recursive public output checkpoint')
    deadline = _out_raw_deadline(m, deadline)
    try:
        additions = {} if m['transition'] is None else m['transition']['additions']
        if time.monotonic() >= m['last_audit'] + _OUT_DUE:
            _out_audit(m, additions=additions, deadline=deadline, reason='scheduled')
        return _out_sample_live(m, deadline=deadline)
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
        if not m['auditing'] and not m.get('sampling_live') and time.monotonic() >= m['last_audit'] + _OUT_DUE:
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
        _out_audit(m, reason=phase + '-enter')
        if phase == 'section-open':
            sections = _out_sections(m)
            need(m['section'] is None and len(m['finished_sections']) < len(sections) and
                 section == sections[len(m['finished_sections'])], 'out-of-order section open')
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
            need(m.get('diagnostic') is None, 'diagnostic policy cannot close full collection')
            need(section is None and m['section'] is None and
                 m['finished_sections'] == list(_OUT_SECTIONS), 'incomplete collection close')
            additions = {m['collection'].parent / 'collection.json': 'file'}
        else:
            need(False, 'unknown finite public output transition')
        need(not any(os.path.lexists(path) for path in additions), 'pre-existing output transition member')
        m['transition'] = {'phase': phase, 'additions': additions}
        if phase == 'collection-close':
            parent = m['collection'].parent
            before = m['sealed'].get(str(parent))
            now = parent.lstat()
            need(before is not None and before['kind'] == 'directory' and
                 stat.S_ISDIR(now.st_mode) and before['stat'] == _out_stat(now),
                 'unchanged collection-close parent at entry')
            m['transition']['collection_parent'] = {
                'path': str(parent), 'before': dict(before),
                'identity': {key: getattr(now, key) for key in
                             ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid')}}
        yield
        actual = _out_audit(m, additions=additions, reason=phase + '-exit')
        need(all(str(path) in actual and actual[str(path)]['kind'] == kind for path, kind in additions.items()),
             'missing or wrong output transition member')
        sealed = {}
        for path, kind in additions.items():
            row = actual[str(path)]
            if kind == 'file':
                row = dict(row, sha256=_out_hash(path, row['stat'], m, _out_raw_deadline(m)))
            sealed[str(path)] = row
        parent = _out_collection_parent(m, actual, {str(path) for path in additions})
        _out_promote(m, sealed)
        if parent is not None:
            # Only this already-verified zero-byte directory is explicitly resealed.
            # Files and all other directories retain the no-reseal policy.
            m['sealed'][parent] = dict(actual[parent])
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
        _out_audit(m, deadline=deadline, reason='failure-preservation')
        result = {'verified': True, 'last_full_audit': m['last_audit']}
    except BaseException as error:
        result = {'verified': False, 'error': repr(error)[:2048]}
    finally:
        m['failure_read'] = False
        m['run']['failed'] = True
    result['first_failed_attempt'] = m.get('first_failed_attempt')
    result['preservation_attempt'] = m.get('last_attempt')
    m['failure_audit'] = result
    return _copy(result)


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
    return _copy(m['failure_audit'])


def _public_output_report(m):
    if m is None:
        return None
    stats = _out_stats(m)
    return {'schema': 'public-output-sampled-ledger-v2',
            'full_observed_peak_bytes': m['full_peak'],
            'active_accounted_peak_bytes': m['accounted_peak'],
            'sealed_bytes': m['sealed_bytes'], 'full_audit_count': stats['successes'],
            'last_full_audit': m['last_audit'], 'max_completed_audit_gap': stats['max_completed_gap'],
            'policy': {'due_seconds': _OUT_DUE, 'max_idle_start_seconds': _OUT_START_LIMIT,
                       'execution_seconds': m['execution_seconds'],
                       'service_due_seconds': _OUT_SERVICE_DUE,
                       'service_latency_is_hard_bound': False,
                       'completion_gap_bound': _OUT_START_LIMIT + m['execution_seconds'],
                       'persistent_metadata_revisit_bound': _OUT_START_LIMIT + 2 * m['execution_seconds']},
            'timing': dict(stats), 'last_success': m.get('last_success'),
            'first_failed_attempt': m.get('first_failed_attempt'),
            'diagnostic_errors': list(m.get('diagnostic_errors', ())),
            'phase_seconds_include_service': True, 'live_seconds_include_fast_service': True,
            'continuous_peak_claim': False, 'body_cache_authority': False,
            'invocation_poisoned': m['run']['failed']}


# A diagnostic selection never changes the production five-section constants.
_PUBLIC_DIAGNOSTIC_SECTIONS = ('lifecycle', 'guards')
_PUBLIC_DIAGNOSTICS = {}
_PUBLIC_DIAGNOSTIC_INVOCATIONS = {}
_PUBLIC_DIAGNOSTIC_TERMINALS = {}


class PublicDiagnosticPolicy:
    __slots__ = ()

    def __new__(cls, *args, **kwargs):
        raise TypeError('diagnostic policy requires the original unused live permit')


class PublicDiagnosticTerminal:
    __slots__ = ()

    def __new__(cls, *args, **kwargs):
        raise TypeError('diagnostic terminal requires authenticated selected closure')


def _diagnostic_quiescent():
    need(not _CHECKED and _LAUNCH is None and _PUBLIC_REPLAY is None and not _PUBLIC_INPUTS,
         'diagnostic transition while a read/launch lease is active')


def issue_public_diagnostic(invocation, runtime_permit):
    """Lock the original unused invocation to the one fixed partial policy."""
    _diagnostic_quiescent()
    inv, captured, run = _run_state(invocation)
    full_verify_invocation(invocation)
    d = _runtime_permit_full(runtime_permit, invocation, 'public')
    need(captured['scope'] == 'combined' and d['invocation_nonce'] == inv['nonce'] and
         Path(d['root']) == run['root'], 'original combined diagnostic invocation required')
    need(invocation not in _OUTPUT_MONITORS and invocation not in _PUBLIC_DIAGNOSTIC_INVOCATIONS and
         not os.path.lexists(run['root'] / 'public-runtime/collection'),
         'full or diagnostic public collection already started')
    need(runtime_permit in _PUBLIC_CONTRACTS and runtime_permit in _PUBLIC_TOOLS,
         'original contracts and runtime tools required')
    need(not any(owner['invocation'] is invocation and
                 probe.document()['operation_kind'] in ('generator', 'public-runtime', 'public-native')
                 for probe, owner in _PROBES.items()), 'public work already started')
    contracts = _PUBLIC_CONTRACTS[runtime_permit]
    policy = object.__new__(PublicDiagnosticPolicy)
    owned = {'invocation': invocation, 'permit': runtime_permit, 'invocation_raw': canonical(inv),
             'permit_raw': canonical(d), 'contracts': contracts,
             'rosters_raw': canonical(contracts['rosters']), 'thread': threading.get_ident(),
             'pid': os.getpid(), 'started': False, 'terminal': None}
    _PUBLIC_DIAGNOSTICS[policy] = owned
    _PUBLIC_DIAGNOSTIC_INVOCATIONS[invocation] = policy
    return policy


def _diagnostic_association(invocation, runtime_permit, policy):
    need(type(policy) is PublicDiagnosticPolicy and policy in _PUBLIC_DIAGNOSTICS,
         'original live diagnostic policy required')
    owned = _PUBLIC_DIAGNOSTICS[policy]
    need(owned['invocation'] is invocation and owned['permit'] is runtime_permit and
         _PUBLIC_DIAGNOSTIC_INVOCATIONS.get(invocation) is policy and
         owned['thread'] == threading.get_ident() and owned['pid'] == os.getpid(),
         'foreign or cross-thread/process diagnostic policy')
    return owned


def _diagnostic_owner(invocation, runtime_permit, policy):
    owned = _diagnostic_association(invocation, runtime_permit, policy)
    inv, captured = full_verify_invocation(invocation)
    d = _runtime_permit_full(runtime_permit, invocation, 'public')
    need(captured['scope'] == 'combined' and canonical(inv) == owned['invocation_raw'] and
         d['invocation_nonce'] == inv['nonce'] and canonical(d) == owned['permit_raw'],
         'diagnostic invocation/nonce/permit drift')
    contracts = _PUBLIC_CONTRACTS.get(runtime_permit)
    need(contracts is owned['contracts'] and canonical(contracts['rosters']) == owned['rosters_raw'],
         'diagnostic authenticated contract roster drift')
    for row in [contracts['helper'], *contracts['pins']]:
        check_deadline(invocation)
        need(identity(row['path']) == row, 'diagnostic contract source closure drift')
    return owned


def _out_sections(m):
    policy = m.get('diagnostic')
    if policy is None:
        return _OUT_SECTIONS
    _diagnostic_owner(m['invocation'], m['permit'], policy)
    return _PUBLIC_DIAGNOSTIC_SECTIONS


def public_diagnostic_start(invocation, runtime_permit, policy):
    _diagnostic_quiescent()
    owned = _diagnostic_owner(invocation, runtime_permit, policy)
    need(not owned['started'] and owned['terminal'] is None, 'diagnostic policy is single-use')
    owned['started'] = True
    try:
        return _public_output_start(invocation, runtime_permit, policy)
    except BaseException:
        _poison(invocation)
        raise


def _out_diagnostic_parent(m, actual, additions):
    """One fixed diagnostic carrier sibling; no production ancestor exception."""
    transition = m['transition']
    if transition is None or transition['phase'] != 'diagnostic-close':
        return None
    policy = m.get('diagnostic')
    need(type(policy) is PublicDiagnosticPolicy and policy in _PUBLIC_DIAGNOSTICS and
         _PUBLIC_DIAGNOSTICS[policy]['invocation'] is m['invocation'] and
         _PUBLIC_DIAGNOSTICS[policy]['permit'] is m['permit'] and
         _PUBLIC_DIAGNOSTICS[policy]['thread'] == threading.get_ident() and
         _PUBLIC_DIAGNOSTICS[policy]['pid'] == os.getpid(), 'original diagnostic close owner')
    parent = m['collection'].parent
    expected = {parent / 'diagnostic-collection.json': 'file'}
    need(transition['additions'] == expected and set(additions) == {str(p) for p in expected},
         'exact diagnostic-close addition authority')
    authority = transition['diagnostic_parent']
    path = str(parent)
    need(authority['path'] == path and m['sealed'].get(path) == authority['before'],
         'original diagnostic-close parent authority')
    now = parent.lstat()
    need(stat.S_ISDIR(now.st_mode) and
         all(getattr(now, key) == value for key, value in authority['identity'].items()),
         'diagnostic-close parent identity/type/mode/owner drift')
    need(path in actual and actual[path]['kind'] == 'directory' and
         actual[path]['stat'] == _out_stat(now), 'diagnostic-close parent changed after snapshot')
    return path


def public_diagnostic_terminal(invocation, runtime_permit, policy, document):
    """Close only the selected-public monitor and publish a nonqualifying carrier."""
    _diagnostic_quiescent()
    owned = _diagnostic_owner(invocation, runtime_permit, policy)
    m = _out_auth(invocation, runtime_permit)
    try:
        need(owned['started'] and owned['terminal'] is None and m.get('diagnostic') is policy,
             'unused diagnostic terminal policy required')
        need(m['case'] is None and m['operation'] is None and m['transition'] is None and
             m['section'] is None and m.get('owned_service') is None and
             not m['auditing'] and not m.get('sampling_live') and not m.get('servicing_fast'),
             'diagnostic terminal while case/child/transition/service active')
        need(m['finished_sections'] == list(_PUBLIC_DIAGNOSTIC_SECTIONS),
             'complete ordered diagnostic sections required')
        expected = {(section, index) for section in _PUBLIC_DIAGNOSTIC_SECTIONS
                    for index, row in enumerate(owned['contracts']['rosters'][section])
                    if row['scope'] == 'execute'}
        need(m['finished_cases'] == expected and m['case_next'] == len(m['case_order']),
             'complete original diagnostic local case closure required')
        need(type(document) is dict and document.get('schema') == 'package-unit4-public-diagnostic-v1' and
             document.get('selected_sections') == list(_PUBLIC_DIAGNOSTIC_SECTIONS) and
             set(document.get('reports', {})) == set(_PUBLIC_DIAGNOSTIC_SECTIONS) and
             document.get('collection_complete') is False and document.get('local_complete') is False and
             document.get('execution_qualified') is False and document.get('global_host_qualification') is False,
             'diagnostic terminal cannot grant full collection/session authority')
        need(document.get('execution_context', {}).get('invocation') == decode(owned['invocation_raw']),
             'diagnostic document original invocation binding')
        entered = _out_audit(m, reason='diagnostic-close-enter')
        need(set(entered) == set(m['sealed']), 'unsealed output at diagnostic terminal')
        parent = m['collection'].parent
        carrier = parent / 'diagnostic-collection.json'
        need(not os.path.lexists(carrier), 'pre-existing diagnostic terminal carrier')
        before, now = m['sealed'].get(str(parent)), parent.lstat()
        need(before is not None and before['kind'] == 'directory' and
             stat.S_ISDIR(now.st_mode) and before['stat'] == _out_stat(now),
             'unchanged diagnostic-close parent at entry')
        additions = {carrier: 'file'}
        m['transition'] = {'phase': 'diagnostic-close', 'additions': additions,
                           'diagnostic_parent': {'path': str(parent), 'before': dict(before),
                           'identity': {key: getattr(now, key) for key in
                                        ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid')}}}
        raw = canonical(document)
        need(len(raw) <= 8 * 1024 ** 2, 'bounded diagnostic terminal carrier')
        with carrier.open('xb') as sink:
            sink.write(raw)
        actual = _out_audit(m, additions=additions, reason='diagnostic-close-exit')
        need(set(actual) == set(m['sealed']) | {str(carrier)},
             'unsealed output during diagnostic terminal')
        need(str(carrier) in actual and actual[str(carrier)]['kind'] == 'file',
             'missing diagnostic terminal carrier')
        row = dict(actual[str(carrier)])
        row['sha256'] = _out_hash(carrier, row['stat'], m, _out_raw_deadline(m))
        need(row['sha256'] == sha(raw), 'diagnostic terminal bytes changed during close')
        verified_parent = _out_diagnostic_parent(m, actual, {str(carrier)})
        _out_promote(m, {str(carrier): row})
        m['sealed'][verified_parent] = dict(actual[verified_parent])
        m['transition'] = None
        _out_audit(m, reason='diagnostic-terminal-sealed')
        terminal = object.__new__(PublicDiagnosticTerminal)
        _PUBLIC_DIAGNOSTIC_TERMINALS[terminal] = {
            'policy': policy, 'raw': raw,
            'carrier': {'path': str(carrier), 'bytes': len(raw), 'sha256': row['sha256']}}
        owned['terminal'] = terminal
        m['closed'] = True
        return terminal
    except BaseException:
        _poison(invocation)
        raise


def verify_public_diagnostic_terminal(invocation, runtime_permit, policy, terminal):
    """After parser writes, reverify only the original terminal authority/carrier."""
    _diagnostic_quiescent()
    owned = _diagnostic_owner(invocation, runtime_permit, policy)
    need(type(terminal) is PublicDiagnosticTerminal and terminal in _PUBLIC_DIAGNOSTIC_TERMINALS and
         owned['terminal'] is terminal, 'original live diagnostic terminal required')
    result = _PUBLIC_DIAGNOSTIC_TERMINALS[terminal]
    m = _out_get(invocation)
    need(result['policy'] is policy and m['closed'] and m.get('diagnostic') is policy and
         m['finished_sections'] == list(_PUBLIC_DIAGNOSTIC_SECTIONS) and
         m['case'] is None and m['operation'] is None and m['transition'] is None and
         m['section'] is None and m.get('owned_service') is None,
         'closed original diagnostic monitor required')
    carrier = m['collection'].parent / 'diagnostic-collection.json'
    need(result['carrier']['path'] == str(carrier) and identity(carrier) == result['carrier'] and
         body(carrier, 8 * 1024 ** 2) == result['raw'], 'diagnostic terminal carrier drift')
    return _copy(result['carrier'])
