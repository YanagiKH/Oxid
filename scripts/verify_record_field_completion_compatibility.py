#!/usr/bin/env python3
"""Bind record-field editor inputs to the complete retained Windows-source replay.

Only inherited default-emission semantics and resource predicates are replayed.
Record-field completion and all new language/platform/debug claims stay false.
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

PREDECESSOR = '8180b1bbcbcf9ccd4d648ff5ba0ec9ea3d5b971f'
PREDECESSOR_POLICY = 'scripts/verify_windows_native_compatibility.py'
PREDECESSOR_POLICY_PIN = (55942, '498fb28cfa67a7c4ab03754824323a51b651400b80264996cf1549716ad28dbd')
PINS_PROVISIONAL = True
RECORD_FIELD_COMPLETION_CHANGED = frozenset(['src/frontend/declaration_index/editor_completion.rs',
 'src/frontend/driver.rs',
 'src/frontend/editor.rs',
 'src/frontend/parser/editor_query.rs',
 'src/lsp_cli.rs'])
RECORD_FIELD_COMPLETION_ADDED = frozenset(['src/frontend/declaration_index/editor_completion/members.rs',
 'src/frontend/editor/members.rs',
 'src/frontend/editor/members/resources.rs',
 'src/frontend/editor/members/tests.rs',
 'src/frontend/parser/editor_query/members.rs'])
REVIEWED_INPUTS = {'src/frontend/declaration_index/editor_completion.rs': (18441,
                                                         '09a8406f7b4d293658ed3e99c4dd6aef417f0c7aee49ba57ff4216187a1aea1f'),
 'src/frontend/declaration_index/editor_completion/members.rs': (11147,
                                                                 '15b102c1b43b3ebcc4ad367f71b26740c0fdb59285ed6e27bc3c5403b26dd1fd'),
 'src/frontend/driver.rs': (98540, '45b5763dd897320def85bc4a8c9fe0ba948a94225d28f052228f74ab9535f08b'),
 'src/frontend/editor.rs': (12635, '5b0cf7e87f1c4f1a36906d74294039741113f975b3abbe52c58fb9812a53711d'),
 'src/frontend/editor/members.rs': (18286,
                                    'eac1f74900daf4f6ad9a5f7cdd72932bbb5641bb64a686b913f6e78e5c78e1a4'),
 'src/frontend/editor/members/resources.rs': (15400,
                                              '872ff86decd50f135df37763057e0df1b963b0e3c52c62e622178057b116d220'),
 'src/frontend/editor/members/tests.rs': (12670,
                                          'a1454a898065ad5ef994fa497835899081c5c28b7c22c8a4307b0c3aeb85564b'),
 'src/frontend/parser/editor_query.rs': (23946,
                                         '0d187ac400350c888e4850c736ec3fcf3eec6e185d55f8b86c2d81e8018fe1ec'),
 'src/frontend/parser/editor_query/members.rs': (24478,
                                                 'a288d0bdd6e9fde589f3a732bcb20e1ff3ae8c67682dbc7fab9a17ece101ecbc'),
 'src/lsp_cli.rs': (34532, '4c9e5d62a26bb5c030e816aae797e735da71637951076a9b194cea97fd6bc4fd')}
REVIEWED_DIRECTIVES = {'src/frontend/declaration_index/editor_completion.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug)]', 1),
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
 'src/frontend/declaration_index/editor_completion/members.rs': {'attributes': ((b'#[derive(Clone, Copy)]',
                                                                                 1),
                                                                                (b'#[repr(C)]', 1),
                                                                                (b'#[allow(dead_code)]', 1),
                                                                                (b'#[cfg(test)]', 1),
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
 'src/frontend/editor.rs': {'attributes': ((b'#[derive(Clone, Debug)]', 2),
                                           (b'#[derive(Debug)]', 1),
                                           (b'#[cfg(test)]', 2),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 1),
                                           (b'#[cfg(target_os = "linux")]', 1),
                                           (b'#[test]', 4)),
                            'build_script': None,
                            'c_preprocessor': (),
                            'legacy': ([], []),
                            'macros': ()},
 'src/frontend/editor/members.rs': {'attributes': ((b'#[cfg(test)]', 1),
                                                   (b'#[derive(Clone, Copy, Debug, PartialEq, Eq)]', 1),
                                                   (b'#[cfg(test)]', 1),
                                                   (b'#[derive(Default)]', 1),
                                                   (b'#[cfg(not(test))]', 1),
                                                   (b'#[cfg(test)]', 2)),
                                    'build_script': None,
                                    'c_preprocessor': (),
                                    'legacy': ([], []),
                                    'macros': ()},
 'src/frontend/editor/members/resources.rs': {'attributes': ((b'#[repr(C)]', 1),
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
                                                             (b'#[cfg(test)]', 1),
                                                             (b'#[cfg(target_pointer_width = "64")]', 1),
                                                             (b'#[test]', 1)),
                                              'build_script': None,
                                              'c_preprocessor': (),
                                              'legacy': ([], []),
                                              'macros': ()},
 'src/frontend/editor/members/tests.rs': {'attributes': ((b'#[test]', 1),
                                                         (b'#[cfg(target_pointer_width = "64")]', 1),
                                                         (b'#[test]', 5)),
                                          'build_script': None,
                                          'c_preprocessor': (),
                                          'legacy': ([], []),
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
 'src/frontend/parser/editor_query/members.rs': {'attributes': ((b'#[derive(Clone, Copy, Debug, PartialEq, '
                                                                 b'Eq)]',
                                                                 1),
                                                                (b'#[derive(Clone, Copy, Debug)]', 1),
                                                                (b'#[derive(Clone, Copy)]', 1),
                                                                (b'#[derive(Clone, Copy, Debug)]', 1),
                                                                (b'#[cfg(test)]', 1),
                                                                (b'#[test]', 1)),
                                                 'build_script': None,
                                                 'c_preprocessor': (),
                                                 'legacy': ([], []),
                                                 'macros': ()},
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
            "symlink pinned Windows native policy")
    before = path.stat()
    require(path.is_file() and before.st_nlink == 1 and not before.st_mode & 0o111,
            "unsafe pinned Windows native policy")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, field) for field in fields)
            == tuple(getattr(path.stat(), field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned Windows native policy")
    module = types.ModuleType("_record_field_completion_retained_windows_native")
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
PARENT_ADDED = PREDECESSOR_ADDED | RECORD_FIELD_COMPLETION_ADDED
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
    expected = expected_parent | RECORD_FIELD_COMPLETION_ADDED
    require(set(parent) == expected_parent and len(parent) == 512,
            "wrong Windows native compiler input membership")
    require(not set(UNCHANGED_INPUTS) & (RECORD_FIELD_COMPLETION_CHANGED | RECORD_FIELD_COMPLETION_ADDED)
            and set(UNCHANGED_INPUTS) <= expected_parent,
            "protected input cannot enter the generic reviewed delta")
    require(set(REVIEWED_INPUTS) == RECORD_FIELD_COMPLETION_CHANGED | RECORD_FIELD_COMPLETION_ADDED
            and RECORD_FIELD_COMPLETION_CHANGED <= expected_parent
            and not RECORD_FIELD_COMPLETION_ADDED & expected_parent,
            "wrong reviewed record-field-completion input membership")
    require(set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS),
            "wrong reviewed record-field-completion directive membership")
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
        if name not in RECORD_FIELD_COMPLETION_CHANGED:
            require(inputs[name] == before,
                    "unreviewed change outside record-field-completion boundary: " + name)
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "record-field-completion source does not match reviewed input: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "record-field-completion compile-time input directive changed: " + name)
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
        "record_field_completion_pins_provisional": PINS_PROVISIONAL,
        "retained_protected_inputs": {name: identity(name, parent[name])
                                      for name in sorted(UNCHANGED_INPUTS)},
        "record_field_completion_reviewed_inputs": [
            {"path": name, "bytes": length, "sha256": digest}
            for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
        "record_field_completion_changes": [
            {"path": name, "before": identity(name, parent[name]),
             "after": identity(name, inputs[name])}
            for name in sorted(RECORD_FIELD_COMPLETION_CHANGED)],
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
    require(Path(__file__).resolve() == repo / "scripts/verify_record_field_completion_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained default-emission scalar/record semantics and resources; record-field completion and new language/platform/debug semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_record_field_completion_compatibility.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        require(args.prepare_only or not PINS_PROVISIONAL,
                "provisional record-field-completion pins permit prepare-only admission, not qualification")
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
