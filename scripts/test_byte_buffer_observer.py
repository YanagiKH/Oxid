"""Mutation controls for the exact ByteBuf observer exclusion successor.

The frozen predecessor is reconstructed by its unchanged authenticated helpers.
These Python controls execute no compiler and make no semantic execution claim.
"""
import hashlib
import importlib.util
import os
from pathlib import Path
import re
import unittest
from unittest.mock import patch

import byte_buffer_observer as adapter


REPO = Path(os.environ.get("OXID_OBSERVER_TEST_REPO", Path(__file__).resolve().parents[1]))
RUNNER = "tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py"
RUNNER_SHA = "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"


def sha(body):
    return hashlib.sha256(body).hexdigest()


class ByteBufferObserverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        runner = REPO / RUNNER
        if sha(runner.read_bytes()) != RUNNER_SHA:
            raise ValueError("changed frozen observer adapter authority")
        spec = importlib.util.spec_from_file_location("retained_byte_buffer_observer_parent", runner)
        cls.parent = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.parent)
        historical = (REPO / cls.parent.U2 / cls.parent.OBSERVER).read_bytes()
        aggregate = cls.parent.adapt_unit2_observer(historical)
        borrowed = cls.parent.adapt_borrowed_unit2_observer(aggregate)
        cls.original = cls.parent.adapt_enum_unit2_observer(borrowed)

    def test_exact_parent_successor_and_complete_reverse_identity(self):
        self.assertEqual(adapter.PREDECESSOR_VERSION, self.parent.ENUM_OBSERVER_ADAPTER_VERSION)
        self.assertEqual(adapter.PREDECESSOR_SHA, self.parent.ENUM_OBSERVER_DERIVED_SHA)
        self.assertEqual(adapter.PREDECESSOR_BYTES, self.parent.ENUM_OBSERVER_DERIVED_BYTES)
        derived = adapter.adapt(self.original)
        self.assertEqual((len(derived), sha(derived)), (adapter.DERIVED_BYTES, adapter.DERIVED_SHA))
        self.assertEqual(len(adapter.SEAMS), 2)
        self.assertTrue(derived.endswith(adapter.CONTROL_TESTS))
        restored = derived[:-len(adapter.CONTROL_TESTS)]
        for before, after in reversed(adapter.SEAMS):
            self.assertEqual(restored.count(after), 1)
            restored = restored.replace(after, before, 1)
        self.assertEqual(restored, self.original)
        old_filter = rb"^fn (current_unit2_aggregate_adapter_[A-Za-z_0-9]+)\("
        old_names = re.findall(old_filter, self.original, re.M)
        self.assertEqual(len(old_names), 6)
        self.assertEqual(re.findall(old_filter, derived, re.M), old_names)
        self.assertEqual(derived.count(b"fn current_unit2_byte_buffer_adapter_"), 3)
        self.assertNotIn(b"fn current_unit2_byte_buffer_adapter_", self.original)
        self.assertEqual(derived.count(
            b'AggregateTy::ByteBuf => panic!("current Unit2 observer excludes byte-buffer projection")'
        ), 1)
        self.assertEqual(derived.count(
            b'BorrowedTy::Exact(AggregateTy::ByteBuf) => panic!("current Unit2 observer excludes byte-buffer projection")'
        ), 1)

    def test_unknown_parent_truncation_same_length_edit_extra_fixture_and_readaptation_reject(self):
        same_length = self.original.replace(b'"kind"', b'"KIND"', 1)
        self.assertEqual(len(same_length), len(self.original))
        self.assertNotEqual(same_length, self.original)
        extra_fixture = b"\n#[test]\nfn unreviewed_observer_fixture() {}\n"
        for body in (b"", self.original[:-1], self.original + b"\n", same_length,
                     self.original + extra_fixture, adapter.adapt(self.original)):
            with self.subTest(sha=sha(body)), self.assertRaisesRegex(ValueError, "wrong predecessor"):
                adapter.adapt(body)

    def test_parent_and_successor_require_both_lengths_and_hashes(self):
        for key, value, error in (
            ("PREDECESSOR_BYTES", adapter.PREDECESSOR_BYTES + 1, "wrong predecessor"),
            ("PREDECESSOR_SHA", "0" * 64, "wrong predecessor"),
            ("DERIVED_BYTES", adapter.DERIVED_BYTES + 1, "wrong derived"),
            ("DERIVED_SHA", "0" * 64, "wrong derived"),
        ):
            with self.subTest(key=key), patch.object(adapter, key, value):
                with self.assertRaisesRegex(ValueError, error):
                    adapter.adapt(self.original)

    def test_exact_substitution_count_and_unique_context_are_required(self):
        for seams in ((), adapter.SEAMS[:1], adapter.SEAMS + adapter.SEAMS[:1]):
            with self.subTest(count=len(seams)), patch.object(adapter, "SEAMS", seams):
                with self.assertRaisesRegex(ValueError, "substitution count"):
                    adapter.adapt(self.original)
        old, new = adapter.SEAMS[0]
        for replacement in ((old + b"missing context", new), (old, old)):
            with patch.object(adapter, "SEAMS", (replacement, adapter.SEAMS[1])):
                with self.assertRaisesRegex(ValueError, "seam drift"):
                    adapter.adapt(self.original)

    def test_each_deny_arm_cannot_be_omitted_replaced_or_made_a_wildcard(self):
        for index, (old, new) in enumerate(adapter.SEAMS):
            altered = (
                new.replace(b'panic!("current Unit2 observer excludes byte-buffer projection")', b"0"),
                new.replace(b"AggregateTy::ByteBuf", b"_"),
                new + b"// extra observer instruction\n",
            )
            for replacement in altered:
                seams = adapter.SEAMS[:index] + ((old, replacement),) + adapter.SEAMS[index + 1:]
                with self.subTest(index=index, sha=sha(replacement)), patch.object(adapter, "SEAMS", seams):
                    with self.assertRaisesRegex(ValueError, "wrong derived"):
                        adapter.adapt(self.original)

    def test_reversible_same_length_edit_still_fails_the_pinned_derived_hash(self):
        seams = tuple((old, new.replace(b"byte-buffer projection", b"fake-buffer projection"))
                      for old, new in adapter.SEAMS)
        changed = self.original
        for old, new in seams:
            changed = changed.replace(old, new, 1)
        changed += adapter.CONTROL_TESTS
        self.assertEqual(len(changed), adapter.DERIVED_BYTES)
        self.assertNotEqual(sha(changed), adapter.DERIVED_SHA)
        restored = changed[:-len(adapter.CONTROL_TESTS)]
        for old, new in reversed(seams):
            restored = restored.replace(new, old, 1)
        self.assertEqual(restored, self.original)
        with patch.object(adapter, "SEAMS", seams):
            with self.assertRaisesRegex(ValueError, "wrong derived"):
                adapter.adapt(self.original)

    def test_control_body_changes_or_extra_tests_cannot_enter_the_successor(self):
        for body, error in (
            (adapter.CONTROL_TESTS + b"\n", "wrong derived"),
            (adapter.CONTROL_TESTS.replace(b"kind: BorrowKind::Shared", b"kind: BorrowKind::Exclusive"),
             "wrong derived"),
            (adapter.CONTROL_TESTS.replace(b"ValueTy::Owned(AggregateTy::ByteBuf)",
                                           b"ValueTy::Scalar(hir::Ty::I32)"), "wrong derived"),
            (adapter.CONTROL_TESTS + b"\n#[test]\nfn extra_fixture() {}\n", "rejection controls"),
            (adapter.CONTROL_TESTS.replace(b"#[should_panic", b"#[not_a_panic", 1), "rejection controls"),
        ):
            with self.subTest(sha=sha(body)), patch.object(adapter, "CONTROL_TESTS", body):
                with self.assertRaisesRegex(ValueError, error):
                    adapter.adapt(self.original)

    def test_control_names_and_filter_cannot_hide_missing_or_duplicate_tests(self):
        for key, value in (
            ("CONTROL_NAMES", adapter.CONTROL_NAMES[:2]),
            ("CONTROL_NAMES", adapter.CONTROL_NAMES[:2] + adapter.CONTROL_NAMES[:1]),
            ("CONTROL_NAMES", tuple(reversed(adapter.CONTROL_NAMES))),
            ("CONTROL_FILTER", "current_unit2_aggregate_adapter_"),
        ):
            with self.subTest(key=key), patch.object(adapter, key, value):
                with self.assertRaisesRegex(ValueError, "rejection controls"):
                    adapter.adapt(self.original)

    def test_descriptor_records_exact_scope_without_claiming_execution(self):
        value = adapter.descriptor()
        self.assertEqual(value["predecessor"], {
            "path": adapter.OBSERVER, "bytes": len(self.original), "sha256": sha(self.original)})
        derived = adapter.adapt(self.original)
        self.assertEqual(value["derived"], {
            "path": adapter.OBSERVER, "bytes": len(derived), "sha256": sha(derived)})
        self.assertEqual(len(value["substitutions"]), 2)
        self.assertEqual(value["appended_controls"], {
            "bytes": len(adapter.CONTROL_TESTS), "sha256": sha(adapter.CONTROL_TESTS)})
        self.assertEqual(value["control_names"], list(adapter.CONTROL_NAMES))
        self.assertEqual(value["control_filter"], adapter.CONTROL_FILTER)
        self.assertIs(value["byte_buffer_semantics_qualified"], False)
        self.assertNotIn("semantic_pass", value)
        value["control_names"].clear()
        self.assertEqual(adapter.descriptor()["control_names"], list(adapter.CONTROL_NAMES))


if __name__ == "__main__":
    unittest.main()
