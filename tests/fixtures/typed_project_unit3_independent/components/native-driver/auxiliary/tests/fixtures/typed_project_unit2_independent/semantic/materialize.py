#!/usr/bin/env python3
"""Materialize the frozen byte corpus; never execute a semantic model."""
from pathlib import Path
import argparse, base64, gzip, hashlib, json

HERE=Path(__file__).resolve().parent
parser=argparse.ArgumentParser();parser.add_argument('output',type=Path);args=parser.parse_args()
manifest=json.loads((HERE/'corpus-manifest.json').read_text())
compressed=(HERE/'corpus.jsonl.gz').read_bytes()
assert hashlib.sha256(compressed).hexdigest()==manifest['compressed_corpus_sha256']
with gzip.open(HERE/'corpus.jsonl.gz','rb') as source:
    payload=source.read(manifest['uncompressed_bytes']+1)
assert len(payload)==manifest['uncompressed_bytes']
cases=[json.loads(line) for line in payload.splitlines()]
assert len(cases)==manifest['cases'] and len({c['id'] for c in cases})==len(cases)
assert hashlib.sha256(('\n'.join(c['id'] for c in cases)+'\n').encode()).hexdigest()==manifest['case_ids_sha256']
assert not args.output.exists(), 'Never overwrite an earlier materialization'
args.output.mkdir(parents=True)
queue=[];tsv=[]
for case in cases:
    directory=args.output/case['cohort']/case['case'];directory.mkdir(parents=True)
    request_bytes=base64.b64decode(case['request_base64'],validate=True)
    assert hashlib.sha256(request_bytes).hexdigest()==case['request_sha256']
    request=json.loads(request_bytes);(directory/'request.json').write_bytes(request_bytes)
    sources={s['path']:s for s in case['sources']}
    assert set(sources)==set(case['creation_order']) and len(sources)==len(case['creation_order'])
    for name in case['creation_order']:
        relative=Path(name);assert not relative.is_absolute() and '..' not in relative.parts
        source=sources[name];data=base64.b64decode(source['base64'],validate=True)
        assert len(data)==source['bytes'] and hashlib.sha256(data).hexdigest()==source['sha256']
        target=directory/'files'/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
    entry=directory/'files'/request['entry']
    row={'case':case['case'],'cohort':case['cohort'],'mode':case['mode'],'request':str((directory/'request.json').relative_to(args.output)),
         'entry':str(entry.relative_to(args.output)),'source_files':request['source_files'],'request_sha256':case['request_sha256'],
         'creation_order':case['creation_order'],'replaces':case['replaces']}
    queue.append(row)
    fields=[case['id'],case['mode'],str(entry.resolve())]
    assert all(not any(c in value for c in '\t\n\r') for value in fields)
    tsv.append('\t'.join(fields))
# The observation input directories receive requests and source bytes only.
assert all(p.name=='request.json' for p in args.output.rglob('*.json'))
(args.output/'queue.json').write_text(json.dumps({'schema':1,'cases':queue},indent=2)+'\n')
(args.output/'queue.tsv').write_text('\n'.join(tsv)+'\n')
print(json.dumps({'cases':len(queue),'source_files':manifest['source_files'],'source_bytes':manifest['source_bytes'],'expectation_files_written':0}))
