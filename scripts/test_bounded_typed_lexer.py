import unittest

from verify_bounded_typed_lexer import decode


def success():
    return bytearray(b'OXL1' + bytes([0, 1, 0, 0]) + bytes([47]) + bytes(386))


class TranscriptTests(unittest.TestCase):
    def test_empty_eof(self):
        self.assertEqual(decode(success(), b''), {'tokens': [[47, 0, 0]]})

    def test_reject_extent_magic_count_and_unused(self):
        for offset, value in ((0, 0), (5, 0), (6, 1), (9, 1), (138, 1), (267, 1)):
            data = success()
            data[offset] = value
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                decode(data, b'')
        for data in (success()[:-1], success() + bytes(1)):
            with self.assertRaises(ValueError):
                decode(data, b'')

    def test_diagnostic_prefix_must_be_cleared(self):
        data = bytearray(b'OXL1' + bytes([1, 0, 1, 2]) + bytes(387))
        self.assertEqual(decode(data, b' "'), {'diagnostic': {'tag': 1, 'start': 1, 'end': 2}})
        data[8] = 1
        with self.assertRaises(ValueError):
            decode(data, b' "')

    def test_domain_and_diagnostic_opener(self):
        # This structurally valid tape passed the old decoder without the
        # explicit source-domain guard; EOF/coverage alone do not reject it.
        overlong = bytearray(b'OXL1' + bytes([0, 2, 0, 0]) + bytes(387))
        overlong[8:10] = bytes([3, 47])
        overlong[137:139] = bytes([0, 129])
        overlong[266:268] = bytes([129, 129])
        with self.assertRaises(ValueError):
            decode(overlong, b"1" * 129)
        high_bit_error = b'OXL1' + bytes([1, 0, 0, 2]) + bytes(387)
        with self.assertRaises(ValueError):
            decode(high_bit_error, b'"\xff')
        for tag in (1, 2):
            data = b'OXL1' + bytes([tag, 0, 0, 1]) + bytes(387)
            with self.assertRaises(ValueError):
                decode(data, b'x')

    def test_eof_matches_exact_source_length(self):
        with self.assertRaises(ValueError):
            decode(success(), b'x')


if __name__ == '__main__':
    unittest.main()
