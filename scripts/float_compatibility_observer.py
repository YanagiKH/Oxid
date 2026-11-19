"""Explicit floating-point exclusions preserve every preceding narrow observation."""
import hashlib
from pathlib import Path
import types
PARENT = "integer128_compatibility_observer.py"
PARENT_SHA = '2395da52217e376742ed859a63d4be0dcf90fc935a2dde25f32e46c1727870e8'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned integer-width observer adapter")
predecessor = types.ModuleType("_float_observer_parent")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
VERSION = "unit2-floating-point-exclusion-observer-v9"
for _name in ("OBSERVER", "CONTROL_FILTER", "CONTROL_NAMES", "FILE_CONTROL_FILTER", "FILE_CONTROL_NAMES", "INTEGER_CONTROL_FILTER", "INTEGER_CONTROL_NAMES", "WIDTH_CONTROL_FILTER", "WIDTH_CONTROL_NAMES"):
    globals()[_name] = getattr(predecessor, _name)
QUAD_CONTROL_FILTER = predecessor.QUAD_CONTROL_FILTER
QUAD_CONTROL_NAMES = predecessor.QUAD_CONTROL_NAMES
FLOAT_CONTROL_FILTER = 'current_unit2_float_adapter_'
FLOAT_CONTROL_NAMES = ('frontend::oir::unit2_observer::current_unit2_float_adapter_denies_scalar_f32', 'frontend::oir::unit2_observer::current_unit2_float_adapter_denies_parameter_f32', 'frontend::oir::unit2_observer::current_unit2_float_adapter_denies_scalar_f64', 'frontend::oir::unit2_observer::current_unit2_float_adapter_denies_parameter_f64')
SEAMS = ((b'        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),\n', b'        ValueTy::Scalar(hir::Ty::F32 | hir::Ty::F64) => panic!("current Unit2 observer excludes floating-point projection"),\n        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),\n'),)
CONTROL_TESTS = b'\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes floating-point projection")]\nfn current_unit2_float_adapter_denies_scalar_f32() {\n    value_type(ValueTy::Scalar(hir::Ty::F32));\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes floating-point projection")]\nfn current_unit2_float_adapter_denies_parameter_f32() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::F32)));\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes floating-point projection")]\nfn current_unit2_float_adapter_denies_scalar_f64() {\n    value_type(ValueTy::Scalar(hir::Ty::F64));\n}\n\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes floating-point projection")]\nfn current_unit2_float_adapter_denies_parameter_f64() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::F64)));\n}\n'
DERIVED_BYTES = 25379
DERIVED_SHA = '39b7e93c71a5f696db91bf56e08018514b1a88661cd1f4344b4e36bf31f7c742'

def adapt(original):
    inherited = predecessor.adapt(original)
    result = inherited
    if len(SEAMS) != 1 or CONTROL_TESTS in result:
        raise ValueError("wrong floating-point observer substitution count")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("floating-point observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if len(result) != DERIVED_BYTES or hashlib.sha256(result).hexdigest() != DERIVED_SHA:
        raise ValueError("wrong derived floating-point observer")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if restored.count(after) != 1:
            raise ValueError("floating-point observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    if restored != inherited:
        raise ValueError("floating-point observer changed predecessor bytes")
    return result

def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "float_controls": list(FLOAT_CONTROL_NAMES), "float_semantics_qualified": False}
