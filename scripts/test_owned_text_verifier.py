"""Mutation controls for the owned-text public acceptance driver."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_owned_text as gate


def completed(rows=(), status=0, stderr=b""):
    return subprocess.CompletedProcess([], status, b"\n".join(json.dumps(row).encode() for row in rows) + b"\n", stderr)


def row(kind, **fields):
    return {"schema_version": 1, "edition": "typed-preview", "kind": kind, **fields}


def rejection_rows(case, summary="check-summary"):
    return [row("diagnostic", severity="error", stage=case.stage, code=case.code),
            row(summary, success=False)]


def driver(output):
    return gate.Driver(output, {}, {"materializations": [], "deployments": []})


class FakeProcess:
    pid = 424242

    def __init__(self, streams, *, running=True, status=0, stdout=b"", stderr=b""):
        self.running, self.status = running, status
        streams["stdout"].write(stdout)
        streams["stdout"].flush()
        streams["stderr"].write(stderr)
        streams["stderr"].flush()

    def poll(self):
        return None if self.running else self.status

    def wait(self, timeout=None):
        self.running = False
        return self.status


class OwnedTextOracleTests(unittest.TestCase):
    def test_numeric_oracle_checks_complete_output_status_and_stderr(self):
        case = gate.CASES[0]
        gate.outcome(subprocess.CompletedProcess([], 0, b"16\n", b""), case)
        for status, stdout, stderr in ((16, b"", b""), (0, b"16", b""), (0, b"16\r\n", b""),
                                       (0, b"16\n0\n", b""), (0, b"15\n", b""),
                                       (0, b"16\n", b"warning\n"), (-11, b"16\n", b"")):
            with self.subTest(status=status, stdout=stdout, stderr=stderr), self.assertRaises(ValueError):
                gate.outcome(subprocess.CompletedProcess([], status, stdout, stderr), case)

    def test_each_negative_requires_its_exact_diagnostic_and_summary(self):
        for case in gate.REJECTIONS:
            with self.subTest(case=case.name):
                self.assertEqual(gate.refused(completed(rejection_rows(case), 1), case, "check-summary"),
                                 [(case.stage, case.code)])
                for stage, code in (("oir-owned-verify", "E0500"), (case.stage, "E0400"),
                                    ("native-emission", case.code), (case.stage, "E0701")):
                    bad = rejection_rows(case)
                    bad[0].update(stage=stage, code=code)
                    with self.subTest(stage=stage, code=code), self.assertRaises(ValueError):
                        gate.refused(completed(bad, 1), case, "check-summary")

    def test_negative_cannot_pass_on_warning_success_or_extra_internal_error(self):
        case = gate.REJECTIONS[0]
        good = rejection_rows(case)
        mutants = (completed(good, 0), completed(good, -11), completed(good, 1, b"unexpected\n"),
                   completed([good[-1]], 1),
                   completed([{**good[0], "severity": "warning"}, good[-1]], 1),
                   completed([good[0], row("compile-summary", success=False)], 1),
                   completed([good[0], row("check-summary", success=True)], 1),
                   completed([good[0], row("diagnostic", stage="type", code="E0500", severity="error"), good[-1]], 1))
        for result in mutants:
            with self.subTest(result=result), self.assertRaises(ValueError):
                gate.refused(result, case, "check-summary")

    def test_json_requires_framing_exact_schema_and_no_stderr(self):
        good = row("check-summary", success=True)
        gate.successful(completed([good]), "check-summary")
        for result in (completed([{**good, "schema_version": True}]),
                       completed([{**good, "schema_version": 2}]),
                       completed([{**good, "edition": "legacy"}]),
                       completed([good, good]), completed([row("compile-summary", success=True)]),
                       completed([good], stderr=b"warning\n"),
                       subprocess.CompletedProcess([], 0, json.dumps(good).encode(), b""),
                       subprocess.CompletedProcess([], 0, b"truncated {\n", b"")):
            with self.subTest(result=result), self.assertRaises((ValueError, json.JSONDecodeError)):
                gate.successful(result, "check-summary")

    def test_declared_test_requires_clean_successful_zero_exit_and_expected_name(self):
        first = row("test-result", name="text", success=True, exit_status=0, stdout="", stderr="")
        last = row("test-summary", success=True, passed=1, failed=0)
        gate.declared_test(completed([first, last]))
        for changed in (dict(name="other"), dict(success=False), dict(exit_status=1), dict(exit_status=None),
                        dict(exit_status=False), dict(stdout="0\n"), dict(stderr="warning\n")):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                gate.declared_test(completed([{**first, **changed}, last]))
        workspace = {**first, "name": "utf8", "member": "contracts"}
        gate.declared_test(completed([workspace, gate.workspace_summary("test", 1)]), True)
        with self.assertRaises(ValueError):
            gate.declared_test(completed([{**workspace, "member": "app"}, gate.workspace_summary("test", 1)]), True)

    def test_workspace_operation_refuses_missing_duplicate_or_wrong_members(self):
        rows = [row("compile-summary", success=True), row("workspace-member", member="contracts", status="ok"),
                gate.workspace_summary("build", 1)]
        gate.workspace_operation(completed(rows), "build", ("contracts",))
        for bad in (rows[:-1], [rows[0], {**rows[1], "member": "app"}, rows[2]],
                    [rows[0], {**rows[1], "status": "failed"}, rows[2]],
                    [rows[0], rows[1], {**rows[2], "failed": 1}], rows + rows[:2],
                    [{**rows[0], "success": False}, *rows[1:]]):
            with self.subTest(rows=bad), self.assertRaises(ValueError):
                gate.workspace_operation(completed(bad), "build", ("contracts",))

    def test_finite_census_and_oracles_have_no_implicit_cases_or_internal_error_allowance(self):
        self.assertEqual([(case.name, case.result) for case in gate.CASES],
                         [("main", 16), ("text-tests", 0), ("contracts", 0)])
        self.assertEqual([(case.name, case.stage, case.code) for case in gate.REJECTIONS],
                         [("self-append", "ownership", "E0311"), ("private-bytes", "type", "E0206"),
                          ("moved-text", "ownership", "E0310"), ("returned-borrow", "parse", "E0101")])
        self.assertEqual((len(gate.input_paths()), len(set(gate.input_paths()))), (21, 21))
        self.assertTrue(set(gate.CONTROLLERS) <= set(gate.input_paths()))
        for case in gate.REJECTIONS:
            self.assertIn(str(gate.EXAMPLE / "tests/reject" / (case.name + ".ox")), gate.input_paths())
        self.assertEqual(len("Hello, Oxid 🦀".encode("utf-8")), 16)
        self.assertEqual(len("Hello, Oxid 🦀"), 13)
        self.assertEqual(2 * len(gate.POLICIES) * len(gate.PROFILES) * len(gate.CASES), 24)

    def test_copied_manifest_edits_are_narrow_and_preserve_library_example_and_readme_bytes(self):
        with tempfile.TemporaryDirectory(prefix="oxid-text-manifest-") as folder:
            output = Path(folder)
            observer = driver(output)
            app = observer.materialize(output / "fixture", gate.CASES[1], "hosted")
            original = (gate.ROOT / gate.EXAMPLE / "oxid.toml").read_bytes()
            expected = original.replace(b'entry = "src/main.ox"', b'entry = "tests/text.ox"', 1)
            expected = expected.replace(b'policy = "bounded"', b'policy = "hosted"', 1)
            self.assertEqual((app / "oxid.toml").read_bytes(), expected)
            for relative in gate.EXAMPLE_FILES:
                if relative != "oxid.toml":
                    self.assertEqual((app / relative).read_bytes(), (gate.ROOT / gate.EXAMPLE / relative).read_bytes())
            for relative in gate.LIBRARY_FILES:
                self.assertEqual((output / "fixture" / gate.LIBRARY / relative).read_bytes(),
                                 (gate.ROOT / gate.LIBRARY / relative).read_bytes())
            self.assertEqual(observer.report["materializations"][0]["changes"],
                             [{"entry": "tests/text.ox"}, {"policy": "hosted"}])
        for body in (b"no anchor", b"entry entry"):
            with self.subTest(body=body), self.assertRaises(ValueError):
                gate.replace_once(body, b"entry", b"replacement")

    def test_command_capture_rejects_deadline_live_overflow_and_final_overflow(self):
        for name, timeout, running, stdout in (("deadline", 0, True, b""),
                                               ("live-overflow", 180, True, b"12345"),
                                               ("final-overflow", 180, False, b"12345")):
            with self.subTest(name=name), tempfile.TemporaryDirectory(prefix="oxid-text-command-") as folder:
                output = Path(folder)
                observer = driver(output)
                with patch.object(gate.public, "TIMEOUT", timeout), patch.object(gate.public, "MAX_STREAM", 4), \
                        patch.object(gate.public.subprocess, "Popen", side_effect=lambda *args, **kwargs:
                                     FakeProcess(kwargs, running=running, stdout=stdout)), \
                        patch.object(gate.public.os, "killpg") as kill:
                    with self.assertRaises(ValueError):
                        observer.command(name, ["mocked-process"], output)
                receipt = json.loads((output / "commands" / ("0000-" + name) / "command.json").read_text())
                self.assertEqual(receipt["status"], "failed")
                self.assertEqual(receipt["streams"]["stdout"]["bytes"], len(stdout))
                self.assertEqual(kill.call_count, int(running))

    def test_exact_capture_boundary_is_retained_and_wrong_status_still_fails(self):
        for status in (0, 1):
            with self.subTest(status=status), tempfile.TemporaryDirectory(prefix="oxid-text-capture-") as folder:
                output = Path(folder)
                observer = driver(output)
                with patch.object(gate.public, "MAX_STREAM", 4), \
                        patch.object(gate.public.subprocess, "Popen", side_effect=lambda *args, **kwargs:
                                     FakeProcess(kwargs, running=False, status=status, stdout=b"1234", stderr=b"abcd")):
                    if status:
                        with self.assertRaises(ValueError):
                            observer.command("exact", ["mocked-process"], output)
                    else:
                        result = observer.command("exact", ["mocked-process"], output)
                        self.assertEqual((result.stdout, result.stderr), (b"1234", b"abcd"))

    def test_deployment_uses_a_real_copy_empty_environment_and_restores_sources_on_failure(self):
        for succeeds in (True, False):
            with self.subTest(succeeds=succeeds), tempfile.TemporaryDirectory(prefix="oxid-text-deploy-") as folder:
                output = Path(folder)
                observer = driver(output)
                base, cache = output / "projects/debug", output / "debug-cache"
                base.mkdir()
                cache.mkdir()
                (base / "source.ox").write_text("fn main()->i32{return 16;}")
                (cache / "object").write_bytes(b"cached object")
                artifact = output / "artifacts/main"
                artifact.write_bytes(b"retained executable bytes")
                pin = gate.public.identity(artifact)
                pending = [(artifact, pin, gate.CASES[0], "bounded", "dev")]

                def invoke(name, argv, cwd):
                    self.assertFalse(base.exists())
                    self.assertFalse(cache.exists())
                    self.assertEqual(observer.environment, {})
                    self.assertNotEqual(argv[0], artifact)
                    self.assertEqual(Path(argv[0]).read_bytes(), artifact.read_bytes())
                    return subprocess.CompletedProcess([], 0, b"16\n" if succeeds else b"wrong\n", b"")

                with patch.object(observer, "command", side_effect=invoke):
                    if succeeds:
                        observer.deploy(base, cache, pending, "debug")
                        self.assertTrue(observer.report["deployments"][0]["source_and_cache_unavailable"])
                    else:
                        with self.assertRaises(ValueError):
                            observer.deploy(base, cache, pending, "debug")
                self.assertEqual((base / "source.ox").read_text(), "fn main()->i32{return 16;}")
                self.assertEqual((cache / "object").read_bytes(), b"cached object")
                self.assertEqual(gate.public.identity(artifact), pin)


if __name__ == "__main__":
    unittest.main()
