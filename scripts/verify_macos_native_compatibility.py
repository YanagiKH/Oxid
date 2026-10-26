#!/usr/bin/env python3
"""Replay retained bounded assertions after macOS native support.

The reviewed delta preserves all other compiler inputs and preceding policies.
macOS native support is qualified separately through public tests.
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

PREDECESSOR = 'b40dfdf471331863b1b77204f95aa6263b18172d'
PREDECESSOR_POLICY = 'scripts/verify_scalar_debug_storage_compatibility.py'
PREDECESSOR_POLICY_SHA = 'a33e700c9b9a5fe5673ddd97e0f5a0f3a764db294bd6194b07b77b168b042487'
PREDECESSOR_POLICY_BYTES = 20564
MACOS_NATIVE_CHANGED = frozenset({'src/frontend/oir/owned/byte_storage_native_tests.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/source/output_lower_tests.rs', 'src/frontend/project/filesystem.rs', 'src/frontend/oir/source/hir_import/verify_fact_tests.rs', 'src/frontend/oir/owned/process.rs', 'src/frontend/oir/owned/source/output_typing_tests.rs', 'src/frontend/declaration_index/builtin_tests.rs', 'src/frontend/oir/project_execution_tests.rs', 'src/frontend/project/tests.rs', 'src/frontend/oir/source/hir_import/emit_failure_tests.rs', 'src/frontend/driver.rs', 'src/frontend/project/packages/tests.rs', 'src/frontend/project/unit2_tests.rs', 'src/frontend/build_config.rs', 'src/frontend/oir/owned/file_resources.rs', 'src/frontend/stdin_public_tests.rs', 'src/frontend/project/enum_index_tests.rs', 'src/frontend/oir/source/hir_import/emit_tests.rs', 'src/frontend/lexical_provider/tests.rs', 'src/frontend/project/packages.rs', 'src/frontend/project/array_syntax_tests.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/owned/source/output_source_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/output.rs', 'src/frontend/oir/owned/enum_index_allocation_tests.rs', 'src/frontend/oir/owned/binary_io.rs', 'native/experimental_files.c', 'src/frontend/native/doctor.rs', 'src/frontend/native.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/project_cli.rs', 'src/frontend/oir/owned/input.rs', 'build.rs', 'src/frontend/oir/owned/source/enum_type_tests.rs', 'tests/typed_project_dispatch.rs'})
MACOS_NATIVE_ADDED = frozenset()
MACOS_NATIVE_INPUTS = {'build.rs': (1896, 'a079492b9533e8283959b87048019e3ac90cbdf0550ca527322bae16c5075539'),
 'native/experimental_files.c': (7168,
                                 'd2875afe16a251bdae632b41ac05f5f7ef0c2447bdec4f24e9b34554661c14e4'),
 'src/frontend/build_config.rs': (23904,
                                  'eee71ee31d0225806f95131572753779ea46a809608f7904b30cf089f5c05e95'),
 'src/frontend/declaration_index/builtin_tests.rs': (80160,
                                                     '8f01fd5d18c97d5700ba121b8b42a8e4f3b99e668843f9f232b8952948af0ad6'),
 'src/frontend/driver.rs': (93477,
                            'b07558a88bbd1861fe7e8e9ad99bce0a101eb86b0e293b069914a819b8d818aa'),
 'src/frontend/lexical_provider/tests.rs': (45387,
                                            '6665e1ccc7fea1671e13342501cb924ee834a8acc3576a3e27f52f2538a03bb8'),
 'src/frontend/native.rs': (21205,
                            'a10c13bbb3b66a1938ea4bda534f143a4a29563a08e880b60bf33fa2e1ccc8b8'),
 'src/frontend/native/doctor.rs': (21693,
                                   '7387d816329ea550590e2c09da941cac4e9b7f4bfed20bf4599acd81c2fa8688'),
 'src/frontend/oir/native.rs': (125129,
                                'd11918359634bd83e9880db1191dc1fcc15987387a0e45fadbe841f5f84fc4a0'),
 'src/frontend/oir/owned/binary_io.rs': (13857,
                                         '4631b6895dc39d1ae58f64e0c14394e0d249d7d89eef40600454fe249afeca31'),
 'src/frontend/oir/owned/byte_storage_native_tests.rs': (66435,
                                                         'e3a9b1af06da6e530974e3128535840cf3606348b353848dd67bc0e214cd62de'),
 'src/frontend/oir/owned/enum_index_allocation_tests.rs': (30222,
                                                           'b7b7015fe3e6dbf7fc4a7d530fcefd7a7ba8299c89b20d85a605ec856c164432'),
 'src/frontend/oir/owned/execute.rs': (209721,
                                       'de2447e941fd9944370dd5765c1376844e89a3b6622dac534eebc0b28622701f'),
 'src/frontend/oir/owned/file_resources.rs': (14337,
                                              '04649564ffaa0f2c26b41913b28641084c499322b9c37d4d4ad1cc02401cc6c1'),
 'src/frontend/oir/owned/input.rs': (1880,
                                     '9e2c350eb09d6eb5802b40d69524f2e79c60810bcf143e491c44b4e338fa8d45'),
 'src/frontend/oir/owned/native.rs': (263369,
                                      '519e05d034bb31fb6405fe4220d3aff7650731ab710b2c7b261554aaa5460b73'),
 'src/frontend/oir/owned/output.rs': (3015,
                                      'a6e9e8d76df32c2914af61acb53680c822639275c0adfaa28bcd9b9ab166c2bc'),
 'src/frontend/oir/owned/process.rs': (3239,
                                       'ac4fb5d363e89dbaa368565471b2d6e7a898bb245ec667ce7c139113ff6e5eae'),
 'src/frontend/oir/owned/source/enum_type_tests.rs': (22000,
                                                      'dda5ad708786dd97466cfc500acdca2642df814b7224e79731b1a88fbf7de98a'),
 'src/frontend/oir/owned/source/output_lower_tests.rs': (31987,
                                                         '5bf857b1f211f53bdabcdac4477b8d1695b30ccbcfba90b0cc8e2e5dac47f18a'),
 'src/frontend/oir/owned/source/output_source_tests.rs': (22286,
                                                          'b89af3ba7c7cf46bb3559dd189869d0b4cd2382050cfcbf3c617c997364765d9'),
 'src/frontend/oir/owned/source/output_typing_tests.rs': (44502,
                                                          'aea2867b337397ddc5b63e14af360e7e9467578b17133fa84f690924d40fcc7e'),
 'src/frontend/oir/project_execution_tests.rs': (10261,
                                                 '7c6f04d73ef9d75d281e61510c658d944519af6a15147547afa919feb8fb1253'),
 'src/frontend/oir/source/hir_import/emit_failure_tests.rs': (10475,
                                                              'c95e1920e56cbc5e9317e9040aeb2f373f1e10b1fd83c32b21b7bdb28651a147'),
 'src/frontend/oir/source/hir_import/emit_resource_tests.rs': (12361,
                                                               '24310b363df14532b965ee9f93872056940342f2c5e38f0542f0d6f4bce2fcfd'),
 'src/frontend/oir/source/hir_import/emit_tests.rs': (22088,
                                                      'a61fa65387449d6daef0b5ab8e67bab975325530e100bb793a210caee4d2e975'),
 'src/frontend/oir/source/hir_import/verify_fact_tests.rs': (16666,
                                                             '7080f9672559a3c0672653ae593ec7cad83a09bec25969983f51f97ba203401f'),
 'src/frontend/project/array_syntax_tests.rs': (7131,
                                                '089c54bc7f5f0c73e68eb0fb36c5cad26c8ed497b37f502ece395e1b2ac6b784'),
 'src/frontend/project/enum_index_tests.rs': (16740,
                                              '2aa23ab15e82a2998817bf19f7654bc84422d87dfee0a4545bbc65913cf94afe'),
 'src/frontend/project/filesystem.rs': (5263,
                                        'da9e45e1d0c59782d8f97759aff4484a52559fbcd4255f932fd92f4949f99d4a'),
 'src/frontend/project/packages.rs': (18865,
                                      'f739389ef1b491720e780f2d3347a5bf6aac2b633a77daee0ad69fd1507e6c9c'),
 'src/frontend/project/packages/tests.rs': (16914,
                                            'de57707cedb1ea3a46d8323859d2a668058e350b8851b34ce74c425869b1a6e6'),
 'src/frontend/project/tests.rs': (30464,
                                   '8c5b26d1ee5a14c8571cfe9fa465fa6979ddb4c4cb01ee621e9b0b5f2ad0d049'),
 'src/frontend/project/unit2_tests.rs': (9831,
                                         '95a92ad4c295f6e8ea56314c5bfe9e7a1ac728c74a0ed6eb6b99cd20f96dd7ee'),
 'src/frontend/stdin_public_tests.rs': (5154,
                                        '3ca59074780eed91432246f11a17aa9976a9b46c0b2c2d674ce69fde49233930'),
 'src/project_cli.rs': (63112, 'd6c92fd940aa559bed135a771adff7b2dcc81226fc6cd907579d812974728597'),
 'tests/typed_project_dispatch.rs': (5888,
                                       'b24162962f6149422ebe38f58f6fe05d16de71ea06d81aa809df6b979035666d')}
REVIEWED_DIRECTIVES = {'build.rs': ([], []),
 'native/experimental_files.c': ([], []),
 'src/frontend/build_config.rs': ([], []),
 'src/frontend/declaration_index/builtin_tests.rs': ([], [b'#[path = "experimental_tests.rs"]']),
 'src/frontend/driver.rs': ([], []),
 'src/frontend/lexical_provider/tests.rs': ([], []),
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
 'src/frontend/oir/native.rs': ([],
                                [b'#[path = "native_private_emit.rs"]',
                                 b'#[path = "native_scalar_resource.rs"]',
                                 b'#[path = "native_emit_work.rs"]',
                                 b'#[path = "native_emit_cost.rs"]',
                                 b'#[path = "native_private_emit_tests.rs"]',
                                 b'#[path = "native_u8_tests.rs"]',
                                 b'#[path = "native_emit_observation.rs"]',
                                 b'#[path = "native_hosted_capacity_tests.rs"]']),
 'src/frontend/oir/owned/binary_io.rs': ([], []),
 'src/frontend/oir/owned/byte_storage_native_tests.rs': ([], []),
 'src/frontend/oir/owned/enum_index_allocation_tests.rs': ([], []),
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
                                       b'#[path = "native_reachability.rs"]',
                                       b'#[path = "native_resources.rs"]',
                                       b'#[path = "builtin_input_native_tests.rs"]',
                                       b'#[path = "builtin_output_native_tests.rs"]',
                                       b'#[path = "native_tests.rs"]',
                                       b'#[path = "native_inventory_admission_tests.rs"]',
                                       b'#[path = "hosted_capacity_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.rs"]']),
 'src/frontend/oir/owned/output.rs': ([], []),
 'src/frontend/oir/owned/process.rs': ([], []),
 'src/frontend/oir/owned/source/enum_type_tests.rs': ([], []),
 'src/frontend/oir/owned/source/output_lower_tests.rs': ([], []),
 'src/frontend/oir/owned/source/output_source_tests.rs': ([], []),
 'src/frontend/oir/owned/source/output_typing_tests.rs': ([], []),
 'src/frontend/oir/project_execution_tests.rs': ([], []),
 'src/frontend/oir/source/hir_import/emit_failure_tests.rs': ([b'include_str!(concat!(\n    env!("'
                                                               b'CARGO_MANIFEST_DIR")',
                                                               b'include_bytes!(concat!(\n    env!'
                                                               b'("CARGO_MANIFEST_DIR")'],
                                                              []),
 'src/frontend/oir/source/hir_import/emit_resource_tests.rs': ([b'include_str!(concat!(\n    env!("'
                                                                b'CARGO_MANIFEST_DIR")',
                                                                b'include_bytes!(concat!(\n    env!'
                                                                b'("CARGO_MANIFEST_DIR")',
                                                                b'include_str!(concat!(\n        en'
                                                                b'v!("CARGO_MANIFEST_DIR")',
                                                                b'include_bytes!(concat!(\n        '
                                                                b'env!("CARGO_MANIFEST_DIR")'],
                                                               []),
 'src/frontend/oir/source/hir_import/emit_tests.rs': ([b'include_str!(concat!(\n    env!("CARGO_MA'
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
 'src/frontend/oir/source/hir_import/verify_fact_tests.rs': ([b'include_str!(concat!(\n          '
                                                              b'      env!("CARGO_MANIFEST_DIR")',
                                                              b'include_bytes!(concat!(\n        '
                                                              b'        env!("CARGO_MANIFEST_DIR'
                                                              b'")'],
                                                             []),
 'src/frontend/project/array_syntax_tests.rs': ([], []),
 'src/frontend/project/enum_index_tests.rs': ([], []),
 'src/frontend/project/filesystem.rs': ([], []),
 'src/frontend/project/packages.rs': ([], []),
 'src/frontend/project/packages/tests.rs': ([], []),
 'src/frontend/project/tests.rs': ([], []),
 'src/frontend/project/unit2_tests.rs': ([], []),
 'src/frontend/stdin_public_tests.rs': ([], []),
 'src/project_cli.rs': ([], []),
 'tests/typed_project_dispatch.rs': ([], [])}


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
            "changed pinned scalar debug storage source policy")
    module = types.ModuleType("_macos_native_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | MACOS_NATIVE_ADDED
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
    expected = expected_parent | MACOS_NATIVE_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(MACOS_NATIVE_INPUTS) == MACOS_NATIVE_CHANGED | MACOS_NATIVE_ADDED
            and MACOS_NATIVE_CHANGED <= expected_parent and not MACOS_NATIVE_ADDED & expected_parent,
            "wrong reviewed macos-native input membership")
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
        if name not in MACOS_NATIVE_CHANGED:
            require(inputs[name] == before, "unreviewed change outside macos-native boundary: " + name)
    for name, (length, digest) in MACOS_NATIVE_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "macos-native source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "macos-native compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "macos_native_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(MACOS_NATIVE_INPUTS.items())],
              "macos_native_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(MACOS_NATIVE_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns", "record_enum_fields", "constructor_inference", "linux_native", "range_for", "hosted_heap", "documentation", "native_debug_variables", "language_server", "editor_navigation", "disjoint_fields", "local_references", "local_reference_bindings", "generic_field_borrows", "native_reachability", "scalar_debug_storage", "macos_native"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_macos_native_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources; macOS native support qualified separately through source, reference and native tests",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_macos_native_compatibility.py", Path(__file__).read_bytes())}
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
