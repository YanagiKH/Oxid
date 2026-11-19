"""Three reversible record-vector exclusions, preserving every prior observation."""
import hashlib
from pathlib import Path
import re
import types

PARENT = "typed_vector_compatibility_observer.py"
PARENT_PIN = (6550, "713eb117c08f1fd8110e5f943f3adc98af80d8b0efc53d154fee17fa8b47129e")
path = Path(__file__).with_name(PARENT)
if path.is_symlink() or any(parent.is_symlink() for parent in path.parents):
    raise ValueError("symlink pinned record-vector observer predecessor")
before = path.stat()
body = path.read_bytes()
fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
if (not path.is_file() or before.st_nlink != 1 or before.st_mode & 0o111
        or (len(body), hashlib.sha256(body).hexdigest()) != PARENT_PIN
        or tuple(getattr(before, field) for field in fields)
        != tuple(getattr(path.stat(), field) for field in fields)):
    raise ValueError("changed pinned record-vector observer predecessor")
predecessor = types.ModuleType("_record_vector_observer_predecessor")
predecessor.__file__ = str(path)
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)

VERSION = "unit2-record-vector-exclusion-observer-v12"
for _name in (
    "OBSERVER", "CONTROL_FILTER", "CONTROL_NAMES", "FILE_CONTROL_FILTER", "FILE_CONTROL_NAMES",
    "INTEGER_CONTROL_FILTER", "INTEGER_CONTROL_NAMES", "WIDTH_CONTROL_FILTER", "WIDTH_CONTROL_NAMES",
    "QUAD_CONTROL_FILTER", "QUAD_CONTROL_NAMES", "FLOAT_CONTROL_FILTER", "FLOAT_CONTROL_NAMES",
    "CHARACTER_CONTROL_FILTER", "CHARACTER_CONTROL_NAMES", "VECTOR_CONTROL_FILTER", "VECTOR_CONTROL_NAMES",
    "SCALAR_NAMES",
):
    globals()[_name] = getattr(predecessor, _name)

RECORD_VECTOR_CONTROL_FILTER = "current_unit2_record_vector_adapter_"
RECORD_VECTOR_CONTROL_NAMES = tuple("frontend::oir::unit2_observer::" + name for name in (
    "current_unit2_record_vector_adapter_denies_owned_vectors",
    "current_unit2_record_vector_adapter_denies_value_parameters",
    "current_unit2_record_vector_adapter_denies_shared_vectors",
    "current_unit2_record_vector_adapter_denies_exclusive_vectors",
    "current_unit2_record_vector_adapter_denies_shared_slices",
    "current_unit2_record_vector_adapter_denies_exclusive_slices",
))
SEAMS = (
    (b'        AggregateTy::Vector(_) => panic!("current Unit2 observer excludes vector projection"),\n',
     b'        AggregateTy::Vector(_) => panic!("current Unit2 observer excludes vector projection"),\n'
     b'        AggregateTy::RecordVector(_) => panic!("current Unit2 observer excludes record-vector projection"),\n'),
    (b'        BorrowedTy::Exact(AggregateTy::Vector(_)) => panic!("current Unit2 observer excludes vector projection"),\n',
     b'        BorrowedTy::Exact(AggregateTy::Vector(_)) => panic!("current Unit2 observer excludes vector projection"),\n'
     b'        BorrowedTy::Exact(AggregateTy::RecordVector(_)) => panic!("current Unit2 observer excludes record-vector projection"),\n'),
    (b'        BorrowedTy::ScalarSlice(_) => panic!("current Unit2 observer excludes scalar-slice projection"),\n',
     b'        BorrowedTy::ScalarSlice(_) => panic!("current Unit2 observer excludes scalar-slice projection"),\n'
     b'        BorrowedTy::RecordSlice(_) => panic!("current Unit2 observer excludes record-vector projection"),\n'),
)
CONTROL_TESTS = b'''
fn current_unit2_record_vector_ordinals() -> [oir::owned_types::RecordId; 3] {
    [oir::owned_types::RecordId(0), oir::owned_types::RecordId(7), oir::owned_types::RecordId(4095)]
}
fn current_unit2_record_vector_expect_rejection(action: impl FnOnce() -> String + std::panic::UnwindSafe) {
    let failure = std::panic::catch_unwind(action).expect_err("record-vector projection was accepted");
    let message = failure.downcast_ref::<&str>().copied()
        .or_else(|| failure.downcast_ref::<String>().map(String::as_str));
    assert_eq!(message, Some("current Unit2 observer excludes record-vector projection"));
}

#[test]
fn current_unit2_record_vector_adapter_denies_owned_vectors() {
    for record in current_unit2_record_vector_ordinals() {
        current_unit2_record_vector_expect_rejection(|| value_type(ValueTy::Owned(AggregateTy::RecordVector(record))));
    }
}

#[test]
fn current_unit2_record_vector_adapter_denies_value_parameters() {
    for record in current_unit2_record_vector_ordinals() {
        current_unit2_record_vector_expect_rejection(|| parameter_type(ParameterTy::Value(ValueTy::Owned(AggregateTy::RecordVector(record)))));
    }
}

#[test]
fn current_unit2_record_vector_adapter_denies_shared_vectors() {
    for record in current_unit2_record_vector_ordinals() {
        current_unit2_record_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::RecordVector(record)), kind: BorrowKind::Shared }));
    }
}

#[test]
fn current_unit2_record_vector_adapter_denies_exclusive_vectors() {
    for record in current_unit2_record_vector_ordinals() {
        current_unit2_record_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::RecordVector(record)), kind: BorrowKind::Exclusive }));
    }
}

#[test]
fn current_unit2_record_vector_adapter_denies_shared_slices() {
    for record in current_unit2_record_vector_ordinals() {
        current_unit2_record_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::RecordSlice(record), kind: BorrowKind::Shared }));
    }
}

#[test]
fn current_unit2_record_vector_adapter_denies_exclusive_slices() {
    for record in current_unit2_record_vector_ordinals() {
        current_unit2_record_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::RecordSlice(record), kind: BorrowKind::Exclusive }));
    }
}
'''
DERIVED_PIN = (30768, '96b8c45c257740a56b550ee24287bede07d3ca4ecd728ff5d40903cefc8b84d0')


def pin(body):
    return len(body), hashlib.sha256(body).hexdigest()


def controls():
    names = tuple("frontend::oir::unit2_observer::" + name.decode("ascii") for name in re.findall(
        rb"^fn (current_unit2_record_vector_adapter_[A-Za-z_0-9]+)\(\)", CONTROL_TESTS, re.M))
    if (len(SEAMS) != 3 or names != RECORD_VECTOR_CONTROL_NAMES
            or len(names) != len(set(names)) or len(names) != 6
            or CONTROL_TESTS.count(b"#[test]\n") != 6):
        raise ValueError("wrong record-vector observer substitution/control inventory")


def reverse(derived):
    controls()
    if type(derived) is not bytes or pin(derived) != DERIVED_PIN or not derived.endswith(CONTROL_TESTS):
        raise ValueError("wrong pinned record-vector observer output")
    result = derived[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if result.count(after) != 1:
            raise ValueError("record-vector observer reverse seam drift")
        result = result.replace(after, before, 1)
    if pin(result) != (predecessor.DERIVED_BYTES, predecessor.DERIVED_SHA):
        raise ValueError("record-vector observer changed predecessor bytes")
    return result


def adapt(original):
    controls()
    inherited = predecessor.adapt(original)
    result = inherited
    if CONTROL_TESTS in result:
        raise ValueError("repeated record-vector observer adaptation")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("record-vector observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if reverse(result) != inherited:
        raise ValueError("record-vector observer reverse identity differs")
    return result


def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_PIN[0], "sha256": DERIVED_PIN[1]},
            "record_vector_controls": list(RECORD_VECTOR_CONTROL_NAMES),
            "record_ordinals_per_control": 3, "rejection_arms": 3,
            "copy_record_vector_semantics_qualified": False}


# Existing verification helpers use these public identity fields.
DERIVED_BYTES, DERIVED_SHA = DERIVED_PIN
