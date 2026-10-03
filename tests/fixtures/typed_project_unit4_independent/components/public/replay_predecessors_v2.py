#!/usr/bin/env python3
"""Actual public CLI replay using frozen predecessor bytes and projections.
No model execution, production build, private adapter, or expectation mutation.
"""
import argparse,base64,collections,copy,gzip,json,os,shutil,sys
from pathlib import Path
import public_cli as h
U2=h.ROOT/'oxid-typed-project-activation/tests/fixtures/typed_project_unit2_independent/semantic'
U3=h.ROOT/'typed-project-unit3-oracles'
PROJECTION_SHA='c76542221e507494fa7027b500beb053605103a9405f886264dc2e02193a5588'
PROJECTION=h.OUT/'predecessor-projection-v1.json'

def dataset():
 h.need(h.fsha(PROJECTION)==PROJECTION_SHA,'projection freeze changed')
 table=h.load(PROJECTION)
 for b in table['authority_bindings']:h.verify_file(b)
 corpus={};linehash={};requests={}
 for family,path in [('Unit2',U2/'corpus.jsonl.gz'),('Unit3',U3/'expected.jsonl.gz')]:
  for line in gzip.open(path,'rb'):
   d=json.loads(line);key=(family,d['id']);h.need(key not in corpus,'duplicate corpus row');corpus[key]=d;linehash[key]=h.sha(line)
 for line in (U3/'requests.jsonl').read_text().splitlines():
  req=json.loads(line);requests[req['id']]=req
 for row in table['rows']:
  key=(row['family'],row['id']);auth=(row['family'],row['authority_id'])
  h.need(linehash[key]==row['corpus_line_sha256'] and linehash[auth]==row['authority_line_sha256'],'frozen corpus row identity')
  expected=corpus[auth]['expected'];h.need(h.sha(json.dumps(expected,sort_keys=True,separators=(',',':')).encode())==row['frozen_expected_sha256'],'frozen projection expectation')
  if row['family']=='Unit2':
   d=corpus[key];req=json.loads(base64.b64decode(d['request_base64']));sources={s['path']:base64.b64decode(s['base64']) for s in d['sources']}
  else:
   req=requests[row['id']];sources={s['path']:(U3/req['source_root']/s['path']).read_bytes() for s in req['source_files']}
  h.need(req['entry']==row['entry'],'projection entry')
  for source in row['source_files']:
   b=sources[source['path']];h.need(len(b)==source['bytes'] and h.sha(b)==source['sha256'],'predecessor source hash')
  row['_sources']=sources;row['_expected']=expected;row['_corpus']=corpus[key]
 return table

def binary_stat(path):
 s=Path(path).stat();return {k:getattr(s,k) for k in ('st_dev','st_ino','st_size','st_mtime_ns','st_ctime_ns','st_mode')}

def public_envelope(process,op):
 h.need(process['status'] in (0,1) and not process['timed_out'],'unexpected process completion')
 h.need(process['stderr']=='','unexpected stderr')
 for stream in ('stdout','stderr'):h.need(h.sha(process[stream].encode())==process[stream+'_sha256'],'stream identity')
 objs=[json.loads(line) for line in process['stdout'].splitlines()];h.need(bool(objs),'missing public output')
 for obj in objs:h.need(obj['schema_version']==1 and obj['edition']=='typed-preview','schema/edition')
 summary=objs[-1];diags=objs[:-1]
 h.need(summary['kind']==op+'-summary' and summary['errors']==len(diags),'public summary')
 h.need(summary['success']==(process['status']==0),'status/summary inconsistency')
 h.need((len(diags)==0)==summary['success'],'diagnostic/summary inconsistency')
 field='functions' if op=='check' else 'result'
 if not summary['success']:h.need(summary[field] is None,'failure payload not null')
 for d in diags:
  h.need(d['kind']=='diagnostic' and d['severity']=='error' and isinstance(d['message'],str),'diagnostic shape')
  h.need(set(d)=={'schema_version','edition','kind','severity','code','stage','message','primary','secondary','notes'},'diagnostic unknown/missing fields')
 h.need(set(summary)=={'schema_version','edition','kind','success','errors',field},'summary unknown/missing fields')
 return summary,diags

def loc_span(p):
 if p is None:return None
 return {'file':p['path'],'start':p['start'],'end':p['end']}

def unit2_compare(row,process,prose):
 e=row['_expected'];summary,ds=public_envelope(process,'check');kind=row['kind']
 if kind=='schema-and-source-only':return
 if kind=='first-diagnostic-only':
  h.need(process['status']==1 and bool(ds),'first failure missing')
  h.need({k:ds[0][k] for k in e['first_diagnostic']}==e['first_diagnostic'],'first diagnostic projection')
  return
 if kind=='front-end-prefix-only':
  blocked={'source','lex','parse'} if 'no-source-lex-or-parse-diagnostic' in row['claims'] else {'lex','parse'}
  h.need(not any(d['stage'] in blocked for d in ds),'unexpected early-stage diagnostic');return
 h.need(kind=='complete-check-projection','unknown Unit2 projection')
 expected=copy.deepcopy(e['diagnostics'])
 if row['_corpus']['cohort']=='original93':
  for i,d in enumerate(expected):d.update(prose.get((row['_corpus']['case'],i),{}))
 h.need(len(ds)==len(expected),'Unit2 diagnostic count')
 h.need(process['status']==(1 if expected else 0),'Unit2 status')
 for i,(actual,want) in enumerate(zip(ds,expected)):
  projected={'code':actual['code'],'stage':actual['stage'],'primary':loc_span(actual['primary']),
   'secondary':[loc_span(s['span']) for s in actual['secondary']], 'message':actual['message'],
   'labels':[s['message'] for s in actual['secondary']],'notes':actual['notes']}
  compare={k:v for k,v in want.items() if k not in ('phase','order')}
  h.need({k:projected[k] for k in compare}==compare,f'Unit2 diagnostic {i}')
 if not expected:
  h.need(summary['functions']==len(e['function_declarations']),'Unit2 accepted function count')

def unit3_compare(row,process,op):
 e=row['_expected'];summary,ds=public_envelope(process,op)
 rejection=e if e.get('status')=='reject' or e.get('check')=='reject' else e.get(op)
 if isinstance(rejection,dict):
  h.need(process['status']==1,'Unit3 expected failure')
  expected=rejection.get('reachable_diagnostics',[rejection])
  if row['kind']=='first-source-failure-check-run-projection':
   h.need(bool(ds),'Unit3 missing first failure');h.diagnostics(ds[:1],expected[:1])
  else:h.diagnostics(ds,expected)
 else:
  h.need(process['status']==0 and not ds,'Unit3 expected acceptance')
  if op=='check':h.need(summary['functions']==row['_corpus']['function_count'],'Unit3 function count')
  else:h.need(summary['result']=={'type':e['result_type'],'value':e['result']},'Unit3 result')

def key(row,profile,op):return f"{row['family']}/{row['id']}/{profile}/{op}"
def expected_keys(table):return [key(r,p,op) for r in table['rows'] for p in r['profiles'] for op in r['operations']]

def check_receipt(row,got,cfg,prose,observer_hash):
 profile=got['profile'];op=got['operation'];h.need(profile in row['profiles'] and op in row['operations'],'wrong profile/operation')
 h.need(got['key']==key(row,profile,op),'wrong receipt key')
 h.need(got['host']==table_host and got['projection_sha256']==PROJECTION_SHA,'host/projection')
 h.need(got['observer_sha256']==observer_hash and got['candidate_binding_sha256']==cfg['_binding_sha256'],'observer/candidate binding')
 h.need(got['corpus_line_sha256']==row['corpus_line_sha256'] and got['authority_line_sha256']==row['authority_line_sha256'],'corpus line authority')
 h.need(got['projection_kind']==row['kind'] and got['source_files']==row['source_files'],'projection/source binding')
 h.need(got['binary']==cfg['binaries'][profile],'wrong binary')
 h.need(got['binary_stat_before']==got['binary_stat_after']==cfg['_binary_stats'][profile],'binary changed at invocation')
 h.need(got['output_initially_absent'] is True and got['directory_before']==got['directory_after'],'source/directory mutation')
 h.need(sorted(h.sources_from_inventory(got['directory_before']),key=lambda s:s['path'])==sorted(row['source_files'],key=lambda s:s['path']),'initial source map')
 p=got['process'];h.need(p['argv']==[cfg['binaries'][profile]['path'],op,row['entry'],'--edition','typed-preview','--message-format=json'],'wrong public argv')
 h.need(p['cwd']==got['fixture_directory'],'wrong public cwd')
 if row['family']=='Unit2':unit2_compare(row,p,prose)
 else:unit3_compare(row,p,op)

def roster_check(table,keys):
 h.need(bool(keys),'zero replay')
 h.need(len(keys)==len(set(keys)),'duplicate replay key')
 h.need(set(keys)==set(expected_keys(table)),'missing/extra replay key')

def controls(table,good,cfg,prose,observer_hash):
 rows={(r['family'],r['id']):r for r in table['rows']};res=[]
 keys=[g['key'] for g in good];roster_check(table,keys)
 for name,altered in [('zero',[]),('missing',keys[1:]),('duplicate',keys+[keys[0]]),('extra',keys+['EXTRA'])]:
  try:roster_check(table,altered)
  except h.Reject as e:res.append({'name':name,'rejected':True,'reason':str(e)})
  else:raise h.Reject('accepted roster mutation '+name)
 g=next(x for x in good if x['projection_kind']=='complete-check-projection')
 row=rows[(g['family'],g['id'])]
 for name,path,value in [('profile',('profile',),'fake'),('host',('host',),'fake'),('binary',('binary','sha256'),'wrong'),
   ('source',('source_files',0,'sha256'),'wrong'),('argv',('process','argv'),['fake']),('status',('process','status'),42),
   ('stdout',('process','stdout'),'{}\n'),('stderr',('process','stderr'),'unexpected'),('output',('output_initially_absent',),False)]:
  a=copy.deepcopy(g);node=a
  for k in path[:-1]:node=node[k]
  node[path[-1]]=value
  try:check_receipt(row,a,cfg,prose,observer_hash)
  except (h.Reject,KeyError,TypeError,ValueError) as e:res.append({'name':name,'rejected':True,'reason':str(e)})
  else:raise h.Reject('accepted receipt mutation '+name)
 return {'all_rejected':True,'controls':res}

table_host='Linux x86_64'
def main():
 p=argparse.ArgumentParser();p.add_argument('--binding',required=True);p.add_argument('--run',default='predecessor-run-v1');a=p.parse_args()
 h.need(h.host()==table_host,'unsupported replay host');table=dataset();cfg=h.read_config(a.binding)
 cfg['_binary_stats']={profile:binary_stat(b['path']) for profile,b in cfg['binaries'].items()}
 run=h.OUT/a.run;run.mkdir(exist_ok=False);work=run/'work';work.mkdir();observer_hash=h.fsha(__file__)
 h.save(run/'binary-preflight.json',{'binaries':cfg['binaries'],'stats':cfg['_binary_stats'],'candidate_binding_sha256':cfg['_binding_sha256'],'projection_sha256':PROJECTION_SHA})
 prose={(r['case'],r['diagnostic_index']):r['expected'] for r in h.load(U2/'original93-prose-expectations.json')['diagnostics']}
 env={**os.environ,'OXID_LLVM_BIN':str(h.LLVM),'LD_LIBRARY_PATH':str(h.LIBS)}
 for name in ('UNIT4_TOOL_LOG','UNIT4_TOOL_EVIDENCE','UNIT4_REAL_LLVM'):env.pop(name,None)
 observations=[];failures=[];ordinal=0
 with gzip.open(run/'observations.jsonl.gz','wt') as sink:
  for row in table['rows']:
   for profile in row['profiles']:
    for op in row['operations']:
     fixture=work/f'{ordinal:05d}';fixture.mkdir();ordinal+=1
     for name,b in row['_sources'].items():
      path=fixture/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(b)
     before=h.inventory(fixture);binary=cfg['binaries'][profile]
     got={'key':key(row,profile,op),'family':row['family'],'id':row['id'],'profile':profile,'operation':op,'host':h.host(),
      'projection_sha256':PROJECTION_SHA,'observer_sha256':observer_hash,'candidate_binding_sha256':cfg['_binding_sha256'],
      'corpus_line_sha256':row['corpus_line_sha256'],'authority_line_sha256':row['authority_line_sha256'],
      'projection_kind':row['kind'],'source_files':row['source_files'],'binary':binary,
      'binary_stat_before':binary_stat(binary['path']),'fixture_directory':str(fixture),'directory_before':before,
      'output_initially_absent':not os.path.lexists(fixture/'out.bin')}
     got['process']=h.proc([binary['path'],op,row['entry'],'--edition','typed-preview','--message-format=json'],fixture,env)
     got['binary_stat_after']=binary_stat(binary['path']);got['directory_after']=h.inventory(fixture)
     try:check_receipt(row,got,cfg,prose,observer_hash);got['comparison']='PASS'
     except (h.Reject,KeyError,TypeError,ValueError) as e:
      got['comparison']='FAIL';failure={'key':got['key'],'reason':str(e)};failures.append(failure)
      if len(failures)<=20:print('FAIL',failure,flush=True)
     sink.write(json.dumps(got,sort_keys=True)+'\n');observations.append(got)
     if got['comparison']=='PASS':shutil.rmtree(fixture)
     if ordinal%1000==0:print(json.dumps({'completed':ordinal,'failures':len(failures)}),flush=True)
 roster_check(table,[x['key'] for x in observations]);h.read_config(a.binding);dataset()
 post={p:h.binding(b['path']) for p,b in cfg['binaries'].items()};h.need(post==cfg['binaries'],'binary bytes changed')
 h.save(run/'binary-postflight.json',post)
 report={'status':'FAIL' if failures else 'PASS_WITH_EXPLICIT_PROJECTION_LIMITS','observations':len(observations),
 'unit2_observations':sum(x['family']=='Unit2' for x in observations),'unit3_observations':sum(x['family']=='Unit3' for x in observations),
 'kind_counts':dict(collections.Counter(x['projection_kind'] for x in observations)),
 'failures':failures,'projection':h.binding(PROJECTION),'observer':h.binding(__file__),'public_harness':h.binding(h.__file__),
 'comparison_boundary':'3557 Unit2 complete semantic projections,31 first failures,12 frontend prefixes,3 schema/source-only rows, each in both profiles; Unit3 full source-bound check/run projections, with12 first-source-failure rows explicitly partial',
 'native_runs':0,'private_raw_event_fuel_claims':False}
 h.save(run/'comparison.json',report)
 if not failures:h.save(run/'negative-controls.json',controls(table,observations,cfg,prose,observer_hash))
 print(json.dumps({k:v for k,v in report.items() if k not in ('failures',)},indent=2));return bool(failures)
if __name__=='__main__':
 try:sys.exit(main())
 except h.Reject as e:print('REJECT:',e,file=sys.stderr);sys.exit(1)
