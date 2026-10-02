"""Independent handwritten RFC Batch schedule; does not import the source model.

Counts are built from the source's concrete loop iterations and published event
costs. This is a non-blind reviewer recomputation, not compiler/native evidence.
"""
from collections import Counter
import hashlib
import json
from pathlib import Path

ROOT=Path(__file__).resolve().parent
FRAMES={
 'retry':dict(S=4,A=0,P=0,O=0,R=1,L=0,C=0),
 'commit':dict(S=10,A=0,P=0,O=0,R=1,L=0,C=0),
 'dispatch':dict(S=4,A=2,P=0,O=0,R=1,L=1,C=1),
 'done':dict(S=3,A=0,P=0,O=0,R=1,L=0,C=0),
 'relay':dict(S=0,A=0,P=8,O=2,R=0,L=0,C=0),
 'finish':dict(S=7,A=0,P=4,O=1,R=0,L=0,C=0),
 'main':dict(S=26,A=6,P=32,O=8,R=0,L=3,C=5),
}
for d in FRAMES.values(): d['X']=d['S']+d['A']+d['P']+4*d['O']+8*d['R']+12*d['L']+2*d['C']
seq=[]
def add(op,cost=1,where=''):
 seq.append(dict(operation=op,cost=cost,where=where))
def scalars(n,where=''):
 for _ in range(n): add('Scalar',where=where)
def field_read(where=''): add('ReadField',where=where)
def end_owner(where=''): add('StorageEnd',5,where)
def name_move(where=''):
 add('StorageLive',where=where);add('MoveInitialize',5,where)
def let_install(where=''):
 name_move(where);end_owner(where)
def return_scalar(fn):
 d=FRAMES[fn];add('ReturnScalar',1+d['P']+d['L']+d['C']+d['R'],fn)
def call(fn,kind):
 # Exact source call signatures, with one reference or one owned argument,
 # except dispatch/commit's reference plus scalar.
 owned=int(kind=='owned');arity=2 if fn in ('dispatch','commit') else 1
 add('OpenCall',1+owned,fn)
 if owned:
  name_move(fn);add('PrepareOwned',5,fn);end_owner(fn)
 else:
  add('PrepareBorrow',where=fn)
  if arity==2: scalars(1,fn);add('PrepareScalar',where=fn)
 add('Invoke',1+arity+FRAMES[fn]['X']+4*owned,fn)
 {'retry':retry,'commit':commit,'dispatch':dispatch,'done':done,'relay':relay,'finish':finish}[fn]()
def retry():
 field_read('retry');scalars(2,'retry');add('WriteField',where='retry.retries');scalars(1,'retry');return_scalar('retry')
def commit():
 field_read('commit.completed');scalars(2,'commit.completed');add('WriteField',where='commit.completed')
 field_read('commit.checksum');scalars(4,'commit.checksum');add('WriteField',where='commit.checksum')
 scalars(1,'commit');return_scalar('commit')
def dispatch():
 call('commit','reference');scalars(1,'dispatch');return_scalar('dispatch')
def done():
 field_read('done');scalars(2,'done');return_scalar('done')
def relay():
 name_move('relay.return');end_owner('relay.parameter');add('ReturnOwned',13,'relay')
def finish():
 # checksum + completed * 100 + retries, left subtree complete before right.
 field_read('finish.checksum');field_read('finish.completed');scalars(3,'finish.left')
 field_read('finish.retries');scalars(1,'finish.final');end_owner('finish.parameter');return_scalar('finish')

add('Root',1+FRAMES['main']['X'],'main')
scalars(4,'Batch.initializer');add('StorageLive',where='Batch.literal');add('Construct',5,'Batch.literal');let_install('state')
add('Goto',where='outer.preheader')
for job in range(1,7):
 field_read('outer.condition');add('Branch',where='outer.condition')
 scalars(2,'attempt.initializer');add('Goto',where='inner.preheader')
 for attempt in (1,2):
  scalars(3,'attempt.condition');add('Branch',where='attempt.condition')
  scalars(4,'attempt.increment')
  scalars(3,'retry.condition');add('Branch',where='retry.condition')
  if attempt==1:
   call('retry','reference');add('Goto',where='continue.inner')
  else:
   field_read('job.completed');scalars(3,'job.initializer')
   call('dispatch','reference');add('Goto',where='break.inner')
 call('done','reference');add('Branch',where='done.condition')
 if job==6:
  scalars(1,'active.false');add('WriteField',where='state.active');add('Goto',where='break.outer')
 else: add('Goto',where='continue.outer')
call('relay','owned');let_install('completed')
call('finish','owned');end_owner('completed');end_owner('state');return_scalar('main')

spent=0
for item in seq:
 item['start_fuel']=spent;spent+=item['cost'];item['end_fuel']=spent
frozen=ROOT.parent/'ownership-unit4-source-model/freeze-v3/frozen/batch.json'
expected=json.loads(frozen.read_text())
observed=[(i['operation'],i['cost'],i['start_fuel'],i['end_fuel']) for i in expected['schedule']['items']]
reviewed=[(i['operation'],i['cost'],i['start_fuel'],i['end_fuel']) for i in seq]
assert reviewed==observed, next(((i,a,b) for i,(a,b) in enumerate(zip(reviewed,observed)) if a!=b),('length',len(reviewed),len(observed)))
assert len(seq)==506 and spent==1297
assert FRAMES=={fn:{k:d[k] for k in FRAMES[fn]} for fn,d in expected['schedule']['frames'].items()}
# Each Batch occupies 16 aligned bytes: three i32 fields, one bool, tail padding.
path=['main','dispatch','commit']
resources=[]
for fn in path:
 d=FRAMES[fn];arena=16*d['O']
 resources.append(dict(function=fn,X=d['X'],B=arena,
  Dref=8*(d['S']+d['A'])+arena+32*d['O']+64*d['R']+96*d['L']+16*d['C'],
  Dnative=8*(d['S']+d['A'])+8*(d['R']+d['L'])+arena))
peak={key:sum(row[key] for row in resources) for key in ('X','Dref','Dnative')}
assert peak==dict(X=188,Dref=1376,Dnative=560)
rows=[];completed=retries=checksum=0
for job in range(1,7):
 retries+=1; rows.append(('retries',retries))
 completed+=1;rows.append(('completed',completed));checksum+=job*10;rows.append(('checksum',checksum))
rows.append(('active',False))
assert rows==[(w['field'],w['value']) for w in expected['dynamic']['writes']]
result=checksum+completed*100+retries
assert result==816
report=dict(evidence='Reviewer handwritten non-blind source/template recomputation; no compiler/raw/native observations',
  source_sha256=expected['source_sha256'],frozen_batch_sha256=hashlib.sha256(frozen.read_bytes()).hexdigest(),
  result=result,charged_operations=len(seq),fuel=spent,operations=dict(Counter(i['operation'] for i in seq)),
  frame_census=FRAMES,peak_path_resources=resources,peak=peak,
  reserved_reference_bytes_default=1024*272+peak['Dref']+8,
  reserved_reference_bytes_three_frames=3*272+peak['Dref']+8,
  write_fuel_boundaries=[dict(start=i['start_fuel'],end=i['end_fuel'],where=i['where']) for i in seq if i['operation']=='WriteField'],
  all_1298_budget_prefixes_match=True)
for budget in range(spent+1):
 first=next((i for i in seq if i['end_fuel']>budget),None)
 other=next((i for i in expected['schedule']['items'] if i['end_fuel']>budget),None)
 assert (None if first is None else (first['operation'],first['start_fuel']))==(None if other is None else (other['operation'],other['start_fuel']))
(ROOT/'independent-batch-ledger.json').write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
print(json.dumps({k:report[k] for k in ('result','charged_operations','fuel','peak','reserved_reference_bytes_default','reserved_reference_bytes_three_frames','all_1298_budget_prefixes_match')}))
