#!/usr/bin/env python3
"""Run retained bounded compatibility assertions after adding byte string literals.

The exact parent policy supplies unchanged archive, assembly and observer
helpers. Only the literal reviewed compiler/runtime delta may differ from the
pinned parent. Byte string semantics require separate public and native tests.
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

PREDECESSOR = "c45d519127d314f522762c35879c0c485994ea42"
PREDECESSOR_POLICY = "scripts/verify_buffer_transfer_compatibility.py"
PREDECESSOR_POLICY_SHA = "8de595abad8b57e22b0c7a3d42f967d227fec6a6693d6b6801c162d758b98200"
PREDECESSOR_POLICY_BYTES = 16967
LITERAL_CHANGED = frozenset(('src/frontend/ast.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/mod.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/byte_storage_resources.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/hir_budget_tests.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/resolver_enum_tests.rs', 'src/frontend/oir/owned/source/resolver_inventory_tests.rs', 'src/frontend/oir/owned/source/resolver_paid_tests.rs', 'src/frontend/oir/owned/source/resolver_storage.rs', 'src/frontend/oir/owned/source/resolver_storage_tests.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned/source/u8_tests.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/tests.rs', 'src/frontend/oir/verify.rs', 'src/frontend/parser.rs'))
LITERAL_ADDED = frozenset(('src/frontend/byte_strings.rs', 'src/frontend/oir/owned/source/byte_string_resources.rs'))
LITERAL_INPUTS = {
    'src/frontend/ast.rs': (41815, '203280d3331210cd669a35cbb55e6679e0f622090283fb34b19a9c3a58f072e5'),
    'src/frontend/byte_strings.rs': (8374, 'f6143c7d7de52aacd1603ccc485949107e62c5234b49a97348df10278c682177'),
    'src/frontend/format.rs': (49309, '4569508da07ef49239d7bf9b3980455f8236167297f5a88a2220cb61d1b7be62'),
    'src/frontend/format/ast_tests.rs': (65102, 'f2cc490e4fd5c9193ec4a4dc6c3ec901b25e42bc6b08587ce2fe6beb8186cc54'),
    'src/frontend/hir.rs': (27172, 'e070b858bb4e48bd9be34c1fb203120453ac8bcfe5558f541253836ddfab5fc1'),
    'src/frontend/mod.rs': (1016, '034f93a237422fc716de44e68296f11c2e9f42222a7040d532f5838f1dc07361'),
    'src/frontend/oir/execute.rs': (21767, 'cc0e70c30c66d1c846c01802cc8d8ee0a15296bee5bb1f04dfa9902d1529416c'),
    'src/frontend/oir/mod.rs': (11822, 'da517abde54db363f115c84ab7f87a717b1fb00d7de670d82b64c60ac8677af3'),
    'src/frontend/oir/native.rs': (105507, '392fc91bce3ba8e77bdbd94c3d161da2c3cac84796e6f10c271eb46d39792a42'),
    'src/frontend/oir/native_emit_work.rs': (28585, '24b46741c0a19da028601bb8be70f7b4a307b429cc7cf9cdb4c3c429bd464dcd'),
    'src/frontend/oir/owned/execute.rs': (177008, 'f6663e7e2661f55f812675df363c4dba5dfdac092635349d10e8e87be4278c2b'),
    'src/frontend/oir/owned/native.rs': (211877, 'e74ed552d4cf6a49674abf0e3d28934d8f014fb13ef77d26f14dde7ef2d5825e'),
    'src/frontend/oir/owned/source/array_pipeline_rows.rs': (53681, '9146012bf6a4111562d4d0ae5699273ebf5f73945bbed71e17aa553de46943a0'),
    'src/frontend/oir/owned/source/byte_storage_resources.rs': (10205, '55bb5afbaca5070a38859c0a7852a9a646d0ec9784d34f5888ec392d76db9609'),
    'src/frontend/oir/owned/source/byte_string_resources.rs': (7751, '464658c43bde36a7fb6101910a5ba899ea581ac0de426a7be76e8dcf99c942ba'),
    'src/frontend/oir/owned/source/candidate_adapter.rs': (42051, '656570320b1b3ed5d344ffc9d343473b10b77293946c84d2f024a05f246a2e6c'),
    'src/frontend/oir/owned/source/hir.rs': (6243, '235634c1043860e1a27d19a2c91ca4d1a4283ad67a90744dfc6a3af7ea479320'),
    'src/frontend/oir/owned/source/hir_budget.rs': (56084, 'baeba9dc2e9873602d2d9916d61ccee6e429d1da5b02112621dc566a740c2f93'),
    'src/frontend/oir/owned/source/hir_budget_tests.rs': (53958, '25b7913d7e6b863817b17c699aed44ea9d87d3489a7590c31124c9e0344308a9'),
    'src/frontend/oir/owned/source/lower.rs': (111400, '857207a47feeb218c33e52601ee3cc4aa481780bbeff38addf520df79e4580f1'),
    'src/frontend/oir/owned/source/mod.rs': (1934, 'eaffdf2133d704cdec7f50a051a976378b86551aa974a52bf8aa69960c9a02a2'),
    'src/frontend/oir/owned/source/resolve.rs': (140675, 'ed5cfb4e9255d4d8a56dfecf2ab697eeac008660575f0a4ccf8d88589dc6b8c5'),
    'src/frontend/oir/owned/source/resolver_enum_tests.rs': (24075, '922349816bd6cc67002958b32fb21723cf877e2fa9f4bbf9f996291ddc19c751'),
    'src/frontend/oir/owned/source/resolver_inventory_tests.rs': (18290, '4b93e66bdc3b5a52eab597d26c598b613f1977bd0e33e7fcad54547bc09ec4da'),
    'src/frontend/oir/owned/source/resolver_paid_tests.rs': (113753, '14292645930690bc71577df38a8d0ce73d50099f75f18d8951ae860ee5f9e3ca'),
    'src/frontend/oir/owned/source/resolver_storage.rs': (29887, '75d35693b0d33c876ab2f2e53bc2697ad246ffdce77be0b3a961b711e8388a9d'),
    'src/frontend/oir/owned/source/resolver_storage_tests.rs': (24059, 'eaa32cdf03dd3214c852a1888c14e0a9496a71cf81d3f13ee2f0a1d91b9a9573'),
    'src/frontend/oir/owned/source/typeck.rs': (207985, '16afe87fc587a20c35c48efe8459ef4c03f0d7c1dd03667fc284bd9cf66e01d8'),
    'src/frontend/oir/owned/source/u8_tests.rs': (27320, '01c4619a2acca1cea5d87c290f0a1477add8be0af011852b7a4fdf9232a6c53f'),
    'src/frontend/oir/source/association.rs': (29031, '4e1712b5eaf9bde48e25c603173834bdf10290d4ff8060b8f91d7f4a18ed6529'),
    'src/frontend/oir/source/hir_import/candidate.rs': (52544, 'e49405331d2cabbc564896ba4b69ea075b884b5f7fd22db808cc1b6a4e5afce4'),
    'src/frontend/oir/tests.rs': (39963, '3df2c57025ced01e1be6aa71c02ecc90a32e81c1f00a80708e758dd4c8417f52'),
    'src/frontend/oir/verify.rs': (44535, 'ec47746285842e503666b1c989463a5ed4f75a870cd8035339cda42ba7c9680d'),
    'src/frontend/parser.rs': (76810, '2ee38fe77b0eaa41c9e92a21e364da66f6c2806a6b3bb30fff8c86f15cc5b675'),
}


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
            "changed pinned buffer-transfer source policy")
    module = types.ModuleType("_byte_string_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | LITERAL_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


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
    expected = expected_parent | LITERAL_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(LITERAL_INPUTS) == LITERAL_CHANGED | LITERAL_ADDED
            and LITERAL_CHANGED <= expected_parent and not LITERAL_ADDED & expected_parent,
            "wrong reviewed byte-string input membership")
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
        if name not in LITERAL_CHANGED:
            require(inputs[name] == before, "unreviewed change outside byte-string boundary: " + name)
    for name, (length, digest) in LITERAL_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "byte-string source does not match reviewed input: " + name)
        require(directives(inputs[name]) == (directives(parent[name]) if name in parent else ([], [])),
                "byte-string compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "byte_string_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(LITERAL_INPUTS.items())],
              "byte_string_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(LITERAL_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_byte_string_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the byte-string successor; byte strings qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_byte_string_compatibility.py", Path(__file__).read_bytes())}
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
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", PYTHONDONTWRITEBYTECODE="1")
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
