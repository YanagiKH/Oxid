#!/usr/bin/env python3
"""Identity-bound minimal cfg(test) adapter materialization; no compiler invocation."""
from pathlib import Path
import json,hashlib,shutil,difflib
R=Path(__file__).resolve().parent; B=R.parent/'oxid-typed-project-execution'; S=R/'source-v2'
M=R.parent/'typed-project-unit3-evidence/source-inputs-core-v1.json'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(M)=='53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
assert not S.exists(),'do not overwrite evidence'
m=json.loads(M.read_text()); rows=[]
for x in m['files']:
 p=B/x['path'];assert sha(p)==x['sha256']and p.stat().st_size==x['bytes'],x['path']
 rows.append({'path':x['path'],'verified_sha256':sha(p)})
for x in m['files']:
 p=S/x['path'];p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(B/x['path'],p)
for name in ['tests/fixtures','fixtures']:shutil.copytree(B/name,S/name,dirs_exist_ok=True)
original={}
def edit(name,fn):
 p=S/name;data=p.read_text();original[name]=data;p.write_text(fn(data))
def replace(s,a,b):assert s.count(a)==1,(a,s.count(a));return s.replace(a,b)
J='crate::frontend::driver::unit3_native_driver'
edit('src/frontend/mod.rs',lambda s:s+'\n#[cfg(test)]\npub fn qualification_main() -> i32 { driver::unit3_native_driver::main() }\n')
edit('src/frontend/driver.rs',lambda s:s+'\n#[cfg(test)]\npub(in crate::frontend) mod unit3_native_driver;\n')
p=S/'src/frontend/driver/unit3_native_driver.rs';p.parent.mkdir();shutil.copyfile(R/'adapter.rs',p)
edit('src/frontend/oir/native.rs',lambda s:replace(s,'        self.native_module_fuel(entry, sources, execute::MAX_FUEL)','        let fuel = execute::MAX_FUEL;\n        #[cfg(test)]\n        let fuel = '+J+'::fuel(fuel);\n        self.native_module_fuel(entry, sources, fuel)'))
edit('src/frontend/oir/owned/native.rs',lambda s:replace(s,'    native_module_limits(witness, entry, sources, plan::MAX_FUEL, Limits::DEFAULT)','    let fuel = plan::MAX_FUEL;\n    #[cfg(test)]\n    let fuel = '+J+'::fuel(fuel);\n    native_module_limits(witness, entry, sources, fuel, Limits::DEFAULT)'))
edit('src/frontend/oir/execute.rs',lambda s:replace(s,'        fuel: limits.fuel.min(MAX_FUEL),','        #[cfg(test)]\n        fuel: '+J+'::fuel(limits.fuel).min(MAX_FUEL),\n        #[cfg(not(test))]\n        fuel: limits.fuel.min(MAX_FUEL),'))
edit('src/frontend/oir/owned/execute.rs',lambda s:replace(s,'    let limits = limits.bounded();','    let limits = limits.bounded();\n    #[cfg(test)]\n    let limits = Limits { fuel: '+J+'::fuel(limits.fuel), ..limits };'))
def native(s):
 s=replace(s,'fn run(tool: &Path, args: &[&OsStr], cwd: &Path) -> Result<Output, Box<Diagnostic>> {','fn run(tool: &Path, args: &[&OsStr], cwd: &Path) -> Result<Output, Box<Diagnostic>> {\n    #[cfg(test)]\n    '+J+'::record_tool(tool, args, cwd);')
 return replace(s,'pub(super) fn compile(module: &str, output: &str) -> Result<(), Box<Diagnostic>> {','pub(super) fn compile(module: &str, output: &str) -> Result<(), Box<Diagnostic>> {\n    #[cfg(test)]\n    '+J+'::record_module(module);')
edit('src/frontend/native.rs',native)
patch=[]
for path,a in original.items():patch.extend(difflib.unified_diff(a.splitlines(True),(S/path).read_text().splitlines(True),fromfile='a/'+path,tofile='b/'+path))
name='src/frontend/driver/unit3_native_driver.rs';patch.extend(difflib.unified_diff([],(S/name).read_text().splitlines(True),fromfile='/dev/null',tofile='b/'+name))
(R/'overlay-v2.patch').write_text(''.join(patch))
(R/'main.rs').write_text('#![allow(dead_code,unused_imports)]\n#[path="source-v2/src/frontend/mod.rs"] mod frontend;\nfn main(){ std::process::exit(frontend::qualification_main()); }\n')
manifest={'schema':1,'compiler_executed':False,'core_manifest_sha256':sha(M),'verified_core_count':len(rows),'overlay_sha256':sha(R/'overlay-v2.patch'),'adapter_sha256':sha(R/'adapter.rs'),'main_sha256':sha(R/'main.rs'),'files':[{'path':str(p.relative_to(S)),'sha256':sha(p),'bytes':p.stat().st_size}for p in sorted(S.rglob('*'))if p.is_file()]}
(R/'overlay-manifest-v2.json').write_text(json.dumps(manifest,indent=2)+'\n')
requests={}
for name in ['native-requests.json','driver-requests.proposed.json','fuel-requests.proposed.json']:
 for x in json.loads((R/name).read_text()):requests[x['id']]=x
for x in requests.values():
 root=R/'fixtures'/x['id'];root.mkdir(parents=True)
 for sf in x['source_files']:
  source=R.parent/'typed-project-unit3-oracles'/x['source_root']/sf['path'];assert sha(source)==sf['sha256'];assert source.stat().st_size==sf['bytes'];target=root/sf['path'];target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,target)
(R/'input-verification.json').write_text(json.dumps({'schema':1,'core':rows,'fixture_count':len(requests),'fixture_ids':sorted(requests),'compiler_executed':False},indent=2)+'\n')
print(json.dumps({k:v for k,v in manifest.items()if k!='files'}))
