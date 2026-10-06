"""Only the named current match rejection may differ from mutable history."""
import contextlib
import copy
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

import verify_mutable_locals as mutable


SELECTION = "bounded-enum-match-v1"
SOURCE = "// 雪\r\nfn main() -> () { let mut x = 1; match true { x = 2; } return; }"
HISTORICAL = ("unsupported_control", SOURCE, "E0101", "parse", None, None)
CURRENT = ("unsupported_control", SOURCE, "E0100", "parse", 45, 4)
SOURCE_SHA256 = "dd68ba7d2d902bce84163bd305d09ee4ef4ce63efccfe20dc4417a8892dde2fe"
OLD_EXPECTATION_SHA256 = "3a8bb56c474222dd5d503c6e04de5ef6883244dfdf258a2a63cd6ab9ffd2858f"
MESSAGE = "match requires a bare binding name"


def canonical_hash(value):
    encoded = json.dumps(value, ensure_ascii=True, separators=(",", ":"),
                         default=lambda scalar: scalar.record()).encode()
    return hashlib.sha256(encoded).hexdigest()


class BoundedEnumNegativeAmendmentTests(unittest.TestCase):
    def test_one_explicit_change_preserves_all_historical_sources_and_categories(self):
        historical = list(mutable.negative_cases())
        self.assertEqual(mutable.amended_negative_cases(), historical)
        self.assertEqual(mutable.amended_negative_cases(None), historical)
        current = mutable.amended_negative_cases(SELECTION)
        self.assertEqual(len(current), 49)
        self.assertEqual(len(historical), len(current))
        self.assertEqual([(old, new) for old, new in zip(historical, current) if old != new],
                         [(HISTORICAL, CURRENT)])
        self.assertEqual([(row[0], row[1].encode()) for row in current],
                         [(row[0], row[1].encode()) for row in historical])
        self.assertEqual([row for row in current if row[0] == "unsupported_control"],
                         [CURRENT] + [row for row in historical if row[0] == "unsupported_control" and row != HISTORICAL])
        self.assertEqual(list(mutable.negative_cases()), historical)
        self.assertEqual(mutable.amended_negative_cases(None), historical)

    def test_exact_source_original_tuple_and_metadata_seals(self):
        self.assertEqual(hashlib.sha256(SOURCE.encode()).hexdigest(), SOURCE_SHA256)
        self.assertEqual(canonical_hash(HISTORICAL), OLD_EXPECTATION_SHA256)
        self.assertEqual(mutable.ENUM_MATCH_NEGATIVE_SOURCE_SHA256, SOURCE_SHA256)
        self.assertEqual(mutable.ENUM_MATCH_NEGATIVE_OLD_EXPECTATION_SHA256, OLD_EXPECTATION_SHA256)
        metadata = mutable.negative_expectation_amendment(SELECTION)
        self.assertEqual(metadata["amendment_id"], SELECTION)
        self.assertEqual(metadata["scope"], "current-production-cli-only")
        self.assertEqual(metadata["source_sha256"], SOURCE_SHA256)
        self.assertEqual(metadata["old_expectation_sha256"], OLD_EXPECTATION_SHA256)
        self.assertEqual(metadata["old_expected"], {
            "category": "unsupported_control", "code": "E0101", "stage": "parse",
            "offset": None, "width": None,
        })
        self.assertEqual(metadata["effective_expected"], {
            "category": "unsupported_control", "code": "E0100", "stage": "parse",
            "offset": 45, "width": 4, "message": MESSAGE,
        })
        self.assertIsNone(mutable.negative_expectation_amendment(None))

    def test_independent_scrutinee_span_uses_characters_and_utf8_bytes(self):
        # The bounded grammar requires a binding immediately after match.
        # true is the first invalid token; no compiler output sets this oracle.
        offset = len("// 雪\r\nfn main() -> () { let mut x = 1; match ")
        self.assertEqual((offset, len("true")), CURRENT[-2:])
        self.assertEqual(SOURCE[offset:offset + 4], "true")
        self.assertEqual((len(SOURCE[:offset].encode()), len(SOURCE[:offset + 4].encode())), (47, 51))
        self.assertEqual(SOURCE[:offset].count("\n") + 1, 2)
        self.assertEqual(len(SOURCE[:offset].rsplit("\n", 1)[-1]) + 1, 40)

    def test_unknown_selection_rejected_before_model_or_binary_access(self):
        for selection in ("unchecked-v2", "checked-unary-negation-v1", "", True):
            with self.subTest(selection=selection):
                with self.assertRaises(ValueError):
                    mutable.negative_expectation_amendment(selection)
                with self.assertRaises(ValueError):
                    mutable.amended_negative_cases(selection)
                with mock.patch.object(mutable, "self_check_model") as model, self.assertRaises(ValueError):
                    mutable.verify(["missing-compiler"], expectation_amendment=selection)
                model.assert_not_called()

    def test_missing_duplicate_extra_fields_and_source_or_tuple_drift_rejected(self):
        cases = list(mutable.negative_cases())
        index = cases.index(HISTORICAL)
        variants = {
            "missing": cases[:index] + cases[index + 1:],
            "duplicate": cases + [HISTORICAL],
            "extra_tuple_field": cases[:index] + [HISTORICAL + ("extra",)] + cases[index + 1:],
        }
        for label, field, value in (
            ("category", 0, "invalid_scrutinee"),
            ("source_suffix", 1, SOURCE + " "),
            ("source_newlines", 1, SOURCE.replace("\r\n", "\n")),
            ("source_unicode", 1, SOURCE.replace("雪", "x")),
            ("code", 2, "E0100"), ("stage", 3, "type"),
            ("offset", 4, 45), ("offset_retyped", 4, False),
            ("width", 5, 4), ("width_retyped", 5, 4.0),
        ):
            row = list(HISTORICAL)
            row[field] = value
            variants[label] = cases[:index] + [tuple(row)] + cases[index + 1:]
        drifted_duplicate = list(HISTORICAL)
        drifted_duplicate[2] = "E9999"
        variants["extra_drifted_match"] = cases + [tuple(drifted_duplicate)]
        for label, changed in variants.items():
            with self.subTest(label=label):
                with mock.patch.object(mutable, "negative_cases", return_value=iter(changed)), self.assertRaises(ValueError):
                    mutable.amended_negative_cases(SELECTION)
                with mock.patch.object(mutable, "negative_cases", return_value=iter(changed)):
                    self.assertEqual(mutable.amended_negative_cases(None), changed)

    def test_extra_unrelated_case_is_never_amended(self):
        cases = list(mutable.negative_cases())
        extra = ("unsupported_control", SOURCE.replace("true", "false"), "E0101", "parse", None, None)
        with mock.patch.object(mutable, "negative_cases", return_value=iter(cases + [extra])):
            current = mutable.amended_negative_cases(SELECTION)
        self.assertEqual(current[-1], extra)
        self.assertEqual([(old, new) for old, new in zip(cases + [extra], current) if old != new],
                         [(HISTORICAL, CURRENT)])

    def assert_diagnostic(self, row, diagnostic, amendment):
        category, source, code, stage, offset, width = row
        mutable.assert_negative_diagnostic(category, source, diagnostic, code, stage, offset, width,
                                           amendment=amendment)

    def test_exact_current_code_stage_span_and_message_predicate(self):
        amendment = mutable.negative_expectation_amendment(SELECTION)
        diagnostic = {"code": "E0100", "stage": "parse", "message": MESSAGE,
                      "primary": {"start": 47, "end": 51, "line": 2, "column": 40}}
        self.assert_diagnostic(CURRENT, diagnostic, amendment)
        for key, wrong in (("code", "E0101"), ("stage", "type"),
                           ("message", "expected binding"), ("message", MESSAGE + ".")):
            with self.subTest(key=key, wrong=wrong), self.assertRaises(AssertionError):
                self.assert_diagnostic(CURRENT, {**diagnostic, key: wrong}, amendment)
        for key, wrong in (("start", 45), ("end", 50), ("end", 52), ("line", 1), ("column", 42)):
            changed = copy.deepcopy(diagnostic)
            changed["primary"][key] = wrong
            with self.subTest(key=key, wrong=wrong), self.assertRaises(AssertionError):
                self.assert_diagnostic(CURRENT, changed, amendment)

    def test_historical_message_and_span_scope_stays_unchanged(self):
        # The predecessor asserted neither a message nor a primary span.
        self.assert_diagnostic(HISTORICAL, {"code": "E0101", "stage": "parse"}, None)
        amendment = mutable.negative_expectation_amendment(SELECTION)
        for row in mutable.negative_cases():
            if row == HISTORICAL:
                continue
            _, source, code, stage, offset, width = row
            diagnostic = {"code": code, "stage": stage}
            if offset is not None:
                diagnostic["primary"] = {
                    "start": len(source[:offset].encode()), "end": len(source[:offset + width].encode()),
                    "line": source[:offset].count("\n") + 1,
                    "column": len(source[:offset].rsplit("\n", 1)[-1]) + 1,
                }
            with self.subTest(category=row[0], source=source[:100]):
                self.assert_diagnostic(row, diagnostic, amendment)

    def test_positive_model_and_resource_corpora_retain_historical_seals(self):
        mutable.self_check_model()
        corpus_hash, trace_hash = hashlib.sha256(), hashlib.sha256()
        positive_count = 0
        for _, source, _, _, trace in mutable.cases():
            positive_count += 1
            corpus_hash.update(source.encode() + b"\0")
            trace_hash.update(repr(trace).encode() + b"\0")
        self.assertEqual(positive_count, 163)
        self.assertEqual(corpus_hash.hexdigest(), "c031dd4729a3a182db387b678faacc0c190a8b8c3233c560782250353841d0c8")
        self.assertEqual(trace_hash.hexdigest(), "37caf97ae9362ba59ffdd6873f73e233819dcfd467f4021a823e08be9a6e18eb")
        for generator, count, expected_hash in (
            (mutable.negative_cases, 49, "3d896bed22c9c2b91446a9c4858073954451b198fdc5f7c9f0c2d60b711216d3"),
            (mutable.native_resource_cases, 13, "0b4e8a2b870749d2de5b82249351ff5c648194036781fbb11c854e7d7e25d935"),
            (mutable.reference_only_cases, 7, "e467233061249c89680d674981b9459f81bf6c913f3ceb2748dd6fa1212933f4"),
        ):
            rows = list(generator())
            with self.subTest(generator=generator.__name__):
                self.assertEqual(len(rows), count)
                self.assertEqual(canonical_hash(rows), expected_hash)

    def test_cli_routes_only_explicit_selection_and_keeps_historical_default(self):
        for argv, expected in (
            (["debug", "release"], None),
            (["debug", "release", "--expectation-amendment", SELECTION], SELECTION),
            (["--expectation-amendment", SELECTION, "debug", "release"], SELECTION),
        ):
            with self.subTest(argv=argv), mock.patch.object(mutable, "verify") as verify:
                mutable.main(argv)
                verify.assert_called_once_with(["debug", "release"], expectation_amendment=expected)
        for argv in ([], ["debug", "--expectation-amendment", "unchecked-v2"],
                     ["debug", "--expectation-amendment", "checked-unary-negation-v1"],
                     ["debug", "--expectation-amendment"]):
            with self.subTest(argv=argv), mock.patch.object(mutable, "verify") as verify:
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as exited:
                    mutable.main(argv)
                self.assertEqual(exited.exception.code, 2)
                verify.assert_not_called()

    def test_optimized_python_cannot_disable_oracle_assertions(self):
        result = subprocess.run([sys.executable, "-O", str(Path(mutable.__file__).resolve()), "missing-compiler",
                                 "--expectation-amendment", SELECTION], capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertIn(b"Python assertions must remain enabled", result.stderr)


if __name__ == "__main__":
    unittest.main()
