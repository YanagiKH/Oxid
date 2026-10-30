#!/usr/bin/env python3
"""Bind reviewed Windows local-package inputs to unchanged retained controls.

The finite source admission and retained observer replay do not qualify Windows
NTFS package behavior. That behavior requires independent Windows execution.
"""
import argparse
from contextlib import contextmanager
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import types


ROOT = Path(__file__).resolve().parents[1]
PREDECESSOR = '1ed9a4b9c373b62d115f31ef9abf1b84debd2e3e'
PREDECESSOR_POLICY = 'scripts/verify_generic_owned_vector_compatibility.py'
PREDECESSOR_POLICY_PIN = (43357, '8e80cbdb0b42e01dd4e125d36215dc94ff37422a0e10786216ec02ad66831ad0')
REVIEWED_SOURCE_HEAD = 'be2d3b1584467ba84eac7ea8667f1161591549ec'
# Reviewed source identity only; Windows runtime qualification remains separate.
PINS_PROVISIONAL = False
# The inherited observer and its derived bytes are already frozen unchanged.
OBSERVER_PINS_PROVISIONAL = False
OBSERVER_ADAPTER = 'scripts/owned_enum_vector_compatibility_observer.py'
OBSERVER_ADAPTER_PIN = (7495, 'ff932fb13ae4db32fe15a389505be7a7ffa2fc0557b15f9b1b153c3602de3fd9')
OBSERVER_DERIVED_PIN = (34582, '73b30bca73d10126977744d50fb4cc42f6dc1cb876c5ee82b7335f3ffc14a3d3')

WINDOWS_CHANGED = frozenset(('src/frontend/driver.rs',
 'src/frontend/mod.rs',
 'src/frontend/oir/owned/source/owned_text_source_resource_tests.rs',
 'src/frontend/oir/source/owned_vector_route_tests.rs',
 'src/frontend/project.rs',
 'src/frontend/project/editor_sources.rs',
 'src/frontend/project/filesystem.rs',
 'src/frontend/project/filesystem/windows.rs',
 'src/frontend/project/packages.rs',
 'src/frontend/project/packages/tests.rs',
 'src/main.rs',
 'src/project_cli/workspace.rs',
 'src/project_lock.rs'))

# These existing Git files are new members of this retained source authority.
WINDOWS_ADDED = frozenset(('tests/typed_documentation.rs',
 'tests/typed_generics.rs',
 'tests/typed_language_server.rs',
 'tests/typed_package_lock.rs',
 'tests/typed_path_dependencies.rs',
 'tests/typed_windows_modules.rs',
 'tests/typed_workspace.rs'))

REVIEWED_INPUTS = {'src/frontend/driver.rs': (103954, 'ed53ec43697679153f55ccb8fba16aef4a27ed623e235a21414d6ff2f5e5b861'),
 'src/frontend/mod.rs': (1218, '505f2694ead7fec5a4d7f20daf633d82bee557684ba5e7e57ddb8e0f8e297c62'),
 'src/frontend/oir/owned/source/owned_text_source_resource_tests.rs': (11544,
                                                                       'ac3699ceb294892a369ffca7c581b2b3558fcfd3f8ec3eadcf01f5e4995259d1'),
 'src/frontend/oir/source/owned_vector_route_tests.rs': (24798,
                                                         '45a9cbf62948bca5e629a956ce3286e9529a65cda1ab3fd589278d6e2e8cc09d'),
 'src/frontend/project.rs': (180949, 'af7c21f84b879e05f3c92dd7208bf695fc225bcecbb6a21980544a66ac374c09'),
 'src/frontend/project/editor_sources.rs': (8285,
                                            'e250ab94e339ad9a103cdaa3e0095b73d12cf2132258bc46674e44c15a325362'),
 'src/frontend/project/filesystem.rs': (5987,
                                        'a67f661001ea3c5d3159139d5eb59322787b50fdca8e13c2c363c016daed9dd5'),
 'src/frontend/project/filesystem/windows.rs': (43375,
                                                '8d6e591c61d49a59684a79086bd4155dfe37cb7b704615e638bb3eb929f0b545'),
 'src/frontend/project/packages.rs': (25792,
                                      'fe8b6af4cd28c3992b133ac9764603cb7ae0bc36ca1d018db0a0d15887c72df3'),
 'src/frontend/project/packages/tests.rs': (21668,
                                            '1fda13e69eff2e219ee63127e493d3c213ec624094ebbf2c7580166ff2f53fcd'),
 'src/main.rs': (164763, '6082ffa6a8cbd740bfecc08eac7702a7bddeeef205273163927999176a10195f'),
 'src/project_cli/workspace.rs': (38868, '21567a17e61d92bc842f61ed604957cb5b7218d686fd52720a220e245bc5fa9e'),
 'src/project_lock.rs': (63499, 'd9024f5add6637504e6e0e3b8d055a467b621ca2d18de3da83d39db9324e52d7'),
 'tests/typed_documentation.rs': (14067, '1288c067ff4ac3188e04d38ecddd47aeb627aab1ae9bbe96b05aa458dfd68059'),
 'tests/typed_generics.rs': (38145, 'b76ee1f9ee808027ac7f4e90bdd373a84a6bc3444ff2425c1780a09d270cbf32'),
 'tests/typed_language_server.rs': (74996,
                                    '33c465f6f802e54f4349db4606cc911b11b666c8f6165766d9ddb6498fc75db4'),
 'tests/typed_package_lock.rs': (20993, '8b6af5ec384013af6f5ea3dbd0beb39c6eeaab7352e91b5bac82a9bf8fe020da'),
 'tests/typed_path_dependencies.rs': (22937,
                                      '36d29ba7b66045e2fb586d431b076ad3fea5cd95d97cf02008e48c4219b84034'),
 'tests/typed_windows_modules.rs': (25888,
                                    'e0e2ac63e323bd0954e644d5689d9ff364ca4efe265bd9f39cc60bccd88d95e3'),
 'tests/typed_workspace.rs': (49944, '2cfd7008827704025c535a9294b0b79671092e732661a40ab7d9957f603a900c')}

REVIEWED_DIRECTIVES = {'src/frontend/driver.rs': {'attributes': ((b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                           (b'#[derive(Debug)]', 2),
                                           (b'#[allow(clippy::large_enum_variant)]', 1),
                                           (b'#[derive(Debug)]', 1),
                                           (b'#[cfg(not(windows))]', 1),
                                           (b'#[cfg(windows)]', 2),
                                           (b'#[cfg(not(windows))]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[test]', 2),
                                           (b'#[cfg(not(windows))]', 1),
                                           (b'#[cfg(windows)]', 1),
                                           (b'#[cfg(any(target_os = "linux", target_os = "macos"))]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(any(\n            target_os = "linux",\n            targ'
                                            b'et_os = "macos",\n            all(\n                windows,\n '
                                            b'               target_arch = "x86_64",\n                targe'
                                            b't_pointer_width = "64",\n                target_endian = "lit'
                                            b'tle"\n            )\n        ))]',
                                            1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(not(windows))]', 1),
                                           (b'#[cfg(windows)]', 1),
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
                                           (b'#[cfg(any(\n        target_os = "linux",\n        target_os = '
                                            b'"macos",\n        all(\n            windows,\n            targe'
                                            b't_arch = "x86_64",\n            target_pointer_width = "64",\n'
                                            b'            target_endian = "little"\n        )\n    ))]',
                                            1),
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
                            'macros': ((b'cfg!(windows)', 1),
                                       (b'cfg!(target_arch = "x86_64")', 1),
                                       (b'cfg!(all(\n            target_pointer_width = "64",\n            t'
                                        b'arget_endian = "little",\n            any(target_arch = "x86_64",'
                                        b' target_arch = "aarch64")\n        ))',
                                        1))},
 'src/frontend/mod.rs': {'attributes': ((b'#[cfg(windows)]', 1), (b'#[cfg(test)]', 3)),
                         'build_script': None,
                         'c_preprocessor': (),
                         'legacy': ([], []),
                         'macros': ()},
 'src/frontend/oir/owned/source/owned_text_source_resource_tests.rs': {'attributes': ((b'#[cfg(any(\n     '
                                                                                       b'   target_os = "linu'
                                                                                       b'x",\n        targ'
                                                                                       b'et_os = "macos",'
                                                                                       b'\n        all(\n  '
                                                                                       b'          window'
                                                                                       b's,\n            t'
                                                                                       b'arget_arch = "x86_64'
                                                                                       b'",\n            t'
                                                                                       b'arget_pointer_width '
                                                                                       b'= "64",\n        '
                                                                                       b'    target_endian = '
                                                                                       b'"little"\n       '
                                                                                       b' )\n    ))]',
                                                                                       1),
                                                                                      (b'#[test]', 3),
                                                                                      (b'#[cfg(not(any(\n '
                                                                                       b'   target_os = "linu'
                                                                                       b'x",\n    target_o'
                                                                                       b's = "macos",\n   '
                                                                                       b' all(\n        wi'
                                                                                       b'ndows,\n        t'
                                                                                       b'arget_arch = "x86_64'
                                                                                       b'",\n        targe'
                                                                                       b't_pointer_width = "6'
                                                                                       b'4",\n        targ'
                                                                                       b'et_endian = "little"'
                                                                                       b'\n    )\n)))]',
                                                                                       1),
                                                                                      (b'#[test]', 1)),
                                                                       'build_script': None,
                                                                       'c_preprocessor': (),
                                                                       'legacy': ([b'include_str!("../../../.'
                                                                                   b'./../packages/utf8_text/'
                                                                                   b'src/lib.ox")'],
                                                                                  []),
                                                                       'macros': ((b'include_str!("../../../.'
                                                                                   b'./../packages/utf8_text/'
                                                                                   b'src/lib.ox")',
                                                                                   1),)},
 'src/frontend/oir/source/owned_vector_route_tests.rs': {'attributes': ((b'#[test]', 2),
                                                                        (b'#[cfg(any(\n    target_os = "linu'
                                                                         b'x",\n    target_os = "macos",\n   '
                                                                         b' all(\n        windows,\n        t'
                                                                         b'arget_arch = "x86_64",\n        t'
                                                                         b'arget_pointer_width = "64",\n    '
                                                                         b'    target_endian = "little"\n   '
                                                                         b' )\n))]',
                                                                         1),
                                                                        (b'#[test]', 1),
                                                                        (b'#[cfg(not(any(\n    target_os = "'
                                                                         b'linux",\n    target_os = "macos",'
                                                                         b'\n    all(\n        windows,\n '
                                                                         b'       target_arch = "x86_64",\n '
                                                                         b'       target_pointer_width = "6'
                                                                         b'4",\n        target_endian = "lit'
                                                                         b'tle"\n    )\n)))]',
                                                                         1),
                                                                        (b'#[test]', 3),
                                                                        (b'#[cfg(target_pointer_width = "64'
                                                                         b'")]',
                                                                         1),
                                                                        (b'#[test]', 7)),
                                                         'build_script': None,
                                                         'c_preprocessor': (),
                                                         'legacy': ([], []),
                                                         'macros': ()},
 'src/frontend/project.rs': {'attributes': ((b'#![allow(dead_code)]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[path = "project/builtin_tests.rs"]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[path = "project/array_syntax_tests.rs"]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[path = "project/enum_carrier_tests.rs"]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[path = "project/enum_index_tests.rs"]', 1),
                                            (b'#[cfg(test)]', 2),
                                            (b'#[path = "project/unit2_tests.rs"]', 1),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 9),
                                            (b'#[derive(Clone, Copy, Debug)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1),
                                            (b'#[derive(Debug)]', 3),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[cfg_attr(test, allow(clippy::result_large_err))]', 1),
                                            (b'#[cfg(not(windows))]', 1),
                                            (b'#[cfg(test)]', 5),
                                            (b'#[allow(clippy::too_many_arguments)]', 1),
                                            (b'#[cfg_attr(test, allow(clippy::result_large_err))]', 1),
                                            (b'#[allow(clippy::too_many_arguments)]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[derive(Debug, Default)]', 1),
                                            (b'#[allow(clippy::boxed_local)]', 1),
                                            (b'#[derive(Clone, Copy)]', 1),
                                            (b'#[cfg(test)]', 3),
                                            (b'#[allow(dead_code)]', 1),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[derive(Clone, Copy)]', 1),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[cfg(not(windows))]', 1),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[cfg(not(windows))]', 1),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[cfg(not(windows))]', 2),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[cfg(not(windows))]', 1),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[cfg(not(windows))]', 1),
                                            (b'#[cfg(windows)]', 1),
                                            (b'#[derive(Debug)]', 1),
                                            (b'#[allow(clippy::too_many_arguments)]', 1),
                                            (b'#[derive(Clone, Copy)]', 1),
                                            (b'#[test]', 1),
                                            (b'#[cfg(test)]', 2),
                                            (b'#[allow(dead_code)]', 1),
                                            (b'#[test]', 2),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Copy, Debug)]', 2),
                                            (b'#[test]', 11),
                                            (b'#[derive(Clone, Copy, Debug)]', 1),
                                            (b'#[allow(clippy::drop_non_drop)]', 1),
                                            (b'#[test]', 1),
                                            (b'#[allow(clippy::drop_non_drop)]', 1),
                                            (b'#[cfg(target_os = "linux")]', 1),
                                            (b'#[test]', 1),
                                            (b'#[allow(dead_code)]', 2),
                                            (b'#[test]', 1),
                                            (b'#[derive(Clone, Copy, Debug)]', 1),
                                            (b'#[test]', 4),
                                            (b'#[allow(dead_code)]', 1),
                                            (b'#[test]', 2),
                                            (b'#[cfg(target_os = "linux")]', 1),
                                            (b'#[test]', 3)),
                             'build_script': None,
                             'c_preprocessor': (),
                             'legacy': ([],
                                        [b'#[path = "project/builtin_tests.rs"]',
                                         b'#[path = "project/array_syntax_tests.rs"]',
                                         b'#[path = "project/enum_carrier_tests.rs"]',
                                         b'#[path = "project/enum_index_tests.rs"]',
                                         b'#[path = "project/unit2_tests.rs"]']),
                             'macros': ()},
 'src/frontend/project/editor_sources.rs': {'attributes': ((b'#[derive(Debug)]', 1),
                                                           (b'#[allow(clippy::large_enum_variant)]', 1),
                                                           (b'#[cfg_attr(test, allow(clippy::result_large_'
                                                            b'err))]',
                                                            1),
                                                           (b'#[cfg(test)]', 1),
                                                           (b'#[cfg(not(windows))]', 1),
                                                           (b'#[cfg(windows)]', 2)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/frontend/project/filesystem.rs': {'attributes': ((b'#[cfg(not(windows))]', 1),
                                                       (b'#[cfg(windows)]', 3),
                                                       (b'#[cfg(not(windows))]', 1),
                                                       (b'#[cfg(any(not(windows), test))]', 1)),
                                        'build_script': None,
                                        'c_preprocessor': (),
                                        'legacy': ([], []),
                                        'macros': ((b'cfg!(any(\n        target_os = "linux",\n        targe'
                                                    b't_os = "macos",\n        all(\n            target_os ='
                                                    b' "windows",\n            target_arch = "x86_64",\n    '
                                                    b'        target_pointer_width = "64",\n            tar'
                                                    b'get_endian = "little"\n        )\n    ))',
                                                    1),)},
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
                                                               (b'#[test]', 11)),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ((b'cfg!(all(\n        target_arch = "x86_64",\n  '
                                                            b'      target_pointer_width = "64",\n        t'
                                                            b'arget_endian = "little"\n    ))',
                                                            1),)},
 'src/frontend/project/packages.rs': {'attributes': ((b'#[derive(Clone, Debug)]', 2),
                                                     (b'#[derive(Debug)]', 2),
                                                     (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                     (b'#[cfg(windows)]', 1),
                                                     (b'#[allow(clippy::result_large_err)]', 3),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[cfg(windows)]', 1),
                                                     (b'#[cfg_attr(test, allow(clippy::result_large_err))]',
                                                      1),
                                                     (b'#[cfg(test)]', 1),
                                                     (b'#[cfg(windows)]', 1),
                                                     (b'#[cfg(not(windows))]', 1),
                                                     (b'#[cfg(windows)]', 1),
                                                     (b'#[cfg(not(windows))]', 1),
                                                     (b'#[cfg(windows)]', 2),
                                                     (b'#[cfg(test)]', 2),
                                                     (b'#[allow(dead_code)]', 1),
                                                     (b'#[test]', 1)),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([], []),
                                      'macros': ((b'cfg!(any(\n                target_os = "linux",\n     '
                                                  b'           target_os = "macos",\n                all('
                                                  b'\n                    target_os = "windows",\n        '
                                                  b'            target_arch = "x86_64",\n                    '
                                                  b'target_pointer_width = "64",\n                    target_'
                                                  b'endian = "little"\n                )\n            ))',
                                                  1),
                                                 (b'cfg!(all(\n            target_arch = "x86_64",\n      '
                                                  b'      target_pointer_width = "64",\n            target_en'
                                                  b'dian = "little"\n        ))',
                                                  1))},
 'src/frontend/project/packages/tests.rs': {'attributes': ((b'#[cfg(all(\n    windows,\n    target_arch = "x'
                                                            b'86_64",\n    target_pointer_width = "64",\n   '
                                                            b' target_endian = "little"\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(all(\n    windows,\n    target_arch = "x'
                                                            b'86_64",\n    target_pointer_width = "64",\n   '
                                                            b' target_endian = "little"\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 3),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(\n    target_os = "linux",\n    targ'
                                                            b'et_os = "macos",\n    all(\n        windows,\n '
                                                            b'       target_arch = "x86_64",\n        targe'
                                                            b't_pointer_width = "64",\n        target_endia'
                                                            b'n = "little"\n    )\n))]',
                                                            1),
                                                           (b'#[test]', 1),
                                                           (b'#[cfg(any(target_os = "linux", target_os = "'
                                                            b'macos"))]',
                                                            1),
                                                           (b'#[test]', 2),
                                                           (b'#[allow(dead_code)]', 1)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/main.rs': {'attributes': ((b'#[path = "runtime/artifact.rs"]', 1),
                                (b'#[path = "runtime/data.rs"]', 1),
                                (b'#[path = "runtime/network.rs"]', 1),
                                (b'#[path = "runtime/packages.rs"]', 1),
                                (b'#[path = "runtime/benchmark.rs"]', 1),
                                (b'#[path = "legacy/syntax.rs"]', 1),
                                (b'#[derive(Clone)]', 1),
                                (b'#[derive(Clone, Debug)]', 4),
                                (b'#[derive(Debug, Clone)]', 1),
                                (b'#[derive(Clone, Debug, Default)]', 2),
                                (b'#[derive(Default)]', 1),
                                (b'#[derive(Clone, Debug)]', 1),
                                (b'#[derive(Clone, Debug, Default)]', 1),
                                (b'#[cfg(not(windows))]', 1),
                                (b'#[cfg(windows)]', 1),
                                (b'#[cfg(not(windows))]', 1),
                                (b'#[cfg(unix)]', 1),
                                (b'#[cfg(test)]', 1),
                                (b'#[test]', 16),
                                (b'#[cfg(test)]', 1),
                                (b'#[test]', 2)),
                 'build_script': None,
                 'c_preprocessor': ((b'# This file demonstrates how project-level automation can live in Ox'
                                     b'id source files.\n',
                                     1),
                                    (b'#define OXID_BRIDGE_H\n', 1),
                                    (b'#include <stddef.h>\n', 1),
                                    (b'#endif\n', 1),
                                    (b'#include <stdio.h>\n', 1),
                                    (b'#if defined(_WIN32)\n', 1),
                                    (b'#define OXID_POPEN _popen\n', 1),
                                    (b'#define OXID_PCLOSE _pclose\n', 1),
                                    (b'#else\n', 1),
                                    (b'#define OXID_POPEN popen\n', 1),
                                    (b'#define OXID_PCLOSE pclose\n', 1),
                                    (b'#endif\n', 1),
                                    (b'#include <array>\n', 1),
                                    (b'#include <cstdio>\n', 1),
                                    (b'#include <stdexcept>\n', 1),
                                    (b'#include <string>\n', 1),
                                    (b'#if defined(_WIN32)\n', 1),
                                    (b'#else\n', 1),
                                    (b'#endif\n', 1),
                                    (b'#if defined(_WIN32)\n', 1),
                                    (b'#else\n', 1),
                                    (b'#endif\n', 1)),
                 'legacy': ([b'include_str!("../stdlib/frontend/bytecode.ox")',
                             b'include_str!("../compiler/main.ox")',
                             b'include_str!("../compiler/providers.toml")',
                             b'include_str!("../compiler/providers.toml")'],
                            [b'#[path = "runtime/artifact.rs"]',
                             b'#[path = "runtime/data.rs"]',
                             b'#[path = "runtime/network.rs"]',
                             b'#[path = "runtime/packages.rs"]',
                             b'#[path = "runtime/benchmark.rs"]',
                             b'#[path = "legacy/syntax.rs"]']),
                 'macros': ((b'cfg!(windows)', 1),
                            (b'include_str!("../stdlib/frontend/bytecode.ox")', 1),
                            (b'include_str!("../compiler/main.ox")', 1),
                            (b'include_str!("../compiler/providers.toml")', 2))},
 'src/project_cli/workspace.rs': {'attributes': ((b'#[cfg(windows)]', 1),
                                                 (b'#[cfg(not(windows))]', 1),
                                                 (b'#[allow(clippy::too_many_arguments)]', 1)),
                                  'build_script': None,
                                  'c_preprocessor': (),
                                  'legacy': ([], []),
                                  'macros': ()},
 'src/project_lock.rs': {'attributes': ((b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                        (b'#[cfg(windows)]', 1),
                                        (b'#[cfg(not(windows))]', 1),
                                        (b'#[cfg(unix)]', 1),
                                        (b'#[repr(C)]', 1),
                                        (b'#[cfg(windows)]', 1),
                                        (b'#[cfg(not(windows))]', 1),
                                        (b'#[cfg(unix)]', 1),
                                        (b'#[cfg(test)]', 1),
                                        (b'#[test]', 1)),
                         'build_script': None,
                         'c_preprocessor': (),
                         'legacy': ([], []),
                         'macros': ()},
 'tests/typed_documentation.rs': {'attributes': ((b'#[test]', 2),
                                                 (b'#[derive(Copy)]', 8),
                                                 (b'#[test]', 6),
                                                 (b'#[cfg(unix)]', 1),
                                                 (b'#[test]', 1)),
                                  'build_script': None,
                                  'c_preprocessor': (),
                                  'legacy': ([], []),
                                  'macros': ((b'env!("CARGO_BIN_EXE_oxid")', 1),
                                             (b'cfg!(any(\n        target_os = "linux",\n        target_os'
                                              b' = "macos",\n        all(\n            target_os = "window'
                                              b's",\n            target_arch = "x86_64",\n            targ'
                                              b'et_pointer_width = "64",\n            target_endian = "little'
                                              b'"\n        )\n    ))',
                                              1),
                                             (b'cfg!(any(\n        target_os = "linux",\n        target_os'
                                              b' = "macos",\n        all(\n            windows,\n          '
                                              b'  target_arch = "x86_64",\n            target_pointer_width ='
                                              b' "64",\n            target_endian = "little"\n        )\n  '
                                              b'  ))',
                                              1))},
 'tests/typed_generics.rs': {'attributes': ((b'#[test]', 6),
                                            (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width'
                                             b' = "64",\n    target_endian = "little",\n    any(target_arch ='
                                             b' "x86_64", target_arch = "aarch64")\n))]',
                                             1),
                                            (b'#[test]', 1),
                                            (b'#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]',
                                             1),
                                            (b'#[test]', 6),
                                            (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width'
                                             b' = "64",\n    target_endian = "little",\n    any(target_arch ='
                                             b' "x86_64", target_arch = "aarch64")\n))]',
                                             1),
                                            (b'#[test]', 1),
                                            (b'#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]',
                                             1),
                                            (b'#[test]', 15),
                                            (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_width'
                                             b' = "64",\n    target_endian = "little",\n    any(target_arch ='
                                             b' "x86_64", target_arch = "aarch64")\n))]',
                                             1),
                                            (b'#[test]', 1),
                                            (b'#[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]',
                                             1)),
                             'build_script': None,
                             'c_preprocessor': (),
                             'legacy': ([], []),
                             'macros': ((b'cfg!(all(\n            target_os = "windows",\n            target_'
                                         b'arch = "x86_64",\n            target_pointer_width = "64",\n      '
                                         b'      target_endian = "little"\n        ))',
                                         1),
                                        (b'cfg!(any(\n            target_os = "linux",\n            target_os'
                                         b' = "macos",\n            all(\n                windows,\n      '
                                         b'          target_arch = "x86_64",\n                target_pointer'
                                         b'_width = "64",\n                target_endian = "little"\n        '
                                         b'    )\n        ))',
                                         1),
                                        (b'env!("CARGO_BIN_EXE_oxid")', 1),
                                        (b'cfg!(debug_assertions)', 1))},
 'tests/typed_language_server.rs': {'attributes': ((b'#[test]', 2),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 1),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 1),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 2),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 2),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 4),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 2),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 10),
                                                   (b'#[cfg(target_os = "linux")]', 1),
                                                   (b'#[test]', 5),
                                                   (b'#[cfg(any(\n    target_os = "linux",\n    target_os = '
                                                    b'"macos",\n    all(\n        windows,\n        target_ar'
                                                    b'ch = "x86_64",\n        target_pointer_width = "64",\n'
                                                    b'        target_endian = "little"\n    )\n))]',
                                                    1),
                                                   (b'#[test]', 1),
                                                   (b'#[cfg(any(\n    target_os = "linux",\n    target_os = '
                                                    b'"macos",\n    all(\n        windows,\n        target_ar'
                                                    b'ch = "x86_64",\n        target_pointer_width = "64",\n'
                                                    b'        target_endian = "little"\n    )\n))]',
                                                    1),
                                                   (b'#[test]', 2),
                                                   (b'#[derive(Copy)]', 1),
                                                   (b'#[test]', 8)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ((b'env!("CARGO_BIN_EXE_oxid")', 1),
                                               (b'cfg!(any(\n        target_os = "linux",\n        target_os'
                                                b' = "macos",\n        all(\n            target_os = "window'
                                                b's",\n            target_arch = "x86_64",\n            targ'
                                                b'et_pointer_width = "64",\n            target_endian = "li'
                                                b'ttle"\n        )\n    ))',
                                                1),
                                               (b'cfg!(any(\n        target_os = "linux",\n        target_os'
                                                b' = "macos",\n        all(\n            windows,\n          '
                                                b'  target_arch = "x86_64",\n            target_pointer_wid'
                                                b'th = "64",\n            target_endian = "little"\n        '
                                                b')\n    ))',
                                                1),
                                               (b'cfg!(any(\n        target_os = "linux",\n        target_os'
                                                b' = "macos",\n        all(\n            target_os = "window'
                                                b's",\n            target_arch = "x86_64",\n            targ'
                                                b'et_pointer_width = "64",\n            target_endian = "li'
                                                b'ttle"\n        )\n    ))',
                                                1))},
 'tests/typed_package_lock.rs': {'attributes': ((b'#[test]', 9),
                                                (b'#[cfg(unix)]', 1),
                                                (b'#[test]', 1),
                                                (b'#[cfg(unix)]', 1),
                                                (b'#[test]', 2),
                                                (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_w'
                                                 b'idth = "64",\n    target_endian = "little",\n    any(targe'
                                                 b't_arch = "x86_64", target_arch = "aarch64")\n))]',
                                                 1),
                                                (b'#[test]', 1),
                                                (b'#[ignore = "requires pinned LLVM 19.1.7 in OXID_LLVM_BIN'
                                                 b'"]',
                                                 1)),
                                 'build_script': None,
                                 'c_preprocessor': (),
                                 'legacy': ([], []),
                                 'macros': ((b'env!("CARGO_BIN_EXE_oxid")', 1),
                                            (b'cfg!(any(\n            target_os = "linux",\n            targe'
                                             b't_os = "macos",\n            all(\n                windows'
                                             b',\n                target_arch = "x86_64",\n                ta'
                                             b'rget_pointer_width = "64",\n                target_endian = "'
                                             b'little"\n            )\n        ))',
                                             1),
                                            (b'cfg!(all(\n        target_os = "linux",\n        target_pointe'
                                             b'r_width = "64",\n        target_endian = "little",\n        an'
                                             b'y(target_arch = "x86_64", target_arch = "aarch64")\n    ))',
                                             2))},
 'tests/typed_path_dependencies.rs': {'attributes': ((b'#[test]', 13),
                                                     (b'#[cfg(unix)]', 1),
                                                     (b'#[test]', 1),
                                                     (b'#[cfg(unix)]', 1),
                                                     (b'#[test]', 3),
                                                     (b'#[ignore = "requires qualified Linux x86_64 or ARM64'
                                                      b' with LLVM 19.1.7"]',
                                                      1)),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([], []),
                                      'macros': ((b'cfg!(all(\n    target_os = "linux",\n    target_pointe'
                                                  b'r_width = "64",\n    target_endian = "little",\n    an'
                                                  b'y(target_arch = "x86_64", target_arch = "aarch64")\n))',
                                                  1),
                                                 (b'env!("CARGO_BIN_EXE_oxid")', 1),
                                                 (b'cfg!(any(\n        target_os = "linux",\n        targe'
                                                  b't_os = "macos",\n        all(\n            windows,\n  '
                                                  b'          target_arch = "x86_64",\n            target_poi'
                                                  b'nter_width = "64",\n            target_endian = "litt'
                                                  b'le"\n        )\n    ))',
                                                  1),
                                                 (b'cfg!(any(\n            target_os = "linux",\n         '
                                                  b'   target_os = "macos",\n            all(\n           '
                                                  b'     windows,\n                target_arch = "x86_64"'
                                                  b',\n                target_pointer_width = "64",\n     '
                                                  b'           target_endian = "little"\n            )\n  '
                                                  b'      ))',
                                                  1))},
 'tests/typed_windows_modules.rs': {'attributes': ((b'#![cfg(all(\n    target_os = "windows",\n    target_ar'
                                                    b'ch = "x86_64",\n    target_pointer_width = "64",\n    '
                                                    b'target_endian = "little"\n))]',
                                                    1),
                                                   (b'#[test]', 20)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ((b'env!("CARGO_BIN_EXE_oxid")', 2),)},
 'tests/typed_workspace.rs': {'attributes': ((b'#[test]', 4),
                                             (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_w'
                                              b'idth = "64",\n    target_endian = "little",\n    any(targe'
                                              b't_arch = "x86_64", target_arch = "aarch64")\n))]',
                                              1),
                                             (b'#[test]', 11),
                                             (b'#[cfg(target_os = "linux")]', 1),
                                             (b'#[test]', 6),
                                             (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_w'
                                              b'idth = "64",\n    target_endian = "little",\n    any(targe'
                                              b't_arch = "x86_64", target_arch = "aarch64")\n))]',
                                              1),
                                             (b'#[test]', 1),
                                             (b'#[cfg(unix)]', 1),
                                             (b'#[test]', 3),
                                             (b'#[cfg(target_os = "linux")]', 1),
                                             (b'#[test]', 1),
                                             (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_w'
                                              b'idth = "64",\n    target_endian = "little",\n    any(targe'
                                              b't_arch = "x86_64", target_arch = "aarch64")\n))]',
                                              1),
                                             (b'#[test]', 1),
                                             (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_w'
                                              b'idth = "64",\n    target_endian = "little",\n    any(targe'
                                              b't_arch = "x86_64", target_arch = "aarch64")\n))]',
                                              1),
                                             (b'#[test]', 1),
                                             (b'#[ignore = "requires pinned LLVM 19.1.7 in OXID_LLVM_BIN"]',
                                              1),
                                             (b'#[test]', 1),
                                             (b'#[cfg(all(\n    target_os = "linux",\n    target_pointer_w'
                                              b'idth = "64",\n    target_endian = "little",\n    any(targe'
                                              b't_arch = "x86_64", target_arch = "aarch64")\n))]',
                                              1),
                                             (b'#[test]', 1),
                                             (b'#[ignore = "requires pinned LLVM 19.1.7 in OXID_LLVM_BIN"]',
                                              1)),
                              'build_script': None,
                              'c_preprocessor': (),
                              'legacy': ([], []),
                              'macros': ((b'env!("CARGO_BIN_EXE_oxid")', 1),
                                         (b'cfg!(any(\n            target_os = "linux",\n            targe'
                                          b't_os = "macos",\n            all(\n                windows,\n  '
                                          b'              target_arch = "x86_64",\n                target_poi'
                                          b'nter_width = "64",\n                target_endian = "little"\n'
                                          b'            )\n        ))',
                                          1),
                                         (b'cfg!(all(\n        target_os = "linux",\n        target_pointe'
                                          b'r_width = "64",\n        target_endian = "little",\n        an'
                                          b'y(target_arch = "x86_64", target_arch = "aarch64")\n    ))',
                                          3),
                                         (b'cfg!(any(\n        target_os = "linux",\n        target_os = "'
                                          b'macos",\n        all(\n            windows,\n            target'
                                          b'_arch = "x86_64",\n            target_pointer_width = "64",\n '
                                          b'           target_endian = "little"\n        )\n    ))',
                                          1),
                                         (b'env!("CARGO_BIN_EXE_oxid")', 1))}}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents),
            "symlink pinned generic-vector controller")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_PIN[0], "unsafe pinned generic-vector controller")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, name) for name in fields) == tuple(getattr(path.stat(), name) for name in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned generic-vector controller")
    module = types.ModuleType("_windows_local_package_retained_generic")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    return module


predecessor = load_predecessor(ROOT)
library = predecessor.library
owned_policy = predecessor.owned_policy
enum_policy = predecessor.predecessor
RETAINED_RUN = predecessor.RETAINED_RUN
PREDECESSOR_ADDED = predecessor.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | WINDOWS_ADDED
PREDECESSOR_POLICIES = {**predecessor.PREDECESSOR_POLICIES,
                        PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "BINDING", "RETAINED", "RETAINED_RUNNER_SHA",
              "sha", "encoded", "identity", "regular", "git", "source_directives", "directive_manifest"):
    globals()[_name] = getattr(predecessor, _name)


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | WINDOWS_ADDED
    require(set(parent) == expected_parent and len(parent) == 594,
            "wrong pinned Windows local-package parent membership")
    require(len(WINDOWS_CHANGED) == 13 and len(WINDOWS_ADDED) == 7
            and WINDOWS_CHANGED <= expected_parent and not WINDOWS_ADDED & expected_parent
            and len(expected) == 601, "wrong finite Windows local-package source membership")
    require(set(REVIEWED_INPUTS) == WINDOWS_CHANGED | WINDOWS_ADDED
            and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong Windows local-package pin/directive membership")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing compiler source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink compiler source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler and len(compiler) == 500, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    try:
        inputs = {name: regular(repo, name) for name in sorted(expected)}
    except OSError as error:
        raise ValueError("unreadable Windows local-package compiler input: " + str(error)) from error
    for name, before in parent.items():
        if name not in WINDOWS_CHANGED:
            require(inputs[name] == before, "unreviewed change outside Windows local-package boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require((len(inputs[name]), sha(inputs[name])) == (length, digest),
                "Windows local-package reviewed input changed: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "Windows local-package compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    previous_names = set(baseline) | predecessor.PREDECESSOR_ADDED
    inherited = predecessor.current_manifest(inputs=parent, baseline=baseline, checkout=checkout,
                                            parent={name: parent[name] for name in previous_names})
    result = {**inherited, "source_policy_parent": PREDECESSOR,
              "source_policy_inputs": PREDECESSOR_POLICIES,
              "source_policy_input_members": len(PREDECESSOR_POLICIES),
              "predecessor_compiler_input_members": len(parent),
              "windows_local_package_pins_provisional": PINS_PROVISIONAL,
              "windows_local_package_observer_pins_provisional": OBSERVER_PINS_PROVISIONAL,
              "windows_local_package_reviewed_source_head": REVIEWED_SOURCE_HEAD,
              "windows_local_package_controller": identity("scripts/verify_windows_local_package_compatibility.py",
                  regular(ROOT, "scripts/verify_windows_local_package_compatibility.py")),
              "windows_local_package_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                  for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
              "windows_local_package_directive_inventory": [
                  {"path": name, "before": directive_manifest(source_directives(name, parent[name])) if name in parent else None,
                   "after": directive_manifest(REVIEWED_DIRECTIVES[name])} for name in sorted(REVIEWED_INPUTS)],
              "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, value)}
                          for name, value in sorted(inputs.items()) if name in baseline and value != baseline[name]],
              "additions": sorted(PARENT_ADDED),
              "files": [identity(name, value) for name, value in sorted(inputs.items())],
              "unchanged_baseline_inputs": sum(inputs[name] == value for name, value in baseline.items()),
              "windows_local_package_semantics_qualified_by_this_gate": False,
              "windows_local_package_ntfs_qualified_by_this_gate": False,
              "windows_local_package_races_qualified_by_this_gate": False,
              "windows_local_package_platform_qualified_by_this_gate": False}
    require(all(value is False for name, value in result.items() if name.endswith("qualified_by_this_gate")),
            "unexpected new qualification claim")
    return result


@contextmanager
def bound_predecessor():
    bindings = {"PREDECESSOR": PREDECESSOR, "PREDECESSOR_ADDED": PREDECESSOR_ADDED,
                "PREDECESSOR_POLICIES": PREDECESSOR_POLICIES, "PINS_PROVISIONAL": PINS_PROVISIONAL,
                "capture_sources": capture_sources, "current_manifest": current_manifest}
    require(library.run is RETAINED_RUN and predecessor.RETAINED_RUN is RETAINED_RUN
            and owned_policy.RETAINED_RUN is RETAINED_RUN
            and enum_policy.RETAINED_RUN is RETAINED_RUN, "retained execution function was replaced")
    require(predecessor.OBSERVER_ADAPTER == OBSERVER_ADAPTER
            and predecessor.OBSERVER_ADAPTER_PIN == OBSERVER_ADAPTER_PIN,
            "Windows local-package observer must preserve the exact enum adapter")
    with predecessor.bound_predecessor() as context:
        before = {name: getattr(library, name) for name in bindings}
        try:
            for name, value in bindings.items():
                setattr(library, name, value)
            yield context
            require(context["adapter"].DERIVED_PIN == OBSERVER_DERIVED_PIN,
                    "Windows local-package observer output changed")
            require(library.run is RETAINED_RUN, "retained execution function changed")
        finally:
            for name, value in before.items():
                setattr(library, name, value)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_windows_local_package_compatibility.py",
            "run the Windows local-package controller from the selected checkout")
    require(output != repo and repo not in output.parents and not output.exists(), "output must be new outside repository")
    output.mkdir(parents=True)
    controller = regular(repo, "scripts/verify_windows_local_package_compatibility.py")
    report = {"schema": "oxid-windows-local-package-retained-wrapper-v1", "status": "running", "semantic_pass": False,
              "compiler_executions": 0, "controller": identity("scripts/verify_windows_local_package_compatibility.py", controller),
              "source_policy_parent": PREDECESSOR, "inherited_input_members": 594, "expected_current_input_members": 601,
              "pins_provisional": PINS_PROVISIONAL, "observer_pins_provisional": OBSERVER_PINS_PROVISIONAL,
              "reviewed_source_head": REVIEWED_SOURCE_HEAD, "new_feature_qualification": False,
              "platform_qualification": False, "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional Windows local-package source pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not OBSERVER_PINS_PROVISIONAL,
                "provisional Windows local-package observer pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not predecessor.PINS_PROVISIONAL, "provisional predecessor source pins")
        delegated = argparse.Namespace(**vars(args))
        delegated.output = output / "retained"
        with bound_predecessor() as context:
            status = RETAINED_RUN(delegated)
            require(status == 0, "unchanged retained replay refused; inspect retained raw evidence")
            artifact = "prepared.json" if args.prepare_only else "result.json"
            raw = regular(delegated.output, artifact)
            retained = json.loads(raw)
            require(retained.get("status") == ("prepared" if args.prepare_only else "passed")
                    and retained.get("semantic_pass") is (not args.prepare_only)
                    and retained.get("current_input_members") == 601
                    and retained.get("historical_results_apply_to_current") is False
                    and retained.get("controller") == identity(owned_policy.PREDECESSOR_POLICY,
                                                               regular(repo, owned_policy.PREDECESSOR_POLICY)),
                    "retained replay lacks the exact Windows local-package source/materialization binding")
            require(context and context["output"] == delegated.output, "missing unchanged observer preparation context")
            report["windows_local_package_inherited_observer_adapter"] = context["adapter"].descriptor()
            report["windows_local_package_inherited_observer_controller"] = identity(OBSERVER_ADAPTER, regular(repo, OBSERVER_ADAPTER))
            if not args.prepare_only:
                report.update(owned_policy.run_owned_vector_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], retained["test_function_executions"]))
                report.update(enum_policy.run_enum_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], report["test_function_executions"]))
        require(regular(repo, "scripts/verify_windows_local_package_compatibility.py") == controller,
                "Windows local-package controller changed")
        report.update(status="prepared" if args.prepare_only else "passed", semantic_pass=retained["semantic_pass"],
                      compiler_executions=retained["compiler_executions"], current_input_members=retained["current_input_members"],
                      retained_artifact=identity("retained/" + artifact, raw), retained_source=retained["current_source"])
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        report.setdefault("finished_at_utc", datetime.datetime.now(datetime.timezone.utc).isoformat())
        (output / "wrapper.json").write_bytes(encoded(report))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    return run(parser.parse_args())



if __name__ == "__main__":
    raise SystemExit(main())
