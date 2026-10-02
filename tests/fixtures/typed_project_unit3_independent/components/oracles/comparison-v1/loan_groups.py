"""Compare sibling release membership inside exact atomic return groups only.

Physical source/model event order is never mutated. Group identity is supplied
by independently derived source return events or actual checked resume calls.
"""
import json,sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')

def project(events):
    result=[];seen=set();i=0
    while i<len(events):
        e=events[i]
        if e['kind']=='acquire':result.append(e);i+=1;continue
        assert e['kind']=='release' and 'return_group' in e
        group=e['return_group'];identity=json.dumps(group,sort_keys=True)
        assert identity not in seen,'a return group cannot reopen after another loan event'
        seen.add(identity);members=[]
        while i<len(events) and events[i]['kind']=='release' and events[i].get('return_group')==group:
            members.append({k:v for k,v in events[i].items() if k!='return_group'});i+=1
        handles=[m['handle'] for m in members];assert len(set(handles))==len(handles),'duplicate release member'
        result.append(dict(kind='normal-return-release-group',return_group=group,members=sorted(members,key=lambda m:m['handle'])))
    return result
