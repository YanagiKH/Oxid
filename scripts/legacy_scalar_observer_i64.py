"""Exact i64 exclusions for frozen legacy scalar observer wrappers.

Original wrappers, predecessor adaptation and scalar/borrow wire assertions stay intact.
Only two explicit AST refusal arms are added.
"""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-i64-compatibility-3"
PARENT = "legacy_scalar_observer_byte_strings.py"
PARENT_SHA = 'daddba5963cac5a4fb7caa376ead17c6f02d798d607839be0620fbac5ed0380d'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_i64_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': '718deadf20d43ae2cae44332e431b0da9b348dedaba22f01fe74f57811f09026',
 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/frontend_mod.rs': '92eed4fd8ebae3ac69f87ba5737c34582124b9962c9095dad88e3ecec0d9e92a',
 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'            ExprKind::ByteString { .. } => "byte_string",\n',
  b'            ExprKind::ByteString { .. } => "byte_string",\n            ExprKind::I64Number { '
  b'.. } => "i64_literal",\n'),
 (b'        | ExprKind::ByteString { .. }\n',
  b'        | ExprKind::ByteString { .. }\n        | ExprKind::I64Number { .. }\n'))


def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong i64 observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("i64 observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("i64 observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("i64 observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned i64 observer output")
    return result
