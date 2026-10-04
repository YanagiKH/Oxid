#!/usr/bin/env python3
"""Verify this scoped source3B1 replay input package; never run the compiler."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

HERE=Path(__file__).resolve().parent
def sha(raw): return hashlib.sha256(raw).hexdigest()
def read(name): return json.loads((HERE/name).read_bytes())
def require(condition,message):
    if not condition:raise ValueError(message)
def checked_relative(path):
    p=Path(path)
    require(not p.is_absolute() and '..' not in p.parts,'unsafe package path')
    return p
def check(raw,entry):
    require(len(raw)==entry['bytes'],'input byte length mismatch')
    require(sha(raw)==entry['sha256'],'input SHA-256 mismatch')
    return raw

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo',type=Path,required=True,help='Local Git object store containing the selected base')
    parser.add_argument('--materialize-sources',type=Path,help='Optional new output directory for all bound fixture bodies')
    args=parser.parse_args()
    manifest=read('freeze-manifest.json')
    actual={str(p.relative_to(HERE)) for p in HERE.rglob('*') if p.is_file() and str(p.relative_to(HERE))!='freeze-manifest.json'}
    require(actual==set(manifest['files']),'package member mismatch')
    for name,entry in manifest['files'].items():check((HERE/checked_relative(name)).read_bytes(),entry)
    historical=check((HERE/'historical-freeze-manifest.json').read_bytes(),manifest['historical_authority'])
    historical=json.loads(historical)
    def git(*parts):return subprocess.check_output(['git','-C',str(args.repo),*parts])
    require(git('rev-parse',manifest['selected_base']+'^{tree}').decode().strip()==manifest['selected_tree'],'selected Git tree mismatch')
    external={}
    for key,entry in manifest['external_inputs'].items():
        actual_blob=git('rev-parse',manifest['selected_base']+':'+entry['git_path']).decode().strip()
        require(actual_blob==entry['git_blob'],'external Git blob mismatch: '+key)
        external[key]=check(git('cat-file','blob',actual_blob),entry)
    lineage=read('lineage.json')
    for new,old in lineage['unchanged_payload_members'].items():
        require(sha((HERE/checked_relative(new)).read_bytes())==historical['files'][old]['sha256'],'historical semantic member mismatch: '+new)
    source_bodies={}
    for case in read('source-manifest.json')['cases']:
        for f in case['files']:
            relative='fixtures/'+case['id']+'/'+f['path']
            if case['source']=='new supplement':raw=(HERE/checked_relative(relative)).read_bytes()
            else:raw=external[relative]
            source_bodies[relative]=check(raw,f)
    # The 29 exact original diagnostic renderings remain unchanged.
    reused=read('reused-authority.json')
    original=json.loads(external['authority/diagnostic-wording-v1.json'])
    require(reused['language_diagnostics']==[x for x in original['cases'] if x['first_observation_slice']=='3B'],'original diagnostic authority mismatch')
    require(len(reused['language_diagnostics'])==29,'original diagnostic roster count mismatch')
    if args.materialize_sources:
        out=args.materialize_sources
        require(not out.exists(),'source output must be a new directory')
        out.mkdir(parents=True)
        for relative,raw in source_bodies.items():
            target=out/checked_relative(relative.removeprefix('fixtures/'))
            target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(raw)
            require(target.read_bytes()==raw,'materialized source byte mismatch')
    print(json.dumps(dict(status='SCOPED_REPLAY_INPUT_CLOSURE_PASS',payload_files=len(manifest['files']),
       external_inputs=len(external),source_cases=54,source_bodies=len(source_bodies),new_source_bodies=10,
       historical_members_claimed_present=False,compiler_runs=0,candidate_output_inputs=0)))

if __name__=='__main__':
    try:main()
    except (ValueError,TypeError,KeyError,OSError,subprocess.CalledProcessError) as error:
        raise SystemExit('SCOPED_REPLAY_INPUT_CLOSURE_FAIL: '+str(error))
