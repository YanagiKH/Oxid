"""Exact byte-string exclusions for frozen legacy scalar observer wrappers.

Original wrappers, u8 adaptation and scalar/borrow wire assertions stay intact.
Only byte-string module wiring and two explicit AST refusal arms are added.
"""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-byte-string-compatibility-2"
PARENT = "legacy_scalar_observer_u8.py"
PARENT_SHA = '89c1fe17a7caedfd1c0130518089fe03ddd61ac977e26c81f1129308edab2768'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_byte_string_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': 'e4675c5835038cd54141daa6b483d5b992b4d16ed94ff05a19e7dfdac2068b12', 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704', 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/frontend_mod.rs': 'c96d1ed964a14399a81e1944264d14b3f84ca7c8a2c32d59125603f38dbd4938', 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225', 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'mod ast;\n', b'mod ast;\nmod byte_strings;\n'), (b'            ExprKind::Conversion { .. } => "conversion",\n', b'            ExprKind::Conversion { .. } => "conversion",\n            ExprKind::ByteString { .. } => "byte_string",\n'), (b'        | ExprKind::Conversion { .. }\n', b'        | ExprKind::Conversion { .. }\n        | ExprKind::ByteString { .. }\n'))


def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 3:
            raise ValueError("wrong byte-string observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("byte-string observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("byte-string observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("byte-string observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned byte-string observer output")
    return result
