"""Explicit 128-bit exclusions preserve every preceding narrow observation."""
import hashlib
from pathlib import Path
import types
PARENT = "integer_width_compatibility_observer.py"
PARENT_SHA = "a137fd88df8ce841bd182ec8549db33bb919a7c9c2c9f0f1b7df802754fab9de"
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned integer-width observer adapter")
predecessor = types.ModuleType("_integer128_observer_parent")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
VERSION = "unit2-integer128-exclusion-observer-v8"
for _name in ("OBSERVER", "CONTROL_FILTER", "CONTROL_NAMES", "FILE_CONTROL_FILTER", "FILE_CONTROL_NAMES", "INTEGER_CONTROL_FILTER", "INTEGER_CONTROL_NAMES", "WIDTH_CONTROL_FILTER", "WIDTH_CONTROL_NAMES"):
    globals()[_name] = getattr(predecessor, _name)
QUAD_CONTROL_FILTER = "current_unit2_integer128_adapter_"
QUAD_CONTROL_NAMES = ('frontend::oir::unit2_observer::current_unit2_integer128_adapter_denies_scalar_i128',
 'frontend::oir::unit2_observer::current_unit2_integer128_adapter_denies_parameter_i128',
 'frontend::oir::unit2_observer::current_unit2_integer128_adapter_denies_scalar_u128',
 'frontend::oir::unit2_observer::current_unit2_integer128_adapter_denies_parameter_u128')
SEAMS = ((b'        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),\n', b'        ValueTy::Scalar(hir::Ty::I128 | hir::Ty::U128) => panic!("current Unit2 observer excludes 128-bit integer projection"),\n        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),\n'),)
CONTROL_TESTS = b'\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes 128-bit integer projection")]\nfn current_unit2_integer128_adapter_denies_scalar_i128() {\n    value_type(ValueTy::Scalar(hir::Ty::I128));\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes 128-bit integer projection")]\nfn current_unit2_integer128_adapter_denies_parameter_i128() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I128)));\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes 128-bit integer projection")]\nfn current_unit2_integer128_adapter_denies_scalar_u128() {\n    value_type(ValueTy::Scalar(hir::Ty::U128));\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes 128-bit integer projection")]\nfn current_unit2_integer128_adapter_denies_parameter_u128() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::U128)));\n}\n'
DERIVED_BYTES = 24404
DERIVED_SHA = 'f51f3a1fcaf0664e6b65376ecfa0699d3009497908f4e15bc0ca5faf3167826c'

def adapt(original):
    inherited = predecessor.adapt(original)
    result = inherited
    if len(SEAMS) != 1 or CONTROL_TESTS in result:
        raise ValueError("wrong 128-bit observer substitution count")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("128-bit observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if len(result) != DERIVED_BYTES or hashlib.sha256(result).hexdigest() != DERIVED_SHA:
        raise ValueError("wrong derived 128-bit observer")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if restored.count(after) != 1:
            raise ValueError("128-bit observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    if restored != inherited:
        raise ValueError("128-bit observer changed predecessor bytes")
    return result

def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "integer128_controls": list(QUAD_CONTROL_NAMES), "integer128_semantics_qualified": False}
