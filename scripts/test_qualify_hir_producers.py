"""Fast harness controls; the explicit native recipe performs real integration."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from qualify_hir_producers import Qualification, digest


class QualificationControls(unittest.TestCase):
    def test_existing_evidence_directory_is_never_reused(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            executable = root / "compiler"
            executable.write_bytes(b"existing")
            with self.assertRaises(FileExistsError):
                Qualification(executable, root, root, "cc")
            self.assertEqual(executable.read_bytes(), b"existing")

    def test_outer_timeout_preserves_partial_streams_and_receipt(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = Qualification.__new__(Qualification)
            run.root = run.repo = Path(tmp)
            run.env = {}
            run.receipts = []
            failure = subprocess.TimeoutExpired(["helper"], 12, output=b"partial", stderr=b"error")
            with patch("qualify_hir_producers.subprocess.run", side_effect=failure):
                with self.assertRaises(subprocess.TimeoutExpired):
                    run.call("timeout", ["helper"], timeout=12)
            self.assertEqual((run.root / "timeout.stdout").read_bytes(), b"partial")
            self.assertEqual((run.root / "timeout.stderr").read_bytes(), b"error")
            receipt = json.loads((run.root / "receipts.json").read_text())[0]
            self.assertEqual(receipt["status"], "outer-timeout")
            self.assertEqual(receipt["stdout_sha256"], digest(b"partial"))

    def test_synthetic_parent_and_fork_child_have_independent_lifetime_caps(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = Qualification.__new__(Qualification)
            run.root = Path(tmp)
            run.opa = b"OPA1"
            run.wire = b"wire"
            run.cc = "cc"
            bundle = run.root / "bundle"
            bundle.mkdir()
            with patch.object(run, "call"):
                run.helper(bundle / "parser", "parser", "descendant")
            source = (bundle / "parser.c").read_text()
            self.assertEqual(source.count("alarm(15)"), 2)
            self.assertIn("signal(SIGALRM,SIG_DFL)", source)

    def test_optimized_python_cannot_silently_disable_verification(self):
        result = subprocess.run(["python3", "-O", str(Path(__file__).with_name("qualify_hir_producers.py")),
                                 "--compiler", "/unused", "--llvm-bin", "/unused", "--output", "/unused"],
                                capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"assertions must remain enabled", result.stderr)


if __name__ == "__main__":
    unittest.main()
