#!/usr/bin/env python3
"""Write or verify the exact pre-execution package manifest."""
from pathlib import Path
import argparse,hashlib,json

HERE=Path(__file__).resolve().parent
MANIFEST=HERE/'pre-execution-manifest.json'
def inventory():
    out=[]
    for p in sorted(HERE.rglob('*')):
        if not p.is_file() or p==MANIFEST or '__pycache__' in p.parts:continue
        data=p.read_bytes();out.append(dict(path=p.relative_to(HERE).as_posix(),bytes=len(data),sha256=hashlib.sha256(data).hexdigest()))
    return out
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--verify',action='store_true');args=parser.parse_args()
    if args.verify:
        expected=json.loads(MANIFEST.read_text());assert expected['files']==inventory(),'frozen package changed'
        print(json.dumps(dict(status='EXACT_PRE_EXECUTION_PACKAGE_MATCH',manifest_sha256=hashlib.sha256(MANIFEST.read_bytes()).hexdigest(),files=len(expected['files']))));return
    if MANIFEST.exists():raise SystemExit('Refusing to overwrite an existing freeze; preserve it and review any correction separately')
    payload=dict(schema='oxid-unit3-independent-pre-execution-v1',freeze_label='unit3-source-oracles-v1',
        evidence_kind='MODEL_ONLY_AND_SOURCE_INSPECTION',candidate_observations_read=0,compiler_invocations=0,llvm_invocations=0,
        approved_design_sha256='f8351b4e63d68cf11995192bad93fb2b527020e9eeab11572ca0dc15452302a8',
        corrected_unit2_manifest_sha256='bcbbebf6f22d5055069cdd61dee663e15470f2d4215d739725ca935fb2f8b24f',
        expected_file='expected.jsonl.gz',source_only_request_file='requests.jsonl',mutation_expectation_file='mutation-expectations.json',
        source_only_mutation_request_file='mutation-requests.json',coverage=json.loads((HERE/'coverage.json').read_text()),
        mutation_count=114,files=inventory())
    MANIFEST.write_text(json.dumps(payload,indent=2)+'\n')
    print(json.dumps(dict(manifest=str(MANIFEST),sha256=hashlib.sha256(MANIFEST.read_bytes()).hexdigest(),files=len(payload['files']))))

if __name__=='__main__':main()
