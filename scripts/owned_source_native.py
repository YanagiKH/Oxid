"""Independent normalizer of actual native collector artifacts; no execution.

This is a separate draft after the frozen adapter review snapshot. It verifies
outcomes/store prefixes, not all-store CFG dominance or resource admission.
"""
from pathlib import Path
import argparse,json,re,hashlib,sys
import owned_source_model as m
import verify_owned_source as h


def strict_json(path):
    return h.strict_json_loads(Path(path).read_text())


def same(want,observed,label):
    if not h.RawInspector.equal(want,observed):raise ValueError(label+': '+repr(observed)+' != '+repr(want))


def bits_value(bits, ty):
    if type(bits) is not int or not 0<=bits<(1<<64):raise ValueError('native store bits must be u64')
    if ty=='i32':
        bits &= (1<<32)-1
        return bits-(1<<32) if bits >= (1<<31) else bits
    if ty=='bool':
        if bits not in (0,1):raise ValueError('non-boolean stored bits')
        return bool(bits)
    if ty=='()':
        if bits:raise ValueError('nonzero unit stored bits')
        return None
    raise ValueError('unknown source scalar payload type')


def validate_physical_bindings(raw,catalog,enrichment):
    """Reconstruct physical joins from immutable LLVM and raw authority tables.

    Layout assumptions are the frozen private ABI: declaration-order fields,
    i32 width/alignment4, bool/unit width/alignment1, scalar slots of8 bytes.
    No producer enrichment value is used as its own expected physical value.
    """
    same(catalog['llvm_sha256'],h.digest(catalog['llvm_path']),'physical LLVM identity')
    lines=Path(catalog['llvm_path']).read_bytes().decode().splitlines();definitions={};function_lines={};function=None
    for line_number,line in enumerate(lines,1):
        match=re.match(r'define internal .* @__oxid_owned_fn_(\d+)\(',line)
        if match:function=int(match[1]);function_lines[function]=[]
        if function is not None:
            function_lines[function].append(line)
            match=re.fullmatch(r'  (%\w+) = getelementptr i8, ptr (%\w+), i64 (\d+)',line)
            if match:definitions[function,match[1]]={'base':match[2],'byte_offset':int(match[3]),'llvm_line':line_number,'llvm_text':line.strip()}
        if line=='}':function=None
    if type(catalog['stores']) is not list or type(enrichment['sites']) is not list:raise ValueError('physical sites must be arrays')
    sites={s['site']:s for s in catalog['stores']};extra={s['site']:s for s in enrichment['sites']}
    if len(sites)!=len(catalog['stores']) or len(extra)!=len(enrichment['sites']):raise ValueError('duplicate physical site')
    same(list(range(len(sites))),[s['site'] for s in catalog['stores']],'canonical physical site identities')
    same(set(sites),set(extra),'physical enrichment inventory')
    functions={f['id']:f for f in raw['functions']};records={r['id']:r for r in raw['records']}
    for identity,site in sites.items():
        enriched=extra[identity]
        if type(site['site']) is not int or type(enriched['site']) is not int or type(site['function']) is not int:
            raise ValueError('physical site/function identities must be integers')
        same(site['site'],enriched['site'],'physical site identity')
        same(site['function'],enriched['function'],'physical function identity')
        same(site['type'],enriched['wire_type'],'physical wire type')
        semantics='occurrence-only' if site['type']=='ptr' else 'unsigned-stored-bits'
        if site['type'] not in ('ptr','i1','i8','i32','i64'):raise ValueError('unknown physical store wire type')
        same(semantics,site['value_semantics'],'catalog value semantics');same(semantics,enriched['value_semantics'],'enriched value semantics')
        if type(site['llvm_line']) is not int or not 1<=site['llvm_line']<=len(lines):raise ValueError('invalid physical store line')
        same(lines[site['llvm_line']-1].strip(),site['store_text'],'actual physical store text')
        store=re.fullmatch(r'store (ptr|i1|i8|i32|i64) .+, ptr (%\w+), align \d+',site['store_text'])
        if not store:raise ValueError('unsupported physical store syntax')
        same(store[1],site['type'],'actual physical store type');same(store[2],site['pointer'],'actual physical destination')
        gep=definitions.get((site['function'],site['pointer']))
        same(gep,enriched['actual_destination_gep'],'actual destination GEP definition')
        record_id=enriched['record']
        if record_id is not None:
            if type(record_id) is not int or record_id not in records:raise ValueError('unknown physical record identity')
            record=records[record_id];offset=0;layout={}
            for field in record['fields']:
                width={'i32':4,'bool':1,'()':1}[field['type']]
                offset=(offset+width-1)//width*width;layout[offset]=field;offset+=width
            if record['fields']:
                if gep is None or gep['byte_offset'] not in layout:raise ValueError('payload destination outside record layout')
                field=layout[gep['byte_offset']]
                same(gep['byte_offset'],enriched['payload_byte_offset'],'payload byte offset')
                same(field['id'],enriched['field'],'physical nominal field');same(field['span'],enriched['field_span'],'physical field origin')
                same(field['type'],enriched['semantic_type'],'physical field scalar type')
                same({'i32':'i32','bool':'i1','()':'i8'}[field['type']],site['type'],'physical field wire type')
                same(False,enriched['empty_record_token'],'nonempty payload marker')
            else:
                for key,wanted in [('field',None),('field_span',None),('semantic_type',None),('payload_byte_offset',0),('empty_record_token',True),('wire_type','i8')]:same(wanted,enriched[key],'empty payload '+key)
        else:
            for key,wanted in [('field',None),('field_span',None),('payload_byte_offset',None),('empty_record_token',False)]:same(wanted,enriched[key],'scalar/pointer '+key)
            if site['type']=='ptr':same(None,enriched['semantic_type'],'pointer semantic type')
        fn=functions[site['function']];kind=site['kind'];raw_op=site['raw_operation'];instruction=None
        if raw_op['kind']=='statement':
            block=next(b for b in fn['blocks'] if b['id']==site['raw_block']);event=block['instructions'][raw_op['index']];instruction=event['instruction']
            expected_kind='Scalar('+instruction['scalar']['kind']+')' if instruction['operation']=='Scalar' else instruction['operation']
            same(expected_kind,kind,'physical source operation class');same(event['span'],site['charge_span'],'physical source charge span')
        elif raw_op['kind']=='terminator':
            block=next(b for b in fn['blocks'] if b['id']==site['raw_block']);event=block['terminator'];instruction=event['instruction']
            same(instruction['operation'],kind,'physical terminator class');same(event['span'],site['charge_span'],'physical terminator origin')
        elif raw_op['kind']=='parameter':same('ParameterCopy',kind,'physical parameter class')
        elif raw_op['kind']=='merge':same('BoolMerge',kind,'physical merge class')
        else:raise ValueError('unknown physical raw-operation class')
        if kind=='Scalar(Store)':
            place=fn['places'][instruction['scalar']['place']]
            same(place['type'],enriched['semantic_type'],'scalar store semantic type');same('i64',site['type'],'scalar store slot width')
            if gep is None:raise ValueError('scalar store lost destination GEP')
            same('%scalars',gep['base'],'scalar store array');same(8*(len(fn['locals'])+place['id']),gep['byte_offset'],'scalar place slot offset')
        elif kind=='WriteField':
            same(instruction['field'],enriched['field'],'source field store identity')
            if gep is None:raise ValueError('field store lost destination GEP')
            base=instruction['base']
            if base['mode']=='owner':same('%o'+str(base['id']),gep['base'],'field owner destination')
            else:
                expected=f"  {gep['base']} = load ptr, ptr %r{base['id']}, align 8"
                if expected not in function_lines[site['function']]:raise ValueError('field destination does not dereference authoritative reference slot')
        elif kind=='Replace':
            owner=fn['owners'][instruction['destination']];same(owner['record'],record_id,'whole-store destination record')
            if gep is None:raise ValueError('whole store lost destination GEP')
            same('%o'+str(owner['id']),gep['base'],'whole-store destination owner')
    return sites


def validate_wire_events(events,sites):
    if type(events) is not list:raise ValueError('native store events must be an array')
    for event in events:
        if type(event) is not dict or set(event)!={'site','bits'} or type(event['site']) is not int or event['site'] not in sites:raise ValueError('unknown physical store event')
        bits=event['bits'];wire=sites[event['site']]['type']
        if type(bits) is not int or bits<0:raise ValueError('store bits must be a nonnegative integer')
        if wire=='ptr':
            if bits!=0:raise ValueError('pointer observation reports occurrence only')
        elif bits >= 1<<int(wire[1:]):raise ValueError('store event exceeds physical wire width')



def semantic_stores(item,raw,catalog,enrichment,events):
    sites=validate_physical_bindings(raw,catalog,enrichment)
    validate_wire_events(events,sites)
    return project_semantic_stores(item,raw,catalog,enrichment,events)


def project_semantic_stores(item,raw,catalog,enrichment,events):
    sites={s['site']:s for s in catalog['stores']}; enriched={s['site']:s for s in enrichment['sites']}
    if len(sites)!=len(catalog['stores']) or len(enriched)!=len(enrichment['sites']):raise ValueError('duplicate physical store site')
    same(set(sites),set(enriched),'enrichment site inventory')
    functions={f['id']:f for f in raw['functions']}; records={r['id']:r for r in raw['records']}
    source=item['source'].encode(); result=[];pending=None
    for event in events:
        if set(event)!={'site','bits'} or type(event['site']) is not int or event['site'] not in sites:raise ValueError('unknown store observation')
        site=sites[event['site']]; extra=enriched[event['site']];kind=site['kind']
        if kind not in ('Scalar(Store)','WriteField','Replace'):continue
        function=functions[site['function']];name=source[slice(*function['span'])].decode()
        block=next(b for b in function['blocks'] if b['id']==site['raw_block'])
        origin=site['raw_operation'];same('statement',origin['kind'],'semantic store control point')
        raw_event=block['instructions'][origin['index']]; ins=raw_event['instruction'];span=site['charge_span']
        same(span,raw_event['span'],'physical store charge origin')
        fact=next(s for s in item['facts']['stores'] if s['function']==name and s['charge_span']==span and s['kind']!='let')
        if kind=='Scalar(Store)':
            same('Scalar',ins['operation'],'scalar store operation');same('Store',ins['scalar']['kind'],'scalar store kind')
            same(fact['target_type'],extra['semantic_type'],'scalar store type')
            result.append({'kind':'store-scalar','span':span,'value':bits_value(event['bits'],fact['target_type'])})
        elif kind=='WriteField':
            same('WriteField',ins['operation'],'field store operation');same(ins['field'],extra['field'],'field store identity')
            record=records[ins['field']['record']];field=record['fields'][ins['field']['index']]
            same(field['span'],extra['field_span'],'field origin');same(field['type'],extra['semantic_type'],'field type')
            result.append({'kind':'field-write','span':span,'field':source[slice(*field['span'])].decode(),'value':bits_value(event['bits'],field['type'])})
        else:
            same('Replace',ins['operation'],'whole store operation')
            owner=next(o for o in function['owners'] if o['id']==ins['destination']);record=records[owner['record']]
            same(owner['record'],extra['record'],'whole-store nominal record')
            if not record['fields']:
                same(True,extra['empty_record_token'],'empty whole store token');same(0,event['bits'],'empty token bits')
                result.append({'kind':'replace-owned','span':span,'fields':[]});continue
            key=(site['function'],site['raw_block'],origin['index'])
            if pending is None:pending={'key':key,'span':span,'fields':{},'count':len(record['fields'])}
            same(key,pending['key'],'interleaved partial whole store')
            field=extra['field'];same(owner['record'],field['record'],'whole-store field record')
            index=field['index'];decl=record['fields'][index]
            same(decl['span'],extra['field_span'],'whole-store field origin');same(decl['type'],extra['semantic_type'],'whole-store field type')
            if index in pending['fields']:raise ValueError('duplicate payload field store')
            pending['fields'][index]=[source[slice(*decl['span'])].decode(),bits_value(event['bits'],decl['type'])]
            if len(pending['fields'])==pending['count']:
                result.append({'kind':'replace-owned','span':span,'fields':[pending['fields'][i] for i in range(pending['count'])]});pending=None
    if pending:raise ValueError('native failure occurred after partial whole store')
    return result


def native_outcome(item,row,source_name):
    budget=row['budget']
    if type(budget) is not int or budget<0:raise ValueError('native budget must be a nonnegative integer')
    expected=h.expected_budget_row(item,budget);native=row['native']
    same(expected['reference'],row['reference'],'reference outcome')
    failure=expected['reference']['failure'];code=None if failure is None else failure['code']
    stdout='' if failure else h.scalar_text(expected['reference']['result_type'],expected['reference']['result']).decode()
    stderr='';location=None
    if failure:
        before=item['source'].encode()[:failure['span'][0]].decode();line=before.count('\n')+1;column=len(before.rsplit('\n',1)[-1])+1
        location=f'{source_name}:{line}:{column}'
        message={'E0601':'execution fuel exhausted','E0604':'checked i32 arithmetic overflow'}[code]
        stderr=f'error[{code}] (oir-run): {message}\n  --> {location}\n'
    same(1 if failure else 0,native['status'],'native terminal status');same(stdout,native['stdout'],'native stdout')
    same(stderr,native['diagnostic_stderr'],'native diagnostics');same(code,native['code'],'native code');same(location,native['source_location'],'native location')
    same(True,native['source_free'],'source-free observation');same('env_clear; PATH=/no-tools',native['environment'],'source-free environment')
    parsed=[];diagnostic=[]
    for line in native['stderr'].splitlines(keepends=True):
        match=re.fullmatch(r'__UNIT4B_STORE (\d+) (\d+)\n',line)
        if match:parsed.append({'site':int(match[1]),'bits':int(match[2])})
        else:diagnostic.append(line)
    same(stderr,''.join(diagnostic),'actual native diagnostic bytes');same(native['stores'],parsed,'probe stderr/store correspondence')
    if native['instrumented'] is not True:same([],parsed,'ordinary module wrote probe output')
    charges=[{'cost':e['cost'],'span':e['span']} for e in row['reference_events'] if e['kind']=='Charge']
    want=[{'cost':e['cost'],'span':item['origins'][e['origin']]} for e in item['schedule']['items'] if e['end_fuel']<=budget]
    same(want,charges,'successful reference charge prefix')
    same(True,row['reference_native_equal'],'reference/native parity metadata')
    return expected


def required_case_files(identifier,total):
    names={'COMPLETE','admission.json','frames.json','operations.json','stores.json','raw-view.json','receipts.jsonl',identifier+'.ox',
           'bounded.ll','argv.ll','probe-original.ll','probe-original-stripped.ll','probe.ll','probe-stripped.ll','production-prefix.ll.txt','default.ll',
           'argv.elf','probe.elf','default.elf','default.json','default.stdout','default.stderr'}
    for prefix in ('budget','probe'):
        for budget in range(total+1):
            names.update(f'{prefix}-{budget}.{suffix}' for suffix in ('json','stdout','stderr'))
    for budget in (0,total-1,total):names.update(f'fixed-{budget}.{suffix}' for suffix in ('elf','ll','json','stdout','stderr'))
    return names


def receipt_identity(directory,row,name,budget,instrumented,artifact,arguments):
    same(name,row['name'],'receipt name');same(budget,row['budget'],'receipt budget')
    native=row['native']
    for key,value in [('name',name),('artifact',artifact),('run_directory',name+'-run'),('instrumented',instrumented),('arguments',arguments)]:
        same(value,native[key],'native execution '+key)
    same((directory/(name+'.stdout')).read_bytes(),native['stdout'].encode(),'captured native stdout bytes')
    same((directory/(name+'.stderr')).read_bytes(),native['stderr'].encode(),'captured native stderr bytes')
    execution=directory/(name+'-run')
    if execution.is_symlink() or not execution.is_dir():raise ValueError('missing real source-free run directory')
    if any(p.suffix=='.ox' for p in execution.rglob('*')):raise ValueError('native execution directory contains source')


def compare_native_case(item,program,directory):
    directory=Path(directory);catalog=strict_json(directory/'catalog.json');enrichment=strict_json(directory/'payload-enrichment.json')
    same(item['source_sha256'],catalog['source_sha256'],'catalog source identity')
    required=required_case_files(item['id'],item['schedule']['total_fuel'])
    same(required,set(catalog['files']),'complete native artifact hash inventory')
    same(required|{'catalog.json','payload-enrichment.json'},{p.name for p in directory.iterdir() if p.is_file()},'native direct-file inventory')
    same(item['source_sha256'],h.digest(catalog['source_path']),'original source bytes')
    same(item['source_sha256'],h.digest(directory/(item['id']+'.ox')),'canonical copied source bytes')
    for stem,name in [('raw_witness','raw-view.json'),('llvm','bounded.ll'),('argv_llvm','argv.ll'),('probe_llvm','probe-original.ll'),('argv_probe_llvm','probe.ll')]:
        same(catalog[stem+'_sha256'],h.digest(directory/name),'canonical '+stem+' identity')
        same(catalog[stem+'_sha256'],h.digest(catalog[stem+'_path']),'declared '+stem+' identity')
    catalog={**catalog,'llvm_path':str(directory/'bounded.ll')}
    for name,sha in catalog['files'].items():
        if Path(name).name!=name or (directory/name).is_symlink():raise ValueError('catalog file is not a direct regular artifact')
        same(sha,h.digest(directory/name),'artifact '+name)
        if name.endswith('.elf'):
            head=(directory/name).read_bytes()[:64]
            if len(head)!=64 or head[:7]!=b'\x7fELF\x02\x01\x01' or head[18:20]!=b'\x3e\x00' or head[16:18] not in (b'\x02\x00',b'\x03\x00'):
                raise ValueError('artifact is not an x86_64 ELF executable: '+name)
    for key,file in [('catalog_sha256','catalog.json'),('llvm_sha256','bounded.ll'),('raw_witness_sha256','raw-view.json')]:same(enrichment[key],h.digest(directory/file),'enrichment '+key)
    same(catalog['compiler']['sha256'],h.digest(catalog['compiler']['binary']),'native collector binary')
    raw=strict_json(directory/'raw-view.json');h.validate_raw_variants(raw,len(item['source'].encode()));checked=m.NamesTypes(program).check()
    inspection=h.RawInspector(item,item['source'],raw,checked).inspect()
    if inspection['issues']:raise ValueError('native raw/source correspondence mismatch: '+repr(inspection['issues']))
    bounded=(directory/'bounded.ll').read_bytes().decode();probe=(directory/'probe-original.ll').read_bytes().decode()
    same(bounded,''.join(line for line in probe.splitlines(keepends=True) if not line.rstrip().endswith('; UNIT4B_PROBE')),'stripped probe module')
    def bodies(text):return [b for b in re.findall(r'^define [^\n]+\n.*?^\}',text,re.M|re.S) if not re.match(r'^define [^\n]*@main\(',b)]
    same(bodies(bounded),bodies((directory/'argv.ll').read_bytes().decode()),'ordinary function body byte identity')
    for name in ['default.ll']+[f'fixed-{budget}.ll' for budget in (0,item['schedule']['total_fuel']-1,item['schedule']['total_fuel'])]:
        same(bodies(bounded),bodies((directory/name).read_bytes().decode()),'fixed/default ordinary body bytes')
    physical=validate_physical_bindings(raw,catalog,enrichment)
    rows=[];total=item['schedule']['total_fuel'];count=0
    for budget in range(total+1):
        ordinary=strict_json(directory/f'budget-{budget}.json');probe_row=strict_json(directory/f'probe-{budget}.json')
        for row,instrumented in [(ordinary,False),(probe_row,True)]:
            prefix='probe' if instrumented else 'budget'
            receipt_identity(directory,row,f'{prefix}-{budget}',budget,instrumented,'probe.elf' if instrumented else 'argv.elf',[str(budget)])
            same(budget,row['budget'],'ordered budget');same(instrumented,row['native']['instrumented'],'module observation kind')
            native_outcome(item,row,item['id']+'.ox');count+=1
        validate_wire_events(probe_row['native']['stores'],physical)
        stores=project_semantic_stores(item,raw,catalog,enrichment,probe_row['native']['stores'])
        rows.append({'budget':budget,'reference':ordinary['reference'],'committed_stores':stores})
    prefixes=h.compare_budget_receipts(item,rows,require_stores=True)
    if prefixes['issues']:raise ValueError('committed store prefix mismatch: '+repr(prefixes['issues'][:2]))
    same(1000000,catalog['admission']['default_fuel'],'ordinary default fuel contract')
    default=strict_json(directory/'default.json');receipt_identity(directory,default,'default',1000000,False,'default.elf',[])
    native_outcome(item,default,item['id']+'.ox');count+=1
    for budget in (0,total-1,total):
        fixed=strict_json(directory/f'fixed-{budget}.json');receipt_identity(directory,fixed,f'fixed-{budget}',budget,False,f'fixed-{budget}.elf',[]);native_outcome(item,fixed,item['id']+'.ox');count+=1
    return {'id':item['id'],'budgets':total+1,'actual_process_observations':count,'raw_fact_checks':inspection['checks'],'outcomes_and_committed_stores':'MATCH',
            'catalog_sha256':h.digest(directory/'catalog.json'),'payload_enrichment_sha256':h.digest(directory/'payload-enrichment.json'),
            'compiler_sha256':catalog['compiler']['sha256'],'all_store_CFG_guard_audit':'SEPARATE','resource_admission':'SEPARATE','full_qualification':False}


NATIVE_IDS=('guarded-overflow-after-write','guarded-replace-self','guarded-replace-relay','guarded-owned-result-assign',
            'guarded-empty-between-scalars','guarded-scalar-store','batch-overflow-after-earlier-write','batch')

def verify_native_collection(root, fixture):
    root=Path(root); collection=strict_json(root/'collection-manifest.json');data=strict_json(fixture)
    same(list(NATIVE_IDS),[row['id'] for row in data['cases']],'complete independently frozen native inventory')
    if collection.get('schema') == 'unit4b-final-native-production-receipts-v2':
        same(data['pre_observation_manifest_sha256'],collection['frozen_source_manifest_sha256'],'native source freeze identity')
    elif collection.get('schema') == 'owned-source-native-collection-v1':
        same(h.digest(fixture),collection['specification_sha256'],'native source specification identity')
        frozen_path=Path(collection['frozen_manifest_path'])
        same(h.digest(frozen_path),collection['frozen_manifest_sha256'],'portable source manifest identity')
        frozen=strict_json(frozen_path)
        same(data['model_sha256'],frozen['model_sha256'],'portable source model identity')
        same(h.digest(fixture),frozen['specification_sha256'],'portable source specification')
        expected_rows=[{key:row[key] for key in ('id','source_sha256','expectation_sha256','budget_count','fuel')} for row in data['cases']]
        same(expected_rows,frozen['files'],'complete portable source freeze inventory')
        for row in expected_rows:
            same(row['source_sha256'],h.digest(frozen_path.parent/(row['id']+'.ox')),'portable original source')
            same(row['expectation_sha256'],h.digest(frozen_path.parent/(row['id']+'.json')),'portable source expectation')
    else:raise ValueError('unsupported native collection schema')
    same(collection['compiler_manifest_sha256'],h.digest(collection['compiler_manifest_path']),'compiler manifest identity')
    entries=collection['cases'];expected={(profile,identifier) for profile in ('debug','release') for identifier in NATIVE_IDS}
    actual=[(row['profile'],row['id']) for row in entries]
    if len(actual)!=len(expected) or set(actual)!=expected:raise ValueError('native collection must contain both complete unique profile inventories')
    for row in entries:
        directory=root/row['profile']/row['id']
        same(row['catalog_sha256'],h.digest(directory/'catalog.json'),'collected catalog identity')
        same(row['payload_enrichment_sha256'],h.digest(directory/'payload-enrichment.json'),'collected payload identity')
    return collection


def compare_native_collection(root, fixture, profile):
    root=Path(root);fixture=Path(fixture);collection=verify_native_collection(root,fixture)
    rows=strict_json(fixture)['cases'];expected=h.native_probe_expectations(fixture);reports=[]
    if profile not in ('debug','release'):raise ValueError('unknown native profile')
    for row,(source,item) in zip(rows,expected):
        item={**item,'source':source};report=compare_native_case(item,h.decode_source_program(row['program']),root/profile/item['id']);reports.append(report);print(report['id'],report['budgets'],'MATCH',flush=True)
    return {'scope':'independent actual native/reference outcomes and semantic committed-store prefixes; CFG/resource/full CLI qualification remains separate',
            'model_sha256':h.digest(Path(m.__file__)),'adapter_sha256':h.digest(Path(h.__file__)),'native_matcher_sha256':h.digest(Path(__file__)),
            'native_fixture_sha256':h.digest(fixture),'collection_manifest_sha256':h.digest(root/'collection-manifest.json'),'profile':profile,'cases':reports,'full_qualification':False}


def main():
    parser=argparse.ArgumentParser();parser.add_argument('root',type=Path);parser.add_argument('output',type=Path);parser.add_argument('--profile',default='debug');args=parser.parse_args()
    fixture=Path(h.__file__).resolve().parents[1]/'tests/fixtures/owned_source/native-probe-sources.json'
    h.write_json(args.output,compare_native_collection(args.root,fixture,args.profile))

if __name__=='__main__':main()
