#!/usr/bin/env python3
"""Current Unit2D enum-carrier adapter; frozen runner and inputs stay unchanged.

Only the old non-enum harness receives empty carriers. Its two aggregate matches
reject enums explicitly. Every transformation pins both byte identities and is
reversible; no assertion, expectation, test selection, or compiler file changes.
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


def digest(data):
    return hashlib.sha256(data).hexdigest()


def exact_replace(data, identities, replacements, *, reverse=False):
    before, after = identities[::-1] if reverse else identities
    if digest(data) != before:
        raise ValueError("Unit2D enum adapter input identity differs")
    for old, new, count in (tuple((new, old, count) for old, new, count in reversed(replacements))
                            if reverse else replacements):
        if data.count(old) != count:
            raise ValueError("Unit2D enum adapter replacement count differs")
        data = data.replace(old, new)
    if digest(data) != after:
        raise ValueError("Unit2D enum adapter output identity differs")
    return data


def enum_carrier_bytes(name, data, *, reverse=False):
    return exact_replace(data, IDENTITIES[name], REPLACEMENTS[name], reverse=reverse)


def current_runner_bytes(data, *, reverse=False):
    return exact_replace(data, (FROZEN_RUNNER_SHA, CURRENT_RUNNER_SHA), RUNNER_REPLACEMENTS, reverse=reverse)


def load_runner(path=None):
    path = Path(path) if path is not None else Path(__file__).with_name("replay_fixed_array_unit2d.py")
    module = types.ModuleType("unit2d_current_enum_carriers")
    module.__file__ = str(path)
    module.ENUM_CARRIER_COMPATIBILITY = {
        "adapter": VERSION, "runner_sha256": CURRENT_RUNNER_SHA,
        "members": {name: list(identities) for name, identities in IDENTITIES.items()},
    }
    module.ENUM_ADAPTER_SHA = digest(Path(__file__).read_bytes())
    module.enum_carrier_bytes = enum_carrier_bytes
    exec(compile(current_runner_bytes(path.read_bytes()), str(path), "exec"), module.__dict__)
    return module


if __name__ == "__main__":
    raise SystemExit(load_runner().main())
