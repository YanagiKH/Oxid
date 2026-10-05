"""The projected-borrow CLI successor preserves all historical oracle bytes."""
import contextlib
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import verify_owned_source as owned

SELECTION = "projected-array-slices-v1"
IDENTIFIER = "excluded-field-borrow"


class ProjectedCliAmendmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.frozen = Path(cls.temporary.name) / "frozen"
        cls.manifest = owned.freeze(cls.frozen)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def amendment(self, directory=None):
        return owned.load_cli_amendment(SELECTION, directory or self.frozen, mode="cli")

    def test_exact_diagnostic_change_extends_unchanged_unary_predecessor(self):
        before = {p.name: owned.digest(p) for p in self.frozen.iterdir()}
        predecessor = owned.load_cli_amendment("checked-unary-negation-v1", self.frozen, mode="cli")
        amended = self.amendment()
        self.assertEqual(amended["cases"][:-1], predecessor["cases"])
        self.assertEqual(amended["predecessor_amendment_sha256"], predecessor["data_sha256"])
        self.assertEqual(amended["data_sha256"], owned.digest(owned.PROJECTED_CLI_AMENDMENT_PATH))
        self.assertEqual(amended["scope"], "current-production-cli-only")
        self.assertIs(amended["full_qualification"], False)
        changed = []
        for row in self.manifest["files"]:
            item = owned.strict_json_loads((self.frozen / (row["id"] + ".json")).read_text())
            original = copy.deepcopy(item)
            effective = owned.apply_cli_amendment(item, amended)
            if effective != owned.apply_cli_amendment(item, predecessor):
                changed.append(item["id"])
                source = (self.frozen / (item["id"] + ".ox")).read_bytes()
                start = source.index(b"read(")
                self.assertEqual(effective["expected"], {
                    "status": "reject", "code": "E0200", "stage": "resolve",
                    "message": "unknown direct function `read`", "span": [start, start + 4],
                    "span_match": "exact", "related": [],
                })
                self.assertEqual(item["expected"]["code"], "E0101")
                self.assertEqual(item["expected"]["stage"], "parse")
                self.assertEqual(effective, {**item, "expected": effective["expected"]})
            self.assertEqual(item, original)
        self.assertEqual(changed, [IDENTIFIER])
        self.assertEqual(owned.verify_frozen(self.frozen), self.manifest)
        self.assertEqual(before, {p.name: owned.digest(p) for p in self.frozen.iterdir()})

    def test_changed_incomplete_retyped_or_overqualified_document_rejected(self):
        original = owned.strict_json_loads(owned.PROJECTED_CLI_AMENDMENT_PATH.read_text())
        for mutate in (
            lambda a: a["cases"].pop(),
            lambda a: a["cases"].append(copy.deepcopy(a["cases"][-1])),
            lambda a: a["cases"][0]["effective_expected"].update(result=99),
            lambda a: a["cases"][-1]["effective_expected"].update(stage="parse"),
            lambda a: a["cases"][-1]["effective_expected"].update(span=[67.0, 71]),
            lambda a: a["cases"][-1].update(source_sha256="0" * 64),
            lambda a: a["cases"][-1].update(old_expectation_sha256="0" * 64),
            lambda a: a.update(predecessor_amendment_sha256="0" * 64),
            lambda a: a.update(full_qualification=True),
            lambda a: a.update(scope="candidate"),
        ):
            data = copy.deepcopy(original)
            mutate(data)
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "amendment.json"
                path.write_text(json.dumps(data))
                with mock.patch.object(owned, "PROJECTED_CLI_AMENDMENT_PATH", path), self.assertRaises(ValueError):
                    self.amendment()

    def test_source_old_expectation_and_function_count_are_pinned(self):
        amendment = self.amendment()
        item = owned.strict_json_loads((self.frozen / (IDENTIFIER + ".json")).read_text())
        for key, value in (("source_sha256", "0" * 64), ("function_count", 1),
                           ("expected", {"status": "accept"})):
            with self.subTest(key=key), self.assertRaises(ValueError):
                owned.apply_cli_amendment({**item, key: value}, amendment)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for row in amendment["cases"]:
                for suffix in (".ox", ".json"):
                    name = row["id"] + suffix
                    (root / name).write_bytes((self.frozen / name).read_bytes())
            for suffix in (".ox", ".json"):
                path = root / (IDENTIFIER + suffix)
                original = path.read_bytes()
                path.write_bytes(original + b" ")
                with self.subTest(path=path), self.assertRaises(ValueError):
                    self.amendment(root)
                path.write_bytes(original)

    def test_explicit_selection_cannot_widen_other_modes(self):
        self.assertIsNone(owned.load_cli_amendment(None, self.frozen, mode="cli"))
        for mode in ("model", "freeze", "candidate", "production"):
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                owned.load_cli_amendment(SELECTION, self.frozen, mode=mode)
            with mock.patch.object(owned, "verify_frozen") as frozen, contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(owned.main(["--mode", mode, "--cli-amendment", SELECTION]), 1)
                frozen.assert_not_called()


if __name__ == "__main__":
    unittest.main()
