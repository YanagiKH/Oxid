"""Pure current-source context controls; never materializes or runs a worker."""
import copy,os,sys,types,unittest
from pathlib import Path
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
path=HERE/'source.py';s=types.ModuleType('tested_source_bridge');s.__file__=str(path);exec(compile(path.read_bytes(),str(path),'exec'),s.__dict__)
REPO=Path(os.environ.get('OXID_PACKAGE_UNIT4_TEST_REPOSITORY',str(HERE.parents[2]))).resolve()
class SourceControls(unittest.TestCase):
 @classmethod
 def setUpClass(cls):cls.context=s.capture(REPO)
 def test_complete_capture(self):self.assertTrue(s.verify_context(self.context));self.assertEqual(len(self.context['current_inputs']),376)
 def test_current_body_mutations(self):
  for change in ('changed','missing','extra'):
   c=copy.deepcopy(self.context)
   if change=='changed':c['current_inputs']['src/main.rs']+=b'\n'
   elif change=='missing':c['current_inputs'].pop('src/main.rs')
   else:c['current_inputs']['src/extra.rs']=b''
   with self.subTest(change=change),self.assertRaises(ValueError):s.verify_context(c)
 def test_wrong_stage_and_double_inverse(self):
  for label in ('current_inputs','predecessor_inputs'):
   c=copy.deepcopy(self.context);c[label]=copy.deepcopy(c['predecessor_inputs' if label=='current_inputs' else 'current_inputs'])
   with self.subTest(label=label),self.assertRaises(ValueError):s.verify_context(c)
 def test_manifest_patch_checkpoint_and_claim_mutations(self):
  for key,value in (('current_manifest',self.context['predecessor_manifest']),('transition_patch',b'changed'),('source_checkpoint',{'head':'0'*40,'tree':'0'*40}),('execution_qualified',True)):
   c=copy.deepcopy(self.context);c[key]=value
   with self.subTest(key=key),self.assertRaises(ValueError):s.verify_context(c)
 def test_duplicate_json_and_relative_escape(self):
  with self.assertRaises(ValueError):s.decode(b'{"x":1,"x":2}')
  for name in ('../outside','/absolute','x/../other'):
   with self.assertRaises(ValueError):s.safe(name)
if __name__=='__main__':unittest.main()
