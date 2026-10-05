"""Independent successor arithmetic; historical resource facts remain unchanged."""
import hashlib
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / 'tests/qualification/projected_slice_resource_v1'


def cells(c):
    return c['S'] + c['A'] + c['P'] + 4*c['O'] + 10*c['R'] + 14*c['L'] + 2*c['C']


def requested(c):
    return 8*(c['S'] + c['A']) + c['B'] + 32*c['O'] + 80*c['R'] + 112*c['L'] + 16*c['C']


class ProjectedResourceContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ledger = json.loads((PACKAGE / 'ledger.json').read_bytes())

    def test_exact_successor_artifacts_and_frozen_predecessor(self):
        manifest = json.loads((PACKAGE / 'manifest.json').read_bytes())
        self.assertEqual({p.name for p in PACKAGE.iterdir()},
                         {r['path'] for r in manifest['files']} | {'manifest.json'})
        for row in manifest['files']:
            raw = (PACKAGE / row['path']).read_bytes()
            self.assertEqual((len(raw), hashlib.sha256(raw).hexdigest()), (row['bytes'], row['sha256']))
        old = ROOT / self.ledger['historical_resource_package'] / 'artifact-manifest.json'
        self.assertEqual(hashlib.sha256(old.read_bytes()).hexdigest(), self.ledger['historical_package_sha256'])
        for row in json.loads(old.read_bytes())['files']:
            raw = (old.parent / row['path']).read_bytes()
            self.assertEqual((len(raw), hashlib.sha256(raw).hexdigest()), (row['bytes'], row['sha256']))

    def test_layout_and_logical_fuel_are_separate(self):
        x = self.ledger
        self.assertEqual(x['representation']['ReferenceHandle'], 4*8 + 4*8 + 16)
        self.assertEqual(x['representation']['LoanRuntime'], 2*8 + 4*8 + 4*8 + 16 + 2*8)
        for name, ty in [('reference', 'ReferenceHandle'), ('loan', 'LoanRuntime')]:
            self.assertEqual(x['physical_cells'][name]*8, x['representation'][ty])
            self.assertEqual(x['physical_cells'][name]-2, x['logical_activation_cells'][name])
        self.assertEqual(x['representation']['TypedBody'], 6*24)
        self.assertEqual(x['representation']['TypedBody_growth'], 24)
        self.assertEqual(x['representation']['FieldId'], 2*8)

    def test_runtime_exact_source_census_and_one_more(self):
        r = self.ledger['runtime_exact']
        activations = r['countdown'] + 1
        self.assertEqual(activations + 1, r['frames'])
        self.assertEqual(31+r['main_padding'] + activations*(37+r['recursive_padding']), r['cells'])
        self.assertEqual(31+r['over_main_padding'] + activations*(37+r['recursive_padding']), r['over_cells'])
        self.assertEqual(3+r['main_padding'] + activations*(9+r['recursive_padding']), r['scalar_slots'])
        self.assertEqual(50*activations+45+2*r['main_padding']+2*activations*r['recursive_padding'], r['fuel'])
        caps = self.ledger['unchanged_caps']
        self.assertEqual(r['cells'], caps['expanded_cells'])
        self.assertEqual(r['over_cells'], caps['expanded_cells']+1)
        for field, cap in [('scalar_slots','scalar_slots'),('frames','frames'),('fuel','fuel')]:
            self.assertLess(r[field], caps[cap])

    def test_native_exact_source_census_and_one_more(self):
        n = self.ledger['native_exact']
        main_s = n['main_padding']+3
        middle_s = n['middle_padding']+2
        leaf_s = n['leaf_padding']+1
        main_x = main_s+1+2+8+14+2
        middle_x = middle_s+1+10+14+2
        leaf_x = leaf_s+10
        self.assertEqual((main_x,middle_x,leaf_x), (256,256,256))
        self.assertEqual(main_x+n['middle_count']*middle_x+leaf_x, n['cells'])
        self.assertEqual(n['cells']+n['over_leaf_padding']-n['leaf_padding'], n['over_cells'])
        self.assertEqual(main_s+n['middle_count']*middle_s+leaf_s, n['scalar_slots'])
        self.assertEqual(8*(main_s+1)+8+8+n['middle_count']*(8*(middle_s+1)+16)+8*leaf_s+8, n['bytes'])
        self.assertEqual(n['cells'], self.ledger['unchanged_caps']['native_cells'])
        self.assertEqual(n['over_cells'], n['cells']+1)

    def test_owner_class_peak_moves_to_read_and_headers_reserve_capacity(self):
        o = self.ledger['owner_classes']
        self.assertEqual([cells(o[k]) for k in ('main','relay','read')], [42,10,11])
        self.assertEqual([requested(o[k]) for k in ('main','relay','read')], [320,72,88])
        self.assertEqual(cells(o['main'])+cells(o['read']), o['peak_cells'])
        self.assertEqual(requested(o['main'])+requested(o['read']), o['peak_bytes'])
        for n, key in [(2,'two_headers_bytes'), (3,'three_headers_bytes')]:
            self.assertEqual(n*272+8+o['peak_bytes'], o[key])
        self.assertEqual(3*272+8+self.ledger['batch']['active_bytes'], self.ledger['batch']['requested_bytes'])

    def test_production_byte_caps_remain_masked(self):
        caps = self.ledger['unchanged_caps']
        self.assertLess(caps['frames']*272+8+8*caps['expanded_cells'], caps['requested_bytes'])
        self.assertLess(8*caps['native_cells']+8, caps['native_bytes'])
        self.assertEqual(caps['fuel'], 1_000_000)
        self.assertEqual(caps['expanded_cells'], 200_000)
        self.assertEqual(caps['native_cells'], 8192)


if __name__ == '__main__':
    unittest.main()
