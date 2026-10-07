"""Keep exact external observer migrations checked by ordinary Python CI."""
import hashlib
import importlib.util
import json
from contextlib import contextmanager
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
PACKAGE = REPO / 'tests/qualification/record_composition_current'
MANIFEST_SHA = '31bf4b08efe03b52095bbc854d655746747e307c4ef7806ad2937d656c9eb302'


def source_binding():
    spec = importlib.util.spec_from_file_location(
        'record_observer_source_binding', REPO / 'tests/fixtures/typed_project_source_binding/run.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@contextmanager
def admitted_composition_view():
    """Validate current stdout inputs, then expose the exact sealed predecessor.

    This is an archived adapter control, not a current stdout runtime observation. The
    binding checks the complete current source roster and reversible transition.
    Admission deliberately happens in normal Python, before any -O child.
    """
    binding = source_binding()
    captured = binding.preflight(REPO)
    inputs = dict(captured['composition_inputs'])
    identities = json.loads((PACKAGE / 'observer-adapter-identities.json').read_bytes())['adapters']
    for identity in identities.values():
        for kind in ('original', 'archived_container'):
            if kind == 'original' and 'archived_container' in identity:
                continue  # These original modules are extracted from the archive.
            if kind in identity:
                path = identity[kind]['path']
                if path not in inputs:
                    inputs[path] = (REPO / path).read_bytes()
    with tempfile.TemporaryDirectory() as directory:
        view = Path(directory) / 'sealed-composition-predecessor'
        binding.materialize(view, inputs)
        try:
            yield view, captured
        finally:
            binding.assert_unchanged(REPO, captured)


def frozen_controls_argv(script, view, optimized=False):
    # The frozen module and assertions remain byte-for-byte unchanged. Only its
    # repository input root is rebound; package-local seals still use __file__.
    runner = (
        'import importlib.util,sys,unittest;from pathlib import Path;'
        'sys.path.insert(0,str(Path(sys.argv[1]).parent));'
        "s=importlib.util.spec_from_file_location('frozen_controls',sys.argv[1]);"
        'm=importlib.util.module_from_spec(s);s.loader.exec_module(m);'
        'm.REPO=Path(sys.argv[2]);'
        'r=unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromModule(m));'
        'sys.exit(not r.wasSuccessful())'
    )
    return [sys.executable, '-B', *(['-O'] if optimized else []),
            '-c', runner, str(script), str(view)]


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
        with admitted_composition_view() as (view, captured):
            self.assertEqual(len(captured['inputs']), 262)
            self.assertEqual(len(captured['enum_inputs']), 237)
            self.assertEqual(len(captured['projected_inputs']), 201)
            self.assertEqual(len(captured['composition_inputs']), 196)
            for optimized in (False, True):
                with self.subTest(optimized=optimized):
                    argv = frozen_controls_argv(PACKAGE / 'test_observer_adapters.py', view, optimized)
                    result = subprocess.run(argv, capture_output=True, text=True, timeout=30)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertIn('Ran 13 tests', result.stderr)
                    self.assertIn('OK', result.stderr)

    def test_predecessor_view_is_explicit_and_current_stdout_inputs_remain_bound(self):
        with admitted_composition_view() as (view, captured):
            path = 'src/frontend/oir/owned/execute.rs'
            self.assertEqual((view / path).read_bytes(), captured['composition_inputs'][path])
            self.assertEqual((REPO / path).read_bytes(), captured['inputs'][path])
            self.assertNotEqual(captured['inputs'][path], captured['composition_inputs'][path])
            identities = json.loads((PACKAGE / 'observer-adapter-identities.json').read_bytes())['adapters']
            self.assertEqual(hashlib.sha256((view / path).read_bytes()).hexdigest(),
                             identities['store']['original']['sha256'])
            # A changed current source is rejected before a historical view can
            # be admitted; the binding's own mutation suite covers every input.
            binding = source_binding()
            changed = dict(captured['inputs']); changed[path] += b'\n'
            with self.assertRaises(binding.BindingError):
                binding.check_bytes(changed, captured['current']['files'])


if __name__ == '__main__':
    unittest.main()
