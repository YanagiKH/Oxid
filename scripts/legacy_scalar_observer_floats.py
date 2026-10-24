"""Exact floating-point exclusions for frozen legacy scalar observer wrappers.

Original wrappers, predecessor adaptation and scalar/borrow wire assertions stay intact.
Explicit floating-point literal refusal arms are added.
"""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-floating-point-compatibility-6"
PARENT = "legacy_scalar_observer_integer_bits.py"
PARENT_SHA = 'd02d9fe787bff19f75752fedf26a6c60a55224c7d58455a3fcf9a2f70ef3ba9a'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_integer_bit_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704', 'parser/frontend_mod.rs': '15c890792affab7cf43dfd8e480d77f33f52d56c7c144b2a2601989dbeea9196', 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225', 'static/frontend_mod.rs': 'be3e70cda61d46c0b30b51d5203773acf73adacd8bb234b475c44342dce9ff3f', 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'            ExprKind::IntegerNumber { .. } => "integer_literal",\n', b'            ExprKind::IntegerNumber { .. } => "integer_literal",\n            ExprKind::FloatNumber { .. } => "float_literal",\n'), (b'        | ExprKind::IntegerNumber { .. }\n', b'        | ExprKind::IntegerNumber { .. }\n        | ExprKind::FloatNumber { .. }\n'))

def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong floating-point observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("floating-point observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("floating-point observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("floating-point observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned floating-point observer output")
    return result
