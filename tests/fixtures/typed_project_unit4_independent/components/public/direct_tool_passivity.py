#!/usr/bin/env python3
"""Two fresh ordinary public compiles against direct real LLVM tools.
This is separate from the frozen main roster. No private compiler API is used.
"""
import argparse,json,os
from pathlib import Path
import public_cli as h
p=argparse.ArgumentParser();p.add_argument('--binding',required=True);p.add_argument('--main-run',default='run-v1');p.add_argument('--run',default='direct-tool-passivity-v1');a=p.parse_args()
c,integrity=h.authorities();cfg=h.read_config(a.binding)
main=h.OUT/a.main_run
ordinary=[json.loads(x) for x in (main/'observations.jsonl').read_text().splitlines()]
bykey={x['key']:x for x in ordinary}
rows=[r for r in h.roster(c) if r['case_id']=='public-pilot-scalar' and r['argv_template'][1]=='compile']
h.need(len(rows)==2 and {r['profile'] for r in rows}=={'debug','release'},'passivity profile roster')
run=h.OUT/a.run;run.mkdir(exist_ok=False)
env={**os.environ,'OXID_LLVM_BIN':str(h.LLVM),'LD_LIBRARY_PATH':str(h.LIBS)}
for key in ('UNIT4_TOOL_LOG','UNIT4_TOOL_EVIDENCE','UNIT4_REAL_LLVM'):env.pop(key,None)
tools=h.load(main/'tool-bindings.json')
for t in tools:h.verify_file(t['executable'])
results=[]
for row in rows:
 d=run/row['profile'];d.mkdir();fixture=d/'fixture';h.materialize(row,fixture)
 binary=cfg['binaries'][row['profile']]
 process=h.proc([binary['path']]+row['argv_template'][1:],fixture,env)
 result={'key':row['key'],'profile':row['profile'],'evidence_kind':'DIRECT_REAL_TOOL_ORDINARY_PUBLIC_CLI',
 'executable_before':h.binding(binary['path']),'source_files':row['source_files'],'process':process,
 'output_initially_absent':True,'tool_authorities':[t['executable'] for t in tools]}
 for field in ('status','stdout','stderr'):
  h.need(process[field]==row['observation'][field],f'direct process expected {field}')
  h.need(process[field]==bykey[row['key']]['process'][field],f'wrapper passivity {field}')
 result['native_execution']=h.native_execution(row,fixture,d,env)
 for field in ('status','stdout','stderr'):
  h.need(result['native_execution']['process'][field]==row['case']['native_execution'][field],f'direct native expected {field}')
  h.need(result['native_execution']['process'][field]==bykey[row['key']]['native_execution']['process'][field],f'wrapper native passivity {field}')
 result['wrapped_emitted_llvm']=bykey[row['key']]['emitted_llvm']
 result['direct_emitted_llvm']=None
 result['direct_ir_observation_boundary']='Public compile removes its internal IR workspace; no direct IR identity is claimed'
 result['elf_identity_equal']=result['native_execution']['original_elf']['sha256']==bykey[row['key']]['native_execution']['original_elf']['sha256']
 h.need(result['elf_identity_equal'],'wrapped/direct ELF identity differs; investigate')
 result['executable_after']=h.binding(binary['path']);h.need(result['executable_after']==binary,'stale binary')
 result['directory_after']=h.inventory(fixture)
 h.need(h.sources_from_inventory(result['directory_after'])==sorted(row['source_files'],key=lambda x:x['path']),'direct restored source map')
 h.save(d/'receipt.json',result);results.append(result)
_,after=h.authorities();h.need(after==integrity,'source authority changed')
h.read_config(a.binding)
for t in tools:h.verify_file(t['executable'])
report={'status':'PASS','actual_public_compiles':2,'source_free_elf_runs':2,'main_roster_count':0,
 'direct_tool_invocation_observation':'real tool identities and OXID_LLVM_BIN bound; exact nested calls are observed only in wrapped companions',
 'results':results,'observer':h.binding(__file__),'public_harness':h.binding(h.__file__)}
h.save(run/'comparison.json',report);print(json.dumps({k:v for k,v in report.items() if k!='results'},indent=2))
