#!/usr/bin/env python3
"""Independently authored synthetic trace controls; no candidate data input."""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")

from pathlib import Path
import copy,hashlib,json
from actual_trace import normalize
from compare_static import eq

SOURCE='fn main() -> i32 { let mut x = 0; return x; }\n'
def span(fragment,start=0):
    a=SOURCE.index(fragment,start);return [0,a,a+len(fragment)]
NAME=span('main');LITERAL=span('0');BINDING=span('x');READ=span('x',BINDING[2]);LET=span('let mut x = 0;');RETURN=span('return x;')
RAW={'tag':'Program','functions':[{'tag':'Function','id':0,'span':NAME,'result':{'tag':'I32'},'param_count':0,'entry':0,
    'locals':[{'tag':'LocalDecl','ty':{'tag':'I32'},'kind':{'tag':'Temporary'},'span':LITERAL},
              {'tag':'LocalDecl','ty':{'tag':'I32'},'kind':{'tag':'Temporary'},'span':READ}],
    'places':[{'tag':'PlaceDecl','ty':{'tag':'I32'},'span':BINDING}],
    'blocks':[{'tag':'BasicBlock','span':NAME,'merge':None,'statements':[
      {'tag':'Assign','destination':0,'value':{'tag':'I32','value':0},'span':LITERAL},
      {'tag':'Initialize','place':{'tag':'Place','id':0,'span':BINDING},'value':{'tag':'Operand','local':0,'span':LITERAL},'span':LET},
      {'tag':'Assign','destination':1,'value':{'tag':'Load','place':{'tag':'Place','id':0,'span':READ}},'span':READ}],
      'terminator':{'tag':'Terminator','span':RETURN,'kind':{'tag':'Return','value':{'tag':'Operand','local':1,'span':READ}}}}]}]}

def trace():
    rows=[];available=100;context=None
    def row(kind,payload):rows.append({'tag':'Row','kind':kind,'context':copy.deepcopy(context),'payload':payload})
    def op(position,value,cost,origin,events=()):
        nonlocal context,available
        context={'tag':'Context','route':'scalar','function':0,'activation':None,'block':None if position=='entry' else 0,'position':position,'operation':copy.deepcopy(value)}
        row('operation_start',[]);row('charge_attempt',[cost,available,origin,True]);available-=cost
        for kind,payload in events:row(kind,payload)
        row('operation_commit',[])
    op('entry',0,4,NAME,[('existing_scalar_event',{'tag':'Enter','items':[0]})])
    block=RAW['functions'][0]['blocks'][0]
    op('statement[0]',block['statements'][0],1,LITERAL)
    op('statement[1]',block['statements'][1],1,LET,[('scalar_place_write',[0,0,None,{'tag':'I32','items':[0]},BINDING,True]),
        ('existing_scalar_event',{'tag':'Initialize','items':[0,0,{'tag':'I32','items':[0]}]})])
    op('statement[2]',block['statements'][2],1,READ)
    op('terminator',block['terminator'],1,RETURN,[('existing_scalar_event',{'tag':'Return','items':[0,{'tag':'I32','items':[0]}]})])
    return rows

EXPECTED_CHARGES=[dict(operation=op,function=0,cost=cost,span=origin,allowed=True,**({} if op=='Root' else {'activation':0}))
                  for op,cost,origin in [('Root',4,NAME),('Scalar',1,LITERAL),('Scalar',1,LET),('Scalar',1,READ),('Return',1,RETURN)]]
EXPECTED_WRITES=[dict(kind='initialize-scalar',function=0,activation=0,span=LET,target_span=BINDING,value=0)]
def check(rows):
    actual=normalize(RAW,'scalar',rows,100)
    eq(EXPECTED_CHARGES,actual['charges'],'independent synthetic charge sequence')
    eq(EXPECTED_WRITES,actual['writes'],'independent synthetic committed write')
    eq([],actual['active_call_stack'],'synthetic successful root returned')
    eq(False,actual['ended_with_uncommitted_operation'],'synthetic final operation committed')
    return actual
def main():
    original=trace();check(original);mutants=[]
    for name,predicate in [('root-return',lambda r:r['kind']=='existing_scalar_event' and r['payload']['tag']=='Return'),
                           ('charge',lambda r:r['kind']=='charge_attempt'),('commit',lambda r:r['kind']=='operation_commit'),
                           ('effect',lambda r:r['kind']=='scalar_place_write')]:
        i=next(i for i,r in enumerate(original) if predicate(r))
        for action in ('drop','duplicate'):
            rows=copy.deepcopy(original)
            if action=='drop':rows.pop(i)
            else:rows.insert(i,copy.deepcopy(rows[i]))
            try:check(rows)
            except Exception as e:mutants.append(dict(id=action+'-'+name,status='REJECTED',reason=str(e)))
            else:raise AssertionError(('mutant escaped',action,name))
    final=copy.deepcopy(original);final.pop()
    try:check(final)
    except Exception as e:mutants.append(dict(id='drop-final-commit',status='REJECTED',reason=str(e)))
    else:raise AssertionError('final commit omission escaped')
    report=dict(schema='unit3-synthetic-stream-sensitivity-v1',source=SOURCE,source_sha256=hashlib.sha256(SOURCE.encode()).hexdigest(),
                evidence_kind='INDEPENDENT_SYNTHETIC_TRANSPORT_CONTROL',candidate_receipts_read=0,compiler_invocations=0,
                positive='PASS',mutants=mutants,mutants_rejected=len(mutants))
    Path(__file__).with_name('stream-sensitivity.json').write_text(json.dumps(report,indent=2)+'\n');print('synthetic stream mutants rejected',len(mutants))

if __name__=='__main__':main()
