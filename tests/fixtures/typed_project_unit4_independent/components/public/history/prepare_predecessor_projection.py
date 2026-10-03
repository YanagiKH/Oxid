#!/usr/bin/env python3
"""Freeze only transport projections of already-frozen predecessor authorities.
No candidate executable or result file is opened by this preparation script.
"""
import base64,collections,gzip,json
import public_cli as h
U2=h.ROOT/'oxid-typed-project-activation/tests/fixtures/typed_project_unit2_independent/semantic'
U3=h.ROOT/'typed-project-unit3-oracles'
u2=[]
for line in gzip.open(U2/'corpus.jsonl.gz','rb'):
 d=json.loads(line);d['_line_sha256']=h.sha(line);u2.append(d)
byid={d['id']:d for d in u2};rows=[]
redirect={'grammar-amendment/module-qualified-projection':'grammar-amendment/qualified-projection',
          'grammar-amendment/original-qualified-borrow':'parser/qualified-borrow-place'}
for d in u2:
 reqraw=base64.b64decode(d['request_base64']);h.need(h.sha(reqraw)==d['request_sha256'],'request identity')
 req=json.loads(reqraw)
 for s in d['sources']:
  b=base64.b64decode(s['base64']);h.need(len(b)==s['bytes'] and h.sha(b)==s['sha256'],'corpus source identity')
 expected=d['expected'];authority=d
 if d['id'] in redirect:
  authority=byid[redirect[d['id']]]
  h.need(d['sources']==authority['sources'],'parser redirect is not exact source identity')
  expected=authority['expected']
 if d['mode']=='legacy-scalar':kind='schema-and-source-only';claims=[]
 elif 'diagnostics' in expected:
  kind='complete-check-projection';claims=['status','diagnostic-count-and-order','diagnostic-code-stage-primary-secondary','literal-prose-when-frozen','check-summary']
 elif 'first_diagnostic' in expected:kind='first-diagnostic-only';claims=['failure-status','first-diagnostic-code-stage']
 elif expected.get('parser_accepts'):
  kind='front-end-prefix-only';claims=['no-root-lex-or-parse-diagnostic']
  if expected.get('load_accepts'):claims=['no-source-lex-or-parse-diagnostic']
 else:raise h.Reject('unmapped Unit2 '+d['id'])
 excluded=['private-route','private-flavor','loader-read-counters','phase-events','DefId-RecordId','import-transaction-events','private-index-probes','private-resource-counters']
 if kind=='schema-and-source-only':excluded.append('private-scalar-diagnostics-do-not-authorize-ordinary-owned-outcome')
 if kind in ('first-diagnostic-only','front-end-prefix-only'):excluded.append('complete-semantic-success-and-later-diagnostic-sequence')
 rows.append({'family':'Unit2','id':d['id'],'corpus_line_sha256':d['_line_sha256'],'authority_id':authority['id'],
 'authority_line_sha256':authority['_line_sha256'],'kind':kind,'claims':claims,'excluded_assertions':excluded,
 'entry':req['entry'],'operations':['check'],'profiles':['debug','release'],
 'source_files':[{k:s[k] for k in ('path','bytes','sha256')} for s in d['sources']],
 'argv_templates':[['{oxid}','check',req['entry'],'--edition','typed-preview','--message-format=json']],
 'frozen_expected_sha256':h.sha(json.dumps(expected,sort_keys=True,separators=(',',':')).encode()),
 'redirect_basis':('exact source-byte identity with frozen ProjectCandidate row; original private-mode assertion remains historical' if d['id'] in redirect else None)})
requests={x['id']:x for x in (json.loads(l) for l in (U3/'requests.jsonl').read_text().splitlines())}
u3=[]
for line in gzip.open(U3/'expected.jsonl.gz','rb'):
 d=json.loads(line);req=requests[d['id']];e=d['expected']
 kind='complete-check-run-projection'
 if e.get('selection')=='one specified first source failure':kind='first-source-failure-check-run-projection'
 u3.append(d)
 rows.append({'family':'Unit3','id':d['id'],'corpus_line_sha256':h.sha(line),'authority_id':d['id'],
 'authority_line_sha256':h.sha(line),'kind':kind,
 'claims':['status','check-function-count-when-accepted','run-type-and-value-or-diagnostic','frozen-diagnostic-code-stage-primary-secondary'],
 'excluded_assertions':['raw-IR','event-order','exact-fuel','DefId-RecordId','route-selection','private-source-association-visits']+(['later-diagnostic-sequence'] if kind.startswith('first-') else []),
 'entry':req['entry'],'operations':['check','run'],'profiles':['debug','release'],'source_files':req['source_files'],
 'argv_templates':[['{oxid}',op,req['entry'],'--edition','typed-preview','--message-format=json'] for op in ('check','run')],
 'frozen_expected_sha256':h.sha(json.dumps(e,sort_keys=True,separators=(',',':')).encode())})
h.need(len(u2)==3603 and len(u3)==152,'predecessor finite roster')
path=h.OUT/'predecessor-projection-v1.json'
h.save(path,{'schema':1,'evidence_kind':'PRE_EXECUTION_FROZEN_TRANSPORT_PROJECTION_ONLY','candidate_invocations':0,'candidate_result_files_read':0,
 'host':'Linux x86_64','authority_bindings':[h.binding(U2/f) for f in ['package-manifest.json','corpus-manifest.json','corpus.jsonl.gz','original93-prose-expectations.json','legacy-scalar-schedule-clarification.json']]+[h.binding(U3/f) for f in ['pre-execution-manifest.json','requests.jsonl','expected.jsonl.gz']],
 'unit2_source_maps':3603,'unit2_public_check_invocations':7206,'unit3_source_maps':152,'unit3_public_check_run_invocations':608,
 'kind_counts':dict(collections.Counter(x['kind'] for x in rows)),
 'execution_contract':'New exact source map for each case/profile/operation; supplied entry spelling; public executable only; no output files; raw stdout/stderr/status retained. Sources are removed only after post-invocation identity verification to avoid retaining duplicate corpora.',
 'binary_binding_contract':'Hash each immutable debug/release executable before and after the whole replay; compare file device/inode/size/mtime_ns/ctime_ns before and after every invocation. This is explicitly distinct from rehashing executable bytes per invocation.',
 'comparison_contract':'Complete-check rows compare all frozen diagnostics exactly after public field projection, including original93 frozen prose. First-only rows compare only the frozen first failure and label later sequence unqualified. Prefix-only rows compare absence of early-stage failures and leave later semantic outcome unqualified. Three private legacy-scalar rows have no authorized ordinary-route semantic expectation; retain actual observation with schema/source integrity only. No private facts are claimed as public.',
 'rows':rows,'projection_author':h.binding(__file__)})
print(json.dumps({'path':str(path),'sha256':h.fsha(path),'kind_counts':dict(collections.Counter(x['kind'] for x in rows))},indent=2))
