#!/usr/bin/env python3
"""Portable path/tool translation around the immutable reviewed native controller."""
import pathlib
import sys
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from common import *
from assemble import apply_patch
import argparse,importlib.util,shutil,platform,re

NATIVE_EVIDENCE='f73c125e3b3099a93d9f3be5e20a51abadc5d28feab58dfc8558024f670130ec'
CORE='53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
SELECTED=['adapter.rs','main.rs','run.py','prepare.py','audit_protocol.py','overlay-v2.patch','overlay-manifest-v2.json','controller-manifest-v3.json','request-input-manifest.json','native-requests.json','fuel-requests.frozen.json','driver-requests.proposed.json','no-clobber-requests.json','spy-bin/clang','spy-bin/opt','spy-bin/ld.lld','report.md']

def library_inventory(path):
    """Bound the resolved library directory and every regular/symlink target byte."""
    root=pathlib.Path(path).resolve();require(root.is_dir(),'LLVM library directory missing')
    maximum_entries=2048;maximum_file=256*1024*1024;maximum_total=512*1024*1024
    stack=[(root,0)];entries=[];hashed={};total=0
    while stack:
        directory,depth=stack.pop();require(depth<=16,'LLVM library directory depth limit')
        with os.scandir(directory)as children:
          for child in children:
            require(len(entries)<maximum_entries,'LLVM library inventory entry limit');item=pathlib.Path(child.path);name=item.relative_to(root).as_posix()
            if item.is_symlink():
                target=item.resolve(strict=True);require(target.is_file(),'LLVM library symlink must resolve to a regular file')
                row={'path':name,'kind':'symlink','link':os.readlink(item),'resolved_target':str(target)}
            elif item.is_dir():entries.append({'path':name,'kind':'directory'});stack.append((item,depth+1));continue
            else:require(item.is_file(),'unsupported LLVM library file type');target=item;row={'path':name,'kind':'file'}
            key=str(target)
            if key not in hashed:
                expected_size=target.stat().st_size;require(expected_size<=maximum_file and total+expected_size<=maximum_total,'LLVM library byte inventory limit')
                digest=hashlib.sha256();size=0
                with target.open('rb')as stream:
                    while chunk:=stream.read(min(1024*1024,maximum_file-size+1)):
                        size+=len(chunk);require(size<=maximum_file,'LLVM library grew beyond byte limit');digest.update(chunk)
                require(size==expected_size,'LLVM library changed during inventory');hashed[key]={'bytes':size,'sha256':digest.hexdigest()};total+=size
            row.update(hashed[key]);entries.append(row)
    return {'resolved_directory':str(root),'entries':sorted(entries,key=lambda item:item['path']),'unique_target_bytes':total,'limits':{'entries':maximum_entries,'file_bytes':maximum_file,'unique_target_bytes':maximum_total,'depth':16}}

def qualified_environment(controller,a):
    return {'RUSTC':str(controller.RUST.resolve()),'LLVM_BIN':str(controller.LLVM.resolve()),'LLVM_LIB_DIR':str(pathlib.Path(a.llvm_lib_dir).resolve()),**{name:controller.ENV[name]for name in ('CARGO_HOME','RUSTUP_HOME','CARGO_MANIFEST_DIR','LD_LIBRARY_PATH','OXID_LLVM_BIN')}}

def prepare(a):
    original=pathlib.Path(a.native_root).resolve();evidence=pathlib.Path(a.native_manifest).resolve();require(identity(evidence)['sha256']==NATIVE_EVIDENCE,'native evidence selection changed')
    manifest=read_json(evidence);known={x['path']:x for x in manifest['files']};selected=[]
    for name in SELECTED:
        item=known[name];require(item.get('symlink_target')is None,'component symlink not admitted');verify(original/name,item);selected.append({'path':name,**identity(original/name)})
    core_path=pathlib.Path(a.core_manifest).resolve();require(identity(core_path)['sha256']==CORE,'this wrapper supports frozen core-v1 only; successor requires reviewed rebind')
    core=read_json(core_path);repo=pathlib.Path(a.repo).resolve();check_manifest(repo,core['files'])
    material_path=pathlib.Path(a.materialization).resolve();material=read_json(material_path);require(material['schema']==SCHEMA+'-materialized'and material['status']=='materialized','shared445-source materialization required');check_manifest(material_path.parent,material['members']);verify(material_path.parent/'requests.jsonl',material['requests'])
    oracle=pathlib.Path(a.oracle_manifest).resolve();supplement=pathlib.Path(a.native_supplement).resolve();fuel=pathlib.Path(a.native_fuel_requests).resolve()
    require(identity(oracle)['sha256']==manifest['oracle_manifest_sha256'],'original oracle identity mismatch');require(identity(supplement)['sha256']==manifest['native_supplement_manifest_sha256'],'native supplement identity mismatch')
    supplement_files={x['path']:x for x in read_json(supplement)['files']};verify(fuel,supplement_files['fuel-requests.jsonl'])
    out=fresh(a.output);component=out/'native-component';component.mkdir()
    for item in selected:
        target=component/relative(item['path']);target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(original/item['path'],target)
        if item['path'].startswith('spy-bin/'):target.chmod(0o755)
    shutil.copyfile(evidence,component/'upstream-artifact-manifest.json')
    # The immutable controller's historical relative names are a virtual layout,
    # not a manifest retarget. All compiler files are still frozen old core-v1.
    base=out/'oxid-typed-project-execution';source=component/'source-v2'
    for item in core['files']:
        rel=relative(item['path'])
        for directory in (base,source):
            target=directory/rel;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(repo/rel,target)
    overlay=read_json(component/'overlay-manifest-v2.json');require(overlay['core_manifest_sha256']==CORE,'native overlay/core mismatch')
    require(identity(component/'overlay-v2.patch')['sha256']==overlay['overlay_sha256'],'native patch mismatch')
    patched=apply_patch(source,(component/'overlay-v2.patch').read_text())
    # Preserve the frozen controller's exact input inventory, including its
    # auxiliary historical test files; none is a second Unit3 source corpus.
    for item in overlay['files']:
        target=source/relative(item['path'])
        if not target.exists():
            verify(repo/relative(item['path']),item);target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(repo/item['path'],target)
        verify(target,item)
    check_manifest(source,overlay['files'],exact=True)
    evidence_dir=out/'typed-project-unit3-evidence';evidence_dir.mkdir();shutil.copyfile(core_path,evidence_dir/'source-inputs-core-v1.json')
    oracle_dir=out/'typed-project-unit3-oracles';native_dir=oracle_dir/'supplements/native-diagnostics-v1';native_dir.mkdir(parents=True)
    shutil.copyfile(oracle,oracle_dir/'pre-execution-manifest.json');shutil.copyfile(material_path.parent/'requests.jsonl',oracle_dir/'requests.jsonl');shutil.copyfile(supplement,native_dir/'supplement-manifest.json');shutil.copyfile(fuel,native_dir/'fuel-requests.jsonl')
    result={'schema':SCHEMA+'-native-prepared','status':'prepared-not-executed','assertion_mode':assertion_mode(),'upstream_native_evidence':bound_file(evidence),'core_manifest':bound_file(core_path),'materialization':bound_file(material_path),'shared_source_root':str(material_path.parent),'native_component_directory':'native-component','component_files':selected,'patched_files':patched,'prepared_files':files_manifest(out),'fixture_translation':'verified request.source_root below shared physical materialization; no symlinked case roots; no corpus copy'}
    write_json(out/'native-prepared.json',result);return result

def load(a):
    prepared_path=pathlib.Path(a.prepared_manifest).resolve();prepared=read_json(prepared_path);require(prepared['schema']==SCHEMA+'-native-prepared','wrong native prepared manifest')
    workspace=prepared_path.parent;component=workspace/relative(prepared['native_component_directory']);check_manifest(workspace,prepared['prepared_files'])
    require(pathlib.Path(prepared['shared_source_root'])==pathlib.Path(prepared['materialization']['path']).parent,'shared source root rebound');verify(prepared['materialization']['path'],prepared['materialization'])
    material=read_json(prepared['materialization']['path']);check_manifest(prepared['shared_source_root'],material['members'])
    rustc=pathlib.Path(a.rustc).resolve();llvm=pathlib.Path(a.llvm_bin).resolve();library=pathlib.Path(a.llvm_lib_dir).resolve()
    require(rustc.is_file()and os.access(rustc,os.X_OK)and library.is_dir(),'missing explicit native tools/library')
    for name in ('clang','opt','ld.lld'):require((llvm/name).is_file()and os.access(llvm/name,os.X_OK),'LLVM tool missing')
    spec=importlib.util.spec_from_file_location('unit3_frozen_native_controller',component/'run.py');controller=importlib.util.module_from_spec(spec);spec.loader.exec_module(controller)
    # Only host/workspace/tool/fixture locations are rebound. Original validation,
    # build, invocation, source hiding, tool spies and ELF execution stay intact.
    controller.R=component;controller.ROOT=workspace;controller.RUST=rustc;controller.LLVM=llvm
    environment=safe_env({'CARGO_HOME':str(pathlib.Path(a.cargo_home).resolve()),'RUSTUP_HOME':str(pathlib.Path(a.rustup_home).resolve()),'CARGO_INCREMENTAL':'0','CARGO_BUILD_JOBS':'2','CARGO_MANIFEST_DIR':str(component/'source-v2'),'OXID_LLVM_BIN':str(llvm),'LD_LIBRARY_PATH':str(library)})
    for name in ('RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_BUILD_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER'):environment.pop(name,None)
    controller.ENV=environment
    root=pathlib.Path(prepared['shared_source_root'])
    by_id={x['id']:x for x in map(json.loads,(root/'requests.jsonl').read_text().splitlines())}
    def fixture_input(request):
        require(request['id']in by_id,'unknown source case')
        original=by_id[request['id']]
        for key in ('entry','source_root','source_files'):require(request[key]==original[key],'source-only native fixture metadata changed')
        fixture=root/relative(request['source_root']);require(fixture.resolve()==fixture,'aliased native source root')
        for item in request['source_files']:verify(fixture/relative(item['path']),item)
        return fixture,fixture/relative(request['entry'])
    controller.fixture_input=fixture_input
    controller.verify_inputs()
    return prepared_path,prepared,controller

def validate_invocation(record,controller):
    """The frozen audit's per-invocation transport checks, without a300-run roster."""
    compiler=record['compiler'];require(compiler['status']is not None and not compiler.get('qualification_failure')and not record.get('qualification_failure'),'native invocation failed to complete')
    require(record['tool_receipt_exists']and isinstance(record['tools'],list),'missing initialized native tool ledger')
    require(record['source_restored']and record['source_restored_after_execution'],'source restoration failed')
    states=record.get('source_state');require(isinstance(states,list)and[v['phase']for v in states]==['after_load_before_check','restored'],'missing/invalid native source-state protocol')
    require(states[0].get('all_source_paths_absent')is True,'canonical source paths remained available')
    require(set(states[0]['files'])=={str(pathlib.Path(record['fixture_root'])/f['path'])for f in record['source_files']},'actual loaded source membership differs')
    loc=pathlib.Path(record['environment_overrides']['OXID_UNIT3_NATIVE_RECEIPT'])
    for stream in ('stdout','stderr'):
        require(identity(loc/stream)['sha256']==compiler[stream+'_sha256']==hashlib.sha256(compiler[stream].encode()).hexdigest(),'compiler output bytes changed')
    require(identity(controller.R/('adapter-'+record['profile']))['sha256']==record['binary_sha256'],'native adapter identity changed')
    execution=record.get('execution')
    if execution:
        require(execution['source_unavailable']and execution['cwd_before_files']==['program']and execution['environment']==controller.MINIMAL,'source-free native execution protocol')
        require(execution['status']is not None and not execution.get('qualification_failure'),'native execution did not complete')
        require(identity(loc/'isolated/program')['sha256']==record['artifact_sha256']==execution['elf_sha256'],'ELF identity changed')
        with(loc/'isolated/program').open('rb')as stream:require(stream.read(4)==b'\x7fELF','missing ELF magic')
        require([pathlib.Path(t['tool']).name for t in record['tools']]==['clang','opt','ld.lld','opt','clang','clang','clang'],'LLVM tool sequence changed')
        require([t['argv']for t in record['tools'][:3]]==[['--version']]*3,'LLVM version command sequence')
        require(record['tools'][3]['argv']==['-passes=verify','-disable-output','program.ll'],'LLVM verifier command changed')
        require(all('-O0'in t['argv']for t in record['tools'][4:6]),'native optimization changed')
        require(identity(loc/'actual.ll')['sha256']==record['ir_sha256'],'emitted LLVM identity changed')
        for stream in ('stdout','stderr'):require(identity(loc/'execution'/stream)['sha256']==execution[stream+'_sha256'],'ELF output bytes changed')
    if compiler['status']!=0:require(record['tools']==[],'declared compiler rejection invoked external tools')
    if record['operation']!='compile':require(record['tools']==[]and not execution,'check/run unexpectedly invoked native tools')
    if record['operation']=='compile'and compiler['status']==0:require(execution is not None,'successful compile omitted actual ELF execution')
    if record['noclobber_before'] is not None:require(record['noclobber_before']==record['noclobber_after']and record['tools']==[]and record['spy_calls']=='','no-clobber protocol failure')
    fixture=pathlib.Path(record['fixture_root']);require({p.relative_to(fixture).as_posix()for p in fixture.rglob('*')if p.is_file()}=={f['path']for f in record['source_files']},'source tree changed or gained cache')

def main():
    p=argparse.ArgumentParser();sub=p.add_subparsers(dest='command',required=True)
    q=sub.add_parser('prepare')
    for flag in ('repo','core-manifest','native-root','native-manifest','oracle-manifest','native-supplement','native-fuel-requests','materialization','output'):q.add_argument('--'+flag,required=True)
    for command in ('build','invoke'):
        q=sub.add_parser(command)
        for flag in ('prepared-manifest','rustc','llvm-bin','llvm-lib-dir','cargo-home','rustup-home','profile','receipt'):q.add_argument('--'+flag,required=True)
        if command=='invoke':
            q.add_argument('--build-wrapper-receipt',required=True)
            q.add_argument('--case',required=True);q.add_argument('--operation',choices=('check','run','compile'),required=True);q.add_argument('--format',choices=('text','json'),default='json');q.add_argument('--fuel',type=int);q.add_argument('--noclobber',choices=('file','symlink'))
    a=p.parse_args()
    if a.command=='prepare':result=prepare(a);print(json.dumps({'status':result['status']}));return 0
    require(a.profile in (('debug','release','both')if a.command=='build'else('debug','release')),'explicit native profile required')
    receipt_path=pathlib.Path(a.receipt).resolve();require(not receipt_path.exists(),'native wrapper receipt exists');receipt_path.parent.mkdir(parents=True,exist_ok=True)
    record={'schema':SCHEMA+'-native-wrapper','status':'native-wrapper-failure','assertion_mode':assertion_mode(),'command':a.command,'profile':a.profile,'wrapper':bound_file(pathlib.Path(__file__))}
    try:
        prepared_path,prepared,controller=load(a);record.update(prepared=bound_file(prepared_path),frozen_controller=bound_file(controller.R/'run.py'),tool_paths={'rustc':str(controller.RUST),'llvm_bin':str(controller.LLVM),'llvm_lib_dir':a.llvm_lib_dir})
        require(platform.system()=='Linux'and platform.machine()=='x86_64','unqualified native host')
        tool_paths={'rustc':controller.RUST,**{name:controller.LLVM/name for name in ('clang','opt','ld.lld')}}
        record['host']=platform.uname()._asdict();qualification_path=controller.R/'qualified-build-tools.json'
        if a.command=='build':
            require(not qualification_path.exists(),'qualified native build/tool set already exists')
            versions=[]
            for name,path in tool_paths.items():
                args=['--version','--verbose']if name=='rustc'else['--version'];result=run_bounded([str(path),*args],env=controller.ENV,timeout=20)
                versions.append({'name':name,'tool':bound_file(path),'argv':[str(path),*args],'status':result.returncode,'stdout':result.stdout,'stderr':result.stderr})
                require(result.returncode==0,'native tool version probe failed')
                require(result.stdout.startswith('rustc 1.99.0 ')if name=='rustc'else re.search(r'\b19\.1\.7\b',result.stdout)is not None,'unqualified native tool version')
            record['tool_versions']=versions
            profiles=['debug','release']if a.profile=='both'else[a.profile]
            controller.build(profiles)
            qualification={'schema':SCHEMA+'-native-qualified-tools','prepared':bound_file(prepared_path),'wrapper':bound_file(pathlib.Path(__file__)),'frozen_controller':bound_file(controller.R/'run.py'),'host':record['host'],'tool_versions':versions,'environment_paths':qualified_environment(controller,a),'llvm_libraries':library_inventory(a.llvm_lib_dir),'profiles':{profile:{'build_receipt':bound_file(controller.R/('build-'+profile)/'verified-build.json'),'binary':bound_file(controller.R/('adapter-'+profile))}for profile in profiles}}
            write_json(qualification_path,qualification);record['qualified_build_tools']=bound_file(qualification_path)
        else:
            # No version process may run here: source/native admission and output
            # checks must retain their original ordering before external tools.
            build_wrapper=read_json(a.build_wrapper_receipt);require(build_wrapper['schema']==SCHEMA+'-native-wrapper'and build_wrapper['command']=='build'and build_wrapper['status']=='completed','qualified native build wrapper receipt required')
            require(build_wrapper['profile']in(a.profile,'both'),'native wrapper profile binding mismatch')
            verify(prepared_path,build_wrapper['prepared']);verify(pathlib.Path(__file__),build_wrapper['wrapper']);verify(qualification_path,build_wrapper['qualified_build_tools'])
            record['build_wrapper_receipt']=bound_file(a.build_wrapper_receipt)
            qualification=read_json(qualification_path);require(qualification['schema']==SCHEMA+'-native-qualified-tools','native build/tool qualification missing')
            require(qualified_environment(controller,a)==qualification['environment_paths'],'qualified native environment paths changed')
            require(library_inventory(a.llvm_lib_dir)==qualification['llvm_libraries'],'qualified LLVM library content/targets changed')
            verify(prepared_path,qualification['prepared']);verify(pathlib.Path(__file__),qualification['wrapper']);verify(controller.R/'run.py',qualification['frozen_controller'])
            require(a.profile in qualification['profiles'],'profile absent from qualified native build')
            verify(controller.R/('build-'+a.profile)/'verified-build.json',qualification['profiles'][a.profile]['build_receipt']);verify(controller.R/('adapter-'+a.profile),qualification['profiles'][a.profile]['binary'])
            require({item['name']for item in qualification['tool_versions']}==set(tool_paths),'qualified tool membership changed')
            for item in qualification['tool_versions']:verify(tool_paths[item['name']],item['tool'])
            record['qualified_build_tools']=bound_file(qualification_path)
            native=read_json(controller.R/'native-requests.json');driver=read_json(controller.R/'driver-requests.proposed.json');native_by_id={x['id']:x for x in native};driver_by_id={x['id']:x for x in driver};requests={**native_by_id,**driver_by_id};require(a.case in requests,'case not in frozen native/driver roster')
            request=requests[a.case];group='driver'
            if a.noclobber is not None:group='no-clobber'
            elif a.fuel is not None:
                require((a.operation,a.format)in(('compile','json'),('run','text')),'fuel operation/format outside frozen roster');group='native-fuel'if a.operation=='compile'else'reference-fuel'
            elif a.operation=='compile'and a.format=='json'and a.case in native_by_id:group='native-default';request=native_by_id[a.case]
            else:require(a.case in driver_by_id,'unlisted driver operation/format');request=driver_by_id[a.case]
            allowed=set(request.get('driver_operations',request['operations']))
            if request.get('native'):allowed.add('compile')
            require(a.operation in allowed,'operation outside frozen source request')
            if a.fuel is not None:
                fuel={x['id']:x for x in read_json(controller.R/'fuel-requests.frozen.json')};require(a.case in fuel and a.fuel in fuel[a.case]['fuel_budgets'],'fuel outside frozen native allowance');request=fuel[a.case]
            if a.noclobber is not None:
                kind='regular'if a.noclobber=='file'else'symlink';controls=read_json(controller.R/'no-clobber-requests.json')['requests'];require(any(x['id']==a.case and x['format']==a.format and x['output_kind']==kind for x in controls),'unlisted no-clobber control');require(a.operation=='compile','no-clobber operation')
            # A native invocation temporarily hides this physical source root.
            # Serialize callers of this wrapper on the shared materialization.
            import fcntl
            with(pathlib.Path(prepared['shared_source_root'])/'.native-qualification.lock').open('a')as lock:
                fcntl.flock(lock,fcntl.LOCK_EX|fcntl.LOCK_NB)
                record['actual']=controller.invocation(request,a.profile,group,a.operation,a.format,a.fuel,a.noclobber,spy=a.noclobber is not None)
                validate_invocation(record['actual'],controller)
        record['status']='completed'
    except BaseException as error:record['error']=f'{type(error).__name__}: {error}'
    write_json(receipt_path,record);print(json.dumps({'status':record['status'],'receipt':str(receipt_path)}));return 0 if record['status']=='completed'else 1
if __name__=='__main__':sys.exit(main())
