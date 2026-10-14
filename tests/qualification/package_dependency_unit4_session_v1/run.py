"""Thin local Unit4 session producer. A reviewed entrypoint pin anchors this file.

It invokes only the closed successor APIs. This is local execution evidence;
CI event provenance, fresh hosted staging and global host qualification are separate.
Import and plan_request do not execute preparation, probes, compilers or targets.
"""
import argparse,hashlib,json,os,re,signal,stat,sys,time,types
from pathlib import Path
HERE=Path(__file__).resolve().parent
REPOSITORY=HERE.parents[2]
DEPENDENCIES_SHA256='c9413a7bbdae334d017a4b7eacd790461dee142addae0f4f0b6778399fcb0a3c'
_OWNERS={}
class Rejected(ValueError):pass
class Cancelled(BaseException):pass
def _failure(report,error):
 report['local_complete']=False
 if report['failure'] is None:report['failure']={'type':type(error).__name__,'message':str(error)[:4096]}

class _Cancellation:
 """Only the executing producer owns signals; finalization is non-interrupting."""
 def __init__(self):self.report=None;self.error=None;self.finalizing=False;self.sealed=False;self.owner=None
 def poison(self):
  if self.owner is not None:
   api,invocation=self.owner
   try:
    if invocation in api._RUNS:api.cancel_invocation(invocation)
   except BaseException as error:
    if self.report is not None:self.report.setdefault('cancellation_poison_error',str(error)[:2048])
 def check(self):
  if self.error is not None:raise self.error
 def __call__(self,signum,frame):
  if self.sealed:return
  if self.error is None:self.error=Cancelled('local Unit4 session cancelled by '+signal.Signals(signum).name)
  if self.report is not None:_failure(self.report,self.error)
  self.poison()
  if not self.finalizing:
   # A second signal must not cut short unwinding the first cancellation.
   self.finalizing=True
   raise self.error

def need(value,message):
 if not value:raise Rejected(message)
def canonical(value):return (json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()
def decode(raw):
 def pairs(items):
  value={}
  for k,v in items:need(k not in value,'duplicate JSON key');value[k]=v
  return value
 return json.loads(raw,object_pairs_hook=pairs,parse_constant=lambda _:need(False,'nonfinite JSON'))
def real(path,exists=True):
 p=Path(path);need(p.is_absolute() and p.resolve(strict=exists)==p and not any(x.is_symlink() for x in (p,*p.parents)),'canonical path required');return p
def body(path,cap=8*1024**2):
 p=real(path);s=p.lstat();need(stat.S_ISREG(s.st_mode) and s.st_size<=cap,'bounded regular input');raw=p.read_bytes();need(len(raw)==s.st_size,'input changed');return raw
def identity(path):
 raw=body(path);return {'path':str(path),'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}
def dependencies():
 need(isinstance(DEPENDENCIES_SHA256,str),'session awaits reviewed dependency seal')
 raw=body(HERE/'dependencies.json');need(hashlib.sha256(raw).hexdigest()==DEPENDENCIES_SHA256,'exact dependency inventory')
 value=decode(raw);need(value['schema']=='oxid-unit4-session-dependencies-v1','dependency schema')
 names=set();roots=set()
 for row in value['files']:
  name=row['path'];p=Path(name);need(not p.is_absolute() and '..' not in p.parts and p.as_posix()==name and name not in names,'closed dependency path');names.add(name)
  need(dict(identity(REPOSITORY/p),path=name)==row,'dependency body changed');roots.add(p.parent)
 for root in roots:
  expected={p for p in names if Path(p).parent==root}
  actual={p.relative_to(REPOSITORY).as_posix() for p in (REPOSITORY/root).iterdir()}
  need(actual==expected,'exact dependency root roster')
 return value

def _load(relative,key):
 inventory=dependencies();rows={r['path']:r for r in inventory['files']};need(relative in rows,'named dependency entrypoint')
 path=REPOSITORY/relative;raw=body(path);existing=sys.modules.get(key)
 if existing is not None:
  need(_OWNERS.get(key) is existing,'unissued session dependency owner');return existing
 module=types.ModuleType(key);module.__file__=str(path);module.__source_sha256__=rows[relative]['sha256'];sys.modules[key]=module
 try:exec(compile(raw,str(path),'exec'),module.__dict__)
 except BaseException:
  if sys.modules.get(key) is module:del sys.modules[key]
  raise
 _OWNERS[key]=module;return module

def apis():
 binding=_load('tests/qualification/package_dependency_unit4_public_v1/binding.py','_oxid_package_dependency_unit4_public_binding_v1')
 preparation=binding.preparation_api();machine=binding.machine_api()
 parser=_load('tests/qualification/package_dependency_unit4_parser_v1/portable.py','_oxid_package_dependency_unit4_parser_session_owner_v1')
 need(parser._preparation_api() is preparation and parser._machine_api() is machine,'one shared producer/machine owner')
 return {'binding':binding,'preparation':preparation,'context':preparation.context_api(),'machine':machine,'parser':parser}

REQUEST_KEYS={'schema','scope','expected_head','event_sha','run_root','original_repo','historical_repo','public','parser'}
def plan_request(request):
 need(type(request) is dict and set(request)==REQUEST_KEYS and request['schema']=='oxid-unit4-local-session-request-v1','closed session request')
 scope=request['scope'];need(scope in ('public','parser','combined'),'exact scope')
 for k in ('expected_head','event_sha'):need(type(request[k]) is str and re.fullmatch('[0-9a-f]{40}',request[k]),'exact local commit identity')
 need(request['expected_head']==request['event_sha'],'local event must identify actual tested head')
 root=real(request['run_root'],False);need(not os.path.lexists(root) and root.parent.is_dir(),'fresh session root')
 need(REPOSITORY!=root and REPOSITORY not in root.parents and root not in REPOSITORY.parents,'external session root')
 wants_public=scope in ('public','combined');wants_parser=scope in ('parser','combined')
 if wants_public:
  p=request['public'];need(type(p) is dict and set(p)=={'contract_root','path_map','amendment_root','llvm_root','library_root','runtime_receipt','existing_user'},'closed public inputs')
  for k in ('contract_root','path_map','amendment_root'):real(p[k])
  for k in ('llvm_root','library_root','runtime_receipt'):
   if p[k] is not None:real(p[k])
  need(type(p['existing_user']) is str and re.fullmatch('[a-z_][a-z0-9_-]{0,31}',p['existing_user']),'existing hosted account name')
 else:need(request['public'] is None,'public inputs forbidden outside scope')
 if wants_parser:
  p=request['parser'];need(type(p) is dict and set(p)=={'contract_root'},'closed parser inputs');real(p['contract_root']);real(request['original_repo']);real(request['historical_repo'])
 else:need(request['parser'] is None and request['original_repo'] is None and request['historical_repo'] is None,'parser inputs forbidden outside scope')
 return {'schema':'oxid-unit4-local-session-plan-v1','scope':scope,'checkout':str(REPOSITORY),'expected_head':request['expected_head'],'event_sha':request['event_sha'],'root':str(root),'roles':(['ordinary','lifecycle'] if wants_public else [])+(['parser_observed','parser_control'] if wants_parser else []),'status':'NotReady','execution_qualified':False,'ci_runtime_proof':False,'producer_order':'prepare-machine-all-builds-runtime-final-readback','shared_total_seconds':36000}

def verify_committed(api,request,inventory):
 """Bind every producer/dependency body to the actual local commit, shallow-safe."""
 git=api['context']._git;head=request['expected_head'];need(git(REPOSITORY,'rev-parse','HEAD')==head,'actual integration HEAD changed')
 names=[r['path'] for r in inventory['files']]
 names.extend((HERE/name).relative_to(REPOSITORY).as_posix() for name in ('run.py','test_run.py','dependencies.json','package-manifest.json'))
 need(len(set(names))==len(names),'unique committed input roster')
 raw=git(REPOSITORY,'ls-tree','-rz',head,'--',*sorted(names));found={}
 for entry in raw.split('\0'):
  if not entry:continue
  prefix,name=entry.split('\t',1);mode,kind,digest=prefix.split(' ')
  need(mode in ('100644','100755') and kind=='blob' and name not in found,'regular committed input')
  found[name]=digest
 need(set(found)==set(names),'untracked or missing producer/consumer input')
 for name,digest in found.items():
  data=body(REPOSITORY/name);actual=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()
  need(actual==digest,'dirty committed producer/consumer input')
 need(git(REPOSITORY,'rev-parse','HEAD')==head,'integration HEAD changed during readback')
 return {'head':head,'files':len(found),'working_bodies_equal_commit':True}

def produce(request):
 cancellation=_Cancellation();previous={}
 try:
  for sig in (signal.SIGINT,signal.SIGTERM):
   previous[sig]=signal.getsignal(sig);signal.signal(sig,cancellation)
  return _produce(request,cancellation)
 finally:
  cancellation.sealed=True
  for sig,handler in previous.items():signal.signal(sig,handler)

def _produce(request,cancellation):
 need(sys.dont_write_bytecode and not sys.flags.optimize,'normal bytecode-disabled Python required')
 own_before=own_package()
 plan=plan_request(request);before=dependencies();api=apis();root=Path(plan['root']);report={'schema':'oxid-unit4-local-session-result-v1','plan':plan,'request':request,'request_sha256':hashlib.sha256(canonical(request)).hexdigest(),'entrypoint':identity(Path(__file__).resolve()),'dependency_inventory':identity(HERE/'dependencies.json'),'started_ns':time.time_ns(),'stages':[],'status':'NotReady','execution_qualified':False,'ci_runtime_proof':False,'local_complete':False,'failure':None,'preservation':{},'terminal_receipt':None}
 invocation=context=machine=captured=None;public_builds={};parser_builds={};public_result=parser_result=parser_handle=None;public_joined=False
 def step(name,fn):
  cancellation.check()
  if invocation is not None:api['preparation'].check_deadline(invocation)
  row={'name':name,'started_ns':time.time_ns(),'completed':False};report['stages'].append(row)
  result=fn();cancellation.check();row.update(completed=True,completed_ns=time.time_ns());return result
 try:
  cancellation.report=report
  report['committed_inputs_before']=step('verify-committed-inputs',lambda:verify_committed(api,request,before))
  captured=step('capture-current-checkout',lambda:api['context'].capture_source(REPOSITORY,request['expected_head'],request['event_sha'],scope=request['scope']))
  invocation=api['context'].begin_invocation(captured,root);report['invocation']=invocation.document();cancellation.owner=(api['preparation'],invocation)
  context=step('prepare-source',lambda:api['preparation'].produce(invocation,request['scope'],original_repo=request['original_repo'],historical_repo=request['historical_repo']))
  report['source_context']=context.document()
  machine=step('capture-machine',lambda:api['machine'].capture_machine(invocation))
  report['machine_sha256']=hashlib.sha256(canonical(machine.document())).hexdigest()
  if request['public'] is not None:
   build=api['binding'].build_api()
   for role in ('ordinary','lifecycle'):
    public_builds[role]=step('build-public-'+role,lambda role=role:build.build(context,role,root/'targets'/('public-'+role),root/'public-build'/role,machine))
  if request['parser'] is not None:
   parser=api['parser'];authority=step('admit-parser-source',lambda:parser.authority(context));session=step('prepare-parser-layout',lambda:parser.prepare(authority))
   for control in (False,True):
    for profile in ('debug','release'):
     parser_builds[profile,control]=step('build-parser-'+('control-' if control else 'observed-')+profile,lambda profile=profile,control=control:parser.build(session['path'],profile,authority,machine,control=control))
  if request['public'] is not None:
   p=request['public'];runtime=api['binding'].runtime_api()
   tools=step('capture-public-runtime-tools',lambda:api['machine'].capture_runtime_tools(machine,llvm_root=p['llvm_root'],library_root=p['library_root'],runtime_receipt=p['runtime_receipt']))
   admission=step('admit-public-runtime',lambda:runtime.admit_runtime(public_builds['ordinary'],public_builds['lifecycle'],contract_root=p['contract_root'],path_map=p['path_map'],amendment_root=p['amendment_root'],runtime_tools=tools))
   public_result=step('collect-complete-public',lambda:runtime.collect(admission));step('verify-public-collection',lambda:runtime.verify_collection(public_result))
   if public_result.document()['mandatory_hosted_join_required']:
    need(sys.platform.startswith('linux') and os.geteuid()==0,'mandatory Linux capability transport unavailable')
    hosted=api['binding'].hosted_api();transport=step('prepare-hosted-transport',lambda:hosted.prepare_transport(admission,existing_user=p['existing_user']))
    shard=step('execute-hosted-capability',lambda:hosted.execute_transport(transport));public_result=step('join-public-hosted',lambda:runtime.join_hosted(public_result,shard));public_joined=True;step('verify-public-join',lambda:runtime.verify_join(public_result))
   report['public']=public_result.document();need(report['public']['status']=='COMPLETE_LOCAL_OBSERVATIONS','complete local public coverage required')
  if request['parser'] is not None:
   inputs=step('capture-parser-runtime-inputs',lambda:api['machine'].capture_parser_runtime(machine,invocation))
   parser_handle=step('admit-parser-runtime',lambda:parser.create_runtime(authority,list(parser_builds.values()),request['parser']['contract_root'],inputs))
   step('complete-parser-runtime',lambda:parser.complete_runtime(parser_handle));parser_result=step('verify-parser-runtime',lambda:parser.verify_runtime(parser_handle));report['parser']=parser_result
  if public_result is not None:step('readmit-final-public',lambda:runtime.verify_join(public_result) if public_joined else runtime.verify_collection(public_result))
  if parser_handle is not None:report['parser']=step('readmit-final-parser',lambda:parser.verify_runtime(parser_handle))
  report['committed_inputs_after']=step('readmit-committed-inputs',lambda:verify_committed(api,request,before))
  step('verify-final-source',lambda:api['context'].verify_context(context));step('verify-final-machine',lambda:api['machine'].verify_machine(machine,invocation));need(dependencies()==before and own_package()==own_before,'consumer/producer code changed')
  report['local_complete']=report['failure'] is None
 except BaseException as error:
  _failure(report,error)
 finally:
  cancellation.finalizing=True
  if cancellation.error is not None:cancellation.poison()
  for name,fn in [('committed_inputs',lambda:verify_committed(api,request,before)),('consumer_code',lambda:need(dependencies()==before and own_package()==own_before,'consumer/producer code changed')),('source',lambda:api['context'].verify_context(context)),('machine',lambda:api['machine'].verify_machine(machine,invocation))]:
   if name=='source' and context is None or name=='machine' and machine is None:report['preservation'][name]='not-created';continue
   try:
    if invocation is not None:api['preparation'].check_deadline(invocation)
    fn();report['preservation'][name]='verified'
   except BaseException as error:report['preservation'][name]={'error':str(error)[:4096]};report['local_complete']=False
  report['completed_ns']=time.time_ns()
  # Fix the terminal outcome after all preservation checks. Late/repeated signals
  # cannot interrupt serialization or leave a partially written cancellation row.
  cancellation.sealed=True
  owner=api['preparation']._RUNS.get(invocation) if invocation is not None else None
  if owner is not None and owner['root']==root:
   try:
    report['terminal_receipt']=str(root/'unit4-local-session.json');raw=canonical(report);need(len(raw)<=8*1024**2,'bounded terminal session receipt');real(root)
    with (root/'unit4-local-session.json').open('xb') as stream:stream.write(raw)
   except BaseException as error:
    report['local_complete']=False;report['terminal_receipt']=None;report['terminal_capture_error']=str(error)[:2048]
    print(json.dumps({'failure':report['failure'],'terminal_capture_error':report['terminal_capture_error']},sort_keys=True),file=sys.stderr)
 need(report['failure'] is None and report['local_complete'],'local Unit4 session failed: '+str(report['failure'])+'; terminal receipt='+str(report['terminal_receipt']))
 return report

def own_package():
 need({p.name for p in HERE.iterdir()}=={'run.py','test_run.py','dependencies.json','package-manifest.json'},'closed session producer package')
 manifest=decode(body(HERE/'package-manifest.json'));need(manifest['schema']=='oxid-unit4-session-package-v1','session package schema')
 need([r['path'] for r in manifest['files']]==['dependencies.json','run.py','test_run.py'],'session helper roster')
 for row in manifest['files']:need(dict(identity(HERE/row['path']),path=row['path'])==row,'session body changed')
 need(identity(Path(__file__).resolve())==_LOADED_ENTRY,'loaded entrypoint changed')
 return manifest

_LOADED_ENTRY=identity(Path(__file__).resolve())

def main(argv=None):
 parser=argparse.ArgumentParser();parser.add_argument('--request',required=True);parser.add_argument('--execute',action='store_true');args=parser.parse_args(argv)
 need(sys.dont_write_bytecode and not sys.flags.optimize,'normal bytecode-disabled Python required')
 request=decode(body(real(args.request)))
 result=produce(request) if args.execute else plan_request(request)
 print(json.dumps(result,sort_keys=True))
if __name__=='__main__':main()
