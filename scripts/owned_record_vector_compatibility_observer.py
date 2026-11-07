"""Two reversible owned-vector exclusions; the retained wire stays unchanged."""
import hashlib
from pathlib import Path
import re
import stat
import types

PARENT = "record_vector_compatibility_observer.py"
PARENT_PIN = (8158, "22ccfc071ce18a03c809f226d5bf260e3622741bf1c364d6fbc8c294b364e48d")
PARENT_DERIVED_PIN = (30768, "96b8c45c257740a56b550ee24287bede07d3ca4ecd728ff5d40903cefc8b84d0")
path = Path(__file__).with_name(PARENT)
if path.is_symlink() or any(parent.is_symlink() for parent in path.parents):
    raise ValueError("symlink pinned owned-vector observer predecessor")
before = path.stat()
body = path.read_bytes()
fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_mode & 0o111
        or (len(body), hashlib.sha256(body).hexdigest()) != PARENT_PIN
        or tuple(getattr(before, field) for field in fields)
        != tuple(getattr(path.stat(), field) for field in fields)):
    raise ValueError("changed pinned owned-vector observer predecessor")
predecessor = types.ModuleType("_owned_record_vector_observer_predecessor")
predecessor.__file__ = str(path)
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
if predecessor.DERIVED_PIN != PARENT_DERIVED_PIN:
    raise ValueError("wrong owned-vector observer predecessor output pin")

VERSION = "unit2-owned-record-vector-exclusion-observer-v13"
for _name in (
    "OBSERVER", "CONTROL_FILTER", "CONTROL_NAMES", "FILE_CONTROL_FILTER", "FILE_CONTROL_NAMES",
    "INTEGER_CONTROL_FILTER", "INTEGER_CONTROL_NAMES", "WIDTH_CONTROL_FILTER", "WIDTH_CONTROL_NAMES",
    "QUAD_CONTROL_FILTER", "QUAD_CONTROL_NAMES", "FLOAT_CONTROL_FILTER", "FLOAT_CONTROL_NAMES",
    "CHARACTER_CONTROL_FILTER", "CHARACTER_CONTROL_NAMES", "VECTOR_CONTROL_FILTER", "VECTOR_CONTROL_NAMES",
    "RECORD_VECTOR_CONTROL_FILTER", "RECORD_VECTOR_CONTROL_NAMES", "SCALAR_NAMES",
):
    globals()[_name] = getattr(predecessor, _name)

OWNED_RECORD_VECTOR_CONTROL_FILTER = "current_unit2_owned_record_vector_adapter_"
OWNED_RECORD_VECTOR_CONTROL_NAMES = tuple("frontend::oir::unit2_observer::" + name for name in (
    "current_unit2_owned_record_vector_adapter_denies_values",
    "current_unit2_owned_record_vector_adapter_denies_shared_vectors",
    "current_unit2_owned_record_vector_adapter_denies_exclusive_vectors",
))
SEAMS = (
    (b'        AggregateTy::RecordVector(_) => panic!("current Unit2 observer excludes record-vector projection"),\n',
     b'        AggregateTy::RecordVector(_) => panic!("current Unit2 observer excludes record-vector projection"),\n'
     b'        AggregateTy::OwnedRecordVector(_) => panic!("current Unit2 observer excludes owned-record-vector projection"),\n'),
    (b'        BorrowedTy::Exact(AggregateTy::RecordVector(_)) => panic!("current Unit2 observer excludes record-vector projection"),\n',
     b'        BorrowedTy::Exact(AggregateTy::RecordVector(_)) => panic!("current Unit2 observer excludes record-vector projection"),\n'
     b'        BorrowedTy::Exact(AggregateTy::OwnedRecordVector(_)) => panic!("current Unit2 observer excludes owned-record-vector projection"),\n'),
)
CONTROL_TESTS = b'''
fn current_unit2_owned_record_vector_ordinals() -> [oir::owned_types::RecordId; 3] {
    [oir::owned_types::RecordId(0), oir::owned_types::RecordId(7), oir::owned_types::RecordId(4095)]
}
fn current_unit2_owned_record_vector_expect_rejection(action: impl FnOnce() -> String + std::panic::UnwindSafe) {
    let failure = std::panic::catch_unwind(action).expect_err("owned-record-vector projection was accepted");
    let message = failure.downcast_ref::<&str>().copied()
        .or_else(|| failure.downcast_ref::<String>().map(String::as_str));
    assert_eq!(message, Some("current Unit2 observer excludes owned-record-vector projection"));
}

#[test]
fn current_unit2_owned_record_vector_adapter_denies_values() {
    for record in current_unit2_owned_record_vector_ordinals() {
        current_unit2_owned_record_vector_expect_rejection(|| parameter_type(ParameterTy::Value(ValueTy::Owned(AggregateTy::OwnedRecordVector(record)))));
    }
}

#[test]
fn current_unit2_owned_record_vector_adapter_denies_shared_vectors() {
    for record in current_unit2_owned_record_vector_ordinals() {
        current_unit2_owned_record_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::OwnedRecordVector(record)), kind: BorrowKind::Shared }));
    }
}

#[test]
fn current_unit2_owned_record_vector_adapter_denies_exclusive_vectors() {
    for record in current_unit2_owned_record_vector_ordinals() {
        current_unit2_owned_record_vector_expect_rejection(|| parameter_type(ParameterTy::Reference { referent: BorrowedTy::Exact(AggregateTy::OwnedRecordVector(record)), kind: BorrowKind::Exclusive }));
    }
}
'''
DERIVED_PIN = (32684, "8e65804b8486ec2fda4bca21875c3c296f0b722bd6c40041ee1144b4c44e6042")


def pin(body):
    return len(body), hashlib.sha256(body).hexdigest()


def controls():
    names = tuple("frontend::oir::unit2_observer::" + name.decode("ascii") for name in re.findall(
        rb"^fn (current_unit2_owned_record_vector_adapter_[A-Za-z_0-9]+)\(\)", CONTROL_TESTS, re.M))
    if (len(SEAMS) != 2 or names != OWNED_RECORD_VECTOR_CONTROL_NAMES
            or len(names) != len(set(names)) or len(names) != 3
            or CONTROL_TESTS.count(b"#[test]\n") != 3):
        raise ValueError("wrong owned-vector observer substitution/control inventory")


def reverse(derived):
    controls()
    if type(derived) is not bytes or pin(derived) != DERIVED_PIN or not derived.endswith(CONTROL_TESTS):
        raise ValueError("wrong pinned owned-vector observer output")
    result = derived[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if result.count(after) != 1:
            raise ValueError("owned-vector observer reverse seam drift")
        result = result.replace(after, before, 1)
    if pin(result) != PARENT_DERIVED_PIN:
        raise ValueError("owned-vector observer changed predecessor bytes")
    return result


def adapt(original):
    controls()
    inherited = predecessor.adapt(original)
    if pin(inherited) != PARENT_DERIVED_PIN:
        raise ValueError("wrong owned-vector observer predecessor bytes")
    result = inherited
    if CONTROL_TESTS in result:
        raise ValueError("repeated owned-vector observer adaptation")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("owned-vector observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if reverse(result) != inherited:
        raise ValueError("owned-vector observer reverse identity differs")
    return result


def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_PIN[0], "sha256": DERIVED_PIN[1]},
            "owned_record_vector_controls": list(OWNED_RECORD_VECTOR_CONTROL_NAMES),
            "record_ordinals_per_control": 3, "rejection_arms": 2,
            "owned_record_vector_semantics_qualified": False}


DERIVED_BYTES, DERIVED_SHA = DERIVED_PIN
