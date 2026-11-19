#!/usr/bin/env python3
"""Check byte arithmetic and results against Python bigints and native ELFs.

The complete integer-width oracle is authenticated before loading. Its checks,
limits and raw transcript stay unchanged. Byte operands use an i32 argument
parser so reference execution also exercises the eight-byte scalar cells.
"""
import hashlib
from pathlib import Path
import sys
import types

PARENT = "verify_integer_widths.py"
PARENT_SHA = "bf92986fdc3f997104564b051bd019f398986b2e05b1d0c396d6bd0104f7b22d"
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned integer-width oracle")
predecessor = types.ModuleType("_byte_integer_oracle_parent")
# The receipt must identify this controller, including its parent pin and the
# narrow parser, while tracebacks still identify the original library source.
predecessor.__file__ = __file__
exec(compile(body, str(Path(__file__).with_name(PARENT)), "exec"), predecessor.__dict__)
predecessor.ADDED += ("u8",)
parent_operations = predecessor.operations
parent_source = predecessor.source

NARROW_INPUT = """use std::env::arg_bytes;
use std::env::ArgStatus;
fn number(argument:i32)->i32{
    let mut bytes=b"...";
    let status=arg_bytes(argument,&mut bytes);
    let mut length=0;
    match status{
        ArgStatus::Copied(count)=>{length=count;},
        ArgStatus::TooSmall(required)=>{return 0;},
        ArgStatus::Missing=>{return 0;},
        ArgStatus::TooLong=>{return 0;},
    }
    let mut index=0;
    let mut value=0;
    while index<length{
        let byte=bytes[index];
        let digit=byte.to_i32();
        value=value*10+digit-48;
        index=index+1;
    }
    return value;
}
"""


def operations(ty):
    result = parent_operations(ty)
    return result + ["to_u8"] if ty == "u8" else result


def source(ty, operation):
    text, result_type, origin = parent_source(ty, operation)
    if ty != "u8":
        return text, result_type, origin
    old_prefix = text.index("fn apply(")
    text = NARROW_INPUT + text[old_prefix:]
    if operation == "identity":
        before = "fn apply(a:u8,b:u8)->i32{\n    return a.to_i32();"
        after = "fn apply(a:u8,b:u8)->u8{\n    return a;"
        if text.count(before) != 1 or text.count("fn main()->i32{") != 1:
            raise ValueError("byte result source seam changed")
        text = text.replace(before, after, 1).replace("fn main()->i32{", "fn main()->u8{", 1)
        result_type = "u8"
    if origin is not None:
        start = origin["start"] + len(NARROW_INPUT) - old_prefix
        origin = {"start": start, "end": start + origin["end"] - origin["start"],
                  "line": text[:start].count("\n") + 1,
                  "column": len(text[:start].rsplit("\n", 1)[-1]) + 1}
    return text, result_type, origin


predecessor.operations = operations
predecessor.source = source

if __name__ == "__main__":
    if not any(arg == "--types" or arg.startswith("--types=") for arg in sys.argv[1:]):
        sys.argv += ["--types", "u8"]
    predecessor.main()
