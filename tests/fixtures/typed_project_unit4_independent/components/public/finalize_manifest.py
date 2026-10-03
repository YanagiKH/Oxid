#!/usr/bin/env python3
"""Final local evidence audit and content manifest; no compiler invocation."""
import json,datetime
import public_cli as h
root=h.OUT
required={
 'run-v1/comparison.json':'PASS',
 'direct-tool-passivity-v1/comparison.json':'PASS',
 'guard-run-v1/comparison.json':'PASS',
 'output-race-run-v1/comparison.json':'PASS',
 'predecessor-recomparison-v2.json':'PASS_WITH_EXPLICIT_PROJECTION_LIMITS'}
reports={}
for path,want in required.items():
 d=h.load(root/path);h.need(d['status']==want,'required verdict '+path);reports[path]=d
cfg=h.read_config(root/'candidate-binding-v1.json');contract,_=h.authorities()
cfg['_tool_bindings']={t['name']:t['executable'] for t in h.load(root/'run-v1/tool-bindings.json')}
for tool in cfg['_tool_bindings'].values():h.verify_file(tool)
actual=[json.loads(line) for line in (root/'run-v1/observations.jsonl').read_text().splitlines()]
h.validate(h.roster(contract),actual,cfg,h.fsha(h.__file__))
main=reports['run-v1/comparison.json'];h.need((main['executed'],main['host_inapplicable'],main['rows'])==(140,6,146),'primary roster')
pred=reports['predecessor-recomparison-v2.json'];h.need((pred['actual_public_observations_recompared'],pred['unit2_observations'],pred['unit3_observations'])==(7814,7206,608),'predecessor roster')
h.need(pred['failures']==[] and pred['compiler_invocations']==0,'predecessor comparison boundary')
checks=[root/'run-v1/negative-controls.json',root/'synthetic-preflight-controls-v1.json']
for p in checks:h.need(h.load(p)['all_rejected'] is True,'negative controls '+str(p))
h.need(pred['negative_controls']['all_rejected'] is True,'predecessor negative controls')
for path in ['guard-run-v1/comparison.json','output-race-run-v1/comparison.json']:
 h.need(all(x['rejected'] for x in reports[path]['negative_controls']),'guard control failure')
artifacts=[]
for p in sorted(root.rglob('*')):
 if '__pycache__' in p.parts or p.name=='final-manifest-v1.json' or not p.is_file():continue
 if p.is_symlink():artifacts.append({'path':p.relative_to(root).as_posix(),'kind':'symlink','target':str(p.readlink())})
 else:artifacts.append({'path':p.relative_to(root).as_posix(),'bytes':p.stat().st_size,'sha256':h.fsha(p)})
review_root=h.ROOT/'typed-project-unit4-dispatch-review'
reviews=[h.binding(review_root/name) for name in [
 'unused-nominal-native-review-v1.json','predecessor-interface-review-v1.json','guard-interface-review-v1.json','concurrent-output-supplement-review-v1.json','race-interface-review-v1.json']]
summary={'schema':1,'created_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),
 'verdict':'PASS_FOR_REVIEWED_LINUX_PUBLIC_OBSERVATION_SCOPES','publication_claim':False,'host':h.host(),
 'actual_public_cli_invocations':7978,'actual_public_cli_breakdown':{'primary_literal_and_inherited':140,'predecessor_replay':7814,'existing_output_guards':20,'concurrent_output_guards':2,'direct_tool_passivity':2},
 'operation_counts':{'check':7556,'run':362,'compile':60},'source_free_elf_runs':18,'primary_source_free_elf_runs':16,'passivity_source_free_elf_runs':2,
 'native_guard_success_claims':0,'explicit_host_inapplicable_primary_rows':6,
 'predecessor_scope':{'complete_public_projections':7674,'limited_first_failure_or_prefix_projections':134,'schema_source_only_observations':6},
 'comparator_controls':{'synthetic_pre_execution':18,'actual_primary_mutations':24,'predecessor_roster_receipt_mutations':13,'nested_source_map_mutations':2,'existing_output_guard_mutations':7,'concurrent_output_guard_mutations':6},
 'correction_history':'Original predecessor v1 raw receipts and FAIL report remain; v2 corrects only path-list ordering and re-compares the exact receipts with unchanged expected values. No duplicated compiler count.',
 'residual_boundaries':['No Windows/macOS execution claimed','Private lifecycle/flavor/route/index/check/source-reopen facts belong to separate reviewer evidence','Raw IR/event/fuel/private-depth semantic claims retain predecessor companions','Final publication-source rebinding and hosted CI remain separate gates'],
 'candidate_binding':h.binding(root/'candidate-binding-v1.json'),'report':h.binding(root/'qualification-report.md'),
 'reports':[h.binding(root/path) for path in required],'independent_pre_execution_reviews':reviews,'artifacts':artifacts,
 'compiler_invocations_by_final_audit':0,'remote_writes':0,'production_edits':0}
h.save(root/'final-manifest-v1.json',summary)
print(json.dumps({'verdict':summary['verdict'],'actual_public_cli_invocations':summary['actual_public_cli_invocations'],'source_free_elf_runs':summary['source_free_elf_runs'],
 'artifact_entries':len(artifacts),'manifest':h.binding(root/'final-manifest-v1.json')},indent=2))
