"""Additive Unit2 evidence validation. No process execution and no CI promotion.
The producer supplies authenticated preparation and its in-memory invocation
context; expected identities must never be reconstructed from result.json.
"""
import base64,gzip,hashlib,io,json,pathlib,re,stat,time
from pathlib import Path,PurePosixPath

def require(value,message):
 if not value:raise ValueError(message)
def sha(body):return hashlib.sha256(body).hexdigest()
def strict_json(body):
 def pairs(items):
  value={}
  for k,v in items:require(k not in value,'duplicate JSON key');value[k]=v
  return value
 def bad(value):raise ValueError('nonfinite JSON')
 return json.loads(body,object_pairs_hook=pairs,parse_constant=bad)
def read(path,maximum=256*1024**2):
 path=pathlib.Path(path);require(stat.S_ISREG(path.lstat().st_mode),'nonregular input');require(path.stat().st_size<=maximum,'input byte limit');return path.read_bytes()
def rows(path):
 lines=read(path).splitlines();require(bool(lines) and all(lines),'blank/empty JSONL');return [strict_json(line) for line in lines]
def observation_rows(path,contract,window,raw=None):
 values=rows(path);require([x['case'] for x in values]==contract['case_ids'],'exact ordered observation IDs')
 require([x['mode'] for x in values]==contract['modes'],'exact observation modes')
 for index,value in enumerate(values):
  require('normalization_error' not in value,'normalization failure')
  begin=value['started_ns'];end=value['finished_ns'];require(type(begin) is int and type(end) is int and window[0]<=begin<=end<=window[1],'stale/out-of-window observation')
  if raw is not None:require(all(value[k]==raw[index][k] for k in ('case','mode','started_ns','finished_ns')),'normalizer altered actual identity/time')
 return values

def listing(body,names):
 lines=body.decode().splitlines();nonempty=[x for x in lines if x]
 want=[name+': test' for name in names];suffix=f'{len(names)} '+('test' if len(names)==1 else 'tests')+', 0 benchmarks'
 require(nonempty[-1:]==[suffix] and len(nonempty)==len(want)+1 and sorted(nonempty[:-1])==sorted(want),'exact harness listing')

def success(body,names,resource=False):
 lines=[x for x in body.decode().splitlines() if x]
 start=f'running {len(names)} '+('test' if len(names)==1 else 'tests')
 require(lines and lines[0]==start,'harness start')
 tests=lines[1:1+len(names)]
 parsed=[re.fullmatch(r'test (.+) \.\.\. ok',line) for line in tests]
 require(all(parsed) and sorted(x.group(1) for x in parsed)==sorted(names),'exact successful test rows')
 tail=lines[1+len(names):];require(bool(tail),'missing harness terminal')
 match=re.fullmatch(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out; finished in [0-9.]+s',tail[-1]);require(match is not None and int(match.group(1))==len(names),'success count')
 middle=tail[:-1]
 if not resource:require(not middle,'unexpected observer output')
 else:
  require(middle and middle[0]=='successes:' and middle.count('successes:')==2,'resource success sections')
  split=middle.index('successes:',1);captured=middle[1:split];listed=middle[split+1:]
  require(sorted(listed)==sorted('    '+name for name in names),'resource success name list')
  groups={};current=None
  for line in captured:
   heading=re.fullmatch(r'---- (.+) stdout ----',line)
   if heading:
    current=heading.group(1);require(current in names and current not in groups,'resource stdout heading');groups[current]=[]
   else:require(current is not None,'payload without heading');groups[current].append(line)
  prefix='frontend::declaration_index::independent_unit2_resource_review::'
  layout=prefix+'reviewer_actual_layout_and_source_node_envelope';space=prefix+'reviewer_nine_independent_space_cases';merge=prefix+'reviewer_byte_comparators_and_stable_merge_bounds'
  require(set(groups)=={layout,space,merge},'resource captured output roster')
  require(groups[merge]==['reviewed merge sequences=19531'],'merge output')
  tuples=[(0,0,0,1,0,68,4),(1,0,0,1,0,120,8),(0,1,0,1,0,136,8),(0,1,3,1,0,184,8),(0,0,0,2,0,180,12),(0,1,0,2,1,288,28),(1,0,0,2,1,272,28),(1,1,0,2,1,340,32),(1,1,0,2,2,380,44)]
  require(groups[space]==[f'space F={f} R={r} K={k} M={m} U={u} payload={p} scratch={s}' for f,r,k,m,u,p,s in tuples],'space output')
  types="OriginalRow|FunctionRow|RecordRow|FieldRow|ModuleRow|ImportRow|AliasCell|SeenCell|DeclarationIndex<'_>|DeclarationFacts<'_>|IndexPlan|Counts|Scratch|SourceOwner<'_>|PreparedTypeName<'_>|QuerySession<'_,'_>|SourceFile|SourceMap|Span|lexer::Token|ast::Program|ast::Function|ast::Param|ast::BodyBlock|ast::Stmt|ast::StructDecl|ast::StructField|ast::Expr|ast::Argument|ast::FieldInit|ast::ItemId|ast::ModuleDecl|ast::AbsolutePath|ast::ImportDecl|ProjectSources".split('|')
  payload=groups[layout];require(len(payload)==len(types)+1,'layout output count')
  actual=[]
  for line in payload[:-1]:
   item=re.fullmatch(r'layout (.+) bytes=(\d+) alignment=(\d+)',line);require(item is not None,'layout row')
   actual.append(''.join(item.group(1).split()));size=int(item.group(2));alignment=int(item.group(3));require(0<size<=65536 and 0<alignment<=64 and alignment&(alignment-1)==0,'layout numeric envelope')
  require(actual==types,'layout type order');fixed=re.fullmatch(r'fixed scratch=(\d+)',payload[-1]);require(fixed is not None and int(fixed.group(1))<=4096,'scratch output')
 return {'passed':len(names),'filtered':int(match.group(2))}

def comparison(root,package,ids,python,profile):
 require(profile in ('debug','release'),'unknown profile')
 root=pathlib.Path(root);package=pathlib.Path(package);current=strict_json(read(root/(profile+'-comparison.json')));historical_path=root/(profile+'-comparison-before-enum-enabled-qualified-values.json');historical=strict_json(read(historical_path));descriptor_body=read(package/'semantic/enum-enabled-qualified-values-v1.json');descriptor=strict_json(descriptor_body);case_ids=[x['id'] for x in descriptor['rows']]
 for value in (current,historical):require(value['profile']==profile and value['cases']==3603 and [x['case'] for x in value['results']]==ids,'comparison profile/ordered IDs')
 for value in (current,historical):
  for row in value['results']:
   require(set(row)=={'case','status','failures','unavailable'} and isinstance(row['failures'],list) and row['unavailable']==[],'comparison row schema')
   require((row['status']=='match' and row['failures']==[]) or (row['status']=='mismatch' and bool(row['failures'])),'comparison row status consistency')
 require(current['counts']=={'match':3603} and all(x['status']=='match' and not x.get('failures') and not x.get('unavailable') for x in current['results']),'incomplete current comparison')
 require(historical['counts']=={'match':3599,'mismatch':4},'historical counts')
 require({x['case'] for x in historical['results'] if x['status']=='mismatch'}==set(case_ids) and all(x['status'] in ('match','mismatch') for x in historical['results']),'historical mismatch roster')
 normalized=root/(profile+'-normalized.jsonl');require(current['normalized_observations']==str(normalized) and historical['normalized_observations']==str(normalized),'comparison observed input')
 expected_binding={'identity':'enum-enabled-qualified-values-v1','argv':[python,'-B',str(package/'semantic/compare_before_enum_enabled_qualified_values.py'),profile,str(normalized),str(historical_path)],'status':0,'normalized_sha256':sha(read(normalized)),'historical_mismatches':case_ids,'historical_qualification_claim':False}
 for key,path in [('comparison',historical_path),('stdout',historical_path.with_suffix('.stdout')),('stderr',historical_path.with_suffix('.stderr'))]:
  body=read(path,256*1024**2 if key=='comparison' else 16*1024**2)
  if key=='stderr':require(body==b'','historical child stderr')
  expected_binding[key]={'path':str(path),'bytes':len(body),'sha256':sha(body)}
 require(current['historical_semantic_comparison']==expected_binding,'historical subprocess binding')
 amendment={key:descriptor[key] for key in ('identity','source_manifest','unit2_corpus','base_public_freeze_sha256','public_predecessors')}
 amendment.update(descriptor_sha256=sha(descriptor_body),cases=case_ids,frozen_authorities_unchanged=True,observations_unchanged=True,historical_qualification_claim=False,rows=[{'case':row['id'],'frozen_expected_canonical_sha256':row['frozen_expected_canonical_sha256'],'old_expected':row['old_unit2_expected'],'current_expected':row['current_unit2_expected'],'current_result':row['current_unit2_result']} for row in descriptor['rows']])
 require(current['semantic_amendment']==amendment,'semantic amendment identity')
 return {'current_matches':3603,'historical_matches':3599,'historical_mismatch_ids':case_ids,'historical_qualification_claim':False}

def safe_name(name):
 require(isinstance(name,str) and bool(name),'empty path');path=PurePosixPath(name)
 require(not path.is_absolute() and str(path)==name and all(p not in ('.','..') for p in path.parts),'unsafe path')
 require(not any(c in name for c in '\x00\t\r\n\\'),'unsafe path character');return name

def derive_corpus(package,output):
 package=Path(package);output=Path(output)
 require(output.is_absolute() and output==Path(str(output)) and '..' not in output.parts,'absolute output required')
 descriptor=strict_json((package/'semantic/corpus-manifest.json').read_bytes());compressed=(package/'semantic/corpus.jsonl.gz').read_bytes()
 require(len(compressed)==descriptor['compressed_bytes'] and sha(compressed)==descriptor['compressed_corpus_sha256'],'compressed corpus identity')
 require(descriptor['uncompressed_bytes']==13747865 and descriptor['cases']==3603,'frozen corpus envelope')
 with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as stream:payload=stream.read(descriptor['uncompressed_bytes']+1)
 require(len(payload)==descriptor['uncompressed_bytes'],'decompression bound')
 cases=[strict_json(line) for line in payload.splitlines()];require(len(cases)==3603,'case count')
 return derive_cases(cases,descriptor,output)

def derive_cases(cases,descriptor,output):
 bodies={};queue=[];tsv=[];ids=[];source_count=source_bytes=request_bytes=0
 def add(name,body):
  safe_name(name);require(name not in bodies,'duplicate output path');bodies[name]=body
 for case in cases:
  cohort=safe_name(case['cohort']);name=safe_name(case['case']);require('/' not in cohort and '/' not in name,'cohort/case must be components')
  identity=cohort+'/'+name;require(case['id']==identity and identity not in ids,'case identity/duplicate');ids.append(identity)
  mode=case['mode'];require(mode in ('project','project-parser','parser-ProjectCandidate','parser-OwnedCandidate','parser-ModuleCandidate','legacy-scalar','legacy-owned'),'unknown mode')
  request_body=base64.b64decode(case['request_base64'],validate=True);require(sha(request_body)==case['request_sha256'],'request digest');request=strict_json(request_body)
  schemas=[{'case','entry','instruction','mode','probes','schema','source_files','trace'},{'case','entry','mode','schema','source_files'},{'case','entry','parser_mode','schema','source_files'},{'case','direct_api','entry','mode','schema','source_files'},{'also_test_node_exhaustion_at_final_segment','case','entry','mode','schema','source_files'}]
  require(set(request) in schemas,'request schema')
  entry=safe_name(request['entry']);source_rows={};source_data={}
  for source in case['sources']:
   path=safe_name(source['path']);require(path not in source_rows,'duplicate source')
   body=base64.b64decode(source['base64'],validate=True);require(len(body)==source['bytes'] and sha(body)==source['sha256'],'source digest')
   source_rows[path]={'path':path,'bytes':len(body),'sha256':sha(body)};source_data[path]=body
  order=case['creation_order'];require(len(order)==len(set(order)) and set(order)==set(source_data),'creation order')
  advertised=request['source_files'];require(len(advertised)==len(source_rows) and {x['path']:x for x in advertised}==source_rows,'request source map')
  require(entry in source_data,'missing entry');add(identity+'/request.json',request_body);request_bytes+=len(request_body)
  for path in order:add(identity+'/files/'+path,source_data[path]);source_count+=1;source_bytes+=len(source_data[path])
  queue.append({'case':name,'cohort':cohort,'mode':mode,'request':identity+'/request.json','entry':identity+'/files/'+entry,'source_files':advertised,'request_sha256':case['request_sha256'],'creation_order':order,'replaces':case['replaces']})
  tsv.append(identity+'\t'+mode+'\t'+str(output/identity/'files'/entry))
 require(len(ids)==len(set(ids))==descriptor['cases'],'exact case count')
 require(sha(('\n'.join(ids)+'\n').encode())==descriptor['case_ids_sha256'],'case order digest')
 require(source_count==descriptor['source_files'] and source_bytes==descriptor['source_bytes'],'source totals')
 add('queue.json',(json.dumps({'schema':1,'cases':queue},indent=2)+'\n').encode());add('queue.tsv',('\n'.join(tsv)+'\n').encode())
 directories=set()
 for name in bodies:
  for parent in PurePosixPath(name).parents:
   if str(parent)!='.':directories.add(str(parent))
 rows=[{'path':name,'bytes':len(body),'sha256':sha(body)} for name,body in sorted(bodies.items())]
 return {'schema':'oxid-unit2-materialization-contract-v1','output':str(output),'files':rows,'directories':sorted(directories),'file_count':len(rows),'total_bytes':sum(row['bytes'] for row in rows),'request_bytes':request_bytes,'source_bytes':source_bytes,'source_files':source_count,'case_ids':ids,'case_ids_sha256':descriptor['case_ids_sha256'],'queue_sha256':sha(bodies['queue.json']),'queue_tsv_sha256':sha(bodies['queue.tsv']),'modes':[case['mode'] for case in cases],'expectation_files':0}


def compiled_map_digest(rows):
 return sha(json.dumps({'schema':'unit2-assembled-body-map-v1','files':rows},sort_keys=True,separators=(',',':'),allow_nan=False).encode())

def validate_compile_window(invocation,compilation,observer):
 values=[invocation['started_ns'],compilation['spawn_ns'],compilation['finished_ns'],observer['spawn_ns'],invocation['finished_ns']]
 require(all(type(value) is int for value in values) and values==sorted(values),'compilation/observer invocation window')

def verify_body_pins(pins,routes,deadline):
 require(isinstance(pins,dict) and bool(pins) and len(pins)<=17000,'trusted body pin envelope')
 total=0
 for name,digest in pins.items():
  require(time.monotonic()<deadline,'preservation deadline');path=Path(name)
  require(path.is_absolute() and path.resolve(strict=True)==path and stat.S_ISREG(path.lstat().st_mode),'canonical pinned body')
  total+=path.stat().st_size;require(total<=2*1024**3,'pinned body byte envelope');h=hashlib.sha256()
  with path.open('rb') as stream:
   while True:
    require(time.monotonic()<deadline,'preservation deadline');body=stream.read(1024*1024)
    if not body:break
    h.update(body)
  require(h.hexdigest()==digest,'live preservation body mismatch')
 for route,body in routes.items():require(body in pins and str(Path(route).resolve(strict=True))==body,'live tool route mismatch')

def validate_pair_metadata(prepared,invocation,sessions):
 """Validate authority-input relationships; this alone proves no execution."""
 require(set(sessions)=={'debug','release'},'both unique profiles required')
 require(invocation['checkout_before']==invocation['checkout_after']==prepared['checkout'],'checkout changed')
 require(isinstance(invocation['nonce'],str) and re.fullmatch('[0-9a-f]{32}',invocation['nonce']),'session nonce')
 require(type(invocation['started_ns']) is int and type(invocation['finished_ns']) is int and invocation['started_ns']<=invocation['finished_ns'],'invocation time envelope')
 for profile,value in sessions.items():
  require(value==invocation['profiles'][profile]['session'],'session differs from producer context')
  require(value['profile']==profile and value['source_inputs_sha256']==prepared['source_inputs_sha256'] and value['package_inputs_sha256']==prepared['package_inputs_sha256'],'profile/source/package mismatch')
  require(value['accounting_transport']==prepared['accounting_transport'] and value['current_source']==prepared['current_source'] and value['historical_source_sha256']==prepared['historical_source_sha256'],'accounting/source lineage mismatch')
  require(type(value['started_ns']) is int and invocation['started_ns']<=value['started_ns']<=invocation['finished_ns'],'profile session time')
  require(value['execution_qualified'] is False,'premature qualification claim')
  require(value['current_source']['sha256']==value['source_inputs_sha256'],'source manifest association')
  require(re.fullmatch('[0-9a-f]{32}',value['invocation_id']) is not None,'profile nonce')
 a,b=sessions['debug'],sessions['release']
 for key in ('invocation_id','binary_sha256','queue_tsv_sha256'):require(a[key]!=b[key],'profile replay '+key)
 require(a['queue_sha256']==b['queue_sha256'],'cross-profile corpus queue mismatch')
 expected_map=compiled_map_digest(prepared['assembled380'])
 require(a['compiled_source_map_sha256']==b['compiled_source_map_sha256']==expected_map,'prepared assembled map binding')
 return True


def validate_session(prepared_context,invocation_context,profile_runs):
 """Validate live evidence under a reviewed producer's captured contexts.

 Caller dictionaries are authority inputs only when built by the reviewed
 producer, before/through actual execution. Manually supplying dictionaries
 cannot create provenance. This is session validation, not an attestation.
 No subprocess, historical runner, remote service or execution admission occurs.
 """
 prepared=prepared_context;invocation=invocation_context
 require(set(profile_runs)==set(invocation['profiles'])=={'debug','release'},'exact profiles')
 begin=time.monotonic();deadline=begin+90
 def tick():require(time.monotonic()<deadline,'session validation deadline')
 def root(path):
  path=Path(path);require(path.is_absolute() and path.resolve(strict=True)==path and path.is_dir(),'noncanonical evidence root');return path
 def file_hash(path):
  tick();path=Path(path);require(stat.S_ISREG(path.lstat().st_mode),'nonregular body');require(path.stat().st_size<=256*1024**2,'body size envelope');h=hashlib.sha256()
  with path.open('rb') as stream:
   while True:
    tick();chunk=stream.read(1024*1024)
    if not chunk:break
    h.update(chunk)
  return h.hexdigest()
 def verify_tree(path,expected,extra_dirs=()):
  path=root(path);require(len(expected)<=17000,'tree file count envelope');wanted={row['path']:row for row in expected};require(len(wanted)==len(expected),'duplicate expected paths');dirs=set(extra_dirs)
  for name in wanted:
   safe_name(name)
   for parent in PurePosixPath(name).parents:
    if str(parent)!='.':dirs.add(str(parent))
  seen=set();seen_dirs=set();actual=[]
  for entry in path.rglob('*'):
   tick();mode=entry.lstat().st_mode;name=entry.relative_to(path).as_posix();require(stat.S_ISDIR(mode) or stat.S_ISREG(mode),'special tree member')
   if stat.S_ISDIR(mode):seen_dirs.add(name);continue
   require(name in wanted,'unexpected file');value={'path':name,'bytes':entry.stat().st_size,'sha256':file_hash(entry)};require(value==wanted[name],'body drift');seen.add(name);actual.append(value)
  require(seen==set(wanted) and seen_dirs==dirs,'closed tree membership');return actual
 package=root(prepared['package_root']);require(len(prepared['package_files'])==40 and len(prepared['source376'])==376 and len(prepared['assembled380'])==380,'prepared cardinalities')
 verify_tree(package,prepared['package_files']);verify_tree(prepared['source_root'],prepared['source376'])
 require(file_hash(package/'source-inputs.json')==prepared['source_inputs_sha256'] and file_hash(package/'package-inputs.json')==prepared['package_inputs_sha256'],'prepared manifest bodies')
 accounting=prepared['accounting_transport'];require(accounting['current_source']==prepared['current_source'] and prepared['current_source']['sha256']==prepared['source_inputs_sha256'],'new accounting manifest')
 require(accounting['predecessor_source_v2_accounting_binding']['current_source']['sha256']==prepared['historical_source_sha256'],'retained source-v2 association')
 require(accounting['predecessor_source_v2_accounting_binding']['predecessor_accounting_source_binding']==accounting['older_byte_storage_accounting_binding'],'direct/older accounting relationship')
 expected_names=strict_json(read(package/'resource-test-names.json'));require(len(expected_names)==len(set(expected_names))==21,'resource names')
 sessions={};summaries={};observer='frontend::oir::unit2_observer::observe_source_queue';top=root(invocation['root'])
 for profile in ('debug','release'):
  tick();run=root(profile_runs[profile]);context=invocation['profiles'][profile]
  require(run==Path(context['root']) and run.is_relative_to(top) and run!=top,'wrong profile root')
  session_value=strict_json(read(run/'session.json',16*1024**2));sessions[profile]=session_value
  report=strict_json(read(run/'report.json',16*1024**2));require(report['session']==session_value==context['session'],'submitted session differs from captured context')
  require(report['result']==profile+'-unit2-semantic-resource-passed' and report['reason'] is None and report['semantic_cases']==3603 and report['resource_tests']==21 and report['preservation_verified'] is True,'incomplete profile result')
  require(report['current_execution_status']=='NotReady' and report['semantic_pass'] is False and report['execution_qualified'] is False,'premature profile admission')
  require(report['operations']==context['process_records'] and report['pins']==context['pins'],'process/pin records differ from producer capture')
  verify_body_pins(context['pins'],context['tool_routes'],deadline)
  command=strict_json(read(run/'command.json'));require(command==context['command'],'command/environment drift')
  require(len(command['argv'])==len(report['operations'])==report['top_level_processes']==7,'seven top-level processes')
  require(all(op['index']==index and op['exit']==0 and op['failure'] is None and op['cleanup_error'] is None and op['remaining_owned_group']==[] for index,op in enumerate(report['operations'])),'incomplete owned process cleanup')
  for index,op in enumerate(report['operations']):
   require(type(op['pid']) is int and op['pid']>0 and invocation['started_ns']<=op['spawn_ns']<=op['finish_ns']<=invocation['finished_ns'],'process identity/time')
   owner=strict_json(read(run/f'operation-{index}/ownership.json'));require(owner=={'pid':op['pid'],'pgid':op['pid'],'argv':command['argv'][index]},'ownership record')
   for label in ('stdout.log','stderr.log','ownership.json'):
    path=run/f'operation-{index}'/label;require(op['evidence'][label]=={'bytes':path.stat().st_size,'sha256':file_hash(path)},'process evidence identity')
   require(read(run/f'operation-{index}/stderr.log')==b'','child stderr')
  compilation=context['compilation'];validate_compile_window(invocation,compilation,report['operations'][3]);source=root(compilation['source_root']);target=root(compilation['target_root']);verify_tree(source,prepared['assembled380'])
  elf=Path(compilation['elf']);require(elf.is_absolute() and elf.resolve(strict=True)==elf and elf.is_relative_to(target/('x86_64-unknown-linux-gnu/'+profile+'/deps')),'wrong ELF path')
  require(file_hash(elf)==compilation['elf_sha256']==session_value['binary_sha256'],'ELF identity')
  import struct
  with elf.open('rb') as stream:header=stream.read(64)
  require(len(header)==64 and header[:7]==b'\x7fELF\x02\x01\x01' and struct.unpack_from('<HHI',header,16)[0] in (2,3) and struct.unpack_from('<HHI',header,16)[1:]==(62,1) and struct.unpack_from('<H',header,52)[0]==64,'ELF64 architecture/header')
  cargo_lines=read(compilation['cargo_stdout'],32*1024**2).splitlines();require(bool(cargo_lines) and all(cargo_lines),'empty Cargo stream');cargo_events=[strict_json(line) for line in cargo_lines]
  require(all(isinstance(event,dict) for event in cargo_events) and cargo_events[-1].get('reason')=='build-finished' and [event.get('success') for event in cargo_events if event.get('reason')=='build-finished']==[True],'Cargo terminal')
  emitted=[event for event in cargo_events if event.get('reason')=='compiler-artifact' and event.get('executable')];require(emitted==[compilation['cargo_event']] and compilation['returncode']==0 and compilation['remaining_owned_group']==[] and compilation['cleanup_error'] is None,'captured compilation differs from live Cargo stream')
  event=compilation['cargo_event'];want_profile={'opt_level':'0' if profile=='debug' else '3','debuginfo':2 if profile=='debug' else 0,'debug_assertions':profile=='debug','overflow_checks':profile=='debug','test':True}
  require(event['package_id']=='path+'+source.as_uri()+'#oxid@0.9.0' and event['manifest_path']==str(source/'Cargo.toml') and event['target']['src_path']==str(source/'src/cli.rs') and event['target']['name']=='oxid' and event['target']['kind']==['bin'] and event['profile']==want_profile and event['fresh'] is False and event['executable']==str(elf),'path-qualified compilation event')
  require(compilation['spawn_ns']<=elf.stat().st_mtime_ns<=compilation['finished_ns'] and compilation['spawn_ns']<=elf.stat().st_ctime_ns<=compilation['finished_ns'],'cached/out-of-window ELF')
  corpus=derive_corpus(package,run/'observer-inputs');verify_tree(run/'observer-inputs',corpus['files'])
  require(session_value['queue_sha256']==corpus['queue_sha256'] and session_value['queue_tsv_sha256']==corpus['queue_tsv_sha256'],'queue binding')
  listing(read(run/'operation-1/stdout.log'),[observer]);listing(read(run/'operation-2/stdout.log'),expected_names)
  one=success(read(run/'operation-3/stdout.log'),[observer]);resources=success(read(run/'operation-4/stdout.log'),expected_names,True);require(one['filtered']+1==resources['filtered']+21,'harness inventory changed')
  window=(report['operations'][3]['spawn_ns'],report['operations'][3]['finish_ns']);raw=observation_rows(run/(profile+'-raw.jsonl'),corpus,window);observation_rows(run/(profile+'-normalized.jsonl'),corpus,window,raw)
  result=comparison(run,package,corpus['case_ids'],command['argv'][6][0],profile)
  expected_files={'command.json','session.json',profile+'-raw.jsonl',profile+'-normalized.jsonl',profile+'-comparison.json',profile+'-comparison-before-enum-enabled-qualified-values.json',profile+'-comparison-before-enum-enabled-qualified-values.stdout',profile+'-comparison-before-enum-enabled-qualified-values.stderr'}|{'observer-inputs/'+row['path'] for row in corpus['files']}|{f'operation-{i}/{label}' for i in range(7) for label in ('stdout.log','stderr.log','ownership.json')}
  require(set(report['evidence'])==expected_files,'profile evidence roster')
  expected=[{'path':name,**row} for name,row in report['evidence'].items()]+[{'path':'report.json','bytes':(run/'report.json').stat().st_size,'sha256':file_hash(run/'report.json')}]
  verify_tree(run,expected,('home','tmp','resource-fixtures'))
  verify_body_pins(context['pins'],context['tool_routes'],deadline)
  summaries[profile]={'invocation_id':session_value['invocation_id'],'profile':profile,'binary_sha256':session_value['binary_sha256'],'report_sha256':file_hash(run/'report.json'),'queue_sha256':corpus['queue_sha256'],'queue_tsv_sha256':corpus['queue_tsv_sha256'],**result}
 validate_pair_metadata(prepared,invocation,sessions);verify_tree(package,prepared['package_files']);verify_tree(prepared['source_root'],prepared['source376']);tick()
 return {'schema':'package-dependency-unit2-both-profile-session-v1','status':'session-validated','invocation_id':invocation['nonce'],'checkout':prepared['checkout'],'current_source':prepared['current_source'],'accounting_transport':prepared['accounting_transport'],'profiles':summaries,'current_execution_status':'NotReady','semantic_pass':False,'execution_qualified':False,'ci_runtime_proof':False,'scope':'validation under reviewed producer authority inputs; not a tamper-proof attestation or portable CI receipt'}
