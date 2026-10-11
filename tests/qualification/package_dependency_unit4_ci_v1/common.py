"""Current-source CI admission. Live registry ownership is process-local only.

The reviewed executing workflow/entrypoint supplies the trust anchor. GitHub
metadata is captured from that process; these records are not signed attestations.
"""
import hashlib,json,os,platform,re,shutil,stat,struct,subprocess,sys,time,types,uuid
from pathlib import Path,PurePosixPath
KEY='_oxid_package_dependency_unit4_ci_common_v1'
HERE=Path(__file__).resolve().parent
INPUTS_SHA256='e4dfb7b1d52f251ceaf61ddd439a625697a3e4ce16bb54172cfd8544113d6a34'
FILES=frozenset(('common.py','gate.py','staging.py','evidence.py','join.py','facade.py','test_integration.py','test_facades.py','inputs.json','consumer-dispositions.json','package-manifest.json','README.md'))
DOMAINS=('linux-public','linux-parser','macos-x86_64-public','macos-arm64-public','windows-x86_64-public')
JOBS={'linux-public':'unit4-linux','linux-parser':'unit4-parser','macos-x86_64-public':'unit4-portable-hosts','macos-arm64-public':'unit4-portable-hosts','windows-x86_64-public':'unit4-portable-hosts'}
JOIN_JOB='unit4-qualification'
SOURCE='tests/fixtures/package_dependency_source_v1/package-dependency-source-v1.json'
# Explicit reviewed checkpoint, distinct from every actual execution HEAD.
SOURCE_SHA='f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
CHECKPOINT={'head':'ce522e393646ce6478848cee31f307cb4b6ad5e8','tree':'8c3f4837e1fc89a6272ebee3552ce617ce584131'}
ACTIVATION={'approved':False,'workflow_sha256':None,'job_limit_seconds':21600,'work_seconds':None,'failure_preservation_seconds':None,'reason':'Awaiting final workflow and measured complete-domain timing review'}
ENV_KEYS=('GITHUB_ACTIONS','GITHUB_REPOSITORY','GITHUB_RUN_ID','GITHUB_RUN_ATTEMPT','GITHUB_WORKFLOW','GITHUB_EVENT_NAME','GITHUB_JOB','GITHUB_SHA','GITHUB_EVENT_PATH','GITHUB_WORKSPACE','GITHUB_WORKFLOW_REF','GITHUB_WORKFLOW_SHA','UNIT4_CI_DOMAIN','UNIT4_CI_NEEDS_JSON')
_TOKEN=object();_CONTEXTS={};_JOINS={};_BINDINGS={};_SIBLINGS={};_APIS={};_CURRENT_DEADLINE=None
_OWNER=sys.modules.get(KEY)
class Rejected(ValueError):pass
def need(ok,message):
 if not ok:raise Rejected(message)
def tick():
 if _CURRENT_DEADLINE is not None:need(time.monotonic()<_CURRENT_DEADLINE,'CI admission deadline')
def sha(raw):return hashlib.sha256(raw).hexdigest()
def canonical(value):return (json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()
def decode(raw):
 def pairs(xs):
  d={}
  for k,v in xs:need(k not in d,'duplicate JSON key');d[k]=v
  return d
 return json.loads(raw,object_pairs_hook=pairs,parse_constant=lambda _:need(False,'nonfinite JSON'))
def safe_relative(name):
 need(type(name) is str and name and ':' not in name and PurePosixPath(name).parts and '\\' not in name and str(PurePosixPath(name))==name and not name.startswith('/') and all(p not in ('..','.') for p in PurePosixPath(name).parts),'unsafe relative path');return name
def real(path,exists=True):
 p=Path(path);need(p.is_absolute() and p.resolve(strict=exists)==p and not any(q.is_symlink() for q in (p,*p.parents)),'canonical nonsymlink path');return p
def read(path,cap=64*1024**2):
 p=real(path);s=p.lstat();need(stat.S_ISREG(s.st_mode) and s.st_size<=cap,'bounded regular body');parts=[]
 with p.open('rb') as f:
  while True:
   tick();b=f.read(1024**2)
   if not b:break
   parts.append(b)
 after=p.stat();need(all(getattr(s,k)==getattr(after,k) for k in ('st_dev','st_ino','st_mode','st_size','st_mtime_ns','st_ctime_ns')),'body changed during read');return b''.join(parts)
def identity(path):
 b=read(path);return {'path':str(path),'bytes':len(b),'sha256':sha(b)}
def write_new(path,value):
 p=real(path,False);need(not os.path.lexists(p) and p.parent.is_dir(),'fresh output file');b=canonical(value);need(len(b)<=32*1024**2,'bounded JSON output')
 with p.open('xb') as f:f.write(b)
 return identity(p)
_LOADED_SHA=sha(read(Path(__file__).resolve()))
__source_sha256__=_LOADED_SHA
def owner():
 need(_OWNER is not None and sys.modules.get(KEY) is _OWNER and _OWNER.__dict__ is globals(),'unissued canonical common owner')
 need(identity(Path(__file__).resolve())['sha256']==_LOADED_SHA,'loaded common body drift');return _OWNER
def owns_sibling(module):
 need(module in _SIBLINGS.values(),'unissued CI sibling owner');name=next(k for k,v in _SIBLINGS.items() if v is module)
 need(sys.modules.get(module.__name__) is module and Path(module.__file__)==HERE/(name+'.py'),'CI sibling identity');return True
def package():
 need({p.name for p in HERE.iterdir()}==FILES,'closed twelve-file CI package')
 p=decode(read(HERE/'package-manifest.json'));need(set(p)=={'schema','files'} and p['schema']=='oxid-package-unit4-ci-package-v1','CI package schema')
 names=[r['path'] for r in p['files']];need(names==sorted(FILES-{'package-manifest.json'}),'CI package exact roster')
 for row in p['files']:need(dict(identity(HERE/row['path']),path=row['path'])==row,'CI package body drift')
 need(identity(HERE/'common.py')['sha256']==_LOADED_SHA,'loaded common package mismatch');return p
def load_sibling(name):
 need(name in ('gate','staging','evidence','join','facade'),'named CI sibling');owner();package()
 if name in _SIBLINGS:owns_sibling(_SIBLINGS[name]);return _SIBLINGS[name]
 key='_oxid_package_dependency_unit4_ci_'+name+'_v1';need(key not in sys.modules,'unissued CI sibling module slot')
 m=types.ModuleType(key);m.__file__=str(HERE/(name+'.py'));m._COMMON_OWNER=owner();sys.modules[key]=m;_SIBLINGS[name]=m
 try:exec(compile(read(m.__file__),m.__file__,'exec'),m.__dict__)
 except BaseException:
  del _SIBLINGS[name]
  if sys.modules.get(key) is m:del sys.modules[key]
  raise
 return m

def host_policy(domain):
 need(domain in DOMAINS,'unknown CI domain')
 host={'linux-public':'Linux x86_64','linux-parser':'Linux x86_64','macos-x86_64-public':'macOS x86_64','macos-arm64-public':'macOS arm64','windows-x86_64-public':'Windows x86_64'}[domain]
 public=domain!='linux-parser';return {'domain':domain,'host':host,'scope':'public' if public else 'parser','public_required':public,'parser_required':not public,'root_required':domain=='linux-public','staging_required':domain=='linux-public','hosted_required':domain=='linux-public','profiles':['debug','release'],'public_sections':['public','original','predecessors','lifecycle','guards'] if public else []}
def measured_host():
 name={'Darwin':'macOS'}.get(platform.system(),platform.system())+' '+{'AMD64':'x86_64','aarch64':'arm64'}.get(platform.machine(),platform.machine())
 need(struct.calcsize('P')*8==64 and name in {host_policy(d)['host'] for d in DOMAINS},'supported measured 64-bit host')
 return {'name':name,'system':platform.system(),'machine':platform.machine(),'release':platform.release(),'pointer_width':64,'python':sys.version,'executable':str(real(Path(sys.executable).resolve())),'effective_uid':os.geteuid() if hasattr(os,'geteuid') else None}

def _git(repo,*args):
 tick();route=shutil.which('git');need(route is not None,'Git required');path=real(Path(route).resolve());before=identity(path)
 env={'PATH':str(path.parent)+os.pathsep+os.defpath,'HOME':'/nonexistent/ci-admission','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':os.devnull,'GIT_NO_REPLACE_OBJECTS':'1','GIT_TERMINAL_PROMPT':'0'}
 if os.name=='nt':
  need(os.environ.get('SYSTEMROOT'),'Windows system environment')
  for k in ('SYSTEMROOT','WINDIR'):
   if k in os.environ:env[k]=os.environ[k]
 command=[str(path),'--no-optional-locks','-c','core.hooksPath='+os.devnull,'-c','core.fsmonitor=false','-C',str(repo),*args]
 r=subprocess.run(command,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
 need(identity(path)==before and r.returncode==0 and not r.stderr and len(r.stdout)<=4*1024**2,'bounded read-only Git failed');tick();return r.stdout

def inputs(repo):
 raw=read(HERE/'inputs.json');need(type(INPUTS_SHA256) is str and sha(raw)==INPUTS_SHA256,'fixed CI inputs inventory')
 value=decode(raw);need(value['schema']=='oxid-package-unit4-ci-inputs-v1','CI inputs schema');names=[r['path'] for r in value['files']]
 need(names==sorted(set(names)),'unique ordered CI inputs')
 for row in value['files']:need(dict(identity(repo/safe_relative(row['path'])),path=row['path'])==row,'CI input body drift')
 for name in value['closed_roots']:
  base=real(repo/safe_relative(name));need(base.is_dir(),'closed input directory');wanted={p for p in names if p.startswith(name+'/')};actual=set();dirs={name}
  for p in wanted:
   dirs|={q.as_posix() for q in PurePosixPath(p).parents if q.as_posix().startswith(name)}
  for p in base.rglob('*'):
   tick();s=p.lstat();rel=p.relative_to(repo).as_posix();need(stat.S_ISREG(s.st_mode) or stat.S_ISDIR(s.st_mode),'closed input special member')
   if stat.S_ISREG(s.st_mode):actual.add(rel)
   else:need(rel in dirs,'extra closed input directory')
  need(actual==wanted,'closed input roster drift')
 return value

def _committed(repo,head,rows):
 need(_git(repo,'rev-parse','HEAD').decode().strip()==head,'actual checkout HEAD')
 found={}
 names=sorted({r['path'] for r in rows});batches=[];batch=[];units=0
 for name in names:
  size=len(subprocess.list2cmdline([name]).encode('utf-16-le'))//2+1
  need(size<12000,'bounded Git input name')
  if units+size>12000:batches.append(batch);batch=[];units=0
  batch.append(name);units+=size
 if batch:batches.append(batch)
 for batch in batches:
  raw=_git(repo,'ls-tree','-rz',head,'--',*batch)
  for entry in raw.split(b'\0'):
   if not entry:continue
   prefix,name=entry.split(b'\t',1);mode,kind,oid=prefix.decode().split();need(name.decode() not in found,'duplicate committed input');found[name.decode()]=(mode,kind,oid)
 need(set(found)==set(names),'exact committed input membership')
 for row in rows:
  name=safe_relative(row['path']);need(name in found and found[name][0] in ('100644','100755') and found[name][1]=='blob','untracked/special CI input')
  b=read(repo/name);need(hashlib.sha1(b'blob '+str(len(b)).encode()+b'\0'+b).hexdigest()==found[name][2],'dirty CI committed body')
 return _git(repo,'rev-parse','HEAD^{tree}').decode().strip()

def _metadata(repo,head,event_sha,domain):
 env={k:os.environ.get(k) for k in ENV_KEYS};need(env['GITHUB_ACTIONS']=='true','actual GitHub Actions producer required')
 need(env['GITHUB_REPOSITORY']=='YanagiKH/Oxid','fixed repository')
 for k in ('GITHUB_RUN_ID','GITHUB_RUN_ATTEMPT'):need(type(env[k]) is str and re.fullmatch('[1-9][0-9]*',env[k]),'positive CI run identity')
 for k in ('GITHUB_SHA','GITHUB_WORKFLOW_SHA'):need(type(env[k]) is str and re.fullmatch('[0-9a-f]{40}',env[k]),'CI commit identity')
 need(env['GITHUB_SHA']==event_sha and env['GITHUB_EVENT_NAME'] in ('push','pull_request','workflow_dispatch'),'bound supported CI event')
 need(real(env['GITHUB_WORKSPACE'])==repo,'actual CI workspace')
 need(env['GITHUB_JOB']==(JOIN_JOB if domain is None else JOBS[domain]),'fixed CI domain job')
 need(env['UNIT4_CI_DOMAIN']==('join' if domain is None else domain),'fixed domain declaration')
 if domain is None:
  need(type(env['UNIT4_CI_NEEDS_JSON']) is str and len(env['UNIT4_CI_NEEDS_JSON'].encode())<=4096,'bounded upstream result projection')
  upstream=decode(env['UNIT4_CI_NEEDS_JSON'].encode());need(type(upstream) is dict and set(upstream)==set(JOBS.values()) and all(v in ('success','failure','cancelled','skipped') for v in upstream.values()),'exact upstream CI results')
 else:need(env['UNIT4_CI_NEEDS_JSON'] is None,'upstream join metadata forbidden on domain job');upstream=None
 need(type(env['GITHUB_WORKFLOW']) is str and env['GITHUB_WORKFLOW'],'workflow name')
 need(type(env['GITHUB_WORKFLOW_REF']) is str and env['GITHUB_WORKFLOW_REF'].startswith('YanagiKH/Oxid/.github/workflows/ci.yml@'),'fixed workflow path/ref')
 event_path=real(env['GITHUB_EVENT_PATH']);event_raw=read(event_path,4*1024**2);event=decode(event_raw)
 need(event.get('repository',{}).get('full_name')=='YanagiKH/Oxid','event repository')
 if env['GITHUB_EVENT_NAME']=='pull_request':
  need(event.get('pull_request',{}).get('head',{}).get('sha')==head and event['pull_request'].get('merge_commit_sha')==event_sha,'PR head/merge event distinction')
 else:need(head==event_sha,'push/dispatch exact tested event head')
 # Provisioning must make the exact workflow commit available read-only.
 workflow_raw=_git(repo,'show',env['GITHUB_WORKFLOW_SHA']+':.github/workflows/ci.yml')
 need(workflow_raw==read(repo/'.github/workflows/ci.yml'),'executing workflow differs from tested committed workflow')
 ci={'repository':env['GITHUB_REPOSITORY'],'run_id':env['GITHUB_RUN_ID'],'run_attempt':env['GITHUB_RUN_ATTEMPT'],'workflow':env['GITHUB_WORKFLOW'],'event_name':env['GITHUB_EVENT_NAME'],'job':env['GITHUB_JOB'],'workflow_ref':env['GITHUB_WORKFLOW_REF'],'workflow_sha':env['GITHUB_WORKFLOW_SHA']}
 return ci,env,identity(event_path),upstream

class _Record:
 __slots__=()
 def __new__(cls,token=None):need(token is _TOKEN,'CI records are producer-issued only');return object.__new__(cls)
 def document(self):
  registry=_CONTEXTS if type(self) is CIContext else _JOINS
  need(self in registry,'unissued CI record');return decode(registry[self]['document'])
class CIContext(_Record):__slots__=()
class JoinContext(_Record):__slots__=()

def _snapshot(repo,head,event_sha,domain):
 owner();package();need(not sys.flags.optimize and sys.dont_write_bytecode,'normal bytecode-disabled producer')
 repo=real(repo);need(repo.is_dir(),'repository directory')
 for v in (head,event_sha):need(type(v) is str and re.fullmatch('[0-9a-f]{40}',v),'exact execution commit')
 inventory=inputs(repo);source=decode(read(repo/SOURCE));need(identity(repo/SOURCE)['sha256']==SOURCE_SHA and len(source['files'])==376,'exact current source manifest')
 for row in source['files']:need(dict(identity(repo/safe_relative(row['path'])),path=row['path'])==row,'current selected source drift')
 selected={r['path'] for r in source['files'] if r['path'].startswith(('src/','native/'))};observed=set()
 for sub in ('src','native'):
  base=real(repo/sub);need(base.is_dir(),'real selected source root')
  for path in base.rglob('*'):
   tick();mode=path.lstat().st_mode;need(stat.S_ISREG(mode) or stat.S_ISDIR(mode),'selected source special member')
   if stat.S_ISREG(mode):observed.add(path.relative_to(repo).as_posix())
 need(observed==selected,'selected source membership')
 own=[dict(identity(p),path=p.relative_to(repo).as_posix()) for p in sorted(HERE.iterdir())]
 workflow=dict(identity(repo/'.github/workflows/ci.yml'),path='.github/workflows/ci.yml')
 allrows={r['path']:r for r in inventory['files']+source['files']+own+[workflow]};tree=_committed(repo,head,list(allrows.values()))
 ci,env,event,upstream=_metadata(repo,head,event_sha,domain);host=measured_host()
 if domain is not None:
  policy=host_policy(domain);need(host['name']==policy['host'],'domain measured host mismatch')
  if policy['root_required']:need(host['effective_uid']==0,'Linux root capability branch required')
 need(_git(repo,'rev-parse','HEAD').decode().strip()==head,'HEAD drift during CI admission')
 return {'repository_path':str(repo),'checkout_head':head,'checkout_tree':tree,'event_sha':event_sha,'workflow':identity(repo/'.github/workflows/ci.yml'),'ci':ci,'environment':env,'event_document':event,'upstream_job_results':upstream,'host':host,'package_identity':identity(HERE/'package-manifest.json'),'inputs_identity':identity(HERE/'inputs.json'),'source_manifest_identity':identity(repo/SOURCE),'current_source_checkpoint':CHECKPOINT,'activation':dict(ACTIVATION),'status':'NotReady'}

def _bounded_snapshot(repo,head,event_sha,domain):
 global _CURRENT_DEADLINE
 prior=_CURRENT_DEADLINE;_CURRENT_DEADLINE=min(prior,time.monotonic()+120) if prior else time.monotonic()+120
 try:return _snapshot(repo,head,event_sha,domain)
 finally:_CURRENT_DEADLINE=prior

def admit_ci(repository,*,expected_head,event_sha,domain,fresh_output):
 need(domain in DOMAINS,'complete domain required');started=time.time_ns();snap=_bounded_snapshot(Path(repository),expected_head,event_sha,domain)
 outer=real(fresh_output,False);repo=Path(snap['repository_path']);need(not os.path.lexists(outer) and outer.parent.is_dir() and repo!=outer and repo not in outer.parents and outer not in repo.parents,'fresh external CI root')
 doc=dict(snap,schema='oxid-package-unit4-ci-context-v1',domain=domain,scope=host_policy(domain)['scope'],outer_root=str(outer),producer_root=str(outer/'producer'),ci_nonce=uuid.uuid4().hex,capture_started_ns=started,capture_finished_ns=time.time_ns())
 record=CIContext(_TOKEN);_CONTEXTS[record]={'document':canonical(doc),'snapshot':canonical(snap)};return record

def verify_ci(context):
 need(type(context) is CIContext and context in _CONTEXTS,'original CI context required');doc=context.document();snap=_bounded_snapshot(Path(doc['repository_path']),doc['checkout_head'],doc['event_sha'],doc['domain'])
 need(canonical(snap)==_CONTEXTS[context]['snapshot'],'CI source/host/environment drift');return doc

def expected_join(repository,*,expected_head,event_sha):
 started=time.time_ns();snap=_bounded_snapshot(Path(repository),expected_head,event_sha,None)
 doc=dict(snap,schema='oxid-package-unit4-ci-join-context-v1',required_domains=list(DOMAINS),domain_policies={d:host_policy(d) for d in DOMAINS},capture_started_ns=started,capture_finished_ns=time.time_ns())
 record=JoinContext(_TOKEN);_JOINS[record]={'document':canonical(doc),'snapshot':canonical(snap)};return record

def verify_join_context(context):
 need(type(context) is JoinContext and context in _JOINS,'original independent join context required');doc=context.document();snap=_bounded_snapshot(Path(doc['repository_path']),doc['checkout_head'],doc['event_sha'],None)
 need(canonical(snap)==_JOINS[context]['snapshot'],'join CI source/environment drift');return doc

def require_activation(context):
 doc=verify_ci(context) if type(context) is CIContext else verify_join_context(context);a=doc['activation']
 need(a==ACTIVATION and a['approved'] is True and a['workflow_sha256']==doc['workflow']['sha256'],'CI activation remains unapproved')
 need(a['job_limit_seconds']==21600 and type(a['work_seconds']) is int and type(a['failure_preservation_seconds']) is int and a['work_seconds']>0 and a['failure_preservation_seconds']>0 and a['work_seconds']+a['failure_preservation_seconds']<=21600,'reviewed hosted timing contract')
 dispositions=decode(read(HERE/'consumer-dispositions.json'));need(not dispositions['unresolved_mandatory_ids'],'mandatory consumer dispositions unresolved');return dict(a)

def _record(context):
 need(type(context) in (CIContext,JoinContext),'original CI context type');registry=_CONTEXTS if type(context) is CIContext else _JOINS;need(context in registry,'original CI registry');return context.document()
def apis(context):
 doc=_record(context);repo=Path(doc['repository_path']);inputs(repo);package();owner()
 if repo not in _APIS:
  def load(relative,key):
   path=repo/relative;need(key not in sys.modules,'unissued frozen consumer module slot');m=types.ModuleType(key);m.__file__=str(path);m.__source_sha256__=identity(path)['sha256'];sys.modules[key]=m
   try:exec(compile(read(path),str(path),'exec'),m.__dict__)
   except BaseException:
    if sys.modules.get(key) is m:del sys.modules[key]
    raise
   return m
  binding=load('tests/qualification/package_dependency_unit4_public_v1/binding.py','_oxid_package_dependency_unit4_public_binding_v1');preparation=binding.preparation_api();machine=binding.machine_api();parser=load('tests/qualification/package_dependency_unit4_parser_v1/portable.py','_oxid_package_dependency_unit4_parser_session_owner_v1')
  need(parser._preparation_api() is preparation and parser._machine_api() is machine,'canonical CI consumer owners')
  _APIS[repo]={'binding':binding,'preparation':preparation,'context':preparation.context_api(),'machine':machine,'parser':parser,'build':binding.build_api(),'public_runtime':binding.runtime_api(),'hosted':binding.hosted_api()}
 result=_APIS[repo];binding=result['binding'];parser=result['parser']
 need(sys.modules.get('_oxid_package_dependency_unit4_public_binding_v1') is binding and sys.modules.get('_oxid_package_dependency_unit4_parser_session_owner_v1') is parser,'canonical frozen owner slot drift')
 need(binding.preparation_api() is result['preparation'] and binding.machine_api() is result['machine'] and parser._preparation_api() is result['preparation'] and parser._machine_api() is result['machine'],'canonical frozen producer drift')
 need(binding.build_api() is result['build'] and binding.runtime_api() is result['public_runtime'] and binding.hosted_api() is result['hosted'],'canonical frozen consumer drift')
 return dict(result)

def bind_invocation(context,invocation):
 doc=verify_ci(context);need(context not in _BINDINGS,'CI invocation already bound');api=apis(context);inv,captured=api['context']._invocation(invocation)
 need(inv['run_root']==doc['producer_root'] and doc['capture_finished_ns']<=inv['started_ns']<=time.time_ns(),'fresh CI producer root/window')
 need(captured['checkout']=={'path':doc['repository_path'],'head':doc['checkout_head'],'tree':doc['checkout_tree'],'event_sha':doc['event_sha']} and captured['scope']==doc['scope'] and captured['source_checkpoint']==doc['current_source_checkpoint'] and captured['current_source_manifest']['sha256']==SOURCE_SHA,'CI producer captured source/scope')
 _BINDINGS[context]=invocation;return dict(inv)
def invocation(context):
 need(type(context) is CIContext and context in _CONTEXTS and context in _BINDINGS,'bound original CI invocation required');inv=_BINDINGS[context];doc=context.document();value,_=apis(context)['context']._invocation_association(inv);need(value['run_root']==doc['producer_root'],'CI invocation root drift');return inv
