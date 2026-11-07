"""Explicit character exclusions preserve every preceding narrow observation."""
import hashlib
from pathlib import Path
import types
PARENT = "float_compatibility_observer.py"
PARENT_SHA = '9dcc12063886d4ced2906cab239d147c4554266c7a6bed28659a11a3a49ec101'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned integer-width observer adapter")
predecessor = types.ModuleType("_float_observer_parent")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
VERSION = "unit2-character-exclusion-observer-v10"
for _name in ("OBSERVER", "CONTROL_FILTER", "CONTROL_NAMES", "FILE_CONTROL_FILTER", "FILE_CONTROL_NAMES", "INTEGER_CONTROL_FILTER", "INTEGER_CONTROL_NAMES", "WIDTH_CONTROL_FILTER", "WIDTH_CONTROL_NAMES"):
    globals()[_name] = getattr(predecessor, _name)
QUAD_CONTROL_FILTER = predecessor.QUAD_CONTROL_FILTER
QUAD_CONTROL_NAMES = predecessor.QUAD_CONTROL_NAMES
FLOAT_CONTROL_FILTER = predecessor.FLOAT_CONTROL_FILTER
FLOAT_CONTROL_NAMES = predecessor.FLOAT_CONTROL_NAMES
CHARACTER_CONTROL_FILTER = 'current_unit2_character_adapter_'
CHARACTER_CONTROL_NAMES = ('frontend::oir::unit2_observer::current_unit2_character_adapter_denies_scalar_char', 'frontend::oir::unit2_observer::current_unit2_character_adapter_denies_parameter_char')
SEAMS = ((b'        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),\n', b'        ValueTy::Scalar(hir::Ty::Char) => panic!("current Unit2 observer excludes character projection"),\n        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),\n'),)
CONTROL_TESTS = b'\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes character projection")]\nfn current_unit2_character_adapter_denies_scalar_char() {\n    value_type(ValueTy::Scalar(hir::Ty::Char));\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes character projection")]\nfn current_unit2_character_adapter_denies_parameter_char() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Char)));\n}\n'
DERIVED_BYTES = 25912
DERIVED_SHA = '05776a5b0712d0f84b1604533458e8442afc5dbbb76b19f6bee406adf2642823'

def adapt(original):
    inherited = predecessor.adapt(original)
    result = inherited
    if len(SEAMS) != 1 or CONTROL_TESTS in result:
        raise ValueError("wrong character observer substitution count")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("character observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if len(result) != DERIVED_BYTES or hashlib.sha256(result).hexdigest() != DERIVED_SHA:
        raise ValueError("wrong derived character observer")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if restored.count(after) != 1:
            raise ValueError("character observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    if restored != inherited:
        raise ValueError("character observer changed predecessor bytes")
    return result

def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "character_controls": list(CHARACTER_CONTROL_NAMES), "character_semantics_qualified": False}
