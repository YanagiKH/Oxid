#!/usr/bin/env python3
"""Compare integer bit patterns with Python bigints and source-free binaries."""
import hashlib
from pathlib import Path
import random
import types

PARENT = "verify_integer128.py"
PARENT_SHA = "48536a6dea1d0076a7fb123b0e7195aab44e597c439ea419ec70f1f82c720196"
path = Path(__file__).with_name(PARENT)
body = path.read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned 128-bit oracle")
parent = types.ModuleType("_integer_bits_oracle_parent")
parent.__file__ = __file__
exec(compile(body, str(path), "exec"), parent.__dict__)
predecessor = parent.predecessor
OPERATIONS = {"and": "&", "or": "|", "xor": "^", "shl": "<<", "shr": ">>", "not": "~"}

NARROW_INPUT = """fn shift_amount(argument:i32)->i32{
    let mut bytes=b"............";
    let status=arg_bytes(argument,&mut bytes);
    let mut length=0;
    match status{
        ArgStatus::Copied(count)=>{length=count;},
        ArgStatus::TooSmall(required)=>{return 0;},
        ArgStatus::Missing=>{return 0;},
        ArgStatus::TooLong=>{return 0;},
    }
    let mut index=0;
    let first=bytes[0];
    let negative=first.to_i32()==45;
    if negative{index=1;}
    let mut value=0;
    while index<length{
        let byte=bytes[index];
        let digit=byte.to_i32()-48;
        if negative{value=value*10-digit;}else{value=value*10+digit;}
        index=index+1;
    }
    return value;
}
"""


def operations(_ty):
    return list(OPERATIONS)


def normalized(ty, value):
    width, signed = predecessor.TYPES[ty]
    value &= (1 << width) - 1
    return value - (1 << width) if signed and value & (1 << (width - 1)) else value


def cases(ty, operation):
    width, _ = predecessor.TYPES[ty]
    low, high = predecessor.bounds(ty)
    patterns = [int("55" * (width // 8), 16), int("aa" * (width // 8), 16), 1 << (width - 1)]
    values = sorted(set(predecessor.boundary_values(ty)) | {normalized(ty, n) for n in patterns})
    if operation == "not":
        return [(a, 0) for a in values]
    if operation in ("shl", "shr"):
        return [(a, b) for a in values for b in sorted({-1, 0, 1, width // 2, width - 1, width, width + 1, 2**31 - 1})]
    special = {low, high, 0, 1}
    if low < 0:
        special.add(-1)
    pairs = {(a, b) for a in values for b in special}
    pairs |= {(b, a) for a, b in pairs.copy()}
    rng = random.Random(0xB175 + list(predecessor.TYPES).index(ty))
    pairs |= {(rng.randint(low, high), rng.randint(low, high)) for _ in range(16)}
    return sorted(pairs)


def expected(ty, operation, a, b):
    width, _ = predecessor.TYPES[ty]
    if operation in ("shl", "shr") and not 0 <= b < width:
        return None, "E0611"
    value = {"and": lambda: a & b, "or": lambda: a | b, "xor": lambda: a ^ b,
             "shl": lambda: a << b, "shr": lambda: a >> b, "not": lambda: ~a}[operation]()
    return normalized(ty, value), None


def source(ty, operation):
    text, _, _ = parent.source(ty, "identity")
    parser = text[:text.index("fn apply(")]
    narrow = ty in ("i32", "u8")
    if narrow:
        parser = "use std::env::arg_bytes;\nuse std::env::ArgStatus;\n" + NARROW_INPUT.replace("fn shift_amount(", "fn number(")
        number = "i32"
    else:
        number = "i128" if ty == "i128" else "u128" if ty == "u128" else "i64" if predecessor.TYPES[ty][1] else "u64"
    shift = operation in ("shl", "shr")
    if shift:
        parser += NARROW_INPUT
    rhs = "i32" if shift else ty
    expression = "~a" if operation == "not" else "a" + OPERATIONS[operation] + "b"
    prefix = parser + f"fn apply(a:{ty},b:{rhs})->{ty}{{\n    return "
    convert = "" if number == ty else f".to_{ty}_checked()"
    parse_b = "shift_amount(2)" if shift else "number(2)"
    convert_b = "" if shift else convert
    text = prefix + expression + f";\n}}\nfn main()->{ty}{{let parsed_a=number(1);let a=parsed_a{convert};let parsed_b={parse_b};let b=parsed_b{convert_b};return apply(a,b);}}\n"
    marker = OPERATIONS[operation]
    start = len(prefix.encode()) + expression.index(marker)
    origin = {"start": start, "end": start + len(marker), "line": text[:start].count("\n") + 1,
              "column": len(text[:start].rsplit("\n", 1)[-1]) + 1}
    return text, ty, origin


predecessor.operations = operations
predecessor.cases = cases
predecessor.expected = expected
predecessor.source = source

if __name__ == "__main__":
    predecessor.main()
