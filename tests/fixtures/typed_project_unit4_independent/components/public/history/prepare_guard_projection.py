#!/usr/bin/env python3
"""Repackage frozen no-clobber streams through their existing placeholders only."""
import json
import public_cli as h
SUP=h.ROOT/'typed-project-unit3-oracles/supplements/native-diagnostics-v1'
REQ=h.ROOT/'typed-project-unit3-native-driver-review'
manifest=SUP/'supplement-manifest.json';h.need(h.fsha(manifest)=='9cae46e804a2776eb44732ced7201f99e37312346ef81958d025ee255f644c0a','native supplement identity')
for row in h.load(manifest)['files']:h.verify_file(row,SUP)
request_path=REQ/'no-clobber-requests.json';h.need(h.fsha(request_path)=='bf2955e64077d12513438ed2cad386287f7ab1f6c5ba6ab13c4e80ad98cc4f68','no-clobber roster identity')
req=h.load(request_path);h.need(h.load(REQ/'request-input-manifest.json')['no_clobber_controls']==req,'no-clobber roster parent identity')
e=h.load(SUP/'expectations.json');requests={r['id']:r for r in (json.loads(l) for l in (h.ROOT/'typed-project-unit3-oracles/requests.jsonl').read_text().splitlines())}
rows=[]
for i,r in enumerate(req['requests']):
 if r['id'].startswith('negative-native_recursion'):
  envelope=next(x for c in e['driver'] if c['id']==r['id'] for x in c['expectations'] if x['operation']=='compile' and x['format']==r['format'])
  authority=f"driver/{r['id']}/compile/{r['format']}"
 else:
  envelope=next(x for c in e['no_clobber'] if c['id']==r['id'] and c['output_kind']==r['output_kind'] for x in c['expectations'] if x['format']==r['format'])
  authority=f"no_clobber/{r['id']}/{r['output_kind']}/{r['format']}"
 for profile in r['profiles']:
  rows.append({'key':f"no-clobber/{i}/{profile}",'request_index':i,'id':r['id'],'profile':profile,'output_kind':r['output_kind'],'format':r['format'],
   'source_files':requests[r['id']]['source_files'],'source_root':str(h.ROOT/'typed-project-unit3-oracles'/requests[r['id']]['source_root']),
   'entry_template':'${FIXTURE_ROOT}/'+r['id']+'/'+requests[r['id']]['entry'],
   'argv_template':['{oxid}','compile','${FIXTURE_ROOT}/'+r['id']+'/'+requests[r['id']]['entry'],'--edition','typed-preview','--backend','llvm','--output','${OUTPUT}']+(['--message-format=json'] if r['format']=='json' else []),
   'frozen_envelope':envelope,'envelope_authority':authority,'expected_tools':0,
   'expected_preservation':'sources, existing output kind/bytes/link destination and target bytes unchanged; no native workspace remains'})
h.need(len(rows)==20,'guard roster')
p=h.OUT/'guard-projection-v1.json'
h.save(p,{'schema':1,'evidence_kind':'FROZEN_PREDECESSOR_TRANSPORT_PROJECTION','candidate_invocations':0,
 'host':'Linux x86_64','authority_bindings':[h.binding(x) for x in [manifest,SUP/'expectations.json',request_path,REQ/'request-input-manifest.json',h.ROOT/'typed-project-unit3-oracles/requests.jsonl']],
 'path_substitution':e['path_substitution'],'new_semantic_expectations':False,'rows':rows,'projection_author':h.binding(__file__)})
print(json.dumps({'path':str(p),'sha256':h.fsha(p),'rows':len(rows)}))
