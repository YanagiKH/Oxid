"""Synthetic protocol controls only; no compiler/corpus expectations or execution."""
import copy,hashlib,json,unittest,os,pathlib,subprocess,sys,tempfile
from parse_debug import parse,DebugSyntaxError
from run import normalize,unique
class AdapterShape(unittest.TestCase):
    def setUp(self):
        self.host={'os':'linux','architecture':'x86_64','python_pointer_width':64}
        self.case={'id':'shape-only','source':{'sha256':hashlib.sha256(b'').hexdigest(),'bytes':0,'path':'shape.ox'},'expected':{'relation_to_original':[]}}
        self.receipt={'candidate_source_manifest_sha256':'a'*64,'observer_source_sha256':'b'*64,'binary':{'sha256':'c'*64},'profile':'debug','target':'x86_64-unknown-linux-gnu'}
        row={'mode':'ProjectCandidate','mode_execution_index':0,'source_generation':7,'runtime_os':'linux','runtime_architecture':'x86_64','pointer_width':64,'executed':True,'diagnostics':[],'json_diagnostic_renderings':[],'human_diagnostic_rendering':'','ast':None}
        second=copy.deepcopy(row);second.update(mode='OwnedCandidate',mode_execution_index=1)
        self.raw={'case_id':self.case['id'],'nonce':'nonce','schema':'oxid-unit4-parser-raw-v1','source_utf8':'','display_path':'shape.ox','observations':[row,second]}
    def check_failure(self,fn):
        fn(self.raw)
        with self.assertRaises((AssertionError,KeyError)):normalize(self.raw,self.case,self.receipt,'nonce',self.host)
    def test_missing(self):self.check_failure(lambda r:r['observations'].pop())
    def test_extra(self):self.check_failure(lambda r:r['observations'].append(copy.deepcopy(r['observations'][0])))
    def test_duplicate(self):self.check_failure(lambda r:r['observations'].__setitem__(1,copy.deepcopy(r['observations'][0])))
    def test_unexecuted(self):self.check_failure(lambda r:r['observations'][0].update(executed=False))
    def test_zero(self):self.check_failure(lambda r:r.update(observations=[]))
    def test_wrong_nonce(self):self.check_failure(lambda r:r.update(nonce='other'))
    def test_wrong_source(self):self.check_failure(lambda r:r.update(source_utf8='different'))
    def test_wrong_generation(self):self.check_failure(lambda r:r['observations'][1].update(source_generation=8))
    def test_measured_host_mismatch(self):
        self.host['architecture']='aarch64'
        with self.assertRaisesRegex(AssertionError,'measured host differ'):
            normalize(self.raw,self.case,self.receipt,'nonce',self.host)
    def test_duplicate_json_key(self):
        with self.assertRaises(AssertionError):json.loads('{"a":1,"a":2}',object_pairs_hook=unique)
    def test_unsupported_hosts_reject_before_candidate(self):
        contract=os.environ.get('UNIT4_HOST_CONTROL_CONTRACT')
        if not contract:self.skipTest('requires explicit frozen-contract input for process controls')
        rows=[]
        with tempfile.TemporaryDirectory(prefix='oxid-parser-host-control-') as directory:
            root=pathlib.Path(directory);dummy=root/'dummy';dummy.write_text('never execute')
            identity={'path':str(dummy),'bytes':dummy.stat().st_size,'sha256':hashlib.sha256(dummy.read_bytes()).hexdigest()}
            receipt=root/'build.json';receipt.write_text(json.dumps({'status':'built','exit_code':0,'binary':identity,'overlay_manifest':identity,'rustc':identity,'target':'x86_64-unknown-linux-gnu'}))
            checkpoint=root/'checkpoint.json';checkpoint.write_text('{}')
            for mode in ['machine','os','width']:
                with self.subTest(unsupported=mode):
                    work=root/mode;work.mkdir();marker=work/'candidate-invocation';destination=work/'results'
                    patch="import platform, struct, subprocess, pathlib, types\nactual=platform.uname()\nvalues={name:getattr(actual,name) for name in ['system','node','release','version','machine']}\n"
                    if mode=='machine':patch+="values['machine']='aarch64'\nplatform.uname=lambda: types.SimpleNamespace(**values)\n"
                    elif mode=='os':patch+="values['system']='Darwin'\nplatform.uname=lambda: types.SimpleNamespace(**values)\n"
                    else:patch+="original_size=struct.calcsize\nstruct.calcsize=lambda fmt: 4 if fmt=='P' else original_size(fmt)\n"
                    patch+="def forbidden(*args, **kwargs):\n    pathlib.Path("+repr(str(marker))+").write_text('attempted')\n    raise RuntimeError('unexpected candidate invocation')\nsubprocess.Popen=forbidden\n"
                    (work/'sitecustomize.py').write_text(patch)
                    helper=pathlib.Path(__file__).resolve().parent/'run.py'
                    args=[sys.executable,str(helper),'--build-receipt',str(receipt),'--contract-dir',contract,'--checkpoint',str(checkpoint),'--output',str(destination)]
                    environment=os.environ.copy();environment['PYTHONPATH']=str(work);environment.pop('PYTHONOPTIMIZE',None)
                    result=subprocess.run(args,env=environment,capture_output=True,text=True)
                    self.assertNotEqual(result.returncode,0);self.assertIn('UNSUPPORTED_PLATFORM',result.stderr)
                    self.assertFalse(destination.exists());self.assertFalse(marker.exists())
                    rows.append({'unsupported':mode,'helper_sha256':hashlib.sha256(helper.read_bytes()).hexdigest(),'argv':args,'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr,'destination_created':destination.exists(),'candidate_invocation_attempted':marker.exists()})
        if os.environ.get('UNIT4_HOST_CONTROL_REPORT'):
            pathlib.Path(os.environ['UNIT4_HOST_CONTROL_REPORT']).write_text(json.dumps({'schema':'oxid-unit4-unsupported-host-controls-v1','status':'pass'if len(rows)==3else'failed-incomplete-controls','controls':rows},sort_keys=True,indent=2)+'\n')
    def test_optimized_helpers_reject_before_work(self):
        rows=[]
        with tempfile.TemporaryDirectory(prefix='oxid-parser-opt-control-') as directory:
            root=pathlib.Path(directory);marker=root/'child-invocation';hook=root/'sitecustomize.py'
            hook.write_text("import subprocess, pathlib\ndef forbidden(*args, **kwargs):\n    pathlib.Path("+repr(str(marker))+").write_text('attempted')\n    raise RuntimeError('unexpected child invocation')\nsubprocess.Popen=forbidden\n")
            for name in ['prepare.py','build.py','run.py','passivity.py']:
                for mode in ['-O','PYTHONOPTIMIZE']:
                    with self.subTest(helper=name,mode=mode):
                        destination=root/(name+'-'+mode)
                        helper=pathlib.Path(__file__).resolve().parent/name
                        args=[sys.executable]+(['-O']if mode=='-O'else[])+[str(helper),'--output',str(destination)]
                        environment=os.environ.copy();environment['PYTHONPATH']=str(root);environment.pop('PYTHONOPTIMIZE',None)
                        if mode=='PYTHONOPTIMIZE':environment['PYTHONOPTIMIZE']='1'
                        result=subprocess.run(args,env=environment,capture_output=True,text=True)
                        self.assertNotEqual(result.returncode,0)
                        self.assertIn('OPTIMIZED_PYTHON_UNSUPPORTED',result.stderr)
                        self.assertFalse(destination.exists());self.assertFalse(marker.exists())
                        rows.append({'helper':name,'helper_sha256':hashlib.sha256(helper.read_bytes()).hexdigest(),'mode':mode,'argv':args,'exit_code':result.returncode,'stdout':result.stdout,'stderr':result.stderr,'destination_created':destination.exists(),'child_invocation_attempted':marker.exists()})
        if os.environ.get('UNIT4_OPT_CONTROL_REPORT'):
            pathlib.Path(os.environ['UNIT4_OPT_CONTROL_REPORT']).write_text(json.dumps({'schema':'oxid-unit4-optimized-python-controls-v1','status':'pass'if len(rows)==8else'failed-incomplete-controls','controls':rows},sort_keys=True,indent=2)+'\n')
    def test_debug_complete(self):
        self.assertEqual(parse('ExprId(3)'),{'tag':'ExprId','items':[3]})
        self.assertEqual(parse('Span { file: SourceFileId(0), start: 1, end: 4 }')['file'],{'tag':'SourceFileId','items':[0]})
        with self.assertRaises(DebugSyntaxError):parse('Expr { span: 0, .. }')
        with self.assertRaises(DebugSyntaxError):parse('ExprId(1) trailing')
if __name__=='__main__':unittest.main()
