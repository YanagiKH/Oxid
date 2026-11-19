"""Runtime-oracle channel controls; no fake process compiles or executes Oxid."""
from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("scalar_runtime_verifier", ROOT / "tests/verify.py")
verifier = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verifier)

# Independent expected bytes from emit.ox/typed_preview.c and diagnostic tuples
# from frontend/oir/mod.rs; neither is derived from the verifier's lookup table.
FAMILIES = (
    ("overflow", b"integer overflow\n", "E0604", "checked i32 arithmetic overflow"),
    ("division by zero", b"division by zero\n", "E0607", "checked i32 division by zero"),
)


def case(family):
    return {"name": "runtime-" + family.replace(" ", "-"), "kind": "runtime", "error": family}


def result(stdout=b"", stderr=b"", status=1):
    return subprocess.CompletedProcess(["protocol-only-validator-input"], status, stdout, stderr)


def records(code, message):
    return [{
        "schema_version": 1, "edition": "typed-preview", "kind": "diagnostic",
        "severity": "error", "code": code, "stage": "oir-run", "message": message,
        "primary": {"file_id": 0, "path": "main.ox", "start": 22, "end": 25,
                    "line": 1, "column": 23, "end_line": 1, "end_column": 26},
        "secondary": [], "notes": [],
    }, {
        "schema_version": 1, "edition": "typed-preview", "kind": "run-summary",
        "success": False, "errors": 1, "result": None,
    }]


def framed(rows):
    return b"".join(json.dumps(row, ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n"
                    for row in rows)


class RuntimeDiagnostics(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cases, _ = verifier.fixture_data.load_cases(ROOT.parents[1])

    def reject_native(self, selected, value):
        with self.assertRaises(RuntimeError):
            verifier.check_native_behavior(selected, value)

    def reject_reference(self, selected, value):
        with self.assertRaises(RuntimeError):
            verifier.check_reference_behavior(selected, value)

    def test_all_nine_original_runtime_cases_use_their_own_native_and_reference_family(self):
        runtime = [selected for selected in self.cases if selected["kind"] == "runtime"]
        self.assertEqual(len(runtime), 9)
        for selected in runtime:
            for family, native, code, message in FAMILIES:
                with self.subTest(case=selected["name"], family=family):
                    native_result = result(stderr=native)
                    reference_result = result(stdout=framed(records(code, message)))
                    if selected["error"] == family:
                        verifier.check_native_behavior(selected, native_result)
                        verifier.check_reference_behavior(selected, reference_result)
                    else:
                        self.reject_native(selected, native_result)
                        self.reject_reference(selected, reference_result)

    def test_native_errors_require_exact_fixed_bytes_and_empty_stdout(self):
        for family, native, _, _ in FAMILIES:
            selected = case(family)
            for value in (
                result(stderr=native, status=0), result(stderr=native, status=65),
                result(stderr=native, status=-13), result(stdout=b"0\n", stderr=native),
                result(stderr=b"banner\n" + native), result(stderr=native + b"banner\n"),
                result(stderr=b"\x1b[31m" + native), result(stderr=native + b"\x00"),
                result(stderr=native * 2), result(stderr=native[:-1]), result(),
                result(stderr=b"error[E0400] (resolve-project): declaration index count overflow\n"),
            ):
                with self.subTest(family=family, value=value):
                    self.reject_native(selected, value)

    def test_frontend_count_overflow_counterexample_cannot_satisfy_runtime_oracle(self):
        selected = case("overflow")
        self.reject_reference(selected, result(
            stderr=b"error[E0400] (resolve-project): declaration index count overflow\n"))
        wrong = records("E0400", "declaration index count overflow")
        wrong[0]["stage"] = "resolve-project"
        self.reject_reference(selected, result(stdout=framed(wrong)))
        wrong[0]["code"] = "E0604"
        wrong[0]["stage"] = "oir-run"
        self.reject_reference(selected, result(stdout=framed(wrong)))
        # Expected words in location/notes cannot repair the wrong diagnostic.
        wrong[0]["primary"]["path"] = "work-overflow-add/main.ox"
        wrong[0]["notes"] = ["checked i32 arithmetic overflow"]
        self.reject_reference(selected, result(stdout=framed(wrong)))

    def test_reference_requires_exact_runtime_code_stage_family_and_diagnostic_shape(self):
        for family, _, code, message in FAMILIES:
            good = records(code, message)
            for key, value in (
                ("code", "E0400"), ("stage", "resolve-project"),
                ("message", "declaration index count overflow"),
                ("message", "checked i64 arithmetic overflow"), ("message", message + " banner"),
                ("severity", "warning"), ("kind", "run-summary"), ("edition", "legacy"),
                ("schema_version", True), ("schema_version", 1.0), ("notes", [message]),
                ("secondary", [{"span": good[0]["primary"], "message": message}]),
                ("unexpected", "field"),
            ):
                wrong = copy.deepcopy(good)
                wrong[0][key] = value
                with self.subTest(family=family, key=key, value=value):
                    self.reject_reference(case(family), result(stdout=framed(wrong)))

    def test_reference_rejects_ansi_banner_extra_and_partial_channels(self):
        for family, _, code, message in FAMILIES:
            stream = framed(records(code, message))
            for value in (
                result(stdout=stream, status=0), result(stdout=stream, status=2),
                result(stdout=stream, stderr=b"banner\n"),
                result(stdout=b"banner\n" + stream), result(stdout=stream + b"0\n"),
                result(stdout=b"\x1b[31m" + stream), result(stdout=stream + b"\x1b[0m"),
                result(stdout=stream + b"{}\n"), result(stdout=stream + b"\n"),
                result(stdout=stream * 2), result(stdout=stream[:-1]),
                result(stdout=stream.replace(b"\n", b"\r\n")), result(stdout=b" " + stream),
            ):
                with self.subTest(family=family, value=value):
                    self.reject_reference(case(family), value)

    def test_reference_rejects_malformed_duplicate_and_non_object_json(self):
        stream = framed(records("E0604", "checked i32 arithmetic overflow"))
        duplicate_code = stream.replace(b'"code":"E0604"', b'"code":"E0400","code":"E0604"', 1)
        duplicate_span = stream.replace(b'"file_id":0', b'"file_id":1,"file_id":0', 1)
        for corrupted in (
            duplicate_code, duplicate_span, b"not-json\n{}\n", b"[]\n{}\n",
            stream.replace(b"main.ox", b"\xffmain.ox", 1),
            stream.replace(b"\n", b" trailing\n", 1),
        ):
            with self.subTest(corrupted=corrupted):
                self.reject_reference(case("overflow"), result(stdout=corrupted))

    def test_reference_summary_and_primary_reject_near_valid_types_and_extents(self):
        good = records("E0604", "checked i32 arithmetic overflow")
        for key, value in (
            ("schema_version", True), ("schema_version", 1.0), ("success", 0), ("errors", True),
            ("errors", 1.0),
            ("errors", 2), ("result", 42), ("kind", "diagnostic"), ("extra", "field"),
        ):
            wrong = copy.deepcopy(good)
            wrong[1][key] = value
            with self.subTest(summary=key, value=value):
                self.reject_reference(case("overflow"), result(stdout=framed(wrong)))
        for key, value in (
            ("file_id", True), ("start", -1), ("end", 21), ("line", 0),
            ("column", 0), ("end_column", 22), ("path", ""), ("path", "\x1b[31moverflow.ox"),
            ("extra", 1),
        ):
            wrong = copy.deepcopy(good)
            wrong[0]["primary"][key] = value
            with self.subTest(primary=key, value=value):
                self.reject_reference(case("overflow"), result(stdout=framed(wrong)))
        valid_unicode = copy.deepcopy(good)
        valid_unicode[0]["primary"]["path"] = "資料-👩‍💻.ox"
        verifier.check_reference_behavior(case("overflow"), result(stdout=framed(valid_unicode)))

    def test_original_value_and_reject_reference_contracts_are_preserved(self):
        self.assertEqual(len(self.cases), 42)
        for selected in self.cases:
            if selected["kind"] == "value":
                value = result(stdout=selected["stdout"].encode(), status=0)
                verifier.check_native_behavior(selected, value)
                verifier.check_reference_behavior(selected, value)
                self.reject_native(selected, result(stdout=value.stdout, stderr=b"banner\n", status=0))
                self.reject_reference(selected, result(stdout=b"wrong value\n", status=0))
            elif selected["kind"] == "reject":
                verifier.check_reference_behavior(selected, result(status=65))
                self.reject_reference(selected, result(status=0))
                self.reject_native(selected, result(status=65))

    def test_unknown_failure_families_and_crossed_native_reference_channels_are_rejected(self):
        unknown = case("resource count overflow")
        self.reject_native(unknown, result(stderr=b"integer overflow\n"))
        self.reject_reference(unknown, result(stdout=framed(records("E0604", "checked i32 arithmetic overflow"))))
        for family, native, code, message in FAMILIES:
            self.reject_native(case(family), result(stdout=framed(records(code, message))))
            self.reject_reference(case(family), result(stderr=native))


if __name__ == "__main__":
    unittest.main()
