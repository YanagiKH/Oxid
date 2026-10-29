#!/usr/bin/env python3
"""Bind private-registry sources to the complete inherited scalar-library replay.

Only retained default-emission semantic/resource predicates are replayed.
Registry and all platform/debug claims remain false. Provisional source pins
permit preparation only; they can never launch qualification.
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

PREDECESSOR = "8f2c29eaffabbd90c591bbdc440ae6e23acf1353"
PREDECESSOR_POLICY = "scripts/verify_scalar_library_compatibility.py"
PREDECESSOR_POLICY_PIN = (63079, '29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119')
PINS_PROVISIONAL = False
REGISTRY_CHANGED = frozenset(['src/cli.rs',
 'src/main.rs',
 'src/project_cli.rs',
 'src/project_cli/documentation.rs',
 'src/project_cli/editor.rs',
 'src/project_cli/package.rs',
 'src/project_cli/workspace.rs',
 'src/project_lock.rs',
 'src/project_package.rs',
 'src/project_package/archive.rs'])
REGISTRY_ADDED = frozenset(['src/project_cli/registry.rs',
 'src/project_package/archive/library.rs',
 'src/project_registry.rs',
 'src/project_registry/resources.rs',
 'src/project_registry/tests.rs'])
REVIEWED_INPUTS = {'src/cli.rs': (1578, '3a19d6bc43bc91a54f31972e3105252ddcf1974a25c5bd46b1dc55e1b8bb26c3'),
 'src/main.rs': (164323, 'f6d0b2e92ca0d92523c9072fe24863a11d56805caa3e05a78f8595d43f6ef60c'),
 'src/project_cli.rs': (72723, '36840d5fc53755e02cf2acd37777d6668421880b6b008966bfed3050b67e6771'),
 'src/project_cli/documentation.rs': (5895,
                                      'f8792db6dd543e216b0e7c7998a1a83093b36890756b73ce051ed35d63990ebc'),
 'src/project_cli/editor.rs': (4991,
                               'fe9ed7f03361f3a5c8675ce01e2ecc750394c8e5ace276a8436f1a84217e7b8b'),
 'src/project_cli/package.rs': (63418,
                                '35479d0550f503817e8244c03d83c2696ae4122bb64f4bc46cc22c0628195ac6'),
 'src/project_cli/registry.rs': (20004,
                                 '6070016179e0f924940b19cafbfc050c466242b0f2d68f938a41064958541088'),
 'src/project_cli/workspace.rs': (38668,
                                  'aea71a2060d51f540401052e21db6726c25f438fa946b5204ce96e304e997b89'),
 'src/project_lock.rs': (62668, 'e44ef0566de7c3c40cfefbdf4ae83d3e2a1ccfea38462219f02f424450717065'),
 'src/project_package.rs': (29124,
                            '8deb8ba95517ce25296a6a9a4a0662bdae56aa6b759c2044f9be196f102bc5fb'),
 'src/project_package/archive.rs': (37942,
                                    'e12f5195870e0c4d8cf9ec61aab3a3c07822d6a8d57a0f6ccc904a605f51e676'),
 'src/project_package/archive/library.rs': (13555,
                                            '5e5b0ed7dfa2d49733be569082a2f009187bb5612774bccd5e8c9b4293ceb946'),
 'src/project_registry.rs': (63840,
                             '1536be1b24b27d019d0c85a86948dc00db7c8626921fb2b52854bee437403769'),
 'src/project_registry/resources.rs': (16481,
                                       'e76d0dc379f354860c40eb0d5195d615428d5e1c76d37f78a63e4f4551e3ed29'),
 'src/project_registry/tests.rs': (27194,
                                   'ade3972de8292f6730c175e71cb5c6fbb77debf2b9ad2605e581a1447f67124d')}
REVIEWED_DIRECTIVES = {'src/cli.rs': {'attributes': ((b'#[allow(clippy::items_after_test_module)]', 1),),
                'build_script': None,
                'c_preprocessor': (),
                'legacy': ([b'include!("main.rs")'], []),
                'macros': ((b'include!("main.rs")', 1),)},
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
                                (b'#[cfg(unix)]', 1),
                                (b'#[cfg(test)]', 1),
                                (b'#[test]', 16),
                                (b'#[cfg(test)]', 1),
                                (b'#[test]', 2)),
                 'build_script': None,
                 'c_preprocessor': ((b'# This file demonstrates how project-level automation can li'
                                     b've in Oxid source files.\n',
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
 'src/project_cli.rs': {'attributes': ((b'#[derive(Default)]', 1),
                                       (b'#[derive(Clone, Copy, Default)]', 1),
                                       (b'#[cfg(test)]', 3),
                                       (b'#[test]', 5)),
                        'build_script': None,
                        'c_preprocessor': (),
                        'legacy': ([], []),
                        'macros': ((b'cfg!(target_os = "linux")', 1), (b'cfg!(windows)', 1))},
 'src/project_cli/documentation.rs': {'attributes': ((b'#[cfg(unix)]', 1),),
                                      'build_script': None,
                                      'c_preprocessor': (),
                                      'legacy': ([], []),
                                      'macros': ()},
 'src/project_cli/editor.rs': {'attributes': (),
                               'build_script': None,
                               'c_preprocessor': (),
                               'legacy': ([], []),
                               'macros': ()},
 'src/project_cli/package.rs': {'attributes': ((b'#[repr(C)]', 1),
                                               (b'#[cfg(all(\n    test,\n    target_os = "linux"'
                                                b',\n    target_pointer_width = "64",\n    targe'
                                                b't_endian = "little",\n    any(target_arch = "x86_'
                                                b'64", target_arch = "aarch64")\n))]',
                                                1),
                                               (b'#[test]', 11)),
                                'build_script': None,
                                'c_preprocessor': (),
                                'legacy': ([], []),
                                'macros': ((b'cfg!(all(\n        target_os = "linux",\n        t'
                                            b'arget_pointer_width = "64",\n        target_endian = '
                                            b'"little",\n        any(target_arch = "x86_64", target'
                                            b'_arch = "aarch64")\n    ))',
                                            1),)},
 'src/project_cli/registry.rs': {'attributes': ((b'#[repr(C)]', 1),
                                                (b'#[cfg(test)]', 1),
                                                (b'#[test]', 2)),
                                 'build_script': None,
                                 'c_preprocessor': (),
                                 'legacy': ([], []),
                                 'macros': ()},
 'src/project_cli/workspace.rs': {'attributes': ((b'#[allow(clippy::too_many_arguments)]', 1),),
                                  'build_script': None,
                                  'c_preprocessor': (),
                                  'legacy': ([], []),
                                  'macros': ()},
 'src/project_lock.rs': {'attributes': ((b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]', 1),
                                        (b'#[derive(Debug, PartialEq, Eq)]', 1),
                                        (b'#[cfg(unix)]', 1),
                                        (b'#[repr(C)]', 1),
                                        (b'#[cfg(unix)]', 1),
                                        (b'#[cfg(test)]', 1),
                                        (b'#[test]', 1)),
                         'build_script': None,
                         'c_preprocessor': (),
                         'legacy': ([], []),
                         'macros': ()},
 'src/project_package.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                           (b'#[cfg(unix)]', 4),
                                           (b'#[cfg(not(unix))]', 1),
                                           (b'#[cfg(unix)]', 3),
                                           (b'#[cfg(not(unix))]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[cfg(not(target_os = "linux"))]', 1)),
                            'build_script': None,
                            'c_preprocessor': (),
                            'legacy': ([], []),
                            'macros': ()},
 'src/project_package/archive.rs': {'attributes': ((b'#[derive(Clone, Debug, Eq, PartialEq)]', 4),
                                                   (b'#[cfg(test)]', 1),
                                                   (b'#[test]', 17)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ()},
 'src/project_package/archive/library.rs': {'attributes': ((b'#[cfg(test)]', 1), (b'#[test]', 2)),
                                            'build_script': None,
                                            'c_preprocessor': (),
                                            'legacy': ([], []),
                                            'macros': ()},
 'src/project_registry.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                            (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Debug)]', 1),
                                            (b'#[cfg(unix)]', 2),
                                            (b'#[cfg(not(unix))]', 1),
                                            (b'#[cfg(unix)]', 1),
                                            (b'#[derive(Clone, Debug, PartialEq, Eq)]', 1),
                                            (b'#[derive(Clone, Debug)]', 1),
                                            (b'#[derive(Debug)]', 3),
                                            (b'#[cfg(test)]', 3),
                                            (b'#[cfg(all(\n    test,\n    target_os = "linux",\n  '
                                             b'  target_pointer_width = "64",\n    target_endian'
                                             b' = "little",\n    any(target_arch = "x86_64", tar'
                                             b'get_arch = "aarch64")\n))]',
                                             1),
                                            (b'#[cfg(test)]', 6),
                                            (b'#[cfg(unix)]', 2)),
                             'build_script': None,
                             'c_preprocessor': (),
                             'legacy': ([], []),
                             'macros': ((b'cfg!(target_os = "linux")', 1),)},
 'src/project_registry/resources.rs': {'attributes': ((b'#[repr(C)]', 7),
                                                      (b'#[cfg(test)]', 1),
                                                      (b'#[test]', 1)),
                                       'build_script': None,
                                       'c_preprocessor': (),
                                       'legacy': ([], []),
                                       'macros': ()},
 'src/project_registry/tests.rs': {'attributes': ((b'#[test]', 6),
                                                  (b'#[cfg(target_os = "linux")]', 1),
                                                  (b'#[test]', 6)),
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
            "symlink pinned scalar-library policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned scalar-library policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned scalar-library policy")
    module = types.ModuleType("_registry_retained_scalar_library")
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
PARENT_ADDED = PREDECESSOR_ADDED | REGISTRY_ADDED
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
    require(not any(name.startswith(("src/frontend/", "native/"))
                    for name in REGISTRY_CHANGED | REGISTRY_ADDED),
            "registry cannot change an inherited frontend or native source owner")
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | REGISTRY_ADDED
    require(set(parent) == expected_parent and len(parent) == 522,
            "wrong scalar-library compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (REGISTRY_CHANGED | REGISTRY_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == REGISTRY_CHANGED | REGISTRY_ADDED
            and REGISTRY_CHANGED <= expected_parent
            and not REGISTRY_ADDED & expected_parent,
            "wrong reviewed private-registry input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed private-registry directive membership")
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
        if name not in REGISTRY_CHANGED:
            require(inputs[name] == before,
                    "unreviewed change outside private-registry boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "private-registry source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "private-registry compile-time input directive changed: " + name)
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
        "registry_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: identity(name, parent[name])
                                      for name in sorted(UNCHANGED_INPUTS)},
        "registry_reviewed_inputs": [
            {"path": name, "bytes": length, "sha256": digest}
            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "registry_changes": [
            {"path": name, "before": identity(name, parent[name]),
             "after": identity(name, inputs[name])}
            for name in sorted(REGISTRY_CHANGED)],
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
        "private_registry_semantics_qualified_by_this_gate": False,
        "registry_supply_chain_semantics_qualified_by_this_gate": False,
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
    require(Path(__file__).resolve() == repo / "scripts/verify_registry_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained default-emission scalar/record semantics and resources; private registries, scalar native libraries, general FFI and new platform/debug semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_registry_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional private-registry pins permit prepare-only admission, not qualification")
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
