"""The real package application cannot hide missing or neighboring sources."""
from pathlib import Path
import tomllib
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
                    "packages/float_vector", "examples/typed-float-vector", "examples/typed-vectors", "packages/matrix", "examples/typed-matrix", "apps/cpu-training", "apps/minibatch-training", "apps/streaming-training",
                    "packages/owned_vec", "examples/owned-vector-results")
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


class GenericOwnedVectorApplications(unittest.TestCase):
    PROJECTS = {
        "packages/owned_vec": ("packages/owned_vec/src/lib.ox",),
        "examples/owned-vector-results": ("examples/owned-vector-results/src/main.ox",),
    }

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for project, members in self.PROJECTS.items():
            for name in (project + "/oxid.toml", *members):
                path = self.root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes((verify_repo.ROOT / name).read_bytes())

    def sources(self):
        return sorted(self.root.rglob("*.ox"))

    def test_both_exact_projects_keep_their_own_members_and_real_dependency(self):
        self.assertEqual({name: members for name, members in verify_repo.TYPED_MANIFEST_PROJECTS.items()
                          if name in self.PROJECTS}, self.PROJECTS)
        self.assertEqual(len(verify_repo.TYPED_MANIFEST_PROJECTS), 24)
        for project, members in self.PROJECTS.items():
            self.assertTrue(all((self.root / name).is_relative_to(self.root / project) for name in members))
        library = tomllib.loads((self.root / "packages/owned_vec/oxid.toml").read_text())
        example = tomllib.loads((self.root / "examples/owned-vector-results/oxid.toml").read_text())
        self.assertEqual((library["project"]["kind"], library["project"]["entry"]), ("lib", "src/lib.ox"))
        self.assertEqual(example["project"]["entry"], "src/main.ox")
        self.assertEqual(example["dependencies"], {"owned_vec": "../../packages/owned_vec"})
        self.assertEqual((self.root / "examples/owned-vector-results" / example["dependencies"]["owned_vec"]).resolve(),
                         (self.root / "packages/owned_vec").resolve())
        self.assertEqual((example["build"]["entry-mode"], example["execution"]["policy"]), ("result", "bounded"))

    def test_library_is_check_only_and_application_runs_without_a_zero_test_registration(self):
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_MANIFEST_PROJECTS.count("packages/owned_vec"), 1)
        self.assertNotIn("examples/owned-vector-results", verify_repo.TYPED_CHECK_ONLY_MANIFEST_PROJECTS)
        self.assertFalse(self.PROJECTS.keys() & set(verify_repo.TYPED_TESTED_MANIFEST_PROJECTS))
        self.assertEqual(len(verify_repo.TYPED_TESTED_MANIFEST_PROJECTS), 9)
        selected, roots = verify_repo.manifest_project_members(self.sources(), self.root)
        self.assertEqual(selected, set(self.sources()))
        self.assertEqual(roots, [self.root / project for project in self.PROJECTS])

    def test_missing_source_and_manifest_fail_for_each_project(self):
        for project, members in self.PROJECTS.items():
            for name in (project + "/oxid.toml", *members):
                path = self.root / name
                body = path.read_bytes()
                with self.subTest(missing=name):
                    path.unlink()
                    with self.assertRaisesRegex(RuntimeError, "missing"):
                        verify_repo.manifest_project_members(self.sources(), self.root)
                    path.write_bytes(body)

    def test_required_generic_projects_cannot_disappear_completely(self):
        for project, members in self.PROJECTS.items():
            paths = [self.root / name for name in (project + "/oxid.toml", *members)]
            bodies = [path.read_bytes() for path in paths]
            with self.subTest(project=project):
                for path in paths:
                    path.unlink()
                with patch.dict(verify_repo.TYPED_MANIFEST_PROJECTS, self.PROJECTS, clear=True):
                    with self.assertRaisesRegex(RuntimeError, "missing"):
                        verify_repo.manifest_project_members(self.sources(), self.root, required=True)
                for path, body in zip(paths, bodies, strict=True):
                    path.write_bytes(body)

    def test_unknown_neighbors_are_not_admitted_as_project_members(self):
        expected = set(self.sources())
        neighbors = [self.root / "packages/owned_vec/src/unlisted.ox",
                     self.root / "examples/owned-vector-results/src/unlisted.ox",
                     self.root / "examples/owned-vector-results-neighbor/main.ox"]
        for path in neighbors:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("unregistered legacy source\n")
        selected, roots = verify_repo.manifest_project_members(self.sources(), self.root)
        self.assertEqual(selected, expected)
        self.assertEqual(roots, [self.root / project for project in self.PROJECTS])
        self.assertFalse(selected & set(neighbors))

    def test_inventory_cannot_move_library_members_below_the_application_or_duplicate_them(self):
        for project, members in self.PROJECTS.items():
            other = next(values[0] for name, values in self.PROJECTS.items() if name != project)
            for wrong in ((other,), members + members, (project + "/../unlisted.ox",), ()):
                with self.subTest(project=project, members=wrong), \
                        patch.dict(verify_repo.TYPED_MANIFEST_PROJECTS, {project: wrong}, clear=True), \
                        self.assertRaisesRegex(RuntimeError, "invalid"):
                    verify_repo.manifest_project_members(self.sources(), self.root)

    def test_root_legacy_exclusion_is_one_exact_example_path(self):
        document = tomllib.loads((verify_repo.ROOT / "oxid.toml").read_text())
        configured = document["test-fixtures"]
        self.assertIs(configured["examples/owned-vector-results/src/main.ox"], True)
        self.assertEqual({name for name in configured if name.startswith("examples/owned-vector-results")},
                         {"examples/owned-vector-results/src/main.ox"})
        self.assertFalse(any(name.startswith("packages/owned_vec") for name in configured))
        self.assertNotIn("examples/owned-vector-results/src/unlisted.ox", configured)
        self.assertNotIn("examples/owned-vector-results-neighbor/main.ox", configured)


if __name__ == "__main__":
    unittest.main()
