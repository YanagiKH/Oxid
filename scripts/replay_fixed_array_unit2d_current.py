#!/usr/bin/env python3
"""Current Unit2D carrier successors; frozen runner and inputs stay unchanged.

Only the old non-enum harness receives empty carriers. Its two aggregate matches
reject enums explicitly. Every transformation pins both byte identities and is
reversible; no assertion, expectation, test selection, or compiler file changes.
The stdin successor adds only explicit BuiltinOrigins::None after the preserved
enum predecessor. The pre-u8 successor rejects u8 in eight historical oracle
match sites. Each stage retains its own identities and inverse operation.
"""
import hashlib
from pathlib import Path
import types


VERSION = "unit2d-empty-enum-carriers-v1"
FROZEN_RUNNER_SHA = "f76aa8a1b418adfc25487a240a9a2d9e9e76c893e04a27bfd7aaa73a48777d9b"
CURRENT_RUNNER_SHA = "a5de16ebca1f1ba8c12500d1c20f41b24e6f5f8ba20d730c04573a820fa4f230"
IDENTITIES = {
    "reviewer": ("3a2181252e10d3e9a18c836583dbf70765d25f60d0ce9c9749d42682d81d2241", "2c65a8ca0a8d54fbfb6f621fb9a688638afa629403d8b77b4160e76315ae40ae"),
    "checkpoint1-heldout-v1.rs": ("cac3ef16eda5e2c04c505365c8603c9928873f20706220760c0c14211cb43d3a", "5644ed70d4b6e649912dab93f98516429f656266b710240021ca846102df52e4"),
    "checkpoint1-partial-peak-v1.rs": ("c02e047c8e321cada6385068fc30992aeb882abf76a9c3cf8a67a9c54540f9f7", "f9ccf15c70fb72ee43e79cc60f79b9d60b8e20bfe70d5d2c3f3980d0045eab41"),
    "checkpoint1-early-denial-v1.rs": ("f451212b3730aab27ed8739c88358b967eb24966eb182378dec57cf6b6be76ae", "34c1ff7971becd92e320592c700a8a5f5908fca5767c08f38a9be6fbb8f8ef34"),
    # This member already has the separately named resource adapter, unchanged.
    "old-ir-export-v1.rs": ("7c18f7264596a08f2ee6cf365933243301e00cc838e1d80e60f436ae340b59b5",) * 2,
}
REPLACEMENTS = {
    "reviewer": (
        (b"RawOwnedProgram {\n            records:", b"RawOwnedProgram {\n            enums: vec![],\n            records:", 5),
        (b"RawOwnedProgram {\n        records:", b"RawOwnedProgram {\n        enums: vec![],\n        records:", 1),
        (b"        loans: vec![],\n        entry:", b"        loans: vec![],\n        matches: vec![],\n        entry:", 1),
        (b"        AggregateTy::Record(_) => (hir::Ty::I32, 1),",
         b'        AggregateTy::Record(_) => (hir::Ty::I32, 1),\n        AggregateTy::Enum(_) => panic!("Unit2D non-enum fixture received an enum"),', 1),
        (b"    let other_constructor = match other {",
         b'    let other_constructor = match other {\n        AggregateTy::Enum(_) => panic!("Unit2D non-enum fixture received an enum"),', 1),
    ),
    "checkpoint1-heldout-v1.rs": ((b"RawOwnedProgram{records:vec![],functions}",
                                  b"RawOwnedProgram{enums:vec![],records:vec![],functions}", 1),),
    "checkpoint1-partial-peak-v1.rs": ((b"RawOwnedProgram{records:vec![],functions:vec![f]}",
                                       b"RawOwnedProgram{enums:vec![],records:vec![],functions:vec![f]}", 1),),
    "checkpoint1-early-denial-v1.rs": ((b"RawOwnedProgram { records: vec![], functions: vec![f] }",
                                       b"RawOwnedProgram { enums: vec![], records: vec![], functions: vec![f] }", 1),),
    "old-ir-export-v1.rs": (),
}

# Exactly four seams in the frozen Python runner: reviewer bytes, control bytes,
# source-binding metadata, and binding admission. Marker creation and all phases
# remain the original code, now binding the current derived bytes from the start.
RUNNER_REPLACEMENTS = (
    (b'        (self.source / MODULE_REL).write_bytes(current)',
     b'        current = enum_carrier_bytes("reviewer", current)\n        (self.source / MODULE_REL).write_bytes(current)', 1),
    (b'            appendix += control + b"\\n"',
     b'            control = enum_carrier_bytes(name, control)\n            appendix += control + b"\\n"', 1),
    (b'                       module_sha256=sha(current), public_array_activation=PUBLIC_ARRAY_ACTIVATION,',
     b'                       enum_carrier_compatibility=ENUM_CARRIER_COMPATIBILITY,\n                       enum_adapter_sha256=ENUM_ADAPTER_SHA,\n                       module_sha256=sha(current), public_array_activation=PUBLIC_ARRAY_ACTIVATION,', 1),
    (b'            and binding.get("module_sha256") == PROJECTED_LOAN_COMPATIBILITY["current_module_sha256"],',
     b'            and binding.get("enum_carrier_compatibility") == ENUM_CARRIER_COMPATIBILITY\n            and binding.get("enum_adapter_sha256") == ENUM_ADAPTER_SHA\n            and binding.get("module_sha256") == ENUM_CARRIER_COMPATIBILITY["members"]["reviewer"][1],', 1),
)

STDIN_VERSION = "unit2d-builtin-none-carriers-v1"
STDIN_RUNNER_SHA = "02fa2da3c2406eb10c14f4acf513da6e5e12a735f946215005342633392ee430"
STDIN_IDENTITIES = {
    "reviewer": (IDENTITIES["reviewer"][1], "193ac9c0950e95c8130104a58017ec5bb85474a249e77b324c2e6c1e4333eaf5"),
    "checkpoint1-heldout-v1.rs": (IDENTITIES["checkpoint1-heldout-v1.rs"][1], "ef7e30f0ee34ee7c4f81564ab71bc1b53a6aea88782c429d1570fe6b534b123a"),
    "checkpoint1-partial-peak-v1.rs": (IDENTITIES["checkpoint1-partial-peak-v1.rs"][1], "e6861cc0d94afaa7146145b3c91114244453227630b9266406d4a0858d9279d5"),
    "checkpoint1-early-denial-v1.rs": (IDENTITIES["checkpoint1-early-denial-v1.rs"][1], "1779a39d57137e31775101724371da02b078a71db7bdbec14f2e16faeeea32ef"),
    "old-ir-export-v1.rs": (IDENTITIES["old-ir-export-v1.rs"][1],) * 2,
}
STDIN_REPLACEMENTS = {
    "reviewer": (
        (b"RawOwnedProgram {\n            enums:",
         b"RawOwnedProgram {\n            builtins: BuiltinOrigins::None,\n            enums:", 5),
        (b"RawOwnedProgram {\n        enums:",
         b"RawOwnedProgram {\n        builtins: BuiltinOrigins::None,\n        enums:", 1),
    ),
    "checkpoint1-heldout-v1.rs": ((b"RawOwnedProgram{enums:vec![],records:vec![],functions}",
                                  b"RawOwnedProgram{builtins:BuiltinOrigins::None,enums:vec![],records:vec![],functions}", 1),),
    "checkpoint1-partial-peak-v1.rs": ((b"RawOwnedProgram{enums:vec![],records:vec![],functions:vec![f]}",
                                       b"RawOwnedProgram{builtins:BuiltinOrigins::None,enums:vec![],records:vec![],functions:vec![f]}", 1),),
    "checkpoint1-early-denial-v1.rs": ((b"RawOwnedProgram { enums: vec![], records: vec![], functions: vec![f] }",
                                       b"RawOwnedProgram { builtins: BuiltinOrigins::None, enums: vec![], records: vec![], functions: vec![f] }", 1),),
    "old-ir-export-v1.rs": (),
}
STDIN_RUNNER_REPLACEMENTS = (
    (b'        current = enum_carrier_bytes("reviewer", current)',
     b'        current = enum_carrier_bytes("reviewer", current)\n        current = stdin_carrier_bytes("reviewer", current)', 1),
    (b'            control = enum_carrier_bytes(name, control)',
     b'            control = enum_carrier_bytes(name, control)\n            control = stdin_carrier_bytes(name, control)', 1),
    (b'                       enum_adapter_sha256=ENUM_ADAPTER_SHA,',
     b'                       enum_adapter_sha256=ENUM_ADAPTER_SHA,\n                       stdin_carrier_compatibility=STDIN_CARRIER_COMPATIBILITY,\n                       stdin_adapter_sha256=STDIN_ADAPTER_SHA,', 1),
    (b'            and binding.get("module_sha256") == ENUM_CARRIER_COMPATIBILITY["members"]["reviewer"][1],',
     b'            and binding.get("stdin_carrier_compatibility") == STDIN_CARRIER_COMPATIBILITY\n            and binding.get("stdin_adapter_sha256") == STDIN_ADAPTER_SHA\n            and binding.get("module_sha256") == STDIN_CARRIER_COMPATIBILITY["members"]["reviewer"][1],', 1),
)


# The historical oracle covers i32/bool/unit only. Explicit fail-closed arms
# restore exhaustiveness without defining u8 expectations or changing its cases.
PRE_U8_VERSION = "unit2d-pre-u8-oracle-exclusion-v1"
PRE_U8_IDENTITIES = ('193ac9c0950e95c8130104a58017ec5bb85474a249e77b324c2e6c1e4333eaf5', 'aa90cb3dd0f5413d4c79775b45c9f0e803ce9474d9893a9f1cf09ce094aace51')
PRE_U8_RUNNER_SHA = '1bfb2a154cf3bab3b89fd20ad2456ddedc7203d39dc56b0a1db6d60925f4dc6e'
PRE_U8_REPLACEMENTS = (
    (b'fn literal(id: usize, value: Scalar, span: Span) -> OwnedStatement {\n    let value = match value {',
     b'fn literal(id: usize, value: Scalar, span: Span) -> OwnedStatement {\n    let value = match value {\n        Scalar::U8(_) => panic!("Unit2D pre-u8 oracle received u8"),', 1),
    (b'fn value(ty: hir::Ty, j: usize) -> Scalar {\n    match ty {',
     b'fn value(ty: hir::Ty, j: usize) -> Scalar {\n    match ty {\n        hir::Ty::U8 => panic!("Unit2D pre-u8 oracle received u8"),', 1),
    (b'fn replacement(ty: hir::Ty) -> Scalar {\n    match ty {',
     b'fn replacement(ty: hir::Ty) -> Scalar {\n    match ty {\n        hir::Ty::U8 => panic!("Unit2D pre-u8 oracle received u8"),', 1),
    (b'    for v in sequence {\n        match v {',
     b'    for v in sequence {\n        match v {\n            Scalar::U8(_) => panic!("Unit2D pre-u8 oracle received u8"),', 1),
    (b'fn native_core_value(ty: hir::Ty, ordinal: usize) -> Scalar {\n    match ty {',
     b'fn native_core_value(ty: hir::Ty, ordinal: usize) -> Scalar {\n    match ty {\n        hir::Ty::U8 => panic!("Unit2D pre-u8 oracle received u8"),', 1),
    (b'fn native_core_replacement(ty: hir::Ty) -> Scalar {\n    match ty {',
     b'fn native_core_replacement(ty: hir::Ty) -> Scalar {\n    match ty {\n        hir::Ty::U8 => panic!("Unit2D pre-u8 oracle received u8"),', 1),
    (b'    let scalar_rvalue = |s| match s {',
     b'    let scalar_rvalue = |s| match s {\n        Scalar::U8(_) => panic!("Unit2D pre-u8 oracle received u8"),', 1),
    (b"fn kind_name(ty: hir::Ty) -> &'static str {\n    match ty {",
     b'fn kind_name(ty: hir::Ty) -> &\'static str {\n    match ty {\n        hir::Ty::U8 => panic!("Unit2D pre-u8 oracle received u8"),', 1),
)

PRE_U8_RUNNER_REPLACEMENTS = (
    (b'        current = stdin_carrier_bytes("reviewer", current)',
     b'        current = stdin_carrier_bytes("reviewer", current)\n        current = pre_u8_oracle_bytes(current)', 1),
    (b'                       stdin_adapter_sha256=STDIN_ADAPTER_SHA,',
     b'                       stdin_adapter_sha256=STDIN_ADAPTER_SHA,\n                       pre_u8_oracle_compatibility=PRE_U8_ORACLE_COMPATIBILITY,\n                       pre_u8_adapter_sha256=PRE_U8_ADAPTER_SHA,', 1),
    (b'            and binding.get("module_sha256") == STDIN_CARRIER_COMPATIBILITY["members"]["reviewer"][1],',
     b'            and binding.get("pre_u8_oracle_compatibility") == PRE_U8_ORACLE_COMPATIBILITY\n            and binding.get("pre_u8_adapter_sha256") == PRE_U8_ADAPTER_SHA\n            and binding.get("module_sha256") == PRE_U8_ORACLE_COMPATIBILITY["reviewer_sha256"][1],', 1),
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def exact_replace(data, identities, replacements, *, reverse=False, adapter="enum"):
    before, after = identities[::-1] if reverse else identities
    if digest(data) != before:
        raise ValueError(f"Unit2D {adapter} adapter input identity differs")
    for old, new, count in (tuple((new, old, count) for old, new, count in reversed(replacements))
                            if reverse else replacements):
        if data.count(old) != count:
            raise ValueError(f"Unit2D {adapter} adapter replacement count differs")
        data = data.replace(old, new)
    if digest(data) != after:
        raise ValueError(f"Unit2D {adapter} adapter output identity differs")
    return data


def enum_carrier_bytes(name, data, *, reverse=False):
    return exact_replace(data, IDENTITIES[name], REPLACEMENTS[name], reverse=reverse)


def current_runner_bytes(data, *, reverse=False):
    return exact_replace(data, (FROZEN_RUNNER_SHA, CURRENT_RUNNER_SHA), RUNNER_REPLACEMENTS, reverse=reverse)


def stdin_carrier_bytes(name, data, *, reverse=False):
    return exact_replace(data, STDIN_IDENTITIES[name], STDIN_REPLACEMENTS[name],
                         reverse=reverse, adapter="stdin")


def stdin_runner_bytes(data, *, reverse=False):
    return exact_replace(data, (CURRENT_RUNNER_SHA, STDIN_RUNNER_SHA), STDIN_RUNNER_REPLACEMENTS,
                         reverse=reverse, adapter="stdin")


def pre_u8_oracle_bytes(data, *, reverse=False):
    return exact_replace(data, PRE_U8_IDENTITIES, PRE_U8_REPLACEMENTS,
                         reverse=reverse, adapter="pre-u8")


def pre_u8_runner_bytes(data, *, reverse=False):
    return exact_replace(data, (STDIN_RUNNER_SHA, PRE_U8_RUNNER_SHA), PRE_U8_RUNNER_REPLACEMENTS,
                         reverse=reverse, adapter="pre-u8")


def load_runner(path=None):
    path = Path(path) if path is not None else Path(__file__).with_name("replay_fixed_array_unit2d.py")
    module = types.ModuleType("unit2d_current_pre_u8_oracle")
    module.__file__ = str(path)
    module.ENUM_CARRIER_COMPATIBILITY = {
        "adapter": VERSION, "runner_sha256": CURRENT_RUNNER_SHA,
        "members": {name: list(identities) for name, identities in IDENTITIES.items()},
    }
    module.ENUM_ADAPTER_SHA = digest(Path(__file__).read_bytes())
    module.enum_carrier_bytes = enum_carrier_bytes
    module.STDIN_CARRIER_COMPATIBILITY = {
        "adapter": STDIN_VERSION, "predecessor_adapter": VERSION,
        "predecessor_runner_sha256": CURRENT_RUNNER_SHA, "runner_sha256": STDIN_RUNNER_SHA,
        "members": {name: list(identities) for name, identities in STDIN_IDENTITIES.items()},
        "substitutions": {name: [{"old_sha256": digest(old), "new_sha256": digest(new), "count": count}
                                  for old, new, count in replacements]
                          for name, replacements in STDIN_REPLACEMENTS.items()},
    }
    module.STDIN_ADAPTER_SHA = digest(Path(__file__).read_bytes())
    module.stdin_carrier_bytes = stdin_carrier_bytes
    module.PRE_U8_ORACLE_COMPATIBILITY = {
        "adapter": PRE_U8_VERSION, "predecessor_adapter": STDIN_VERSION,
        "predecessor_runner_sha256": STDIN_RUNNER_SHA, "runner_sha256": PRE_U8_RUNNER_SHA,
        "reviewer_sha256": list(PRE_U8_IDENTITIES),
        "substitutions": [{"old_sha256": digest(old), "new_sha256": digest(new), "count": count}
                          for old, new, count in PRE_U8_REPLACEMENTS],
        "domain": ["i32", "bool", "unit"], "excluded_domain": ["u8"],
    }
    module.PRE_U8_ADAPTER_SHA = digest(Path(__file__).read_bytes())
    module.pre_u8_oracle_bytes = pre_u8_oracle_bytes
    current = pre_u8_runner_bytes(stdin_runner_bytes(current_runner_bytes(path.read_bytes())))
    exec(compile(current, str(path), "exec"), module.__dict__)
    return module


if __name__ == "__main__":
    raise SystemExit(load_runner().main())
