#!/usr/bin/env python3
"""Independent artifact binding and producer payload catalogue packaging.

The check function is preserved from the reviewed file/body/argv/source-free
audit; enrichment preserves the original collector-side GEP/raw join. Neither
replaces the independent semantic matcher or complete source-store guard audit.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import owned_source_native_audit as a


def file_hash(path):
    path = Path(path)
    h=hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda:f.read(1024*1024),b''):
            h.update(chunk)
    return h.hexdigest()


def check(catalog, model_dir):
    c=json.loads(catalog.read_text())
    root=catalog.parent
    counts=Counter()
    for filename,expected in c['files'].items():
        path=root/filename
        a.require(path.parent==root and path.is_file(),'manifest must list direct existing files')
        a.require(file_hash(path)==expected,f'manifest hash differs: {path}')
        counts['hashed_files']+=1
    _,bounded=a.read_bound(c,'llvm',root)
    _,argv=a.read_bound(c,'argv_llvm',root)
    _,probe=a.read_bound(c,'probe_llvm',root)
    _,argv_probe=a.read_bound(c,'argv_probe_llvm',root)
    header=b'define i32 @main() {\nentry:\n'
    argv_header=b'declare i64 @strtoull(ptr, ptr, i32)\ndefine i32 @main(i32 %argc, ptr %argv) {\nentry:\n  %test_arg_slot = getelementptr ptr, ptr %argv, i64 1\n  %test_arg = load ptr, ptr %test_arg_slot\n  %test_parsed_fuel = call i64 @strtoull(ptr %test_arg, ptr null, i32 10)\n  %test_over_cap = icmp ugt i64 %test_parsed_fuel, 1000000\n  %test_fuel = select i1 %test_over_cap, i64 1000000, i64 %test_parsed_fuel\n'
    initial=re.search(rb'store i64 (\d+), ptr %fuel, align 8',bounded)
    a.require(initial is not None and bounded.count(header)==1,'missing unique guarded wrapper')
    old_store=initial[0]
    expected_argv=bounded.replace(header,argv_header,1).replace(old_store,b'store i64 %test_fuel, ptr %fuel, align 8',1)
    a.require(argv==expected_argv,'argv wrapper makes changes beyond argument parse/cap and initial fuel store')
    a.require(argv_probe==probe.replace(header,argv_header,1).replace(old_store,b'store i64 %test_fuel, ptr %fuel, align 8',1),'argv probe makes additional changes')
    strip=lambda data:b''.join(l for l in data.splitlines(keepends=True) if not l.rstrip().endswith(b'; UNIT4B_PROBE'))
    a.require(strip(probe)==bounded and strip(argv_probe)==argv,'probe stripping does not restore each corresponding entire module')
    prefix=bounded.split(header,1)[0]
    body_hashes={}
    for filename in ('default.ll','bounded.ll','argv.ll',*sorted(p.name for p in root.glob('fixed-*.ll'))):
        data=(root/filename).read_bytes()
        actual_prefix=data.split(argv_header if filename=='argv.ll' else header,1)[0]
        a.require(actual_prefix==prefix,f'{filename}: production function/global prefix differs')
        body_hashes[filename]=a.sha(actual_prefix)
        if filename!='argv.ll':
            fuel=re.search(rb'store i64 (\d+), ptr %fuel, align 8',data)
            a.require(fuel is not None and data.replace(fuel[0],old_store,1)==bounded,f'{filename}: changes beyond fixed initial wrapper fuel')
    expected=json.loads((model_dir/(root.name+'.json')).read_text())
    a.require(expected['source_sha256']==c['source_sha256'],'source differs from independent expected model')
    by_budget={r['budget']:r['reference'] for r in expected['native_budget_expectations']}
    selected_budgets=set()
    probe_budgets=set()
    site_ids={r['site'] for r in c['stores']}
    a.require(len(site_ids)==len(c['stores']),'duplicate physical observation sites')
    receipts=[]
    elf_hashes={}
    for path in sorted(root.glob('*.elf')):
        with path.open('rb') as f:
            a.require(f.read(4)==b'\x7fELF',f'not an ELF: {path}')
        elf_hashes[path.name]=file_hash(path)
    for line in (root/'receipts.jsonl').read_text().splitlines():
        r=json.loads(line);native=r['native'];name=r['name'];budget=r['budget']
        a.require(json.loads((root/(name+'.json')).read_text())==r,'per-run receipt differs from stream')
        a.require(native['stdout']==(root/(name+'.stdout')).read_text() and native['stderr']==(root/(name+'.stderr')).read_text(),'receipt differs from physical stdout/stderr')
        run=root/native['run_directory']
        members=list(run.iterdir())
        a.require(len(members)==1 and members[0].name=='program.elf','run directory is not source-free one-ELF directory')
        executable=root/native['artifact']
        # Native collector hardlinks precisely its previously built ELF.
        a.require(members[0].samefile(executable),'run ELF differs from cited compiled artifact')
        a.require(native['source_free'] is True and native['environment']=='env_clear; PATH=/no-tools','source-free process controls missing')
        a.require(all(s['site'] in site_ids for s in native['stores']),'observation cites unknown store site')
        a.require(native['instrumented']==name.startswith('probe-'),'probe/nonprobe classification differs')
        if not native['instrumented']:
            a.require(not native['stores'] and '__UNIT4B_STORE' not in native['stderr'],'ordinary output contains observation instrumentation')
        if name.startswith('budget-'):
            selected_budgets.add(budget)
            counts['ordinary_budget_executions']+=1
        elif name.startswith('probe-'):
            probe_budgets.add(budget)
            counts['probe_budget_executions']+=1
        elif name=='default':
            counts['production_default_executions']+=1
        elif name.startswith('fixed-'):
            counts['production_fixed_executions']+=1
        else:
            raise a.AuditFailure(f'unknown receipt kind {name}')
        if budget in by_budget:
            want=by_budget[budget]
            a.require(r['reference']==want,f'{name}: actual reference differs from frozen independent expected outcome')
            failure=want['failure']
            if failure:
                a.require(native['status']==1 and native['stdout']=='' and native['code']==failure['code'],f'{name}: native failure differs from frozen independent outcome')
            else:
                value='()' if want['result_type']=='()' else str(want['result']).lower()
                a.require(native['status']==0 and native['stdout']==value+'\n' and native['diagnostic_stderr']=='',f'{name}: native success differs from frozen independent outcome')
        counts['source_free_receipts']+=1
        receipts.append(name)
    a.require(len(receipts)==len(set(receipts)),'duplicate process receipt names')
    a.require(selected_budgets==set(by_budget),'ordinary native sweep is missing a frozen expected budget')
    a.require(probe_budgets==set(by_budget),'observation sweep is missing a frozen expected budget')
    return dict(status='pass',catalog=str(catalog),catalog_sha256=file_hash(catalog),model_sha256=file_hash(model_dir/(root.name+'.json')),counts=dict(counts),production_prefix_sha256=a.sha(prefix),same_body_artifacts=body_hashes,elf_hashes=elf_hashes,limitation='Execution evidence is the reviewed native collector plus preserved receipts and source-free run directories; this audit does not re-execute binaries. Observation modules remain separate from unchanged production-body claims.')



def enrich(folder):
    """Write the original producer join for independent matcher scrutiny."""
    folder = Path(folder)
    sha = file_hash
    catalog=json.loads((folder/'catalog.json').read_text())
    raw=json.loads(Path(catalog['raw_witness_path']).read_text())
    module=Path(catalog['llvm_path']);geps={};function=None
    for lineno,line in enumerate(module.read_text().splitlines(),1):
        match=re.match(r'define internal .* @__oxid_owned_fn_(\d+)\(',line)
        if match: function=int(match[1]);geps[function]={}
        if line=='}': function=None
        match=re.match(r'  (%\w+) = getelementptr i8, ptr (%\w+), i64 (\d+)$',line)
        if function is not None and match: geps[function][match[1]]=dict(base=match[2],byte_offset=int(match[3]),llvm_line=lineno,llvm_text=line.strip())
    layouts={}
    for record in raw['records']:
        offset=0;mapping={}
        for field in record['fields']:
            size=4 if field['type']=='i32' else 1
            offset += (-offset)%size
            mapping[offset]=field;offset+=size
        layouts[record['id']]=mapping
    enriched=[]
    for store in catalog['stores']:
        f=raw['functions'][store['function']];op=store['raw_operation'];index=op['index']
        record=None;expected_field=None;semantic=None;inst=None;parameter=None
        if op['kind']=='parameter':
            parameter=f['parameters'][index]
            if parameter['mode']=='owned': record=f['owners'][parameter['id']]['record']
            elif parameter['mode']=='scalar': semantic=parameter['type']['type']
        elif op['kind']=='statement': inst=f['blocks'][store['raw_block']]['instructions'][index]['instruction']
        elif op['kind']=='terminator': inst=f['blocks'][store['raw_block']]['terminator']['instruction']
        else: assert op['kind']=='merge';semantic='bool'
        if inst:
            kind=inst['operation']
            if kind=='Construct': record=f['owners'][inst['destination']]['record']
            elif kind in ['Replace','MoveInitialize','PrepareOwned']: record=f['owners'][inst['source']]['record']
            elif kind=='ReturnOwned': record=f['owners'][inst['owner']]['record']
            elif kind=='WriteField': record=inst['field']['record'];expected_field=inst['field']
            elif kind=='ReadField': semantic=f['locals'][inst['destination']]['type']
            elif kind=='PrepareScalar': semantic=f['locals'][inst['value']['local']]['type']
            elif kind=='Scalar':
                scalar=inst['scalar']
                semantic=f['locals'][scalar['destination']]['type'] if scalar['kind']=='Assign' else f['places'][scalar['place']]['type']
            elif kind=='Invoke':
                result=f['calls'][inst['call']]['result']
                assert result['mode']=='scalar';semantic=f['locals'][result['id']]['type']
        field=None;offset=None;gep=geps[store['function']].get(store['pointer'])
        if record is not None:
            if layouts[record]:
                assert gep,(folder,store)
                offset=gep['byte_offset'];field=layouts[record][offset]
                assert {'i32':'i32','bool':'i1','()':'i8'}[field['type']]==store['type']
                if expected_field: assert field['id']==expected_field
                if inst and inst['operation']=='Construct': assert field['id'] in [item['field'] for item in inst['fields']]
                semantic=field['type']
            else:
                assert store['type']=='i8';offset=0
        enriched.append(dict(site=store['site'],function=store['function'],record=record,field=None if field is None else field['id'],
                             field_span=None if field is None else field['span'],payload_byte_offset=offset,actual_destination_gep=gep,
                             semantic_type=semantic,empty_record_token=record is not None and not layouts[record],
                             wire_type=store['type'],value_semantics=store['value_semantics']))
    result=dict(schema='unit4b-payload-enrichment-v1',catalog_sha256=sha(folder/'catalog.json'),
                llvm_sha256=sha(module),raw_witness_sha256=sha(catalog['raw_witness_path']),
                basis='Actual destination GEP byte offsets joined to immutable nominal raw declaration layout; no source expectations consumed',sites=enriched)
    path=folder/'payload-enrichment.json'
    assert not path.exists(),path
    path.write_text(json.dumps(result,indent=2)+'\n')
