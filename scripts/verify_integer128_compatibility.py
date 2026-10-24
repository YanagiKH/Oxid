#!/usr/bin/env python3
"""Replay retained narrow assertions after checked 128-bit arithmetic.

The complete integer-width predecessor and twelve preceding source policies
remain byte-authenticated. 128-bit semantics use separate public/native oracles.
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

PREDECESSOR = '5c350ff06f7b1fa74a867329488921ae0601543b'
PREDECESSOR_POLICY = 'scripts/verify_byte_integer_compatibility.py'
PREDECESSOR_POLICY_SHA = '674c0443965117a218c6990c30ccbe558261608acac00c8813d27894b326065a'
PREDECESSOR_POLICY_BYTES = 19302
INTEGER_CHANGED = frozenset(('src/frontend/ast.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/driver.rs', 'src/frontend/hir.rs', 'src/frontend/native.rs', 'src/frontend/oir/comparison_tests.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/enum_reference_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_integers.rs', 'src/frontend/oir/owned/execute_values.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_integers.rs', 'src/frontend/oir/owned/native_storage.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/program.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/oir/verify.rs', 'src/frontend/parser/conversions.rs'))
INTEGER_ADDED = frozenset(('native/typed_integer128.c', 'src/frontend/oir/owned/execute_integer128.rs', 'src/frontend/oir/owned/native_integer128.rs', 'src/frontend/oir/owned/source/integer128_tests.rs'))
INTEGER_INPUTS = {'native/typed_integer128.c': (4909,
                               'c09bbab80252cbf2d6d2f661287b1eb0a53b99071f12d63f6b60a290046f2ac8'),
 'src/frontend/ast.rs': (46362,
                         '18f4797b97aad23272aae330a514fc3a1a58f24fc00c84baf3b058c3a8c6c2e8'),
 'src/frontend/declaration_index/u8_reservation.rs': (31489,
                                                      '57651dfb693b3b9578681da30ca0e4fad7283b06f9f4d569d42023794122c1f3'),
 'src/frontend/driver.rs': (82323,
                            '22efa33dac901e8441398e693e6df21a6c0c25f127a4abe6ac9ba990db6e07b2'),
 'src/frontend/hir.rs': (30408,
                         '3c223d2281c462a6b089298cd301e929dfce2b3d8e29d7b377dbf1642ab40a0c'),
 'src/frontend/native.rs': (18127,
                            '286ba53995db1481f3c3b5cef3e2c5556968fb3874477ba8e1ce3fb3c01ae296'),
 'src/frontend/oir/comparison_tests.rs': (9748,
                                          '917206623921d5e835f15a09e8da6e56f0eee25d2504dd06e92131fd19c0f19c'),
 'src/frontend/oir/execute.rs': (22381,
                                 'e1a8763c75e6215ebaae04a0cf666d6d05b0a60a4c0a9fae068a58a1da713b22'),
 'src/frontend/oir/mod.rs': (21805,
                             '894b7d1a7b1569cf87f3a763ce6a24bc7aa860fb0a80df6894a12cb7592f511f'),
 'src/frontend/oir/native.rs': (106438,
                                'f536ff8548b0fe729075bd04efb64a904b3efa740c782d43ed70d03eec7e9ed4'),
 'src/frontend/oir/native_emit_work.rs': (29054,
                                          'c2601325c5836b78cbd7942073f617e7921600d36a991a510b35baa0c0a820c9'),
 'src/frontend/oir/owned/array_native_tests.rs': (52372,
                                                  '7b7f0c30c71c908fe4d5c6bfa271489df4af24e7bb711758666da2e05f0e71be'),
 'src/frontend/oir/owned/array_reference_boundary_tests.rs': (33960,
                                                              'a60d651d6f588338cd9bd2deca29dd64541653831ee6710c8bc9957f307b24c3'),
 'src/frontend/oir/owned/array_reference_tests.rs': (45787,
                                                     '73071fdf1775aa70750a83b1bc2d04dd3768a05e89e6b5d40b7769a347d826d0'),
 'src/frontend/oir/owned/array_tests.rs': (33997,
                                           '1d56cd15b8d8f9e31c9d11b1ca9932323e4366ec4694a191a0dba86d8fc4f986'),
 'src/frontend/oir/owned/enum_consumer_fixtures.rs': (31608,
                                                      '1090e3dabb13d490fc157d1fb56ca91487b24fa143f484b28f0d7789180aa012'),
 'src/frontend/oir/owned/enum_match_tests.rs': (50034,
                                                '9c5c51f1b2ad796543064ff2c1989d25cc95ca35fed588fb8b4ba32a8670bb2d'),
 'src/frontend/oir/owned/enum_native_tests.rs': (62573,
                                                 '3236cd5c811e47cc84bfd129fbce1dcb89b45e349a3645234e0e2d37069767ef'),
 'src/frontend/oir/owned/enum_reference_tests.rs': (43482,
                                                    'b69fd29d5bdcbba9000ac47ac0307a4df7c06f5b21d4b2138cb7d98224ea6196'),
 'src/frontend/oir/owned/execute.rs': (186493,
                                       '3e142c8d2e9c03ee1efd121e4e4057098e0fb8d1d398ec1c5568593e2bd1e392'),
 'src/frontend/oir/owned/execute_integer128.rs': (5015,
                                                  '009356a073b9b78774cd74bca842b25d8037da4383d1d05939b065c8430ff975'),
 'src/frontend/oir/owned/execute_integers.rs': (6460,
                                                '1d3a273777388b440e1c9fa60b72f278b5195e45657aaf01a03106ec054d3e09'),
 'src/frontend/oir/owned/execute_values.rs': (11552,
                                              '0a6c9d22356bd2ad61447c8d134897bfb47978dd829e1e149dfed5721e493726'),
 'src/frontend/oir/owned/native.rs': (229718,
                                      'fd942df6ad4d70e8b760633ce5c7fe807436ef64d1aa541d44cbc22fbad3a5ab'),
 'src/frontend/oir/owned/native_integer128.rs': (4656,
                                                 '8a3bbb3b5a327e5d83f8f94e0716feaf82f67741b39e6771494830ac963850bc'),
 'src/frontend/oir/owned/native_integers.rs': (6569,
                                               '2e5d70a43561dc67fe830e320f0c9ebd7fa87422c0f0bf76a81209be45054605'),
 'src/frontend/oir/owned/native_storage.rs': (19083,
                                              '6aaf91bd3d09c38f41af3650ac6ad75386677fbe055722b594545d5f39869d46'),
 'src/frontend/oir/owned/plan.rs': (29445,
                                    'a39b2f139f590cb172f0f6fa14e9458bb605353910bfb27ebc6421e482f296ed'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (66051,
                                                              '3dc3843026a1a7cdc22e303a173495d7b905d19d12ac1b8711d4f7d4d4cbcdf5'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (55197,
                                                          '4b86a0497c66271aea99a1b7207b708d37373a73530b971abd59492f9757cab9'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (43108,
                                                        '0bbcaf9109230c0da9e33a208e74730a56ba5cc742b9adfbfd47d3551fa996f3'),
 'src/frontend/oir/owned/source/hir.rs': (6390,
                                          '7e7d341f2b7c3cf43a91412e157cea419635053f17f293988db5ee026b6ea423'),
 'src/frontend/oir/owned/source/integer128_tests.rs': (4195,
                                                       'e4c9b6ec3fdd6f63d2e94735dd59de1ded510364f8daf7fd8656ec5d302fceec'),
 'src/frontend/oir/owned/source/lower.rs': (114947,
                                            '30fc4d3086ebf3c31ee47921c01e72ac9fb6f7c6fa9c2926f92ea6f834287179'),
 'src/frontend/oir/owned/source/mod.rs': (2047,
                                          '39d555ab9756d04b4104cfd20fc884c984509fc249ddaf0038d42d15a3bbc7c0'),
 'src/frontend/oir/owned/source/program.rs': (37466,
                                              '5810a4743775776bda523191d9428b0c579f17f9223e3a2e1768981aa5894f19'),
 'src/frontend/oir/owned/source/resolve.rs': (145140,
                                              'f6b54649be08bdb8113de0d8d897cc05563aea2dfe93415033a00e279a12a0ba'),
 'src/frontend/oir/owned/source/typeck.rs': (210721,
                                             'be26e82fe8e3932c8bc28983487b05875a7b5a163606ac9566a5e3e633bc621c'),
 'src/frontend/oir/owned_types.rs': (79660,
                                     '1cedc794fd8c3cec9549464fd9a05467af461ce56b09dbaae217e47acffcf7db'),
 'src/frontend/oir/owned_types/enums.rs': (57564,
                                           'ad43cba662cbd18a7ef5cd27c125396da296d108167b824c5023d556565a9300'),
 'src/frontend/oir/source/association.rs': (31667,
                                            '9d9f0a76213c458eae6da10e44f288fab0ec826b19499d22bbf8e63b8f8d85c1'),
 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs': (17544,
                                                                   '54395a807911f2310fe220b89677101f0d3ac4dae95a0fd6f186152ca4f52e3b'),
 'src/frontend/oir/source/sealed.rs': (23473,
                                       'a0f20b567490f4e985b9d3b02d20342381ade027e53034cce35a60bdab13255e'),
 'src/frontend/oir/verify.rs': (49751,
                                '24da20fff28f70278a29ec92004fefbf04d99de277500ff8b77853cb410b7164'),
 'src/frontend/parser/conversions.rs': (3207,
                                        '29c911581858f9db2d42d34dbde647b1aa23327fb3b29c5383890c98bdb0523a')}
REVIEWED_DIRECTIVES = {'native/typed_integer128.c': ([], []),
 'src/frontend/native.rs': ([b'include_str!("../../native/typed_preview.c")',
                             b'include_str!("../../native/typed_integer128.c")',
                             b'include_str!("../../native/typed_bytebuf.c")',
                             b'include_str!("../../native/experimental_bytebuf.'
                             b'c")',
                             b'include_str!("../../native/experimental_files.c"'
                             b')',
                             b'include_str!("../../native/experimental_argument'
                             b's.c")',
                             b'include_str!("../../native/experimental_bytebuf.'
                             b'h")'],
                            []),
 'src/frontend/oir/owned/execute.rs': ([],
                                       [b'#[path = "execute_values.rs"]',
                                        b'#[path = "execute_integer128.rs"]',
                                        b'#[path = "execute_integers.rs"]',
                                        b'#[path = "execute_experimental.rs"]',
                                        b'#[path = "execute_resources.rs"]',
                                        b'#[path = "array_observe.rs"]',
                                        b'#[path = "execute_tests.rs"]',
                                        b'#[path = "builtin_output_reference_t'
                                        b'ests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.r'
                                        b's"]']),
 'src/frontend/oir/owned/execute_integer128.rs': ([], []),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_experimental.rs"]',
                                       b'#[path = "native_integer128.rs"]',
                                       b'#[path = "native_integers.rs"]',
                                       b'#[path = "native_numeric.rs"]',
                                       b'#[path = "native_resources.rs"]',
                                       b'#[path = "builtin_input_native_tests'
                                       b'.rs"]',
                                       b'#[path = "builtin_output_native_test'
                                       b's.rs"]',
                                       b'#[path = "native_tests.rs"]',
                                       b'#[path = "native_inventory_admission'
                                       b'_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.'
                                       b'rs"]']),
 'src/frontend/oir/owned/native_integer128.rs': ([], []),
 'src/frontend/oir/owned/source/integer128_tests.rs': ([], [])}

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
            "changed pinned integer-width source policy")
    module = types.ModuleType("_integer_width_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | INTEGER_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


OBSERVER_ADAPTER = "scripts/integer128_compatibility_observer.py"
OBSERVER_ADAPTER_BYTES = 3990
OBSERVER_ADAPTER_SHA = '2395da52217e376742ed859a63d4be0dcf90fc935a2dde25f32e46c1727870e8'

def authenticate_observer_adapter(repo):
    body = regular(repo, OBSERVER_ADAPTER)
    require(len(body) == OBSERVER_ADAPTER_BYTES and sha(body) == OBSERVER_ADAPTER_SHA,
            "changed pinned 128-bit observer adapter")
    adapter = types.ModuleType("_integer128_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    return adapter, body

run_file_observer_controls = library.run_file_observer_controls
run_i64_observer_controls = library.run_i64_observer_controls
run_integer_width_observer_controls = library.run_integer_width_observer_controls


def run_integer128_observer_controls(repo, output, captured, seam, original, adapter):
    """Run i64 exclusions in addition to all unchanged prior observer controls."""
    run = output / "unit2/run"
    controls = output / "integer128-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "changed control protocol")
    protocol = types.ModuleType("_integer128_observer_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary = Path(receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "integer128 control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "integer128 control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.QUAD_CONTROL_FILTER]),
                               ("run", [adapter.QUAD_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
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
        names = list(adapter.QUAD_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], [name + " - should panic" for name in names])
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "integer128 control binary changed")
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "integer128 observer source binding changed")
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
    return {"integer128_observer_control_tests_per_profile": 4,
            "integer128_observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6 + 3 + 3 + 2 + 14 + 4)}


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
    expected = expected_parent | INTEGER_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(INTEGER_INPUTS) == INTEGER_CHANGED | INTEGER_ADDED
            and INTEGER_CHANGED <= expected_parent and not INTEGER_ADDED & expected_parent,
            "wrong reviewed 128-bit integer input membership")
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
        if name not in INTEGER_CHANGED:
            require(inputs[name] == before, "unreviewed change outside 128-bit integer boundary: " + name)
    for name, (length, digest) in INTEGER_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "128-bit integer source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "128-bit integer compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "integer128_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(INTEGER_INPUTS.items())],
              "integer128_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(INTEGER_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "integer128"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_integer128_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the 128-bit integer successor; 128-bit arithmetic qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_integer128_compatibility.py", Path(__file__).read_bytes())}
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
            verified.update(run_i64_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_integer_width_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_integer128_observer_controls(repo, output, selected, seam, original, observer_adapter))
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
