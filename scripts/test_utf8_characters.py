"""Unicode corpus coverage for owned-text character access."""
import unittest

import verify_utf8_characters as oracle


class CharacterTextOracle(unittest.TestCase):
    def test_native_ranges_cover_every_scalar_without_gaps_or_duplicates(self):
        previous = 0
        count = 0
        for first, end in oracle.scalar_ranges():
            self.assertEqual(first, previous)
            self.assertTrue(first < end <= 0x110000)
            self.assertLessEqual(end - first, 128)
            actual = oracle.expected_range(first, end).decode("utf-8", errors="strict")
            expected = []
            for code in range(first, end):
                try:
                    value = chr(code)
                    value.encode("utf-8", errors="strict")
                    expected.append(value)
                except UnicodeEncodeError:
                    pass
            self.assertEqual(actual, "".join(expected))
            count += len(actual)
            previous = end
        self.assertEqual((previous, count), (0x110000, 1112064))

    def test_boundary_encodings_and_empty_surrogate_ranges_are_exact(self):
        for first, end, expected in [
            (0, 1, b"\0"), (0x7f, 0x81, b"\x7f\xc2\x80"),
            (0x7ff, 0x801, b"\xdf\xbf\xe0\xa0\x80"),
            (0xd800, 0xe000, b""), (0x10ffff, 0x110000, b"\xf4\x8f\xbf\xbf"),
            (65, 65, b""), (65, 68, b"ABC"),
        ]:
            self.assertEqual(oracle.expected_range(first, end), expected)

    def test_reference_ranges_retain_all_encoding_transitions(self):
        ranges = oracle.reference_ranges()
        self.assertEqual(ranges, sorted(set(ranges)))
        for boundary in (0x80, 0x800, 0xd800, 0xe000, 0x10000):
            self.assertTrue(any(first < boundary < end for first, end in ranges))
        self.assertTrue(any(end == 0x110000 for _, end in ranges))


if __name__ == "__main__":
    unittest.main()
