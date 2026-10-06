"""Exercise independent wire expectations and real saved-output publication controls.

These Python controls do not claim Oxid compilation or runtime qualification.
"""
import json
from pathlib import Path
import sys
import tempfile
import unittest

import verify_bounded_stack_artifact as verifier


class ArtifactControllerTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.empty = self.root / "empty"
        self.empty.mkdir()
        self.cases = json.loads((verifier.CORPUS / "artifact-cases.json").read_bytes())["cases"]
        self.sample = next(case for case in self.cases if case["name"] == "sample-39")
        self.expected = (verifier.CORPUS / self.sample["file"]).read_bytes()

    def child(self, script, *, data=b"", mode="file", timeout=10):
        return verifier.invoke(self.root / "producer", [sys.executable, "-I", "-B", "-c", script],
                               data=data, mode=mode, cwd=self.empty, env={}, timeout=timeout)

    def publication(self, producer, valid=True):
        return verifier.publish(self.root / "producer/stdout.partial", self.root / "saved.oxs",
                                producer, self.expected, consumers_passed=valid)

    def test_external_decoder_accepts_all_28_independent_expectations(self):
        self.assertEqual(len(self.cases), 28)
        for case in self.cases:
            with self.subTest(case=case["name"]):
                artifact = verifier.CORPUS / case["file"]
                data = artifact.read_bytes()
                self.assertEqual(data.hex(), case["hex"])
                self.assertEqual(len(data), case["bytes"])
                self.assertEqual(verifier.digest(artifact), case["sha256"])
                row, stdout, stderr = verifier.invoke(self.root / "external" / case["name"],
                    [sys.executable, "-I", "-B", verifier.CORPUS / "decode_oxs1.py", artifact],
                    cwd=self.empty, env={})
                self.assertTrue(verifier.expect_decoder(row, stdout, stderr, case["expected"]))
        self.assertEqual(list(self.empty.iterdir()), [])

    def test_saved_bytes_are_published_after_successful_exit_and_validation(self):
        row, stdout, stderr = self.child("import os; os.write(1, bytes.fromhex(" + repr(self.expected.hex()) + "))")
        self.assertEqual((row["status"], stdout, stderr), (0, self.expected, b""))
        self.assertTrue(self.publication(row))
        self.assertEqual((self.root / "saved.oxs").read_bytes(), self.expected)
        self.assertFalse((self.root / "producer/stdout.partial").exists())

    def test_complete_output_followed_by_failure_is_never_published(self):
        row, stdout, _ = self.child("import os; os.write(1, bytes.fromhex(" + repr(self.expected.hex()) + ")); raise SystemExit(74)")
        self.assertEqual(stdout, self.expected)
        self.assertFalse(self.publication(row))
        self.assertFalse((self.root / "saved.oxs").exists())
        self.assertEqual((self.root / "producer/stdout.partial").read_bytes(), self.expected)

    def test_partial_output_followed_by_failure_is_retained(self):
        row, _, _ = self.child("import os; os.write(1, b'OXS1'); raise SystemExit(74)")
        self.assertFalse(self.publication(row))
        self.assertEqual((self.root / "producer/stdout.partial").read_bytes(), b"OXS1")

    def test_decoder_or_loader_failure_prevents_publication(self):
        row, _, _ = self.child("import os; os.write(1, bytes.fromhex(" + repr(self.expected.hex()) + "))")
        self.assertFalse(self.publication(row, valid=False))
        self.assertTrue((self.root / "producer/stdout.partial").is_file())

    def test_success_with_trailing_bytes_does_not_publish(self):
        row, _, _ = self.child("import os; os.write(1, bytes.fromhex(" + repr(self.expected.hex()) + ") + b'\\0')")
        self.assertFalse(self.publication(row))
        self.assertEqual((self.root / "producer/stdout.partial").stat().st_size, 81)

    def test_success_with_stderr_does_not_publish(self):
        row, _, _ = self.child("import os; os.write(1, bytes.fromhex(" + repr(self.expected.hex()) + ")); os.write(2, b'failure')")
        self.assertFalse(self.publication(row))

    def test_success_does_not_replace_existing_destination(self):
        row, _, _ = self.child("import os; os.write(1, bytes.fromhex(" + repr(self.expected.hex()) + "))")
        (self.root / "saved.oxs").write_bytes(b"existing")
        with self.assertRaises(FileExistsError):
            self.publication(row)
        self.assertEqual((self.root / "saved.oxs").read_bytes(), b"existing")

    def test_file_and_pipe_unread_bytes_are_observed_independently(self):
        for mode in ("pipe", "file"):
            row, stdout, stderr = verifier.invoke(self.root / mode,
                [sys.executable, "-I", "-B", "-c", "import os; os.write(1, os.read(0, 81))"],
                data=b"A" * 81 + b"B", mode=mode, cwd=self.empty, env={})
            self.assertEqual((row["status"], stdout, stderr), (0, b"A" * 81, b""))
            self.assertEqual(row["remaining_hex"], "42")

    def test_timeout_preserves_partial_stdout_without_publication(self):
        row, _, _ = self.child("import os,time; os.write(1,b'OXS1'); time.sleep(10)", timeout=0.2)
        self.assertIsNone(row["status"])
        self.assertIsNotNone(row["failure"])
        self.assertFalse(self.publication(row))
        self.assertEqual((self.root / "producer/stdout.partial").read_bytes(), b"OXS1")

    def test_wrong_external_result_and_error_are_rejected(self):
        row = {"failure": None, "status": 0}
        self.assertFalse(verifier.expect_decoder(row, b'{"value":39}', b"", self.sample["expected"]))
        self.assertFalse(verifier.expect_decoder(row, b'not JSON', b"", self.sample["expected"]))
        row["status"] = 65
        invalid = next(case for case in self.cases if case["name"] == "wrong-version")
        self.assertFalse(verifier.expect_decoder(row, b"", b'{"error":"magic","offset":0}', invalid["expected"]))


if __name__ == "__main__":
    unittest.main()
