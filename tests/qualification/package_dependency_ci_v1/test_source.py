import contextlib,hashlib,io,json,sys,tempfile,types,unittest
from pathlib import Path
from unittest.mock import patch
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
q=types.ModuleType('_source_family_test');q.__file__=str(HERE/'source.py');exec(compile((HERE/'source.py').read_bytes(),q.__file__,'exec'),q.__dict__)
class SourceTests(unittest.TestCase):
 def test_registry_complete(self):
  r=q._registry();self.assertEqual(len(r['original_entries']),173);self.assertEqual(len(r['assignments']),173);self.assertEqual(len(r['job_recipes']),15);self.assertEqual(len(r['build_recipes']),6)
  self.assertEqual(sum(x['family'] is not None for x in r['assignments'].values()),92)
  self.assertTrue(all(x['recipe_ids'] for x in r['assignments'].values()))
 def test_original_commands_covered(self):
  r=q._registry()
  for entry in r['original_entries']:
   original=entry['original'];command=original['active_command'];rid=entry['id']
   for recipe in r['assignments'][rid]['recipe_ids']:
    matches=[s for s in r['job_recipes'][recipe]['steps'] if rid in s['original_ids']]
    self.assertEqual(len(matches),1);self.assertIn(' '.join(command.split()),' '.join(matches[0]['command'].replace('\\\n',' ').replace('`\n',' ').split()))
 def test_projection_exact(self):
  r=q._registry();p=r['projections'];self.assertEqual(set(p),{'current-all376'});self.assertEqual(len(p['current-all376']['files']),376)
 def test_activation_blocked(self):
  r=q._registry();self.assertFalse(r['activation']['approved']);self.assertEqual(r['activation']['provider_jobs'],{});self.assertTrue(all(not x['operational'] for x in r['job_recipes'].values()))
 def test_build_fixed(self):
  for r in q._registry()['build_recipes'].values():
   self.assertEqual(r['argv'],['cargo','build','--release','--locked']);self.assertEqual(r['target_path'],'src/cli.rs');self.assertFalse(r['operational'])
 def test_no_dict_owner(self):
  for f in (q.verify_job,q.verify_join,q.registry):
   with self.assertRaises(ValueError):f({})
  with self.assertRaises(ValueError):q.JobSourceContext()
  with self.assertRaises(ValueError):q.JoinSourceContext()
 def test_unknown_projection_before_effect(self):
  with self.assertRaises(ValueError):q.projection({},'caller-subset')
 def test_duplicate_json(self):
  with self.assertRaises(ValueError):q.decode(b'{"a":1,"a":2}')
 def test_nonfinite(self):
  with self.assertRaises(ValueError):q.decode(b'{"a":NaN}')
 def test_safe_names(self):
  for n in ('../x','/x','a/../b','a\\b','C:x',''):
   with self.assertRaises(ValueError):q.safe_relative(n)
 def test_special_read(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'x';p.symlink_to('absent')
   with self.assertRaises((ValueError,FileNotFoundError)):q.read(p)
 def test_read_bounded(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'x';p.write_bytes(b'123')
   with self.assertRaises(ValueError):q.read(p,2)
 def test_deadline(self):
  with patch.object(q,'_CURRENT_DEADLINE',0):
   with self.assertRaises(ValueError):q.tick()
 def test_batch_union(self):
  names=['src/'+('x'*130)+str(i)+'.rs' for i in range(619)]
  with patch.object(q,'_command',lambda repo,*args:['/fixed/git','-C',str(repo),*args]):
   batches=q._batches(Path('/long source root'), 'a'*40,names);self.assertEqual([x for b in batches for x in b],sorted(names));self.assertGreater(len(batches),1)
   for b in batches:self.assertLessEqual(q._units(q._command(Path('/long source root'),'ls-tree','-rz','a'*40,'--',*b)),16000)
 def test_empty_batches(self):
  with patch.object(q,'_command',side_effect=AssertionError('no command')):self.assertEqual(q._batches(Path('/x'),'a'*40,[]),[])
 def test_oversized_path(self):
  with patch.object(q,'_command',lambda repo,*args:['git',*args]):
   with self.assertRaises(ValueError):q._batches(Path('/x'),'a'*40,['x'*17000])
 def test_owner_slot_rejected(self):
  with self.assertRaises(ValueError):q.owner()
 def test_input_schema_closure(self):
  d=q.decode((HERE/'inputs.json').read_bytes());self.assertEqual(set(d),{'schema','files','closed_roots'});names=[r['path'] for r in d['files']];self.assertEqual(names,sorted(set(names)));self.assertIn(q.BRIDGE,names)
 def test_immutable_record_document(self):
  record=q.JobSourceContext(q._TOKEN);q._CONTEXTS[record]={'document':q.canonical({'nested':{'x':1}})}
  try:
   value=record.document();value['nested']['x']=2;self.assertEqual(record.document()['nested']['x'],1)
  finally:del q._CONTEXTS[record]
 def test_committed_dirty_rejects(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);(root/'x').write_bytes(b'new');old=b'old';oid=hashlib.sha1(b'blob 3\0'+old).hexdigest()
   with patch.object(q,'_git',side_effect=[b'a'*40+b'\n',('100644 blob '+oid+'\tx\0').encode()]),patch.object(q,'_batches',return_value=[['x']]):
    with self.assertRaisesRegex(ValueError,'dirty committed'):q._committed(root,'a'*40,[{'path':'x'}])
 def test_committed_missing_rejects(self):
  with patch.object(q,'_git',side_effect=[b'a'*40+b'\n',b'']),patch.object(q,'_batches',return_value=[['x']]):
   with self.assertRaisesRegex(ValueError,'membership'):q._committed(Path('/inert'),'a'*40,[{'path':'x'}])
 def test_committed_head_drift_rejects(self):
  with patch.object(q,'_git',side_effect=[b'a'*40+b'\n',b'b'*40+b'\n',b'c'*40+b'\n']):
   with self.assertRaisesRegex(ValueError,'HEAD drift'):q._committed(Path('/inert'),'a'*40,[])
 def test_same_process_capture_owner(self):
  snap={'repository_path':'/inert','checkout_head':'a'*40,'event_sha':'b'*40,'job_recipe_id':'fixed'}
  with patch.object(q,'_bounded_snapshot',return_value=(snap,{})):
   record=q.capture_job('/inert',expected_head='a'*40,event_sha='b'*40,recipe_id='fixed')
   try:
    self.assertEqual(q.verify_job(record)['repository_path'],'/inert')
    with self.assertRaises(ValueError):q.verify_job(record.document())
   finally:del q._CONTEXTS[record]
 def test_source_drift_invalidates_owner(self):
  snap={'repository_path':'/inert','checkout_head':'a'*40,'event_sha':'b'*40,'job_recipe_id':'fixed'}
  with patch.object(q,'_bounded_snapshot',return_value=(snap,{})):
   record=q.capture_job('/inert',expected_head='a'*40,event_sha='b'*40,recipe_id='fixed')
  try:
   with patch.object(q,'_bounded_snapshot',return_value=(dict(snap,checkout_head='c'*40),{})):
    with self.assertRaisesRegex(ValueError,'drift'):q.verify_job(record)
  finally:del q._CONTEXTS[record]
 def test_complete_mapping_original_identity(self):
  r=q._registry()
  for entry in r['original_entries']:
   original=entry['original'];self.assertEqual(hashlib.sha256(original['active_command'].encode()).hexdigest(),original['active_command_sha256'])
 def test_read_growth_cap(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'x';p.write_bytes(b'123')
   with patch.object(Path,'open',return_value=io.BytesIO(b'1234')):
    with self.assertRaisesRegex(ValueError,'grew beyond'):q.read(p,3)
 def test_push_payload(self):
  env={'GITHUB_EVENT_NAME':'push','GITHUB_REF':'refs/heads/main'};event={'after':'a'*40,'ref':'refs/heads/main'}
  q._event_commit(event,env,'a'*40,'a'*40)
  for changed in (dict(event,after='b'*40),dict(event,ref='refs/heads/other'),dict(event,head_commit={'id':'b'*40})):
   with self.assertRaises(ValueError):q._event_commit(changed,env,'a'*40,'a'*40)
 def test_dispatch_optional_payload_association(self):
  env={'GITHUB_EVENT_NAME':'workflow_dispatch','GITHUB_REF':'refs/heads/main'}
  q._event_commit({},env,'a'*40,'a'*40)
  with self.assertRaises(ValueError):q._event_commit({'ref':'refs/heads/other'},env,'a'*40,'a'*40)
 def test_pr_distinction(self):
  env={'GITHUB_EVENT_NAME':'pull_request'};event={'pull_request':{'head':{'sha':'a'*40},'merge_commit_sha':'b'*40}}
  q._event_commit(event,env,'a'*40,'b'*40)
  with self.assertRaises(ValueError):q._event_commit(event,env,'b'*40,'b'*40)
 @contextlib.contextmanager
 def adapter_fixture(self, body=b"VALUE = 1\n"):
  with tempfile.TemporaryDirectory() as temp:
   base=Path(temp);a=base/'package_dependency_ci_v1';a.mkdir();b=base/q._ADAPTER_DIRECTORY;b.mkdir()
   values={'bounded.py':body,'recipes.json':b'{}\n','test_bounded.py':b'# inert fixture\n'}
   for name,raw in values.items():(b/name).write_bytes(raw)
   manifest={'schema':'oxid-package-dependency-bounded-current-package-v1','source_dependency_sha256':q._LOADED_SHA,'operational_release':None,'files':{n:{'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()} for n,raw in values.items()}}
   raw=q.canonical(manifest);(b/'package-manifest.json').write_bytes(raw);pkg={'adapters':{'bounded':{'directory':q._ADAPTER_DIRECTORY,'manifest':{'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}}}}
   first=q.JobSourceContext(q._TOKEN);second=q.JobSourceContext(q._TOKEN);old=sys.modules.pop(q._ADAPTER_KEY,None)
   with patch.object(q,'HERE',a),patch.object(q,'_ADAPTERS',{}),patch.object(q,'_CONTEXTS',{first:{},second:{}}),patch.object(q,'owner',return_value=q),patch.object(q,'verify_job',return_value={'authority':'synthetic-pure-test-only'}),patch.object(q,'package',return_value=pkg):
    try:yield b,pkg,first,second
    finally:
     sys.modules.pop(q._ADAPTER_KEY,None)
     if old is not None:sys.modules[q._ADAPTER_KEY]=old
 def test_adapter_issued_and_reused_only_for_registered_context(self):
  with self.adapter_fixture() as (b,pkg,first,second):
   module=q.load_adapter(first,'bounded');self.assertEqual(module.VALUE,1);self.assertTrue(q.owns_adapter(module,first))
   with self.assertRaisesRegex(ValueError,'not issued'):q.owns_adapter(module,second)
   self.assertIs(q.load_adapter(second,'bounded'),module);self.assertTrue(q.owns_adapter(module,second))
 def test_adapter_occupied_slot_rejected(self):
  with self.adapter_fixture() as (b,pkg,first,second):
   sys.modules[q._ADAPTER_KEY]=types.ModuleType(q._ADAPTER_KEY)
   with self.assertRaisesRegex(ValueError,'preexisting'):q.load_adapter(first,'bounded')
   self.assertEqual(q._ADAPTERS,{})
 def test_adapter_failed_import_cleanup(self):
  with self.adapter_fixture(b"raise RuntimeError('synthetic import failure')\n") as (b,pkg,first,second):
   with self.assertRaisesRegex(RuntimeError,'synthetic'):q.load_adapter(first,'bounded')
   self.assertNotIn(q._ADAPTER_KEY,sys.modules);self.assertEqual(q._ADAPTERS,{})
 def test_adapter_extra_missing_directory_rejects(self):
  for kind in ('extra','missing','directory'):
   with self.adapter_fixture() as (b,pkg,first,second):
    if kind=='extra':(b/'extra.py').write_bytes(b'')
    elif kind=='missing':(b/'test_bounded.py').unlink()
    else:(b/'empty').mkdir()
    with self.assertRaisesRegex(ValueError,'roster'):q.load_adapter(first,'bounded')
 def test_adapter_postload_body_drift(self):
  with self.adapter_fixture() as (b,pkg,first,second):
   module=q.load_adapter(first,'bounded');(b/'bounded.py').write_bytes(b'VALUE = 2\n')
   with self.assertRaisesRegex(ValueError,'body drift'):q.owns_adapter(module,first)
 def test_adapter_manifest_drift(self):
  with self.adapter_fixture() as (b,pkg,first,second):
   module=q.load_adapter(first,'bounded');(b/'package-manifest.json').write_bytes(b'{}')
   with self.assertRaisesRegex(ValueError,'manifest body'):q.owns_adapter(module,first)
 def test_adapter_slot_substitution_after_load(self):
  with self.adapter_fixture() as (b,pkg,first,second):
   module=q.load_adapter(first,'bounded');sys.modules[q._ADAPTER_KEY]=types.ModuleType(q._ADAPTER_KEY)
   with self.assertRaisesRegex(ValueError,'slot drift'):q.owns_adapter(module,first)
 def test_adapter_source_dependency_rejected(self):
  with self.adapter_fixture() as (b,pkg,first,second):
   value=q.decode((b/'package-manifest.json').read_bytes());value['source_dependency_sha256']='0'*64;raw=q.canonical(value);(b/'package-manifest.json').write_bytes(raw);pkg['adapters']['bounded']['manifest']={'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}
   with self.assertRaisesRegex(ValueError,'source dependency'):q.load_adapter(first,'bounded')
 @contextlib.contextmanager
 def production_fixture(self, process_body=b"VALUE=1\n"):
  with tempfile.TemporaryDirectory() as temp:
   base=Path(temp);a=base/'package_dependency_ci_v1';a.mkdir();pkg={'production_owners':{}};roots={};old={}
   for name,spec in q._PRODUCTION_SPECS.items():
    root=base/spec['directory'];root.mkdir();roots[name]=root
    values={spec['entry']:process_body if name=='process' else b'VALUE=2\n',spec['test']:b'# inert fixture\n'}
    for path,body in values.items():(root/path).write_bytes(body)
    manifest={'schema':spec['schema'],'baseline':spec['baseline'],'files':[{'path':path,'bytes':len(body),'sha256':hashlib.sha256(body).hexdigest()} for path,body in sorted(values.items())]};raw=q.canonical(manifest);(root/'package-manifest.json').write_bytes(raw)
    pkg['production_owners'][name]={'directory':spec['directory'],'manifest':{'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}};old[spec['key']]=sys.modules.pop(spec['key'],None)
   with patch.object(q,'HERE',a),patch.object(q,'_PRODUCTION_OWNERS',{}),patch.object(q,'owner',return_value=q),patch.object(q,'package',side_effect=lambda:(q._production_rows(pkg),pkg)[1]):
    try:yield roots,pkg
    finally:
     for key,value in old.items():
      sys.modules.pop(key,None)
      if value is not None:sys.modules[key]=value
 def test_production_canonical_reuse(self):
  with self.production_fixture():
   process=q.load_production_owner('process');machine=q.load_production_owner('machine');self.assertEqual(process.VALUE,1);self.assertEqual(machine.VALUE,2);self.assertTrue(q.owns_production_owner(process));self.assertIs(q.load_production_owner('process'),process)
 def test_production_authenticates_other_package_before_load(self):
  with self.production_fixture() as (roots,pkg):
   (roots['machine']/'machine.py').write_bytes(b'changed')
   with self.assertRaisesRegex(ValueError,'body drift'):q.load_production_owner('process')
   self.assertEqual(q._PRODUCTION_OWNERS,{})
 def test_production_foreign_slot_rejects(self):
  with self.production_fixture():
   sys.modules[q._PRODUCTION_SPECS['machine']['key']]=types.ModuleType('forged')
   with self.assertRaisesRegex(ValueError,'preexisting'):q.load_production_owner('process')
 def test_production_partial_import_cleanup(self):
  with self.production_fixture(b"raise RuntimeError('synthetic failure')\n"):
   with self.assertRaisesRegex(RuntimeError,'synthetic'):q.load_production_owner('process')
   self.assertEqual(q._PRODUCTION_OWNERS,{});self.assertNotIn(q._PRODUCTION_SPECS['process']['key'],sys.modules)
 def test_production_postload_drift(self):
  with self.production_fixture() as (roots,pkg):
   owner=q.load_production_owner('process');(roots['machine']/'machine.py').write_bytes(b'changed')
   with self.assertRaisesRegex(ValueError,'body drift'):q.owns_production_owner(owner)
 def test_production_baseline_identity(self):
  with self.production_fixture() as (roots,pkg):
   p=roots['process']/'package-manifest.json';v=q.decode(p.read_bytes());v['baseline']['sha256']='0'*64;raw=q.canonical(v);p.write_bytes(raw);pkg['production_owners']['process']['manifest']={'bytes':len(raw),'sha256':hashlib.sha256(raw).hexdigest()}
   with self.assertRaisesRegex(ValueError,'baseline'):q.load_production_owner('process')
 def test_production_extra_directory(self):
  with self.production_fixture() as (roots,pkg):
   (roots['machine']/'extra').mkdir()
   with self.assertRaisesRegex(ValueError,'roster'):q.load_production_owner('process')
 def test_production_unknown_role(self):
  with self.assertRaisesRegex(ValueError,'unknown'):q.load_production_owner('arbitrary')
if __name__=='__main__':unittest.main()
