"""Exact owned-record vector DATA admission, including checkout conversion."""
from pathlib import Path
import os
import shutil
import subprocess
import tempfile
import unittest

import verify_owned_record_vector_fixture_data as data


ROOT = Path(__file__).resolve().parents[1]
SOURCES = tuple("scripts/owned_record_vector_acceptance/" + name + ".ox" for name in (
    "aliases", "bytebuf", "conflicting_borrows", "copy_operation", "double_move",
    "empty_new", "enum16", "failed_reserve", "fault_pop", "fault_push_file", "files",
    "generic_record", "index_read", "nested_vector", "package_main", "package_model",
    "unused_pop", "unused_push", "unused_template", "zero_key",
))


class OwnedRecordVectorDataTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in SOURCES:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / name, path)

    def test_exact_twenty_inputs_are_returned_without_directory_exclusions(self):
        self.assertEqual(len(SOURCES), 20)
        self.assertEqual(set(data.INPUTS), set(SOURCES))
        expected = {self.root / name for name in SOURCES}
        self.assertEqual(data.fixture_data_sources(self.root), expected)
        extras = (self.root / "scripts/owned_record_vector_acceptance/unlisted.ox",
                  self.root / "scripts/owned_record_vector_acceptance_neighbor/main.ox")
        for path in extras:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"unregistered source\n")
        self.assertEqual(data.fixture_data_sources(self.root), expected)
        self.assertTrue(expected.isdisjoint(extras))

    def test_missing_same_size_mutation_and_size_drift_are_refused(self):
        for name in SOURCES:
            path = self.root / name
            body = path.read_bytes()
            for replacement in (None, bytes([body[0] ^ 1]) + body[1:], body + b"\n", body[:-1]):
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
        moved = directory.with_name("preserved-owned-vector-inputs")
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
    def test_actual_crlf_checkout_preserves_only_the_twenty_pinned_data_bytes(self):
        shutil.copy2(ROOT / ".gitattributes", self.root / ".gitattributes")
        ordinary = self.root / "scripts/owned_record_vector_acceptance/unlisted.ox"
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
