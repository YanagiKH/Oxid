"""Pure bytes/AST/mock controls. Run with python -B; SOURCE may name a fixture.

SOURCE defaults to the installed repository root. No retained/derived controller
imports, fixture generators, subprocesses, Git, compilers, targets, or network.
"""
import ast
import copy
import json
import os
from pathlib import Path
import sys
import types
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import unit1

SOURCE = Path(os.environ.get('SOURCE', str(Path(__file__).resolve().parents[3])))


class Poison:
    def __getattribute__(self, name):
        raise AssertionError('caller inspected: ' + name)

    def __iter__(self):
        raise AssertionError('caller iterated')

    def __repr__(self):
        raise AssertionError('caller rendered')


class Unit1SourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.registry = unit1.authenticate()
        cls.bodies = unit1.read_retained_bodies(SOURCE)
        cls.result = unit1.fixed_derivation(cls.bodies)
        cls.derived = cls.result['members']['unit1-both']

    def reject(self, bodies):
        with self.assertRaises(ValueError):
            unit1.fixed_derivation(bodies)

    def changed(self, name, old, new):
        bodies = dict(self.bodies)
        self.assertIn(old, bodies[name])
        bodies[name] = bodies[name].replace(old, new, 1)
        self.reject(bodies)

    def test_closed_four_file_package(self):
        self.assertEqual({p.name for p in Path(unit1.__file__).parent.iterdir()}, unit1.PACKAGE_FILES)
        self.assertEqual(self.registry['members'], ['unit1-both'])

    def test_exact_retained_closure(self):
        self.assertEqual(len(self.bodies), 34)
        for name, body in self.bodies.items():
            with self.subTest(name=name):
                self.assertEqual(unit1.identity(body), self.registry['inputs'][name])

    def test_every_retained_body_mutation_rejected(self):
        for name, body in self.bodies.items():
            with self.subTest(name=name):
                changed = dict(self.bodies)
                changed[name] = body + b'\n'
                self.reject(changed)

    def test_every_retained_body_omission_rejected(self):
        for name in self.bodies:
            with self.subTest(name=name):
                changed = dict(self.bodies)
                del changed[name]
                self.reject(changed)

    def test_extra_retained_body_rejected(self):
        self.reject({**self.bodies, 'unexpected.py': b'pass\n'})

    def test_non_exact_mapping_rejected(self):
        class Ambient(dict):
            pass
        self.reject(Ambient(self.bodies))

    def test_non_exact_bytes_rejected(self):
        for value in (bytearray(b'bytes'), 'text', Poison()):
            changed = dict(self.bodies)
            changed[unit1.RUNNER] = value
            self.reject(changed)

    def test_authenticates_all_retained_before_parse(self):
        changed = dict(self.bodies)
        changed[unit1.RUNNER] = b'import untrusted\n'
        with patch.object(unit1, 'authenticate', return_value=self.registry), \
             patch.object(unit1.ast, 'parse', side_effect=AssertionError('parsed untrusted bytes')), \
             patch.object(unit1, 'decode', side_effect=AssertionError('decoded untrusted bytes')):
            self.reject(changed)

    def test_input_mapping_snapshotted_before_authentication(self):
        caller = dict(self.bodies)
        validate = unit1._authenticate_retained
        def checked(registry, received):
            self.assertIsNot(received, caller)
            validate(registry, received)
            caller[unit1.RUNNER] = b'untrusted caller replacement after authentication'
        with patch.object(unit1, '_authenticate_retained', side_effect=checked):
            result = unit1.fixed_derivation(caller)
        self.assertEqual(result['members']['unit1-both'], self.derived)

    def test_first_executable_unconditional_barrier(self):
        self.assertTrue(self.derived.startswith(unit1.BARRIER))
        tree = ast.parse(self.derived)
        self.assertIsInstance(tree.body[0], ast.Raise)
        self.assertIsInstance(tree.body[0].exc.func, ast.Name)
        self.assertEqual(tree.body[0].exc.func.id, 'RuntimeError')

    def test_derived_bodies_never_executed_by_package(self):
        tree = ast.parse(Path(unit1.__file__).read_bytes())
        for node in ast.walk(tree):
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
                self.assertNotIn(node.func.id, ('exec', 'eval', 'compile', '__import__'))
            if isinstance(node, ast.Import):
                self.assertFalse(any(name.name in ('subprocess', 'socket', 'importlib') for name in node.names))

    def test_exact_inverse(self):
        self.assertEqual(unit1.inverse('unit1-both', self.derived), self.bodies[unit1.RUNNER])

    def test_unknown_inverse_member_rejected(self):
        for member in ('debug', 'release', 'unit1', '../unit1-both', '', None):
            with self.subTest(member=member), self.assertRaises(ValueError):
                unit1.inverse(member, self.derived)

    def test_mutated_inverse_rejected(self):
        for body in (self.derived + b'\n', self.derived[:-1], self.derived[len(unit1.BARRIER):],
                     self.derived.replace(b'69', b'68', 1), self.derived * 2):
            with self.assertRaises(ValueError):
                unit1.inverse('unit1-both', body)

    def test_fixed_exact_occurrence_edits(self):
        raw = self.bodies[unit1.RUNNER]
        record = self.registry['recipes']['unit1-both']
        self.assertEqual(len(record['edits']), 11)
        for edit in record['edits']:
            before, after = edit['before'].encode(), edit['after'].encode()
            self.assertEqual(raw.count(before), edit['count'])
            raw = raw.replace(before, after)
        self.assertEqual(raw, self.derived)

    def test_duplicate_edit_occurrence_rejected(self):
        record = copy.deepcopy(self.registry['recipes']['unit1-both'])
        raw = self.bodies[unit1.RUNNER] + record['edits'][0]['before'].encode()
        record['retained'] = unit1.identity(raw)
        with self.assertRaisesRegex(ValueError, 'exact occurrence'):
            unit1._transform(raw, record)

    def test_missing_edit_occurrence_rejected(self):
        record = copy.deepcopy(self.registry['recipes']['unit1-both'])
        raw = self.bodies[unit1.RUNNER].replace(record['edits'][0]['before'].encode(), b'', 1)
        record['retained'] = unit1.identity(raw)
        with self.assertRaisesRegex(ValueError, 'exact occurrence'):
            unit1._transform(raw, record)

    def test_only_main_definition_changes(self):
        proof = self.result['proofs']
        self.assertEqual(proof['changed_runner_definitions'], ['main'])
        self.assertEqual(proof['unchanged_runner_definitions'], ['digest', 'save_json', 'run', 'checked', 'main.ignore'])

    def test_complete_retained_definition_inventory(self):
        inventory = self.result['proofs']['retained_definitions']
        self.assertEqual(inventory, self.registry['retained_definitions'])
        self.assertEqual(sum(map(len, inventory.values())), 73)
        self.assertIn('authenticate', [row['name'] for row in inventory[unit1.RETAINED + 'unit1_current_support.py']])

    def test_complete_predicate_command_constant_inventory(self):
        self.assertEqual(self.result['proofs']['retained_predicates_commands_constants'],
                         self.registry['retained_predicates_commands_constants'])
        self.assertIs(self.result['proofs']['all_original_runner_predicates_commands_constants_preserved'], True)

    def test_historical_runner_and_reviewer_inverses(self):
        proofs = self.result['proofs']['historical_inverses']
        self.assertEqual([row['seams'] for row in proofs], [13, 5, 1])
        for row in proofs:
            self.assertEqual(row['inverse'], unit1.identity(self.bodies[unit1.ORIGINAL + row['original']]))

    def test_historical_constants_unchanged(self):
        raw = self.bodies[unit1.RETAINED + 'unit1_current_support.py']
        values = unit1._literal_assignments(raw, ('CHECKPOINT', 'ORIGINAL_CHECKPOINT', 'CURRENT_SOURCE_SHA', 'ORIGINAL_MAP_SHA', 'CURRENT_MAP_SHA'))
        self.assertEqual(values['CHECKPOINT'], 'b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489')
        self.assertEqual(values['CURRENT_SOURCE_SHA'], '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261')
        self.assertNotIn(b'CHECKPOINT =', self.derived)

    def test_current_main_and_manifest_relationship_only(self):
        relationship = self.result['source_relationship']
        self.assertEqual(relationship['changed_manifest_members'], ['src/main.rs'])
        self.assertEqual((relationship['manifest_members'], relationship['compiler_members']), (376, 291))
        self.assertIs(relationship['live_checkout_admitted'], False)
        self.assertIs(relationship['all376_bodies_read'], False)

    def test_historical_manifest_substitution_rejected(self):
        self.reject({**self.bodies, unit1.CURRENT_MANIFEST: self.bodies[unit1.HISTORICAL_MANIFEST]})

    def test_stale_or_omitted_main_rejected(self):
        self.reject({**self.bodies, 'src/main.rs': b'// stale historical main\n'})
        changed = dict(self.bodies)
        del changed['src/main.rs']
        self.reject(changed)

    def test_wrong_checkpoint_rejected(self):
        self.changed(unit1.CURRENT_MANIFEST, b'ce522e393646ce6478848cee31f307cb4b6ad5e8', b'0000000000000000000000000000000000000000')

    def test_unexpected_prepared_source_change_rejected_as_retained_input(self):
        self.changed('src/frontend/project.rs', b'\n', b'\n// unreviewed source overlay\n')

    def test_original69_plus_control_both_profiles(self):
        coverage = self.result['coverage']
        self.assertEqual(len(set(coverage['original_names'])), 69)
        self.assertEqual(len(coverage['control_names']), 1)
        self.assertFalse(set(coverage['original_names']) & set(coverage['control_names']))
        self.assertEqual(coverage['profiles'], ['debug', 'release'])
        self.assertEqual((coverage['original_executions_required'], coverage['control_executions_required']), (138, 2))

    def test_incomplete_roster_rejected(self):
        self.changed(unit1.ORIGINAL + 'expected-test-roster.json', b'heldout_bytes_exact', b'heldout_bytes_wrong')

    def test_changed_resource_boundary_rejected(self):
        self.changed(unit1.ORIGINAL + 'expected-resources.json', b'1048576', b'1048575')

    def test_resource_inventories_and_socket_exclusion(self):
        coverage = self.result['coverage']
        self.assertEqual((coverage['filesystem_authored'], coverage['filesystem_available'], coverage['resources']), (42, 41, 12))
        self.assertEqual(len(coverage['resource_inventories']), 12)
        unavailable = [row['id'] for row in self.registry['coverage']['fixture_expectations'] if row['unavailable']]
        self.assertEqual(unavailable, ['child_final_socket'])

    def test_all_resource_boundaries_literal(self):
        rows = {row['id']: row['expected'] for row in self.registry['coverage']['resource_expectations']}
        for prefix, field, pair in (('bytes', 'aggregate_source_bytes', (1048576,1048577)),
                                    ('tokens', 'tokens', (100000,100001)), ('modules','modules',(256,257)),
                                    ('depth','depth',(32,33)), ('entries','scan_entries',(100000,100200)),
                                    ('name_units','scan_name_units',(16777216,16777344))):
            self.assertEqual((rows[prefix+'_exact'][field], rows[prefix+'_over'][field]), pair)

    def test_exact_cargo_vector_preserved(self):
        vector = b"argv=[args.cargo,'test','--offline','--manifest-path',str(source/'Cargo.toml'),'--bin','oxid','--no-run','--message-format=json']"
        self.assertIn(vector, self.derived)
        self.assertIn(b"if profile=='release':argv.append('--release')", self.derived)
        for flag in (b'--locked', b'--jobs', b"'--target'"):
            self.assertNotIn(flag, self.derived)

    def test_exact_project_include_and_control_preserved(self):
        self.assertIn(b"current.include_control(test)", self.derived)
        self.assertIn(b"if 'mod reviewer_unit1;' in project.read_text():raise RuntimeError", self.derived)
        self.assertIn(b"f.write('\\n#[cfg(all(test, target_os = \"linux\", target_arch = \"x86_64\"))]\\n#[path = '+rust_string_literal(str(test))+']\\nmod reviewer_unit1;\\n')", self.derived)
        self.assertIn(b"include!(\"unit1_lexer_controls.rs\");", self.bodies[unit1.RETAINED+'unit1_current_support.py'])

    def test_fixture_equality_removes_only_canonical_relocations(self):
        self.assertIn(b"f['expected'].pop('canonical_root_file_hex',None);f['expected'].pop('canonical_child_hex',None)", self.derived)
        self.assertIn(b"assert actual==json.loads((PACKAGE/'expected-fixtures.json').read_text())", self.derived)
        self.assertIn(b"assert (q/'resource-fixture-manifest.json').read_bytes()==(PACKAGE/'expected-resources.json').read_bytes()", self.derived)

    def test_profile_fixed_to_both(self):
        self.assertIn(b"if args.profile != 'both':raise RuntimeError", self.derived)
        self.assertEqual(self.derived.count(b'current.profile_guard(_package_dependency_original_job_context,'), 2)

    def test_copied_directory_not_readmitted_as_git_checkout(self):
        self.assertNotIn(b'current.check_admission(source,current_admission)', self.derived)
        self.assertIn(b'current.capture_prepared_input(', self.derived)
        self.assertIn(b'current.check_prepared_input(', self.derived)
        self.assertIn(b'current.seal_prepared_source(', self.derived)

    def test_historical_support_import_not_current_authority(self):
        self.assertNotIn(b'import unit1_current_support as current', self.derived)
        self.assertIn(b'current_adapter.current_policy(_package_dependency_original_job_context).unit1_support', self.derived)

    def test_future_artifact_and_terminal_association_are_explicit(self):
        self.assertIn(b'current.associate_test_artifact(', self.derived)
        self.assertIn(b'current.check_terminal(', self.derived)
        contract = self.registry['required_owner_contract']
        self.assertIn('CompletedProcess', contract['test_artifact'])
        self.assertIn('insufficient', contract['test_artifact'])
        self.assertIn('required executable artifacts', contract['terminal'])

    def test_original_per_name_execution_predicates_preserved(self):
        body = self.bodies[unit1.RETAINED+'unit1_run_reviewer_current.py']
        for literal in (b'timeout=25', b'(1,0,0,0)', b'code==0 and one_summary and named_ok and ran_one',
                        b'sorted(tests)!=sorted(expected)', b'len(expected)==70'):
            self.assertIn(literal, body)

    def test_lexer_ordinals_targets_and_full_span_control_preserved(self):
        body = self.bodies[unit1.RETAINED+'unit1_reviewer_additional.current.rs']
        self.assertIn(b'vec![5,6,19,20,21]', body)
        self.assertIn(b'[(4,(0,0,3)),(8,(0,6,7)),(4,(1,0,2)),(8,(1,5,6)),(16,(1,10,11))]', body)
        control = self.bodies[unit1.RETAINED+'unit1_lexer_controls.rs']
        self.assertIn(';;;;🦀'.encode(), control)
        self.assertIn(b'usize::from(!overflow)', control)

    def test_all_18_retained_controls_are_separate_obligations(self):
        tree = ast.parse(self.bodies[unit1.RETAINED+'test_unit1_current.py'])
        names = [node.name for node in ast.walk(tree) if isinstance(node, ast.FunctionDef) and node.name.startswith('test_')]
        self.assertEqual(len(names), 18)
        self.assertIs(self.result['execution_qualified'], False)

    def test_policy_rejects_before_caller_inspection(self):
        with self.assertRaisesRegex(RuntimeError, 'unregistered'):
            unit1.current_policy(Poison())

    def test_plan_rejects_before_caller_inspection(self):
        with self.assertRaisesRegex(RuntimeError, 'unregistered'):
            unit1.plan(Poison())

    def test_run_rejects_before_caller_inspection(self):
        with self.assertRaisesRegex(RuntimeError, 'unregistered'):
            unit1.run(Poison(), context=Poison(), artifact=Poison())

    def test_forged_owners_paths_and_serialized_capabilities_rejected(self):
        forged = {'schema':'JobSourceContext', 'head':self.registry['source_transition']['current_checkpoint']['head'],
                  'manifest':self.result['source_relationship']['current_manifest'],
                  'prepared_source':{'role':'unit1-review'}, 'test_artifact':{'profile':'debug','exit':0},
                  'operational_release':True, 'owner_registration':'unit1'}
        ambient = types.ModuleType('_oxid_package_dependency_ci_source_v1')
        ambient.__file__ = str(SOURCE/'tests/qualification/package_dependency_ci_v1/source.py')
        with patch.dict(sys.modules, {ambient.__name__:ambient}):
            for candidate in (forged, ambient, {'profile':'release'}, {'profile':'wrong'}, {'artifact':'reused'}, {'process_completion':None}):
                for function in (unit1.current_policy, unit1.plan, unit1.run):
                    with self.assertRaises(RuntimeError):
                        function(candidate)

    def test_gate_bodies_are_unconditional_raises(self):
        tree = ast.parse(Path(unit1.__file__).read_bytes())
        for node in tree.body:
            if isinstance(node, ast.FunctionDef) and node.name in ('current_policy','plan','run'):
                self.assertEqual(len(node.body), 1)
                self.assertIsInstance(node.body[0], ast.Raise)

    def test_ast_normalization_is_versioned_and_bounded(self):
        self.assertEqual(self.registry['ast_normalization'], 'explicit-fields-no-locations-v1')
        with patch.object(unit1.sys, 'version_info', (3, 99, 0)), self.assertRaises(ValueError):
            unit1._node_id(ast.parse('pass'))

    def test_proofs_are_detached_data_not_authority(self):
        result = unit1.fixed_derivation(self.bodies)
        result['operational_release'] = 'forged'
        result['proofs'].clear()
        with self.assertRaises(RuntimeError):
            unit1.plan(result)
        self.assertEqual(unit1.fixed_derivation(self.bodies)['operational_release'], None)

    def test_duplicate_and_nonfinite_json_rejected(self):
        for raw in (b'{"a":1,"a":2}', b'{"a":NaN}', b'{"a":Infinity}'):
            with self.assertRaises(ValueError):
                unit1.decode(raw)

    def test_unsafe_relative_paths_rejected(self):
        for name in ('../x', '/x', './x', 'a//b', 'a/../b', ''):
            with self.assertRaises(ValueError):
                unit1._relative(name)

    def test_recipe_body_mutation_rejected(self):
        original = unit1._read
        def changed(path):
            raw = original(path)
            return raw+b'\n' if Path(path).name == 'recipes.json' else raw
        with patch.object(unit1, '_read', side_effect=changed), self.assertRaises(ValueError):
            unit1.authenticate()

    def test_unsealed_release_manifest_rejected(self):
        original = unit1._read
        def changed(path):
            raw = original(path)
            if Path(path).name == 'package-manifest.json':
                value = json.loads(raw)
                value['operational_release'] = True
                return json.dumps(value).encode()
            return raw
        with patch.object(unit1, '_read', side_effect=changed), self.assertRaises(ValueError):
            unit1.authenticate()

    def test_installed_default_has_no_hidden_workspace_dependency(self):
        root = Path(unit1.__file__).parent
        for name in unit1.PACKAGE_FILES:
            raw = (root/name).read_bytes()
            self.assertNotIn(b'K' + b'H/', raw)
            self.assertNotIn(b'/work' + b'space/', raw)
        body = (root/'test_unit1.py').read_text()
        self.assertIn("os.environ.get('SOURCE', str(Path(__file__).resolve().parents[3]))", body)


if __name__ == '__main__':
    unittest.main()
