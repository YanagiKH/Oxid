"""Synthetic protocol controls only; no compiler/corpus expectations or execution."""
import copy,hashlib,json,unittest,os,pathlib,subprocess,sys,tempfile
from parse_debug import parse,DebugSyntaxError
from run import normalize,unique
class AdapterShape(unittest.TestCase):
    def setUp(self):
        self.case={'id':'shape-only','source':{'sha256':hashlib.sha256(b'').hexdigest(),'bytes':0,'path':'shape.ox'},'expected':{'relation_to_original':[]}}
        self.receipt={'candidate_source_manifest_sha256':'a'*64,'observer_source_sha256':'b'*64,'binary':{'sha256':'c'*64},'profile':'debug','target':'x86_64-unknown-linux-gnu'}
        row={'mode':'ProjectCandidate','mode_execution_index':0,'source_generation':7,'runtime_os':'linux','runtime_architecture':'x86_64','pointer_width':64,'executed':True,'diagnostics':[],'json_diagnostic_renderings':[],'human_diagnostic_rendering':'','ast':None}
        second=copy.deepcopy(row);second.update(mode='OwnedCandidate',mode_execution_index=1)
        self.raw={'case_id':self.case['id'],'nonce':'nonce','schema':'oxid-unit4-parser-raw-v1','source_utf8':'','display_path':'shape.ox','observations':[row,second]}
    def check_failure(self,fn):
        fn(self.raw)
        with self.assertRaises((AssertionError,KeyError)):normalize(self.raw,self.case,self.receipt,'nonce')
    def test_missing(self):self.check_failure(lambda r:r['observations'].pop())
    def test_extra(self):self.check_failure(lambda r:r['observations'].append(copy.deepcopy(r['observations'][0])))
    def test_duplicate(self):self.check_failure(lambda r:r['observations'].__setitem__(1,copy.deepcopy(r['observations'][0])))
    def test_unexecuted(self):self.check_failure(lambda r:r['observations'][0].update(executed=False))
    def test_zero(self):self.check_failure(lambda r:r.update(observations=[]))
    def test_wrong_nonce(self):self.check_failure(lambda r:r.update(nonce='other'))
    def test_wrong_source(self):self.check_failure(lambda r:r.update(source_utf8='different'))
    def test_wrong_generation(self):self.check_failure(lambda r:r['observations'][1].update(source_generation=8))
    def test_duplicate_json_key(self):
        with self.assertRaises(AssertionError):json.loads('{"a":1,"a":2}',object_pairs_hook=unique)
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
            pathlib.Path(os.environ['UNIT4_OPT_CONTROL_REPORT']).write_text(json.dumps({'schema':'oxid-unit4-optimized-python-controls-v1','status':'pass','controls':rows},sort_keys=True,indent=2)+'\n')
    def test_debug_complete(self):
        self.assertEqual(parse('ExprId(3)'),{'tag':'ExprId','items':[3]})
        self.assertEqual(parse('Span { file: SourceFileId(0), start: 1, end: 4 }')['file'],{'tag':'SourceFileId','items':[0]})
        with self.assertRaises(DebugSyntaxError):parse('Expr { span: 0, .. }')
        with self.assertRaises(DebugSyntaxError):parse('ExprId(1) trailing')
if __name__=='__main__':unittest.main()
