"""The real package application cannot hide missing or neighboring sources."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_repo


class ManifestApplications(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.application = "apps/byte-count"
        self.members = [self.root / name for name in verify_repo.TYPED_MANIFEST_PROJECTS[self.application]]
        manifest = self.root / self.application / "oxid.toml"
        manifest.parent.mkdir(parents=True)
        manifest.write_text('[project]\nedition="typed-preview"\n')

    def test_only_complete_named_application_members_are_selected(self):
        extra = self.root / self.application / "unregistered.ox"
        selected, entries = verify_repo.manifest_project_members(self.members + [extra], self.root)
        self.assertEqual(selected, set(self.members))
        self.assertEqual(entries, [self.root / self.application])
        self.assertNotIn(extra, selected)
        for missing in self.members:
            with self.subTest(missing=missing), self.assertRaisesRegex(RuntimeError, "missing"):
                verify_repo.manifest_project_members([p for p in self.members if p != missing], self.root)

    def test_required_application_cannot_disappear(self):
        (self.root / self.application / "oxid.toml").unlink()
        with self.assertRaisesRegex(RuntimeError, "missing"):
            verify_repo.manifest_project_members(self.members, self.root)
        self.assertEqual(verify_repo.manifest_project_members([], self.root), (set(), []))
        with self.assertRaisesRegex(RuntimeError, "missing"):
            verify_repo.manifest_project_members([], self.root, required=True)

    def test_inventory_cannot_hide_another_directory_or_duplicate_a_member(self):
        member = str(self.members[0].relative_to(self.root))
        for members in ((member, member), ("unrelated.ox",), ()):
            with patch.dict(verify_repo.TYPED_MANIFEST_PROJECTS, {self.application: members}, clear=True):
                with self.subTest(members=members), self.assertRaisesRegex(RuntimeError, "invalid"):
                    verify_repo.manifest_project_members(self.members, self.root, required=True)

    def test_new_enum_projects_cannot_hide_missing_members_or_neighbors(self):
        projects = ("apps/frame-filter", "apps/byte-reverse", "packages/utf8_text", "apps/utf8-normalize",
                    "examples/projects/owned_enum_payload_acceptance/ready",
                    "examples/projects/owned_enum_payload_acceptance/failed",
                    "examples/projects/owned_enum_payload_acceptance/values",
                    "examples/projects/owned_enum_payload_acceptance/library", "examples/typed-unicode-range",
                    "packages/float_vector", "examples/typed-float-vector", "examples/typed-vectors", "packages/matrix", "examples/typed-matrix", "apps/cpu-training", "apps/minibatch-training", "apps/streaming-training")
        selected = set(self.members)
        entries = [self.root / self.application]
        for project in projects:
            members = [self.root / name for name in verify_repo.TYPED_MANIFEST_PROJECTS[project]]
            manifest = self.root / project / "oxid.toml"
            manifest.parent.mkdir(parents=True, exist_ok=True)
            manifest.write_text('[project]\nedition="typed-preview"\n')
            selected.update(members)
            entries.append(self.root / project)
        copy_project = "examples/copy-records"
        copy_members = ("examples/copy-records/src/main.ox", "examples/copy-records/tests/copies.ox")
        self.assertEqual(verify_repo.TYPED_MANIFEST_PROJECTS[copy_project], copy_members)
        for name in (copy_project + "/oxid.toml", *copy_members):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((verify_repo.ROOT / name).read_bytes())
        copy_paths = {self.root / name for name in copy_members}
        self.assertFalse(copy_paths & selected)
        record_vector_project = "examples/copy-record-vectors"
        record_vector_members = ("examples/copy-record-vectors/src/main.ox",
                                 "examples/copy-record-vectors/tests/records.ox")
        self.assertEqual(verify_repo.TYPED_MANIFEST_PROJECTS[record_vector_project], record_vector_members)
        for name in (record_vector_project + "/oxid.toml", *record_vector_members):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((verify_repo.ROOT / name).read_bytes())
        record_vector_paths = {self.root / name for name in record_vector_members}
        self.assertFalse(record_vector_paths & (selected | copy_paths))
        owned_vector_project = "examples/owned-record-vectors"
        owned_vector_members = ("examples/owned-record-vectors/src/main.ox",
                                "examples/owned-record-vectors/tests/messages.ox")
        self.assertEqual(verify_repo.TYPED_MANIFEST_PROJECTS[owned_vector_project], owned_vector_members)
        for name in (owned_vector_project + "/oxid.toml", *owned_vector_members):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((verify_repo.ROOT / name).read_bytes())
        owned_vector_paths = {self.root / name for name in owned_vector_members}
        self.assertFalse(owned_vector_paths & (selected | copy_paths | record_vector_paths))
        text_project = "examples/typed-owned-text"
        text_members = ("examples/typed-owned-text/src/main.ox", "examples/typed-owned-text/tests/text.ox")
        self.assertEqual(verify_repo.TYPED_MANIFEST_PROJECTS[text_project], text_members)
        for name in (text_project + "/oxid.toml", *text_members):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((verify_repo.ROOT / name).read_bytes())
        text_paths = {self.root / name for name in text_members}
        self.assertFalse(text_paths & (selected | copy_paths | record_vector_paths | owned_vector_paths))
        self.assertIn(self.root / "packages/utf8_text/src/lib.ox", selected)
        neighbor = self.root / "apps/frame-filter/unregistered.ox"
        owned_neighbors = [self.root / owned_vector_project / "tests/unregistered.ox",
                           self.root / "examples/owned-record-vectors-neighbor/main.ox"]
        text_neighbors = [self.root / text_project / "tests/unregistered.ox",
                          self.root / "examples/typed-owned-text-neighbor/main.ox"]
        admitted, roots = verify_repo.manifest_project_members(
            list(selected | copy_paths | record_vector_paths | owned_vector_paths | text_paths)
            + [neighbor, *owned_neighbors, *text_neighbors],
            self.root, required=True)
        self.assertTrue(text_paths <= admitted)
        self.assertEqual(roots.count(self.root / text_project), 1)
        admitted -= text_paths
        roots = [path for path in roots if path != self.root / text_project]
        self.assertTrue(owned_vector_paths <= admitted)
        self.assertEqual(roots.count(self.root / owned_vector_project), 1)
        admitted -= owned_vector_paths
        roots = [path for path in roots if path != self.root / owned_vector_project]
        self.assertTrue(record_vector_paths <= admitted)
        self.assertEqual(roots.count(self.root / record_vector_project), 1)
        admitted -= record_vector_paths
        roots = [path for path in roots if path != self.root / record_vector_project]
        self.assertTrue(copy_paths <= admitted)
        self.assertEqual(roots.count(self.root / copy_project), 1)
        admitted -= copy_paths
        roots = [path for path in roots if path != self.root / copy_project]
        self.assertEqual(admitted, selected)
        self.assertEqual(roots, entries)
        self.assertNotIn(neighbor, admitted)
        for path in owned_neighbors:
            self.assertNotIn(path, admitted)
        for path in text_neighbors:
            self.assertNotIn(path, admitted)
        for missing in selected - set(self.members):
            with self.subTest(missing=missing), self.assertRaisesRegex(RuntimeError, "missing"):
                verify_repo.manifest_project_members(
                    list((selected - {missing}) | copy_paths | record_vector_paths | owned_vector_paths | text_paths),
                    self.root, required=True)
        for missing in record_vector_paths:
            with self.subTest(missing=missing), self.assertRaisesRegex(RuntimeError, "missing"):
                verify_repo.manifest_project_members(list(selected | copy_paths | (record_vector_paths - {missing}) | owned_vector_paths | text_paths),
                                                    self.root, required=True)
        for missing in owned_vector_paths:
            with self.subTest(missing=missing), self.assertRaisesRegex(RuntimeError, "missing"):
                verify_repo.manifest_project_members(
                    list(selected | copy_paths | record_vector_paths | (owned_vector_paths - {missing}) | text_paths),
                    self.root, required=True)
        for missing in text_paths:
            with self.subTest(missing=missing), self.assertRaisesRegex(RuntimeError, "missing"):
                verify_repo.manifest_project_members(
                    list(selected | copy_paths | record_vector_paths | owned_vector_paths | (text_paths - {missing})),
                    self.root, required=True)
        (self.root / text_project / "oxid.toml").unlink()
        with self.assertRaisesRegex(RuntimeError, "missing"):
            verify_repo.manifest_project_members(
                list(selected | copy_paths | record_vector_paths | owned_vector_paths | text_paths),
                self.root, required=True)


if __name__ == "__main__":
    unittest.main()
