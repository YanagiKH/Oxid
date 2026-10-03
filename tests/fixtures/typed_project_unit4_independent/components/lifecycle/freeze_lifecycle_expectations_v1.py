#!/usr/bin/env python3
"""Source-derived lifecycle controls; never runs or reads candidate outputs."""
import hashlib,json
from pathlib import Path
OUT=Path(__file__).resolve().parent
ROOT=OUT.parent
PHASES=['checker_attempts','checked_program_completions','route_attempts','route_completions','index_attempts','index_completions']
FILE_PHASES=['source_open_attempt','source_open_complete','source_read_attempt','source_read_complete','lex_attempt','lex_complete','parse_attempt','parse_complete']
def stage(*v):return dict(zip(PHASES,v))
def fs(*v):return dict(zip(FILE_PHASES,v))
full=fs(1,1,1,1,1,1,1,1)
rows=[]
def add(id,files,phases,per_file,route=None,flavor=None,args=None):
    rows.append({'id':id,'files_hex':{k:v.hex() for k,v in files.items()},'expected_lifecycle':phases,
      'file_counts':per_file,'expected_route':route,'expected_flavor':flavor,'expected_status':1,
      'argv':args or ['check','main.ox','--edition=typed-preview','--message-format=json']})
zero=stage(0,0,0,0,0,0)
add('cli-before-read',{'main.ox':b'fn main()->i32{return 1;}'},zero,{},args=['check','main.ox','--edition=typed-preview','--invented','--message-format=json'])
add('missing-root-open',{},zero,{'main.ox':fs(1,0,0,0,0,0,0,0)})
add('invalid-utf8-read',{'main.ox':b'\xff'},zero,{'main.ox':fs(1,1,1,1,0,0,0,0)})
add('oxbc-before-lex',{'main.ox':b'OXBC\x01\0\0\0'},zero,{'main.ox':fs(1,1,1,1,0,0,0,0)})
add('root-byte-limit',{'main.ox':b' '*(1048576+1)},zero,{'main.ox':fs(1,1,1,0,0,0,0,0)})
add('unterminated-comment',{'main.ox':b'/*'},zero,{'main.ox':fs(1,1,1,1,1,0,0,0)})
add('root-parse-stops-child',{'main.ox':b'mod child; fn pub()->i32{return 1;}','child.ox':b'fn child()->i32{return 1;}'},zero,{'main.ox':fs(1,1,1,1,1,1,1,0)})
add('child-lex-stops-later-read',{'main.ox':b'mod child; mod later;','child.ox':b'/*','later.ox':b'fn later()->i32{return 1;}'},zero,{'main.ox':full,'child.ox':fs(1,1,1,1,1,0,0,0)})
add('child-utf8-stops-later-read',{'main.ox':b'mod child; mod later;','child.ox':b'\xff','later.ox':b'fn later()->i32{return 1;}'},zero,{'main.ox':full,'child.ox':fs(1,1,1,1,0,0,0,0)})
index_fail=stage(1,0,1,1,1,0)
add('project-original-conflict',{'main.ox':b'pub fn f()->i32{return 1;} fn f()->i32{return 2;}'},index_fail,{'main.ox':full},'scalar','project')
add('project-import-resolution',{'main.ox':b'use crate::missing;'},index_fail,{'main.ox':full},'scalar','project')
body_fail=stage(1,0,1,1,1,1)
add('project-body-name-resolution',{'main.ox':b'pub fn main()->i32{return missing;}'},body_fail,{'main.ox':full},'scalar','project')
add('original-scalar-type',{'main.ox':b'fn main()->i32{return true;}'},body_fail,{'main.ox':full},'scalar','original')
add('original-owned-type',{'main.ox':b'struct R{} fn main()->i32{return true;}'},body_fail,{'main.ox':full},'owned','original')
add('project-unused-child-type',{'main.ox':b'mod child;','child.ox':b'fn bad()->i32{return true;}'},body_fail,{'main.ox':full,'child.ox':full},'scalar','project')

# Bind all public lifecycle expectations before reading candidate observations.
contract_path=ROOT/'typed-project-unit4-oracles/public-cli-contract-v3.json'
contract=json.loads(contract_path.read_text())
assert hashlib.sha256(contract_path.read_bytes()).hexdigest()=='1cd3b42d2f93158e88b6a37ac99b89b9a10e85c926e2e7eb4d63675ab170073f'
public=[]
for group in ['literal_cases','inherited_negative_and_route_cases']:
  for case in contract[group]:
    expected=case.get('expected_lifecycle') or stage(1,1,1,1,1,1)
    files={f['path']:full for f in case['source_files']}
    if case['id'] in ('host-malformed-root-first','host-old-reserved-name'):
      files={'main.ox':fs(1,1,1,1,1,1,1,0)}
    # This case is not executed on Linux; retain its root-load projection only.
    if case['id']=='host-child-denied':files={'main.ox':full}
    public.append({'id':case['id'],'expected_lifecycle':expected,'file_counts':files,
      'expected_route':case.get('expected_route','scalar' if case['id']=='host-no-main' else None),
      'expected_flavor':('original' if case.get('successful_flavor')=='OriginalSingleFile' else 'project') if expected['checker_attempts'] else None})
result={'authority_sha256':hashlib.sha256(contract_path.read_bytes()).hexdigest(),
  'counter_order':PHASES,'file_counter_order':FILE_PHASES,'public_cases':public,'heldout_cases':rows,
  'derivation':'Public v3 conditionals and exact source phase boundaries; no command status used to infer completion. Heldouts exercise actual CLI default limits; no limit injection.',
  'source_read_definition':'One logical byte-read sequence after a successful open; completion is clean EOF, before OXBC and UTF-8 validation. This is not an OS syscall count.',
  'consumer_source_reopens':0,'candidate_invocations':0,'candidate_outputs_read':0}
p=OUT/'lifecycle-expectations-v1.json';p.write_text(json.dumps(result,indent=2)+'\n')
print(hashlib.sha256(p.read_bytes()).hexdigest(),len(public),len(rows))
