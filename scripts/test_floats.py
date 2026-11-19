import math
import random
import struct
import unittest
from fractions import Fraction
import verify_floats as oracle


class FloatOracleTests(unittest.TestCase):
    def test_exact_values_round_trip_without_a_host_float(self):
        randomizer = random.Random(192831)
        for width in (32, 64):
            for bits in [randomizer.getrandbits(width) for _ in range(2000)]:
                kind, sign, value = oracle.classify(bits, width)
                if kind == "finite":
                    self.assertEqual(oracle.rounded(value, width, sign), bits)

    def test_ties_round_to_even_at_normal_and_subnormal_boundaries(self):
        self.assertEqual(oracle.rounded(Fraction(1) + Fraction(1, 2**24), 32), 0x3f800000)
        self.assertEqual(oracle.rounded(Fraction(1) + Fraction(3, 2**24), 32), 0x3f800002)
        self.assertEqual(oracle.rounded(Fraction(1, 2**150), 32), 0)
        self.assertEqual(oracle.rounded(Fraction(3, 2**150), 32), 2)
        self.assertEqual(oracle.rounded(-Fraction(1, 2**150), 32), 0x80000000)
        self.assertEqual(oracle.rounded(Fraction(1) + Fraction(1, 2**53), 64), 0x3ff0000000000000)
        self.assertEqual(oracle.rounded(Fraction(1, 2**1075), 64), 0)

    def test_finite_overflow_threshold_is_rounded_before_infinity(self):
        maximum = Fraction(2**128 - 2**104)
        self.assertEqual(oracle.rounded(maximum + 2**103 - 1, 32), 0x7f7fffff)
        self.assertEqual(oracle.rounded(maximum + 2**103, 32), 0x7f800000)

    def test_nan_rules_and_zero_signs_are_explicit(self):
        for width in (32, 64):
            zero = 0
            negative = 1 << (width - 1)
            infinity = oracle.special(width, "inf")
            nan = oracle.special(width, "nan")
            self.assertEqual(oracle.calculate(width, "div", zero, zero), nan)
            self.assertEqual(oracle.calculate(width, "sub", infinity, infinity), nan)
            self.assertEqual(oracle.calculate(width, "mul", infinity, zero), nan)
            self.assertEqual(oracle.calculate(width, "add", negative, negative), negative)
            self.assertEqual(oracle.calculate(width, "add", zero, negative), zero)
            self.assertEqual(oracle.calculate(width, "sub", negative, zero), negative)
            self.assertTrue(oracle.calculate(width, "ne", nan, nan))
            for operation in ("eq", "lt", "le", "gt", "ge"):
                self.assertFalse(oracle.calculate(width, operation, nan, infinity))

    def test_f64_arithmetic_matches_standard_binary64_for_finite_samples(self):
        rng = random.Random(9187)
        for _ in range(1000):
            values = [rng.uniform(-1e100, 1e100) for _ in range(2)]
            bits = [int.from_bytes(struct.pack('<d', v), 'little') for v in values]
            for name, expected in [('add', values[0]+values[1]), ('sub', values[0]-values[1]),
                                   ('mul', values[0]*values[1]), ('div', values[0]/values[1]),
                                   ('rem', math.fmod(*values))]:
                self.assertEqual(oracle.calculate(64, name, *bits), int.from_bytes(struct.pack('<d', expected), 'little'))


if __name__ == '__main__':
    unittest.main()
