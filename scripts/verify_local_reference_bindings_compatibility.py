#!/usr/bin/env python3
"""Replay retained bounded assertions after local reference source bindings.

The reviewed delta preserves all other compiler inputs and preceding policies.
Local-reference source semantics are qualified separately through public tests.
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

PREDECESSOR = 'ee5dd9c59735888d8a2995b5ddc2dbf62e5973c8'
PREDECESSOR_POLICY = 'scripts/verify_local_references_compatibility.py'
PREDECESSOR_POLICY_SHA = '912be851b9921fe977d5649a321afb6364c2557d23041e4c4b58b39648b7b935'
PREDECESSOR_POLICY_BYTES = 33504
LOCAL_REFERENCE_BINDINGS_CHANGED = frozenset({'src/frontend/oir/owned/source/association.rs', 'src/frontend/owned_syntax_tests.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/diagnostic.rs', 'src/frontend/ast.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/parser.rs', 'src/frontend/hir.rs', 'src/frontend/editor/navigation.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/declaration_index/generics.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/slice_tests.rs', 'src/frontend/oir/owned/source/budget_tests.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/experimental_source_tests.rs', 'src/frontend/oir/owned/source/reviewer_heldout.rs', 'src/frontend/format.rs'})
LOCAL_REFERENCE_BINDINGS_ADDED = frozenset()
LOCAL_REFERENCE_BINDINGS_INPUTS = {'src/frontend/ast.rs': (59322,
                         '016edd9280557230cb3b28097e06b3b7d308bfca6b08801539b9f217729d4144'),
 'src/frontend/declaration_index/generics.rs': (109765,
                                                'dac1b05de360c59e45bb660aff53c6bfd09da9ddada0458708931bbf8a880de2'),
 'src/frontend/declaration_index/source_owner.rs': (23526,
                                                    'd89b5b938d1a00a3007aeddd94596272d88e5e2f40c37e2b609b8bcd18676d4f'),
 'src/frontend/editor/navigation.rs': (11787,
                                       '8ce723588a85f9406a71cb732d394d7d2f3cf8e7a41ed6c7690283fef4add188'),
 'src/frontend/format.rs': (53255,
                            '9f13d0a4d7d8cc13c10f7616fa8fe3ef656afef1113fdf6ca93f7982dd0128b5'),
 'src/frontend/format/ast_tests.rs': (76299,
                                      '46946f52b989261443ce0f303d343a5a746e39bb044070b58fa6e40e9355efff'),
 'src/frontend/hir.rs': (33305,
                         '70c7addd191f7b5626e152a8f15f8c3a7d28d2adbde4e5d1083430719e29508a'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (57976,
                                                          '491d342fb018ee4b57940fd0271d56d4184601048b48d9e0334678384787120c'),
 'src/frontend/oir/owned/source/association.rs': (88535,
                                                  'cce901550f997a5dc82ee64e5f07f2a3f406760e3ec25e7120b8be35cb284d89'),
 'src/frontend/oir/owned/source/budget_tests.rs': (14153,
                                                   'ef73c27ffc5657b27a37ba41bf9e0b1358f174e9ed52d1f485aa0e34a0e245bf'),
 'src/frontend/oir/owned/source/diagnostic.rs': (8294,
                                                 '5c1d3cd3eaa8ee7ef312b09f97f9f236642c07e64aa2e9e10691bebfed425688'),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': (25238,
                                                                '2ac87f170058f879a0c42c5f8759f7a06cc85a74efa046d62f85427c65f4c505'),
 'src/frontend/oir/owned/source/hir.rs': (7445,
                                          '14c720f5439c7be54713f646066a20294521113261697817c1902b80729ee352'),
 'src/frontend/oir/owned/source/hir_budget.rs': (58185,
                                                 '3fc6af38cae7051265cce6194fa4a9c1d7041d98048902ba1d2498f2971664c5'),
 'src/frontend/oir/owned/source/lower.rs': (132634,
                                            '523dc03be48b6fd337b9b6de177140291fb9cd3eeb49fb53908384702f66e6d3'),
 'src/frontend/oir/owned/source/resolve.rs': (165342,
                                              '0006d6d4d5b77a3ea9d6a2884ba7fabc0fb3de19517bf2d0867500649ddba782'),
 'src/frontend/oir/owned/source/reviewer_heldout.rs': (27100,
                                                       '5ad4a952a18586489786c3e0ac5c02d6eed214e7927d21207ceaa925c175ee0d'),
 'src/frontend/oir/owned/source/slice_tests.rs': (6785,
                                                  '65868ca6cff474c50a1b87a9d6afc2862865608f14306e55c8f6b2dc615d307f'),
 'src/frontend/oir/owned/source/typeck.rs': (221021,
                                             '14bbca8ccf19633f299c067b4a1e5b167cfacf139812aca7a30b8c8d48a0439a'),
 'src/frontend/oir/source/hir_import/candidate.rs': (53028,
                                                     'e407530fe79e51116a7bbab87a148239d7d07ece4496edd10b169f73091597b1'),
 'src/frontend/owned_syntax_tests.rs': (14322,
                                        '454fb2f30e0d77793eb43efb333df3c9b87ff7893b6f20d7a0a10fd695a0de8d'),
 'src/frontend/parser.rs': (104863,
                            'a4013219c5dd1d5b0ada79e49d9503dccddf4559abaf51934e081eb91bbad9e2')}
REVIEWED_DIRECTIVES = {'src/frontend/ast.rs': ([], []),
 'src/frontend/declaration_index/generics.rs': ([], []),
 'src/frontend/declaration_index/source_owner.rs': ([], []),
 'src/frontend/editor/navigation.rs': ([], []),
 'src/frontend/format.rs': ([],
                            [b'#[path = "format/resource_tests.rs"]',
                             b'#[path = "format/ast_tests.rs"]',
                             b'#[path = "format/enum_candidate_tests.rs"]']),
 'src/frontend/format/ast_tests.rs': ([], []),
 'src/frontend/hir.rs': ([], []),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': ([], []),
 'src/frontend/oir/owned/source/association.rs': ([],
                                                  [b'#[path = "output_lower_t'
                                                   b'ests.rs"]',
                                                   b'#[path = "byte_storage_a'
                                                   b'ssociation_tests.rs"]']),
 'src/frontend/oir/owned/source/budget_tests.rs': ([], []),
 'src/frontend/oir/owned/source/diagnostic.rs': ([], []),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': ([],
                                                                [b'#[path = "io'
                                                                 b'_delivery_so'
                                                                 b'urce_tests.r'
                                                                 b's"]',
                                                                 b'#[path = "pr'
                                                                 b'ocess_argume'
                                                                 b'nts_tests.rs'
                                                                 b'"]']),
 'src/frontend/oir/owned/source/hir.rs': ([], []),
 'src/frontend/oir/owned/source/hir_budget.rs': ([],
                                                 [b'#[path = "byte_storage_r'
                                                  b'esources.rs"]',
                                                  b'#[path = "hir_budget_tes'
                                                  b'ts.rs"]']),
 'src/frontend/oir/owned/source/lower.rs': ([], []),
 'src/frontend/oir/owned/source/resolve.rs': ([],
                                              [b'#[path = "resolver_paid_test'
                                               b's.rs"]',
                                               b'#[path = "resolver_enum_test'
                                               b's.rs"]',
                                               b'#[path = "enum_storage_failu'
                                               b're_tests.rs"]',
                                               b'#[path = "builtin_signature_'
                                               b'tests.rs"]',
                                               b'#[path = "output_typing_test'
                                               b's.rs"]',
                                               b'#[path = "byte_storage_type_'
                                               b'tests.rs"]']),
 'src/frontend/oir/owned/source/reviewer_heldout.rs': ([], []),
 'src/frontend/oir/owned/source/slice_tests.rs': ([], []),
 'src/frontend/oir/owned/source/typeck.rs': ([],
                                             [b'#[path = "enum_type_tests.rs'
                                              b'"]']),
 'src/frontend/oir/source/hir_import/candidate.rs': ([], []),
 'src/frontend/owned_syntax_tests.rs': ([], []),
 'src/frontend/parser.rs': ([],
                            [b'#[path = "parser/arrays.rs"]',
                             b'#[path = "parser/characters.rs"]',
                             b'#[path = "parser/conversions.rs"]',
                             b'#[path = "parser/enums.rs"]',
                             b'#[path = "parser/u8_syntax_tests.rs"]',
                             b'#[path = "parser/builtin_tests.rs"]',
                             b'#[path = "parser/array_syntax_tests.rs"]',
                             b'#[path = "parser/enum_syntax_tests.rs"]',
                             b'#[path = "parser/project_tests.rs"]',
                             b'#[path = "parser/activation_tests.rs"]'])}


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
            "changed pinned local-reference core source policy")
    module = types.ModuleType("_local_reference_bindings_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | LOCAL_REFERENCE_BINDINGS_ADDED
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
    expected = expected_parent | LOCAL_REFERENCE_BINDINGS_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(LOCAL_REFERENCE_BINDINGS_INPUTS) == LOCAL_REFERENCE_BINDINGS_CHANGED | LOCAL_REFERENCE_BINDINGS_ADDED
            and LOCAL_REFERENCE_BINDINGS_CHANGED <= expected_parent and not LOCAL_REFERENCE_BINDINGS_ADDED & expected_parent,
            "wrong reviewed local-reference-bindings input membership")
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
        if name not in LOCAL_REFERENCE_BINDINGS_CHANGED:
            require(inputs[name] == before, "unreviewed change outside local-reference-bindings boundary: " + name)
    for name, (length, digest) in LOCAL_REFERENCE_BINDINGS_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "local-reference-bindings source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "local-reference-bindings compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "local_reference_bindings_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(LOCAL_REFERENCE_BINDINGS_INPUTS.items())],
              "local_reference_bindings_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(LOCAL_REFERENCE_BINDINGS_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns", "record_enum_fields", "constructor_inference", "linux_native", "range_for", "hosted_heap", "documentation", "native_debug_variables", "language_server", "editor_navigation", "disjoint_fields", "local_references", "local_reference_bindings"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_local_reference_bindings_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources; local reference bindings qualified separately through source, reference and native tests",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_local_reference_bindings_compatibility.py", Path(__file__).read_bytes())}
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
