"""Exact character exclusions for frozen legacy scalar observer wrappers.

Original wrappers, predecessor adaptation and scalar/borrow wire assertions stay intact.
Explicit character literal refusal arms are added.
"""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-character-compatibility-7"
PARENT = "legacy_scalar_observer_floats.py"
PARENT_SHA = '20b8572110b795c5b19169ca10a999ec739c3d4925034ab0677254c17a231ce8'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_integer_bit_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704', 'parser/frontend_mod.rs': '98cf20e87da8a1bd1e5259daefe471d2c9492de545779b3b654c9748e0fce4aa', 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225', 'static/frontend_mod.rs': '9176ae028767d5773382b77557193ac6464748caf7d053de65d71ed8d6e6f5d9', 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'            ExprKind::FloatNumber { .. } => "float_literal",\n', b'            ExprKind::FloatNumber { .. } => "float_literal",\n            ExprKind::Character(_) => "character_literal",\n'), (b'        | ExprKind::FloatNumber { .. }\n', b'        | ExprKind::FloatNumber { .. }\n        | ExprKind::Character(_)\n'))

def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong character observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("character observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("character observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("character observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned character observer output")
    return result
