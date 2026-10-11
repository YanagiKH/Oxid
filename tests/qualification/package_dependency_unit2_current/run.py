"""Fresh package-source Unit2 producer; dormant until separately reviewed routing.

The reviewed caller authenticates this entrypoint and selects the trusted machine
contract digest. A digest supplied by untrusted evidence is not authority.
"""
import argparse,copy,hashlib,json,os,shutil,signal,stat,subprocess,sys,time,types,uuid
from pathlib import Path
HERE=Path(__file__).resolve().parent
TRANSPORT_SHA='6f7781ec832b53e66578440603d15a9ed71f6ed6131d6cda77a927926d4d33a8'
HELPER_PINS={'session.py': 'f9eb5a5d6e2ab830330d540d1d64c71467588bafda723d7851d703dc1b0d8913', 'process.py': 'd09d0c177a5e4d6470698ce988201e2946bcf17b59dfe59f140332e4da989f2e', 'test_session.py': '6999f9f6d7d4e3fb6e753deee4ce78cdbf0d2a55ce66991b6d124645364438bc', 'test_run.py': 'b6f0cb55069b91abd53b99dd346d5df683f0c9d747fed5c4ae3fef961edb48f2'} # This entrypoint is authenticated by the reviewed caller.
STOP=False

def need(value,message):
 if not value:raise ValueError(message)
def stopped(_signum,_frame):
 global STOP
 STOP=True

def canonical(path,directory=True):
 path=Path(path);need(path.is_absolute() and path.resolve(strict=True)==path,'canonical absolute path required')
 need(path.is_dir() if directory else stat.S_ISREG(path.lstat().st_mode),'wrong path type');return path

def fresh(path):
 path=Path(path);canonical(path.parent);need(path.is_absolute() and not path.exists() and not path.is_symlink(),'fresh output required');return path

def load(path,digest,name):
 path=canonical(path,False);body=path.read_bytes();need(hashlib.sha256(body).hexdigest()==digest,'helper identity')
 module=types.ModuleType(name);module.__file__=str(path);exec(compile(body,str(path),'exec'),module.__dict__);return module

def helpers():
 need(set(HELPER_PINS)=={'session.py','process.py','test_session.py','test_run.py'},'unsealed producer helpers')
 need({p.name for p in HERE.iterdir()}==set(HELPER_PINS)|{'run.py'},'closed producer helper roster')
 for name,digest in HELPER_PINS.items():
  path=canonical(HERE/name,False);need(hashlib.sha256(path.read_bytes()).hexdigest()==digest,'changed producer helper')
 return load(HERE/'session.py',HELPER_PINS['session.py'],'package_unit2_session'),load(HERE/'process.py',HELPER_PINS['process.py'],'package_unit2_process')

def tree(root,tick,p):
 root=canonical(root);result=[]
 for path in sorted(root.rglob('*')):
  tick();mode=path.lstat().st_mode;need(stat.S_ISREG(mode) or stat.S_ISDIR(mode),'special tree member')
  if stat.S_ISREG(mode):result.append({'path':path.relative_to(root).as_posix(),'bytes':path.stat().st_size,'sha256':p.digest(path,tick)})
 return result

def checkout_identity(repo,git):
 env={'PATH':'/usr/bin:/bin','HOME':'/nonexistent','LANG':'C','LC_ALL':'C','GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null','GIT_OPTIONAL_LOCKS':'0'}
 values=[]
 for expression in ('HEAD','HEAD^{tree}'):
  result=subprocess.run([git,'-C',str(repo),'rev-parse','--verify',expression],env=env,stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=15)
  need(result.returncode==0 and not result.stderr and len(result.stdout)==41,'Git identity readback');value=result.stdout.decode().strip();need(len(value)==40 and set(value)<=set('0123456789abcdef'),'Git identity syntax');values.append(value)
 return {'head':values[0],'tree':values[1]}

def absent(paths):
 for name in paths:
  path=Path(name);need(path.is_absolute() and not path.exists() and not path.is_symlink(),'unexpected config/tool '+name)

def machine_context(path,expected,q,p,tick):
 """Authenticate externally reviewed finite machine contract; execute no probes."""
 path=canonical(path,False);need(p.digest(path,tick)==expected,'machine contract identity');value=q.strict_json(q.read(path,16*1024**2))
 need(set(value)=={'schema','roles','path','tools','registry_root','registry_files','absent_paths','metadata','host_build_scripts'},'machine contract schema')
 need(value['schema']=='package-unit2-reviewed-machine-v1','machine contract version')
 need(set(value['roles'])=={'cargo','rustc','rustdoc','python','cc','cxx','ar','git'},'complete executable roles')
 need(set(value['tools'])=={'files','routes'},'tool inventory schema');pins={str(path):expected};routes=value['tools']['routes']
 for row in value['tools']['files']:
  name=str(canonical(row['path'],False));need(name not in pins and set(row)=={'path','bytes','sha256'},'unique complete tool row');need(Path(name).stat().st_size==row['bytes'],'tool size');pins[name]=row['sha256']
 for role,route in value['roles'].items():need(route in routes and routes[route] in pins and os.access(route,os.X_OK),'pinned executable role '+role)
 need(value['roles']['python']=='/usr/bin/python3' and value['roles']['git']=='/usr/bin/git','fixed Python/Git roles')
 need(value['path']==str(Path(value['roles']['rustc']).parent)+':/usr/bin:/bin','fixed build search path')
 registry=canonical(value['registry_root']);need(tree(registry,tick,p)==value['registry_files'],'registry inventory')
 for row in value['registry_files']:pins[str(registry/row['path'])]=row['sha256']
 metadata=value['metadata'];need(set(metadata)=={'path','sha256'},'native metadata identity');pins[str(canonical(metadata['path'],False))]=metadata['sha256']
 q.verify_body_pins(pins,routes,time.monotonic()+90);absent(value['absent_paths'])
 for directory in value['path'].split(':'):absent([str(Path(directory)/name) for name in ('emcc','emcc.bat')])
 return value,pins,routes

def verify_dependencies(source,machine,q,p,tick):
 import tarfile,tomllib
 registry=Path(machine['registry_root']);lock=tomllib.loads(q.read(source/'Cargo.lock').decode());scripts=[];count=0
 for package in lock['package']:
  if 'checksum' not in package:continue
  tick();count+=1;name=package['name']+'-'+package['version'];archives=list((registry/'cache').glob('*/'+name+'.crate'));roots=list((registry/'src').glob('*/'+name))
  need(len(archives)==len(roots)==1,'unique locked dependency');archive=archives[0];root=roots[0];need(p.digest(archive,tick)==package['checksum'],'Cargo.lock archive checksum');expected={};size=0
  with tarfile.open(archive,'r:gz') as stream:
   for member in stream:
    tick();parts=Path(member.name).parts;need(parts and parts[0]==name and not Path(member.name).is_absolute() and '..' not in parts,'archive path')
    if member.isdir():continue
    need(member.isfile(),'special dependency archive member');relative=Path(member.name).relative_to(name).as_posix();q.safe_name(relative);need(relative not in expected,'duplicate archive body');size+=member.size;need(size<=256*1024**2 and len(expected)<17000,'dependency archive envelope');body=stream.extractfile(member).read();expected[relative]={'path':relative,'bytes':len(body),'sha256':q.sha(body)}
  actual={row['path']:row for row in tree(root,tick,p)};need(actual.pop('.cargo-ok')=={'path':'.cargo-ok','bytes':7,'sha256':q.sha(b'{"v":1}')},'Cargo marker');need(actual==expected,'unpacked dependency differs from archive')
  declared=tomllib.loads(q.read(root/'Cargo.toml').decode())['package'].get('build','build.rs')
  if declared is not False and (root/declared).is_file():scripts.append({'package':name,'path':str(root/declared),'sha256':p.digest(root/declared,tick)})
 need(count==13 and sorted(scripts,key=lambda row:row['package'])==machine['host_build_scripts'] and {row['package'] for row in scripts}=={'libc-0.2.177','generic-array-0.14.7'},'finite host build script closure')

def forbidden_config_paths(source,home,work):
 paths={str(base/name) for base in (home,work/'home/.cargo',source/'.cargo') for name in ('config','config.toml')}
 paths.update(str(parent/'.cargo'/name) for parent in source.parents for name in ('config','config.toml'))
 paths.update(str(parent/'Cargo.toml') for parent in source.parents)
 return sorted(paths)

def compile_command(machine,profile,source,target,work,cargo_home):
 need(profile in ('debug','release'),'known profile');roles=machine['roles'];env={'HOME':str(work/'home'),'TMPDIR':str(work/'tmp'),'PATH':machine['path'],'CARGO_HOME':str(cargo_home),'CARGO_TARGET_DIR':str(target),'RUSTC':roles['rustc'],'RUSTDOC':roles['rustdoc'],'CARGO_BUILD_JOBS':'1','CARGO_INCREMENTAL':'0','CARGO_NET_OFFLINE':'true','CARGO_TERM_COLOR':'never','RUST_BACKTRACE':'0','LANG':'C','LC_ALL':'C','TZ':'UTC','CC_ENABLE_DEBUG_OUTPUT':'1','CC':roles['cc'],'CXX':roles['cxx'],'AR':roles['ar'],'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER':roles['cc']}
 key='TEST' if profile=='debug' else 'RELEASE'
 for name,value in {'OPT_LEVEL':'0' if profile=='debug' else '3','DEBUG':'2' if profile=='debug' else '0','DEBUG_ASSERTIONS':'true' if profile=='debug' else 'false','OVERFLOW_CHECKS':'true' if profile=='debug' else 'false','CODEGEN_UNITS':'256' if profile=='debug' else '16','INCREMENTAL':'false'}.items():env['CARGO_PROFILE_'+key+'_'+name]=value
 command=[roles['cargo'],'test','--locked','--offline','--bin','oxid','--target','x86_64-unknown-linux-gnu','--no-run','--jobs','1','--message-format=json-render-diagnostics']
 if profile=='release':command.append('--release')
 return command,env

def cargo_artifact(stdout,source,target,profile,q,p,tick):
 lines=q.read(stdout,32*1024**2).splitlines();need(bool(lines) and all(lines),'Cargo stream');events=[q.strict_json(line) for line in lines];need(all(isinstance(e,dict) for e in events),'Cargo object events');need(events[-1].get('reason')=='build-finished' and [e.get('success') for e in events if e.get('reason')=='build-finished']==[True],'Cargo terminal')
 artifacts=[e for e in events if e.get('reason')=='compiler-artifact' and e.get('executable')];need(len(artifacts)==1,'single emitted ELF');event=artifacts[0]
 expected={'opt_level':'0' if profile=='debug' else '3','debuginfo':2 if profile=='debug' else 0,'debug_assertions':profile=='debug','overflow_checks':profile=='debug','test':True}
 need(event['package_id']=='path+'+source.as_uri()+'#oxid@0.9.0' and event['manifest_path']==str(source/'Cargo.toml') and event['target']['src_path']==str(source/'src/cli.rs') and event['target']['name']=='oxid' and event['target']['kind']==['bin'] and event['profile']==expected and event['fresh'] is False,'fresh path-qualified Cargo profile')
 elf=canonical(event['executable'],False);need(elf.is_relative_to(target/'x86_64-unknown-linux-gnu'/profile/'deps'),'artifact containment')
 import struct
 with elf.open('rb') as stream:header=stream.read(64)
 need(len(header)==64 and header[:7]==b'\x7fELF\x02\x01\x01' and struct.unpack_from('<HHI',header,16)[0] in (2,3) and struct.unpack_from('<HHI',header,16)[1:]==(62,1) and struct.unpack_from('<H',header,52)[0]==64,'ELF64 architecture')
 return event,elf,p.digest(elf,tick)

def runtime_commands(package,elf,run,profile,python):
 observer='frontend::oir::unit2_observer::observe_source_queue';inputs=run/'observer-inputs';raw=run/(profile+'-raw.jsonl');normalized=run/(profile+'-normalized.jsonl')
 commands=[[python,'-B',str(package/'semantic/materialize.py'),str(inputs)],[str(elf),'--list','--exact',observer],[str(elf),'--list','independent_unit2_resource_review'],[str(elf),'--exact',observer,'--test-threads=1'],[str(elf),'independent_unit2_resource_review','--test-threads=1','--show-output'],[python,'-B',str(package/'semantic/normalize.py'),str(raw),str(inputs/'queue.json'),str(normalized)],[python,'-B',str(package/'semantic/compare.py'),profile,str(normalized),str(run/(profile+'-comparison.json'))]]
 env={'HOME':str(run/'home'),'TMPDIR':str(run/'tmp'),'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C','TZ':'UTC','RUST_BACKTRACE':'0','PYTHONDONTWRITEBYTECODE':'1'};envs=[dict(env) for _ in commands];envs[3].update(OXID_OBSERVER_QUEUE=str(inputs/'queue.tsv'),OXID_OBSERVER_OUTPUT=str(raw));envs[4]['OXID_RESOURCE_FIXTURES']=str(run/'resource-fixtures')
 return {'argv':commands,'environments':envs,'cwd':str(run)}

def run_profile(profile,prepared,compilation,root,machine,pins,routes,q,p,tick,preserve):
 run=fresh(root);run.mkdir()
 for name in ('home','tmp','resource-fixtures'):(run/name).mkdir()
 phase_deadline=time.monotonic()+600
 def phase_tick():tick();need(time.monotonic()<phase_deadline,'runtime600s deadline')
 package=Path(prepared['package_root']);elf=Path(compilation['elf']);contract=q.derive_corpus(package,run/'observer-inputs');names=q.strict_json(q.read(package/'resource-test-names.json'));observer='frontend::oir::unit2_observer::observe_source_queue'
 command=runtime_commands(package,elf,run,profile,machine['roles']['python'])
 session={'schema':'package-source-unit2-'+profile+'-session-v1','invocation_id':uuid.uuid4().hex,'started_ns':time.time_ns(),'profile':profile,'binary_sha256':compilation['elf_sha256'],'source_inputs_sha256':prepared['source_inputs_sha256'],'package_inputs_sha256':prepared['package_inputs_sha256'],'compiled_source_map_sha256':q.compiled_map_digest(prepared['assembled380']),'accounting_transport':copy.deepcopy(prepared['accounting_transport']),'current_source':copy.deepcopy(prepared['current_source']),'historical_source_sha256':prepared['historical_source_sha256'],'queue_sha256':contract['queue_sha256'],'queue_tsv_sha256':contract['queue_tsv_sha256'],'execution_qualified':False}
 # These expectations are captured before any runtime subprocess is started.
 context={'root':str(run),'session':copy.deepcopy(session),'process_records':[],'pins':dict(pins),'tool_routes':dict(routes),'command':copy.deepcopy(command),'compilation':copy.deepcopy(compilation)}
 p.write_json(run/'session.json',session);p.write_json(run/'command.json',command)
 reason=None;preservation=False;preservation_error=None;raw=None;window=None;operations=context['process_records'];evidence={}
 def materialized():need(tree(run/'observer-inputs',phase_tick,p)==sorted(contract['files'],key=lambda row:Path(row['path'])),'materialized source/queue body map')
 try:
  walls=[120,10,10,180,120,120,180]
  for index,argv in enumerate(command['argv']):
   phase_tick();preserve(phase_tick);need(p.digest(elf,phase_tick)==compilation['elf_sha256'],'ELF drift');bounds={**p.RUNTIME,'wall':walls[index]}
   op=p.run_operation(argv,run,command['environments'][index],run/('operation-'+str(index)),index,bounds,phase_tick,[run]);operations.append(op);need(op['failure'] is None,'runtime operation failed: '+str(op['failure']))
   need(q.read(run/f'operation-{index}/stderr.log')==b'','unexpected runtime stderr');body=q.read(run/f'operation-{index}/stdout.log')
   if index==0:materialized()
   elif index in (1,2):q.listing(body,[observer] if index==1 else names)
   elif index==3:
    q.success(body,[observer]);window=(op['spawn_ns'],op['finish_ns']);raw=q.observation_rows(run/(profile+'-raw.jsonl'),contract,window)
   elif index==4:q.success(body,names,True);need(not list((run/'resource-fixtures').iterdir()),'resource fixture residue')
   elif index==5:q.observation_rows(run/(profile+'-normalized.jsonl'),contract,window,raw)
   else:q.comparison(run,package,contract['case_ids'],machine['roles']['python'],profile)
  materialized();preserve(phase_tick);q.verify_body_pins(pins,routes,min(phase_deadline,time.monotonic()+90));preservation=True
  for name in ('home','tmp','resource-fixtures'):need(not list((run/name).iterdir()),'runtime temporary residue')
 except BaseException as error:reason=type(error).__name__+': '+str(error)
 finally:
  try:
   preserve(phase_tick);q.verify_body_pins(pins,routes,min(phase_deadline,time.monotonic()+30));preservation=True
  except BaseException as error:preservation_error=type(error).__name__+': '+str(error);reason=reason or 'preservation: '+preservation_error;preservation=False
  try:
   for row in tree(run,phase_tick,p):evidence[row['path']]={'bytes':row['bytes'],'sha256':row['sha256']}
  except BaseException as error:reason=reason or 'evidence: '+str(error)
  report={'schema':'oxid-package-source-full-'+profile+'-unit2-v1','session':session,'result':profile+'-unit2-semantic-resource-passed' if reason is None else 'failed','reason':reason,'pins':dict(pins),'operations':copy.deepcopy(operations),'top_level_processes':len(operations),'semantic_cases':3603 if reason is None else None,'resource_tests':21 if reason is None else None,'preservation_verified':preservation,'preservation_error':preservation_error,'evidence':evidence,'current_execution_status':'NotReady','semantic_pass':False,'execution_qualified':False}
  p.write_json(run/'report.json',report)
 need(reason is None,'runtime failed; retained report: '+str(reason));return context

def produce(repo,output,machine_path,machine_sha):
 need(not sys.flags.optimize and sys.dont_write_bytecode,'normal Python -B required');q,p=helpers();repo=canonical(repo);output=fresh(output)
 need(not output.is_relative_to(repo) and not repo.is_relative_to(output),'output overlaps checkout')
 started=time.monotonic();deadline=started+2700;nonce=uuid.uuid4().hex;started_ns=time.time_ns()
 def tick():need(not STOP and time.monotonic()<deadline,'stopped/2700s producer deadline')
 machine,pins,routes=machine_context(machine_path,machine_sha,q,p,tick)
 before=checkout_identity(repo,machine['roles']['git']);tick();output.mkdir()
 invocation={'root':str(output),'nonce':nonce,'started_ns':started_ns,'checkout_before':before,'profiles':{}}
 transport_path=HERE.parent/'package_dependency_unit2_transport_v1/transport.py';transport=load(transport_path,TRANSPORT_SHA,'current_package_transport');gate,_=transport.anchor(HERE.parent/'package_dependency_current')
 for helper in HERE.iterdir():pins[str(helper)]=p.digest(helper,tick)
 pins[str(transport_path)]=TRANSPORT_SHA
 preserved_views=[];preserved_absence=[]
 capture,references=gate.capture(repo);p.write_json(output/'invocation-start.json',{'schema':'package-unit2-producer-start-v1','nonce':nonce,'started_ns':started_ns,'checkout':before,'machine_contract_sha256':machine_sha})
 def failure_preserve():
  end=min(deadline,time.monotonic()+30)
  def bounded():tick();need(time.monotonic()<end,'failure readback deadline')
  bounded();absent(machine['absent_paths']+preserved_absence);q.verify_body_pins(pins,routes,end)
  for root,expected in preserved_views:need(tree(root,bounded,p)==expected,'copied input view drift')
  now,refs=gate.capture(repo);gate.require_unchanged_capture(capture,references,now,refs);bounded()
 old_term=signal.signal(signal.SIGTERM,stopped);old_int=signal.signal(signal.SIGINT,stopped);result=None;failure=None;preservation_error=None
 try:
  prepared_result=transport.prepare(repo,output/'prepared');tick();report=prepared_result['prepared'];worker=output/'prepared/worker-output';package=worker/'derived-package';source376=output/'prepared/current-inputs';assembled=worker/'assembled'
  prepared={'checkout':before,'package_root':str(package),'package_files':report['derived_package'],'source_root':str(source376),'source376':tree(source376,tick,p),'assembled380':report['assembled'],'current_source':report['current_source'],'historical_source_sha256':report['historical_source_sha256'],'source_inputs_sha256':p.digest(package/'source-inputs.json',tick),'package_inputs_sha256':p.digest(package/'package-inputs.json',tick),'accounting_transport':report['accounting_transport']}
  need(len(prepared['source376'])==376 and tree(assembled,tick,p)==sorted(report['assembled'],key=lambda row:Path(row['path'])),'prepared source cardinality/map');verify_dependencies(assembled,machine,q,p,tick)
  for helper in HERE.iterdir():pins[str(helper)]=p.digest(helper,tick)
  pins[str(transport_path)]=TRANSPORT_SHA
  for row in tree(output/'prepared',tick,p):pins[str(output/'prepared'/row['path'])]=row['sha256']
  captured_pins=dict(pins)
  def preserve(callback):
   callback();absent(machine['absent_paths']);need(p.digest(machine_path,callback)==machine_sha,'machine contract drift');q.verify_body_pins(captured_pins,routes,min(deadline,time.monotonic()+90));need(tree(Path(machine['registry_root']),callback,p)==machine['registry_files'],'registry closure drift')
   now,refs=gate.capture(repo);gate.require_unchanged_capture(capture,references,now,refs)
  for profile in ('debug','release'):
   tick();preserve(tick);usage=os.statvfs(output);need(usage.f_bavail*usage.f_frsize>=20*1024**3,'compile requires20GiB free')
   profile_root=output/profile;profile_root.mkdir();source=profile_root/'source';target=profile_root/'target';work=profile_root/'compile';home=profile_root/'cargo-home';source.mkdir();target.mkdir();work.mkdir();home.mkdir()
   for row in report['assembled']:
    path=source/row['path'];path.parent.mkdir(parents=True,exist_ok=True);body=q.read(assembled/row['path']);need(len(body)==row['bytes'] and q.sha(body)==row['sha256'],'assembled copy drift');path.write_bytes(body)
   expected_source=sorted(report['assembled'],key=lambda row:Path(row['path']));need(tree(source,tick,p)==expected_source,'fresh source copy');preserved_views.append((source,expected_source))
   shutil.copytree(machine['registry_root'],home/'registry',symlinks=False);need(tree(home/'registry',tick,p)==machine['registry_files'],'fresh Cargo registry copy');preserved_views.append((home/'registry',machine['registry_files']))
   for name in ('home','tmp'):(work/name).mkdir()
   config_paths=forbidden_config_paths(source,home,work);absent(config_paths);preserved_absence.extend(config_paths)
   command,env=compile_command(machine,profile,source,target,work,home);p.write_json(work/'command.json',{'argv':command,'env':env,'cwd':str(source)})
   phase_deadline=min(deadline,time.monotonic()+600)
   def compile_tick():tick();need(time.monotonic()<phase_deadline,'compile600s deadline')
   operation=p.run_operation(command,source,env,work/'operation',0,p.COMPILE,compile_tick,[target,work/'tmp']);p.write_json(work/'process.json',operation);need(operation['failure'] is None,'compile failed; evidence retained: '+str(operation['failure']))
   event,elf,elf_sha=cargo_artifact(work/'operation/stdout.log',source,target,profile,q,p,compile_tick)
   need(operation['spawn_ns']<=elf.stat().st_mtime_ns<=operation['finish_ns'] and operation['spawn_ns']<=elf.stat().st_ctime_ns<=operation['finish_ns'],'ELF freshness window')
   need(tree(source,compile_tick,p)==sorted(report['assembled'],key=lambda row:Path(row['path'])),'compiled source drift');need(tree(home/'registry',compile_tick,p)==machine['registry_files'],'compiled registry drift');absent(config_paths);preserve(compile_tick)
   compilation={'source_root':str(source),'target_root':str(target),'elf':str(elf),'elf_sha256':elf_sha,'cargo_event':event,'cargo_stdout':str(work/'operation/stdout.log'),'returncode':operation['exit'],'remaining_owned_group':operation['remaining_owned_group'],'cleanup_error':operation['cleanup_error'],'spawn_ns':operation['spawn_ns'],'finished_ns':operation['finish_ns']}
   profile_pins=dict(captured_pins);profile_pins[str(elf)]=elf_sha
   for base in (source,home/'registry',work):
    for row in tree(base,tick,p):profile_pins[str(base/row['path'])]=row['sha256']
   pins.update(profile_pins)
   invocation['profiles'][profile]=run_profile(profile,prepared,compilation,profile_root/'runtime',machine,profile_pins,routes,q,p,tick,preserve)
  preserve(tick);invocation['checkout_after']=checkout_identity(repo,machine['roles']['git']);invocation['finished_ns']=time.time_ns();tick()
  # This is the first public validator invocation: expectations came from this
  # producer's live captured state, never from a submitted result report.
  result=q.validate_session(prepared,invocation,{profile:invocation['profiles'][profile]['root'] for profile in ('debug','release')});tick()
  p.write_json(output/'validated-session.json',result)
 except BaseException as error:failure=type(error).__name__+': '+str(error)
 finally:
  try:failure_preserve()
  except BaseException as error:preservation_error=type(error).__name__+': '+str(error);failure=failure or 'preservation: '+preservation_error
  p.write_json(output/'producer-result.json',{'schema':'package-unit2-producer-result-v1','status':'session-validated' if result is not None and failure is None else 'failed','reason':failure,'preservation_verified':preservation_error is None,'preservation_error':preservation_error,'invocation_id':nonce,'started_ns':started_ns,'finished_ns':time.time_ns(),'elapsed':time.monotonic()-started,'current_execution_status':'NotReady','semantic_pass':False,'execution_qualified':False,'ci_runtime_proof':False,'scope':'fresh local source-bound session; CI event/head admission remains separate'})
  signal.signal(signal.SIGTERM,old_term);signal.signal(signal.SIGINT,old_int)
 need(failure is None,'producer failed; evidence retained: '+str(failure));return result

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--repo',required=True);parser.add_argument('--output',required=True);parser.add_argument('--machine-contract',required=True);parser.add_argument('--machine-sha256',required=True);args=parser.parse_args()
 produce(Path(args.repo),Path(args.output),Path(args.machine_contract),args.machine_sha256)
if __name__=='__main__':main()
