#!/usr/bin/env python3
"""Prepare requests containing only case label, scope, source path and limits."""
import hashlib,json,pathlib,sys
contracts,supplement,out=map(pathlib.Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=False)
cases=json.loads((contracts/'cases.json').read_text())['cases'];lines=[]
for c in cases:
 if c['first_observation_slice']!='3A':continue
 root=contracts/'fixtures'/c['id']
 for f in c['files']:
  b=(root/f['path']).read_bytes();assert len(b)==f['bytes'] and hashlib.sha256(b).hexdigest()==f['sha256']
 lines.append('\t'.join([c['id'],'single'if len(c['files'])==1 else 'project',str(root),str(200000),str(16*1024*1024)]))
assert len(lines)==58;(out/'core.tsv').write_text('\n'.join(lines)+'\n')
lines=[]
for c in json.loads((supplement/'cases.json').read_text())['cases']:
 b=(supplement/c['source_path']).read_bytes();assert len(b)==c['source_bytes'] and hashlib.sha256(b).hexdigest()==c['source_sha256']
 root=out/'supplement-fixtures'/c['id'];root.mkdir(parents=True);(root/'main.ox').write_bytes(b)
 lines.append('\t'.join([c['id'],'single',str(root),str(200000),str(16*1024*1024)]))
assert len(lines)==6;(out/'supplement.tsv').write_text('\n'.join(lines)+'\n')
# Separate observer-only failure controls; never source-language rejection evidence.
root=contracts/'fixtures'/'grouped-complete-access-and-index'
(out/'observer-limits.tsv').write_text('\n'.join('\t'.join([label,'single',str(root),str(rows),str(size)])for label,rows,size in [('zero-rows',0,16*1024*1024),('one-row',1,16*1024*1024),('zero-bytes',200000,0),('one-byte',200000,1)])+'\n')
print(json.dumps({'core':58,'supplement':6,'observer_limit_controls':4,'deferred':31}))
