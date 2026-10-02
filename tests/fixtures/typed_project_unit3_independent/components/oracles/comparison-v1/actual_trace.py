"""Normalize actual runtime journals using only actual raw tables and event IDs.

No expected data is imported. Runtime transition epochs are checked separately
from source lexical generations; those numbers have different contracts.
"""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")

from actual_raw import Raw,base,parameter_value,scalar,value_type
from collections import Counter

def same(a,b,label):assert type(a)is type(b) and a==b,(label,a,b)
def key(v):return tuple(v[k] for k in ('frame','activation','loan','instance'))
def physical_scalar(ty,value,before,after):
    same({'i32':'I32','bool':'Bool','()':'Unit'}[ty],value['tag'],'recorded scalar type equals physical field type')
    width=4 if ty=='i32' else 1
    for image in (before,after):
        assert isinstance(image,list) and len(image)==width and all(type(v)is int and 0<=v<=255 for v in image),'exact physical field byte width'
    if ty=='bool':assert after[0] in (0,1),'canonical bool byte'
    if ty=='()':same(0,after[0],'canonical unit byte')
    val=scalar(value);data=bytes(after)
    decoded=int.from_bytes(data,'little',signed=True) if ty=='i32' else bool(data[0]) if ty=='bool' else None
    same(val,decoded,'actual payload bytes after write')
    return val

def normalize(raw,route,rows,budget=None):
    r=Raw(raw,route);charges=[];writes=[];loans=[];pending=None;fuel=None
    activations={};stack=[];parents={};active={};loan_slots={};epochs={};states={};transfer_initial={};lifetimes={};epoch_journal=[]
    activation_frames={};activation_calls={};physical_values={};transfer_journal=[];loan_instances={};return_groups=[]
    committed=0;seen_handles=0
    def owner(v):
        aid=v['activation'];assert aid in activations,('unknown owner activation',v)
        ordinal,fid=activations[aid];return ordinal,r.owner(fid,v['owner'])
    def actual_operation(context):
        fid=context['function'];assert 0<=fid<len(r.functions)
        if context['position']=='entry':
            return dict(operation='Root',function=fid,span=r.functions[fid]['span'],payload=None)
        op=r.op_at(fid,context['block'],context['position'])
        raw_value=(r.functions[fid]['blocks'][context['block']]['terminator'] if context['position']=='terminator'
                   else r.functions[fid]['blocks'][context['block']]['merge'] if context['position']=='merge'
                   else r.functions[fid]['blocks'][context['block']]['statements'][int(context['position'][10:-1])])
        same(raw_value,context['operation'],'context payload equals immutable actual raw control point')
        return op
    def get_payload(items,count):assert isinstance(items,list)and len(items)==count,items;return items
    def actual_epoch(v):return (v['activation'],v['owner'])
    def current_key(aid,oid):
        assert aid in activations
        fid=activations[aid][1];assert 0<=oid<len(r.functions[fid]['owners'])
        return dict(tag='OwnerKey',frame=activation_frames[aid],activation=aid,owner=oid,generation=epochs.get((aid,oid),transfer_initial.get((aid,oid),0)))
    def record_at(aid,oid):return r.functions[activations[aid][1]]['owners'][oid]['record']
    def return_destination(aid):
        call=activation_calls[aid];parent=r.functions[call['function']]['calls'][call['call']]
        assert parent['result']['tag']=='Owned'
        return call['activation'],parameter_value(parent['result'])
    def move_roster(op,aid):
        tag=op['operation'];v=op['payload'];fid=op['function']
        if tag in ('MoveInitialize','Replace'):return [((aid,v['source']),(aid,v['destination']))]
        if tag=='PrepareOwned':
            arg=r.functions[fid]['calls'][v['call']]['arguments'][v['argument']];assert arg['tag']=='Owned'
            return [((aid,v['source']),(aid,parameter_value(arg)))]
        if tag=='ReturnOwned':return [((aid,v['owner']),return_destination(aid))]
        if tag=='Invoke':
            call=r.functions[fid]['calls'][v['call']];callee=r.functions[call['target']];out=[]
            for a,p in zip(call['arguments'],callee['parameters']):
                if a['tag']=='Owned':assert p['tag']=='Owned';out.append(((aid,parameter_value(a)),(None,parameter_value(p))))
            return out
        return []
    def transition_roster(op,aid):
        tag=op['operation'];v=op['payload'];fid=op['function']
        if tag in ('StorageLive','StorageEnd','Discard'):return [(aid,fid,v['owner'],{'StorageLive':1,'StorageEnd':0,'Discard':3}[tag])]
        if tag=='Construct':return [(aid,fid,v['destination'],2)]
        if tag in ('MoveInitialize','Replace','PrepareOwned','ReturnOwned'):
            src,dst=move_roster(op,aid)[0]
            return [(src[0],activations[src[0]][1],src[1],3),(dst[0],activations[dst[0]][1],dst[1],2)]
        if tag in ('OpenCall','Invoke'):
            call=r.functions[fid]['calls'][v['call']]
            return [(aid,fid,parameter_value(a),1 if tag=='OpenCall' else 0) for a in call['arguments'] if a['tag']=='Owned']
        return []
    def field_destination(op,aid):
        v=op['payload'];b=base(v['base'])
        if b['space']=='owner':return current_key(aid,b['id'])
        h=parents[aid,b['id']];assert h in active,'field reference parent loan is inactive'
        same('Exclusive',r.functions[op['function']]['references'][b['id']]['kind']['tag'],'field write has exclusive reference')
        return active[h]['owner_key']
    def payload_roster(op,aid):
        tag=op['operation'];v=op['payload']
        if tag=='Construct':return [((aid,v['destination']),f['field']) for f in v['fields']]
        if tag=='WriteField':
            k=field_destination(op,aid);return [((k['activation'],k['owner']),v['field'])]
        if tag in ('MoveInitialize','Replace','PrepareOwned','ReturnOwned'):
            _,dst=move_roster(op,aid)[0];record=record_at(*dst)
            return [(dst,f['id']) for f in r.records[record]['fields']]
        return []
    def callbacks(op):
        tag=op['operation'];wanted=Counter()
        if route=='owned':
            wanted['Charge']=1
            if tag in ('Root','Invoke'):wanted['Enter']=1
            if tag in ('ReturnScalar','ReturnOwned'):wanted['Return']=1
            if tag=='WriteField':wanted['WriteField']=1
            wanted['Transfer']=len(move_roster(op,pending['activation']))
            if tag=='PrepareBorrow':wanted['Acquire']=1
            if tag in ('ReturnScalar','ReturnOwned') and pending['activation'] in activation_calls:
                call=activation_calls[pending['activation']]
                wanted['Release']=sum(a['tag']=='Borrow' for a in r.functions[call['function']]['calls'][call['call']]['arguments'])
        else:
            if tag in ('Root','Call'):wanted['Enter']=1
            if tag=='Return':wanted['Return']=1
            if tag=='Branch':wanted['Branch']=1
            if tag=='Scalar' and op['payload']['tag'] in ('Initialize','Store'):wanted[op['payload']['tag']]=1
        return +wanted
    for row in rows:
        kind=row['kind'];context=row['context'];payload=row['payload']
        if kind=='operation_start':
            assert pending is None or pending['committed'],('uncommitted operation before next start',pending)
            same(route,context['route'],'context route');op=actual_operation(context)
            aid=context['activation'] if route=='owned' else (stack[-1] if stack else None)
            pending=dict(op=op,context=context,activation=aid,attempt=None,committed=False,replacement_fields=[],transitions=0,transfers=0,payloads=0,callbacks=Counter(),future_aid=None,field_effect=None,scalar_effect=None,released_keys=[])
            continue
        assert pending is not None,('event without actual operation',kind)
        same(pending['context'],context,'event remains associated with its actual operation')
        assert not pending['committed'],('effect/callback after operation commit',kind)
        op=pending['op'];fid=op['function'];ins=op['payload']
        if kind=='charge_attempt':
            cost,available,span,allowed=get_payload(payload,4)
            assert type(cost)is int and cost>0 and type(available)is int and type(allowed)is bool
            same(available>=cost,allowed,'charge admission')
            same(op['span'],span,'operation charge origin')
            if fuel is None:
                if budget is not None:same(budget,available,'requested reduced initial fuel')
            else:same(fuel,available,'one nonresetting execution fuel')
            assert pending['attempt'] is None,'multiple charges for one actual operation'
            fuel=available-cost if allowed else available;pending['attempt']=allowed
            charge=dict(operation=op['operation'],function=fid,cost=cost,span=span,allowed=allowed)
            if op['operation']!='Root':
                assert pending['activation'] in activations
                charge['activation']=activations[pending['activation']][0]
            if op['operation'] in ('OpenCall','Invoke'):
                charge['target']=r.functions[fid]['calls'][ins['call']]['target']
            elif op['operation']=='Call':charge['target']=ins['target']
            if op['operation'] in ('StorageLive','StorageEnd'):charge['owner']=r.owner(fid,ins['owner'])
            elif op['operation'] in ('MoveInitialize','Replace'):
                charge['source_owner']=r.owner(fid,ins['source']);charge['destination_owner']=r.owner(fid,ins['destination'])
            elif op['operation']=='PrepareOwned':charge['source_owner']=r.owner(fid,ins['source'])
            elif op['operation']=='PrepareBorrow':
                loan=r.functions[fid]['loans'][ins['loan']]
                charge.update(mode={'Shared':'shared','Exclusive':'exclusive'}[loan['kind']['tag']],position=ins['argument'])
            elif op['operation']=='ReturnOwned':charge['owner']=r.owner(fid,ins['owner'])
            charges.append(charge);continue
        # Every effect and commit must follow a paid charge. A denied operation
        # can have no subsequent effect, callback, install, cleanup or unwind.
        assert pending['attempt'] is True,('effect after unpaid charge',kind,pending)
        if kind=='operation_commit':
            assert not pending['committed'],'duplicate operation commit'
            if route=='owned':
                same(len(transition_roster(op,pending['activation'])),pending['transitions'],'all actual raw-required owner transitions observed')
                same(len(move_roster(op,pending['activation'])),pending['transfers'],'all actual raw-required transfers observed')
                same(len(payload_roster(op,pending['activation'])),pending['payloads'],'all actual raw-required payload writes observed')
            same(callbacks(op),pending['callbacks'],'exact existing consumer callbacks for committed operation')
            if route=='owned' and op['operation'] in ('ReturnScalar','ReturnOwned') and pending['activation'] in activation_calls:
                resume=activation_calls[pending['activation']]
                same(set(resume['loan_keys']),set(pending['released_keys']),'exact resume-call loan membership released by normal return')
                same(len(resume['loan_keys']),len(pending['released_keys']),'no missing/duplicate same-return release')
                return_groups.append(dict(return_raw_activation=pending['activation'],return_function=fid,return_span=op['span'],
                    actual_resume=resume,released_keys_in_physical_order=list(pending['released_keys'])))
            pending['committed']=True;committed+=1
            if op['operation']=='Replace':
                writes.append(dict(kind='replace-owned',function=fid,activation=activations[pending['activation']][0],span=op['span'],
                    fields=pending['replacement_fields'],owner=r.owner(fid,ins['destination'])))
            continue
        if kind in ('scalar_place_write','owned_scalar_place_write'):
            if route=='owned':
                frame,aid,event_fid,place,before,after,span,initialize=get_payload(payload,8)
                same(pending['activation'],aid,'scalar write activation');same(op['span'],span,'owned scalar statement charge origin')
                same(activation_frames[aid],frame,'owned scalar write physical frame belongs to activation')
            else:
                event_fid,place,before,after,span,initialize=get_payload(payload,6)
                same(ins['place']['span'],span,'scalar place-use origin');aid=pending['activation']
            same(fid,event_fid,'scalar write function');assert op['operation']=='Scalar'
            s=ins['scalar'] if route=='owned' else ins;same('Initialize' if initialize else 'Store',s['tag'],'physical scalar write kind')
            same(place,s['place']['id'],'physical scalar write target')
            effect=dict(kind='initialize-scalar' if initialize else 'store-scalar',function=fid,activation=activations[aid][0],span=op['span'],
                        target_span=r.functions[fid]['places'][place]['span'],value=scalar(after))
            if not initialize:effect['before']=scalar(before)
            pending['scalar_effect']=(fid,place,effect['value'])
            writes.append(effect);continue
        if kind=='payload_write':
            target,field,value,span,before,after=get_payload(payload,6);same(op['span'],span,'physical payload write charge origin')
            roster=payload_roster(op,pending['activation']);assert pending['payloads']<len(roster),'extra physical payload write'
            destination,wanted_field=roster[pending['payloads']];pending['payloads']+=1
            same(current_key(*destination),target,'physical payload target is the exact current raw base/reference/transfer destination')
            same(wanted_field,field,'physical field follows actual raw instruction/layout order')
            same(record_at(*destination),field[0],'physical field record matches target owner')
            record,index=field;decl=r.records[record]['fields'][index];ty=value_type(decl['ty'])['type']
            val=physical_scalar(ty,value,before,after)
            if op['operation']=='WriteField':
                same(2,states.get(destination,2 if destination in transfer_initial else None),'written owner is available')
            physical_values[destination,field[1]]=val
            if op['operation']=='WriteField':
                same(ins['field'],field,'physical field id')
                if ty=='bool':assert before[0] in (0,1),'canonical prior available bool byte'
                if ty=='()':same(0,before[0],'canonical prior available unit byte')
                prior=int.from_bytes(bytes(before),'little',signed=True) if ty=='i32' else bool(before[0]) if ty=='bool' else None
                pending['field_effect']=(target,field,val)
                writes.append(dict(kind='field-write',function=fid,activation=activations[pending['activation']][0],span=op['span'],field=field,value=val,before=prior))
            elif op['operation']=='Replace':
                same(ins['destination'],target['owner'],'replacement physical target')
                pending['replacement_fields'].append([field,val])
            continue
        if kind=='owner_transition':
            frame,event_fid,owner_key,state,span=get_payload(payload,5)
            roster=transition_roster(op,pending['activation']);assert pending['transitions']<len(roster),'extra owner transition'
            aid,wanted_fid,oid,wanted_state=roster[pending['transitions']];pending['transitions']+=1
            same((aid,wanted_fid,oid,wanted_state),(owner_key['activation'],event_fid,owner_key['owner'],state),'transition matches actual raw role/order')
            same(activations[aid][1],event_fid,'transition activation/function association')
            same(activation_frames[aid],frame,'transition frame identity');same(frame,owner_key['frame'],'transition key frame identity');same(op['span'],span,'transition origin')
            pair=actual_epoch(owner_key);previous=epochs.get(pair,transfer_initial.get(pair,0))
            same(previous+1,owner_key['generation'],'runtime epoch increases exactly once per actual owner transition')
            prior_state=states.get(pair,0)
            role=r.functions[event_fid]['owners'][owner_key['owner']]['kind']['tag']
            if state==1 or role=='CallResult' and state==2 and prior_state==0:lifetimes[pair]=lifetimes.get(pair,0)+1
            epoch_journal.append(dict(raw_owner_key=owner_key,function=event_fid,prior_epoch=previous,state=state,span=span,
                                      derived_lifetime=lifetimes.get(pair,0)))
            epochs[pair]=owner_key['generation'];states[pair]=state
            continue
        if kind not in ('existing_owned_event','existing_scalar_event'):raise AssertionError(('unknown actual trace event',kind))
        tag=payload['tag'];items=payload.get('items',[])
        pending['callbacks'][tag]+=1
        if tag=='Charge':
            span,cost=get_payload(items,2);same(charges[-1]['span'],span,'existing paid charge origin');same(charges[-1]['cost'],cost,'existing paid charge cost')
        elif tag=='Enter':
            if route=='owned':event_fid,aid=get_payload(items,2)
            else:event_fid=get_payload(items,1)[0];aid=len(activations)
            assert aid not in activations,'activation id reused';ordinal=len(activations)
            if op['operation']=='Root':same(fid,event_fid,'root activation identity')
            else:
                same('Invoke' if route=='owned' else 'Call',op['operation'],'activation is actual invocation')
                call=r.functions[fid]['calls'][ins['call']] if route=='owned' else ins;same(call['target'],event_fid,'invoked target identity')
                if route=='owned':
                    loan_keys=[active[loan_slots[pending['activation'],parameter_value(a)]]['key'] for a in call['arguments'] if a['tag']=='Borrow']
                    assert len(set(loan_keys))==len(loan_keys),'duplicate borrowed key in actual invocation'
                    activation_calls[aid]=dict(activation=pending['activation'],function=fid,call=ins['call'],loan_keys=loan_keys)
                    if pending['future_aid'] is not None:same(pending['future_aid'],aid,'owned parameter transfers join actual next Enter')
                    for position,p in enumerate(r.functions[event_fid]['parameters']):
                        if p['tag']=='Reference':
                            arg=call['arguments'][position];assert arg['tag']=='Borrow'
                            loan_slot=parameter_value(arg);handle=loan_slots[(pending['activation'],loan_slot)]
                            assert handle in active;parents[aid,parameter_value(p)]=handle
            activation_frames[aid]=len(stack);activations[aid]=(ordinal,event_fid);stack.append(aid)
        elif tag=='Return':
            event_fid=get_payload(items,1 if route=='owned' else 2)[0]
            assert stack;same(event_fid,activations[stack[-1]][1],'actual return stack');stack.pop()
        elif tag=='Acquire':
            loan_key,owner_key,mode=get_payload(items,3);same('PrepareBorrow',op['operation'],'acquisition belongs to PrepareBorrow')
            aid=loan_key['activation'];same(pending['activation'],aid,'loan activation');same(ins['loan'],loan_key['loan'],'loan slot')
            same(activation_frames[aid],loan_key['frame'],'acquired loan frame belongs to activation')
            instance_site=(aid,loan_key['loan']);same(loan_instances.get(instance_site,0)+1,loan_key['instance'],'loan instance advances exactly once per acquisition')
            loan_instances[instance_site]=loan_key['instance']
            same(current_key(owner_key['activation'],owner_key['owner']),owner_key,'acquisition retains complete exact current OwnerKey including frame')
            decl=r.functions[fid]['loans'][loan_key['loan']];authority=base(decl['authority']);parent=None
            if authority['space']=='reference':
                parent=parents[aid,authority['id']];assert parent in active,'forwarded parent already released'
                same(active[parent]['owner_key'],owner_key,'forwarded authority retains exact owner epoch')
            else:same(aid,owner_key['activation'],'direct owner activation');same(authority['id'],owner_key['owner'],'direct owner identity')
            pair=actual_epoch(owner_key)
            current=epochs.get(pair,transfer_initial.get(pair))
            same(current,owner_key['generation'],'loan acquires the current exact owner epoch')
            same(2,states.get(pair,2 if pair in transfer_initial else None),'loan source available')
            owner_activation,descriptor=owner(owner_key);handle=seen_handles;seen_handles+=1
            assert key(loan_key) not in [v['key'] for v in active.values()],'loan runtime key reused while live'
            normalized_mode={'Shared':'shared','Exclusive':'exclusive'}[mode['tag']]
            same(decl['kind']['tag'],mode['tag'],'actual acquired mode')
            loans.append(dict(kind='acquire',handle=handle,owner=descriptor,owner_activation=owner_activation,
                              owner_lexical_generation=lifetimes[pair],span=decl['span'],mode=normalized_mode,parent=parent))
            active[handle]=dict(key=key(loan_key),owner_key=owner_key,owner=descriptor,owner_activation=owner_activation,
                span=decl['span'],parent=parent,owner_lifetime=lifetimes[pair],call_target=r.functions[fid]['calls'][decl['call']]['target'],
                call_owner=dict(activation=aid,function=fid,call=decl['call']))
            loan_slots[aid,loan_key['loan']]=handle
        elif tag=='Release':
            loan_key=get_payload(items,1)[0]
            same(activation_frames[loan_key['activation']],loan_key['frame'],'released loan frame belongs to activation')
            found=[h for h,v in active.items() if v['key']==key(loan_key)]
            same(1,len(found),'release exact active runtime loan');handle=found[0];value=active[handle]
            assert op['operation'] in ('ReturnOwned','ReturnScalar'),'loan release outside actual normal return'
            same(value['call_target'],fid,'release belongs to returning callee');assert not any(v['parent']==handle for v in active.values()),'parent released before child'
            assert pending['activation'] in activation_calls,'root return cannot release caller loans'
            resume=activation_calls[pending['activation']]
            same(value['call_owner'],{k:resume[k] for k in ('activation','function','call')},'release belongs to exact entered resume call')
            assert key(loan_key) in resume['loan_keys'],'release was not borrowed by this exact actual invocation'
            assert key(loan_key) not in pending['released_keys'],'duplicate release inside normal return'
            pending['released_keys'].append(key(loan_key))
            return_group=dict(return_activation=activations[pending['activation']][0],return_function=fid,return_span=op['span'],
                caller_activation=activations[resume['activation']][0],caller_function=resume['function'],
                call_span=r.functions[resume['function']]['calls'][resume['call']]['span'])
            loans.append(dict(kind='release',handle=handle,owner=value['owner'],owner_activation=value['owner_activation'],
                              owner_lexical_generation=value['owner_lifetime'],span=value['span'],return_group=return_group))
            del active[handle]
        elif tag=='Transfer':
            source,target=get_payload(items,2)
            roster=move_roster(op,pending['activation']);assert pending['transfers']<len(roster),'extra runtime transfer'
            src,dst=roster[pending['transfers']];pending['transfers']+=1
            same(current_key(*src),source,'transfer uses exact current raw source key');same(2,states.get(src,2 if src in transfer_initial else None),'transferred source is available')
            if dst[0] is None:
                call=r.functions[fid]['calls'][ins['call']];callee=r.functions[call['target']]
                assert target['activation'] not in activations,'new parameter transfer targets existing activation'
                if pending['future_aid'] is None:pending['future_aid']=target['activation']
                same(pending['future_aid'],target['activation'],'one new callee activation for all owned arguments')
                wanted=dict(tag='OwnerKey',frame=len(stack),activation=target['activation'],owner=dst[1],generation=1)
                same(wanted,target,'transfer targets exact new callee parameter');same(record_at(*src),callee['owners'][dst[1]]['record'],'parameter nominal identity')
                pair=actual_epoch(target);transfer_initial[pair]=1;states[pair]=2;lifetimes[pair]=1
            else:
                wanted=current_key(*dst);wanted['generation']+=1;same(wanted,target,'transfer predicts exact next destination epoch')
                same(record_at(*src),record_at(*dst),'transfer nominal identity')
                allowed=(2,3) if op['operation']=='Replace' else (0,) if op['operation']=='ReturnOwned' else (1,)
                assert states.get(dst,0) in allowed,('transfer destination state',op['operation'],states.get(dst,0))
            destination_pair=actual_epoch(target)
            future_lifetime=(lifetimes.get(destination_pair,0)+1 if op['operation']=='ReturnOwned' else lifetimes[destination_pair])
            transfer_journal.append(dict(source=source,destination=target,operation=op['operation'],function=fid,span=op['span'],
                                         source_lifetime=lifetimes[actual_epoch(source)],destination_lifetime=future_lifetime))
        elif tag=='WriteField':
            target,field,value=get_payload(items,3)
            same(pending['field_effect'],(target,field,scalar(value)),'existing field callback matches physical write identity/value')
        elif tag in ('Initialize','Store'):
            event_fid,place,value=get_payload(items,3)
            same(pending['scalar_effect'],(event_fid,place,scalar(value)),'existing scalar callback matches physical place write')
        elif tag=='Branch':
            event_fid,value=get_payload(items,2);same(fid,event_fid,'branch callback function');assert type(value)is bool
        else:raise AssertionError(('unknown existing consumer event',tag))
    transfers=[]
    for move in transfer_journal:
        normalized=dict(operation=move['operation'],function=move['function'],span=move['span'])
        for side in ('source','destination'):
            k=move[side];ordinal,descriptor=owner(k)
            normalized[side]=dict(activation=ordinal,owner=descriptor,lexical_generation=move[side+'_lifetime'])
        transfers.append(normalized)
    return dict(charges=charges,writes=writes,loans=loans,transfers=transfers,operation_commits=committed,
                activations=len(activations),live_loan_handles=sorted(active),runtime_epoch_slots=len(epochs),
                ended_with_uncommitted_operation=bool(pending and not pending['committed']),raw_epoch_journal=epoch_journal,
                active_call_stack=[dict(raw_activation=a,ordinal=activations[a][0],function=activations[a][1]) for a in stack],
                raw_transfer_journal=transfer_journal,raw_return_groups=return_groups)
