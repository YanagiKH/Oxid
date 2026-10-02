"""Read frozen expectations and project them into neutral comparison facts.

This module does not read candidate receipts. It never resolves source. Original
and supplemental expectations are verified by immutable file hashes first.
"""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")

from pathlib import Path
import gzip,hashlib,json

PACKAGE=Path(__file__).resolve().parents[1]
ORIGINAL='b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba'
SUPPLEMENT='8dd370bfd1dad8158c9ebfde0b0e90e448ae81cd86494cd2952fca5ea91247d5'

def verify_manifest(path,wanted):
    data=path.read_bytes();assert hashlib.sha256(data).hexdigest()==wanted
    manifest=json.loads(data)
    for row in manifest['files']:
        content=(path.parent/row['path']).read_bytes()
        assert len(content)==row['bytes'] and hashlib.sha256(content).hexdigest()==row['sha256'],row['path']

def load():
    verify_manifest(PACKAGE/'pre-execution-manifest.json',ORIGINAL)
    supplement=PACKAGE/'supplements/authored-declarations-v1'
    verify_manifest(supplement/'supplement-manifest.json',SUPPLEMENT)
    decls={c['id']:c for c in json.loads((supplement/'declarations.json').read_text())['cases']}
    cases={c['id']:c for c in map(json.loads,gzip.open(PACKAGE/'expected.jsonl.gz','rt'))}
    assert cases.keys()==decls.keys()
    return {id:Case(case,decls[id]) for id,case in cases.items()}

class Case:
    def __init__(self,case,declarations):
        self.case=case;self.declaration_supplement=declarations;self.id=case['id']
        self.origins=case.get('origins',{});self.facts=case.get('correspondence')
        self.functions={};self.records={};self.bindings={};self.calls={};self.owners={}
        if self.facts is None:return
        self.functions={r['model_label']:r['id'] for r in case['declarations'] if r['kind']=='function'}
        self.records={r['model_label']:r['id'] for r in case['declarations'] if r['kind']=='record'}
        self.bindings={b['identity']:b for b in self.facts['bindings']}
        self.calls={c['identity']:c for c in self.facts['calls']}
        if case['route']=='owned':
            for name,frame in case['schedule']['frames'].items():
                for owner in frame['owners']:
                    site=owner['site'];role={'parameter':'Parameter','local':'Local','temporary':'Temporary','staged-argument':'StagedArgument','call-result':'CallResult'}[owner['kind']]
                    extra={}
                    if site.startswith('temporary:'):span=self.origins[site[10:]]
                    elif site.startswith('result:'):
                        call=self.calls[site[7:]];span=call['span'];extra['call_span']=call['span']
                    elif site.startswith('staged:'):
                        _,key,arg=site.split(':');call=self.calls[key];arg=int(arg)
                        span=call['arguments'][arg]['span'];extra.update(call_span=call['span'],argument=arg)
                    else:
                        binding=self.bindings[site];span=binding['span']
                        if role=='Parameter':extra['position']=binding['position']
                        if role=='Local':extra['mutable']=binding['mutable']
                    self.owners[name,site]=dict(function=self.functions[name],role=role,span=span,record=self.records[owner['record']],**extra)

    def ty(self,value):
        if value in ('i32','bool','()'):return dict(mode='scalar',type=value)
        if value.startswith('&mut '):return dict(mode='exclusive',record=self.records[value[5:]])
        if value.startswith('&'):return dict(mode='shared',record=self.records[value[1:]])
        return dict(mode='owned',record=self.records[value])

    def owner(self,function,identity):
        # Model lexical generation is retained separately from runtime epochs.
        activation,site,generation=identity
        descriptor=self.owners[function,site]
        return dict(activation=activation,owner=descriptor,lexical_generation=generation)

    def owner_any(self,identity):
        activation,site,generation=identity
        found=[(fn,value) for (fn,key),value in self.owners.items() if key==site]
        assert len(found)==1,('ambiguous frozen owner site',site)
        return dict(activation=activation,owner=found[0][1],lexical_generation=generation)

    def expected_charges(self,budget=None):
        schedule=self.case.get('schedule')
        if schedule is None:return None
        dynamic=self.case['dynamic']['events'];result=[]
        returned_sources={e['source'][0]:e['source'] for e in dynamic if e['kind']=='call-result'}
        for item in schedule['items']:
            event=dynamic[item['semantic_event']] if item['semantic_event']>=0 else None
            fn=self.case['entry'] if event is None else self.functions[event['function']]
            allowed=budget is None or item['end_fuel']<=budget
            row=dict(operation=item['operation'],function=fn,cost=item['cost'],span=self.origins[item['origin']],allowed=allowed)
            if event:
                row['activation']=event['activation']
                op=item['operation'];kind=event['kind']
                if op in ('OpenCall','Invoke','Call'):
                    row['target']=self.functions[event['callee']]
                if op=='StorageEnd':
                    identity=event['source'] if kind in ('let-owned','replace-owned') else event['owner']
                    row['owner']=self.owner_any(identity)['owner']
                elif op=='StorageLive':
                    identity=event['destination'] if kind=='owned-name' else event['owner']
                    row['owner']=self.owner_any(identity)['owner']
                elif op in ('MoveInitialize','Replace'):
                    row['source_owner']=self.owner_any(event['source'])['owner']
                    row['destination_owner']=self.owner_any(event['destination'] if kind=='owned-name' else event['owner'])['owner']
                elif op=='PrepareOwned':
                    row['source_owner']=self.owner_any(event['source'])['owner']
                elif op=='PrepareBorrow':
                    row.update(mode=event['mode'],position=event['position'])
                elif op=='ReturnOwned':row['owner']=self.owner_any(returned_sources[event['activation']])['owner']
            result.append(row)
            if not allowed:break
        return result

    def paid_events(self,budget=None):
        if budget is None:return self.case['dynamic']['events']
        schedule=self.case['schedule']['items']
        denied=next((s for s in schedule if s['end_fuel']>budget),None)
        count=max(0,denied['semantic_event']) if denied else len(self.case['dynamic']['events'])
        return self.case['dynamic']['events'][:count]

    def expected_loans(self,budget=None):
        if not self.facts or self.case['route']!='owned':return []
        out=[];returned=None
        for e in self.paid_events(budget):
            if e['kind'] in ('return-scalar','return-owned'):returned=e
            if e['kind'] not in ('borrow-acquire','borrow-release'):continue
            row=dict(kind='acquire' if e['kind']=='borrow-acquire' else 'release',handle=e['handle'],
                     owner=self.owner_any(e['owner'])['owner'],owner_activation=e['owner'][0],
                     owner_lexical_generation=e['owner'][2],span=self.origins[e['origin']])
            if e['kind']=='borrow-acquire':row.update(mode=e['mode'],parent=e['parent'])
            else:
                assert returned is not None
                called=self.calls[e['call'][1]];assert called['callee']==returned['function']
                row['return_group']=dict(return_activation=returned['activation'],return_function=self.functions[returned['function']],
                    return_span=self.origins[returned['origin']],caller_activation=e['call'][0],caller_function=self.functions[e['function']],
                    call_span=called['span'])
            out.append(row)
        return out

    def expected_transfers(self,budget=None):
        if not self.facts or self.case['route']!='owned':return []
        paid={(s['semantic_event'],s['operation']) for s in self.case['schedule']['items'] if budget is None or s['end_fuel']<=budget}
        out=[];prepared={};invocation=None;returned={}
        def emit(operation,event,source,destination,charge_event=None):
            charge_event=charge_event or event
            if (charge_event['index'],operation) not in paid:return
            out.append(dict(operation=operation,function=self.functions[charge_event['function']],span=self.origins[charge_event['origin']],
                            source=self.owner_any(source),destination=self.owner_any(destination)))
        for e in self.case['dynamic']['events']:
            kind=e['kind']
            if kind=='owned-name':emit('MoveInitialize',e,e['source'],e['destination'])
            elif kind=='let-owned':emit('MoveInitialize',e,e['source'],e['owner'])
            elif kind=='replace-owned':emit('Replace',e,e['source'],e['owner'])
            elif kind=='prepare-owned':
                prepared[tuple(e['call']),e['position']]=e['staged'];emit('PrepareOwned',e,e['source'],e['staged'])
            elif kind=='call-invoke':invocation=e
            elif kind=='owned-parameter':
                assert invocation is not None and invocation['callee']==e['function']
                emit('Invoke',e,prepared[tuple(invocation['call']),e['position']],e['owner'],invocation)
            elif kind=='return-owned':returned[e['activation']]=e
            elif kind=='call-result':emit('ReturnOwned',e,e['source'],e['destination'],returned[e['source'][0]])
        return out

    def expected_writes(self,budget=None):
        if not self.facts:return None
        # Charge identity decides commitment for every scalar/field/whole-value
        # write. A complete source event is not assumed at a partially paid
        # multi-operation template boundary.
        allowed={s['semantic_event'] for s in self.case['schedule']['items']
                 if (budget is None or s['end_fuel']<=budget) and
                 s['operation'] in ('Scalar','WriteField','Replace')}
        out=[]
        for e in self.case['dynamic']['events']:
            if e['index'] not in allowed:continue
            kind=e['kind'];row=dict(function=self.functions[e['function']],activation=e['activation'],span=self.origins[e['origin']])
            if kind=='store-scalar' or kind=='let-scalar' and e['mutable']:
                binding=self.bindings[e['binding']]
                row.update(kind='initialize-scalar' if kind=='let-scalar' else 'store-scalar',target_span=binding['span'],value=e['value'])
                if kind=='store-scalar':row['before']=e['before']
            elif kind=='field-write':
                store=next(x for x in self.facts['stores'] if x['statement']==e['origin'])
                field=store['projection']['field_identity'].split('.')
                record=self.facts['records'][int(field[1])]['global_id']
                row.update(kind=kind,field=[record,int(field[-1])],value=e['value'],before=e['before'])
            elif kind=='replace-owned':
                owner=self.owner_any(e['owner'])['owner'];record=owner['record']
                decl=next(r for r in self.declaration_supplement['declarations'] if r['kind']=='record' and r['id']==record)
                fields={f['name']:f['id'] for f in decl['fields']}
                row.update(kind=kind,fields=[[fields[name],value] for name,value in e['fields']],owner=owner)
            else:continue
            out.append(row)
        return out

    def expected_reference(self,budget=None):
        expected=self.case['expected']
        if budget is not None:
            denied=next((x for x in self.case['schedule']['items'] if x['end_fuel']>budget),None)
            if denied:return dict(failure=dict(code='E0601',stage='oir-run',span=self.origins[denied['origin']],related=[]))
        if expected.get('run'):return dict(failure=expected['run'])
        return dict(result_type=expected['result_type'],result=expected['result'],failure=None)
