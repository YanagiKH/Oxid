"""Active trim output, source admission and generation-separation controls."""
import importlib.util
import json
from pathlib import Path
import sys
import types
import unittest
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
sys.path.insert(0,str(HERE))
import activate
import retained as r


def load(name,path):
    module=types.ModuleType(name);module.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module


class ActiveControls(unittest.TestCase):
    def test_two_generations_and_complete_output_map(self):
        first=activate.generate();second=activate.generate()
        self.assertEqual(first,second)
        for name,body in first.items():self.assertEqual((REPO/name).read_bytes(),body,name)
        f=r.Fixture(REPO)
        self.assertNotEqual(f.read(r.S+'run.py'),(REPO/r.S/'run.py').read_bytes())
        self.assertNotEqual(f.read(r.U+'inputs.json'),(REPO/r.U/'inputs.json').read_bytes())
        f.assert_scheduling_outputs()

    def test_actual_dispatcher_and_current_hosted_bootstrap(self):
        runtime=load('trim_current_dispatcher',REPO/r.S/'run.py')
        capture=runtime.preflight(REPO)
        self.assertEqual(len(capture['inputs']),376)
        self.assertEqual(capture['inputs'],runtime._modules['package_current'].admit(REPO,runtime._SOURCE_SEAL)['inputs'])
        helper=runtime._modules['predecessor']
        bodies=helper.capture(REPO,capture['package_json_predecessor']['inputs'],runtime._SOURCE_SEAL['predecessor_view'])
        self.assertEqual(len(bodies),2724)
        self.assertEqual(bodies['stdlib/strings.ox'],(HERE/'archive/stdlib/strings.ox.data').read_bytes())
        hosted=load('trim_active_hosted',HERE/'hosted_controls.py')
        _,fresh=hosted.bootstrap_successor(REPO)
        self.assertEqual(fresh['inputs'],capture['inputs'])
        self.assertEqual(hosted.SourceControls().countTestCases(),92)
        f=r.Fixture(REPO)
        previous=hosted._SUCCESSOR['dispatcher']
        old=f.read(r.S+'run.py')
        hosted._SUCCESSOR['dispatcher']={'path':r.S+'run.py','bytes':len(old),'sha256':r.sha(old)}
        try:
            with self.assertRaisesRegex(ValueError,'successor dispatcher'):
                hosted.bootstrap_successor(REPO)
        finally:
            hosted._SUCCESSOR['dispatcher']=previous

    def test_live_inventory_contains_feature_and_archive_closure(self):
        sys.path.insert(0,str(REPO/r.U))
        q=load('trim_inventory',REPO/r.U/'common.py')
        manifest=q.read(q.HERE/'inputs.json');q.verify_package(REPO,manifest)
        names={x['path'] for x in manifest['files']}
        for name in ('stdlib/strings.ox','tests/stdlib_strings.rs',r.ROOT+'predecessor.py',r.ROOT+'archive/stdlib/strings.ox.data',r.ROOT+'retained.py'):
            self.assertIn(name,names)
        with self.assertRaises(q.Reject):
            q.verify_package(REPO,{**manifest,'files':[x for x in manifest['files'] if x['path']!=r.ROOT+'retained.py']})
        # Archives are data; live and unregistered neighboring .ox inputs must
        # still reach the unchanged verifier's language plan.
        sys.path.insert(0,str(REPO/'scripts'))
        verifier=load('trim_current_repo_verifier',REPO/'scripts/verify_repo.py')
        archive=HERE/'archive/stdlib/strings.ox.data'
        live=REPO/'stdlib/strings.ox'
        sources=sorted(path for path in REPO.rglob('*.ox')
                       if '.oxid' not in path.parts and 'target' not in path.parts)
        self.assertNotIn(archive,sources)
        self.assertIn(live,sources)
        neighbor=HERE/'archive/stdlib/unregistered.ox'
        self.assertFalse(neighbor.exists())
        checks,_,_=verifier.source_plan(sources+[neighbor],REPO)
        self.assertIn((live,False),checks)
        self.assertIn((neighbor,False),checks)
        self.assertFalse(any(path==archive for path,_ in checks))
        self.assertNotIn(archive,verifier.runnable_sources(REPO))
        f=r.Fixture(REPO)
        historical=json.loads(f.read(r.U+'inputs.json'))
        with self.assertRaises(q.Reject):q.verify_package(REPO,historical)


if __name__=='__main__':unittest.main()
