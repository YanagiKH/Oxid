#!/usr/bin/env python3
"""Compare 128-bit runtime operands with Python bigints and source-free ELFs."""
import hashlib
from pathlib import Path
import types

PARENT = "verify_integer_widths.py"
PARENT_SHA = "bf92986fdc3f997104564b051bd019f398986b2e05b1d0c396d6bd0104f7b22d"
path = Path(__file__).with_name(PARENT)
body = path.read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned integer-width oracle")
# All result, diagnostic, source-removal, memory-limit and transcript checks
# remain active. The two new exact JSON integer types also use decimal strings.
before = b'TYPES[ty][0] == 64 else value'
if body.count(before) != 1:
    raise ValueError("changed wide JSON representation seam")
expanded = body.replace(before, b'TYPES[ty][0] >= 64 else value', 1)
predecessor = types.ModuleType("_integer128_oracle_parent")
predecessor.__file__ = __file__
exec(compile(expanded, str(path), "exec"), predecessor.__dict__)
predecessor.TYPES.update({"i128": (128, True), "u128": (128, False)})
predecessor.ADDED += ("i128", "u128")
parent_operations = predecessor.operations
parent_source = predecessor.source
parent_boundaries = predecessor.boundary_values


def operations(ty):
    if ty not in ("i128", "u128"):
        return ["to_i128", "to_u128"]
    return parent_operations(ty)


def boundary_values(ty):
    low, high = predecessor.bounds(ty)
    extra = {2**64 - 1, 2**64, 2**64 + 1, 2**126, 2**127 - 1,
             2**127, -(2**64), -(2**64) - 1, -(2**126)}
    return sorted(set(parent_boundaries(ty)) | {n for n in extra if low <= n <= high})


def source(ty, operation):
    text, result_type, origin = parent_source(ty, operation)
    if ty not in ("i128", "u128"):
        return text, result_type, origin
    split = text.index("fn apply(")
    parser, rest = text[:split], text[split:]
    narrow = "i64" if ty == "i128" else "u64"
    if parser.count('b"....................."') != 1:
        raise ValueError("changed integer argument capacity seam")
    parser = parser.replace('b"....................."', 'b"........................................."', 1)
    if ty == "i128":
        parser = parser.replace("narrow.to_i64()", "narrow.to_i128_checked()")
    parser = parser.replace(narrow, ty)
    text = parser + rest
    if origin is not None:
        start = origin["start"] + len(parser) - split
        origin = {"start": start, "end": start + origin["end"] - origin["start"],
                  "line": text[:start].count("\n") + 1,
                  "column": len(text[:start].rsplit("\n", 1)[-1]) + 1}
    return text, result_type, origin


predecessor.operations = operations
predecessor.source = source
predecessor.boundary_values = boundary_values

if __name__ == "__main__":
    predecessor.main()
