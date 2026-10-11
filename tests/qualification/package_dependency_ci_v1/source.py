"""Exact-source job admission for a dormant CI successor.

Process-local objects are issued by the reviewed producer. Environment metadata
and caller-supplied expected HEAD are authority inputs only in that producer.
Boundary documents cannot prove test outcomes or recreate a live owner.
"""
import hashlib,json,os,platform,re,shutil,stat,struct,subprocess,sys,time,types,uuid
from pathlib import Path,PurePosixPath
KEY='_oxid_package_dependency_ci_source_v1'
HERE=Path(__file__).resolve().parent
FILES=frozenset(('source.py','jobs.py','artifacts.py','test_source.py','test_jobs.py','test_artifacts.py','routes.json','inputs.json','package-manifest.json','README.md'))
SOURCE='tests/fixtures/package_dependency_source_v1/package-dependency-source-v1.json'
SOURCE_SHA='f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
BRIDGE='tests/qualification/package_dependency_unit4_source_v1/source.py'
CHECKPOINT={'head':'ce522e393646ce6478848cee31f307cb4b6ad5e8','tree':'8c3f4837e1fc89a6272ebee3552ce617ce584131'}
_TOKEN=object();_CONTEXTS={};_JOINS={};_SIBLINGS={};_ADAPTERS={};_PRODUCTION_OWNERS={};_CURRENT_DEADLINE=None
_OWNER=sys.modules.get(KEY)
INPUTS_SHA256='7cd747b2fd793187d5dc8bb7cfa592a39d03312429c99fb8a6ab89359ec64260'
ROUTES_SHA256='a5ebe4d44326d31a12db587f299fc2a184fb0ecfafed2411f88204a7fa526e53'
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
 p=real(path);s=p.lstat();need(stat.S_ISREG(s.st_mode) and s.st_size<=cap,'bounded regular body');parts=[];size=0
 with p.open('rb') as f:
  while True:
   tick();b=f.read(1024**2)
   if not b:break
   size+=len(b);need(size<=cap,'body grew beyond read cap');parts.append(b)
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
 need({p.name for p in HERE.iterdir()}==FILES,'closed ten-file source package')
 p=decode(read(HERE/'package-manifest.json'));need(set(p)=={'schema','files','adapters','production_owners'} and p['schema']=='oxid-package-dependency-ci-package-v1','CI package schema')
 names=[r['path'] for r in p['files']];need(names==sorted(FILES-{'package-manifest.json'}),'CI package exact roster')
 for row in p['files']:need(dict(identity(HERE/row['path']),path=row['path'])==row,'CI package body drift')
 need(identity(HERE/'source.py')['sha256']==_LOADED_SHA,'loaded source package mismatch');_adapter_rows(p);_production_rows(p);return p
def load_sibling(name):
 need(name in ('jobs','artifacts'),'named CI sibling');owner();package()
 if name in _SIBLINGS:owns_sibling(_SIBLINGS[name]);return _SIBLINGS[name]
 key='_oxid_package_dependency_ci_'+name+'_v1';need(key not in sys.modules,'unissued CI sibling module slot')
 m=types.ModuleType(key);m.__file__=str(HERE/(name+'.py'));m._SOURCE_OWNER=owner();sys.modules[key]=m;_SIBLINGS[name]=m
 try:exec(compile(read(m.__file__),m.__file__,'exec'),m.__dict__)
 except BaseException:
  del _SIBLINGS[name]
  if sys.modules.get(key) is m:del sys.modules[key]
  raise
 return m

def _git(repo,*args):
 tick();route=shutil.which('git');need(route is not None,'Git required');path=real(Path(route).resolve());before=identity(path)
 env={'PATH':str(path.parent)+os.pathsep+os.defpath,'HOME':'/nonexistent/ci-admission','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':os.devnull,'GIT_NO_REPLACE_OBJECTS':'1','GIT_TERMINAL_PROMPT':'0'}
 if os.name=='nt':
  need(os.environ.get('SYSTEMROOT'),'Windows system environment')
  for k in ('SYSTEMROOT','WINDIR'):
   if k in os.environ:env[k]=os.environ[k]
 command=[str(path),'--no-optional-locks','-c','core.hooksPath='+os.devnull,'-c','core.fsmonitor=false','-C',str(repo),*args]
 need(_units(command)<=16000,'read-only Git command exceeds fixed UTF-16 budget')
 r=subprocess.run(command,env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
 need(identity(path)==before and r.returncode==0 and not r.stderr and len(r.stdout)<=4*1024**2,'bounded read-only Git failed');tick();return r.stdout

def inputs(repo):
 raw=read(HERE/'inputs.json');need(type(INPUTS_SHA256) is str and sha(raw)==INPUTS_SHA256,'fixed CI inputs inventory')
 value=decode(raw);need(value['schema']=='oxid-package-dependency-ci-inputs-v1','CI inputs schema');names=[r['path'] for r in value['files']]
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


def _command(repo,*args):
 route=shutil.which('git');need(route is not None,'Git required');path=real(Path(route).resolve())
 return [str(path),'--no-optional-locks','-c','core.hooksPath='+os.devnull,'-c','core.fsmonitor=false','-C',str(repo),*args]
def _units(command):return len(subprocess.list2cmdline(command).encode('utf-16-le'))//2+1
def _batches(repo,head,names):
 names=sorted(set(names));out=[];batch=[]
 for name in names:
  safe_relative(name)
  if _units(_command(repo,'ls-tree','-rz',head,'--',*(batch+[name])))>16000:
   need(batch,'single Git path exceeds fixed command budget');out.append(batch);batch=[]
  need(_units(_command(repo,'ls-tree','-rz',head,'--',*(batch+[name])))<=16000,'single Git path exceeds fixed command budget');batch.append(name)
 if batch:out.append(batch)
 return out

def _committed(repo,head,rows):
 need(_git(repo,'rev-parse','HEAD').decode().strip()==head,'actual checkout HEAD');found={};names=sorted({r['path'] for r in rows})
 for batch in _batches(repo,head,names):
  raw=_git(repo,'ls-tree','-rz',head,'--',*batch)
  for entry in raw.split(b'\0'):
   if not entry:continue
   prefix,name=entry.split(b'\t',1);mode,kind,oid=prefix.decode().split();name=name.decode();need(name not in found,'duplicate committed input');found[name]=(mode,kind,oid)
 need(set(found)==set(names),'exact committed input membership')
 for row in rows:
  name=safe_relative(row['path']);need(found[name][0] in ('100644','100755') and found[name][1]=='blob','untracked/special committed input')
  raw=read(repo/name);need(hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest()==found[name][2],'dirty committed body '+name)
 tree=_git(repo,'rev-parse','HEAD^{tree}').decode().strip();need(_git(repo,'rev-parse','HEAD').decode().strip()==head,'HEAD drift during commit admission');return tree

def _registry():
 raw=read(HERE/'routes.json');need(sha(raw)==ROUTES_SHA256,'fixed route registry drift');value=decode(raw)
 need(set(value)=={'schema','activation','job_recipes','build_recipes','projections','original_entries','assignments','join_job'} and value['schema']=='oxid-package-dependency-ci-routes-v1','closed route registry')
 ids=[r['id'] for r in value['original_entries']];need(len(ids)==len(set(ids))==173 and set(ids)==set(value['assignments']),'complete original route union')
 for key,recipe in value['job_recipes'].items():
  need(recipe['id']==key and recipe['operational'] is False,'dormant fixed job recipe')
  need([s['number'] for s in recipe['steps']]==list(range(1,len(recipe['steps'])+1)),'ordered unchanged workflow steps')
 return value

def _host():
 need(struct.calcsize('P')==8,'64-bit host required');system=platform.system();machine=platform.machine().lower()
 name={('Linux','x86_64'):'linux-x86_64',('Windows','amd64'):'windows-x86_64',('Darwin','x86_64'):'macos-x86_64',('Darwin','arm64'):'macos-aarch64'}.get((system,machine))
 need(name is not None,'unsupported host');return name

def _event_commit(event,env,head,event_sha):
 if env['GITHUB_EVENT_NAME']=='pull_request':need(event.get('pull_request',{}).get('head',{}).get('sha')==head and event['pull_request'].get('merge_commit_sha')==event_sha,'PR exact head versus merge event')
 else:
  need(head==event_sha,'push/dispatch exact event head')
  if env['GITHUB_EVENT_NAME']=='push':need(event.get('after')==event_sha,'push payload exact commit')
  if 'ref' in event:need(type(env['GITHUB_REF']) is str and event['ref']==env['GITHUB_REF'],'event ref association')
  if event.get('head_commit') is not None:need(type(event['head_commit']) is dict and event['head_commit'].get('id')==event_sha,'event head commit association')

def _metadata(repo,head,event_sha,recipe,registry):
 keys=('GITHUB_ACTIONS','GITHUB_REPOSITORY','GITHUB_RUN_ID','GITHUB_RUN_ATTEMPT','GITHUB_WORKFLOW','GITHUB_EVENT_NAME','GITHUB_JOB','GITHUB_SHA','GITHUB_EVENT_PATH','GITHUB_WORKSPACE','GITHUB_WORKFLOW_REF','GITHUB_WORKFLOW_SHA','GITHUB_REF','RUNNER_TEMP','PACKAGE_CI_MATRIX_JSON')
 env={k:os.environ.get(k) for k in keys};need(env['GITHUB_ACTIONS']=='true' and env['GITHUB_REPOSITORY']=='YanagiKH/Oxid','actual fixed CI repository')
 for k in ('GITHUB_RUN_ID','GITHUB_RUN_ATTEMPT'):need(type(env[k]) is str and re.fullmatch('[1-9][0-9]*',env[k]),'positive CI run identity')
 for k in ('GITHUB_SHA','GITHUB_WORKFLOW_SHA'):need(type(env[k]) is str and re.fullmatch('[0-9a-f]{40}',env[k]),'CI commit identity')
 need(env['GITHUB_SHA']==event_sha and env['GITHUB_EVENT_NAME'] in ('push','pull_request','workflow_dispatch'),'exact supported CI event')
 need(real(env['GITHUB_WORKSPACE'])==repo,'CI workspace mismatch');need(env['GITHUB_JOB']==(recipe['github_job'] if recipe else registry['join_job']),'fixed job association')
 matrix_raw=env['PACKAGE_CI_MATRIX_JSON'];need(type(matrix_raw) is str and len(matrix_raw.encode())<=4096,'explicit bounded matrix projection')
 matrix=decode(matrix_raw.encode());need(matrix==(recipe['matrix'] if recipe else {}),'exact matrix cell')
 need(env['GITHUB_WORKFLOW']=='Repository CI','fixed workflow name')
 need(type(env['GITHUB_WORKFLOW_REF']) is str and env['GITHUB_WORKFLOW_REF'].startswith('YanagiKH/Oxid/.github/workflows/ci.yml@'),'fixed workflow reference')
 event_path=real(env['GITHUB_EVENT_PATH']);event=decode(read(event_path,4*1024**2));need(event.get('repository',{}).get('full_name')=='YanagiKH/Oxid','event repository')
 _event_commit(event,env,head,event_sha)
 need(_git(repo,'show',env['GITHUB_WORKFLOW_SHA']+':.github/workflows/ci.yml')==read(repo/'.github/workflows/ci.yml'),'actual workflow body mismatch')
 ci={short:env[long] for short,long in [('repository','GITHUB_REPOSITORY'),('run_id','GITHUB_RUN_ID'),('run_attempt','GITHUB_RUN_ATTEMPT'),('workflow','GITHUB_WORKFLOW'),('event_name','GITHUB_EVENT_NAME'),('job','GITHUB_JOB'),('workflow_ref','GITHUB_WORKFLOW_REF'),('workflow_sha','GITHUB_WORKFLOW_SHA')]}
 return ci,env,identity(event_path)

class _Record:
 __slots__=()
 def __new__(cls,token=None):need(token is _TOKEN,'source contexts are producer-issued only');return object.__new__(cls)
 def document(self):
  registry=_CONTEXTS if type(self) is JobSourceContext else _JOINS
  need(self in registry and not registry[self].get('invalidated'),'unissued or invalidated source context');return decode(registry[self]['document'])
class JobSourceContext(_Record):__slots__=()
class JoinSourceContext(_Record):__slots__=()

def _snapshot(repo,head,event_sha,recipe_id):
 owner();pkg=package();need(not sys.flags.optimize and sys.dont_write_bytecode,'normal bytecode-disabled source admission');repo=real(repo)
 for v in (head,event_sha):need(type(v) is str and re.fullmatch('[0-9a-f]{40}',v),'exact commit identity')
 inventory=inputs(repo);routes=_registry();recipe=None if recipe_id is None else routes['job_recipes'].get(recipe_id);need(recipe_id is None or recipe is not None,'unknown job/matrix recipe')
 # Both bridge and every transitive checkout helper are authenticated first.
 bridge_raw=read(repo/BRIDGE);need(any(r['path']==BRIDGE and r['sha256']==sha(bridge_raw) for r in inventory['files']),'fixed source bridge identity')
 bridge=types.ModuleType('authenticated_package_ci_source_bridge');bridge.__file__=str(repo/BRIDGE);exec(compile(bridge_raw,bridge.__file__,'exec'),bridge.__dict__)
 captured=bridge.capture(repo);tick();need(captured['source_checkpoint']==CHECKPOINT,'source checkpoint mismatch')
 source=captured['current_source'];need(sha(captured['current_manifest'])==SOURCE_SHA and source['files']==routes['projections']['current-all376']['files'],'fixed complete current source')
 own=[dict(identity(p),path=p.relative_to(repo).as_posix()) for p in sorted(HERE.iterdir())]
 adapter_rows=[dict(row,path=path.relative_to(repo).as_posix()) for path,row in _adapter_rows(pkg)+_production_rows(pkg)];rows={r['path']:r for r in inventory['files']+source['files']+own+adapter_rows};tree=_committed(repo,head,list(rows.values()))
 ci,env,event=_metadata(repo,head,event_sha,recipe,routes);temp=real(env['RUNNER_TEMP']);need(temp.is_dir(),'actual runner temporary directory');ts=temp.stat();runner_temp={'path':str(temp),'identity':{name:getattr(ts,'st_'+name,None) for name in ('dev','ino','mode','uid','gid')}};host=_host();need(recipe is None or host==recipe['host'],'fixed matrix host mismatch')
 need(_git(repo,'rev-parse','HEAD').decode().strip()==head,'terminal HEAD drift')
 doc={'repository_path':str(repo),'checkout_head':head,'checkout_tree':tree,'event_sha':event_sha,'source_checkpoint':CHECKPOINT,'source_manifest_identity':identity(repo/SOURCE),'source_files':source['files'],'workflow_identity':identity(repo/'.github/workflows/ci.yml'),'input_inventory_identity':identity(HERE/'inputs.json'),'route_registry_identity':identity(HERE/'routes.json'),'package_identity':identity(HERE/'package-manifest.json'),'ci':ci,'host':host,'runner_temp':runner_temp,'activation':routes['activation'],'status':'NotReady'}
 if recipe:doc.update(job_recipe_id=recipe_id,matrix=recipe['matrix'])
 else:doc['required_recipes']=sorted(routes['job_recipes'])
 return doc,{'environment':env,'event':event,'adapter_rows':adapter_rows}

def _bounded_snapshot(repo,head,event_sha,recipe):
 global _CURRENT_DEADLINE
 prior=_CURRENT_DEADLINE;_CURRENT_DEADLINE=min(prior,time.monotonic()+120) if prior else time.monotonic()+120
 try:result=_snapshot(Path(repo),head,event_sha,recipe);tick();return result
 finally:_CURRENT_DEADLINE=prior

def _capture(repository,head,event_sha,recipe,join):
 start=time.time_ns();snap,extra=_bounded_snapshot(repository,head,event_sha,recipe)
 doc=dict(snap,schema='oxid-package-ci-join-source-v1' if join else 'oxid-package-ci-job-source-v1',capture_started_ns=start,capture_finished_ns=time.time_ns(),nonce=uuid.uuid4().hex)
 cls=JoinSourceContext if join else JobSourceContext;record=cls(_TOKEN);registry=_JOINS if join else _CONTEXTS;registry[record]={'document':canonical(doc),'snapshot':canonical([snap,extra])};return record

def capture_job(repository,*,expected_head,event_sha,recipe_id):
 need(type(recipe_id) is str,'fixed recipe ID');return _capture(repository,expected_head,event_sha,recipe_id,False)
def capture_expected_join(repository,*,expected_head,event_sha):return _capture(repository,expected_head,event_sha,None,True)
def _verify(context,cls,registry):
 need(type(context) is cls and context in registry,'original source context required')
 try:
  doc=context.document();snap,extra=_bounded_snapshot(doc['repository_path'],doc['checkout_head'],doc['event_sha'],doc.get('job_recipe_id'))
  need(canonical([snap,extra])==registry[context]['snapshot'],'source/CI identity drift');return doc
 except BaseException:
  registry[context]['invalidated']=True;raise
def verify_job(context):return _verify(context,JobSourceContext,_CONTEXTS)
def verify_join(context):return _verify(context,JoinSourceContext,_JOINS)
def registry(context):
 (verify_job if type(context) is JobSourceContext else verify_join)(context);return _registry()
def projection(context,fixed_projection_id):
 doc=verify_job(context);routes=_registry();need(fixed_projection_id in routes['projections'],'unknown source projection')
 value=routes['projections'][fixed_projection_id];return {'id':fixed_projection_id,'files':value['files'],'manifest_identity':doc['source_manifest_identity'],'source_checkpoint':doc['source_checkpoint']}
def route(context,original_id):
 doc=verify_job(context);routes=_registry();need(original_id in routes['assignments'] and doc['job_recipe_id'] in routes['assignments'][original_id]['recipe_ids'],'route outside fixed job/matrix')
 return next(r for r in routes['original_entries'] if r['id']==original_id)
def require_operational(context,recipe=None):
 doc=(verify_job if type(context) is JobSourceContext else verify_join)(context);routes=_registry();a=routes['activation']
 need(a['approved'] is True and a['workflow_sha256']==doc['workflow_identity']['sha256'],'source-only activation remains blocked')
 if recipe is not None:need(recipe.get('operational') is True,'fixed recipe remains blocked')
 return a


_ADAPTER_DIRECTORY='package_dependency_bounded_current_v1'
_ADAPTER_KEY='_oxid_package_dependency_ci_bounded_adapter_v1'
_ADAPTER_FILES=frozenset(('bounded.py','recipes.json','test_bounded.py','package-manifest.json'))
def _adapter_rows(pkg):
 need(type(pkg.get('adapters')) is dict and set(pkg['adapters'])=={'bounded'},'exact named adapter registry')
 spec=pkg['adapters']['bounded'];need(set(spec)=={'directory','manifest'} and spec['directory']==_ADAPTER_DIRECTORY,'fixed adapter directory')
 root=real(HERE.parent/_ADAPTER_DIRECTORY);need(root.is_dir() and {p.name for p in root.iterdir()}==_ADAPTER_FILES,'closed adapter member/directory roster')
 raw=read(root/'package-manifest.json');need({'bytes':len(raw),'sha256':sha(raw)}==spec['manifest'],'adapter manifest body drift')
 manifest=decode(raw);need(set(manifest)=={'schema','files','source_dependency_sha256','operational_release'} and manifest['schema']=='oxid-package-dependency-bounded-current-package-v1','adapter manifest schema')
 need(manifest['operational_release'] is None and manifest['source_dependency_sha256']==_LOADED_SHA,'adapter source dependency or release mismatch')
 need(type(manifest['files']) is dict and set(manifest['files'])==_ADAPTER_FILES-{'package-manifest.json'},'exact adapter body roster')
 rows=[]
 for name in sorted(_ADAPTER_FILES):
  path=root/name;row=identity(path)
  if name!='package-manifest.json':need({'bytes':row['bytes'],'sha256':row['sha256']}==manifest['files'][name],'adapter body drift '+name)
  rows.append((path,row))
 return rows

def owns_adapter(module,context):
 need(type(context) is JobSourceContext and context in _CONTEXTS,'original adapter source context required')
 need(type(module) is types.ModuleType and module in _ADAPTERS,'unissued named adapter owner')
 state=_ADAPTERS[module];need(context in state['contexts'],'adapter not issued to this source context')
 need(sys.modules.get(_ADAPTER_KEY) is module and module.__dict__.get('_SOURCE_OWNER') is owner() and Path(module.__file__)==HERE.parent/_ADAPTER_DIRECTORY/'bounded.py','canonical adapter slot drift')
 verify_job(context);rows=_adapter_rows(package());need(canonical([row for _,row in rows])==state['rows'],'post-load adapter dependency drift');return True

def load_adapter(context,name):
 need(name=='bounded','unknown fixed adapter');verify_job(context);pkg=package();rows=_adapter_rows(pkg);encoded=canonical([row for _,row in rows])
 if _ADAPTERS:
  need(len(_ADAPTERS)==1,'one canonical adapter');module=next(iter(_ADAPTERS));state=_ADAPTERS[module]
  need(sys.modules.get(_ADAPTER_KEY) is module and encoded==state['rows'],'canonical adapter changed')
  state['contexts'].add(context)
  try:owns_adapter(module,context)
  except BaseException:state['contexts'].discard(context);raise
  return module
 need(_ADAPTER_KEY not in sys.modules,'preexisting unissued adapter slot')
 path=HERE.parent/_ADAPTER_DIRECTORY/'bounded.py';module=types.ModuleType(_ADAPTER_KEY);module.__file__=str(path);module._SOURCE_OWNER=owner();sys.modules[_ADAPTER_KEY]=module
 try:
  body=read(path);need(identity(path)==next(row for p,row in rows if p==path),'adapter entry body drift')
  exec(compile(body,str(path),'exec'),module.__dict__)
  need(canonical([row for _,row in _adapter_rows(package())])==encoded,'adapter changed during import')
  _ADAPTERS[module]={'contexts':{context},'rows':encoded};owns_adapter(module,context)
 except BaseException:
  _ADAPTERS.pop(module,None)
  if sys.modules.get(_ADAPTER_KEY) is module:del sys.modules[_ADAPTER_KEY]
  raise
 return module


_PRODUCTION_SPECS={
 'process':{'directory':'package_dependency_production_process_v1','key':'_oxid_package_dependency_production_process_v1','entry':'preparation.py','test':'test_preparation.py','schema':'oxid-package-dependency-production-process-v1','baseline':{'path':'tests/qualification/package_dependency_unit4_current/preparation.py','bytes':117696,'sha256':'7c475867d93065b123f5c6f4acd12a7c622efb0c8392c0f7748d0637fa11e6e6'}},
 'machine':{'directory':'package_dependency_production_machine_v1','key':'_oxid_package_dependency_production_machine_v1','entry':'machine.py','test':'test_machine.py','schema':'oxid-package-dependency-production-machine-v1','baseline':{'path':'tests/qualification/package_dependency_unit4_machine_v1/machine.py','bytes':94821,'sha256':'19a6c3bd82e119b501bc1f6910fa96ad8d2b86f50beac4b4bae3b7a8de0a3056'}}}
def _production_rows(pkg):
 need(type(pkg.get('production_owners')) is dict and set(pkg['production_owners'])==set(_PRODUCTION_SPECS),'exact production owner roster')
 rows=[]
 for name,spec in _PRODUCTION_SPECS.items():
  binding=pkg['production_owners'][name];need(set(binding)=={'directory','manifest'} and binding['directory']==spec['directory'],'fixed production owner directory')
  root=real(HERE.parent/spec['directory']);files={spec['entry'],spec['test'],'package-manifest.json'}
  need(root.is_dir() and {p.name for p in root.iterdir()}==files,'closed production package member/directory roster')
  raw=read(root/'package-manifest.json');need({'bytes':len(raw),'sha256':sha(raw)}==binding['manifest'],'production manifest body drift')
  manifest=decode(raw);need(set(manifest)=={'schema','baseline','files'} and manifest['schema']==spec['schema'] and manifest['baseline']==spec['baseline'],'production exact baseline/schema')
  expected=manifest['files'];need(type(expected) is list and [r['path'] for r in expected]==sorted(files-{'package-manifest.json'}),'production ordered body roster')
  for file in sorted(files):
   path=root/file;row=identity(path)
   if file!='package-manifest.json':need(dict(row,path=file)==next(r for r in expected if r['path']==file),'production owner body drift '+file)
   rows.append((path,row))
 return rows

def owns_production_owner(module):
 need(type(module) is types.ModuleType and module in _PRODUCTION_OWNERS.values(),'unissued production owner')
 name=next(name for name,value in _PRODUCTION_OWNERS.items() if value is module);spec=_PRODUCTION_SPECS[name]
 need(sys.modules.get(spec['key']) is module and Path(module.__file__)==HERE.parent/spec['directory']/spec['entry'] and module.__dict__.get('_SOURCE_OWNER') is owner(),'canonical production owner drift')
 package();return True

def load_production_owner(name):
 need(name in _PRODUCTION_SPECS,'unknown fixed production owner');owner();pkg=package();rows=_production_rows(pkg)
 if name in _PRODUCTION_OWNERS:owns_production_owner(_PRODUCTION_OWNERS[name]);return _PRODUCTION_OWNERS[name]
 # Authenticate both complete packages before executing either entrypoint.
 for other,spec in _PRODUCTION_SPECS.items():need(spec['key'] not in sys.modules or (other in _PRODUCTION_OWNERS and sys.modules[spec['key']] is _PRODUCTION_OWNERS[other]),'preexisting unissued production owner slot')
 spec=_PRODUCTION_SPECS[name];path=HERE.parent/spec['directory']/spec['entry'];key=spec['key'];module=types.ModuleType(key);module.__file__=str(path);module._SOURCE_OWNER=owner();sys.modules[key]=module
 try:
  body=read(path);need(identity(path)==next(row for p,row in rows if p==path),'production entry body changed')
  exec(compile(body,str(path),'exec'),module.__dict__)
  need(canonical([r for _,r in _production_rows(package())])==canonical([r for _,r in rows]),'production owners changed during import')
  _PRODUCTION_OWNERS[name]=module;owns_production_owner(module)
 except BaseException:
  _PRODUCTION_OWNERS.pop(name,None)
  if sys.modules.get(key) is module:del sys.modules[key]
  raise
 return module
