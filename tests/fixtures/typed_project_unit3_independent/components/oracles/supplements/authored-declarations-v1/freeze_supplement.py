#!/usr/bin/env python3
from pathlib import Path
import hashlib,json
from build_supplement import original_unchanged,ORIGINAL_SHA
HERE=Path(__file__).resolve().parent
path=HERE/'supplement-manifest.json'
original_unchanged()
rows=[]
for p in sorted(HERE.iterdir()):
    if not p.is_file() or p==path:continue
    data=p.read_bytes();rows.append(dict(path=p.name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
payload=dict(schema='unit3-source-only-supplement-manifest-v1',parent_manifest_sha256=ORIGINAL_SHA,
             supplement='authored-declarations-v1',evidence_kind='SOURCE_ONLY_NO_CANDIDATE_INPUT',files=rows)
if path.exists():assert json.loads(path.read_text())==payload,'supplement changed after freeze'
else:path.write_text(json.dumps(payload,indent=2)+'\n')
print(json.dumps(dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),files=len(rows),parent_files_unchanged=True)))
