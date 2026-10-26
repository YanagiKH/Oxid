#!/usr/bin/env python3
"""Replay retained bounded assertions after Linux ARM64 native support.

The constructor-inference predecessor and twenty-seven preceding policies stay
byte-authenticated. Current target behavior has separate native qualification.
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

PREDECESSOR = '00b813bd9735abf3a9a6c95b3826b17f95ab4c1d'
PREDECESSOR_POLICY = 'scripts/verify_constructor_inference_compatibility.py'
PREDECESSOR_POLICY_SHA = 'bd63a49665cd7a3340f0b7d51448a157015eed8be043c768721486a25773ea3d'
PREDECESSOR_POLICY_BYTES = 20650
LINUX_NATIVE_CHANGED = frozenset(('build.rs', 'src/frontend/build_config.rs', 'src/frontend/driver.rs', 'src/frontend/hir_producer.rs', 'src/frontend/lexical_provider.rs', 'src/frontend/native.rs', 'src/frontend/native/doctor.rs', 'src/frontend/native_cache.rs', 'src/frontend/oir/cyclic_tests.rs', 'src/frontend/oir/logical_tests.rs', 'src/frontend/oir/loop_control_tests.rs', 'src/frontend/oir/mutable_tests.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/negation_raw_tests.rs', 'src/frontend/oir/owned/allocator_review_controls.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/binary_io.rs', 'src/frontend/oir/owned/builtin_output_process_tests.rs', 'src/frontend/oir/owned/byte_storage_native_tests.rs', 'src/frontend/oir/owned/composition_native_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/file_resource_tests.rs', 'src/frontend/oir/owned/file_resources.rs', 'src/frontend/oir/owned/input.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_composition.rs', 'src/frontend/oir/owned/native_heldout_review.rs', 'src/frontend/oir/owned/native_inventory_admission_tests.rs', 'src/frontend/oir/owned/native_tests.rs', 'src/frontend/oir/owned/negation_raw_tests.rs', 'src/frontend/oir/owned/output.rs', 'src/frontend/oir/owned/process.rs', 'src/frontend/oir/owned/projected_slice_native_tests.rs', 'src/frontend/oir/owned/slice_native_tests.rs', 'src/frontend/oir/owned/source/array_consumer_tests.rs', 'src/frontend/oir/owned/source/builtin_source_tests.rs', 'src/frontend/oir/owned/source/byte_integer_tests.rs', 'src/frontend/oir/owned/source/candidate_native.rs', 'src/frontend/oir/owned/source/character_tests.rs', 'src/frontend/oir/owned/source/enum_native_source_tests.rs', 'src/frontend/oir/owned/source/experimental_source_tests.rs', 'src/frontend/oir/owned/source/float_tests.rs', 'src/frontend/oir/owned/source/i64_source_tests.rs', 'src/frontend/oir/owned/source/integer128_tests.rs', 'src/frontend/oir/owned/source/integer_bit_tests.rs', 'src/frontend/oir/owned/source/integer_width_tests.rs', 'src/frontend/oir/owned/source/io_delivery_source_tests.rs', 'src/frontend/oir/owned/source/native_resource_tests.rs', 'src/frontend/oir/owned/source/output_source_tests.rs', 'src/frontend/oir/owned/source/process_arguments_tests.rs', 'src/frontend/oir/owned/source/u8_tests.rs', 'src/frontend/oir/owned/u8_tests.rs', 'src/frontend/oir/source/hir_import/emit_native_tests.rs', 'src/frontend/options.rs', 'src/project_cli.rs', 'tests/typed_record_composition.rs'))
LINUX_NATIVE_ADDED = frozenset()
LINUX_NATIVE_INPUTS = {'build.rs': (1864, 'e1a78e13abfbe3fd7580d27ae5d567085ea3d1344148f87dbbc5dad224a625ab'),
 'src/frontend/build_config.rs': (22370, '41038e0e9263c29fbf3bb869bd01b72bc9047389e9f8ae63dda162ccc7f6d71a'),
 'src/frontend/driver.rs': (82569, '41ea8b15e9cd31524bc8e7e9bd8b0f755d6ac055860951f0557926a4b94137be'),
 'src/frontend/hir_producer.rs': (10466, '892d81cf61bef8afa038e9858cdb2b6bfaebe632de4142f3618e1934288b844d'),
 'src/frontend/lexical_provider.rs': (30526,
                                      'd722e7b95d7a68f2c798741f13ba0d8a4202f6b275ed352eebb4633495d13875'),
 'src/frontend/native.rs': (19906, 'dde5566c8fb5e61d178229cc38d9068f560426dc79dce569d3f40e15a206d32f'),
 'src/frontend/native/doctor.rs': (20408, 'c6c481d75bb29ac4278eefb5a1e4f513f8284096f26ce78a66b9915d8cc1c3bf'),
 'src/frontend/native_cache.rs': (25178, 'b46f8e5d88d7eba1f7c16ad0c6157b32e27b424c0d08a26e7800ef2f43a1061a'),
 'src/frontend/oir/cyclic_tests.rs': (42020,
                                      'ea64f42a8305b64e1e81a1dc01e861495d7802ff275a2635a26decd9bae0f74d'),
 'src/frontend/oir/logical_tests.rs': (19325,
                                       '21fa7f13459207e49d5426a3e77cbc197acb500cc531f960e6b420e7e800c204'),
 'src/frontend/oir/loop_control_tests.rs': (9965,
                                            'a7c1c45508cd2e8ea5057ddf641d10dda30bf99071a7435c1180a712281d9933'),
 'src/frontend/oir/mutable_tests.rs': (18824,
                                       '1e640b9622a61839c286aaf8df533a8eeda8b68297f81fab7317814bf7879b8d'),
 'src/frontend/oir/native.rs': (115780, '94bc4e758ee13ecedd770ef234787f03ee8e238ce2ea14bc848945be1faa1352'),
 'src/frontend/oir/negation_raw_tests.rs': (16543,
                                            '40531638c6ac938b043fdd0fd972d871e299953577be62c3ff1c5f34bcaae8da'),
 'src/frontend/oir/owned/allocator_review_controls.rs': (17008,
                                                         '04ce1760dab6ec4c60898448e17f5829e8747f10abd408d7014d5c93ba738df1'),
 'src/frontend/oir/owned/array_native_tests.rs': (53648,
                                                  '38a8ddb36ecf3c31f655e0289766651e4f259cfd2864645fc3edb98360ce1a51'),
 'src/frontend/oir/owned/binary_io.rs': (13727,
                                         '16bc9430e60a34580b1a9afe29ed8979972b97491e231143ee09f4ad8148cd6c'),
 'src/frontend/oir/owned/builtin_output_process_tests.rs': (19770,
                                                            'e47431857d066ccc36ff300ae54b0054a5780f22a591e79c73290af60bb2f350'),
 'src/frontend/oir/owned/byte_storage_native_tests.rs': (66035,
                                                         '18ad85a341aa29108ff347f83f503ca850bb7104f00033bffff9716942cffd0d'),
 'src/frontend/oir/owned/composition_native_tests.rs': (24386,
                                                        'd17cc83a6207cc42841aecb1bb438d043ff85915bb821bba34301edf8bc030a3'),
 'src/frontend/oir/owned/enum_native_tests.rs': (63606,
                                                 '2b5d1e678f091b9fedd4962ea4ba37eedee62c3ff7584d17b542d6059ac5eabe'),
 'src/frontend/oir/owned/execute.rs': (202474,
                                       'd82960f160b3285d4872f0dc75fa72d67117e364488f6b948421edbde661b98a'),
 'src/frontend/oir/owned/file_resource_tests.rs': (5800,
                                                   'f16a79d13825e499606860847c9f504ac4ff1e867bebe17c47eba2649113b203'),
 'src/frontend/oir/owned/file_resources.rs': (13911,
                                              '2f06702429c01c233fbfe2cbbbb705ede4afd95cd6763e8e822da9d1ae65f4ba'),
 'src/frontend/oir/owned/input.rs': (1802,
                                     '38f94df1a22960b201ad57547fe581d1857a73abf3fb4c2e8c6fdbda8c0c2045'),
 'src/frontend/oir/owned/native.rs': (253804,
                                      '6ab6d8dad43fa356f83022b840562b24e63af4f9adaa538a10f2745555246f56'),
 'src/frontend/oir/owned/native_composition.rs': (26220,
                                                  'b0c51b79db9f81d2874afdef720b60a2167ca96bc1c00e3f6f35dae717058e38'),
 'src/frontend/oir/owned/native_heldout_review.rs': (25704,
                                                     '8db2d8cf85d2747872cf89ac4c1b8b20e00c848dddbe4a975b58ef6a1587f528'),
 'src/frontend/oir/owned/native_inventory_admission_tests.rs': (17908,
                                                                'a74a78190519454ceacb95a0a27cfb266140d1912718945858d3eeee18d37563'),
 'src/frontend/oir/owned/native_tests.rs': (86786,
                                            'd8f67e336860c38227c4f83d1bcdcda5549ee26c7ee6128b2b1a7ab0ecae6a6a'),
 'src/frontend/oir/owned/negation_raw_tests.rs': (13709,
                                                  'e334a878a44a5dc7483aab1b217d7fc63954ed3b1b9384de3fb546f6a8a695cc'),
 'src/frontend/oir/owned/output.rs': (2937,
                                      '08d5aa65711e165d2b3851c99ef5da46a3e34b3e2d803e63e389b0cf7c9140d6'),
 'src/frontend/oir/owned/process.rs': (3216,
                                       'b4f99be3190e3ef1c6c46c4aadc97f1dc4d286892f3166fbef5464494a876808'),
 'src/frontend/oir/owned/projected_slice_native_tests.rs': (18619,
                                                            '28a7f0bf2266aa049d605b17fd122d9ad1ef7a5f75bc2a85b2611ab4dfcdfe07'),
 'src/frontend/oir/owned/slice_native_tests.rs': (13099,
                                                  '4d784f228703b36bced3779d85e487bfbc7dcc611193c9b6953a014b88528226'),
 'src/frontend/oir/owned/source/array_consumer_tests.rs': (18190,
                                                           'fb979cfc9ebc6597e8acaeeaee8b4dda15e6885a52bba42014e4b8f7b0c00cb9'),
 'src/frontend/oir/owned/source/builtin_source_tests.rs': (11742,
                                                           'c61e419ee33cbd7d70f1397d1f1c963beb9400c9b2a70b33689edb32e69ba865'),
 'src/frontend/oir/owned/source/byte_integer_tests.rs': (3327,
                                                         'ab928813327607ac583bcc71a13325567071fca3ec30fb890f0b26f68f6b7b80'),
 'src/frontend/oir/owned/source/candidate_native.rs': (32520,
                                                       'e0ed5beba4d920b06fcfdcaf95e50c6a7d0c6079bf6e586e26b3816b8dc39cbe'),
 'src/frontend/oir/owned/source/character_tests.rs': (3321,
                                                      'd86fa63a8f90e47d10fb9071da078cb464c3ff2387f5c8b1afbd457ed2fddcc6'),
 'src/frontend/oir/owned/source/enum_native_source_tests.rs': (27090,
                                                               '80c047d17a515f0a73ba32291635eaabbe1056214503fa5bc3b20d59e337a2c5'),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': (25223,
                                                                'ff68e831d8230a8b34a108935efdaccef4285108661bb582737c38fe7e32d190'),
 'src/frontend/oir/owned/source/float_tests.rs': (3282,
                                                  '40d4c1500b9f5dcf75c8e2b2c7f3b60cad8c9986cf1574785dacb3813fca3343'),
 'src/frontend/oir/owned/source/i64_source_tests.rs': (9660,
                                                       '71d1feef8097d3fa8ad5bf49a084c23f799e1ce32e751970d79b66b288986d93'),
 'src/frontend/oir/owned/source/integer128_tests.rs': (4298,
                                                       '58e7b855180301474270cc291136577594542571d225b505ee83b3ffcae26716'),
 'src/frontend/oir/owned/source/integer_bit_tests.rs': (3421,
                                                        'ec72192ae28775573f345a9c498dab5bc75854cc77ffcd4fab6155f23cb8ce34'),
 'src/frontend/oir/owned/source/integer_width_tests.rs': (3542,
                                                          '567264a27abfdd15815615baacf10edd3833d42963f2aece211babd610629168'),
 'src/frontend/oir/owned/source/io_delivery_source_tests.rs': (14148,
                                                               '1c01f4b5154b08c709f6008c7ac3176e22bac5afff9c5a530539f01b38722791'),
 'src/frontend/oir/owned/source/native_resource_tests.rs': (10401,
                                                            'e7383069cb9aa44afd2d544810ca95042f9aeb1b3922f45c417eb0820b2ec046'),
 'src/frontend/oir/owned/source/output_source_tests.rs': (22260,
                                                          '52d6107a2beb28bd0224bf073e2768bf30df4eb992e50b37060d9c7129458dd9'),
 'src/frontend/oir/owned/source/process_arguments_tests.rs': (6686,
                                                              '4986842f334c9bd6368f6bfdbee2d4d7f8283c043f6feff2243d1910c5053c5b'),
 'src/frontend/oir/owned/source/u8_tests.rs': (27924,
                                               '445aa54ea964d026549ceab33a20ac491a4d56eba701ab18e1bf12e9c502b291'),
 'src/frontend/oir/owned/u8_tests.rs': (11301,
                                        '04785ecc97c99e699dc05b613b038429167b4c727b6b9f6ada4549bb876d88a5'),
 'src/frontend/oir/source/hir_import/emit_native_tests.rs': (5102,
                                                             'df965968305d5493e83ed97babb295d7ee1020769b9ebc9e64484d5847d67f48'),
 'src/frontend/options.rs': (66142, '18f86cb574124541c2261e246a1d389fd0bddb2412abcf2262d411de24bf8388'),
 'src/project_cli.rs': (60991, '9dc5ff6f026921bf97f88305bb26018989585a530fc812ba03d4b36602e58ef9'),
 'tests/typed_record_composition.rs': (13321,
                                       '799beb7e0728e0406ea29d4ac93aa952ead370342e27f06edaa3273b82fb5255')}
REVIEWED_DIRECTIVES = {'build.rs': ([], []),
 'src/frontend/build_config.rs': ([], []),
 'src/frontend/driver.rs': ([], []),
 'src/frontend/hir_producer.rs': ([], []),
 'src/frontend/lexical_provider.rs': ([], []),
 'src/frontend/native.rs': ([b'include_str!("../../native/typed_preview.c")',
                             b'include_str!("../../native/typed_character.c")',
                             b'include_str!("../../native/typed_float.c")',
                             b'include_bytes!(concat!(env!("OUT_DIR")',
                             b'include_str!("../../native/typed_integer128.c")',
                             b'include_str!("../../native/typed_bytebuf.c")',
                             b'include_str!("../../native/experimental_bytebuf.c")',
                             b'include_str!("../../native/experimental_files.c")',
                             b'include_str!("../../native/experimental_arguments.c")',
                             b'include_str!("../../native/experimental_bytebuf.h")'],
                            []),
 'src/frontend/native/doctor.rs': ([], []),
 'src/frontend/native_cache.rs': ([], []),
 'src/frontend/oir/cyclic_tests.rs': ([], []),
 'src/frontend/oir/logical_tests.rs': ([], []),
 'src/frontend/oir/loop_control_tests.rs': ([], []),
 'src/frontend/oir/mutable_tests.rs': ([], []),
 'src/frontend/oir/native.rs': ([],
                                [b'#[path = "native_private_emit.rs"]',
                                 b'#[path = "native_scalar_resource.rs"]',
                                 b'#[path = "native_emit_work.rs"]',
                                 b'#[path = "native_emit_cost.rs"]',
                                 b'#[path = "native_private_emit_tests.rs"]',
                                 b'#[path = "native_u8_tests.rs"]',
                                 b'#[path = "native_emit_observation.rs"]']),
 'src/frontend/oir/negation_raw_tests.rs': ([], []),
 'src/frontend/oir/owned/allocator_review_controls.rs': ([], []),
 'src/frontend/oir/owned/array_native_tests.rs': ([], []),
 'src/frontend/oir/owned/binary_io.rs': ([], []),
 'src/frontend/oir/owned/builtin_output_process_tests.rs': ([], []),
 'src/frontend/oir/owned/byte_storage_native_tests.rs': ([], []),
 'src/frontend/oir/owned/composition_native_tests.rs': ([], []),
 'src/frontend/oir/owned/enum_native_tests.rs': ([], []),
 'src/frontend/oir/owned/execute.rs': ([],
                                       [b'#[path = "execute_floats.rs"]',
                                        b'#[path = "execute_values.rs"]',
                                        b'#[path = "execute_bitwise.rs"]',
                                        b'#[path = "execute_integer128.rs"]',
                                        b'#[path = "execute_integers.rs"]',
                                        b'#[path = "execute_experimental.rs"]',
                                        b'#[path = "execute_composition.rs"]',
                                        b'#[path = "execute_resources.rs"]',
                                        b'#[path = "array_observe.rs"]',
                                        b'#[path = "execute_tests.rs"]',
                                        b'#[path = "builtin_output_reference_tests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.rs"]']),
 'src/frontend/oir/owned/file_resource_tests.rs': ([], []),
 'src/frontend/oir/owned/file_resources.rs': ([], [b'#[path = "file_resource_tests.rs"]']),
 'src/frontend/oir/owned/input.rs': ([], []),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_bitwise.rs"]',
                                       b'#[path = "native_composition.rs"]',
                                       b'#[path = "native_experimental.rs"]',
                                       b'#[path = "native_floats.rs"]',
                                       b'#[path = "native_integer128.rs"]',
                                       b'#[path = "native_integers.rs"]',
                                       b'#[path = "native_numeric.rs"]',
                                       b'#[path = "native_resources.rs"]',
                                       b'#[path = "builtin_input_native_tests.rs"]',
                                       b'#[path = "builtin_output_native_tests.rs"]',
                                       b'#[path = "native_tests.rs"]',
                                       b'#[path = "native_inventory_admission_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.rs"]']),
 'src/frontend/oir/owned/native_composition.rs': ([], []),
 'src/frontend/oir/owned/native_heldout_review.rs': ([], []),
 'src/frontend/oir/owned/native_inventory_admission_tests.rs': ([], []),
 'src/frontend/oir/owned/native_tests.rs': ([],
                                            [b'#[path = "source/native_resource_tests.rs"]',
                                             b'#[path = "native_heldout_review.rs"]',
                                             b'#[path = "source/reviewer_resource_native.rs"]',
                                             b'#[path = "array_native_resource_tests.rs"]',
                                             b'#[path = "array_native_tests.rs"]',
                                             b'#[path = "slice_native_tests.rs"]',
                                             b'#[path = "composition_native_tests.rs"]',
                                             b'#[path = "projected_slice_native_tests.rs"]',
                                             b'#[path = "enum_native_tests.rs"]']),
 'src/frontend/oir/owned/negation_raw_tests.rs': ([], []),
 'src/frontend/oir/owned/output.rs': ([], []),
 'src/frontend/oir/owned/process.rs': ([], []),
 'src/frontend/oir/owned/projected_slice_native_tests.rs': ([], []),
 'src/frontend/oir/owned/slice_native_tests.rs': ([], []),
 'src/frontend/oir/owned/source/array_consumer_tests.rs': ([], []),
 'src/frontend/oir/owned/source/builtin_source_tests.rs': ([], []),
 'src/frontend/oir/owned/source/byte_integer_tests.rs': ([], []),
 'src/frontend/oir/owned/source/candidate_native.rs': ([],
                                                       [b'#[ignore = "bounded native source collector; req'
                                                        b'uires frozen original source, budget controls, p'
                                                        b'inned LLVM and a fresh output path"]']),
 'src/frontend/oir/owned/source/character_tests.rs': ([], []),
 'src/frontend/oir/owned/source/enum_native_source_tests.rs': ([b'include_str!(concat!(\n    env!("CARGO_MA'
                                                                b'NIFEST_DIR")',
                                                                b'include_str!(concat!(\n    env!("CARGO_MA'
                                                                b'NIFEST_DIR")',
                                                                b'include_str!("enum_native_source_tests.r'
                                                                b's")'],
                                                               []),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': ([],
                                                                [b'#[path = "io_delivery_source_tests.rs"]',
                                                                 b'#[path = "process_arguments_tests.rs"]']),
 'src/frontend/oir/owned/source/float_tests.rs': ([], []),
 'src/frontend/oir/owned/source/i64_source_tests.rs': ([], []),
 'src/frontend/oir/owned/source/integer128_tests.rs': ([], []),
 'src/frontend/oir/owned/source/integer_bit_tests.rs': ([], []),
 'src/frontend/oir/owned/source/integer_width_tests.rs': ([], []),
 'src/frontend/oir/owned/source/io_delivery_source_tests.rs': ([], []),
 'src/frontend/oir/owned/source/native_resource_tests.rs': ([], []),
 'src/frontend/oir/owned/source/output_source_tests.rs': ([], []),
 'src/frontend/oir/owned/source/process_arguments_tests.rs': ([], []),
 'src/frontend/oir/owned/source/u8_tests.rs': ([], []),
 'src/frontend/oir/owned/u8_tests.rs': ([], []),
 'src/frontend/oir/source/hir_import/emit_native_tests.rs': ([b'include_str!("emit_native_tests.rs")',
                                                              b'include_str!("../../../../../native/typed_pr'
                                                              b'eview.c")'],
                                                             []),
 'src/frontend/options.rs': ([], []),
 'src/project_cli.rs': ([], []),
 'tests/typed_record_composition.rs': ([b'include_str!(\n        "../fixtures/typed-record-composition-samp'
                                        b'les/main.ox"\n    )',
                                        b'include_str!("../fixtures/typed-record-composition-samples/model'
                                        b'.ox")',
                                        b'include_str!("../fixtures/typed-record-composition-samples/ops.o'
                                        b'x")'],
                                       [])}


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
            "changed pinned constructor-inference source policy")
    module = types.ModuleType("_git_source_bundle_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | LINUX_NATIVE_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


PARSER_RESOURCE = "archive/resource/parser-resource-review-tests.rs"
PARSER_RESOURCE_INPUT = (2114, "e3081188e6dfb0bd171e806992091f3a58b9b463d38a194b481341ff3064a2b5")
PARSER_RESOURCE_OUTPUT = (2185, 'ac2d1933c7aa16a4a0b7db1f4af6d96b4d8801da7a646da29f5e9050980c72c1')
PARSER_RESOURCE_BEFORE = b"storage:enums::SyntaxStorage::default(),tokens,"
PARSER_RESOURCE_AFTER = b"storage:enums::SyntaxStorage::default(),generic_headers:Vec::new(),generic_calls:Vec::new(),pending_equal:None,tokens,"


def adapt_parser_resource(body):
    require(type(body) is bytes and (len(body), sha(body)) == PARSER_RESOURCE_INPUT,
            "wrong parser resource predecessor")
    require(body.count(PARSER_RESOURCE_BEFORE) == 1 and PARSER_RESOURCE_AFTER not in body,
            "parser resource initializer drift")
    result = body.replace(PARSER_RESOURCE_BEFORE, PARSER_RESOURCE_AFTER, 1)
    require((len(result), sha(result)) == PARSER_RESOURCE_OUTPUT
            and result.replace(PARSER_RESOURCE_AFTER, PARSER_RESOURCE_BEFORE, 1) == body,
            "wrong parser resource successor")
    return result


def prepare_with_parser_resource(original, output, captured, adapter, resource):
    return library.prepare_with_parser_resource(original, output, captured, adapter, resource)


def prepare_current_observer(original, output, captured, adapter):
    return prepare_with_parser_resource(original, output, captured, adapter,
                                        adapt_parser_resource(captured["resource"]))


run_file_observer_controls = library.run_file_observer_controls
run_i64_observer_controls = library.run_i64_observer_controls
run_integer_width_observer_controls = library.run_integer_width_observer_controls
run_integer128_observer_controls = library.run_integer128_observer_controls
run_float_observer_controls = library.run_float_observer_controls
run_character_observer_controls = library.run_character_observer_controls


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
    expected = expected_parent | LINUX_NATIVE_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(LINUX_NATIVE_INPUTS) == LINUX_NATIVE_CHANGED | LINUX_NATIVE_ADDED
            and LINUX_NATIVE_CHANGED <= expected_parent and not LINUX_NATIVE_ADDED & expected_parent,
            "wrong reviewed linux-native input membership")
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
        if name not in LINUX_NATIVE_CHANGED:
            require(inputs[name] == before, "unreviewed change outside linux-native boundary: " + name)
    for name, (length, digest) in LINUX_NATIVE_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "linux-native source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "linux-native compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "linux_native_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(LINUX_NATIVE_INPUTS.items())],
              "linux_native_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(LINUX_NATIVE_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns", "record_enum_fields", "constructor_inference", "linux_native"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_linux_native_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources after Linux ARM64 native support; new target behavior qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_linux_native_compatibility.py", Path(__file__).read_bytes())}
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
            verified.update(run_integer_width_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_integer128_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_float_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_character_observer_controls(repo, output, selected, seam, original, observer_adapter))
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
