#!/usr/bin/env python3
import json,pathlib,sys
from compare import check,validate_artifacts
source,out=map(pathlib.Path,sys.argv[1:]);ids=['zero-rows','one-row','zero-bytes','one-byte'];validate_artifacts(source,ids)
results=[]
for ident in ids:
 p=source/(ident+'.error');check(p.exists(),'limit control emitted success: '+ident)
 text=p.read_text();check('incomplete-observation:'in text,'wrong error category')
 results.append({'id':ident,'status':'PASS','error':text})
out.write_text(json.dumps({'schema':'oxid-unit3a-observer-limits-v1','controls':results,'pass':len(results)},indent=2)+'\n');print('4 observer limits failed closed')
