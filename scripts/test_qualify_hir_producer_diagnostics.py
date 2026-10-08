"""Controller-only evidence checks; fake records are never producer qualification."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import qualify_hir_producer_diagnostics as gate


def records(operation="check"):
    return [dict(kind="diagnostic", severity="error", code="E0300", primary={"start": 1, "end": 2}),
            dict(kind=operation + "-summary", success=False, errors=1)]


def execution(source, opa, wire, hashes, version):
    ast = b"AST" + str(version).encode() + bytes([len(source)]) + source + opa
    result = []
    for role, data, output in zip(("parser", "static"), (source, ast), (opa, wire)):
        result.append(dict(kind="hir-producer", role=role, executable_sha256=hashes[role],
            input_sha256=gate.digest(data), input_written=len(data),
            captured_stdout_sha256=gate.digest(output), captured_stdout_bytes=len(output),
            captured_stderr_sha256=gate.digest(b""), captured_stderr_bytes=0,
            spawned=True, leader_reaped=True, stop="exited", exit_status=0, signal=None))
    return result


class DiagnosticControllerTests(unittest.TestCase):
    def test_roster_is_fixed_nonempty_and_covers_all_fourteen_kinds(self):
        self.assertEqual(len(gate.load_cases()), 81)
        self.assertEqual(len(gate.KINDS), 48)
        self.assertEqual(set(gate.KINDS.values()), set(range(1, 15)))
        with patch.object(gate, "ROSTER_SHA256", "changed"):
            with self.assertRaisesRegex(RuntimeError, "roster changed"):
                gate.load_cases()

    def test_extra_sources_retain_boundary_long_names_and_multi_error_controls(self):
        self.assertEqual(len(gate.extra_cases(1)), 4)
        self.assertEqual(len(gate.extra_cases(2)), 6)
        boundary = next(source for name, source, _ in gate.extra_cases(2) if name == "real-primary-end255")
        self.assertEqual(len(boundary), 255)
        self.assertEqual(boundary[-1:], b"}")
        self.assertTrue(all(len(source) <= 128 for _, source, _ in gate.extra_cases(1)))

    def test_only_producer_provenance_is_removed_preserving_every_other_field(self):
        original = records()
        original[0].update(message="message", secondary=[{"label": "first", "span": [2, 3]}], notes=["note"])
        with_provenance = [dict(kind="hir-producer", role="parser"), original[0],
                           dict(kind="hir-producer", role="static"), original[1]]
        self.assertEqual(gate.without_provenance(with_provenance), original)
        altered = copy.deepcopy(original)
        altered[0]["notes"] = []
        self.assertNotEqual(gate.without_provenance(with_provenance), altered)
        self.assertEqual(gate.without_provenance([{"kind": "runtime-output"}]), [{"kind": "runtime-output"}])

    def test_text_normalization_removes_only_two_exact_verified_provenance_lines(self):
        hashes = {"parser": "a" * 64, "static": "b" * 64}
        prefix = b"".join((f"experimental HIR producer {role} sha256={hashes[role]} "
            "spawned=true leader_reaped=true stop=exited exit=0 signal=null "
            f"captured_stdout={size} captured_stderr=0\n").encode()
            for role, size in (("parser", 3), ("static", 4)))
        diagnostic = b"error[E0300]: type mismatch\n"
        self.assertEqual(gate.without_text_provenance(prefix + diagnostic, hashes, b"opa", b"wire"), diagnostic)
        for changed in (diagnostic, prefix[::-1] + diagnostic,
                        prefix.replace(b"spawned=true", b"spawned=false") + diagnostic,
                        prefix.replace(b"captured_stdout=4", b"captured_stdout=3") + diagnostic):
            with self.assertRaises(RuntimeError):
                gate.without_text_provenance(changed, hashes, b"opa", b"wire")

    def test_failure_requires_all_diagnostics_exact_summary_and_no_execution(self):
        for operation in gate.OPERATIONS:
            valid = records(operation)
            self.assertEqual(gate.validate_failure(valid, operation), valid)
            for changed in ([], valid[:-1], [valid[-1]], valid + [{"kind": "execution"}],
                            [{"kind": "runtime-output"}, valid[-1]]):
                with self.subTest(operation=operation, changed=changed), self.assertRaises(RuntimeError):
                    gate.validate_failure(changed, operation)
            for key, value in (("kind", "other-summary"), ("success", True), ("errors", 0), ("errors", True)):
                changed = copy.deepcopy(valid)
                changed[-1][key] = value
                with self.assertRaises(RuntimeError):
                    gate.validate_failure(changed, operation)
            with self.assertRaises(RuntimeError):
                gate.validate_failure(valid, operation, "E0703")

    def test_json_failure_requires_exit_one_and_clean_stderr(self):
        data = b"\n".join(json.dumps(row).encode() for row in records())
        self.assertEqual(gate.json_rows(subprocess.CompletedProcess([], 1, data, b"")), records())
        for status, stdout, stderr in ((0, data, b""), (1, data, b"error"), (1, b"", b""),
                                       (1, b"[]", b""), (1, b"not JSON", b"")):
            with self.assertRaises((RuntimeError, ValueError)):
                gate.json_rows(subprocess.CompletedProcess([], status, stdout, stderr))

    def test_provenance_binds_every_input_stream_executable_and_status(self):
        source, opa, wire = b"source", b"opa", b"wire"
        hashes = {"parser": "a" * 64, "static": "b" * 64}
        original = execution(source, opa, wire, hashes, 2)
        gate.validate_provenance(original, source, opa, wire, hashes, 2)
        for index in (0, 1):
            for key, value in (("role", "other"), ("executable_sha256", "wrong"),
                               ("input_sha256", "wrong"), ("input_written", 0),
                               ("captured_stdout_sha256", "wrong"), ("captured_stdout_bytes", 0),
                               ("captured_stderr_sha256", "wrong"), ("captured_stderr_bytes", 1),
                               ("spawned", False), ("leader_reaped", False), ("stop", "deadline"),
                               ("exit_status", 1), ("signal", 9)):
                changed = copy.deepcopy(original)
                changed[index][key] = value
                with self.subTest(index=index, key=key), self.assertRaises(RuntimeError):
                    gate.validate_provenance(changed, source, opa, wire, hashes, 2)
        for changed in (original[:1], original[::-1], original + original[:1]):
            with self.assertRaises(RuntimeError):
                gate.validate_provenance(changed, source, opa, wire, hashes, 2)
        with self.assertRaises(RuntimeError):
            gate.validate_provenance(original, source, opa, wire, hashes, 1)

    def test_negative_mutations_do_not_modify_the_original_seed(self):
        opa = b"OPA1" + bytes(1555)
        wire = opa + b"STF1" + bytes([1, 5, 8, 19, 23, 0, 0, 0, 2, 1, 0, 0])
        original = bytes(wire)
        controls = gate.mutations(opa, wire, 1)
        self.assertEqual(len(controls), 25)
        self.assertEqual(wire, original)
        self.assertTrue(all(value != wire for value in controls.values()))
        self.assertEqual(controls["truncated-empty"], b"")
        self.assertEqual(controls["primary-end255"][1567], 255)
        self.assertEqual(controls["inactive-opa-cell"][1558], 1)

    def test_existing_evidence_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            compiler = root / "compiler"
            compiler.write_bytes(b"keep")
            with self.assertRaises(FileExistsError):
                gate.Run(compiler, root, root, "cc")
            self.assertEqual(compiler.read_bytes(), b"keep")

    def test_failure_timeout_and_spawn_error_preserve_streams_and_receipts(self):
        cases = ((subprocess.CompletedProcess([], 9, b"out", b"err"), 9),
                 (subprocess.TimeoutExpired([], 120, output=b"out", stderr=b"err"), "outer-timeout"),
                 (OSError("missing"), "spawn-failed"))
        for outcome, expected in cases:
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                run = gate.Run.__new__(gate.Run)
                run.root, run.env, run.receipts = Path(temporary), {}, []
                kwargs = {"side_effect": outcome} if isinstance(outcome, Exception) else {"return_value": outcome}
                with patch.object(gate.subprocess, "run", **kwargs), self.assertRaises(RuntimeError):
                    run.call("failure", ["fake-control"], b"input")
                receipt = json.loads((run.root / "receipts.json").read_text())[0]
                self.assertEqual(receipt["status"], expected)
                self.assertEqual(receipt["stdin_sha256"], gate.digest(b"input"))
                for stream in ("stdout", "stderr"):
                    self.assertEqual(receipt[stream + "_sha256"], gate.digest((run.root / "failure" / stream).read_bytes()))
                self.assertFalse((run.root / "summary.json").exists())

    def test_incomplete_or_changed_input_recipe_cannot_publish_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = gate.Run.__new__(gate.Run)
            run.root, run.identities = Path(temporary), {"compiler_sha256": "old"}
            run.captures, run.parities, run.refusals = [], [], []
            with patch.object(run, "input_identities", return_value={"compiler_sha256": "new"}):
                with self.assertRaisesRegex(RuntimeError, "inputs changed"):
                    run.finish()
            with patch.object(run, "input_identities", return_value=run.identities):
                with self.assertRaisesRegex(RuntimeError, "incomplete"):
                    run.finish()
            self.assertFalse((run.root / "summary.json").exists())

    def test_complete_recipe_is_required_before_retained_evidence_can_be_readmitted(self):
        captures, parities, refusals = gate.expected_recipe()
        self.assertEqual((len(captures), len(parities), len(refusals)), (106, 636, 384))
        expected = {"compiler_sha256": "a" * 64, "harness_sha256": "b" * 64}
        report = dict(expected, schema_version=1, status="passed", llvm_invocations=0,
            captures=[dict(version=version, name=name, source_sha256=gate.digest(source), kind=kind)
                      for version, name, source, kind in captures],
            parity_cases=parities, refusal_cases=refusals)
        gate.validate_recipe(report, expected)
        changes = [lambda r: r.update(status="partial"), lambda r: r.update(schema_version=True),
                   lambda r: r.update(llvm_invocations=True), lambda r: r.update(compiler_sha256="wrong"),
                   lambda r: r["captures"].pop(), lambda r: r["parity_cases"].pop(),
                   lambda r: r["refusal_cases"].pop(),
                   lambda r: r["captures"][0].update(name="other"),
                   lambda r: r["captures"][0].update(kind=True),
                   lambda r: r["captures"][0].update(source_sha256="wrong"),
                   lambda r: r["parity_cases"].reverse(), lambda r: r["refusal_cases"].reverse()]
        for change in changes:
            altered = copy.deepcopy(report)
            change(altered)
            with self.assertRaises(RuntimeError):
                gate.validate_recipe(altered, expected)


if __name__ == "__main__":
    unittest.main()
