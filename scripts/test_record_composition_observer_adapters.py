"""Keep exact external observer migrations checked by ordinary Python CI."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import unittest

PACKAGE = Path(__file__).resolve().parents[1] / 'tests/qualification/record_composition_current'
MANIFEST_SHA = '31bf4b08efe03b52095bbc854d655746747e307c4ef7806ad2937d656c9eb302'


class RecordObserverPackageControls(unittest.TestCase):
    def test_exact_package_members_and_hashes(self):
        raw = (PACKAGE / 'package-manifest.json').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), MANIFEST_SHA)
        rows = json.loads(raw)['files']
        self.assertEqual({p.name for p in PACKAGE.iterdir()},
                         {r['path'] for r in rows} | {'package-manifest.json'})
        for row in rows:
            raw = (PACKAGE / row['path']).read_bytes()
            self.assertEqual(len(raw), row['bytes'], row['path'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), row['sha256'], row['path'])

    def test_normal_and_optimized_reversal_and_scope_controls(self):
        for optimized in (False, True):
            with self.subTest(optimized=optimized):
                argv = [sys.executable, '-B'] + (['-O'] if optimized else [])
                result = subprocess.run([*argv, str(PACKAGE / 'test_observer_adapters.py')],
                                        capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn('Ran 13 tests', result.stderr)
                self.assertIn('OK', result.stderr)


if __name__ == '__main__':
    unittest.main()
