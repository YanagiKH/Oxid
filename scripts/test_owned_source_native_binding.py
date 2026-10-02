"""Reviewer regressions: one positive and 32 coherently rebound rejection controls.

Run with NATIVE_OBSERVATIONS pointing to a complete native collection. These
controls inspect and copy receipts; they do not execute native programs.
"""
import copy,hashlib,json,os,shutil,sys,tempfile,unittest
from pathlib import Path
import owned_source_native as n
ROOT=Path(os.environ['NATIVE_OBSERVATIONS']) if 'NATIVE_OBSERVATIONS' in os.environ else None
FIXTURE=Path(n.h.__file__).resolve().parents[1]/'tests/fixtures/owned_source/native-probe-sources.json'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,v):p.write_text(json.dumps(v,indent=2)+'\n')
class FullBinding(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  if ROOT is None: raise unittest.SkipTest('requires actual collected native receipts')
  cls.items={i['id']:{**i,'source':s} for s,i in n.h.native_probe_expectations(FIXTURE)}
  cls.programs={r['id']:n.h.decode_source_program(r['program']) for r in n.strict_json(FIXTURE)['cases']}
 def control(self,change,case='guarded-scalar-store',reject=True):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/case;shutil.copytree(ROOT/'debug'/case,p)
   c=n.strict_json(p/'catalog.json');e=n.strict_json(p/'payload-enrichment.json')
   # Preserve the independently checked one-ELF hardlink structure in our copy.
   for r in map(json.loads,(p/'receipts.jsonl').read_text().splitlines()):
    link=p/r['native']['run_directory']/'program.elf';link.unlink();os.link(p/r['native']['artifact'],link)
   change(p,c,e)
   c['files']={name:sha(p/name) for name in c['files']}
   save(p/'catalog.json',c);e['catalog_sha256']=sha(p/'catalog.json');save(p/'payload-enrichment.json',e)
   if reject:
    with self.assertRaises((ValueError,KeyError,TypeError,IndexError,StopIteration)):n.compare_native_case(self.items[case],self.programs[case],p)
   else:n.compare_native_case(self.items[case],self.programs[case],p)
 def rewrite_row(self,p,name,change):
  row=n.strict_json(p/(name+'.json'));change(row)
  native=row['native'];native['stderr']=''.join(f"__UNIT4B_STORE {s['site']} {s['bits']}\n" for s in native['stores'])+native['diagnostic_stderr']
  save(p/(name+'.json'),row);(p/(name+'.stdout')).write_text(native['stdout']);(p/(name+'.stderr')).write_text(native['stderr'])
  rows=[row if r['name']==name else r for r in map(json.loads,(p/'receipts.jsonl').read_text().splitlines())]
  (p/'receipts.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in rows))
 def test_exact_copied_case_baseline(self):self.control(lambda *_:None,reject=False)
 def test_individual_enrichment_physical_fields(self):
  mutations=[('wire_type','ptr'),('value_semantics','occurrence-only'),('function',99),('payload_byte_offset',999),('record',0),('empty_record_token',True)]
  for key,value in mutations:
   with self.subTest(key=key):self.control(lambda p,c,e:e['sites'][3].update({key:value}))
  for key,value in [('byte_offset',999),('base','%false'),('llvm_line',1),('llvm_text','%false = getelementptr i8, ptr %wrong, i64 999')]:
   with self.subTest(gep=key):self.control(lambda p,c,e:e['sites'][3]['actual_destination_gep'].update({key:value}))
 def test_ignored_physical_u64_domain(self):
  self.control(lambda p,c,e:self.rewrite_row(p,'probe-12',lambda r:r['native']['stores'][0].update(bits=2**80)))
 def test_i32_physical_width_not_modulo(self):
  def change(p,c,e):
   site=next(s['site'] for s in c['stores'] if s['kind']=='WriteField' and s['function']==1)
   self.rewrite_row(p,'probe-67',lambda r:next(v for v in r['native']['stores'] if v['site']==site).update(bits=2**32+1))
  self.control(change,case='guarded-overflow-after-write')
 def test_pointer_occurrence_placeholder(self):
  def change(p,c,e):
   observed=n.strict_json(p/'probe-67.json')['native']['stores'];site=next(s['site'] for s in c['stores'] if s['type']=='ptr' and any(v['site']==s['site'] for v in observed))
   self.rewrite_row(p,'probe-67',lambda r:next(v for v in r['native']['stores'] if v['site']==site).update(bits=1))
  self.control(change,case='guarded-overflow-after-write')
 def test_contradictory_comparison_summary(self):self.control(lambda p,c,e:self.rewrite_row(p,'probe-12',lambda r:r.update(reference_native_equal=False)))
 def test_default_budget_binding(self):
  def change(p,c,e):
   r=n.strict_json(p/'budget-0.json');r['name']='default';r['native'].update(name='default',artifact='default.elf',run_directory='default-run',arguments=[])
   self.rewrite_row(p,'default',lambda row:(row.clear(),row.update(r)))
  self.control(change)
 def test_default_and_fixed_observation_kind(self):
  for name in ('default','fixed-14'):
   with self.subTest(name=name):self.control(lambda p,c,e:self.rewrite_row(p,name,lambda r:r['native'].update(instrumented=True)))
 def test_required_artifact_inventory(self):
  for missing in ('all','argv.elf','budget-0.json','raw-view.json','probe.ll'):
   def change(p,c,e):
    if missing=='all':c['files']={};(p/'argv.elf').write_bytes(b'not ELF\n')
    else:c['files'].pop(missing)
   with self.subTest(missing=missing):self.control(change)
 def test_canonical_catalog_hashes(self):
  for key in ('raw_witness_sha256','argv_llvm_sha256','probe_llvm_sha256','argv_probe_llvm_sha256'):
   with self.subTest(key=key):self.control(lambda p,c,e:c.update({key:'0'*64}))
 def test_crossbound_local_module_and_source(self):
  def module(p,c,e):
   for name in ('bounded.ll','probe-original.ll'):
    with (p/name).open('a') as f:f.write('; distinct reviewer bytes\n')
   e['llvm_sha256']=sha(p/'bounded.ll')
  with self.subTest(binding='module'):self.control(module)
  with self.subTest(binding='source'):self.control(lambda p,c,e:(p/'guarded-scalar-store.ox').write_text('fn main() -> i32 { return 999; }\n'))
 def test_exact_captured_bytes(self):
  for case,name in [('guarded-scalar-store','budget-0.stderr'),('guarded-replace-self','budget-42.stdout')]:
   def change(p,c,e):
    file=p/name;file.write_bytes(file.read_bytes().replace(b'\n',b'\r\n'))
   with self.subTest(name=name):self.control(change,case=case)
 def test_exact_module_bytes(self):
  for name,prefix in [('probe-original.ll','probe_llvm'),('argv.ll','argv_llvm')]:
   def change(p,c,e):
    file=p/name;file.write_bytes(file.read_bytes().replace(b'\n',b'\r\n'));c[prefix+'_sha256']=sha(file);c[prefix+'_path']=str(file)
   with self.subTest(module=name):self.control(change)
if __name__=='__main__':unittest.main()
