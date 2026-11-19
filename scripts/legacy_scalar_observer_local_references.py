"""Exact local-reference exclusions for the retained scalar observer wrappers."""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-local-reference-compatibility-11"
PARENT = "legacy_scalar_observer_range_for.py"
PARENT_SHA = '750743bf011b7311d857581ad02b4fa6e13a01574e937d40425207a8d765a8fd'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_local_reference_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': '7a09e24a516f28b024fdb318c10730d6858b90ee7184184098fc4f43f607a4f7',
 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/frontend_mod.rs': '2d4db58f37c9bfc9ccc264fb14fa7a48504148b496df408f786509d4ec401fbe',
 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'            ExprKind::Conversion { .. } => "conversion",\n', b'            ExprKind::Borrow { .. } => "local_reference",\n            ExprKind::Conversion { .. } => "conversion",\n'), (b'        | ExprKind::Conversion { .. }\n', b'        | ExprKind::Borrow { .. }\n        | ExprKind::Conversion { .. }\n'))

def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong local-reference observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("local-reference observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("local-reference observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("local-reference observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned local-reference observer output")
    return result
