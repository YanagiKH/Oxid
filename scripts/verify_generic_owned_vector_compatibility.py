#!/usr/bin/env python3
"""Bind generic vector ownership inputs to the unchanged retained replay.

All inherited semantic/resource checks, observer bytes and debug/release control
executions remain unchanged. New generic behavior is qualified separately.
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
PREDECESSOR = '698043182916dda7276c363e20a98dfbb5b22ed3'
PREDECESSOR_POLICY = 'scripts/verify_owned_enum_vector_compatibility.py'
PREDECESSOR_POLICY_PIN = (67623, '3505f8238f1d24754916075fb1ad46aa022c740cd71c9f10a5509656b7e9877c')
REVIEWED_SOURCE_HEAD = '355ebf61731f15f4881a0b6e27eced81e1b2d311'
PINS_PROVISIONAL = True
OBSERVER_PINS_PROVISIONAL = True
OBSERVER_ADAPTER = 'scripts/owned_enum_vector_compatibility_observer.py'
OBSERVER_ADAPTER_PIN = (7495, 'ff932fb13ae4db32fe15a389505be7a7ffa2fc0557b15f9b1b153c3602de3fd9')
OBSERVER_DERIVED_PIN = (34582, '73b30bca73d10126977744d50fb4cc42f6dc1cb876c5ee82b7335f3ffc14a3d3')
PUBLIC_TEST = 'tests/typed_owned_vector_wrappers.rs'

GENERIC_CHANGED = frozenset(('src/frontend/declaration_index/generics.rs',
 'src/frontend/declaration_index/generics/nominal.rs',
 'src/frontend/declaration_index/generics/owned_builtin_enum_key_tests.rs',
 'src/frontend/declaration_index/generics/owned_candidate.rs',
 'src/frontend/declaration_index/generics/vector_tests.rs',
 'src/frontend/oir/owned/source/program.rs',
 'src/frontend/owned_vector_builtins.rs'))

GENERIC_ADDED = frozenset(('src/frontend/declaration_index/generics/owned_parameter_tests.rs',
 'src/frontend/oir/owned/source/owned_parameter_cases.rs',
 'src/frontend/oir/owned/source/owned_parameter_source_tests.rs'))

REVIEWED_INPUTS = {'src/frontend/declaration_index/generics.rs': (139412,
                                                '836bc587ef4a79511637a4fc2069309f166362d5a73be2c3c88864aef0e6ef28'),
 'src/frontend/declaration_index/generics/nominal.rs': (39177,
                                                        'ef00c7eff775a2c3867ae82da7d0d5a2528777c50e0a53634465424ba72bdbd8'),
 'src/frontend/declaration_index/generics/owned_builtin_enum_key_tests.rs': (30834,
                                                                             'f470597e376a68bf1877996fccbeb25fb3fb26e5c80df581f7455518a5110ddb'),
 'src/frontend/declaration_index/generics/owned_candidate.rs': (88618,
                                                                '119af91f05f2c30a269ee7ae5d69656c77db763fb5f9ef3dd3b517d60c951639'),
 'src/frontend/declaration_index/generics/owned_parameter_tests.rs': (18803,
                                                                      'e7c1a76c2c0832f4e462f8b655401548521a4a3036c885f8c82ba744767acbce'),
 'src/frontend/declaration_index/generics/vector_tests.rs': (23567,
                                                             '9cb9b3cb8cfe58e76c9f975ad4581caed73891902477b67c614094e55f1d6c43'),
 'src/frontend/oir/owned/source/owned_parameter_cases.rs': (10025,
                                                            '48699b26dd1424940315d43620966e3182e7ba13030029b53c91bbacc8c30c5a'),
 'src/frontend/oir/owned/source/owned_parameter_source_tests.rs': (14765,
                                                                   'cfe282b44e6b7469319fd4283f75afd67a60ca9e8fc95cc31ce439b5276fd695'),
 'src/frontend/oir/owned/source/program.rs': (51393,
                                              'd804567265f797afc5d5a30b06f37d9c1683df79f924d0825e7f8397c39c0386'),
 'src/frontend/owned_vector_builtins.rs': (12537,
                                           '483750c76ceca5e7e708076a91737810c647510feefb364d33110f1cdbceff5f'),
 'tests/typed_owned_vector_wrappers.rs': (18544,
                                          'd901d072461e6ba4277f4bad104bda02a0a33d2ce346e4918a608fd1d83b0f3e')}

SOURCE_ASSETS = {'examples/owned-vector-results/src/main.ox': (2807,
                                               'ae7a756cb72d1d95b7df94d04ec75b98501895f60445bf8c8cea7443fd37e1ea'),
 'packages/owned_vec/oxid.toml': (107, 'f90e59709351dfec7405b89f28d12177087e872b542e4d37e8e964382c469b53'),
 'packages/owned_vec/src/lib.ox': (1681, '735557ebc26493915e2a5ea7a18c04378e341db71162746fe741521f6cd4d5da')}

SOURCE_ASSET_INCLUDE_BINDINGS = {'examples/owned-vector-results/src/main.ox': ('src/frontend/oir/owned/source/owned_parameter_cases.rs',
                                               '../../../../../examples/owned-vector-results/src/main.ox',
                                               1),
 'packages/owned_vec/oxid.toml': ('tests/typed_owned_vector_wrappers.rs', '../packages/owned_vec/oxid.toml', 1),
 'packages/owned_vec/src/lib.ox': ('src/frontend/oir/owned/source/owned_parameter_cases.rs',
                                   '../../../../../packages/owned_vec/src/lib.ox',
                                   1)}

REVIEWED_DIRECTIVES = {'src/frontend/declaration_index/generics.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                                               (b'#[derive(Default)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 1),
                                                               (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]',
                                                                3),
                                                               (b'#[allow(dead_code)]', 1),
                                                               (b'#[derive(Debug, PartialEq, Eq)]', 3),
                                                               (b'#[allow(clippy::too_many_arguments)]', 1),
                                                               (b'#[derive(Clone, Copy)]', 2),
                                                               (b'#[allow(clippy::too_many_arguments)]', 1),
                                                               (b'#[cfg(test)]', 1)),
                                                'build_script': None,
                                                'c_preprocessor': (),
                                                'legacy': ([], []),
                                                'macros': ()},
 'src/frontend/declaration_index/generics/nominal.rs': {'attributes': ((b'#[derive(Debug, PartialEq, Eq)]', 2),
                                                                       (b'#[derive(Debug, Default, PartialEq, Eq)]',
                                                                        1),
                                                                       (b'#[allow(dead_code)]', 1),
                                                                       (b'#[allow(clippy::too_many_arguments)]',
                                                                        1),
                                                                       (b'#[cfg(test)]', 1),
                                                                       (b'#[allow(clippy::too_many_arguments)]',
                                                                        3)),
                                                        'build_script': None,
                                                        'c_preprocessor': (),
                                                        'legacy': ([], []),
                                                        'macros': ()},
 'src/frontend/declaration_index/generics/owned_builtin_enum_key_tests.rs': {'attributes': ((b'#[test]', 1),
                                                                                            (b'#[cfg(target_poi'
                                                                                             b'nter_width = "64'
                                                                                             b'")]',
                                                                                             1),
                                                                                            (b'#[test]', 4),
                                                                                            (b'#[cfg(target_poi'
                                                                                             b'nter_width = "64'
                                                                                             b'")]',
                                                                                             1),
                                                                                            (b'#[test]', 1),
                                                                                            (b'#[allow(dead_cod'
                                                                                             b'e)]',
                                                                                             7)),
                                                                             'build_script': None,
                                                                             'c_preprocessor': (),
                                                                             'legacy': ([], []),
                                                                             'macros': ()},
 'src/frontend/declaration_index/generics/owned_candidate.rs': {'attributes': ((b'#[path = "owned_builtin_enum_key'
                                                                                b's.rs"]',
                                                                                1),
                                                                               (b'#[cfg(test)]', 1),
                                                                               (b'#[path = "owned_parameter_tests.'
                                                                                b'rs"]',
                                                                                1),
                                                                               (b'#[derive(Debug, PartialEq, Eq)]',
                                                                                1),
                                                                               (b'#[repr(C)]', 1),
                                                                               (b'#[derive(Debug)]', 2),
                                                                               (b'#[derive(Debug, PartialEq, Eq)]',
                                                                                1),
                                                                               (b'#[allow(dead_code)]', 5),
                                                                               (b'#[cfg(test)]', 3),
                                                                               (b'#[allow(dead_code)]', 1),
                                                                               (b'#[allow(clippy::too_many_argumen'
                                                                                b'ts)]',
                                                                                4),
                                                                               (b'#[cfg(test)]', 1),
                                                                               (b'#[test]', 14),
                                                                               (b'#[allow(dead_code)]', 10),
                                                                               (b'#[test]', 1)),
                                                                'build_script': None,
                                                                'c_preprocessor': (),
                                                                'legacy': ([],
                                                                           [b'#[path = "owned_builtin_enum_keys.rs'
                                                                            b'"]',
                                                                            b'#[path = "owned_parameter_tests.rs"]']),
                                                                'macros': ()},
 'src/frontend/declaration_index/generics/owned_parameter_tests.rs': {'attributes': ((b'#[test]', 8),
                                                                                     (b'#[derive(Copy)]', 1),
                                                                                     (b'#[test]', 1)),
                                                                      'build_script': None,
                                                                      'c_preprocessor': (),
                                                                      'legacy': ([], []),
                                                                      'macros': ()},
 'src/frontend/declaration_index/generics/vector_tests.rs': {'attributes': ((b'#[test]', 13),),
                                                             'build_script': None,
                                                             'c_preprocessor': (),
                                                             'legacy': ([], []),
                                                             'macros': ()},
 'src/frontend/oir/owned/source/owned_parameter_cases.rs': {'attributes': ((b'#[derive(Copy)]', 1),
                                                                           (b'#[cfg(all(\n    any(target_os = "'
                                                                            b'linux", target_os = "macos"),\n  '
                                                                            b'  target_pointer_width = "64",\n '
                                                                            b'   target_endian = "little",\n   '
                                                                            b' any(target_arch = "x86_64", target_'
                                                                            b'arch = "aarch64")\n))]',
                                                                            1)),
                                                            'build_script': None,
                                                            'c_preprocessor': (),
                                                            'legacy': ([b'include_str!("../../../../../packages/ow'
                                                                        b'ned_vec/src/lib.ox")',
                                                                        b'include_str!("../../../../../examples/ow'
                                                                        b'ned-vector-results/src/main.ox")'],
                                                                       []),
                                                            'macros': ((b'include_str!("../../../../../packages/ow'
                                                                        b'ned_vec/src/lib.ox")',
                                                                        1),
                                                                       (b'include_str!("../../../../../examples/ow'
                                                                        b'ned-vector-results/src/main.ox")',
                                                                        1))},
 'src/frontend/oir/owned/source/owned_parameter_source_tests.rs': {'attributes': ((b'#[test]', 3),
                                                                                  (b'#[cfg(all(\n    any(target_os'
                                                                                   b' = "linux", target_os = "mac'
                                                                                   b'os"),\n    target_pointer_wid'
                                                                                   b'th = "64",\n    target_endian'
                                                                                   b' = "little",\n    any(target_'
                                                                                   b'arch = "x86_64", target_arch'
                                                                                   b' = "aarch64")\n))]',
                                                                                   1),
                                                                                  (b'#[test]', 3)),
                                                                   'build_script': None,
                                                                   'c_preprocessor': (),
                                                                   'legacy': ([], []),
                                                                   'macros': ()},
 'src/frontend/oir/owned/source/program.rs': {'attributes': ((b'#[cfg(test)]', 2),
                                                             (b'#[path = "experimental_source_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "i64_source_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "owned_enum_vector_source_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "owned_parameter_source_tests.rs"]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "owned_text_source_resource_tests.rs"]',
                                                              1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "owned_vector_source_execution_tests.rs'
                                                              b'"]',
                                                              1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[path = "owned_vector_source_facade_tests.rs"]',
                                                              1),
                                                             (b'#[derive(Debug)]', 3),
                                                             (b'#[test]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 5),
                                                             (b'#[path = "candidate_adapter.rs"]', 1),
                                                             (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[derive(Debug)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[allow(dead_code)]', 1),
                                                             (b'#[cfg(test)]', 2),
                                                             (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
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
                                                          b'#[path = "owned_enum_vector_source_tests.rs"]',
                                                          b'#[path = "owned_parameter_source_tests.rs"]',
                                                          b'#[path = "owned_text_source_resource_tests.rs"]',
                                                          b'#[path = "owned_vector_source_execution_tests.rs"]',
                                                          b'#[path = "owned_vector_source_facade_tests.rs"]',
                                                          b'#[path = "candidate_adapter.rs"]']),
                                              'macros': ()},
 'src/frontend/owned_vector_builtins.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialO'
                                                           b'rd, Ord)]',
                                                           1),
                                                          (b'#[repr(u8)]', 1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[path = "oir/owned/source/owned_enum_vector_cases.r'
                                                           b's"]',
                                                           1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[path = "owned_enum_vector_catalog_tests.rs"]', 1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[path = "oir/owned/source/owned_parameter_cases.rs"'
                                                           b']',
                                                           1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                          (b'#[repr(u8)]', 1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 3),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialO'
                                                           b'rd, Ord)]',
                                                           1),
                                                          (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                          (b'#[cfg(test)]', 1),
                                                          (b'#[test]', 3)),
                                           'build_script': None,
                                           'c_preprocessor': (),
                                           'legacy': ([],
                                                      [b'#[path = "oir/owned/source/owned_enum_vector_cases.rs"]',
                                                       b'#[path = "owned_enum_vector_catalog_tests.rs"]',
                                                       b'#[path = "oir/owned/source/owned_parameter_cases.rs"]']),
                                           'macros': ()},
 'tests/typed_owned_vector_wrappers.rs': {'attributes': ((b'#[path = "../src/frontend/oir/owned/source/owned_par'
                                                          b'ameter_cases.rs"]',
                                                          1),
                                                         (b'#[cfg(any(target_os = "linux", target_os = "macos"))]',
                                                          1),
                                                         (b'#[test]', 2),
                                                         (b'#[cfg(any(target_os = "linux", target_os = "macos"))]',
                                                          1),
                                                         (b'#[test]', 1),
                                                         (b'#[cfg(all(\n    any(target_os = "linux", target_os = '
                                                          b'"macos"),\n    target_pointer_width = "64",\n    targe'
                                                          b't_endian = "little",\n    any(target_arch = "x86_64",'
                                                          b' target_arch = "aarch64")\n))]',
                                                          1),
                                                         (b'#[test]', 3),
                                                         (b'#[cfg(any(target_os = "linux", target_os = "macos"))]',
                                                          1),
                                                         (b'#[test]', 1),
                                                         (b'#[cfg(any(target_os = "linux", target_os = "macos"))]',
                                                          1),
                                                         (b'#[test]', 3),
                                                         (b'#[derive(Copy)]', 1),
                                                         (b'#[cfg(all(\n    any(target_os = "linux", target_os = '
                                                          b'"macos"),\n    target_pointer_width = "64",\n    targe'
                                                          b't_endian = "little",\n    any(target_arch = "x86_64",'
                                                          b' target_arch = "aarch64")\n))]',
                                                          1),
                                                         (b'#[test]', 1),
                                                         (b'#[ignore = "requires LLVM 19.1.7 on a supported Linu'
                                                          b'x or macOS host"]',
                                                          1)),
                                          'build_script': None,
                                          'c_preprocessor': (),
                                          'legacy': ([b'include_str!("../packages/owned_vec/oxid.toml")'],
                                                     [b'#[path = "../src/frontend/oir/owned/source/owned_paramet'
                                                      b'er_cases.rs"]']),
                                          'macros': ((b'include_str!("../packages/owned_vec/oxid.toml")', 1),
                                                     (b'env!("CARGO_BIN_EXE_oxid")', 1))}}

def require(condition, message):
    if not condition:
        raise ValueError(message)

def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents),
            "symlink pinned enum-vector controller")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_PIN[0], "unsafe pinned enum-vector controller")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, name) for name in fields) == tuple(getattr(path.stat(), name) for name in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned enum-vector controller")
    module = types.ModuleType("_generic_vector_retained_enum")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    return module

predecessor = load_predecessor(ROOT)
library = predecessor.library
owned_policy = predecessor.owned_policy
RETAINED_RUN = predecessor.RETAINED_RUN
PREDECESSOR_ADDED = predecessor.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | GENERIC_ADDED | frozenset(SOURCE_ASSETS) | {PUBLIC_TEST}
PREDECESSOR_POLICIES = {**predecessor.PREDECESSOR_POLICIES,
                        PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "BINDING", "RETAINED", "RETAINED_RUNNER_SHA",
              "sha", "encoded", "identity", "regular", "git", "source_directives", "directive_manifest"):
    globals()[_name] = getattr(predecessor, _name)


def validate_source_assets(inputs):
    require(set(SOURCE_ASSETS) == {
        "packages/owned_vec/src/lib.ox", "examples/owned-vector-results/src/main.ox",
        "packages/owned_vec/oxid.toml"}, "wrong finite generic-vector asset membership")
    require(set(SOURCE_ASSET_INCLUDE_BINDINGS) == set(SOURCE_ASSETS),
            "wrong generic-vector asset binding membership")
    for name, (length, digest) in SOURCE_ASSETS.items():
        require((len(inputs[name]), sha(inputs[name])) == (length, digest),
                "generic-vector compiled asset changed: " + name)
        owner, relative, count = SOURCE_ASSET_INCLUDE_BINDINGS[name]
        parts = list(PurePosixPath(owner).parent.parts)
        for part in PurePosixPath(relative).parts:
            if part == "..":
                require(parts, "generic-vector asset include escapes repository")
                parts.pop()
            else:
                parts.append(part)
        included = ('include_str!("' + relative + '")').encode()
        require(PurePosixPath(*parts).as_posix() == name and count == 1
                and owner in REVIEWED_INPUTS and inputs[owner].count(included) == count,
                "generic-vector compiled asset include binding changed: " + name)


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | GENERIC_ADDED | set(SOURCE_ASSETS) | {PUBLIC_TEST}
    require(set(parent) == expected_parent and len(parent) == 587,
            "wrong pinned enum-vector parent membership")
    require(len(GENERIC_CHANGED) == 7 and len(GENERIC_ADDED) == 3
            and GENERIC_CHANGED <= expected_parent and not GENERIC_ADDED & expected_parent
            and not set(SOURCE_ASSETS) & expected_parent and PUBLIC_TEST not in expected_parent
            and len(expected) == 594, "wrong finite generic-vector source membership")
    require(set(REVIEWED_INPUTS) == GENERIC_CHANGED | GENERIC_ADDED | {PUBLIC_TEST}
            and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong generic-vector pin/directive membership")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing compiler source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink compiler source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    try:
        inputs = {name: regular(repo, name) for name in sorted(expected)}
    except OSError as error:
        raise ValueError("unreadable generic-vector compiler input: " + str(error)) from error
    for name, before in parent.items():
        if name not in GENERIC_CHANGED:
            require(inputs[name] == before, "unreviewed change outside generic-vector boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require((len(inputs[name]), sha(inputs[name])) == (length, digest),
                "generic-vector reviewed input changed: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "generic-vector compile-time input directive changed: " + name)
    validate_source_assets(inputs)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    previous_names = set(baseline) | predecessor.PREDECESSOR_ADDED
    inherited = predecessor.current_manifest(inputs=parent, baseline=baseline, checkout=checkout,
                                            parent={name: parent[name] for name in previous_names})
    result = {**inherited, "source_policy_parent": PREDECESSOR,
              "source_policy_inputs": PREDECESSOR_POLICIES,
              "source_policy_input_members": len(PREDECESSOR_POLICIES),
              "predecessor_compiler_input_members": len(parent),
              "generic_owned_vector_pins_provisional": PINS_PROVISIONAL,
              "generic_owned_vector_observer_pins_provisional": OBSERVER_PINS_PROVISIONAL,
              "generic_owned_vector_reviewed_source_head": REVIEWED_SOURCE_HEAD,
              "generic_owned_vector_controller": identity("scripts/verify_generic_owned_vector_compatibility.py",
                  regular(ROOT, "scripts/verify_generic_owned_vector_compatibility.py")),
              "generic_owned_vector_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                  for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
              "generic_owned_vector_directive_inventory": [
                  {"path": name, "before": directive_manifest(source_directives(name, parent[name])) if name in parent else None,
                   "after": directive_manifest(REVIEWED_DIRECTIVES[name])} for name in sorted(REVIEWED_INPUTS)],
              "generic_owned_vector_compiled_assets": [
                  {**identity(name, inputs[name]), "included_by": SOURCE_ASSET_INCLUDE_BINDINGS[name][0],
                   "relative_include": SOURCE_ASSET_INCLUDE_BINDINGS[name][1],
                   "include_occurrences": SOURCE_ASSET_INCLUDE_BINDINGS[name][2]} for name in sorted(SOURCE_ASSETS)],
              "generic_owned_vector_public_test": identity(PUBLIC_TEST, inputs[PUBLIC_TEST]),
              "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, value)}
                          for name, value in sorted(inputs.items()) if name in baseline and value != baseline[name]],
              "additions": sorted(PARENT_ADDED),
              "files": [identity(name, value) for name, value in sorted(inputs.items())],
              "unchanged_baseline_inputs": sum(inputs[name] == value for name, value in baseline.items()),
              "generic_owned_vector_semantics_qualified_by_this_gate": False,
              "generic_owned_vector_allocator_failures_qualified_by_this_gate": False,
              "generic_owned_vector_platform_qualified_by_this_gate": False,
              "general_heap_collections_qualified_by_this_gate": False,
              "g2_complete_qualified_by_this_gate": False}
    require(all(value is False for name, value in result.items() if name.endswith("qualified_by_this_gate")),
            "unexpected new qualification claim")
    return result


@contextmanager
def bound_predecessor():
    bindings = {"PREDECESSOR": PREDECESSOR, "PREDECESSOR_ADDED": PREDECESSOR_ADDED,
                "PREDECESSOR_POLICIES": PREDECESSOR_POLICIES, "PINS_PROVISIONAL": PINS_PROVISIONAL,
                "capture_sources": capture_sources, "current_manifest": current_manifest}
    require(library.run is RETAINED_RUN and predecessor.RETAINED_RUN is RETAINED_RUN
            and owned_policy.RETAINED_RUN is RETAINED_RUN, "retained execution function was replaced")
    require(predecessor.OBSERVER_ADAPTER == OBSERVER_ADAPTER
            and predecessor.OBSERVER_ADAPTER_PIN == OBSERVER_ADAPTER_PIN,
            "generic-vector observer must preserve the exact enum adapter")
    with predecessor.bound_predecessor() as context:
        before = {name: getattr(library, name) for name in bindings}
        try:
            for name, value in bindings.items():
                setattr(library, name, value)
            yield context
            require(context["adapter"].DERIVED_PIN == OBSERVER_DERIVED_PIN,
                    "generic-vector observer output changed")
            require(library.run is RETAINED_RUN, "retained execution function changed")
        finally:
            for name, value in before.items():
                setattr(library, name, value)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_generic_owned_vector_compatibility.py",
            "run the generic-vector controller from the selected checkout")
    require(output != repo and repo not in output.parents and not output.exists(), "output must be new outside repository")
    output.mkdir(parents=True)
    controller = regular(repo, "scripts/verify_generic_owned_vector_compatibility.py")
    report = {"schema": "oxid-owned-generic-vector-retained-wrapper-v1", "status": "running", "semantic_pass": False,
              "compiler_executions": 0, "controller": identity("scripts/verify_generic_owned_vector_compatibility.py", controller),
              "source_policy_parent": PREDECESSOR, "inherited_input_members": 587, "expected_current_input_members": 594,
              "pins_provisional": PINS_PROVISIONAL, "observer_pins_provisional": OBSERVER_PINS_PROVISIONAL,
              "reviewed_source_head": REVIEWED_SOURCE_HEAD, "new_feature_qualification": False,
              "platform_qualification": False, "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional generic-vector source pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not OBSERVER_PINS_PROVISIONAL,
                "provisional generic-vector observer pins permit prepare-only admission, not qualification")
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
                    and retained.get("current_input_members") == 594
                    and retained.get("historical_results_apply_to_current") is False
                    and retained.get("controller") == identity(owned_policy.PREDECESSOR_POLICY,
                                                               regular(repo, owned_policy.PREDECESSOR_POLICY)),
                    "retained replay lacks the exact generic-vector source/materialization binding")
            require(context and context["output"] == delegated.output, "missing unchanged observer preparation context")
            report["generic_vector_inherited_observer_adapter"] = context["adapter"].descriptor()
            report["generic_vector_inherited_observer_controller"] = identity(OBSERVER_ADAPTER, regular(repo, OBSERVER_ADAPTER))
            if not args.prepare_only:
                report.update(owned_policy.run_owned_vector_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], retained["test_function_executions"]))
                report.update(predecessor.run_enum_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], report["test_function_executions"]))
        require(regular(repo, "scripts/verify_generic_owned_vector_compatibility.py") == controller,
                "generic-vector controller changed")
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
