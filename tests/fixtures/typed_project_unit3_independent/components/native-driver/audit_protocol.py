#!/usr/bin/env python3
"""Audit transport/native protocol only; semantic comparisons belong to frozen oracle comparator."""
from pathlib import Path
import hashlib,json,collections
R=Path(__file__).resolve().parent
PROFILES=['debug','release']
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
expected=[]
for profile in PROFILES:
 for x in json.loads((R/'native-requests.json').read_text()):expected.append(('native-default',profile,x['id'],'compile','json',None,None))
 for x in json.loads((R/'fuel-requests.frozen.json').read_text()):
  for fuel in x['fuel_budgets']:
   expected.append(('native-fuel',profile,x['id'],'compile','json',fuel,None));expected.append(('reference-fuel',profile,x['id'],'run','text',fuel,None))
 for x in json.loads((R/'driver-requests.proposed.json').read_text()):
  for operation in ['check','run','compile']:
   for fmt in ['text','json']:expected.append(('driver',profile,x['id'],operation,fmt,None,None))
 for x in json.loads((R/'no-clobber-requests.json').read_text())['requests']:
  expected.append(('no-clobber',profile,x['id'],'compile',x['format'],None,'file'if x['output_kind']=='regular'else'symlink'))
rows=[];problems=[]
for p in sorted((R/'receipts').glob('*/*/*/receipt.json')):
 x=json.loads(p.read_text());relative=str(p.relative_to(R));c=x['compiler']
 def require(condition,message):
  if not condition:problems.append({'receipt':relative,'problem':message})
 require(not x.get('qualification_failure'),'qualification failure flag');require(c['status']is not None,'compiler completion')
 require(x['tool_receipt_exists']and isinstance(x['tools'],list),'initialized tool receipt')
 require(x['source_restored']and x['source_restored_after_execution'],'complete restored source bytes')
 require(x.get('source_state')is not None,'source unavailable receipt present')
 if x.get('source_state'):
  require([v['phase']for v in x['source_state']]==['after_load_before_check','restored'],'correct source state transition order')
  require(x['source_state'][0].get('all_source_paths_absent')is True,'all original source paths absent')
  require(set(x['source_state'][0]['files'])=={str(Path(x['fixture_root'])/sf['path'])for sf in x['source_files']},'exact loaded source-file membership')
 for kind in ['stdout','stderr']:
  require(hashlib.sha256(c[kind].encode()).hexdigest()==c[kind+'_sha256'],'exact UTF8 compiler '+kind)
  require(sha(p.parent/kind)==c[kind+'_sha256'],'compiler output file hash '+kind)
 require(sha(R/('adapter-'+x['profile']))==x['binary_sha256'],'current adapter binary identity')
 require(sha(R/('build-'+x['profile'])/'verified-build.json')==x['verified_build_sha256'],'successful profile build identity')
 if x.get('execution'):
  e=x['execution'];require(e['source_unavailable']and e['cwd_before_files']==['program'],'source-free ELF execution')
  require(e['environment']=={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C'},'minimal standalone environment')
  require(e['status']is not None and not e.get('qualification_failure'),'standalone completion')
  require(sha(p.parent/'isolated/program')==x['artifact_sha256']==e['elf_sha256'],'standalone ELF identity')
  require((p.parent/'isolated/program').read_bytes()[:4]==b'\x7fELF','ELF magic')
  require(len(x['tools'])==7,'actual external LLVM command count')
  require([Path(t['tool']).name for t in x['tools']]==['clang','opt','ld.lld','opt','clang','clang','clang'],'actual LLVM command sequence')
  require([t['argv']for t in x['tools'][:3]]==[['--version']]*3,'actual LLVM version checks')
  require(x['tools'][3]['argv']==['-passes=verify','-disable-output','program.ll'],'actual LLVM verifier')
  require(all('-O0'in t['argv']for t in x['tools'][4:6]),'native opt-level0')
  require(sha(p.parent/'actual.ll')==x['ir_sha256'],'actual IR hash')
  for kind in ['stdout','stderr']:require(sha(p.parent/'execution'/kind)==e[kind+'_sha256'],'standalone output file hash '+kind)
 if c['status']!=0:require(x['tools']==[],'all declared rejection cases occur before external tools')
 if x['operation']!='compile':
  require(x['tools']==[],'check/reference never invokes tools')
  allowed={'invocation.json','stdout','stderr','process.json','tools.jsonl','source-state.jsonl','receipt.json'}
  require({v.name for v in p.parent.iterdir()}==allowed,'check/reference creates only explicit evidence receipts, no output/cache')
 require({str(v.relative_to(Path(x['fixture_root'])))for v in Path(x['fixture_root']).rglob('*')if v.is_file()}=={sf['path']for sf in x['source_files']},'fixture tree has no unexpected files/cache')
 if x.get('noclobber_before'):
  require(x['noclobber_before']==x['noclobber_after'],'preexisting file/symlink/content unchanged');require(x['tools']==[]and x['spy_calls']=='','no-clobber cannot invoke LLVM')
  kind='symlink'if x['noclobber_before']['is_symlink']else'file'
 else:kind=None
 key=(x['group'],x['profile'],x['id'],x['operation'],x['format'],x['fuel'],kind)
 rows.append({'key':key,'path':relative,'elf_execution':bool(x.get('execution')),'tool_calls':len(x['tools'])if x['tools']is not None else None})
actual=[tuple(x['key'])for x in rows if x['key'][0]!='smoke'];duplicates=[list(k)for k,n in collections.Counter(actual).items()if n!=1]
missing=sorted(set(expected)-set(actual),key=str);unexpected=sorted(set(actual)-set(expected),key=str)
summary={'schema':1,'semantics_compared':False,'core_manifest_sha256':sha(R.parent/'typed-project-unit3-evidence/source-inputs-core-v1.json'),'expected_primary_invocations':len(expected),'actual_primary_invocations':len(actual),'separate_smoke_invocations':sum(x['key'][0]=='smoke'for x in rows),'groups':dict(collections.Counter(x['key'][0]for x in rows)),'elf_executions_by_group':dict(collections.Counter(x['key'][0]for x in rows if x['elf_execution'])),'missing':missing,'unexpected':unexpected,'duplicates':duplicates,'protocol_problems':problems,'protocol_pass':not(missing or unexpected or duplicates or problems),'receipts':rows}
(R/'protocol-audit.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({k:v for k,v in summary.items()if k!='receipts'},indent=2))
