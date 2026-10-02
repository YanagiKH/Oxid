#!/usr/bin/env python3
"""Normalize actual observer events using only source-only requests and raw data.

No expected/model module or file is read by this adapter. Targets always join
actual query/checker IDs to actual collected declaration events. Source text is
used only to label those emitted origins and immutable loader relationships.
"""
from pathlib import Path
import argparse, datetime, hashlib, json


def key(origin):
    return origin['file'],origin['start'],origin['end']


def normalize(raw, request, entry):
    files={}
    for f in raw['sources']:
        rel=Path(f['path']).relative_to(Path(entry).parent).as_posix()
        content=f['text'].encode('utf-8')
        assert len(content)==f['bytes']
        files[f['file']]={'path':rel,'bytes':content}
    advertised={r['path']:r for r in request['source_files']}
    for f in files.values():
        ad=advertised[f['path']]
        assert len(f['bytes'])==ad['bytes'] and hashlib.sha256(f['bytes']).hexdigest()==ad['sha256']

    def origin(span):
        if span is None:return None
        file=span.get('file',span.get('file_id'))
        data=files[file]['bytes'];start,end=span['start'],span['end']
        assert 0<=start<=end<=len(data)
        data[:start].decode();data[:end].decode()
        return {'file':files[file]['path'],'start':start,'end':end}

    def spelling(span):
        file=span.get('file',span.get('file_id'))
        origin(span)
        return files[file]['bytes'][span['start']:span['end']].decode()

    def diagnostic(d,phase,order):
        return {'code':d['code'],'stage':d['stage'],'phase':phase,'order':order,
                'primary':origin(d['primary']),'secondary':[origin(s['span']) for s in d['secondary']],
                'message':d['message'],'labels':[s['message'] for s in d['secondary']],'notes':d['notes']}

    events=raw.get('observations',[])
    if any(e['event']=='unmapped' for e in events):
        raise AssertionError('Unmapped observation variants cannot silently disappear')
    phase='collection';phase_events=[];observed_diagnostics=[]
    for event in events:
        if event['event']=='phase':phase=event['phase'];phase_events.append(phase)
        elif event['event']=='diagnostic':observed_diagnostics.append((event['diagnostic'],event['phase']))
    returned=raw.get('diagnostics',[])
    diagnostics_out=[];position=0
    for d in returned:
        if position<len(observed_diagnostics) and observed_diagnostics[position][0]==d:
            dphase=observed_diagnostics[position][1];position+=1
        else:
            # The seam contract permits whole-phase direct checked failures.
            # The phase is the last actual Phase event, never a code inference.
            assert position==len(observed_diagnostics), 'diagnostic event/return disagreement'
            dphase=phase
        diagnostics_out.append(diagnostic(d,dphase,len(diagnostics_out)))
    assert position==len(observed_diagnostics), 'unreturned diagnostic observations'

    if request.get('mode')!='private-project-candidate':
        return {'diagnostics':diagnostics_out,'actual_phases':phase_events,
                'signature_starts':[e for e in events if e['event']=='signature-start'],
                'record_starts':[e for e in events if e['event']=='record-start'],
                'load_ok':raw.get('load_ok'),'parser_ok':raw.get('parser_ok'),
                'syntax_flavor':raw.get('syntax_flavor'),'content_files_retained':[f['path'] for f in files.values()],
                'human':raw.get('human','')}

    assert raw['load_ok'], 'Semantic source corpus failed actual loading'
    modules=[];module_names={}
    for m in raw['modules']:
        if m['parent'] is None:
            assert m['id']==0
            name='crate'
        else:
            assert m['parent'] in module_names
            name=module_names[m['parent']]+'::'+spelling(m['declaration'])
        module_names[m['id']]=name
        modules.append({'path':name,'module_id':m['id'],'file_id':m['file'],'file':files[m['file']]['path']})
    definitions={};functions=[];records=[];function_by_local={};function_name={}
    for e in events:
        if e['event']!='original':continue
        assert e['kind'] in ('record','function')
        ns='type' if e['kind']=='record' else 'value'
        row={'symbol':{'module':module_names[e['module']],'namespace':ns,'name':spelling(e['name'])},
             'kind':e['kind'],'origin':origin(e['name']),'id':e['id']}
        index=(e['kind'],e['id']);assert index not in definitions
        definitions[index]=row
        if e['kind']=='function':
            functions.append(row)
            function_by_local[(e['name']['file'],e['local'])]=e['id']
            function_name[e['id']]=row['symbol']['module']+'::'+row['symbol']['name']
        else:records.append(row)
    functions.sort(key=lambda d:d['id']);records.sort(key=lambda d:d['id'])
    ranges=[]
    for f in raw['ast_functions']:
        fid=function_by_local.get((f['file'],f['local']))
        if fid is not None:ranges.append((f['file'],f['name']['start'],f['end']['end'],fid))

    def owner(span):
        owners=[fid for file,start,end,fid in ranges if file==span['file'] and start<=span['start'] and span['end']<=end]
        assert len(owners)==1, ('actual query origin lacks a unique actual AST function',span,owners)
        return function_name[owners[0]]

    imports={};import_events=[];last_state={'aliases':[],'seen_targets':[]}
    pending_rejection=None;diagnostic_count=0

    def target(kind,id):return definitions[(kind,id)]

    def import_state(e):
        aliases=[];seen=[]
        for cell in e['aliases']:
            for ns,kind,lane in [('type','record','type'),('value','function','value')]:
                selected=cell[lane+'_target'];first=cell[lane+'_first']
                if selected is None:
                    assert first is None
                    continue
                assert first is not None and imports[first]['committed']
                aliases.append({'requester':module_names[cell['module']],'namespace':ns,'alias':spelling(cell['alias']),
                                'target':target(kind,selected),'first_origin':origin(imports[first]['alias'])})
        for cell in e['seen']:
            for ns,kind,lane in [('type','record','type'),('value','function','value')]:
                first=cell[lane+'_first']
                if first is None:continue
                # Join real seen first-import identity to that actual committed
                # event's target; never resolve a syntax path in this adapter.
                earlier=imports[first];assert earlier['committed']
                selected=earlier[lane+'_target'];assert selected is not None
                seen.append({'requester':module_names[cell['module']],'namespace':ns,
                             'symbol':target(kind,selected)['symbol'],'first_origin':origin(earlier['alias'])})
        aliases.sort(key=lambda a:(a['requester'],a['namespace'],a['alias']))
        seen.sort(key=lambda a:(a['requester'],a['namespace'],a['symbol']['module'],a['symbol']['name']))
        return {'aliases':aliases,'seen_targets':seen}

    for e in events:
        if e['event']=='diagnostic':
            if pending_rejection is not None:
                assert e['phase']=='imports'
                pending_rejection['diagnostic_index']=diagnostic_count
                pending_rejection=None
            diagnostic_count+=1
        elif e['event']=='import':
            assert pending_rejection is None and e['id'] not in imports
            imports[e['id']]=e
            last_state=import_state(e)
            row={'ordinal':e['id'],'requester':module_names[e['module']],'alias':spelling(e['alias']),
                 'origin':origin(e['alias']),'status':'committed' if e['committed'] else 'rejected',
                 'targets':None,'state_after':last_state}
            if e['committed']:
                row['targets']=[]
                for kind,lane in [('record','type'),('function','value')]:
                    if e[lane+'_target'] is not None:row['targets'].append(target(kind,e[lane+'_target']))
            else:
                assert e['type_target'] is None and e['value_target'] is None
                pending_rejection=row
            import_events.append(row)
    assert pending_rejection is None

    probe_index={(p['phase'],p['owner'],p['kind'],key(p['origin'])) for p in request['probes']}
    observed_targets=[];phase='collection';selected_probes=set()

    def emit(phase,owner_name,kind,where,target_kind,target_id,field=None):
        normalized=origin(where)
        probe=(phase,owner_name,kind,key(normalized))
        if probe not in probe_index:return
        assert probe not in selected_probes, ('duplicate successful passive site',probe)
        selected_probes.add(probe)
        row={'phase':phase,'owner':owner_name,'kind':kind,'origin':normalized,'target':target(target_kind,target_id)}
        if field is not None:row['field_id']=field
        observed_targets.append(row)

    for e in events:
        kind=e['event']
        if kind=='phase':phase=e['phase']
        elif kind=='target':
            if e['operation'] in ('callee','value-type','reference-type'):
                emit(phase,owner(e['origin']),'lookup',e['origin'],e['kind'],e['id'])
            elif e['operation']=='constructor-resolved':
                emit(phase,owner(e['origin']),'construct',e['origin'],e['kind'],e['id'])
            elif e['operation']!='constructor-type':raise AssertionError(('unmapped target operation',e))
        elif kind=='projection':
            emit(phase,function_name[e['function']],e['operation'],e['origin'],'record',e['field']['record'],e['field'])
        elif kind in ('expression','borrow-argument') and 'record' in e['type']:
            emit(phase,function_name[e['function']],'opaque-transfer',e['origin'],'record',e['type']['record'])

    routes=[e['owned'] for e in events if e['event']=='route'];assert len(routes)==1
    frozen=[e for e in events if e['event']=='frozen'];assert len(frozen)<=1
    route='owned' if routes[0] else 'scalar'
    if raw['check']['success']:
        assert raw['check']['route'].lower()==route and not diagnostics_out and frozen
    result={'syntax_flavor':raw['syntax_flavor'],'selected_route':route,'modules':modules,
            'function_declarations':functions,'record_declarations':records,'final_index_available':bool(frozen),
            'first_stopping_phase':diagnostics_out[0]['phase'] if diagnostics_out else None,
            'diagnostics':diagnostics_out,'import_events':import_events,'observed_targets':observed_targets}
    if frozen:
        result['index_expectations']={**last_state,'root_original_main':frozen[0]['root_main'],
                                      'original_function_count':len(functions)}
    return result


def main():
    parser=argparse.ArgumentParser();parser.add_argument('raw',type=Path);parser.add_argument('queue',type=Path);parser.add_argument('output',type=Path)
    args=parser.parse_args();queue_root=args.queue.parent
    queues={row['cohort']+'/'+row['case']:row for row in json.loads(args.queue.read_text())['cases']}
    count=0;errors=[]
    with args.output.open('w') as out:
        for line in args.raw.read_text().splitlines():
            item=json.loads(line);row=queues[item['case']]
            request=json.loads((queue_root/row['request']).read_text())
            try:
                result=normalize(item['raw'],request,str(queue_root/row['entry']))
                value={**{k:item[k] for k in ('case','mode','started_ns','finished_ns')},'result':result}
                if 'read_source_entries' in item:
                    entries=[]
                    for entry_event in item['read_source_entries']:
                        native=bytes.fromhex(entry_event['native_path_hex'])
                        assert native
                        entries.append({'display_file':Path(entry_event['display']).relative_to((queue_root/row['entry']).parent).as_posix(),
                                        'display':entry_event['display'],'native_path_hex':entry_event['native_path_hex'],
                                        'origin':entry_event['origin']})
                    value['program_read_source_entries']=entries
            except Exception as error:
                value={'case':item['case'],'normalization_error':repr(error)};errors.append(value)
            out.write(json.dumps(value,sort_keys=True)+'\n');count+=1
    print(json.dumps({'cases':count,'normalization_errors':len(errors),'first_errors':errors[:8]}))
    return bool(errors)


if __name__=='__main__':raise SystemExit(main())
