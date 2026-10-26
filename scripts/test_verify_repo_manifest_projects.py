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
        neighbor = self.root / "apps/frame-filter/unregistered.ox"
        admitted, roots = verify_repo.manifest_project_members(list(selected) + [neighbor], self.root, required=True)
        self.assertEqual(admitted, selected)
        self.assertEqual(roots, entries)
        self.assertNotIn(neighbor, admitted)
        for missing in selected - set(self.members):
            with self.subTest(missing=missing), self.assertRaisesRegex(RuntimeError, "missing"):
                verify_repo.manifest_project_members(list(selected - {missing}), self.root, required=True)


if __name__ == "__main__":
    unittest.main()
