"""Exact record-pattern exclusions for the retained scalar observer wrappers."""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-record-pattern-compatibility-9"
PARENT = "legacy_scalar_observer_nominals.py"
PARENT_SHA = 'ef94cde94d88bc8e8a0646c5a1788d0630f45df9b09a7f15050511893b87fce6'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned legacy scalar observer adapter")
predecessor = types.ModuleType("_record_pattern_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': '84da9da0f8cd303e6ffe9098857624374bc464ccd7236bceeb97f7ab3ea4de11',
 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704',
 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/frontend_mod.rs': '3e996b2eb9d56e5b6ae6499d513fe76f0f53ad576ae13f7a573fa9611d56e3fb',
 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225',
 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b',
 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'                    StmtKind::Match { .. } => return Some(("match", statement.span)),\n', b'                    StmtKind::Match { .. } => return Some(("match", statement.span)),\n                    StmtKind::DestructureRecord { .. } => return Some(("record_pattern", statement.span)),\n'), (b'        StmtKind::FieldAssign { .. } | StmtKind::IndexAssign { .. } | StmtKind::Match { .. } => {\n', b'        StmtKind::DestructureRecord { .. } | StmtKind::FieldAssign { .. } | StmtKind::IndexAssign { .. } | StmtKind::Match { .. } => {\n'))


def adapt_wrapper(kind, name, original):
    derived = predecessor.adapt_wrapper(kind, name, original)
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 2:
            raise ValueError("wrong record pattern observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("record pattern observer seam drift")
            result = result.replace(before, after, 1)
        restored = result
        for before, after in reversed(EDITS):
            if restored.count(after) != 1:
                raise ValueError("record pattern observer reverse seam drift")
            restored = restored.replace(after, before, 1)
        if restored != derived:
            raise ValueError("record pattern observer changed preceding projection")
    if hashlib.sha256(result).hexdigest() != DERIVED_PINS[kind + "/" + name]:
        raise ValueError("wrong pinned record pattern observer output")
    return result
