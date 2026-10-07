"""Focused selection, process-boundary and evidence controls for the parser gate."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import verify_bounded_typed_parser as gate


class SelectionTests(unittest.TestCase):
    def test_exact_nonzero_selection(self):
        cases, transport = gate.load_cases(gate.FIXTURES / "cases.json")
        self.assertEqual(len(cases), 139)
        self.assertEqual(len(transport), 2)
        self.assertEqual(sum(case["role"] == "comparison" for case in cases), 118)
        self.assertEqual(sum(case["role"] == "refusal" for case in cases), 21)
        self.assertEqual(gate.digest(gate.FIXTURES / "corruption_controls.py"), gate.CONTROLS_SHA256)

    def test_reject_missing_duplicate_reclassified_or_changed_inputs(self):
        original = json.loads((gate.FIXTURES / "cases.json").read_bytes())
        changes = [lambda m: m.update(cases=[]),
                   lambda m: m["cases"].pop(),
                   lambda m: m["cases"].append(copy.deepcopy(m["cases"][0])),
                   lambda m: m["cases"][1].update(name=m["cases"][0]["name"]),
                   lambda m: m["cases"][0].update(role="refusal"),
                   lambda m: m["cases"][0].update(input_hex="20"),
                   lambda m: m["cases"][0].update(input_hex="ff"),
                   lambda m: m["cases"][0].update(input_hex="20" * 129),
                   lambda m: m["cases"][0].update(parser_status="stage_pending"),
                   lambda m: m["transport"].pop(),
                   lambda m: m["transport"][0].update(input_hex="20" * 128)]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "cases.json"
            for index, change in enumerate(changes):
                with self.subTest(change=index):
                    manifest = copy.deepcopy(original)
                    change(manifest)
                    gate.save(path, manifest)
                    with self.assertRaises(ValueError):
                        gate.load_cases(path)


class SourceMembershipTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.fixture_source = self.root / "fixtures"
        shutil.copytree(gate.ROOT / "fixtures/typed-lexer-samples", self.fixture_source)
        self.snapshot = self.root / "snapshot"
        self.snapshot.mkdir()

    def test_static_siblings_preserve_exact_historical_snapshot_and_parser_closure(self):
        self.assertEqual(len(gate.MEMBERS), 21)
        self.assertEqual(len(gate.PARSER_MEMBERS), 13)
        self.assertTrue((self.fixture_source / "ast_static_main.ox").is_file())
        gate.copy_parser_sources(self.fixture_source, self.snapshot)
        self.assertEqual(set(gate.inventory(self.snapshot)), set(gate.MEMBERS))
        self.assertEqual(gate.inventory(self.snapshot),
                         {name: gate.digest(self.fixture_source / name) for name in gate.MEMBERS})

    def test_unrelated_sibling_is_not_silently_added_to_snapshot(self):
        (self.fixture_source / "unregistered.ox").write_text("invalid candidate\n")
        gate.copy_parser_sources(self.fixture_source, self.snapshot)
        self.assertEqual(set(gate.inventory(self.snapshot)), set(gate.MEMBERS))

    def test_missing_or_symlinked_historical_source_fails_before_copy(self):
        for name in gate.MEMBERS:
            with self.subTest(name=name):
                source = self.fixture_source / name
                body = source.read_bytes()
                source.unlink()
                with self.assertRaisesRegex(ValueError, "missing or not regular"):
                    gate.copy_parser_sources(self.fixture_source, self.snapshot)
                self.assertEqual(list(self.snapshot.iterdir()), [])
                source.write_bytes(body)
        source = self.fixture_source / "buffers.ox"
        target = self.root / "buffers.ox"
        source.rename(target)
        try:
            source.symlink_to(target)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "missing or not regular"):
            gate.copy_parser_sources(self.fixture_source, self.snapshot)

    def test_historical_registration_cannot_expand_or_drop_members(self):
        prefix = "fixtures/typed-lexer-samples/"
        for root, members in (("main.ox", (prefix + "static_main.ox",)),
                              ("main.ox", ()),
                              ("parser_main.ox", (prefix + "parser_main.ox",)),
                              ("parser_main.ox", gate.verify_repo.TYPED_PROJECTS[prefix + "parser_main.ox"]
                               + (prefix + "tape.ox",))):
            registry = dict(gate.verify_repo.TYPED_PROJECTS)
            registry[prefix + root] = members
            with self.subTest(root=root, members=members), \
                    patch.object(gate.verify_repo, "TYPED_PROJECTS", registry):
                with self.assertRaisesRegex(ValueError, "registration changed"):
                    gate.copy_parser_sources(self.fixture_source, self.snapshot)

    def test_missing_duplicate_and_unregistered_parser_imports_are_rejected(self):
        root = self.fixture_source / "parser_main.ox"
        original = root.read_text()
        for changed in (original.replace("mod keywords;", ""),
                        original + "\nmod keywords;\n",
                        original + "\nmod ast_static_main;\n",
                        original + "\nmod unregistered;\n",
                        original + "\nmod /* comment */ unregistered /* comment */ ;\n"):
            with self.subTest(changed=changed):
                root.write_text(changed)
                with self.assertRaisesRegex(ValueError, "module closure changed"):
                    gate.copy_parser_sources(self.fixture_source, self.snapshot)
                self.assertEqual(list(self.snapshot.iterdir()), [])

    def test_child_imports_cannot_expand_the_flat_parser_closure(self):
        source = self.fixture_source / "parser_state.ox"
        source.write_text(source.read_text() + "\npub mod /* nested */ unregistered;\n")
        with self.assertRaisesRegex(ValueError, "unexpected parser child module"):
            gate.copy_parser_sources(self.fixture_source, self.snapshot)
        self.assertEqual(list(self.snapshot.iterdir()), [])

    def test_comment_trivia_cannot_change_declared_membership(self):
        root = self.fixture_source / "parser_main.ox"
        root.write_text(root.read_text().replace("mod keywords;", "mod /* comment */ keywords; ")
                        + "\n// mod unregistered;\n/* mod static_main; */\n")
        gate.copy_parser_sources(self.fixture_source, self.snapshot)


class ExecutionTests(unittest.TestCase):
    def test_exact_exit_streams_and_input_consumption(self):
        gate.check_execution(subprocess.CompletedProcess([], 0, b"OPA1", b""), b"", b"abc")
        gate.check_execution(subprocess.CompletedProcess([], 64, b"", b""), b"", b" " * 129, True)
        for code, stdout, stderr, remaining, transport in (
                (64, b"", b"", b"", False),
                (0, b"", b"error", b"", False),
                (0, b"", b"", b"abc", False),
                (0, b"", b"", b"", True),
                (64, b"partial", b"", b"", True),
                (64, b"", b"error", b"", True)):
            with self.subTest(code=code, stdout=stdout, stderr=stderr, remaining=remaining, transport=transport):
                with self.assertRaises(ValueError):
                    gate.check_execution(subprocess.CompletedProcess([], code, stdout, stderr), remaining, b"abc", transport)

    def test_read_only_observer_identity_rejects_unbound_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "observer"
            output.mkdir()
            binary = output / "canonical-parser-observer"
            binary.write_bytes(b"binary")
            entries = []
            for original, copied in (("src/frontend/parser.rs", "parser.rs"),
                                     ("tests/fixtures/bounded_typed_parser/observer/main.rs", "main.rs")):
                path = root / original
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"unchanged source")
                (output / copied).write_bytes(path.read_bytes())
                entries.append({"original_path": original, "copied_path": copied, "sha256": gate.digest(path)})
            manifest = {"source_patches": [], "files": entries[:1], "wrappers": entries[1:], "commit": "frozen"}
            receipt = {"exit_code": 0, "binary": binary.name, "binary_sha256": gate.digest(binary)}
            builder = SimpleNamespace(SOURCE_FILES=("parser.rs",), WRAPPER_FILES=(("main.rs", "main.rs"),))
            gate.save(output / "source-manifest.json", manifest)
            gate.save(output / "build-evidence.json", receipt)
            with patch.object(gate, "ROOT", root):
                self.assertEqual(gate.observer_identity(binary, builder, "parser")["source_commit"], "frozen")
                for field, value in (("files", []), ("wrappers", []), ("source_patches", ["patch"])):
                    with self.subTest(field=field):
                        gate.save(output / "source-manifest.json", {**manifest, field: value})
                        with self.assertRaises(ValueError):
                            gate.observer_identity(binary, builder, "parser")
                gate.save(output / "source-manifest.json", manifest)
                (output / "parser.rs").write_bytes(b"altered")
                with self.assertRaises(ValueError):
                    gate.observer_identity(binary, builder, "parser")
                (output / "parser.rs").write_bytes(b"unchanged source")
                binary.write_bytes(b"other binary")
                with self.assertRaises(ValueError):
                    gate.observer_identity(binary, builder, "parser")


class CompletionTests(unittest.TestCase):
    def setUp(self):
        self.cases, _ = gate.load_cases(gate.FIXTURES / "cases.json")
        self.modes = ["reference", "native"]
        self.report = {
            "checks": [{"case": case["name"], "mode": mode, "role": case["role"], "passed": True}
                       for case in self.cases for mode in self.modes],
            "transport": [{"case": name, "mode": mode, "passed": True}
                          for name in ("ascii129", "non_ascii_byte") for mode in self.modes],
            "controls": [{"case": str(index), "mode": mode, "passed": True}
                         for index in range(41) for mode in self.modes]}

    def test_complete_counts(self):
        gate.completed(self.report, self.cases, self.modes)
        self.assertEqual(self.report["counts"]["native"], {
            "comparison": 118, "refusal": 21, "pending": 0, "transport": 2, "corruption_controls": 41})

    def test_partial_duplicate_failed_and_wrong_mode_never_pass(self):
        for group in ("checks", "controls", "transport"):
            for kind in ("empty", "missing", "duplicate", "failed", "wrong_mode"):
                with self.subTest(group=group, kind=kind):
                    report = copy.deepcopy(self.report)
                    if kind == "empty":
                        report[group] = []
                    elif kind == "missing":
                        report[group].pop()
                    elif kind == "duplicate":
                        report[group][0] = copy.deepcopy(report[group][-1])
                    elif kind == "failed":
                        report[group][0]["passed"] = False
                    else:
                        report[group][0]["mode"] = "unselected"
                    with self.assertRaises(ValueError):
                        gate.completed(report, self.cases, self.modes)


if __name__ == "__main__":
    unittest.main()
