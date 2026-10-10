#!/usr/bin/env python3
"""Inactive candidate-only source-v2 authority/seal serializer; never activates."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import types
sys.dont_write_bytecode=True
HOLD=False
RECIPE_PATH='tests/qualification/lexer_reservation_current/generate_source_v2.py'
CANDIDATE_PARENT=Path('/workspace/scratch/8266abf56995/source-v2-final-candidates-attempt1')
PROOF_DIR=Path('/workspace/scratch/8266abf56995/lexer-reservation-evidence/publication-ci-required/crosshost-source-candidate-proof')
C2='b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489'
T2='7f5c9aa08c569c4d0b5a27391d8fc68337075d36'
C1='c8e9a72afd9866f32b96f98ae24f61390039f421'
T1='9b35515f096b5619d4d5b3d4b0cb88ea2ccf2c37'
BASE='e3c1b4a1a3ef457326f11a802896c125202fe797'
BT='5fdb4f06a8fcbc61724676df55a4c3bee130eaf7'
SOURCE_DIR='tests/fixtures/typed_project_source_binding/'
SAVED_DIR='tests/fixtures/typed_project_source_binding_byte_storage_v1/'
Q='tests/qualification/lexer_reservation_current/'
PINS={
 'report.json':'4b2b4482b312f9d7bddc7f5bea6bbc401331dcef199a4f129c44578e2fef4703',
 'lexer-reservation-source-v2.candidate.json':'9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261',
 'lexer-reservation-transition-v2.candidate.patch':'c8cbadeac02f6876b884c36d0d7d00f7dee275480cdc4b99542e10cf0a1b4438',
 'current-input-identities.json':'33ae2d9d93b33688ee668b11b7a66ccdd4454004a40a8f5142ff25ab579dbb08',
 'predecessor-input-identities.json':'591d66894c359c5555b293b18edf23955ca8174b8ba12fc6d63298494e5a8618',
 'changed-input-identities.json':'ede93d320d66610dba30bd3b0a1549803253dccdaef6e99e9e91eb45a83db537',
 'fixture-include-accounting-identities.json':'3db2ba3ab0db41ee9172d31e1a73ed7c8a058bbf704ca8ceae28b6331df88e09',
}
OUTPUTS=('lexer-reservation-source-v2.json','lexer-reservation-authority-v2.json',
 'lexer-reservation-transition-v2.patch','seal.py','source-generation-receipt.json')
MUTABLE_AUTHORITY_FIELDS={'adapter_head','reviewed_source_head','source_only_tree','current_source_bytes',
 'current_source_sha256','current_input_identities','transition_inputs','transition_patch_bytes',
 'transition_patch_sha256','transition_paths'}


def need(ok,message):
 if not ok:raise ValueError(message)

def sha(raw):return hashlib.sha256(raw).hexdigest()
def serial(value):return (json.dumps(value,sort_keys=True,indent=2)+'\n').encode()
def row(name,raw):return {'path':name,'bytes':len(raw),'sha256':sha(raw)}
def identity(name,raw):return dict(row(name,raw),mode='100644',git_blob=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest())
def regular(path):
 need(path.is_file() and not any(p.is_symlink() for p in (path,*path.parents)),'unsafe regular input '+str(path));return path.read_bytes()
def module(name,path,raw):
 value=types.ModuleType(name);value.__file__=str(path);exec(compile(raw,str(path),'exec'),value.__dict__);return value


def derive(repository,evidence,adapter_head):
 repository=Path(repository).resolve();evidence=Path(evidence).resolve()
 need(re.fullmatch('[0-9a-f]{40}',adapter_head) is not None,'explicit full reviewed A2 required')
 temp={k:os.environ.get(k) for k in ('TMPDIR','TEMP','TMP')}
 need(len(set(temp.values()))==1 and None not in temp.values(),'three reviewed workspace temporary variables required')
 td=Path(temp['TMPDIR']);need(td.is_dir() and td==td.resolve() and td.stat().st_dev==repository.stat().st_dev,'workspace temporary directory')
 env={'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','HOME':'/nonexistent/oxid-source-git-home',
  'GIT_CONFIG_NOSYSTEM':'1','GIT_CONFIG_GLOBAL':'/dev/null','GIT_NO_REPLACE_OBJECTS':'1','GIT_TERMINAL_PROMPT':'0',**temp}
 def git(*args):return subprocess.check_output(['/usr/bin/git','-c','core.hooksPath=/dev/null','-c','core.fsmonitor=false','-C',str(repository),*args],env=env)
 def blob(commit,name):return git('show',commit+':'+name)
 for commit,tree in ((C2,T2),(C1,T1),(BASE,BT)):need(git('rev-parse',commit+'^{tree}').decode().strip()==tree,'fixed source tree')
 # The external reviewed invocation supplies A2 after the enabled recipe is committed.
 # A2 is not the generated-output commit or a self-filled source literal.
 need(blob(adapter_head,RECIPE_PATH)==regular(Path(__file__).resolve()),'A2 does not contain this exact enabled serializer')
 git('merge-base','--is-ancestor',C2,adapter_head)
 data={name:regular(evidence/name) for name in PINS}
 for name,raw in data.items():need(sha(raw)==PINS[name],'changed independent source proof '+name)
 report=json.loads(data['report.json']);manifest_raw=data['lexer-reservation-source-v2.candidate.json'];manifest=json.loads(manifest_raw)
 patch=data['lexer-reservation-transition-v2.candidate.patch']
 old_manifest_raw=blob(C2,SOURCE_DIR+'byte-storage-source-v1.json')
 need(sha(old_manifest_raw)=='402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa','byte-storage predecessor')
 old_manifest=json.loads(old_manifest_raw)
 old_authority_raw=blob(C2,SOURCE_DIR+'lexer-reservation-authority-v1.json')
 need(sha(old_authority_raw)=='90e995e932ea3a0d5697936efef04569886d7d6d2f353a5e032e13e8350cb046','immutable source-v1 authority')
 old_authority=json.loads(old_authority_raw)
 source_v1_raw=blob(C2,SOURCE_DIR+'lexer-reservation-source-v1.json')
 need(sha(source_v1_raw)=='aa021e6046786300d13b12c22e5cff3f2565b1ae8f739e0698bdad93fd33a633','immutable source-v1 manifest')
 source_v1=json.loads(source_v1_raw)
 need((manifest['reviewed_source_head'],manifest['source_only_tree'])==(C2,T2),'source checkpoint')
 omitted={'files','purpose','reviewed_source_head','source_only_tree','lexer_reservation_predecessor_sha256'}
 need({k:v for k,v in manifest.items() if k not in omitted}=={k:v for k,v in old_manifest.items() if k not in omitted},'retained manifest provenance')
 names=[r['path'] for r in manifest['files']]
 need(names==sorted(set(names)) and len(names)==376,'complete ordered source membership')
 def committed(commit,expected):
  bodies={n:blob(commit,n) for n in names};rows=[identity(n,bodies[n]) for n in names]
  need([row(n,bodies[n]) for n in names]==expected['files'],'committed source manifest')
  tree={}
  for item in git('ls-tree','-r','-z',commit,'--',*names).split(b'\0'):
   if not item:continue
   meta,name=item.split(b'\t',1);mode,kind,obj=meta.decode().split();need(kind=='blob' and mode=='100644','source Git mode/type')
   tree[name.decode()]=(mode,obj)
  need(tree=={r['path']:(r['mode'],r['git_blob']) for r in rows},'complete source Git identity')
  complete=[x.split(b'\t',1)[1].decode() for x in git('ls-tree','-r','-z',commit,'--','src','native').split(b'\0') if x]
  need(sorted(complete)==[n for n in names if n.startswith(('src/','native/'))] and len(complete)==288,'complete compiler directories')
  return bodies,rows
 current,current_ids=committed(C2,manifest);old,old_ids=committed(BASE,old_manifest);v1,v1_ids=committed(C1,source_v1)
 need([n for n in names if current[n]!=v1[n]]==['src/frontend/project/enum_index_tests.rs'],'one approved C1-to-C2 change')
 changed=json.loads(data['changed-input-identities.json']);paths=[n for n in names if current[n]!=old[n]]
 need(paths==changed['transition_paths'] and len(paths)==15,'exact fifteen transition paths')
 need(git('diff','--binary','--no-ext-diff','--no-renames','--abbrev=7',BT,T2,'--',*paths)==patch,'exact accepted Git patch')
 need(sum(line.startswith(b'@@ ') for line in patch.splitlines())==62,'exact hunk count')
 old_source_helper=blob(C2,Q+'source_transition.py')
 need(sha(old_source_helper)=='6e86082f71df4b135ffcffec03ccbe71850dfa82cebd8378bc3b1081dec3142a','retained public source helper')
 helper=module('serializer_retained_source',repository/Q/'source_transition.py',old_source_helper)
 saved=repository/SAVED_DIR
 package_raw=regular(saved/'package-manifest.json');need(sha(package_raw)==helper.RETAINED_PACKAGE_SHA,'retained package manifest')
 package={'package-manifest.json':package_raw}
 for rec in json.loads(package_raw)['files']:
  raw=regular(saved/rec['path']);need(row(rec['path'],raw)==rec,'retained public package member');package[rec['path']]=raw
 api,_=helper.retained_public_api(saved,package)
 recovered,touched=api.apply_inverse_patch(current,patch,sha(patch),len(patch),tuple(paths))
 need(recovered==old and list(touched)==paths,'whole complete source-v2 inverse')
 old_patch=blob(C2,SOURCE_DIR+'lexer-reservation-transition-v1.patch')
 need(sha(old_patch)=='a9c58470b2c280a4ecc0bdd253e39354d0ae037048181952a89545fadec80255','immutable v1 patch')
 need(git('diff','--binary','--no-ext-diff','--no-renames','--abbrev=7',BT,T1,'--',*old_authority['transition_paths'])==old_patch,'unchanged v1 Git recipe')
 old_recovered,old_touched=api.apply_inverse_patch(v1,old_patch,sha(old_patch),len(old_patch),tuple(old_authority['transition_paths']))
 need(old_recovered==old and list(old_touched)==old_authority['transition_paths'],'unchanged complete v1 inverse')
 includes=api.include_directives(current,api);fixtures=[r for r in current_ids if r['path'].startswith(('fixtures/','tests/fixtures/'))]
 accounting=[identity(n,current[n]) for n in helper.ACCOUNTING_PATHS]
 need(includes==api.include_directives(old,api)==old_authority['compile_time_include_directives'] and len(includes)==136,'unchanged includes')
 need(fixtures==old_authority['compile_time_fixture_inputs'] and len(fixtures)==78,'unchanged fixtures')
 need(accounting==old_authority['unit2_accounting_dependencies'] and all(current[n]==old[n] for n in helper.ACCOUNTING_PATHS),'unchanged accounting')
 need(current_ids==json.loads(data['current-input-identities.json'])['files'] and old_ids==json.loads(data['predecessor-input-identities.json'])['files'],'independent complete identities')
 transitions=[{'path':n,'before':identity(n,old[n]),'after':identity(n,current[n])} for n in paths]
 need(transitions==changed['changes'] and changed['compiler_additions']==changed['fixture_additions']==changed['removed_paths']==[],'independent transition identities')
 closure=json.loads(data['fixture-include-accounting-identities.json'])
 need(closure==dict(fixtures=fixtures,ordered_includes=includes,include_additions=[],unit2_accounting_dependencies=accounting),'independent fixture/include/accounting closure')
 authority=dict(old_authority)
 authority.update(adapter_head=adapter_head,reviewed_source_head=C2,source_only_tree=T2,current_source_bytes=len(manifest_raw),
  current_source_sha256=sha(manifest_raw),current_input_identities=current_ids,transition_inputs=transitions,
  transition_patch_bytes=len(patch),transition_patch_sha256=sha(patch),transition_paths=paths)
 need({k:v for k,v in authority.items() if k not in MUTABLE_AUTHORITY_FIELDS}=={k:v for k,v in old_authority.items() if k not in MUTABLE_AUTHORITY_FIELDS},'authority scope changed')
 need((authority['current_source_members'],authority['predecessor_source_members'],authority['compiler_source_members'],authority['compiler_bodies'])==(len(current),len(old),288,291),'complete source counts')
 authority_raw=serial(authority)
 seal={'schema':'oxid-lexer-reservation-seal-v1','source_manifest':row(OUTPUTS[0],manifest_raw),
  'authority':row(OUTPUTS[1],authority_raw),'patch':row(OUTPUTS[2],patch),'compiler_head':C2,'compiler_tree':T2,'adapter_head':adapter_head}
 seal_raw=b'"""Exact generated source seal. Qualification requires independent review."""\nSEAL = '+repr(seal).encode()+b'\n'
 outputs=dict(zip(OUTPUTS[:4],(manifest_raw,authority_raw,patch,seal_raw)))
 outputs[OUTPUTS[4]]=serial({'status':'source-v2-candidates-awaiting-whole-artifact-review','compiler_head':C2,'compiler_tree':T2,'adapter_head':adapter_head,
  'proof_sha256':PINS['report.json'],'full_inverse_body_equality':True,'unchanged_source_v1_inverse':True,
  'outputs':[row(n,b) for n,b in outputs.items()],'source_materializations':0,'compiler_invocations':0,'execution_qualified':False})
 return outputs


def main():
 need(not HOLD,'HOLD: review executable recipe/A2 and exact staged candidate invocation before enabling')
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--repository',type=Path,required=True);parser.add_argument('--source-proof-dir',type=Path,required=True)
 parser.add_argument('--adapter-head',required=True);parser.add_argument('--candidate-output-dir',type=Path,required=True)
 args=parser.parse_args();out=args.candidate_output_dir;repo=args.repository.resolve()
 need(repo==CANDIDATE_PARENT/'stage' and args.source_proof_dir==PROOF_DIR and out==CANDIDATE_PARENT/'source','exact reviewed staged input/output invocation')
 need(out.is_absolute() and out==out.resolve(strict=False) and not out.exists() and out.parent.is_dir(),'fresh absolute candidate directory')
 need(not any(p.is_symlink() for p in (out,*out.parents)) and not(out==repo or repo in out.parents or out in repo.parents),'candidate path overlap')
 products=derive(repo,args.source_proof_dir,args.adapter_head)
 need(tuple(products)==OUTPUTS,'closed source output roster');out.mkdir()
 for name,raw in products.items():
  with (out/name).open('xb') as f:f.write(raw)
 print(json.dumps({'status':'candidate-only','outputs':[row(n,b) for n,b in products.items()]},sort_keys=True))

if __name__=='__main__':main()
