"""Exact i64 refusal layered over the authenticated narrow Unit2 observer.

Every prior observation and all twelve rejection controls remain unchanged.
Two new controls refuse full-width scalar and parameter projection.
"""
import hashlib
from pathlib import Path
import types

PARENT = "i64_compatibility_observer.py"
PARENT_SHA = 'c0a53a4887138984f8047274bdf72d080b805456468a1c74250c4365263f2c41'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned File observer adapter")
predecessor = types.ModuleType("_integer_width_observer_parent")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
VERSION = "unit2-integer-width-exclusion-observer-v7"
OBSERVER = predecessor.OBSERVER
CONTROL_FILTER = predecessor.CONTROL_FILTER
CONTROL_NAMES = predecessor.CONTROL_NAMES
FILE_CONTROL_FILTER = predecessor.FILE_CONTROL_FILTER
FILE_CONTROL_NAMES = predecessor.FILE_CONTROL_NAMES
INTEGER_CONTROL_FILTER = predecessor.INTEGER_CONTROL_FILTER
INTEGER_CONTROL_NAMES = predecessor.INTEGER_CONTROL_NAMES
WIDTH_CONTROL_FILTER = "current_unit2_integer_width_adapter_"
WIDTH_CONTROL_NAMES = ('frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_scalar_i8',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_parameter_i8',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_scalar_i16',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_parameter_i16',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_scalar_u16',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_parameter_u16',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_scalar_u32',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_parameter_u32',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_scalar_u64',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_parameter_u64',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_scalar_isize',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_parameter_isize',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_scalar_usize',
 'frontend::oir::unit2_observer::current_unit2_integer_width_adapter_denies_parameter_usize')
DERIVED_BYTES = 23394
DERIVED_SHA = '85dbde03710200bf1cbf00b2bea1308d9aecddc2c405ffa024e8bd89becae7fa'
SEAMS = ((b'        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",q(&format!("{ty:?}'
  b'")))]),\n',
  b'        ValueTy::Scalar(hir::Ty::I8 | hir::Ty::I16 | hir::Ty::U16 | hir::Ty::U32 | hir::Ty::'
  b'U64 | hir::Ty::Isize | hir::Ty::Usize) => panic!("current Unit2 observer excludes integer wi'
  b'dth projection"),\n        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar",'
  b'q(&format!("{ty:?}")))]),\n'),)
CONTROL_TESTS = b'\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_scalar_i8() {\n    value_type(ValueTy::Scalar(hir::Ty::I8));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_parameter_i8() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I8)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_scalar_i16() {\n    value_type(ValueTy::Scalar(hir::Ty::I16));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_parameter_i16() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I16)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_scalar_u16() {\n    value_type(ValueTy::Scalar(hir::Ty::U16));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_parameter_u16() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::U16)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_scalar_u32() {\n    value_type(ValueTy::Scalar(hir::Ty::U32));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_parameter_u32() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::U32)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_scalar_u64() {\n    value_type(ValueTy::Scalar(hir::Ty::U64));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_parameter_u64() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::U64)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_scalar_isize() {\n    value_type(ValueTy::Scalar(hir::Ty::Isize));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_parameter_isize() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Isize)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_scalar_usize() {\n    value_type(ValueTy::Scalar(hir::Ty::Usize));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes integer width projection")]\nfn current_unit2_integer_width_adapter_denies_parameter_usize() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::Usize)));\n}\n'


def adapt(original):
    inherited = predecessor.adapt(original)
    result = inherited
    if len(SEAMS) != 1 or CONTROL_TESTS in result:
        raise ValueError("wrong integer-width observer substitution count")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("integer-width observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if len(result) != DERIVED_BYTES or hashlib.sha256(result).hexdigest() != DERIVED_SHA:
        raise ValueError("wrong derived integer-width observer")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if restored.count(after) != 1:
            raise ValueError("integer-width observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    if restored != inherited:
        raise ValueError("integer-width observer changed predecessor bytes")
    return result


def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "integer_width_controls": list(WIDTH_CONTROL_NAMES), "integer_width_semantics_qualified": False}
