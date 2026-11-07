"""Closed named-slice exclusions for retained scalar observer wrappers.

Only the new AST variant is rejected. The complete local-reference predecessor
can be reconstructed, including every historical scalar/refusal wire behavior.
Canonical Vec support is compiled as a dependency, never exposed as scalar AST.
"""
import hashlib
from pathlib import Path
import types

SCHEMA = "legacy-scalar-observer-vector-compatibility-12"
PARENT = "legacy_scalar_observer_local_references.py"
PARENT_BYTES = 2741
PARENT_SHA = '6178badfd8ca16f0cab6b088f073bca28a2c98162d6bf7d3317f8eda859b0d2b'
body = Path(__file__).with_name(PARENT).read_bytes()
if (len(body), hashlib.sha256(body).hexdigest()) != (PARENT_BYTES, PARENT_SHA):
    raise ValueError("changed pinned local-reference scalar observer adapter")
predecessor = types.ModuleType("_vector_scalar_predecessor")
predecessor.__file__ = str(Path(__file__).with_name(PARENT))
exec(compile(body, predecessor.__file__, "exec"), predecessor.__dict__)
PARENT_SCHEMA = predecessor.SCHEMA
PINS = predecessor.PINS
DERIVED_PINS = {'parser/frontend_mod.rs': '4359f3c9847432172932de735a6a7cfb0b0552632b2e1cedd12e3fa4e871372d', 'parser/main.rs': '384d007dab5a0562a66eac087d33ac54b2712dff5be06960dc692687519b7704', 'parser/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/frontend_mod.rs': '7743e8012b481a491d667ee4ef1740e1223c8087471b3498e56dec1fc41ba640', 'static/main.rs': '08e1b7ef3763af12520523870babc3e432d8fad0e1982017c04579e0a2ede225', 'static/oir_mod.rs': 'a41583a4ae61fabc62fc798687f20d281cea896a7f1c4fcb2736d42effb4dc3b', 'static/static_observer.rs': 'ab372ffdb7d03d39df6170a0cbc8584fcaabb6331157a1bf631c617c2b6671bf'}
EDITS = ((b'        TypeSyntaxKind::SliceReference { .. } => "slice_reference_type",\n', b'        TypeSyntaxKind::SliceReference { .. } => "slice_reference_type",\n        TypeSyntaxKind::GenericSliceReference { .. } => "named_slice_reference_type",\n'),)


def reverse_wrapper(kind, name, derived):
    key = kind + "/" + name
    if key not in DERIVED_PINS or hashlib.sha256(derived).hexdigest() != DERIVED_PINS[key]:
        raise ValueError("wrong pinned vector scalar observer output")
    result = derived
    if name == "frontend_mod.rs":
        if len(EDITS) != 1:
            raise ValueError("wrong named-slice observer substitution count")
        for before, after in reversed(EDITS):
            if result.count(after) != 1:
                raise ValueError("named-slice observer reverse seam drift")
            result = result.replace(after, before, 1)
    if hashlib.sha256(result).hexdigest() != predecessor.DERIVED_PINS[key]:
        raise ValueError("named-slice observer changed preceding projection")
    return result


def adapt_wrapper(kind, name, original):
    inherited = predecessor.adapt_wrapper(kind, name, original)
    result = inherited
    if name == "frontend_mod.rs":
        if len(EDITS) != 1:
            raise ValueError("wrong named-slice observer substitution count")
        for before, after in EDITS:
            if result.count(before) != 1 or after in result:
                raise ValueError("named-slice observer seam drift")
            result = result.replace(before, after, 1)
    if reverse_wrapper(kind, name, result) != inherited:
        raise ValueError("named-slice observer changed predecessor bytes")
    return result
