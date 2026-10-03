#!/usr/bin/env python3
"""Two actual public real-LLVM failed-publication controls.
All expected outcomes come from the separately authored/reviewed supplement.
"""
import argparse,copy,json,os,stat,subprocess,time
from pathlib import Path
import public_cli as h
SUP=h.ORACLE/'public-concurrent-output-supplement-v1.json'
SUP_SHA='c9b7d968fb0c95c84af199082ce6b20ac5720bb8766a6a3e750081fb0233ce9a'
FREEZE=h.ORACLE/'public-concurrent-output-freeze-v1.json'
FREEZE_SHA='ff4faba405163fdebdb1f6f1ecb0edc3dab5df337500dc841b0ab76e78f4da3b'

def identity(path):
 s=path.stat();return {k:getattr(s,k) for k in ('st_dev','st_ino','st_mode','st_size','st_mtime_ns','st_ctime_ns')}

def verify_authority(cfg):
 h.need(h.fsha(SUP)==SUP_SHA and h.fsha(FREEZE)==FREEZE_SHA,'race freeze identities')
 sup=h.load(SUP)
 for b in sup['spec_bindings']:h.verify_file(b)
 actual=Path(cfg['source_root']);bindings=[]
 for authority in sup['source_authorities']:
  p=actual/authority['path'];data=p.read_bytes()
  if authority['path']!='src/frontend/driver.rs':h.need(h.sha(data)==authority['sha256'],'changed race source authority '+authority['path'])
  for region in authority['regions']:
   start=data.index(region['start_anchor'].encode());end=data.index(region['end_anchor_exclusive'].encode(),start) if region['end_anchor_exclusive'] else len(data)
   part=data[start:end];h.need(len(part)==region['bytes'] and h.sha(part)==region['sha256'],'changed race proof region '+region['label'])
  bindings.append(h.binding(p))
 return sup,bindings

def execute(argv,cwd,env):
 start=time.time_ns();p=subprocess.Popen(argv,cwd=cwd,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
 timed_out=False
 try:stdout,stderr=p.communicate(timeout=120)
 except subprocess.TimeoutExpired:
  timed_out=True;p.kill();stdout,stderr=p.communicate()
 return {'argv':argv,'cwd':str(cwd),'pid':p.pid,'started_ns':start,'completed_ns':time.time_ns(),'status':p.returncode,
  'stdout':stdout.decode(),'stderr':stderr.decode(),'stdout_sha256':h.sha(stdout),'stderr_sha256':h.sha(stderr),'timed_out':timed_out}

def marker_matches(text,marker):
 return any(marker in line and (not line.split(marker,1)[1] or line.split(marker,1)[1].startswith((' ','('))) for line in text.strip().splitlines())

def check(control,got,sup,cfg,toolmap):
 e=sup['shared_failed_publication_expectation'];case=sup['source_case'];p=got['process']
 h.need(got['case_id']==control['case_id'] and got['profile']==control['profile'] and got['host']==control['platform'],'race roster identity')
 h.need(got['supplement_sha256']==SUP_SHA and got['candidate_binding_sha256']==cfg['_binding_sha256'],'race provenance')
 h.need(got['executable_before']==got['executable_after']==cfg['binaries'][control['profile']],'race binary binding')
 h.need(got['initial_out_bin_absent'] and got['initial_native_workspace_absent'],'race initial state')
 h.need(p['argv']==[cfg['binaries'][control['profile']]['path']]+control['argv'][1:] and p['cwd']==got['fixture'],'race actual argv/cwd')
 h.need(p['status']==e['exit_status'] and p['stderr']==e['stderr'] and not p['timed_out'],'race status/stderr')
 for s in ('stdout','stderr'):h.need(h.sha(p[s].encode())==p[s+'_sha256'],'race stream hash')
 lines=p['stdout'].splitlines(keepends=True);h.need(len(lines)==2 and all(l.endswith('\n') for l in lines),'race JSON record framing')
 d=json.loads(lines[0]);summary=json.loads(lines[1]);want=e['diagnostic_stable_fields']
 h.need(set(d)==set(want)|{'message'},'race diagnostic unknown/missing fields')
 h.need({k:d[k] for k in want}==want,'race stable diagnostic projection')
 prefix=e['diagnostic_message']['stable_prefix'];h.need(isinstance(d['message'],str) and d['message'].startswith(prefix) and len(d['message'])>len(prefix),'race host error prefix')
 h.need(summary==e['compile_summary'] and lines[1]==e['compile_summary_json_line'],'race exact compile summary')
 sources=lambda inv:sorted(h.sources_from_inventory(inv),key=lambda s:s['path'])
 h.need(sources(got['directory_before'])==sources(got['directory_after'])==sorted(case['source_files'],key=lambda s:s['path']),'race source integrity')
 h.need([x['path'] for x in got['directory_after'] if x not in got['directory_before']]==e['filesystem']['directory_delta'],'race directory delta')
 h.need(all(x in got['directory_after'] for x in got['directory_before']),'race altered pre-existing file')
 h.need(got['native_workspaces_after']==[],'race native workspace cleanup')
 output=e['filesystem']['out_bin'];h.need(got['output_after']['bytes']==output['bytes'] and got['output_after']['sha256']==output['sha256'],'competitor bytes')
 h.need(got['output_after']['regular_not_symlink'] is True,'competitor file kind')
 calls=got['tool_invocations'];h.need(all(c['status']==0 and c['wrapper_return_status']==0 for c in calls),'race real tool failure')
 for c in calls:
  t=toolmap[c['tool']];h.need(c['real_path']==t['path'] and c['real_sha256']==t['sha256'],'race real tool identity')
  for stream in ('stdout','stderr'):h.need(h.sha(c[stream].encode())==c[stream+'_sha256'],'race tool stream integrity')
 direct=sorted([c for c in calls if c['wrapper_ppid']==p['pid']],key=lambda c:c['started_ns'])
 nested=[c for c in calls if c['wrapper_ppid']!=p['pid']]
 phases=sup['required_tool_phase_evidence']['exact_order'];h.need(len(direct)==sup['required_tool_phase_evidence']['direct_compiler_invocation_count']==len(phases),'race direct tool phase count')
 native_workspace=direct[3]['cwd'];h.need(Path(native_workspace).parent==Path(got['fixture']) and Path(native_workspace).name.startswith('.oxid-native-'),'race native workspace identity')
 previous=p['started_ns']
 for c,phase in zip(direct,phases):
  expected_argv=[x.replace('{resolved_wrapper_ld_lld}',str(Path(got['wrapper_bin'])/'ld.lld')) for x in phase['argv']]
  h.need(c['tool']==phase['tool'] and c['argv']==expected_argv,'race direct phase argv/order')
  h.need(c['cwd']==(got['fixture'] if phase['cwd']=='fixture_root' else native_workspace),'race direct phase cwd')
  h.need(previous<=c['started_ns']<=c['completed_ns']<=c['wrapper_return_ready_ns'],'race phase causal order')
  previous=c['wrapper_return_ready_ns']
  if 'required_marker' in phase:h.need(marker_matches(c['stdout'],phase['required_marker']),'race actual tool version')
 final=direct[-1]
 h.need(len(nested)>=1 and all(c['tool']=='ld.lld' and c['status']==0 and c['wrapper_ppid']==final['real_tool_pid'] for c in nested),'race nested real linker evidence')
 h.need(all(final['started_ns']<=c['started_ns']<=c['wrapper_return_ready_ns']<=final['completed_ns'] for c in nested),'race linker inside real clang')
 h.need(any('--version' not in c['argv'] and 'program.o' in c['argv'] and 'runtime.o' in c['argv'] and any(a=='-o' and b=='executable' for a,b in zip(c['argv'],c['argv'][1:])) for c in nested),'race nested linker did not perform actual object link')
 injected=[c for c in calls if 'race' in c];h.need(len(injected)==1 and injected[0] is final,'race injection phase')
 b=final['race'];h.need(b['created_exclusively'] and b['created_after_real_link_success'] and b['created_before_compiler_return'],'race exclusive barrier')
 h.need(final['completed_ns']<=b['linked_verified_ns']<=b['absent_verified_ns']<=b['created_ns']<=b['closed_ns']<=b['readback_verified_ns']<=final['wrapper_return_ready_ns']<=p['completed_ns'],'race publication timing')
 h.need(b['output']==str(Path(got['fixture'])/'out.bin') and b['bytes']==output['bytes'] and b['sha256']==output['sha256'],'race competing output')
 h.need(b['file_identity']==got['output_after']['identity'],'competitor identity changed')
 h.need(b['linked_elf_sha256']==got['linked_artifact']['sha256'] and got['linked_artifact_is_elf'],'race real linked artifact')
 h.need(got['linked_artifact_executed'] is False,'race must not execute linked artifact')

def main():
 parser=argparse.ArgumentParser();parser.add_argument('--binding',required=True);parser.add_argument('--run',default='output-race-run-v1');a=parser.parse_args()
 cfg=h.read_config(a.binding);sup,source_bindings=verify_authority(cfg);h.need(h.host()=='Linux x86_64','race host')
 run=h.OUT/a.run;run.mkdir(exist_ok=False);env,tool_bindings=h.tools_setup(run)
 wrapper=(h.OUT/'race_tool_wrapper.py').read_bytes();wrapper_bin=run/'tool-wrappers'
 for name in ('clang','opt','ld.lld'):
  p=wrapper_bin/name;p.write_bytes(wrapper);p.chmod(0o755)
 for row in tool_bindings:row['wrapper']=h.binding(wrapper_bin/row['name'])
 h.save(run/'tool-bindings.json',tool_bindings);toolmap={x['name']:x['executable'] for x in tool_bindings}
 observations=[];failures=[]
 for control in sup['controls']:
  evidence=run/control['profile'];evidence.mkdir();fixture=evidence/'fixture'
  row={'source_files':sup['source_case']['source_files'],'case':sup['source_case']};h.materialize(row,fixture)
  binary=cfg['binaries'][control['profile']]
  got={'case_id':control['case_id'],'profile':control['profile'],'host':h.host(),'supplement_sha256':SUP_SHA,'candidate_binding_sha256':cfg['_binding_sha256'],
   'fixture':str(fixture),'wrapper_bin':str(wrapper_bin),'source_files':row['source_files'],'directory_before':h.inventory(fixture),
   'executable_before':h.binding(binary['path']),'initial_out_bin_absent':not os.path.lexists(fixture/'out.bin'),
   'initial_native_workspace_absent':not list(fixture.glob('.oxid-native-*'))}
  log=evidence/'tool-log.jsonl';per_env={**env,'UNIT4_TOOL_LOG':str(log),'UNIT4_TOOL_EVIDENCE':str(evidence),'UNIT4_RACE_OUTPUT':str(fixture/'out.bin')}
  got['process']=execute([binary['path']]+control['argv'][1:],fixture,per_env)
  h.save(evidence/'process.json',got['process'])
  for stream in ('stdout','stderr'):(evidence/stream).write_text(got['process'][stream])
  got['directory_after']=h.inventory(fixture);got['executable_after']=h.binding(binary['path'])
  got['tool_invocations']=[json.loads(x) for x in log.read_text().splitlines()] if log.exists() else []
  got['native_workspaces_after']=[str(x.relative_to(fixture)) for x in fixture.glob('.oxid-native-*')]
  output=fixture/'out.bin';got['output_after']={'bytes':output.stat().st_size,'sha256':h.fsha(output),'identity':identity(output),'regular_not_symlink':output.is_file() and not output.is_symlink()}
  linked=evidence/'linked-before-publication.elf';got['linked_artifact']=h.binding(linked);got['linked_artifact_is_elf']=linked.read_bytes()[:4]==b'\x7fELF';got['linked_artifact_executed']=False
  got['emitted_llvm']=h.binding(evidence/'program.ll')
  try:check(control,got,sup,cfg,toolmap);got['comparison']='PASS'
  except (h.Reject,KeyError,TypeError,ValueError) as e:got['comparison']='FAIL';failures.append({'case_id':control['case_id'],'reason':str(e)})
  for stream in ('stdout','stderr'):(evidence/stream).write_text(got['process'][stream])
  h.save(evidence/'receipt.json',got);observations.append(got)
 controls=[]
 if not failures:
  for name,fn in [('wrong-phase-count',lambda a:a['tool_invocations'].pop(0)),('no-race-evidence',lambda a:next(c for c in a['tool_invocations'] if 'race' in c).pop('race')),
   ('wrong-winner-bytes',lambda a:a['output_after'].__setitem__('sha256','wrong')),('changed-winner-inode',lambda a:a['output_after']['identity'].__setitem__('st_ino',0)),
   ('wrong-diagnostic-status',lambda a:a['process'].__setitem__('status',0)),('remaining-native-workspace',lambda a:a['native_workspaces_after'].append('unexpected'))]:
   x=copy.deepcopy(observations[0]);fn(x)
   try:check(sup['controls'][0],x,sup,cfg,toolmap)
   except (h.Reject,KeyError,TypeError,ValueError) as e:controls.append({'name':name,'rejected':True,'reason':str(e)})
   else:raise h.Reject('race negative control accepted '+name)
 h.need({x['case_id'] for x in observations}=={x['case_id'] for x in sup['controls']} and len(observations)==2,'race complete roster')
 verify_authority(cfg);h.read_config(a.binding)
 report={'status':'FAIL' if failures else 'PASS','actual_public_compiles':2,'native_success_claims':0,'elf_executions':0,'failures':failures,
 'negative_controls':controls,'supplement':h.binding(SUP),'freeze':h.binding(FREEZE),'source_authorities_rebound':source_bindings,
 'observer':h.binding(__file__),'wrapper':h.binding(h.OUT/'race_tool_wrapper.py'),'public_harness':h.binding(h.__file__)}
 h.save(run/'comparison.json',report);print(json.dumps(report,indent=2));return bool(failures)
if __name__=='__main__':raise SystemExit(main())
