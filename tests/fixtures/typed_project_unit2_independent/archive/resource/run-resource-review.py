#!/usr/bin/env python3
"""Reproduce the independent runtime controls against an exact immutable source freeze.

No expected result is obtained from the candidate. The caller supplies a manifest
hash and a new output directory. All source changes occur only inside that output.
Use the qualified Rust toolchain on PATH and its configured CARGO_HOME.
"""
import argparse,hashlib,json,os,shutil,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('freeze',type=Path);p.add_argument('manifest_sha256');p.add_argument('--output',type=Path,required=True);p.add_argument('--profile',choices=['debug','release','both'],default='both');p.add_argument('--cargo',default='cargo');p.add_argument('--prepare-only',action='store_true');args=p.parse_args()
package=Path(__file__).resolve().parent;freeze=args.freeze.resolve();raw=(freeze/'manifest.json').read_bytes();assert hashlib.sha256(raw).hexdigest()==args.manifest_sha256,'wrong freeze manifest';manifest=json.loads(raw);source=freeze/manifest['source_root'];expected={e['path']:e for e in manifest['source_files']}
assert {str(p.relative_to(source)) for p in source.rglob('*') if p.is_file()}==set(expected),'unmanifested or missing source file'
for name,e in expected.items():
 b=(source/name).read_bytes();assert len(b)==e['bytes'] and hashlib.sha256(b).hexdigest()==e['sha256'],name
output=args.output.resolve();assert not output.exists(),'output must be new';output.mkdir(parents=True);repo=output/'source';shutil.copytree(source,repo);changes=[]
for module,dest,payloads in [
 ('src/frontend/declaration_index.rs','src/frontend/declaration_index/reviewer_resource.rs',['resource-review-tests.rs','supplemental-resource-review-tests.rs','old-admission-review-test.rs','exposure-origin-review-test.rs']),
 ('src/frontend/parser.rs','src/frontend/parser/reviewer_resource.rs',['parser-resource-review-tests.rs']),
 ('src/frontend/source.rs','src/frontend/source/reviewer_resource.rs',['source-identity-review-tests.rs'])]:
 target=repo/dest;assert not target.exists();target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(b''.join((package/f).read_bytes() for f in payloads));parent=repo/module;parent.write_text(parent.read_text()+f'\n#[cfg(test)]\n#[path = "{Path(dest).relative_to(Path(module).parent)}"]\nmod independent_unit2_resource_review;\n');changes.append({'module':module,'module_sha256':hashlib.sha256(parent.read_bytes()).hexdigest(),'test':dest,'test_sha256':hashlib.sha256(target.read_bytes()).hexdigest()})
for name in ['tmp','fixtures']:(output/name).mkdir()
if args.prepare_only:
 (output/'assembly.json').write_text(json.dumps({'candidate_manifest_sha256':args.manifest_sha256,'instrumentation':changes},indent=2)+'\n');print(repo);raise SystemExit(0)
env=dict(os.environ,CARGO_TARGET_DIR=str(output/'target'),CARGO_BUILD_JOBS='2',TMPDIR=str(output/'tmp'),OXID_RESOURCE_FIXTURES=str(output/'fixtures'));results=[]
for profile in (['debug','release'] if args.profile=='both' else [args.profile]):
 command=[args.cargo,'test','--offline','--locked','--bin','oxid']+(['--release']if profile=='release'else[])+['independent_unit2_resource_review','--','--nocapture'];start=time.time();r=subprocess.run(command,cwd=repo,env=env,capture_output=True,text=True);(output/f'{profile}.stdout').write_text(r.stdout);(output/f'{profile}.stderr').write_text(r.stderr);results.append({'profile':profile,'argv':command,'exit_status':r.returncode,'elapsed_seconds':time.time()-start});(output/'receipt.json').write_text(json.dumps({'candidate_manifest_sha256':args.manifest_sha256,'instrumentation':changes,'runs':results},indent=2)+'\n');assert r.returncode==0,f'{profile} failed; inspect preserved output';print(profile,'passed',flush=True)
