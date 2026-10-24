#!/usr/bin/env python3
"""Check Unicode scalar conversions and comparisons in isolated native programs."""
import hashlib
from pathlib import Path
import random
import types

PARENT = "verify_integer128.py"
PARENT_SHA = "48536a6dea1d0076a7fb123b0e7195aab44e597c439ea419ec70f1f82c720196"
path = Path(__file__).with_name(PARENT)
body = path.read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned integer transport and verification controller")
parent = types.ModuleType("_character_integer_transport")
parent.__file__ = __file__
exec(compile(body, str(path), "exec"), parent.__dict__)
oracle = parent.predecessor
COMPARISONS = {"eq": "==", "ne": "!=", "lt": "<", "le": "<=", "gt": ">", "ge": ">="}


def scalar(value):
    return 0 <= value <= 0x10ffff and not 0xd800 <= value <= 0xdfff


def operations(_ty):
    return ["to_char", "roundtrip", *COMPARISONS]


def boundary_values(ty):
    low, high = oracle.bounds(ty)
    values = {low, low + 1, high - 1, high, -1, 0, 1, 9, 10, 13, 34, 39, 92,
              0x7f, 0x80, 0x7ff, 0x800, 0xd7ff, 0xd800, 0xdbff, 0xdc00,
              0xdfff, 0xe000, 0xffff, 0x10000, 0x1f980, 0x10ffff, 0x110000,
              2**32 + 65, 2**64 + 65, 2**127 + 65}
    return sorted(n for n in values if low <= n <= high)


def cases(ty, operation):
    low, high = oracle.bounds(ty)
    rng = random.Random(0xC4A2 + list(oracle.TYPES).index(ty))
    values = set(boundary_values(ty)) | {rng.randint(low, high) for _ in range(16)}
    values |= {n for n in (rng.randrange(0x110000) for _ in range(32)) if low <= n <= high}
    if operation in ("to_char", "roundtrip"):
        return [(n, 0) for n in sorted(values)]
    # Only the first conversion can fail. Its exact origin is independently
    # checked by the inherited text and JSON diagnostic assertions.
    maximum = min(high, 0x10ffff)
    if 0xd800 <= maximum <= 0xdfff:
        maximum = 0xd7ff
    return sorted({(a, b) for a in values for b in ({0, maximum, a} if scalar(a) else {0})})


def expected(_ty, operation, a, b):
    if not scalar(a):
        return None, "E0612"
    if operation == "to_char":
        return chr(a), None
    if operation == "roundtrip":
        return a, None
    if not scalar(b):
        raise ValueError("comparison transport supplied an invalid right scalar")
    return {"eq": a == b, "ne": a != b, "lt": a < b,
            "le": a <= b, "gt": a > b, "ge": a >= b}[operation], None


def source(ty, operation):
    original, original_result, _ = parent.source(ty, "identity")
    start = original.index("fn apply(")
    main_start = original.index("fn main(", start)
    result = "char" if operation == "to_char" else "u32" if operation == "roundtrip" else "bool"
    statement = "return c;" if operation == "to_char" else "return c.to_u32();" if operation == "roundtrip" else f"let d=b.to_char_checked();return c{COMPARISONS[operation]}d;"
    apply = f"fn apply(a:{ty},b:{ty})->{result}{{\n    let c=a.to_char_checked();\n    {statement}\n}}\n"
    tail = original[main_start:]
    before, after = f"fn main()->{original_result}", f"fn main()->{result}"
    if tail.count(before) != 1:
        raise ValueError("changed integer argument transport result seam")
    text = original[:start] + apply + tail.replace(before, after, 1)
    location = text.index("to_char_checked", start)
    origin = {"start": location, "end": location + len("to_char_checked"),
              "line": text[:location].count("\n") + 1,
              "column": len(text[:location].rsplit("\n", 1)[-1]) + 1}
    return text, result, origin


oracle.operations = operations
oracle.boundary_values = boundary_values
oracle.cases = cases
oracle.expected = expected
oracle.source = source

if __name__ == "__main__":
    oracle.main()
