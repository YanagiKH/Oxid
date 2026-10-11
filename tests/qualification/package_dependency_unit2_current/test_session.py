"""Portable pure tests. Synthetic contexts are not execution provenance."""
import copy,json,pathlib,tempfile,types,unittest,sys,time
sys.dont_write_bytecode=True
HERE=pathlib.Path(__file__).parent
s=types.ModuleType('session_under_test');exec(compile((HERE/'session.py').read_bytes(),str(HERE/'session.py'),'exec'),s.__dict__)
class SessionTests(unittest.TestCase):
 def contexts(self):
  prepared={'checkout':{'head':'a'*40,'tree':'b'*40},'source_inputs_sha256':'f'*64,'package_inputs_sha256':'c'*64,'accounting_transport':{'synthetic':'lineage'},'current_source':{'sha256':'f'*64},'historical_source_sha256':'d'*64,'assembled380':[{'path':'synthetic','bytes':0,'sha256':'0'*64}]}
  sessions={}
  for i,profile in enumerate(('debug','release')):
   sessions[profile]={'profile':profile,'source_inputs_sha256':prepared['source_inputs_sha256'],'package_inputs_sha256':prepared['package_inputs_sha256'],'accounting_transport':copy.deepcopy(prepared['accounting_transport']),'current_source':copy.deepcopy(prepared['current_source']),'historical_source_sha256':prepared['historical_source_sha256'],'started_ns':20+i,'execution_qualified':False,'invocation_id':str(i+1)*32,'binary_sha256':str(i+3)*64,'queue_tsv_sha256':str(i+5)*64,'queue_sha256':'7'*64,'compiled_source_map_sha256':s.compiled_map_digest(prepared['assembled380'])}
  invocation={'checkout_before':copy.deepcopy(prepared['checkout']),'checkout_after':copy.deepcopy(prepared['checkout']),'nonce':'0'*32,'started_ns':10,'finished_ns':30,'profiles':{p:{'session':copy.deepcopy(v)} for p,v in sessions.items()}}
  return prepared,invocation,sessions
 def test_metadata_join(self):
  self.assertTrue(s.validate_pair_metadata(*self.contexts()))
 def test_missing_duplicate_profile(self):
  p,i,v=self.contexts();del v['release']
  with self.assertRaises(ValueError):s.validate_pair_metadata(p,i,v)
  p,i,v=self.contexts();v['release']['profile']='debug';i['profiles']['release']['session']=copy.deepcopy(v['release'])
  with self.assertRaises(ValueError):s.validate_pair_metadata(p,i,v)
 def test_profile_replay(self):
  for field in ('invocation_id','binary_sha256','queue_tsv_sha256'):
   p,i,v=self.contexts();v['release'][field]=v['debug'][field];i['profiles']['release']['session']=copy.deepcopy(v['release'])
   with self.subTest(field=field),self.assertRaises(ValueError):s.validate_pair_metadata(p,i,v)
 def test_source_and_accounting_mutations(self):
  for field in ('source_inputs_sha256','package_inputs_sha256','historical_source_sha256','accounting_transport','current_source','compiled_source_map_sha256','queue_sha256'):
   p,i,v=self.contexts();v['release'][field]={'mutated':True} if isinstance(v['release'][field],dict) else '9'*64;i['profiles']['release']['session']=copy.deepcopy(v['release'])
   with self.subTest(field=field),self.assertRaises(ValueError):s.validate_pair_metadata(p,i,v)
 def test_checkout_drift_and_context_substitution(self):
  p,i,v=self.contexts();i['checkout_after']['head']='c'*40
  with self.assertRaises(ValueError):s.validate_pair_metadata(p,i,v)
  p,i,v=self.contexts();v['release']['invocation_id']='9'*32
  with self.assertRaises(ValueError):s.validate_pair_metadata(p,i,v)
 def test_no_premature_promotion_or_stale_time(self):
  for field,value in [('execution_qualified',True),('started_ns',1),('invocation_id','not-a-nonce')]:
   p,i,v=self.contexts();v['release'][field]=value;i['profiles']['release']['session']=copy.deepcopy(v['release'])
   with self.assertRaises(ValueError):s.validate_pair_metadata(p,i,v)
 def test_observer_exact_transcript(self):
  raw=b'\nrunning 1 test\ntest observer ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.01s\n'
  s.success(raw,['observer'])
  for bad in (raw+b'junk\n',raw+raw,raw.replace(b'1 passed',b'0 passed')):
   with self.assertRaises(ValueError):s.success(bad,['observer'])
 def test_raw_normalized_identity(self):
  with tempfile.TemporaryDirectory() as d:
   path=pathlib.Path(d)/'rows';row={'case':'a','mode':'project','started_ns':10,'finished_ns':20};contract={'case_ids':['a'],'modes':['project']};path.write_text(json.dumps(row)+'\n');s.observation_rows(path,contract,(9,21),[row]);row2=dict(row,started_ns=11);path.write_text(json.dumps(row2)+'\n')
   with self.assertRaises(ValueError):s.observation_rows(path,contract,(9,21),[row])
 def test_paths_and_json(self):
  for path in ('../x','/x','a//b','a\tb'):
   with self.assertRaises(ValueError):s.safe_name(path)
  with self.assertRaises(ValueError):s.strict_json('{"x":1,"x":2}')
 def test_manual_context_cannot_run_processes(self):
  import ast
  tree=ast.parse((HERE/'session.py').read_bytes());imports=[n.names[0].name for n in ast.walk(tree) if isinstance(n,ast.Import)];self.assertNotIn('subprocess',imports)
 def test_compile_window(self):
  inv={'started_ns':10,'finished_ns':50};comp={'spawn_ns':20,'finished_ns':30};observer={'spawn_ns':40};s.validate_compile_window(inv,comp,observer)
  for bad in ({'spawn_ns':1,'finished_ns':30},{'spawn_ns':20,'finished_ns':41},{'spawn_ns':31,'finished_ns':30}):
   with self.assertRaises(ValueError):s.validate_compile_window(inv,bad,observer)
 def test_live_pin_and_route_drift(self):
  with tempfile.TemporaryDirectory() as d:
   p=pathlib.Path(d)/'body';p.write_bytes(b'expected');pins={str(p):s.sha(b'expected')};s.verify_body_pins(pins,{},time.monotonic()+1);p.write_bytes(b'changed')
   with self.assertRaises(ValueError):s.verify_body_pins(pins,{},time.monotonic()+1)
   p.unlink();p.symlink_to('/dev/null')
   with self.assertRaises(ValueError):s.verify_body_pins(pins,{},time.monotonic()+1)
 def test_public_api_early_noncanonical_root(self):
  with tempfile.TemporaryDirectory() as d:
   target=pathlib.Path(d)/'real';target.mkdir();link=pathlib.Path(d)/'link';link.symlink_to(target)
   with self.assertRaises(ValueError):s.validate_session({'package_root':str(link)},{'profiles':{'debug':{},'release':{}}},{'debug':'x','release':'y'})
if __name__=='__main__':unittest.main()
