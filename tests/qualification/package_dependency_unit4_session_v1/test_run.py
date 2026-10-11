"""Synthetic orchestration controls. No preparation, probe or target is executed."""
import hashlib,json,sys,tempfile,types,unittest
from pathlib import Path
from unittest.mock import patch
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent

def load_api():
 path=HERE/'run.py';module=types.ModuleType('unit4_session_test');module.__file__=str(path)
 exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__);return module

class Value:
 def __init__(self,document):self.value=document
 def document(self):return self.value

class Controls(unittest.TestCase):
 def setUp(self):
  self.q=load_api();self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name)
  self.original_handlers={sig:object() for sig in (self.q.signal.SIGINT,self.q.signal.SIGTERM)};self.handlers=dict(self.original_handlers)
  def install(sig,handler):
   old=self.handlers[sig];self.handlers[sig]=handler;return old
  self.install_signal=self.enterContext(patch.object(self.q.signal,'signal',side_effect=install))
  self.enterContext(patch.object(self.q.signal,'getsignal',side_effect=self.handlers.__getitem__))
  for name in ('original','historical','contract','amendment','parser-contract'):(self.root/name).mkdir()
  (self.root/'path-map.json').write_text('{}')
  self.request={'schema':'oxid-unit4-local-session-request-v1','scope':'combined','expected_head':'a'*40,'event_sha':'a'*40,'run_root':str(self.root/'run'),'original_repo':str(self.root/'original'),'historical_repo':str(self.root/'historical'),'public':{'contract_root':str(self.root/'contract'),'path_map':str(self.root/'path-map.json'),'amendment_root':str(self.root/'amendment'),'llvm_root':None,'library_root':None,'runtime_receipt':None,'existing_user':'nobody'},'parser':{'contract_root':str(self.root/'parser-contract')}}

 def fake(self,hosted=False,fail=None):
  calls=[];inv=Value({'nonce':'fresh-synthetic','run_root':self.request['run_root']});context=Value({'status':'NotReady','source':'new','invocation':inv.document()});machine=Value({'snapshot':{'host':{'name':'Linux x86_64'}}});prep=types.SimpleNamespace(_RUNS={})
  def event(name,result=None):
   def call(*args,**kwargs):
    calls.append((name,args,kwargs))
    if name==fail:raise RuntimeError('synthetic '+name+' failure')
    return result
   return call
  ctx=types.SimpleNamespace(capture_source=event('capture',object()),begin_invocation=event('begin',inv),verify_context=event('verify-context'))
  def prepare(*args,**kwargs):
   calls.append(('prepare',args,kwargs));p=Path(self.request['run_root']);p.mkdir();prep._RUNS[inv]={'root':p};return context
  prep.produce=prepare;prep.check_deadline=event('deadline');prep.cancel_invocation=lambda inv:prep._RUNS[inv].update(failed=True)
  host=types.SimpleNamespace(prepare_transport=event('transport',object()),execute_transport=event('hosted',object()))
  collection=Value({'status':'INCOMPLETE_LOCAL_CAPABILITY' if hosted else 'COMPLETE_LOCAL_OBSERVATIONS','mandatory_hosted_join_required':hosted})
  joined=Value({'status':'COMPLETE_LOCAL_OBSERVATIONS','primary_receipts_unchanged':True})
  runtime=types.SimpleNamespace(admit_runtime=event('public-admit',object()),collect=event('public-collect',collection),verify_collection=event('public-verify'),join_hosted=event('public-join',joined),verify_join=event('join-verify'))
  build=types.SimpleNamespace(build=event('public-build',object()))
  binding=types.SimpleNamespace(build_api=lambda:build,runtime_api=lambda:runtime,hosted_api=lambda:host)
  mach=types.SimpleNamespace(capture_machine=event('machine',machine),verify_machine=event('verify-machine'),capture_runtime_tools=event('public-tools',object()),capture_parser_runtime=event('parser-tools',object()))
  parser=types.SimpleNamespace(authority=event('parser-authority',object()),prepare=event('parser-prepare',{'path':str(self.root/'run/parser/package-source-session.json')}),build=event('parser-build',object()),create_runtime=event('parser-admit',object()),complete_runtime=event('parser-complete'),verify_runtime=event('parser-verify',{'local_semantics_status':'pass','execution_qualified':False}))
  return {'preparation':prep,'context':ctx,'machine':mach,'binding':binding,'parser':parser},calls

 def execute_fake(self,hosted=False,fail=None,change=None):
  api,calls=self.fake(hosted,fail)
  if change is not None:change(api,calls)
  with patch.object(self.q,'apis',return_value=api),patch.object(self.q,'verify_committed',return_value={'head':'a'*40}),patch.object(self.q,'dependencies',return_value={'reviewed':'synthetic'}),patch.object(self.q,'own_package',return_value={'reviewed':'synthetic'}),patch.object(self.q,'identity',return_value={'sha256':'synthetic','bytes':0,'path':'synthetic'}):
   result=self.q.produce(self.request)
  return result,calls

 def test_entrypoint_literal_matches_actual_sealed_dependency_inventory(self):
  import hashlib
  raw=(Path(__file__).parent/'dependencies.json').read_bytes()
  self.assertEqual(self.q.DEPENDENCIES_SHA256,hashlib.sha256(raw).hexdigest())
  self.assertNotEqual(self.q.DEPENDENCIES_SHA256,'f166fc7badd4b83fb3fe6265b42a14dffa3cff48b34016bbc5d41696cd8a0222')

 def test_exact_scope_roles_and_no_effectful_plan(self):
  with patch.object(self.q,'apis',side_effect=AssertionError('plan must not load producer')):
   result=self.q.plan_request(self.request)
  self.assertEqual(result['roles'],['ordinary','lifecycle','parser_observed','parser_control'])
  self.assertEqual(result['shared_total_seconds'],36000)
  self.assertFalse(result['execution_qualified']);self.assertFalse(result['ci_runtime_proof'])
  self.install_signal.assert_not_called()

 def test_execution_restores_handlers_after_success(self):
  self.execute_fake()
  self.assertEqual(self.handlers,self.original_handlers)
  self.assertEqual(self.install_signal.call_count,4)

 def test_both_cancellation_signals_finalize_failed_receipt(self):
  for sig in (self.q.signal.SIGTERM,self.q.signal.SIGINT):
   with self.subTest(signal=sig):
    self.request['run_root']=str(self.root/('run-'+str(sig)))
    def change(api,calls):
     api['parser'].authority=lambda *_:self.handlers[sig](sig,None)
    with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
    report=json.loads((Path(self.request['run_root'])/'unit4-local-session.json').read_bytes())
    self.assertEqual(report['failure'],{'type':'Cancelled','message':'local Unit4 session cancelled by '+self.q.signal.Signals(sig).name})
    self.assertFalse(report['local_complete']);self.assertEqual(report['status'],'NotReady')
    self.assertFalse(report['execution_qualified']);self.assertFalse(report['ci_runtime_proof'])
    self.assertEqual(report['preservation']['source'],'verified');self.assertEqual(report['preservation']['machine'],'verified')
    self.assertEqual(report['stages'][-1]['name'],'admit-parser-source');self.assertFalse(report['stages'][-1]['completed'])
    self.assertEqual(self.handlers,self.original_handlers)

 def test_repeated_cancellation_does_not_interrupt_finally(self):
  observed=[]
  def change(api,calls):
   def cancel(*_):self.handlers[self.q.signal.SIGTERM](self.q.signal.SIGTERM,None)
   api['parser'].authority=cancel
   def verify(*_):
    for sig in (self.q.signal.SIGINT,self.q.signal.SIGTERM):self.handlers[sig](sig,None)
    observed.append('preserved')
   api['context'].verify_context=verify
  with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(observed,['preserved']);self.assertIn('SIGTERM',report['failure']['message'])
  self.assertEqual(report['preservation']['source'],'verified');self.assertEqual(report['preservation']['machine'],'verified')
  self.assertEqual(self.handlers,self.original_handlers)

 def test_caught_cancellation_cannot_continue_or_claim_completion(self):
  observed=[]
  def change(api,calls):
   def swallowed(*_):
    try:self.handlers[self.q.signal.SIGTERM](self.q.signal.SIGTERM,None)
    except BaseException:observed.append('caught')
    return object()
   api['parser'].authority=swallowed
  with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(observed,['caught']);self.assertEqual(report['failure']['type'],'Cancelled')
  self.assertFalse(report['local_complete']);self.assertEqual(report['stages'][-1]['name'],'admit-parser-source')
  self.assertFalse(report['stages'][-1]['completed']);self.assertEqual(self.handlers,self.original_handlers)

 def test_original_failure_survives_signal_during_preservation(self):
  def change(api,calls):
   api['context'].verify_context=lambda *_:self.handlers[self.q.signal.SIGTERM](self.q.signal.SIGTERM,None)
  with self.assertRaises(self.q.Rejected):self.execute_fake(fail='parser-complete',change=change)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(report['failure'],{'type':'RuntimeError','message':'synthetic parser-complete failure'})
  self.assertFalse(report['local_complete']);self.assertEqual(report['preservation']['machine'],'verified')
  self.assertEqual(self.handlers,self.original_handlers)

 def test_poison_error_preserves_original_cancellation_and_receipt(self):
  def change(api,calls):
   api['preparation'].cancel_invocation=lambda *_:(_ for _ in ()).throw(ValueError('unverified owner'))
   api['parser'].authority=lambda *_:self.handlers[self.q.signal.SIGTERM](self.q.signal.SIGTERM,None)
  with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(report['failure']['type'],'Cancelled')
  self.assertEqual(report['cancellation_poison_error'],'unverified owner')
  self.assertFalse(report['local_complete'])

 def test_cancellation_during_preservation_poison_stops_following_reads(self):
  def change(api,calls):
   original=api['context'].verify_context
   def verify(*args):
    handler=self.handlers[self.q.signal.SIGTERM]
    if handler.finalizing:
     handler(self.q.signal.SIGTERM,None)
     api['preparation'].check_deadline(args[0])
    return original(*args)
   api['context'].verify_context=verify
   def deadline(*_):
    if any(row.get('failed') for row in api['preparation']._RUNS.values()):raise self.q.Rejected('cancelled live invocation')
   api['preparation'].check_deadline=deadline
  with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(report['failure']['type'],'Cancelled')
  self.assertEqual(report['preservation']['machine'],{'error':'cancelled live invocation'})
  self.assertEqual(report['preservation']['source'],{'error':'cancelled live invocation'})

 def test_cancellation_keeps_exhausted_preservation_deadline_unverified(self):
  def change(api,calls):
   api['parser'].authority=lambda *_:self.handlers[self.q.signal.SIGTERM](self.q.signal.SIGTERM,None)
   def deadline(*_):
    if self.handlers[self.q.signal.SIGTERM].finalizing:raise self.q.Rejected('synthetic shared deadline exhausted')
   api['preparation'].check_deadline=deadline
  with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(report['failure']['type'],'Cancelled');self.assertFalse(report['local_complete'])
  self.assertEqual(set(report['preservation']),{'committed_inputs','consumer_code','source','machine'})
  for result in report['preservation'].values():self.assertEqual(result,{'error':'synthetic shared deadline exhausted'})
  self.assertEqual(self.handlers,self.original_handlers)

 def test_first_signal_during_preservation_fails_completion(self):
  def change(api,calls):
   def verify(*_):
    handler=self.handlers[self.q.signal.SIGTERM]
    if handler.finalizing:handler(self.q.signal.SIGTERM,None)
   api['context'].verify_context=verify
  with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(report['failure']['type'],'Cancelled');self.assertFalse(report['local_complete'])
  self.assertEqual(report['preservation']['source'],'verified');self.assertEqual(report['preservation']['machine'],'verified')
  self.assertEqual(self.handlers,self.original_handlers)

 def test_terminal_serialization_is_non_interrupting(self):
  canonical=self.q.canonical;serialized=[]
  def serialize(value):
   if value.get('schema')=='oxid-unit4-local-session-result-v1':
    for sig in (self.q.signal.SIGINT,self.q.signal.SIGTERM):self.handlers[sig](sig,None)
    serialized.append(True)
   return canonical(value)
  with patch.object(self.q,'canonical',side_effect=serialize):
   with self.assertRaises(self.q.Rejected):self.execute_fake(fail='parser-complete')
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertEqual(serialized,[True]);self.assertEqual(report['failure']['type'],'RuntimeError')
  self.assertFalse(report['local_complete']);self.assertEqual(self.handlers,self.original_handlers)

 def test_nested_supervisor_restores_producer_handlers(self):
  observed=[]
  def change(api,calls):
   def nested(*_):
    old={}
    try:
     for sig in self.original_handlers:old[sig]=self.q.signal.signal(sig,lambda *_:None)
    finally:
     for sig,handler in old.items():self.q.signal.signal(sig,handler)
    observed.append(all(self.handlers[sig] is handler for sig,handler in old.items()))
    self.handlers[self.q.signal.SIGTERM](self.q.signal.SIGTERM,None)
   api['parser'].authority=nested
  with self.assertRaises(self.q.Rejected):self.execute_fake(change=change)
  self.assertEqual(observed,[True]);self.assertEqual(self.handlers,self.original_handlers)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes());self.assertEqual(report['failure']['type'],'Cancelled')

 def test_handlers_restore_when_validation_fails_before_root(self):
  with patch.object(self.q,'own_package',side_effect=self.q.Rejected('synthetic package failure')):
   with self.assertRaises(self.q.Rejected):self.q.produce(self.request)
  self.assertFalse((self.root/'run').exists());self.assertEqual(self.handlers,self.original_handlers)

 def test_partial_handler_installation_is_restored_without_execution(self):
  install=self.install_signal.side_effect
  def fail_install(sig,handler):
   if sig==self.q.signal.SIGTERM and isinstance(handler,self.q._Cancellation):raise ValueError('synthetic handler install failure')
   return install(sig,handler)
  self.install_signal.side_effect=fail_install
  with patch.object(self.q,'_produce',side_effect=AssertionError('must not execute')) as execute:
   with self.assertRaisesRegex(ValueError,'synthetic handler install failure'):self.q.produce(self.request)
   execute.assert_not_called()
  self.assertEqual(self.handlers,self.original_handlers)

 def test_terminal_write_failure_restores_handlers_and_fails(self):
  real_open=Path.open
  def open_path(path,*args,**kwargs):
   if path.name=='unit4-local-session.json':raise OSError('synthetic terminal write failure')
   return real_open(path,*args,**kwargs)
  with patch.object(Path,'open',new=open_path),patch.object(self.q.sys,'stderr') as stderr:
   with self.assertRaises(self.q.Rejected):self.execute_fake()
  self.assertTrue(stderr.write.called);self.assertEqual(self.handlers,self.original_handlers)
  self.assertFalse((self.root/'run/unit4-local-session.json').exists())

 def test_request_closure_and_head_binding(self):
  for bad in (dict(self.request,extra=1),dict(self.request,event_sha='b'*40),dict(self.request,scope='optional')):
   with self.assertRaises(self.q.Rejected):self.q.plan_request(bad)

 def test_fresh_output_and_symlink_ancestry(self):
  (self.root/'run').mkdir()
  with self.assertRaises(self.q.Rejected):self.q.plan_request(self.request)
  (self.root/'alias').symlink_to(self.root/'run',target_is_directory=True)
  with self.assertRaises(self.q.Rejected):self.q.plan_request(dict(self.request,run_root=str(self.root/'alias/new')))

 def test_scope_cannot_silently_skip_mandatory_domain_inputs(self):
  for bad in (dict(self.request,public=None),dict(self.request,parser=None),dict(self.request,scope='parser')):
   with self.assertRaises(self.q.Rejected):self.q.plan_request(bad)

 def test_public_only_has_exact_two_roles(self):
  request=dict(self.request,scope='public',parser=None,original_repo=None,historical_repo=None)
  self.assertEqual(self.q.plan_request(request)['roles'],['ordinary','lifecycle'])

 def test_parser_only_has_exact_two_roles(self):
  request=dict(self.request,scope='parser',public=None)
  self.assertEqual(self.q.plan_request(request)['roles'],['parser_observed','parser_control'])

 def test_full_synthetic_producer_order_and_no_ci_promotion(self):
  result,calls=self.execute_fake();names=[x[0] for x in calls]
  self.assertLess(names.index('begin'),names.index('prepare'))
  self.assertLess(names.index('prepare'),names.index('machine'))
  self.assertEqual(names.count('public-build'),2);self.assertEqual(names.count('parser-build'),4)
  self.assertLess(max(i for i,n in enumerate(names) if n in ('public-build','parser-build')),names.index('public-tools'))
  parser_admit=next(x for x in calls if x[0]=='parser-admit')
  self.assertIsInstance(parser_admit[1][1],list);self.assertEqual(len(parser_admit[1][1]),4)
  self.assertTrue(result['local_complete']);self.assertFalse(result['execution_qualified']);self.assertFalse(result['ci_runtime_proof'])
  self.assertEqual(result['status'],'NotReady')

 def test_mandatory_hosted_branch_runs_and_joins_once(self):
  with patch.object(self.q.sys,'platform','linux'),patch.object(self.q.os,'geteuid',return_value=0):result,calls=self.execute_fake(hosted=True)
  names=[x[0] for x in calls]
  self.assertEqual(names.count('hosted'),1);self.assertEqual(names.count('public-join'),1)
  self.assertTrue(result['public']['primary_receipts_unchanged'])

 def test_failed_hosted_attempt_is_retained_without_retry(self):
  with patch.object(self.q.sys,'platform','linux'),patch.object(self.q.os,'geteuid',return_value=0):
   with self.assertRaises(self.q.Rejected):self.execute_fake(hosted=True,fail='hosted')
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertFalse(report['local_complete']);self.assertIn('synthetic hosted failure',report['failure']['message'])
  self.assertEqual(sum(r['name']=='execute-hosted-capability' for r in report['stages']),1)
  self.assertEqual(report['preservation']['source'],'verified')

 def test_failed_parser_attempt_never_claims_completion(self):
  with self.assertRaises(self.q.Rejected):self.execute_fake(fail='parser-complete')
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertFalse(report['local_complete']);self.assertFalse(report['execution_qualified'])
  self.assertIn('synthetic parser-complete failure',report['failure']['message'])

 def test_final_public_readmission_failure_after_parser_is_retained(self):
  api,calls=self.fake();count=[0]
  def verify(*args):
   count[0]+=1
   if count[0]==2:raise RuntimeError('synthetic final public drift')
  api['binding'].runtime_api().verify_collection=verify
  with patch.object(self.q,'apis',return_value=api),patch.object(self.q,'verify_committed',return_value={}),patch.object(self.q,'dependencies',return_value={}),patch.object(self.q,'own_package',return_value={}),patch.object(self.q,'identity',return_value={}):
   with self.assertRaises(self.q.Rejected):self.q.produce(self.request)
  report=json.loads((self.root/'run/unit4-local-session.json').read_bytes())
  self.assertIn('synthetic final public drift',report['failure']['message']);self.assertFalse(report['local_complete'])
  self.assertTrue(any(r['name']=='complete-parser-runtime' and r['completed'] for r in report['stages']))

 def test_untracked_input_refuses_before_preparation(self):
  api,calls=self.fake()
  with patch.object(self.q,'apis',return_value=api),patch.object(self.q,'verify_committed',side_effect=self.q.Rejected('untracked input')),patch.object(self.q,'dependencies',return_value={}),patch.object(self.q,'own_package',return_value={}),patch.object(self.q,'identity',return_value={}):
   with self.assertRaises(self.q.Rejected):self.q.produce(self.request)
  self.assertFalse(any(row[0]=='prepare' for row in calls));self.assertFalse((self.root/'run').exists())

 def test_committed_readback_refuses_missing_and_dirty_bodies(self):
  repo=self.root/'repo';here=repo/'tests/qualification/session';here.mkdir(parents=True)
  names=['run.py','test_run.py','dependencies.json','package-manifest.json']
  for n in names:(here/n).write_bytes(b'fixed')
  rows=[]
  for n in names:
   path=(here/n).relative_to(repo).as_posix();digest=hashlib.sha1(b'blob 5\0fixed').hexdigest();rows.append('100644 blob '+digest+'\t'+path+'\0')
  outputs=['a'*40,''.join(rows),'a'*40]
  api={'context':types.SimpleNamespace(_git=lambda *args:outputs.pop(0))}
  with patch.object(self.q,'REPOSITORY',repo),patch.object(self.q,'HERE',here):
   self.assertTrue(self.q.verify_committed(api,self.request,{'files':[]})['working_bodies_equal_commit'])
   outputs[:]=['a'*40,''.join(rows[:-1])]
   with self.assertRaises(self.q.Rejected):self.q.verify_committed(api,self.request,{'files':[]})
   (here/'run.py').write_bytes(b'drift');outputs[:]=['a'*40,''.join(rows)]
   with self.assertRaises(self.q.Rejected):self.q.verify_committed(api,self.request,{'files':[]})

 def test_dependencies_unsealed_fail_before_module_load(self):
  with patch.object(self.q,'DEPENDENCIES_SHA256',None):
   with self.assertRaises(self.q.Rejected):self.q.dependencies()

 def test_duplicate_and_nonfinite_request_rejected(self):
  for raw in (b'{"a":1,"a":2}',b'{"a":NaN}'):
   with self.assertRaises(self.q.Rejected):self.q.decode(raw)

if __name__=='__main__':unittest.main()
