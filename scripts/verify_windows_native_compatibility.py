#!/usr/bin/env python3
"""Bind Windows native source changes to the complete retained Copy debug replay.

Only its inherited default-emission semantics and resource domain are covered.
Windows native, Win32 doctor, PE execution and new platform/debug semantics stay
false and require their own real host evidence. Provisional pins permit source
preparation only; they cannot start qualification or prove an MSVC/PE build.
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

PREDECESSOR = '9a5ecaaeb5fa50aff2296bf990d63167143cf312'
PREDECESSOR_POLICY = 'scripts/verify_copy_record_debug_compatibility.py'
PREDECESSOR_POLICY_PIN = (50526, '9d4c5981b6f027d782a4a7e7505cc2cce812c6c689e8fe8bbb592b5745b7f688')
PINS_PROVISIONAL = True
WINDOWS_NATIVE_CHANGED = frozenset(['native/typed_preview.c',
 'src/frontend/build_config.rs',
 'src/frontend/driver.rs',
 'src/frontend/mod.rs',
 'src/frontend/native.rs',
 'src/frontend/native/doctor.rs',
 'src/frontend/oir/native.rs',
 'src/frontend/oir/owned/native.rs',
 'src/frontend/oir/source/sealed.rs',
 'src/frontend/project/filesystem/windows.rs',
 'src/project_cli.rs',
 'src/project_cli/workspace.rs'])
WINDOWS_NATIVE_ADDED = frozenset(['src/frontend/native/doctor_windows.rs', 'src/frontend/windows_native.rs'])
REVIEWED_INPUTS = {'native/typed_preview.c': (5984, '8cac647ec6ac8d08c2dca1e430895230895ab07e25b703484926622f890322c4'),
 'src/frontend/build_config.rs': (29350, '9d8f1e1931e57ff643f56c88764210f65a614eb6b38fa34a979e52acb9baa146'),
 'src/frontend/driver.rs': (98286, '27777587cd3a4cd3386ecc575e9ac1a89b94dfc1afa4e92fdab7fd54cca827ab'),
 'src/frontend/mod.rs': (1096, 'cf533e44c182a4674cafdd063ddb3a1132b323eccb1a1969e7c62766bcac5eea'),
 'src/frontend/native.rs': (26254, 'b0946fd1327e1b73cd528bb9da6c956b1d2c3e37810b3bf9b0c3612c257ca93a'),
 'src/frontend/native/doctor.rs': (22082, '3a1b884953c231381377d69f1de0a07c86ea468e047c6722eaeafd390a3a7b07'),
 'src/frontend/native/doctor_windows.rs': (36747,
                                           '1007ebe0f69357a188b36bfb5512381769ad2b8841af0f0528d6361123600f8a'),
 'src/frontend/oir/native.rs': (123158, 'ba9d2dff71290959e3186ecd1961bd5a8dca4b57000d1328ce5516672adcd5e1'),
 'src/frontend/oir/owned/native.rs': (275386,
                                      'e8b9f255356796ac4202607934e362e5b7144f734bbbaa8f1db09b5d264fdc28'),
 'src/frontend/oir/source/sealed.rs': (27198,
                                       '1bc6198c7eecce967298a7c226e3808bc09611831f64a8fdca6280ccb1b665f9'),
 'src/frontend/project/filesystem/windows.rs': (34555,
                                                '400d009dd41b607bcdc078cbb615b51187740c8201f59d09ff8b2afc442f0ee0'),
 'src/frontend/windows_native.rs': (9001, '37803f883571faa819633aea3dfebaaf538fa8f90478fc217647e986527de49a'),
 'src/project_cli.rs': (64635, '83c232c1c02a515974d08393fa1bf23af405cc17941b043a3bccce2a80c5885b'),
 'src/project_cli/workspace.rs': (33234, '7ea63f9b370d15bfc65a4ca41e1b2f7f0ce8bcd83a0d32d6673ed0f93498c24c')}
REVIEWED_DIRECTIVES = {'native/typed_preview.c': {'attributes': (),
                            'build_script': None,
                            'c_preprocessor': ((b'#include <errno.h>\n', 1),
                                               (b'#include <signal.h>\n', 1),
                                               (b'#include <stddef.h>\n', 1),
                                               (b'#include <stdint.h>\n', 1),
                                               (b'#if defined(_WIN32)\n', 1),
                                               (b'#define WIN32_LEAN_AND_MEAN\n', 1),
                                               (b'#define NOMINMAX\n', 1),
                                               (b'#include <windows.h>\n', 1),
                                               (b'#define STDIN_FILENO 0\n', 1),
                                               (b'#define STDOUT_FILENO 1\n', 1),
                                               (b'#define STDERR_FILENO 2\n', 1),
                                               (b'#else\n', 1),
                                               (b'#include <unistd.h>\n', 1),
                                               (b'#define native_read read\n', 1),
                                               (b'#define native_write write\n', 1),
                                               (b'#define native_exit _exit\n', 1),
                                               (b'#define native_interrupted() (errno == EINTR)\n', 1),
                                               (b'#endif\n', 1),
                                               (b'#if defined(_WIN32)\n', 1),
                                               (b'#else\n', 1),
                                               (b'#endif\n', 1),
                                               (b'#if !defined(_WIN32)\n', 1),
                                               (b'#endif\n', 1)),
                            'legacy': ([], []),
                            'macros': ()},
 'src/frontend/build_config.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 7),
                                                 (b'#[cfg(all(\n        target_os = "linux",\n        targ'
                                                  b'et_pointer_width = "64",\n        target_endian = "little'
                                                  b'",\n        any(target_arch = "x86_64", target_arch = "aa'
                                                  b'rch64")\n    ))]',
                                                  1),
                                                 (b'#[derive(Debug)]', 1),
                                                 (b'#[derive(Clone, Copy)]', 1),
                                                 (b'#[cfg(test)]', 1),
                                                 (b'#[test]', 1),
                                                 (b'#[cfg(all(\n                target_os = "linux",\n    '
                                                  b'            target_pointer_width = "64",\n               '
                                                  b' target_endian = "little",\n                any(target_ar'
                                                  b'ch = "x86_64", target_arch = "aarch64")\n            ))]',
                                                  1),
                                                 (b'#[test]', 5),
                                                 (b'#[cfg(unix)]', 1),
                                                 (b'#[test]', 2),
                                                 (b'#[cfg(unix)]', 1)),
                                  'build_script': None,
                                  'c_preprocessor': (),
                                  'legacy': ([], []),
                                  'macros': ((b'cfg!(all(target_os = "macos", target_arch = "aarch64"))', 1),
                                             (b'cfg!(all(target_os = "macos", target_arch = "x86_64"))', 1),
                                             (b'cfg!(all(target_os = "linux", target_arch = "aarch64"))', 1),
                                             (b'cfg!(all(\n            target_os = "windows",\n           '
                                              b' target_env = "msvc",\n            target_arch = "x86_64"'
                                              b',\n            target_pointer_width = "64",\n            t'
                                              b'arget_endian = "little"\n        ))',
                                              1),
                                             (b'cfg!(all(\n                any(target_os = "linux", target_os'
                                              b' = "macos"),\n                target_pointer_width = "64"'
                                              b',\n                target_endian = "little",\n            '
                                              b'    any(target_arch = "x86_64", target_arch = "aarch64")\n   '
                                              b'         ))',
                                              1),
                                             (b'cfg!(windows)', 1),
                                             (b'cfg!(target_os = "linux")', 1),
                                             (b'cfg!(all(target_os = "linux", target_arch = "aarch64"))', 3),
                                             (b'cfg!(target_env = "msvc")', 1))},
 'src/frontend/driver.rs': {'attributes': ((b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
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
 'src/frontend/native/doctor.rs': {'attributes': ((b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                                  (b'#[cfg(all(\n        target_os = "windows",\n        ta'
                                                   b'rget_arch = "x86_64",\n        target_pointer_width ='
                                                   b' "64",\n        target_endian = "little"\n    ))]',
                                                   1),
                                                  (b'#[cfg(all(\n        any(target_os = "linux", target_o'
                                                   b's = "macos"),\n        target_pointer_width = "64",\n '
                                                   b'       target_endian = "little",\n        any(target_'
                                                   b'arch = "x86_64", target_arch = "aarch64")\n    ))]',
                                                   1),
                                                  (b'#[cfg(all(\n    any(target_os = "linux", target_os = '
                                                   b'"macos"),\n    target_pointer_width = "64",\n    targe'
                                                   b't_endian = "little",\n    any(target_arch = "x86_64",'
                                                   b' target_arch = "aarch64")\n))]',
                                                   1),
                                                  (b'#[cfg(target_os = "linux")]', 2),
                                                  (b'#[cfg(all(\n    target_os = "windows",\n    target_arc'
                                                   b'h = "x86_64",\n    target_pointer_width = "64",\n    t'
                                                   b'arget_endian = "little"\n))]',
                                                   1),
                                                  (b'#[path = "doctor_windows.rs"]', 1)),
                                   'build_script': None,
                                   'c_preprocessor': (),
                                   'legacy': ([], [b'#[path = "doctor_windows.rs"]']),
                                   'macros': ((b'cfg!(target_os = "macos")', 1),
                                              (b'cfg!(target_arch = "aarch64")', 1),
                                              (b'cfg!(target_os = "macos")', 1))},
 'src/frontend/native/doctor_windows.rs': {'attributes': ((b'#[repr(C)]', 6),
                                                          (b'#[link(name = "Kernel32")]', 1),
                                                          (b'#[repr(align(16))]', 1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[test]', 5),
                                                          (b'#[link(name = "Shell32")]', 1),
                                                          (b'#[link(name = "Kernel32")]', 1)),
                                           'build_script': None,
                                           'c_preprocessor': (),
                                           'legacy': ([], []),
                                           'macros': ()},
 'src/frontend/oir/native.rs': {'attributes': ((b'#[allow(dead_code)]', 1),
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
                                           [b'#[path = "native_private_emit.rs"]',
                                            b'#[path = "native_scalar_resource.rs"]',
                                            b'#[path = "native_emit_work.rs"]',
                                            b'#[path = "native_emit_cost.rs"]',
                                            b'#[path = "native_private_emit_tests.rs"]',
                                            b'#[path = "native_u8_tests.rs"]',
                                            b'#[path = "native_emit_observation.rs"]',
                                            b'#[path = "native_hosted_capacity_tests.rs"]']),
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
                                       'macros': ()},
 'src/frontend/project/filesystem/windows.rs': {'attributes': ((b'#[repr(C)]', 1),
                                                               (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                               (b'#[repr(C)]', 1),
                                                               (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                               (b'#[link(name = "Kernel32")]', 1),
                                                               (b'#[derive(Clone, Copy, Debug, Eq, Partial'
                                                                b'Eq)]',
                                                                1),
                                                               (b'#[derive(Debug)]', 2),
                                                               (b'#[cfg(test)]', 2),
                                                               (b'#[test]', 8)),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/windows_native.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 6)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ()},
 'src/project_cli.rs': {'attributes': ((b'#[derive(Default)]', 1),
                                       (b'#[derive(Clone, Copy, Default)]', 1),
                                       (b'#[cfg(test)]', 1),
                                       (b'#[test]', 5)),
                        'build_script': None,
                        'c_preprocessor': (),
                        'legacy': ([], []),
                        'macros': ((b'cfg!(target_os = "linux")', 1), (b'cfg!(windows)', 1))},
 'src/project_cli/workspace.rs': {'attributes': ((b'#[allow(clippy::too_many_arguments)]', 1),),
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
            "symlink pinned Copy debug policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned Copy debug policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned Copy debug policy")
    module = types.ModuleType("_windows_native_retained_copy_debug")
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
PARENT_ADDED = PREDECESSOR_ADDED | WINDOWS_NATIVE_ADDED
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
    expected = expected_parent | WINDOWS_NATIVE_ADDED
    require(set(parent) == expected_parent and len(parent) == 510,
            "wrong Copy debug compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (WINDOWS_NATIVE_CHANGED | WINDOWS_NATIVE_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == WINDOWS_NATIVE_CHANGED | WINDOWS_NATIVE_ADDED
            and WINDOWS_NATIVE_CHANGED <= expected_parent
            and not WINDOWS_NATIVE_ADDED & expected_parent,
            "wrong reviewed Windows-native input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed Windows-native directive membership")
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
        if name not in WINDOWS_NATIVE_CHANGED:
            require(inputs[name] == before,
                    "unreviewed change outside Windows-native boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "Windows-native source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "Windows-native compile-time input directive changed: " + name)
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
        "windows_native_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: identity(name, parent[name])
                                      for name in sorted(UNCHANGED_INPUTS)},
        "windows_native_reviewed_inputs": [
            {"path": name, "bytes": length, "sha256": digest}
            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "windows_native_changes": [
            {"path": name, "before": identity(name, parent[name]),
             "after": identity(name, inputs[name])}
            for name in sorted(WINDOWS_NATIVE_CHANGED)],
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
    require(Path(__file__).resolve() == repo / "scripts/verify_windows_native_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained default-emission scalar/record semantics and resources; Windows native, Win32 doctor, MSVC/PE and new platform/debug semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_windows_native_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional Windows-native pins permit prepare-only admission, not qualification")
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
