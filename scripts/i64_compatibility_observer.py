"""Exact i64 refusal layered over the authenticated narrow Unit2 observer.

Every prior observation and all twelve rejection controls remain unchanged.
Two new controls refuse full-width scalar and parameter projection.
"""
import hashlib
from pathlib import Path
import types

PARENT = "native_file_observer.py"
PARENT_SHA = '70810d5315b864aa81a1323d5ac2bfac571870ac8675264325dd34b5d6eaed8f'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned File observer adapter")
predecessor = types.ModuleType("_i64_observer_parent")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
VERSION = "unit2-i64-exclusion-observer-v6"
OBSERVER = predecessor.OBSERVER
CONTROL_FILTER = predecessor.CONTROL_FILTER
CONTROL_NAMES = predecessor.CONTROL_NAMES
FILE_CONTROL_FILTER = predecessor.FILE_CONTROL_FILTER
FILE_CONTROL_NAMES = predecessor.FILE_CONTROL_NAMES
INTEGER_CONTROL_FILTER = "current_unit2_i64_adapter_"
INTEGER_CONTROL_NAMES = (
    "frontend::oir::unit2_observer::current_unit2_i64_adapter_denies_scalar",
    "frontend::oir::unit2_observer::current_unit2_i64_adapter_denies_parameter",
)
DERIVED_BYTES = 20120
DERIVED_SHA = '4fd3b644c71d00b871e3dffe3b7837969ba8c10c743ae87aca4e9aa92b863df4'
SEAMS = ((b'        ValueTy::Scalar(ty) => object(vec![("kind",q("scalar")),("scalar'
  b'",q(&format!("{ty:?}")))]),\n',
  b'        ValueTy::Scalar(hir::Ty::I64) => panic!("current Unit2 observer '
  b'excludes i64 projection"),\n        ValueTy::Scalar(ty) => object(vec![("'
  b'kind",q("scalar")),("scalar",q(&format!("{ty:?}")))]),\n'),)
CONTROL_TESTS = b'\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes i64 projection")]\nfn current_unit2_i64_adapter_denies_scalar() {\n    value_type(ValueTy::Scalar(hir::Ty::I64));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes i64 projection")]\nfn current_unit2_i64_adapter_denies_parameter() {\n    parameter_type(ParameterTy::Value(ValueTy::Scalar(hir::Ty::I64)));\n}\n'


def adapt(original):
    inherited = predecessor.adapt(original)
    result = inherited
    if len(SEAMS) != 1 or CONTROL_TESTS in result:
        raise ValueError("wrong i64 observer substitution count")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("i64 observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if len(result) != DERIVED_BYTES or hashlib.sha256(result).hexdigest() != DERIVED_SHA:
        raise ValueError("wrong derived i64 observer")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if restored.count(after) != 1:
            raise ValueError("i64 observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    if restored != inherited:
        raise ValueError("i64 observer changed predecessor bytes")
    return result


def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "i64_controls": list(INTEGER_CONTROL_NAMES), "i64_semantics_qualified": False}
