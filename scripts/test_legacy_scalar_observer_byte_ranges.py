#!/usr/bin/env python3
"""Real narrow-observer builds for byte ranges and their exact source closure."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import build_typed_parser_observer as parser
import build_typed_static_observer as static
import legacy_scalar_observer_record_patterns as adapter
import verify_bounded_typed_parser as gate

ROOT = Path(__file__).resolve().parents[1]
ADDED_SOURCES = ("experimental_builtins.rs", "oir/owned_types/resources.rs", "byte_strings.rs", "parser/characters.rs", "declaration_index/generics/nominal.rs")
MODES = (("parser", ()), ("static", ()), ("static", ("--resolve-only",)))

# Raw stdout hashes from separately built, unchanged parent158 observers
# (158f5bf670a4760be585afb104599f9e8a6c4a9e), in MODES order. These retain
# complete AST/HIR/diagnostic wire bytes rather than only success statuses.
LEGACY_WIRE = (
    ("fn f()->i32{return 7;}", (
        "69d40da83aa84d4a7d897f3cb89ce315fe5089f800e22a1d7a6454cbdb40de87",
        "ae3756b8a589ea564cc74ce23fdfa1ffef0a9f534356f8540c2dfbc0829e93f0",
        "3dde10279ba248aba91815481d52d3aa352e07796b0217cae86204dbdf386b57")),
    ("fn BorrowRange(u8:i32)->i32{return u8;}", (
        "11903bb238759bff302647f2ec2b1999107e1bee1a6e412a241cf011515dc3da",
        "58c71f9af1cc1b1a009b599c91b732a2b9d9b8733a0be19cbb8dd27fafadb5b0",
        "bc7a5b934da8f0f10e3bf7eb7b15a14fe52c7d1fddbd6a27d3c344e035f766e1")),
    ("fn f()->(){take(&bytes);return;}", (
        "9fba92fd209f7455d8f9f62489c55d7eb9c2d794cfeef83d928de8266335d133",
        "b429993fce10c9e91c3b8f4425e17908e551a350a34a29a9e331d4eca7c8bdf7",
        "7b3315a9a3154f8fd5da51fb9dca47cddd027a08b0df21338a6d70fd4bfae037")),
    ("fn f()->i32{return missing;}", (
        "a54960df3ca3ce662fc0c2c2a22ac8bcc4f0d723e4781fa9a22c11f024d96aad",
        "7b7efa8f5502d06e1362fd2da650844239def6eed884049b0440317d655f301e",
        "3ef0dfa60b2fde25be44d7667cd032205eca8b5610fd88ab5259f3e1bd111e0e")),
)


class ByteRangeObserverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(temporary.cleanup)
        cls.root = Path(temporary.name)
        cls.binaries = {}
        for kind, builder in (("parser", parser), ("static", static)):
            cls.binaries[kind] = Path(builder.build(cls.root / kind)["observer"])

    def observe(self, kind, source, args=()):
        result = subprocess.run([str(self.binaries[kind]), *args], input=source.encode(),
                                capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b"")
        return result.stdout

    def test_shared_mutable_forwarded_and_empty_ranges_are_outside_scalar_schema(self):
        for argument in ("&bytes[0..1]", "&mut bytes[1..2]", "&*bytes[0..1]", "&bytes[0..0]"):
            source = "fn f()->(){take(" + argument + ");return;}"
            for kind, args in MODES:
                with self.subTest(argument=argument, kind=kind, args=args):
                    value = json.loads(self.observe(kind, source, args))
                    self.assertEqual((value["status"], value["family"]),
                                     ("outside_subset", "borrow_argument"))
                    self.assertEqual(value["span"], {"file_id": 0, "start": source.index(argument),
                                                     "end": source.index(argument) + len(argument)})
                    self.assertFalse({"ast", "typed_hir", "resolved_hir"} & value.keys())

    def test_byte_strings_are_explicitly_outside_every_legacy_scalar_domain(self):
        for literal in ('b""', 'b"Hello"', r'b"A\0\xFF"', r'b"\u{1F44B}"'):
            source = "fn f()->(){let bytes=" + literal + ";return;}"
            for kind, args in MODES:
                with self.subTest(literal=literal, kind=kind, args=args):
                    value = json.loads(self.observe(kind, source, args))
                    self.assertEqual((value["status"], value["family"]),
                                     ("outside_subset", "byte_string"))
                    self.assertEqual(value["span"], {"file_id": 0, "start": source.index(literal),
                                                     "end": source.index(literal) + len(literal)})
                    self.assertFalse({"ast", "typed_hir", "resolved_hir"} & value.keys())

    def test_parent_scalar_and_borrow_wire_outputs_remain_byte_identical(self):
        for source, expected in LEGACY_WIRE:
            for (kind, args), digest in zip(MODES, expected):
                with self.subTest(source=source, kind=kind, args=args):
                    self.assertEqual(hashlib.sha256(self.observe(kind, source, args)).hexdigest(), digest)

    def test_i64_literals_are_explicitly_outside_every_legacy_scalar_domain(self):
        for literal in ('0i64', '9223372036854775807i64', '-9223372036854775808i64'):
            source = "fn f()->i64{return " + literal + ";}"
            for kind, args in MODES:
                with self.subTest(literal=literal, kind=kind, args=args):
                    value = json.loads(self.observe(kind, source, args))
                    self.assertEqual((value["status"], value["family"]),
                                     ("outside_subset", "i64_literal"))
                    self.assertEqual(value["span"], {"file_id": 0, "start": source.index(literal),
                                                     "end": source.index(literal) + len(literal)})
                    self.assertFalse({"ast", "typed_hir", "resolved_hir"} & value.keys())

    def test_new_integer_literals_are_outside_every_legacy_scalar_domain(self):
        for literal in ("-128i8", "255u8", "-32768i16", "65535u16", "4294967295u32",
                        "18446744073709551615u64", "-9223372036854775808isize",
                        "18446744073709551615usize"):
            source = "fn f()->i32{let value=" + literal + ";return 0;}"
            for kind, args in MODES:
                with self.subTest(literal=literal, kind=kind, args=args):
                    value = json.loads(self.observe(kind, source, args))
                    self.assertEqual((value["status"], value["family"]),
                                     ("outside_subset", "integer_literal"))
                    self.assertEqual(value["span"], {"file_id": 0, "start": source.index(literal),
                                     "end": source.index(literal) + len(literal)})

    def test_float_literals_are_explicitly_outside_the_retained_scalar_schema(self):
        for literal in ("0.1", "-0.5f32", "1e-3", "1e+2f64"):
            source = "fn f()->i32{let value=" + literal + ";return 0;}"
            for kind, args in MODES:
                with self.subTest(literal=literal, kind=kind, args=args):
                    value = json.loads(self.observe(kind, source, args))
                    self.assertEqual((value["status"], value["family"]), ("outside_subset", "float_literal"))
                    self.assertEqual(value["span"], {"file_id": 0, "start": source.index(literal), "end": source.index(literal)+len(literal)})

    def test_record_patterns_stay_outside_the_retained_scalar_schema(self):
        statement = "let Cell { value: item } = source;"
        source = "fn f()->(){" + statement + "return;}"
        for kind, args in MODES:
            with self.subTest(kind=kind, args=args):
                value = json.loads(self.observe(kind, source, args))
                self.assertEqual((value["status"], value["family"]),
                                 ("outside_subset", "record_pattern"))
                self.assertEqual(value["span"], {"file_id": 0,
                                 "start": source.index(statement),
                                 "end": source.index(statement) + len(statement)})
                self.assertFalse({"ast", "typed_hir", "resolved_hir"} & value.keys())

    def test_new_modules_are_exact_copies_and_wrapper_pins_are_unchanged(self):
        for kind, builder in (("parser", parser), ("static", static)):
            binary = self.binaries[kind]
            manifest = json.loads((binary.parent / "source-manifest.json").read_text())
            gate.observer_identity(binary, builder, kind)
            self.assertEqual(manifest["source_patches"], [])
            self.assertEqual(manifest["wrapper_adaptation"], adapter.SCHEMA)
            for name in ADDED_SOURCES:
                self.assertEqual(builder.SOURCE_FILES.count(name), 1)
                row, = [entry for entry in manifest["files"]
                        if entry["original_path"] == "src/frontend/" + name]
                original = (ROOT / row["original_path"]).read_bytes()
                self.assertEqual((binary.parent / row["copied_path"]).read_bytes(), original)
                self.assertEqual(row["sha256"], hashlib.sha256(original).hexdigest())
                self.assertIs(row["unchanged"], True)
            for row, (name, _) in zip(manifest["wrappers"], builder.WRAPPER_FILES):
                self.assertEqual(row["original_sha256"], adapter.PINS[kind + "/" + name])
                self.assertEqual(row["sha256"], adapter.DERIVED_PINS[kind + "/" + name])

    def test_missing_each_new_module_fails_real_rustc(self):
        for kind, builder in (("parser", parser), ("static", static)):
            for index, missing in enumerate(ADDED_SOURCES):
                with self.subTest(kind=kind, missing=missing):
                    output = self.root / f"{kind}-missing-{index}"
                    roster = tuple(name for name in builder.SOURCE_FILES if name != missing)
                    with patch.object(builder, "SOURCE_FILES", roster):
                        with self.assertRaisesRegex(ValueError, "rustc failed"):
                            builder.build(output)
                    self.assertIn(Path(missing).stem.encode(), (output / "build.stderr").read_bytes())
                    receipt = json.loads((output / "build-evidence.json").read_text())
                    self.assertNotEqual(receipt["exit_code"], 0)

    def test_source_selection_and_coherently_rehashed_module_changes_are_rejected(self):
        for kind, builder in (("parser", parser), ("static", static)):
            binary = self.binaries[kind]
            path = binary.parent / "source-manifest.json"
            original_manifest = path.read_bytes()
            baseline = json.loads(original_manifest)
            for name in ADDED_SOURCES:
                try:
                    omitted = copy.deepcopy(baseline)
                    omitted["files"] = [row for row in omitted["files"]
                                        if row["original_path"] != "src/frontend/" + name]
                    path.write_text(json.dumps(omitted))
                    with self.assertRaisesRegex(ValueError, "source selection changed"):
                        gate.observer_identity(binary, builder, kind)
                    changed = copy.deepcopy(baseline)
                    row = next(row for row in changed["files"]
                               if row["original_path"] == "src/frontend/" + name)
                    copied = binary.parent / row["copied_path"]
                    original_source = copied.read_bytes()
                    try:
                        altered = original_source + b"\n// unreviewed copied-source edit\n"
                        copied.write_bytes(altered)
                        row["sha256"] = hashlib.sha256(altered).hexdigest()
                        row["byte_count"] = len(altered)
                        path.write_text(json.dumps(changed))
                        with self.assertRaisesRegex(ValueError, "source identity mismatch"):
                            gate.observer_identity(binary, builder, kind)
                    finally:
                        copied.write_bytes(original_source)
                finally:
                    path.write_bytes(original_manifest)


if __name__ == "__main__":
    unittest.main()
