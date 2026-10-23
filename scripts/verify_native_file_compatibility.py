#!/usr/bin/env python3
"""Run retained bounded compatibility assertions after adding native file literals.

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

PREDECESSOR = '22d04a015a444edcb600a6011cdb0e2c8529cb34'
PREDECESSOR_POLICY = 'scripts/verify_byte_string_compatibility.py'
PREDECESSOR_POLICY_SHA = '055858acb3005308519faf590102f2e988ba454cdab548821526894764d1f9aa'
PREDECESSOR_POLICY_BYTES = 21577
FILE_CHANGED = frozenset(('native/experimental_bytebuf.c', 'native/typed_bytebuf.c', 'src/frontend/builtin_catalog.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/builtin_tests.rs', 'src/frontend/declaration_index/enum_query_tests.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/experimental_tests.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/declaration_index/u8_integration_tests.rs', 'src/frontend/driver.rs', 'src/frontend/experimental_builtins.rs', 'src/frontend/native.rs', 'src/frontend/oir/owned/byte_buffer.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_experimental.rs', 'src/frontend/oir/owned/execute_resources.rs', 'src/frontend/oir/owned/experimental_descriptors.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_experimental.rs', 'src/frontend/oir/owned/native_resources.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/byte_storage_resources.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/experimental_lower.rs', 'src/frontend/oir/owned/source/experimental_source_tests.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/hir_budget_tests.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned/source/u8_tests.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/owned_types/resources.rs'))
FILE_ADDED = frozenset(('native/experimental_files.c', 'src/frontend/oir/owned/execute_file_delivery.rs', 'src/frontend/oir/owned/file_resource_tests.rs', 'src/frontend/oir/owned/file_resources.rs'))
FILE_INPUTS = {'native/experimental_bytebuf.c': (10406, 'b9477cef7673842d710de970f4bc8ffac189aa9afdf1936788b270e648a3d464'),
 'native/experimental_files.c': (6992, '6141062c9fc0aed87c149ad6fab2ce168f02f7e891f29b61e6204eff0e704968'),
 'native/typed_bytebuf.c': (6526, 'cbb42a6dd452bde3e2f55d04be836f1e8317fc8a892ff43ad7be4f2092ef2b05'),
 'src/frontend/builtin_catalog.rs': (14016,
                                     'c686a788f42dc04d67cecc4baf3e38b9e9c89041bfad305cc3eacd7679d3555e'),
 'src/frontend/declaration_index.rs': (75822,
                                       'a9863a476b78edc84a68b3d60d651b996129f84bfcb319902f2da3751ea9c574'),
 'src/frontend/declaration_index/builtin_tests.rs': (80082,
                                                     '1459a86170a6262d506dc11c15becbe8c92a2e7fc841418415dfa4cb4c05392d'),
 'src/frontend/declaration_index/enum_query_tests.rs': (46006,
                                                        'a28a7750b5e00facad6a994a760177fe9f653c025029e404f334b602a9b30f58'),
 'src/frontend/declaration_index/enum_views.rs': (11108,
                                                  '753d33c3d5818c8a40684aa3ee7281f5da6addcf39bb4cb2e2721f2a7e419772'),
 'src/frontend/declaration_index/experimental_tests.rs': (3417,
                                                          '31cbc87e46d8f07c286dd1c2b242e3d46db6f3ba84719ddbc613a5b474c07264'),
 'src/frontend/declaration_index/sealed.rs': (82630,
                                              '37c52cd885347bc8bebd242c28efc6e823864fa78fe46f4bf453bfe853012053'),
 'src/frontend/declaration_index/u8_integration_tests.rs': (19731,
                                                            'a30c8400e8061594d2ad239e2acf37912e5aadb29c4d8360cf6d77c65f6f8b94'),
 'src/frontend/driver.rs': (82398, '2ade1dc26916901355ac28d524246b5c4a29d6c6a0ed48609285180f07aee1b5'),
 'src/frontend/experimental_builtins.rs': (15887,
                                           '6e0130ca47ac44a6af09b8a17806d34928da36020501e4ce8170cdfdc6d58f8c'),
 'src/frontend/native.rs': (17324, 'a305672e278d60bf4f225f1ba7434cf24b543a79e551ab5a8ffb440f3ec5a0f8'),
 'src/frontend/oir/owned/byte_buffer.rs': (20397,
                                           '467c32892b210d93d5bc0dd4b94eaf5da6be195406ea09cec93c9161c0b17710'),
 'src/frontend/oir/owned/execute.rs': (178258,
                                       '4842c4c53104f23366552247d8d1edfbe63ac9ffed07f5e58c23bb23fc32e78c'),
 'src/frontend/oir/owned/execute_experimental.rs': (24427,
                                                    '7fa727b3a3a2bf294ecbbdd40b1182d4836ef4ca7be6c9bd9b1f8d59fdf9082a'),
 'src/frontend/oir/owned/execute_file_delivery.rs': (16126,
                                                     '5b3ef082bf8a65c814eb1e0b3a0d422419dda650d4893d427a3151dfc826a3a3'),
 'src/frontend/oir/owned/execute_resources.rs': (5774,
                                                 'd274c74bea2f1264f09c0d7c1e021dfed3aafd012e30cbaea238dd519eb32125'),
 'src/frontend/oir/owned/experimental_descriptors.rs': (6265,
                                                        'c2a75af5f3089771240d92911b497a0dd386896cf76262a2ab57c5f1fe63b002'),
 'src/frontend/oir/owned/file_resource_tests.rs': (5697,
                                                   '51ffc190209bafc19f11d6e3f2e99a11b0b2258a71e8980b867a805ace671160'),
 'src/frontend/oir/owned/file_resources.rs': (12521,
                                              '5c64ff9137a6952c0814a4a1eab9d38f88d07d21a16606fe5251cadd1c9ab052'),
 'src/frontend/oir/owned/native.rs': (212891,
                                      '3f366243430a9aaa8dcb3293e954706389b6dd137fbe8f87b6350358cbf87e55'),
 'src/frontend/oir/owned/native_experimental.rs': (13138,
                                                   'b19862defe577252674b61526052acfea593c1bc5cd80aae7679f71ed7ed3811'),
 'src/frontend/oir/owned/native_resources.rs': (6205,
                                                'f1aad4715a4235bbdf08c3a7c2588e2898a94599bcf6c6b709bcd94463b9c781'),
 'src/frontend/oir/owned/plan.rs': (25607,
                                    'a1fa23e62a56ec49841de8f71096a63a4ca86b963d11bad86dd82ff01c4e0caf'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (65243,
                                                              '120aabbaacd2267c7d044cc49cc475d143efd4e27460c36724a43560f69721c6'),
 'src/frontend/oir/owned/shape.rs': (56023,
                                     '5c2b6351240a08f1eb841d98d04f1b083935f4f71fe01feea4ccac45f5984210'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (53732,
                                                          '91a2da94dfccc400d44a3b097b99f81a2267141097c3a5fb3dbee38e0bc772a2'),
 'src/frontend/oir/owned/source/byte_storage_resources.rs': (10336,
                                                             '62b0686884f049813ec8eb2845bbc9a972211eb9b35e071f992b1a49d2612e6a'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (42095,
                                                        '5001c975781afe8880e9e76635ed23f64f473f3477d3a0bb52282fb1ad1928ef'),
 'src/frontend/oir/owned/source/experimental_lower.rs': (11199,
                                                         '682490518f6a418d88560f70ec24e2955a14b0cd44b0cec839b16bb1279a02d5'),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': (24646,
                                                                '16bd247554dd4656ab2593df24c3de54be3b1b18e3366dc17ebe4be7b6ec9336'),
 'src/frontend/oir/owned/source/hir_budget.rs': (56845,
                                                 '1f491f2f39af177786e534bd89761dd64b00fef7b9761ae455ac78ed86248af7'),
 'src/frontend/oir/owned/source/hir_budget_tests.rs': (54396,
                                                       '79b90da51643c762e3731d6068bb6084d77fda6da861e6428e0f3742a89ffb54'),
 'src/frontend/oir/owned/source/resolve.rs': (141137,
                                              '7c15f3e8c143cc5663eefac1d1b3ef223a06384a16f8689bd80db21cc8878af2'),
 'src/frontend/oir/owned/source/typeck.rs': (209394,
                                             'c9764db813409b30cb0519c53de7c07f31599f0f1fc404a6f166468e575e17cd'),
 'src/frontend/oir/owned/source/u8_tests.rs': (27594,
                                               '90c6a3e22289ba04c6cd5229e5d021a85f4e9fe5d02ff2dcfc52b7ad3c7b3202'),
 'src/frontend/oir/owned_types.rs': (77800,
                                     '73eabb8990ebb01fa43a689bc0014c0f76638e8962d1a30d8ac795efd93f234b'),
 'src/frontend/oir/owned_types/enums.rs': (54820,
                                           'e39b39769eee2802ff3d7297110c410a432720f012605ccd7ee2ac3fce8e0aa9'),
 'src/frontend/oir/owned_types/resources.rs': (24488,
                                               '6f5d3ee03e2f8290e8e37dedc8ce0979d58c7f63440fd238c5a721d3a1cbc770')}


REVIEWED_DIRECTIVES = {'src/frontend/native.rs': ([b'include_str!("../../native/typed_preview.c")',
                             b'include_str!("../../native/typed_bytebuf.c")',
                             b'include_str!("../../native/experimental_bytebuf.c")',
                             b'include_str!("../../native/experimental_files.c")',
                             b'include_str!("../../native/experimental_bytebuf.h")'],
                            []),
 'src/frontend/oir/owned/byte_buffer.rs': ([], [b'#[path = "file_resources.rs"]']),
 'src/frontend/oir/owned/execute_experimental.rs': ([],
                                                    [b'#[path = "execute_io_delivery.rs"]',
                                                     b'#[path = "execute_file_delivery.rs"]']),
 'src/frontend/oir/owned/file_resources.rs': ([], [b'#[path = "file_resource_tests.rs"]'])}

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
            "changed pinned byte-string source policy")
    module = types.ModuleType("_native_file_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | FILE_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


OBSERVER_ADAPTER = "scripts/native_file_observer.py"
OBSERVER_ADAPTER_BYTES = 4103
OBSERVER_ADAPTER_SHA = '70810d5315b864aa81a1323d5ac2bfac571870ac8675264325dd34b5d6eaed8f'


def authenticate_observer_adapter(repo):
    body = regular(repo, OBSERVER_ADAPTER)
    require(len(body) == OBSERVER_ADAPTER_BYTES and sha(body) == OBSERVER_ADAPTER_SHA,
            "changed pinned native-file observer adapter")
    adapter = types.ModuleType("_native_file_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    return adapter, body


def run_file_observer_controls(repo, output, captured, seam, original, adapter):
    """Run File exclusions in addition to all unchanged prior observer controls."""
    run = output / "unit2/run"
    controls = output / "native-file-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "changed control protocol")
    protocol = types.ModuleType("_file_observer_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary = Path(receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "file control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "file control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.FILE_CONTROL_FILTER]),
                               ("run", [adapter.FILE_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            with out.open("xb") as stdout, err.open("xb") as stderr:
                completed = subprocess.run(argv, cwd=run / "source", stdout=stdout, stderr=stderr, timeout=60)
            commands.append({"argv": argv, "exit_status": completed.returncode,
                             "stdout": identity(out.name, out.read_bytes()),
                             "stderr": identity(err.name, err.read_bytes())})
            (controls / (profile + "-commands.json")).write_bytes(encoded(commands))
            require(completed.returncode == 0, "File observer controls failed; see retained streams")
            streams[label] = out.read_text(encoding="utf-8")
        names = list(adapter.FILE_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], [name + " - should panic" for name in names])
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "file control binary changed")
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "file observer source binding changed")
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        original.assert_unchanged(repo, captured)
        record = {"status": "passed", "profile": profile, "tests": names, "result": result,
                  "binary": identity(str(binary), binary.read_bytes()),
                  "source_inputs_sha256": original.CURRENT_SOURCE_SHA,
                  "commands": identity(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        path.write_bytes(encoded(record))
        receipts.append(identity(str(path.relative_to(output)), path.read_bytes()))
    return {"native_file_observer_control_tests_per_profile": 3,
            "native_file_observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6 + 3 + 3)}


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
    expected = expected_parent | FILE_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(FILE_INPUTS) == FILE_CHANGED | FILE_ADDED
            and FILE_CHANGED <= expected_parent and not FILE_ADDED & expected_parent,
            "wrong reviewed native-file input membership")
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
        if name not in FILE_CHANGED:
            require(inputs[name] == before, "unreviewed change outside native-file boundary: " + name)
    for name, (length, digest) in FILE_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "native-file source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "native-file compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "native_file_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(FILE_INPUTS.items())],
              "native_file_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(FILE_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_native_file_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the native-file successor; native files qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_native_file_compatibility.py", Path(__file__).read_bytes())}
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
            verified.update(run_file_observer_controls(repo, output, selected, seam, original, observer_adapter))
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
