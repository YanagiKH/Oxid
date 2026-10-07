"""Focused selection, process-boundary and evidence controls for the parser gate."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import verify_bounded_typed_parser as gate


class SelectionTests(unittest.TestCase):
    def test_exact_nonzero_selection(self):
        cases, transport = gate.load_cases(gate.FIXTURES / "cases.json")
        self.assertEqual(len(cases), 139)
        self.assertEqual(len(transport), 2)
        self.assertEqual(sum(case["role"] == "comparison" for case in cases), 118)
        self.assertEqual(sum(case["role"] == "refusal" for case in cases), 21)
        self.assertEqual(gate.digest(gate.FIXTURES / "corruption_controls.py"), gate.CONTROLS_SHA256)

    def test_reject_missing_duplicate_reclassified_or_changed_inputs(self):
        original = json.loads((gate.FIXTURES / "cases.json").read_bytes())
        changes = [lambda m: m.update(cases=[]),
                   lambda m: m["cases"].pop(),
                   lambda m: m["cases"].append(copy.deepcopy(m["cases"][0])),
                   lambda m: m["cases"][1].update(name=m["cases"][0]["name"]),
                   lambda m: m["cases"][0].update(role="refusal"),
                   lambda m: m["cases"][0].update(input_hex="20"),
                   lambda m: m["cases"][0].update(input_hex="ff"),
                   lambda m: m["cases"][0].update(input_hex="20" * 129),
                   lambda m: m["cases"][0].update(parser_status="stage_pending"),
                   lambda m: m["transport"].pop(),
                   lambda m: m["transport"][0].update(input_hex="20" * 128)]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "cases.json"
            for index, change in enumerate(changes):
                with self.subTest(change=index):
                    manifest = copy.deepcopy(original)
                    change(manifest)
                    gate.save(path, manifest)
                    with self.assertRaises(ValueError):
                        gate.load_cases(path)


class ExecutionTests(unittest.TestCase):
    def test_exact_exit_streams_and_input_consumption(self):
        gate.check_execution(subprocess.CompletedProcess([], 0, b"OPA1", b""), b"", b"abc")
        gate.check_execution(subprocess.CompletedProcess([], 64, b"", b""), b"", b" " * 129, True)
        for code, stdout, stderr, remaining, transport in (
                (64, b"", b"", b"", False),
                (0, b"", b"error", b"", False),
                (0, b"", b"", b"abc", False),
                (0, b"", b"", b"", True),
                (64, b"partial", b"", b"", True),
                (64, b"", b"error", b"", True)):
            with self.subTest(code=code, stdout=stdout, stderr=stderr, remaining=remaining, transport=transport):
                with self.assertRaises(ValueError):
                    gate.check_execution(subprocess.CompletedProcess([], code, stdout, stderr), remaining, b"abc", transport)

    def test_read_only_observer_identity_rejects_unbound_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "observer"
            output.mkdir()
            binary = output / "canonical-parser-observer"
            binary.write_bytes(b"binary")
            entries = []
            for original, copied in (("src/frontend/parser.rs", "parser.rs"),
                                     ("tests/fixtures/bounded_typed_parser/observer/main.rs", "main.rs")):
                path = root / original
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"unchanged source")
                (output / copied).write_bytes(path.read_bytes())
                entries.append({"original_path": original, "copied_path": copied, "sha256": gate.digest(path)})
            manifest = {"source_patches": [], "files": entries[:1], "wrappers": entries[1:], "commit": "frozen"}
            receipt = {"exit_code": 0, "binary": binary.name, "binary_sha256": gate.digest(binary)}
            builder = SimpleNamespace(SOURCE_FILES=("parser.rs",), WRAPPER_FILES=(("main.rs", "main.rs"),))
            gate.save(output / "source-manifest.json", manifest)
            gate.save(output / "build-evidence.json", receipt)
            with patch.object(gate, "ROOT", root):
                self.assertEqual(gate.observer_identity(binary, builder, "parser")["source_commit"], "frozen")
                for field, value in (("files", []), ("wrappers", []), ("source_patches", ["patch"])):
                    with self.subTest(field=field):
                        gate.save(output / "source-manifest.json", {**manifest, field: value})
                        with self.assertRaises(ValueError):
                            gate.observer_identity(binary, builder, "parser")
                gate.save(output / "source-manifest.json", manifest)
                (output / "parser.rs").write_bytes(b"altered")
                with self.assertRaises(ValueError):
                    gate.observer_identity(binary, builder, "parser")
                (output / "parser.rs").write_bytes(b"unchanged source")
                binary.write_bytes(b"other binary")
                with self.assertRaises(ValueError):
                    gate.observer_identity(binary, builder, "parser")


class CompletionTests(unittest.TestCase):
    def setUp(self):
        self.cases, _ = gate.load_cases(gate.FIXTURES / "cases.json")
        self.modes = ["reference", "native"]
        self.report = {
            "checks": [{"case": case["name"], "mode": mode, "role": case["role"], "passed": True}
                       for case in self.cases for mode in self.modes],
            "transport": [{"case": name, "mode": mode, "passed": True}
                          for name in ("ascii129", "non_ascii_byte") for mode in self.modes],
            "controls": [{"case": str(index), "mode": mode, "passed": True}
                         for index in range(41) for mode in self.modes]}

    def test_complete_counts(self):
        gate.completed(self.report, self.cases, self.modes)
        self.assertEqual(self.report["counts"]["native"], {
            "comparison": 118, "refusal": 21, "pending": 0, "transport": 2, "corruption_controls": 41})

    def test_partial_duplicate_failed_and_wrong_mode_never_pass(self):
        for group in ("checks", "controls", "transport"):
            for kind in ("empty", "missing", "duplicate", "failed", "wrong_mode"):
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
                        report[group][0]["mode"] = "unselected"
                    with self.assertRaises(ValueError):
                        gate.completed(report, self.cases, self.modes)


if __name__ == "__main__":
    unittest.main()
