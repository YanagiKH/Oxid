"""Dormant Unit2 assembly-only transport. Never return runtime authority."""
import ast
import copy
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import types

HERE=Path(__file__).resolve().parent
SEAL_SHA='e09f0d4d8802e73c4e9f459937e609426319cc5d9eafd39015db40064a5b4bfd'
U2='tests/fixtures/typed_project_unit2_independent/'
OLD_KEY='u8_accounting_source_binding'
DIRECT_KEY='lexer_reservation_unit2_accounting_source_binding'
NEW_KEY='package_dependency_unit2_accounting_source_binding'
RESOURCE_MODULES=(('src/frontend/declaration_index.rs','src/frontend/declaration_index/reviewer_resource.rs',('resource-review-tests.rs','supplemental-resource-review-tests.rs','old-admission-review-test.rs','exposure-origin-review-test.rs')),('src/frontend/parser.rs','src/frontend/parser/reviewer_resource.rs',('parser-resource-review-tests.rs',)),('src/frontend/source.rs','src/frontend/source/reviewer_resource.rs',('source-identity-review-tests.rs',)))

def need(ok,message):
    if not ok:raise ValueError(message)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def row(name,raw):return dict(path=name,bytes=len(raw),sha256=sha(raw))
def encoded(value):return (json.dumps(value,indent=2,sort_keys=True)+'\n').encode()
def regular(path):
    path=Path(path)
    need(path.is_file() and not any(p.is_symlink() for p in (path,*path.parents)) and not path.stat().st_mode & 0o111,'nonregular input')
    return path.read_bytes()

def anchor(checkout_helper):
    need(sys.flags.optimize==0 and __debug__,'normal Python required')
    raw=regular(HERE/'seal.py');need(sha(raw)==SEAL_SHA,'transport seal pin')
    syntax=ast.parse(raw);need(len(syntax.body)==1 and isinstance(syntax.body[0],ast.Assign) and len(syntax.body[0].targets)==1 and isinstance(syntax.body[0].targets[0],ast.Name) and syntax.body[0].targets[0].id=='SEAL','literal seal only')
    seal=ast.literal_eval(syntax.body[0].value)
    for n,key in (('historical_prepare_worker.py','worker_sha256'),('test_transport.py','tests_sha256')):need(sha(regular(HERE/n))==seal[key],'transport dependency pin')
    helper=Path(checkout_helper);need(helper.is_absolute() and helper==helper.resolve(),'canonical frozen helper')
    base=helper.parents[2]
    need(helper==base/'tests/qualification/package_dependency_current','frozen helper relative path')
    for entry in seal['frozen_files']:need(row(entry['path'],regular(base/entry['path']))==entry,'changed frozen checkout closure')
    code=regular(helper/'checkout.py')
    expected_entry=next(r for r in seal['frozen_files'] if r['path']=='tests/qualification/package_dependency_current/checkout.py')
    need(row(expected_entry['path'],code)==expected_entry,'frozen entrypoint identity')
    gate=types.ModuleType('frozen_package_checkout');gate.__file__=str(helper/'checkout.py')
    exec(compile(code,gate.__file__,'exec'),gate.__dict__)
    gate.anchored_data();gate.closed(HERE,{'transport.py','seal.py','historical_prepare_worker.py','test_transport.py'})
    return gate,seal

def expected_resource_package(old):
    bodies=dict(old['historical_bytes'])
    updates={'archive/resource/parser-resource-review-tests.rs':old['resource'],'archive/resource/resource-review-tests.rs':old['u8_index_resource'],'semantic/compare.py':old['unit2_comparator'],'semantic/compare_before_enum_enabled_qualified_values.py':old['historical_bytes']['semantic/compare.py'],'semantic/enum_enabled_qualified_values_v1.py':old['package_bytes']['enum_enabled_qualified_values_v1.py'],'semantic/enum-enabled-qualified-values-v1.json':old['package_bytes']['enum-enabled-qualified-values-v1.json'],'enum-source.json':old['package_bytes']['enum-source.json'],'semantic/observer.rs':old['observer']}
    bodies.update(updates)
    need({n for n,b in bodies.items() if b!=old['historical_bytes'].get(n)}==set(updates),'unexpected resource derivation')
    manifest={**old['historical'],'files':[row(n,b) for n,b in sorted(bodies.items())]}
    need(len(old['historical']['files'])==35 and len(manifest['files'])==39,'resource roster')
    bodies['package-inputs.json']=encoded(manifest)
    return bodies

def derive_current_package(resource_package,new_manifest):
    manifest=json.loads(resource_package['package-inputs.json'])
    need([r['path'] for r in manifest['files']].count('source-inputs.json')==1,'source row count')
    need({r['path'] for r in manifest['files']}==set(resource_package)-{'package-inputs.json'},'resource package closure')
    need(all(row(r['path'],resource_package[r['path']])==r for r in manifest['files']),'resource package hash')
    bodies=dict(resource_package);bodies['source-inputs.json']=new_manifest
    final={**manifest,'files':[row(r['path'],new_manifest) if r['path']=='source-inputs.json' else r for r in manifest['files']]}
    bodies['package-inputs.json']=encoded(final)
    need({n for n in bodies if bodies[n]!=resource_package[n]}=={'source-inputs.json','package-inputs.json'},'exact two-file transport')
    return bodies

def expected_assembly(current,package):
    bodies=dict(current);changes=[];added=[]
    def append(name,tail):
        before=bodies[name];bodies[name]=before+tail
        changes.append(dict(path=name,before_sha256=sha(before),after_sha256=sha(bodies[name])))
    for module,destination,payloads in RESOURCE_MODULES:
        need(destination not in bodies,'already instrumented resource')
        bodies[destination]=b''.join(package['archive/resource/'+n] for n in payloads);added.append(destination)
        relative=str(Path(destination).relative_to(Path(module).parent))
        append(module,('\n#[cfg(test)]\n#[path = "'+relative+'"]\nmod independent_unit2_resource_review;\n').encode())
    observer='src/frontend/oir/unit2_observer.rs';need(observer not in bodies,'already instrumented observer')
    bodies[observer]=package['semantic/observer.rs'];added.append(observer)
    append('src/frontend/oir/mod.rs',b'\n#[cfg(test)]\npub(in crate::frontend) mod unit2_observer;\n')
    name='src/frontend/project.rs';before=bodies[name]
    old=b'    ) -> Result<String, Box<Diagnostic>> {\n        let mut file = File::open(path)'
    new=(b'    ) -> Result<String, Box<Diagnostic>> {\n        #[cfg(test)]\n        crate::frontend::oir::unit2_observer::record_source_read(path, display, origin);\n        let mut file = File::open(path)')
    need(before.count(old)==1,'source read seam');bodies[name]=before.replace(old,new)
    changes.append(dict(path=name,before_sha256=sha(before),after_sha256=sha(bodies[name])))
    need(len(current)==376 and len(bodies)==380 and len(changes)==5 and len(added)==4,'assembly cardinalities')
    need(bodies['src/main.rs']==current['src/main.rs'],'new main changed')
    return dict(sorted(bodies.items())),changes,sorted(added)

def accounting_binding(context,old,gate):
    older=old[OLD_KEY];direct=old[DIRECT_KEY]
    need(direct['predecessor_accounting_source_binding']==older,'historical nested accounting mismatch')
    need(direct['current_source']==row('current-source.json',old['package_bytes']['current-source.json']),'direct source-v2 accounting manifest')
    need(direct['reviewed_source_head']==context['predecessor_source']['reviewed_source_head'] and direct['source_only_tree']==context['predecessor_source']['source_only_tree'],'direct source-v2 checkpoint')
    need(older['version']=='unit2-byte-storage-identical-accounting-source-v1' and direct['version']=='unit2-lexer-reservation-identical-accounting-source-v1','historical accounting versions')
    need(old['current']==context['predecessor_source'] and old['inputs']==context['predecessor_inputs'],'old capture is not direct predecessor')
    authority=old['u8_index_resource_authority'];dependencies=authority['source_dependencies']
    need(tuple(r['path'] for r in dependencies)==gate.ACCOUNTING,'accounting dependency roster')
    for entry in dependencies:
        name=entry['path'];need(row(name,context['current_inputs'][name])==entry and context['current_inputs'][name]==context['predecessor_inputs'][name],'accounting changed')
    need(row('archive/resource/resource-review-tests.rs',old['u8_index_resource'])==authority['derived'],'derived accounting resource')
    return dict(version='unit2-package-dependency-identical-accounting-source-v1',current_source=row('package-dependency-source-v1.json',encoded(context['current_source'])),reviewed_source_head=gate.C,source_only_tree=gate.CT,source_dependencies=copy.deepcopy(dependencies),derived_resource=copy.deepcopy(authority['derived']),predecessor_source_v2_accounting_binding=copy.deepcopy(direct),older_byte_storage_accounting_binding=copy.deepcopy(older))

def validate_transport(current,package,actual,transcript,added):
    expected,changes,additions=expected_assembly(current,package)
    need(actual==expected,'complete assembly mismatch')
    need(transcript==changes and added==additions,'instrumentation transcript/additions mismatch')
    return expected

def execute(*args,**kwargs):raise RuntimeError('NotReady: assembly preparation cannot authorize current execution')


def historical_bindings_data(references,gate):
    retained={n:references[gate.SAVED+n] for n in ('current-source.json','u8-source.json','unit2-u8-resource-authority.json','unit2_u8_resource.py')}
    predecessor=json.loads(retained['current-source.json']);u8=json.loads(retained['u8-source.json']);authority=json.loads(retained['unit2-u8-resource-authority.json'])
    derived=copy.deepcopy(authority['derived'])
    older=dict(version='unit2-byte-storage-identical-accounting-source-v1',current_source=row('current-source.json',retained['current-source.json']),reviewed_source_head=predecessor['reviewed_source_head'],source_only_tree=predecessor['source_only_tree'],retained_accounting_source=row('u8-source.json',retained['u8-source.json']),retained_reviewed_source_head=u8['reviewed_source_head'],retained_source_only_tree=u8['source_only_tree'],retained_authority=row('unit2-u8-resource-authority.json',retained['unit2-u8-resource-authority.json']),retained_helper=row('unit2_u8_resource.py',retained['unit2_u8_resource.py']),derived_resource=derived,source_dependencies=authority['source_dependencies'])
    raw=references[gate.SOURCE+'current-source.json'];source=json.loads(raw)
    direct={**copy.deepcopy(older),'version':'unit2-lexer-reservation-identical-accounting-source-v1','current_source':row('current-source.json',raw),'reviewed_source_head':source['reviewed_source_head'],'source_only_tree':source['source_only_tree'],'predecessor_accounting_source_binding':copy.deepcopy(older)}
    return {OLD_KEY:older,DIRECT_KEY:direct}


def require_exact_accounting(actual,expected):
    need(actual==expected,'accounting binding changed or historical fields relabeled')


def verify_worker_output(output,context,references,gate):
    report_raw=gate.regular(output/'prepared-inputs.json');report=gate.decode(report_raw)
    need(set(report)=={'schema','status','compiler_executions','semantic_pass','execution_qualified','current_execution_status','current_source','source_checkpoint','historical_source_sha256','historical_selected_members','historical_archive_members','resource_package','derived_package','assembled','instrumentation','added_files','historical_accounting','accounting_transport','output_file_count','output_byte_budget','protocol_identity','assembler_identity'},'closed preparation report schema')
    verify_declared_modules(report,references)
    names=[gate.safe_name(r['path']) for r in report['resource_package']]
    need(len(names)==len(set(names)) and all(set(r)=={'path','bytes','sha256'} for r in report['resource_package']),'resource row schema/membership')
    need(report['schema']=='oxid-package-dependency-unit2-inputs-prepared-v1' and report['status']=='prepared-inputs-only' and report['compiler_executions']==0 and report['semantic_pass'] is False and report['execution_qualified'] is False and report['current_execution_status']=='NotReady','execution claim in preparation')
    need(report['historical_source_sha256']==gate.OLD_SHA and report['historical_selected_members']==376 and report['historical_archive_members']==117,'historical provenance')
    _,_,raw=gate.anchored_data();new=raw['package-dependency-source-v1.json']
    need(report['current_source']==row('package-dependency-source-v1.json',new) and report['source_checkpoint']==dict(head=gate.C,tree=gate.CT),'new source provenance')
    resource={r['path']:gate.regular(output/'compatibility/typed_project_unit2_independent'/r['path']) for r in report['resource_package']}
    gate.verify_rows(resource,report['resource_package']);gate.closed(output/'compatibility/typed_project_unit2_independent',resource)
    verify_resource_authorities(resource,references,gate)
    package=derive_current_package(resource,new)
    need(report['derived_package']==[row(n,b) for n,b in sorted(package.items())],'derived package map')
    assembled,changes,added=expected_assembly(context['current_inputs'],package)
    need(report['assembled']==[row(n,b) for n,b in assembled.items()] and report['instrumentation']==changes and report['added_files']==[row(n,assembled[n]) for n in added],'assembly evidence differs')
    historical=historical_bindings(references,resource['archive/resource/resource-review-tests.rs'],gate)
    require_exact_accounting(report['historical_accounting'],historical)
    expected_new=dict(version='unit2-package-dependency-identical-accounting-source-v1',current_source=row('package-dependency-source-v1.json',new),reviewed_source_head=gate.C,source_only_tree=gate.CT,source_dependencies=copy.deepcopy(historical[DIRECT_KEY]['source_dependencies']),derived_resource=copy.deepcopy(historical[DIRECT_KEY]['derived_resource']),predecessor_source_v2_accounting_binding=copy.deepcopy(historical[DIRECT_KEY]),older_byte_storage_accounting_binding=copy.deepcopy(historical[OLD_KEY]))
    require_exact_accounting(report['accounting_transport'],expected_new)
    wanted={**{'compatibility/typed_project_unit2_independent/'+n:b for n,b in resource.items()},'compatibility/typed_project_unit3_compatibility/run.py':references['tests/fixtures/typed_project_unit3_compatibility/run.py'],**{'derived-package/'+n:b for n,b in package.items()},**{'assembled/'+n:b for n,b in assembled.items()},'prepared-inputs.json':report_raw}
    expected_budget=sum(len(b) for n,b in wanted.items() if n!='prepared-inputs.json')+2*1024*1024
    need(len(wanted)==report['output_file_count']==462 and report['output_byte_budget']==expected_budget and sum(map(len,wanted.values()))<=expected_budget<=24*1024*1024,'worker output count/budget')
    gate.closed(output,wanted)
    for n,b in wanted.items():need(gate.regular(output/n)==b,'worker output mutation')
    return report,wanted


def prepare(repo,output,checkout_helper=None):
    helper=Path(checkout_helper) if checkout_helper is not None else HERE.parent/'package_dependency_current'
    gate,seal=anchor(helper);repo=Path(repo);output=Path(output)
    context,references=gate.capture(repo);_,layout,_=gate.anchored_data()
    need(output.is_absolute() and output==output.resolve(strict=False) and not output.exists(),'fresh canonical transport output')
    need(repo!=output and repo not in output.parents and output not in repo.parents,'output overlaps repository')
    need(output.parent.is_dir() and not any(p.is_symlink() for p in output.parents),'regular output parent')
    # Bound initial writes from sealed layout/current rows; reserve the fixed
    # reviewed maximum for worker derivations, whose exact budget precedes writes.
    budget=layout['total_bytes']+sum(map(len,context['current_inputs'].values()))+24*1024*1024+4*1024*1024
    need(budget<=72*1024*1024,'transport byte budget')
    output.mkdir()
    admission=gate.admit(repo,output/'admission')
    need(admission['current_inputs']==context['current_inputs'] and admission['predecessor_inputs']==context['predecessor_inputs'],'source changed before preparation')
    current=output/'current-inputs';gate.materialize_predecessor(current,context['current_inputs'])
    worker=output/'worker-output';scratch=output/'worker-scratch';scratch.mkdir()
    command=['/usr/bin/timeout','--kill-after=5s','90s','/usr/bin/python3','-B','-s',str(HERE/'historical_prepare_worker.py'),str(helper),str(repo),str(output/'admission/predecessor'),str(current),str(worker)]
    env=dict(PATH='/usr/bin:/bin',HOME='/nonexistent',LANG='C.UTF-8',LC_ALL='C.UTF-8',PYTHONNOUSERSITE='1',TMPDIR=str(scratch))
    with (output/'worker.stdout').open('xb') as stdout,(output/'worker.stderr').open('xb') as stderr:
        process=subprocess.Popen(command,env=env,cwd=output,stdout=stdout,stderr=stderr,start_new_session=True)
        try:returncode=process.wait(timeout=100)
        finally:
            try:os.killpg(process.pid,signal.SIGKILL)
            except ProcessLookupError:pass
            process.wait()
    need(returncode==0,'source-only Unit2 worker failed; evidence retained')
    need(not gate.regular(output/'worker.stderr'),'worker stderr')
    report,worker_files=verify_worker_output(worker,context,references,gate)
    summary=json.loads(gate.regular(output/'worker.stdout'))
    need(summary==dict(status='prepared-inputs-only',files=len(worker_files),bytes=sum(map(len,worker_files.values())),prepared_sha256=sha(worker_files['prepared-inputs.json']),compiler_executions=0,current_execution_status='NotReady'),'worker summary')
    fresh,fresh_refs=gate.capture(repo);gate.require_unchanged_capture(context,references,fresh,fresh_refs);anchor(helper)
    gate.closed(scratch,set());gate.closed(output/'admission/scratch',set())
    wanted={**{'admission/predecessor/'+n:b for n,b in {**context['predecessor_inputs'],**references}.items()},**{'current-inputs/'+n:b for n,b in context['current_inputs'].items()},**{'worker-output/'+n:b for n,b in worker_files.items()},**{n:gate.regular(output/n) for n in ('admission/stdout','admission/stderr','worker.stdout','worker.stderr')}}
    # Empty scratch directories are required and checked separately. Root closure
    # names them as explicit directories rather than allowing general exclusions.
    expected_dirs={str(p) for n in wanted for p in Path(n).parents if str(p)!='.'}|{'worker-scratch','admission/scratch'}
    actual_files=set();actual_dirs=set()
    import stat
    for p in output.rglob('*'):
        mode=p.lstat().st_mode;need(not stat.S_ISLNK(mode) and (stat.S_ISREG(mode) or stat.S_ISDIR(mode)),'special transport output')
        (actual_dirs if stat.S_ISDIR(mode) else actual_files).add(p.relative_to(output).as_posix())
    need(actual_files==set(wanted) and actual_dirs==expected_dirs,'complete transport output closure')
    for n,b in wanted.items():need(gate.regular(output/n)==b,'transport output drift')
    need(sum(map(len,wanted.values()))<=budget,'transport output byte bound')
    return dict(status='prepared-inputs-only',current_execution_status='NotReady',execution_qualified=False,compiler_executions=0,semantic_pass=False,prepared=report,worker_pid=process.pid,worker_returncode=returncode,owned_group_cleanup='kill-and-reap-complete',command=command,output_files=len(wanted),output_bytes=sum(map(len,wanted.values())),output_byte_budget=budget)


def verify_resource_authorities(package,references,gate):
    original=json.loads(references[U2+'package-inputs.json'])
    expected={r['path']:r for r in original['files']}
    stdin=json.loads(references[gate.SOURCE+'stdin-authority.json'])
    enum=json.loads(references[gate.SOURCE+'enum-authority.json'])
    u8=json.loads(references[gate.SOURCE+'unit2-u8-resource-authority.json'])
    for entry in (stdin['resource_adapter']['derived'],stdin['unit2_semantic_adapter']['derived'],enum['unit2_observer_adapter']['derived'],u8['derived']):expected[entry['path']]=entry
    for name,body in {'semantic/compare_before_enum_enabled_qualified_values.py':references[U2+'semantic/compare.py'],'semantic/enum_enabled_qualified_values_v1.py':references[gate.SOURCE+'enum_enabled_qualified_values_v1.py'],'semantic/enum-enabled-qualified-values-v1.json':references[gate.SOURCE+'enum-enabled-qualified-values-v1.json'],'enum-source.json':references[gate.SOURCE+'enum-source.json']}.items():expected[name]=row(name,body)
    need(len(original['files'])==35 and len(expected)==39,'authority resource roster')
    need(set(package)==set(expected)|{'package-inputs.json'},'authority resource membership')
    need(all(row(n,package[n])==entry for n,entry in expected.items()),'resource body differs from independent immutable authority')
    manifest={**original,'files':[expected[n] for n in sorted(expected)]}
    need(package['package-inputs.json']==encoded(manifest),'resource manifest derivation')


def historical_bindings(references,resource,gate):
    value=historical_bindings_data(references,gate)
    need(row('archive/resource/resource-review-tests.rs',resource)==value[OLD_KEY]['derived_resource'],'immutable resource authority')
    return value


def verify_declared_modules(report,references):
    need(report['protocol_identity']==row('protocol.py',references[U2+'protocol.py']) and report['assembler_identity']==row('assemble.py',references[U2+'assemble.py']),'claimed protocol/assembler identity')


def assembler_inventory_rows(bodies):
    """Match unchanged assembler.inventory's deterministic Path-component order."""
    return [row(n,bodies[n]) for n in sorted(bodies,key=Path)]


def verify_assembler_receipt(receipt,bodies,source_manifest,changes):
    need(set(receipt)=={'files','instrumentation','source_inputs_sha256'},'closed assembler receipt')
    need(receipt['files']==assembler_inventory_rows(bodies) and receipt['source_inputs_sha256']==sha(source_manifest) and receipt['instrumentation']==changes,'assembly manifest/source binding')
