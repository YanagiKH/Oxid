#!/usr/bin/env python3
"""Replay unchanged narrow semantics and resource assertions after adding i64.

The complete argument predecessor policy and nine predecessor policies remain
authenticated. Full-width semantics have independent public/native oracles.
"""
import argparse
import copy
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import types

PREDECESSOR = '2cd06c17079a929f81c8c92f3bebb32516f2c4dd'
PREDECESSOR_POLICY = 'scripts/verify_process_arguments_compatibility.py'
PREDECESSOR_POLICY_SHA = '5eb7c6c37e63b70e0d7b3267193689fb19578c134d54696355168378f068ec68'
PREDECESSOR_POLICY_BYTES = 21315
INTEGER_CHANGED = frozenset(('native/typed_preview.c', 'src/frontend/ast.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/driver.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/oir/comparison_tests.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/lower.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/owned/array_native_resource_tests.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_observe.rs', 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/enum_reference_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_experimental.rs', 'src/frontend/oir/owned/execute_file_delivery.rs', 'src/frontend/oir/owned/execute_io_delivery.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_experimental.rs', 'src/frontend/oir/owned/native_storage.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/native_resource_tests.rs', 'src/frontend/oir/owned/source/program.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/oir/verify.rs', 'src/frontend/parser.rs', 'src/frontend/parser/arrays.rs', 'src/frontend/parser/conversions.rs', 'src/frontend/parser/enums.rs', 'src/frontend/typeck.rs'))
INTEGER_ADDED = frozenset(('src/frontend/oir/owned/execute_values.rs', 'src/frontend/oir/owned/native_numeric.rs', 'src/frontend/oir/owned/source/i64_source_tests.rs'))
INTEGER_INPUTS = {'native/typed_preview.c': (3827, '5ef18f89ea3a889273ae4e53ac80a91e0519231f5f32884e7e023e2a337ec5e9'),
 'src/frontend/ast.rs': (42241, '8e4993abe0cf77707fed25a45873a7d8306e39404d1ce34a2ab699c733d99c54'),
 'src/frontend/declaration_index.rs': (76245,
                                       'd2b78b74ea594cf3bf34ce6700c65b76c56b6ca5775950b73637f71175517f48'),
 'src/frontend/declaration_index/enum_views.rs': (11163,
                                                  '4a48dc5f0eb3d69a3515fa267007fd7017e7d8175ac6603ac9a5e547e0a6d107'),
 'src/frontend/declaration_index/u8_reservation.rs': (27902,
                                                      'e9d28030a66904852e4f7e1e9d7c4239c98829d75f99768e35c857e85a044475'),
 'src/frontend/driver.rs': (82735, '98d2fb751e0b6377037abb2d023689ec2649bb2e8402c74487ed927c697c66ae'),
 'src/frontend/format.rs': (49378, '84032189ebdd7bbf07842c7afe5b2cc38f4f9e304697e946852a3508fb4e4035'),
 'src/frontend/format/ast_tests.rs': (65640,
                                      'c1102e6d4e9b5c000ab0c7004b1570ddb897d83ccb636c7c3cbc50dc7a02e59f'),
 'src/frontend/hir.rs': (27500, 'a823b5ef5e5fe8230c863005e44e1b579025774eb4a36afa9bc5e57def620a6b'),
 'src/frontend/oir/comparison_tests.rs': (9260,
                                          '98ae5aebe525783b42d943f322ed1a94837b705d72fb9b1490aecf20ef5e180f'),
 'src/frontend/oir/execute.rs': (22142, 'a32006d6b315c1a2a33fae7350e0084ce5b30805e82c8dbf1b3198a01435e1eb'),
 'src/frontend/oir/lower.rs': (42126, 'a3f32c43453e9163bac33a0c96c46a07a508aebdcc86a16032ea8800426926fe'),
 'src/frontend/oir/mod.rs': (14197, '15c442d0ef8e0827be634d34584c3d3e2330cd5147e84364cb91840daa4bfce0'),
 'src/frontend/oir/native.rs': (105811, '3d42c475c262ed3d2f99e2d35e9649436690011b847aca159f2be3db5e452c2b'),
 'src/frontend/oir/native_emit_work.rs': (28807,
                                          'd8769d1910a99f55a0f197430ed25fe42d997c9370d8481556b5632cf21502b4'),
 'src/frontend/oir/owned/array_native_resource_tests.rs': (53102,
                                                           'c12ad7d7890f8c7bf860654334bd61d166ab45a5f3d7d539cc27579e73602e76'),
 'src/frontend/oir/owned/array_native_tests.rs': (51301,
                                                  '49bdf4b5ad9cf0c30dbf0bd87c6f94dbebc7ced5f52af6aff00c390036290ad5'),
 'src/frontend/oir/owned/array_observe.rs': (24074,
                                             'c4671154175c8973bd4383d4c8c37d1c7894c9b0a9fe9daf13a56948713f5bb2'),
 'src/frontend/oir/owned/array_reference_boundary_tests.rs': (33515,
                                                              '324a55b1dfec61bae9e0e3ef950e3b9bad521b97cc4c17831c07748da48c4125'),
 'src/frontend/oir/owned/array_reference_tests.rs': (45015,
                                                     'd9d7ec71775528bd04add6fd4a812eea3e398431234ec3d5f7f210e22d0c31d0'),
 'src/frontend/oir/owned/array_tests.rs': (33684,
                                           '6154c758ed322e190284fe5e2106267f9d94ce828dd1537d076ea2f83726f285'),
 'src/frontend/oir/owned/enum_consumer_fixtures.rs': (30982,
                                                      'e91759b989bf3648cbe2ccb67b13a2ca2208210a6425066175516052cec212ba'),
 'src/frontend/oir/owned/enum_match_tests.rs': (49721,
                                                '2c9f2c321f02629226dd28ee6d06c75a6b13929c63965bcd6212bd96ff441b14'),
 'src/frontend/oir/owned/enum_native_tests.rs': (62134,
                                                 'ed6c6eb79404094a21789d4cf41bd40b8ab840ae8cb3c708eb3625eb219df443'),
 'src/frontend/oir/owned/enum_reference_tests.rs': (42513,
                                                    '9df1b000f1f3d4820e3324cc9466dfdbfc730bc6bbff8af69714f23e07b51786'),
 'src/frontend/oir/owned/execute.rs': (183204,
                                       'a811632211e5fd8a9e509bdcb7589644f49e21781bf14a4e8454522b1432b1de'),
 'src/frontend/oir/owned/execute_experimental.rs': (25025,
                                                    'ff88628e116ba0c318655acc378094b77720068e380285a64bccac5c2a2beb78'),
 'src/frontend/oir/owned/execute_file_delivery.rs': (16150,
                                                     '1c3a31c06685ff8f3f2b7259605ba1a3b8ec2c1684aa415638ebd07131781ecc'),
 'src/frontend/oir/owned/execute_io_delivery.rs': (12875,
                                                   'd2c37dcebf1680543ec97639053a9955bc7fa67482e5a8356eadc6a86856a0cd'),
 'src/frontend/oir/owned/execute_values.rs': (5887,
                                              '5d9122a0972d5517291f8f9f7b91f15b430b446db39a8a8911f6a996d2f5c670'),
 'src/frontend/oir/owned/native.rs': (218020,
                                      '506915428314ab0f7cc70d147f1046777dccbe2a8db79a876e70a2b37a1b4586'),
 'src/frontend/oir/owned/native_experimental.rs': (16885,
                                                   '5c1dc0b7fd50f983a49d5616bb4c654bd5abc8f12d96dab7775993d9219e8942'),
 'src/frontend/oir/owned/native_numeric.rs': (4248,
                                              '5197131d9bad812f1cb73f2f675e6d0126ba161dbec009edefbf5c700aa79340'),
 'src/frontend/oir/owned/native_storage.rs': (18561,
                                              '5ee38ac09604ade04df96678940af8e0dc871ad235ac74b03883e13d09b3b4ae'),
 'src/frontend/oir/owned/plan.rs': (26585,
                                    'f2470126436823e4dc800e2847c55cadaf14be53481a34d8dbca40593fba1be7'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (65425,
                                                              'e933e55fb8cada6245743370513bfcf4c7ced645f2e4c7023384e6881e005fec'),
 'src/frontend/oir/owned/shape.rs': (56258,
                                     '129d51004e75206f3b64d1166697ec91c8bdc3ab962ed1ab8cb8e70b341acc2a'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (54310,
                                                          '381583f4b9a382f38858d6e17b3dfcf628ebca2ed8f2d23b2fa67cf292c971fa'),
 'src/frontend/oir/owned/source/association.rs': (59836,
                                                  '8e28992030219d6da9eddaa796c8400486a97a8c50d3a0c3323ce0709b6c691a'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (42495,
                                                        '0f2c0baa7f8648a3af96571dcb38f8d8a566144dbd424fdf759789cf5cec3611'),
 'src/frontend/oir/owned/source/hir.rs': (6257,
                                          'f4ad76f99e4a7049699ba9bf4d43fe157fae37876383a558cc65114b60c0050c'),
 'src/frontend/oir/owned/source/i64_source_tests.rs': (9557,
                                                       '312980df8d9f511633ca148e1942e5ac8b797f4ea2340ff5acd1b6acaa619c9d'),
 'src/frontend/oir/owned/source/lower.rs': (113039,
                                            '5981a6c68f095cd7c09aed6856255150fa0cdfbeed1a68bfbcbaf4adfd6578ea'),
 'src/frontend/oir/owned/source/native_resource_tests.rs': (10298,
                                                            '793d1c3b995610effbf3b6d1b70cf33190c5f6e7afae82445255ffa0f7fe71e6'),
 'src/frontend/oir/owned/source/program.rs': (36734,
                                              '94a1a3597c9bec0f27fe0bea98269c1679804fbd8b359a9b2aace9b525eb335f'),
 'src/frontend/oir/owned/source/resolve.rs': (141811,
                                              '61e072e8d78ee24d9a54d021de5878715319f5546054ed062f39cf45f95b152f'),
 'src/frontend/oir/owned/source/typeck.rs': (210291,
                                             '9da2bd027655e69f8a15ed4dc33a14a1a05358297a0dcbedc3512d5e8f308f9c'),
 'src/frontend/oir/owned_types.rs': (77980,
                                     '9ef8d3e39f613cb20e0c2f15ba8e96def4308155f87a97d1e707e7adb4441be9'),
 'src/frontend/oir/owned_types/enums.rs': (55688,
                                           'aa0c2a9d0fef012d822ae5a27ec37a646a7b23b0bfa7d5d23b4aa86bdbc62f86'),
 'src/frontend/oir/source/association.rs': (30440,
                                            'bcc5a1ad4fed9bee2ad4ffd2b36b87e266c85c0a626db24e8ae219636f652cb1'),
 'src/frontend/oir/source/hir_import/candidate.rs': (52615,
                                                     '9f0fa4af053c2c234a1af42e8403ffb54073cca70e9b35fb5bff380fc4f2922c'),
 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs': (17324,
                                                                   'ff76c5bcf59ce9ce8e56fd35413e788ba67ec41283439a3ea5483bd215053d33'),
 'src/frontend/oir/source/sealed.rs': (22576,
                                       '227c14f4b01bec6af1f1883da2a733c9a930ccd75ba53a772d5d5bde1261f6fe'),
 'src/frontend/oir/verify.rs': (47173, '09d4254c11cd202349bd429edf393e4337ef8b8b2dc7c54e848031ee79f9ae84'),
 'src/frontend/parser.rs': (77313, 'e1993ef56855dfb6ed06578eab1c90ab58f117a29f5633ccf4d3c0267ef82deb'),
 'src/frontend/parser/arrays.rs': (9795, 'a103a11942d01d854ff4398a97ed93ab1af394b07eeae89de88d977457201fbd'),
 'src/frontend/parser/conversions.rs': (1697,
                                        '9aeeadea28f437c71c2fd341aebd3710b5971227c615e3f7e4fed3e7bddd1366'),
 'src/frontend/parser/enums.rs': (15814, '91d2243d2529e119169de2fbb5060c45cda8e1ec1f54d085fac9983c272e6d8e'),
 'src/frontend/typeck.rs': (35158, 'b54528aa50819e9ccafe94c987c972281a9171534b67ba6692933484574d056e')}
REVIEWED_DIRECTIVES = {'native/typed_preview.c': ([], []),
 'src/frontend/ast.rs': ([], []),
 'src/frontend/declaration_index.rs': ([], []),
 'src/frontend/declaration_index/enum_views.rs': ([], []),
 'src/frontend/declaration_index/u8_reservation.rs': ([], []),
 'src/frontend/driver.rs': ([], []),
 'src/frontend/format.rs': ([],
                            [b'#[path = "format/resource_tests.rs"]',
                             b'#[path = "format/ast_tests.rs"]',
                             b'#[path = "format/enum_candidate_tests.rs"]']),
 'src/frontend/format/ast_tests.rs': ([], []),
 'src/frontend/hir.rs': ([], []),
 'src/frontend/oir/comparison_tests.rs': ([], []),
 'src/frontend/oir/execute.rs': ([],
                                 [b'#[path = "execute_measurement.rs"]',
                                  b'#[path = "execute_tests.rs"]',
                                  b'#[path = "source_oracle.rs"]',
                                  b'#[path = "u8_execute_tests.rs"]']),
 'src/frontend/oir/lower.rs': ([], [b'#[path = "lower_measurement.rs"]']),
 'src/frontend/oir/mod.rs': ([], []),
 'src/frontend/oir/native.rs': ([],
                                [b'#[path = "native_private_emit.rs"]',
                                 b'#[path = "native_scalar_resource.rs"]',
                                 b'#[path = "native_emit_work.rs"]',
                                 b'#[path = "native_emit_cost.rs"]',
                                 b'#[path = "native_private_emit_tests.rs"]',
                                 b'#[path = "native_u8_tests.rs"]',
                                 b'#[path = "native_emit_observation.rs"]']),
 'src/frontend/oir/native_emit_work.rs': ([b'include_str!(concat!(\n        env!("CARGO_MANIFEST_DIR")'], []),
 'src/frontend/oir/owned/array_native_resource_tests.rs': ([], []),
 'src/frontend/oir/owned/array_native_tests.rs': ([], []),
 'src/frontend/oir/owned/array_observe.rs': ([], [b'#[path = "reviewer_array_observer_tests.rs"]']),
 'src/frontend/oir/owned/array_reference_boundary_tests.rs': ([], []),
 'src/frontend/oir/owned/array_reference_tests.rs': ([], [b'#[path = "array_reference_boundary_tests.rs"]']),
 'src/frontend/oir/owned/array_tests.rs': ([], []),
 'src/frontend/oir/owned/enum_consumer_fixtures.rs': ([], []),
 'src/frontend/oir/owned/enum_match_tests.rs': ([], []),
 'src/frontend/oir/owned/enum_native_tests.rs': ([], []),
 'src/frontend/oir/owned/enum_reference_tests.rs': ([], []),
 'src/frontend/oir/owned/execute.rs': ([],
                                       [b'#[path = "execute_values.rs"]',
                                        b'#[path = "execute_experimental.rs"]',
                                        b'#[path = "execute_resources.rs"]',
                                        b'#[path = "array_observe.rs"]',
                                        b'#[path = "execute_tests.rs"]',
                                        b'#[path = "builtin_output_reference_tests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.rs"]']),
 'src/frontend/oir/owned/execute_experimental.rs': ([],
                                                    [b'#[path = "execute_io_delivery.rs"]',
                                                     b'#[path = "execute_file_delivery.rs"]']),
 'src/frontend/oir/owned/execute_file_delivery.rs': ([], []),
 'src/frontend/oir/owned/execute_io_delivery.rs': ([], []),
 'src/frontend/oir/owned/execute_values.rs': ([], []),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_experimental.rs"]',
                                       b'#[path = "native_numeric.rs"]',
                                       b'#[path = "native_resources.rs"]',
                                       b'#[path = "builtin_input_native_tests.rs"]',
                                       b'#[path = "builtin_output_native_tests.rs"]',
                                       b'#[path = "native_tests.rs"]',
                                       b'#[path = "native_inventory_admission_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.rs"]']),
 'src/frontend/oir/owned/native_experimental.rs': ([], []),
 'src/frontend/oir/owned/native_numeric.rs': ([], []),
 'src/frontend/oir/owned/native_storage.rs': ([],
                                              [b'#[path = "native_inventory_tests.rs"]',
                                               b'#[path = "native_storage_tests.rs"]']),
 'src/frontend/oir/owned/plan.rs': ([], [b'#[path = "native_storage.rs"]']),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': ([], []),
 'src/frontend/oir/owned/shape.rs': ([], []),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': ([], []),
 'src/frontend/oir/owned/source/association.rs': ([],
                                                  [b'#[path = "output_lower_tests.rs"]',
                                                   b'#[path = "byte_storage_association_tests.rs"]']),
 'src/frontend/oir/owned/source/candidate_adapter.rs': ([],
                                                        [b'#[path = "candidate_mutations.rs"]',
                                                         b'#[path = "candidate_native.rs"]']),
 'src/frontend/oir/owned/source/hir.rs': ([], []),
 'src/frontend/oir/owned/source/i64_source_tests.rs': ([], []),
 'src/frontend/oir/owned/source/lower.rs': ([], []),
 'src/frontend/oir/owned/source/native_resource_tests.rs': ([], []),
 'src/frontend/oir/owned/source/program.rs': ([],
                                              [b'#[path = "experimental_source_tests.rs"]',
                                               b'#[path = "i64_source_tests.rs"]',
                                               b'#[path = "candidate_adapter.rs"]']),
 'src/frontend/oir/owned/source/resolve.rs': ([],
                                              [b'#[path = "resolver_paid_tests.rs"]',
                                               b'#[path = "resolver_enum_tests.rs"]',
                                               b'#[path = "enum_storage_failure_tests.rs"]',
                                               b'#[path = "builtin_signature_tests.rs"]',
                                               b'#[path = "output_typing_tests.rs"]',
                                               b'#[path = "byte_storage_type_tests.rs"]']),
 'src/frontend/oir/owned/source/typeck.rs': ([], [b'#[path = "enum_type_tests.rs"]']),
 'src/frontend/oir/owned_types.rs': ([], []),
 'src/frontend/oir/owned_types/enums.rs': ([], []),
 'src/frontend/oir/source/association.rs': ([],
                                            [b'#[path = "conversion_seen.rs"]',
                                             b'#[path = "../u8_association_tests.rs"]']),
 'src/frontend/oir/source/hir_import/candidate.rs': ([], []),
 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs': ([], []),
 'src/frontend/oir/source/sealed.rs': ([], []),
 'src/frontend/oir/verify.rs': ([], [b'#[path = "cyclic_tests.rs"]', b'#[path = "verify_measurement.rs"]']),
 'src/frontend/parser.rs': ([],
                            [b'#[path = "parser/arrays.rs"]',
                             b'#[path = "parser/conversions.rs"]',
                             b'#[path = "parser/enums.rs"]',
                             b'#[path = "parser/u8_syntax_tests.rs"]',
                             b'#[path = "parser/builtin_tests.rs"]',
                             b'#[path = "parser/array_syntax_tests.rs"]',
                             b'#[path = "parser/enum_syntax_tests.rs"]',
                             b'#[path = "parser/project_tests.rs"]',
                             b'#[path = "parser/activation_tests.rs"]']),
 'src/frontend/parser/arrays.rs': ([], []),
 'src/frontend/parser/conversions.rs': ([], []),
 'src/frontend/parser/enums.rs': ([], []),
 'src/frontend/typeck.rs': ([], [b'#[path = "typeck_measurement.rs"]', b'#[path = "branch_tests.rs"]'])}

def require(condition, message):
    if not condition:
        raise ValueError(message)


def regular(root, name):
    path = root / name
    require(not path.is_symlink() and all(not p.is_symlink() for p in path.parents),
            "symlink source: " + name)
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
            and not before.st_mode & 0o111 and before.st_size <= 2 * 1024 * 1024,
            "nonregular, executable, linked or oversized source: " + name)
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, f) for f in fields)
            == tuple(getattr(path.stat(), f) for f in fields) and len(body) == before.st_size,
            "source changed during read: " + name)
    return body


def load_predecessor(repo):
    body = regular(repo, PREDECESSOR_POLICY)
    require(len(body) == PREDECESSOR_POLICY_BYTES and hashlib.sha256(body).hexdigest() == PREDECESSOR_POLICY_SHA,
            "changed pinned program-argument source policy")
    module = types.ModuleType("_program_arguments_compatibility_parent")
    module.__file__ = str(repo / PREDECESSOR_POLICY)
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return module


library = load_predecessor(Path(__file__).resolve().parents[1])
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "RETAINED_RUNNER_SHA", "BINDING", "RETAINED", "OBSERVER_ADAPTER",
              "sha", "encoded", "identity", "load", "git", "checkout_identity", "extract_baseline",
              "authenticate_observer_adapter", "prepare_current_observer", "verify_current_observer",
              "verify_materialized", "run_byte_buffer_observer_controls"):
    globals()[_name] = getattr(library, _name)
PREDECESSOR_ADDED = library.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | INTEGER_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


OBSERVER_ADAPTER = "scripts/i64_compatibility_observer.py"
OBSERVER_ADAPTER_BYTES = 3330
OBSERVER_ADAPTER_SHA = 'c0a53a4887138984f8047274bdf72d080b805456468a1c74250c4365263f2c41'


def authenticate_observer_adapter(repo):
    body = regular(repo, OBSERVER_ADAPTER)
    require(len(body) == OBSERVER_ADAPTER_BYTES and sha(body) == OBSERVER_ADAPTER_SHA,
            "changed pinned i64 observer adapter")
    adapter = types.ModuleType("_i64_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    return adapter, body


run_file_observer_controls = library.run_file_observer_controls



def run_i64_observer_controls(repo, output, captured, seam, original, adapter):
    """Run i64 exclusions in addition to all unchanged prior observer controls."""
    run = output / "unit2/run"
    controls = output / "i64-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "changed control protocol")
    protocol = types.ModuleType("_i64_observer_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary = Path(receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "i64 control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "i64 control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.INTEGER_CONTROL_FILTER]),
                               ("run", [adapter.INTEGER_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            with out.open("xb") as stdout, err.open("xb") as stderr:
                completed = subprocess.run(argv, cwd=run / "source", stdout=stdout, stderr=stderr, timeout=60)
            commands.append({"argv": argv, "exit_status": completed.returncode,
                             "stdout": identity(out.name, out.read_bytes()),
                             "stderr": identity(err.name, err.read_bytes())})
            (controls / (profile + "-commands.json")).write_bytes(encoded(commands))
            require(completed.returncode == 0, "File observer controls failed; see retained streams")
            streams[label] = out.read_text(encoding="utf-8")
        names = list(adapter.INTEGER_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], [name + " - should panic" for name in names])
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "i64 control binary changed")
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "i64 observer source binding changed")
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        original.assert_unchanged(repo, captured)
        record = {"status": "passed", "profile": profile, "tests": names, "result": result,
                  "binary": identity(str(binary), binary.read_bytes()),
                  "source_inputs_sha256": original.CURRENT_SOURCE_SHA,
                  "commands": identity(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        path.write_bytes(encoded(record))
        receipts.append(identity(str(path.relative_to(output)), path.read_bytes()))
    return {"i64_observer_control_tests_per_profile": 2,
            "i64_observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6 + 3 + 3 + 2)}


def capture_parent(repo, parent, names):
    objects = {}
    for row in git(repo, "ls-tree", "-rzl", "--full-tree", PREDECESSOR).split(b"\0"):
        if not row:
            continue
        metadata, name = row.split(b"\t", 1)
        mode, kind, object_id, length = metadata.split()
        require(kind == b"blob" and name not in objects, "invalid parent blob roster")
        objects[name] = (mode, object_id, length)
    result = {}
    for name in sorted(names):
        body = regular(parent, name)
        mode, object_id, length = objects[name.encode()]
        blob = b"blob " + str(len(body)).encode() + b"\0" + body
        require(mode == b"100644" and int(length) == len(body)
                and hashlib.sha1(blob, usedforsecurity=False).hexdigest().encode() == object_id,
                "parent compiler input differs from pinned Git blob: " + name)
        result[name] = body
    return result


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | INTEGER_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(INTEGER_INPUTS) == INTEGER_CHANGED | INTEGER_ADDED
            and INTEGER_CHANGED <= expected_parent and not INTEGER_ADDED & expected_parent,
            "wrong reviewed i64 input membership")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    inputs = {name: regular(repo, name) for name in sorted(expected)}
    for name, before in parent.items():
        if name not in INTEGER_CHANGED:
            require(inputs[name] == before, "unreviewed change outside i64 boundary: " + name)
    for name, (length, digest) in INTEGER_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "i64 source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "i64 compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "i64_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(INTEGER_INPUTS.items())],
              "i64_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(INTEGER_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_i64_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the i64 successor; full-width semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_i64_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        checkout = checkout_identity(repo, args.expected_head, args.allow_dirty)
        baseline_root = output / "baseline"
        extract_baseline(repo, baseline_root, args.fetch_baseline)
        old_source = regular(baseline_root, BINDING + "/current-source.json")
        require(sha(old_source) == BASELINE_SOURCE_SHA, "wrong historical source manifest")
        original_body = regular(baseline_root, RETAINED)
        require(sha(original_body) == RETAINED_RUNNER_SHA, "wrong retained Unit2 predicates")
        historical = load("r1_historical_source_binding", baseline_root / BINDING / "run.py")
        captured = historical.preflight(baseline_root)
        original = load("r1_retained_unit2_assertions", baseline_root / RETAINED)
        predecessor_root = output / "predecessor"
        extract_baseline(repo, predecessor_root, args.fetch_baseline, PREDECESSOR)
        authenticate_policies(repo, predecessor_root)
        observer_adapter, observer_adapter_body = authenticate_observer_adapter(repo)
        predecessor_inputs = capture_parent(repo, predecessor_root, set(captured["inputs"]) | PREDECESSOR_ADDED)
        inputs = capture_sources(repo, captured["inputs"], predecessor_inputs)
        manifest = current_manifest(inputs, captured["inputs"], checkout, predecessor_inputs)
        manifest_raw = encoded(manifest)
        manifest_path = output / "current-source.json"
        manifest_path.write_bytes(manifest_raw)
        report.update(checkout=checkout, current_source=identity("current-source.json", manifest_raw),
                      historical_source=identity(BINDING + "/current-source.json", old_source),
                      retained_assertions=identity(RETAINED, original_body),
                      current_input_members=len(inputs), historical_results_apply_to_current=False)
        selected, seam = prepare_current_observer(original, output, captured, observer_adapter)
        report["byte_buffer_observer_adapter"] = observer_adapter.descriptor()
        report["byte_buffer_observer_adapter_controller"] = identity(OBSERVER_ADAPTER, observer_adapter_body)
        (output / "resource-seam.json").write_bytes(encoded(seam))
        # The separate source manifest is the existing compatibility seam. Other
        # historical captures, identities, comparators and expectations stay exact.
        selected["package_bytes"]["current-source.json"] = manifest_raw

        def unchanged(actual_repo, _capture):
            require(checkout_identity(actual_repo, args.expected_head, args.allow_dirty) == checkout,
                    "checkout changed during qualification")
            authenticate_policies(actual_repo, predecessor_root)
            require(capture_parent(actual_repo, predecessor_root, predecessor_inputs) == predecessor_inputs,
                    "pinned parent compiler materialization changed")
            require(authenticate_observer_adapter(actual_repo)[1] == observer_adapter_body,
                    "ByteBuf observer adapter changed during qualification")
            require(capture_sources(actual_repo, captured["inputs"], predecessor_inputs) == inputs,
                    "current source changed during qualification")
            require(manifest_path.read_bytes() == manifest_raw, "current manifest changed")

        original.assert_unchanged = unchanged
        original.CURRENT_SOURCE_SHA = sha(manifest_raw)
        unchanged(repo, selected)
        command = [sys.executable, "-B", seam["compatibility_runner"], "--repo", str(repo),
                   "--source-manifest", str(manifest_path), "--output", str(output / "unit2"),
                   "--cargo", args.cargo, "--rustc", args.rustc]
        if args.prepare_only:
            command.append("--prepare-only")
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", PYTHONDONTWRITEBYTECODE="1")
        env.pop("PYTHONOPTIMIZE", None)
        report["compiler_executions"] = 0 if args.prepare_only else "see retained Cargo command evidence"
        with (output / "runner.stdout").open("xb") as stdout, (output / "runner.stderr").open("xb") as stderr:
            completed = subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=5400)
        (output / "command.json").write_bytes(encoded({"argv": command, "exit_status": completed.returncode,
            "stdout": identity("runner.stdout", (output / "runner.stdout").read_bytes()),
            "stderr": identity("runner.stderr", (output / "runner.stderr").read_bytes())}))
        require(completed.returncode == 0, "retained Unit2 gate failed; see runner.stderr and unit2 evidence")
        verified = original.verify_unit2_result(output, selected, seam, args.prepare_only)
        verify_materialized(output, inputs)
        verify_current_observer(output, observer_adapter, selected["observer"])
        if not args.prepare_only:
            verified.update(original.run_unit2_observer_controls(repo, output, selected, seam))
            verified.update(run_byte_buffer_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_file_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_i64_observer_controls(repo, output, selected, seam, original, observer_adapter))
            original.verify_unit2_result(output, selected, seam, False)
            verify_materialized(output, inputs)
            verify_current_observer(output, observer_adapter, selected["observer"])
        unchanged(repo, selected)
        report.update(verified)
        report["status"] = "prepared" if args.prepare_only else "passed"
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        artifact = "prepared.json" if args.prepare_only else "result.json"
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"],
                          "source_kind": checkout["source_kind"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        for name in set(sys.modules) - set(aliases):
            sys.modules.pop(name, None)
        sys.modules.update(aliases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true", help="fetch only the fixed public baseline commit if missing")
    parser.add_argument("--allow-dirty", action="store_true", help="record local worktree evidence; never label it committed")
    parser.add_argument("--prepare-only", action="store_true", help="exercise admission/assembly with zero compiler executions")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
