"""Correctly rounded binary square roots using integer arithmetic only."""
from math import isqrt


def sqrt_bits(bits, width):
    if width not in (32, 64) or type(bits) is not int or not 0 <= bits < 1 << width:
        raise ValueError("invalid IEEE bit pattern")
    fraction, exponent_bits, bias = (23, 8, 127) if width == 32 else (52, 11, 1023)
    mask = (1 << exponent_bits) - 1
    exponent = (bits >> fraction) & mask
    mantissa = bits & ((1 << fraction) - 1)
    canonical_nan = (mask << fraction) | (1 << (fraction - 1))
    # Both signed zeros survive; every NaN result has one canonical payload.
    if exponent == 0 and mantissa == 0:
        return bits
    if bits >> (width - 1) or exponent == mask and mantissa:
        return canonical_nan
    if exponent == mask:
        return bits
    significant = mantissa + ((1 << fraction) if exponent else 0)
    power = (exponent or 1) - bias - fraction
    output_power = (significant.bit_length() - 1 + power) // 2
    # Even the smallest input has a normal square root. Scale its significand
    # to an integer radicand, then compare the exact squared rounding midpoint.
    radicand = significant << (power - 2 * output_power + 2 * fraction)
    rounded = isqrt(radicand)
    midpoint = (2 * rounded + 1) ** 2
    if 4 * radicand > midpoint or 4 * radicand == midpoint and rounded & 1:
        rounded += 1
    if rounded == 1 << (fraction + 1):
        rounded >>= 1
        output_power += 1
    return ((output_power + bias) << fraction) | (rounded - (1 << fraction))
