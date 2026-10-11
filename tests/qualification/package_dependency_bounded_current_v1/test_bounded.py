#!/usr/bin/env python3
"""Pure bytes/JSON/AST controls only. No old/current target module is executed."""
from __future__ import annotations

import ast
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import types
import unittest

HERE = Path(__file__).absolute().parent
spec = importlib.util.spec_from_file_location('_bounded_current_pure_controls', HERE / 'bounded.py')
bounded = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bounded)


def retained_repository():
    # Installed checkouts need no KH workspace. Staging fixtures are explicit.
    selected = os.environ.get('BOUNDED_CURRENT_RETAINED_REPOSITORY')
    return Path(selected).absolute() if selected else HERE.parents[2]


class PureDerivation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = bounded.authenticate()
        cls.repository = retained_repository()
        cls.before = {name: bounded.identity(bounded._read(cls.repository / name))
                      for name in cls.registry['inputs']}
        cls.bodies = bounded.read_retained_bodies(cls.repository)
        cls.modules_before = dict(sys.modules)
        cls.result = bounded.fixed_derivation('enum', cls.bodies)

    @classmethod
    def tearDownClass(cls):
        after = {name: bounded.identity(bounded._read(cls.repository / name))
                 for name in cls.registry['inputs']}
        if after != cls.before:
            raise AssertionError('retained inputs changed during pure controls')
        forbidden = {Path(row['historical_path']).stem for row in cls.registry['modules'].values()}
        if any(name in sys.modules and name not in cls.modules_before for name in forbidden):
            raise AssertionError('historical module was imported')

    def module(self, name):
        return self.result['modules'][self.registry['modules'][name]['current_path']]

    def tree(self, name):
        return ast.parse(self.module(name))

    def function(self, module, name):
        return next(node for node in self.tree(module).body if isinstance(node, ast.FunctionDef) and node.name == name)

    def test_closed_seven_members_and_six_controller_bodies(self):
        self.assertEqual(tuple(self.registry['members']), bounded.MEMBERS)
        self.assertEqual(len(self.registry['recipes']), 7)
        self.assertEqual(self.registry['recipes']['byte-storage']['module'],
                         self.registry['recipes']['byte-storage-public-build']['module'])
        self.assertEqual(self.registry['recipes']['byte-storage-public-build']['entrypoint'], 'prepare_public_build')
        self.assertEqual(self.registry['recipes']['owned-source-collector']['build_association'], 'independent-collector-builds')

    def test_all_members_are_inert_and_share_same_bundle(self):
        for member in bounded.MEMBERS:
            result = bounded.fixed_derivation(member, self.bodies)
            self.assertIs(result['inert'], True)
            self.assertIs(result['operational'], False)
            self.assertEqual(result['modules'], self.result['modules'])
            self.assertEqual(result['recipe']['ordinary_profiles'], ['debug', 'release'])
            self.assertIs(result['recipe']['operational'], False)

    def test_exact_current_manifest_relationship(self):
        proof = self.result['source_relationship']
        self.assertEqual(proof['source_members'], 376)
        self.assertEqual(proof['compiler_bodies'], 291)
        self.assertEqual(proof['changed_bodies'], ['src/main.rs'])
        self.assertEqual(proof['current_manifest_sha256'], bounded.CURRENT_MANIFEST_SHA256)
        self.assertNotEqual(proof['historical_manifest_sha256'], proof['current_manifest_sha256'])

    def test_full_inverse_and_all_function_proofs(self):
        unchanged = 0
        for name, record in self.registry['modules'].items():
            self.assertEqual(bounded.inverse(name, self.module(name)), self.bodies[record['historical_path']])
            proof = self.result['proofs'][name]
            self.assertTrue(proof['exact_inverse'])
            self.assertTrue(proof['full_original_ast_recovered'])
            unchanged += proof['unchanged_function_count']
            old = ast.parse(self.bodies[record['historical_path']])
            self.assertEqual(bounded._roster_assignments(old), bounded._roster_assignments(self.tree(name)))
            self.assertEqual(bounded._process_calls(old), bounded._process_calls(self.tree(name)))
        self.assertEqual(unchanged, 249)

    def test_all_changed_functions_are_explicitly_enumerated(self):
        count = 0
        for name, record in self.registry['modules'].items():
            old = bounded._functions(ast.parse(self.bodies[record['historical_path']]))
            new = bounded._functions(self.tree(name))
            actual = {key for key in old if bounded._dump(old[key]) != bounded._dump(new[key])}
            self.assertEqual(actual, set(record['changed_functions']))
            for value in record['changed_functions'].values():
                self.assertTrue(value['exact_changes'])
            count += len(actual)
        self.assertEqual(count, 20)

    def test_only_named_current_repository_import_edges(self):
        old_names = {Path(record['historical_path']).stem for record in self.registry['modules'].values()}
        for name, record in self.registry['modules'].items():
            calls = []
            for node in ast.walk(self.tree(name)):
                if isinstance(node, ast.Import):
                    self.assertFalse(any(alias.name in old_names for alias in node.names))
                if isinstance(node, ast.ImportFrom):
                    self.assertNotIn(node.module, old_names)
                if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == '_load_current':
                    calls.append(node.args[0].value)
                    self.assertIn(node.args[0].value, self.registry['modules'])
            expected = [edge['target'] for edge in record['repository_import_edges']]
            # from-import with multiple names yields one accessor per name.
            if name == 'byte-storage':
                expected.append('helper-verify-fixture-data')
            expected += self.registry['import_policy']['dynamic_edges'].get(name, [])
            self.assertCountEqual(calls, expected)

    def test_stdout_controls_transitive_imports_are_current(self):
        body = self.module('stdout-controls').decode()
        self.assertIn("enum_gate = _load_current('enum')", body)
        self.assertIn("stdin_gate = _load_current('stdin')", body)
        self.assertIn("controls = _load_current('stdout-controls')", self.module('stdout').decode())
        self.assertIn("controls = _load_current('stdin-controls')", self.module('stdin').decode())

    def test_all_derived_modules_raise_before_any_import_effect(self):
        for name in self.registry['modules']:
            nodes = [node for node in self.tree(name).body
                     if not (isinstance(node, ast.ImportFrom) and node.module == '__future__')
                     and not (isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant)
                              and isinstance(node.value.value, str))]
            self.assertIsInstance(nodes[0], ast.Raise)
            self.assertEqual(bounded._dump(nodes[0]), bounded._dump(ast.parse(
                'raise RuntimeError("bounded current execution is not reviewed")').body[0]))

    def test_source_policy_is_explicit_required_and_forwarded(self):
        functions = {'enum': ('read_reviewed_manifest', 'verify', 'main'),
                     'stdin': ('source_manifest', 'verify', 'main'),
                     'u8': ('verify', 'main', 'controller_inputs'),
                     'byte-storage': ('verify', 'prepare_public_build', 'main'),
                     'stdout': ('verify', 'main', 'controller_inputs'),
                     'owned-source-collector': ('source_inventory', 'run_qualification', 'main')}
        for module, names in functions.items():
            for name in names:
                function = self.function(module, name)
                position = [arg.arg for arg in function.args.kwonlyargs].index('source_policy')
                self.assertIsNone(function.args.kw_defaults[position])
                self.assertIn('_bounded_current.admit_policy(source_policy)', ast.unparse(function))
            self.assertIn('current_cli_policy()', self.module(module).decode())

    def test_old_digest_constants_have_no_live_reads(self):
        for name in ('enum', 'stdin', 'stdout', 'byte-storage'):
            for node in ast.walk(self.tree(name)):
                if isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
                    self.assertNotIn(node.id, ('REVIEWED_SOURCE_SHA256', 'SOURCE_SHA256', 'MANIFEST_PATH'))
        for name in ('u8', 'stdout'):
            for node in ast.walk(self.tree(name)):
                if isinstance(node, ast.Attribute):
                    self.assertNotIn(node.attr, ('REVIEWED_SOURCE_SHA256', 'MANIFEST_PATH'))

    def test_local_byte_storage_receipt_joins_use_policy(self):
        for name in ('verify', 'prepare_public_build'):
            body = ast.unparse(self.function('byte-storage', name))
            self.assertIn('source_policy.manifest_sha256', body)
            self.assertIn("get('reviewed_source_manifest'", body)
            self.assertIn('source_policy=source_policy', body)
        record = self.registry['modules']['byte-storage']
        # Public build's exact source-receipt and binary/profile equality are unchanged;
        # its sole changed node is the explicit named stdin import.
        old = next(node for node in ast.parse(self.bodies[record['historical_path']]).body
                   if isinstance(node, ast.FunctionDef) and node.name == 'admit_public_build')
        new = copy.deepcopy(self.function('byte-storage', 'admit_public_build'))
        new.body[1] = old.body[1]
        self.assertEqual(bounded._dump(old), bounded._dump(new))

    def test_stdout_receipt_retains_complete_manifest_event_and_inputs(self):
        self.assertIn('admit_source_receipt', self.registry['modules']['stdout']['unchanged_functions'])
        body = ast.unparse(self.function('stdout', 'admit_source_receipt'))
        self.assertIn('event_sha', body)
        self.assertIn('reviewed_source_manifest', body)
        self.assertIn('reviewed_inputs', body)
        verify = ast.unparse(self.function('stdout', 'verify'))
        self.assertIn('source_policy.manifest_sha256', verify)
        self.assertIn('repo / source_policy.manifest_path', verify)

    def test_collector_inventory_has_current_manifest_and_derived_controllers(self):
        source = ast.unparse(self.function('owned-source-collector', 'source_inventory'))
        self.assertIn('source_policy.require_manifest_document(reviewed)', source)
        self.assertIn('source_policy.manifest_path', source)
        self.assertIn('source_policy.controller_members', source)
        self.assertIn('current_source.reviewed_input_identities(repo, reviewed)', source)
        self.assertNotIn('current-source.json', source)
        run = ast.unparse(self.function('owned-source-collector', 'run_qualification'))
        self.assertIn("'--jobs', '1'", run)
        self.assertIn('source_policy=source_policy', run)
        self.assertIn('7792', run)

    def test_original_seven_audit_records_are_verbatim(self):
        expected = {'ci.yml:L729': '0b7505463883e7d2d649816279c24ed16e1638ff8d9b983c2c1196d48320dc6c', 'ci.yml:L725': '073d041dddae350baaef626f5448e80afe2013cf4868db9fdd0dd92a17c9e018', 'ci.yml:L673': '004e4959a87cb25c783c61b2ef5eec0108903122ac75374788b833ab954e8a53', 'ci.yml:L899': '6f9000def274b394d1ebadc561b6b5d83194f526ecf0d20cd1707d86984125a6', 'ci.yml:L751': '06e7afedbbf4e59a78050b17031698dfc6d86f502deb4348b2b0f41980568471', 'ci.yml:L764': '95b918079d04116fe365f42ed8c9f805cd9008a53290a0ae17a88e4898e25acc', 'ci.yml:L694': 'ed8264ef5135e232085bded65ee640f5586e52a27a42dc78d8c4087db376b2e8'}
        for recipe in self.registry['recipes'].values():
            record = recipe['original_record']
            encoded = json.dumps(record, sort_keys=True, separators=(',', ':')).encode()
            self.assertEqual(hashlib.sha256(encoded).hexdigest(), expected[recipe['original_id']])
            self.assertEqual(record['disposition'], 'blocked')
            for command, seal in (('command', 'command_sha256'), ('active_command', 'active_command_sha256')):
                self.assertEqual(hashlib.sha256(record['original'][command].encode()).hexdigest(), record['original'][seal])

    def test_fixture_provider_retry_and_native_rosters_are_retained(self):
        for module in ('stdin-controls', 'stdout-controls', 'u8', 'byte-storage', 'owned-source-collector'):
            self.assertTrue(self.result['proofs'][module]['unchanged_rosters'])
        for name in self.registry['inputs']:
            self.assertEqual(bounded.identity(self.bodies[name]), self.registry['inputs'][name])
        self.assertIn('tests/qualification/byte_storage_current/registry.json', self.bodies)
        self.assertIn('tests/fixtures/owned_source/native-probe-sources.json', self.bodies)
        self.assertIn('tests/qualification/bounded_stdin_current/read_retry_shim.c', self.bodies)

    def test_changed_retained_body_rejects_before_parsing(self):
        bodies = dict(self.bodies)
        name = self.registry['modules']['enum']['historical_path']
        bodies[name] = b'not valid Python'
        with self.assertRaisesRegex(ValueError, 'retained input changed'):
            bounded.fixed_derivation('enum', bodies)

    def test_missing_extra_or_wrong_type_retained_inputs_reject(self):
        for mutation in ('missing', 'extra', 'type'):
            bodies = dict(self.bodies)
            if mutation == 'missing':
                bodies.pop(next(iter(bodies)))
            elif mutation == 'extra':
                bodies['scripts/unreviewed.py'] = b''
            else:
                bodies[next(iter(bodies))] = 'text is not bytes'
            with self.assertRaises(ValueError):
                bounded.fixed_derivation('enum', bodies)

    def test_old_manifest_substitution_and_mixed_main_reject(self):
        bodies = dict(self.bodies)
        bodies[bounded.CURRENT_MANIFEST_PATH] = bodies[bounded.HISTORICAL_MANIFEST_PATH]
        with self.assertRaisesRegex(ValueError, 'retained input changed'):
            bounded.fixed_derivation('enum', bodies)
        bodies = dict(self.bodies)
        current = json.loads(bodies[bounded.CURRENT_MANIFEST_PATH])
        for row in current['files']:
            if row['path'] == 'src/main.rs':
                row.update(self.registry['historical_main'])
        bodies[bounded.CURRENT_MANIFEST_PATH] = json.dumps(current).encode()
        with self.assertRaisesRegex(ValueError, 'retained input changed'):
            bounded.fixed_derivation('enum', bodies)

    def test_unknown_member_and_module_reject(self):
        for name in ('all', 'stdin-controls', '', '../enum', None, 1):
            with self.assertRaises(ValueError):
                bounded.fixed_derivation(name, self.bodies)
        with self.assertRaises(ValueError):
            bounded.inverse('arbitrary', b'')

    def test_inverse_rejects_body_and_seam_tampering(self):
        for name in ('enum', 'stdout-controls', 'owned-source-collector'):
            with self.assertRaisesRegex(ValueError, 'identity differs'):
                bounded.inverse(name, self.module(name) + b'\n')
        record = copy.deepcopy(self.registry['modules']['enum'])
        record['edits'][0]['count'] += 1
        with self.assertRaisesRegex(ValueError, 'occurrence differs'):
            bounded._transform(self.bodies[record['historical_path']], record)

    def test_context_plans_and_derivations_reject_fake_success(self):
        class Bomb:
            def __getattribute__(self, name):
                raise AssertionError('unsealed context was consulted')
        for context in (Bomb(), {'status': 'passed', 'manifest_sha256': bounded.CURRENT_MANIFEST_SHA256}):
            with self.assertRaises(ValueError):
                bounded.plan(context, 'enum')
            with self.assertRaises(ValueError):
                bounded.derive('enum', self.bodies, context)
            with self.assertRaises(ValueError):
                bounded.current_policy(context)

    def test_run_and_loader_fail_before_any_output_or_object_access(self):
        class Bomb:
            def __getattribute__(self, name):
                raise AssertionError('blocked runtime touched an argument')
        with self.assertRaisesRegex(ValueError, 'execution is blocked'):
            bounded.run(Bomb(), Bomb(), Bomb(), Bomb())
        with self.assertRaisesRegex(ValueError, 'operational release'):
            bounded.load_derived_sibling(Bomb())
        with self.assertRaisesRegex(ValueError, 'producer seam'):
            bounded.current_cli_policy()

    def test_policy_has_no_public_or_deserialized_constructor(self):
        with self.assertRaises(TypeError):
            bounded.CurrentPolicy()
        forged = object.__new__(bounded.CurrentPolicy)
        with self.assertRaisesRegex(ValueError, 'original bounded CurrentPolicy'):
            forged.document()
        self.assertEqual(bounded._POLICIES, {})
        self.assertEqual(self.registry['ast_normalization'], 'cpython-3.12-compatible-show-empty-v1')

    def test_forged_canonical_source_slot_cannot_issue_policy(self):
        key = bounded.SOURCE_OWNER_KEY
        original = sys.modules.get(key)
        forged = types.ModuleType(key)
        forged.__file__ = str(HERE.parent / 'package_dependency_ci_v1/source.py')
        forged.__source_sha256__ = bounded.SOURCE_DEPENDENCY_SHA256
        touched = []
        def forbidden(*args, **kwargs):
            touched.append(True)
            raise AssertionError('forged owner was consulted')
        forged.owner = forbidden
        forged.verify_job = forbidden
        forged.projection = forbidden
        sys.modules[key] = forged
        try:
            with self.assertRaises(ValueError):
                bounded.current_policy({'status': 'passed'})
            self.assertEqual(touched, [])
            self.assertEqual(bounded._POLICIES, {})
        finally:
            if original is None:
                del sys.modules[key]
            else:
                sys.modules[key] = original
        # A future dependency hash alone cannot release ambient-slot adoption.
        source = ast.parse((HERE / 'bounded.py').read_bytes())
        owner = next(node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == '_source_owner')
        self.assertIn("globals().get('_SOURCE_OWNER')", ast.unparse(owner))
        self.assertIn('owns_adapter', ast.unparse(owner))

    def test_package_and_decoder_are_closed(self):
        self.assertEqual({p.name for p in HERE.iterdir()}, bounded.PACKAGE_FILES)
        self.assertEqual(bounded.SOURCE_DEPENDENCY_SHA256, hashlib.sha256((HERE.parent / 'package_dependency_ci_v1/source.py').read_bytes()).hexdigest())
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            bounded.decode(b'{"x": 1, "x": 2}')
        with self.assertRaisesRegex(ValueError, 'nonfinite'):
            bounded.decode(b'{"x": NaN}')
        for value in ('../main.rs', '/src/main.rs', 'src//main.rs', 'src/./main.rs'):
            with self.assertRaises(ValueError):
                bounded._safe_relative(value)


if __name__ == '__main__':
    unittest.main(verbosity=2)
