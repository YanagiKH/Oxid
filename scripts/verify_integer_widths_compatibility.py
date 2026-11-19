#!/usr/bin/env python3
"""Replay unchanged narrow semantics and resource assertions after integer widths.

The complete i64 predecessor and all ten preceding source policies remain
byte-authenticated. New integer semantics use independent runtime oracles.
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

PREDECESSOR = 'a3f76f786904b0625b7932f46fb7c5577a2c31a0'
PREDECESSOR_POLICY = 'scripts/verify_i64_compatibility.py'
PREDECESSOR_POLICY_SHA = '7836235ff5b845504afa87fe3c4dc973cf667cd493fe0f032ace633bdc91fd44'
PREDECESSOR_POLICY_BYTES = 40386
INTEGER_CHANGED = frozenset(('native/typed_preview.c', 'src/frontend/ast.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/oir/comparison_tests.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/lower.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/owned/array_native_resource_tests.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/enum_reference_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_values.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/byte_storage_type_tests.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/owned_types/u8_tests.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/verify.rs', 'src/frontend/parser.rs', 'src/frontend/parser/arrays.rs', 'src/frontend/parser/conversions.rs', 'src/frontend/parser/enums.rs', 'src/frontend/parser/u8_syntax_tests.rs', 'src/frontend/typeck.rs'))
INTEGER_ADDED = frozenset(('src/frontend/oir/owned/execute_integers.rs', 'src/frontend/oir/owned/native_integers.rs', 'src/frontend/oir/owned/source/integer_width_tests.rs'))
INTEGER_INPUTS = {'native/typed_preview.c': (4173, 'b88ebe95c4630805a9a0542bc7bde8b887b1dad8d1bcf8afa7bf313da5020f71'),
 'src/frontend/ast.rs': (45057, 'd09c263e87974721162ed51facaabd3c8e55b086a6404f2c319a9c402f6e2f22'),
 'src/frontend/declaration_index.rs': (77055,
                                       'ce68bbc7659848ed8b2d1bb41380521826046770fda77ea9c828bd3054226ac8'),
 'src/frontend/declaration_index/enum_views.rs': (11252,
                                                  '2bd03ebead9f1c9691b7a9d916968da748c13042a56f17116da52653b9d3adc0'),
 'src/frontend/declaration_index/u8_reservation.rs': (30957,
                                                      '5a24db8635bddf54e63536dd9bf95a3d704a6062738f478b0e064a841b7830dd'),
 'src/frontend/format.rs': (49439, '9d1b67b144fafd0c223933f86a9e99338303d5933d57f2a3f733ed09d2acb897'),
 'src/frontend/format/ast_tests.rs': (66483,
                                      'c86ca9bee45e72111cf110ce199e470e97d9d8abe64c9880660d718c16b360e9'),
 'src/frontend/hir.rs': (29921, 'b4b849094181ed03c2fed177866503bbc31eae716085f4adc380e19dbab4a938'),
 'src/frontend/oir/comparison_tests.rs': (9668,
                                          '6782797b42a50c9c5c539dc3b98549665697943d6ca06a00bee14b46a2951bca'),
 'src/frontend/oir/execute.rs': (22337, '5543e9274dc6f126719cd438c0915d295cbe159a14a22384c2f81bae5e188ca2'),
 'src/frontend/oir/lower.rs': (42215, 'cd9ea630d33806030777b95d0f6c2079f50b93f7e6034bd34f6550d185db50f5'),
 'src/frontend/oir/mod.rs': (18844, '26b998ef4bb49e2e12c4a39da24fe8b34839db11b6082217b78429c89f0f6e48'),
 'src/frontend/oir/native.rs': (106270, '3178affd393f0d2db3446c12a8355d6d79d24a5c03ba4d80bb26db9b1119e280'),
 'src/frontend/oir/native_emit_work.rs': (29014,
                                          '5fac53561d26de0849ecf690f2db773f18c437e3ad7a11cdf3757cf8c43a64fb'),
 'src/frontend/oir/owned/array_native_resource_tests.rs': (53192,
                                                           '4e224973519e96d66892b06b006c1115f61bd66fba5516bb4038cc21fb5484a6'),
 'src/frontend/oir/owned/array_native_tests.rs': (52204,
                                                  '4f349dcd9c005fc775a674e0c86f954b1f920cfb0a6e59aa05f9fc695b4e9133'),
 'src/frontend/oir/owned/array_reference_boundary_tests.rs': (33888,
                                                              'dd2eada8a5e8c6f79e4360bedbfc830113d074310faf1a91437fb1471648d925'),
 'src/frontend/oir/owned/array_reference_tests.rs': (45667,
                                                     '272751fbc19e4fb2f93fb007d68f95845e603b8e4a2cecbea833122cbc338c77'),
 'src/frontend/oir/owned/array_tests.rs': (33949,
                                           'faf5fa124faa9607494f49b6abc42f8a1c1de6a380b38ea91b67f4ffce146702'),
 'src/frontend/oir/owned/enum_consumer_fixtures.rs': (31512,
                                                      'bfe7b1a1e1eda11455b33e16948f377c38ade07d00437b585ca5e25b7a5e118e'),
 'src/frontend/oir/owned/enum_match_tests.rs': (49986,
                                                '32cd455815fd2eb40b6bc6c5126b455a6780aedeef13d1be0ef6de83f5f9a30b'),
 'src/frontend/oir/owned/enum_native_tests.rs': (62501,
                                                 '5b3b71a8835b0b0b0982abd7a4132a59abc8a8ccb7d120452146022814c27f7f'),
 'src/frontend/oir/owned/enum_reference_tests.rs': (43330,
                                                    'eb56aa2309a5a640997b21cf073b566977fabdda5f3d36a41a2f817b582a5649'),
 'src/frontend/oir/owned/execute.rs': (184686,
                                       'fd65f2f1b04720e47a163ef0c5cd2af609e935e105c854bae49ddf322c9646f2'),
 'src/frontend/oir/owned/execute_integers.rs': (5930,
                                                'f5de6fc68291120c3c2131275a8edac0095754f8b4132dc414ce5de3b111f60b'),
 'src/frontend/oir/owned/execute_values.rs': (8607,
                                              'efb66ea552409bdf8ba0a9090790baaac9a2d257ae6ab9e882708df3a11e991a'),
 'src/frontend/oir/owned/native.rs': (225956,
                                      'cdfb6c7b6526310df08631bde2c222e780b17737a7c3df5c5b67a3dd3574affe'),
 'src/frontend/oir/owned/native_integers.rs': (6292,
                                               'b0758ede713aa455852d1195772cc5ff97d9bc97264f277792cf55834778b7b7'),
 'src/frontend/oir/owned/plan.rs': (28053,
                                    '7caa7203fbfe536a482dfcd0d25df0c08d4770e25f5984cbd44bf93f7ca092ed'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (65955,
                                                              '6f706634c44ef30009897cb7ebc4f15878e81b7d12b7225757023805f9521562'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (55059,
                                                          '2fcf7d11a2960584088702583c50c54d4d6dc06a10e32abbc321fa63e71cb846'),
 'src/frontend/oir/owned/source/association.rs': (59908,
                                                  '613080c83e3bb910692b9c486cbb938371582531318e713c462c4d7c8206e3a5'),
 'src/frontend/oir/owned/source/byte_storage_type_tests.rs': (11491,
                                                              '65df9c5b057b42b27f528ee9b41f76ca189c147812f58989dcbfbb7deca05ac1'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (43024,
                                                        'b941681dfb4d6b0d864d95fac6af69a447af96ca3333abb365c142b73348e9ec'),
 'src/frontend/oir/owned/source/hir.rs': (6313,
                                          '439ffcb550e264d9bc9831fb76265c93ba6af6c6e6d50fa3e3b5866811303fcb'),
 'src/frontend/oir/owned/source/integer_width_tests.rs': (3439,
                                                          '0b69a43bc834af822c21010ac49e2d7d3ce4dd6936714df6efd0f52ccdb401fd'),
 'src/frontend/oir/owned/source/lower.rs': (114847,
                                            '6fd63bf4c0bdcc84e061af8a8c79e329b3efef3f5a0f7877132d343a9f84ff61'),
 'src/frontend/oir/owned/source/mod.rs': (1973,
                                          '8ef44370d52e62ae2e48fc01385364e331123fa3e4bf59536d3e36fe563f1f3d'),
 'src/frontend/oir/owned/source/resolve.rs': (143331,
                                              'cecb3f3e2e847a9dfd24ff8442386144e8e8104505feb192c7615f8177c48527'),
 'src/frontend/oir/owned/source/typeck.rs': (210731,
                                             '348831072cfff22138025e13e77b67d215b3f4664149c6721f12737727226036'),
 'src/frontend/oir/owned_types.rs': (79276,
                                     'dbe78ae88f1c53c532e4a3ecd382707a7380eaed9fec237b5e90bde66399c6f6'),
 'src/frontend/oir/owned_types/enums.rs': (55866,
                                           'ca629545d0474bfe02152595c41b286408a07f4672a575bf16913dc852ae31d8'),
 'src/frontend/oir/owned_types/u8_tests.rs': (3123,
                                              '85e427eccd6012920ed3ddaa37987e940016162377569298e37b1be98fe3aec9'),
 'src/frontend/oir/source/association.rs': (31619,
                                            '99f84cee48c47524cd23a541622c9c68f397267841a4d4792f21610e3230d7db'),
 'src/frontend/oir/source/hir_import/candidate.rs': (52676,
                                                     '76df87a88e17846c308e0d9f7b39bc88864e1be217203f34b53f52cba9411a8e'),
 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs': (17496,
                                                                   '4709dd29f58feee0e3596fb8592dce6b0712506f73a5f09c5e25a81cacd3777d'),
 'src/frontend/oir/verify.rs': (49460, '563988ae0b127d5fb70a2fb554f20de4812d5ddd9e6806ff029bf466f33b0ab9'),
 'src/frontend/parser.rs': (78027, 'cdd15c8845537af5025d95301aac9c533ed6b36469ea3ef97643a9d5871ba282'),
 'src/frontend/parser/arrays.rs': (10227, '3435b794986edae5ab6d1025fc8014242888f0afe336cba7de84e5caec76aa70'),
 'src/frontend/parser/conversions.rs': (2903,
                                        '77cc7ff8449a9fb980f121bc8cd078ea77a796f46ef683624a4e122140c989e6'),
 'src/frontend/parser/enums.rs': (16198, '04fb40cdeded594e7dfeb39f714728fc12bbf67dc50b276289b6faa014ba5d44'),
 'src/frontend/parser/u8_syntax_tests.rs': (12639,
                                            'f1ea657c89bf074a15a61609f978fa5283db6123e9a674eb9e7b27184138a48a'),
 'src/frontend/typeck.rs': (35587, '2dd2a1246a5d212aef0b967fee48857ffaaa385a06aca5a9ec60f550794bf1a0')}
REVIEWED_DIRECTIVES = {'src/frontend/oir/owned/execute.rs': ([],
                                       [b'#[path = "execute_values.rs"]',
                                        b'#[path = "execute_integers.rs"]',
                                        b'#[path = "execute_experimental.rs"]',
                                        b'#[path = "execute_resources.rs"]',
                                        b'#[path = "array_observe.rs"]',
                                        b'#[path = "execute_tests.rs"]',
                                        b'#[path = "builtin_output_reference_tests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.rs"]']),
 'src/frontend/oir/owned/execute_integers.rs': ([], []),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_experimental.rs"]',
                                       b'#[path = "native_integers.rs"]',
                                       b'#[path = "native_numeric.rs"]',
                                       b'#[path = "native_resources.rs"]',
                                       b'#[path = "builtin_input_native_tests.rs"]',
                                       b'#[path = "builtin_output_native_tests.rs"]',
                                       b'#[path = "native_tests.rs"]',
                                       b'#[path = "native_inventory_admission_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.rs"]']),
 'src/frontend/oir/owned/native_integers.rs': ([], []),
 'src/frontend/oir/owned/source/integer_width_tests.rs': ([], [])}

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
            "changed pinned i64 source policy")
    module = types.ModuleType("_i64_compatibility_parent")
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


OBSERVER_ADAPTER = "scripts/integer_width_compatibility_observer.py"
OBSERVER_ADAPTER_BYTES = 7509
OBSERVER_ADAPTER_SHA = 'a137fd88df8ce841bd182ec8549db33bb919a7c9c2c9f0f1b7df802754fab9de'


def authenticate_observer_adapter(repo):
    body = regular(repo, OBSERVER_ADAPTER)
    require(len(body) == OBSERVER_ADAPTER_BYTES and sha(body) == OBSERVER_ADAPTER_SHA,
            "changed pinned integer-width observer adapter")
    adapter = types.ModuleType("_i64_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    return adapter, body


run_file_observer_controls = library.run_file_observer_controls
run_i64_observer_controls = library.run_i64_observer_controls



def run_integer_width_observer_controls(repo, output, captured, seam, original, adapter):
    """Run i64 exclusions in addition to all unchanged prior observer controls."""
    run = output / "unit2/run"
    controls = output / "integer-width-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "changed control protocol")
    protocol = types.ModuleType("_integer_width_observer_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary = Path(receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "integer-width control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "integer-width control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.WIDTH_CONTROL_FILTER]),
                               ("run", [adapter.WIDTH_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
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
        names = list(adapter.WIDTH_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], [name + " - should panic" for name in names])
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "integer-width control binary changed")
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "integer-width observer source binding changed")
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
    return {"integer_width_observer_control_tests_per_profile": 14,
            "integer_width_observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6 + 3 + 3 + 2 + 14)}


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
            "wrong reviewed integer-width input membership")
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
            require(inputs[name] == before, "unreviewed change outside integer-width boundary: " + name)
    for name, (length, digest) in INTEGER_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "integer-width source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "integer-width compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "integer_widths_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(INTEGER_INPUTS.items())],
              "integer_widths_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(INTEGER_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_integer_widths_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the integer-width successor; integer semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_integer_widths_compatibility.py", Path(__file__).read_bytes())}
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
