#!/usr/bin/env python3
"""Bounded publication-admission controls; no compiler or corpus execution."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

import verify_publication as verifier

PACKAGE = Path(__file__).absolute().parent


class PublicationControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="oxid-doc-projection-")
        self.root = Path(self.temp.name) / "package"
        shutil.copytree(PACKAGE, self.root)
        self.frozen = self.root / "frozen/v3"

    def tearDown(self):
        self.temp.cleanup()

    def reject(self):
        with self.assertRaises((verifier.Rejected, OSError)):
            verifier.verify(self.root)

    def test_exact_projection_and_runtime_inspect(self):
        self.assertEqual(verifier.verify(self.root)["unchanged_checkpoint_bound_members"], 66)
        result = subprocess.run([sys.executable, "-B", str(self.frozen / "portable.py"), "inspect"],
                                capture_output=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)["authority_sha256"], verifier.AUTHORITY_SHA256)

    def test_every_unchanged_checkpoint_member_is_enforced(self):
        rows = json.loads((self.frozen / "checkpoint.json").read_bytes())["files"]
        for row in rows:
            if row["path"] == "DESIGN.md":
                continue
            with self.subTest(member=row["path"]):
                path = self.frozen / row["path"]
                original = path.read_bytes()
                path.write_bytes(original + b"\ncontent mutation\n")
                self.reject()
                path.write_bytes(original)

    def test_wrong_document_body(self):
        path = self.frozen / "DESIGN.md"
        path.write_bytes(path.read_bytes() + b"\nunapproved wording\n")
        self.reject()

    def test_original_document_requires_explicit_projection(self):
        path = self.frozen / "DESIGN.md"
        row = json.loads((self.root / "publication-projection.json").read_bytes())["replacement"]
        path.write_bytes(path.read_bytes().replace(row["new_utf8"].encode(), row["old_utf8"].encode()))
        self.reject()

    def test_coherently_rehashed_document_and_ledger(self):
        path = self.frozen / "DESIGN.md"
        path.write_bytes(path.read_bytes() + b"\naltered\n")
        ledger_path = self.root / "publication-projection.json"
        ledger = json.loads(ledger_path.read_bytes())
        ledger["published_member"].update(bytes=path.stat().st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        ledger_path.write_text(json.dumps(ledger))
        self.reject()

    def test_runtime_mutation_with_rehashed_checkpoint(self):
        path = self.frozen / "portable.py"
        path.write_bytes(path.read_bytes() + b"\n# altered runtime\n")
        checkpoint_path = self.frozen / "checkpoint.json"
        checkpoint = json.loads(checkpoint_path.read_bytes())
        row = next(r for r in checkpoint["files"] if r["path"] == "portable.py")
        row.update(bytes=path.stat().st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        checkpoint_path.write_text(json.dumps(checkpoint))
        self.reject()

    def test_widened_exception_is_rejected(self):
        path = self.root / "publication-projection.json"
        ledger = json.loads(path.read_bytes())
        ledger["projected_member"] = "portable.py"
        path.write_text(json.dumps(ledger))
        self.reject()

    def test_top_readme_identity(self):
        path = self.root / "README.md"
        path.write_text(path.read_text() + "Unreviewed status.\n")
        self.reject()

    def test_missing_member(self):
        (self.frozen / "frozen/helpers/run.py").unlink()
        self.reject()

    def test_extra_file(self):
        (self.frozen / "unexpected.py").write_text("pass\n")
        self.reject()

    def test_extra_directory(self):
        (self.frozen / "unlisted").mkdir()
        self.reject()

    def test_symlink_member(self):
        path = self.frozen / "frozen/helpers/run.py"
        target = self.root / "preserved-run.py"
        path.rename(target)
        path.symlink_to(target)
        self.reject()

    def test_symlink_package_ancestry(self):
        alias = Path(self.temp.name) / "alias"
        alias.symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(verifier.Rejected):
            verifier.verify(alias)


if __name__ == "__main__":
    unittest.main(verbosity=2)
