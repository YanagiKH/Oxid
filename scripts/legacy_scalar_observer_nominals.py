"""Exact nominal-type exclusions for the retained scalar observer wrappers."""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-nominal-compatibility-8"
PARENT = "legacy_scalar_observer_characters.py"
PARENT_SHA = 'e135dc57d2a98022dcdedf85ec3b6170874b3f858958d16d23524eb9e8298711'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_nominal_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': '7422542884c0a29578df1e95190feda7696e6b3f518452d302c25ed395f4e48b',
 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/frontend_mod.rs': 'd06c434d00b8dba7e5b8e81f35a95e2156d8fdd1204ce1e9545e0cb7a7acca72',
 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'        TypeSyntaxKind::ArrayReference { .. } => "array_reference_type",\n', b'        TypeSyntaxKind::ArrayReference { .. } => "array_reference_type",\n        TypeSyntaxKind::Applied(_) | TypeSyntaxKind::AppliedReference { .. } => "nominal_type",\n'), (b'            ExprKind::QualifiedValue { .. } => "qualified_value",\n', b'            ExprKind::QualifiedValue { .. } => "qualified_value",\n            ExprKind::GenericVariant { .. } => "generic_variant",\n'), (b'        | ExprKind::QualifiedValue { .. }\n', b'        | ExprKind::QualifiedValue { .. }\n        | ExprKind::GenericVariant { .. }\n'))


def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 3:
            raise ValueError("wrong nominal observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("nominal observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("nominal observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("nominal observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned nominal observer output")
    return result
