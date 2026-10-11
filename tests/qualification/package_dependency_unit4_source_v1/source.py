"""Pure package-source admission bridge for named public Unit4 successors.

The reviewed caller authenticates this entrypoint. It loads only the original
fixed checkout admission helper after checking its immutable body hash.
No preflight worker, materializer, compiler, target or subprocess is invoked.
"""
import hashlib,json,stat,sys,types
from pathlib import Path,PurePosixPath
CHECKOUT='tests/qualification/package_dependency_current/checkout.py'
CHECKOUT_SHA='dd7b5d4f28b5649592dd3fafc935b0b40fa53a1fdaff05979e2aea59ea909c82'
CURRENT='tests/fixtures/package_dependency_source_v1/package-dependency-source-v1.json'
PREDECESSOR='tests/fixtures/typed_project_source_binding/current-source.json'
CURRENT_SHA='f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
PREDECESSOR_SHA='9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
PATCH='tests/fixtures/package_dependency_source_v1/package-dependency-transition-v1.patch'
PATCH_SHA='3b123cfa29136244e62038e885761e68858d2b9b472bc14231d2e7a60a13586d'
SOURCE_HEAD='ce522e393646ce6478848cee31f307cb4b6ad5e8'
SOURCE_TREE='8c3f4837e1fc89a6272ebee3552ce617ce584131'

def need(value,message):
 if not value:raise ValueError(message)
def sha(raw):return hashlib.sha256(raw).hexdigest()
def row(name,raw):return {'path':name,'bytes':len(raw),'sha256':sha(raw)}
def encoded(value):return (json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()
def decode(raw):
 def pairs(items):
  result={}
  for key,value in items:need(key not in result,'duplicate JSON key');result[key]=value
  return result
 def reject(_):raise ValueError('nonfinite JSON')
 return json.loads(raw,object_pairs_hook=pairs,parse_constant=reject)
def safe(name):
 need(isinstance(name,str) and str(PurePosixPath(name))==name and not name.startswith('/') and '..' not in PurePosixPath(name).parts,'unsafe relative path');return name

def regular(path,maximum=4*1024**2):
 path=Path(path);need(path.is_absolute() and path.resolve(strict=True)==path and stat.S_ISREG(path.lstat().st_mode) and not path.stat().st_mode&0o111,'canonical nonexecutable regular input')
 need(path.stat().st_size<=maximum,'input body envelope');return path.read_bytes()

def verify_context(context):
 """Check exact body/manifest relationships; no execution provenance claim."""
 need(context['status']=='NotReady' and context['execution_qualified'] is False,'preparation-only context')
 need(context['source_checkpoint']=={'head':SOURCE_HEAD,'tree':SOURCE_TREE},'reviewed source checkpoint')
 for label,path,digest in (('current',CURRENT,CURRENT_SHA),('predecessor',PREDECESSOR,PREDECESSOR_SHA)):
  raw=context[label+'_manifest'];need(isinstance(raw,bytes) and sha(raw)==digest,'source manifest identity');need(context[label+'_manifest_binding']==row(path,raw),'manifest binding')
  manifest=decode(raw);need(context[label+'_source']==manifest,'manifest declaration drift');inputs=context[label+'_inputs'];expected=manifest['files'];need(isinstance(inputs,dict) and len(inputs)==len(expected)==376,'complete selected source count')
  need(set(inputs)=={entry['path'] for entry in expected},'complete selected source membership');need(sum(map(len,inputs.values()))<=8*1024**2,'selected byte bound')
  for entry in expected:need(row(safe(entry['path']),inputs[entry['path']])==entry,'selected source body drift')
 need(sha(context['transition_patch'])==PATCH_SHA,'exact one-file transition patch')
 before=context['predecessor_inputs'];after=context['current_inputs'];need({name for name in before if before[name]!=after[name]}=={'src/main.rs'},'complete one-file inverse boundary')
 need(context['current_source']['reviewed_source_head']==SOURCE_HEAD and context['current_source']['source_only_tree']==SOURCE_TREE,'manifest checkpoint')
 return True

def capture(repo):
 need(not sys.flags.optimize and sys.dont_write_bytecode,'normal bytecode-disabled Python required')
 repo=Path(repo);need(repo.is_absolute() and repo.resolve(strict=True)==repo and repo.is_dir(),'canonical repository')
 path=repo/CHECKOUT;body=regular(path);need(sha(body)==CHECKOUT_SHA,'fixed checkout helper identity')
 helper=types.ModuleType('authenticated_package_unit4_source_checkout');helper.__file__=str(path);exec(compile(body,str(path),'exec'),helper.__dict__)
 admitted,_=helper.capture(repo)
 current=regular(repo/CURRENT);previous=regular(repo/PREDECESSOR);patch=regular(repo/PATCH)
 context={'schema':'package-dependency-unit4-source-context-v1','repository':str(repo),'current_inputs':admitted['current_inputs'],'predecessor_inputs':admitted['predecessor_inputs'],'current_manifest':current,'predecessor_manifest':previous,'current_source':admitted['current_source'],'predecessor_source':admitted['predecessor_source'],'current_manifest_binding':row(CURRENT,current),'predecessor_manifest_binding':row(PREDECESSOR,previous),'transition_patch':patch,'source_checkpoint':{'head':SOURCE_HEAD,'tree':SOURCE_TREE},'status':'NotReady','execution_qualified':False,'predecessor_preflight':'not-run-by-this-pure-bridge','bridge_helper':row(CHECKOUT,body)}
 verify_context(context);return context
