"""Exact File exclusions layered over the unchanged ByteBuf Unit2 observer.

Every predecessor scalar/record observation and rejection control is preserved.
The three additional controls refuse File in the legacy projection.
"""
import hashlib
from pathlib import Path
import types

PARENT = "byte_buffer_observer.py"
PARENT_SHA = '46063f42b5e169b0d7536bedbcbf5df57e9b1c56d19e0c45bffd7e1522bfeccf'
body = Path(__file__).with_name(PARENT).read_bytes()
if hashlib.sha256(body).hexdigest() != PARENT_SHA:
    raise ValueError("changed pinned ByteBuf observer adapter")
predecessor = types.ModuleType("_native_file_observer_parent")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
VERSION = "unit2-record-native-file-exclusion-observer-v5"
OBSERVER = predecessor.OBSERVER
CONTROL_FILTER = predecessor.CONTROL_FILTER
CONTROL_NAMES = predecessor.CONTROL_NAMES
FILE_CONTROL_FILTER = "current_unit2_native_file_adapter_"
FILE_CONTROL_NAMES = ('frontend::oir::unit2_observer::current_unit2_native_file_adapter_denies_owned_native_file', 'frontend::oir::unit2_observer::current_unit2_native_file_adapter_denies_shared_native_file', 'frontend::oir::unit2_observer::current_unit2_native_file_adapter_denies_exclusive_native_file')
DERIVED_BYTES = 19631
DERIVED_SHA = 'e76de3d711ae0cece2c4bf1d99a3fd1ba2994b518cda801596e6a97cf2f7d692'
SEAMS = ((b'        AggregateTy::Enum(_) => panic!("current Unit2 observer excludes enum projection"),\n', b'        AggregateTy::Enum(_) => panic!("current Unit2 observer excludes enum projection"),\n        AggregateTy::File => panic!("current Unit2 observer excludes file projection"),\n'), (b'        BorrowedTy::Exact(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection"),\n', b'        BorrowedTy::Exact(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection"),\n        BorrowedTy::Exact(AggregateTy::File) => panic!("current Unit2 observer excludes file projection"),\n'))
CONTROL_TESTS = b'\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes file projection")]\nfn current_unit2_native_file_adapter_denies_owned_native_file() {\n    value_type(ValueTy::Owned(AggregateTy::File));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes file projection")]\nfn current_unit2_native_file_adapter_denies_shared_native_file() {\n    parameter_type(ParameterTy::Reference {\n        referent: BorrowedTy::Exact(AggregateTy::File), kind: BorrowKind::Shared,\n    });\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes file projection")]\nfn current_unit2_native_file_adapter_denies_exclusive_native_file() {\n    parameter_type(ParameterTy::Reference {\n        referent: BorrowedTy::Exact(AggregateTy::File), kind: BorrowKind::Exclusive,\n    });\n}\n'


def adapt(original):
    inherited = predecessor.adapt(original)
    result = inherited
    if len(SEAMS) != 2 or CONTROL_TESTS in result:
        raise ValueError("wrong File observer substitution count")
    for before, after in SEAMS:
        if result.count(before) != 1 or after in result:
            raise ValueError("File observer seam drift")
        result = result.replace(before, after, 1)
    result += CONTROL_TESTS
    if len(result) != DERIVED_BYTES or hashlib.sha256(result).hexdigest() != DERIVED_SHA:
        raise ValueError("wrong derived File observer")
    restored = result[:-len(CONTROL_TESTS)]
    for before, after in reversed(SEAMS):
        if restored.count(after) != 1:
            raise ValueError("File observer reverse seam drift")
        restored = restored.replace(after, before, 1)
    if restored != inherited:
        raise ValueError("File observer changed predecessor bytes")
    return result


def descriptor():
    return {"version": VERSION, "predecessor": predecessor.descriptor(),
            "derived": {"path": OBSERVER, "bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "file_controls": list(FILE_CONTROL_NAMES),
            "native_file_semantics_qualified": False}
