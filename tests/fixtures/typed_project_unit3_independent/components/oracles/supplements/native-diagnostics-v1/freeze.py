#!/usr/bin/env python3
from pathlib import Path
import hashlib,json
from build import verify,ORIGINAL
HERE=Path(__file__).resolve().parent
verify();path=HERE/'supplement-manifest.json';rows=[]
for p in sorted(HERE.iterdir()):
    if p.is_file() and p!=path:
        data=p.read_bytes();rows.append(dict(path=p.name,bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
payload=dict(schema='unit3-native-diagnostic-supplement-manifest-v1',parent_manifest_sha256=ORIGINAL,
             evidence_kind='SOURCE_ONLY_PRE_EXECUTION',files=rows)
if path.exists():assert json.loads(path.read_text())==payload,'supplement changed after freeze'
else:path.write_text(json.dumps(payload,indent=2)+'\n')
print(json.dumps(dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),files=len(rows))))
