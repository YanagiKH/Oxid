#!/usr/bin/env python3
"""Independent raw-inventory checks of bounded adapter-smoke observations only."""
import collections,gzip,hashlib,json,pathlib,sys
HERE=pathlib.Path(__file__).resolve().parent
sys.path.insert(0,str(HERE.parent/'typed-project-unit3-oracles'))
from raw_span_oracle import inventory
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
results=[];data={}
for profile in ('debug','release'):
 for case in ('scalar','owned','owned-loan-denial'):
  directory=HERE/'smoke-v1'/f'{case}-{profile}-final'
  actual=json.loads(gzip.decompress((directory/'observations.json.gz').read_bytes()));data[(profile,case)]=actual
  raw=actual.get('raw',actual.get('raw_before_audit'));inv=inventory(raw,actual['route'])
  wanted=collections.Counter([('span',x['path'])for x in inv['spans']]+[('declaration',x['path'])for x in inv['declarations']]);spans={x['path']:x['span']for x in inv['spans']}
  for phase in ('count','validate'):
   visits=actual['audit'][phase+'_visits']
   assert collections.Counter((x['kind'],x['path'])for x in visits)==wanted
   assert all(x['succeeded']and(x['kind']!='span'or x['span']==spans[x['path']])for x in visits)
  usage=actual['audit']['usage'];assert usage['count']['declarations']==usage['validation']['declarations']==inv['D'];assert usage['count']['spans']==usage['validation']['spans']==inv['Sspan']
  trace_summary={}
  for name,run in actual['reference'].items():
   rows=run['trace'];charges=[x['payload']for x in rows if x['kind']=='charge_attempt'];commits=[x for x in rows if x['kind']=='operation_commit']
   # These prescribed controls contain no arithmetic overflow, so every paid
   # operation succeeds. This equality is not generalized to abruptly failing IR.
   assert len(commits)==sum(x[3]for x in charges)
   trace_summary[name]={'charge_attempts':len(charges),'successful_charges':sum(x[3]for x in charges),'commits':len(commits),'spent':sum(x[0]for x in charges if x[3]),'rows':len(rows)}
  receipt=json.loads((directory/'receipt.json').read_text())
  results.append({'profile':profile,'case':case,'counts':{k:inv[k]for k in ('D','Sspan','Vbind')},'path_and_value_multisets_equal':True,'trace':trace_summary,'uncompressed_bytes':sum(x['bytes']for x in receipt['artifacts']),'retained_bytes':sum(x.stat().st_size for x in directory.iterdir()if x.is_file()),'receipt_sha256':digest(directory/'receipt.json')})
for case in ('scalar','owned','owned-loan-denial'):
 a,b=data[('debug',case)],data[('release',case)]
 for key in ('raw','raw_before_audit','audit','reference','diagnostics','route','entry'):
  assert a.get(key)==b.get(key),(case,key)
# A same-valued but differently stored duplicate cannot cancel a missing branch.
visits=data[('debug','owned')]['audit']['validate_visits'];pair=None
for i,a in enumerate(visits):
 if a['kind']!='span':continue
 for j,b in enumerate(visits[i+1:],i+1):
  if b['kind']=='span'and a['span']==b['span']and a['path']!=b['path']:pair=(i,j);break
 if pair:break
assert pair is not None
i,j=pair;mutated=visits.copy();mutated[i]=visits[j]
assert len(mutated)==len(visits)
assert collections.Counter((x['kind'],x['span'][0]if x['span']else None)for x in mutated)==collections.Counter((x['kind'],x['span'][0]if x['span']else None)for x in visits)
assert collections.Counter((x['kind'],x['path'])for x in mutated)!=collections.Counter((x['kind'],x['path'])for x in visits)
report={'scope':'bounded3hand-smoke adapter structural/passivity support; no full source oracle/native execution claim','independent_inventory_sha256':digest(HERE.parent/'typed-project-unit3-oracles/raw_span_oracle.py'),'profile_raw_audit_reference_equal':True,'equal_span_duplicate_missing_detected':{'missing_path':visits[i]['path'],'duplicated_path':visits[j]['path'],'same_span':visits[i]['span']},'results':results}
(HERE/'smoke-v1/structural-final.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'cases_per_profile':3,'all_structural_checks_pass':True,'equal_span_duplicate_missing_detected':True}))
