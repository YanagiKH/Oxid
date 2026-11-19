"""Keep preceding native assertions intact outside eight exact diagnostic changes."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import integer_bit_native_expectations as adapter


class NativeBitExpectations(unittest.TestCase):
    def test_all_old_case_bytes_are_preserved_outside_exact_pinned_changes(self):
        for suite, count in (("scalar_comparisons", 2), ("boolean_logic", 6)):
            original = adapter.load(suite)
            versions = [list(original.negative_cases())]
            if suite == "boolean_logic":
                versions.append(original.amended_negative_cases("checked-unary-negation-v1"))
            for before in versions:
                after = adapter.amended_cases(suite, before)
                self.assertEqual(len(before), len(after))
                changed = [(a, b) for a, b in zip(before, after) if a != b]
                self.assertEqual(len(changed), count)
                for a, b in changed:
                    self.assertEqual(a[:2], b[:2])
                    self.assertTrue(any(hashlib.sha256(a[1].encode()).hexdigest() == row[0]
                                        for row in adapter.CHANGES[suite]))
                self.assertEqual(list(original.negative_cases()), versions[0])

    def test_case_drift_and_coherent_amendment_changes_are_rejected(self):
        suite = "scalar_comparisons"
        before = list(adapter.load(suite).negative_cases())
        with self.assertRaisesRegex(ValueError, "roster"):
            adapter.amended_cases(suite, before[:-1])
        changed = list(before)
        changed[0] = (*changed[0][:2], "E9999", *changed[0][3:])
        with self.assertRaisesRegex(ValueError, "roster"):
            adapter.amended_cases(suite, changed)
        source, old, after = adapter.CHANGES[suite][0]
        replacements = ((source, old, (*after[:2], "E9999", *after[3:])),
                        *adapter.CHANGES[suite][1:])
        with patch.dict(adapter.CHANGES, {suite: replacements}), self.assertRaisesRegex(ValueError, "derived"):
            adapter.amended_cases(suite, before)

    def test_parent_hash_is_checked_before_python_execution(self):
        suite = "scalar_comparisons"
        name, _, _ = adapter.PARENTS[suite]
        original = Path(adapter.__file__).with_name(name).read_bytes()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in (original + b"\n", b'raise RuntimeError("untrusted executed")\n'):
                (root / name).write_bytes(body)
                with patch.object(adapter, "__file__", str(root / "adapter.py")), self.assertRaisesRegex(ValueError, "before Python execution"):
                    adapter.load(suite)

    def test_current_boolean_view_keeps_the_exact_unary_amendment(self):
        original = adapter.load("boolean_logic")
        current = adapter.prepare("boolean_logic")
        selection = "checked-unary-negation-v1"
        self.assertEqual(current.negative_expectation_amendment(selection),
                         original.negative_expectation_amendment(selection))
        self.assertEqual(current.amended_negative_cases(selection),
                         adapter.amended_cases("boolean_logic", original.amended_negative_cases(selection)))


if __name__ == "__main__":
    unittest.main()
