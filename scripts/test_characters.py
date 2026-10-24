"""Independent Unicode and complete-width controls for the character oracle."""
import unittest

import verify_characters as oracle


class CharacterModelTests(unittest.TestCase):
    def test_scalar_domain_matches_strict_utf8_encoding(self):
        for code in range(0x110001):
            try:
                chr(code).encode("utf-8", errors="strict")
                accepted = True
            except (ValueError, UnicodeEncodeError):
                accepted = False
            self.assertEqual(oracle.scalar(code), accepted, hex(code))
        self.assertFalse(oracle.scalar(-1))

    def test_conversion_rejects_high_bits_before_narrowing(self):
        for value in (-1, -(2**127), 0xd800, 0xdfff, 0x110000,
                      2**32 + 65, 2**64 + 65, 2**127 + 65, 2**128 - 1):
            self.assertEqual(oracle.expected("u128", "to_char", value, 0), (None, "E0612"))
        for value in (0, 0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xe000, 0x10ffff):
            self.assertEqual(oracle.expected("u128", "to_char", value, 0), (chr(value), None))
            self.assertEqual(oracle.expected("u128", "roundtrip", value, 0), (value, None))

    def test_argument_parsing_and_exact_failure_origin_are_preserved(self):
        for ty in oracle.oracle.TYPES:
            parent, _, _ = oracle.parent.source(ty, "identity")
            for operation in oracle.operations(ty):
                source, _, origin = oracle.source(ty, operation)
                self.assertEqual(source[:source.index("fn apply(")], parent[:parent.index("fn apply(")])
                self.assertEqual(source[origin["start"]:origin["end"]], "to_char_checked")
                self.assertEqual(source[:origin["start"]].count("\n") + 1, origin["line"])
                low, high = oracle.oracle.bounds(ty)
                for a, b in oracle.cases(ty, operation):
                    self.assertTrue(low <= a <= high and low <= b <= high)
                    self.assertTrue(oracle.scalar(b))


if __name__ == "__main__":
    unittest.main()
