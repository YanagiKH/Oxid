#!/usr/bin/env python3
"""Bind File seek inputs to the complete retained record-field-source replay.

Only inherited default-emission semantics and resource predicates are replayed.
File seeking, streaming datasets and all new platform/debug claims stay false.
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

PREDECESSOR = 'b273b71237cc6a1d2db76dfe7c971e073d1d5cb3'
PREDECESSOR_POLICY = 'scripts/verify_record_field_completion_compatibility.py'
PREDECESSOR_POLICY_PIN = (34962, 'ead92afc11561aa67df6ac1ed6ed6b468a5a323ab2dd7d34e7cb5a5131386484')
PINS_PROVISIONAL = False
FILE_SEEK_CHANGED = frozenset(['native/experimental_files.c',
 'src/frontend/builtin_catalog.rs',
 'src/frontend/declaration_index/experimental_tests.rs',
 'src/frontend/experimental_builtins.rs',
 'src/frontend/oir/owned/byte_buffer.rs',
 'src/frontend/oir/owned/execute.rs',
 'src/frontend/oir/owned/execute_experimental.rs',
 'src/frontend/oir/owned/execute_file_delivery.rs',
 'src/frontend/oir/owned/file_resources.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/owned/native_experimental.rs',
 'src/frontend/oir/owned/plan.rs',
 'src/frontend/oir/owned/source/experimental_source_tests.rs'])
FILE_SEEK_ADDED = frozenset(['src/frontend/oir/owned/execute_file_seek_tests.rs',
 'src/frontend/oir/owned/source/file_seek_tests.rs'])
REVIEWED_INPUTS = {'native/experimental_files.c': (8683, '1e51ca1fdc198598d71c0e90cc3aaa7237cd81a305f5236af76754f560f01d18'),
 'src/frontend/builtin_catalog.rs': (15910,
                                     '2085f8675a8990b2a60be1767b22290e37a19b30c3d9eb7ab2c270146f3bd65d'),
 'src/frontend/declaration_index/experimental_tests.rs': (3520,
                                                          'efc86f81808ea4130f2b281bc980cebb5904e0a8a1ce6e3d3e0bb47ef15bc50e'),
 'src/frontend/experimental_builtins.rs': (27663,
                                           'c57674ac52b10220a4a23dafb642149cba7c8b1bc84874f09aed56b4b83ddb58'),
 'src/frontend/oir/owned/byte_buffer.rs': (21236,
                                           '6f67e6d8e29d221b83f494fef9e492e4fa8fe125ed2169559ca63d79634ca289'),
 'src/frontend/oir/owned/execute.rs': (216754,
                                       '4f515afd1001d697faa92f6ff5cb767433bb54d30bbe69f51cfb7feac8ac0726'),
 'src/frontend/oir/owned/execute_experimental.rs': (27699,
                                                    '25c22714959e69b14c74cfd16673f594dce2f01c14d388f77880a7049caf3cfd'),
 'src/frontend/oir/owned/execute_file_delivery.rs': (20213,
                                                     'ccaa52690c86b283ad3564f37c6238a6032d7e35d1b5ae39013aa3cef8cd3d74'),
 'src/frontend/oir/owned/execute_file_seek_tests.rs': (6301,
                                                       'e052abc86e38ef0e1ef467409a7443a2c76efdddd2ccb4b88724379348a7707b'),
 'src/frontend/oir/owned/file_resources.rs': (16111,
                                              '0a93fc4c92676f194c6d054c9b2035c389a57fdadea1a5e26b746793261c0416'),
 'src/frontend/oir/owned/native.rs': (276082,
                                      'c1adac08f3128ddfc8a3a74ef52963d918095fd10db6802c04d2f31893c561cc'),
 'src/frontend/oir/owned/native_experimental.rs': (20622,
                                                   '83f348ce7260a3f4b01b0abb911fd7cf93b89c139847bd9f362d411072c148ae'),
 'src/frontend/oir/owned/plan.rs': (38505,
                                    '27860e4a16583676464201426b771b492e853127105e4a3d7ef23f2f1a6c3fa6'),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': (25284,
                                                                '88493df7f88ea6de92773de805946eaf35c41cf6ccf3e2ed47930a8540bb8e56'),
 'src/frontend/oir/owned/source/file_seek_tests.rs': (4619,
                                                      '704a8662f77fd008e8827b15c1e275baf90fd9314969c9cbd5927fc659f9bcd3')}
REVIEWED_DIRECTIVES = {'native/experimental_files.c': {'attributes': (),
                                 'build_script': None,
                                 'c_preprocessor': ((b'#include <fcntl.h>\n', 1),
                                                    (b'#ifndef O_CLOEXEC\n', 1),
                                                    (b'#if defined(__linux__)\n', 1),
                                                    (b'#define O_CLOEXEC 02000000\n', 1),
                                                    (b'#else\n', 1),
                                                    (b'#error "O_CLOEXEC is required for native file handle'
                                                     b's"\n',
                                                     1),
                                                    (b'#endif\n', 2)),
                                 'legacy': ([], []),
                                 'macros': ()},
 'src/frontend/builtin_catalog.rs': {'attributes': ((b'#![allow(dead_code)]', 1),
                                                    (b'#[path = "experimental_builtins.rs"]', 1),
                                                    (b'#[path = "vector_builtins.rs"]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]',
                                                     1),
                                                    (b'#[default]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                    (b'#[allow(dead_code)]', 1),
                                                    (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 4),
                                                    (b'#[cfg(test)]', 1),
                                                    (b'#[test]', 1)),
                                     'build_script': None,
                                     'c_preprocessor': (),
                                     'legacy': ([],
                                                [b'#[path = "experimental_builtins.rs"]',
                                                 b'#[path = "vector_builtins.rs"]']),
                                     'macros': ()},
 'src/frontend/declaration_index/experimental_tests.rs': {'attributes': ((b'#[test]', 1),),
                                                          'build_script': None,
                                                          'c_preprocessor': (),
                                                          'legacy': ([], []),
                                                          'macros': ()},
 'src/frontend/experimental_builtins.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                           1),
                                                          (b'#[repr(u8)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                           2),
                                                          (b'#[derive(Clone, Copy, Debug, Default, PartialEq,'
                                                           b' Eq)]',
                                                           1),
                                                          (b'#[repr(transparent)]', 1),
                                                          (b'#[allow(dead_code)]', 3),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[test]', 4)),
                                           'build_script': None,
                                           'c_preprocessor': (),
                                           'legacy': ([], []),
                                           'macros': ()},
 'src/frontend/oir/owned/byte_buffer.rs': {'attributes': ((b'#[path = "file_resources.rs"]', 1),
                                                          (b'#[path = "vector_resources.rs"]', 1),
                                                          (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                          (b'#[repr(C)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                           1),
                                                          (b'#[repr(i32)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                           1),
                                                          (b'#[repr(C)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, Default, PartialEq,'
                                                           b' Eq)]',
                                                           1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                           1),
                                                          (b'#[cfg(target_pointer_width = "64")]', 1),
                                                          (b'#[cfg(all(not(test), target_pointer_width = "64"'
                                                           b'))]',
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
 'src/frontend/oir/owned/execute_experimental.rs': {'attributes': ((b'#[path = "execute_io_delivery.rs"]', 1),
                                                                   (b'#[path = "execute_file_delivery.rs"]',
                                                                    1),
                                                                   (b'#[derive(Clone, Copy)]', 1),
                                                                   (b'#[cfg(test)]', 3)),
                                                    'build_script': None,
                                                    'c_preprocessor': (),
                                                    'legacy': ([],
                                                               [b'#[path = "execute_io_delivery.rs"]',
                                                                b'#[path = "execute_file_delivery.rs"]']),
                                                    'macros': ()},
 'src/frontend/oir/owned/execute_file_delivery.rs': {'attributes': ((b'#[cfg(all(\n    test,\n    any(target_'
                                                                     b'os = "linux", target_os = "macos"),\n'
                                                                     b'    target_pointer_width = "64",\n   '
                                                                     b' target_endian = "little",\n    any(t'
                                                                     b'arget_arch = "x86_64", target_arch ='
                                                                     b' "aarch64")\n))]',
                                                                     1),
                                                                    (b'#[path = "execute_file_seek_tests.rs'
                                                                     b'"]',
                                                                     1),
                                                                    (b'#[derive(Clone, Copy)]', 1),
                                                                    (b'#[allow(dead_code)]', 2),
                                                                    (b'#[cfg(test)]', 2)),
                                                     'build_script': None,
                                                     'c_preprocessor': (),
                                                     'legacy': ([],
                                                                [b'#[path = "execute_file_seek_tests.rs"]']),
                                                     'macros': ()},
 'src/frontend/oir/owned/execute_file_seek_tests.rs': {'attributes': ((b'#[test]', 1),),
                                                       'build_script': None,
                                                       'c_preprocessor': (),
                                                       'legacy': ([], []),
                                                       'macros': ()},
 'src/frontend/oir/owned/file_resources.rs': {'attributes': ((b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                             (b'#[repr(C)]', 1),
                                                             (b'#[allow(dead_code)]', 2),
                                                             (b'#[cfg(target_os = "macos")]', 1),
                                                             (b'#[cfg(not(target_os = "macos"))]', 1),
                                                             (b'#[cfg(not(all(\n            any(target_os = "'
                                                              b'linux", target_os = "macos"),\n            ta'
                                                              b'rget_pointer_width = "64",\n            targe'
                                                              b't_endian = "little",\n            any(target_'
                                                              b'arch = "x86_64", target_arch = "aarch64")\n  '
                                                              b'      )))]',
                                                              1),
                                                             (b'#[cfg(all(\n            any(target_os = "linu'
                                                              b'x", target_os = "macos"),\n            target'
                                                              b'_pointer_width = "64",\n            target_en'
                                                              b'dian = "little",\n            any(target_arch'
                                                              b' = "x86_64", target_arch = "aarch64")\n      '
                                                              b'  ))]',
                                                              2),
                                                             (b'#[cfg(not(all(\n            any(target_os = "'
                                                              b'linux", target_os = "macos"),\n            ta'
                                                              b'rget_pointer_width = "64",\n            targe'
                                                              b't_endian = "little",\n            any(target_'
                                                              b'arch = "x86_64", target_arch = "aarch64")\n  '
                                                              b'      )))]',
                                                              1),
                                                             (b'#[cfg(all(\n            any(target_os = "linu'
                                                              b'x", target_os = "macos"),\n            target'
                                                              b'_pointer_width = "64",\n            target_en'
                                                              b'dian = "little",\n            any(target_arch'
                                                              b' = "x86_64", target_arch = "aarch64")\n      '
                                                              b'  ))]',
                                                              1),
                                                             (b'#[cfg(not(all(\n            any(target_os = "'
                                                              b'linux", target_os = "macos"),\n            ta'
                                                              b'rget_pointer_width = "64",\n            targe'
                                                              b't_endian = "little",\n            any(target_'
                                                              b'arch = "x86_64", target_arch = "aarch64")\n  '
                                                              b'      )))]',
                                                              1),
                                                             (b'#[cfg(all(\n        any(target_os = "linux", '
                                                              b'target_os = "macos"),\n        target_pointer'
                                                              b'_width = "64",\n        target_endian = "litt'
                                                              b'le",\n        any(target_arch = "x86_64", tar'
                                                              b'get_arch = "aarch64")\n    ))]',
                                                              1),
                                                             (b'#[cfg(not(all(\n        any(target_os = "linu'
                                                              b'x", target_os = "macos"),\n        target_poi'
                                                              b'nter_width = "64",\n        target_endian = "'
                                                              b'little",\n        any(target_arch = "x86_64",'
                                                              b' target_arch = "aarch64")\n    )))]',
                                                              1),
                                                             (b'#[cfg(all(\n            any(target_os = "linu'
                                                              b'x", target_os = "macos"),\n            target'
                                                              b'_pointer_width = "64",\n            target_en'
                                                              b'dian = "little",\n            any(target_arch'
                                                              b' = "x86_64", target_arch = "aarch64")\n      '
                                                              b'  ))]',
                                                              1),
                                                             (b'#[cfg(not(all(\n            any(target_os = "'
                                                              b'linux", target_os = "macos"),\n            ta'
                                                              b'rget_pointer_width = "64",\n            targe'
                                                              b't_endian = "little",\n            any(target_'
                                                              b'arch = "x86_64", target_arch = "aarch64")\n  '
                                                              b'      )))]',
                                                              1),
                                                             (b'#[cfg(all(\n            any(target_os = "linu'
                                                              b'x", target_os = "macos"),\n            target'
                                                              b'_pointer_width = "64",\n            target_en'
                                                              b'dian = "little",\n            any(target_arch'
                                                              b' = "x86_64", target_arch = "aarch64")\n      '
                                                              b'  ))]',
                                                              1),
                                                             (b'#[cfg(not(all(\n            any(target_os = "'
                                                              b'linux", target_os = "macos"),\n            ta'
                                                              b'rget_pointer_width = "64",\n            targe'
                                                              b't_endian = "little",\n            any(target_'
                                                              b'arch = "x86_64", target_arch = "aarch64")\n  '
                                                              b'      )))]',
                                                              1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "file_resource_tests.rs"]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([], [b'#[path = "file_resource_tests.rs"]']),
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
                                                                  (b'#[allow(dead_code)]', 2),
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
                                                                  (b'#[allow(clippy::too_many_arguments)]',
                                                                   4)),
                                                   'build_script': None,
                                                   'c_preprocessor': (),
                                                   'legacy': ([], []),
                                                   'macros': ()},
 'src/frontend/oir/owned/plan.rs': {'attributes': ((b'#[path = "native_storage.rs"]', 1),
                                                   (b'#[path = "native_vector_layout.rs"]', 1),
                                                   (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                   (b'#[cfg(test)]', 1),
                                                   (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]',
                                                    1),
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
 'src/frontend/oir/owned/source/experimental_source_tests.rs': {'attributes': ((b'#[cfg(all(\n    target_os'
                                                                                b' = "linux",\n    target_p'
                                                                                b'ointer_width = "64",\n   '
                                                                                b' target_endian = "little'
                                                                                b'",\n    any(target_arch ='
                                                                                b' "x86_64", target_arch ='
                                                                                b' "aarch64")\n))]',
                                                                                1),
                                                                               (b'#[path = "file_seek_test'
                                                                                b's.rs"]',
                                                                                1),
                                                                               (b'#[path = "io_delivery_so'
                                                                                b'urce_tests.rs"]',
                                                                                1),
                                                                               (b'#[path = "process_argume'
                                                                                b'nts_tests.rs"]',
                                                                                1),
                                                                               (b'#[test]', 2),
                                                                               (b'#[cfg(all(\n    target_os'
                                                                                b' = "linux",\n    target_p'
                                                                                b'ointer_width = "64",\n   '
                                                                                b' target_endian = "little'
                                                                                b'",\n    any(target_arch ='
                                                                                b' "x86_64", target_arch ='
                                                                                b' "aarch64")\n))]',
                                                                                1),
                                                                               (b'#[test]', 1),
                                                                               (b'#[cfg(all(\n    target_os'
                                                                                b' = "linux",\n    target_p'
                                                                                b'ointer_width = "64",\n   '
                                                                                b' target_endian = "little'
                                                                                b'",\n    any(target_arch ='
                                                                                b' "x86_64", target_arch ='
                                                                                b' "aarch64")\n))]',
                                                                                1),
                                                                               (b'#[test]', 1),
                                                                               (b'#[cfg(all(\n    target_os'
                                                                                b' = "linux",\n    target_p'
                                                                                b'ointer_width = "64",\n   '
                                                                                b' target_endian = "little'
                                                                                b'",\n    any(target_arch ='
                                                                                b' "x86_64", target_arch ='
                                                                                b' "aarch64")\n))]',
                                                                                1),
                                                                               (b'#[test]', 9),
                                                                               (b'#[cfg(all(\n    target_os'
                                                                                b' = "linux",\n    target_p'
                                                                                b'ointer_width = "64",\n   '
                                                                                b' target_endian = "little'
                                                                                b'",\n    any(target_arch ='
                                                                                b' "x86_64", target_arch ='
                                                                                b' "aarch64")\n))]',
                                                                                1)),
                                                                'build_script': None,
                                                                'c_preprocessor': (),
                                                                'legacy': ([],
                                                                           [b'#[path = "file_seek_tests.rs"]',
                                                                            b'#[path = "io_delivery_source'
                                                                            b'_tests.rs"]',
                                                                            b'#[path = "process_arguments_'
                                                                            b'tests.rs"]']),
                                                                'macros': ((b'env!("CARGO_MANIFEST_DIR")',
                                                                            1),)},
 'src/frontend/oir/owned/source/file_seek_tests.rs': {'attributes': ((b'#[test]', 2),
                                                                     (b'#[cfg(all(\n    any(target_os = "linu'
                                                                      b'x", target_os = "macos"),\n    target'
                                                                      b'_pointer_width = "64",\n    target_en'
                                                                      b'dian = "little",\n    any(target_arch'
                                                                      b' = "x86_64", target_arch = "aarch64"'
                                                                      b')\n))]',
                                                                      1)),
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
            "symlink pinned record-field-completion policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned record-field-completion policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned record-field-completion policy")
    module = types.ModuleType("_file_seek_retained_record_field_completion")
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
PARENT_ADDED = PREDECESSOR_ADDED | FILE_SEEK_ADDED
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
    expected = expected_parent | FILE_SEEK_ADDED
    require(set(parent) == expected_parent and len(parent) == 517,
            "wrong record-field-completion compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (FILE_SEEK_CHANGED | FILE_SEEK_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == FILE_SEEK_CHANGED | FILE_SEEK_ADDED
            and FILE_SEEK_CHANGED <= expected_parent
            and not FILE_SEEK_ADDED & expected_parent,
            "wrong reviewed file-seek input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed file-seek directive membership")
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
        if name not in FILE_SEEK_CHANGED:
            require(inputs[name] == before,
                    "unreviewed change outside file-seek boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "file-seek source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "file-seek compile-time input directive changed: " + name)
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
        "file_seek_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: identity(name, parent[name])
                                      for name in sorted(UNCHANGED_INPUTS)},
        "file_seek_reviewed_inputs": [
            {"path": name, "bytes": length, "sha256": digest}
            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "file_seek_changes": [
            {"path": name, "before": identity(name, parent[name]),
             "after": identity(name, inputs[name])}
            for name in sorted(FILE_SEEK_CHANGED)],
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
    require(Path(__file__).resolve() == repo / "scripts/verify_file_seek_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained default-emission scalar/record semantics and resources; File seeking, streaming datasets and new platform/debug semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_file_seek_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional file-seek pins permit prepare-only admission, not qualification")
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
