"""Retain all 35 published setup-policy tests independently of the older replay."""
import argparse
import ast
import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import replay_windows_download_source_policy as replay


ROOT = Path(__file__).resolve().parents[1]
MODULE = "test_scalar_library_compatibility"


def frozen_body(path):
    return subprocess.check_output(
        ["git", "--no-replace-objects", "-C", str(ROOT), "show", replay.REVISION + ":scripts/" + path],
        timeout=30)


class WindowsDownloadPredecessorEvidence(unittest.TestCase):
    def setUp(self):
        self.engine = replay.load_engine(ROOT)
        # The current module is the distinct 34-test predecessor. Read the
        # complete 35-test successor from its immutable published Git object.
        self.body = frozen_body(MODULE + ".py")
        self.names = sorted(MODULE + "." + cls.name + "." + method.name
                            for cls in ast.parse(self.body).body if isinstance(cls, ast.ClassDef)
                            for method in cls.body if isinstance(method, ast.FunctionDef)
                            and method.name.startswith("test_"))
        self.lines = [name.rsplit(".", 1)[1] + " (" + name + ") ... ok\n"
                      for name in self.names]
        self.stderr = ("".join(self.lines) + "\n" + "-" * 70 +
                       "\nRan 35 tests in 0.001s\n\nOK\n").encode()
        self.stderr = self.stderr.replace(replay.DENIAL_SUCCESS, replay.EXPECTED_DENIAL, 1)

    def test_complete_cb_source_and_required_policies_are_bound_to_exact_c693(self):
        self.assertEqual(replay.REVISION, "c693662501c7629c67aabaa2675dbb6b5a76d460")
        self.assertEqual(replay.MODULES, {MODULE: (
            "cb5186db4f64fc82c043e0cded2a642914bc64c2e17993cd79aff57e7a809c73", 35)})
        self.assertEqual(replay.POLICIES, {
            "verify_scalar_library_compatibility.py":
                "29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119",
            "verify_file_seek_compatibility.py":
                "eec5daba34c6380781289a3bb01ed58e44e359a4b03fd23288f950be0ca99e38",
            "run_file_seek_predecessor_tests.py":
                "dadfefe4bca26f008456a5c018b53e3ba5697a22a7618714e0a738aa825614a0",
            "run_record_vector_predecessor_tests.py":
                "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c",
        })
        self.assertEqual(self.engine.sha(self.body), replay.MODULES[MODULE][0])
        self.assertEqual(len(self.names), 35)
        for method in (
            "test_CI_comparison_refuses_changed_SDK_transport_and_bounds",
            "test_CI_keeps_only_the_new_replay_job_and_exact_reviewed_Windows_setup_patches",
            "test_all_58_predecessor_policies_and_adapters_are_exact",
        ):
            self.assertIn(MODULE + ".FrozenFileSeekReplay." + method, self.names)
        child = subprocess.CompletedProcess([], 0, b"", self.stderr)
        self.assertEqual(self.engine.validate_execution(MODULE, self.body, child), self.names)
        self.assertEqual(child.stderr, self.stderr)

    def test_no_incomplete_duplicate_skipped_failed_or_unbound_output_is_accepted(self):
        for stream in (
            b"", self.stderr.replace(self.lines[0].encode(), b"", 1),
            self.stderr + self.lines[0].encode(),
            self.stderr.replace(b"... ok", b"... skipped 'platform'", 1),
            self.stderr.replace(b"Ran 35 tests", b"Ran 34 tests"),
            self.stderr.replace(b"\nOK\n", b"\nOK (skipped=1)\n"),
            self.stderr.replace(b"\nOK\n", b"\nFAILED (failures=1)\n"),
            self.stderr.replace(b"provisional scalar-library", b"unexpected scalar-library"),
            self.stderr.replace(replay.EXPECTED_DENIAL, replay.DENIAL_SUCCESS),
            self.stderr + replay.EXPECTED_DENIAL,
            self.stderr + b"ValueError: provisional scalar-library pins permit "
            b"prepare-only admission, not qualification\n",
            self.stderr + b"ValueError: unrelated failure\n",
            self.stderr + b"unbound output\n",
        ):
            with self.subTest(stream=stream), self.assertRaises(ValueError):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], 0, b"", stream))
        changed = self.body.replace(b"def test_", b"def no_longer_a_test_", 1)
        with self.assertRaisesRegex(ValueError, "census changed"):
            self.engine.validate_execution(
                MODULE, changed, subprocess.CompletedProcess([], 0, b"", self.stderr))

    def test_failure_and_stdout_cannot_report_complete_replay_success(self):
        for code, stdout in ((1, b""), (0, b"unbound output")):
            with self.subTest(code=code), self.assertRaisesRegex(ValueError, "controls failed"):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], code, stdout, self.stderr))

    def test_wrong_revision_and_dirty_tree_fail_before_reading_policies(self):
        with patch.object(self.engine, "git", return_value=b"0" * 40), \
                patch.object(Path, "read_bytes") as reads:
            with self.assertRaisesRegex(ValueError, "wrong predecessor"):
                self.engine.identity(ROOT)
            reads.assert_not_called()
        with patch.object(self.engine, "git", side_effect=[replay.REVISION.encode(), b" M source.rs"]), \
                patch.object(Path, "read_bytes") as reads:
            with self.assertRaisesRegex(ValueError, "must be clean"):
                self.engine.identity(ROOT)
            reads.assert_not_called()

    def test_changed_engine_and_each_frozen_input_fail_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            engine_path = root / "scripts" / replay.ENGINE
            engine_body = (ROOT / "scripts" / replay.ENGINE).read_bytes()
            engine_path.write_bytes(engine_body + b"\n")
            with self.assertRaisesRegex(ValueError, "changed predecessor"):
                replay.load_engine(root)
            engine_path.unlink()
            engine_path.symlink_to(ROOT / "scripts" / replay.ENGINE)
            with self.assertRaisesRegex(ValueError, "unsafe predecessor"):
                replay.load_engine(root)
            engine_path.unlink()
            pins = {**{name + ".py": pin for name, (pin, _) in replay.MODULES.items()},
                    **replay.POLICIES}
            originals = {name: frozen_body(name) for name in pins}
            for name, body in originals.items():
                (root / "scripts" / name).write_bytes(body)
            with patch.object(self.engine, "git", side_effect=[replay.REVISION.encode(), b""]):
                self.assertEqual(self.engine.identity(root), originals)
            for name, body in originals.items():
                path = root / "scripts" / name
                path.write_bytes(body + b"\n")
                with self.subTest(input=name), patch.object(
                        self.engine, "git", side_effect=[replay.REVISION.encode(), b""]):
                    with self.assertRaisesRegex(ValueError, "changed pinned predecessor input"):
                        self.engine.identity(root)
                path.write_bytes(body)

    def test_real_engine_retains_raw_success_and_failure_streams_without_rewriting_denial(self):
        for accepted in (True, False):
            stream = self.stderr if accepted else self.stderr + b"unbound output\n"
            child = subprocess.CompletedProcess([], 0, b"", stream)
            with self.subTest(accepted=accepted), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "replay"
                args = argparse.Namespace(repo=ROOT, output=output)
                with patch.object(self.engine, "identity", return_value={MODULE + ".py": self.body}), \
                        patch.object(self.engine.subprocess, "run", return_value=child), \
                        contextlib.redirect_stdout(io.StringIO()):
                    if accepted:
                        self.assertEqual(self.engine.run(args), 0)
                    else:
                        with self.assertRaisesRegex(ValueError, "unrecognized predecessor test output"):
                            self.engine.run(args)
                self.assertEqual((output / (MODULE + ".stdout")).read_bytes(), child.stdout)
                self.assertEqual((output / (MODULE + ".stderr")).read_bytes(), stream)
                self.assertEqual(child.stderr, stream)
                report = json.loads((output / "result.json").read_bytes())
                self.assertEqual(report["status"], "passed" if accepted else "failed")
                self.assertIs(report["historical_results_apply_to_current"], False)
                row = report["modules"][MODULE]
                self.assertEqual(row["exit_status"], 0)
                self.assertEqual(row["stderr_sha256"], self.engine.sha(stream))
                if accepted:
                    self.assertEqual(report["test_executions"], 35)
                    self.assertEqual(row["tests"], self.names)
                else:
                    self.assertNotIn("test_executions", report)


if __name__ == "__main__":
    unittest.main()
