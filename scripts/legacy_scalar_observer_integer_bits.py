"""Exact integer-bit exclusions for frozen legacy scalar observer wrappers.

Original wrappers, predecessor adaptation and scalar/borrow wire assertions stay intact.
Explicit bitwise AST refusal arms are added.
"""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-integer-bits-compatibility-5"
PARENT = "legacy_scalar_observer_integer_widths.py"
PARENT_SHA = '86cb59b21f31f3f25553175ead429d35c888732d8d19c73c4d6916f4fb4e32ad'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_integer_width_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': '82a2dc5c107663f5ccf687e8a152546587a29aeb40db44e368df58d06c69dcdf',
 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/frontend_mod.rs': '63d42f483ae51bcbd1859112c14d755a3bde3f8fdef2d7fa698483a8d30feddc',
 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'            ExprKind::I64Number { .. } => "i64_literal",\n            ExprKind::IntegerNumber { .. } => "integer_literal",\n', b'            ExprKind::I64Number { .. } => "i64_literal",\n            ExprKind::IntegerNumber { .. } => "integer_literal",\n            ExprKind::Bitwise { .. } => "integer_bits",\n            ExprKind::BitNot { .. } => "integer_bits",\n'), (b'        | ExprKind::I64Number { .. }\n        | ExprKind::IntegerNumber { .. }\n', b'        | ExprKind::I64Number { .. }\n        | ExprKind::IntegerNumber { .. }\n        | ExprKind::Bitwise { .. }\n        | ExprKind::BitNot { .. }\n'))

def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong integer-bit observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("integer-bit observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("integer-bit observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("integer-bit observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned integer-bit observer output")
    return result
