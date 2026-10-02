"""Compare immutable source expectations against actual raw facts by site joins."""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")

import sys
from pathlib import Path
from collections import Counter
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
import raw_span_oracle
from actual_raw import Raw,value_type,base,parameter_value

def eq(want,got,label):
    def strict(a,b):
        if type(a)is not type(b):return False
        if isinstance(a,dict):return a.keys()==b.keys() and all(strict(a[k],b[k]) for k in a)
        if isinstance(a,(list,tuple)):return len(a)==len(b) and all(strict(x,y) for x,y in zip(a,b))
        return a==b
    assert strict(want,got),dict(check=label,expected=want,actual=got)
def unique(rows,predicate,label):
    found=[r for r in rows if predicate(r)];assert len(found)==1,(label,'expected one actual site',len(found));return found[0]

def association(case,actual):
    raw=actual.get('raw',actual.get('raw_before_audit'));assert raw is not None,'actual raw snapshot missing'
    route=actual['route'];eq(case.case['route'],route,'route')
    loaded=actual['loaded']['sources']['files'];files={f['id']:f['text'].encode() for f in loaded}
    eq(len(case.case['source_manifest']),len(loaded),'loaded source count')
    import hashlib
    for i,row in enumerate(case.case['source_manifest']):
        eq(row['bytes'],len(files[i]),'source bytes');eq(row['sha256'],hashlib.sha256(files[i]).hexdigest(),'source hash')
    inv=raw_span_oracle.inventory(raw,route);audit=actual['audit'];usage=audit['usage'];assert usage is not None,'actual audit counts missing'
    counts={}
    for name,field in [('count','count'),('validate','validation')]:
        value=usage[field];counts[name]=dict(D=value['declarations'],Sspan=value['spans'],Vbind=value['declarations']+value['spans'])
    eq(1 if route=='scalar' else 2+len(raw['records']),usage['dimensions'],'fixed dimension checks')
    lookup={r['path']:r['span'] for r in inv['spans']}
    for phase in ('count','validate'):
        visits=audit[phase+'_visits'];assert isinstance(visits,list)
        for visit in visits:
            eq(True,visit['succeeded'],'successful audit call');eq(phase,visit['pass'],'actual audit phase')
            if visit['kind']=='span':eq(lookup[visit['path']],visit['span'],'logged span equals actual stored raw field')
            else:eq(None,visit['span'],'declaration visit does not invent a span')
        raw_span_oracle.check(raw,route,files,case.declaration_supplement['declarations'],visits,counts)
    if 'raw' in actual:
        eq(case.case['entry'],actual['entry'],'root-original entry');eq(True,actual['checked_immutable'],'bound wrapper immutable across consumers')
    # Raw field types were absent from the original authored declaration table;
    # the accepted source-only supplement now supplies every one explicitly.
    for decl in case.declaration_supplement['declarations']:
        if decl['kind']=='record':
            for f in decl['fields']:eq(dict(mode='scalar',type=f['type']),value_type(raw['records'][decl['id']]['fields'][f['id'][1]]['ty']),'field scalar type')
    return dict(D=inv['D'],Sspan=inv['Sspan'],Vbind=inv['Vbind'],two_pass_units=2*inv['Vbind'])

def correspondence(case,raw):
    if case.facts is None:return dict(status='NO_TAGGED_BODY_MODEL',scope='declarations and explicit source outcomes only')
    r=Raw(raw,case.case['route']);facts=case.facts;slots={};owner_ids={};calls={}
    def findop(fid,tag,span,extra=lambda o:True):
        return unique(r.all_operations[fid],lambda o:o['operation']==tag and o['span']==span and extra(o),('operation',fid,tag,span))
    def operand(fid,o,span):
        eq(span,o['span'],'operand origin');eq(span,r.functions[fid]['locals'][o['local']]['span'],'operand definition origin')
        eq('Temporary',r.functions[fid]['locals'][o['local']]['kind']['tag'],'expression result slot')
    for label,fid in case.functions.items():
        decl=next(d for d in case.case['declarations'] if d['kind']=='function' and d['model_label']==label)
        sig=r.signature(fid);eq(case.ty(decl['result']),sig['result'],'function result')
        eq([case.ty(ty) for _,ty in decl['parameters']],[p['type'] for p in sig['parameters']],'parameter types/modes')
        frame=case.case['schedule']['frames'][label]
        wanted={k:frame[k] for k in 'SAPORLCX'} if r.route=='owned' else frame
        eq(wanted,r.frame_counts(fid),'frame expansion count')
    for b in facts['bindings']:
        fid=case.functions[b['function']];actual=unique(r.bindings(fid),lambda s:s['span']==b['span'],('binding',fid,b['span']))
        slots[b['identity']]=actual
        eq(case.ty(b['type']),actual['type'],'binding type');eq(b['mutable'],actual['mutable'],'binding mutability')
        eq(b['class'],actual['role'],'binding role');eq(b['position'],actual['position'],'parameter position')
    for (label,site),descriptor in case.owners.items():
        fid=case.functions[label]
        found=[i for i,d in enumerate(r.owners(fid)) if d==descriptor]
        eq(1,len(found),'source owner role/origin correspondence');owner_ids[label,site]=found[0]
    for c in facts['calls']:
        fid=c['global_caller'];actual=unique(r.calls(fid),lambda a:a['span']==c['span'],('call',fid,c['span']))
        calls[c['identity']]=actual;eq(c['global_target'],actual['target'],'numeric call target')
    for label,fid in case.functions.items():eq(sum(c['global_caller']==fid for c in facts['calls']),len(r.calls(fid)),'all static calls accounted')
    for c in facts['calls']:
        fid=c['global_caller'];a=calls[c['identity']]
        if r.route=='scalar':
            eq(len(c['arguments']),len(a['args']),'scalar call arity')
            for want,arg in zip(c['arguments'],a['args']):operand(fid,arg,want['span'])
            eq(case.ty(c['result_type']),value_type(r.functions[fid]['locals'][a['destination']]['ty']),'scalar call result')
            continue
        parent=None if c['parent'] is None else [calls[c['parent']['call']]['id'],c['parent']['argument']]
        eq(parent,a['parent'],'nested parent call/argument');eq(len(c['arguments']),len(a['arguments']),'owned call arity')
        for want,arg in zip(c['arguments'],a['arguments']):
            mode=want['mode'];position=want['position'];op='PrepareBorrow' if mode in ('shared','exclusive') else 'PrepareOwned' if mode=='owned' else 'PrepareScalar'
            event=findop(fid,op,want['span'],lambda o:o['payload']['call']==a['id'] and o['payload']['argument']==position);ins=event['payload']
            eq('Borrow' if mode in ('shared','exclusive') else 'Owned' if mode=='owned' else 'Scalar',arg['tag'],'argument slot kind')
            if mode in ('shared','exclusive'):
                loan=r.functions[fid]['loans'][ins['loan']];eq(parameter_value(arg),ins['loan'],'loan argument identity')
                eq(mode,{'Shared':'shared','Exclusive':'exclusive'}[loan['kind']['tag']],'borrow mode')
                eq(want['span'],loan['span'],'loan origin');eq(a['id'],loan['call'],'loan owning call');eq(position,loan['argument'],'loan argument position')
            elif mode=='scalar':operand(fid,ins['value'],want['span'])
        result=a['result'];eq('Owned' if c['result_owner'] else 'Scalar',result['tag'],'call result mode')
        if c['result_owner']:eq(owner_ids[c['function'],c['result_owner']],parameter_value(result),'caller result owner')
    def checkbase(fid,binding,actual):eq(dict(space=slots[binding]['space'],id=slots[binding]['id']),base(actual),'field base binding')
    def field(identity):
        bits=identity.split('.');return [facts['records'][int(bits[1])]['global_id'],int(bits[-1])]
    for p in facts['projections']:
        fid=case.functions[p['function']];o=findop(fid,'ReadField',p['span'])['payload']
        eq(field(p['field_identity']),o['field'],'read field identity');checkbase(fid,p['base_binding'],o['base'])
        eq(case.ty(p['result_type']),value_type(r.functions[fid]['locals'][o['destination']]['ty']),'field read result')
    for p in facts['literals']:
        fid=case.functions[p['function']];o=findop(fid,'Construct',p['span'])['payload']
        eq(owner_ids[p['function'],p['result_owner']],o['destination'],'constructed owner')
        eq([field(x['field_identity']) for x in p['written_fields']],[x['field'] for x in o['fields']],'constructor field order')
        for x,y in zip(p['written_fields'],o['fields']):operand(fid,y['value'],x['value_span'])
    for t in facts['transfers']:
        binding=case.bindings[t['source_binding']];label=binding['function'];fid=case.functions[label]
        event=findop(fid,'MoveInitialize',t['charge']);o=event['payload']
        eq(slots[t['source_binding']]['id'],o['source'],'move source');eq(owner_ids[label,t['destination']],o['destination'],'move destination')
        eq(t['primary'],event['diagnostic_origins']['primary'],'move diagnostic primary');eq(t['cause'],event['diagnostic_origins']['cause'],'move diagnostic cause')
    for s in facts['stores']:
        label=s['function'];fid=case.functions[label];target=slots[s['target_binding']]
        if s['kind']=='write':
            o=findop(fid,'WriteField',s['charge_span'])['payload'];checkbase(fid,s['target_binding'],o['base'])
            eq(field(s['projection']['field_identity']),o['field'],'write field identity');operand(fid,o['value'],s['value_span'])
        elif s['source_owner']:
            o=findop(fid,'MoveInitialize' if s['kind']=='let' else 'Replace',s['charge_span'])['payload']
            eq(target['id'],o['destination'],'owned store destination');eq(owner_ids[label,s['source_owner']],o['source'],'owned store input')
        else:
            ev=findop(fid,'Scalar',s['charge_span']);o=ev['payload']['scalar'] if r.route=='owned' else ev['payload']
            tag=('Initialize' if target['space']=='place' else 'Assign') if s['kind']=='let' else 'Store';eq(tag,o['tag'],'scalar binding store kind')
            if tag=='Assign':eq(target['id'],o['destination'],'scalar binding target');operand(fid,o['value']['operand'],s['value_span'])
            else:eq(target['id'],o['place']['id'],'place store target');operand(fid,o['value'],s['value_span'])
    return dict(status='MATCH',static_calls=len(facts['calls']),bindings=len(facts['bindings']),owned_sites=len(owner_ids),
                field_projections=len(facts['projections']),constructors=len(facts['literals']),transfers=len(facts['transfers']),stores=len(facts['stores']))
