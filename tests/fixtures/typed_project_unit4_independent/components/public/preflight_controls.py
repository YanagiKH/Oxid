#!/usr/bin/env python3
"""SYNTHETIC comparator sensitivity only. No compiler or tool is invoked."""
import copy,json
from pathlib import Path
import public_cli as h
contract,_=h.authorities()
row=next(r for r in h.roster(contract) if r['applicable'] and r['argv_template'][1]=='check')
binrow={'path':'/SYNTHETIC/oxid','bytes':1,'sha256':'SYNTHETIC'}
config={'binaries':{'debug':binrow,'release':binrow},'_binding_sha256':'SYNTHETIC'}
inv=[{**s,'kind':'file'} for s in sorted(row['source_files'],key=lambda x:x['path'])]
p={'argv':[binrow['path']]+row['argv_template'][1:],'cwd':'/SYNTHETIC/fixture','status':row['observation']['status'],
   'stdout':row['observation']['stdout'],'stderr':row['observation']['stderr'],'started_ns':1,'completed_ns':2,'timed_out':False}
for stream in ('stdout','stderr'):p[stream+'_sha256']=h.sha(p[stream].encode())
g={k:copy.deepcopy(row[k]) for k in ('key','group','case_id','operation_index','profile','host','applicable')}
g.update(contract_sha256=h.CONTRACT_HASH,observer_sha256='SYNTHETIC',candidate_binding_sha256='SYNTHETIC',executed=True,
  executable_before=copy.deepcopy(binrow),executable_after=copy.deepcopy(binrow),source_files=copy.deepcopy(row['source_files']),output_initially_absent=True,
  process=p,fixture_directory=p['cwd'],directory_before=copy.deepcopy(inv),directory_after=copy.deepcopy(inv),tool_invocations=[])
h.validate([row],[g],config,'SYNTHETIC')
results=[]
def mutation(name,fn):
 a=[copy.deepcopy(g)];fn(a)
 try:h.validate([row],a,config,'SYNTHETIC')
 except (h.Reject,KeyError,TypeError,ValueError) as e:results.append({'name':name,'rejected':True,'reason':str(e)[:400]})
 else:raise RuntimeError('accepted mutation '+name)
mutation('zero-roster',lambda a:a.clear())
mutation('duplicate-roster',lambda a:a.append(copy.deepcopy(a[0])))
mutation('extra-row',lambda a:a.append({**copy.deepcopy(a[0]),'key':'EXTRA'}))
for key,value in [('profile','wrong'),('host','wrong'),('executed',False),('contract_sha256','wrong'),('observer_sha256','wrong')]:
 mutation('wrong-'+key,lambda a,k=key,v=value:a[0].__setitem__(k,v))
mutation('wrong-argv',lambda a:a[0]['process']['argv'].append('--fake'))
mutation('stale-binary',lambda a:a[0]['executable_before'].__setitem__('sha256','wrong'))
mutation('wrong-source',lambda a:a[0]['source_files'][0].__setitem__('sha256','wrong'))
mutation('wrong-status',lambda a:a[0]['process'].__setitem__('status',1))
mutation('unexpected-output-file',lambda a:a[0]['directory_after'].append({'path':'out.bin','kind':'file','bytes':0,'sha256':h.sha(b'')}))
mutation('external-tool-on-check',lambda a:a[0]['tool_invocations'].append({'tool':'fake'}))
for stream in ('stdout','stderr'):
 mutation('missing-'+stream,lambda a,s=stream:a[0]['process'].pop(s))
 def changed(a,s=stream):a[0]['process'][s]+='altered';a[0]['process'][s+'_sha256']=h.sha(a[0]['process'][s].encode())
 mutation('changed-'+stream,changed)
h.save(h.OUT/'synthetic-preflight-controls-v1.json',{'evidence_kind':'SYNTHETIC_COMPARATOR_TEST_ONLY','compiler_invocations':0,'llvm_invocations':0,
 'ordinary_execution_claim':False,'positive_kernel_accepted':True,'controls':results,'all_rejected':True,'harness':h.binding(h.__file__)})
print(json.dumps({'synthetic_controls':len(results),'all_rejected':True,'compiler_invocations':0}))
