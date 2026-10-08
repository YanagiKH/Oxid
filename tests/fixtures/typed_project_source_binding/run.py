#!/usr/bin/env python3
"""Explicit current-input and archived-input adapters; no language oracle."""
import argparse
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import types
import uuid

sys.dont_write_bytecode = True
PACKAGE = Path(__file__).resolve().parent
U2 = "tests/fixtures/typed_project_unit2_independent"
U3 = "tests/fixtures/typed_project_unit3_independent"
COMPAT = "tests/fixtures/typed_project_unit3_compatibility/run.py"
EXTRA = {
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/array_native_tests.rs',
    'src/frontend/oir/owned/array_observe.rs',
    'src/frontend/oir/owned/array_reference_boundary_tests.rs',
    'src/frontend/oir/owned/array_reference_tests.rs',
    'src/frontend/oir/owned/array_tests.rs',
    'src/frontend/oir/owned/reviewer_array_observer_tests.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned_types/array_tests.rs',
    'src/frontend/parser/activation_tests.rs',
    'tests/typed_frontend.rs',
    'tests/typed_project_dispatch.rs',
}
RESOURCE = "archive/resource/parser-resource-review-tests.rs"
OLD_SEAM = b"mode:SourceMode::ProjectCandidate,tokens,cursor:0"
PREDECESSOR_SEAM = b"mode:SourceMode::ProjectCandidate,project_recovery:false,tokens,cursor:0"
NEW_SEAM = b"mode:SourceMode::ProjectCandidate,project_recovery:false,arrays:ArraySyntaxPolicy::Closed,tokens,cursor:0"
ENUM_RESOURCE_SEAM = b'mode:SourceMode::ProjectCandidate,project_recovery:false,arrays:ArraySyntaxPolicy::Closed,enums:EnumSyntaxPolicy::Closed,storage:enums::SyntaxStorage::default(),tokens,cursor:0'
STDIN_RESOURCE_SEAM = b'mode:SourceMode::ProjectCandidate,project_recovery:false,arrays:ArraySyntaxPolicy::Closed,enums:EnumSyntaxPolicy::Closed,std_imports:StdImportPolicy::Closed,storage:enums::SyntaxStorage::default(),tokens,cursor:0'
STDIN_RESOURCE_ADAPTER = {
    "version": "unit2-closed-stdin-parser-resource-v1",
    "predecessor_version": "unit2-closed-enum-parser-resource-v1",
    "predecessor": {"path": RESOURCE, "bytes": 2078,
                    "sha256": "b79e045596fab2a54ddc888435e10a12986c950658a68d3686bc0d339c4a9b23"},
    "derived": {"path": RESOURCE, "bytes": 2114,
                "sha256": "e3081188e6dfb0bd171e806992091f3a58b9b463d38a194b481341ff3064a2b5"},
    "substitution": {"old_sha256": "3d98537500bc2c6e8083523cc8d14e07142019c8ed0e3805b3fcb96939c3ca71",
                     "new_sha256": "579627d44b8b65e7a5a4ff18a99060a123ab1e8e4d5a5ebcba8e7a708573d990", "count": 1},
    "scope": "Add only the closed std-import policy to the exact enum-era direct Parser initializer; frozen resource controls and expectations remain unchanged.",
}
SLICES_SOURCE_SHA = 'f3fcde4169957c850dfe14491b0ddc4fcc6e75ac0ba81fccb4b3ebe9041c6660'
SLICES_SOURCE_BYTES = 35876
COMPOSITION_SOURCE_SHA = 'eff7e18f49b30ebd24a10645f352502211b03de1359127edefc9a43d004f2c16'
UNARY_SOURCE_SHA = 'd9a1e93d59a479f3965b6770257583ec66e06c98a5a74063f7fe29db17df5220'
PROJECTED_SOURCE_SHA = '850555bcc78b355029ed2ff0a4a094762f0ea4c0c5bcf5f728d30bbbcc213304'
PROJECTED_SOURCE_BYTES = 38636
COMPOSITION_SOURCE_BYTES = 37404
UNARY_SOURCE_BYTES = 38090
ENUM_SOURCE_SHA = '21ebc2e9f7c1b29111b35488334850aa27317bfc2400ad32963c3d7e18a16669'
ENUM_SOURCE_BYTES = 45493
STDIN_SOURCE_SHA = 'bad88720c3002658bbc85de8cc50f63d88186df2871ee5a03ea8a7da0722d13f'
STDIN_SOURCE_BYTES = 48300
STDOUT_SOURCE_SHA = '3ae8ee6cbaf6697f0735fcf4e0cb345724d76c2bae6f5046fdfb441d02ecc936'
STDOUT_SOURCE_BYTES = 50310
NATIVE_STORAGE_SOURCE_SHA = '0a4d6471f394e42e0a584cadab2c758190c25b79fb2e49bebde99aae06884303'
NATIVE_STORAGE_SOURCE_BYTES = 50842
CURRENT_SOURCE_SHA = '8911a4d5964408ee94c9bb1a108b157e9143405d93118cfcec4ca9e63fd12746'
CURRENT_SOURCE_BYTES = 62558
HIR_IMPORT_AUTHORITY_SHA = 'a4105c042d49b8145393d59482ae1e6b3872cb1de5ef6760fd098c71bb42ce34'
HIR_IMPORT_AUTHORITY_BYTES = 158041
HIR_IMPORT_HEAD = 'a1dc6fc823d36d8858375eb651b48c3c682ac2c4'
HIR_IMPORT_TREE = '74ba20e2dcecb6056d4b13448a643ba289d83f75'
HIR_IMPORT_BASE_TREE = '8a717f36016d86130ad5acc28a23f87852c76064'
HIR_IMPORT_PATCH_SHA = 'a3a0b5f8e109e7baf6a9713418ce6bc132a9a2e481be5a6a4f42dee08efb8947'
HIR_IMPORT_PATCH_BYTES = 656303
HIR_IMPORT_PATHS = ('src/frontend/driver.rs', 'src/frontend/hir.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/execute_measurement.rs', 'src/frontend/oir/lower.rs', 'src/frontend/oir/lower_measurement.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_cost.rs', 'src/frontend/oir/native_emit_observation.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/native_private_emit.rs', 'src/frontend/oir/native_private_emit_tests.rs', 'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/source.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/allocation.rs', 'src/frontend/oir/source/hir_import/allocation/tests.rs', 'src/frontend/oir/source/hir_import/ast_compare.rs', 'src/frontend/oir/source/hir_import/ast_compare/tests.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/cleanup_controls.rs', 'src/frontend/oir/source/hir_import/candidate/emit_entry_tests.rs', 'src/frontend/oir/source/hir_import/candidate/emit_terminal.rs', 'src/frontend/oir/source/hir_import/candidate/resolution_controls.rs', 'src/frontend/oir/source/hir_import/candidate/run_entry_tests.rs', 'src/frontend/oir/source/hir_import/candidate/tests.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/hir_import/candidate/verify_terminal.rs', 'src/frontend/oir/source/hir_import/emit_failure_tests.rs', 'src/frontend/oir/source/hir_import/emit_measurements.rs', 'src/frontend/oir/source/hir_import/emit_native_tests.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/frontend/oir/source/hir_import/emit_tests.rs', 'src/frontend/oir/source/hir_import/leaf.rs', 'src/frontend/oir/source/hir_import/pass_measurements.rs', 'src/frontend/oir/source/hir_import/public_facade.rs', 'src/frontend/oir/source/hir_import/run_execution_tests.rs', 'src/frontend/oir/source/hir_import/run_measurements.rs', 'src/frontend/oir/source/hir_import/tests.rs', 'src/frontend/oir/source/hir_import/verify_diagnostic_tests.rs', 'src/frontend/oir/source/hir_import/verify_fact_tests.rs', 'src/frontend/oir/source/hir_import/verify_tests.rs', 'src/frontend/oir/verify.rs', 'src/frontend/oir/verify_measurement.rs', 'src/frontend/options.rs', 'src/frontend/project/budget.rs', 'src/frontend/project/budget_real_null_observer.rs', 'src/frontend/project/budget_string_null_tests.rs', 'src/frontend/typeck.rs', 'src/frontend/typeck_measurement.rs')
HIR_IMPORT_ADDITIONS = ('src/frontend/oir/execute_measurement.rs', 'src/frontend/oir/lower_measurement.rs', 'src/frontend/oir/native_emit_cost.rs', 'src/frontend/oir/native_emit_observation.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/native_private_emit.rs', 'src/frontend/oir/native_private_emit_tests.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/allocation.rs', 'src/frontend/oir/source/hir_import/allocation/tests.rs', 'src/frontend/oir/source/hir_import/ast_compare.rs', 'src/frontend/oir/source/hir_import/ast_compare/tests.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/cleanup_controls.rs', 'src/frontend/oir/source/hir_import/candidate/emit_entry_tests.rs', 'src/frontend/oir/source/hir_import/candidate/emit_terminal.rs', 'src/frontend/oir/source/hir_import/candidate/resolution_controls.rs', 'src/frontend/oir/source/hir_import/candidate/run_entry_tests.rs', 'src/frontend/oir/source/hir_import/candidate/tests.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/hir_import/candidate/verify_terminal.rs', 'src/frontend/oir/source/hir_import/emit_failure_tests.rs', 'src/frontend/oir/source/hir_import/emit_measurements.rs', 'src/frontend/oir/source/hir_import/emit_native_tests.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/frontend/oir/source/hir_import/emit_tests.rs', 'src/frontend/oir/source/hir_import/leaf.rs', 'src/frontend/oir/source/hir_import/pass_measurements.rs', 'src/frontend/oir/source/hir_import/public_facade.rs', 'src/frontend/oir/source/hir_import/run_execution_tests.rs', 'src/frontend/oir/source/hir_import/run_measurements.rs', 'src/frontend/oir/source/hir_import/tests.rs', 'src/frontend/oir/source/hir_import/verify_diagnostic_tests.rs', 'src/frontend/oir/source/hir_import/verify_fact_tests.rs', 'src/frontend/oir/source/hir_import/verify_tests.rs', 'src/frontend/oir/verify_measurement.rs', 'src/frontend/project/budget_string_null_tests.rs', 'src/frontend/typeck_measurement.rs')
HIR_IMPORT_FIXTURES = ('tests/fixtures/checked_hir_import/public-source.txt', 'tests/fixtures/checked_hir_import/public-success.bin', 'tests/fixtures/checked_hir_import/rich-source.txt', 'tests/fixtures/checked_hir_import/rich-success.bin', 'tests/fixtures/checked_hir_import/scalar-arithmetic-source.txt', 'tests/fixtures/checked_hir_import/scalar-arithmetic-success.bin', 'tests/fixtures/checked_hir_import/scalar-assignment-source.txt', 'tests/fixtures/checked_hir_import/scalar-assignment-success.bin', 'tests/fixtures/checked_hir_import/scalar-boolean-source.txt', 'tests/fixtures/checked_hir_import/scalar-boolean-success.bin', 'tests/fixtures/checked_hir_import/scalar-comparison-source.txt', 'tests/fixtures/checked_hir_import/scalar-comparison-success.bin', 'tests/fixtures/checked_hir_import/scalar-loop-source.txt', 'tests/fixtures/checked_hir_import/scalar-loop-success.bin', 'tests/fixtures/checked_hir_import/scalar-unit-source.txt', 'tests/fixtures/checked_hir_import/scalar-unit-success.bin', 'tests/fixtures/checked_hir_import/synthetic-division-source.txt', 'tests/fixtures/checked_hir_import/synthetic-division-success.bin', 'tests/fixtures/checked_hir_import/synthetic-overflow-source.txt', 'tests/fixtures/checked_hir_import/synthetic-overflow-success.bin')
NATIVE_INVENTORY_SOURCE_SHA = '52eeeb97c2b13d04315bcc0eac68995c0587ade263078ca7adf944d9be92f842'
NATIVE_INVENTORY_SOURCE_BYTES = 51412
HIR_IMPORT_FIXTURE_ROWS = [{'path': 'tests/fixtures/checked_hir_import/public-source.txt', 'bytes': 117, 'sha256': '5cc21baa00b3555df83a8ac154e2e497052d6a11dac7e2e390a038efe51cf08f'}, {'path': 'tests/fixtures/checked_hir_import/public-success.bin', 'bytes': 2607, 'sha256': '8bd6c66e859cfd0692e18ef7e3ca94f01e6d280eb2aca55849d415b26862cf6c'}, {'path': 'tests/fixtures/checked_hir_import/rich-source.txt', 'bytes': 113, 'sha256': 'ebc49f61b8f3941b8aafdfa779cfdd34577c0c3a2ba0f44bf54c683f656a4522'}, {'path': 'tests/fixtures/checked_hir_import/rich-success.bin', 'bytes': 2607, 'sha256': '399a9cb43dd2e16d3e0eed1c160fb1428371e90f089d72ea23149acfee961f52'}, {'path': 'tests/fixtures/checked_hir_import/scalar-arithmetic-source.txt', 'bytes': 40, 'sha256': '45426a4f69ab245011785fc5ffa2c188ec84d1fa1f92c703978fd133e9677d11'}, {'path': 'tests/fixtures/checked_hir_import/scalar-arithmetic-success.bin', 'bytes': 2607, 'sha256': 'a4f09ea7f3143a3dd0711643e9c345b4612a10047f5fb4e4bfa1710dd47a8bbf'}, {'path': 'tests/fixtures/checked_hir_import/scalar-assignment-source.txt', 'bytes': 66, 'sha256': '069ced25a829737d0a5fa0b22afff5e0ff6bf125f40e97782ac5585969b827dc'}, {'path': 'tests/fixtures/checked_hir_import/scalar-assignment-success.bin', 'bytes': 2607, 'sha256': '67266a6ffcfc8936de9168268ebc97eee08a993b0bf821bb785556e93d9a75f3'}, {'path': 'tests/fixtures/checked_hir_import/scalar-boolean-source.txt', 'bytes': 49, 'sha256': '62d9083f423cf86ee8d3c5cf17328ebdca4eeeb35cf28a1d6c10e345201c98d0'}, {'path': 'tests/fixtures/checked_hir_import/scalar-boolean-success.bin', 'bytes': 2607, 'sha256': '8b5158aad56e24ed5aaf16aacde46c305df5dfff74151dec230d87a67f01da8f'}, {'path': 'tests/fixtures/checked_hir_import/scalar-comparison-source.txt', 'bytes': 65, 'sha256': '4ecd3203de480fd90db18b79ec2fe1488f7d5a9bd16b474b209be4be1b5c4b17'}, {'path': 'tests/fixtures/checked_hir_import/scalar-comparison-success.bin', 'bytes': 2607, 'sha256': '21aa0ebd769e1a512156da0d1c211d30d5ce8376fa0b03dee0130d71301e7ad6'}, {'path': 'tests/fixtures/checked_hir_import/scalar-loop-source.txt', 'bytes': 61, 'sha256': 'ee822fb34204aa7a35cc51a88485884f3e0c333a2ba7120ee4afe514f687873b'}, {'path': 'tests/fixtures/checked_hir_import/scalar-loop-success.bin', 'bytes': 2607, 'sha256': '7db60e88f053890b3c3b28dea3c6fac4f0cae461fbd71b388e07e4742d3ea30c'}, {'path': 'tests/fixtures/checked_hir_import/scalar-unit-source.txt', 'bytes': 40, 'sha256': '6f577d023451219fa71acb94a38b5d0d9a9ca4a881f7d4c43e24544fa1ef2570'}, {'path': 'tests/fixtures/checked_hir_import/scalar-unit-success.bin', 'bytes': 2607, 'sha256': 'f521d526e3565116ada4f5f5a034e2e81295f2e5e63638ebf56ed86632a74812'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-division-source.txt', 'bytes': 27, 'sha256': 'd2c2bc9fa0c23c66e0c143b0f3df1840aadf64809cbd620892ed04e01b5593ba'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-division-success.bin', 'bytes': 2607, 'sha256': 'cad0fa1af44a5a5a147945f09c46d9c3046e802e0a52e9340b0ae2201ce86dbb'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-overflow-source.txt', 'bytes': 36, 'sha256': 'be94cc8f44f475237901d1de3ca8ac9dd9856c88ea8d1bc9874e7319d9553f09'}, {'path': 'tests/fixtures/checked_hir_import/synthetic-overflow-success.bin', 'bytes': 2607, 'sha256': 'a525797bfa202b73a1e09910bf5c2080abb73671e33e6ea9ed29f767a7226313'}]
NATIVE_INVENTORY_ADDITIONS = ('src/frontend/oir/owned/native_inventory_admission_tests.rs', 'src/frontend/oir/owned/native_inventory_tests.rs')
NATIVE_INVENTORY_AUTHORITY_BYTES = 104690
NATIVE_INVENTORY_AUTHORITY_SHA = '7c8edaeea1a69abce66b40f7b59bd29584c4927584fbb3f05a633b0ebdb8ca58'
NATIVE_INVENTORY_BASE = 'cbd44c3fff9f2c843cf1d8c03ed1c67e7bfd7050'
NATIVE_INVENTORY_BASE_TREE = '4c315696f1e7d324dcf3f00f077fbf9e17f9a722'
NATIVE_INVENTORY_HEAD = 'ffa2e00543b7a1958321b719677ed3b48f42bc74'
NATIVE_INVENTORY_PATCH_BYTES = 61893
NATIVE_INVENTORY_PATCH_SHA = '4be68264904f4c059d98f49fb86de16dc9f9af6332801a175b0e118bf2277f76'
NATIVE_INVENTORY_PATHS = ('src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_inventory_admission_tests.rs', 'src/frontend/oir/owned/native_inventory_tests.rs', 'src/frontend/oir/owned/native_storage.rs', 'src/frontend/oir/owned/native_tests.rs', 'src/frontend/oir/owned/source/native_resource_tests.rs')
NATIVE_INVENTORY_TREE = '8a717f36016d86130ad5acc28a23f87852c76064'
NATIVE_STORAGE_AUTHORITY_SHA = '1124d02c4aa34b6c6caf31bfac47332ea9938294d8813ae49d42a975e3bce4da'
NATIVE_STORAGE_AUTHORITY_BYTES = 102974
NATIVE_STORAGE_PATCH_SHA = '9e2260d93e908363833cd114902a6f73aa54a5f2528a92d98fa317e26240565c'
NATIVE_STORAGE_PATCH_BYTES = 53819
NATIVE_STORAGE_BASE = '63db2c290031d76b5925fdd672c38eac2ca50578'
NATIVE_STORAGE_HEAD = '983dad1f21bdc9d37cf0d1364b58aad219e553d0'
NATIVE_STORAGE_TREE = '4380d906e868ad992d97ffc652e657773f1c2947'
NATIVE_STORAGE_PATHS = ('src/frontend/oir/owned/native.rs',
                        'src/frontend/oir/owned/native_storage.rs',
                        'src/frontend/oir/owned/native_storage_tests.rs',
                        'src/frontend/oir/owned/native_tests.rs',
                        'src/frontend/oir/owned/plan.rs',
                        'src/frontend/oir/owned/slice_native_tests.rs')
NATIVE_STORAGE_ADDITIONS = ('src/frontend/oir/owned/native_storage.rs',
                            'src/frontend/oir/owned/native_storage_tests.rs')
STDOUT_AUTHORITY_SHA = '3015dfb1237578b4903b5a865f3ae7daf7819485c3399c2bfd589b8749d4066e'
STDOUT_AUTHORITY_BYTES = 132738
STDOUT_PATCH_SHA = '80dad62cc0e9fa87b026753401b3e5bb59e0bc200696b1fa825676cd662fb1ea'
STDOUT_PATCH_BYTES = 527951
STDOUT_BASE = 'c1d73740268d64d4e908ad86ed9dabaa48dd1c23'
STDOUT_HEAD = '63db2c290031d76b5925fdd672c38eac2ca50578'
STDOUT_TREE = 'f01525a95f2e4b3dfa69237108cfbc67a1f43eab'
STDOUT_PATHS = ('native/typed_preview.c', 'src/frontend/builtin_catalog.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/builtin_tests.rs', 'src/frontend/declaration_index/enum_query_tests.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/driver.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/owned/array_native_resource_tests.rs', 'src/frontend/oir/owned/array_observe.rs', 'src/frontend/oir/owned/builtin_descriptor_tests.rs', 'src/frontend/oir/owned/builtin_input_tests.rs', 'src/frontend/oir/owned/builtin_origin_tests.rs', 'src/frontend/oir/owned/builtin_output_descriptor_tests.rs', 'src/frontend/oir/owned/builtin_output_fixtures.rs', 'src/frontend/oir/owned/builtin_output_native_tests.rs', 'src/frontend/oir/owned/builtin_output_process_tests.rs', 'src/frontend/oir/owned/builtin_output_reference_tests.rs', 'src/frontend/oir/owned/builtins.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/flow.rs', 'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/output.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/process.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/budget.rs', 'src/frontend/oir/owned/source/builtin_lower.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/candidate_native.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/hir_budget_tests.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/output_lower_tests.rs', 'src/frontend/oir/owned/source/output_source_tests.rs', 'src/frontend/oir/owned/source/output_typing_tests.rs', 'src/frontend/oir/owned/source/program.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned/verified.rs', 'src/frontend/oir/source.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/options.rs', 'src/frontend/parser.rs', 'src/frontend/parser/builtin_tests.rs', 'src/frontend/project.rs')
STDOUT_ADDITIONS = ('src/frontend/oir/owned/builtin_output_descriptor_tests.rs', 'src/frontend/oir/owned/builtin_output_fixtures.rs', 'src/frontend/oir/owned/builtin_output_native_tests.rs', 'src/frontend/oir/owned/builtin_output_process_tests.rs', 'src/frontend/oir/owned/builtin_output_reference_tests.rs', 'src/frontend/oir/owned/output.rs', 'src/frontend/oir/owned/process.rs', 'src/frontend/oir/owned/source/output_lower_tests.rs', 'src/frontend/oir/owned/source/output_source_tests.rs', 'src/frontend/oir/owned/source/output_typing_tests.rs')
STDIN_AUTHORITY_SHA = 'ff9f806e0211367c8c31d1084ce5aa80f3175b0e65c54a0d3860df0ced8cac08'
STDIN_AUTHORITY_BYTES = 150228
STDIN_PATCH_SHA = '3bebb1cb45dab0cc5a24c6d1f7aac0b011ef543f35cab984b51fa2dd91e518e8'
STDIN_PATCH_BYTES = 429695
STDIN_BASE = '78651228b8233ec2cc8a4e28c2fd1e23fdcb40cd'
STDIN_HEAD = 'c1d73740268d64d4e908ad86ed9dabaa48dd1c23'
STDIN_TREE = 'b19991275b22426397d708ac0afa1874e6511b00'
STDIN_PATHS = ('native/typed_preview.c', 'src/frontend/ast.rs', 'src/frontend/builtin_catalog.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/builtin_tests.rs', 'src/frontend/declaration_index/enum_query_tests.rs', 'src/frontend/declaration_index/enum_tests.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/format_cli.rs', 'src/frontend/mod.rs', 'src/frontend/oir/owned/array_native_resource_tests.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/budget.rs', 'src/frontend/oir/owned/builtin_descriptor_tests.rs', 'src/frontend/oir/owned/builtin_input_fixtures.rs', 'src/frontend/oir/owned/builtin_input_native_tests.rs', 'src/frontend/oir/owned/builtin_input_tests.rs', 'src/frontend/oir/owned/builtin_origin_tests.rs', 'src/frontend/oir/owned/builtins.rs', 'src/frontend/oir/owned/composition_native_tests.rs', 'src/frontend/oir/owned/composition_reference_tests.rs', 'src/frontend/oir/owned/consumer_fixtures.rs', 'src/frontend/oir/owned/consumer_pilot.rs', 'src/frontend/oir/owned/enum_admission_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_tests.rs', 'src/frontend/oir/owned/flow.rs', 'src/frontend/oir/owned/input.rs', 'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_heldout_review.rs', 'src/frontend/oir/owned/native_tests.rs', 'src/frontend/oir/owned/negation_raw_tests.rs', 'src/frontend/oir/owned/oracle_tests.rs', 'src/frontend/oir/owned/origin_tests.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/reviewer_heldout.rs', 'src/frontend/oir/owned/reviewer_origins.rs', 'src/frontend/oir/owned/reviewer_reference_tests.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/budget.rs', 'src/frontend/oir/owned/source/budget_tests.rs', 'src/frontend/oir/owned/source/builtin_lower.rs', 'src/frontend/oir/owned/source/builtin_signature_tests.rs', 'src/frontend/oir/owned/source/builtin_source_tests.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/candidate_native.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/hir_budget_tests.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/program.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/resolver_paid_tests.rs', 'src/frontend/oir/owned/source/resolver_storage.rs', 'src/frontend/oir/owned/source/resolver_storage_tests.rs', 'src/frontend/oir/owned/source/reviewer_source.rs', 'src/frontend/oir/owned/source/type_storage.rs', 'src/frontend/oir/owned/source/type_storage_tests.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned/tests.rs', 'src/frontend/oir/owned/verified.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/parser.rs', 'src/frontend/parser/builtin_tests.rs', 'src/frontend/parser/enum_syntax_tests.rs', 'src/frontend/parser/project_tests.rs', 'src/frontend/project.rs', 'src/frontend/project/builtin_tests.rs', 'src/frontend/stdin_public_tests.rs')
STDIN_ADDITIONS = ('src/frontend/builtin_catalog.rs', 'src/frontend/declaration_index/builtin_tests.rs', 'src/frontend/oir/owned/builtin_descriptor_tests.rs', 'src/frontend/oir/owned/builtin_input_fixtures.rs', 'src/frontend/oir/owned/builtin_input_native_tests.rs', 'src/frontend/oir/owned/builtin_input_tests.rs', 'src/frontend/oir/owned/builtin_origin_tests.rs', 'src/frontend/oir/owned/builtins.rs', 'src/frontend/oir/owned/input.rs', 'src/frontend/oir/owned/source/builtin_lower.rs', 'src/frontend/oir/owned/source/builtin_signature_tests.rs', 'src/frontend/oir/owned/source/builtin_source_tests.rs', 'src/frontend/parser/builtin_tests.rs', 'src/frontend/project/builtin_tests.rs', 'src/frontend/stdin_public_tests.rs')
STDIN_COMPARATOR_OLD = b"amendment=enum_module.Amendment(ROOT.parent/'source-inputs.json',ROOT/'corpus.jsonl.gz',ROOT/'enum-enabled-qualified-values-v1.json')"
STDIN_COMPARATOR_NEW = b"amendment=enum_module.Amendment(ROOT.parent/'enum-source.json',ROOT/'corpus.jsonl.gz',ROOT/'enum-enabled-qualified-values-v1.json')"
STDIN_COMPARATOR_DERIVED_SHA = '65e1d7b5885428e37f1c893992868bad448223c46712812d34f9a81fe90a964d'
STDIN_COMPARATOR_DERIVED_BYTES = 8296
ENUM_AUTHORITY_SHA = 'e539957635eaa99b1ca806f73ecc04e99a05d7319231f2385c213c63902a9923'
ENUM_AUTHORITY_BYTES = 156306
ENUM_PATCH_SHA = '57b5f94476c5c419c66dae9dd609de4350a1e224209290221bf80190c8b39204'
ENUM_PATCH_BYTES = 1805505
ENUM_BASE = '052ad52ac876c01b91701132cffb466689b24d01'
ENUM_HEAD = '78651228b8233ec2cc8a4e28c2fd1e23fdcb40cd'
ENUM_TREE = '4970ee660f670cfcb23f42a9cb182a4ca7996388'
ENUM_PATHS = ('src/frontend/ast.rs',
 'src/frontend/declaration_index.rs',
 'src/frontend/declaration_index/enum_query_tests.rs',
 'src/frontend/declaration_index/enum_tests.rs',
 'src/frontend/declaration_index/enum_views.rs',
 'src/frontend/declaration_index/resource.rs',
 'src/frontend/declaration_index/sealed.rs',
 'src/frontend/declaration_index/source_owner.rs',
 'src/frontend/declaration_index/tests.rs',
 'src/frontend/enum_public_tests.rs',
 'src/frontend/format.rs',
 'src/frontend/format/ast_tests.rs',
 'src/frontend/format/enum_candidate_tests.rs',
 'src/frontend/hir.rs',
 'src/frontend/mod.rs',
 'src/frontend/oir/owned/allocator_review_controls.rs',
 'src/frontend/oir/owned/array_native_resource_tests.rs',
 'src/frontend/oir/owned/array_native_tests.rs',
 'src/frontend/oir/owned/array_observe.rs',
 'src/frontend/oir/owned/array_reference_tests.rs',
 'src/frontend/oir/owned/array_tests.rs',
 'src/frontend/oir/owned/budget.rs',
 'src/frontend/oir/owned/cfg.rs',
 'src/frontend/oir/owned/composition_native_tests.rs',
 'src/frontend/oir/owned/composition_reference_tests.rs',
 'src/frontend/oir/owned/consumer_fixtures.rs',
 'src/frontend/oir/owned/consumer_pilot.rs',
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
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/execute_tests.rs',
 'src/frontend/oir/owned/flow.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/native_heldout_review.rs',
 'src/frontend/oir/owned/native_tests.rs',
 'src/frontend/oir/owned/negation_raw_tests.rs',
 'src/frontend/oir/owned/oracle_tests.rs',
 'src/frontend/oir/owned/origin_tests.rs',
 'src/frontend/oir/owned/plan.rs',
 'src/frontend/oir/owned/reviewer_allocator.rs',
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
 'src/frontend/oir/owned/reviewer_heldout.rs',
 'src/frontend/oir/owned/reviewer_origins.rs',
 'src/frontend/oir/owned/reviewer_reference_tests.rs',
 'src/frontend/oir/owned/shape.rs',
 'src/frontend/oir/owned/source/array_pipeline_rows.rs',
 'src/frontend/oir/owned/source/association.rs',
 'src/frontend/oir/owned/source/budget.rs',
 'src/frontend/oir/owned/source/budget_tests.rs',
 'src/frontend/oir/owned/source/candidate_adapter.rs',
 'src/frontend/oir/owned/source/candidate_mutations.rs',
 'src/frontend/oir/owned/source/candidate_native.rs',
 'src/frontend/oir/owned/source/diagnostic.rs',
 'src/frontend/oir/owned/source/enum_native_source_tests.rs',
 'src/frontend/oir/owned/source/enum_storage_failure_tests.rs',
 'src/frontend/oir/owned/source/enum_type_tests.rs',
 'src/frontend/oir/owned/source/hir.rs',
 'src/frontend/oir/owned/source/hir_budget.rs',
 'src/frontend/oir/owned/source/hir_budget_tests.rs',
 'src/frontend/oir/owned/source/lower.rs',
 'src/frontend/oir/owned/source/mod.rs',
 'src/frontend/oir/owned/source/program.rs',
 'src/frontend/oir/owned/source/resolve.rs',
 'src/frontend/oir/owned/source/resolver_enum_tests.rs',
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs',
 'src/frontend/oir/owned/source/resolver_paid_tests.rs',
 'src/frontend/oir/owned/source/resolver_storage.rs',
 'src/frontend/oir/owned/source/resolver_storage_tests.rs',
 'src/frontend/oir/owned/source/reviewer_source.rs',
 'src/frontend/oir/owned/source/type_storage.rs',
 'src/frontend/oir/owned/source/type_storage_tests.rs',
 'src/frontend/oir/owned/source/typeck.rs',
 'src/frontend/oir/owned/tests.rs',
 'src/frontend/oir/owned/verified.rs',
 'src/frontend/oir/owned_types.rs',
 'src/frontend/oir/owned_types/enum_integration_tests.rs',
 'src/frontend/oir/owned_types/enums.rs',
 'src/frontend/oir/source.rs',
 'src/frontend/oir/source/sealed.rs',
 'src/frontend/parser.rs',
 'src/frontend/parser/arrays.rs',
 'src/frontend/parser/enum_syntax_tests.rs',
 'src/frontend/parser/enums.rs',
 'src/frontend/parser/project_tests.rs',
 'src/frontend/project.rs',
 'src/frontend/project/array_syntax_tests.rs',
 'src/frontend/project/budget.rs',
 'src/frontend/project/budget_real_null_observer.rs',
 'src/frontend/project/enum_carrier_tests.rs',
 'src/frontend/project/enum_index_tests.rs',
 'src/frontend/source.rs',
 'tests/fixtures/bounded_enum_scanner/main.ox',
 'tests/fixtures/bounded_enum_scanner/scanner.ox')
ENUM_ADDITIONS = ('src/frontend/declaration_index/enum_query_tests.rs',
 'src/frontend/declaration_index/enum_tests.rs',
 'src/frontend/declaration_index/enum_views.rs',
 'src/frontend/enum_public_tests.rs',
 'src/frontend/format/enum_candidate_tests.rs',
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
 'src/frontend/oir/owned/source/enum_native_source_tests.rs',
 'src/frontend/oir/owned/source/enum_storage_failure_tests.rs',
 'src/frontend/oir/owned/source/enum_type_tests.rs',
 'src/frontend/oir/owned/source/hir_budget.rs',
 'src/frontend/oir/owned/source/hir_budget_tests.rs',
 'src/frontend/oir/owned/source/resolver_enum_tests.rs',
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs',
 'src/frontend/oir/owned/source/resolver_paid_tests.rs',
 'src/frontend/oir/owned/source/resolver_storage.rs',
 'src/frontend/oir/owned/source/resolver_storage_tests.rs',
 'src/frontend/oir/owned/source/type_storage.rs',
 'src/frontend/oir/owned/source/type_storage_tests.rs',
 'src/frontend/oir/owned_types/enum_integration_tests.rs',
 'src/frontend/oir/owned_types/enums.rs',
 'src/frontend/parser/enum_syntax_tests.rs',
 'src/frontend/parser/enums.rs',
 'src/frontend/project/budget_real_null_observer.rs',
 'src/frontend/project/enum_carrier_tests.rs',
 'src/frontend/project/enum_index_tests.rs',
 'tests/fixtures/bounded_enum_scanner/main.ox',
 'tests/fixtures/bounded_enum_scanner/scanner.ox')
ENUM_SCANNER_PATHS = ('tests/fixtures/bounded_enum_scanner/main.ox', 'tests/fixtures/bounded_enum_scanner/scanner.ox')
ENUM_SCANNER_INCLUDERS = ('src/frontend/enum_public_tests.rs', 'src/frontend/oir/owned/source/enum_native_source_tests.rs')
PROJECTED_AUTHORITY_SHA = 'f3d9ea09236fd17532cf896ae8df510945a93f5d7b576295cfe05c030d809bdc'
PROJECTED_AUTHORITY_BYTES = 48552
PROJECTED_PATCH_SHA = '55d60b92bc3cb828a4cb1ddb610ef74bc10afa65e95a7668476048b1fb2ecf73'
PROJECTED_PATCH_BYTES = 124986
PROJECTED_PATHS = ('src/frontend/ast.rs',
 'src/frontend/format/ast_tests.rs',
 'src/frontend/oir/owned/array_native_tests.rs',
 'src/frontend/oir/owned/array_reference_tests.rs',
 'src/frontend/oir/owned/budget.rs',
 'src/frontend/oir/owned/composition_reference_tests.rs',
 'src/frontend/oir/owned/consumer_fixtures.rs',
 'src/frontend/oir/owned/consumer_pilot.rs',
 'src/frontend/oir/owned/consumer_tests.rs',
 'src/frontend/oir/owned/denial_tests.rs',
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/execute_tests.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/native_heldout_review.rs',
 'src/frontend/oir/owned/native_tests.rs',
 'src/frontend/oir/owned/oracle_tests.rs',
 'src/frontend/oir/owned/plan.rs',
 'src/frontend/oir/owned/projected_slice_native_tests.rs',
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
 'src/frontend/oir/owned/reviewer_heldout.rs',
 'src/frontend/oir/owned/reviewer_reference_tests.rs',
 'src/frontend/oir/owned/shape.rs',
 'src/frontend/oir/owned/source/hir.rs',
 'src/frontend/oir/owned/source/lower.rs',
 'src/frontend/oir/owned/source/mod.rs',
 'src/frontend/oir/owned/source/native_resource_tests.rs',
 'src/frontend/oir/owned/source/projected_slice_raw_tests.rs',
 'src/frontend/oir/owned/source/resolve.rs',
 'src/frontend/oir/owned/source/resource_fixtures.rs',
 'src/frontend/oir/owned/source/reviewer_heldout.rs',
 'src/frontend/oir/owned/source/reviewer_resource_runtime.rs',
 'src/frontend/oir/owned/source/runtime_resource_tests.rs',
 'src/frontend/oir/owned/source/slice_raw_tests.rs',
 'src/frontend/oir/owned/source/typeck.rs',
 'src/frontend/oir/owned/storage.rs',
 'src/frontend/oir/owned/tests.rs',
 'src/frontend/owned_syntax_tests.rs',
 'src/frontend/parser.rs',
 'tests/typed_record_composition.rs')
PROJECTED_ADDITIONS = ('src/frontend/oir/owned/projected_slice_native_tests.rs', 'src/frontend/oir/owned/source/projected_slice_raw_tests.rs')
UNARY_AUTHORITY_SHA = 'ed2d16dd5b24a55005e399630f3ad7402017fca9e8615b98d232d273ec418a31'
UNARY_AUTHORITY_BYTES = 36993
UNARY_PATCH_SHA = '4a1e4bfa577ff02bb3c5320eb1994b281831929692acd347daf243b15ae31796'
UNARY_PATCH_BYTES = 72327
UNARY_PATHS = ('src/frontend/ast.rs',
 'src/frontend/format.rs',
 'src/frontend/format/ast_tests.rs',
 'src/frontend/hir.rs',
 'src/frontend/oir/execute.rs',
 'src/frontend/oir/lower.rs',
 'src/frontend/oir/mod.rs',
 'src/frontend/oir/native.rs',
 'src/frontend/oir/negation_raw_tests.rs',
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/negation_raw_tests.rs',
 'src/frontend/oir/owned/source/array_pipeline_rows.rs',
 'src/frontend/oir/owned/source/candidate_adapter.rs',
 'src/frontend/oir/owned/source/hir.rs',
 'src/frontend/oir/owned/source/lower.rs',
 'src/frontend/oir/owned/source/resolve.rs',
 'src/frontend/oir/owned/source/reviewer_heldout.rs',
 'src/frontend/oir/owned/source/typeck.rs',
 'src/frontend/oir/source/association.rs',
 'src/frontend/oir/unary_source_tests.rs',
 'src/frontend/oir/verify.rs',
 'src/frontend/parser.rs',
 'src/frontend/typeck.rs')
UNARY_ADDITIONS = ('src/frontend/oir/negation_raw_tests.rs', 'src/frontend/oir/owned/negation_raw_tests.rs', 'src/frontend/oir/unary_source_tests.rs')
COMPOSITION_AUTHORITY_SHA = 'f387deb3d73643cf51109e1aee7a59717c12cfe1ac70cd3a6e14f1aafa01ee8a'
COMPOSITION_AUTHORITY_BYTES = 47077
COMPOSITION_PATCH_SHA = '5862ed320a9823b20eb1854b888fd2f66d3498cf58289fecac469a087cef09a6'
COMPOSITION_PATCH_BYTES = 238025
COMPOSITION_PATHS = ('fixtures/typed-record-composition-samples/main.ox',
 'fixtures/typed-record-composition-samples/model.ox',
 'fixtures/typed-record-composition-samples/ops.ox',
 'src/frontend/ast.rs',
 'src/frontend/format.rs',
 'src/frontend/format/ast_tests.rs',
 'src/frontend/oir/owned/array_observe.rs',
 'src/frontend/oir/owned/budget.rs',
 'src/frontend/oir/owned/cfg.rs',
 'src/frontend/oir/owned/composition_native_tests.rs',
 'src/frontend/oir/owned/composition_reference_tests.rs',
 'src/frontend/oir/owned/composition_verifier_tests.rs',
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/flow.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/native_tests.rs',
 'src/frontend/oir/owned/plan.rs',
 'src/frontend/oir/owned/shape.rs',
 'src/frontend/oir/owned/source/array_pipeline_rows.rs',
 'src/frontend/oir/owned/source/association.rs',
 'src/frontend/oir/owned/source/budget.rs',
 'src/frontend/oir/owned/source/candidate_adapter.rs',
 'src/frontend/oir/owned/source/candidate_native.rs',
 'src/frontend/oir/owned/source/hir.rs',
 'src/frontend/oir/owned/source/lower.rs',
 'src/frontend/oir/owned/source/resolve.rs',
 'src/frontend/oir/owned/source/reviewer_heldout.rs',
 'src/frontend/oir/owned/source/tests.rs',
 'src/frontend/oir/owned/source/typeck.rs',
 'src/frontend/oir/owned/tests.rs',
 'src/frontend/oir/owned_types.rs',
 'src/frontend/oir/owned_types/array_tests.rs',
 'src/frontend/oir/owned_types/composition_tests.rs',
 'src/frontend/owned_syntax_tests.rs',
 'src/frontend/parser.rs',
 'src/frontend/parser/activation_tests.rs',
 'src/frontend/parser/project_tests.rs',
 'src/frontend/project/unit2_tests.rs',
 'tests/typed_record_composition.rs')
COMPOSITION_ADDITIONS = ('src/frontend/oir/owned/composition_native_tests.rs',
 'src/frontend/oir/owned/composition_reference_tests.rs',
 'src/frontend/oir/owned/composition_verifier_tests.rs',
 'src/frontend/oir/owned_types/composition_tests.rs',
 'fixtures/typed-record-composition-samples/main.ox',
 'fixtures/typed-record-composition-samples/model.ox',
 'fixtures/typed-record-composition-samples/ops.ox',
 'tests/typed_record_composition.rs')
DIVISION_SOURCE_SHA = 'd3f3d2c8dc254bdb2b86381325a943925a39fde0eb2b89a10d1de8e6bfbd7f33'
DIVISION_SOURCE_BYTES = 35161
SLICES_AUTHORITY_SHA = '2e8dc2ab5506e179ffe5628e8a46eb6ec362ddb2e26a8a007800eb7029f3069f'
SLICES_AUTHORITY_BYTES = 55961
SLICES_PATCH_SHA = '7e41c881086ab6816d302177aad5ea580547a7577ff1e0c0055843f59ba4de20'
SLICES_PATCH_BYTES = 134654
SLICES_BASE = 'c5798a232ebdacaf720d580007ee8d760957a081'
SLICES_HEAD = '03aead9755b1dd6aaec2b4b165ee3881a7a1f7b7'
SLICES_TREE = '450f016ed57bc3d960e0857bb8253e71a8aa718a'
SLICES_PATHS = (
    'src/frontend/ast.rs',
    'src/frontend/declaration_index.rs',
    'src/frontend/declaration_index/source_owner.rs',
    'src/frontend/declaration_index/tests.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/hir.rs',
    'src/frontend/oir/owned/array_native_tests.rs',
    'src/frontend/oir/owned/array_reference_boundary_tests.rs',
    'src/frontend/oir/owned/array_reference_tests.rs',
    'src/frontend/oir/owned/array_tests.rs',
    'src/frontend/oir/owned/consumer_fixtures.rs',
    'src/frontend/oir/owned/consumer_pilot.rs',
    'src/frontend/oir/owned/denial_tests.rs',
    'src/frontend/oir/owned/execute.rs',
    'src/frontend/oir/owned/execute_tests.rs',
    'src/frontend/oir/owned/flow.rs',
    'src/frontend/oir/owned/mod.rs',
    'src/frontend/oir/owned/native.rs',
    'src/frontend/oir/owned/native_heldout_review.rs',
    'src/frontend/oir/owned/native_tests.rs',
    'src/frontend/oir/owned/oracle_tests.rs',
    'src/frontend/oir/owned/plan.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned/reviewer_heldout.rs',
    'src/frontend/oir/owned/reviewer_reference_tests.rs',
    'src/frontend/oir/owned/shape.rs',
    'src/frontend/oir/owned/slice_native_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/array_types_tests.rs',
    'src/frontend/oir/owned/source/candidate_adapter.rs',
    'src/frontend/oir/owned/source/diagnostic.rs',
    'src/frontend/oir/owned/source/hir.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/mod.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/resource_fixtures.rs',
    'src/frontend/oir/owned/source/reviewer_heldout.rs',
    'src/frontend/oir/owned/source/slice_raw_tests.rs',
    'src/frontend/oir/owned/source/slice_tests.rs',
    'src/frontend/oir/owned/source/tests.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned/tests.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/array_tests.rs',
    'src/frontend/parser.rs',
    'src/frontend/parser/array_syntax_tests.rs',
    'src/frontend/parser/arrays.rs',
)
SLICES_ADDITIONS = (
    'src/frontend/oir/owned/slice_native_tests.rs',
    'src/frontend/oir/owned/source/slice_raw_tests.rs',
    'src/frontend/oir/owned/source/slice_tests.rs',
)
COMBINED_SOURCE_SHA = '221524ad3faf7ea8e8b336cf8497a2eb7e2fbe476a1e829f510b2ee98dc82487'
COMBINED_SOURCE_BYTES = 35021
DIVISION_AUTHORITY_SHA = 'f2a848cf361ba2907d1f1e437256a28c0996189de9228de148f041a0ce9c0164'
DIVISION_AUTHORITY_BYTES = 29723
DIVISION_PATCH_SHA = '65319908325ce79bd46fb6014b0392b697e3a9d16447562d213dd882a0e2efb1'
DIVISION_PATCH_BYTES = 49895
DIVISION_BASE = '8a3b8683d911bdabfcdc7ca7d3ba867f6235ded3'
DIVISION_HEAD = '2c46521caa902b2afb88ef6b7bae58b9a1382776'
DIVISION_TREE = '7a74bf86edb53469dbcfd7839a8d3717a0d59a9a'
DIVISION_PATHS = (
    'src/frontend/ast.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/lexer.rs',
    'src/frontend/oir/arithmetic_tests.rs',
    'src/frontend/oir/execute.rs',
    'src/frontend/oir/mod.rs',
    'src/frontend/oir/native.rs',
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/execute.rs',
    'src/frontend/oir/owned/native.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/reviewer_heldout.rs',
    'src/frontend/parser.rs',
    'src/frontend/parser/arrays.rs',
)
FORMATTER_SOURCE_SHA = '69d89c46f23a99f7dc20911a4054cde7d97a98352d3fc1349e63ee7949ffcf06'
FORMATTER_SOURCE_BYTES = 23052
COMBINED_AUTHORITY_SHA = 'f28aae703e7f3c1010d91e2f53a4f66a9f728fe5248edf1f4832f12f59d90670'
COMBINED_AUTHORITY_BYTES = 19738
COMBINED_PATCH_SHA = '8ef58e282f1e37a7c04e653222fb74cb40f144aaa4364723773a608df2182683'
COMBINED_PATCH_BYTES = 376315
COMBINED_BASE = '595f681c2a906d686ddea90c65d060cff97e0a75'
COMBINED_HEAD = 'a5fb98b4f1ad2fa95ee6e4637f4e9d7700cbe909'
COMBINED_TREE = 'b30b0628c45e4a308bbb0ae5b35122794cd7ac12'
COMBINED_PATHS = (
    'src/frontend/ast.rs',
    'src/frontend/declaration_index.rs',
    'src/frontend/declaration_index/source_owner.rs',
    'src/frontend/format.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/format/resource_tests.rs',
    'src/frontend/hir.rs',
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/array_native_tests.rs',
    'src/frontend/oir/owned/array_tests.rs',
    'src/frontend/oir/owned/mod.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned/source/array_consumer_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/array_pipeline_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline_transport.rs',
    'src/frontend/oir/owned/source/array_type_controls.rs',
    'src/frontend/oir/owned/source/array_types_tests.rs',
    'src/frontend/oir/owned/source/budget.rs',
    'src/frontend/oir/owned/source/diagnostic.rs',
    'src/frontend/oir/owned/source/hir.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/mod.rs',
    'src/frontend/oir/owned/source/program.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/tests.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned/tests.rs',
    'src/frontend/oir/owned/verified.rs',
    'src/frontend/parser.rs',
    'src/frontend/parser/array_syntax_tests.rs',
    'src/frontend/parser/arrays.rs',
    'src/frontend/parser/project_tests.rs',
    'src/frontend/project.rs',
    'src/frontend/project/array_syntax_tests.rs',
    'src/frontend/project/budget.rs',
    'src/main.rs',
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
)
COMBINED_SOURCE_ADDITIONS = (
    'src/frontend/oir/owned/source/array_consumer_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/array_pipeline_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline_transport.rs',
    'src/frontend/oir/owned/source/array_type_controls.rs',
    'src/frontend/oir/owned/source/array_types_tests.rs',
    'src/frontend/parser/array_syntax_tests.rs',
    'src/frontend/parser/arrays.rs',
    'src/frontend/project/array_syntax_tests.rs',
)
COMBINED_FIXTURE_ADDITIONS = (
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
)
COMBINED_ADDITIONS = COMBINED_SOURCE_ADDITIONS + COMBINED_FIXTURE_ADDITIONS
COMPILE_FIXTURE_SOURCE = 'src/frontend/oir/owned/source/array_types_tests.rs'
COMBINED_COMPILE_FIXTURE_SOURCE_SHA = '10687b76ac4c048d21467653b55a4c322ac011209f6d1ebd503ce2fc778810cc'
COMBINED_COMPILE_FIXTURE_SOURCE_BYTES = 44176
COMPILE_FIXTURE_SOURCE_SHA = '93dd962176ccf43c4bfd67b5fd1d138b651f4e06e3a3c7ffd25691c09f3a5dab'
COMPILE_FIXTURE_SOURCE_BYTES = 44194
COMPILE_FIXTURE_PATTERN = b'include_str!\\(concat!\\(env!\\("CARGO_MANIFEST_DIR"\\), "/(tests/fixtures/fixed_array_source_unit3/(?:contracts-v2|typing-contracts-v1)/fixtures/[a-z0-9-]+/main\\.ox)"\\)\\)'
COMPILE_FIXTURE_REFERENCES_SHA = 'c93538ccae1080771921a761412b3d5380a584b663d6dcc719cfc50b98380732'

RETAINED_NON_SOURCE_PATHS = (
    'Cargo.lock',
    'Cargo.toml',
    'build.rs',
    'compiler/main.ox',
    'compiler/providers.toml',
    'fixtures/owned_source/batch.ox',
    'rfcs/0014-owned-structs-call-borrows.md',
    'stdlib/frontend/bytecode.ox',
    'tests/typed_frontend.rs',
    'tests/typed_project_dispatch.rs',
)
PREDECESSOR_SOURCE_SHA = '7c3de8673eca2bf2267251a9b3235a123bcefb1538785f3400a1fa0d073c5bb8'
FORMATTER_AUTHORITY_SHA = 'f060dd4e264a7261517f496176d9d3def438a1e151616e313972db0545f9b4d2'
FORMATTER_PATCH_SHA = '8e3bb083c6fbf8846a99476a57a80cb19c7163e5ebbabbd7f3e0305f9ac752b4'
FORMATTER_PATCH_BYTES = 75550
FORMATTER_PATHS = ('src/frontend/driver.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/format/resource_tests.rs', 'src/frontend/format_cli.rs', 'src/frontend/mod.rs', 'src/frontend/options.rs', 'src/frontend/source.rs')
FORMATTER_ADDITIONS = ('src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/format/resource_tests.rs', 'src/frontend/format_cli.rs')
PATCH_SHA = '63055a4b1a2cb63ce6a160a53e5c8131c4c288c198cd9af6ea421b5c2931fc18'
PATCH_BYTES = 605300
PATCH_PREFIX_BYTES = 28881
PATCH_PREFIX_SHA = "04f0588360aac12b96cd69a34b282329ea696eb69d7b979c8ffc385b7a42aab8"
SOURCE_DELTA_BASE = "0ef3be1df3643febdff1f859a4eb1ce567ab8164"
SOURCE_DELTA_RECIPE = "git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE CHECKPOINT -- PATHS"
PATCH_PATHS = (
    'src/frontend/driver.rs',
    'src/frontend/oir/project.rs',
    'src/frontend/parser.rs',
    'src/frontend/parser/activation_tests.rs',
    'src/frontend/parser/project_tests.rs',
    'src/frontend/project.rs',
    'src/frontend/project/unit2_tests.rs',
    'tests/typed_frontend.rs',
    'tests/typed_project_dispatch.rs',
    'src/frontend/declaration_index.rs',
    'src/frontend/declaration_index/tests.rs',
    'src/frontend/diagnostic.rs',
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/array_native_tests.rs',
    'src/frontend/oir/owned/array_observe.rs',
    'src/frontend/oir/owned/array_reference_boundary_tests.rs',
    'src/frontend/oir/owned/array_reference_tests.rs',
    'src/frontend/oir/owned/array_tests.rs',
    'src/frontend/oir/owned/budget.rs',
    'src/frontend/oir/owned/cfg.rs',
    'src/frontend/oir/owned/consumer_fixtures.rs',
    'src/frontend/oir/owned/consumer_pilot.rs',
    'src/frontend/oir/owned/denial_tests.rs',
    'src/frontend/oir/owned/execute.rs',
    'src/frontend/oir/owned/execute_tests.rs',
    'src/frontend/oir/owned/flow.rs',
    'src/frontend/oir/owned/mod.rs',
    'src/frontend/oir/owned/native.rs',
    'src/frontend/oir/owned/native_heldout_review.rs',
    'src/frontend/oir/owned/native_tests.rs',
    'src/frontend/oir/owned/oracle_tests.rs',
    'src/frontend/oir/owned/origin_tests.rs',
    'src/frontend/oir/owned/plan.rs',
    'src/frontend/oir/owned/reviewer_array_observer_tests.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned/reviewer_heldout.rs',
    'src/frontend/oir/owned/reviewer_origins.rs',
    'src/frontend/oir/owned/reviewer_reference_tests.rs',
    'src/frontend/oir/owned/shape.rs',
    'src/frontend/oir/owned/source/association.rs',
    'src/frontend/oir/owned/source/budget.rs',
    'src/frontend/oir/owned/source/candidate_adapter.rs',
    'src/frontend/oir/owned/source/candidate_mutations.rs',
    'src/frontend/oir/owned/source/candidate_native.rs',
    'src/frontend/oir/owned/source/diagnostic.rs',
    'src/frontend/oir/owned/source/hir.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/reviewer_heldout.rs',
    'src/frontend/oir/owned/source/reviewer_source.rs',
    'src/frontend/oir/owned/source/tests.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned/tests.rs',
    'src/frontend/oir/owned/verified.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/array_tests.rs',
)

SEMANTIC_HELPER = 'enum_enabled_qualified_values_v1.py'
SEMANTIC_DESCRIPTOR = 'enum-enabled-qualified-values-v1.json'
SEMANTIC_HELPER_SHA = '999e9f8cd75ae2010a11d20a40c357bf28d42b658d6293e93cb73a36cb665288'
SEMANTIC_DESCRIPTOR_SHA = 'aad90776af8cda1a305efc9581d8fcbb40e4eedb8da0ca9aef703fc4fd8af425'
UNIT2_COMPARATOR = 'semantic/compare.py'
UNIT2_FROZEN_COMPARATOR = 'semantic/compare_before_enum_enabled_qualified_values.py'
UNIT2_SEMANTIC_HELPER = 'semantic/enum_enabled_qualified_values_v1.py'
UNIT2_SEMANTIC_DESCRIPTOR = 'semantic/enum-enabled-qualified-values-v1.json'
UNIT2_COMPARATOR_ORIGINAL_SHA = '6e6642dfc28e37bf5c8c147719f52088ce3e2e5037cf15356ff287d6dd01df5c'
UNIT2_COMPARATOR_DERIVED_SHA = '677e7afed200527760c9b8480f798fb94c6446eec845f9f664c2c556c50a310f'
UNIT2_COMPARATOR_SEAMS = ((b'results=[]\n',
  b"enum_spec=importlib.util.spec_from_file_location('enum_enabled_qualified_values_v1',ROOT/'en"
  b"um_enabled_qualified_values_v1.py')\nenum_module=importlib.util.module_from_spec(enum_spec);e"
  b"num_spec.loader.exec_module(enum_module)\namendment=enum_module.Amendment(ROOT.parent/'source"
  b"-inputs.json',ROOT/'corpus.jsonl.gz',ROOT/'enum-enabled-qualified-values-v1.json')\n# Preserv"
  b'e an independent original-oracle report before current comparison.\nimport subprocess\nhistori'
  b"cal_path=Path(sys.argv[3]).with_name(profile+'-comparison-before-enum-enabled-qualified-valu"
  b"es.json')\nhistorical_stdout=historical_path.with_suffix('.stdout')\nhistorical_stderr=histori"
  b"cal_path.with_suffix('.stderr')\nhistorical_argv=[sys.executable,'-B',str(ROOT/'compare_befor"
  b"e_enum_enabled_qualified_values.py'),profile,str(actual_path),str(historical_path)]\nwith his"
  b"torical_stdout.open('xb') as sink, historical_stderr.open('xb') as errors:\n    historical_pr"
  b'ocess=subprocess.run(historical_argv,stdout=sink,stderr=errors,timeout=120)\nassert historica'
  b"l_process.returncode==0, 'frozen comparison execution failed'\nhistorical_report=json.loads(h"
  b"istorical_path.read_bytes())\nassert historical_report['profile']==profile and historical_rep"
  b"ort['cases']==3603, 'frozen comparison profile/count'\nassert historical_report['counts']=={'"
  b"match':3599,'mismatch':4}, 'unexpected frozen comparison changes'\nassert len(historical_repo"
  b"rt['results'])==3603 and {r['case'] for r in historical_report['results']}==set(corpus), 'fr"
  b"ozen comparison exact roster'\nassert {r['case'] for r in historical_report['results'] if r['"
  b"status']=='mismatch'}==set(enum_module.CASE_IDS), 'unapproved frozen mismatch'\nhistorical_bi"
  b"nding={'identity':enum_module.IDENTITY,'argv':historical_argv,'status':historical_process.re"
  b"turncode,\n                    'comparison':{'path':str(historical_path),'bytes':historical_p"
  b"ath.stat().st_size,'sha256':enum_module.sha(historical_path.read_bytes())},\n                "
  b"    'stdout':{'path':str(historical_stdout),'bytes':historical_stdout.stat().st_size,'sha256"
  b"':enum_module.sha(historical_stdout.read_bytes())},\n                    'stderr':{'path':str"
  b"(historical_stderr),'bytes':historical_stderr.stat().st_size,'sha256':enum_module.sha(histor"
  b"ical_stderr.read_bytes())},\n                    'normalized_sha256':enum_module.sha(actual_p"
  b"ath.read_bytes()),'historical_mismatches':list(enum_module.CASE_IDS),\n                    'h"
  b"istorical_qualification_claim':False}\nresults=[]\n"),
 (b"    elif cohort in ('parser','grammar-amendment'):\n        root=ROOT/('parser-cases' if coho"
  b"rt=='parser' else 'grammar-amendment-cases')\n        expected=copy.deepcopy(corpus[item['cas"
  b"e']]['expected'])\n",
  b"    elif cohort in ('parser','grammar-amendment'):\n        root=ROOT/('parser-cases' if coho"
  b"rt=='parser' else 'grammar-amendment-cases')\n        expected=copy.deepcopy(corpus[item['cas"
  b"e']]['expected'])\n        expected=amendment.project_unit2(corpus[item['case']],expected,ite"
  b'm,failures)\n'),
 (b"report={'profile':profile,'normalized_observations':str(actual_path),'cases':len(results),'c"
  b"ounts':counts,'results':results}\n",
  b"report={'profile':profile,'normalized_observations':str(actual_path),'cases':len(results),'c"
  b"ounts':counts,'results':results}\nreport['semantic_amendment']=amendment.unit2_report(results"
  b")\nreport['historical_semantic_comparison']=historical_binding\n"))

INDEX_RESOURCE = 'archive/resource/resource-review-tests.rs'
ENUM_INDEX_RESOURCE_VERSION = 'unit2-enum-free-index-resource-v1'
ENUM_INDEX_RESOURCE_AUTHORITY_SHA = '9e81f265f1cef93ee41ca326580d19f2728133569d4453310a4a086545031527'
ENUM_INDEX_RESOURCE_AUTHORITY_BYTES = 10804
ENUM_INDEX_RESOURCE_ORIGINAL_SHA = 'a5472f85b6dca1ed9e649c96575b0cf8a45fe37f204040806cd0747a237f1669'
ENUM_INDEX_RESOURCE_ORIGINAL_BYTES = 22687
ENUM_INDEX_RESOURCE_DERIVED_SHA = '289d310395a7abb701c03d3ca065f87accba6ed02884fbcf21e2d13235c08dde'
ENUM_INDEX_RESOURCE_DERIVED_BYTES = 22732
ENUM_INDEX_RESOURCE_CONTROLS = ('reviewer_nine_independent_space_cases',
 'reviewer_independent_exact_admission_and_failure_order',
 'reviewer_fourteen_actual_index_reserve_failures',
 'reviewer_actual_layout_and_source_node_envelope')
ENUM_INDEX_RESOURCE_SEAMS = ((b'],[0,0,0,1,0,0,60,4]),', b'],[0,0,0,1,0,0,68,4]),'),
 (b'],[1,0,0,1,0,1,112,8]),', b'],[1,0,0,1,0,1,120,8]),'),
 (b'],[0,1,0,1,0,1,128,8]),', b'],[0,1,0,1,0,1,136,8]),'),
 (b'],[0,1,3,1,0,1,176,8]),', b'],[0,1,3,1,0,1,184,8]),'),
 (b'],[0,0,0,2,0,1,164,12]),', b'],[0,0,0,2,0,1,180,12]),'),
 (b'],[0,1,0,2,1,2,272,28]),', b'],[0,1,0,2,1,2,288,28]),'),
 (b'],[1,0,0,2,1,2,256,28]),', b'],[1,0,0,2,1,2,272,28]),'),
 (b'],[1,1,0,2,1,3,324,32]),', b'],[1,1,0,2,1,3,340,32]),'),
 (b'],[1,1,0,2,2,3,364,44]),', b'],[1,1,0,2,2,3,380,44]),'),
 (b'assert_eq!(al.attempts,14);', b'assert_eq!(al.attempts,16);'),
 (b'al.trace.iter().take(10)', b'al.trace.iter().take(12)'),
 (b'al.trace.iter().skip(10)', b'al.trace.iter().skip(12)'),
 (b'let retained=112+size_of::<DeclarationIndex', b'let retained=120+size_of::<DeclarationIndex'),
 (b'assert_eq!(alloc.attempts,14);', b'assert_eq!(alloc.attempts,16);'),
 (b'("index modules",2,60)',
  b'("index enums",0,20),("index variants",0,12),("index modules",2,68)'),
 (b'for fail in 1..=14 {', b'for fail in 1..=16 {'),
 (b'[36,12,28,16,60,20,16,8]', b'[36,12,28,16,68,20,16,8]'))

OBSERVER = 'semantic/observer.rs'
OBSERVER_ADAPTER_VERSION = 'unit2-record-aggregate-observer-v1'
OBSERVER_ORIGINAL_SHA = 'f2403aace53b6255a94b8b3ec0290db025638c5af94571d5729355672681fb00'
OBSERVER_DERIVED_SHA = 'ddeff8bd0acfaa9af0301c74f3aabd26ef69954eb5a216fdd9cc21e1837901a3'
OBSERVER_SEAMS = (
    (b'use oir::owned_types::{BorrowKind, FieldId, ParameterTy, ValueTy};', b'use oir::owned_types::{AggregateTy, BorrowKind, FieldId, ParameterTy, ValueTy};'),
    (b'fn value_type(value: ValueTy) -> String {', b'fn current_unit2_record_ordinal(aggregate: AggregateTy) -> usize {\n    match aggregate {\n        AggregateTy::Record(record) => record.0,\n        AggregateTy::FixedArray(_) => panic!("current Unit2 observer excludes fixed-array projection"),\n    }\n}\n\n#[test]\nfn current_unit2_aggregate_adapter_preserves_scalar_and_record_json() {\n    use oir::owned_types::RecordId;\n    assert_eq!(value_type(ValueTy::Scalar(hir::Ty::I32)), "{\\"kind\\":\\"scalar\\",\\"scalar\\":\\"I32\\"}");\n    assert_eq!(value_type(ValueTy::Owned(AggregateTy::Record(RecordId(7)))), "{\\"kind\\":\\"owned\\",\\"record\\":7}");\n    assert_eq!(parameter_type(ParameterTy::Reference { aggregate: AggregateTy::Record(RecordId(7)), kind: BorrowKind::Shared }), "{\\"kind\\":\\"shared\\",\\"record\\":7}");\n    assert_eq!(parameter_type(ParameterTy::Reference { aggregate: AggregateTy::Record(RecordId(7)), kind: BorrowKind::Exclusive }), "{\\"kind\\":\\"exclusive\\",\\"record\\":7}");\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]\nfn current_unit2_aggregate_adapter_denies_owned_array() {\n    let array = oir::owned_types::FixedArrayTy::check(hir::Ty::I32, 1).unwrap();\n    value_type(ValueTy::Owned(AggregateTy::FixedArray(array)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]\nfn current_unit2_aggregate_adapter_denies_shared_array() {\n    let array = oir::owned_types::FixedArrayTy::check(hir::Ty::I32, 0).unwrap();\n    parameter_type(ParameterTy::Reference { aggregate: AggregateTy::FixedArray(array), kind: BorrowKind::Shared });\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]\nfn current_unit2_aggregate_adapter_denies_exclusive_array() {\n    let array = oir::owned_types::FixedArrayTy::check(hir::Ty::Bool, 1024).unwrap();\n    parameter_type(ParameterTy::Reference { aggregate: AggregateTy::FixedArray(array), kind: BorrowKind::Exclusive });\n}\n\nfn value_type(value: ValueTy) -> String {'),
    (b'ValueTy::Owned(record) => object(vec![("kind",q("owned")),("record",number(record.0))]),', b'ValueTy::Owned(aggregate) => object(vec![("kind",q("owned")),("record",number(current_unit2_record_ordinal(aggregate)))]),'),
    (b'ParameterTy::Reference { record, kind } => object(vec![("kind",q(match kind { BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"})),("record",number(record.0))]),', b'ParameterTy::Reference { aggregate, kind } => object(vec![("kind",q(match kind { BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"})),("record",number(current_unit2_record_ordinal(aggregate)))]),'),
)


BORROWED_OBSERVER_ADAPTER_VERSION = 'unit2-record-borrowed-observer-v2'
BORROWED_OBSERVER_DERIVED_SHA = 'abe639a07549c67db03df2e1d549173c327056f42e49c87795032a5266c2883b'
BORROWED_OBSERVER_DERIVED_BYTES = 17039
BORROWED_OBSERVER_SCOPE = 'Seven exact substitutions after validating the unchanged four-seam aggregate adapter: BorrowedTy import, four exact reference constructors, borrowed Reference projection, and explicit fail-closed borrowed-record projection with shared/exclusive ScalarSlice controls. Owner ValueTy remains aggregate-based. Scalar/record JSON and frozen expectations remain unchanged; Exact(FixedArray) and ScalarSlice projection panic.'
BORROWED_OBSERVER_SEAMS = (
    (b'use oir::owned_types::{AggregateTy, BorrowKind, FieldId, ParameterTy, ValueTy};', b'use oir::owned_types::{AggregateTy, BorrowedTy, BorrowKind, FieldId, ParameterTy, ValueTy};'),
    (b'ParameterTy::Reference { aggregate: AggregateTy::Record(RecordId(7)), kind: BorrowKind::Shared }', b'ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(7))), kind: BorrowKind::Shared }'),
    (b'ParameterTy::Reference { aggregate: AggregateTy::Record(RecordId(7)), kind: BorrowKind::Exclusive }', b'ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::Record(RecordId(7))), kind: BorrowKind::Exclusive }'),
    (b'ParameterTy::Reference { aggregate: AggregateTy::FixedArray(array), kind: BorrowKind::Shared }', b'ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::FixedArray(array)), kind: BorrowKind::Shared }'),
    (b'ParameterTy::Reference { aggregate: AggregateTy::FixedArray(array), kind: BorrowKind::Exclusive }', b'ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::FixedArray(array)), kind: BorrowKind::Exclusive }'),
    (b'ParameterTy::Reference { aggregate, kind } => object(vec![("kind",q(match kind { BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"})),("record",number(current_unit2_record_ordinal(aggregate)))]),', b'ParameterTy::Reference { referent, kind } => object(vec![("kind",q(match kind { BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"})),("record",number(current_unit2_borrowed_record_ordinal(referent)))]),'),
    (b'fn value_type(value: ValueTy) -> String {', b'fn current_unit2_borrowed_record_ordinal(referent: BorrowedTy) -> usize {\n    match referent {\n        BorrowedTy::Exact(AggregateTy::Record(record)) => record.0,\n        BorrowedTy::Exact(AggregateTy::FixedArray(_)) => panic!("current Unit2 observer excludes fixed-array projection"),\n        BorrowedTy::ScalarSlice(_) => panic!("current Unit2 observer excludes scalar-slice projection"),\n    }\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes scalar-slice projection")]\nfn current_unit2_aggregate_adapter_denies_shared_slice() {\n    parameter_type(ParameterTy::Reference { referent: BorrowedTy::ScalarSlice(hir::Ty::I32), kind: BorrowKind::Shared });\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes scalar-slice projection")]\nfn current_unit2_aggregate_adapter_denies_exclusive_slice() {\n    parameter_type(ParameterTy::Reference { referent: BorrowedTy::ScalarSlice(hir::Ty::Unit), kind: BorrowKind::Exclusive });\n}\n\nfn value_type(value: ValueTy) -> String {'),
)


ENUM_OBSERVER_ADAPTER_VERSION = 'unit2-record-enum-exclusion-observer-v3'
ENUM_OBSERVER_DERIVED_SHA = '5780a5b20f24d1f8bf5bcdb0ddae628be0f9cc53e10be527c490d6a427def085'
ENUM_OBSERVER_DERIVED_BYTES = 17611
ENUM_OBSERVER_SCOPE = 'Three exact substitutions after the unchanged borrowed observer: explicit owner-enum and borrowed-enum rejection arms, and owned/shared/exclusive enum rejection assertions within the existing scalar/record preservation control. Retains the six existing control names and frozen scalar/record JSON.'
ENUM_OBSERVER_SEAMS = ((b'        AggregateTy::Record(record) => record.0,',
  b'        AggregateTy::Record(record) => record.0,\n        AggregateTy::Enum(_) => panic!("cur'
  b'rent Unit2 observer excludes enum projection"),'),
 (b'        BorrowedTy::Exact(AggregateTy::Record(record)) => record.0,',
  b'        BorrowedTy::Exact(AggregateTy::Record(record)) => record.0,\n        BorrowedTy::Exac'
  b't(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection"),'),
 (b'    use oir::owned_types::RecordId;',
  b'    use oir::owned_types::{EnumId, RecordId};\n    assert!(std::panic::catch_unwind(|| value_'
  b'type(ValueTy::Owned(AggregateTy::Enum(EnumId(0))))).is_err());\n    for kind in [BorrowKind::'
  b'Shared, BorrowKind::Exclusive] {\n        assert!(std::panic::catch_unwind(|| parameter_type('
  b'ParameterTy::Reference {\n            referent: BorrowedTy::Exact(AggregateTy::Enum(EnumId(0)'
  b')), kind,\n        })).is_err());\n    }'))


class BindingError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise BindingError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def read_json(path):
    return json.loads(path.read_bytes())


def write_json(path, value):
    path.write_bytes(encoded(value))


def relative(name):
    require(isinstance(name, str) and name, "empty member")
    value = PurePosixPath(name)
    require(not value.is_absolute() and str(value) == name
            and all(x not in (".", "..") for x in value.parts), "noncanonical member")
    return value


def regular(root, name):
    path = Path(root)
    require(path.is_dir() and not path.is_symlink(), "invalid input root")
    for part in relative(name).parts:
        path = path / part
        require(not path.is_symlink(), "symlink input: " + name)
    require(path.is_file() and stat.S_ISREG(path.stat().st_mode), "missing regular input: " + name)
    return path


def entry(name, data):
    return {"path": name, "bytes": len(data), "sha256": digest(data)}


def members(root):
    result = []
    for parent, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(parent) / name
            require(not path.is_symlink(), "symlink member: " + str(path))
        result.extend((Path(parent) / name).relative_to(root).as_posix() for name in files)
    return sorted(result)


def check_entries(root, entries, exact=False):
    require(isinstance(entries, list) and entries, "zero input members")
    names = [x["path"] for x in entries]
    require(len(names) == len(set(names)), "duplicate member")
    if exact:
        require(members(root) == sorted(names), "missing or extra member")
    result = {}
    for item in entries:
        data = regular(root, item["path"]).read_bytes()
        require(entry(item["path"], data) == item, "changed input: " + item["path"])
        result[item["path"]] = data
    return result


def check_bytes(inputs, entries):
    require(set(inputs) == {x["path"] for x in entries}, "missing or extra reconstructed member")
    require(len(inputs) == len(entries), "duplicate reconstructed member")
    for item in entries:
        require(entry(item["path"], inputs[item["path"]]) == item,
                "changed reconstructed input: " + item["path"])


def inverse_patch(inputs, patch):
    """Apply the pinned git patch backwards with exact offsets and byte context."""
    return apply_inverse_patch(inputs, patch, PATCH_SHA, PATCH_BYTES, PATCH_PATHS)


def inverse_enum_patch(inputs, patch):
    """Restore exactly the frozen projected-array-slices source view."""
    return apply_inverse_patch(inputs, patch, ENUM_PATCH_SHA, ENUM_PATCH_BYTES, ENUM_PATHS)


def inverse_hir_import_patch(inputs, patch):
    """Remove only verified additive fixtures, then recover the exact prior source."""
    reduced = dict(inputs)
    for row in HIR_IMPORT_FIXTURE_ROWS:
        name = row["path"]
        require(name in reduced and entry(name, reduced[name]) == row,
                "changed HIR import fixture input: " + name)
        del reduced[name]
    return apply_inverse_patch(reduced, patch, HIR_IMPORT_PATCH_SHA,
                               HIR_IMPORT_PATCH_BYTES, HIR_IMPORT_PATHS)


def admit_hir_import(repo, package_bytes):
    """Admit the complete opt-in compiler view before any historical inverse."""
    current = json.loads(package_bytes["current-source.json"])
    raw = package_bytes["hir-import-authority.json"]
    require(digest(raw) == HIR_IMPORT_AUTHORITY_SHA and len(raw) == HIR_IMPORT_AUTHORITY_BYTES,
            "stale HIR import authority")
    require(digest(package_bytes["native-inventory-source.json"]) == NATIVE_INVENTORY_SOURCE_SHA
            and len(package_bytes["native-inventory-source.json"]) == NATIVE_INVENTORY_SOURCE_BYTES,
            "unapproved native inventory source manifest")
    authority = json.loads(raw)
    predecessor = json.loads(package_bytes["native-inventory-source.json"])
    require(authority["schema"] == "oxid-hir-import-source-transition-v1"
            and authority["recipe"] == "git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS"
            and authority["base_tree"] == current["hir_import_base_tree"] == predecessor["source_only_tree"] == HIR_IMPORT_BASE_TREE
            and authority["predecessor_source_head"] == predecessor["reviewed_source_head"] == NATIVE_INVENTORY_HEAD
            and authority["reviewed_source_head"] == current["reviewed_source_head"] == HIR_IMPORT_HEAD
            and authority["source_only_tree"] == current["source_only_tree"] == HIR_IMPORT_TREE
            and authority["current_source_sha256"] == CURRENT_SOURCE_SHA
            and authority["current_source_bytes"] == CURRENT_SOURCE_BYTES
            and authority["native_inventory_source_sha256"] == current["native_inventory_source_sha256"] == NATIVE_INVENTORY_SOURCE_SHA
            and authority["native_inventory_source_bytes"] == NATIVE_INVENTORY_SOURCE_BYTES
            and authority["native_inventory_authority_sha256"] == NATIVE_INVENTORY_AUTHORITY_SHA
            and authority["transition_patch_sha256"] == HIR_IMPORT_PATCH_SHA
            and authority["transition_patch_bytes"] == HIR_IMPORT_PATCH_BYTES
            and authority["transition_paths"] == list(HIR_IMPORT_PATHS)
            and authority["compiler_additions"] == list(HIR_IMPORT_ADDITIONS)
            and authority["fixture_additions"] == list(HIR_IMPORT_FIXTURES)
            and authority["removed_paths"] == []
            and (authority["current_source_members"], authority["native_inventory_source_members"],
                 authority["compiler_source_members"], authority["compiler_bodies"], current["hir_import_fixture_members"]) == (324, 266, 246, 249, 20),
            "stale HIR import transition authority")
    omit = {"files", "purpose", "reviewed_source_head", "source_only_tree", "hir_import_base_tree",
            "native_inventory_source_sha256", "hir_import_fixture_members"}
    require({k: v for k, v in current.items() if k not in omit}
            == {k: v for k, v in predecessor.items() if k not in omit}, "stale HIR import provenance")
    inputs = check_entries(repo, current["files"])
    require([r["path"] for r in current["files"]] == sorted(inputs), "unordered HIR import source inventory")
    before = {r["path"]: r for r in predecessor["files"]}
    require(set(inputs) == set(before) | set(HIR_IMPORT_ADDITIONS) | set(HIR_IMPORT_FIXTURES),
            "unexpected HIR import source membership")
    require([r["path"] for r in current["files"] if r != before.get(r["path"])]
            == sorted(list(HIR_IMPORT_PATHS) + list(HIR_IMPORT_FIXTURES)), "unexpected HIR import source delta")
    identities = []
    for name, data in inputs.items():
        require(regular(repo, name).stat().st_mode & 0o111 == 0, "changed input mode: " + name)
        identities.append({**entry(name, data), "mode": "100644",
            "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()})
    require(authority["current_input_identities"] == identities
            and authority["current_input_git_modes"] == [{"path": n, "mode": "100644"} for n in inputs],
            "stale HIR import complete input identities")
    require(authority["fixture_inputs"] == [r for r in identities if r["path"] in HIR_IMPORT_FIXTURES],
            "stale HIR import fixture identities")
    actual = [part + "/" + name for part in ("src", "native") for name in members(repo / part)]
    expected = [name for name in inputs if name.startswith(("src/", "native/"))]
    require(len(expected) == 246 and sorted(actual) == expected, "missing or extra compiler source member")
    fixture_root = "tests/fixtures/checked_hir_import"
    actual_fixtures = [fixture_root + "/" + name for name in members(repo / fixture_root)
                       if name.endswith(("-source.txt", "-success.bin"))]
    require(actual_fixtures == list(HIR_IMPORT_FIXTURES), "missing or extra HIR import fixture member")
    restored, touched = inverse_hir_import_patch(inputs, package_bytes["hir-import-transition.patch"])
    check_bytes(restored, predecessor["files"])
    changes = []
    for name in HIR_IMPORT_PATHS:
        row = {"path": name}
        for label, source in (("before", restored), ("after", inputs)):
            data = source.get(name)
            row[label] = None if data is None else {**entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        changes.append(row)
    require(authority["transition_inputs"] == changes, "stale HIR import transition input identities")
    return current, inputs, authority, restored, touched


def inverse_native_inventory_patch(inputs, patch):
    """Restore exactly the frozen Phase1 native storage source view."""
    return apply_inverse_patch(inputs, patch, NATIVE_INVENTORY_PATCH_SHA,
                               NATIVE_INVENTORY_PATCH_BYTES, NATIVE_INVENTORY_PATHS)


def inverse_native_storage_patch(inputs, patch):
    """Restore exactly the frozen bounded-stdout source view."""
    return apply_inverse_patch(inputs, patch, NATIVE_STORAGE_PATCH_SHA,
                               NATIVE_STORAGE_PATCH_BYTES, NATIVE_STORAGE_PATHS)


def inverse_stdout_patch(inputs, patch):
    """Restore exactly the frozen bounded-stdin source view."""
    return apply_inverse_patch(inputs, patch, STDOUT_PATCH_SHA, STDOUT_PATCH_BYTES, STDOUT_PATHS)


def inverse_stdin_patch(inputs, patch):
    """Restore exactly the frozen bounded-enum source view."""
    return apply_inverse_patch(inputs, patch, STDIN_PATCH_SHA, STDIN_PATCH_BYTES, STDIN_PATHS)


def inverse_projected_patch(inputs, patch):
    """Restore exactly the frozen unary source view."""
    return apply_inverse_patch(inputs, patch, PROJECTED_PATCH_SHA, PROJECTED_PATCH_BYTES, PROJECTED_PATHS)


def inverse_unary_patch(inputs, patch):
    """Restore exactly the frozen record-composition source view."""
    return apply_inverse_patch(inputs, patch, UNARY_PATCH_SHA, UNARY_PATCH_BYTES, UNARY_PATHS)


def inverse_composition_patch(inputs, patch):
    """Remove only the bounded composition delta, restoring exact slice inputs."""
    return apply_inverse_patch(inputs, patch, COMPOSITION_PATCH_SHA, COMPOSITION_PATCH_BYTES, COMPOSITION_PATHS)


def inverse_slices_patch(inputs, patch):
    """Remove the pinned borrowed-slices delta before division reconstruction."""
    return apply_inverse_patch(inputs, patch, SLICES_PATCH_SHA, SLICES_PATCH_BYTES, SLICES_PATHS)


def inverse_division_patch(inputs, patch):
    """Remove the pinned division delta before combined-source reconstruction."""
    return apply_inverse_patch(inputs, patch, DIVISION_PATCH_SHA, DIVISION_PATCH_BYTES, DIVISION_PATHS)


def inverse_combined_patch(inputs, patch):
    """Remove the pinned combined source delta before formatter reconstruction."""
    return apply_inverse_patch(inputs, patch, COMBINED_PATCH_SHA, COMBINED_PATCH_BYTES, COMBINED_PATHS)


def inverse_formatter_patch(inputs, patch):
    """Remove only the pinned formatter delta before historical reconstruction."""
    return apply_inverse_patch(inputs, patch, FORMATTER_PATCH_SHA, FORMATTER_PATCH_BYTES, FORMATTER_PATHS)


def apply_inverse_patch(inputs, patch, expected_sha, expected_bytes, expected_paths):
    require(digest(patch) == expected_sha and len(patch) == expected_bytes, "wrong transition patch")
    lines = patch.splitlines(keepends=True)
    at, touched = 0, []
    result = dict(inputs)
    while at < len(lines):
        match = re.fullmatch(rb"diff --git a/(\S+) b/(\S+)\n", lines[at])
        require(match is not None and match[1] == match[2], "invalid git patch header")
        name = match[1].decode("utf-8")
        relative(name)
        require(name in result and name not in touched, "missing or duplicate transition member")
        touched.append(name)
        at += 1
        added = lines[at] == b"new file mode 100644\n"
        if added:
            at += 1
        require(re.fullmatch(rb"index [0-9a-f]+\.\.[0-9a-f]+(?: 100644)?\n", lines[at]),
                "invalid git patch index")
        index = lines[at]
        at += 1
        if at == len(lines) or lines[at].startswith(b"diff --git "):
            require(added and index == b"index 0000000..e69de29\n" and result[name] == b"",
                    "invalid empty transition addition")
            del result[name]
            continue
        require(lines[at] == (b"--- /dev/null\n" if added else b"--- a/" + name.encode() + b"\n"),
                "invalid transition old path")
        require(lines[at + 1] == b"+++ b/" + name.encode() + b"\n", "invalid transition new path")
        at += 2
        before, after, cursor, hunks = result[name].splitlines(keepends=True), [], 0, 0
        while at < len(lines) and lines[at].startswith(b"@@ "):
            match = re.fullmatch(rb"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@[^\n]*\n", lines[at])
            require(match is not None, "invalid transition hunk")
            old_line, old_count, new_line, new_count = [int(v or b"1") for v in match.groups()]
            offset = max(new_line - 1, 0)
            require(cursor <= offset <= len(before), "transition offset out of bounds")
            after.extend(before[cursor:offset])
            cursor = offset
            require(len(after) == max(old_line - 1, 0), "transition old offset differs")
            at += 1
            removed = inserted = 0
            hunks += 1
            while at < len(lines) and not lines[at].startswith((b"@@ ", b"diff --git ")):
                line = lines[at]
                require(line[:1] in (b" ", b"+", b"-"), "unsupported transition record")
                if line[:1] in (b" ", b"+"):
                    require(cursor < len(before) and before[cursor] == line[1:],
                            "transition current context differs: " + name)
                    cursor += 1
                    removed += 1
                if line[:1] in (b" ", b"-"):
                    after.append(line[1:])
                    inserted += 1
                at += 1
            require(removed == new_count and inserted == old_count, "transition hunk cardinality differs")
        require(hunks > 0, "zero transition hunks")
        after.extend(before[cursor:])
        if added:
            require(not after, "inverse addition retained bytes")
            del result[name]
        else:
            result[name] = b"".join(after)
    require(tuple(touched) == expected_paths, "wrong transition scope")
    return result, touched


def adapt_unit2_observer(original):
    """Versioned current-only aggregate projection; frozen scalar/record JSON is retained."""
    require(digest(original) == OBSERVER_ORIGINAL_SHA, "wrong original Unit2 observer")
    result = original
    require(len(OBSERVER_SEAMS) == 4, "wrong Unit2 observer substitution count")
    for old, new in OBSERVER_SEAMS:
        require(result.count(old) == 1 and new not in result, "Unit2 observer seam drift")
        result = result.replace(old, new)
    require(digest(result) == OBSERVER_DERIVED_SHA, "wrong derived Unit2 observer")
    return result


def adapt_borrowed_unit2_observer(aggregate):
    """Derive the current borrowed-reference projection from the exact prior adapter."""
    require(digest(aggregate) == OBSERVER_DERIVED_SHA and len(aggregate) == 15986,
            "wrong predecessor Unit2 observer")
    require(len(BORROWED_OBSERVER_SEAMS) == 7, "wrong borrowed Unit2 observer substitution count")
    result = aggregate
    for old, new in BORROWED_OBSERVER_SEAMS:
        require(result.count(old) == 1 and new not in result, "borrowed Unit2 observer seam drift")
        result = result.replace(old, new)
    require(digest(result) == BORROWED_OBSERVER_DERIVED_SHA
            and len(result) == BORROWED_OBSERVER_DERIVED_BYTES, "wrong derived borrowed Unit2 observer")
    restored = result
    for old, new in reversed(BORROWED_OBSERVER_SEAMS):
        require(restored.count(new) == 1, "borrowed Unit2 observer reverse seam drift")
        restored = restored.replace(new, old)
    require(restored == aggregate, "borrowed Unit2 observer reverse identity differs")
    return result


def adapt_stdin_parser_resource(data, *, reverse=False):
    """Add only a closed std policy, preserving the exact enum predecessor."""
    before, after = (("derived", "predecessor") if reverse else ("predecessor", "derived"))
    old, new = ((STDIN_RESOURCE_SEAM, ENUM_RESOURCE_SEAM) if reverse
                else (ENUM_RESOURCE_SEAM, STDIN_RESOURCE_SEAM))
    require(entry(RESOURCE, data) == STDIN_RESOURCE_ADAPTER[before], "wrong stdin parser resource input")
    require(STDIN_RESOURCE_ADAPTER["substitution"] == {
        "old_sha256": digest(ENUM_RESOURCE_SEAM), "new_sha256": digest(STDIN_RESOURCE_SEAM), "count": 1,
    }, "stdin parser resource substitution identity differs")
    require(data.count(old) == 1 and new not in data, "stdin parser resource seam drift")
    result = data.replace(old, new, 1)
    require(entry(RESOURCE, result) == STDIN_RESOURCE_ADAPTER[after], "wrong stdin parser resource output")
    require(result.replace(new, old, 1) == data, "stdin parser resource reverse identity differs")
    return result


def adapt_enum_unit2_observer(borrowed):
    """Keep frozen scalar/record observations and reject each new enum projection."""
    require(digest(borrowed) == BORROWED_OBSERVER_DERIVED_SHA
            and len(borrowed) == BORROWED_OBSERVER_DERIVED_BYTES, "wrong predecessor enum Unit2 observer")
    require(len(ENUM_OBSERVER_SEAMS) == 3, "wrong enum Unit2 observer substitution count")
    result = borrowed
    for old, new in ENUM_OBSERVER_SEAMS:
        require(result.count(old) == 1 and new not in result, "enum Unit2 observer seam drift")
        result = result.replace(old, new)
    require(digest(result) == ENUM_OBSERVER_DERIVED_SHA and len(result) == ENUM_OBSERVER_DERIVED_BYTES,
            "wrong derived enum Unit2 observer")
    restored = result
    for old, new in reversed(ENUM_OBSERVER_SEAMS):
        require(restored.count(new) == 1, "enum Unit2 observer reverse seam drift")
        restored = restored.replace(new, old)
    require(restored == borrowed, "enum Unit2 observer reverse identity differs")
    return result


def adapt_enum_index_resource(original):
    """Prescribe only independently derived representation/reservation successors."""
    require(digest(original) == ENUM_INDEX_RESOURCE_ORIGINAL_SHA
            and len(original) == ENUM_INDEX_RESOURCE_ORIGINAL_BYTES, "wrong original enum index resource")
    require(len(ENUM_INDEX_RESOURCE_SEAMS) == 17, "wrong enum index resource substitution count")
    result = original
    for old, new in ENUM_INDEX_RESOURCE_SEAMS:
        require(result.count(old) == 1 and new not in result, "enum index resource seam drift")
        result = result.replace(old, new)
    require(digest(result) == ENUM_INDEX_RESOURCE_DERIVED_SHA
            and len(result) == ENUM_INDEX_RESOURCE_DERIVED_BYTES, "wrong derived enum index resource")
    restored = result
    for old, new in reversed(ENUM_INDEX_RESOURCE_SEAMS):
        require(restored.count(new) == 1, "enum index resource reverse seam drift")
        restored = restored.replace(new, old)
    require(restored == original, "enum index resource reverse identity differs")
    return result


def adapt_enum_unit2_comparator(original):
    """Keep the frozen comparator and add only the four-row named successor."""
    require(digest(original) == UNIT2_COMPARATOR_ORIGINAL_SHA, "wrong original Unit2 comparator")
    require(len(UNIT2_COMPARATOR_SEAMS) == 3, "wrong enum Unit2 comparator substitution count")
    derived = original
    for old, new in UNIT2_COMPARATOR_SEAMS:
        require(derived.count(old) == 1 and new not in derived, "enum Unit2 comparator seam drift")
        derived = derived.replace(old, new)
    require(digest(derived) == UNIT2_COMPARATOR_DERIVED_SHA, "wrong derived enum Unit2 comparator")
    restored = derived
    for old, new in reversed(UNIT2_COMPARATOR_SEAMS):
        require(restored.count(new) == 1, "enum Unit2 comparator reverse seam drift")
        restored = restored.replace(new, old)
    require(restored == original, "enum Unit2 comparator reverse identity differs")
    return derived


def adapt_stdin_unit2_comparator(data, *, reverse=False):
    """Bind the unchanged enum amendment to its exact predecessor manifest."""
    predecessor_sha = UNIT2_COMPARATOR_DERIVED_SHA
    old, new = ((STDIN_COMPARATOR_NEW, STDIN_COMPARATOR_OLD) if reverse
                else (STDIN_COMPARATOR_OLD, STDIN_COMPARATOR_NEW))
    require(digest(data) == (STDIN_COMPARATOR_DERIVED_SHA if reverse else predecessor_sha),
            "wrong stdin Unit2 comparator input")
    require(data.count(old) == 1 and new not in data, "stdin Unit2 comparator seam drift")
    derived = data.replace(old, new)
    require(digest(derived) == (predecessor_sha if reverse else STDIN_COMPARATOR_DERIVED_SHA)
            and (reverse or len(derived) == STDIN_COMPARATOR_DERIVED_BYTES),
            "wrong stdin Unit2 comparator output")
    require(derived.replace(new, old) == data, "stdin Unit2 comparator reverse identity differs")
    return derived


def compile_fixture_paths(source, *, combined=False):
    """Verify the exact published includer and its literal compile-time dependencies."""
    expected_sha = COMBINED_COMPILE_FIXTURE_SOURCE_SHA if combined else COMPILE_FIXTURE_SOURCE_SHA
    expected_bytes = COMBINED_COMPILE_FIXTURE_SOURCE_BYTES if combined else COMPILE_FIXTURE_SOURCE_BYTES
    require(digest(source) == expected_sha and len(source) == expected_bytes,
            "wrong compile-time fixture includer")
    references = [name.decode("ascii") for name in re.findall(COMPILE_FIXTURE_PATTERN, source)]
    require(source.count(b"include_str!") == len(references) == 47
            and digest(encoded(references)) == COMPILE_FIXTURE_REFERENCES_SHA,
            "wrong literal compile-time fixture references")
    paths = sorted(set(references))
    require(len(paths) == 42 and paths == list(COMBINED_FIXTURE_ADDITIONS),
            "wrong compile-time fixture path roster")
    return paths


def preflight(repo, package=PACKAGE):
    require(sys.flags.optimize == 0 and __debug__, "optimized Python is not supported")
    package_manifest = regular(package, "package-manifest.json").read_bytes()
    package_entries = json.loads(package_manifest)["files"]
    require(members(package) == sorted([x["path"] for x in package_entries] + ["package-manifest.json"]),
            "missing or extra adapter member")
    package_bytes = check_entries(package, package_entries)
    require(digest(package_bytes["current-source.json"]) == CURRENT_SOURCE_SHA
            and len(package_bytes["current-source.json"]) == CURRENT_SOURCE_BYTES,
            "unapproved current source manifest")
    require(digest(package_bytes["native-storage-source.json"]) == NATIVE_STORAGE_SOURCE_SHA
            and len(package_bytes["native-storage-source.json"]) == NATIVE_STORAGE_SOURCE_BYTES,
            "unapproved native storage source manifest")
    require(digest(package_bytes["native-inventory-authority.json"]) == NATIVE_INVENTORY_AUTHORITY_SHA
            and len(package_bytes["native-inventory-authority.json"]) == NATIVE_INVENTORY_AUTHORITY_BYTES,
            "stale native inventory authority")
    require(digest(package_bytes["stdout-source.json"]) == STDOUT_SOURCE_SHA
            and len(package_bytes["stdout-source.json"]) == STDOUT_SOURCE_BYTES,
            "unapproved stdout source manifest")
    require(digest(package_bytes["native-storage-authority.json"]) == NATIVE_STORAGE_AUTHORITY_SHA
            and len(package_bytes["native-storage-authority.json"]) == NATIVE_STORAGE_AUTHORITY_BYTES,
            "stale native storage authority")
    require(digest(package_bytes["stdin-source.json"]) == STDIN_SOURCE_SHA
            and len(package_bytes["stdin-source.json"]) == STDIN_SOURCE_BYTES,
            "unapproved stdin source manifest")
    require(digest(package_bytes["stdout-authority.json"]) == STDOUT_AUTHORITY_SHA
            and len(package_bytes["stdout-authority.json"]) == STDOUT_AUTHORITY_BYTES,
            "stale stdout authority")
    require(digest(package_bytes["enum-source.json"]) == ENUM_SOURCE_SHA
            and len(package_bytes["enum-source.json"]) == ENUM_SOURCE_BYTES,
            "unapproved enum source manifest")
    require(digest(package_bytes["stdin-authority.json"]) == STDIN_AUTHORITY_SHA
            and len(package_bytes["stdin-authority.json"]) == STDIN_AUTHORITY_BYTES,
            "stale stdin authority")
    require(digest(package_bytes["combined-source.json"]) == COMBINED_SOURCE_SHA
            and len(package_bytes["combined-source.json"]) == COMBINED_SOURCE_BYTES,
            "unapproved combined source manifest")
    require(digest(package_bytes["division-authority.json"]) == DIVISION_AUTHORITY_SHA
            and len(package_bytes["division-authority.json"]) == DIVISION_AUTHORITY_BYTES,
            "stale division authority")
    require(digest(package_bytes["division-source.json"]) == DIVISION_SOURCE_SHA
            and len(package_bytes["division-source.json"]) == DIVISION_SOURCE_BYTES,
            "unapproved division source manifest")
    require(digest(package_bytes["slices-authority.json"]) == SLICES_AUTHORITY_SHA
            and len(package_bytes["slices-authority.json"]) == SLICES_AUTHORITY_BYTES,
            "stale slices authority")
    slices = json.loads(package_bytes["slices-authority.json"])
    require(slices["schema"] == "oxid-borrowed-slices-source-transition-v1"
            and slices["current_source_sha256"] == SLICES_SOURCE_SHA
            and slices["current_source_bytes"] == SLICES_SOURCE_BYTES
            and slices["division_source_sha256"] == DIVISION_SOURCE_SHA
            and slices["division_source_bytes"] == DIVISION_SOURCE_BYTES
            and slices["division_authority_sha256"] == DIVISION_AUTHORITY_SHA
            and slices["transition_patch_sha256"] == SLICES_PATCH_SHA
            and slices["transition_patch_bytes"] == SLICES_PATCH_BYTES
            and slices["transition_touched_paths"] == list(SLICES_PATHS)
            and slices["added_source_paths"] == list(SLICES_ADDITIONS)
            and slices["removed_source_paths"] == []
            and slices["base_head"] == SLICES_BASE
            and slices["reviewed_source_head"] == SLICES_HEAD
            and slices["source_only_tree"] == SLICES_TREE
            and slices["recipe"] == SOURCE_DELTA_RECIPE
            and (slices["current_source_members"], slices["division_source_members"],
                 slices["compiler_source_members"], slices["compiler_bodies"]) == (188, 185, 136, 139),
            "stale slices transition authority")
    require(digest(package_bytes["slices-transition.patch"]) == SLICES_PATCH_SHA
            and len(package_bytes["slices-transition.patch"]) == SLICES_PATCH_BYTES,
            "wrong slices transition patch")
    division = json.loads(package_bytes["division-authority.json"])
    require(division["schema"] == "oxid-checked-division-source-transition-v1"
            and division["current_source_sha256"] == DIVISION_SOURCE_SHA
            and division["current_source_bytes"] == DIVISION_SOURCE_BYTES
            and division["combined_source_sha256"] == COMBINED_SOURCE_SHA
            and division["combined_source_bytes"] == COMBINED_SOURCE_BYTES
            and division["combined_authority_sha256"] == COMBINED_AUTHORITY_SHA
            and division["transition_patch_sha256"] == DIVISION_PATCH_SHA
            and division["transition_patch_bytes"] == DIVISION_PATCH_BYTES
            and division["transition_touched_paths"] == list(DIVISION_PATHS)
            and division["added_source_paths"] == [] and division["removed_source_paths"] == []
            and division["base_head"] == DIVISION_BASE
            and division["reviewed_source_head"] == DIVISION_HEAD
            and division["source_only_tree"] == DIVISION_TREE
            and division["recipe"] == SOURCE_DELTA_RECIPE
            and (division["current_source_members"], division["combined_source_members"],
                 division["compiler_source_members"], division["compiler_bodies"]) == (185, 185, 133, 136),
            "stale division transition authority")
    require(digest(package_bytes["division-transition.patch"]) == DIVISION_PATCH_SHA
            and len(package_bytes["division-transition.patch"]) == DIVISION_PATCH_BYTES,
            "wrong division transition patch")
    require(digest(package_bytes["formatter-source.json"]) == FORMATTER_SOURCE_SHA
            and len(package_bytes["formatter-source.json"]) == FORMATTER_SOURCE_BYTES,
            "unapproved formatter source manifest")
    require(digest(package_bytes["combined-authority.json"]) == COMBINED_AUTHORITY_SHA
            and len(package_bytes["combined-authority.json"]) == COMBINED_AUTHORITY_BYTES,
            "stale combined authority")
    combined = json.loads(package_bytes["combined-authority.json"])
    require(combined["current_source_sha256"] == COMBINED_SOURCE_SHA
            and combined["current_source_bytes"] == COMBINED_SOURCE_BYTES
            and combined["formatter_source_sha256"] == FORMATTER_SOURCE_SHA
            and combined["formatter_source_bytes"] == FORMATTER_SOURCE_BYTES
            and combined["formatter_authority_sha256"] == FORMATTER_AUTHORITY_SHA
            and combined["transition_patch_sha256"] == COMBINED_PATCH_SHA
            and combined["transition_patch_bytes"] == COMBINED_PATCH_BYTES
            and combined["transition_touched_paths"] == list(COMBINED_PATHS)
            and combined["added_source_paths"] == list(COMBINED_SOURCE_ADDITIONS)
            and combined["added_fixture_paths"] == list(COMBINED_FIXTURE_ADDITIONS)
            and combined["added_input_paths"] == list(COMBINED_ADDITIONS)
            and combined["retained_non_source_paths"] == list(RETAINED_NON_SOURCE_PATHS)
            and combined["base_head"] == COMBINED_BASE
            and combined["reviewed_source_head"] == COMBINED_HEAD
            and combined["source_only_tree"] == COMBINED_TREE
            and combined["recipe"] == SOURCE_DELTA_RECIPE
            and (combined["current_source_members"], combined["formatter_source_members"],
                 combined["compiler_source_members"], combined["compiler_bodies"]) == (185, 133, 133, 136),
            "stale combined transition authority")
    require(combined["compile_time_fixture_derivation"] == {
        "source": {"path": COMPILE_FIXTURE_SOURCE, "bytes": COMBINED_COMPILE_FIXTURE_SOURCE_BYTES,
                   "sha256": COMBINED_COMPILE_FIXTURE_SOURCE_SHA},
        "include_str_references": 47, "unique_fixture_inputs": 42,
        "literal_pattern": COMPILE_FIXTURE_PATTERN.decode("ascii"),
        "ordered_references_sha256": COMPILE_FIXTURE_REFERENCES_SHA,
    }, "stale compile-time fixture authority")
    require(digest(package_bytes["combined-transition.patch"]) == COMBINED_PATCH_SHA
            and len(package_bytes["combined-transition.patch"]) == COMBINED_PATCH_BYTES,
            "wrong combined transition patch")
    require(digest(package_bytes["predecessor-source.json"]) == PREDECESSOR_SOURCE_SHA,
            "unapproved predecessor source manifest")
    require(digest(package_bytes["formatter-authority.json"]) == FORMATTER_AUTHORITY_SHA,
            "stale formatter authority")
    formatter = json.loads(package_bytes["formatter-authority.json"])
    require(formatter["current_source_sha256"] == FORMATTER_SOURCE_SHA
            and formatter["predecessor_source_sha256"] == PREDECESSOR_SOURCE_SHA
            and formatter["transition_patch_sha256"] == FORMATTER_PATCH_SHA
            and formatter["transition_patch_bytes"] == FORMATTER_PATCH_BYTES
            and formatter["transition_touched_paths"] == list(FORMATTER_PATHS)
            and formatter["added_source_paths"] == list(FORMATTER_ADDITIONS), "stale formatter transition authority")
    authority = json.loads(package_bytes["authority.json"])
    require(authority["current_source_sha256"] == PREDECESSOR_SOURCE_SHA, "stale predecessor manifest authority")
    patch = package_bytes["source-transition.patch"]
    require(authority["transition_patch_sha256"] == PATCH_SHA
            and authority["transition_patch_bytes"] == PATCH_BYTES
            and authority["transition_touched_paths"] == list(PATCH_PATHS), "stale transition authority")
    require(authority["transition_patch_prefix"] == {"bytes": PATCH_PREFIX_BYTES, "sha256": PATCH_PREFIX_SHA}
            and digest(patch[:PATCH_PREFIX_BYTES]) == PATCH_PREFIX_SHA, "changed activation patch prefix")
    delta = authority["transition_source_delta"]
    require(delta["bytes"] == len(patch) - PATCH_PREFIX_BYTES
            and delta["sha256"] == digest(patch[PATCH_PREFIX_BYTES:])
            and delta["paths"] == list(PATCH_PATHS[9:])
            and delta["base_head"] == SOURCE_DELTA_BASE
            and delta["recipe"] == SOURCE_DELTA_RECIPE, "stale source delta authority")
    references = check_entries(repo, authority["repository_inputs"])
    historical = json.loads(references[U2 + "/package-inputs.json"])
    historical_bytes = check_entries(repo / U2, historical["files"])
    require(members(repo / U2) == sorted([x["path"] for x in historical["files"]] + ["package-inputs.json"]),
            "missing or extra historical Unit2 member")
    current = json.loads(package_bytes["slices-source.json"])
    division_source = json.loads(package_bytes["division-source.json"])
    combined_source = json.loads(package_bytes["combined-source.json"])
    formatter_source = json.loads(package_bytes["formatter-source.json"])
    predecessor = json.loads(package_bytes["predecessor-source.json"])
    selected = json.loads(references[U3 + "/manifests/selected-current.json"])
    require(len(current["files"]) == 188 and len(division_source["files"]) == 185
            and len(combined_source["files"]) == 185
            and len(formatter_source["files"]) == 133
            and len(predecessor["files"]) == 129 and len(selected["files"]) == 117,
            "wrong source count")
    require(delta["reviewed_source_head"] == predecessor["reviewed_source_head"]
            and delta["source_only_tree"] == predecessor["source_only_tree"], "stale source checkpoint provenance")
    require(formatter_source["reviewed_source_head"] == formatter["reviewed_source_head"]
            and formatter_source["source_only_tree"] == formatter["source_only_tree"]
            and formatter_source["formatter_base_head"] == formatter["base_head"]
            and formatter_source["predecessor_source_sha256"] == PREDECESSOR_SOURCE_SHA,
            "stale formatter checkpoint provenance")
    require(combined_source["reviewed_source_head"] == COMBINED_HEAD
            and combined_source["source_only_tree"] == COMBINED_TREE
            and combined_source["combined_base_head"] == COMBINED_BASE
            and combined_source["formatter_source_sha256"] == FORMATTER_SOURCE_SHA
            and combined_source["compile_time_fixture_source"] == COMPILE_FIXTURE_SOURCE
            and combined_source["compile_time_fixture_references"] == 47
            and combined_source["compile_time_fixture_members"] == 42,
            "stale combined checkpoint provenance")
    require(division_source["reviewed_source_head"] == DIVISION_HEAD
            and division_source["source_only_tree"] == DIVISION_TREE
            and division_source["division_base_head"] == DIVISION_BASE
            and division_source["combined_source_sha256"] == COMBINED_SOURCE_SHA
            and division_source["combined_base_head"] == COMBINED_BASE
            and division_source["formatter_source_sha256"] == FORMATTER_SOURCE_SHA
            and division_source["compile_time_fixture_source"] == COMPILE_FIXTURE_SOURCE
            and division_source["compile_time_fixture_references"] == 47
            and division_source["compile_time_fixture_members"] == 42,
            "stale division checkpoint provenance")
    require(current["reviewed_source_head"] == SLICES_HEAD
            and current["source_only_tree"] == SLICES_TREE
            and current["slices_base_head"] == SLICES_BASE
            and current["division_source_sha256"] == DIVISION_SOURCE_SHA
            and {key: value for key, value in current.items()
                 if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree",
                                "slices_base_head", "division_source_sha256")}
                == {key: value for key, value in division_source.items()
                    if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree")},
            "stale slices checkpoint provenance")
    require({x["path"] for x in predecessor["files"]} == {x["path"] for x in selected["files"]} | EXTRA,
            "unexpected predecessor source membership")
    require({x["path"] for x in formatter_source["files"]}
            == {x["path"] for x in predecessor["files"]} | set(FORMATTER_ADDITIONS),
            "unexpected formatter source membership")
    require({x["path"] for x in combined_source["files"]}
            == {x["path"] for x in formatter_source["files"]} | set(COMBINED_ADDITIONS),
            "unexpected combined source membership")
    require([x["path"] for x in division_source["files"]] == [x["path"] for x in combined_source["files"]],
            "unexpected division source membership")
    require({x["path"] for x in current["files"]}
            == {x["path"] for x in division_source["files"]} | set(SLICES_ADDITIONS),
            "unexpected current source membership")
    current_rows = {x["path"]: x for x in current["files"]}
    division_rows = {x["path"]: x for x in division_source["files"]}
    combined_rows = {x["path"]: x for x in combined_source["files"]}
    require([name for name in division_rows if division_rows[name] != combined_rows[name]] == list(DIVISION_PATHS),
            "unexpected division source delta")
    require([name for name in current_rows if current_rows[name] != division_rows.get(name)] == list(SLICES_PATHS),
            "unexpected slices source delta")
    require(division["current_input_git_modes"] == [{"path": name, "mode": "100644"} for name in division_rows],
            "unexpected division source modes")
    require(slices["current_input_git_modes"] == [{"path": name, "mode": "100644"} for name in current_rows],
            "unexpected current source modes")
    retained = [x for x in current["files"] if not x["path"].startswith(("src/", "native/"))
                and x["path"] not in COMBINED_FIXTURE_ADDITIONS]
    require([x["path"] for x in retained] == list(RETAINED_NON_SOURCE_PATHS)
            and retained == [x for x in formatter_source["files"]
                             if not x["path"].startswith(("src/", "native/"))],
            "changed retained non-source inputs")
    hir_current, hir_inputs, hir_authority, native_inventory_inputs, hir_touched = admit_hir_import(repo, package_bytes)
    native_inventory_current = json.loads(package_bytes["native-inventory-source.json"])
    native_storage_current = json.loads(package_bytes["native-storage-source.json"])
    native_inventory = json.loads(package_bytes["native-inventory-authority.json"])
    require(native_inventory["schema"] == "oxid-native-inventory-source-transition-v1"
            and native_inventory["base_head"] == native_inventory_current["native_inventory_base_head"] == NATIVE_INVENTORY_BASE
            and native_inventory["base_tree"] == NATIVE_INVENTORY_BASE_TREE
            and native_inventory["reviewed_source_head"] == native_inventory_current["reviewed_source_head"] == NATIVE_INVENTORY_HEAD
            and native_inventory["source_only_tree"] == native_inventory_current["source_only_tree"] == NATIVE_INVENTORY_TREE
            and native_inventory["recipe"] == SOURCE_DELTA_RECIPE
            and native_inventory["current_source_sha256"] == NATIVE_INVENTORY_SOURCE_SHA
            and native_inventory["current_source_bytes"] == NATIVE_INVENTORY_SOURCE_BYTES
            and native_inventory["native_storage_source_sha256"] == native_inventory_current["native_storage_source_sha256"] == NATIVE_STORAGE_SOURCE_SHA
            and native_inventory["native_storage_source_bytes"] == NATIVE_STORAGE_SOURCE_BYTES
            and native_inventory["native_storage_authority_sha256"] == NATIVE_STORAGE_AUTHORITY_SHA
            and native_inventory["transition_patch_sha256"] == NATIVE_INVENTORY_PATCH_SHA
            and native_inventory["transition_patch_bytes"] == NATIVE_INVENTORY_PATCH_BYTES
            and native_inventory["transition_paths"] == list(NATIVE_INVENTORY_PATHS)
            and native_inventory["additions"] == list(NATIVE_INVENTORY_ADDITIONS)
            and native_inventory["removed_paths"] == []
            and (native_inventory["current_source_members"], native_inventory["native_storage_source_members"],
                 native_inventory["compiler_source_members"], native_inventory["compiler_bodies"]) == (266, 264, 208, 211),
            "stale native inventory transition authority")
    require({key: value for key, value in native_inventory_current.items()
             if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree",
                            "native_inventory_base_head", "native_storage_source_sha256")}
            == {key: value for key, value in native_storage_current.items()
                if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree")},
            "stale native inventory checkpoint provenance")
    check_bytes(native_inventory_inputs, native_inventory_current["files"])
    native_storage_rows = {row["path"]: row for row in native_storage_current["files"]}
    require(set(native_inventory_inputs) == set(native_storage_rows) | set(NATIVE_INVENTORY_ADDITIONS),
            "unexpected native inventory source membership")
    require([row["path"] for row in native_inventory_current["files"] if row != native_storage_rows.get(row["path"])]
            == list(NATIVE_INVENTORY_PATHS), "unexpected native inventory source delta")
    require(native_inventory["current_input_git_modes"] == [
        {"path": row["path"], "mode": "100644"} for row in native_inventory_current["files"]],
        "unexpected native inventory input modes")
    native_inventory_identities = []
    for name, data in native_inventory_inputs.items():
        require(regular(repo, name).stat().st_mode & 0o111 == 0, "changed input mode: " + name)
        native_inventory_identities.append({**entry(name, data), "mode": "100644",
            "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()})
    require(native_inventory["current_input_identities"] == native_inventory_identities,
            "stale native inventory complete input identities")
    actual = [part + "/" + name for part in ("src", "native") for name in members(repo / part)]
    expected = [name for name in native_inventory_inputs if name.startswith(("src/", "native/"))]
    require(len(expected) == 208 and sorted(actual) == sorted(n for n in hir_inputs if n.startswith(("src/", "native/"))),
            "missing or extra compiler source member")
    require(len([name for name in native_inventory_inputs if name.startswith(("src/", "native/"))
                 or name in ("Cargo.toml", "Cargo.lock", "build.rs")]) == 211,
            "unexpected native inventory compiler/build closure")
    native_storage_inputs, native_inventory_touched = inverse_native_inventory_patch(
        native_inventory_inputs, package_bytes["native-inventory-transition.patch"])
    check_bytes(native_storage_inputs, native_storage_current["files"])
    native_inventory_transition = []
    for name in NATIVE_INVENTORY_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", native_storage_inputs), ("after", native_inventory_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        native_inventory_transition.append(identities)
    require(native_inventory["transition_inputs"] == native_inventory_transition,
            "stale native inventory transition input identities")
    stdout_current = json.loads(package_bytes["stdout-source.json"])
    native_storage = json.loads(package_bytes["native-storage-authority.json"])
    require(native_storage["schema"] == "oxid-native-storage-source-transition-v1"
            and native_storage["base_head"] == native_storage_current["native_storage_base_head"] == NATIVE_STORAGE_BASE == STDOUT_HEAD
            and native_storage["reviewed_source_head"] == native_storage_current["reviewed_source_head"] == NATIVE_STORAGE_HEAD
            and native_storage["source_only_tree"] == native_storage_current["source_only_tree"] == NATIVE_STORAGE_TREE
            and native_storage["recipe"] == SOURCE_DELTA_RECIPE
            and native_storage["current_source_sha256"] == NATIVE_STORAGE_SOURCE_SHA
            and native_storage["current_source_bytes"] == NATIVE_STORAGE_SOURCE_BYTES
            and native_storage["stdout_source_sha256"] == native_storage_current["stdout_source_sha256"] == STDOUT_SOURCE_SHA
            and native_storage["stdout_source_bytes"] == STDOUT_SOURCE_BYTES
            and native_storage["stdout_authority_sha256"] == STDOUT_AUTHORITY_SHA
            and native_storage["transition_patch_sha256"] == NATIVE_STORAGE_PATCH_SHA
            and native_storage["transition_patch_bytes"] == NATIVE_STORAGE_PATCH_BYTES
            and native_storage["transition_paths"] == list(NATIVE_STORAGE_PATHS)
            and native_storage["additions"] == list(NATIVE_STORAGE_ADDITIONS)
            and native_storage["removed_paths"] == []
            and (native_storage["current_source_members"], native_storage["stdout_source_members"],
                 native_storage["compiler_source_members"], native_storage["compiler_bodies"]) == (264, 262, 206, 209),
            "stale native storage transition authority")
    require({key: value for key, value in native_storage_current.items()
             if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree",
                            "native_storage_base_head", "stdout_source_sha256")}
            == {key: value for key, value in stdout_current.items()
                if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree")},
            "stale native storage checkpoint provenance")
    stdout_rows = {row["path"]: row for row in stdout_current["files"]}
    require(set(native_storage_inputs) == set(stdout_rows) | set(NATIVE_STORAGE_ADDITIONS),
            "unexpected native storage source membership")
    require([row["path"] for row in native_storage_current["files"] if row != stdout_rows.get(row["path"])]
            == list(NATIVE_STORAGE_PATHS), "unexpected native storage source delta")
    require(native_storage["current_input_git_modes"] == [
        {"path": row["path"], "mode": "100644"} for row in native_storage_current["files"]],
        "unexpected native storage input modes")
    native_storage_identities = []
    for name, data in native_storage_inputs.items():
        require(regular(repo, name).stat().st_mode & 0o111 == 0, "changed input mode: " + name)
        native_storage_identities.append({**entry(name, data), "mode": "100644",
            "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()})
    require(native_storage["current_input_identities"] == native_storage_identities,
            "stale native storage complete input identities")
    require(len([name for name in native_storage_inputs if name.startswith(("src/", "native/"))]) == 206,
            "unexpected native storage compiler source membership")
    require(len([name for name in native_storage_inputs if name.startswith(("src/", "native/"))
                 or name in ("Cargo.toml", "Cargo.lock", "build.rs")]) == 209,
            "unexpected native storage compiler/build closure")
    stdout_inputs, native_storage_touched = inverse_native_storage_patch(
        native_storage_inputs, package_bytes["native-storage-transition.patch"])
    check_bytes(stdout_inputs, stdout_current["files"])
    native_storage_transition = []
    for name in NATIVE_STORAGE_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", stdout_inputs), ("after", native_storage_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        native_storage_transition.append(identities)
    require(native_storage["transition_inputs"] == native_storage_transition,
            "stale native storage transition input identities")
    stdin_current = json.loads(package_bytes["stdin-source.json"])
    stdout_authority = json.loads(package_bytes["stdout-authority.json"])
    require(stdout_authority["schema"] == "oxid-bounded-stdout-source-transition-v1"
            and stdout_authority["base_head"] == stdout_current["stdout_base_head"] == STDOUT_BASE == STDIN_HEAD
            and stdout_authority["reviewed_source_head"] == stdout_current["reviewed_source_head"] == STDOUT_HEAD
            and stdout_authority["source_only_tree"] == stdout_current["source_only_tree"] == STDOUT_TREE
            and stdout_authority["recipe"] == SOURCE_DELTA_RECIPE
            and stdout_authority["current_source_sha256"] == STDOUT_SOURCE_SHA
            and stdout_authority["current_source_bytes"] == STDOUT_SOURCE_BYTES
            and stdout_authority["stdin_source_sha256"] == stdout_current["stdin_source_sha256"] == STDIN_SOURCE_SHA
            and stdout_authority["stdin_source_bytes"] == STDIN_SOURCE_BYTES
            and stdout_authority["stdin_authority_sha256"] == STDIN_AUTHORITY_SHA
            and stdout_authority["transition_patch_sha256"] == STDOUT_PATCH_SHA
            and stdout_authority["transition_patch_bytes"] == STDOUT_PATCH_BYTES
            and stdout_authority["transition_paths"] == list(STDOUT_PATHS)
            and stdout_authority["additions"] == list(STDOUT_ADDITIONS)
            and stdout_authority["removed_paths"] == []
            and (stdout_authority["current_source_members"], stdout_authority["stdin_source_members"],
                 stdout_authority["compiler_source_members"], stdout_authority["compiler_bodies"]) == (262, 252, 204, 207),
            "stale stdout transition authority")
    require({key: value for key, value in stdout_current.items()
             if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree",
                            "stdout_base_head", "stdin_source_sha256")}
            == {key: value for key, value in stdin_current.items()
                if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree")},
            "stale stdout checkpoint provenance")
    stdin_rows = {row["path"]: row for row in stdin_current["files"]}
    require(set(stdout_inputs) == set(stdin_rows) | set(STDOUT_ADDITIONS),
            "unexpected stdout source membership")
    require([row["path"] for row in stdout_current["files"] if row != stdin_rows.get(row["path"])]
            == list(STDOUT_PATHS), "unexpected stdout source delta")
    require(stdout_authority["current_input_git_modes"] == [
        {"path": row["path"], "mode": "100644"} for row in stdout_current["files"]],
        "unexpected stdout input modes")
    stdout_identities = []
    for name, data in stdout_inputs.items():
        require(regular(repo, name).stat().st_mode & 0o111 == 0, "changed input mode: " + name)
        stdout_identities.append({**entry(name, data), "mode": "100644",
            "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()})
    require(stdout_authority["current_input_identities"] == stdout_identities,
            "stale stdout complete input identities")
    require(len([name for name in stdout_inputs if name.startswith(("src/", "native/"))]) == 204,
            "unexpected restored stdout compiler source membership")
    require(len([name for name in stdout_inputs if name.startswith(("src/", "native/"))
                 or name in ("Cargo.toml", "Cargo.lock", "build.rs")]) == 207,
            "unexpected stdout compiler/build closure")
    stdin_inputs, stdout_touched = inverse_stdout_patch(stdout_inputs, package_bytes["stdout-transition.patch"])
    check_bytes(stdin_inputs, stdin_current["files"])
    stdout_transition = []
    for name in STDOUT_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", stdin_inputs), ("after", stdout_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        stdout_transition.append(identities)
    require(stdout_authority["transition_inputs"] == stdout_transition,
            "stale stdout transition input identities")
    enum_current = json.loads(package_bytes["enum-source.json"])
    stdin_authority = json.loads(package_bytes["stdin-authority.json"])
    require(stdin_authority["schema"] == "oxid-bounded-stdin-source-transition-v1"
            and stdin_authority["base_head"] == stdin_current["stdin_base_head"] == STDIN_BASE == ENUM_HEAD
            and stdin_authority["reviewed_source_head"] == stdin_current["reviewed_source_head"] == STDIN_HEAD
            and stdin_authority["source_only_tree"] == stdin_current["source_only_tree"] == STDIN_TREE
            and stdin_authority["recipe"] == SOURCE_DELTA_RECIPE
            and stdin_authority["current_source_sha256"] == STDIN_SOURCE_SHA
            and stdin_authority["current_source_bytes"] == STDIN_SOURCE_BYTES
            and stdin_authority["enum_source_sha256"] == stdin_current["enum_source_sha256"] == ENUM_SOURCE_SHA
            and stdin_authority["enum_source_bytes"] == ENUM_SOURCE_BYTES
            and stdin_authority["enum_authority_sha256"] == ENUM_AUTHORITY_SHA
            and stdin_authority["transition_patch_sha256"] == STDIN_PATCH_SHA
            and stdin_authority["transition_patch_bytes"] == STDIN_PATCH_BYTES
            and stdin_authority["transition_paths"] == list(STDIN_PATHS)
            and stdin_authority["additions"] == list(STDIN_ADDITIONS)
            and stdin_authority["removed_paths"] == []
            and (stdin_authority["current_source_members"], stdin_authority["enum_source_members"],
                 stdin_authority["compiler_source_members"], stdin_authority["compiler_bodies"]) == (252, 237, 194, 197),
            "stale stdin transition authority")
    require({key: value for key, value in stdin_current.items()
             if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree",
                            "stdin_base_head", "enum_source_sha256")}
            == {key: value for key, value in enum_current.items()
                if key not in ("files", "purpose", "reviewed_source_head", "source_only_tree")},
            "stale stdin checkpoint provenance")
    enum_rows = {row["path"]: row for row in enum_current["files"]}
    require(set(stdin_inputs) == set(enum_rows) | set(STDIN_ADDITIONS),
            "unexpected stdin source membership")
    require([row["path"] for row in stdin_current["files"] if row != enum_rows.get(row["path"])]
            == list(STDIN_PATHS), "unexpected stdin source delta")
    require(stdin_authority["current_input_git_modes"] == [
        {"path": row["path"], "mode": "100644"} for row in stdin_current["files"]],
        "unexpected stdin input modes")
    stdin_identities = []
    for name, data in stdin_inputs.items():
        require(regular(repo, name).stat().st_mode & 0o111 == 0, "changed input mode: " + name)
        stdin_identities.append({**entry(name, data), "mode": "100644",
            "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()})
    require(stdin_authority["current_input_identities"] == stdin_identities,
            "stale stdin complete input identities")
    enum_inputs, stdin_touched = inverse_stdin_patch(stdin_inputs, package_bytes["stdin-transition.patch"])
    check_bytes(enum_inputs, enum_current["files"])
    stdin_transition = []
    for name in STDIN_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", enum_inputs), ("after", stdin_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        stdin_transition.append(identities)
    require(stdin_authority["transition_inputs"] == stdin_transition,
            "stale stdin transition input identities")
    require(digest(package_bytes["projected-source.json"]) == PROJECTED_SOURCE_SHA
            and len(package_bytes["projected-source.json"]) == PROJECTED_SOURCE_BYTES,
            "unapproved projected source manifest")
    projected_current = json.loads(package_bytes["projected-source.json"])
    require(digest(package_bytes["enum-authority.json"]) == ENUM_AUTHORITY_SHA
            and len(package_bytes["enum-authority.json"]) == ENUM_AUTHORITY_BYTES,
            "stale enum authority")
    enumeration = json.loads(package_bytes["enum-authority.json"])
    require(enumeration["schema"] == "oxid-bounded-enum-source-transition-v1"
            and enumeration["base_head"] == enum_current["enum_base_head"] == ENUM_BASE
            and enumeration["reviewed_source_head"] == enum_current["reviewed_source_head"] == ENUM_HEAD
            and enumeration["source_only_tree"] == enum_current["source_only_tree"] == ENUM_TREE
            and enumeration["recipe"] == SOURCE_DELTA_RECIPE
            and enumeration["current_source_sha256"] == ENUM_SOURCE_SHA
            and enumeration["current_source_bytes"] == ENUM_SOURCE_BYTES
            and enumeration["projected_source_sha256"] == enum_current["projected_source_sha256"] == PROJECTED_SOURCE_SHA
            and enumeration["projected_source_bytes"] == PROJECTED_SOURCE_BYTES
            and enumeration["projected_authority_sha256"] == PROJECTED_AUTHORITY_SHA
            and enumeration["transition_patch_sha256"] == ENUM_PATCH_SHA
            and enumeration["transition_patch_bytes"] == ENUM_PATCH_BYTES
            and enumeration["transition_paths"] == list(ENUM_PATHS)
            and enumeration["additions"] == list(ENUM_ADDITIONS)
            and enumeration["removed_paths"] == []
            and (enumeration["current_source_members"], enumeration["projected_source_members"],
                 enumeration["compiler_source_members"], enumeration["compiler_bodies"]) == (237, 201, 179, 182),
            "stale enum transition authority")
    projected_rows = {r["path"]: r for r in projected_current["files"]}
    require(set(enum_inputs) == set(projected_rows) | set(ENUM_ADDITIONS),
            "unexpected enum source membership")
    require([r["path"] for r in enum_current["files"] if r != projected_rows.get(r["path"])]
            == list(ENUM_PATHS), "unexpected enum source delta")
    require(enumeration["current_input_git_modes"] == [
        {"path": r["path"], "mode": "100644"} for r in enum_current["files"]],
        "unexpected enum input modes")
    current_identities = []
    for name, data in enum_inputs.items():
        require(regular(repo, name).stat().st_mode & 0o111 == 0, "changed input mode: " + name)
        current_identities.append({**entry(name, data), "mode": "100644",
            "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()})
    require(enumeration["current_input_identities"] == current_identities, "stale enum complete input identities")
    projected_inputs, enum_touched = inverse_enum_patch(enum_inputs, package_bytes["enum-transition.patch"])
    check_bytes(projected_inputs, projected_current["files"])
    transition = []
    for name in ENUM_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", projected_inputs), ("after", enum_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        transition.append(identities)
    require(enumeration["transition_inputs"] == transition, "stale enum transition input identities")
    scanner_closure = []
    patterns = (
        rb'include_str!\("\.\./\.\./(tests/fixtures/bounded_enum_scanner/(?:main|scanner)\.ox)"\)',
        rb'include_str!\(concat!\(\s*env!\("CARGO_MANIFEST_DIR"\),\s*"/(tests/fixtures/bounded_enum_scanner/(?:main|scanner)\.ox)"\s*\)\)',
    )
    for name, pattern in zip(ENUM_SCANNER_INCLUDERS, patterns):
        scanner_references = [x.decode("ascii") for x in re.findall(pattern, enum_inputs[name])]
        require(scanner_references == list(ENUM_SCANNER_PATHS), "changed enum scanner include references")
        scanner_closure.append({"includer": entry(name, enum_inputs[name]), "ordered_references": scanner_references})
    require(enumeration["scanner_include_closure"] == scanner_closure
            and enum_current["enum_scanner_fixture_members"] == 2
            and enum_current["enum_scanner_fixture_references"] == 4,
            "changed enum scanner include closure")
    require(digest(package_bytes["unary-source.json"]) == UNARY_SOURCE_SHA
            and len(package_bytes["unary-source.json"]) == UNARY_SOURCE_BYTES,
            "unapproved unary source manifest")
    unary_current = json.loads(package_bytes["unary-source.json"])
    require(digest(package_bytes["projected-authority.json"]) == PROJECTED_AUTHORITY_SHA
            and len(package_bytes["projected-authority.json"]) == PROJECTED_AUTHORITY_BYTES,
            "stale projected authority")
    projected = json.loads(package_bytes["projected-authority.json"])
    unary_rows = {r["path"]: r for r in unary_current["files"]}
    require(set(projected_inputs) == set(unary_rows) | set(PROJECTED_ADDITIONS),
            "unexpected projected source membership")
    require([r["path"] for r in projected_current["files"] if r != unary_rows.get(r["path"])]
            == list(PROJECTED_PATHS), "unexpected projected source delta")
    require(projected["current_input_git_modes"] == [
        {"path": r["path"], "mode": "100644"} for r in projected_current["files"]],
        "unexpected projected input modes")
    for item in projected["current_input_git_modes"]:
        require(regular(repo, item["path"]).stat().st_mode & 0o111 == 0,
                "changed input mode: " + item["path"])
    unary_inputs, projected_touched = inverse_projected_patch(
        projected_inputs, package_bytes["projected-transition.patch"])
    check_bytes(unary_inputs, unary_current["files"])
    transition = []
    for name in PROJECTED_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", unary_inputs), ("after", projected_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        transition.append(identities)
    require(projected["transition_inputs"] == transition, "stale projected input identities")
    require(digest(package_bytes["composition-source.json"]) == COMPOSITION_SOURCE_SHA
            and len(package_bytes["composition-source.json"]) == COMPOSITION_SOURCE_BYTES,
            "unapproved composition source manifest")
    composition_current = json.loads(package_bytes["composition-source.json"])
    require(digest(package_bytes["unary-authority.json"]) == UNARY_AUTHORITY_SHA
            and len(package_bytes["unary-authority.json"]) == UNARY_AUTHORITY_BYTES,
            "stale unary authority")
    unary = json.loads(package_bytes["unary-authority.json"])
    composition_rows = {r["path"]: r for r in composition_current["files"]}
    require(set(unary_inputs) == set(composition_rows) | set(UNARY_ADDITIONS),
            "unexpected unary source membership")
    require([r["path"] for r in unary_current["files"] if r != composition_rows.get(r["path"])]
            == list(UNARY_PATHS), "unexpected unary source delta")
    require(unary["current_input_git_modes"] == [
        {"path": r["path"], "mode": "100644"} for r in unary_current["files"]],
        "unexpected unary input modes")
    for item in unary["current_input_git_modes"]:
        require(regular(repo, item["path"]).stat().st_mode & 0o111 == 0,
                "changed input mode: " + item["path"])
    composition_inputs, unary_touched = inverse_unary_patch(
        unary_inputs, package_bytes["unary-transition.patch"])
    check_bytes(composition_inputs, composition_current["files"])
    transition = []
    for name in UNARY_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", composition_inputs), ("after", unary_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        transition.append(identities)
    require(unary["transition_inputs"] == transition, "stale unary input identities")
    require(digest(package_bytes["slices-source.json"]) == SLICES_SOURCE_SHA
            and len(package_bytes["slices-source.json"]) == SLICES_SOURCE_BYTES,
            "unapproved slices source manifest")
    require(digest(package_bytes["composition-authority.json"]) == COMPOSITION_AUTHORITY_SHA
            and len(package_bytes["composition-authority.json"]) == COMPOSITION_AUTHORITY_BYTES,
            "stale composition authority")
    composition = json.loads(package_bytes["composition-authority.json"])
    require(set(composition_inputs) == set(current_rows) | set(COMPOSITION_ADDITIONS),
            "unexpected composition source membership")
    require([r["path"] for r in composition_current["files"] if r != current_rows.get(r["path"])]
            == list(COMPOSITION_PATHS), "unexpected composition source delta")
    require(composition["current_input_git_modes"] == [
        {"path": r["path"], "mode": "100644"} for r in composition_current["files"]],
        "unexpected composition input modes")
    inputs, composition_touched = inverse_composition_patch(
        composition_inputs, package_bytes["composition-transition.patch"])
    check_bytes(inputs, current["files"])
    transition = []
    for name in COMPOSITION_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", inputs), ("after", composition_inputs)):
            data = source_inputs.get(name)
            identities[label] = None if data is None else {
                **entry(name, data), "mode": "100644",
                "git_blob": hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()}
        transition.append(identities)
    require(composition["transition_inputs"] == transition, "stale composition input identities")
    closure = composition["public_sample_closure"]
    includer = composition_inputs[closure["includer"]["path"]]
    sample_references = [x.decode("ascii") for x in re.findall(
        rb'include_str!\(\s*"\.\./(fixtures/typed-record-composition-samples/[a-z]+\.ox)"\s*\)', includer)]
    require(entry(closure["includer"]["path"], includer) == closure["includer"]
            and sample_references == closure["ordered_references"] and len(sample_references) == 3
            and len(set(sample_references)) == 3
            and set(sample_references) == {n for n in COMPOSITION_ADDITIONS if n.startswith("fixtures/")},
            "changed public composition sample closure")
    for item in composition["current_input_git_modes"]:
        require(regular(repo, item["path"]).stat().st_mode & 0o111 == 0,
                "changed input mode: " + item["path"])
    fixture_paths = compile_fixture_paths(inputs[COMPILE_FIXTURE_SOURCE])
    require([x["path"] for x in current["files"] if x["path"] in COMBINED_FIXTURE_ADDITIONS]
            == fixture_paths, "missing or extra compile-time fixture input")
    actual = [part + "/" + name for part in ("src", "native") for name in members(repo / part)]
    expected = [x for x in native_inventory_inputs if x.startswith(("src/", "native/"))]
    require(sorted(actual) == sorted(n for n in hir_inputs if n.startswith(("src/", "native/"))), "missing or extra compiler source member")
    require(slices["compile_time_fixture_derivation"] == {
        **combined["compile_time_fixture_derivation"],
        "source": entry(COMPILE_FIXTURE_SOURCE, inputs[COMPILE_FIXTURE_SOURCE]),
    }, "stale slices compile-time fixture authority")
    division_inputs, slices_touched = inverse_slices_patch(inputs, package_bytes["slices-transition.patch"])
    check_bytes(division_inputs, division_source["files"])
    slices_transition_inputs = []
    for name in SLICES_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", division_inputs), ("after", inputs)):
            if name not in source_inputs:
                identities[label] = None
                continue
            data = source_inputs[name]
            blob = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
            identities[label] = {**entry(name, data), "mode": "100644", "git_blob": blob}
        slices_transition_inputs.append(identities)
    require(slices["transition_inputs"] == slices_transition_inputs, "stale slices input identities")
    require(compile_fixture_paths(division_inputs[COMPILE_FIXTURE_SOURCE], combined=True) == fixture_paths,
            "changed predecessor compile-time fixture roster")
    combined_inputs, division_touched = inverse_division_patch(division_inputs, package_bytes["division-transition.patch"])
    check_bytes(combined_inputs, combined_source["files"])
    transition_inputs = []
    for name in DIVISION_PATHS:
        identities = {"path": name}
        for label, source_inputs in (("before", combined_inputs), ("after", division_inputs)):
            data = source_inputs[name]
            blob = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") + b"\0" + data).hexdigest()
            identities[label] = {**entry(name, data), "mode": "100644", "git_blob": blob}
        transition_inputs.append(identities)
    require(division["transition_inputs"] == transition_inputs, "stale division input identities")
    formatter_inputs, combined_touched = inverse_combined_patch(combined_inputs, package_bytes["combined-transition.patch"])
    check_bytes(formatter_inputs, formatter_source["files"])
    predecessor_inputs, formatter_touched = inverse_formatter_patch(formatter_inputs, package_bytes["formatter-transition.patch"])
    check_bytes(predecessor_inputs, predecessor["files"])
    reconstructed, touched = inverse_patch(predecessor_inputs, package_bytes["source-transition.patch"])
    archived_extra = authority["inverse_only_inputs"]
    for item in archived_extra:
        require(entry(item["path"], reconstructed.pop(item["path"])) == item,
                "inverse integration-test identity differs")
    check_bytes(reconstructed, selected["files"])
    resource = historical_bytes[RESOURCE]
    require(resource.count(OLD_SEAM) == 1 and PREDECESSOR_SEAM not in resource
            and NEW_SEAM not in resource, "resource seam drift")
    predecessor_resource = resource.replace(OLD_SEAM, PREDECESSOR_SEAM)
    require(entry(RESOURCE, predecessor_resource) == authority["derived_resource"],
            "derived predecessor resource drift")
    require(predecessor_resource.count(PREDECESSOR_SEAM) == 1 and NEW_SEAM not in predecessor_resource,
            "combined resource seam drift")
    adapted_resource = predecessor_resource.replace(PREDECESSOR_SEAM, NEW_SEAM)
    require(entry(RESOURCE, resource) == combined["original_resource"]
            and entry(RESOURCE, predecessor_resource) == combined["predecessor_derived_resource"]
            and entry(RESOURCE, adapted_resource) == combined["derived_resource"]
            and combined["resource_substitution"] == {
                "old_sha256": digest(PREDECESSOR_SEAM), "new_sha256": digest(NEW_SEAM), "count": 1},
            "derived combined resource drift")
    require(adapted_resource.replace(NEW_SEAM, OLD_SEAM) == resource,
            "resource reverse identity differs")
    require(adapted_resource.count(NEW_SEAM) == 1 and ENUM_RESOURCE_SEAM not in adapted_resource,
            "enum resource seam drift")
    enum_resource = adapted_resource.replace(NEW_SEAM, ENUM_RESOURCE_SEAM)
    require(enumeration["resource_adapter"] == {
        "version": "unit2-closed-enum-parser-resource-v1",
        "predecessor": entry(RESOURCE, adapted_resource), "derived": entry(RESOURCE, enum_resource),
        "substitution": {"old_sha256": digest(NEW_SEAM), "new_sha256": digest(ENUM_RESOURCE_SEAM), "count": 1},
        "scope": "Add only closed enum syntax policy and empty syntax storage to the direct Parser initializer; existing resource controls and expectations remain unchanged.",
    } and enum_resource.replace(ENUM_RESOURCE_SEAM, NEW_SEAM) == adapted_resource,
            "derived enum resource drift")
    require(stdin_authority["resource_adapter"] == STDIN_RESOURCE_ADAPTER,
            "unapproved stdin parser resource adapter")
    stdin_resource = adapt_stdin_parser_resource(enum_resource)
    adapted_observer = adapt_unit2_observer(historical_bytes[OBSERVER])
    require(authority["unit2_observer_adapter"] == {
        "version": OBSERVER_ADAPTER_VERSION,
        "original": entry(OBSERVER, historical_bytes[OBSERVER]),
        "derived": entry(OBSERVER, adapted_observer),
        "substitutions": [{"old_sha256": digest(old), "new_sha256": digest(new), "count": 1}
                          for old, new in OBSERVER_SEAMS],
        "scope": "Four exact substitutions in an isolated current Unit2 copy: AggregateTy import, fail-closed record projection and four compiled adapter controls, Owned projection, Reference aggregate projection. Scalar/record JSON and frozen expectations remain unchanged; FixedArray projection panics.",
    }, "stale Unit2 observer adapter authority")
    borrowed_observer = adapt_borrowed_unit2_observer(adapted_observer)
    require(slices["unit2_observer_adapter"] == {
        "version": BORROWED_OBSERVER_ADAPTER_VERSION,
        "predecessor_version": OBSERVER_ADAPTER_VERSION,
        "original": entry(OBSERVER, historical_bytes[OBSERVER]),
        "predecessor_derived": entry(OBSERVER, adapted_observer),
        "derived": entry(OBSERVER, borrowed_observer),
        "substitutions": [{"old_sha256": digest(old), "new_sha256": digest(new), "count": 1}
                          for old, new in BORROWED_OBSERVER_SEAMS],
        "control_tests": list(OBSERVER_CONTROL_NAMES), "control_tests_per_profile": 6,
        "scope": BORROWED_OBSERVER_SCOPE,
    }, "stale borrowed Unit2 observer adapter authority")
    require(digest(package_bytes["enum-resource-authority.json"]) == ENUM_INDEX_RESOURCE_AUTHORITY_SHA
            and len(package_bytes["enum-resource-authority.json"]) == ENUM_INDEX_RESOURCE_AUTHORITY_BYTES,
            "stale enum index resource authority")
    index_resource_authority = json.loads(package_bytes["enum-resource-authority.json"])
    index_resource = adapt_enum_index_resource(historical_bytes[INDEX_RESOURCE])
    enum_rows = {row["path"]: row for row in enum_current["files"]}
    require(index_resource_authority["schema"] == "oxid-unit2-enum-free-index-resource-v1"
            and index_resource_authority["version"] == ENUM_INDEX_RESOURCE_VERSION
            and index_resource_authority["current_source_sha256"] == ENUM_SOURCE_SHA
            and index_resource_authority["reviewed_source_head"] == ENUM_HEAD
            and index_resource_authority["source_only_tree"] == ENUM_TREE
            and index_resource_authority["original"] == entry(INDEX_RESOURCE, historical_bytes[INDEX_RESOURCE])
            and index_resource_authority["derived"] == entry(INDEX_RESOURCE, index_resource)
            and index_resource_authority["changed_controls"] == list(ENUM_INDEX_RESOURCE_CONTROLS)
            and index_resource_authority["logical_resource_tests"] == 21
            and index_resource_authority["preserved_test_names"] == json.loads(historical_bytes["resource-test-names.json"])
            and index_resource_authority["substitutions"] == [
                {"old_sha256": digest(old), "new_sha256": digest(new), "count": 1}
                for old, new in ENUM_INDEX_RESOURCE_SEAMS]
            and all(row == enum_rows[row["path"]] for row in index_resource_authority["source_dependencies"]),
            "stale enum index resource binding")
    require(digest(package_bytes[SEMANTIC_HELPER]) == SEMANTIC_HELPER_SHA
            and digest(package_bytes[SEMANTIC_DESCRIPTOR]) == SEMANTIC_DESCRIPTOR_SHA,
            "unapproved enum semantic amendment")
    semantic_module = types.ModuleType("current_enum_semantic_amendment")
    semantic_module.__file__ = str(package / SEMANTIC_HELPER)
    exec(compile(package_bytes[SEMANTIC_HELPER], semantic_module.__file__, "exec"), semantic_module.__dict__)
    semantic_amendment = semantic_module.Amendment(package / "enum-source.json",
        repo / U2 / "semantic/corpus.jsonl.gz", package / SEMANTIC_DESCRIPTOR)
    semantic_receipt = semantic_amendment.receipt()
    semantic_report = {**semantic_receipt, "rows": [
        {"case": case_id,
         "frozen_expected_canonical_sha256": semantic_amendment.rows[case_id]["frozen_expected_canonical_sha256"],
         "old_expected": semantic_amendment.rows[case_id]["old_unit2_expected"],
         "current_expected": semantic_amendment.rows[case_id]["current_unit2_expected"],
         "current_result": semantic_amendment.rows[case_id]["current_unit2_result"]}
        for case_id in semantic_module.CASE_IDS]}
    enum_unit2_comparator = adapt_enum_unit2_comparator(historical_bytes[UNIT2_COMPARATOR])
    unit2_comparator = adapt_stdin_unit2_comparator(enum_unit2_comparator)
    require(stdin_authority["unit2_semantic_adapter"] == {
        "version": "unit2-stdin-enum-predecessor-semantic-binding-v1",
        "predecessor": entry(UNIT2_COMPARATOR, enum_unit2_comparator),
        "derived": entry(UNIT2_COMPARATOR, unit2_comparator),
        "substitution": {"old_sha256": digest(STDIN_COMPARATOR_OLD),
                         "new_sha256": digest(STDIN_COMPARATOR_NEW), "count": 1},
        "enum_source": entry("enum-source.json", package_bytes["enum-source.json"]),
        "scope": "Redirect only the exact enum semantic amendment constructor to the byte-identical enum predecessor manifest. Preserve its four-row roster, descriptor, helper, frozen comparison, and all observations.",
    }, "unapproved stdin Unit2 semantic adapter")
    enum_observer = adapt_enum_unit2_observer(borrowed_observer)
    require(enumeration["unit2_observer_adapter"] == {
        "version": ENUM_OBSERVER_ADAPTER_VERSION, "predecessor_version": BORROWED_OBSERVER_ADAPTER_VERSION,
        "original": entry(OBSERVER, historical_bytes[OBSERVER]),
        "predecessor_derived": entry(OBSERVER, borrowed_observer), "derived": entry(OBSERVER, enum_observer),
        "substitutions": [{"old_sha256": digest(old), "new_sha256": digest(new), "count": 1}
                          for old, new in ENUM_OBSERVER_SEAMS],
        "control_tests": list(OBSERVER_CONTROL_NAMES), "control_tests_per_profile": 6,
        "scope": ENUM_OBSERVER_SCOPE,
    }, "stale enum Unit2 observer adapter authority")
    require(digest(package_bytes["authority.json"]) == formatter["predecessor_authority_sha256"],
            "changed predecessor authority")
    return {"current": hir_current, "hir_import_authority": hir_authority, "hir_import_touched": hir_touched,
            "native_inventory_source": native_inventory_current, "native_inventory_inputs": native_inventory_inputs,
            "native_inventory_authority": native_inventory, "native_inventory_touched": native_inventory_touched,
            "native_storage_source": native_storage_current, "native_storage_inputs": native_storage_inputs,
            "stdout_source": stdout_current, "stdout_inputs": stdout_inputs,
            "native_storage_authority": native_storage, "native_storage_touched": native_storage_touched,
            "stdin_source": stdin_current, "stdin_inputs": stdin_inputs,
            "stdout_authority": stdout_authority, "stdout_touched": stdout_touched,
            "enum_source": enum_current, "enum_inputs": enum_inputs,
            "enum_authority": enumeration, "enum_touched": enum_touched,
            "stdin_authority": stdin_authority, "stdin_touched": stdin_touched,
            "projected_source": projected_current, "projected_inputs": projected_inputs, "unary_source": unary_current,
            "projected_authority": projected, "projected_touched": projected_touched,
            "unary_inputs": unary_inputs, "composition_source": composition_current,
            "unary_authority": unary, "unary_touched": unary_touched,
            "composition_inputs": composition_inputs, "slices_source": current,
            "composition_authority": composition, "composition_touched": composition_touched,
            "slices_inputs": inputs, "selected": selected, "historical": historical,
            "inputs": hir_inputs, "archived": reconstructed, "references": references,
            "historical_bytes": historical_bytes, "resource": stdin_resource,
            "enum_resource": enum_resource, "combined_resource": adapted_resource,
            "index_resource": index_resource, "index_resource_authority": index_resource_authority,
            "unit2_comparator": unit2_comparator, "enum_unit2_comparator": enum_unit2_comparator,
            "semantic_amendment": semantic_receipt,
            "semantic_report": semantic_report,
            "observer": enum_observer, "borrowed_observer": borrowed_observer, "aggregate_observer": adapted_observer,
            "package_bytes": package_bytes, "package_manifest": package_manifest,
            "touched": touched, "authority": authority,
            "predecessor_inputs": predecessor_inputs, "formatter_touched": formatter_touched,
            "formatter_authority": formatter, "formatter_source": formatter_source,
            "formatter_inputs": formatter_inputs, "combined_touched": combined_touched,
            "combined_authority": combined, "predecessor_resource": predecessor_resource,
            "combined_source": combined_source, "combined_inputs": combined_inputs,
            "division_authority": division, "division_touched": division_touched,
            "division_source": division_source, "division_inputs": division_inputs,
            "slices_authority": slices, "slices_touched": slices_touched}


def materialize(root, inputs):
    require(not root.exists(), "materialization already exists")
    root.mkdir(parents=True)
    for name, data in sorted(inputs.items()):
        target = root / relative(name)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    check_entries(root, [entry(name, data) for name, data in sorted(inputs.items())], exact=True)


def assert_unchanged(repo, captured, package=PACKAGE):
    fresh = preflight(repo, package)
    require(fresh == captured, "input identity changed during operation")


def prepare_archived(output, captured):
    materialize(output / "archived-selected", captured["archived"])
    return {"status": "prepared-archived-selected", "compiler_executions": 0,
            "semantic_pass": False, "direct_current_execution": False,
            "label": "Archived selected-current inputs reconstructed by pinned inverse source changes",
            "archived_root": str(output / "archived-selected"),
            "archived_files": captured["selected"]["files"],
            "archived_manifest_sha256": digest(captured["references"][U3 + "/manifests/selected-current.json"]),
            "inverse_patch_sha256": PATCH_SHA, "inverse_touched": captured["touched"],
            "formatter_inverse_patch_sha256": FORMATTER_PATCH_SHA,
            "formatter_inverse_touched": captured["formatter_touched"],
            "predecessor_source_sha256": PREDECESSOR_SOURCE_SHA,
            "combined_inverse_patch_sha256": COMBINED_PATCH_SHA,
            "combined_inverse_touched": captured["combined_touched"],
            "formatter_source_sha256": FORMATTER_SOURCE_SHA,
            "division_inverse_patch_sha256": DIVISION_PATCH_SHA,
            "division_inverse_touched": captured["division_touched"],
            "combined_source_sha256": COMBINED_SOURCE_SHA,
            "current_source_sha256": CURRENT_SOURCE_SHA,
            "hir_import_authority_sha256": HIR_IMPORT_AUTHORITY_SHA,
            "hir_import_inverse_patch_sha256": HIR_IMPORT_PATCH_SHA,
            "hir_import_inverse_touched": captured["hir_import_touched"],
            "native_inventory_source_sha256": NATIVE_INVENTORY_SOURCE_SHA,
            "native_inventory_authority_sha256": NATIVE_INVENTORY_AUTHORITY_SHA,
            "native_inventory_inverse_patch_sha256": NATIVE_INVENTORY_PATCH_SHA,
            "native_inventory_inverse_touched": captured["native_inventory_touched"],
            "native_storage_source_sha256": NATIVE_STORAGE_SOURCE_SHA,
            "native_storage_authority_sha256": NATIVE_STORAGE_AUTHORITY_SHA,
            "native_storage_inverse_patch_sha256": NATIVE_STORAGE_PATCH_SHA,
            "native_storage_inverse_touched": captured["native_storage_touched"],
            "stdout_source_sha256": STDOUT_SOURCE_SHA,
            "stdout_authority_sha256": STDOUT_AUTHORITY_SHA,
            "stdout_inverse_patch_sha256": STDOUT_PATCH_SHA,
            "stdout_inverse_touched": captured["stdout_touched"],
            "stdin_source_sha256": STDIN_SOURCE_SHA,
            "stdin_inverse_patch_sha256": STDIN_PATCH_SHA,
            "stdin_inverse_touched": captured["stdin_touched"],
            "enum_source_sha256": ENUM_SOURCE_SHA,
            "enum_inverse_patch_sha256": ENUM_PATCH_SHA,
            "enum_inverse_touched": captured["enum_touched"],
            "projected_source_sha256": PROJECTED_SOURCE_SHA,
            "projected_inverse_patch_sha256": PROJECTED_PATCH_SHA,
            "projected_inverse_touched": captured["projected_touched"],
            "unary_source_sha256": UNARY_SOURCE_SHA,
            "unary_inverse_patch_sha256": UNARY_PATCH_SHA,
            "unary_inverse_touched": captured["unary_touched"],
            "composition_source_sha256": COMPOSITION_SOURCE_SHA,
            "composition_inverse_patch_sha256": COMPOSITION_PATCH_SHA,
            "composition_inverse_touched": captured["composition_touched"],
            "slices_inverse_patch_sha256": SLICES_PATCH_SHA,
            "slices_inverse_touched": captured["slices_touched"],
            "division_source_sha256": DIVISION_SOURCE_SHA}


def prepare_unit2(output, captured):
    inputs = dict(captured["historical_bytes"])
    inputs[RESOURCE] = captured["resource"]
    inputs[INDEX_RESOURCE] = captured["index_resource"]
    inputs[UNIT2_COMPARATOR] = captured["unit2_comparator"]
    inputs[UNIT2_FROZEN_COMPARATOR] = captured["historical_bytes"][UNIT2_COMPARATOR]
    inputs[UNIT2_SEMANTIC_HELPER] = captured["package_bytes"][SEMANTIC_HELPER]
    inputs[UNIT2_SEMANTIC_DESCRIPTOR] = captured["package_bytes"][SEMANTIC_DESCRIPTOR]
    inputs["enum-source.json"] = captured["package_bytes"]["enum-source.json"]
    inputs[OBSERVER] = captured["observer"]
    require({name for name, data in inputs.items() if data != captured["historical_bytes"].get(name)}
            == {RESOURCE, INDEX_RESOURCE, OBSERVER, UNIT2_COMPARATOR, UNIT2_FROZEN_COMPARATOR,
                UNIT2_SEMANTIC_HELPER, UNIT2_SEMANTIC_DESCRIPTOR, "enum-source.json"}, "unexpected current Unit2 adapter member changes")
    manifest = {**captured["historical"], "files": [entry(name, data) for name, data in sorted(inputs.items())]}
    inputs["package-inputs.json"] = encoded(manifest)
    root = output / "compatibility" / "typed_project_unit2_independent"
    materialize(root, inputs)
    compat = output / "compatibility" / "typed_project_unit3_compatibility"
    materialize(compat, {"run.py": captured["references"][COMPAT]})
    return {"resource_package_root": str(root), "resource_package_inputs_sha256": digest(inputs["package-inputs.json"]),
            "resource_package_changes": [RESOURCE, INDEX_RESOURCE, OBSERVER, UNIT2_COMPARATOR, UNIT2_FROZEN_COMPARATOR,
                                         UNIT2_SEMANTIC_HELPER, UNIT2_SEMANTIC_DESCRIPTOR, "enum-source.json", "package-inputs.json"],
            "semantic_amendment": captured["semantic_amendment"],
            "stdin_semantic_adapter": captured["stdin_authority"]["unit2_semantic_adapter"],
            "index_resource_adapter": captured["index_resource_authority"],
            "observer_adapter": captured["enum_authority"]["unit2_observer_adapter"],
            "resource_before": next(x for x in captured["historical"]["files"] if x["path"] == RESOURCE),
            "resource_predecessor": captured["authority"]["derived_resource"],
            "resource_enum_predecessor": captured["enum_authority"]["resource_adapter"]["derived"],
            "resource_adapter": captured["stdin_authority"]["resource_adapter"],
            "resource_after": captured["stdin_authority"]["resource_adapter"]["derived"],
            "compatibility_runner": str(compat / "run.py"), "files": manifest["files"]}


def verify_unit2_result(output, captured, seam, prepare_only):
    compatibility = output / "unit2"
    run = compatibility / "run"
    artifact = "prepared.json" if prepare_only else "result.json"
    outer, child = read_json(compatibility / artifact), read_json(run / artifact)
    invocation = read_json(run / "invocation.json")
    expected_source = digest(captured["package_bytes"]["current-source.json"])
    final_manifest = {**captured["historical"], "files": [
        entry(item["path"], captured["package_bytes"]["current-source.json"])
        if item["path"] == "source-inputs.json" else item for item in seam["files"]]}
    final_manifest_bytes = encoded(final_manifest)
    package_sha = digest(final_manifest_bytes)
    final_package = compatibility / "derived-package"
    require(regular(final_package, "package-inputs.json").read_bytes() == final_manifest_bytes,
            "unexpected derived Unit2 package manifest")
    require(members(final_package) == sorted([x["path"] for x in final_manifest["files"]] + ["package-inputs.json"]),
            "missing or extra derived Unit2 package member")
    check_entries(final_package, final_manifest["files"])
    require(outer["historical_package_inputs_sha256"] == seam["resource_package_inputs_sha256"], "wrong seam package")
    require(outer["derived_package_inputs_sha256"] == package_sha, "wrong final Unit2 package")
    require(outer["child_result_sha256"] == digest((run / artifact).read_bytes()), "stale child result")
    require(outer["child_invocation_id"] == child["invocation_id"] == invocation["invocation_id"], "stale child invocation")
    for field, expected in (("source_inputs_sha256", expected_source), ("package_inputs_sha256", package_sha)):
        require(child[field] == invocation[field] == expected, "stale child binding: " + field)
    require(outer["source_inputs_sha256"] == expected_source, "wrong compatibility source")
    if prepare_only:
        require(outer["status"] == child["status"] == "prepared", "preparation claimed execution")
        require(outer["compiler_executions"] == child["compiler_executions"] == 0, "preparation executed compiler")
        require(not (run / "result.json").exists() and not (compatibility / "result.json").exists(),
                "preparation emitted pass")
    else:
        require(outer["status"] == child["status"] == "passed", "missing terminal Unit2 pass")
        require(child["profiles"] == ["debug", "release"] and child["semantic_cases_per_profile"] == 3603
                and child["resource_tests_per_profile"] == 21, "zero or partial Unit2 result")
        require([x["profile"] for x in child["receipts"]] == ["debug", "release"], "missing or duplicate profiles")
        for row in child["receipts"]:
            profile = row["profile"]
            receipt_file = run / (profile + "-receipt.json")
            require(row["sha256"] == digest(receipt_file.read_bytes()), "stale profile receipt")
            receipt = read_json(receipt_file)
            require(receipt["status"] == "passed" and receipt["exit_status"] == 0
                    and receipt["semantic_cases"] == 3603 and receipt["resource_tests"] == 21,
                    "zero or partial profile execution")
            for field in ("invocation_id", "source_inputs_sha256", "package_inputs_sha256", "queue_sha256", "queue_tsv_sha256"):
                require(receipt[field] == child[field], "stale profile binding: " + field)
            require(receipt["profile"] == profile, "wrong profile")
            binary = Path(receipt["binary"])
            require(binary.is_absolute() and (run / "target") in binary.parents
                    and not binary.is_symlink(), "test binary outside isolated build")
            require(digest(binary.read_bytes()) == receipt["binary_sha256"], "changed built binary")
            check_entries(run, receipt["artifacts"])
            current_comparison = read_json(run / (profile + "-comparison.json"))
            require(current_comparison["semantic_amendment"] == captured["semantic_report"],
                    "wrong current Unit2 semantic amendment receipt")
            historical = current_comparison["historical_semantic_comparison"]
            historical_name = profile + "-comparison-before-enum-enabled-qualified-values.json"
            require(historical["identity"] == captured["semantic_amendment"]["identity"]
                    and historical["historical_qualification_claim"] is False
                    and historical["status"] == 0
                    and historical["historical_mismatches"] == captured["semantic_amendment"]["cases"]
                    and historical["normalized_sha256"] == digest((run / (profile + "-normalized.jsonl")).read_bytes())
                    and historical["argv"] == [sys.executable, "-B", str(final_package / UNIT2_FROZEN_COMPARATOR),
                                               profile, str(run / (profile + "-normalized.jsonl")), str(run / historical_name)],
                    "wrong historical Unit2 comparison binding")
            for key, suffix in (("comparison", ".json"), ("stdout", ".stdout"), ("stderr", ".stderr")):
                path = (run / historical_name).with_suffix(suffix)
                require(historical[key] == entry(str(path), path.read_bytes()), "changed historical Unit2 report")
            historical_report = read_json(run / historical_name)
            require(historical_report["profile"] == profile and historical_report["cases"] == 3603
                    and historical_report["counts"] == {"match": 3599, "mismatch": 4}
                    and len(historical_report["results"]) == 3603
                    and len({row["case"] for row in historical_report["results"]}) == 3603
                    and sorted(row["case"] for row in historical_report["results"] if row["status"] == "mismatch")
                        == captured["semantic_amendment"]["cases"], "unapproved historical Unit2 comparison changes")

        check_entries(run, child["evidence"])
    check_entries(Path(seam["resource_package_root"]), seam["files"])
    require(digest((Path(seam["resource_package_root"]) / "package-inputs.json").read_bytes())
            == seam["resource_package_inputs_sha256"], "seam package changed")
    require((Path(seam["compatibility_runner"])).read_bytes() == captured["references"][COMPAT], "compatibility runner changed")
    return {"status": "prepared-current-unit2" if prepare_only else "passed-current-unit2",
            "semantic_pass": not prepare_only, "direct_current_execution": not prepare_only,
            "compiler_executions": 0 if prepare_only else "see retained Cargo build commands",
            "test_function_executions": 0 if prepare_only else 2 * (1 + 21),
            "semantic_cases_per_profile": 0 if prepare_only else 3603,
            "resource_tests_per_profile": 0 if prepare_only else 21,
            "child_result": str(run / artifact), "child_result_sha256": digest((run / artifact).read_bytes()),
            "compatibility_result_sha256": digest((compatibility / artifact).read_bytes()),
            "unit2_package_inputs_sha256": package_sha}


OBSERVER_CONTROL_FILTER = "current_unit2_aggregate_adapter_"
OBSERVER_CONTROL_NAMES = tuple("frontend::oir::unit2_observer::" + OBSERVER_CONTROL_FILTER + name for name in (
    "preserves_scalar_and_record_json", "denies_owned_array", "denies_shared_array", "denies_exclusive_array",
    "denies_shared_slice", "denies_exclusive_slice"))


def verify_observer_control_output(protocol, listing, execution):
    protocol.rust_listing(listing, list(OBSERVER_CONTROL_NAMES))
    # Rust's human test format explicitly labels registered should_panic tests.
    expected = [name + (" - should panic" if "_denies_" in name else "") for name in OBSERVER_CONTROL_NAMES]
    return protocol.rust_success(execution, expected)


def run_unit2_observer_controls(repo, output, captured, seam):
    """Execute current-only adapter controls on the two already verified test binaries."""
    run = output / "unit2/run"
    controls = output / "observer-adapter-controls"
    require(not controls.exists(), "observer adapter controls output exists")
    controls.mkdir()
    protocol_path = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_path.read_bytes() == captured["historical_bytes"]["protocol.py"], "observer control protocol changed")
    spec = importlib.util.spec_from_file_location("_current_unit2_control_protocol", protocol_path)
    protocol = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(protocol)
    receipts = []
    for profile in ("debug", "release"):
        assert_unchanged(repo, captured)
        source = run / "source"
        assembly = read_json(run / "assembly.json")
        check_entries(source, assembly["files"], exact=True)
        original_receipt = read_json(run / (profile + "-receipt.json"))
        binary = Path(original_receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "observer control binary outside isolated build")
        require(digest(binary.read_bytes()) == original_receipt["binary_sha256"], "observer control binary changed")
        commands, streams = [], {}
        for label, filters in (("list", ["--list", OBSERVER_CONTROL_FILTER]),
                               ("run", [OBSERVER_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            started = datetime.datetime.now(datetime.timezone.utc).isoformat()
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
            env.pop("PYTHONOPTIMIZE", None)
            status, timeout = None, False
            with out.open("wb") as stdout, err.open("wb") as stderr:
                try:
                    status = subprocess.run(argv, cwd=source, env=env, stdout=stdout, stderr=stderr, timeout=60).returncode
                except subprocess.TimeoutExpired:
                    timeout = True
            command = {"argv": argv, "cwd": str(source), "started_at_utc": started,
                       "finished_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                       "exit_status": status, "timed_out": timeout,
                       "stdout": entry(out.name, out.read_bytes()), "stderr": entry(err.name, err.read_bytes())}
            commands.append(command)
            write_json(controls / (profile + "-commands.json"), commands)
            require(status == 0 and not timeout, "current Unit2 observer controls failed; see retained streams")
            streams[label] = out.read_text(encoding="utf-8")
            require(digest(binary.read_bytes()) == original_receipt["binary_sha256"], "observer control binary changed")
        result = verify_observer_control_output(protocol, streams["list"], streams["run"])
        check_entries(source, assembly["files"], exact=True)
        assert_unchanged(repo, captured)
        record = {"schema": "oxid-current-unit2-observer-controls-v2", "profile": profile,
                  "status": "passed", "tests": list(OBSERVER_CONTROL_NAMES), "result": result,
                  "source_inputs_sha256": CURRENT_SOURCE_SHA, "observer_adapter": seam["observer_adapter"],
                  "original_unit2_receipt": entry(profile + "-receipt.json", (run / (profile + "-receipt.json")).read_bytes()),
                  "binary": entry(str(binary), binary.read_bytes()),
                  "assembly": entry("assembly.json", (run / "assembly.json").read_bytes()),
                  "commands": entry(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        write_json(path, record)
        receipts.append(entry(str(path.relative_to(output)), path.read_bytes()))
    return {"observer_adapter_version": ENUM_OBSERVER_ADAPTER_VERSION,
            "observer_control_tests_per_profile": 6, "observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("preflight", "prepare-archived", "run-unit2"))
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve(strict=True)
    output = args.output.resolve()
    require(output != repo and repo not in output.parents and output != PACKAGE and PACKAGE not in output.parents,
            "output must be outside input repository and adapter")
    require(not output.exists(), "output must be fresh; previous evidence is retained")
    output.mkdir(parents=True)
    result = {"schema": "oxid-current-archive-binding-v1", "invocation_id": uuid.uuid4().hex,
              "action": args.action, "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "status": "failed", "semantic_pass": False, "compiler_executions": 0,
              "controller_sha256": digest(Path(__file__).read_bytes())}
    try:
        captured = preflight(repo)
        result.update(current_source_sha256=digest(captured["package_bytes"]["current-source.json"]),
                      adapter_package_sha256=digest(captured["package_manifest"]),
                      hir_import_authority_sha256=HIR_IMPORT_AUTHORITY_SHA,
                      hir_import_inverse_patch_sha256=HIR_IMPORT_PATCH_SHA,
                      native_inventory_source_sha256=NATIVE_INVENTORY_SOURCE_SHA,
                      authority_sha256=digest(captured["package_bytes"]["authority.json"]),
                      formatter_authority_sha256=FORMATTER_AUTHORITY_SHA,
                      predecessor_source_sha256=PREDECESSOR_SOURCE_SHA,
                      combined_authority_sha256=COMBINED_AUTHORITY_SHA,
                      formatter_source_sha256=FORMATTER_SOURCE_SHA,
                      native_inventory_authority_sha256=NATIVE_INVENTORY_AUTHORITY_SHA,
                      native_inventory_inverse_patch_sha256=NATIVE_INVENTORY_PATCH_SHA,
                      native_storage_source_sha256=NATIVE_STORAGE_SOURCE_SHA,
                      native_storage_authority_sha256=NATIVE_STORAGE_AUTHORITY_SHA,
                      native_storage_inverse_patch_sha256=NATIVE_STORAGE_PATCH_SHA,
                      stdout_source_sha256=STDOUT_SOURCE_SHA,
                      stdout_authority_sha256=STDOUT_AUTHORITY_SHA,
                      stdout_inverse_patch_sha256=STDOUT_PATCH_SHA,
                      stdin_source_sha256=STDIN_SOURCE_SHA,
                      stdin_authority_sha256=STDIN_AUTHORITY_SHA,
                      enum_source_sha256=ENUM_SOURCE_SHA,
                      enum_authority_sha256=ENUM_AUTHORITY_SHA,
                      enum_index_resource_authority_sha256=ENUM_INDEX_RESOURCE_AUTHORITY_SHA,
                      semantic_amendment=captured["semantic_amendment"],
                      projected_source_sha256=PROJECTED_SOURCE_SHA,
                      projected_authority_sha256=PROJECTED_AUTHORITY_SHA,
                      unary_source_sha256=UNARY_SOURCE_SHA,
                      unary_authority_sha256=UNARY_AUTHORITY_SHA,
                      composition_source_sha256=COMPOSITION_SOURCE_SHA,
                      composition_authority_sha256=COMPOSITION_AUTHORITY_SHA,
                      slices_source_sha256=SLICES_SOURCE_SHA,
                      slices_authority_sha256=SLICES_AUTHORITY_SHA,
                      division_source_sha256=DIVISION_SOURCE_SHA,
                      division_authority_sha256=DIVISION_AUTHORITY_SHA,
                      combined_source_sha256=COMBINED_SOURCE_SHA)
        plan = {**result, "status": "planned", "repository": str(repo),
                "current_source_members": len(captured["inputs"]),
                "native_storage_source_members": len(captured["native_storage_inputs"]),
                "stdout_source_members": len(captured["stdout_inputs"]),
                "stdin_source_members": len(captured["stdin_inputs"]),
                "enum_source_members": len(captured["enum_inputs"]),
                "projected_source_members": len(captured["projected_inputs"]),
                "enum_scanner_fixture_members": 2, "enum_scanner_fixture_references": 4,
                "slices_source_members": len(captured["slices_inputs"]),
                "division_source_members": len(captured["division_inputs"]),
                "combined_source_members": len(captured["combined_inputs"]),
                "formatter_source_members": 133,
                "compile_time_fixture_members": 42, "compile_time_fixture_references": 47,
                "predecessor_source_members": 129, "archive_members": 117,
                "unit2_semantic_cases_per_profile": 3603, "unit2_resource_tests_per_profile": 21,
                "unit2_current_observer_controls_per_profile": 6}
        write_json(output / "plan.json", plan)
        result["plan_sha256"] = digest((output / "plan.json").read_bytes())
        if args.action == "preflight":
            result.update(status="verified-inputs", semantic_pass=False)
        elif args.action == "prepare-archived":
            result.update(prepare_archived(output, captured))
        else:
            seam = prepare_unit2(output, captured)
            write_json(output / "resource-seam.json", seam)
            result["resource_seam_sha256"] = digest((output / "resource-seam.json").read_bytes())
            assert_unchanged(repo, captured)
            argv = [sys.executable, "-B", seam["compatibility_runner"], "--repo", str(repo),
                    "--source-manifest", str(PACKAGE / "current-source.json"), "--output", str(output / "unit2"),
                    "--cargo", args.cargo, "--rustc", args.rustc]
            if args.prepare_only:
                argv.append("--prepare-only")
            env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", PYTHONDONTWRITEBYTECODE="1")
            env.pop("PYTHONOPTIMIZE", None)
            result["compiler_executions"] = 0 if args.prepare_only else "unverified-see-child-commands"
            with (output / "runner.stdout").open("wb") as stdout, (output / "runner.stderr").open("wb") as stderr:
                completed = subprocess.run(argv, cwd=repo, env=env, stdout=stdout, stderr=stderr)
            command = {"argv": argv, "cwd": str(repo), "exit_status": completed.returncode,
                       "plan_sha256": result["plan_sha256"],
                       "stdout": entry("runner.stdout", (output / "runner.stdout").read_bytes()),
                       "stderr": entry("runner.stderr", (output / "runner.stderr").read_bytes())}
            write_json(output / "command.json", command)
            result["command_sha256"] = digest((output / "command.json").read_bytes())
            require(completed.returncode == 0, "retained Unit2 gate failed; see runner.stderr and unit2 evidence")
            verified = verify_unit2_result(output, captured, seam, args.prepare_only)
            if not args.prepare_only:
                verified.update(run_unit2_observer_controls(repo, output, captured, seam))
                verify_unit2_result(output, captured, seam, False)
            result.update(verified)
        assert_unchanged(repo, captured)
        if args.action == "prepare-archived":
            check_entries(output / "archived-selected", captured["selected"]["files"], exact=True)
        require(result["plan_sha256"] == digest((output / "plan.json").read_bytes()), "execution plan changed")
    except (Exception, KeyboardInterrupt) as exc:
        result.update(status="failed", semantic_pass=False, error=f"{type(exc).__name__}: {exc}")
        print(result["error"], file=sys.stderr)
    finally:
        result["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        # Preparation and identity verification are never named result.json.
        artifact = "result.json" if result.get("semantic_pass") else "failure.json" if result["status"] == "failed" else "prepared.json"
        write_json(output / artifact, result)
    print(json.dumps({"status": result["status"], "output": str(output), "semantic_pass": result["semantic_pass"]}))
    return 1 if result["status"] == "failed" else 0


if __name__ == "__main__":
    raise SystemExit(main())
