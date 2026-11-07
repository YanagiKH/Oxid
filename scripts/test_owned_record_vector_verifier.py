"""Mutation controls for the public acceptance oracle, not compiler substitutes."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import verify_owned_record_vectors as gate


def completed(rows, status=1, stderr=b""):
    return subprocess.CompletedProcess([], status, b"\n".join(json.dumps(row).encode() for row in rows) + b"\n", stderr)


def diagnostic(code="E0310", stage="ownership"):
    return {"schema_version": 1, "edition": "typed-preview", "kind": "diagnostic",
            "severity": "error", "code": code, "stage": stage}


def summary(kind="check-summary", success=False):
    return {"schema_version": 1, "edition": "typed-preview", "kind": kind, "success": success}


class OwnedRecordVectorOracleTests(unittest.TestCase):
    def test_refusal_requires_the_exact_public_code_and_stage(self):
        case = next(case for case in gate.CASES if case.name == "double-move")
        self.assertEqual(gate.refused(completed([diagnostic(), summary()]), case), [("ownership", "E0310")])
        for code, stage in (("E0500", "oir-owned-verify"), ("E0400", "ownership"),
                            ("E0311", "ownership"), ("E0310", "type")):
            with self.subTest(code=code, stage=stage), self.assertRaises(ValueError):
                gate.refused(completed([diagnostic(code, stage), summary()]), case)

    def test_refusal_cannot_pass_on_a_success_missing_diagnostic_or_warning(self):
        case = next(case for case in gate.CASES if case.name == "double-move")
        warning = {**diagnostic(), "severity": "warning"}
        for result in (completed([diagnostic(), summary()], 0), completed([summary()]),
                       completed([warning, summary()]), completed([diagnostic(), summary(success=True)]),
                       completed([diagnostic(), summary()], stderr=b"unexpected\n")):
            with self.subTest(result=result), self.assertRaises(ValueError):
                gate.refused(result, case)

    def test_unknown_extra_diagnostic_and_wrong_summary_are_not_ignored(self):
        case = next(case for case in gate.CASES if case.name == "double-move")
        for rows in ([diagnostic(), diagnostic("E0500"), summary()],
                     [diagnostic(), summary("compile-summary")],
                     [diagnostic(), {**summary(), "schema_version": 2}]):
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                gate.refused(completed(rows), case)

    def test_runtime_error_keeps_exact_status_code_and_empty_result_output(self):
        case = next(case for case in gate.CASES if case.name == "fault-pop")
        good = b"error[E0606] (oir-owned-run): array index out of bounds\n"
        gate.outcome(subprocess.CompletedProcess([], 1, b"", good), case)
        for result in (subprocess.CompletedProcess([], 0, b"", good),
                       subprocess.CompletedProcess([], 1, b"99\n", good),
                       subprocess.CompletedProcess([], 1, b"", good + good),
                       subprocess.CompletedProcess([], 1, b"", b"error[E0500]: failure\n"),
                       subprocess.CompletedProcess([], 1, b"", b"")):
            with self.subTest(result=result), self.assertRaises(ValueError):
                gate.outcome(result, case)

    def test_numeric_result_is_an_exact_independent_oracle(self):
        bytebuf = next(case for case in gate.CASES if case.name == "bytebuf")
        gate.outcome(subprocess.CompletedProcess([], 0, b"58\n", b""), bytebuf)
        for result in (subprocess.CompletedProcess([], 58, b"", b""),
                       subprocess.CompletedProcess([], 0, b"57\n", b""),
                       subprocess.CompletedProcess([], 0, b"58", b""),
                       subprocess.CompletedProcess([], 0, b"58\n", b"warning\n")):
            with self.subTest(result=result), self.assertRaises(ValueError):
                gate.outcome(result, bytebuf)

    def test_corpus_has_the_full_finite_slice_and_no_internal_error_allowance(self):
        self.assertEqual(len(gate.CASES), 20)
        self.assertEqual(len({case.name for case in gate.CASES}), 20)
        self.assertEqual({case.name for case in gate.CASES if case.diagnostics},
                         {"double-move", "conflicting-borrows", "copy-operation", "index-read", "nested-vector"})
        self.assertEqual({case.name for case in gate.CASES if case.native},
                         {case.name for case in gate.CASES if not case.diagnostics})
        self.assertFalse(any(code == "E0500" for case in gate.CASES for _, code in case.diagnostics))
        for name in ("bytebuf", "enum16", "files", "package"):
            case = next(case for case in gate.CASES if case.name == name)
            source = (gate.ROOT / gate.DATA / case.source).read_text()
            for operation in ("new", "len", "capacity", "try_reserve", "push_reserved", "pop_last", "truncate", "take"):
                self.assertIn("use alloc::vec::" + operation + ";", source)

    def test_real_heap_failure_keeps_the_incoming_owned_input_and_descriptor(self):
        case = next(case for case in gate.CASES if case.name == "failed-reserve")
        self.assertTrue(case.hosted_only)
        self.assertEqual(case.heap_limit, 512)
        source = (gate.ROOT / gate.DATA / case.source).read_text()
        self.assertLess(source.index("let incoming = make(9);"), source.index("let failed = try_reserve"))
        self.assertLess(source.index("let failed = try_reserve"), source.index("push_reserved(&mut values, incoming);"))
        self.assertIn("GrowStatus::MemoryLimit => {}", source)
        self.assertEqual(source.count("len(&values) != old_length || capacity(&values) != old_capacity"), 2)
        self.assertIn("heap-limit = 512\n", gate.manifest("failure", "hosted", 512))
        with self.assertRaises(ValueError):
            gate.manifest("failure", "bounded", 512)

    def test_environment_and_policy_reference_cannot_claim_an_inherited_replay(self):
        self.assertEqual(set(gate.ENV_NAMES), {"PATH", "LANG", "LC_ALL", "TZ", "SOURCE_DATE_EPOCH",
                                             "LD_LIBRARY_PATH", "TMPDIR", "OXID_LLVM_BIN", "OXID_CACHE_DIR"})
        self.assertEqual(gate.POLICY_COMMIT, "af0fe5e1fbaf4fb49b62e894a53ad4b5ed307ee7")
        self.assertEqual(gate.POLICY, "scripts/verify_registry_compatibility.py")
        self.assertIn(gate.POLICY, gate.input_paths())
        self.assertIn("scripts/verify_scalar_library_compatibility.py", gate.input_paths())

    def test_input_reader_refuses_parent_paths_and_symlinked_files(self):
        with tempfile.TemporaryDirectory(prefix="oxid-owned-vector-oracle-") as folder:
            root = Path(folder)
            (root / "source.ox").write_text("fn main()->i32{return 0;}")
            self.assertEqual(gate.regular(root, "source.ox"), root / "source.ox")
            for path in ("../source.ox", str(root / "source.ox"), "missing.ox"):
                with self.subTest(path=path), self.assertRaises(ValueError):
                    gate.regular(root, path)
            (root / "link.ox").symlink_to("source.ox")
            with self.assertRaises(ValueError):
                gate.regular(root, "link.ox")


if __name__ == "__main__":
    unittest.main()
