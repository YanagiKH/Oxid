"""Closed Copy DATA admission and actual Git checkout byte preservation."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

import verify_copy_record_fixture_data as data

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ("tests/fixtures/copy_records/all_scalars.ox",
           "tests/fixtures/copy_records/wide_unicode.ox")


class CopyRecordDataTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in SOURCES:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, path)

    def test_exact_two_inputs_are_returned_without_directory_exclusions(self):
        self.assertEqual(data.fixture_data_sources(self.root), {self.root / name for name in SOURCES})
        extra = self.root / "tests/fixtures/copy_records/unlisted.ox"
        extra.write_bytes(b"unregistered source\n")
        self.assertEqual(data.fixture_data_sources(self.root), {self.root / name for name in SOURCES})
        self.assertNotIn(extra, data.fixture_data_sources(self.root))

    def test_missing_same_size_mutation_and_size_drift_are_refused(self):
        for name in SOURCES:
            path = self.root / name
            body = path.read_bytes()
            for replacement in [None, bytes([body[0] ^ 1]) + body[1:], body + b"\n", body[:-1]]:
                with self.subTest(source=name, size=None if replacement is None else len(replacement)):
                    path.unlink()
                    if replacement is not None:
                        path.write_bytes(replacement)
                    with self.assertRaises(RuntimeError):
                        data.fixture_data_sources(self.root)
                    path.write_bytes(body)

    def test_matching_file_symlink_and_linked_parent_are_refused(self):
        path = self.root / SOURCES[0]
        body = path.read_bytes()
        target = self.root / "external.ox"
        target.write_bytes(body)
        path.unlink()
        path.symlink_to(target)
        with self.assertRaisesRegex(RuntimeError, "symlink"):
            data.fixture_data_sources(self.root)
        path.unlink()
        path.write_bytes(body)
        directory = path.parent
        moved = directory.with_name("preserved-copy-inputs")
        directory.rename(moved)
        directory.symlink_to(moved, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "symlink"):
            data.fixture_data_sources(self.root)

    def test_matching_hard_link_is_refused(self):
        path = self.root / SOURCES[0]
        target = self.root / "external.ox"
        target.write_bytes(path.read_bytes())
        path.unlink()
        os.link(target, path)
        with self.assertRaisesRegex(RuntimeError, "nonregular"):
            data.fixture_data_sources(self.root)

    def test_directory_and_executable_are_refused(self):
        path = self.root / SOURCES[0]
        body = path.read_bytes()
        path.unlink()
        path.mkdir()
        with self.assertRaisesRegex(RuntimeError, "nonregular"):
            data.fixture_data_sources(self.root)
        path.rmdir()
        path.write_bytes(body)
        path.chmod(0o755)
        with self.assertRaisesRegex(RuntimeError, "nonregular"):
            data.fixture_data_sources(self.root)

    @unittest.skipUnless(shutil.which("git"), "Git is required for checkout conversion control")
    def test_actual_crlf_checkout_preserves_only_the_pinned_data_bytes(self):
        shutil.copy2(ROOT / ".gitattributes", self.root / ".gitattributes")
        ordinary = self.root / "tests/fixtures/copy_records/unlisted.ox"
        ordinary.write_bytes(b"ordinary\nsource\n")
        checkout = self.root / "checkout"

        def git(*args):
            return subprocess.run(["git", "-c", "core.autocrlf=true", "-c", "core.eol=crlf",
                                   "-c", "core.safecrlf=false", *args], cwd=self.root,
                                  check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

        git("init", "--quiet")
        git("add", "--", ".gitattributes", *SOURCES, str(ordinary.relative_to(self.root)))
        git("checkout-index", "--all", "--force", "--prefix", str(checkout) + "/")
        self.assertEqual(data.fixture_data_sources(checkout), {checkout / name for name in SOURCES})
        for name in SOURCES:
            self.assertEqual((checkout / name).read_bytes(), (ROOT / name).read_bytes())
        self.assertEqual((checkout / ordinary.relative_to(self.root)).read_bytes(), b"ordinary\r\nsource\r\n")


if __name__ == "__main__":
    unittest.main()
