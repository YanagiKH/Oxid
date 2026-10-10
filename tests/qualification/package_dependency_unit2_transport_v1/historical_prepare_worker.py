"""Reviewed isolated worker: old preflight/resource prep, new assembly, no CLI run."""
import json
from pathlib import Path
import resource
import runpy
import sys
import types
import transport as t


def main(helper,repo,predecessor,current_root,output):
    resource.setrlimit(resource.RLIMIT_AS,(1073741824,1073741824))
    resource.setrlimit(resource.RLIMIT_CPU,(60,60))
    resource.setrlimit(resource.RLIMIT_FSIZE,(67108864,67108864))
    gate,seal=t.anchor(Path(helper));repo=Path(repo);predecessor=Path(predecessor);current_root=Path(current_root);output=Path(output)
    context,references=gate.capture(repo);_,layout,raw=gate.anchored_data()
    old_bodies={**context['predecessor_inputs'],**references}
    gate.verify_rows(old_bodies,layout['files']);gate.closed(predecessor,old_bodies)
    for n,b in old_bodies.items():t.need(gate.regular(predecessor/n)==b,'predecessor view differs')
    gate.closed(current_root,context['current_inputs'])
    for n,b in context['current_inputs'].items():t.need(gate.regular(current_root/n)==b,'current staging differs')
    namespace=runpy.run_path(str(predecessor/gate.SOURCE/'run.py'),run_name='package_unit2_predecessor_only')
    old=namespace['preflight'](predecessor,predecessor/gate.SOURCE)
    t.need(old['current']==context['predecessor_source'] and old['inputs']==context['predecessor_inputs'] and len(old['archived'])==117,'genuine old preflight context')
    expected_resource=t.expected_resource_package(old)
    new_manifest=raw['package-dependency-source-v1.json']
    expected_package=t.derive_current_package(expected_resource,new_manifest)
    expected_assembly,changes,added=t.expected_assembly(context['current_inputs'],expected_package)
    expected_history=t.historical_bindings(references,old['u8_index_resource'],gate)
    t.require_exact_accounting({k:old[k] for k in (t.OLD_KEY,t.DIRECT_KEY)},expected_history)
    new_binding=t.accounting_binding(context,old,gate)
    t.verify_resource_authorities(expected_resource,references,gate)
    compatibility=old['references']['tests/fixtures/typed_project_unit3_compatibility/run.py']
    expected_files={**{'compatibility/typed_project_unit2_independent/'+n:b for n,b in expected_resource.items()},'compatibility/typed_project_unit3_compatibility/run.py':compatibility,**{'derived-package/'+n:b for n,b in expected_package.items()},**{'assembled/'+n:b for n,b in expected_assembly.items()}}
    # Exact bodies and counts are computed before the first worker output write.
    output_budget=sum(map(len,expected_files.values()))+2*1024*1024
    t.need(len(expected_files)+1==462 and output_budget<=24*1024*1024,'worker output budget')
    t.need(output.is_absolute() and output==output.resolve(strict=False) and not output.exists() and output.parent.is_dir(),'fresh output')
    t.need(not any(p.is_symlink() for p in output.parents),'symlink output parent')
    output.mkdir()
    seam=namespace['prepare_unit2'](output,old)
    for key in (t.OLD_KEY,t.DIRECT_KEY):t.need(seam[key]==old[key],'historical prepare accounting changed')
    resource_root=output/'compatibility/typed_project_unit2_independent'
    gate.closed(resource_root,expected_resource)
    for n,b in expected_resource.items():t.need(gate.regular(resource_root/n)==b,'resource package derivation mismatch')
    t.need(gate.regular(output/'compatibility/typed_project_unit3_compatibility/run.py')==compatibility,'compatibility helper differs')
    package=output/'derived-package';gate.materialize_predecessor(package,expected_package)
    # Register only an authenticated protocol in a fresh isolated interpreter.
    # Refuse ambient resolution rather than replacing an existing module.
    t.need('protocol' not in sys.modules,'ambient protocol module')
    protocol_raw=gate.regular(package/'protocol.py');assembler_raw=gate.regular(package/'assemble.py')
    t.need(t.sha(protocol_raw)==seal['protocol_sha256'] and t.sha(assembler_raw)==seal['assembler_sha256'],'assembler/protocol pins')
    protocol=types.ModuleType('protocol');protocol.__file__=str(package/'protocol.py')
    exec(compile(protocol_raw,protocol.__file__,'exec'),protocol.__dict__);sys.modules['protocol']=protocol
    assembler=types.ModuleType('package_unit2_exact_assembler');assembler.__file__=str(package/'assemble.py')
    exec(compile(assembler_raw,assembler.__file__,'exec'),assembler.__dict__)
    t.need(assembler.p is protocol and Path(assembler.p.__file__)==package/'protocol.py','protocol import identity')
    assembly=assembler.assemble(current_root,output/'assembled',package,json.loads(new_manifest))
    actual={n:gate.regular(output/'assembled'/n) for n in expected_assembly}
    gate.closed(output/'assembled',expected_assembly)
    t.validate_transport(context['current_inputs'],expected_package,actual,assembly['instrumentation'],added)
    t.verify_assembler_receipt(assembly,expected_assembly,new_manifest,changes)
    fresh,fresh_refs=gate.capture(repo);gate.require_unchanged_capture(context,references,fresh,fresh_refs)
    for root,bodies in ((predecessor,old_bodies),(current_root,context['current_inputs'])):
        gate.closed(root,bodies)
        for n,b in bodies.items():t.need(gate.regular(root/n)==b,'input changed during assembly')
    report=dict(schema='oxid-package-dependency-unit2-inputs-prepared-v1',status='prepared-inputs-only',compiler_executions=0,semantic_pass=False,execution_qualified=False,current_execution_status='NotReady',current_source=t.row('package-dependency-source-v1.json',new_manifest),source_checkpoint=dict(head=gate.C,tree=gate.CT),historical_source_sha256=gate.OLD_SHA,historical_selected_members=376,historical_archive_members=117,resource_package=[t.row(n,b) for n,b in sorted(expected_resource.items())],derived_package=[t.row(n,b) for n,b in sorted(expected_package.items())],assembled=[t.row(n,b) for n,b in sorted(expected_assembly.items())],instrumentation=changes,added_files=[t.row(n,expected_assembly[n]) for n in added],historical_accounting={t.OLD_KEY:old[t.OLD_KEY],t.DIRECT_KEY:old[t.DIRECT_KEY]},accounting_transport=new_binding,output_file_count=462,output_byte_budget=output_budget,protocol_identity=t.row('protocol.py',protocol_raw),assembler_identity=t.row('assemble.py',assembler_raw))
    report_raw=t.encoded(report);t.need(len(report_raw)<=2*1024*1024,'report budget')
    expected_files['prepared-inputs.json']=report_raw
    with (output/'prepared-inputs.json').open('xb') as stream:stream.write(report_raw)
    gate.closed(output,expected_files)
    for n,b in expected_files.items():t.need(gate.regular(output/n)==b,'closed worker output differs')
    print(json.dumps(dict(status='prepared-inputs-only',files=len(expected_files),bytes=sum(map(len,expected_files.values())),prepared_sha256=t.sha(report_raw),compiler_executions=0,current_execution_status='NotReady'),sort_keys=True))

if __name__=='__main__':
    t.need(len(sys.argv)==6,'exact worker arguments')
    main(*sys.argv[1:])
