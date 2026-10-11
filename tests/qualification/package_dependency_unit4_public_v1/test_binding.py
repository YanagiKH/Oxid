"""Pure fail-closed controls; no compiler, target, or runtime admission."""
import copy
import types
import unittest
from pathlib import Path

path = Path(__file__).absolute().with_name('binding.py')
binding = types.ModuleType('public_binding_under_test')
binding.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), 'exec'), binding.__dict__)


class BindingControls(unittest.TestCase):
    def test_retained_and_successor_identities_are_distinct(self):
        self.assertNotEqual(binding.CURRENT_SOURCE_SHA, binding.PREDECESSOR_SOURCE_SHA)
        self.assertEqual(set(binding.ROLE_MAPS), {'ordinary', 'lifecycle'})
        self.assertEqual([binding.ROLE_MAPS[r][1] for r in ('ordinary', 'lifecycle')], [376, 377])
        self.assertEqual(len(set(binding.KINDS.values())), 2)
        self.assertTrue(all(not k.startswith('unit4-public-v3-') for k in binding.KINDS.values()))

    def test_duplicate_keys_and_nonfinite_values_reject(self):
        for raw in (b'{"role": "ordinary", "role": "lifecycle"}', b'{"x": NaN}', b'{"x": Infinity}'):
            with self.subTest(raw=raw), self.assertRaises(binding.Rejected):
                binding.decode(raw)

    def test_unknown_roles_reject_before_context_access(self):
        for role in ('observer', 'parser_observed', 'unit4-public-v3-candidate', '', None):
            with self.subTest(role=role), self.assertRaises(binding.Rejected):
                binding.source_binding(object(), role)

    def test_all_unimplemented_admissions_reject_even_with_readiness_claim(self):
        claims = ({}, {'execution_ready': True}, {'execution_qualified': True},
                  {'source_manifest_sha256': binding.PREDECESSOR_SOURCE_SHA},
                  {'source_manifest_sha256': binding.CURRENT_SOURCE_SHA, 'status': 'pass'})
        for function in (binding.read_build_binding, binding.runtime_admission, binding.collect):
            for claim in claims:
                before = copy.deepcopy(claim)
                with self.subTest(function=function.__name__, claim=claim), self.assertRaises(binding.NotReady):
                    function(claim, execution_ready=True)
                self.assertEqual(claim, before)

    def test_canonical_roundtrip_returns_independent_data(self):
        original = {'retained': {'execution_source_manifest': binding.PREDECESSOR_SOURCE_SHA}}
        result = binding.decode(binding.canonical(original))
        result['retained']['execution_source_manifest'] = binding.CURRENT_SOURCE_SHA
        self.assertEqual(original['retained']['execution_source_manifest'], binding.PREDECESSOR_SOURCE_SHA)

    def test_fixed_shared_location_and_closed_roster(self):
        # Rehash the real installed package after integration. A missing final
        # pin is an explicit failure, not a fabricated positive source session.
        root, bodies = binding._shared_package()
        self.assertEqual(root.name, 'package_dependency_unit4_current')
        self.assertEqual(set(bodies), {'context.py', 'preparation.py', 'prepare_worker.py', 'test_context.py', 'test_preparation.py'})


if __name__ == '__main__':
    unittest.main()
