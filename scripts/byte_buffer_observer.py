"""Exact ByteBuf exclusions for the retained scalar/record Unit2 observer.

This adapter adds no observation schema or ByteBuf acceptance. The authenticated
predecessor is preserved byte for byte except for two explicit rejection arms;
three separate rejection controls are appended after its existing contents.
"""
import hashlib
import re


VERSION = "unit2-record-byte-buffer-exclusion-observer-v4"
PREDECESSOR_VERSION = "unit2-record-enum-exclusion-observer-v3"
PREDECESSOR_SHA = "5780a5b20f24d1f8bf5bcdb0ddae628be0f9cc53e10be527c490d6a427def085"
PREDECESSOR_BYTES = 17611
DERIVED_SHA = "2fce7ec150df49036cfcfbc84d14497bcad78608d0d4bb627bf75f619b98d826"
DERIVED_BYTES = 18646
OBSERVER = "semantic/observer.rs"
SCOPE = (
    "Two explicit ByteBuf rejection arms and three separately selected rejection "
    "controls; all predecessor scalar/record observations and six aggregate "
    "controls remain unchanged. Does not qualify ByteBuf semantics."
)
CONTROL_FILTER = "current_unit2_byte_buffer_adapter_"
CONTROL_NAMES = (
    "frontend::oir::unit2_observer::current_unit2_byte_buffer_adapter_denies_owned_byte_buffer",
    "frontend::oir::unit2_observer::current_unit2_byte_buffer_adapter_denies_shared_byte_buffer",
    "frontend::oir::unit2_observer::current_unit2_byte_buffer_adapter_denies_exclusive_byte_buffer",
)

# Include the complete existing enum-denial line in each exact context. Neither
# historical enum rejection nor scalar/record formatting is replaced.
SEAMS = (
    (
        b'        AggregateTy::Enum(_) => panic!("current Unit2 observer excludes enum projection"),\n',
        b'        AggregateTy::Enum(_) => panic!("current Unit2 observer excludes enum projection"),\n'
        b'        AggregateTy::ByteBuf => panic!("current Unit2 observer excludes byte-buffer projection"),\n',
    ),
    (
        b'        BorrowedTy::Exact(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection"),\n',
        b'        BorrowedTy::Exact(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection"),\n'
        b'        BorrowedTy::Exact(AggregateTy::ByteBuf) => panic!("current Unit2 observer excludes byte-buffer projection"),\n',
    ),
)
CONTROL_TESTS = b'''
#[test]
#[should_panic(expected = "current Unit2 observer excludes byte-buffer projection")]
fn current_unit2_byte_buffer_adapter_denies_owned_byte_buffer() {
    value_type(ValueTy::Owned(AggregateTy::ByteBuf));
}
#[test]
#[should_panic(expected = "current Unit2 observer excludes byte-buffer projection")]
fn current_unit2_byte_buffer_adapter_denies_shared_byte_buffer() {
    parameter_type(ParameterTy::Reference {
        referent: BorrowedTy::Exact(AggregateTy::ByteBuf), kind: BorrowKind::Shared,
    });
}
#[test]
#[should_panic(expected = "current Unit2 observer excludes byte-buffer projection")]
fn current_unit2_byte_buffer_adapter_denies_exclusive_byte_buffer() {
    parameter_type(ParameterTy::Reference {
        referent: BorrowedTy::Exact(AggregateTy::ByteBuf), kind: BorrowKind::Exclusive,
    });
}
'''


def _require(condition, message):
    if not condition:
        raise ValueError(message)


def _sha(body):
    return hashlib.sha256(body).hexdigest()


def _controls():
    names = tuple("frontend::oir::unit2_observer::" + name.decode("ascii")
                  for name in re.findall(rb"^fn ([A-Za-z_][A-Za-z_0-9]*)\(", CONTROL_TESTS, re.M))
    _require(len(CONTROL_NAMES) == len(set(CONTROL_NAMES)) == 3
             and names == CONTROL_NAMES
             and all(name.rsplit("::", 1)[-1].startswith(CONTROL_FILTER) for name in names)
             and CONTROL_TESTS.count(b"#[test]\n") == 3
             and CONTROL_TESTS.count(
                 b'#[should_panic(expected = "current Unit2 observer excludes byte-buffer projection")]\n'
             ) == 3,
             "wrong ByteBuf observer rejection controls")


def adapt(original):
    """Return the single approved successor, rejecting unknown or edited input."""
    _require(type(original) is bytes and len(original) == PREDECESSOR_BYTES
             and _sha(original) == PREDECESSOR_SHA,
             "wrong predecessor ByteBuf Unit2 observer")
    _require(len(SEAMS) == 2, "wrong ByteBuf observer substitution count")
    _controls()
    _require(CONTROL_FILTER.encode("ascii") not in original and CONTROL_TESTS not in original,
             "ByteBuf observer controls already present")
    result = original
    for before, after in SEAMS:
        _require(result.count(before) == 1 and after not in result,
                 "ByteBuf observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    _require(len(result) == DERIVED_BYTES and _sha(result) == DERIVED_SHA,
             "wrong derived ByteBuf Unit2 observer")
    _require(result.endswith(CONTROL_TESTS) and result.count(CONTROL_TESTS) == 1,
             "ByteBuf observer reverse control drift")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        _require(restored.count(after) == 1, "ByteBuf observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    _require(restored == original, "ByteBuf observer reverse identity differs")
    return result


def descriptor():
    """Describe the literal transformation; this metadata is not execution evidence."""
    return {
        "version": VERSION,
        "predecessor_version": PREDECESSOR_VERSION,
        "predecessor": {"path": OBSERVER, "bytes": PREDECESSOR_BYTES, "sha256": PREDECESSOR_SHA},
        "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
        "substitutions": [{"before_bytes": len(before), "before_sha256": _sha(before),
                           "after_bytes": len(after), "after_sha256": _sha(after)}
                          for before, after in SEAMS],
        "appended_controls": {"bytes": len(CONTROL_TESTS), "sha256": _sha(CONTROL_TESTS)},
        "control_filter": CONTROL_FILTER,
        "control_names": list(CONTROL_NAMES),
        "byte_buffer_semantics_qualified": False,
        "scope": SCOPE,
    }
