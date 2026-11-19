#!/usr/bin/env python3
"""Bind Copy record debug source changes to the complete retained G6 replay.

This gate covers its inherited default-emission semantics and resource domain.
New Copy record DWARF semantics remain false and require separate real debugger
verification. Provisional pins admit preparation only, never qualification.
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

PREDECESSOR = '20ea8ba102975d2e8ac0c667b96ce835a1bc6ef9'
PREDECESSOR_POLICY = 'scripts/verify_qualified_completion_compatibility.py'
PREDECESSOR_POLICY_PIN = (44446, '239385c8506d3ff10f7102c74391ec280f4b3bd7990830f2d2f12a934e8ec194')
PINS_PROVISIONAL = False
COPY_RECORD_DEBUG_CHANGED = frozenset(['src/frontend/driver.rs',
 'src/frontend/native_debug.rs',
 'src/frontend/oir/mod.rs',
 'src/frontend/oir/owned/mod.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/source/program.rs',
 'src/frontend/oir/owned_types.rs',
 'src/frontend/oir/source/sealed.rs'])
COPY_RECORD_DEBUG_ADDED = frozenset(['src/frontend/native_debug/record_tests.rs',
 'src/frontend/native_debug/records.rs',
 'src/frontend/oir/owned/copy_record_debug_tests.rs',
 'src/frontend/oir/owned/debug.rs'])
REVIEWED_INPUTS = {'src/frontend/driver.rs': (96922, 'b99c3efb1ef3a1577f40a68d4794bb8efd4747d593bd691ebbaceb92c977c7f5'),
 'src/frontend/native_debug.rs': (26134, '7d288a9cc69dd211504e46a8e414f8a1b95b9c344a227a96dc2c282b984c3a15'),
 'src/frontend/native_debug/record_tests.rs': (12872, 'f391cdb8af109d9a7612f25c19651d4662cc97ecb0f7642b3ba6d04e7eee14c1'),
 'src/frontend/native_debug/records.rs': (12538,
                                          '3504d4294ea2770be3d36ac81584c4d00626d875f0fb55fae05c31fdb0938aec'),
 'src/frontend/oir/mod.rs': (24483, '8d2b0a09f36b8a2547e7463cdff1879e9192aa840d27dfc59c0a435dcff13560'),
 'src/frontend/oir/owned/copy_record_debug_tests.rs': (8742,
                                                       'f60e8286284efb5991784c0de5c745633350de10cf159d0fd9ad71ccfeea6d58'),
 'src/frontend/oir/owned/debug.rs': (7780,
                                     '3d2e4b644f8ae8f80f010aa8214bbe9655715e0db26dc9dce8658dd9cc6cc0a9'),
 'src/frontend/oir/owned/mod.rs': (17666, 'b7711802cc9e646122ba5a95de40258f54e8cc85735530b83adbc6112eea4916'),
 'src/frontend/oir/owned/native.rs': (277349,
                                      '128c682d79de8d3b8ea8584e1d95d654cd05ea8aa6ba1b962457f6302887e472'),
 'src/frontend/oir/owned/source/program.rs': (41435,
                                              'db94a4fc41692e839446c37339f7d5477c0aa4edde51afb1ec22c9cf1079a27f'),
 'src/frontend/oir/owned_types.rs': (98059,
                                     '0cacc193b5e75329f64f50b9f0a0463a3ed9d7e8ded0c50179a6c4c51cb127a8'),
 'src/frontend/oir/source/sealed.rs': (26371,
                                       '5714a2189281fb933084023ff39b2d1150be37c3b83b9776364be23c43247ae2')}
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
 'src/frontend/native_debug.rs': {'attributes': ((b'#[cfg(test)]', 5), (b'#[test]', 2)),
                                  'build_script': None,
                                  'c_preprocessor': (),
                                  'legacy': ([], []),
                                  'macros': ()},
 'src/frontend/native_debug/record_tests.rs': {'attributes': ((b'#[test]', 1),
                                                              (b'#[derive(Copy)]', 3),
                                                              (b'#[test]', 1),
                                                              (b'#[derive(Copy)]', 7),
                                                              (b'#[test]', 1),
                                                              (b'#[derive(Copy)]', 1),
                                                              (b'#[test]', 1),
                                                              (b'#[derive(Copy)]', 1),
                                                              (b'#[cfg(target_pointer_width = "64")]', 1),
                                                              (b'#[test]', 1),
                                                              (b'#[derive(Copy)]', 1),
                                                              (b'#[cfg(all(\n    target_os = "linux",\n    '
                                                               b'target_pointer_width = "64",\n    target_'
                                                               b'endian = "little",\n    any(target_arch ='
                                                               b' "x86_64", target_arch = "aarch64")\n))]',
                                                               1),
                                                              (b'#[test]', 1),
                                                              (b'#[derive(Copy)]', 1)),
                                               'build_script': None,
                                               'c_preprocessor': (),
                                               'legacy': ([], []),
                                               'macros': ()},
 'src/frontend/native_debug/records.rs': {'attributes': ((b'#[allow(dead_code)]', 3),
                                                         (b'#[cfg(all(test, target_pointer_width = "64"))]',
                                                          1),
                                                         (b'#[test]', 1)),
                                          'build_script': None,
                                          'c_preprocessor': (),
                                          'legacy': ([], []),
                                          'macros': ()},
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
 'src/frontend/oir/owned/copy_record_debug_tests.rs': {'attributes': ((b'#[derive(Copy)]', 1),
                                                                      (b'#[cfg(target_pointer_width = "64")]',
                                                                       1),
                                                                      (b'#[test]', 1),
                                                                      (b'#[allow(clippy::too_many_arguments)]',
                                                                       1),
                                                                      (b'#[test]', 2)),
                                                       'build_script': None,
                                                       'c_preprocessor': (),
                                                       'legacy': ([], []),
                                                       'macros': ()},
 'src/frontend/oir/owned/debug.rs': {'attributes': ((b'#[allow(dead_code)]', 2),
                                                    (b'#[derive(Clone, Copy)]', 3),
                                                    (b'#[cfg(test)]', 1),
                                                    (b'#[test]', 1),
                                                    (b'#[cfg(all(test, target_pointer_width = "64"))]', 1),
                                                    (b'#[test]', 1)),
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
                                                  (b'#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]',
                                                   1),
                                                  (b'#[cfg(test)]', 36)),
                                   'build_script': None,
                                   'c_preprocessor': (),
                                   'legacy': ([], []),
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
                                                     (b'#[allow(dead_code)]', 2),
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
                                                     (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialO'
                                                      b'rd, Ord)]',
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
                                                     (b'#[cfg(all(test, any(target_os = "linux", target_os ='
                                                      b' "macos")))]',
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
                                                     (b'#[path = "copy_record_debug_tests.rs"]', 1),
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
                                                  b'#[path = "copy_record_debug_tests.rs"]',
                                                  b'#[path = "record_vector_native_tests.rs"]']),
                                      'macros': ((b'cfg!(all(target_os="macos",target_arch="aarch64"))', 1),
                                                 (b'cfg!(all(target_os="macos",target_arch="x86_64"))', 1),
                                                 (b'cfg!(all(target_os="linux",target_arch="aarch64"))', 1))},
 'src/frontend/oir/owned/source/program.rs': {'attributes': ((b'#[cfg(test)]', 2),
                                                             (b'#[path = "experimental_source_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "i64_source_tests.rs"]', 1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 5),
                                                             (b'#[path = "candidate_adapter.rs"]', 1),
                                                             (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                              1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[derive(Clone, Copy, Debug)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[cfg(test)]', 3),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[test]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 3),
                                                             (b'#[test]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([],
                                                         [b'#[path = "experimental_source_tests.rs"]',
                                                          b'#[path = "i64_source_tests.rs"]',
                                                          b'#[path = "candidate_adapter.rs"]']),
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
                                                    (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]',
                                                     1),
                                                    (b'#[derive(Debug)]', 2),
                                                    (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]',
                                                     1),
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
 'src/frontend/oir/source/sealed.rs': {'attributes': ((b'#[allow(clippy::large_enum_variant)]', 1),
                                                      (b'#[derive(Debug)]', 2),
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
                                       'macros': ()}}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and all(not parent.is_symlink() for parent in path.parents),
            "symlink pinned G6 policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned G6 policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned G6 policy")
    module = types.ModuleType("_copy_record_debug_retained_G6")
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
PARENT_ADDED = PREDECESSOR_ADDED | COPY_RECORD_DEBUG_ADDED
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
    expected = expected_parent | COPY_RECORD_DEBUG_ADDED
    require(set(parent) == expected_parent and len(parent) == 506,
            "wrong G6 compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (COPY_RECORD_DEBUG_CHANGED | COPY_RECORD_DEBUG_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == COPY_RECORD_DEBUG_CHANGED | COPY_RECORD_DEBUG_ADDED
            and COPY_RECORD_DEBUG_CHANGED <= expected_parent
            and not COPY_RECORD_DEBUG_ADDED & expected_parent,
            "wrong reviewed Copy-record-debug input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed Copy-record-debug directive membership")
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
        if name not in COPY_RECORD_DEBUG_CHANGED:
            require(inputs[name] == before,
                    "unreviewed change outside Copy-record-debug boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "Copy-record-debug source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "Copy-record-debug compile-time input directive changed: " + name)
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
        "copy_record_debug_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: identity(name, parent[name])
                                      for name in sorted(UNCHANGED_INPUTS)},
        "copy_record_debug_reviewed_inputs": [
            {"path": name, "bytes": length, "sha256": digest}
            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "copy_record_debug_changes": [
            {"path": name, "before": identity(name, parent[name]),
             "after": identity(name, inputs[name])}
            for name in sorted(COPY_RECORD_DEBUG_CHANGED)],
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
    require(Path(__file__).resolve() == repo / "scripts/verify_copy_record_debug_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained default-emission scalar/record semantics and resources; Copy record DWARF semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_copy_record_debug_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional Copy-record-debug pins permit prepare-only admission, not qualification")
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
