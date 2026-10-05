"""Only the named current unary cases may differ from historical oracles."""
import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import verify_owned_source as owned
import verify_boolean_logic as boolean


SELECTION = "checked-unary-negation-v1"
IDENTIFIERS = ("scalar-general-minus", "scalar-minus-group")


class OwnedUnaryAmendmentTests(unittest.TestCase):
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

    def test_exact_two_changes_extend_all_unchanged_predecessor_rows(self):
        before = {p.name: owned.digest(p) for p in self.frozen.iterdir()}
        predecessor = owned.load_cli_amendment("owned-record-composition-v1", self.frozen, mode="cli")
        amended = self.amendment()
        self.assertEqual(amended["cases"][:4], predecessor["cases"])
        self.assertEqual(amended["predecessor_amendment_sha256"], predecessor["data_sha256"])
        self.assertEqual(amended["amendment_id"], SELECTION)
        self.assertEqual(amended["scope"], "current-production-cli-only")
        self.assertIs(amended["full_qualification"], False)
        self.assertEqual(amended["data_sha256"], owned.digest(owned.UNARY_CLI_AMENDMENT_PATH))
        self.assertEqual([r["id"] for r in amended["cases"][4:]], list(IDENTIFIERS))
        changed = []
        for row in self.manifest["files"]:
            item = owned.strict_json_loads((self.frozen / (row["id"] + ".json")).read_text())
            original = copy.deepcopy(item)
            effective = owned.apply_cli_amendment(item, amended)
            if effective != owned.apply_cli_amendment(item, predecessor):
                changed.append(item["id"])
                self.assertEqual(effective["expected"], {
                    "status": "accept", "result_type": "i32", "runtime_failure": None,
                    "result": dict(zip(IDENTIFIERS, (-3, -1)))[item["id"]],
                })
                self.assertEqual(item["expected"]["code"], "E0101")
                self.assertEqual(item["expected"]["stage"], "parse")
                self.assertEqual(effective["function_count"], 5)
                self.assertEqual(effective, {**item, "expected": effective["expected"]})
            self.assertEqual(item, original)
        self.assertEqual(changed, list(IDENTIFIERS))
        self.assertEqual(owned.verify_frozen(self.frozen), self.manifest)
        self.assertEqual(before, {p.name: owned.digest(p) for p in self.frozen.iterdir()})

    def test_changed_incomplete_retyped_or_overqualified_document_rejected(self):
        original = owned.strict_json_loads(owned.UNARY_CLI_AMENDMENT_PATH.read_text())
        for mutate in (
            lambda a: a["cases"].pop(),
            lambda a: a["cases"].append(copy.deepcopy(a["cases"][-1])),
            lambda a: a["cases"][0]["effective_expected"].update(result=99),
            lambda a: a["cases"][4]["effective_expected"].update(result=3),
            lambda a: a["cases"][5]["effective_expected"].update(result=-1.0),
            lambda a: a["cases"][5]["effective_expected"].update(result=True),
            lambda a: a["cases"][4].update(source_sha256="0" * 64),
            lambda a: a["cases"][5].update(old_expectation_sha256="0" * 64),
            lambda a: a.update(predecessor_amendment_sha256="0" * 64),
            lambda a: a.update(full_qualification=True),
            lambda a: a.update(scope="candidate"),
        ):
            data = copy.deepcopy(original)
            mutate(data)
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "amendment.json"
                path.write_text(json.dumps(data))
                with mock.patch.object(owned, "UNARY_CLI_AMENDMENT_PATH", path), self.assertRaises(ValueError):
                    self.amendment()

    def test_source_old_expectation_and_function_count_are_pinned(self):
        amendment = self.amendment()
        for identifier in IDENTIFIERS:
            item = owned.strict_json_loads((self.frozen / (identifier + ".json")).read_text())
            for key, value in (("source_sha256", "0" * 64), ("function_count", 6),
                               ("function_count", 5.0), ("expected", {"status": "accept"})):
                with self.subTest(identifier=identifier, key=key), self.assertRaises(ValueError):
                    owned.apply_cli_amendment({**item, key: value}, amendment)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for row in amendment["cases"]:
                for suffix in (".ox", ".json"):
                    name = row["id"] + suffix
                    (root / name).write_bytes((self.frozen / name).read_bytes())
            for identifier in IDENTIFIERS:
                for suffix in (".ox", ".json"):
                    path = root / (identifier + suffix)
                    original = path.read_bytes()
                    path.write_bytes(original + b" ")
                    with self.subTest(path=path), self.assertRaises(ValueError):
                        self.amendment(root)
                    path.write_bytes(original)

    def test_cli_selection_is_explicit_and_cannot_widen_other_modes(self):
        self.assertIsNone(owned.load_cli_amendment(None, self.frozen, mode="cli"))
        for mode in ("model", "freeze", "candidate", "production"):
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                owned.load_cli_amendment(SELECTION, self.frozen, mode=mode)
            with mock.patch.object(owned, "verify_frozen") as frozen, contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(owned.main(["--mode", mode, "--cli-amendment", SELECTION]), 1)
                frozen.assert_not_called()


class BooleanUnaryAmendmentTests(unittest.TestCase):
    def test_one_explicit_successor_preserves_historical_negative_corpus(self):
        historical = list(boolean.negative_cases())
        self.assertEqual(boolean.amended_negative_cases(None), historical)
        current = boolean.amended_negative_cases(SELECTION)
        changes = [(old, new) for old, new in zip(historical, current) if old != new]
        self.assertEqual(len(current), len(historical))
        self.assertEqual(len(changes), 1)
        old, new = changes[0]
        self.assertEqual(old, ("invalid_token", "// 🦀\r\nfn main() -> bool { return -(1); }", "E0101", "parse", 33, 1))
        self.assertEqual(new, ("invalid_return_type", old[1], "E0300", "type", 33, 4))
        self.assertEqual(list(boolean.negative_cases()), historical)
        self.assertEqual(hashlib.sha256(old[1].encode()).hexdigest(), boolean.UNARY_NEGATIVE_SOURCE_SHA256)
        metadata = boolean.negative_expectation_amendment(SELECTION)
        self.assertEqual(metadata["old_expected"], {"category": "invalid_token", "code": "E0101", "stage": "parse", "offset": 33, "width": 1})
        self.assertEqual(metadata["effective_expected"], {"category": "invalid_return_type", "code": "E0300", "stage": "type", "offset": 33, "width": 4})
        self.assertIsNone(boolean.negative_expectation_amendment(None))

    def test_unknown_changed_missing_duplicate_or_retyped_predecessor_rejected(self):
        with self.assertRaises(ValueError):
            boolean.amended_negative_cases("unchecked-v2")
        cases = list(boolean.negative_cases())
        index = next(i for i, item in enumerate(cases) if "-(1)" in item[1])
        old = cases[index]
        variants = [cases[:index] + cases[index + 1:], cases + [old]]
        for field, value in ((1, old[1] + " "), (2, "E9999"), (3, "type"), (4, 33.0), (5, 4)):
            mutated = list(old)
            mutated[field] = value
            variants.append(cases[:index] + [tuple(mutated)] + cases[index + 1:])
        for changed in variants:
            with self.subTest(changed=changed[index:index + 1]), mock.patch.object(boolean, "negative_cases", return_value=iter(changed)), self.assertRaises(ValueError):
                boolean.amended_negative_cases(SELECTION)

    def test_cli_propagates_explicit_selection_and_keeps_historical_default(self):
        for argv, expected in ((["debug", "release"], None), (["debug", "release", "--expectation-amendment", SELECTION], SELECTION)):
            with mock.patch.object(boolean, "verify") as verify:
                boolean.main(argv)
                verify.assert_called_once_with(["debug", "release"], expectation_amendment=expected)
        with mock.patch.object(boolean, "verify") as verify, contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            boolean.main(["debug", "--expectation-amendment", "unchecked-v2"])
        verify.assert_not_called()


if __name__ == "__main__":
    unittest.main()
