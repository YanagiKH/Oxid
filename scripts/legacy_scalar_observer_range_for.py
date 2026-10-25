"""Exact range-loop exclusions for the retained scalar observer wrappers."""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-range-for-compatibility-10"
PARENT = "legacy_scalar_observer_record_patterns.py"
PARENT_SHA = 'cdbc92d516f1e08d4dc9283d20fd831a70f5865b21105b1916113d6216c18297'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_range_for_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': '2f86c673ba7de2464632a1665f4228ca9f1e4af6010d3aef30542d2c6bbc11c5',
 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/frontend_mod.rs': '75506565e1f60cdbb755b245ac002d60dbf831b14962e09b09a85ec933cebf84',
 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'                    StmtKind::DestructureRecord { .. } => return Some(("record_pattern", statement.span)),\n', b'                    StmtKind::DestructureRecord { .. } => return Some(("record_pattern", statement.span)),\n                    StmtKind::ForRange { .. } => return Some(("for_range", statement.span)),\n'), (b'        StmtKind::DestructureRecord { .. } | StmtKind::FieldAssign { .. } | StmtKind::IndexAssign { .. } | StmtKind::Match { .. } => {\n', b'        StmtKind::ForRange { .. } | StmtKind::DestructureRecord { .. } | StmtKind::FieldAssign { .. } | StmtKind::IndexAssign { .. } | StmtKind::Match { .. } => {\n'))


def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong range loop observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("range loop observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("range loop observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("range loop observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned range loop observer output")
    return result
