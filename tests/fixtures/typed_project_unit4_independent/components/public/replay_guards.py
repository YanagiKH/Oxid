#!/usr/bin/env python3
"""Actual public no-clobber replay against existing frozen exact envelopes."""
import argparse,copy,json,os
from pathlib import Path
import public_cli as h
PROJECTION=h.OUT/'guard-projection-v1.json'
PROJECTION_SHA='5c8e0c09b96f2f8700393050a7da6a4f24626c05284dc18cafaa52ed42d5d44a'

def subst(text,root,output):return text.replace('${FIXTURE_ROOT}',str(root)).replace('${OUTPUT}',str(output))
def check(row,got,cfg):
 h.need(got['key']==row['key'] and got['profile']==row['profile'],'guard identity')
 h.need(got['projection_sha256']==PROJECTION_SHA and got['candidate_binding_sha256']==cfg['_binding_sha256'],'guard provenance')
 h.need(got['executable_before']==got['executable_after']==cfg['binaries'][row['profile']],'guard binary')
 h.need(got['source_files']==row['source_files'] and got['source_before']==got['source_after'],'guard source preservation')
 h.need(h.sources_from_inventory(got['source_before'])==sorted(row['source_files'],key=lambda x:x['path']),'guard source map')
 h.need(got['output_before']==got['output_after'],'guard output clobbered')
 h.need(got['output_before']['kind']==row['output_kind'],'guard output kind')
 h.need(got['native_workspaces_after']==[],'native workspace remains')
 h.need(got['tool_invocations']==[],'denied compile invoked tools')
 p=got['process'];h.need(p['argv']==[cfg['binaries'][row['profile']]['path']]+[subst(x,got['fixture_root'],got['output']) for x in row['argv_template'][1:]],'guard argv')
 h.need(p['status']==row['frozen_envelope']['exit_status'] and not p['timed_out'],'guard process status')
 for s in ('stdout','stderr'):
  h.need(p[s]==subst(row['frozen_envelope'][s],got['fixture_root'],got['output']),'guard exact '+s)
  h.need(h.sha(p[s].encode())==p[s+'_sha256'],'guard stream hash')

def output_state(output,target):
 return {'kind':'symlink' if output.is_symlink() else 'regular','output_sha256':h.fsha(output),'target_sha256':h.fsha(target),'link_target':os.readlink(output) if output.is_symlink() else None}

def main():
 a=argparse.ArgumentParser();a.add_argument('--binding',required=True);a.add_argument('--run',default='guard-run-v1');args=a.parse_args()
 h.need(h.fsha(PROJECTION)==PROJECTION_SHA,'guard projection changed');table=h.load(PROJECTION)
 for row in table['authority_bindings']:h.verify_file(row)
 h.need(h.host()==table['host'],'guard host');cfg=h.read_config(args.binding)
 run=h.OUT/args.run;run.mkdir(exist_ok=False);env,tools=h.tools_setup(run);h.save(run/'tool-bindings.json',tools)
 observations=[];failures=[]
 for i,row in enumerate(table['rows']):
  d=run/f'{i:02d}-{row["profile"]}';d.mkdir();fixture_root=d/'fixtures';fixture=fixture_root/row['id']
  source_row={'source_files':row['source_files'],'case':{'source_root':row['source_root']}}
  h.materialize(source_row,fixture)
  target=d/'preserved-target';target.write_bytes(b'Oxid Unit3 no-clobber sentinel\x00\xff\n')
  output=d/'occupied-output'
  if row['output_kind']=='symlink':output.symlink_to(target.name)
  else:output.write_bytes(b'Oxid Unit3 occupied output sentinel\x00\xfe\n')
  binary=cfg['binaries'][row['profile']]
  got={'key':row['key'],'profile':row['profile'],'projection_sha256':PROJECTION_SHA,'candidate_binding_sha256':cfg['_binding_sha256'],
   'source_files':row['source_files'],'source_before':h.inventory(fixture),'output_before':output_state(output,target),
   'fixture_root':str(fixture_root),'output':str(output),'executable_before':h.binding(binary['path'])}
  log=d/'tool-log.jsonl';per_env={**env,'UNIT4_TOOL_LOG':str(log),'UNIT4_TOOL_EVIDENCE':str(d)}
  argv=[binary['path']]+[subst(x,fixture_root,output) for x in row['argv_template'][1:]]
  got['process']=h.proc(argv,fixture,per_env)
  got['source_after']=h.inventory(fixture);got['output_after']=output_state(output,target);got['executable_after']=h.binding(binary['path'])
  got['native_workspaces_after']=[str(x.relative_to(d)) for x in d.rglob('.oxid-native-*')]
  got['tool_invocations']=[json.loads(x) for x in log.read_text().splitlines()] if log.exists() else []
  try:check(row,got,cfg);got['comparison']='PASS'
  except (h.Reject,KeyError,TypeError,ValueError) as e:got['comparison']='FAIL';failures.append({'key':row['key'],'reason':str(e)})
  for stream in ('stdout','stderr'):(d/stream).write_text(got['process'][stream])
  h.save(d/'receipt.json',got);observations.append(got)
 h.need(len(observations)==20 and {x['key'] for x in observations}=={x['key'] for x in table['rows']},'guard roster mismatch')
 controls=[]
 if not failures:
  for name,fn in [('clobbered-output',lambda a:a['output_after'].__setitem__('output_sha256','wrong')),
   ('tool-on-denial',lambda a:a['tool_invocations'].append({'tool':'unexpected'})),('changed-source',lambda a:a['source_after'].pop()),
   ('wrong-argv',lambda a:a['process']['argv'].append('--fake')),('wrong-status',lambda a:a['process'].__setitem__('status',0)),
   ('wrong-stream',lambda a:a['process'].__setitem__('stdout','forged')),('stale-binary',lambda a:a['executable_after'].__setitem__('sha256','wrong'))]:
   g=copy.deepcopy(observations[0]);fn(g)
   try:check(table['rows'][0],g,cfg)
   except (h.Reject,KeyError,TypeError,ValueError) as e:controls.append({'name':name,'rejected':True,'reason':str(e)})
   else:raise h.Reject('guard mutation accepted '+name)
 for row in table['authority_bindings']:h.verify_file(row)
 h.read_config(args.binding)
 report={'status':'FAIL' if failures else 'PASS','actual_public_compiles':len(observations),'successful_native_claims':0,
  'failures':failures,'negative_controls':controls,'projection':h.binding(PROJECTION),'observer':h.binding(__file__),
  'public_harness':h.binding(h.__file__),'roster':[x['key'] for x in observations]}
 h.save(run/'comparison.json',report);print(json.dumps(report,indent=2));return bool(failures)
if __name__=='__main__':
 try:raise SystemExit(main())
 except h.Reject as e:print('REJECT:',e);raise SystemExit(1)
