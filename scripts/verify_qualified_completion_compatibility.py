#!/usr/bin/env python3
"""Bind qualified-completion source changes to the complete retained R2 replay.

Preparation never executes a compiler or qualifies new editor semantics. Existing
source policies, observer controls and resource predicates remain byte-exact.
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

PREDECESSOR = 'af6460826d2a0abe141f36becd23fdcc2de3e479'
PREDECESSOR_POLICY = 'scripts/verify_copy_record_vector_compatibility.py'
PREDECESSOR_POLICY_PIN = (185301, '87f83b91277eb107d09cf88f618842fb7592ce20ef52f632e3a7254c27a8f16f')
PINS_PROVISIONAL = False
QUALIFIED_COMPLETION_CHANGED = frozenset(['src/frontend/declaration_index.rs',
 'src/frontend/driver.rs',
 'src/frontend/editor/completion.rs',
 'src/frontend/parser.rs',
 'src/frontend/project.rs',
 'src/frontend/project/editor_sources.rs',
 'src/frontend/project/packages.rs',
 'src/lsp_cli.rs'])
QUALIFIED_COMPLETION_ADDED = frozenset(['src/frontend/declaration_index/editor_completion.rs',
 'src/frontend/parser/editor_query.rs',
 'src/frontend/project/module_syntax.rs'])
REVIEWED_INPUTS = {'src/frontend/declaration_index.rs': (87801,
                                       '45533cbd26c856e9ab5942740321ef081026e2568c1abe101dee9a47c60cc515'),
 'src/frontend/declaration_index/editor_completion.rs': (18403,
                                                         '8e7b272267b6866432920bf7ed88f455c7eb697091582235d90727756d565961'),
 'src/frontend/driver.rs': (96881, '6d580baa21caadc4d0638a9ba38102a8dd0dbc1af542b94271037b10c9c4aa8e'),
 'src/frontend/editor/completion.rs': (16306,
                                       '30ebbf65e8583ed4eb81978bcaa9d3b336f7b83f6d73ad5c557f42348e00125d'),
 'src/frontend/parser.rs': (108307, '803734248c2e91b63c9087673af2ced0819706be59479f1a4f1a5658624a0b2d'),
 'src/frontend/parser/editor_query.rs': (23908,
                                         '585296769a07d940b9246e607c56c577556c8259b005599482fc8c4544b3732d'),
 'src/frontend/project.rs': (180186, '37c8c8c9ee6a9e1ff3adc09ca22c1a7da9c6418a290ae737d7ef33e16cf7349a'),
 'src/frontend/project/editor_sources.rs': (7799,
                                            '0ef61e8dff2576c52f2eb30c3567b1757f280d326b9083da7b1c1c054b1ef2ef'),
 'src/frontend/project/module_syntax.rs': (4503,
                                           '3dd5ed58b04c7aa1bb1be4d6b6451ea9300475481dade4b68b42e10140da30c9'),
 'src/frontend/project/packages.rs': (21263,
                                      '9ba4fab788bc120c1b33f24b692c9de18e930d05c784c36712fe7edeca9160f6'),
 'src/lsp_cli.rs': (32674, 'e4273700aa7d16aa1c792da690fb0e9dee5e3fdde64182cf8916e2ed60ad50b7')}
REVIEWED_DIRECTIVES = {'src/frontend/declaration_index.rs': {'attributes': ((b'#![allow(dead_code)]', 1),
                                                      (b'#[cfg(test)]', 7),
                                                      (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 2),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug)]', 1),
                                                      (b'#[repr(C)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug)]', 1),
                                                      (b'#[derive(Debug)]', 2),
                                                      (b'#[allow(dead_code)]', 1),
                                                      (b'#[repr(C, align(8))]', 1),
                                                      (b'#[allow(dead_code)]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug)]', 1),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[cfg(not(test))]', 1),
                                                      (b'#[cfg(test)]', 3),
                                                      (b'#[derive(Clone, Copy, Debug)]', 1),
                                                      (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug)]', 2),
                                                      (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug)]', 1),
                                                      (b'#[cfg(test)]', 4),
                                                      (b'#[derive(Debug)]', 1),
                                                      (b'#[derive(Clone, Copy, Debug)]', 1)),
                                       'build_script': None,
                                       'c_preprocessor': (),
                                       'legacy': ([], []),
                                       'macros': ()},
 'src/frontend/declaration_index/editor_completion.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug)]', 1),
                                                                        (b'#[cfg(test)]', 1),
                                                                        (b'#[test]', 1),
                                                                        (b'#[cfg(target_pointer_width = "64'
                                                                         b'")]',
                                                                         1),
                                                                        (b'#[cfg(any(target_os = "linux", t'
                                                                         b'arget_os = "macos"))]',
                                                                         1),
                                                                        (b'#[test]', 1)),
                                                         'build_script': None,
                                                         'c_preprocessor': (),
                                                         'legacy': ([], []),
                                                         'macros': ()},
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
 'src/frontend/editor/completion.rs': {'attributes': (),
                                       'build_script': None,
                                       'c_preprocessor': (),
                                       'legacy': ([], []),
                                       'macros': ()},
 'src/frontend/parser.rs': {'attributes': ((b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[allow(clippy::enum_variant_names)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 3),
                                           (b'#[derive(Clone, Copy, PartialEq, Eq)]', 1),
                                           (b'#[cfg(test)]', 3),
                                           (b'#[allow(dead_code)]', 1),
                                           (b'#[cfg(test)]', 4),
                                           (b'#[allow(clippy::too_many_arguments)]', 1),
                                           (b'#[derive(Copy)]', 1),
                                           (b'#[path = "parser/arrays.rs"]', 1),
                                           (b'#[path = "parser/characters.rs"]', 1),
                                           (b'#[path = "parser/conversions.rs"]', 1),
                                           (b'#[path = "parser/enums.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/u8_syntax_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/builtin_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/array_syntax_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/enum_syntax_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 4),
                                           (b'#[path = "parser/project_tests.rs"]', 1),
                                           (b'#[cfg(test)]', 1),
                                           (b'#[path = "parser/activation_tests.rs"]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(test)]', 1)),
                            'build_script': None,
                            'c_preprocessor': (),
                            'legacy': ([],
                                       [b'#[path = "parser/arrays.rs"]',
                                        b'#[path = "parser/characters.rs"]',
                                        b'#[path = "parser/conversions.rs"]',
                                        b'#[path = "parser/enums.rs"]',
                                        b'#[path = "parser/u8_syntax_tests.rs"]',
                                        b'#[path = "parser/builtin_tests.rs"]',
                                        b'#[path = "parser/array_syntax_tests.rs"]',
                                        b'#[path = "parser/enum_syntax_tests.rs"]',
                                        b'#[path = "parser/project_tests.rs"]',
                                        b'#[path = "parser/activation_tests.rs"]']),
                            'macros': ()},
 'src/frontend/parser/editor_query.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                        (b'#[derive(Clone, Copy, Debug)]', 1),
                                                        (b'#[derive(Debug)]', 2),
                                                        (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                                        (b'#[cfg(test)]', 1),
                                                        (b'#[test]', 2),
                                                        (b'#[derive(Copy)]', 1),
                                                        (b'#[test]', 1),
                                                        (b'#[cfg(target_pointer_width = "64")]', 1),
                                                        (b'#[test]', 1)),
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
                                            (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 9),
                                            (b'#[derive(Clone, Copy, Debug)]', 1),
                                            (b'#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]', 1),
                                            (b'#[derive(Debug)]', 3),
                                            (b'#[cfg(test)]', 1),
                                            (b'#[cfg_attr(test, allow(clippy::result_large_err))]', 1),
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
                                            (b'#[cfg(not(windows))]', 1),
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
                                                           (b'#[cfg(windows)]', 1)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/frontend/project/module_syntax.rs': {'attributes': ((b'#[allow(clippy::too_many_arguments)]', 1),
                                                          (b'#[cfg(test)]', 4)),
                                           'build_script': None,
                                           'c_preprocessor': (),
                                           'legacy': ([], []),
                                           'macros': ()},
 'src/frontend/project/packages.rs': {'attributes': ((b'#[derive(Clone, Debug)]', 2),
                                                     (b'#[derive(Debug)]', 2),
                                                     (b'#[derive(Clone, Copy, Debug, Default)]', 1),
                                                     (b'#[allow(clippy::result_large_err)]', 3),
                                                     (b'#[cfg(test)]', 3),
                                                     (b'#[allow(dead_code)]', 1),
                                                     (b'#[test]', 1)),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([], []),
                                      'macros': ((b'cfg!(any(target_os = "linux", target_os = "macos"))',
                                                  1),)},
 'src/lsp_cli.rs': {'attributes': (),
                    'build_script': None,
                    'c_preprocessor': (),
                    'legacy': ([], []),
                    'macros': ((b'env!("CARGO_PKG_VERSION")', 1),)}}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and all(not parent.is_symlink() for parent in path.parents),
            "symlink pinned R2 policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned R2 policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned R2 policy")
    module = types.ModuleType("_qualified_completion_retained_R2")
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
PARENT_ADDED = PREDECESSOR_ADDED | QUALIFIED_COMPLETION_ADDED
PREDECESSOR_POLICIES = {
    **library.PREDECESSOR_POLICIES,
    PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1],
    library.OBSERVER_ADAPTER: library.OBSERVER_ADAPTER_PIN[1],
    library.DISPATCH_ADAPTER: library.DISPATCH_ADAPTER_PIN[1],
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
    expected = expected_parent | QUALIFIED_COMPLETION_ADDED
    require(set(parent) == expected_parent and len(parent) == 503,
            "wrong R2 compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (QUALIFIED_COMPLETION_CHANGED | QUALIFIED_COMPLETION_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == QUALIFIED_COMPLETION_CHANGED | QUALIFIED_COMPLETION_ADDED
            and QUALIFIED_COMPLETION_CHANGED <= expected_parent
            and not QUALIFIED_COMPLETION_ADDED & expected_parent,
            "wrong reviewed qualified-completion input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed qualified-completion directive membership")
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
        if name not in QUALIFIED_COMPLETION_CHANGED:
            require(inputs[name] == before,
                    "unreviewed change outside qualified-completion boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "qualified-completion source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "qualified-completion compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    # Read the frozen R1 claim inventory without reapplying the R2 Windows
    # activation inverse to an already qualified R2 predecessor. No historical
    # source admission, resource formula or prior result is reused as evidence.
    inherited = library.library.current_manifest(parent, baseline, checkout, parent)
    result = {key: value for key, value in inherited.items()
              if key.endswith("qualified_by_this_gate")}
    result.update({
        "schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
        "checkout": checkout, "assertion_scope": "retained bounded scalar/record semantics and resources",
        "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
        "source_policy_input_members": len(PREDECESSOR_POLICIES),
        "predecessor_compiler_input_members": len(parent),
        "qualified_completion_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: identity(name, parent[name])
                                      for name in sorted(UNCHANGED_INPUTS)},
        "qualified_completion_reviewed_inputs": [
            {"path": name, "bytes": length, "sha256": digest}
            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "qualified_completion_changes": [
            {"path": name, "before": identity(name, parent[name]),
             "after": identity(name, inputs[name])}
            for name in sorted(QUALIFIED_COMPLETION_CHANGED)],
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
    require(Path(__file__).resolve() == repo / "scripts/verify_qualified_completion_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources; qualified module completion and new language/platform semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_qualified_completion_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional qualified-completion pins permit prepare-only admission, not qualification")
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
