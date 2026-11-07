"""Closed Vec exclusions for the retained scalar/record observer.

This successor adds only two explicit rejection arms and four separate controls
covering every scalar element. All preceding observation bytes are reversible;
this is not qualification of vector language or runtime semantics.
"""
import hashlib
from pathlib import Path
import re
import types

PARENT = "character_compatibility_observer.py"
PARENT_BYTES = 3548
PARENT_SHA = '6acae0c103ec7cdd2df4556e00db2497bb0197ed0bda197d3b4ee45df9b812b2'
path = Path(__file__).with_name(PARENT)
if path.is_symlink():
    raise ValueError("symlink pinned character observer adapter")
body = path.read_bytes()
if (len(body), hashlib.sha256(body).hexdigest()) != (PARENT_BYTES, PARENT_SHA):
    raise ValueError("changed pinned character observer adapter")
predecessor = types.ModuleType("_typed_vector_observer_parent")
predecessor.__file__ = str(path)
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)

VERSION = "unit2-vector-exclusion-observer-v11"
for _name in ("OBSERVER", "CONTROL_FILTER", "CONTROL_NAMES", "FILE_CONTROL_FILTER", "FILE_CONTROL_NAMES",
              "INTEGER_CONTROL_FILTER", "INTEGER_CONTROL_NAMES", "WIDTH_CONTROL_FILTER", "WIDTH_CONTROL_NAMES",
              "QUAD_CONTROL_FILTER", "QUAD_CONTROL_NAMES", "FLOAT_CONTROL_FILTER", "FLOAT_CONTROL_NAMES",
              "CHARACTER_CONTROL_FILTER", "CHARACTER_CONTROL_NAMES"):
    globals()[_name] = getattr(predecessor, _name)
VECTOR_CONTROL_FILTER = "current_unit2_vector_adapter_"
VECTOR_CONTROL_NAMES = ('frontend::oir::unit2_observer::current_unit2_vector_adapter_denies_owned_vectors', 'frontend::oir::unit2_observer::current_unit2_vector_adapter_denies_value_parameter_vectors', 'frontend::oir::unit2_observer::current_unit2_vector_adapter_denies_shared_vectors', 'frontend::oir::unit2_observer::current_unit2_vector_adapter_denies_exclusive_vectors')
SCALAR_NAMES = ('I8', 'I16', 'I32', 'I64', 'I128', 'U8', 'U16', 'U32', 'U64', 'U128', 'Isize', 'Usize', 'F32', 'F64', 'Bool', 'Char', 'Unit')
SEAMS = ((b'        AggregateTy::Enum(_) => panic!("current Unit2 observer excludes enum projection"),\n', b'        AggregateTy::Enum(_) => panic!("current Unit2 observer excludes enum projection"),\n        AggregateTy::Vector(_) => panic!("current Unit2 observer excludes vector projection"),\n'), (b'        BorrowedTy::Exact(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection"),\n', b'        BorrowedTy::Exact(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection"),\n        BorrowedTy::Exact(AggregateTy::Vector(_)) => panic!("current Unit2 observer excludes vector projection"),\n'))
CONTROL_TESTS = b'\nfn current_unit2_vector_elements() -> [hir::Ty; 17] {\n    [hir::Ty::I8, hir::Ty::I16, hir::Ty::I32, hir::Ty::I64, hir::Ty::I128, hir::Ty::U8, hir::Ty::U16, hir::Ty::U32, hir::Ty::U64, hir::Ty::U128, hir::Ty::Isize, hir::Ty::Usize, hir::Ty::F32, hir::Ty::F64, hir::Ty::Bool, hir::Ty::Char, hir::Ty::Unit]\n}\nfn current_unit2_vector_expect_rejection(action: impl FnOnce() -> String + std::panic::UnwindSafe) {\n    let failure = std::panic::catch_unwind(action).expect_err("vector projection was accepted");\n    let message = failure.downcast_ref::<&str>().copied()\n        .or_else(|| failure.downcast_ref::<String>().map(String::as_str));\n    assert_eq!(message, Some("current Unit2 observer excludes vector projection"));\n}\n\n#[test]\nfn current_unit2_vector_adapter_denies_owned_vectors() {\n    for element in current_unit2_vector_elements() {\n        current_unit2_vector_expect_rejection(|| value_type(ValueTy::Owned(AggregateTy::Vector(element))));\n    }\n}\n\n#[test]\nfn current_unit2_vector_adapter_denies_value_parameter_vectors() {\n    for element in current_unit2_vector_elements() {\n        current_unit2_vector_expect_rejection(|| parameter_type(ParameterTy::Value(ValueTy::Owned(AggregateTy::Vector(element)))));\n    }\n}\n\n#[test]\nfn current_unit2_vector_adapter_denies_shared_vectors() {\n    for element in current_unit2_vector_elements() {\n        current_unit2_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::Vector(element)), kind: BorrowKind::Shared }));\n    }\n}\n\n#[test]\nfn current_unit2_vector_adapter_denies_exclusive_vectors() {\n    for element in current_unit2_vector_elements() {\n        current_unit2_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::Vector(element)), kind: BorrowKind::Exclusive }));\n    }\n}\n'
DERIVED_BYTES = 27967
DERIVED_SHA = '3206e4d769a790ddcec4dd3d5515c90f062dd304e5ead1fe17ff07f2681e42cf'


def _controls():
    names = tuple("frontend::oir::unit2_observer::" + name.decode("ascii")
                  for name in re.findall(rb"^fn (current_unit2_vector_adapter_[A-Za-z_0-9]+)\(", CONTROL_TESTS, re.M))
    if (len(SEAMS) != 2 or names != VECTOR_CONTROL_NAMES
            or len(names) != len(set(names)) or len(names) != 4
            or len(SCALAR_NAMES) != len(set(SCALAR_NAMES)) or len(SCALAR_NAMES) != 17
            or CONTROL_TESTS.count(b"#[test]\n") != 4):
        raise ValueError("wrong vector observer substitution/control inventory")


def adapt(original):
    _controls()
    inherited = predecessor.adapt(original)
    result = inherited
    if CONTROL_TESTS in result:
        raise ValueError("repeated vector observer adaptation")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("vector observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if (len(result), hashlib.sha256(result).hexdigest()) != (DERIVED_BYTES, DERIVED_SHA):
        raise ValueError("wrong derived vector observer")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if restored.count(after) != 1:
            raise ValueError("vector observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    if restored != inherited:
        raise ValueError("vector observer changed predecessor bytes")
    return result


def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "vector_controls": list(VECTOR_CONTROL_NAMES),
            "vector_elements_per_control": len(SCALAR_NAMES),
            "vector_semantics_qualified": False}
