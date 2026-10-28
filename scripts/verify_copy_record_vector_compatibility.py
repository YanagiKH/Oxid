#!/usr/bin/env python3
"""Bind current Copy record vector inputs to the complete retained R1 replay.

This gate excludes new vector/platform semantics from its claims. Provisional
pins permit preparation only; old policies and resource predicates stay frozen.
"""
import argparse
import copy
import datetime
import hashlib
import itertools
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import types

PREDECESSOR = "b4218473dc5e62738bab18073b09a4a186050950"
PREDECESSOR_POLICY = "scripts/verify_checked_copy_compatibility.py"
PREDECESSOR_POLICY_PIN = (136844, "1cd08956a98baf020548ce4d01823013e31a1cd60e89a1971d480a798ae7db5e")
# This checkpoint admits the inspected R2 snapshot for preparation, never a pass.
PINS_PROVISIONAL = True
WINDOWS_ACTIVATION_ENABLED = False
PROTECTED_CURRENT_PIN = (5888, "b24162962f6149422ebe38f58f6fe05d16de71ea06d81aa809df6b979035666d")
RECORD_VECTOR_CHANGED = frozenset(['native/experimental_vector.c',
 'native/experimental_vector.h',
 'native/typed_vector.c',
 'src/frontend/declaration_index/generics.rs',
 'src/frontend/declaration_index/generics/nominal.rs',
 'src/frontend/declaration_index/generics/vector_tests.rs',
 'src/frontend/declaration_index/sealed.rs',
 'src/frontend/native.rs',
 'src/frontend/oir/owned/array_observe.rs',
 'src/frontend/oir/owned/budget.rs',
 'src/frontend/oir/owned/builtins.rs',
 'src/frontend/oir/owned/byte_buffer.rs',
 'src/frontend/oir/owned/cfg.rs',
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/execute_composition.rs',
 'src/frontend/oir/owned/execute_experimental.rs',
 'src/frontend/oir/owned/execute_resources.rs',
 'src/frontend/oir/owned/execute_tests.rs',
 'src/frontend/oir/owned/execute_vector.rs',
 'src/frontend/oir/owned/flow.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/native_composition.rs',
 'src/frontend/oir/owned/native_experimental.rs',
 'src/frontend/oir/owned/native_resources.rs',
 'src/frontend/oir/owned/native_storage.rs',
 'src/frontend/oir/owned/native_vector.rs',
 'src/frontend/oir/owned/native_vector_layout.rs',
 'src/frontend/oir/owned/plan.rs',
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
 'src/frontend/oir/owned/shape.rs',
 'src/frontend/oir/owned/source/array_pipeline_rows.rs',
 'src/frontend/oir/owned/source/association.rs',
 'src/frontend/oir/owned/source/budget.rs',
 'src/frontend/oir/owned/source/candidate_adapter.rs',
 'src/frontend/oir/owned/source/candidate_native.rs',
 'src/frontend/oir/owned/source/copy_record_tests.rs',
 'src/frontend/oir/owned/source/diagnostic.rs',
 'src/frontend/oir/owned/source/lower.rs',
 'src/frontend/oir/owned/source/record_enum_layouts.rs',
 'src/frontend/oir/owned/source/resolve.rs',
 'src/frontend/oir/owned/source/typeck.rs',
 'src/frontend/oir/owned/source/vector_lower.rs',
 'src/frontend/oir/owned/source/vector_source_tests.rs',
 'src/frontend/oir/owned/vector_descriptors.rs',
 'src/frontend/oir/owned/vector_raw_tests.rs',
 'src/frontend/oir/owned/vector_resources.rs',
 'src/frontend/oir/owned/vector_type.rs',
 'src/frontend/oir/owned/verified.rs',
 'src/frontend/oir/owned_types.rs',
 'src/frontend/oir/owned_types/composition.rs',
 'src/frontend/oir/owned_types/enums.rs',
 'src/frontend/oir/owned_types/resources.rs',
 'src/frontend/vector_builtins.rs'])
RECORD_VECTOR_ADDED = frozenset(['src/frontend/oir/owned/record_vector_native_tests.rs',
 'src/frontend/oir/owned/record_vector_resources_tests.rs',
 'src/frontend/oir/owned/record_vector_shape_tests.rs'])
REVIEWED_INPUTS = {'native/experimental_vector.c': (18989, '20b359ceca6e988712f28e4045be3a5d9446eefc4a40b5fbd3d023a37298ace7'),
 'native/experimental_vector.h': (3645, '6e469b8d3f02fea1a53687ce6d00ece2f1e00a21973b71f5eecc61fb8b615fe9'),
 'native/typed_vector.c': (13138, 'f364cd26e5129ceb09146fc6ac6b17f5aa6b876ee401d00561feeb1a54376bee'),
 'src/frontend/declaration_index/generics.rs': (134152,
                                                'd74dc267ba7bb1a36759147056e3ec108045e8764a5a0ccb401f20bb48f66731'),
 'src/frontend/declaration_index/generics/nominal.rs': (30546,
                                                        '58ec2dfe3dda6108e1a5ba10c750a3b95fdc60ecce505e0d260e9b98c83893fd'),
 'src/frontend/declaration_index/generics/vector_tests.rs': (21981,
                                                             '82a4b038e3410d091587dc9620a41fd67b5dc7a5eb469380d0a960385b53daf6'),
 'src/frontend/declaration_index/sealed.rs': (107999,
                                              '07d7aab57200579bc61f3198ea9bc1a048cdb4c62a24fd31bcd23fdcccae97da'),
 'src/frontend/native.rs': (21910, 'a66c8ea5590b22a0da7d3c21fb08a60db5944fd52ff348b57c7733920c8fb9f8'),
 'src/frontend/oir/owned/array_observe.rs': (24199,
                                             '3595945b7d7ad64549f1f764501aeb9e59f6101edccbe59bb7c7b9bb68ec9546'),
 'src/frontend/oir/owned/budget.rs': (22314, 'b356da179a82cac2141dac44c836deffb8d71e32c4235f25eb4ea06df93b1e49'),
 'src/frontend/oir/owned/builtins.rs': (14862, 'c39bdfb0fab2dcadd29fc62f0835102f50bbd65b7fa3e4fc8129c3e5e302c62e'),
 'src/frontend/oir/owned/byte_buffer.rs': (21131,
                                           '17aacfe9f245312146a1954fe70e7a2dfb73582e93d9ea5ce0dd7f69f476a5ad'),
 'src/frontend/oir/owned/cfg.rs': (8122, 'f5f0f0f5cbcb88e445e60e024fe1b3fecd1ee419e00e0fb54cb95af9864ac977'),
 'src/frontend/oir/owned/execute.rs': (216596, '85e5600be4a3032f13f35bfccb6778f7fbffa196d30a2fdda6d61dfb5429ddf5'),
 'src/frontend/oir/owned/execute_composition.rs': (11319,
                                                   'd68e1cc803cdbcd01eb56965e021973bb1605bb8239a082ea59b1433f57e1ea5'),
 'src/frontend/oir/owned/execute_experimental.rs': (27622,
                                                    '12690f17d79ff350f75f58d0c0dcc95bea115c48f197fd58b27aa8d0f03445e0'),
 'src/frontend/oir/owned/execute_resources.rs': (8163,
                                                 '2c9a7e35b15be1d65ae893a7c98cdd9d610fd95d5ee011745ca670ccbee59de3'),
 'src/frontend/oir/owned/execute_tests.rs': (64990,
                                             '55feecb7890f4673dfc8538e78fdbc250208ffb8a0c8486d9f17509b1ea74516'),
 'src/frontend/oir/owned/execute_vector.rs': (68153,
                                              '2d4d141147222b4c4d44c7561d57a0808d4c169ea120373fe22ea20d903a99b5'),
 'src/frontend/oir/owned/flow.rs': (57139, '0b972983dbd0b549f552490d647955a972b546ac0632ee5ee4b6d850110cb105'),
 'src/frontend/oir/owned/mod.rs': (17730, 'fa9cde5e0c3e398c8c491b86421dee3347ca3d7388712764bee7d3e2f0967e5c'),
 'src/frontend/oir/owned/native.rs': (274964, '8384f5d0a1962f11253d019b2d2050bcbed1b5ec3ad82342eff1c6591bb003c2'),
 'src/frontend/oir/owned/native_composition.rs': (27967,
                                                  '7734e19a638312bae4fbc33a74e90e3eda5fbef22a4a008046432853d0b6d713'),
 'src/frontend/oir/owned/native_experimental.rs': (18527,
                                                   'ec11bbf96ed00f6f4aea0ce570fa8f5c1e506511667b3b8ab6418ef82a17e5e5'),
 'src/frontend/oir/owned/native_resources.rs': (8090,
                                                '2d3e7b472d0aed14f4c673073b9323f17e43ef8cca3b81d64d5b77311f9a0be5'),
 'src/frontend/oir/owned/native_storage.rs': (20251,
                                              '6de8d8a74ba396b415a02fe9bc6a204f75a0704ececf28b0eadc32d858d91000'),
 'src/frontend/oir/owned/native_vector.rs': (22559,
                                             '98ac475f20a95b716d8ce2fc7475bdc7fe7c3f26e606c5761fa9d660a46a3c3e'),
 'src/frontend/oir/owned/native_vector_layout.rs': (4859,
                                                    '98cf67c335d47d511e2badf31d93f1d8b47f16450cac66ae135aa0038bd41f19'),
 'src/frontend/oir/owned/plan.rs': (38268, 'dc9acf4718432687a51443814705f125be515ea1188d85193e459f7ea91b2da6'),
 'src/frontend/oir/owned/record_vector_native_tests.rs': (5550,
                                                          '51a8e4b53cb34049fe34783def417e8b381e17a425c5497e002363988b1ff974'),
 'src/frontend/oir/owned/record_vector_resources_tests.rs': (12681,
                                                             'a78c0b8d4d73d901a580fa33acc1fd9dc0b1189a7d2a35385c927575c4f11404'),
 'src/frontend/oir/owned/record_vector_shape_tests.rs': (6336,
                                                         '78d2b50339050f91d9e4ae71bf0d7d32e5a6b282b01c75444b2dec27e13d1a92'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (66749,
                                                              '52f867b0b0c9d90dfec37489da7a51021edf8b2f21f26d78e4969dbe28431d85'),
 'src/frontend/oir/owned/shape.rs': (66152, 'fb11989d33d0654be7e842db9739335112b8699e3fb07a3bbf7567b933f7d6ac'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (58438,
                                                          '8f5e6fd1a9c64c9bb6900e3fac2263db3e937c47450b43ade18f49b585b8d963'),
 'src/frontend/oir/owned/source/association.rs': (92151,
                                                  'f6867fc93cd615c9176eaa3b3074f7b305133ef7d72e64e025782223aab9972d'),
 'src/frontend/oir/owned/source/budget.rs': (17874,
                                             '86004088d98fd08ec906a5e2e52f7c708d2c74bcbbc4313c1624b28a42cbfa9f'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (44629,
                                                        '997000146eb2bf926140f94b654423a60a937d76ac7d807b617c30f1fe8230b4'),
 'src/frontend/oir/owned/source/candidate_native.rs': (32882,
                                                       'e152681912aa0277cb10607de38889c45dbfae758044b8b88ff8c0806e849435'),
 'src/frontend/oir/owned/source/copy_record_tests.rs': (15663,
                                                        'c2414df535069c93ae1346433ffdd09deef0dbc58023fca3b0be301212d51843'),
 'src/frontend/oir/owned/source/diagnostic.rs': (9057,
                                                 '357ecf5128000c531b80d90139099e1060a9e992beb38045924acb31b0a12297'),
 'src/frontend/oir/owned/source/lower.rs': (137209,
                                            '6247b227f3a6038ca924831faaceb25398d59938fffc30fb8135b1187f9050db'),
 'src/frontend/oir/owned/source/record_enum_layouts.rs': (13543,
                                                          '6d3d97fdb4228921e0f1c3fe9ab845fd29a6a2f4536b113649810ee17033be24'),
 'src/frontend/oir/owned/source/resolve.rs': (170103,
                                              '21853446cb849015652182f4cb49d4fc7ed82f7f33a0d017bd870a1644864245'),
 'src/frontend/oir/owned/source/typeck.rs': (224943,
                                             '870b0ae5158816fc6fc387f2e52f21bc321721403acd2416474d0591dd8e3e7f'),
 'src/frontend/oir/owned/source/vector_lower.rs': (6876,
                                                   '952986e43828c8c4c00080480ae2686bb3d52e817fb1458e37fae9fe579016a4'),
 'src/frontend/oir/owned/source/vector_source_tests.rs': (21223,
                                                          '3bf4a8e080463cce16cf943273a0fd3953b230a2e61e0dd223a48b04ec508542'),
 'src/frontend/oir/owned/vector_descriptors.rs': (7786,
                                                  '4510cd5f4626bd183700e6278df1038d96bfd9e673167b121a4778342cff57b1'),
 'src/frontend/oir/owned/vector_raw_tests.rs': (27007,
                                                '7a5a7b3a1882046b5ef8d142c941bc1c45abd3d2053da4efd6766b89dcb1c78c'),
 'src/frontend/oir/owned/vector_resources.rs': (26487,
                                                'b82015cebcde98e91c4223b1d675ee49ea977bf56f2507ff3ef4d154690a1d24'),
 'src/frontend/oir/owned/vector_type.rs': (4482,
                                           '3142b6bb2929fd47abc34f3b95cd5bed609a5971a07b4629cb4b1856842dcec0'),
 'src/frontend/oir/owned/verified.rs': (29907, 'dac7ceaf33738c1379e48b48f5bf01e3c7cbc8a26e219c8854aebc7fa0c464fd'),
 'src/frontend/oir/owned_types.rs': (97969, 'd4059ab5b9f4a40a2dbd9a5e33bd78ba30272663122de66376175bd1d5009c61'),
 'src/frontend/oir/owned_types/composition.rs': (19981,
                                                 'b32d9e675a884f8c085d88c676f8c7c0a94fe8133ac847ede1776ac7fb6984f7'),
 'src/frontend/oir/owned_types/enums.rs': (61138,
                                           '15a3c232a6bc776ad5cb8f58c9161e552dbd8fa7beaeea8435bbb5ebd5876509'),
 'src/frontend/oir/owned_types/resources.rs': (31685,
                                               '00a7721a3020da253275953b2283993a4e1a06077270381e97aac5fc51c6c866'),
 'src/frontend/vector_builtins.rs': (15854, 'e0cf6daeae32941642cec01174564a0ec8f3f7bd85c7b4ab9bd0eed1bf6d08c7')}
REVIEWED_DIRECTIVES = {'native/experimental_vector.c': {'attributes': (),
                                  'build_script': None,
                                  'c_preprocessor': ((b'#include "experimental_vector.h"\n', 1),
                                                     (b'#include <limits.h>\n', 1),
                                                     (b'#include <stdlib.h>\n', 1),
                                                     (b'#include <string.h>\n', 1),
                                                     (b'#ifndef OXID_VECTOR_ALIGNED_ALLOC\n', 1),
                                                     (b'#define OXID_VECTOR_ALIGNED_ALLOC aligned_alloc\n', 1),
                                                     (b'#endif\n', 1),
                                                     (b'#ifndef OXID_RECORD_VECTOR_SCHEMAS\n', 1),
                                                     (b'#else\n', 1),
                                                     (b'#endif\n', 1)),
                                  'legacy': ([], []),
                                  'macros': ()},
 'native/experimental_vector.h': {'attributes': (),
                                  'build_script': None,
                                  'c_preprocessor': ((b'#ifndef OXID_EXPERIMENTAL_VECTOR_H\n', 1),
                                                     (b'#define OXID_EXPERIMENTAL_VECTOR_H\n', 1),
                                                     (b'#include "experimental_bytebuf.h"\n', 1),
                                                     (b'#define OXID_VECTOR_NODE (UINT32_MAX - UINT32_C(1))\n', 1),
                                                     (b'#define OXID_VECTOR_HEADER UINT64_C(48)\n', 1),
                                                     (b'#define OXID_VECTOR_ALIGNMENT 16u\n', 1),
                                                     (b'#define OXID_VECTOR_RECORD_BIT UINT32_C(0x80000000)\n', 1),
                                                     (b'#define OXID_VECTOR_RECORD_LIMIT UINT32_C(4096)\n', 1),
                                                     (b'#define OXID_VECTOR_RECORD_VALIDATION_WORK UINT64_C(131)'
                                                      b'\n',
                                                      1),
                                                     (b'#ifdef OXID_RECORD_VECTOR_SCHEMAS\n', 1),
                                                     (b'#endif\n', 2)),
                                  'legacy': ([], []),
                                  'macros': ()},
 'native/typed_vector.c': {'attributes': (),
                           'build_script': None,
                           'c_preprocessor': (),
                           'legacy': ([], []),
                           'macros': ()},
 'src/frontend/declaration_index/generics.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                                               (b'#[derive(Default)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 1),
                                                               (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                                3),
                                                               (b'#[allow(dead_code)]', 1),
                                                               (b'#[derive(Debug, PartialEq, Eq)]', 3),
                                                               (b'#[allow(clippy::too_many_arguments)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 2),
                                                               (b'#[allow(clippy::too_many_arguments)]', 1)),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/declaration_index/generics/nominal.rs': {'attributes': ((b'#[derive(Debug, PartialEq, Eq)]', 2),
                                                                       (b'#[derive(Debug, Default, PartialEq, Eq)]',
                                                                        1),
                                                                       (b'#[allow(dead_code)]', 1),
                                                                       (b'#[allow(clippy::too_many_arguments)]',
                                                                        3)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/declaration_index/generics/vector_tests.rs': {'attributes': ((b'#[test]', 12),),
                                                             'build_script': None,
                                                             'c_preprocessor': (),
                                                             'legacy': ([], []),
                                                             'macros': ()},
 'src/frontend/declaration_index/sealed.rs': {'attributes': ((b'#[derive(Debug)]', 1),
                                                             (b'#[repr(C, align(8))]', 1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[cfg(test)]', 3),
                                                             (b'#[derive(Debug)]', 3),
                                                             (b'#[cfg(test)]', 5),
                                                             (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                                             (b'#[cfg(test)]', 3),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 4),
                                                             (b'#[cfg(not(test))]', 1),
                                                             (b'#[allow(unused_mut)]', 1),
                                                             (b'#[cfg(test)]', 11),
                                                             (b'#[cfg(not(test))]', 1),
                                                             (b'#[cfg(test)]', 7),
                                                             (b'#[test]', 2),
                                                             (b'#[cfg(test)]', 3),
                                                             (b'#[test]', 5)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([], []),
                                              'macros': ()},
 'src/frontend/native.rs': {'attributes': ((b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos"),\n  '
                                            b'  target_pointer_width = "64",\n    target_endian = "little",\n   '
                                            b' any(target_arch = "x86_64", target_arch = "aarch64")\n))]',
                                            1),
                                           (b'#[cfg(not(all(\n    any(target_os = "linux", target_os = "macos")'
                                            b',\n    target_pointer_width = "64",\n    target_endian = "little",'
                                            b'\n    any(target_arch = "x86_64", target_arch = "aarch64")\n)))]',
                                            1),
                                           (b'#[cfg(unix)]', 1)),
                            'build_script': None,
                            'c_preprocessor': (),
                            'legacy': ([b'include_str!("../../native/typed_preview.c")',
                                        b'include_str!("../../native/typed_character.c")',
                                        b'include_str!("../../native/typed_float.c")',
                                        b'include_bytes!(concat!(env!("OUT_DIR")',
                                        b'include_str!("../../native/typed_integer128.c")',
                                        b'include_str!("../../native/typed_bytebuf.c")',
                                        b'include_str!("../../native/experimental_bytebuf.c")',
                                        b'include_str!("../../native/experimental_files.c")',
                                        b'include_str!("../../native/experimental_arguments.c")',
                                        b'include_str!("../../native/experimental_bytebuf.h")',
                                        b'include_str!("../../native/experimental_vector.c")',
                                        b'include_str!("../../native/experimental_vector.h")',
                                        b'include_str!("../../native/typed_vector.c")'],
                                       []),
                            'macros': ((b'include_str!("../../native/typed_preview.c")', 1),
                                       (b'include_str!("../../native/typed_character.c")', 1),
                                       (b'include_str!("../../native/typed_float.c")', 1),
                                       (b'include_bytes!(concat!(env!("OUT_DIR"), "/float_format.a"))', 1),
                                       (b'env!("OUT_DIR")', 1),
                                       (b'include_str!("../../native/typed_integer128.c")', 1),
                                       (b'include_str!("../../native/typed_bytebuf.c")', 1),
                                       (b'include_str!("../../native/experimental_bytebuf.c")', 1),
                                       (b'include_str!("../../native/experimental_files.c")', 1),
                                       (b'include_str!("../../native/experimental_arguments.c")', 1),
                                       (b'include_str!("../../native/experimental_bytebuf.h")', 1),
                                       (b'include_str!("../../native/experimental_vector.c")', 1),
                                       (b'include_str!("../../native/experimental_vector.h")', 1),
                                       (b'include_str!("../../native/typed_vector.c")', 1),
                                       (b'cfg!(target_os = "macos")', 4))},
 'src/frontend/oir/owned/array_observe.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                            (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                            (b'#[derive(Debug)]', 1),
                                                            (b'#[derive(Default)]', 1),
                                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                            (b'#[test]', 1),
                                                            (b'#[derive(Clone, Copy, Debug)]', 1),
                                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                            (b'#[path = "reviewer_array_observer_tests.rs"]', 1),
                                                            (b'#[test]', 1)),
                                             'build_script': None,
                                             'c_preprocessor': (),
                                             'legacy': ([], [b'#[path = "reviewer_array_observer_tests.rs"]']),
                                             'macros': ()},
 'src/frontend/oir/owned/budget.rs': {'attributes': ((b'#[derive(Clone, Copy)]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]', 2),
                                                     (b'#[derive(Default)]', 1),
                                                     (b'#[cfg(test)]', 3)),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([], []),
                                      'macros': ()},
 'src/frontend/oir/owned/builtins.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                       (b'#[cfg(test)]', 1),
                                                       (b'#[test]', 2)),
                                        'build_script': None,
                                        'c_preprocessor': (),
                                        'legacy': ([], []),
                                        'macros': ()},
 'src/frontend/oir/owned/byte_buffer.rs': {'attributes': ((b'#[path = "file_resources.rs"]', 1),
                                                          (b'#[path = "vector_resources.rs"]', 1),
                                                          (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                          (b'#[repr(C)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                          (b'#[repr(i32)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                          (b'#[repr(C)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)'
                                                           b']',
                                                           1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                          (b'#[cfg(target_pointer_width = "64")]', 1),
                                                          (b'#[cfg(all(not(test), target_pointer_width = "64"))]',
                                                           1),
                                                          (b'#[cfg(test)]', 2),
                                                          (b'#[cfg(not(test))]', 1),
                                                          (b'#[cfg(test)]', 2),
                                                          (b'#[derive(Clone, Copy, Default)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                          (b'#[cfg(test)]', 1)),
                                           'build_script': None,
                                           'c_preprocessor': (),
                                           'legacy': ([],
                                                      [b'#[path = "file_resources.rs"]',
                                                       b'#[path = "vector_resources.rs"]']),
                                           'macros': ()},
 'src/frontend/oir/owned/cfg.rs': {'attributes': (),
                                   'build_script': None,
                                   'c_preprocessor': (),
                                   'legacy': ([], []),
                                   'macros': ()},
 'src/frontend/oir/owned/execute.rs': {'attributes': ((b'#[path = "execute_floats.rs"]', 1),
                                                      (b'#[path = "execute_values.rs"]', 1),
                                                      (b'#[path = "execute_bitwise.rs"]', 1),
                                                      (b'#[path = "execute_integer128.rs"]', 1),
                                                      (b'#[path = "execute_integers.rs"]', 1),
                                                      (b'#[path = "execute_experimental.rs"]', 1),
                                                      (b'#[path = "execute_vector.rs"]', 1),
                                                      (b'#[path = "execute_composition.rs"]', 1),
                                                      (b'#[path = "execute_resources.rs"]', 1),
                                                      (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                      (b'#[derive(Clone, Copy)]', 2),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                                      (b'#[derive(Clone, Copy)]', 2),
                                                      (b'#[cfg(test)]', 65),
                                                      (b'#[allow(clippy::too_many_arguments)]', 1),
                                                      (b'#[cfg(test)]', 12),
                                                      (b'#[path = "array_observe.rs"]', 1),
                                                      (b'#[cfg(test)]', 4),
                                                      (b'#[path = "execute_tests.rs"]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[path = "builtin_output_reference_tests.rs"]', 1),
                                                      (b'#[cfg(test)]', 2),
                                                      (b'#[test]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[test]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[path = "byte_storage_codec_tests.rs"]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[test]', 4)),
                                       'build_script': None,
                                       'c_preprocessor': (),
                                       'legacy': ([],
                                                  [b'#[path = "execute_floats.rs"]',
                                                   b'#[path = "execute_values.rs"]',
                                                   b'#[path = "execute_bitwise.rs"]',
                                                   b'#[path = "execute_integer128.rs"]',
                                                   b'#[path = "execute_integers.rs"]',
                                                   b'#[path = "execute_experimental.rs"]',
                                                   b'#[path = "execute_vector.rs"]',
                                                   b'#[path = "execute_composition.rs"]',
                                                   b'#[path = "execute_resources.rs"]',
                                                   b'#[path = "array_observe.rs"]',
                                                   b'#[path = "execute_tests.rs"]',
                                                   b'#[path = "builtin_output_reference_tests.rs"]',
                                                   b'#[path = "byte_storage_codec_tests.rs"]']),
                                       'macros': ()},
 'src/frontend/oir/owned/execute_composition.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 2)),
                                                   'build_script': None,
                                                   'c_preprocessor': (),
                                                   'legacy': ([], []),
                                                   'macros': ()},
 'src/frontend/oir/owned/execute_experimental.rs': {'attributes': ((b'#[path = "execute_io_delivery.rs"]', 1),
                                                                   (b'#[path = "execute_file_delivery.rs"]', 1),
                                                                   (b'#[derive(Clone, Copy)]', 1),
                                                                   (b'#[cfg(test)]', 3)),
                                                    'build_script': None,
                                                    'c_preprocessor': (),
                                                    'legacy': ([],
                                                               [b'#[path = "execute_io_delivery.rs"]',
                                                                b'#[path = "execute_file_delivery.rs"]']),
                                                    'macros': ()},
 'src/frontend/oir/owned/execute_resources.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                                (b'#[cfg(test)]', 1),
                                                                (b'#[path = "execute_resource_tests.rs"]', 1),
                                                                (b'#[cfg(test)]', 1),
                                                                (b'#[path = "execute_vector_resource_tests.rs"]',
                                                                 1)),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([],
                                                            [b'#[path = "execute_resource_tests.rs"]',
                                                             b'#[path = "execute_vector_resource_tests.rs"]']),
                                                 'macros': ()},
 'src/frontend/oir/owned/execute_tests.rs': {'attributes': ((b'#[test]', 2),
                                                            (b'#[path = "source/runtime_resource_tests.rs"]', 1),
                                                            (b'#[test]', 17),
                                                            (b'#[path = "source/reviewer_resource_runtime.rs"]',
                                                             1),
                                                            (b'#[test]', 3),
                                                            (b'#[derive(Copy)]', 1),
                                                            (b'#[test]', 1),
                                                            (b'#[derive(Copy)]', 1),
                                                            (b'#[test]', 1),
                                                            (b'#[derive(Copy)]', 1),
                                                            (b'#[test]', 1),
                                                            (b'#[derive(Copy)]', 1)),
                                             'build_script': None,
                                             'c_preprocessor': (),
                                             'legacy': ([b'include!("reviewer_reference_tests.rs")'],
                                                        [b'#[path = "source/runtime_resource_tests.rs"]',
                                                         b'#[path = "source/reviewer_resource_runtime.rs"]']),
                                             'macros': ((b'include!("reviewer_reference_tests.rs")', 1),)},
 'src/frontend/oir/owned/execute_vector.rs': {'attributes': ((b'#[derive(Clone, Copy)]', 1),
                                                             (b'#[allow(dead_code)]', 3),
                                                             (b'#[cfg(test)]', 6),
                                                             (b'#[allow(clippy::too_many_arguments)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(clippy::too_many_arguments)]', 1),
                                                             (b'#[cfg(test)]', 5),
                                                             (b'#[path = "execute_vector_tests.rs"]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([], [b'#[path = "execute_vector_tests.rs"]']),
                                              'macros': ()},
 'src/frontend/oir/owned/flow.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 8),
                                                   (b'#[repr(u8)]', 1),
                                                   (b'#[derive(Clone, Copy)]', 1)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ()},
 'src/frontend/oir/owned/mod.rs': {'attributes': ((b'#![allow(dead_code)]', 1),
                                                  (b'#![allow(clippy::result_large_err)]', 1),
                                                  (b'#[cfg(test)]', 2),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug)]', 1),
                                                  (b'#[derive(Debug)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[cfg(test)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                  (b'#[derive(Clone, Debug)]', 2),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 1),
                                                  (b'#[derive(Clone, Copy, Debug)]', 1),
                                                  (b'#[derive(Clone, Debug)]', 3),
                                                  (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5),
                                                  (b'#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]', 1),
                                                  (b'#[cfg(test)]', 37),
                                                  (b'#[path = "record_vector_shape_tests.rs"]', 1)),
                                   'build_script': None,
                                   'c_preprocessor': (),
                                   'legacy': ([], [b'#[path = "record_vector_shape_tests.rs"]']),
                                   'macros': ()},
 'src/frontend/oir/owned/native.rs': {'attributes': ((b'#[path = "native_bitwise.rs"]', 1),
                                                     (b'#[path = "native_composition.rs"]', 1),
                                                     (b'#[path = "native_experimental.rs"]', 1),
                                                     (b'#[path = "native_floats.rs"]', 1),
                                                     (b'#[path = "native_integer128.rs"]', 1),
                                                     (b'#[path = "native_integers.rs"]', 1),
                                                     (b'#[path = "native_numeric.rs"]', 1),
                                                     (b'#[path = "native_reachability.rs"]', 1),
                                                     (b'#[path = "native_resources.rs"]', 1),
                                                     (b'#[path = "native_vector.rs"]', 1),
                                                     (b'#[allow(dead_code)]', 1),
                                                     (b'#[derive(Clone, Debug, Default)]', 1),
                                                     (b'#[derive(Default)]', 1),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy, Debug)]', 1),
                                                     (b'#[derive(Clone, Copy, Default, Debug)]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[derive(Clone, Copy, Debug)]', 1),
                                                     (b'#[cfg(test)]', 3),
                                                     (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[allow(clippy::too_many_arguments)]', 3),
                                                     (b'#[derive(Clone, Copy)]', 1),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, '
                                                      b'Ord)]',
                                                      1),
                                                     (b'#[repr(transparent)]', 1),
                                                     (b'#[allow(non_upper_case_globals)]', 1),
                                                     (b'#[derive(Clone, Copy)]', 1),
                                                     (b'#[allow(clippy::too_many_arguments)]', 2),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy)]', 2),
                                                     (b'#[allow(dead_code)]', 1),
                                                     (b'#[allow(clippy::too_many_arguments)]', 1),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[derive(Clone, Copy)]', 1),
                                                     (b'#[cfg(test)]', 3),
                                                     (b'#[path = "builtin_input_native_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "builtin_output_native_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "native_tests.rs"]', 1),
                                                     (b'#[cfg(all(test, any(target_os = "linux", target_os = "ma'
                                                      b'cos")))]',
                                                      1),
                                                     (b'#[path = "native_vector_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 2),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "native_inventory_admission_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "hosted_capacity_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 4),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 2),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 1),
                                                     (b'#[allow(dead_code)]', 2),
                                                     (b'#[test]', 1),
                                                     (b'#[allow(dead_code)]', 4),
                                                     (b'#[test]', 1),
                                                     (b'#[cfg(target_pointer_width = "64")]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "byte_storage_native_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[test]', 2),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "copy_record_native_tests.rs"]', 1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[path = "record_vector_native_tests.rs"]', 1)),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([],
                                                 [b'#[path = "native_bitwise.rs"]',
                                                  b'#[path = "native_composition.rs"]',
                                                  b'#[path = "native_experimental.rs"]',
                                                  b'#[path = "native_floats.rs"]',
                                                  b'#[path = "native_integer128.rs"]',
                                                  b'#[path = "native_integers.rs"]',
                                                  b'#[path = "native_numeric.rs"]',
                                                  b'#[path = "native_reachability.rs"]',
                                                  b'#[path = "native_resources.rs"]',
                                                  b'#[path = "native_vector.rs"]',
                                                  b'#[path = "builtin_input_native_tests.rs"]',
                                                  b'#[path = "builtin_output_native_tests.rs"]',
                                                  b'#[path = "native_tests.rs"]',
                                                  b'#[path = "native_vector_tests.rs"]',
                                                  b'#[path = "native_inventory_admission_tests.rs"]',
                                                  b'#[path = "hosted_capacity_tests.rs"]',
                                                  b'#[path = "byte_storage_native_tests.rs"]',
                                                  b'#[path = "copy_record_native_tests.rs"]',
                                                  b'#[path = "record_vector_native_tests.rs"]']),
                                      'macros': ((b'cfg!(all(target_os="macos",target_arch="aarch64"))', 1),
                                                 (b'cfg!(all(target_os="macos",target_arch="x86_64"))', 1),
                                                 (b'cfg!(all(target_os="linux",target_arch="aarch64"))', 1))},
 'src/frontend/oir/owned/native_composition.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                                 (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                                                 (b'#[allow(clippy::too_many_arguments)]', 1),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[path = "native_vector_composition_tests.rs'
                                                                  b'"]',
                                                                  1),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[test]', 2),
                                                                 (b'#[cfg(all(\n        target_os = "linux",\n    '
                                                                  b'    target_pointer_width = "64",\n        tar'
                                                                  b'get_endian = "little",\n        any(target_ar'
                                                                  b'ch = "x86_64", target_arch = "aarch64")\n    '
                                                                  b'))]',
                                                                  1),
                                                                 (b'#[test]', 1),
                                                                 (b'#[ignore = "requires Linux x86_64 or ARM64 a'
                                                                  b'nd LLVM 19.1.7"]',
                                                                  1)),
                                                  'build_script': None,
                                                  'c_preprocessor': ((b'#include <stdint.h>\n', 1),
                                                                     (b'#include <string.h>\n', 1),
                                                                     (b'#define CHECK(condition) do { if (!(cond'
                                                                      b'ition)) return __LINE__; } while (0)\n',
                                                                      1)),
                                                  'legacy': ([],
                                                             [b'#[path = "native_vector_composition_tests.rs"]']),
                                                  'macros': ()},
 'src/frontend/oir/owned/native_experimental.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(dead_code)]', 1),
                                                                  (b'#[repr(C)]', 1),
                                                                  (b'#[allow(clippy::too_many_arguments)]', 4)),
                                                   'build_script': None,
                                                   'c_preprocessor': (),
                                                   'legacy': ([], []),
                                                   'macros': ()},
 'src/frontend/oir/owned/native_resources.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 1),
                                                               (b'#[allow(clippy::too_many_arguments)]', 1)),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/oir/owned/native_storage.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                                             (b'#[path = "native_inventory_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "native_storage_tests.rs"]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([],
                                                         [b'#[path = "native_inventory_tests.rs"]',
                                                          b'#[path = "native_storage_tests.rs"]']),
                                              'macros': ()},
 'src/frontend/oir/owned/native_vector.rs': {'attributes': ((b'#[allow(dead_code)]', 3),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C, align(16))]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(clippy::too_many_arguments)]', 3),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[repr(C)]', 1),
                                                            (b'#[allow(dead_code)]', 2)),
                                             'build_script': None,
                                             'c_preprocessor': (),
                                             'legacy': ([], []),
                                             'macros': ()},
 'src/frontend/oir/owned/native_vector_layout.rs': {'attributes': ((b'#[repr(C)]', 1),),
                                                    'build_script': None,
                                                    'c_preprocessor': (),
                                                    'legacy': ([], []),
                                                    'macros': ()},
 'src/frontend/oir/owned/plan.rs': {'attributes': ((b'#[path = "native_storage.rs"]', 1),
                                                   (b'#[path = "native_vector_layout.rs"]', 1),
                                                   (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                   (b'#[cfg(test)]', 1),
                                                   (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1),
                                                   (b'#[derive(Debug)]', 3),
                                                   (b'#[cfg(test)]', 5),
                                                   (b'#[test]', 1),
                                                   (b'#[cfg(target_pointer_width = "64")]', 1),
                                                   (b'#[test]', 1)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([],
                                               [b'#[path = "native_storage.rs"]',
                                                b'#[path = "native_vector_layout.rs"]']),
                                    'macros': ()},
 'src/frontend/oir/owned/record_vector_native_tests.rs': {'attributes': ((b'#[derive(Copy)]', 1),
                                                                         (b'#[test]', 2),
                                                                         (b'#[cfg(target_pointer_width = "64")]',
                                                                          1),
                                                                         (b'#[test]', 1),
                                                                         (b'#[derive(Copy)]', 1),
                                                                         (b'#[test]', 1),
                                                                         (b'#[derive(Copy)]', 1)),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/oir/owned/record_vector_resources_tests.rs': {'attributes': ((b'#[test]', 8),
                                                                            (b'#[cfg(target_pointer_width = "64'
                                                                             b'")]',
                                                                             1)),
                                                             'build_script': None,
                                                             'c_preprocessor': (),
                                                             'legacy': ([], []),
                                                             'macros': ()},
 'src/frontend/oir/owned/record_vector_shape_tests.rs': {'attributes': ((b'#[derive(Copy)]', 3), (b'#[test]', 2)),
                                                         'build_script': None,
                                                         'c_preprocessor': (),
                                                         'legacy': ([], []),
                                                         'macros': ()},
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': {'attributes': ((b'#[test]', 14),),
                                                              'build_script': None,
                                                              'c_preprocessor': (),
                                                              'legacy': ([], []),
                                                              'macros': ()},
 'src/frontend/oir/owned/shape.rs': {'attributes': ((b'#[derive(Debug, Clone, Copy, PartialEq, Eq)]', 1),
                                                    (b'#[derive(Clone, Default)]', 2),
                                                    (b'#[allow(clippy::too_many_arguments)]', 1),
                                                    (b'#[derive(Clone, Copy)]', 1)),
                                     'build_script': None,
                                     'c_preprocessor': (),
                                     'legacy': ([], []),
                                     'macros': ()},
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 1)),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/oir/owned/source/association.rs': {'attributes': ((b'#[cfg(test)]', 3),
                                                                 (b'#[derive(Clone, Copy)]', 1),
                                                                 (b'#[cfg(test)]', 2),
                                                                 (b'#[allow(dead_code)]', 2),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[path = "output_lower_tests.rs"]', 1),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[cfg(not(test))]', 1),
                                                                 (b'#[cfg(test)]', 2),
                                                                 (b'#[test]', 3),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[allow(dead_code)]', 1),
                                                                 (b'#[test]', 1),
                                                                 (b'#[cfg(target_pointer_width = "64")]', 1),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[test]', 2),
                                                                 (b'#[cfg(test)]', 1),
                                                                 (b'#[test]', 3),
                                                                 (b'#[cfg(test)]', 2),
                                                                 (b'#[path = "byte_storage_association_tests.rs"]',
                                                                  1)),
                                                  'build_script': None,
                                                  'c_preprocessor': (),
                                                  'legacy': ([],
                                                             [b'#[path = "output_lower_tests.rs"]',
                                                              b'#[path = "byte_storage_association_tests.rs"]']),
                                                  'macros': ()},
 'src/frontend/oir/owned/source/budget.rs': {'attributes': ((b'#[derive(Clone, Copy)]', 1),
                                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                            (b'#[cfg(test)]', 6),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(target_pointer_width = "64")]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[cfg(not(test))]', 1),
                                                            (b'#[cfg(test)]', 7),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 3),
                                                            (b'#[derive(Clone, Copy)]', 1),
                                                            (b'#[cfg(test)]', 4),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 2)),
                                             'build_script': None,
                                             'c_preprocessor': (),
                                             'legacy': ([], []),
                                             'macros': ()},
 'src/frontend/oir/owned/source/candidate_adapter.rs': {'attributes': ((b'#[derive(Default)]', 1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[ignore = "source receipt collector; re'
                                                                        b'quires explicit source and fresh output '
                                                                        b'directories"]',
                                                                        1),
                                                                       (b'#[test]', 4),
                                                                       (b'#[cfg(test)]', 1),
                                                                       (b'#[path = "candidate_mutations.rs"]', 1),
                                                                       (b'#[cfg(test)]', 1),
                                                                       (b'#[path = "candidate_native.rs"]', 1)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([],
                                                                   [b'#[path = "candidate_mutations.rs"]',
                                                                    b'#[path = "candidate_native.rs"]']),
                                                        'macros': ()},
 'src/frontend/oir/owned/source/candidate_native.rs': {'attributes': ((b'#[cfg(all(\n    target_os = "linux",\n'
                                                                       b'    target_pointer_width = "64",\n    tar'
                                                                       b'get_endian = "little",\n    any(target_ar'
                                                                       b'ch = "x86_64", target_arch = "aarch64")\n'
                                                                       b'))]',
                                                                       1),
                                                                      (b'#[test]', 1),
                                                                      (b'#[ignore = "bounded native source collec'
                                                                       b'tor; requires frozen original source, bu'
                                                                       b'dget controls, pinned LLVM and a fresh o'
                                                                       b'utput path"]',
                                                                       1),
                                                                      (b'#[test]', 1)),
                                                       'build_script': None,
                                                       'c_preprocessor': (),
                                                       'legacy': ([],
                                                                  [b'#[ignore = "bounded native source collector;'
                                                                   b' requires frozen original source, budget con'
                                                                   b'trols, pinned LLVM and a fresh output path"]']),
                                                       'macros': ()},
 'src/frontend/oir/owned/source/copy_record_tests.rs': {'attributes': ((b'#[derive(Copy)]', 2),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 3),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 11),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 6),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 1),
                                                                       (b'#[test]', 1),
                                                                       (b'#[derive(Copy)]', 2),
                                                                       (b'#[test]', 1)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/oir/owned/source/diagnostic.rs': {'attributes': (),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([], []),
                                                 'macros': ()},
 'src/frontend/oir/owned/source/lower.rs': {'attributes': ((b'#[derive(Clone, Copy)]', 8),
                                                           (b'#[cfg(test)]', 16),
                                                           (b'#[cfg(not(test))]', 1),
                                                           (b'#[cfg(test)]', 5),
                                                           (b'#[test]', 1),
                                                           (b'#[allow(dead_code)]', 2),
                                                           (b'#[test]', 1)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/frontend/oir/owned/source/record_enum_layouts.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 3)),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/oir/owned/source/resolve.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                             (b'#[cfg(test)]', 7),
                                                             (b'#[cfg(not(test))]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[allow(clippy::large_enum_variant)]', 1),
                                                             (b'#[derive(Debug)]', 2),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(clippy::blocks_in_conditions)]', 1),
                                                             (b'#[cfg(test)]', 4),
                                                             (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 11),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[test]', 2),
                                                             (b'#[cfg(test)]', 4),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[test]', 2),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "resolver_paid_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "resolver_enum_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "enum_storage_failure_tests.rs"]', 1),
                                                             (b'#[allow(dead_code)]', 2),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 4),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "builtin_signature_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[path = "output_typing_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "byte_storage_type_tests.rs"]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([],
                                                         [b'#[path = "resolver_paid_tests.rs"]',
                                                          b'#[path = "resolver_enum_tests.rs"]',
                                                          b'#[path = "enum_storage_failure_tests.rs"]',
                                                          b'#[path = "builtin_signature_tests.rs"]',
                                                          b'#[path = "output_typing_tests.rs"]',
                                                          b'#[path = "byte_storage_type_tests.rs"]']),
                                              'macros': ()},
 'src/frontend/oir/owned/source/typeck.rs': {'attributes': ((b'#[derive(Debug)]', 3),
                                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(clippy::large_enum_variant)]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(dead_code)]', 19),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[derive(Debug)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[allow(dead_code)]', 2),
                                                            (b'#[cfg(test)]', 2),
                                                            (b'#[allow(clippy::blocks_in_conditions)]', 1),
                                                            (b'#[allow(dead_code)]', 3),
                                                            (b'#[cfg(test)]', 2),
                                                            (b'#[allow(dead_code)]', 4),
                                                            (b'#[allow(clippy::type_complexity)]', 1),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[cfg(test)]', 4),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 2),
                                                            (b'#[allow(clippy::too_many_arguments)]', 2),
                                                            (b'#[allow(clippy::unused_enumerate_index)]', 1),
                                                            (b'#[cfg(test)]', 3),
                                                            (b'#[cfg(not(test))]', 1),
                                                            (b'#[allow(clippy::too_many_arguments)]', 1),
                                                            (b'#[cfg(test)]', 7),
                                                            (b'#[test]', 2),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 2),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 4),
                                                            (b'#[allow(dead_code)]', 1),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 5),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[path = "enum_type_tests.rs"]', 1),
                                                            (b'#[allow(dead_code)]', 2),
                                                            (b'#[test]', 1),
                                                            (b'#[cfg(test)]', 1),
                                                            (b'#[test]', 1)),
                                             'build_script': None,
                                             'c_preprocessor': (),
                                             'legacy': ([], [b'#[path = "enum_type_tests.rs"]']),
                                             'macros': ()},
 'src/frontend/oir/owned/source/vector_lower.rs': {'attributes': ((b'#[allow(dead_code)]', 1),),
                                                   'build_script': None,
                                                   'c_preprocessor': (),
                                                   'legacy': ([], []),
                                                   'macros': ()},
 'src/frontend/oir/owned/source/vector_source_tests.rs': {'attributes': ((b'#[test]', 15),),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/oir/owned/vector_descriptors.rs': {'attributes': ((b'#[allow(dead_code)]', 1),),
                                                  'build_script': None,
                                                  'c_preprocessor': (),
                                                  'legacy': ([], []),
                                                  'macros': ()},
 'src/frontend/oir/owned/vector_raw_tests.rs': {'attributes': ((b'#[test]', 10),),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/oir/owned/vector_resources.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                                1),
                                                               (b'#[repr(u32)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 2),
                                                               (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                               (b'#[repr(C)]', 1),
                                                               (b'#[repr(C, align(16))]', 1),
                                                               (b'#[cfg(test)]', 1),
                                                               (b'#[path = "vector_resources_tests.rs"]', 1),
                                                               (b'#[cfg(test)]', 1),
                                                               (b'#[path = "record_vector_resources_tests.rs"]',
                                                                1)),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([],
                                                           [b'#[path = "vector_resources_tests.rs"]',
                                                            b'#[path = "record_vector_resources_tests.rs"]']),
                                                'macros': ()},
 'src/frontend/oir/owned/vector_type.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[test]', 1)),
                                           'build_script': None,
                                           'c_preprocessor': (),
                                           'legacy': ([], []),
                                           'macros': ()},
 'src/frontend/oir/owned/verified.rs': {'attributes': ((b'#[derive(Debug)]', 2),
                                                       (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                                       (b'#[cfg(test)]', 1),
                                                       (b'#[test]', 3),
                                                       (b'#[cfg(test)]', 5),
                                                       (b'#[allow(dead_code)]', 1),
                                                       (b'#[test]', 2)),
                                        'build_script': None,
                                        'c_preprocessor': (),
                                        'legacy': ([], []),
                                        'macros': ()},
 'src/frontend/oir/owned_types.rs': {'attributes': ((b'#![allow(dead_code)]', 1),
                                                    (b'#[allow(unused_imports)]', 1),
                                                    (b'#[cfg(test)]', 1),
                                                    (b'#[allow(unused_imports)]', 2),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 5),
                                                    (b'#[repr(C, align(8))]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                    (b'#[repr(C, align(8))]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 8),
                                                    (b'#[derive(Debug)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1),
                                                    (b'#[derive(Debug)]', 2),
                                                    (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1),
                                                    (b'#[derive(Clone, Copy)]', 1),
                                                    (b'#[derive(Debug)]', 2),
                                                    (b'#[derive(Clone, Copy, Debug)]', 1),
                                                    (b'#[derive(Clone, Copy, Default)]', 1),
                                                    (b'#[derive(Default)]', 1),
                                                    (b'#[cfg(test)]', 1),
                                                    (b'#[test]', 2),
                                                    (b'#[allow(dead_code)]', 2),
                                                    (b'#[test]', 29),
                                                    (b'#[cfg(test)]', 6)),
                                     'build_script': None,
                                     'c_preprocessor': (),
                                     'legacy': ([], []),
                                     'macros': ()},
 'src/frontend/oir/owned_types/composition.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 6)),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([], []),
                                                 'macros': ()},
 'src/frontend/oir/owned_types/enums.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                          (b'#[derive(Debug)]', 1),
                                                          (b'#[cfg_attr(test, derive(Clone))]', 1),
                                                          (b'#[derive(Clone, Copy, Debug)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)'
                                                           b']',
                                                           1),
                                                          (b'#[derive(Debug)]', 2),
                                                          (b'#[derive(Clone, Copy, Debug)]', 1),
                                                          (b'#[derive(Debug)]', 2),
                                                          (b'#[allow(clippy::too_many_arguments)]', 1),
                                                          (b'#[derive(Clone, Copy)]', 1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[test]', 22)),
                                           'build_script': None,
                                           'c_preprocessor': (),
                                           'legacy': ([], []),
                                           'macros': ((b'cfg!(target_pointer_width = "64")', 1),)},
 'src/frontend/oir/owned_types/resources.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                              (b'#[allow(dead_code)]', 1),
                                                              (b'#[repr(u8)]', 1),
                                                              (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                              (b'#[derive(Clone, Copy)]', 1),
                                                              (b'#[cfg(test)]', 1),
                                                              (b'#[test]', 8)),
                                               'build_script': None,
                                               'c_preprocessor': (),
                                               'legacy': ([], []),
                                               'macros': ()},
 'src/frontend/vector_builtins.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, '
                                                     b'Ord)]',
                                                     1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                    (b'#[repr(u8)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, '
                                                     b'Ord)]',
                                                     1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                    (b'#[cfg(test)]', 1),
                                                    (b'#[test]', 4)),
                                     'build_script': None,
                                     'c_preprocessor': (),
                                     'legacy': ([], []),
                                     'macros': ()}}
OBSERVER_ADAPTER = 'scripts/record_vector_compatibility_observer.py'
OBSERVER_ADAPTER_PIN = (8158, '22ccfc071ce18a03c809f226d5bf260e3622741bf1c364d6fbc8c294b364e48d')
DISPATCH_ADAPTER = 'scripts/record_vector_dispatch_activation.py'
DISPATCH_ADAPTER_PIN = (3079, '9e2e9dc6fa4b3ccbd03822fc031805e7d7a96c634dff716a38e2ccf34087ca2e')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and all(not parent.is_symlink() for parent in path.parents),
            "symlink pinned R1 policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned R1 policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned R1 policy")
    module = types.ModuleType("_record_vector_retained_R1")
    module.__file__ = str(path)
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return module


library = load_predecessor(Path(__file__).resolve().parents[1])
for _name in (
    "BASELINE", "BASELINE_SOURCE_SHA", "RETAINED_RUNNER_SHA", "BINDING", "RETAINED",
    "sha", "encoded", "identity", "load", "git", "checkout_identity", "extract_baseline",
    "regular", "verify_current_observer", "verify_materialized", "source_directives",
    "authenticate_index_resource_adapter", "authenticate_parser_resource_adapter",
    "INDEX_RESOURCE_ADAPTER", "PARSER_RESOURCE_ADAPTER", "UNCHANGED_INPUTS",
    "run_byte_buffer_observer_controls", "run_file_observer_controls",
    "run_i64_observer_controls", "run_integer_width_observer_controls",
    "run_integer128_observer_controls", "run_float_observer_controls",
    "run_character_observer_controls", "run_vector_observer_controls",
):
    globals()[_name] = getattr(library, _name)
PREDECESSOR_ADDED = library.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | RECORD_VECTOR_ADDED
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES,
                        PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor policy: " + name)
        require(regular(repo, name) == before, "predecessor policy changed: " + name)


def authenticate_adapter(repo, name, expected):
    body = regular(repo, name)
    require((len(body), sha(body)) == expected, "changed pinned successor adapter: " + name)
    module = types.ModuleType("_record_vector_" + Path(name).stem)
    module.__file__ = str(repo / name)
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return module, body


def authenticate_observer_adapter(repo):
    return authenticate_adapter(repo, OBSERVER_ADAPTER, OBSERVER_ADAPTER_PIN)


def authenticate_dispatch_adapter(repo):
    return authenticate_adapter(repo, DISPATCH_ADAPTER, DISPATCH_ADAPTER_PIN)


def dispatch_proof(repo, parent, inputs):
    adapter, body = authenticate_dispatch_adapter(repo)
    require(set(UNCHANGED_INPUTS) == {adapter.PATH}, "unreviewed protected input membership")
    require(UNCHANGED_INPUTS[adapter.PATH] == adapter.PREDECESSOR_PIN,
            "protected dispatch predecessor pin drift")
    proof = adapter.authenticate(parent[adapter.PATH], inputs[adapter.PATH],
                                 PROTECTED_CURRENT_PIN, WINDOWS_ACTIVATION_ENABLED)
    before = source_directives(adapter.PATH, parent[adapter.PATH])
    current = source_directives(adapter.PATH, inputs[adapter.PATH])
    restored = copy.deepcopy(current)
    restored["attributes"] = tuple(
        (adapter.BEFORE if value == adapter.AFTER and WINDOWS_ACTIVATION_ENABLED else value, count)
        for value, count in current["attributes"])
    require(restored == before, "protected dispatch compile-time directive drift")
    proof["compile_time_directive_delta_count"] = int(WINDOWS_ACTIVATION_ENABLED)
    # The inverse is never an input adapter. Exact current bytes remain inputs.
    return proof, body


def prepare_current_observer(original, output, captured, adapter, index_adapter, parser_adapter):
    # Reuse the actual complete R1 parser/index resource transformations.
    return library.prepare_current_observer(original, output, captured,
                                             adapter, index_adapter, parser_adapter)

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
    expected = expected_parent | RECORD_VECTOR_ADDED
    require(set(parent) == expected_parent, "wrong R1 compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (RECORD_VECTOR_CHANGED | RECORD_VECTOR_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == RECORD_VECTOR_CHANGED | RECORD_VECTOR_ADDED
            and RECORD_VECTOR_CHANGED <= expected_parent
            and not RECORD_VECTOR_ADDED & expected_parent,
            "wrong reviewed record-vector input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed record-vector directive membership")
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
    dispatch_proof(repo, parent, inputs)
    for name, before in parent.items():
        if name not in RECORD_VECTOR_CHANGED and name not in UNCHANGED_INPUTS:
            require(inputs[name] == before, "unreviewed change outside record-vector boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "record-vector source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "record-vector compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent, repo=None):
    repo = Path(__file__).resolve().parents[1] if repo is None else repo
    proof, dispatch_body = dispatch_proof(repo, parent, inputs)
    # Read old claim names without mutating or reusing its source admission.
    inherited = library.current_manifest(parent, baseline, checkout, parent)
    result = {key: value for key, value in inherited.items() if key.endswith("qualified_by_this_gate")}
    result.update({
        "schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
        "checkout": checkout, "assertion_scope": "retained bounded scalar/record semantics and resources",
        "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
        "source_policy_input_members": len(PREDECESSOR_POLICIES),
        "predecessor_compiler_input_members": len(parent),
        "record_vector_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: {"bytes": pin[0], "sha256": pin[1]}
                                      for name, pin in sorted(UNCHANGED_INPUTS.items())},
        "protected_dispatch_activation": proof,
        "protected_dispatch_activation_controller": identity(DISPATCH_ADAPTER, dispatch_body),
        "record_vector_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "record_vector_changes": [{"path": name, "before": identity(name, parent[name]),
                                    "after": identity(name, inputs[name])}
                                   for name in sorted(RECORD_VECTOR_CHANGED)],
        "changes": [{"path": name, "before": identity(name, baseline[name]),
                      "after": identity(name, body)} for name, body in sorted(inputs.items())
                    if name in baseline and body != baseline[name]],
        "additions": sorted(PARENT_ADDED),
        "unchanged_baseline_inputs": sum(inputs[name] == body for name, body in baseline.items()),
        "files": [identity(name, body) for name, body in sorted(inputs.items())],
        "copy_record_vectors_semantics_qualified_by_this_gate": False,
        "windows_module_sources_semantics_qualified_by_this_gate": False,
        "windows_activation_qualified_by_this_gate": False,
    })
    return result


def run_record_vector_observer_controls(repo, output, captured, seam, original, adapter, previous_executions):
    """Append six controls per profile without changing any old replay selector."""
    run = output / "unit2/run"
    controls = output / "record-vector-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "changed record-vector control protocol")
    protocol = types.ModuleType("_record_vector_control_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    require(type(previous_executions) is int and previous_executions > 0,
            "missing predecessor execution census")
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary = Path(receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "record-vector control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "record-vector control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.RECORD_VECTOR_CONTROL_FILTER]),
                              ("run", [adapter.RECORD_VECTOR_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            with out.open("xb") as stdout, err.open("xb") as stderr:
                completed = subprocess.run(argv, cwd=run / "source", stdout=stdout, stderr=stderr, timeout=60)
            commands.append({"argv": argv, "exit_status": completed.returncode,
                             "stdout": identity(out.name, out.read_bytes()),
                             "stderr": identity(err.name, err.read_bytes())})
            (controls / (profile + "-commands.json")).write_bytes(encoded(commands))
            require(completed.returncode == 0, "record-vector observer controls failed; see retained streams")
            streams[label] = out.read_text(encoding="utf-8")
        names = list(adapter.RECORD_VECTOR_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], names)
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "record-vector control binary changed")
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "record-vector observer source binding changed")
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        original.assert_unchanged(repo, captured)
        record = {"status": "passed", "profile": profile, "tests": names,
                  "nominal_ordinals_per_control": 3, "result": result,
                  "binary": identity(str(binary), binary.read_bytes()),
                  "source_inputs_sha256": original.CURRENT_SOURCE_SHA,
                  "commands": identity(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        path.write_bytes(encoded(record))
        receipts.append(identity(str(path.relative_to(output)), path.read_bytes()))
    count = len(adapter.RECORD_VECTOR_CONTROL_NAMES)
    return {"record_vector_observer_control_tests_per_profile": count,
            "record_vector_observer_denials_per_profile": count * 3,
            "record_vector_observer_control_receipts": receipts,
            "test_function_executions": previous_executions + 2 * count}

def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_copy_record_vector_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources; Copy record vectors and Windows module activation qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_copy_record_vector_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional record-vector pins permit prepare-only admission, not qualification")
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
        index_adapter, index_adapter_body = authenticate_index_resource_adapter(repo)
        parser_adapter, parser_adapter_body = authenticate_parser_resource_adapter(repo)
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
        selected, seam = prepare_current_observer(original, output, captured, observer_adapter, index_adapter, parser_adapter)
        report["record_vector_observer_adapter"] = observer_adapter.descriptor()
        report["record_vector_observer_adapter_controller"] = identity(OBSERVER_ADAPTER, observer_adapter_body)
        report["typed_vector_index_resource_controller"] = identity(INDEX_RESOURCE_ADAPTER, index_adapter_body)
        report["checked_copy_parser_resource_controller"] = identity(PARSER_RESOURCE_ADAPTER, parser_adapter_body)
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
            require(authenticate_index_resource_adapter(actual_repo)[1] == index_adapter_body,
                    "typed-vector index resource adapter changed during qualification")
            require(authenticate_parser_resource_adapter(actual_repo)[1] == parser_adapter_body,
                    "checked-Copy parser resource adapter changed during qualification")
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
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="1", PYTHONDONTWRITEBYTECODE="1")
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
            verified.update(run_vector_observer_controls(repo, output, selected, seam, original, observer_adapter, verified["test_function_executions"]))
            original.verify_unit2_result(output, selected, seam, False)
            verify_materialized(output, inputs)
            verify_current_observer(output, observer_adapter, selected["observer"])
        if not args.prepare_only:
            verified.update(run_record_vector_observer_controls(
                repo, output, selected, seam, original, observer_adapter, verified["test_function_executions"]))
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
