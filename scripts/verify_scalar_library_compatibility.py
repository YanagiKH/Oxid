#!/usr/bin/env python3
"""Bind scalar-library source inputs to the complete retained File seek replay.

Only inherited default-emission semantics and resource predicates are replayed.
Scalar native libraries and all new platform/debug claims stay false.
Provisional pins admit preparation only and cannot start qualification.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys
import types

PREDECESSOR = '104c900222ead9d9177025102a90520b08e761a4'
PREDECESSOR_POLICY = 'scripts/verify_file_seek_compatibility.py'
PREDECESSOR_POLICY_PIN = (66400, 'eec5daba34c6380781289a3bb01ed58e44e359a4b03fd23288f950be0ca99e38')
PINS_PROVISIONAL = False
SCALAR_LIBRARY_CHANGED = frozenset(['src/frontend/driver.rs',
 'src/frontend/mod.rs',
 'src/frontend/native.rs',
 'src/frontend/oir/mod.rs',
 'src/frontend/oir/native.rs',
 'src/frontend/oir/owned/byte_storage_native_tests.rs',
 'src/frontend/oir/source.rs',
 'src/frontend/oir/source/hir_import/emit_tests.rs',
 'src/frontend/oir/source/sealed.rs',
 'src/frontend/options.rs'])
SCALAR_LIBRARY_ADDED = frozenset(['src/frontend/native/library.rs',
 'src/frontend/native_library.rs',
 'src/frontend/oir/native_library.rs'])
REVIEWED_INPUTS = {'src/frontend/driver.rs': (100317, 'a726da52d510324f304cfda94c1f658e0fd234dc0a34b57132994dee7dcd55ac'),
 'src/frontend/mod.rs': (1116, '914f072af26eecd5eb62311466f8e72c40dff3249a6f9b5f2bfa1cb3a2909dad'),
 'src/frontend/native.rs': (26278, 'a2b3dec1185c5be9d9a935ab13bb90375f63b7a4f998558829440827d578baf9'),
 'src/frontend/native/library.rs': (14468,
                                    'a9e7cc740eb9b9671ce7ecb4fbcf8406207a7163e41741004d9155dd742f1cf0'),
 'src/frontend/native_library.rs': (15276,
                                    '5c56a6d3e882fbac1e1e7c9d9c4af9fc0ccd76b391df17e12593272db47d4dcd'),
 'src/frontend/oir/mod.rs': (24711, 'd9d4cd63dca6bc1f5e93e8fdaa57dbd1e4ff343e555c0c3cceafa2f1b7a7f997'),
 'src/frontend/oir/native.rs': (123231, 'a56c08285dc8902814a53eece47346a45cfefe4207b6e9c3e3be95246dc05920'),
 'src/frontend/oir/native_library.rs': (32712,
                                        '8c74b18c9a4aced609a71a137cbac01721b269c1effb2b30883a13f63a66b41b'),
 'src/frontend/oir/owned/byte_storage_native_tests.rs': (66663,
                                                         '629996219bf8ec06f66315611b62e22acdade01d2e8e293fec5896fcaec75977'),
 'src/frontend/oir/source.rs': (10048, '02d2a8684a9a4280aad99e418241fe62b63b7f1e897681ec91e15797e35d2686'),
 'src/frontend/oir/source/hir_import/emit_tests.rs': (22444,
                                                      '7beca0122e80b00821f711317e8853b7bb88ae1f60f4d0ce816bb07905e728eb'),
 'src/frontend/oir/source/sealed.rs': (33822,
                                       '0ab60e4e82b72abda6cb17ac4a5693349dd8df6f3a3a36b5b46e1d5d992099f2'),
 'src/frontend/options.rs': (66690, 'cb212bbfe0800590784954012ae785c5161a490535a264fdccc6db1e5a06e5a7')}
REVIEWED_DIRECTIVES = {'src/frontend/driver.rs': {'attributes': ((b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                           (b'#[derive(Debug)]', 2),
                                           (b'#[allow(clippy::large_enum_variant)]', 1),
                                           (b'#[derive(Debug)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[test]', 1),
                                           (b'#[cfg(any(target_os = "linux", target_os = "macos"))]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(all(\n        target_os = "linux",\n        target_point'
                                            b'er_width = "64",\n        target_endian = "little",\n        a'
                                            b'ny(target_arch = "x86_64", target_arch = "aarch64")\n    ))]',
                                            1),
                                           (b'#[test]', 2),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(all(\n        target_os = "linux",\n        target_point'
                                            b'er_width = "64",\n        target_endian = "little",\n        a'
                                            b'ny(target_arch = "x86_64", target_arch = "aarch64")\n    ))]',
                                            1),
                                           (b'#[test]', 1),
                                           (b'#[ignore = "requires LLVM 19.1.7 and the qualified native SD'
                                            b'K"]',
                                            1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 3),
                                           (b'#[cfg(all(\n        target_os = "linux",\n        target_point'
                                            b'er_width = "64",\n        target_endian = "little",\n        a'
                                            b'ny(target_arch = "x86_64", target_arch = "aarch64")\n    ))]',
                                            1),
                                           (b'#[test]', 1),
                                           (b'#[ignore = "requires pinned LLVM 19.1.7 tools"]', 1),
                                           (b'#[test]', 2)),
                            'build_script': None,
                            'c_preprocessor': (),
                            'legacy': ([], []),
                            'macros': ((b'cfg!(target_arch = "x86_64")', 1),
                                       (b'cfg!(all(\n            target_pointer_width = "64",\n            t'
                                        b'arget_endian = "little",\n            any(target_arch = "x86_64",'
                                        b' target_arch = "aarch64")\n        ))',
                                        1))},
 'src/frontend/mod.rs': {'attributes': ((b'#[cfg(test)]', 3),),
                         'build_script': None,
                         'c_preprocessor': (),
                         'legacy': ([], []),
                         'macros': ()},
 'src/frontend/native.rs': {'attributes': ((b'#[cfg(all(\n    any(target_os = "linux", target_os = "macos")'
                                            b',\n    target_pointer_width = "64",\n    target_endian = "litt'
                                            b'le",\n    any(target_arch = "x86_64", target_arch = "aarch64"'
                                            b')\n))]',
                                            1),
                                           (b'#[cfg(not(all(\n    any(target_os = "linux", target_os = "mac'
                                            b'os"),\n    target_pointer_width = "64",\n    target_endian = "'
                                            b'little",\n    any(target_arch = "x86_64", target_arch = "aarc'
                                            b'h64")\n)))]',
                                            1),
                                           (b'#[cfg(unix)]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[test]', 2)),
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
                                       (b'cfg!(windows)', 1),
                                       (b'cfg!(target_os = "macos")', 1),
                                       (b'cfg!(windows)', 1),
                                       (b'cfg!(target_os = "macos")', 4))},
 'src/frontend/native/library.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
                                                   (b'#[cfg(all(\n    target_os = "linux",\n    target_point'
                                                    b'er_width = "64",\n    target_endian = "little",\n    a'
                                                    b'ny(target_arch = "x86_64", target_arch = "aarch64")\n'
                                                    b'))]',
                                                    1),
                                                   (b'#[cfg(not(all(\n    target_os = "linux",\n    target_p'
                                                    b'ointer_width = "64",\n    target_endian = "little",\n '
                                                    b'   any(target_arch = "x86_64", target_arch = "aarch6'
                                                    b'4")\n)))]',
                                                    1)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ((b'env!("CARGO_PKG_VERSION")', 1),)},
 'src/frontend/native_library.rs': {'attributes': ((b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                   (b'#[allow(dead_code)]', 2),
                                                   (b'#[cfg(test)]', 1),
                                                   (b'#[test]', 1),
                                                   (b'#[cfg(target_pointer_width = "64")]', 1),
                                                   (b'#[test]', 1)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ((b'cfg!(all(\n        target_os = "linux",\n        target_po'
                                                b'inter_width = "64",\n        target_endian = "little",\n  '
                                                b'      any(target_arch = "x86_64", target_arch = "aarch64'
                                                b'")\n    ))',
                                                1),)},
 'src/frontend/oir/mod.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 4),
                                            (b'#[derive(Clone, Debug, PartialEq, Eq)]', 4),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                            (b'#[cfg(test)]', 2),
                                            (b'#[derive(Clone, Debug, PartialEq, Eq)]', 4),
                                            (b'#[derive(Debug)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 3),
                                            (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[derive(Default)]', 1),
                                            (b'#[cfg(test)]', 12)),
                             'build_script': None,
                             'c_preprocessor': (),
                             'legacy': ([], []),
                             'macros': ()},
 'src/frontend/oir/native.rs': {'attributes': ((b'#[path = "native_library.rs"]', 1),
                                               (b'#[allow(dead_code)]', 1),
                                               (b'#[path = "native_private_emit.rs"]', 1),
                                               (b'#[path = "native_scalar_resource.rs"]', 1),
                                               (b'#[allow(dead_code)]', 1),
                                               (b'#[path = "native_emit_work.rs"]', 1),
                                               (b'#[allow(dead_code)]', 1),
                                               (b'#[path = "native_emit_cost.rs"]', 1),
                                               (b'#[cfg(test)]', 1),
                                               (b'#[path = "native_private_emit_tests.rs"]', 1),
                                               (b'#[cfg(test)]', 1),
                                               (b'#[path = "native_u8_tests.rs"]', 1),
                                               (b'#[cfg(test)]', 1),
                                               (b'#[path = "native_emit_observation.rs"]', 1),
                                               (b'#[derive(Clone, Copy)]', 2),
                                               (b'#[derive(Clone, Copy, Default, Debug)]', 1),
                                               (b'#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]', 1),
                                               (b'#[default]', 1),
                                               (b'#[cfg(test)]', 2),
                                               (b'#[allow(clippy::too_many_arguments)]', 3),
                                               (b'#[cfg(test)]', 5),
                                               (b'#[allow(unreachable_patterns)]', 1),
                                               (b'#[cfg(test)]', 2),
                                               (b'#[derive(Default)]', 1),
                                               (b'#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]', 1),
                                               (b'#[cfg(test)]', 2),
                                               (b'#[allow(clippy::too_many_arguments)]', 1),
                                               (b'#[cfg(test)]', 9),
                                               (b'#[allow(clippy::too_many_arguments)]', 1),
                                               (b'#[cfg(test)]', 2),
                                               (b'#[allow(clippy::too_many_arguments)]', 1),
                                               (b'#[cfg(test)]', 7),
                                               (b'#[allow(unreachable_patterns)]', 1),
                                               (b'#[cfg(test)]', 1),
                                               (b'#[test]', 10),
                                               (b'#[cfg(all(target_arch = "x86_64", target_pointer_width ='
                                                b' "64"))]',
                                                1),
                                               (b'#[test]', 18),
                                               (b'#[cfg(test)]', 1),
                                               (b'#[path = "native_hosted_capacity_tests.rs"]', 1)),
                                'build_script': None,
                                'c_preprocessor': (),
                                'legacy': ([],
                                           [b'#[path = "native_library.rs"]',
                                            b'#[path = "native_private_emit.rs"]',
                                            b'#[path = "native_scalar_resource.rs"]',
                                            b'#[path = "native_emit_work.rs"]',
                                            b'#[path = "native_emit_cost.rs"]',
                                            b'#[path = "native_private_emit_tests.rs"]',
                                            b'#[path = "native_u8_tests.rs"]',
                                            b'#[path = "native_emit_observation.rs"]',
                                            b'#[path = "native_hosted_capacity_tests.rs"]']),
                                'macros': ()},
 'src/frontend/oir/native_library.rs': {'attributes': ((b'#[derive(Clone, Copy, Default)]', 1),
                                                       (b'#[allow(dead_code)]', 1),
                                                       (b'#[repr(C)]', 1),
                                                       (b'#[allow(dead_code)]', 2),
                                                       (b'#[cfg(test)]', 2),
                                                       (b'#[cfg(all(\n    target_os = "linux",\n    target_p'
                                                        b'ointer_width = "64",\n    target_endian = "little'
                                                        b'",\n    any(target_arch = "x86_64", target_arch ='
                                                        b' "aarch64")\n))]',
                                                        1),
                                                       (b'#[test]', 5)),
                                        'build_script': None,
                                        'c_preprocessor': (),
                                        'legacy': ([], []),
                                        'macros': ()},
 'src/frontend/oir/owned/byte_storage_native_tests.rs': {'attributes': ((b'#[cfg(all(\n    target_os = "linu'
                                                                         b'x",\n    target_pointer_width = "'
                                                                         b'64",\n    target_endian = "little'
                                                                         b'",\n    any(target_arch = "x86_64'
                                                                         b'", target_arch = "aarch64")\n))]',
                                                                         1),
                                                                        (b'#[cfg(all(\n        target_os = "'
                                                                         b'linux",\n        target_pointer_w'
                                                                         b'idth = "64",\n        target_endi'
                                                                         b'an = "little",\n        any(targe'
                                                                         b't_arch = "x86_64", target_arch ='
                                                                         b' "aarch64")\n    ))]',
                                                                         1),
                                                                        (b'#[test]', 5),
                                                                        (b'#[cfg(all(\n    target_os = "linu'
                                                                         b'x",\n    target_pointer_width = "'
                                                                         b'64",\n    target_endian = "little'
                                                                         b'",\n    any(target_arch = "x86_64'
                                                                         b'", target_arch = "aarch64")\n))]',
                                                                         2),
                                                                        (b'#[test]', 1),
                                                                        (b'#[ignore = "requires pinned LLVM'
                                                                         b' 19.1.7 and one serialized nativ'
                                                                         b'e execution slot"]',
                                                                         1),
                                                                        (b'#[cfg(all(\n    target_os = "linu'
                                                                         b'x",\n    target_pointer_width = "'
                                                                         b'64",\n    target_endian = "little'
                                                                         b'",\n    any(target_arch = "x86_64'
                                                                         b'", target_arch = "aarch64")\n))]',
                                                                         1),
                                                                        (b'#[test]', 1),
                                                                        (b'#[ignore = "requires pinned LLVM'
                                                                         b' 19.1.7 and one serialized nativ'
                                                                         b'e execution slot"]',
                                                                         1),
                                                                        (b'#[test]', 3),
                                                                        (b'#[cfg(all(\n    target_os = "linu'
                                                                         b'x",\n    target_pointer_width = "'
                                                                         b'64",\n    target_endian = "little'
                                                                         b'",\n    any(target_arch = "x86_64'
                                                                         b'", target_arch = "aarch64")\n))]',
                                                                         1),
                                                                        (b'#[test]', 1),
                                                                        (b'#[ignore = "requires pinned LLVM'
                                                                         b' 19.1.7 and one serialized nativ'
                                                                         b'e execution slot"]',
                                                                         1),
                                                                        (b'#[cfg(all(\n    target_os = "linu'
                                                                         b'x",\n    target_pointer_width = "'
                                                                         b'64",\n    target_endian = "little'
                                                                         b'",\n    any(target_arch = "x86_64'
                                                                         b'", target_arch = "aarch64")\n))]',
                                                                         1),
                                                                        (b'#[test]', 1),
                                                                        (b'#[ignore = "requires pinned LLVM'
                                                                         b' 19.1.7 and one serialized nativ'
                                                                         b'e execution slot"]',
                                                                         1),
                                                                        (b'#[derive(Clone, Copy, Debug, Par'
                                                                         b'tialEq, Eq)]',
                                                                         1),
                                                                        (b'#[test]', 1),
                                                                        (b'#[cfg(all(\n    target_os = "linu'
                                                                         b'x",\n    target_pointer_width = "'
                                                                         b'64",\n    target_endian = "little'
                                                                         b'",\n    any(target_arch = "x86_64'
                                                                         b'", target_arch = "aarch64")\n))]',
                                                                         1),
                                                                        (b'#[test]', 1),
                                                                        (b'#[ignore = "requires LLVM 19.1.7'
                                                                         b'; executes byte record corruptio'
                                                                         b'n controls at O0 and O2"]',
                                                                         1)),
                                                         'build_script': None,
                                                         'c_preprocessor': (),
                                                         'legacy': ([], []),
                                                         'macros': ((b'cfg!(all(\n        target_os = "windo'
                                                                     b'ws",\n        target_env = "msvc",\n  '
                                                                     b'      target_arch = "x86_64",\n      '
                                                                     b'  target_pointer_width = "64",\n     '
                                                                     b'   target_endian = "little"\n    ))',
                                                                     1),
                                                                    (b'cfg!(all(target_os = "macos", target'
                                                                     b'_arch = "aarch64"))',
                                                                     1),
                                                                    (b'cfg!(all(target_os = "macos", target'
                                                                     b'_arch = "x86_64"))',
                                                                     1),
                                                                    (b'cfg!(all(target_os = "linux", target'
                                                                     b'_arch = "aarch64"))',
                                                                     1))},
 'src/frontend/oir/source.rs': {'attributes': ((b'#[cfg(test)]', 2),
                                               (b'#[allow(dead_code)]', 1),
                                               (b'#[cfg(test)]', 1),
                                               (b'#[test]', 7)),
                                'build_script': None,
                                'c_preprocessor': (),
                                'legacy': ([], []),
                                'macros': ()},
 'src/frontend/oir/source/hir_import/emit_tests.rs': {'attributes': ((b'#[test]', 4),
                                                                     (b'#[cfg(any(target_os = "linux", targe'
                                                                      b't_os = "macos"))]',
                                                                      1),
                                                                     (b'#[test]', 1),
                                                                     (b'#[cfg(not(any(\n    target_os = "linu'
                                                                      b'x",\n    target_os = "macos",\n   '
                                                                      b' all(\n        target_os = "windows",'
                                                                      b'\n        target_arch = "x86_64",'
                                                                      b'\n        target_pointer_width = "64"'
                                                                      b',\n        target_endian = "littl'
                                                                      b'e"\n    )\n)))]',
                                                                      1),
                                                                     (b'#[test]', 2),
                                                                     (b'#[path = "emit_native_tests.rs"]',
                                                                      1)),
                                                      'build_script': None,
                                                      'c_preprocessor': (),
                                                      'legacy': ([b'include_str!(concat!(\n    env!("CARGO_MA'
                                                                  b'NIFEST_DIR")',
                                                                  b'include_bytes!(concat!(\n    env!("CARGO_'
                                                                  b'MANIFEST_DIR")',
                                                                  b'include_str!(concat!(\n                en'
                                                                  b'v!("CARGO_MANIFEST_DIR")',
                                                                  b'include_bytes!(concat!(\n                '
                                                                  b'env!("CARGO_MANIFEST_DIR")',
                                                                  b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR")',
                                                                  b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR")',
                                                                  b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR")',
                                                                  b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR")'],
                                                                 [b'#[path = "emit_native_tests.rs"]']),
                                                      'macros': ((b'cfg!(all(\n    target_os = "windows",'
                                                                  b'\n    target_env = "msvc",\n    target'
                                                                  b'_arch = "x86_64",\n    target_pointer_wid'
                                                                  b'th = "64",\n    target_endian = "litt'
                                                                  b'le"\n))',
                                                                  1),
                                                                 (b'cfg!(all(target_os = "macos", target_arc'
                                                                  b'h = "aarch64"))',
                                                                  1),
                                                                 (b'cfg!(all(target_os = "macos", target_arc'
                                                                  b'h = "x86_64"))',
                                                                  1),
                                                                 (b'cfg!(all(target_os = "linux", target_arc'
                                                                  b'h = "aarch64"))',
                                                                  1),
                                                                 (b'include_str!(concat!(\n    env!("CARGO_MA'
                                                                  b'NIFEST_DIR"),\n    "/tests/fixtures/check'
                                                                  b'ed_hir_import/rich-source.txt"\n))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1),
                                                                 (b'include_bytes!(concat!(\n    env!("CARGO_'
                                                                  b'MANIFEST_DIR"),\n    "/tests/fixtures/che'
                                                                  b'cked_hir_import/rich-success.bin"\n))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1),
                                                                 (b'include_str!(concat!(\n                en'
                                                                  b'v!("CARGO_MANIFEST_DIR"),\n              '
                                                                  b'  "/tests/fixtures/checked_hir_import/",'
                                                                  b'\n                $name,\n            '
                                                                  b'    "-source.txt"\n            ))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1),
                                                                 (b'include_bytes!(concat!(\n                '
                                                                  b'env!("CARGO_MANIFEST_DIR"),\n            '
                                                                  b'    "/tests/fixtures/checked_hir_import/'
                                                                  b'",\n                $name,\n          '
                                                                  b'      "-success.bin"\n            ))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1),
                                                                 (b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR"),\n            "/te'
                                                                  b'sts/fixtures/checked_hir_import/syntheti'
                                                                  b'c-overflow-source.txt"\n        ))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1),
                                                                 (b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR"),\n            "/te'
                                                                  b'sts/fixtures/checked_hir_import/syntheti'
                                                                  b'c-overflow-success.bin"\n        ))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1),
                                                                 (b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR"),\n            "/te'
                                                                  b'sts/fixtures/checked_hir_import/syntheti'
                                                                  b'c-division-source.txt"\n        ))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1),
                                                                 (b'include_bytes!(concat!(\n            env!'
                                                                  b'("CARGO_MANIFEST_DIR"),\n            "/te'
                                                                  b'sts/fixtures/checked_hir_import/syntheti'
                                                                  b'c-division-success.bin"\n        ))',
                                                                  1),
                                                                 (b'env!("CARGO_MANIFEST_DIR")', 1))},
 'src/frontend/oir/source/sealed.rs': {'attributes': ((b'#[allow(clippy::large_enum_variant)]', 1),
                                                      (b'#[derive(Debug)]', 3),
                                                      (b'#[allow(dead_code)]', 1),
                                                      (b'#[derive(Clone, Copy)]', 1),
                                                      (b'#[allow(clippy::large_enum_variant)]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[derive(Debug)]', 1),
                                                      (b'#[cfg(test)]', 2),
                                                      (b'#[allow(dead_code)]', 2),
                                                      (b'#[cfg(test)]', 2),
                                                      (b'#[test]', 1),
                                                      (b'#[cfg(target_pointer_width = "64")]', 2),
                                                      (b'#[allow(dead_code)]', 1),
                                                      (b'#[test]', 2)),
                                       'build_script': None,
                                       'c_preprocessor': (),
                                       'legacy': ([], []),
                                       'macros': ()},
 'src/frontend/options.rs': {'attributes': ((b'#[derive(Debug, PartialEq, Eq)]', 4),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[allow(dead_code)]', 1),
                                            (b'#[test]', 1),
                                            (b'#[cfg(target_pointer_width = "64")]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[test]', 16),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[test]', 2),
                                            (b'#[cfg(target_pointer_width = "64")]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[test]', 6),
                                            (b'#[cfg(target_pointer_width = "64")]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[test]', 3),
                                            (b'#[cfg(target_pointer_width = "64")]', 1)),
                             'build_script': None,
                             'c_preprocessor': (),
                             'legacy': ([], []),
                             'macros': ((b'cfg!(all(target_os = "linux", target_arch = "aarch64"))', 1),)}}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and all(not parent.is_symlink() for parent in path.parents),
            "symlink pinned File seek policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned File seek policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned File seek policy")
    module = types.ModuleType("_scalar_library_retained_file_seek")
    module.__file__ = str(path)
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return module


library = load_predecessor(Path(__file__).resolve().parents[1])
for _name in (
    "BASELINE", "BASELINE_SOURCE_SHA", "RETAINED_RUNNER_SHA", "BINDING", "RETAINED",
    "sha", "encoded", "identity", "load", "git", "checkout_identity", "extract_baseline",
    "regular", "verify_current_observer", "verify_materialized", "source_directives",
    "authenticate_observer_adapter", "authenticate_index_resource_adapter",
    "authenticate_parser_resource_adapter", "OBSERVER_ADAPTER", "INDEX_RESOURCE_ADAPTER",
    "PARSER_RESOURCE_ADAPTER", "UNCHANGED_INPUTS",
    "run_byte_buffer_observer_controls", "run_file_observer_controls",
    "run_i64_observer_controls", "run_integer_width_observer_controls",
    "run_integer128_observer_controls", "run_float_observer_controls",
    "run_character_observer_controls", "run_vector_observer_controls",
    "run_record_vector_observer_controls",
):
    globals()[_name] = getattr(library, _name)
PREDECESSOR_ADDED = library.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | SCALAR_LIBRARY_ADDED
PREDECESSOR_POLICIES = {
    **library.PREDECESSOR_POLICIES,
    PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1],
}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor policy: " + name)
        require(regular(repo, name) == before, "predecessor policy changed: " + name)


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
    expected = expected_parent | SCALAR_LIBRARY_ADDED
    require(set(parent) == expected_parent and len(parent) == 519,
            "wrong File seek compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (SCALAR_LIBRARY_CHANGED | SCALAR_LIBRARY_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == SCALAR_LIBRARY_CHANGED | SCALAR_LIBRARY_ADDED
            and SCALAR_LIBRARY_CHANGED <= expected_parent
            and not SCALAR_LIBRARY_ADDED & expected_parent,
            "wrong reviewed scalar-library input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed scalar-library directive membership")
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
        if name not in SCALAR_LIBRARY_CHANGED:
            require(inputs[name] == before,
                    "unreviewed change outside scalar-library boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "scalar-library source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "scalar-library compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    # Inherit only false claim names; no prior source admission, resource
    # formula or historical result is reused as current qualification.
    inherited = library.current_manifest(parent, baseline, checkout, parent)
    result = {key: value for key, value in inherited.items()
              if key.endswith("qualified_by_this_gate")}
    result.update({
        "schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
        "checkout": checkout, "assertion_scope": "retained bounded scalar/record semantics and resources",
        "debug_information_enabled_for_retained_claim": False,
        "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
        "source_policy_input_members": len(PREDECESSOR_POLICIES),
        "predecessor_compiler_input_members": len(parent),
        "scalar_library_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: identity(name, parent[name])
                                      for name in sorted(UNCHANGED_INPUTS)},
        "scalar_library_reviewed_inputs": [
            {"path": name, "bytes": length, "sha256": digest}
            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "scalar_library_changes": [
            {"path": name, "before": identity(name, parent[name]),
             "after": identity(name, inputs[name])}
            for name in sorted(SCALAR_LIBRARY_CHANGED)],
        "changes": [{"path": name, "before": identity(name, baseline[name]),
                     "after": identity(name, body)} for name, body in sorted(inputs.items())
                    if name in baseline and body != baseline[name]],
        "additions": sorted(PARENT_ADDED),
        "unchanged_baseline_inputs": sum(inputs[name] == body for name, body in baseline.items()),
        "files": [identity(name, body) for name, body in sorted(inputs.items())],
        "copy_record_vectors_semantics_qualified_by_this_gate": False,
        "windows_module_sources_semantics_qualified_by_this_gate": False,
        "windows_activation_qualified_by_this_gate": False,
        "qualified_module_completion_semantics_qualified_by_this_gate": False,
        "copy_record_debug_semantics_qualified_by_this_gate": False,
        "windows_native_semantics_qualified_by_this_gate": False,
        "win32_native_doctor_semantics_qualified_by_this_gate": False,
        "windows_native_platform_semantics_qualified_by_this_gate": False,
        "windows_native_debug_semantics_qualified_by_this_gate": False,
        "msvc_pe_execution_qualified_by_this_gate": False,
        "record_field_completion_semantics_qualified_by_this_gate": False,
        "file_seek_semantics_qualified_by_this_gate": False,
        "seekable_streaming_datasets_semantics_qualified_by_this_gate": False,
        "scalar_native_library_semantics_qualified_by_this_gate": False,
        "general_ffi_semantics_qualified_by_this_gate": False,
    })
    require(all(value is False for key, value in result.items()
                if key.endswith("qualified_by_this_gate")), "unexpected new qualification claim")
    return result


def prepare_current_observer(original, output, captured, adapter, index_adapter, parser_adapter):
    # The ordinary observation domain and every resource predicate are unchanged.
    return library.prepare_current_observer(original, output, captured,
                                            adapter, index_adapter, parser_adapter)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_scalar_library_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained default-emission scalar/record semantics and resources; scalar native libraries, general FFI and new platform/debug semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_scalar_library_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional scalar-library pins permit prepare-only admission, not qualification")
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
