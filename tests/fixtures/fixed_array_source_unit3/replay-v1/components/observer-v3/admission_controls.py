#!/usr/bin/env python3
"""No compiler/source observations: reject stale/missing/ambiguous artifact sets."""
import json,pathlib,tempfile,sys
from compare import validate_artifacts,Rejected
out=pathlib.Path(sys.argv[1]);results=[]
controls=[('stale-jsonl-plus-fresh-error',{'c.jsonl':'{"complete":true}\n','c.error':'incomplete-observation: limit'}),('missing',{}),('unknown',{'c.jsonl':'{}','other.jsonl':'{}'}),('error-plus-jsonl',{'c.error':'failed','c.jsonl':'{}'})]
for name,files in controls:
 with tempfile.TemporaryDirectory(prefix='unit3a-artifact-admission-')as raw:
  d=pathlib.Path(raw)
  for n,body in files.items():(d/n).write_text(body)
  try:validate_artifacts(d,['c'])
  except Rejected as e:results.append({'id':name,'rejected':True,'reason':str(e)})
  else:results.append({'id':name,'rejected':False})
report={'schema':'oxid-unit3a-artifact-admission-controls-v1','new_successful_source_observations':0,'controls':results,'pass':sum(r['rejected']for r in results),'total':len(results)}
out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));sys.exit(report['pass']!=report['total'])
