"""Current runtime-view and transfer-hook package admission controls."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / 'tests/qualification/record_composition_current_runtime_view_v1'
MANIFEST_SHA = 'cdc4bd11777e10e0afea8456a1b71df456d44a82d45e86a4d1d5c7959b10125b'
spec = importlib.util.spec_from_file_location(
    'record_observer_controls', ROOT / 'scripts/test_record_composition_observer_adapters.py')
observer_controls = importlib.util.module_from_spec(spec)
spec.loader.exec_module(observer_controls)


class RuntimeViewPackageControls(unittest.TestCase):
    def test_exact_closed_package(self):
        raw = (PACKAGE / 'package-manifest.json').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), MANIFEST_SHA)
        rows = json.loads(raw)['files']
        self.assertEqual({p.name for p in PACKAGE.iterdir()}, {r['path'] for r in rows} | {'package-manifest.json'})
        for row in rows:
            raw = (PACKAGE / row['path']).read_bytes()
            self.assertEqual((len(raw), hashlib.sha256(raw).hexdigest()), (row['bytes'], row['sha256']))

    def test_runtime_view_and_transfer_hook_under_normal_and_optimized_python(self):
        # These are sealed predecessor adapter controls, not current unary
        # runtime observations. Current source is separately admitted in full.
        with observer_controls.admitted_composition_view() as (view, _):
            for script, count in (('test_runtime_view.py', 15), ('test_transfer_hook.py', 4)):
                for optimized in (False, True):
                    with self.subTest(script=script, optimized=optimized, source_view='composition-predecessor'):
                        args = observer_controls.frozen_controls_argv(PACKAGE / script, view, optimized)
                        result = subprocess.run(args, capture_output=True, text=True, timeout=30)
                        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                        self.assertIn('Ran ' + str(count) + ' tests', result.stderr)

    def test_exact_seven_case_parser_amendment_controls(self):
        script = ROOT / 'tests/qualification/unit4_parser_current/test_record_composition_amendment.py'
        result = subprocess.run([sys.executable, '-B', str(script)], capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('Ran 7 tests', result.stderr)


if __name__ == '__main__':
    unittest.main()
