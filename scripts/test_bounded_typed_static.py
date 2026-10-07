"""Controller validation only; fake processes are never native evidence.

These tests run on every CI host. Actual parser, consumer, ELF, multi-file-loader
and kernel-I/O qualification is the separate Linux controller invocation.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_bounded_typed_static as gate


class SelectionTests(unittest.TestCase):
    def test_retained_roster_and_fixed_controls(self):
        cases = gate.load_cases(gate.FIXTURES / "cases.json")
        self.assertEqual(len(cases), 81)
        self.assertEqual(len(gate.RELAYS), 4)
        self.assertEqual(gate.digest(gate.FIXTURES / "malformed_controls.py"), gate.CONTROLS_SHA256)
        controls = gate.load_module("test_bounded_static_controls", gate.FIXTURES / "malformed_controls.py")
        records = controls.records()
        self.assertEqual(tuple(c["name"] for c in records["malformed"]), gate.MALFORMED_NAMES)
        self.assertEqual(len(records["malformed"]), 31)
        for name, expected in gate.IO_SOURCES.items():
            self.assertEqual(gate.digest(gate.FIXTURES / name), expected)
        for control in records["malformed"]:
            self.assertEqual(hashlib.sha256(control["data"]).hexdigest(), control["input_sha256"])
            self.assertLessEqual(control["consumed"], len(control["data"]))
        trailing = next(c for c in records["malformed"] if c["name"] == "trailing-two-witnesses")
        self.assertEqual(trailing["data"][trailing["consumed"]:], b"B")

    def test_missing_duplicate_renamed_changed_and_reclassified_rosters_fail(self):
        original = json.loads((gate.FIXTURES / "cases.json").read_bytes())
        changes = [lambda m: m.update(cases=[]), lambda m: m["cases"].pop(),
                   lambda m: m["cases"].append(copy.deepcopy(m["cases"][0])),
                   lambda m: m["cases"][0].update(name="different"),
                   lambda m: m["cases"][0].update(source=" "),
                   lambda m: m["cases"][0].update(role="refusal")]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "cases.json"
            for index, change in enumerate(changes):
                with self.subTest(change=index):
                    manifest = copy.deepcopy(original)
                    change(manifest)
                    gate.save(path, manifest)
                    with self.assertRaises(ValueError):
                        gate.load_cases(path)

    def test_root_closure_excludes_closed_probe_and_other_roots(self):
        with tempfile.TemporaryDirectory() as temporary:
            candidate, host = gate.snapshot_sources(Path(temporary) / "source")
            self.assertEqual(set(p.name for p in candidate.iterdir()), set(gate.MEMBERS))
            self.assertEqual(len(gate.MEMBERS), 30)
            for name in ("ast_consumer_probe.ox", "typed_main.ox", "static_main.ox", "resolver_main.ox"):
                self.assertFalse((candidate / name).exists())
            self.assertTrue((host / "static_typed_observation.py").is_file())


class BoundaryTests(unittest.TestCase):
    def test_exact_ast1_framing_and_failure_relay(self):
        source = b"fn f()->(){return;}"
        opa = b"OPA1" + bytes([0, 0, 0, 0, 0, 0, len(source)]) + bytes(1548)
        self.assertEqual(gate.frame_parser_output(opa, source), b"AST1" + bytes([len(source)]) + source + opa)
        failure = b"OPA1" + bytes([1, 17, 0, 1, 0, 0, len(source)])
        self.assertIsNone(gate.frame_parser_output(failure, source))
        for data in (b"", b"bad!" + opa[4:], opa[:-1], opa + b"x", failure + b"x",
                     opa[:10] + b"\x7f" + opa[11:], opa[:5] + b"\1" + opa[6:]):
            with self.subTest(size=len(data)):
                with self.assertRaises(ValueError):
                    gate.frame_parser_output(data, source)
        for data in (b" " * 129, b"\xff"):
            with self.assertRaises(ValueError):
                gate.frame_parser_output(opa, data)

    def test_named_record_refusal_has_two_explicit_spans(self):
        canonical = {"schema": "canonical-static-observation-1", "status": "outside_subset", "family": "record",
                     "span": {"file_id": 0, "start": 0, "end": 15}}
        case = {"name": "canonical-039", "source": "struct S{x:i32}"}
        actual = gate.expectation(case, canonical)
        self.assertEqual(actual["span"]["end"], 6)
        self.assertEqual(canonical["span"]["end"], 15)
        for changed_case, changed_canonical in (
            ({**case, "name": "unknown"}, canonical),
            ({**case, "source": "struct T{x:i32}"}, canonical),
            (case, {**canonical, "span": {"file_id": 0, "start": 0, "end": 6}}),
            (case, {**canonical, "family": "unknown"}),
            ({"name": "another"}, {"status": "stage_pending"}),
        ):
            with self.assertRaises(ValueError):
                gate.expectation(changed_case, changed_canonical)

    def test_exact_status_streams_and_consumption(self):
        gate.check_execution(subprocess.CompletedProcess([], 0, b"OPA1", b""), b"")
        gate.check_execution(subprocess.CompletedProcess([], 64, b"", b""), b"B",
                             status=64, expected_remaining=b"B", empty_output=True)
        for status, out, err, remaining in ((0, b"", b"", b"B"), (64, b"partial", b"", b"B"),
                                             (64, b"", b"error", b"B"), (64, b"", b"", b"")):
            with self.subTest(status=status, out=out, err=err, remaining=remaining):
                with self.assertRaises(ValueError):
                    gate.check_execution(subprocess.CompletedProcess([], status, out, err), remaining,
                                         status=64, expected_remaining=b"B", empty_output=True)


class ProcessReceiptTests(unittest.TestCase):
    def test_fake_prefix_process_binds_only_its_stdout_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            def fake(argv, **kwargs):
                overrides = kwargs["env"]
                self.assertEqual(set(overrides), {"LD_PRELOAD", "OXID_STDOUT_PREFIX_BYTES",
                                                 "OXID_STDOUT_TARGET_DEV", "OXID_STDOUT_TARGET_INO"})
                self.assertEqual(overrides["LD_PRELOAD"], str(root / "fake-shim.so"))
                self.assertEqual(overrides["OXID_STDOUT_PREFIX_BYTES"], "2")
                stat = os.fstat(kwargs["stdout"])
                self.assertEqual(overrides["OXID_STDOUT_TARGET_DEV"], str(stat.st_dev))
                self.assertEqual(overrides["OXID_STDOUT_TARGET_INO"], str(stat.st_ino))
                self.assertEqual(os.read(kwargs["stdin"], 3), b"abc")
                os.write(kwargs["stdout"], b"xy")
                return subprocess.CompletedProcess(argv, 74, None, b"")
            with patch.object(gate.subprocess, "run", side_effect=fake):
                result, remaining = gate.run_process(root / "prefix", ["fake"], b"abc", root,
                                                     clear=True, prefix=2, shim=root / "fake-shim.so")
            self.assertEqual(result.stdout, b"xy")
            self.assertEqual(remaining, b"")
            receipt = json.loads((root / "prefix/receipt.json").read_bytes())
            self.assertEqual(receipt["stdout_sha256"], hashlib.sha256(b"xy").hexdigest())
            self.assertEqual(receipt["injected_prefix_bytes"], 2)

    def test_prefix_rejects_unrelated_preload_without_launching(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.dict(gate.os.environ, {"LD_PRELOAD": "unrelated"}), patch.object(gate.subprocess, "run") as run:
                with self.assertRaises(ValueError):
                    gate.run_process(root / "prefix", ["fake"], b"abc", root,
                                     prefix=1, shim=root / "fake-shim.so")
                run.assert_not_called()
            receipt = json.loads((root / "prefix/receipt.json").read_bytes())
            self.assertEqual(receipt["status"], "execution-error")
            self.assertEqual((root / "prefix/remaining-stdin.bin").read_bytes(), b"abc")

    def test_fake_process_records_exact_remaining_bytes_and_cleared_environment(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            def fake(argv, **kwargs):
                self.assertEqual(argv, ["fake-executable"])
                self.assertEqual(kwargs["env"], {})
                self.assertEqual(os.read(kwargs["stdin"], 4), b"1234")
                return subprocess.CompletedProcess(argv, 64, b"", b"")
            with patch.object(gate.subprocess, "run", side_effect=fake):
                result, remaining = gate.run_process(root / "case", ["fake-executable"], b"1234B", root, clear=True)
            self.assertEqual(result.returncode, 64)
            self.assertEqual(remaining, b"B")
            receipt = json.loads((root / "case/receipt.json").read_bytes())
            self.assertEqual(receipt["input_consumed"], 4)
            self.assertEqual(receipt["remaining_hex"], "42")
            self.assertEqual((root / "case/remaining-stdin.bin").read_bytes(), b"B")
            self.assertEqual(receipt["stdout_sha256"], hashlib.sha256(b"").hexdigest())

    def test_timeout_retains_partial_streams_and_consumption(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            def fake(argv, **kwargs):
                os.read(kwargs["stdin"], 1)
                raise subprocess.TimeoutExpired(argv, 120, output=b"partial", stderr=b"detail")
            with patch.object(gate.subprocess, "run", side_effect=fake):
                with self.assertRaises(subprocess.TimeoutExpired):
                    gate.run_process(root / "timeout", ["fake"], b"abc", root)
            receipt = json.loads((root / "timeout/receipt.json").read_bytes())
            self.assertEqual(receipt["status"], "timeout")
            self.assertEqual(receipt["input_consumed"], 1)
            self.assertEqual((root / "timeout/stdout").read_bytes(), b"partial")
            self.assertEqual((root / "timeout/stderr").read_bytes(), b"detail")
            self.assertEqual((root / "timeout/remaining-stdin.bin").read_bytes(), b"bc")

    def test_launch_failure_is_retained(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(gate.subprocess, "run", side_effect=OSError("fixture launch error")):
                with self.assertRaises(OSError):
                    gate.run_process(root / "failure", ["fake"], b"abc", root)
            receipt = json.loads((root / "failure/receipt.json").read_bytes())
            self.assertEqual(receipt["status"], "execution-error")
            self.assertIn("fixture launch error", receipt["error"])
            self.assertEqual(receipt["remaining_hex"], b"abc".hex())


class CompletionTests(unittest.TestCase):
    def setUp(self):
        self.cases = gate.load_cases(gate.FIXTURES / "cases.json")
        self.modes = ["reference", "native"]
        self.report = {
            "checks": [{"case": case["name"], "mode": mode, "passed": True,
                        "route": "parser-relay" if case["name"] in gate.RELAYS else "ast1-pair"}
                       for case in self.cases for mode in self.modes],
            "malformed": [{"case": name, "mode": mode, "passed": True}
                          for name in gate.MALFORMED_NAMES for mode in self.modes],
            "io": [{"case": name, "mode": mode, "passed": True} for name in gate.IO_NAMES for mode in self.modes],
            "baseline": [{"case": name, "mode": mode, "passed": True} for name in gate.BASELINE_NAMES for mode in self.modes],
            "injected": [{"case": name, "mode": mode, "passed": True} for name in gate.INJECTED_NAMES for mode in self.modes],
            "io_pairs": [{"group": group, "case": name, "passed": True}
                         for group, names in (("malformed", gate.MALFORMED_NAMES), ("io", gate.IO_NAMES),
                                              ("baseline", gate.BASELINE_NAMES), ("injected", gate.INJECTED_NAMES))
                         for name in names],
            "calibration": {"passed": True},
            "io_build": [{"name": name, "passed": True} for name in ("shim", "calibrator")],
            "wire_pairs": [{"case": case["name"], "passed": True} for case in self.cases],
            "setup": [{"root": root, "stage": stage, "passed": True}
                      for root in gate.ROOTS for stage in ("check", "compile")]}

    def test_nonzero_complete_counts(self):
        gate.completed(self.report, self.cases, self.modes)
        self.assertEqual(self.report["counts"]["native"],
                         {"ast1-pair": 77, "parser-relay": 4, "pending": 0, "malformed": 31, "io": 5,
                          "baseline": 2, "injected": 10, "io_executions": 48})
        self.assertEqual(len(self.report["io_pairs"]), 48)

    def test_reference_only_counts(self):
        for group in ("checks", "malformed", "io", "baseline", "injected"):
            self.report[group] = [c for c in self.report[group] if c["mode"] == "reference"]
        self.report["wire_pairs"] = []
        self.report["io_pairs"] = []
        self.report["setup"] = [c for c in self.report["setup"] if c["stage"] == "check"]
        gate.completed(self.report, self.cases, ["reference"])
        self.assertEqual(set(self.report["counts"]), {"reference"})

    def test_partial_duplicate_failed_and_wrong_identity_never_pass(self):
        for group in ("checks", "malformed", "io", "baseline", "injected", "wire_pairs", "io_pairs", "setup", "io_build"):
            for kind in ("empty", "missing", "duplicate", "failed", "wrong_identity"):
                with self.subTest(group=group, kind=kind):
                    report = copy.deepcopy(self.report)
                    if kind == "empty":
                        report[group] = []
                    elif kind == "missing":
                        report[group].pop()
                    elif kind == "duplicate":
                        report[group][0] = copy.deepcopy(report[group][-1])
                    elif kind == "failed":
                        report[group][0]["passed"] = False
                    else:
                        identity = "root" if group == "setup" else "name" if group == "io_build" else "case"
                        report[group][0][identity] = "unselected"
                    with self.assertRaises(ValueError):
                        gate.completed(report, self.cases, self.modes)

    def test_missing_or_failed_calibration_never_passes(self):
        for value in ({}, {"passed": False}):
            self.report["calibration"] = value
            with self.assertRaises(ValueError):
                gate.completed(self.report, self.cases, self.modes)


if __name__ == "__main__":
    unittest.main()
