#!/usr/bin/env python3
import hashlib,json,pathlib,sys
from compare import check,Rejected,validate_artifacts,REVIEWED_SOURCE
source,observations,out=map(pathlib.Path,sys.argv[1:]);cases=json.loads((source/'cases.json').read_text())['cases'];results=[]
validate_artifacts(observations,[c['id']for c in cases])
for c in cases:
 result={'id':c['id']}
 try:
  raw=(observations/(c['id']+'.jsonl')).read_bytes();rs=[json.loads(x)for x in raw.splitlines()]
  check(rs[0]['schema']=='oxid-array-source-observation-v1','schema');check(rs[0]['reviewed_source_archive']==REVIEWED_SOURCE,'source build binding');check(rs[0]['scope']=='single','scope');check(sum(r['row']=='header'for r in rs)==1 and sum(r['row']=='complete'for r in rs)==1,'envelope count');check(rs[-1].get('complete')is True and rs[-1]['phase']=='diagnostic','completion')
  check(rs[-1]['rows_before_footer']==len(rs)-1,'row accounting');check(rs[-1]['bytes_before_footer']==len(b'\n'.join(raw.splitlines()[:-1]))+1,'byte accounting')
  check(len(raw)<=rs[0]['byte_limit']<=16*1024*1024 and len(rs)<=rs[0]['row_limit']<=200000,'observer limits')
  ss=[r for r in rs if r['row']=='source'];check(len(ss)==1 and ss[0]['file']==0 and ss[0]['path']=='main.ox','source identity')
  check(isinstance(ss[0].get('identity'),int) and ss[0]['identity']>0,'source allocation identity');b=ss[0]['text'].encode();check(hashlib.sha256(b).hexdigest()==c['source_sha256']and len(b)==c['source_bytes'],'source hash')
  ds=[r for r in rs if r['row']=='diagnostic'];check(len(ds)==1,'diagnostic count');d=ds[0]['diagnostic'];e=c['expected']
  for k in ('code','stage','message','secondary','notes'):check(d[k]==e[k],'diagnostic '+k)
  p=dict(d['primary']);p['file']=p.pop('file_id');p['text']=b[p['start']:p['end']].decode();check(p==e['primary'],'primary origin')
  rr=[r for r in rs if r['row']=='reservation'];check(len(rr)==1024,'reservation count')
  check([r['length']for r in rr]==list(range(1,1025)) and all(r['success']and r['element_bytes']==8 for r in rr),'reservation schedule')
  check(not any(r['row']=='expression'for r in rs),'partial AST escaped')
  result.update(status='PASS',reservations=1024,observation_sha256=hashlib.sha256(raw).hexdigest())
 except Exception as e:result.update(status='FAIL',reason=str(e))
 results.append(result)
report={'schema':'oxid-unit3a-boundary-comparison-v1','cases':results,'pass':sum(x['status']=='PASS'for x in results),'total':len(results)};out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'pass':report['pass'],'total':report['total']}));sys.exit(report['pass']!=report['total'])
