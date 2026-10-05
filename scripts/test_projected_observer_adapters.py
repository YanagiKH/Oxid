"""Current observer derivation and empty-path controls; no runtime-pass claim."""
import copy
import importlib.util
from pathlib import Path
import unittest
import sys

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / 'tests/qualification/projected_slice_observer_v1'

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

adapter = load('projected_adapter', PACKAGE / 'adapter.py')
view = load('projected_view', PACKAGE / 'runtime_view.py')
sys.modules['runtime_view'] = view.prior
controls = load('previous_view_controls', ROOT / 'tests/qualification/record_composition_current_runtime_view_v1/test_runtime_view.py')


class ProjectedObserverControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.captured = adapter.binding.preflight(ROOT)
        cls.originals = {p: cls.captured['inputs'].get(p, b'') for p in adapter.IDENTITY['paths']}

    def test_exact_current_source_derivation_round_trips(self):
        observed = adapter.derive(self.originals)
        self.assertEqual(adapter.reverse(observed), self.originals)
        self.assertEqual(len(observed), 19)
        self.assertIn(b'activation_fuel_cells()', observed['src/frontend/oir/owned/execute.rs'])
        self.assertIn(b'projection: Vec::new()', observed['src/frontend/oir/owned/unit3_raw_owned.rs'])

    def test_every_input_mutation_missing_and_extra_rejected(self):
        for p in self.originals:
            with self.subTest(path=p):
                changed = dict(self.originals);changed[p] += b'\n'
                with self.assertRaises(ValueError):adapter.derive(changed)
                del changed[p]
                with self.assertRaises(ValueError):adapter.derive(changed)
        changed = dict(self.originals);changed['unexpected.rs'] = b''
        with self.assertRaises(ValueError):adapter.derive(changed)

    def test_every_derived_mutation_and_double_adaptation_rejected(self):
        observed = adapter.derive(self.originals)
        for p in observed:
            with self.subTest(path=p):
                changed = dict(observed);changed[p] += b'\n'
                with self.assertRaises(ValueError):adapter.reverse(changed)
        with self.assertRaises(ValueError):adapter.derive(observed)

    def test_empty_path_runtime_view_round_trip(self):
        original = controls.observation()
        for root in ('raw', 'raw_before_audit'):
            if root in original:
                for f in original[root]['functions']:
                    for loan in f['loans']:loan['projection'] = []
        raw = view.prior.canonical(original)
        projected, receipt = view.project_bytes(raw)
        self.assertEqual(view.reverse_bytes(projected), raw)
        self.assertFalse(receipt['semantics_normalized'])
        self.assertTrue(receipt['empty_path_sites'])
        for name in set(original)-{'raw','raw_before_audit'}:
            self.assertEqual(view.prior.decode(projected)[name], original[name])

    def test_nonempty_missing_and_malformed_paths_fail_closed(self):
        for path in [None, False, {}, [0], [{'record':0,'index':0}]]:
            original = controls.observation()
            for f in original['raw']['functions']:
                for loan in f['loans']:loan['projection'] = path
            with self.subTest(path=path), self.assertRaises(ValueError):
                view.project_bytes(view.prior.canonical(original))
        with self.assertRaises(ValueError):view.project_bytes(view.prior.canonical(controls.observation()))


if __name__ == '__main__':
    unittest.main()
