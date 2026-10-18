"""Inactive composition controls; no capacity or native qualification probes."""
import copy
import builtins
import io
from unittest.mock import patch
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import types
import unittest
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
def load(path):
    m=types.ModuleType('activation_'+path.stem);m.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),m.__dict__)
    return m
f=load(HERE/'current_features.py')
v=load(HERE/'successor.py')

class Activation(unittest.TestCase):
    def test_exact_adapter_derivation_and_closed_routes(self):
        d=load(HERE/'derive.py');before=(REPO/'tests/qualification/wide_span_parser_ci_v1/adapters.py').read_bytes()
        after=d.derive(before)
        self.assertEqual(after,(HERE/'adapters.py').read_bytes())
        for name in ('wide_span_fixture_storage_v1/predecessor.py','wide_span_fixture_storage_v1/lifecycle.py','wide_span_fixture_storage_v1/hosted_controls.py'):
            self.assertNotIn(name.encode(),after)
        with self.assertRaises(ValueError):d.derive(before+b'\n')

    def test_fixed_descriptor_rejects_untrusted_rehash_and_each_file_change(self):
        manifest=json.loads((REPO/'tests/qualification/unit4_ci/inputs.json').read_bytes())
        case=manifest['feature_composition'];rows=f.verify(REPO,case)
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            names=[r['path'] for r in rows]+[f.ROOT+'current-feature-files-v1.json','tests/qualification/package_json_current_v1/generation.py']
            for n in names:
                p=root/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((REPO/n).read_bytes())
            self.assertEqual(f.verify(root,case),rows)
            for row in rows:
                p=root/row['path'];body=p.read_bytes()
                for mutation in ('changed','missing','symlink','hardlink','executable'):
                    p.unlink();other=root/'other';other.write_bytes(body)
                    if mutation=='changed':p.write_bytes(body+b'\n')
                    elif mutation=='symlink':p.symlink_to(other)
                    elif mutation=='hardlink':os.link(other,p)
                    elif mutation=='executable':p.write_bytes(body);p.chmod(0o755)
                    with self.subTest(path=row['path'],mutation=mutation),self.assertRaises((ValueError,OSError)):f.verify(root,case)
                    if p.exists() or p.is_symlink():p.unlink()
                    other.unlink();p.write_bytes(body)
            descriptor=root/f.ROOT/'current-feature-files-v1.json';raw=descriptor.read_bytes()
            original=json.loads(raw)
            for mutation in ('omit','duplicate','bool','float','relabel','rehash'):
                value=copy.deepcopy(original);values=value['compositions'][case]
                if mutation=='omit':values.pop()
                elif mutation=='duplicate':values.append(values[0])
                elif mutation=='bool':values[0]['bytes']=True
                elif mutation=='float':values[0]['bytes']=float(values[0]['bytes'])
                elif mutation=='relabel':values[0]['path']='other'
                else:values[0]['sha256']='0'*64
                descriptor.write_text(json.dumps(value))
                with self.subTest(mutation=mutation),self.assertRaises(ValueError):f.verify(root,case)
            descriptor.write_bytes(raw)

    def test_every_overlay_archive_is_exact_and_changed_body_rejects(self):
        rows=json.loads((HERE/'originals.json').read_bytes())['files']
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            for name in ('tests/qualification/stdlib_trim_storage_v1/retained.py','tests/qualification/package_json_current_v1/generation.py',v.ROOT+'originals.json'):
                target=root/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((REPO/name).read_bytes())
            for row in rows:
                body=v.read_original(row['path'],REPO)
                target=root/row['storage'];target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(body)
                self.assertEqual(v.read_original(row['path'],root),body)
                target.write_bytes(body[:-1]+bytes([body[-1]^1]))
                with self.subTest(path=row['path']),self.assertRaises(ValueError):v.read_original(row['path'],root)
                target.unlink()

    def test_current_inventory_and_all_19_outputs(self):
        v.verify_inventory(REPO)
        self.assertEqual(len(v.OUTPUTS),19)
        q=load(REPO/'tests/qualification/unit4_ci/common.py')
        self.assertEqual((q.CURRENT_COMPILER_BODIES,q.CURRENT_SOURCE_MEMBERS,q.CURRENT_SHA),(291,376,'d33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0'))
        manifest=q.read(REPO/'tests/qualification/unit4_ci/inputs.json')
        for name in ('stdlib/numbers.ox','tests/stdlib_numbers.rs'):
            if any(r['path']==name for r in manifest['files']):
                bad=copy.deepcopy(manifest);bad['files']=[r for r in bad['files'] if r['path']!=name]
                with self.assertRaises(q.Reject):q.verify_package(REPO,bad)

    def test_actual_dispatcher_and_complete_historical_view(self):
        runtime=load(REPO/'tests/fixtures/typed_project_source_binding/run.py')
        if sys.flags.optimize:
            with self.assertRaisesRegex((ValueError,RuntimeError),'optimized Python'):runtime.preflight(REPO)
            return
        captured=runtime.preflight(REPO)
        self.assertEqual(len(captured['inputs']),376)
        p=runtime._modules['predecessor'];g=runtime._modules['generation']
        self.assertEqual(Path(p.__file__),REPO/v.CANDIDATE)
        bodies=p.capture(REPO,captured['package_json_predecessor']['inputs'],runtime._SOURCE_SEAL['predecessor_view'])
        self.assertEqual((len(bodies),sum(map(len,bodies.values()))),(2724,78507317))
        self.assertEqual(g.sha(g.encoded([g.identity(n,b) for n,b in sorted(bodies.items())])),'e70b8b790756e0fb9e0cd1341779c51646b964afcf9e2eade2bd9480716b1882')
        self.assertNotIn('tests/stdlib_numbers.rs',bodies)
        for name in p._FEATURE_ROWS:self.assertEqual(bodies[name],p._feature_archive(REPO,p._FEATURE_ROWS[name]))

    def test_fixed_producer_inputs_refuse_code_substitution_and_extra_members(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp)
            for package in ('stdlib_feature_storage_v1','stdlib_feature_storage_activation_v3'):
                shutil.copytree(REPO/'tests/qualification'/package,root/'tests/qualification'/package)
            for name in ('tests/qualification/package_json_current_v1/generation.py','tests/qualification/stdlib_trim_storage_v1/retained.py','tests/qualification/wide_span_parser_ci_v1/adapters.py'):
                p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((REPO/name).read_bytes())
            _,captured=v.producer_inputs(root)
            for name,pin in v.PRODUCER_INPUTS:
                p=root/name;original=p.read_bytes()
                p.write_bytes(original+b'\n')
                with self.subTest(name=name),self.assertRaisesRegex(ValueError,'fixed producer input|unsafe input mode/size'):v.producer_inputs(root)
                p.write_bytes(original)
            p=root/v.ROOT/'current_features.py';original=p.read_bytes()
            p.write_bytes(original+b'\ndef verify(repo, composition): return []\n')
            with self.assertRaisesRegex(ValueError,'fixed producer input|unsafe input mode/size'):v.generate(root,'combined')
            p.write_bytes(original)
            for extra in (root/v.ROOT/'unexpected.py',root/'tests/qualification/stdlib_feature_storage_v1/unexpected.py'):
                extra.write_bytes(b'pass\n')
                with self.assertRaisesRegex(ValueError,'closed producer source roster'):v.producer_inputs(root)
                extra.unlink()
            (root/v.ROOT/'empty-extra-directory').mkdir()
            with self.assertRaisesRegex(ValueError,'unlisted producer directory'):v.producer_inputs(root)

    def test_independent_composition_roster_rejects_empty_verifier_result(self):
        original=v.local
        def changed(path,raw):
            module=original(path,raw)
            if path==HERE/'current_features.py':module.verify=lambda repo,composition: []
            return module
        case=json.loads((REPO/'tests/qualification/unit4_ci/inputs.json').read_bytes())['feature_composition']
        with patch.object(v,'local',changed),self.assertRaisesRegex(ValueError,'fixed independent feature roster'):v.generate(REPO,case)

    def test_current_feature_rows_are_fresh_and_not_shared(self):
        case=json.loads((REPO/'tests/qualification/unit4_ci/inputs.json').read_bytes())['feature_composition']
        first=f.verify(REPO,case);expected=copy.deepcopy(first)
        first[0]['path']='changed';first.clear()
        self.assertEqual(f.verify(REPO,case),expected)

    def test_current_routes_trap_old_helpers_and_cleanup_after_failure(self):
        forbidden={str(REPO/'tests/qualification/wide_span_fixture_storage_v1'/n) for n in ('predecessor.py','lifecycle.py','hosted_controls.py')}
        original_compile=builtins.compile;hits=[]
        def trapped(source,filename,*args,**kwargs):
            if str(filename) in forbidden:
                hits.append(str(filename));raise RuntimeError('old current helper execution trap')
            return original_compile(source,filename,*args,**kwargs)
        # Positive control and exceptional exit both prove the interception and
        # context-manager cleanup; historical temp-view paths are not forbidden.
        with self.assertRaisesRegex(RuntimeError,'old current helper execution trap'):
            with patch.object(builtins,'compile',trapped):compile('pass',next(iter(forbidden)),'exec')
        self.assertIs(builtins.compile,original_compile);self.assertEqual(len(hits),1);hits.clear()
        aliases={p.stem for p in (REPO/'tests/qualification/unit4_public_v3').glob('*.py')}
        saved={n:sys.modules[n] for n in aliases if n in sys.modules};search=list(sys.path)
        try:
            for n in aliases:sys.modules.pop(n,None)
            sys.path.insert(0,str(REPO/'tests/qualification/unit4_public_v3'))
            with patch.object(builtins,'compile',trapped):
                runtime=load(REPO/'tests/fixtures/typed_project_source_binding/run.py')
                if sys.flags.optimize:
                    with self.assertRaisesRegex((ValueError,RuntimeError),'optimized Python'):runtime.preflight(REPO)
                else:
                    captured=runtime.preflight(REPO)
                    old_view=runtime._modules['predecessor'];root=None
                    with self.assertRaisesRegex(RuntimeError,'injected view failure'):
                        with old_view.view(REPO,captured['package_json_predecessor']['inputs'],runtime._SOURCE_SEAL['predecessor_view']) as root:
                            self.assertTrue(root.exists());raise RuntimeError('injected view failure')
                    self.assertFalse(root.exists())
                    with old_view.view(REPO,captured['package_json_predecessor']['inputs'],runtime._SOURCE_SEAL['predecessor_view']) as fresh:
                        self.assertTrue(fresh.exists())
                    self.assertFalse(fresh.exists())
                    parser=load(REPO/'tests/qualification/unit4_parser_current/portable.py');self.assertIsInstance(parser.authority(),dict)
                    life=load(HERE/'lifecycle.py');observer,proof=life.admit(REPO,(REPO/'tests/qualification/unit4_public_v3/observer-lexer-reservation-v2.patch').read_bytes())
                    self.assertEqual(len(proof['base_files']),376)
                    facade=load(REPO/'scripts/test_published_formatter_source.py')
                    self.assertGreater(facade.load_tests(unittest.defaultTestLoader,unittest.TestSuite(),None).countTestCases(),0)
                    hosted=load(HERE/'hosted_controls.py');hosted.bootstrap_successor(REPO)
                    adapters=load(HERE/'adapters.py');stream=io.StringIO()
                    result=unittest.TextTestRunner(stream=stream,verbosity=2).run(adapters.suite(REPO))
                    self.assertEqual(result.testsRun,26);self.assertTrue(result.wasSuccessful(),stream.getvalue())
        finally:
            sys.path[:]=search
            for n in aliases:sys.modules.pop(n,None)
            sys.modules.update(saved)
        self.assertIs(builtins.compile,original_compile);self.assertEqual(hits,[])

if __name__=='__main__':unittest.main(verbosity=2)
