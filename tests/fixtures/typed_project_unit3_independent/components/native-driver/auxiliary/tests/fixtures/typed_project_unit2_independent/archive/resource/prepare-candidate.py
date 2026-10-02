#!/usr/bin/env python3
"""Verify/copy an explicitly named immutable freeze; install reviewer probes only in copy."""
import argparse,hashlib,json,shutil
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('freeze',type=Path);p.add_argument('sha256');a=p.parse_args()
root=Path(__file__).resolve().parent;j=json.loads((a.freeze/'manifest.json').read_text());assert hashlib.sha256((a.freeze/'manifest.json').read_bytes()).hexdigest()==a.sha256
for lane,base in [('source_files',a.freeze/j['source_root']),('evidence_files',a.freeze)]:
 for entry in j[lane]:
  data=(base/entry['path']).read_bytes();assert len(data)==entry['bytes'] and hashlib.sha256(data).hexdigest()==entry['sha256'],entry['path']
copy=root/(a.freeze.name+'-instrumented');assert not copy.exists(),copy
shutil.copytree(a.freeze/j['source_root'],copy)
changes=[]
for parent,payload,destination in [('src/frontend/declaration_index.rs','resource-review-tests.rs','src/frontend/declaration_index/reviewer_resource.rs'),('src/frontend/parser.rs','parser-resource-review-tests.rs','src/frontend/parser/reviewer_resource.rs'),('src/frontend/source.rs','source-identity-review-tests.rs','src/frontend/source/reviewer_resource.rs')]:
 target=copy/destination;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(root/payload,target)
 module=copy/parent;old=module.read_bytes();module.write_text(old.decode()+f'\n#[cfg(test)]\n#[path = "{Path(destination).relative_to(Path(parent).parent)}"]\nmod independent_unit2_resource_review;\n')
 changes.append({'module':parent,'before_sha256':hashlib.sha256(old).hexdigest(),'after_sha256':hashlib.sha256(module.read_bytes()).hexdigest(),'added':destination,'added_sha256':hashlib.sha256(target.read_bytes()).hexdigest()})
(root/(a.freeze.name+'-probe-manifest.json')).write_text(json.dumps({'input_manifest_sha256':a.sha256,'source_files_verified':len(j['source_files']),'evidence_files_verified':len(j['evidence_files']),'copy':str(copy),'instrumentation':changes},indent=2)+'\n')
print(copy)
