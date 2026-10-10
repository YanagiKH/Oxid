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


def stage_lexer_dispatcher_dependencies(root, active):
    """Supply exact new dependencies before the original artifact corruption."""
    rows = list(active['lexer_reservation'].values())
    p.verify_map(p.REPOSITORY, rows)
    names = [row['path'] for row in rows] + [
        'tests/qualification/lexer_reservation_current/' + name
        for name in ('adapters.py', 'source_transition.py', 'seal.py', 'current.py')
    ] + ['tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py']
    for name in dict.fromkeys(names):
        target = root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((p.REPOSITORY / name).read_bytes())


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
        self.assertEqual([len(a['current'][k]) for k in ('current_base_files', 'current_derived_files', 'current_control_derived_files')], [539, 542, 542])
        self.assertEqual(len(a['current_source']['files']), 376)
        self.assertEqual(len(p.compiler_map(a)), 291)
        self.assertNotEqual(a['candidate_source_manifest_sha256'], a['current']['current_candidate_source_manifest_sha256'])
        self.assertEqual([r['path'] for r in a['current']['source_delta']], list(p.CURRENT_PATHS))
        self.assertEqual(len(a['current']['source_delta']), 341)
        self.assertEqual([r['path'] for r in a['current']['source_delta'] if r['before'] is None], ['fixtures/typed-record-composition-samples/main.ox', 'fixtures/typed-record-composition-samples/model.ox', 'fixtures/typed-record-composition-samples/ops.ox', 'src/frontend/builtin_catalog.rs', 'src/frontend/declaration_index/builtin_tests.rs', 'src/frontend/declaration_index/enum_query_tests.rs', 'src/frontend/declaration_index/enum_tests.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/u8_integration_tests.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/enum_public_tests.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/format/enum_candidate_tests.rs', 'src/frontend/format/resource_tests.rs', 'src/frontend/format_cli.rs', 'src/frontend/hir_producer.rs', 'src/frontend/hir_producer/bundle.rs', 'src/frontend/hir_producer/supervisor.rs', 'src/frontend/hir_protocol.rs', 'src/frontend/lexical_provider.rs', 'src/frontend/lexical_provider/bundle.rs', 'src/frontend/lexical_provider/supervisor.rs', 'src/frontend/lexical_provider/tests.rs', 'src/frontend/lexical_provider/wire.rs', 'src/frontend/oir/execute_measurement.rs', 'src/frontend/oir/lower_measurement.rs', 'src/frontend/oir/native_emit_cost.rs', 'src/frontend/oir/native_emit_observation.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/native_private_emit.rs', 'src/frontend/oir/native_private_emit_tests.rs', 'src/frontend/oir/native_scalar_resource.rs', 'src/frontend/oir/native_scalar_resource_proof.md', 'src/frontend/oir/native_u8_tests.rs', 'src/frontend/oir/negation_raw_tests.rs', 'src/frontend/oir/owned/array_native_resource_tests.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_observe.rs', 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/builtin_descriptor_tests.rs', 'src/frontend/oir/owned/builtin_input_fixtures.rs', 'src/frontend/oir/owned/builtin_input_native_tests.rs', 'src/frontend/oir/owned/builtin_input_tests.rs', 'src/frontend/oir/owned/builtin_origin_tests.rs', 'src/frontend/oir/owned/builtin_output_descriptor_tests.rs', 'src/frontend/oir/owned/builtin_output_fixtures.rs', 'src/frontend/oir/owned/builtin_output_native_tests.rs', 'src/frontend/oir/owned/builtin_output_process_tests.rs', 'src/frontend/oir/owned/builtin_output_reference_tests.rs', 'src/frontend/oir/owned/builtins.rs', 'src/frontend/oir/owned/byte_storage_codec_tests.rs', 'src/frontend/oir/owned/byte_storage_native_tests.rs', 'src/frontend/oir/owned/byte_storage_reference_tests.rs', 'src/frontend/oir/owned/composition_native_tests.rs', 'src/frontend/oir/owned/composition_reference_tests.rs', 'src/frontend/oir/owned/composition_verifier_tests.rs', 'src/frontend/oir/owned/enum_admission_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_formatter_allocation_tests.rs', 'src/frontend/oir/owned/enum_index_allocation_tests.rs', 'src/frontend/oir/owned/enum_layout_tests.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/enum_parser_allocation_tests.rs', 'src/frontend/oir/owned/enum_query_allocation_tests.rs', 'src/frontend/oir/owned/enum_reference_tests.rs', 'src/frontend/oir/owned/input.rs', 'src/frontend/oir/owned/native_inventory_admission_tests.rs', 'src/frontend/oir/owned/native_inventory_tests.rs', 'src/frontend/oir/owned/native_storage.rs', 'src/frontend/oir/owned/native_storage_tests.rs', 'src/frontend/oir/owned/negation_raw_tests.rs', 'src/frontend/oir/owned/output.rs', 'src/frontend/oir/owned/process.rs', 'src/frontend/oir/owned/projected_slice_native_tests.rs', 'src/frontend/oir/owned/reviewer_array_observer_tests.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/slice_native_tests.rs', 'src/frontend/oir/owned/source/array_consumer_tests.rs', 'src/frontend/oir/owned/source/array_pipeline.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/array_pipeline_tests.rs', 'src/frontend/oir/owned/source/array_pipeline_transport.rs', 'src/frontend/oir/owned/source/array_type_controls.rs', 'src/frontend/oir/owned/source/array_types_tests.rs', 'src/frontend/oir/owned/source/builtin_lower.rs', 'src/frontend/oir/owned/source/builtin_signature_tests.rs', 'src/frontend/oir/owned/source/builtin_source_tests.rs', 'src/frontend/oir/owned/source/byte_storage_association_tests.rs', 'src/frontend/oir/owned/source/byte_storage_authority_tests.rs', 'src/frontend/oir/owned/source/byte_storage_fuel_tests.rs', 'src/frontend/oir/owned/source/byte_storage_native_fixture.rs', 'src/frontend/oir/owned/source/byte_storage_raw_tests.rs', 'src/frontend/oir/owned/source/byte_storage_resources.rs', 'src/frontend/oir/owned/source/byte_storage_tests.rs', 'src/frontend/oir/owned/source/byte_storage_trust_tests.rs', 'src/frontend/oir/owned/source/byte_storage_type_tests.rs', 'src/frontend/oir/owned/source/enum_native_source_tests.rs', 'src/frontend/oir/owned/source/enum_storage_failure_tests.rs', 'src/frontend/oir/owned/source/enum_type_tests.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/hir_budget_tests.rs', 'src/frontend/oir/owned/source/output_lower_tests.rs', 'src/frontend/oir/owned/source/output_source_tests.rs', 'src/frontend/oir/owned/source/output_typing_tests.rs', 'src/frontend/oir/owned/source/projected_slice_raw_tests.rs', 'src/frontend/oir/owned/source/resolver_enum_tests.rs', 'src/frontend/oir/owned/source/resolver_inventory_tests.rs', 'src/frontend/oir/owned/source/resolver_paid_tests.rs', 'src/frontend/oir/owned/source/resolver_storage.rs', 'src/frontend/oir/owned/source/resolver_storage_tests.rs', 'src/frontend/oir/owned/source/slice_raw_tests.rs', 'src/frontend/oir/owned/source/slice_tests.rs', 'src/frontend/oir/owned/source/type_storage.rs', 'src/frontend/oir/owned/source/type_storage_tests.rs', 'src/frontend/oir/owned/source/u8_resources.rs', 'src/frontend/oir/owned/source/u8_tests.rs', 'src/frontend/oir/owned/u8_tests.rs', 'src/frontend/oir/owned_types/array_tests.rs', 'src/frontend/oir/owned_types/byte_storage_tests.rs', 'src/frontend/oir/owned_types/composition_tests.rs', 'src/frontend/oir/owned_types/enum_integration_tests.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/owned_types/u8_tests.rs', 'src/frontend/oir/source/conversion_seen.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/allocation.rs', 'src/frontend/oir/source/hir_import/allocation/tests.rs', 'src/frontend/oir/source/hir_import/ast_compare.rs', 'src/frontend/oir/source/hir_import/ast_compare/tests.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/cleanup_controls.rs', 'src/frontend/oir/source/hir_import/candidate/emit_entry_tests.rs', 'src/frontend/oir/source/hir_import/candidate/emit_terminal.rs', 'src/frontend/oir/source/hir_import/candidate/resolution_controls.rs', 'src/frontend/oir/source/hir_import/candidate/run_entry_tests.rs', 'src/frontend/oir/source/hir_import/candidate/tests.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/hir_import/candidate/verify_terminal.rs', 'src/frontend/oir/source/hir_import/emit_failure_tests.rs', 'src/frontend/oir/source/hir_import/emit_measurements.rs', 'src/frontend/oir/source/hir_import/emit_native_tests.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/frontend/oir/source/hir_import/emit_tests.rs', 'src/frontend/oir/source/hir_import/leaf.rs', 'src/frontend/oir/source/hir_import/pass_measurements.rs', 'src/frontend/oir/source/hir_import/producer_diagnostic.rs', 'src/frontend/oir/source/hir_import/producer_diagnostic/tests.rs', 'src/frontend/oir/source/hir_import/public_facade.rs', 'src/frontend/oir/source/hir_import/run_execution_tests.rs', 'src/frontend/oir/source/hir_import/run_measurements.rs', 'src/frontend/oir/source/hir_import/tests.rs', 'src/frontend/oir/source/hir_import/u8_resource_successor.rs', 'src/frontend/oir/source/hir_import/verify_diagnostic_tests.rs', 'src/frontend/oir/source/hir_import/verify_fact_tests.rs', 'src/frontend/oir/source/hir_import/verify_tests.rs', 'src/frontend/oir/source/u8_association_resource_proof.md', 'src/frontend/oir/u8_association_tests.rs', 'src/frontend/oir/u8_carrier_tests.rs', 'src/frontend/oir/u8_execute_tests.rs', 'src/frontend/oir/u8_tests.rs', 'src/frontend/oir/unary_source_tests.rs', 'src/frontend/oir/verify_measurement.rs', 'src/frontend/parser/array_syntax_tests.rs', 'src/frontend/parser/arrays.rs', 'src/frontend/parser/builtin_tests.rs', 'src/frontend/parser/conversions.rs', 'src/frontend/parser/enum_syntax_tests.rs', 'src/frontend/parser/enums.rs', 'src/frontend/parser/u8_syntax_tests.rs', 'src/frontend/project/array_syntax_tests.rs', 'src/frontend/project/budget_real_null_observer.rs', 'src/frontend/project/budget_string_null_tests.rs', 'src/frontend/project/builtin_tests.rs', 'src/frontend/project/enum_carrier_tests.rs', 'src/frontend/project/enum_index_tests.rs', 'src/frontend/stdin_public_tests.rs', 'src/frontend/typeck_measurement.rs', 'tests/fixtures/bounded_enum_scanner/main.ox', 'tests/fixtures/bounded_enum_scanner/scanner.ox', 'tests/fixtures/checked_hir_import/public-source.txt', 'tests/fixtures/checked_hir_import/public-success.bin', 'tests/fixtures/checked_hir_import/rich-source.txt', 'tests/fixtures/checked_hir_import/rich-success.bin', 'tests/fixtures/checked_hir_import/scalar-arithmetic-source.txt', 'tests/fixtures/checked_hir_import/scalar-arithmetic-success.bin', 'tests/fixtures/checked_hir_import/scalar-assignment-source.txt', 'tests/fixtures/checked_hir_import/scalar-assignment-success.bin', 'tests/fixtures/checked_hir_import/scalar-boolean-source.txt', 'tests/fixtures/checked_hir_import/scalar-boolean-success.bin', 'tests/fixtures/checked_hir_import/scalar-comparison-source.txt', 'tests/fixtures/checked_hir_import/scalar-comparison-success.bin', 'tests/fixtures/checked_hir_import/scalar-loop-source.txt', 'tests/fixtures/checked_hir_import/scalar-loop-success.bin', 'tests/fixtures/checked_hir_import/scalar-unit-source.txt', 'tests/fixtures/checked_hir_import/scalar-unit-success.bin', 'tests/fixtures/checked_hir_import/synthetic-division-source.txt', 'tests/fixtures/checked_hir_import/synthetic-division-success.bin', 'tests/fixtures/checked_hir_import/synthetic-overflow-source.txt', 'tests/fixtures/checked_hir_import/synthetic-overflow-success.bin', 'tests/fixtures/checked_hir_import_v2/source-255.txt', 'tests/fixtures/checked_hir_import_v2/success-255.bin', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-call-context-excluded/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-no-context/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-nonzero-annotation/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-reassignment-context-excluded/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-return-context-excluded/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-scalar-context/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/grouped-complete-access-and-index/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-first-heterogeneous-element/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/nested-empty-does-not-inherit-context/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-array-bad-index/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-base-kind-before-index-kind/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-internal-type-first/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-literal-range-before-base-type/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-resolution-before-base-type/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-unknown-base-first/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-0/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-1/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-0/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-1/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-1/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/structural-identities-pairwise-distinct/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/utf8-read-primary/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/whole-program-resolution-before-earlier-function-type/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-base-kind-before-index-kind/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-both-subtree-errors-rhs-wins/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-element-before-mutability/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-internal-type-before-base/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-kind-before-rhs-element/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-element/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-index/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-owner-mutability-last/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-base-before-both-operands/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-rhs-before-index/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-rhs-internal-type-first/main.ox', 'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-whole-resolution-before-rhs-typing/main.ox', 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-array-free/main.ox', 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox', 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-record-only/main.ox', 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox', 'tests/fixtures/producer_diagnostic/duplicate-source.txt', 'tests/fixtures/producer_diagnostic/duplicate.bin', 'tests/fixtures/producer_diagnostic/end255-source.txt', 'tests/fixtures/producer_diagnostic/end255.bin', 'tests/fixtures/producer_diagnostic/multiple-source.txt', 'tests/fixtures/producer_diagnostic/multiple.bin', 'tests/fixtures/producer_diagnostic/unknown-type-source.txt', 'tests/fixtures/producer_diagnostic/unknown-type.bin', 'tests/typed_record_composition.rs'])
        self.assertEqual(sum(r['before'] is not None for r in a['current']['source_delta']), 85)
        self.assertEqual(a['current']['reviewed_source_head'], 'c8e9a72afd9866f32b96f98ae24f61390039f421')
        self.assertEqual(a['current']['source_only_tree'], '9b35515f096b5619d4d5b3d4b0cb88ea2ccf2c37')

    def test_native_storage_transition_rejects_parser_instrumentation_overlap(self):
        original_read = p.read
        path = p.REPOSITORY / 'tests/fixtures/typed_project_source_binding/native-storage-authority.json'
        original = original_read(path)
        for name in ('src/frontend/parser.rs', 'src/frontend/lexer.rs'):
            altered = copy.deepcopy(original)
            altered['transition_paths'].append(name)
            with self.subTest(path=name), patch.object(
                    p, 'read', side_effect=lambda candidate: altered if Path(candidate) == path else original_read(candidate)):
                with self.assertRaisesRegex(p.Rejected, 'native storage transition must not overlap parser instrumentation'):
                    p.authority()

    def test_native_inventory_transition_rejects_parser_instrumentation_overlap(self):
        original_read = p.read
        path = p.REPOSITORY / 'tests/fixtures/typed_project_source_binding/native-inventory-authority.json'
        original = original_read(path)
        for name in ('src/frontend/parser.rs', 'src/frontend/lexer.rs'):
            altered = copy.deepcopy(original)
            altered['transition_paths'].append(name)
            with self.subTest(path=name), patch.object(
                    p, 'read', side_effect=lambda candidate: altered if Path(candidate) == path else original_read(candidate)):
                with self.assertRaisesRegex(p.Rejected, 'native inventory transition must not overlap parser instrumentation'):
                    p.authority()

    def test_native_inventory_retains_separate_storage_source_authority(self):
        original_read = p.read
        root = p.REPOSITORY / 'tests/fixtures/typed_project_source_binding'
        inventory_path = root / 'native-inventory-authority.json'
        storage_path = root / 'native-storage-source.json'
        current = original_read(root / 'current-source.json')
        altered = copy.deepcopy(original_read(inventory_path))
        altered['native_storage_source_sha256'] = p.sha((root / 'current-source.json').read_bytes())
        with patch.object(p, 'read', side_effect=lambda candidate: altered if Path(candidate) == inventory_path else original_read(candidate)):
            with self.assertRaisesRegex(p.Rejected, 'native inventory exact storage predecessor'):
                p.authority()
        with patch.object(p, 'read', side_effect=lambda candidate: current if Path(candidate) == storage_path else original_read(candidate)):
            with self.assertRaisesRegex(p.Rejected, 'complete retained native storage source count'):
                p.authority()

    def test_copied_algorithms_have_only_reviewed_change_boundaries(self):
        old_text = (p.FROZEN / 'portable.py').read_text()
        new_text = Path(p.__file__).read_text()
        functions = lambda text: {n.name: ast.get_source_segment(text, n) for n in ast.parse(text).body if isinstance(n, ast.FunctionDef)}
        old, new = functions(old_text), functions(new_text)
        allowed = {'verify_build', 'cargo_cache', 'authority', 'compiler_map', 'verify_checkout', 'prepare', 'verify_overlay',
                   'session_at', 'verify_cargo', 'collect', 'comparator', 'effective_authority', 'compare', 'main'}
        self.assertEqual(set(new) - set(old), {'lexer_module', 'lexer_api', 'lexer_predecessor_active', 'restore_lexer_source', 'lexer_predecessor_body', 'compose_current_lexer', 'byte_storage_predecessor_active', 'restore_byte_storage_source', 'byte_storage_predecessor_body', 'verify_u8_cross_host_parser_predecessor', 'restore_u8_cross_host_source', 'u8_policy_module', 'u8_binding_api', 'u8_predecessor_active', 'restore_u8_source', 'validate_u8_transition', 'u8_predecessor_body', 'compose_u8_closed_policy', 'restore_cache_admission_source', 'validate_cache_admission_transition', 'restore_cache_preservation_source', 'validate_cache_preservation_transition', 'restore_package_integrity_source', 'validate_package_integrity_transition', 'restore_lexical_provider_source', 'validate_lexical_provider_transition', 'restore_producer_diagnostic_source', 'validate_producer_diagnostic_transition', 'validate_current_dependencies', 'dependency_files', 'restore_frontend_v2_source', 'validate_frontend_v2_transition', 'restore_hir_producer_source', 'validate_hir_producer_transition', 'compose_source_read', 'compose_array_instrumentation', 'restore_division_source', 'restore_slices_source', 'restore_composition_source', 'restore_unary_source', 'restore_projected_source', 'restore_enum_source', 'restore_stdin_source', 'restore_stdout_source', 'restore_hir_import_source', 'compose_namespace_resource', 'compose_enum_parser_helper', 'project_enum_observations', 'compose_division_lexer', 'compose_observer_initializer', 'current_candidate', 'current_overlay', 'verify_transition_records', 'verify_historical_overlay', 'current_parser_contract'})
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
                if path == 'src/frontend/parser/conversions.rs':
                    raw = p.compose_u8_closed_policy(a, path, (p.REPOSITORY / path).read_bytes())
                    self.assertEqual(next(r for r in a['current']['current_' + role] if r['path'] == path), {'path': path, 'bytes': len(raw), 'sha256': p.sha(raw)})
                    continue
                if role == 'derived_files' and path == 'src/frontend/lexer.rs':
                    raw = p.compose_current_lexer(a, (p.REPOSITORY / path).read_bytes())
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
        self.assertEqual(len(candidate['files']), 539)
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
            with self.assertRaisesRegex(p.Rejected, 'complete admitted lexer source'):
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


class CurrentDependencyControls(unittest.TestCase):
    def test_exact_current_closure_preserves_historical_dependencies(self):
        a = p.authority()
        closure = a['current']['current_dependency_closure']
        self.assertEqual([(r['name'], r['version']) for r in closure['packages']], [
            ('block-buffer', '0.10.4'), ('cc', '1.4.0'), ('cfg-if', '1.0.5'),
            ('cpufeatures', '0.2.17'), ('crypto-common', '0.1.7'), ('digest', '0.10.7'),
            ('find-msvc-tools', '0.1.9'), ('generic-array', '0.14.7'), ('libc', '0.2.177'),
            ('sha2', '0.10.9'), ('shlex', '2.0.1'), ('typenum', '1.20.1'), ('version_check', '0.9.5')])
        self.assertEqual(a['dependency_files'], p.read(p.FROZEN / 'authority.json')['dependency_files'])
        self.assertEqual(len(a['dependency_files']), 64)
        self.assertEqual(len(closure['additional_files']), 384)
        self.assertEqual(len(p.dependency_files(a)), 448)

    def test_lock_or_archive_identity_changes_reject(self):
        a = p.authority()
        for field in ('cargo_lock', 'packages'):
            active = copy.deepcopy(a['current'])
            if field == 'cargo_lock':
                active['current_dependency_closure'][field]['sha256'] = '0' * 64
            else:
                active['current_dependency_closure'][field][0]['checksum'] = '0' * 64
            with self.subTest(field=field), self.assertRaises(p.Rejected):
                p.validate_current_dependencies(active, a['current_source'], a)

    def fixture(self, root):
        source = root / 'source'; source.mkdir()
        records = []
        for name, raw in [('registry/cache/index.crates.io-1949cf8c6b5b557f/fake-1.0.0.crate', b'archive'),
                          ('registry/src/index.crates.io-1949cf8c6b5b557f/fake-1.0.0/src/lib.rs', b'source')]:
            path = source / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(raw)
            records.append({'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)})
        for name in ('config.json', '.cache/fa/ke/fake'):
            path = source / 'registry/index/index.crates.io-1949cf8c6b5b557f' / name
            path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b'index')
        return source, {'dependency_files': records[:1], 'current': {'current_dependency_closure': {
            'additional_files': records[1:], 'packages': [{'name': 'fake'}]}}}

    def test_cache_copies_only_closed_files_and_required_index(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); source, a = self.fixture(root)
            for name in ('credentials.toml', 'config.toml', 'registry/index/unrelated/secret'):
                path = source / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(b'never copy')
            dest = root / 'copied'; p.cargo_cache(source, dest, a)
            self.assertEqual(sorted(x.relative_to(dest).as_posix() for x in dest.rglob('*') if x.is_file()), sorted([
                *(row['path'] for row in p.dependency_files(a)),
                'registry/index/index.crates.io-1949cf8c6b5b557f/config.json',
                'registry/index/index.crates.io-1949cf8c6b5b557f/.cache/fa/ke/fake']))

    def test_cache_rejects_missing_or_changed_current_dependency(self):
        for missing in (False, True):
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as directory:
                root = Path(directory); source, a = self.fixture(root)
                path = source / p.dependency_files(a)[-1]['path']
                if missing: path.unlink()
                else: path.write_bytes(b'changed')
                with self.assertRaises(p.Rejected): p.cargo_cache(source, root / 'copied', a)
                self.assertFalse((root / 'copied').exists())

    def test_cache_rejects_symlink_index(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); source, a = self.fixture(root)
            path = source / 'registry/index/index.crates.io-1949cf8c6b5b557f/config.json'
            path.unlink(); path.symlink_to(source / p.dependency_files(a)[0]['path'])
            with self.assertRaisesRegex(p.Rejected, 'symlink'): p.cargo_cache(source, root / 'copied', a)


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
        with self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
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
            ('instrumentation', 'src/frontend/driver.rs', 'exact u8 instrumentation overlap roster'),
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
                with self.subTest(path=name, control=control), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity' if name.endswith('/budget.rs') else 'composition current array identity'):
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
            message = (('exact current byte storage composition body') if name in p.UNARY_INSTRUMENTATION_PATHS
                       else 'division transition must recover exact combined source' if name in p.DIVISION_INSTRUMENTATION_PATHS
                       else 'hir_import transition must recover exact native inventory source' if name in p.HIR_IMPORT_INSTRUMENTATION_PATHS
                       else 'enum transition must recover exact projected source')
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
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
                     b'            std_imports: StdImportPolicy::Closed,\n'
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
        previous, raw = p.lexer_predecessor_body(self.a, name, raw)
        composed = p.compose_division_lexer(previous, raw)
        token = b'        crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());\n'
        eof = b'    crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());\n'
        self.assertEqual(composed.count(token), 1)
        without_token = composed.replace(token, b'', 1)
        self.assertEqual(without_token.count(eof), 1)
        self.assertEqual(without_token.replace(eof, b'', 1), raw)
        self.assertEqual(next(row for row in previous['current']['current_derived_files'] if row['path'] == name),
                         {'path': name, 'bytes': len(composed), 'sha256': p.sha(composed)})

    def test_changed_and_coherently_rehashed_division_source_rejects(self):
        for name in p.DIVISION_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unapproved tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current division identity'):
                p.restore_division_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            message = (('exact current byte storage composition body') if name in p.UNARY_INSTRUMENTATION_PATHS
                       else 'division transition must recover exact combined source')
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
                p.restore_division_source(altered, name, raw)
        with self.assertRaisesRegex(p.Rejected, 'unapproved division instrumentation path'):
            p.restore_division_source(self.a, 'src/frontend/project/budget.rs', b'')


    def test_division_patch_and_combined_manifest_reject_before_transform(self):
        fields = ('source_binding_runner', 'division_transition_patch', 'combined_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
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
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
                p.restore_slices_source(altered, name, raw)


    def test_slices_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'slices_transition_patch', 'division_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
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
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
                p.restore_composition_source(altered, name, raw)


    def test_slices_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'composition_transition_patch', 'slices_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
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
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
                p.restore_unary_source(altered, name, raw)


    def test_unary_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'unary_transition_patch', 'composition_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
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
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
                p.restore_projected_source(altered, name, raw)


    def test_projected_patch_and_predecessor_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'projected_transition_patch', 'unary_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
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


class StdoutCompositionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.name = 'src/frontend/parser.rs'
        cls.raw = (p.REPOSITORY / cls.name).read_bytes()

    def test_exact_stdout_overlap_restores_retained_stdin_before_enum(self):
        self.assertEqual(p.STDOUT_INSTRUMENTATION_PATHS, ('src/frontend/parser.rs',))
        source = self.a['current']['stdin_source_manifest']
        self.assertEqual(source['sha256'], 'bad88720c3002658bbc85de8cc50f63d88186df2871ee5a03ea8a7da0722d13f')
        self.assertEqual(source['bytes'], 48300)
        before = p.read(p.REPOSITORY / source['path'])
        self.assertEqual(len(before['files']), 252)
        restored = p.restore_stdout_source(self.a, self.name, self.raw)
        self.assertEqual({'path': self.name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                         next(row for row in before['files'] if row['path'] == self.name))
        enum = p.read(p.REPOSITORY / self.a['current']['enum_source_manifest']['path'])
        restored_enum = p.restore_stdin_source(self.a, self.name, self.raw)
        self.assertEqual({'path': self.name, 'bytes': len(restored_enum), 'sha256': p.sha(restored_enum)},
                         next(row for row in enum['files'] if row['path'] == self.name))

    def test_source_authorities_keep_distinct_checkpoint_roles(self):
        current = self.a['current']
        stdout = p.read(p.REPOSITORY / current['stdout_authority']['path'])
        stdin = p.read(p.REPOSITORY / current['stdin_authority']['path'])
        self.assertEqual(stdout['current_source_sha256'], current['stdout_source_manifest']['sha256'])
        inventory = p.read(p.REPOSITORY / current['native_inventory_authority']['path'])
        storage_source = p.read(p.REPOSITORY / current['native_storage_source_manifest']['path'])
        native_storage = p.read(p.REPOSITORY / current['native_storage_authority']['path'])
        inventory_source = p.read(p.REPOSITORY / current['native_inventory_source_manifest']['path'])
        self.assertEqual(inventory['reviewed_source_head'], inventory_source['reviewed_source_head'])
        self.assertNotEqual(inventory['reviewed_source_head'], current['reviewed_source_head'])
        self.assertEqual(inventory['current_source_sha256'], current['native_inventory_source_manifest']['sha256'])
        self.assertEqual(inventory['native_storage_source_sha256'], current['native_storage_source_manifest']['sha256'])
        self.assertEqual(native_storage['reviewed_source_head'], storage_source['reviewed_source_head'])
        self.assertEqual(native_storage['current_source_sha256'], current['native_storage_source_manifest']['sha256'])
        self.assertNotEqual(storage_source['reviewed_source_head'], current['reviewed_source_head'])
        self.assertEqual(native_storage['stdout_source_sha256'], current['stdout_source_manifest']['sha256'])
        self.assertEqual(native_storage['base_head'], stdout['reviewed_source_head'])
        self.assertNotEqual(stdout['reviewed_source_head'], current['reviewed_source_head'])
        for field in ('instrumentation', 'control_instrumentation'):
            self.assertFalse(set(inventory['transition_paths']).intersection(row['path'] for row in self.a[field]))
            self.assertFalse(set(native_storage['transition_paths']).intersection(row['path'] for row in self.a[field]))
        self.assertEqual(stdin['current_source_sha256'], current['stdin_source_manifest']['sha256'])
        self.assertEqual(stdin['reviewed_source_head'], 'c1d73740268d64d4e908ad86ed9dabaa48dd1c23')
        self.assertNotEqual(stdout['reviewed_source_head'], stdin['reviewed_source_head'])

    def test_changed_and_coherently_rehashed_stdout_source_rejects(self):
        raw = self.raw + b'// unauthorized stdout tail\n'
        with self.assertRaisesRegex(p.Rejected, 'composition current stdout identity'):
            p.restore_stdout_source(self.a, self.name, raw)
        altered = copy.deepcopy(self.a)
        next(row for row in altered['current']['source_delta'] if row['path'] == self.name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
        with self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
            p.restore_stdout_source(altered, self.name, raw)


    def test_double_inverse_and_wrong_paths_reject(self):
        restored = p.restore_stdout_source(self.a, self.name, self.raw)
        with self.assertRaisesRegex(p.Rejected, 'composition current stdout identity'):
            p.restore_stdout_source(self.a, self.name, restored)
        for name in ('src/frontend/ast.rs', 'src/frontend/lexer.rs', 'src/frontend/parser/enums.rs'):
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'unapproved stdout instrumentation path'):
                p.restore_stdout_source(self.a, name, b'')


    def test_transition_predecessor_and_runner_drift_reject_before_transform(self):
        fields = ('source_binding_runner', 'stdout_transition_patch', 'stdin_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_stdout_source(self.a, self.name, self.raw)

    def test_forged_stdin_predecessor_identity_rejects(self):
        altered = copy.deepcopy(self.a)
        altered['current']['stdin_source_manifest']['sha256'] = '0' * 64
        with self.assertRaisesRegex(p.Rejected, 'lexer changes unrelated parser authority'):
            p.restore_stdout_source(altered, self.name, self.raw)

    def test_missing_duplicate_and_wrong_patch_section_reject(self):
        fields = ('source_binding_runner', 'stdout_transition_patch', 'stdin_source_manifest')
        transition = self.a['current']['stdout_transition_patch']
        previous, previous_raw = p.u8_predecessor_body(self.a, self.name, self.raw)
        original = (p.REPOSITORY / transition['path']).read_bytes()
        prefix = ('a/' + self.name + ' b/' + self.name + '\n').encode()
        parts = original.split(b'diff --git ')[1:]
        selected = [b'diff --git ' + part for part in parts if part.startswith(prefix)]
        self.assertEqual(len(selected), 1)
        mutations = {
            'missing': b''.join(b'diff --git ' + part for part in parts if not part.startswith(prefix)),
            'duplicate': original + selected[0],
            'wrong-path': original.replace(prefix, b'a/src/frontend/wrong.rs b/src/frontend/wrong.rs\n', 1),
        }
        for label, raw in mutations.items():
            with self.subTest(mutation=label), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                altered = copy.deepcopy(previous)
                for field in fields:
                    row = altered['current'][field]
                    target = root / row['path']
                    target.parent.mkdir(parents=True, exist_ok=True)
                    body = raw if field == 'stdout_transition_patch' else (p.REPOSITORY / row['path']).read_bytes()
                    target.write_bytes(body)
                    if field == 'stdout_transition_patch':
                        row.update(bytes=len(body), sha256=p.sha(body))
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'exact stdout instrumentation source section'):
                    p.restore_stdout_source(altered, self.name, previous_raw)


class StdinCompositionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()

    def test_exact_stdin_overlap_roster_and_enum_predecessors(self):
        self.assertEqual(p.STDIN_INSTRUMENTATION_PATHS,
                         ('src/frontend/ast.rs', 'src/frontend/parser.rs'))
        before = p.read(p.REPOSITORY / self.a['current']['enum_source_manifest']['path'])
        self.assertEqual(len(before['files']), 237)
        for name in p.STDIN_INSTRUMENTATION_PATHS:
            restored = p.restore_stdin_source(self.a, name, (p.REPOSITORY / name).read_bytes())
            self.assertEqual({'path': name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                             next(row for row in before['files'] if row['path'] == name))

    def test_changed_and_coherently_rehashed_stdin_source_rejects(self):
        for name in p.STDIN_INSTRUMENTATION_PATHS:
            raw = (p.REPOSITORY / name).read_bytes() + b'// unauthorized stdin tail\n'
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'composition current stdin identity'):
                p.restore_stdin_source(self.a, name, raw)
            altered = copy.deepcopy(self.a)
            next(row for row in altered['current']['source_delta'] if row['path'] == name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
                p.restore_stdin_source(altered, name, raw)


    def test_stdin_transition_predecessor_and_runner_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'stdin_transition_patch', 'enum_source_manifest', 'stdin_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                raw = (p.REPOSITORY / 'src/frontend/ast.rs').read_bytes()
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_stdin_source(self.a, 'src/frontend/ast.rs', raw)

    def test_unknown_stdin_instrumentation_path_rejects(self):
        with self.assertRaisesRegex(p.Rejected, 'unapproved stdin instrumentation path'):
            p.restore_stdin_source(self.a, 'src/frontend/project/budget.rs', b'')


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
            with self.subTest(path=name), self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
                p.restore_enum_source(altered, name, raw)


    def test_enum_transition_predecessor_and_runner_identity_reject_before_transform(self):
        fields = ('source_binding_runner', 'enum_transition_patch', 'projected_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
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
        with self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
            p.compose_namespace_resource(self.a, raw + b'// changed\n')

    def test_helper_reversal_closed_bridge_and_preserved_unary_gate_label(self):
        helper = p.compose_enum_parser_helper(self.a)
        raw = (p.REPOSITORY / 'src/frontend/parser.rs').read_bytes()
        current = p.compose_array_instrumentation(self.a, 'src/frontend/parser.rs', raw)
        generated = helper.parser_overlay(raw.decode())
        unclosed = p.compose_u8_closed_policy(self.a, 'src/frontend/parser.rs', current, reverse=True)
        self.assertEqual(unclosed.replace(p.ENUM_NODE_BRIDGE.encode(), b'', 1), generated.encode())
        self.assertEqual(current.count(p.ENUM_NODE_BRIDGE.encode()), 1)
        self.assertIn(b'panic!("enum node route is outside the closed Unit4 parser observation domain")', current)
        self.assertIn(b'self.unit4_node("unary")?;', current)
        self.assertNotIn(b'self.unit4_node("unary_with_prefixes")?;', current)
        control = p.compose_array_instrumentation(self.a, 'src/frontend/parser.rs', raw, True)
        self.assertEqual(p.compose_u8_closed_policy(self.a, 'src/frontend/parser.rs', control, reverse=True), raw + b'\n#[cfg(test)]\npub(super) mod unit4_observer;\n')
        self.assertIn(b'        EnumSyntaxPolicy::Closed,\n        StdImportPolicy::Closed,\n        &mut enums::SyntaxStorage::default(),', raw)
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



class HirProducerTransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        active = cls.a['current']
        current_inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                          for row in cls.a['current_source']['files']}
        cls.inputs = p.restore_u8_source(active, current_inputs)
        cls.active = p.u8_predecessor_active(active)
        cls.a['current'] = cls.active
        cls.a['current_source'] = p.read(p.REPOSITORY / active['cache_admission_source_manifest']['path'])

    def test_exact_inverse_restores_both_cargo_files_and_retained_hir_source(self):
        restored = p.restore_hir_producer_source(self.active, self.inputs)
        predecessor = p.read(p.REPOSITORY / self.active['hir_import_source_manifest']['path'])
        self.assertEqual(len(restored), 324)
        for row in predecessor['files']:
            self.assertEqual({'path': row['path'], 'bytes': len(restored[row['path']]),
                              'sha256': p.sha(restored[row['path']])}, row)
        for name in ('Cargo.toml', 'Cargo.lock'):
            self.assertNotEqual(restored[name], self.inputs[name])
        self.assertFalse(any(name.startswith('src/frontend/hir_producer') for name in restored))

    def test_changed_dependency_module_missing_and_extra_input_reject(self):
        for name in ('Cargo.toml', 'Cargo.lock', 'src/frontend/hir_producer.rs'):
            altered = dict(self.inputs)
            altered[name] += b'\n// unauthorized tail\n'
            with self.subTest(changed=name), self.assertRaises(p.Rejected):
                p.restore_hir_producer_source(self.active, altered)
        for name in ('src/frontend/hir_producer.rs', 'src/frontend/hir_producer/bundle.rs',
                     'src/frontend/hir_producer/supervisor.rs'):
            altered = dict(self.inputs)
            del altered[name]
            with self.subTest(missing=name), self.assertRaises(p.Rejected):
                p.restore_hir_producer_source(self.active, altered)
        altered = dict(self.inputs, **{'src/frontend/hir_producer/extra.rs': b'// extra\n'})
        with self.assertRaises(p.Rejected):
            p.restore_hir_producer_source(self.active, altered)

    def test_predecessor_rebase_transition_and_runner_pin_reject(self):
        for field in ('hir_import_source_manifest', 'hir_producer_transition_patch', 'source_binding_runner'):
            altered = copy.deepcopy(self.active)
            altered[field]['sha256'] = '0' * 64
            with self.subTest(field=field), self.assertRaises(p.Rejected):
                p.restore_hir_producer_source(altered, self.inputs)

    def test_double_inverse_rejects(self):
        restored = p.restore_hir_producer_source(self.active, self.inputs)
        with self.assertRaises(p.Rejected):
            p.restore_hir_producer_source(self.active, restored)


    def test_unexpected_instrumentation_overlap_rejects(self):
        historical = copy.deepcopy(self.a)
        for field in ('instrumentation', 'control_instrumentation'):
            altered = copy.deepcopy(historical)
            altered[field].append({'path': 'src/frontend/hir_producer.rs'})
            with self.subTest(field=field), self.assertRaisesRegex(p.Rejected, 'must not overlap parser instrumentation'):
                p.validate_hir_producer_transition(self.active, p.read(p.REPOSITORY / self.active['hir_producer_source_manifest']['path']), altered)


class HirImportOverlapControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.name = 'src/frontend/project/budget.rs'
        cls.raw = (p.REPOSITORY / cls.name).read_bytes()

    def test_exact_inverse_and_current_reserve_hook(self):
        self.assertEqual(p.HIR_IMPORT_INSTRUMENTATION_PATHS, (self.name,))
        restored = p.restore_hir_import_source(self.a, self.name, self.raw)
        predecessor = p.read(p.REPOSITORY / self.a['current']['native_inventory_source_manifest']['path'])
        self.assertEqual(len(predecessor['files']), 266)
        self.assertEqual({'path': self.name, 'bytes': len(restored), 'sha256': p.sha(restored)},
                         next(row for row in predecessor['files'] if row['path'] == self.name))
        composed = p.compose_array_instrumentation(self.a, self.name, self.raw)
        self.assertIn(b'let _guard = real_null_observer::enter_exact::<u8>', composed)
        hook = b'        #[cfg(test)]\n        crate::frontend::parser::unit4_observer::reserve(kind, length, element_bytes, success);\n'
        self.assertEqual(composed.count(hook), 1)
        self.assertEqual(composed.replace(hook, b'', 1), self.raw)
        current_control = next(r for r in self.a['current']['current_control_derived_files'] if r['path'] == self.name)
        self.assertEqual(current_control, {'path': self.name, 'bytes': len(self.raw), 'sha256': p.sha(self.raw)})

    def test_changed_and_coherently_rehashed_tail_rejects(self):
        raw = self.raw + b'// unauthorized HIR import tail\n'
        with self.assertRaisesRegex(p.Rejected, 'composition current hir_import identity'):
            p.restore_hir_import_source(self.a, self.name, raw)
        altered = copy.deepcopy(self.a)
        next(row for row in altered['current']['source_delta'] if row['path'] == self.name)['after'].update(bytes=len(raw), sha256=p.sha(raw))
        with self.assertRaisesRegex(p.Rejected, 'current lexer composition identity'):
            p.restore_hir_import_source(altered, self.name, raw)


    def test_double_inverse_and_wrong_path_reject(self):
        restored = p.restore_hir_import_source(self.a, self.name, self.raw)
        with self.assertRaisesRegex(p.Rejected, 'composition current hir_import identity'):
            p.restore_hir_import_source(self.a, self.name, restored)
        with self.assertRaisesRegex(p.Rejected, 'unapproved hir_import instrumentation path'):
            p.restore_hir_import_source(self.a, 'src/frontend/parser.rs', b'')


    def test_predecessor_pin_cannot_be_rebased(self):
        altered = copy.deepcopy(self.a)
        altered['current']['native_inventory_source_manifest']['sha256'] = '0' * 64
        with self.assertRaises(p.Rejected):
            p.restore_hir_import_source(altered, self.name, self.raw)

    def test_unexpected_observer_and_control_overlap_reject(self):
        original_read = p.read
        path = p.REPOSITORY / self.a['current']['hir_import_authority']['path']
        for name in ('src/frontend/parser.rs', 'src/frontend/lexer.rs'):
            altered = copy.deepcopy(original_read(path))
            altered['transition_paths'].append(name)
            with self.subTest(path=name), patch.object(p, 'read', side_effect=lambda candidate: altered if Path(candidate) == path else original_read(candidate)):
                with self.assertRaisesRegex(p.Rejected, 'exact HIR import instrumentation overlap roster'):
                    p.authority()

    def test_transition_predecessor_and_runner_drift_reject(self):
        fields = ('source_binding_runner', 'hir_import_transition_patch', 'native_inventory_source_manifest')
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                stage_lexer_dispatcher_dependencies(root, self.a['current'])
                for key in dict.fromkeys((*fields, *p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS)):
                    relative = self.a['current'][key]['path']
                    target = root / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes((p.REPOSITORY / relative).read_bytes() + (b'\n' if key == field else b''))
                with patch.object(p, 'REPOSITORY', root), self.assertRaisesRegex(p.Rejected, 'file bytes differ'):
                    p.restore_hir_import_source(self.a, self.name, self.raw)

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
        # Temporary fixtures must not leave detached writers after Git returns.
        return subprocess.check_output(['/usr/bin/git', '-c', 'user.name=Codex', '-c', 'user.email=codex@local.invalid',
                                        '-c', 'core.autocrlf=false', '-c', 'maintenance.auto=false',
                                        '-C', str(self.root), *args], stderr=subprocess.STDOUT)

    def test_fixture_git_never_starts_automatic_maintenance(self):
        with tempfile.TemporaryDirectory(prefix='oxid-current-parser-trace-') as directory:
            trace = Path(directory).resolve() / 'git-trace.json'
            with patch.dict('os.environ', {
                    'GIT_CONFIG_COUNT': '3',
                    'GIT_CONFIG_KEY_0': 'maintenance.auto', 'GIT_CONFIG_VALUE_0': 'true',
                    'GIT_CONFIG_KEY_1': 'maintenance.autoDetach', 'GIT_CONFIG_VALUE_1': 'true',
                    'GIT_CONFIG_KEY_2': 'gc.auto', 'GIT_CONFIG_VALUE_2': '1',
                    'GIT_TRACE2_EVENT': str(trace)}):
                self.assertEqual(self.git('config', '--bool', 'maintenance.auto'), b'false\n')
                self.git('commit', '--allow-empty', '-qm', 'Maintenance lifetime control')
            events = [json.loads(line) for line in trace.read_text().splitlines()]
            self.assertTrue(any(event['event'] == 'start' and 'commit' in event.get('argv', [])
                                for event in events))
            self.assertFalse(any(event['event'] == 'child_start' and 'maintenance' in event.get('argv', [])
                                 for event in events))

    def rejects_before_git_or_child(self, message):
        with patch.object(p, 'git', side_effect=AssertionError('must reject before Git/tool use')):
            with self.assertRaisesRegex(p.Rejected, message): p.verify_checkout(self.root, self.a)

    def test_exact_current_bodies_and_git_are_admitted(self):
        bound = p.verify_checkout(self.root, self.a)
        self.assertEqual(len(bound['compiler_files']), 291)
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


    def historical_ast_from_retained_transitions(self):
        # No public checkout is required to possess a locally reconstructed
        # historical commit:path. Recover only the immutable authority's body.
        name = 'src/frontend/ast.rs'
        active = self.a['current']
        raw = (p.REPOSITORY / name).read_bytes()
        previous, raw = p.u8_predecessor_body(self.a, name, raw)
        combined = p.restore_division_source(previous, name, raw)
        transition = active['combined_transition_patch']
        p.verify_map(p.REPOSITORY, [transition])
        prefix = ('a/' + name + ' b/' + name + '\n').encode()
        sections = [b'diff --git ' + part for part in
                    (p.REPOSITORY / transition['path']).read_bytes().split(b'diff --git ')[1:]
                    if part.startswith(prefix)]
        self.assertEqual(len(sections), 1)
        section = sections[0]
        restored, touched = p.u8_binding_api(active).apply_inverse_patch(
            {name: combined}, section, p.sha(section), len(section), (name,))
        self.assertEqual(touched, [name])
        raw = restored[name]
        expected = next(row for row in self.a['original_files'] if row['path'] == name)
        self.assertEqual({'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)}, expected)
        self.assertEqual((len(raw), p.sha(raw)),
                         (20717, '3632fc114303be571166307ddab04f78b508226789b7c1495b4a692c538d3d2f'))
        return raw

    def test_old_historical_body_cannot_substitute(self):
        name = 'src/frontend/ast.rs'
        with patch.object(p, 'git', side_effect=AssertionError('no historical Git lookup')):
            raw = self.historical_ast_from_retained_transitions()
            (self.root / name).write_bytes(raw)
            self.rejects_before_git_or_child('file bytes differ')

    def test_historical_mutation_fixture_is_public_history_independent(self):
        with patch.object(p, 'git', side_effect=AssertionError('no historical Git lookup')), \
             patch.object(subprocess, 'check_output', side_effect=AssertionError('no Git subprocess')), \
             patch.object(subprocess, 'run', side_effect=AssertionError('no fetch or other child')):
            raw = self.historical_ast_from_retained_transitions()
        self.assertNotEqual(raw, (self.root / 'src/frontend/ast.rs').read_bytes())

    def test_current_worktree_cannot_hide_wrong_committed_body(self):
        path = self.root / p.CURRENT_PATHS[0]; good = path.read_bytes()
        path.write_bytes(good + b'// committed wrong body\n')
        self.git('add', '.'); self.git('commit', '-qm', 'Wrong fixture Git body')
        path.write_bytes(good)
        with self.assertRaisesRegex(p.Rejected, 'checkout Git current input bytes'):
            p.verify_checkout(self.root, self.a)




class LexicalProviderTransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        active = cls.a['current']
        current_inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                          for row in cls.a['current_source']['files']}
        cls.inputs = p.restore_u8_source(active, current_inputs)
        cls.active = p.u8_predecessor_active(active)
        cls.a['current'] = cls.active
        cls.a['current_source'] = p.read(p.REPOSITORY / active['cache_admission_source_manifest']['path'])

    def test_exact_lexical_inverse_recovers_retained_diagnostic(self):
        restored = p.restore_lexical_provider_source(self.active, self.inputs)
        predecessor = p.read(p.REPOSITORY / self.active['producer_diagnostic_source_manifest']['path'])
        self.assertEqual(len(restored), 340)
        self.assertEqual([{'path': name, 'bytes': len(data), 'sha256': p.sha(data)}
                          for name, data in sorted(restored.items())], predecessor['files'])
        self.assertEqual(set(self.inputs) - set(restored), {
            'src/frontend/lexical_provider.rs', 'src/frontend/lexical_provider/bundle.rs',
            'src/frontend/lexical_provider/supervisor.rs', 'src/frontend/lexical_provider/tests.rs',
            'src/frontend/lexical_provider/wire.rs'})
        with self.assertRaises(p.Rejected):
            p.restore_lexical_provider_source(self.active, restored)
        retained = self.active['producer_diagnostic_parser_authority']
        self.assertEqual(p.sha((p.REPOSITORY / retained['path']).read_bytes()),
                         'ecc09429f644abeaa9f92db9b65b73a181c6ee259472d94660a61861ee0360c9')

    def test_lexical_inputs_and_binding_pins_reject(self):
        successor = p.read(p.REPOSITORY / self.active['lexical_provider_authority']['path'])
        for name in successor['transition_paths']:
            altered = dict(self.inputs)
            altered[name] += b'\n'
            with self.subTest(path=name), self.assertRaises(p.Rejected):
                p.restore_lexical_provider_source(self.active, altered)
        for key in ('lexical_provider_helper', 'lexical_provider_transition_patch',
                    'producer_diagnostic_source_manifest'):
            altered = copy.deepcopy(self.active)
            altered[key]['sha256'] = '0' * 64
            with self.subTest(field=key), self.assertRaises(p.Rejected):
                p.restore_lexical_provider_source(altered, self.inputs)

    def test_lexical_cannot_overlap_either_historical_hook_set(self):
        for field in ('instrumentation', 'control_instrumentation'):
            altered = copy.deepcopy(self.a)
            altered[field].append({'path': 'src/frontend/project.rs'})
            with self.subTest(field=field), self.assertRaisesRegex(p.Rejected, 'must not overlap parser instrumentation'):
                p.validate_lexical_provider_transition(self.active, p.read(p.REPOSITORY / self.active['lexical_provider_source_manifest']['path']), altered)

    def test_lexical_preserves_historical_parser_authority_and_dependencies(self):
        old = p.read(p.REPOSITORY / self.active['producer_diagnostic_parser_authority']['path'])
        for key in ('current_dependency_closure', 'historical_authority', 'historical_portable',
                    'enum_ast_schema_adapter', 'enum_parser_instrumentation_adapter',
                    'observer_initializer_adapter', 'composition_parser_amendment',
                    'composition_parser_amendment_module'):
            self.assertEqual(self.active[key], old[key], key)


class ProducerDiagnosticTransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        active = cls.a['current']
        current_inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                          for row in cls.a['current_source']['files']}
        cls.inputs = p.restore_u8_source(active, current_inputs)
        cls.active = p.u8_predecessor_active(active)
        cls.a['current'] = cls.active
        cls.a['current_source'] = p.read(p.REPOSITORY / active['cache_admission_source_manifest']['path'])

    def test_exact_diagnostic_inverse_recovers_retained_frontend_v2(self):
        restored = p.restore_producer_diagnostic_source(self.active, self.inputs)
        predecessor = p.read(p.REPOSITORY / self.active['frontend_v2_source_manifest']['path'])
        self.assertEqual(len(restored), 330)
        self.assertEqual([{'path': name, 'bytes': len(data), 'sha256': p.sha(data)}
                          for name, data in sorted(restored.items())], predecessor['files'])
        self.assertEqual(self.active['frontend_v2_source_manifest']['sha256'],
                         'd294963af70126d6415e035f5ef1652c00953a22e6d26d586e0e1c1352cd6c8a')
        with self.assertRaises(p.Rejected):
            p.restore_producer_diagnostic_source(self.active, restored)

    def test_diagnostic_inputs_and_binding_pins_reject(self):
        successor = p.read(p.REPOSITORY / self.active['producer_diagnostic_authority']['path'])
        for name in [*successor['transition_paths'], *(r['path'] for r in successor['fixture_inputs'])]:
            altered = dict(self.inputs)
            altered[name] += b'\n'
            with self.subTest(path=name), self.assertRaises(p.Rejected):
                p.restore_producer_diagnostic_source(self.active, altered)
        for key in ('producer_diagnostic_helper', 'producer_diagnostic_transition_patch',
                    'frontend_v2_source_manifest'):
            altered = copy.deepcopy(self.active)
            altered[key]['sha256'] = '0' * 64
            with self.subTest(field=key), self.assertRaises(p.Rejected):
                p.restore_producer_diagnostic_source(altered, self.inputs)

    def test_diagnostic_cannot_overlap_either_historical_hook_set(self):
        for field in ('instrumentation', 'control_instrumentation'):
            altered = copy.deepcopy(self.a)
            altered[field].append({'path': 'src/frontend/oir/source/hir_import/producer_diagnostic.rs'})
            with self.subTest(field=field), self.assertRaises(p.Rejected):
                p.validate_producer_diagnostic_transition(self.active, self.a['current_source'], altered)


class FrontendV2TransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        active = cls.a['current']
        current_inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                          for row in cls.a['current_source']['files']}
        cls.inputs = p.restore_u8_source(active, current_inputs)
        cls.active = p.u8_predecessor_active(active)
        cls.a['current'] = cls.active
        cls.a['current_source'] = p.read(p.REPOSITORY / active['cache_admission_source_manifest']['path'])

    def test_exact_v2_inverse_recovers_unchanged_producer(self):
        restored = p.restore_frontend_v2_source(self.active, self.inputs)
        predecessor = p.read(p.REPOSITORY / self.active['hir_producer_source_manifest']['path'])
        self.assertEqual(len(restored), 327)
        self.assertEqual([{'path':name,'bytes':len(data),'sha256':p.sha(data)}
                          for name,data in sorted(restored.items())], predecessor['files'])
        with self.assertRaises(p.Rejected):
            p.restore_frontend_v2_source(self.active, restored)

    def test_v2_new_files_and_binding_pins_reject(self):
        for name in ('src/frontend/hir_protocol.rs',
                     'tests/fixtures/checked_hir_import_v2/source-255.txt',
                     'tests/fixtures/checked_hir_import_v2/success-255.bin'):
            altered=dict(self.inputs);altered[name]+=b'\n'
            with self.subTest(path=name), self.assertRaises(p.Rejected):
                p.restore_frontend_v2_source(self.active, altered)
        for key in ('frontend_v2_helper','frontend_v2_transition_patch','hir_producer_source_manifest'):
            altered=copy.deepcopy(self.active);altered[key]['sha256']='0'*64
            with self.subTest(field=key), self.assertRaises(p.Rejected):
                p.restore_frontend_v2_source(altered, self.inputs)

    def test_v2_cannot_overlap_either_historical_hook_set(self):
        for field in ('instrumentation','control_instrumentation'):
            altered=copy.deepcopy(self.a)
            altered[field].append({'path':'src/frontend/hir_protocol.rs'})
            with self.subTest(field=field), self.assertRaises(p.Rejected):
                p.validate_frontend_v2_transition(self.active, p.read(p.REPOSITORY / self.active['frontend_v2_source_manifest']['path']), altered)


class PackageIntegrityTransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        active = cls.a['current']
        current_inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                          for row in cls.a['current_source']['files']}
        cls.inputs = p.restore_u8_source(active, current_inputs)
        cls.active = p.u8_predecessor_active(active)
        cls.a['current'] = cls.active
        cls.a['current_source'] = p.read(p.REPOSITORY / active['cache_admission_source_manifest']['path'])

    def test_exact_single_body_inverse_and_immutable_parser_predecessor(self):
        restored = p.restore_package_integrity_source(self.active, self.inputs)
        predecessor = p.read(p.REPOSITORY / self.active['lexical_provider_source_manifest']['path'])
        self.assertEqual([{'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)}
                          for name, raw in sorted(restored.items())], predecessor['files'])
        self.assertEqual([name for name in sorted(self.inputs)
                          if self.inputs[name] != restored[name]], ['src/runtime/packages.rs'])
        retained = self.active['lexical_provider_parser_authority']
        self.assertEqual(p.sha((p.REPOSITORY / retained['path']).read_bytes()),
                         '209ad73bf9240366136787f044c837fffaaeae9a9909faa9855003387fdade2f')
        with self.assertRaises(p.Rejected):
            p.restore_package_integrity_source(self.active, restored)

    def test_wrong_member_hash_missing_and_extra_delta_reject(self):
        for case in ('wrong-hash', 'missing', 'wrong-member', 'extra-delta', 'extra-member'):
            altered = dict(self.inputs)
            if case == 'wrong-hash': altered['src/runtime/packages.rs'] += b'\n'
            if case == 'missing': del altered['src/runtime/packages.rs']
            if case == 'wrong-member': altered['src/runtime/wrong.rs'] = altered.pop('src/runtime/packages.rs')
            if case == 'extra-delta': altered['src/frontend/lexer.rs'] += b'\n'
            if case == 'extra-member': altered['src/runtime/extra.rs'] = b'\n'
            with self.subTest(case=case), self.assertRaises(p.Rejected):
                p.restore_package_integrity_source(self.active, altered)

    def test_all_binding_pins_reject(self):
        for field in ('source_binding_runner', 'package_integrity_helper',
                      'package_integrity_transition_patch', 'lexical_provider_source_manifest'):
            altered = copy.deepcopy(self.active)
            altered[field]['sha256'] = '0' * 64
            with self.subTest(field=field), self.assertRaises(p.Rejected):
                p.restore_package_integrity_source(altered, self.inputs)

    def test_coherently_rebased_predecessor_rejects(self):
        active = copy.deepcopy(self.active)
        path = p.REPOSITORY / active['lexical_provider_source_manifest']['path']
        altered = p.read(path)
        altered['files'][0]['sha256'] = '0' * 64
        original_read = p.read
        with patch.object(p, 'read', side_effect=lambda name: altered if name == path else original_read(name)):
            with self.assertRaisesRegex(p.Rejected, 'must recover exact lexical source'):
                p.restore_package_integrity_source(active, self.inputs)
        active['lexical_provider_source_manifest']['sha256'] = p.sha(
            (json.dumps(altered, sort_keys=True, indent=2) + '\n').encode())
        with patch.object(p, 'verify_map'), self.assertRaisesRegex(p.Rejected, 'exact retained lexical source identity'):
            p.restore_package_integrity_source(active, self.inputs)

    def test_package_cannot_overlap_either_hook_roster(self):
        for field in ('instrumentation', 'control_instrumentation'):
            altered = copy.deepcopy(self.a)
            altered[field].append({'path': 'src/runtime/packages.rs'})
            with self.subTest(field=field), self.assertRaisesRegex(p.Rejected, 'must not overlap parser instrumentation'):
                p.validate_package_integrity_transition(self.active, p.read(p.REPOSITORY / self.active['package_integrity_source_manifest']['path']), altered)

    def test_package_preserves_semantics_and_dependencies(self):
        old = p.read(p.REPOSITORY / self.active['lexical_provider_parser_authority']['path'])
        for key in ('current_dependency_closure', 'historical_authority', 'historical_portable',
                    'enum_ast_schema_adapter', 'enum_parser_instrumentation_adapter',
                    'observer_initializer_adapter', 'composition_parser_amendment',
                    'composition_parser_amendment_module'):
            self.assertEqual(self.active[key], old[key], key)


class CachePreservationTransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        active = cls.a['current']
        current_inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                          for row in cls.a['current_source']['files']}
        cls.inputs = p.restore_u8_source(active, current_inputs)
        cls.active = p.u8_predecessor_active(active)
        cls.a['current'] = cls.active
        cls.a['current_source'] = p.read(p.REPOSITORY / active['cache_admission_source_manifest']['path'])

    def test_exact_single_body_inverse_and_immutable_parser_predecessor(self):
        restored = p.restore_cache_preservation_source(self.active, self.inputs)
        predecessor = p.read(p.REPOSITORY / self.active['package_integrity_source_manifest']['path'])
        self.assertEqual([{'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)}
                          for name, raw in sorted(restored.items())], predecessor['files'])
        self.assertEqual([name for name in sorted(self.inputs)
                          if self.inputs[name] != restored[name]], ['src/runtime/packages.rs'])
        retained = self.active['package_integrity_parser_authority']
        self.assertEqual(p.sha((p.REPOSITORY / retained['path']).read_bytes()),
                         '974aebf4121f323f1674a19b7442d42e495c2075e99ae9f70c6cc1f1fb59a7a5')
        with self.assertRaises(p.Rejected):
            p.restore_cache_preservation_source(self.active, restored)

    def test_wrong_member_hash_missing_and_extra_delta_reject(self):
        for case in ('wrong-hash', 'missing', 'wrong-member', 'extra-delta', 'extra-member'):
            altered = dict(self.inputs)
            if case == 'wrong-hash': altered['src/runtime/packages.rs'] += b'\n'
            if case == 'missing': del altered['src/runtime/packages.rs']
            if case == 'wrong-member': altered['src/runtime/wrong.rs'] = altered.pop('src/runtime/packages.rs')
            if case == 'extra-delta': altered['src/frontend/lexer.rs'] += b'\n'
            if case == 'extra-member': altered['src/runtime/extra.rs'] = b'\n'
            with self.subTest(case=case), self.assertRaises(p.Rejected):
                p.restore_cache_preservation_source(self.active, altered)

    def test_all_binding_pins_reject(self):
        for field in ('source_binding_runner', 'cache_preservation_helper',
                      'cache_preservation_transition_patch', 'package_integrity_source_manifest'):
            altered = copy.deepcopy(self.active)
            altered[field]['sha256'] = '0' * 64
            with self.subTest(field=field), self.assertRaises(p.Rejected):
                p.restore_cache_preservation_source(altered, self.inputs)

    def test_coherently_rebased_predecessor_rejects(self):
        active = copy.deepcopy(self.active)
        path = p.REPOSITORY / active['package_integrity_source_manifest']['path']
        altered = p.read(path)
        altered['files'][0]['sha256'] = '0' * 64
        original_read = p.read
        with patch.object(p, 'read', side_effect=lambda name: altered if name == path else original_read(name)):
            with self.assertRaisesRegex(p.Rejected, 'must recover exact package integrity source'):
                p.restore_cache_preservation_source(active, self.inputs)
        active['package_integrity_source_manifest']['sha256'] = p.sha(
            (json.dumps(altered, sort_keys=True, indent=2) + '\n').encode())
        with patch.object(p, 'verify_map'), self.assertRaisesRegex(p.Rejected, 'exact retained package integrity source identity'):
            p.restore_cache_preservation_source(active, self.inputs)

    def test_cache_cannot_overlap_either_hook_roster(self):
        for field in ('instrumentation', 'control_instrumentation'):
            altered = copy.deepcopy(self.a)
            altered[field].append({'path': 'src/runtime/packages.rs'})
            with self.subTest(field=field), self.assertRaisesRegex(p.Rejected, 'must not overlap parser instrumentation'):
                p.validate_cache_preservation_transition(self.active, p.read(p.REPOSITORY / self.active['cache_preservation_source_manifest']['path']), altered)

    def test_cache_preserves_semantics_and_dependencies(self):
        old = p.read(p.REPOSITORY / self.active['package_integrity_parser_authority']['path'])
        for key in ('current_dependency_closure', 'historical_authority', 'historical_portable',
                    'enum_ast_schema_adapter', 'enum_parser_instrumentation_adapter',
                    'observer_initializer_adapter', 'composition_parser_amendment',
                    'composition_parser_amendment_module'):
            self.assertEqual(self.active[key], old[key], key)


class CacheAdmissionTransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        active = cls.a['current']
        current_inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                          for row in cls.a['current_source']['files']}
        cls.inputs = p.restore_u8_source(active, current_inputs)
        cls.active = p.u8_predecessor_active(active)
        cls.a['current'] = cls.active
        cls.a['current_source'] = p.read(p.REPOSITORY / active['cache_admission_source_manifest']['path'])

    def test_exact_single_body_inverse_and_immutable_parser_predecessor(self):
        restored = p.restore_cache_admission_source(self.active, self.inputs)
        predecessor = p.read(p.REPOSITORY / self.active['cache_preservation_source_manifest']['path'])
        self.assertEqual([{'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)}
                          for name, raw in sorted(restored.items())], predecessor['files'])
        self.assertEqual([name for name in sorted(self.inputs)
                          if self.inputs[name] != restored[name]], ['src/runtime/packages.rs'])
        retained = self.active['cache_preservation_parser_authority']
        self.assertEqual(p.sha((p.REPOSITORY / retained['path']).read_bytes()),
                         'f9202d30c38ca09e51879146a53864c7107a49e6e07ce581e22dfe9afa78d377')
        with self.assertRaises(p.Rejected):
            p.restore_cache_admission_source(self.active, restored)

    def test_wrong_member_hash_missing_and_extra_delta_reject(self):
        for case in ('wrong-hash', 'missing', 'wrong-member', 'extra-delta', 'extra-member'):
            altered = dict(self.inputs)
            if case == 'wrong-hash': altered['src/runtime/packages.rs'] += b'\n'
            if case == 'missing': del altered['src/runtime/packages.rs']
            if case == 'wrong-member': altered['src/runtime/wrong.rs'] = altered.pop('src/runtime/packages.rs')
            if case == 'extra-delta': altered['src/frontend/lexer.rs'] += b'\n'
            if case == 'extra-member': altered['src/runtime/extra.rs'] = b'\n'
            with self.subTest(case=case), self.assertRaises(p.Rejected):
                p.restore_cache_admission_source(self.active, altered)

    def test_all_binding_pins_reject(self):
        for field in ('source_binding_runner', 'cache_admission_helper',
                      'cache_admission_transition_patch', 'cache_preservation_source_manifest'):
            altered = copy.deepcopy(self.active)
            altered[field]['sha256'] = '0' * 64
            with self.subTest(field=field), self.assertRaises(p.Rejected):
                p.restore_cache_admission_source(altered, self.inputs)

    def test_coherently_rebased_predecessor_rejects(self):
        active = copy.deepcopy(self.active)
        path = p.REPOSITORY / active['cache_preservation_source_manifest']['path']
        altered = p.read(path)
        altered['files'][0]['sha256'] = '0' * 64
        original_read = p.read
        with patch.object(p, 'read', side_effect=lambda name: altered if name == path else original_read(name)):
            with self.assertRaisesRegex(p.Rejected, 'must recover exact cache preservation source'):
                p.restore_cache_admission_source(active, self.inputs)
        active['cache_preservation_source_manifest']['sha256'] = p.sha(
            (json.dumps(altered, sort_keys=True, indent=2) + '\n').encode())
        with patch.object(p, 'verify_map'), self.assertRaisesRegex(p.Rejected, 'exact retained cache preservation source identity'):
            p.restore_cache_admission_source(active, self.inputs)

    def test_cache_cannot_overlap_either_hook_roster(self):
        for field in ('instrumentation', 'control_instrumentation'):
            altered = copy.deepcopy(self.a)
            altered[field].append({'path': 'src/runtime/packages.rs'})
            with self.subTest(field=field), self.assertRaisesRegex(p.Rejected, 'must not overlap parser instrumentation'):
                p.validate_cache_admission_transition(self.active, self.a['current_source'], altered)

    def test_cache_preserves_semantics_and_dependencies(self):
        old = p.read(p.REPOSITORY / self.active['cache_preservation_parser_authority']['path'])
        for key in ('current_dependency_closure', 'historical_authority', 'historical_portable',
                    'enum_ast_schema_adapter', 'enum_parser_instrumentation_adapter',
                    'observer_initializer_adapter', 'composition_parser_amendment',
                    'composition_parser_amendment_module'):
            self.assertEqual(self.active[key], old[key], key)


class ByteStorageTransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.active = cls.a['current']
        cls.inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                      for row in cls.a['current_source']['files']}

    def test_exact_complete_predecessor_and_reapplication_refusal(self):
        restored = p.restore_byte_storage_source(self.active, self.inputs)
        previous = p.read(p.REPOSITORY / self.active['u8_cross_host_source_manifest']['path'])
        self.assertEqual(len(restored), 363)
        self.assertEqual([{'path': k, 'bytes': len(v), 'sha256': p.sha(v)} for k, v in sorted(restored.items())],
                         previous['files'])
        with self.assertRaises(p.Rejected):
            p.restore_byte_storage_source(self.active, restored)
        retained = p.byte_storage_predecessor_active(self.active)
        self.assertEqual(retained['reviewed_source_head'], 'd204fbc684b81c6e0deac04007182ebe2bb67b00')
        self.assertEqual(retained['source_only_tree'], '8c00492b24d56b842858b72da9b4442f057ab4de')
        for field in ('u8_closed_policy', 'u8_policy_controls', 'enum_ast_schema_adapter',
                      'current_dependency_closure', 'composition_parser_amendment'):
            self.assertEqual(retained[field], self.active[field])

    def test_byte_storage_identity_and_membership_fail_closed(self):
        for label, mutate in (
            ('changed', lambda d: d.__setitem__('src/frontend/ast.rs', d['src/frontend/ast.rs'] + b'\n')),
            ('missing', lambda d: d.pop('src/frontend/ast.rs')),
            ('extra', lambda d: d.__setitem__('src/unapproved.rs', b'')),
        ):
            bad = dict(self.inputs); mutate(bad)
            with self.subTest(kind=label), self.assertRaises(p.Rejected):
                p.restore_byte_storage_source(self.active, bad)

    def test_byte_storage_cannot_mutate_parser_domain_or_semantics(self):
        for key in ('u8_closed_policy', 'enum_ast_schema_adapter', 'current_dependency_closure'):
            active = copy.deepcopy(self.active); active[key] = {}
            with self.subTest(field=key), self.assertRaisesRegex(p.Rejected, 'lexer changes unrelated parser authority'):
                p.byte_storage_predecessor_active(active)
        for key in p.BYTE_STORAGE_FIELDS:
            active = copy.deepcopy(self.active); active[key]['sha256'] = '0' * 64
            with self.subTest(field=key), self.assertRaises(p.Rejected):
                p.restore_byte_storage_source(active, self.inputs)


class U8TransitionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.active = cls.a['current']
        cls.inputs = {row['path']: (p.REPOSITORY / row['path']).read_bytes()
                      for row in cls.a['current_source']['files']}

    def test_cross_host_inverse_preserves_complete_initial_u8_source(self):
        cross_inputs = p.restore_byte_storage_source(self.active, self.inputs)
        cross_active = p.byte_storage_predecessor_active(self.active)
        restored = p.restore_u8_cross_host_source(cross_active, cross_inputs)
        previous = p.read(p.REPOSITORY / self.active['u8_source_manifest']['path'])
        self.assertEqual(set(restored), set(cross_inputs))
        self.assertEqual(len(restored), 363)
        self.assertEqual([{'path': k, 'bytes': len(v), 'sha256': p.sha(v)} for k, v in sorted(restored.items())], previous['files'])
        self.assertEqual({name for name in restored if restored[name] != cross_inputs[name]},
                         {'src/frontend/project.rs', 'src/frontend/declaration_index/u8_integration_tests.rs'})
        with self.assertRaises(p.Rejected):
            p.restore_u8_cross_host_source(self.active, restored)
        retained = p.verify_u8_cross_host_parser_predecessor(self.active)
        self.assertEqual(retained['reviewed_source_head'], '5e4875d19961b4eba8e465c915ac676c54a9926e')
        for field in ('u8_closed_policy', 'u8_policy_controls', 'u8_authority', 'u8_transition_patch', 'u8_helper'):
            self.assertEqual(retained[field], self.active[field])

    def test_complete_exact_source_inverse(self):
        restored = p.restore_u8_source(self.active, self.inputs)
        previous = p.read(p.REPOSITORY / self.active['cache_admission_source_manifest']['path'])
        self.assertEqual(len(restored), 345)
        self.assertEqual([{'path': k, 'bytes': len(v), 'sha256': p.sha(v)} for k, v in sorted(restored.items())], previous['files'])
        cross_inputs = p.restore_byte_storage_source(self.active, self.inputs)
        self.assertEqual(len(set(cross_inputs) - set(restored)), 18)
        with self.assertRaises(p.Rejected):
            p.restore_u8_source(self.active, restored)

    def test_missing_extra_modified_source_rejects(self):
        for change in ('missing', 'extra', 'changed'):
            inputs = dict(self.inputs)
            if change == 'missing': inputs.pop('src/frontend/parser/conversions.rs')
            elif change == 'extra': inputs['src/frontend/u8_extra.rs'] = b''
            else: inputs['src/frontend/ast.rs'] += b'\n'
            with self.subTest(change=change), self.assertRaises((p.Rejected, ValueError)):
                p.restore_u8_source(self.active, inputs)

    def test_every_u8_binding_is_pinned(self):
        for key in (*p.U8_FIELDS, *p.U8_CROSS_HOST_FIELDS, *p.BYTE_STORAGE_FIELDS):
            active = copy.deepcopy(self.active)
            active[key]['sha256'] = '0' * 64
            with self.subTest(key=key), self.assertRaises(p.Rejected):
                p.restore_u8_source(active, self.inputs)

    def test_closed_policy_exact_reversible_zero_state(self):
        self.assertEqual(self.active['u8_closed_policy']['state_fields_added'], 0)
        for name in p.U8_CLOSED_POLICY_SEAMS:
            raw = self.inputs[name]
            derived = p.compose_u8_closed_policy(self.a, name, raw)
            self.assertNotEqual(derived, raw)
            self.assertEqual(p.compose_u8_closed_policy(self.a, name, derived, reverse=True), raw)
            with self.assertRaises(p.Rejected):
                p.compose_u8_closed_policy(self.a, name, derived)
            with self.assertRaises(p.Rejected):
                p.compose_u8_closed_policy(self.a, name, raw, reverse=True)

    def test_closed_u8_policy_discriminator_is_independent_of_source_mode(self):
        # ScalarOnly must remain admitted by the enabled public typed parser.
        # arrays_enabled() additionally tests mode.owned(), which is too broad.
        for name, (_, after) in p.U8_CLOSED_POLICY_SEAMS.items():
            with self.subTest(path=name):
                self.assertIn('self.arrays.enabled()', after)
                self.assertNotIn('self.arrays_enabled()', after)
                self.assertNotIn('self.mode', after)

    def test_closed_policy_cannot_change_or_expand_scope(self):
        a = copy.deepcopy(self.a)
        a['current']['u8_closed_policy']['state_fields_added'] = 1
        with self.assertRaises(p.Rejected):
            p.compose_u8_closed_policy(a, 'src/frontend/parser.rs', self.inputs['src/frontend/parser.rs'])
        with self.assertRaises(p.Rejected):
            p.compose_u8_closed_policy(self.a, 'src/frontend/ast.rs', self.inputs['src/frontend/ast.rs'])

    def test_unrelated_parser_authority_cannot_drift(self):
        active = copy.deepcopy(self.active)
        active['enum_ast_schema_adapter']['semantic_changes_permitted'] = True
        with self.assertRaisesRegex(p.Rejected, 'lexer changes unrelated parser authority'):
            p.validate_u8_transition(active, self.a['current_source'], self.a)

    def test_u8_overlap_rosters_cannot_expand(self):
        for field in ('instrumentation', 'control_instrumentation'):
            historical = copy.deepcopy(self.a)
            historical[field].append({'path': 'src/frontend/oir/execute.rs'})
            with self.subTest(field=field), self.assertRaisesRegex(p.Rejected, 'exact u8 instrumentation overlap roster'):
                p.validate_u8_transition(self.active, self.a['current_source'], historical)

    def test_prior_policy_and_predicates_are_immutable(self):
        old = p.u8_predecessor_active(self.active)
        for key in ('enum_ast_schema_adapter', 'enum_parser_instrumentation_adapter',
                    'observer_initializer_adapter', 'composition_parser_amendment',
                    'current_dependency_closure', 'historical_authority', 'historical_portable'):
            self.assertEqual(old[key], self.active[key])


class U8PolicyReceiptControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.helper = p.u8_policy_module(p.authority())

    def fixture(self):
        # Deliberately synthetic unit-control data, never an execution receipt.
        data, rows, binaries = {}, [], {}
        self.session = {'root': '/synthetic', 'host': {'name': 'synthetic-control'}, 'prepared_ns': 1}
        def put(name, value):
            body = value if isinstance(value, bytes) else json.dumps(value).encode()
            row = {'path': name, 'bytes': len(body), 'sha256': p.sha(body)}
            data[name] = body
            return row
        for profile in ('debug', 'release'):
            for control in (False, True):
                binary = put(profile + str(control) + '/binary', b'synthetic')
                binaries[(profile, control)] = binary
                for case in [*self.helper.CASES, *(['enabled-public'] if control else [])]:
                    base = profile + str(control) + '/' + case
                    nonce = format(len(rows), '032x')
                    public = case == 'enabled-public'
                    stdout = b'test result: ok. 1 passed; 0 failed; 0 ignored;\n'
                    if not public: stdout += ('UNIT4_EXECUTED ' + nonce).encode()
                    invocation = {'argv': [binary['path'], '--exact', self.helper.PUBLIC_TEST if public else self.helper.OBSERVER_TEST,
                                           *([] if public else ['--ignored']), '--nocapture'], 'exit_code': 0,
                                  'stdout': put(base + '/stdout', stdout), 'stderr': put(base + '/stderr', b'')}
                    env = p.minimal_env(Path(self.session['root']) / 'home')
                    if not public:
                        env.update(UNIT4_SOURCE=base + '/source', UNIT4_RAW_OUTPUT=base + '/raw',
                            UNIT4_DISPLAY_PATH='u8-policy.ox', UNIT4_NONCE=nonce, UNIT4_CASE_ID=case,
                            UNIT4_CONTROL=str(int(control)), UNIT4_ORIGINAL='1', UNIT4_REJECT_KIND='',
                            UNIT4_REJECT_OCCURRENCE='0', UNIT4_RESERVE_FAIL_AT='0', UNIT4_DIRECT='0',
                            UNIT4_TOKEN_LIMIT='200000', UNIT4_NODE_LIMIT='200000')
                    invocation.update(environment=env, cwd=self.session['root'], host=self.session['host'],
                                      started_ns=2, finished_ns=3)
                    row = {'profile': profile, 'control': control, 'case': case, 'binary': binary,
                           'invocation': put(base + '/invocation', invocation)}
                    if not public:
                        source = self.helper.CASES[case]
                        row.update(nonce=nonce, source=put(base + '/source', source.encode()),
                                   raw=put(base + '/raw', {'nonce': nonce, 'case_id': case, 'source_utf8': source, 'display_path': 'u8-policy.ox',
                                   'observations': [{'mode': mode, 'executed': True, 'result': 'parse_error', 'ast': None,
                                                     'diagnostics': [{'code': 'SYNTHETIC'}]}
                                                    for mode in ('ProjectCandidate', 'OwnedCandidate')]}))
                    rows.append(row)
        return {'schema': 'oxid-unit4-u8-policy-controls-v1', 'cases': rows,
                'closed_refusal_observations': 32, 'enabled_public_tests': 2,
                'historical_observations_changed': False}, data, binaries

    def verify(self, receipt, data, binaries):
        def read(record):
            raw = data[record['path']]
            p.same({'path': record['path'], 'bytes': len(raw), 'sha256': p.sha(raw)}, record, 'synthetic identity')
            return raw
        return self.helper.verify(p, receipt, read, binaries, self.session)

    def test_complete_synthetic_control_and_missing_duplicate_roster(self):
        receipt, data, binaries = self.fixture()
        self.assertEqual(self.verify(receipt, data, binaries)['closed_refusal_observations'], 32)
        for variant in ('missing', 'duplicate', 'order'):
            altered = copy.deepcopy(receipt)
            if variant == 'missing': altered['cases'].pop()
            elif variant == 'duplicate': altered['cases'].append(altered['cases'][0])
            else: altered['cases'].reverse()
            with self.subTest(variant=variant), self.assertRaises(p.Rejected): self.verify(altered, data, binaries)

    def test_wrong_binary_nonce_raw_mode_and_zero_execution_reject(self):
        for variant in ('binary', 'nonce', 'mode', 'success', 'zero-public'):
            receipt, data, binaries = self.fixture()
            row = receipt['cases'][-1] if variant == 'zero-public' else receipt['cases'][1]
            if variant == 'binary': row['binary'] = receipt['cases'][-1]['binary']
            elif variant == 'nonce': row['nonce'] = receipt['cases'][0]['nonce']
            else:
                key = 'invocation' if variant == 'zero-public' else 'raw'
                record = row[key]; value = json.loads(data[record['path']])
                if variant == 'zero-public':
                    stream = value['stdout']; data[stream['path']] = b'test result: ok. 0 passed; 0 failed; 0 ignored;'
                    stream.update(bytes=len(data[stream['path']]), sha256=p.sha(data[stream['path']]))
                elif variant == 'mode': value['observations'][0]['mode'] = 'ScalarOnly'
                else: value['observations'][0].update(result='ok', diagnostics=[])
                data[record['path']] = json.dumps(value).encode()
                record.update(bytes=len(data[record['path']]), sha256=p.sha(data[record['path']]))
            with self.subTest(variant=variant), self.assertRaises(p.Rejected): self.verify(receipt, data, binaries)


    def test_environment_host_freshness_and_stderr_reject(self):
        for variant in ('environment', 'host', 'freshness', 'stderr'):
            receipt, data, binaries = self.fixture()
            row = receipt['cases'][0]
            record = row['invocation']; value = json.loads(data[record['path']])
            if variant == 'environment': value['environment']['UNIT4_CONTROL'] = '1'
            elif variant == 'host': value['host'] = {'name': 'wrong-host'}
            elif variant == 'freshness': value['started_ns'] = 0
            else:
                stream = value['stderr']; data[stream['path']] = b'failure'
                stream.update(bytes=7, sha256=p.sha(b'failure'))
            data[record['path']] = json.dumps(value).encode()
            record.update(bytes=len(data[record['path']]), sha256=p.sha(data[record['path']]))
            with self.subTest(variant=variant), self.assertRaises(p.Rejected): self.verify(receipt, data, binaries)


if __name__ == '__main__':
    unittest.main(verbosity=2)
