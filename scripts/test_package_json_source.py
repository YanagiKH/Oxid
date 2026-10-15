"""Focused current-source and immutable v1 refusal controls; no compiler run."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest

sys.dont_write_bytecode = True
REPO = Path(__file__).absolute().parents[1]
HERE = REPO / 'tests/qualification/package_json_current_v1'


def load(path, name):
    module = types.ModuleType(name);module.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), 'exec'), module.__dict__)
    return module


def runtime():
    return load(REPO / 'tests/fixtures/typed_project_source_binding/run.py', 'package_json_test_runtime')


class RuntimeControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.runtime = runtime()
        cls.captured = cls.runtime.preflight(REPO)

    def test_complete_predecessor_capture_and_new_accounting(self):
        c = self.captured
        self.assertEqual(len(c['inputs']), 376)
        self.assertEqual(len(c['package_json_predecessor']['archived']), 117)
        self.assertEqual(c['package_json_predecessor']['current']['reviewed_source_head'],
                         'fbff527c895e6cd4ec9350bd8b5d460bc20bb80d')
        self.assertEqual(c['current']['reviewed_source_head'],'858c70d862587b6a8b5d59a70afe4b8334b1f150')
        self.assertEqual(c['published_source_unit2_accounting_source_binding'],
                         c['package_json_predecessor']['published_source_unit2_accounting_source_binding'])

    def test_genuine_prepared_unit2_and_materialized_data_tamper(self):
        with tempfile.TemporaryDirectory(prefix='oxid-package-json-unit2-') as directory:
            output = Path(directory)
            seam = self.runtime.prepare_unit2(output, self.captured)
            command = [sys.executable, '-B', str(output/'compatibility/typed_project_unit3_compatibility/run.py'),
                       '--repo', str(REPO), '--source-manifest',
                       str(REPO/'tests/fixtures/typed_project_source_binding/current-source.json'),
                       '--output', str(output/'unit2'), '--cargo', 'cargo', '--rustc', 'rustc', '--prepare-only']
            result = subprocess.run(command, cwd=REPO, capture_output=True, timeout=180)
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors='replace'))
            verified = self.runtime.verify_unit2_result(output, self.captured, seam, True)
            self.assertFalse(verified['semantic_pass'])
            self.assertEqual(verified['compiler_executions'], 0)
            target = output/'unit2/run/source/src/runtime/data.rs'
            self.assertEqual(target.read_bytes(), self.captured['inputs']['src/runtime/data.rs'])
            target.write_bytes(target.read_bytes()+b'\n// materialized negative\n')
            with self.assertRaises((ValueError,self.runtime.BindingError)):
                self.runtime.verify_unit2_result(output,self.captured,seam,True)
            # Coherently rehashing the assembly map must not hide a changed
            # ordinary data source from the new post-verifier checkpoint check.
            assembly_path = output/'unit2/run/assembly.json'
            assembly = json.loads(assembly_path.read_bytes())
            g = self.runtime._modules['generation']
            assembly['files'] = [g.identity(r['path'],target.read_bytes())
                                 if r['path']=='src/runtime/data.rs' else r for r in assembly['files']]
            assembly_path.write_bytes(g.encoded(assembly))
            with self.assertRaisesRegex(ValueError,'wrong materialized current source: src/runtime/data.rs'):
                self.runtime.verify_unit2_result(output,self.captured,seam,True)


def load_tests(loader, tests, pattern):
    current = load(HERE/'test_generation.py','package_json_generation_controls')
    suite = unittest.TestSuite([loader.loadTestsFromModule(current),loader.loadTestsFromTestCase(RuntimeControls)])
    hosted = load(HERE/'test_hosted.py','package_json_hosted_bridge_controls')
    suite.addTests(loader.loadTestsFromModule(hosted))
    r = runtime(); modules = r._modules
    old = load(REPO/'tests/qualification/published_formatter_current/test_published_transition.py',
               'package_json_preserved_v1_controls')
    suite.addTests(old.build_unit_suite(modules['published_transition'],modules['published_current'],modules['published_view'],
        retained_source_manifest_bytes=(REPO/'tests/fixtures/typed_project_source_binding/lexer-reservation-source-v2.json').read_bytes(),
        retained_package_manifest_bytes=(REPO/'tests/qualification/published_formatter_current/predecessor-v1/tests/fixtures/typed_project_source_binding/package-manifest.json').read_bytes(),
        retained_runner_bytes=(REPO/'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py').read_bytes()))
    return suite


if __name__ == '__main__':
    unittest.main()
