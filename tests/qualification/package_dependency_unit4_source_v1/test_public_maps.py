"""Focused pure public map controls. No compiler or preparation is called.

Run discovery with python -B; no cache members are exempted from inventories.
"""
import copy
import os
from pathlib import Path
import unittest
import sys
import types

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent


def load_local(name):
    path = HERE / name
    module = types.ModuleType('tested_public_' + path.stem)
    module.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), 'exec'), module.__dict__)
    return module


m = load_local('public_maps.py')
source = load_local('source.py')


class PublicMaps(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.repo = Path(os.environ.get('OXID_PACKAGE_UNIT4_TEST_REPOSITORY', Path(__file__).resolve().parents[3])).resolve()
        cls.context = source.capture(cls.repo)
        cls.retained = m.read_retained(cls.repo)
        cls.authority = m.derive(cls.context, cls.retained)

    def reject_context(self, context):
        with self.assertRaises((m.Rejected, ValueError)):
            m.derive(context, self.retained)

    def test_complete_maps_and_composition(self):
        a = self.authority
        self.assertEqual({k:len(v) for k,v in a['maps'].items()},
                         {'old_ordinary376':376, 'new_ordinary376':376,
                          'old_lifecycle377':377, 'new_lifecycle377':377})
        for name, records in a['maps'].items():
            self.assertEqual(m.sha(m.canonical(records)), a['canonical_map_sha256'][name])
        self.assertNotEqual(a['canonical_map_sha256']['old_ordinary376'], a['canonical_map_sha256']['new_ordinary376'])
        self.assertNotEqual(a['canonical_map_sha256']['old_lifecycle377'], a['canonical_map_sha256']['new_lifecycle377'])
        self.assertEqual(a['composition']['both_inverses_new377'], m.OLD_MAP_SHA)
        self.assertEqual(a['composition']['main_inverse_new377'], m.OLD_OBSERVER_SHA)
        self.assertEqual(a['status'], 'NotReady')
        self.assertIs(a['execution_qualified'], False)

    def test_published_authority_exact(self):
        raw = Path(__file__).with_name(m.AUTHORITY_NAME).read_bytes()
        self.assertEqual(m.validate_authority(raw, self.context, self.retained), self.authority)

    def test_current_changed_missing_extra(self):
        for kind in ('changed', 'missing', 'extra'):
            with self.subTest(kind=kind):
                c = copy.deepcopy(self.context)
                if kind == 'changed': c['current_inputs']['Cargo.toml'] += b'\n'
                elif kind == 'missing': del c['current_inputs']['Cargo.toml']
                else: c['current_inputs']['extra.rs'] = b''
                self.reject_context(c)

    def test_old_main_replay_and_wrong_stage(self):
        c = copy.deepcopy(self.context)
        c['current_inputs']['src/main.rs'] = c['predecessor_inputs']['src/main.rs']
        self.reject_context(c)

    def test_stale_inverse_predecessor(self):
        c = copy.deepcopy(self.context)
        c['predecessor_inputs']['src/main.rs'] += b'\n'
        self.reject_context(c)

    def test_transition_patch_mutation(self):
        c = copy.deepcopy(self.context)
        c['transition_patch'] += b'\n'
        self.reject_context(c)

    def test_retained_changed_missing_extra(self):
        for name in m.PINS:
            with self.subTest(name=name):
                r = dict(self.retained); r[name] += b'\n'
                with self.assertRaises(m.Rejected): m.derive(self.context, r)
        for kind in ('missing', 'extra'):
            r = dict(self.retained)
            if kind == 'missing': r.pop(next(iter(r)))
            else: r['extra.py'] = b''
            with self.assertRaises(m.Rejected): m.derive(self.context, r)

    def test_manifest_old_substitution(self):
        c = copy.deepcopy(self.context)
        c['current_manifest'] = c['predecessor_manifest']
        self.reject_context(c)

    def test_manifest_source_head_tree(self):
        for field in ('reviewed_source_head', 'source_only_tree', 'files'):
            with self.subTest(field=field):
                c = copy.deepcopy(self.context)
                j = m.decode(c['current_manifest'])
                j[field] = [] if field == 'files' else '0' * 40
                c['current_manifest'] = m.serialize(j)
                self.reject_context(c)
        for field in ('head', 'tree'):
            c = copy.deepcopy(self.context); c['source_checkpoint'][field] = '0' * 40
            self.reject_context(c)

    def test_map_row_duplicate_reorder_and_digest(self):
        for key in self.authority['maps']:
            for kind in ('duplicate', 'reorder', 'digest', 'old-substitution'):
                with self.subTest(key=key, kind=kind):
                    a = copy.deepcopy(self.authority)
                    if kind == 'duplicate': a['maps'][key].append(a['maps'][key][0])
                    elif kind == 'reorder': a['maps'][key][:2] = a['maps'][key][:2][::-1]
                    elif kind == 'digest': a['canonical_map_sha256'][key] = '0' * 64
                    else: a['maps'][key] = a['maps']['old_ordinary376'] if key != 'old_ordinary376' else a['maps']['new_ordinary376']
                    with self.assertRaises(m.Rejected): m.validate_authority(m.serialize(a), self.context, self.retained)

    def test_changed_roster_correspondence_or_patch(self):
        for field in ('inserted_roster', 'lexer_hook_correspondence', 'transition_patch'):
            a = copy.deepcopy(self.authority); a[field] = []
            with self.assertRaises(m.Rejected): m.validate_authority(m.serialize(a), self.context, self.retained)

    def test_exact_patch_controls(self):
        api = m.load(self.retained[m.PUBLIC + 'lexer_reservation_lifecycle.py'], m.PUBLIC + 'lexer_reservation_lifecycle.py')
        patch = self.retained[m.PUBLIC + 'observer-lexer-reservation-v2.patch']
        ordinary = self.context['current_inputs']
        observed = api.exact_patch(ordinary, patch)
        with self.assertRaises(api.Rejected): api.exact_patch(observed, patch)
        with self.assertRaises(api.Rejected): api.exact_patch(ordinary, patch, True)
        broken = dict(observed); broken['src/frontend/lifecycle_observer.rs'] += b'x'
        with self.assertRaises(api.Rejected): api.exact_patch(broken, patch, True)
        self.assertEqual(api.exact_patch(observed, patch, True), ordinary)

    def test_no_input_mutation(self):
        context = copy.deepcopy(self.context); retained = dict(self.retained)
        m.derive(context, retained)
        self.assertEqual(context, self.context)
        self.assertEqual(retained, self.retained)

    def test_not_ready(self):
        with self.assertRaises(m.NotReady): m.execution_context()


if __name__ == '__main__':
    unittest.main()
