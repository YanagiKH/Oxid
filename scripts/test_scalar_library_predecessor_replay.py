"""Reject incomplete evidence and edits to the fixed scalar-library predecessor replay."""
import ast
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import replay_scalar_library_source_policy as replay

ROOT = Path(__file__).resolve().parents[1]
MODULE = "test_scalar_library_compatibility"


class ScalarLibraryPredecessorEvidence(unittest.TestCase):
    def setUp(self):
        self.engine = replay.load_engine(ROOT)
        self.body = (ROOT / "scripts" / (MODULE + ".py")).read_bytes()
        self.names = sorted(MODULE + "." + cls.name + "." + method.name
                            for cls in ast.parse(self.body).body if isinstance(cls, ast.ClassDef)
                            for method in cls.body if isinstance(method, ast.FunctionDef)
                            and method.name.startswith("test_"))
        self.lines = [name.rsplit(".", 1)[1] + " (" + name + ") ... ok\n"
                      for name in self.names]
        self.stderr = ("".join(self.lines) + "\nRan 34 tests in 0.001s\n\nOK\n").encode()
        self.stderr = self.stderr.replace(replay.DENIAL_SUCCESS, replay.EXPECTED_DENIAL, 1)

    def test_all_thirty_four_unchanged_tests_are_required_with_terminal_success(self):
        self.assertEqual(replay.REVISION, "8f2c29eaffabbd90c591bbdc440ae6e23acf1353")
        self.assertEqual(replay.MODULES, {MODULE: (
            "9315dbe8332a349b88f617a7a55cc0b6b0795fbb4952885ad8f3e2fd4f5429fa", 34)})
        self.assertEqual(replay.POLICIES, {
            "verify_scalar_library_compatibility.py":
                "29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119",
            "run_record_vector_predecessor_tests.py":
                "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c",
        })
        self.assertEqual(self.engine.sha(self.body), replay.MODULES[MODULE][0])
        child = subprocess.CompletedProcess([], 0, b"", self.stderr)
        self.assertEqual(self.engine.validate_execution(MODULE, self.body, child), self.names)
        self.assertEqual(child.stderr, self.stderr, "raw refusal must remain available as evidence")
        for stream in (
            b"", self.stderr.replace(self.lines[0].encode(), b"", 1),
            self.stderr + self.lines[0].encode(),
            self.stderr.replace(b"... ok", b"... skipped 'platform'", 1),
            self.stderr.replace(b"Ran 34 tests", b"Ran 33 tests"),
            self.stderr.replace(b"\nOK\n", b"\nOK (skipped=1)\n"),
            self.stderr.replace(b"\nOK\n", b"\nFAILED (failures=1)\n"),
            self.stderr.replace(b"provisional scalar-library", b"unexpected scalar-library"),
            self.stderr.replace(replay.EXPECTED_DENIAL, replay.DENIAL_SUCCESS),
            self.stderr + replay.EXPECTED_DENIAL,
            self.stderr + b"ValueError: provisional scalar-library pins permit "
            b"prepare-only admission, not qualification\n",
            self.stderr + b"ValueError: unrelated failure\n",
        ):
            with self.subTest(stream=stream), self.assertRaises(ValueError):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], 0, b"", stream))
        changed = self.body.replace(b"def test_", b"def no_longer_a_test_", 1)
        with self.assertRaisesRegex(ValueError, "census changed"):
            self.engine.validate_execution(MODULE, changed, child)

    def test_failure_and_unexpected_stdout_cannot_be_replay_success(self):
        for code, stdout in ((1, b""), (0, b"unbound output")):
            with self.subTest(code=code), self.assertRaisesRegex(ValueError, "controls failed"):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], code, stdout, self.stderr))

    def test_wrong_revision_and_dirty_checkout_fail_before_policy_loading(self):
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

    def test_replay_engine_and_policy_pins_and_symlinks_fail_before_execution(self):
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
            originals = {name: (ROOT / "scripts" / name).read_bytes() for name in pins}
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


if __name__ == "__main__":
    unittest.main()
