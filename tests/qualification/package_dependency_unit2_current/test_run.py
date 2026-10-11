"""Pure producer boundary controls. No compiler, target or worker invocation."""
import copy,hashlib,json,os,struct,sys,tempfile,time,types,unittest
from pathlib import Path
from unittest import mock
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent

def module(name):
 path=HERE/(name+'.py');value=types.ModuleType('pure_'+name);value.__file__=str(path);exec(compile(path.read_bytes(),str(path),'exec'),value.__dict__);return value
r=module('run');q=module('session');p=module('process')

class ProducerControls(unittest.TestCase):
 def machine(self):return {'roles':{'cargo':'/tools/bin/cargo','rustc':'/tools/bin/rustc','rustdoc':'/tools/bin/rustdoc','cc':'/usr/bin/cc','cxx':'/usr/bin/c++','ar':'/usr/bin/ar','python':'/usr/bin/python3','git':'/usr/bin/git'},'path':'/tools/bin:/usr/bin:/bin'}
 def test_exact_profile_commands(self):
  for profile in ('debug','release'):
   command,env=r.compile_command(self.machine(),profile,Path('/fresh/source'),Path('/fresh/target'),Path('/fresh/work'),Path('/fresh/cargo'))
   self.assertEqual('--release' in command,profile=='release');self.assertIn('--no-run',command);self.assertIn('--offline',command);self.assertIn('--locked',command)
   prefix='TEST' if profile=='debug' else 'RELEASE';other='RELEASE' if profile=='debug' else 'TEST';self.assertEqual(env['CARGO_PROFILE_'+prefix+'_OPT_LEVEL'],'0' if profile=='debug' else '3');self.assertFalse(any(k.startswith('CARGO_PROFILE_'+other+'_') for k in env));self.assertNotIn('LIBC_CI',env);self.assertNotIn('PYTHONPATH',env)
  with self.assertRaises(ValueError):r.compile_command(self.machine(),'other',Path('/s'),Path('/t'),Path('/w'),Path('/c'))
 def test_ancestor_manifests_and_home_config_absent(self):
  names=r.forbidden_config_paths(Path('/fresh/profile/source'),Path('/fresh/cargo'),Path('/fresh/work'));self.assertIn('/fresh/profile/Cargo.toml',names);self.assertIn('/Cargo.toml',names);self.assertIn('/fresh/work/home/.cargo/config.toml',names);self.assertNotIn('/fresh/profile/source/Cargo.toml',names);self.assertNotIn('/fresh/work/home/config.toml',names)
 def test_seven_runtime_commands(self):
  for profile in ('debug','release'):
   command=r.runtime_commands(Path('/package'),Path('/elf'),Path('/fresh/'+profile),profile,'/usr/bin/python3');self.assertEqual(len(command['argv']),7);self.assertEqual(len(command['environments']),7);self.assertEqual(command['argv'][6][3],profile);self.assertEqual(command['argv'][3][1:],["--exact","frontend::oir::unit2_observer::observe_source_queue","--test-threads=1"])
   self.assertIn('OXID_OBSERVER_QUEUE',command['environments'][3]);self.assertNotIn('OXID_OBSERVER_QUEUE',command['environments'][4]);self.assertIn('OXID_RESOURCE_FIXTURES',command['environments'][4])
 def test_fresh_path_rejects_existing_and_symlink(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);self.assertEqual(r.fresh(root/'new'),root/'new');(root/'exists').mkdir();(root/'link').symlink_to(root/'missing')
   for path in (root/'exists',root/'link'):
    with self.assertRaises(ValueError):r.fresh(path)
 def test_absent_config_requires_absolute_and_absent(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);r.absent([str(root/'missing')]);(root/'config.toml').write_text('bad');(root/'link').symlink_to(root/'missing')
   for name in ('relative',str(root/'config.toml'),str(root/'link')):
    with self.assertRaises(ValueError):r.absent([name])
 def test_tree_rejects_fifo_before_open(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);os.mkfifo(root/'fifo')
   with self.assertRaises(ValueError):r.tree(root,lambda:None,p)
   with self.assertRaises(ValueError):p.disk_bytes(root,lambda:None)
 def test_tree_rejects_symlink_root(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);(root/'actual').mkdir();(root/'link').symlink_to(root/'actual',target_is_directory=True)
   with self.assertRaises(ValueError):r.tree(root/'link',lambda:None,p)
 def test_git_identity_capture_is_fixed_readonly(self):
  expected=['a'*40,'b'*40];calls=[]
  def fake(argv,**kw):
   calls.append((argv,kw));return types.SimpleNamespace(returncode=0,stderr=b'',stdout=(expected[len(calls)-1]+'\n').encode())
  with mock.patch.object(r.subprocess,'run',side_effect=fake):self.assertEqual(r.checkout_identity(Path('/repo'),'/usr/bin/git'),{'head':expected[0],'tree':expected[1]})
  self.assertEqual([call[0] for call in calls],[['/usr/bin/git','-C','/repo','rev-parse','--verify',expr] for expr in ('HEAD','HEAD^{tree}')]);self.assertTrue(all(call[1]['timeout']==15 and call[1]['env']['GIT_OPTIONAL_LOCKS']=='0' for call in calls))
 def test_cargo_artifact_real_body_header_and_profile(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);source=root/'source';target=root/'target';source.mkdir();elf=target/'x86_64-unknown-linux-gnu/release/deps/oxid-test';elf.parent.mkdir(parents=True)
   header=bytearray(64);header[:7]=b'\x7fELF\x02\x01\x01';struct.pack_into('<HHI',header,16,3,62,1);struct.pack_into('<H',header,52,64);elf.write_bytes(header)
   event={'reason':'compiler-artifact','package_id':'path+'+source.as_uri()+'#oxid@0.9.0','manifest_path':str(source/'Cargo.toml'),'target':{'src_path':str(source/'src/cli.rs'),'name':'oxid','kind':['bin']},'profile':{'opt_level':'3','debuginfo':0,'debug_assertions':False,'overflow_checks':False,'test':True},'fresh':False,'executable':str(elf)};stdout=root/'cargo.log'
   def write(value):stdout.write_text(json.dumps(value)+'\n'+json.dumps({'reason':'build-finished','success':True})+'\n')
   write(event);self.assertEqual(r.cargo_artifact(stdout,source,target,'release',q,p,lambda:None)[1],elf)
   for key,value in [('fresh',True),('package_id','oxid@0.9.0'),('profile',dict(event['profile'],debuginfo=2))]:
    bad=copy.deepcopy(event);bad[key]=value;write(bad)
    with self.assertRaises(ValueError):r.cargo_artifact(stdout,source,target,'release',q,p,lambda:None)
   write(event);elf.write_bytes(b'\x7fELF')
   with self.assertRaises(ValueError):r.cargo_artifact(stdout,source,target,'release',q,p,lambda:None)
 def test_digest_deadline_precedes_read_loop(self):
  with tempfile.TemporaryDirectory() as d:
   path=Path(d)/'body';path.write_bytes(b'body')
   def stop():raise ValueError('stop')
   with self.assertRaisesRegex(ValueError,'stop'):p.digest(path,stop)
 def test_supervisor_failure_terminates_reaps_and_retains_evidence(self):
  with tempfile.TemporaryDirectory() as d:
   root=Path(d);calls=[];count=[0]
   def tick():
    count[0]+=1
    if count[0]==2:raise ValueError('synthetic stop after spawn')
   child=mock.Mock(pid=123456,poll=mock.Mock(return_value=None),wait=mock.Mock(return_value=-15));selector=mock.Mock();selector.get_map.return_value={'open':1}
   with mock.patch.object(p.subprocess,'Popen',return_value=child),mock.patch.object(p.selectors,'DefaultSelector',return_value=selector),mock.patch.object(p,'members',return_value=[]),mock.patch.object(p.os,'killpg',side_effect=lambda pid,sig:calls.append((pid,sig))):
    result=p.run_operation(['/pinned/target'],root,{},root/'operation',0,p.RUNTIME,tick,[root])
   self.assertIn('synthetic stop',result['failure']);self.assertEqual(result['pid'],123456);self.assertEqual(result['exit'],-15);self.assertIsNone(result['cleanup_error']);self.assertEqual(result['remaining_owned_group'],[]);self.assertEqual(calls,[(123456,p.signal.SIGTERM)]);child.wait.assert_called_once_with(timeout=5);self.assertEqual(set(result['evidence']),{'ownership.json','stdout.log','stderr.log'})
   self.assertEqual(json.loads((root/'operation/ownership.json').read_bytes())['pgid'],123456)
 def test_optional_process_metadata_does_not_erase_membership(self):
  fields=['0']*22;fields[1]='1';fields[2]='123';fields[19]='77';fields[21]='2';path=mock.MagicMock();path.read_text.return_value='123 (worker) '+' '.join(fields);path.parent.name='123';path.parent.__truediv__.return_value.read_bytes.side_effect=OSError('gone');factory=mock.Mock();factory.return_value.glob.return_value=[path]
  with mock.patch.object(p,'Path',factory),mock.patch.object(p.os,'readlink',side_effect=OSError('gone')):rows=p.members(123)
  self.assertEqual(len(rows),1);self.assertEqual(rows[0]['pid'],123);self.assertEqual(rows[0]['start_ticks'],77);self.assertIn('cmdline_unavailable',rows[0]);self.assertIn('exe_unavailable',rows[0])
 def test_unsealed_helpers_refuse(self):
  with mock.patch.object(r,'HELPER_PINS',{}):
   with self.assertRaisesRegex(ValueError,'unsealed'):r.helpers()
 def test_no_kh_import_or_fixed_workspace(self):
  for name in ('run','process'):
   body=(HERE/(name+'.py')).read_text();self.assertNotIn('/workspace/',body);self.assertNotIn('KH/',body)

if __name__=='__main__':unittest.main()
