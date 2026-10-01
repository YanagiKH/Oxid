from __future__ import annotations

import contextlib
import copy
import io
import json
import tempfile
import unittest
from pathlib import Path

from verify_feature_status import verify_feature_status


class FeatureStatusTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="oxid-status-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "docs").mkdir()
        (self.root / "spec.md").write_text("scope", encoding="utf-8")
        self.document = {
            "schema_version": 1,
            "baseline_commit": "a" * 40,
            "scope": "test inventory",
            "features": [{
                "id": "legacy",
                "status": "experimental",
                "classification": "production-path",
                "summary": "existing behavior",
                "owner": "unassigned",
                "reviewer": "unassigned",
                "edition": "legacy-0.9",
                "backend": "ast-interpreter",
                "target_scope": "test only",
                "limitations": "partial specification",
                "spec": ["spec.md"],
                "implementation": [],
                "tests": [],
                "evidence": [],
            }],
        }

    def verify(self) -> None:
        (self.root / "docs/feature-status.json").write_text(
            json.dumps(self.document), encoding="utf-8"
        )
        with contextlib.redirect_stdout(io.StringIO()):
            verify_feature_status(self.root)

    def test_experimental_scope_is_accepted(self) -> None:
        self.verify()

    def test_unknown_status_and_duplicate_ids_are_rejected(self) -> None:
        self.document["features"][0]["status"] = "complete"
        with self.assertRaisesRegex(ValueError, "unknown status"):
            self.verify()
        self.document["features"][0]["status"] = "experimental"
        self.document["features"].append(copy.deepcopy(self.document["features"][0]))
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.verify()

    def test_missing_and_escaping_evidence_paths_are_rejected(self) -> None:
        for path in ("missing.md", "../outside.md"):
            with self.subTest(path=path):
                self.document["features"][0]["evidence"] = [path]
                with self.assertRaisesRegex(ValueError, "missing or escaping"):
                    self.verify()

    def test_implemented_requires_code_and_tests(self) -> None:
        self.document["features"][0]["status"] = "implemented"
        with self.assertRaisesRegex(ValueError, "need code and tests"):
            self.verify()

    def test_demo_is_not_a_production_implementation(self) -> None:
        feature = self.document["features"][0]
        feature.update(status="implemented", classification="demo",
                       implementation=["spec.md"], tests=["spec.md"])
        with self.assertRaisesRegex(ValueError, "production path"):
            self.verify()

    def test_validation_requires_owner_and_execution_evidence(self) -> None:
        feature = self.document["features"][0]
        feature.update(status="validated", implementation=["spec.md"], tests=["spec.md"])
        with self.assertRaisesRegex(ValueError, "owner, reviewer, and evidence"):
            self.verify()
        feature.update(owner="maintainer", reviewer="reviewer", evidence=["spec.md"])
        self.verify()

    def test_stability_requires_review_and_compatibility_policy(self) -> None:
        feature = self.document["features"][0]
        feature.update(status="stable", owner="maintainer", reviewer="reviewer", implementation=["spec.md"],
                       tests=["spec.md"], evidence=["spec.md"])
        with self.assertRaisesRegex(ValueError, "compatibility_policy"):
            self.verify()
        feature["compatibility_policy"] = "reviewed migration contract"
        with self.assertRaisesRegex(ValueError, "review"):
            self.verify()
        feature["review"] = "review record"
        self.verify()


if __name__ == "__main__":
    unittest.main()
