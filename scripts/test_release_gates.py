from __future__ import annotations

import contextlib
import copy
import io
import json
import tempfile
import unittest
from pathlib import Path

from check_release_gates import INVENTORY, load_inventory, main, release_decision


class ReleaseGateTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="oxid-release-gates-")
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "inventory.json"
        self.manifest = Path(self.temp.name) / "Cargo.toml"
        self.manifest.write_text('[package]\nversion = "0.9.0"\n', encoding="utf-8")
        self.document = json.loads(INVENTORY.read_text(encoding="utf-8"))

    def write(self) -> None:
        self.path.write_text(json.dumps(self.document), encoding="utf-8")

    def run_check(self, *arguments: str) -> tuple[int, str, str]:
        self.write()
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = main(list(arguments), inventory=self.path, manifest=self.manifest)
        return status, stdout.getvalue(), stderr.getvalue()

    def test_report_validates_inventory_without_claiming_readiness(self) -> None:
        status, stdout, stderr = self.run_check()
        self.assertEqual(status, 0)
        self.assertEqual(stderr, "")
        self.assertIn("NOT READY", stdout)
        self.assertIn("Stable v1 publication remains blocked", stdout)
        self.assertEqual(len(load_inventory(self.path)), 10)

    def test_stable_v1_cannot_pass_with_inventory_alone(self) -> None:
        for tag in ("v1.0.0", "v1.0.1", "v1.9.0+build.1"):
            with self.subTest(tag=tag):
                status, _, stderr = self.run_check(
                    "--release-tag", tag, "--package-version", tag[1:]
                )
                self.assertEqual(status, 1)
                self.assertIn("complete G1-G10 technical acceptance", stderr)

    def test_prereleases_remain_available_for_qualification(self) -> None:
        for tag in ("v1.0.0-rc.1", "v1.1.0-alpha+build.1"):
            with self.subTest(tag=tag):
                status, stdout, stderr = self.run_check(
                    "--release-tag", tag, "--package-version", tag[1:]
                )
                self.assertEqual(status, 0)
                self.assertEqual(stderr, "")
                self.assertIn("publish it as a prerelease", stdout)
                self.assertIn("does not establish stable v1 readiness", stdout)

    def test_legacy_release_path_remains_available(self) -> None:
        status, stdout, _ = self.run_check("--release-tag", "v0.9.0")
        self.assertEqual(status, 0)
        self.assertIn("v1 technical gates do not block it", stdout)

    def test_v1_package_cannot_bypass_gate_with_a_legacy_or_prerelease_tag(self) -> None:
        self.manifest.write_text('[package]\nversion = "1.0.0"\n', encoding="utf-8")
        for tag in ("v0.9.0", "v1.0.0-rc.1"):
            with self.subTest(tag=tag):
                status, _, stderr = self.run_check("--release-tag", tag)
                self.assertEqual(status, 1)
                self.assertIn("does not match package.version", stderr)
        status, _, stderr = self.run_check("--release-tag", "v1.0.0")
        self.assertEqual(status, 1)
        self.assertIn("complete G1-G10 technical acceptance", stderr)

    def test_prerelease_requires_matching_manifest_version(self) -> None:
        tag = "v1.0.0-rc.1"
        status, _, stderr = self.run_check("--release-tag", tag)
        self.assertEqual(status, 1)
        self.assertIn("does not match package.version", stderr)
        self.manifest.write_text('[package]\nversion = "1.0.0-rc.1"\n', encoding="utf-8")
        self.assertEqual(self.run_check("--release-tag", tag)[0], 0)

    def test_explicit_version_is_validated_when_provided(self) -> None:
        self.manifest.unlink()
        self.assertEqual(self.run_check(
            "--release-tag", "v0.9.0", "--package-version", "0.9.0"
        )[0], 0)
        for version in ("1.0.0", "", "v0.9.0", "0.9.0+different"):
            with self.subTest(version=version):
                status, _, stderr = self.run_check(
                    "--release-tag", "v0.9.0", "--package-version", version
                )
                self.assertEqual(status, 1)
                self.assertIn("does not match package.version", stderr)

    def test_bad_manifest_blocks_release_but_does_not_block_report(self) -> None:
        for content in ("[invalid", "", "[package]\nversion = 1", "package = true"):
            with self.subTest(content=content):
                self.manifest.write_text(content, encoding="utf-8")
                self.assertEqual(self.run_check("--release-tag", "v0.9.0")[0], 1)
                self.assertEqual(self.run_check()[0], 0)
        self.manifest.unlink()
        self.assertEqual(self.run_check("--release-tag", "v0.9.0")[0], 1)
        self.assertEqual(self.run_check()[0], 0)

    def test_release_flags_and_filenames_cannot_authorize_a_release(self) -> None:
        original = copy.deepcopy(self.document)
        for field, value in (("status", "passed"), ("status", True),
                             ("ready", True), ("evidence", ["passing.json"])):
            with self.subTest(field=field, value=value):
                self.document = copy.deepcopy(original)
                self.document["gates"][0][field] = value
                status, _, stderr = self.run_check("--release-tag", "v1.0.0")
                self.assertEqual(status, 1)
                self.assertIn("release gate error", stderr)

    def test_missing_duplicate_unknown_and_empty_gates_are_rejected(self) -> None:
        original = copy.deepcopy(self.document)
        mutations = (
            lambda gates: gates.pop(),
            lambda gates: gates.__setitem__(0, copy.deepcopy(gates[1])),
            lambda gates: gates[0].__setitem__("id", "G11"),
            lambda gates: gates[0].__setitem__("remaining", []),
            lambda gates: gates[0].__setitem__("required", [""]),
        )
        for mutate in mutations:
            self.document = copy.deepcopy(original)
            mutate(self.document["gates"])
            self.write()
            with self.assertRaises(ValueError):
                load_inventory(self.path)

    def test_malformed_tags_and_stable_higher_major_cannot_bypass_check(self) -> None:
        for tag in ("v1", "v1.0", "v01.0.0", "1.0.0", "v1.0.0-rc.01", "v1.0.0-", "v1.0.0\n"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release_decision(tag, tag[1:])
        self.assertFalse(release_decision("v2.0.0", "2.0.0")[0])

    def test_malformed_inventory_fails_cleanly(self) -> None:
        for document in (None, [], {}, {"schema_version": True}):
            self.document = document
            status, _, stderr = self.run_check()
            self.assertEqual(status, 1)
            self.assertIn("release gate error", stderr)


if __name__ == "__main__":
    unittest.main()
