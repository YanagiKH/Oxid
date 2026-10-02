#!/usr/bin/env python3
if not __debug__:
    raise SystemExit('Qualification refuses optimized Python because it can remove verification guards')
import argparse,hashlib,json,os,time
from pathlib import Path
root=Path(__file__).resolve().parent
ap=argparse.ArgumentParser();ap.add_argument('--output',required=True);args=ap.parse_args()
m=json.loads((root/'resource-fixture-manifest.json').read_text())
result=[]
for case in m['fixtures']:
    d=root/'resource-fixtures'/case['id']
    expected={f['path']:f for f in case['files']}
    paths={str(p.relative_to(d)):p for p in d.rglob('*') if p.is_file()}
    assert paths.keys()==expected.keys(),case['id']
    for rel,p in paths.items():
        data=p.read_bytes();r=expected[rel]
        assert len(data)==r['bytes'] and hashlib.sha256(data).hexdigest()==r['sha256'],(case['id'],rel)
    entry_names=[os.fsencode(x) for x in os.listdir(d)]
    e=case['expected']
    if 'directory_entries' in e:assert len(entry_names)==e['directory_entries'],case['id']
    if 'directory_name_units' in e:assert sum(map(len,entry_names))==e['directory_name_units'],case['id']
    result.append({'id':case['id'],'source_inventory_ok':True,'root_entries':len(entry_names),'root_name_bytes':sum(map(len,entry_names)),
                   'ordered_native_names_sha256':hashlib.sha256(b'\x00'.join(sorted(entry_names))).hexdigest()})
record={'observed_at_utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'results':result}
(root/args.output).write_text(json.dumps(record,indent=2)+'\n')
print('Verified exact source/path inventories for',len(result),'resource roots')
