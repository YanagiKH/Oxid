#!/usr/bin/env python3
"""Compare IEEE 754 result bits with exact rational arithmetic and source-free ELFs."""
from fractions import Fraction
import hashlib
from pathlib import Path
import random
import types

PARENT = "verify_integer_widths.py"
PARENT_SHA = "bf92986fdc3f997104564b051bd019f398986b2e05b1d0c396d6bd0104f7b22d"
path = Path(__file__).with_name(PARENT)
body = path.read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned native oracle controller")
# Select the new input types; preserve every execution, JSON, source-removal,
# empty-environment, raw-transcript and 32 MiB native-memory assertion.
before = b'choices=TYPES, default=list(TYPES)'
after = b'choices=("f32", "f64"), default=["f32", "f64"]'
if body.count(before) != 1 or after in body:
    raise ValueError("changed floating-point type selection seam")
expanded = body.replace(before, after, 1)
if expanded.replace(after, before, 1) != body:
    raise ValueError("changed preceding controller bytes")
predecessor = types.ModuleType("_float_oracle_controller")
predecessor.__file__ = __file__
exec(compile(expanded, str(path), "exec"), predecessor.__dict__)
parser_source = predecessor.source
predecessor.TYPES.update({"f32": (32, False), "f64": (64, False)})
ARITHMETIC = {"add": "+", "sub": "-", "mul": "*", "div": "/", "rem": "%"}
COMPARISONS = {"eq": "==", "ne": "!=", "lt": "<", "le": "<=", "gt": ">", "ge": ">="}


def parameters(width):
    return (24, 8, 127) if width == 32 else (53, 11, 1023)


def power(exponent):
    return Fraction(1 << exponent) if exponent >= 0 else Fraction(1, 1 << -exponent)


def classify(bits, width):
    precision, exponent_bits, bias = parameters(width)
    sign = bits >> (width - 1)
    exponent = (bits >> (precision - 1)) & ((1 << exponent_bits) - 1)
    mantissa = bits & ((1 << (precision - 1)) - 1)
    if exponent == (1 << exponent_bits) - 1:
        return ("nan" if mantissa else "inf"), sign, None
    significant = mantissa + ((1 << (precision - 1)) if exponent else 0)
    value = Fraction(significant) * power((exponent or 1) - bias - (precision - 1))
    return "finite", sign, -value if sign else value


def special(width, kind, sign=0):
    precision, exponent_bits, _ = parameters(width)
    if kind == "zero":
        return sign << (width - 1)
    exponent = ((1 << exponent_bits) - 1) << (precision - 1)
    return exponent | (1 << (precision - 2)) if kind == "nan" else exponent | (sign << (width - 1))


def rounded(value, width, zero_sign=0):
    if not value:
        return special(width, "zero", zero_sign)
    precision, exponent_bits, bias = parameters(width)
    sign = int(value < 0)
    value = abs(value)
    exponent = value.numerator.bit_length() - value.denominator.bit_length()
    if value < power(exponent):
        exponent -= 1
    exponent = max(exponent, 1 - bias)
    scaled = value / power(exponent - (precision - 1))
    integer, remainder = divmod(scaled.numerator, scaled.denominator)
    doubled = 2 * remainder
    if doubled > scaled.denominator or (doubled == scaled.denominator and integer & 1):
        integer += 1
    if integer >= 1 << precision:
        integer >>= 1
        exponent += 1
    if exponent > bias:
        return special(width, "inf", sign)
    if integer < 1 << (precision - 1):
        encoded_exponent, mantissa = 0, integer
    else:
        encoded_exponent, mantissa = exponent + bias, integer - (1 << (precision - 1))
    return (sign << (width - 1)) | (encoded_exponent << (precision - 1)) | mantissa


def calculate(width, operation, a, b):
    ka, sa, va = classify(a, width)
    kb, sb, vb = classify(b, width)
    if operation == "identity":
        return a
    if operation == "neg":
        return a ^ (1 << (width - 1))
    if operation == "convert":
        other = 64 if width == 32 else 32
        return special(other, ka, sa) if ka != "finite" else rounded(va, other, sa)
    if operation in COMPARISONS:
        if ka == "nan" or kb == "nan":
            return operation == "ne"
        left = (-float("inf") if sa else float("inf")) if ka == "inf" else va
        right = (-float("inf") if sb else float("inf")) if kb == "inf" else vb
        return {"eq": lambda: left == right, "ne": lambda: left != right,
                "lt": lambda: left < right, "le": lambda: left <= right,
                "gt": lambda: left > right, "ge": lambda: left >= right}[operation]()
    if ka == "nan" or kb == "nan":
        return special(width, "nan")
    if operation in ("add", "sub"):
        if operation == "sub":
            sb ^= 1
            if vb is not None:
                vb = -vb
        if ka == "inf" or kb == "inf":
            if ka == kb == "inf" and sa != sb:
                return special(width, "nan")
            return special(width, "inf", sa if ka == "inf" else sb)
        return rounded(va + vb, width, int(not va and not vb and sa and sb))
    if operation == "mul":
        if (ka == "inf" and kb == "finite" and not vb) or (kb == "inf" and ka == "finite" and not va):
            return special(width, "nan")
        if ka == "inf" or kb == "inf":
            return special(width, "inf", sa ^ sb)
        return rounded(va * vb, width, sa ^ sb)
    if operation == "div":
        if ka == kb == "inf" or ka == kb == "finite" and not va and not vb:
            return special(width, "nan")
        if ka == "inf" or kb == "finite" and not vb:
            return special(width, "inf", sa ^ sb)
        if kb == "inf":
            return special(width, "zero", sa ^ sb)
        return rounded(va / vb, width, sa ^ sb)
    if operation == "rem":
        if ka == "inf" or kb == "finite" and not vb:
            return special(width, "nan")
        if kb == "inf":
            return a
        quotient = abs(va) // abs(vb)
        if sa != sb:
            quotient = -quotient
        return rounded(va - quotient * vb, width, sa)
    raise ValueError(operation)


def boundary_values(ty):
    width = int(ty[1:])
    precision, _, bias = parameters(width)
    base = {0, 1, 2, (1 << (precision - 1)) - 1, 1 << (precision - 1),
            bias << (precision - 1), (bias << (precision - 1)) + 1,
            ((bias + 1) << (precision - 1)) - 1, special(width, "inf") - 1,
            special(width, "inf"), special(width, "nan"), special(width, "nan") + 1,
            special(width, "inf") + 1}
    return sorted(base | {n | (1 << (width - 1)) for n in base})


def operations(_ty):
    return ["identity", "neg", "convert"] + list(ARITHMETIC) + list(COMPARISONS)


def cases(ty, operation):
    width = int(ty[1:])
    values = boundary_values(ty)
    rng = random.Random(0xF03264 + width)
    if operation in ("identity", "neg", "convert"):
        return [(n, 0) for n in sorted(set(values) | {rng.getrandbits(width) for _ in range(64)})]
    special_values = {0, 1, 1 << (width - 1), special(width, "inf"), special(width, "nan"),
                      (127 if width == 32 else 1023) << (23 if width == 32 else 52)}
    pairs = {(a, b) for a in values for b in special_values}
    pairs |= {(b, a) for a, b in pairs.copy()}
    pairs |= {(rng.getrandbits(width), rng.getrandbits(width)) for _ in range(48)}
    return sorted(pairs)


def expected(ty, operation, a, b):
    return calculate(int(ty[1:]), operation, a, b), None


def source(ty, operation):
    text, _, _ = parser_source("u64", "identity")
    parser = text[:text.index("fn apply(")]
    width = int(ty[1:])
    bits = "u32" if width == 32 else "u64"
    target = "f64" if width == 32 else "f32"
    result = ("u64" if width == 32 else "u32") if operation == "convert" else bits
    if operation in COMPARISONS:
        expression, result = "return a" + COMPARISONS[operation] + "b;", "bool"
    else:
        expression = "a" + ARITHMETIC[operation] + "b" if operation in ARITHMETIC else "-a" if operation == "neg" else "a"
        expression = f"a.to_{target}()" if operation == "convert" else expression
        expression = f"let value={expression};return value.to_bits();"
    convert = ".to_u32_checked()" if width == 32 else ""
    text = parser + f"fn apply(a:{ty},b:{ty})->{result}{{{expression}}}\nfn main()->{result}{{let input_a=number(1);let input_b=number(2);let bits_a=input_a{convert};let bits_b=input_b{convert};let a={ty}::from_bits(bits_a);let b={ty}::from_bits(bits_b);return apply(a,b);}}\n"
    return text, result, None


predecessor.operations = operations
predecessor.boundary_values = boundary_values
predecessor.cases = cases
predecessor.expected = expected
predecessor.source = source

if __name__ == "__main__":
    predecessor.main()
