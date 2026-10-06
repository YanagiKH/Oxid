#!/usr/bin/env python3
"""Bounded current-source admission controls, never semantic qualification."""
import ast
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import portable as p


class ProjectedOverlapRosterControls(unittest.TestCase):
    def test_exact_projected_overlap_roster(self):
        self.assertEqual(p.PROJECTED_INSTRUMENTATION_PATHS,
                         ('src/frontend/ast.rs', 'src/frontend/parser.rs'))
        self.assertTrue(callable(p.restore_projected_source))


class UnaryOverlapRosterControls(unittest.TestCase):
    def test_exact_unary_overlap_roster(self):
        self.assertEqual(p.UNARY_INSTRUMENTATION_PATHS,
                         ('src/frontend/ast.rs', 'src/frontend/parser.rs'))
        self.assertTrue(callable(p.restore_unary_source))


class CurrentAuthorityControls(unittest.TestCase):
    def test_historical_and_current_authorities_remain_distinct(self):
        a = p.authority()
        historical = p.read(p.FROZEN / 'authority.json')
        self.assertEqual({k: v for k, v in a.items() if k not in ('current', 'current_source')}, historical)
        self.assertEqual([len(a[k]) for k in ('original_files', 'derived_files', 'control_derived_files')], [283, 286, 286])
        self.assertEqual([len(a['current'][k]) for k in ('current_base_files', 'current_derived_files', 'current_control_derived_files')], [400, 403, 403])
        self.assertEqual(len(a['current_source']['files']), 237)
        self.assertEqual(len(p.compiler_map(a)), 182)
        self.assertNotEqual(a['candidate_source_manifest_sha256'], a['current']['current_candidate_source_manifest_sha256'])
        self.assertEqual([r['path'] for r in a['current']['source_delta']], list(p.CURRENT_PATHS))
        self.assertEqual(len(a['current']['source_delta']), 194)
        self.assertEqual([r['path'] for r in a['current']['source_delta'] if r['before'] is None], ['fixtures/typed-record-composition-samples/main.ox',
 'fixtures/typed-record-composition-samples/model.ox',
 'fixtures/typed-record-composition-samples/ops.ox',
 'src/frontend/declaration_index/enum_query_tests.rs',
 'src/frontend/declaration_index/enum_tests.rs',
 'src/frontend/declaration_index/enum_views.rs',
 'src/frontend/enum_public_tests.rs',
 'src/frontend/format.rs',
 'src/frontend/format/ast_tests.rs',
 'src/frontend/format/enum_candidate_tests.rs',
 'src/frontend/format/resource_tests.rs',
 'src/frontend/format_cli.rs',
 'src/frontend/oir/negation_raw_tests.rs',
 'src/frontend/oir/owned/array_native_resource_tests.rs',
 'src/frontend/oir/owned/array_native_tests.rs',
 'src/frontend/oir/owned/array_observe.rs',
 'src/frontend/oir/owned/array_reference_boundary_tests.rs',
 'src/frontend/oir/owned/array_reference_tests.rs',
 'src/frontend/oir/owned/array_tests.rs',
 'src/frontend/oir/owned/composition_native_tests.rs',
 'src/frontend/oir/owned/composition_reference_tests.rs',
 'src/frontend/oir/owned/composition_verifier_tests.rs',
 'src/frontend/oir/owned/enum_admission_tests.rs',
 'src/frontend/oir/owned/enum_consumer_fixtures.rs',
 'src/frontend/oir/owned/enum_formatter_allocation_tests.rs',
 'src/frontend/oir/owned/enum_index_allocation_tests.rs',
 'src/frontend/oir/owned/enum_layout_tests.rs',
 'src/frontend/oir/owned/enum_match_tests.rs',
 'src/frontend/oir/owned/enum_native_tests.rs',
 'src/frontend/oir/owned/enum_parser_allocation_tests.rs',
 'src/frontend/oir/owned/enum_query_allocation_tests.rs',
 'src/frontend/oir/owned/enum_reference_tests.rs',
 'src/frontend/oir/owned/negation_raw_tests.rs',
 'src/frontend/oir/owned/projected_slice_native_tests.rs',
 'src/frontend/oir/owned/reviewer_array_observer_tests.rs',
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
 'src/frontend/oir/owned/slice_native_tests.rs',
 'src/frontend/oir/owned/source/array_consumer_tests.rs',
 'src/frontend/oir/owned/source/array_pipeline.rs',
 'src/frontend/oir/owned/source/array_pipeline_rows.rs',
 'src/frontend/oir/owned/source/array_pipeline_tests.rs',
 'src/frontend/oir/owned/source/array_pipeline_transport.rs',
 'src/frontend/oir/owned/source/array_type_controls.rs',
 'src/frontend/oir/owned/source/array_types_tests.rs',
 'src/frontend/oir/owned/source/enum_native_source_tests.rs',
 'src/frontend/oir/owned/source/enum_storage_failure_tests.rs',
 'src/frontend/oir/owned/source/enum_type_tests.rs',
 'src/frontend/oir/owned/source/hir_budget.rs',
 'src/frontend/oir/owned/source/hir_budget_tests.rs',
 'src/frontend/oir/owned/source/projected_slice_raw_tests.rs',
 'src/frontend/oir/owned/source/resolver_enum_tests.rs',
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs',
 'src/frontend/oir/owned/source/resolver_paid_tests.rs',
 'src/frontend/oir/owned/source/resolver_storage.rs',
 'src/frontend/oir/owned/source/resolver_storage_tests.rs',
 'src/frontend/oir/owned/source/slice_raw_tests.rs',
 'src/frontend/oir/owned/source/slice_tests.rs',
 'src/frontend/oir/owned/source/type_storage.rs',
 'src/frontend/oir/owned/source/type_storage_tests.rs',
 'src/frontend/oir/owned_types/array_tests.rs',
 'src/frontend/oir/owned_types/composition_tests.rs',
 'src/frontend/oir/owned_types/enum_integration_tests.rs',
 'src/frontend/oir/owned_types/enums.rs',
 'src/frontend/oir/unary_source_tests.rs',
 'src/frontend/parser/array_syntax_tests.rs',
 'src/frontend/parser/arrays.rs',
 'src/frontend/parser/enum_syntax_tests.rs',
 'src/frontend/parser/enums.rs',
 'src/frontend/project/array_syntax_tests.rs',
 'src/frontend/project/budget_real_null_observer.rs',
 'src/frontend/project/enum_carrier_tests.rs',
 'src/frontend/project/enum_index_tests.rs',
 'tests/fixtures/bounded_enum_scanner/main.ox',
 'tests/fixtures/bounded_enum_scanner/scanner.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-call-context-excluded/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-no-context/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-nonzero-annotation/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-reassignment-context-excluded/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-return-context-excluded/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-scalar-context/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/grouped-complete-access-and-index/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-first-heterogeneous-element/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/nested-empty-does-not-inherit-context/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-array-bad-index/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-base-kind-before-index-kind/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-internal-type-first/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-literal-range-before-base-type/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-resolution-before-base-type/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-unknown-base-first/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-0/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-1/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-0/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-1/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-1/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/structural-identities-pairwise-distinct/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/utf8-read-primary/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/whole-program-resolution-before-earlier-function-type/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-base-kind-before-index-kind/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-both-subtree-errors-rhs-wins/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-element-before-mutability/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-internal-type-before-base/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-kind-before-rhs-element/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-element/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-index/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-owner-mutability-last/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-base-before-both-operands/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-rhs-before-index/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-rhs-internal-type-first/main.ox',
 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-whole-resolution-before-rhs-typing/main.ox',
 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-array-free/main.ox',
 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox',
 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-record-only/main.ox',
 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox',
 'tests/typed_record_composition.rs'])
        self.assertEqual(sum(r['before'] is not None for r in a['current']['source_delta']), 77)
        self.assertEqual(a['current']['reviewed_source_head'], '511df03975c2aa1a815f92155d673adb3571ff77')
        self.assertEqual(a['current']['source_only_tree'], '7eb1fa1ef8e3107788ca93758f762882ea031d72')

    def test_copied_algorithms_have_only_reviewed_change_boundaries(self):
        old_text = (p.FROZEN / 'portable.py').read_text()
        new_text = Path(p.__file__).read_text()
        functions = lambda text: {n.name: ast.get_source_segment(text, n) for n in ast.parse(text).body if isinstance(n, ast.FunctionDef)}
        old, new = functions(old_text), functions(new_text)
        allowed = {'authority', 'compiler_map', 'verify_checkout', 'prepare', 'verify_overlay',
                   'session_at', 'verify_cargo', 'comparator', 'effective_authority', 'compare', 'main'}
        self.assertEqual(set(new) - set(old), {'compose_source_read', 'compose_array_instrumentation', 'restore_division_source', 'restore_slices_source', 'restore_composition_source', 'restore_unary_source', 'restore_projected_source', 'restore_enum_source', 'compose_namespace_resource', 'compose_enum_parser_helper', 'project_enum_observations', 'compose_division_lexer', 'compose_observer_initializer', 'current_candidate', 'current_overlay', 'verify_transition_records', 'verify_historical_overlay', 'current_parser_contract'})
        self.assertEqual(set(old) - set(new), set())
        for name in set(old) - allowed:
            with self.subTest(function=name): self.assertEqual(new[name], old[name])
        self.assertEqual(p.comparator()[1], p.read(p.FROZEN / 'derivation-proof.json'))
        self.assertEqual(p.sha((p.FROZEN / 'frozen/comparator/comparator.py').read_bytes()), p.COMPARATOR_SHA)

    def test_current_maps_cannot_be_replaced_with_historical_maps(self):
        a = p.authority()
        self.assertNotEqual(a['current']['current_base_files'], a['original_files'])
        for role in ('derived_files', 'control_derived_files'):
            self.assertNotEqual(a['current']['current_' + role], a[role])
            for path in p.CURRENT_PATHS:
                if role == 'derived_files' and path == 'src/frontend/lexer.rs':
                    raw = p.compose_division_lexer(a, (p.REPOSITORY / path).read_bytes())
                    self.assertEqual(next(r for r in a['current']['current_' + role] if r['path'] == path),
                                     {'path': path, 'bytes': len(raw), 'sha256': p.sha(raw)})
                    continue
                if path in p.ARRAY_INSTRUMENTATION_PATHS and (role == 'derived_files' or path != 'src/frontend/project/budget.rs'):
                    raw = p.compose_array_instrumentation(a, path, (p.REPOSITORY / path).read_bytes(), role == 'control_derived_files')
                    self.assertEqual(next(r for r in a['current']['current_' + role] if r['path'] == path),
                                     {'path': path, 'bytes': len(raw), 'sha256': p.sha(raw)})
                    continue
                if role == 'derived_files' and path == 'src/frontend/declaration_index/resource.rs':
                    raw = p.compose_namespace_resource(a, (p.REPOSITORY / path).read_bytes())
                    self.assertEqual(next(r for r in a['current']['current_' + role] if r['path'] == path),
                                     {'path': path, 'bytes': len(raw), 'sha256': p.sha(raw)})
                    continue
                if role == 'derived_files' and path == 'src/frontend/source.rs':
                    self.assertEqual(next(r for r in a['current']['current_' + role] if r['path'] == path),
                                     {'path': path, 'bytes': 15844, 'sha256': '9269b83d0388c8c29bca7d92e7d785ca6dc7c6688cb73aa9631c82e0a258b12d'})
                    continue
                self.assertEqual(next(r for r in a['current']['current_' + role] if r['path'] == path),
                                 next(r['after'] for r in a['current']['source_delta'] if r['path'] == path))

    def test_generated_candidate_binds_the_complete_current_base(self):
        a = p.authority()
        candidate = p.current_candidate(a)
        raw = (json.dumps(candidate, sort_keys=True, indent=2) + '\n').encode()
        self.assertEqual(len(candidate['files']), 400)
        self.assertEqual(p.sha(raw), a['current']['current_candidate_source_manifest_sha256'])
        for role in ('current_derived_files', 'current_control_derived_files'):
            self.assertEqual(next(r for r in a['current'][role] if r['path'] == 'candidate-source-manifest.json'),
                             {'path': 'candidate-source-manifest.json', 'bytes': len(raw), 'sha256': p.sha(raw)})

    def test_coherently_rewritten_authority_rejects_at_trusted_pin(self):
        value = p.read(p.HERE / 'authority.json')
        value['source_delta'] = value['source_delta'][1:]
        with tempfile.TemporaryDirectory(prefix='oxid-current-authority-') as directory:
            root = Path(directory)
            p.write(root / 'authority.json', value)
            with patch.object(p, 'HERE', root):
                with self.assertRaisesRegex(p.Rejected, 'unreviewed current parser authority'):
                    p.authority()

    def mutated_current_rejects(self, transform, message):
        # Inject below the already-verified manifest pin to exercise the separate
        # structural predicates, without granting altered bytes authority.
        original_read = p.read
        manifest_path = p.REPOSITORY / 'tests/fixtures/typed_project_source_binding/current-source.json'
        current = copy.deepcopy(original_read(manifest_path))
        transform(current['files'])
        with patch.object(p, 'read', side_effect=lambda path: current if Path(path) == manifest_path else original_read(path)):
            with self.assertRaisesRegex(p.Rejected, message):
                p.authority()

    def test_duplicate_current_member_rejects(self):
        self.mutated_current_rejects(lambda files: files.__setitem__(-1, files[0]), 'duplicate current member')

    def test_reordered_transition_rejects(self):
        self.mutated_current_rejects(lambda files: files.reverse(), 'unexpected current transition scope')

    def test_deleted_historical_compiler_member_rejects_even_with_same_count(self):
        historical = p.read(p.FROZEN / 'authority.json')
        replacement = next(row for row in historical['original_files'] if row['path'] == '.dockerignore')
        def remove_compiler(files):
            index = next(i for i, row in enumerate(files) if row['path'] == 'src/cli.rs')
            files[index] = replacement
        self.mutated_current_rejects(remove_compiler, 'current transition deletes historical compiler input')

    def test_every_added_and_changed_source_identity_is_bound(self):
        for name in p.CURRENT_PATHS:
            def change_identity(files):
                next(row for row in files if row['path'] == name)['sha256'] = '0' * 64
            with self.subTest(path=name):
                self.mutated_current_rejects(change_identity, 'current transition before/after identities')


class SourceReadCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()
        self.raw = (p.REPOSITORY / 'src/frontend/source.rs').read_bytes()

    def test_exact_composition_preserves_both_hooks_and_only_adds_accessor(self):
        composed = p.compose_source_read(self.a, self.raw)
        self.assertEqual(len(composed), 15844)
        self.assertEqual(p.sha(composed), '9269b83d0388c8c29bca7d92e7d785ca6dc7c6688cb73aa9631c82e0a258b12d')
        addition = (b'    /// Consume a single-source owner after syntax-only candidate validation.\n'
                    b'    /// Moving its text back out avoids a second full formatter output buffer.\n'
                    b'    pub(super) fn into_single_text(mut self) -> String {\n'
                    b'        assert_eq!(self.files.len(), 1, "expected exactly one source");\n'
                    b'        self.files.pop().expect("one source").text\n'
                    b'    }\n\n')
        self.assertEqual(composed.count(addition), 1)
        projected = p.restore_enum_source(self.a, 'src/frontend/source.rs', self.raw)
        self.assertEqual(p.sha(projected), '3574d2e4598fa77b82aeee2ba3457dfdd39652d750771d4c56102aa706c0db3e')
        historical = projected.replace(addition, b'', 1)
        self.assertEqual(p.sha(historical), next(r['sha256'] for r in self.a['original_files'] if r['path'] == 'src/frontend/source.rs'))
        for call in (b'source_read(span.end.saturating_sub(span.start));', b'source_read(self.text.len());'):
            line = b'        crate::frontend::parser::unit4_observer::' + call + b'\n'
            self.assertEqual(composed.count(line), 1)
            composed = composed.replace(line, b'', 1)
        self.assertEqual(composed, self.raw)
        for role in ('derived_files', 'control_derived_files'):
            self.assertEqual(next(row for row in self.a['current']['current_' + role] if row['path'] == 'instrumentation.patch'),
                             next(row for row in self.a[role] if row['path'] == 'instrumentation.patch'))

    def test_exact_current_overlay_records_composed_source_identity(self):
        observer = p.current_overlay('/controlled/source', self.a, False)['instrumentation']
        self.assertEqual(len(observer), 6)
        for old, new in zip(self.a['instrumentation'], observer):
            if old['path'] == 'src/frontend/source.rs':
                self.assertEqual(new, {'path': old['path'],
                                      'before_sha256': '38a69de7b01cc0d8160079e98d266a83af2feb969796417952663b5a2c69d176',
                                      'after_sha256': '9269b83d0388c8c29bca7d92e7d785ca6dc7c6688cb73aa9631c82e0a258b12d'})
            elif old['path'] in (*p.ARRAY_INSTRUMENTATION_PATHS, 'src/frontend/lexer.rs', 'src/frontend/declaration_index/resource.rs'):
                self.assertEqual(new['path'], old['path'])
                self.assertEqual(new['before_sha256'], next(r['sha256'] for r in self.a['current']['current_base_files'] if r['path'] == old['path']))
                self.assertEqual(new['after_sha256'], next(r['sha256'] for r in self.a['current']['current_derived_files'] if r['path'] == old['path']))
            else:
                self.assertEqual(new, old)
        control = p.current_overlay('/controlled/control-source', self.a, True)['instrumentation']
        self.assertEqual([r['path'] for r in control], ['src/frontend/ast.rs', 'src/frontend/parser.rs'])
        for row in control:
            self.assertEqual(row['before_sha256'], next(r['sha256'] for r in self.a['current']['current_base_files'] if r['path'] == row['path']))
            self.assertEqual(row['after_sha256'], next(r['sha256'] for r in self.a['current']['current_control_derived_files'] if r['path'] == row['path']))

    def test_changed_current_source_rejects_before_composition(self):
        for changed in (self.raw + b'// changed\n', self.raw.replace(b'into_single_text', b'into_other_text'),
                        self.raw.replace(b'        &self.text', b'        panic!("changed")')):
            with self.subTest(sha=p.sha(changed)), self.assertRaisesRegex(p.Rejected, 'composition current source identity'):
                p.compose_source_read(self.a, changed)

    def test_coherently_rehashed_source_cannot_change_historical_body(self):
        changed = self.raw + b'// changed historical body\n'
        row = next(row for row in self.a['current']['source_delta'] if row['path'] == 'src/frontend/source.rs')
        row['after'].update(bytes=len(changed), sha256=p.sha(changed))
        with self.assertRaisesRegex(p.Rejected, 'enum transition must recover exact projected source'):
            p.compose_source_read(self.a, changed)

    def test_changed_formatter_patch_rejects_before_composition(self):
        with tempfile.TemporaryDirectory(prefix='oxid-source-read-patch-') as temporary:
            root = Path(temporary)
            relative = self.a['current']['formatter_transition_patch']['path']
            target = root / relative
            target.parent.mkdir(parents=True)
            target.write_bytes((p.REPOSITORY / relative).read_bytes() + b'\n')
            with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                p.compose_source_read(self.a, self.raw)

    def test_every_other_instrumentation_overlap_remains_rejected(self):
        original_read = p.read
        historical_path = p.FROZEN / 'authority.json'
        for role, name, message in (
            ('instrumentation', 'src/frontend/driver.rs', 'transition overlaps instrumentation outside exact current composition'),
            ('control_instrumentation', 'src/frontend/source.rs', 'exact enum control overlap roster')):
            historical = copy.deepcopy(original_read(historical_path))
            historical[role].append({'path': name, 'before_sha256': '0' * 64, 'after_sha256': '1' * 64})
            with self.subTest(role=role), patch.object(p, 'read', side_effect=lambda path: historical if Path(path) == historical_path else original_read(path)):
                with self.assertRaisesRegex(p.Rejected, message):
                    p.authority()

    def test_uninstrumented_or_stale_composed_map_rejects(self):
        original_load = p.load
        for replacement in (next(row for row in self.a['current']['current_base_files'] if row['path'] == 'src/frontend/source.rs'),
                            next(row for row in self.a['derived_files'] if row['path'] == 'src/frontend/source.rs')):
            active = copy.deepcopy(self.a['current'])
            row = next(row for row in active['current_derived_files'] if row['path'] == replacement['path'])
            row.update(replacement)
            # Inject below the immutable authority pin to exercise derivation too.
            with patch.object(p, 'load', side_effect=lambda raw: active if p.sha(raw) == p.AUTHORITY_SHA else original_load(raw)):
                with self.assertRaisesRegex(p.Rejected, 'unapproved current derived map'):
                    p.authority()


class ArrayCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()

    def test_every_composition_rejects_changed_source(self):
        for name in p.ARRAY_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes()
            for control in (False, True):
                if control and name.endswith('/budget.rs'): continue
                with self.subTest(path=name, control=control), self.assertRaisesRegex(p.Rejected, 'composition current array identity'):
                    p.compose_array_instrumentation(self.a, name, raw + b'// changed\n', control)

    def test_unknown_path_and_unapproved_control_overlap_reject(self):
        with self.assertRaisesRegex(p.Rejected, 'unapproved array instrumentation path'):
            p.compose_array_instrumentation(self.a, 'src/frontend/source.rs', b'')
        with self.assertRaisesRegex(p.Rejected, 'unapproved control composition'):
            p.compose_array_instrumentation(self.a, 'src/frontend/project/budget.rs', b'', True)

    def test_coherent_source_rehash_cannot_change_historical_array_body(self):
        for name in p.ARRAY_INSTRUMENTATION_PATHS:
            a = copy.deepcopy(self.a)
            raw = (p.REPOSITORY / name).read_bytes() + b'// changed historical tail\n'
            next(row for row in a['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            message = ('enum transition must recover exact projected source' if name in p.UNARY_INSTRUMENTATION_PATHS
                       else 'division transition must recover exact combined source' if name in p.DIVISION_INSTRUMENTATION_PATHS
                       else 'enum transition must recover exact projected source')
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, message):
                p.compose_array_instrumentation(a, name, raw)

    def test_changed_inverse_patch_and_runner_reject_before_transform(self):
        for field in ('combined_transition_patch', 'source_binding_runner'):
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for key in ('combined_transition_patch', 'source_binding_runner'):
                    relative = self.a['current'][key]['path']
                    target = root / relative; target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/ast.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.compose_array_instrumentation(self.a, 'src/frontend/ast.rs', raw)

    def test_direct_observer_successor_preserves_closed_policies_and_empty_storage(self):
        raw = p.compose_observer_initializer(self.a)
        insertion = (b'            arrays: ArraySyntaxPolicy::Closed,\n'
                     b'            enums: EnumSyntaxPolicy::Closed,\n'
                     b'            storage: enums::SyntaxStorage::default(),\n')
        self.assertEqual(raw.count(insertion), 1)
        self.assertEqual(raw.replace(insertion, b'', 1), (p.FROZEN / 'frozen/helpers/observer.rs').read_bytes())
        for key in ('current_derived_files', 'current_control_derived_files'):
            row = next(r for r in self.a['current'][key] if r['path'] == 'src/frontend/parser/unit4_observer.rs')
            self.assertEqual(row, {'path': row['path'], 'bytes': len(raw), 'sha256': p.sha(raw)})
        mutated = copy.deepcopy(self.a)
        mutated['current']['observer_initializer_adapter']['derived']['sha256'] = '0' * 64
        with self.assertRaisesRegex(p.Rejected, 'unapproved observer initializer successor'):
            p.compose_observer_initializer(mutated)

    def test_plain_or_stale_array_derived_maps_are_never_accepted(self):
        original_load = p.load
        for role in ('derived_files', 'control_derived_files'):
            for name in p.ARRAY_INSTRUMENTATION_PATHS:
                if role == 'control_derived_files' and name.endswith('/budget.rs'): continue
                for source in ('current_base_files', role):
                    active = copy.deepcopy(self.a['current'])
                    rows = active[source] if source == 'current_base_files' else self.a[source]
                    replacement = next(row for row in rows if row['path'] == name)
                    next(row for row in active['current_' + role] if row['path'] == name).update(replacement)
                    with self.subTest(role=role, path=name, source=source), patch.object(p, 'load', side_effect=lambda raw: active if p.sha(raw) == p.AUTHORITY_SHA else original_load(raw)):
                        with self.assertRaisesRegex(p.Rejected, 'unapproved current derived map'):
                            p.authority()


class DivisionCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()

    def test_all_three_division_overlaps_restore_exact_combined_inputs(self):
        before = p.read(p.REPOSITORY / self.a['current']['combined_source_manifest']['path'])
        for name in p.DIVISION_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes()
            restored = p.restore_division_source(self.a, name, raw)
            self.assertEqual({'path': name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                             next(row for row in before['files'] if row['path'] == name))

    def test_lexer_composition_preserves_exactly_two_frozen_hooks(self):
        name = 'src/frontend/lexer.rs'
        raw = (p.REPOSITORY / name).read_bytes()
        composed = p.compose_division_lexer(self.a, raw)
        token = b'        crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());\n'
        eof = b'    crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());\n'
        self.assertEqual(composed.count(token), 1)
        without_token = composed.replace(token, b'', 1)
        self.assertEqual(without_token.count(eof), 1)
        self.assertEqual(without_token.replace(eof, b'', 1), raw)
        self.assertEqual(next(row for row in self.a['current']['current_derived_files'] if row['path'] == name),
                         {'path': name, 'bytes': len(composed), 'sha256': p.sha(composed)})

    def test_changed_and_coherently_rehashed_division_source_rejects(self):
        for name in p.DIVISION_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unapproved tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current division identity'):
                p.restore_division_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            message = ('enum transition must recover exact projected source' if name in p.UNARY_INSTRUMENTATION_PATHS
                       else 'division transition must recover exact combined source')
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, message):
                p.restore_division_source(altered, name, raw)
        with self.assertRaisesRegex(p.Rejected, 'unapproved division instrumentation path'):
            p.restore_division_source(self.a, 'src/frontend/project/budget.rs', b'')

    def test_division_patch_and_combined_manifest_reject_before_transform(self):
        fields = ('source_binding_runner', 'division_transition_patch', 'combined_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for key in fields:
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/lexer.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.compose_division_lexer(self.a, raw)

    def test_uninstrumented_and_historical_lexer_maps_reject(self):
        original_load = p.load
        name = 'src/frontend/lexer.rs'
        for replacement in (next(row for row in self.a['current']['current_base_files'] if row['path'] == name),
                            next(row for row in self.a['derived_files'] if row['path'] == name)):
            active = copy.deepcopy(self.a['current'])
            next(row for row in active['current_derived_files'] if row['path'] == name).update(replacement)
            with patch.object(p, 'load', side_effect=lambda raw: active if p.sha(raw) == p.AUTHORITY_SHA else original_load(raw)):
                with self.assertRaisesRegex(p.Rejected, 'unapproved current derived map'):
                    p.authority()


class SlicesCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()

    def test_slice_overlaps_restore_the_exact_division_predecessor(self):
        before = p.read(p.REPOSITORY / self.a['current']['division_source_manifest']['path'])
        for name in p.SLICES_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes()
            restored = p.restore_slices_source(self.a, name, raw)
            self.assertEqual({'path': name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                             next(row for row in before['files'] if row['path'] == name))

    def test_changed_and_coherently_rehashed_slice_source_rejects(self):
        for name in p.SLICES_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unapproved slice tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current slices identity'):
                p.restore_slices_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'enum transition must recover exact projected source'):
                p.restore_slices_source(altered, name, raw)

    def test_slices_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'slices_transition_patch', 'division_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for key in fields:
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/ast.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_slices_source(self.a, 'src/frontend/ast.rs', raw)

    def test_every_other_slice_overlap_remains_rejected(self):
        for name in ('src/frontend/lexer.rs', 'src/frontend/source.rs', 'src/frontend/project/budget.rs'):
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'unapproved slices instrumentation path'):
                p.restore_slices_source(self.a, name, b'')


class RecordCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()

    def test_record_overlaps_restore_the_exact_slice_predecessor(self):
        before = p.read(p.REPOSITORY / self.a['current']['slices_source_manifest']['path'])
        for name in p.COMPOSITION_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes()
            restored = p.restore_composition_source(self.a, name, raw)
            self.assertEqual({'path': name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                             next(row for row in before['files'] if row['path'] == name))

    def test_changed_and_coherently_rehashed_slice_source_rejects(self):
        for name in p.COMPOSITION_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unapproved slice tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current composition identity'):
                p.restore_composition_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'enum transition must recover exact projected source'):
                p.restore_composition_source(altered, name, raw)

    def test_slices_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'composition_transition_patch', 'slices_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for key in fields:
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/ast.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_composition_source(self.a, 'src/frontend/ast.rs', raw)

    def test_every_other_slice_overlap_remains_rejected(self):
        for name in ('src/frontend/lexer.rs', 'src/frontend/source.rs', 'src/frontend/project/budget.rs'):
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'unapproved composition instrumentation path'):
                p.restore_composition_source(self.a, name, b'')


class UnaryCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()

    def test_unary_overlaps_restore_the_exact_composition_predecessor(self):
        before = p.read(p.REPOSITORY / self.a['current']['composition_source_manifest']['path'])
        for name in p.UNARY_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes()
            restored = p.restore_unary_source(self.a, name, raw)
            self.assertEqual({'path': name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                             next(row for row in before['files'] if row['path'] == name))

    def test_changed_and_coherently_rehashed_unary_source_rejects(self):
        for name in p.UNARY_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unapproved unary tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current unary identity'):
                p.restore_unary_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'enum transition must recover exact projected source'):
                p.restore_unary_source(altered, name, raw)

    def test_unary_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'unary_transition_patch', 'composition_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for key in fields:
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/ast.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_unary_source(self.a, 'src/frontend/ast.rs', raw)

    def test_every_other_unary_overlap_remains_rejected(self):
        for name in ('src/frontend/lexer.rs', 'src/frontend/source.rs', 'src/frontend/project/budget.rs'):
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'unapproved unary instrumentation path'):
                p.restore_unary_source(self.a, name, b'')


class ProjectedCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()

    def test_projected_overlaps_restore_the_exact_unary_predecessor(self):
        before = p.read(p.REPOSITORY / self.a['current']['unary_source_manifest']['path'])
        for name in p.PROJECTED_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes()
            restored = p.restore_projected_source(self.a, name, raw)
            self.assertEqual({'path': name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                             next(row for row in before['files'] if row['path'] == name))

    def test_changed_and_coherently_rehashed_projected_source_rejects(self):
        for name in p.PROJECTED_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unapproved projected tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current projected identity'):
                p.restore_projected_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'enum transition must recover exact projected source'):
                p.restore_projected_source(altered, name, raw)

    def test_projected_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'projected_transition_patch', 'unary_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for key in fields:
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/ast.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_projected_source(self.a, 'src/frontend/ast.rs', raw)

    def test_every_other_projected_overlap_remains_rejected(self):
        for name in ('src/frontend/lexer.rs', 'src/frontend/source.rs', 'src/frontend/project/budget.rs'):
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'unapproved projected instrumentation path'):
                p.restore_projected_source(self.a, name, b'')


class EnumCompositionControls(unittest.TestCase):
    def setUp(self):
        self.a = p.authority()

    def test_exact_enum_overlap_roster_and_projected_predecessors(self):
        self.assertEqual(p.ENUM_INSTRUMENTATION_PATHS, (
            'src/frontend/ast.rs', 'src/frontend/declaration_index/resource.rs',
            'src/frontend/parser.rs', 'src/frontend/project/budget.rs', 'src/frontend/source.rs'))
        before = p.read(p.REPOSITORY / self.a['current']['projected_source_manifest']['path'])
        self.assertEqual(len(before['files']), 201)
        for name in p.ENUM_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes()
            restored = p.restore_enum_source(self.a, name, raw)
            self.assertEqual({'path': name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                             next(row for row in before['files'] if row['path'] == name))

    def test_changed_and_coherently_rehashed_enum_source_rejects(self):
        for name in p.ENUM_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unauthorized enum tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current enum identity'):
                p.restore_enum_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'enum transition must recover exact projected source'):
                p.restore_enum_source(altered, name, raw)

    def test_enum_transition_predecessor_and_runner_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'enum_transition_patch', 'projected_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                for key in fields:
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/ast.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_enum_source(self.a, 'src/frontend/ast.rs', raw)

    def test_unknown_enum_instrumentation_path_rejects(self):
        for name in ('src/frontend/lexer.rs', 'src/frontend/project.rs', 'src/frontend/parser/enums.rs'):
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'unapproved enum instrumentation path'):
                p.restore_enum_source(self.a, name, b'')

    def test_namespace_composition_preserves_both_hooks_and_current_body(self):
        name = 'src/frontend/declaration_index/resource.rs'
        raw = (p.REPOSITORY / name).read_bytes()
        composed = p.compose_namespace_resource(self.a, raw)
        for argument in (b'1', b'units'):
            hook = b'\n        crate::frontend::parser::unit4_observer::namespace_debit(' + argument + b');'
            self.assertEqual(composed.count(hook), 1)
            composed = composed.replace(hook, b'', 1)
        self.assertEqual(composed, raw)
        with self.assertRaisesRegex(p.Rejected, 'composition current enum identity'):
            p.compose_namespace_resource(self.a, raw + b'// changed\n')

    def test_helper_reversal_closed_bridge_and_preserved_unary_gate_label(self):
        helper = p.compose_enum_parser_helper(self.a)
        raw = (p.REPOSITORY / 'src/frontend/parser.rs').read_bytes()
        current = p.compose_array_instrumentation(self.a, 'src/frontend/parser.rs', raw)
        generated = helper.parser_overlay(raw.decode())
        self.assertEqual(current.replace(p.ENUM_NODE_BRIDGE.encode(), b'', 1), generated.encode())
        self.assertEqual(current.count(p.ENUM_NODE_BRIDGE.encode()), 1)
        self.assertIn(b'panic!("enum node route is outside the closed Unit4 parser observation domain")', current)
        self.assertIn(b'self.unit4_node("unary")?;', current)
        self.assertNotIn(b'self.unit4_node("unary_with_prefixes")?;', current)
        control = p.compose_array_instrumentation(self.a, 'src/frontend/parser.rs', raw, True)
        self.assertEqual(control, raw + b'\n#[cfg(test)]\npub(super) mod unit4_observer;\n')
        self.assertIn(b'        EnumSyntaxPolicy::Closed,\n        &mut enums::SyntaxStorage::default(),', raw)
        enum_module = (p.REPOSITORY / 'src/frontend/parser/enums.rs').read_bytes()
        self.assertIn(b'pub(super) fn enum_qualified_ahead(&self) -> bool {\n        if !self.enums_enabled()', enum_module)

    def test_unapproved_helper_descriptor_and_seam_reject(self):
        altered = copy.deepcopy(self.a)
        altered['current']['enum_parser_instrumentation_adapter']['derived_helper']['sha256'] = '0' * 64
        with self.assertRaisesRegex(p.Rejected, 'unapproved enum parser instrumentation adapter'):
            p.compose_enum_parser_helper(altered)
        with patch.object(p, 'ENUM_HELPER_SUBSTITUTIONS', (("unapproved helper seam", "replacement"),)), self.assertRaisesRegex(p.Rejected, 'exact enum parser helper seam'):
            p.compose_enum_parser_helper(self.a)


class EnumStructuralProjectionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.comparator = p.comparator()

    def setUp(self):
        span = {'tag': 'Span', 'file': {'tag': 'SourceFileId', 'items': [0]}, 'start': 0, 'end': 1}
        root = {'tag': 'Program', 'tokens': [], 'functions': [], 'expressions': [], 'records': [],
                'enums': [], 'items': [], 'modules': [], 'imports': [], 'path_segments': [span],
                'paths': [{'tag': 'QualifiedPath', 'span': span, 'segment_start': 0, 'segment_len': 1,
                           'root': {'tag': 'Crate'}}],
                'source': {'tag': 'SourceProvenance', 'text_len': 1,
                           'file': {'tag': 'SourceFileId', 'items': [0]}, 'debug_non_exhaustive': True},
                'project_syntax': True}
        self.rows = [{'ast': {'canonical': root}, 'diagnostics': ['retained sentinel'],
                      'binding': {'case_id': 'synthetic-shape-control'}, 'events': [{'actual': 17}]},
                     {'ast': None, 'diagnostics': ['unchanged failure']}]
        self.root = root
        self.span = span

    def project(self, rows=None):
        with patch.object(p, 'comparator', return_value=self.comparator):
            return p.project_enum_observations(self.a, self.rows if rows is None else rows)

    def test_exact_shape_projection_preserves_raw_and_all_other_fields(self):
        before = copy.deepcopy(self.rows)
        projected, receipt = self.project()
        expected = copy.deepcopy(before)
        root = expected[0]['ast']['canonical']
        del root['enums']
        root['paths'][0]['tag'] = 'AbsolutePath'
        del root['paths'][0]['root']
        self.assertEqual(self.rows, before)
        self.assertEqual(projected, expected)
        self.assertEqual(receipt['changed_row_indices'], [0])
        self.assertEqual((receipt['observations'], receipt['ast_observations'], receipt['qualified_paths']), (2, 1, 1))
        canonical = self.comparator[0].canonical
        self.assertEqual(receipt['original_observations_canonical_sha256'], p.sha(canonical(before)))
        self.assertEqual(receipt['restored_observations_canonical_sha256'], p.sha(canonical(before)))
        self.assertEqual(receipt['projected_observations_canonical_sha256'], p.sha(canonical(expected)))
        self.assertEqual(receipt['adapter'], p.ENUM_AST_SCHEMA_ADAPTER)
        self.assertFalse(receipt['adapter']['semantic_changes_permitted'])
        self.assertFalse(receipt['adapter']['raw_observations_changed'])
        self.assertFalse(receipt['adapter']['semantic_predicate_handlers_changed'])

    def test_nonempty_enum_declarations_and_changed_program_shape_reject(self):
        for mutation in (
            lambda root: root.update(enums=[{'tag': 'EnumDecl'}]),
            lambda root: root.update(enums={}),
            lambda root: root.pop('enums'),
            lambda root: root.update(unexpected=[]),
            lambda root: root.update(tag='AnotherProgram'),
        ):
            rows = copy.deepcopy(self.rows); mutation(rows[0]['ast']['canonical'])
            with self.subTest(mutation=mutation), self.assertRaises(p.Rejected):
                self.project(rows)

    def test_unknown_root_tag_field_and_path_bound_reject(self):
        for mutation in (
            lambda path: path.update(root={'tag': 'LocalType'}),
            lambda path: path.update(root={'tag': 'Crate', 'hidden': True}),
            lambda path: path.update(root='Crate'),
            lambda path: path.pop('root'),
            lambda path: path.update(tag='AbsolutePath'),
            lambda path: path.update(extra=1),
            lambda path: path.update(segment_len=0),
            lambda path: path.update(segment_len=35),
            lambda path: path.update(segment_len=True),
        ):
            rows = copy.deepcopy(self.rows); mutation(rows[0]['ast']['canonical']['paths'][0])
            with self.subTest(mutation=mutation), self.assertRaises(p.Rejected):
                self.project(rows)

    def test_new_value_and_item_carriers_reject_without_semantic_normalization(self):
        for kind in ({'tag': 'QualifiedValue', 'path': {'tag': 'PathId', 'items': [0]}, 'args': None},
                     {'tag': 'QualifiedValue', 'path': {'tag': 'PathId', 'items': [0]},
                      'args': {'tag': 'Some', 'items': [[]]}},
                     {'tag': 'Match', 'scrutinee': self.span, 'arms': []}):
            rows = copy.deepcopy(self.rows)
            rows[0]['ast']['canonical']['expressions'] = [{'tag': 'Expr', 'kind': kind, 'span': self.span}]
            with self.subTest(kind=kind['tag']), self.assertRaisesRegex(p.Rejected, 'nonhistorical AST member'):
                self.project(rows)
        rows = copy.deepcopy(self.rows)
        rows[0]['ast']['canonical']['items'] = [{'tag': 'Enum', 'items': [0]}]
        with self.assertRaisesRegex(p.Rejected, 'nonhistorical AST member'):
            self.project(rows)

    def test_match_statement_and_unknown_nested_fields_reject(self):
        rows = copy.deepcopy(self.rows)
        rows[0]['ast']['canonical']['functions'] = [{
            'tag': 'Function', 'public': None, 'name': self.span, 'params': [],
            'result': {'tag': 'TypeSyntax', 'span': self.span, 'kind': {'tag': 'Unit'}},
            'body': {'tag': 'BodyBlockId', 'items': [0]}, 'end': self.span,
            'blocks': [{'tag': 'BodyBlock', 'span': self.span, 'end': self.span,
                        'body': [{'tag': 'Stmt', 'span': self.span,
                                  'kind': {'tag': 'Match', 'scrutinee': self.span, 'arms': []}}]}]}]
        with self.assertRaisesRegex(p.Rejected, 'nonhistorical AST member'):
            self.project(rows)
        rows = copy.deepcopy(self.rows)
        rows[0]['ast']['canonical']['source']['unexpected'] = True
        with self.assertRaisesRegex(p.Rejected, 'nonhistorical AST member'):
            self.project(rows)

    def test_historical_call_carrier_is_retained_without_conversion(self):
        rows = copy.deepcopy(self.rows)
        expression = {'tag': 'Expr', 'span': self.span,
                      'kind': {'tag': 'Call', 'callee': {'tag': 'Absolute', 'items': [
                          {'tag': 'PathId', 'items': [0]}]}, 'args': []}}
        rows[0]['ast']['canonical']['expressions'] = [expression]
        projected, _ = self.project(rows)
        self.assertEqual(projected[0]['ast']['canonical']['expressions'], [expression])

    def test_schema_adapter_changes_and_double_projection_reject(self):
        altered = copy.deepcopy(self.a)
        altered['current']['enum_ast_schema_adapter']['semantic_changes_permitted'] = True
        with self.assertRaisesRegex(p.Rejected, 'unapproved enum AST schema adapter'):
            p.project_enum_observations(altered, self.rows)
        projected, _ = self.project()
        with self.assertRaisesRegex(p.Rejected, 'exact current Program shape'):
            self.project(projected)

    def test_unrelated_semantic_fields_are_preserved_and_bound(self):
        original, receipt = self.project()
        rows = copy.deepcopy(self.rows)
        rows[0]['events'][0]['actual'] = 18
        projected, changed = self.project(rows)
        self.assertEqual(projected[0]['events'], [{'actual': 18}])
        self.assertNotEqual(receipt['original_observations_canonical_sha256'], changed['original_observations_canonical_sha256'])
        self.assertNotEqual(receipt['projected_observations_canonical_sha256'], changed['projected_observations_canonical_sha256'])
        self.assertEqual(projected[0]['ast'], original[0]['ast'])


class CheckoutControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='oxid-current-parser-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.a = p.authority()
        for row in [*self.a['current_source']['files'], self.a['current']['current_source_manifest']]:
            dest = self.root / row['path']; dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes((p.REPOSITORY / row['path']).read_bytes())
        self.git('init', '-q')
        self.git('add', '.')
        self.git('commit', '-qm', 'Exact current admission fixture')

    def git(self, *args):
        return subprocess.check_output(['/usr/bin/git', '-c', 'user.name=Codex', '-c', 'user.email=codex@local.invalid',
                                        '-c', 'core.autocrlf=false', '-C', str(self.root), *args], stderr=subprocess.STDOUT)

    def rejects_before_git_or_child(self, message):
        with patch.object(p, 'git', side_effect=AssertionError('must reject before Git/tool use')):
            with self.assertRaisesRegex(p.Rejected, message): p.verify_checkout(self.root, self.a)

    def test_exact_current_bodies_and_git_are_admitted(self):
        bound = p.verify_checkout(self.root, self.a)
        self.assertEqual(len(bound['compiler_files']), 182)
        self.assertIs(bound['historical_source_equivalent'], False)
        self.assertIs(bound['current_source_bound'], True)
        self.assertEqual(bound['head'], self.git('rev-parse', 'HEAD').decode().strip())

    def test_changed_groundwork_rejects_before_git_or_child(self):
        path = self.root / p.CURRENT_PATHS[0]; path.write_bytes(path.read_bytes() + b'// changed\n')
        self.rejects_before_git_or_child('file bytes differ')

    def test_missing_new_member_rejects_before_git_or_child(self):
        for name in p.CURRENT_ADDED_PATHS:
            path = self.root / name
            raw = path.read_bytes()
            with self.subTest(path=name):
                path.unlink()
                try:
                    self.rejects_before_git_or_child('missing regular file')
                finally:
                    path.write_bytes(raw)

    def test_extra_compiler_member_rejects_before_git_or_child(self):
        (self.root / 'src/unapproved.rs').write_bytes(b'// not admitted\n')
        self.rejects_before_git_or_child('checkout complete current compiler roster')

    def test_coherently_rehashed_current_manifest_rejects_before_git_or_child(self):
        name = p.CURRENT_PATHS[-1]; path = self.root / name; path.write_bytes(path.read_bytes() + b'// coherent\n')
        manifest_path = self.root / self.a['current']['current_source_manifest']['path']
        value = p.read(manifest_path)
        row = next(r for r in value['files'] if r['path'] == name)
        row.update(bytes=path.stat().st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        p.write(manifest_path, value)
        self.rejects_before_git_or_child('file bytes differ')

    def test_old_historical_body_cannot_substitute(self):
        name = 'src/frontend/ast.rs'
        raw = p.git(p.REPOSITORY, 'show', self.a['base_commit'] + ':' + name)
        (self.root / name).write_bytes(raw)
        self.rejects_before_git_or_child('file bytes differ')

    def test_current_worktree_cannot_hide_wrong_committed_body(self):
        path = self.root / p.CURRENT_PATHS[0]; good = path.read_bytes()
        path.write_bytes(good + b'// committed wrong body\n')
        self.git('add', '.'); self.git('commit', '-qm', 'Wrong fixture Git body')
        path.write_bytes(good)
        with self.assertRaisesRegex(p.Rejected, 'checkout Git current input bytes'):
            p.verify_checkout(self.root, self.a)


if __name__ == '__main__':
    unittest.main(verbosity=2)
