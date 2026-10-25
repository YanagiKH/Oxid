import decimal
import random
import struct
import unittest

from float_root_oracle import sqrt_bits


class FloatRootOracleTests(unittest.TestCase):
    def test_known_rounded_values_and_extreme_exponents(self):
        cases = {
            32: [(0, 0), (0x80000000, 0x80000000), (0x3f800000, 0x3f800000),
                 (0x40000000, 0x3fb504f3), (0x40800000, 0x40000000),
                 (0x00800000, 0x20000000), (0x7f800000, 0x7f800000)],
            64: [(0, 0), (0x8000000000000000, 0x8000000000000000),
                 (0x3ff0000000000000, 0x3ff0000000000000),
                 (0x4000000000000000, 0x3ff6a09e667f3bcd),
                 (0x4010000000000000, 0x4000000000000000),
                 (1, 0x1e60000000000000),
                 (0x0010000000000000, 0x2000000000000000),
                 (0x7ff0000000000000, 0x7ff0000000000000)]}
        for width, rows in cases.items():
            for source, expected in rows:
                with self.subTest(width=width, source=source):
                    self.assertEqual(sqrt_bits(source, width), expected)

    def test_negative_finite_infinity_and_nan_payloads_are_canonical(self):
        for width, fraction, exponent in [(32, 23, 255), (64, 52, 2047)]:
            canonical = exponent << fraction | 1 << (fraction - 1)
            for bits in [(1 << (width - 1)) | 1, (1 << width) - 1,
                         exponent << fraction | 1, exponent << fraction | 17,
                         (1 << (width - 1)) | exponent << fraction]:
                self.assertEqual(sqrt_bits(bits, width), canonical)

    def test_integer_oracle_agrees_with_high_precision_decimal_rounding(self):
        rng = random.Random(72981)
        with decimal.localcontext() as context:
            context.prec = 200
            for width, packing, exponent, fraction in [(32, 'f', 255, 23), (64, 'd', 2047, 52)]:
                patterns = [1, (1 << fraction) - 1, 1 << fraction, (exponent << fraction) - 1]
                patterns += [rng.randrange(exponent << fraction) for _ in range(512)]
                for bits in patterns:
                    value = struct.unpack('>' + packing, bits.to_bytes(width // 8, 'big'))[0]
                    root = decimal.Decimal.from_float(value).sqrt()
                    expected = int.from_bytes(struct.pack('>' + packing, float(root)), 'big')
                    self.assertEqual(sqrt_bits(bits, width), expected, (width, bits))

    def test_invalid_inputs_fail_before_any_shift_or_rounding(self):
        for bits, width in [(-1, 32), (1 << 32, 32), (1 << 64, 64), (0, 16), (True, 32), (1.0, 64)]:
            with self.assertRaises(ValueError):
                sqrt_bits(bits, width)


if __name__ == '__main__':
    unittest.main()
