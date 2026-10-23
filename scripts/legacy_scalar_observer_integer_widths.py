"""Exact integer-width exclusions for frozen legacy scalar observer wrappers.

Original wrappers, predecessor adaptation and scalar/borrow wire assertions stay intact.
Only two explicit AST refusal arms are added.
"""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-integer-widths-compatibility-4"
PARENT = "legacy_scalar_observer_i64.py"
PARENT_SHA = '865f80724ef3f3ecbc8cf609e299c3223554ee777b2691c32d5bdd4f38d6410b'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_integer_width_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': 'f219fa92e0a12059dad8ab832617e8ce3010feae9142614654e490a5d21bce11',
 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/frontend_mod.rs': '7407b623dc6ed36a3268fff02f7f2438e3cd2fafc2b88d348e91a4d7f38137a0',
 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'            ExprKind::I64Number { .. } => "i64_literal",\n',
  b'            ExprKind::I64Number { .. } => "i64_literal",\n            ExprKind::IntegerNumber'
  b' { .. } => "integer_literal",\n'),
 (b'        | ExprKind::I64Number { .. }\n',
  b'        | ExprKind::I64Number { .. }\n        | ExprKind::IntegerNumber { .. }\n'))


def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong integer-width observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("integer-width observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("integer-width observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("integer-width observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned integer-width observer output")
    return result
