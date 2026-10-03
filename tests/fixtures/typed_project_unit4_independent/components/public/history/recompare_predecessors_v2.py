#!/usr/bin/env python3
"""Versioned comparison of unchanged, complete v1 actual public receipts.
No compiler invocations. Sole v2 transport fix sorts both source-map projections.
"""
import collections,copy,gzip,json
import public_cli as h
import replay_predecessors_v2 as r
run=h.OUT/'predecessor-run-v1';table=r.dataset();cfg=h.read_config(h.OUT/'candidate-binding-v1.json')
before=h.load(run/'binary-preflight.json');cfg['_binary_stats']=before['stats']
h.need(before['binaries']==cfg['binaries'],'original binary binding')
for profile,b in cfg['binaries'].items():h.need(r.binary_stat(b['path'])==before['stats'][profile],'original binary metadata changed')
original=h.load(run/'comparison.json');observer_hash=original['observer']['sha256']
h.verify_file(original['observer']);h.verify_file(original['public_harness'])
obs=[json.loads(l) for l in gzip.open(run/'observations.jsonl.gz','rt')]
r.roster_check(table,[x['key'] for x in obs]);rows={(x['family'],x['id']):x for x in table['rows']}
prose={(x['case'],x['diagnostic_index']):x['expected'] for x in h.load(r.U2/'original93-prose-expectations.json')['diagnostics']}
failures=[]
for got in obs:
 try:r.check_receipt(rows[(got['family'],got['id'])],got,cfg,prose,observer_hash)
 except (h.Reject,KeyError,TypeError,ValueError) as e:failures.append({'key':got['key'],'reason':str(e)})
heldout=next(g for g in obs if g['key']=='Unit2/original93/A-dense-module-major/debug/check')
row=rows[(heldout['family'],heldout['id'])]
r.check_receipt(row,heldout,cfg,prose,observer_hash)
controls=[]
for name,field,value in [('nested-map-altered-bytes','sha256','wrong'),('nested-map-altered-path','path','a/other.ox')]:
 altered=copy.deepcopy(heldout)
 for side in ('directory_before','directory_after'):
  next(x for x in altered[side] if x['path']=='a/c.ox')[field]=value
 try:r.check_receipt(row,altered,cfg,prose,observer_hash)
 except (h.Reject,KeyError,TypeError,ValueError) as e:controls.append({'name':name,'rejected':True,'reason':str(e)})
 else:raise h.Reject('nested-map negative accepted '+name)
report={'status':'FAIL' if failures else 'PASS_WITH_EXPLICIT_PROJECTION_LIMITS','comparison_only':True,'compiler_invocations':0,
 'actual_public_observations_recompared':len(obs),'unit2_observations':sum(x['family']=='Unit2' for x in obs),
 'unit3_observations':sum(x['family']=='Unit3' for x in obs),'failures':failures,
 'kind_counts':dict(collections.Counter(x['projection_kind'] for x in obs)),
 'original_observations':h.binding(run/'observations.jsonl.gz'),'original_failed_comparison':h.binding(run/'comparison.json'),
 'original_observer':original['observer'],'corrected_comparator':h.binding(r.__file__),'recomparison':h.binding(__file__),
 'unchanged_projection':h.binding(r.PROJECTION),'nested_source_map_control':{'positive':'a.ox and a/c.ox exact identities compare regardless of list order','mutations':controls},
 'correction_scope':'One comparison expression sorts actual source-file map by path before comparing to the already sorted expected map; no source, expected, candidate, or raw receipt changes',
 'unobserved_boundaries':original['comparison_boundary']}
if not failures:report['negative_controls']=r.controls(table,obs,cfg,prose,observer_hash)
h.save(h.OUT/'predecessor-recomparison-v2.json',report)
print(json.dumps({k:v for k,v in report.items() if k not in ('negative_controls',)},indent=2));raise SystemExit(bool(failures))
