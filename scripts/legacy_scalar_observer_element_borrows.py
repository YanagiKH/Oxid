"""Preserve scalar observer output when the parser accepts element borrows."""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-element-borrows-13"
PARENT = "legacy_scalar_observer_vectors.py"
PARENT_PIN = (3378, "bee8aace1729a9ab181acd680de63d9bd4873b1c63e54e25dc4c91a4a2e9ef05")

parent_path = Path(__file__).with_name(PARENT)
body = parent_path.read_bytes()
if (len(body), hashlib.sha256(body).hexdigest()) != PARENT_PIN:
    raise ValueError("changed pinned scalar observer adapter")
predecessor = types.ModuleType("_element_borrow_observer_predecessor")
predecessor.__file__ = str(parent_path)
exec(compile(body, str(parent_path), "exec"), predecessor.__dict__)

DERIVED_PINS = {
    'parser/frontend_mod.rs': 'fd014e20a4583c50a25d915f30fe5323ae8e29ada3f485a8896e8d977c63fc64',
    'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
    'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
    'static/frontend_mod.rs': 'bd125c5231cbc6fc25ba94cd7b0d10e5ccff51e0117a5908189ab6b13e80b479',
    'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
    'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
    'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf',
}
EDITS = (
    (b'            ExprKind::Borrow { .. } => "local_reference",\n',
     b'            ExprKind::Borrow { .. } | ExprKind::BorrowElement { .. } => "local_reference",\n'),
    (b'        | ExprKind::Borrow { .. }\n',
     b'        | ExprKind::Borrow { .. }\n        | ExprKind::BorrowElement { .. }\n'),
)


def reverse_wrapper(kind, name, derived):
    key = kind + "/" + name
    if hashlib.sha256(derived).hexdigest() != DERIVED_PINS.get(key):
        raise ValueError("wrong scalar observer output")
    restored = derived
    if name == "frontend_mod.rs":
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("scalar observer reversal drift")
            restored = restored.replace(after, before, 1)
    if hashlib.sha256(restored).hexdigest() != predecessor.DERIVED_PINS[key]:
        raise ValueError("scalar observer changed predecessor bytes")
    return restored


def adapt_wrapper(kind, name, original):
    inherited = predecessor.adapt_wrapper(kind, name, original)
    result = inherited
    if name == "frontend_mod.rs":
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("scalar observer element-borrow boundary drift")
            result = result.replace(before, after, 1)
    if reverse_wrapper(kind, name, result) != inherited:
        raise ValueError("scalar observer changed predecessor bytes")
    return result
