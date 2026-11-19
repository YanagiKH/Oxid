"""Register real compiler modules separately from authenticated subject data."""
from contextlib import ExitStack
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import tempfile
import tomllib
import unittest
from unittest.mock import patch

import verify_native_compiler_fixture_data as data
import verify_repo


ROOT = Path(__file__).resolve().parents[1]
PROJECT = "compiler/native"
MEMBERS = (
    "compiler/native/src/main.ox", "compiler/native/src/model.ox",
    "compiler/native/src/lexer.ox", "compiler/native/src/headers.ox",
    "compiler/native/src/parser.ox", "compiler/native/src/emit.ox",
    "compiler/native/src/delivery.ox",
)
REJECT_NAMES = {
    "late-type", "late-syntax", "unreachable-error", "immutable", "active-shadow", "escaped-scope",
    "function-shadow", "call-type", "call-arity", "condition-type", "break-outside", "missing-return",
    "positive-minimum-magnitude", "missing-main", "main-argument", "recursive", "indirect-recursive",
    "unsupported-array", "unsupported-import", "unterminated-comment",
}


def load_python(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def only_compiler_registration():
    """Project unrelated existing registrations out of temporary-root controls."""
    stack = ExitStack()
    for name, value in (
        ("TYPED_SOURCE_FILES", ()), ("TYPED_CHECK_ONLY_FILES", ()), ("TYPED_CHECK_ONLY_PROJECTS", ()),
        ("TYPED_PROJECT_SHARED_MEMBERS", {}), ("TYPED_CHECK_ONLY_MANIFEST_PROJECTS", (PROJECT,)),
        ("TYPED_TESTED_MANIFEST_PROJECTS", ()),
    ):
        stack.enter_context(patch.object(verify_repo, name, value))
    # Temporary roots contain only this compiler and its authenticated input data.
    # Keep its real reader active while removing the six unrelated data families.
    for reader in (
        "predecessor_fixture_data_sources", "byte_storage_fixture_data_sources",
        "typed_vector_fixture_data_sources", "copy_record_fixture_data_sources",
        "owned_record_vector_fixture_data_sources", "owned_text_fixture_data_sources",
    ):
        stack.enter_context(patch.object(verify_repo, reader, return_value=set()))
    stack.enter_context(patch.dict(verify_repo.TYPED_PROJECTS, {}, clear=True))
    stack.enter_context(patch.dict(verify_repo.TYPED_MANIFEST_PROJECTS, {PROJECT: MEMBERS}, clear=True))
    return stack


class NativeCompilerRegistration(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-compiler-registration-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in (PROJECT + "/oxid.toml", data.CASES_JSON[0],
                     *(row[0] for row in data.FIXTURE_INPUTS), *MEMBERS):
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((ROOT / name).read_bytes())

    def discover(self):
        return sorted(self.root.rglob("*.ox"))

    def test_seven_manifest_modules_match_the_actual_declaration_closure(self):
        self.assertEqual(verify_repo.TYPED_MANIFEST_PROJECTS[PROJECT], MEMBERS)
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_MANIFEST_PROJECTS.count(PROJECT), 1)
        self.assertNotIn(PROJECT, verify_repo.TYPED_TESTED_MANIFEST_PROJECTS)
        actual = {(ROOT / PROJECT / "src" / name).relative_to(ROOT).as_posix()
                  for name in ("main.ox", *(
                      name + ".ox" for name in re.findall(
                          r"(?m)^mod ([a-z_]+);$", (ROOT / MEMBERS[0]).read_text())))}
        self.assertEqual(actual, set(MEMBERS))
        self.assertEqual({path.relative_to(ROOT).as_posix()
                          for path in (ROOT / PROJECT / "src").rglob("*.ox")}, set(MEMBERS))
        manifest = tomllib.loads((ROOT / PROJECT / "oxid.toml").read_text())
        self.assertEqual(manifest["project"]["entry"], "src/main.ox")
        self.assertEqual(manifest["build"]["entry-mode"], "process")
        self.assertEqual(manifest["execution"]["policy"], "hosted")

    def test_all_39_subject_cases_and_all_20_rejections_remain_active_data(self):
        cases, sources = data.load_cases(self.root)
        self.assertEqual(len(cases), 42)
        self.assertEqual(set(sources), {case["name"] for case in cases})
        self.assertEqual({case["name"] for case in cases if case["kind"] == "reject"}, REJECT_NAMES)
        self.assertEqual({kind: sum(case["kind"] == kind for case in cases)
                          for kind in ("value", "reject", "runtime")},
                         {"value": 13, "reject": 20, "runtime": 9})
        self.assertEqual(len([case for case in cases if "file" in case]), 11)
        self.assertEqual(data.fixture_data_sources(self.root),
                         {self.root / row[0] for row in data.FIXTURE_INPUTS})
        for case in cases:
            expected = ((self.root / data.FIXTURE_ROOT / case["file"]).read_bytes()
                        if "file" in case else case["source"].encode())
            self.assertEqual(sources[case["name"]], expected)

    def test_all_twelve_authenticated_files_require_exact_presence_and_bytes(self):
        for name, length, digest in (data.CASES_JSON, *data.FIXTURE_INPUTS):
            path = self.root / name
            body = path.read_bytes()
            self.assertEqual((len(body), hashlib.sha256(body).hexdigest()), (length, digest))
            with self.subTest(missing=name):
                path.unlink()
                with self.assertRaisesRegex(RuntimeError, "unreadable"):
                    data.fixture_data_sources(self.root)
                path.write_bytes(body)
            with self.subTest(changed=name):
                path.write_bytes(b"!" + body[1:])
                with self.assertRaisesRegex(RuntimeError, "identity"):
                    data.fixture_data_sources(self.root)
                path.write_bytes(body)

    def test_fixture_identity_inventory_cannot_shrink_expand_or_escape(self):
        for changed in (data.FIXTURE_INPUTS[:-1], data.FIXTURE_INPUTS + (data.FIXTURE_INPUTS[0],),
                        (("../unregistered.ox", 1, "0" * 64), *data.FIXTURE_INPUTS[1:])):
            with self.subTest(bank=changed), patch.object(data, "FIXTURE_INPUTS", changed):
                with self.assertRaisesRegex(RuntimeError, "inventory"):
                    data.fixture_data_sources(self.root)
        with patch.object(data, "CASES_JSON", ("compiler/native/tests/unregistered.json", 1, "0" * 64)):
            with self.assertRaisesRegex(RuntimeError, "manifest identity"):
                data.fixture_data_sources(self.root)

    def test_case_schema_rejects_missing_duplicate_retyped_or_unregistered_inputs(self):
        inputs = data.authenticated_inputs(self.root, required=True)
        cases = json.loads(inputs[data.CASES_JSON[0]])
        changed_cases = [cases[:-1], [cases[0], *cases[1:-1], cases[0]]]
        for index, updates in ((0, {"file": "../outside.ox"}),
                               (0, {"reference": 1}), (0, {"kind": "reject"}),
                               (0, {"source": "fn main()->i32{return 0;}"})):
            changed = json.loads(json.dumps(cases))
            changed[index].update(updates)
            changed_cases.append(changed)
        for changed in changed_cases:
            with self.subTest(cases=changed):
                body = json.dumps(changed).encode()
                with self.assertRaises(RuntimeError):
                    data.parse_cases({**inputs, data.CASES_JSON[0]: body})
        raw = inputs[data.CASES_JSON[0]].replace(b'"name":"forward"', b'"name":"forward","name":"forward"', 1)
        self.assertNotEqual(raw, inputs[data.CASES_JSON[0]])
        with self.assertRaisesRegex(RuntimeError, "duplicate"):
            data.parse_cases({**inputs, data.CASES_JSON[0]: raw})

    def test_symlink_executable_and_linked_data_are_denied(self):
        path = self.root / data.FIXTURE_INPUTS[0][0]
        body = path.read_bytes()
        original_mode = path.stat().st_mode
        path.chmod(original_mode | 0o100)
        try:
            with self.assertRaisesRegex(RuntimeError, "nonregular"):
                data.fixture_data_sources(self.root)
        finally:
            path.chmod(original_mode)
        linked = self.root / "linked-data"
        try:
            os.link(path, linked)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"hard links unavailable: {error}")
        with self.assertRaisesRegex(RuntimeError, "nonregular"):
            data.fixture_data_sources(self.root)
        linked.unlink()
        path.unlink()
        try:
            path.symlink_to(ROOT / data.FIXTURE_INPUTS[0][0])
        except (OSError, NotImplementedError) as error:
            path.write_bytes(body)
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(RuntimeError, "symlink"):
            data.fixture_data_sources(self.root)

    def test_only_the_exact_manifest_is_checked_and_no_compiler_entry_is_run(self):
        with only_compiler_registration():
            checks, entries, members = verify_repo.source_plan(self.discover(), self.root)
        self.assertEqual(checks, [(self.root / PROJECT, True)])
        self.assertEqual(entries, [])
        self.assertEqual(members, 7)

    def test_unlisted_source_and_fixture_neighbors_keep_ordinary_legacy_checks(self):
        neighbors = ("compiler/native/src/unregistered.ox", "compiler/native/tests/fixtures/extra.ox",
                     "compiler/native-neighbor/main.ox")
        for name in neighbors:
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fn main(){print(42);}\n")
        with only_compiler_registration():
            checks, entries, members = verify_repo.source_plan(self.discover(), self.root)
        self.assertEqual({path for path, typed in checks if not typed}, {self.root / name for name in neighbors})
        self.assertEqual([path for path, typed in checks if typed], [self.root / PROJECT])
        self.assertEqual((entries, members), ([], 7))
        self.assertFalse({path for path, _ in checks} & data.fixture_data_sources(self.root))

    def test_missing_project_members_and_complete_project_disappearance_are_denied(self):
        with only_compiler_registration():
            for name in MEMBERS:
                with self.subTest(member=name), self.assertRaisesRegex(RuntimeError, "missing"):
                    verify_repo.manifest_project_members(
                        [path for path in self.discover() if path != self.root / name], self.root, required=True)
            with self.assertRaisesRegex(RuntimeError, "missing"):
                verify_repo.manifest_project_members([], self.root, required=True)
        for name in (data.CASES_JSON[0], *(row[0] for row in data.FIXTURE_INPUTS)):
            (self.root / name).unlink()
        with self.assertRaisesRegex(RuntimeError, "unreadable"):
            data.fixture_data_sources(self.root)

    def test_root_legacy_test_exclusions_stay_exact_and_need_no_compiler_directory_rule(self):
        configured = tomllib.loads((ROOT / "oxid.toml").read_text())["test-fixtures"]
        self.assertFalse(any(name.startswith("compiler/") for name in configured))
        verify_repo.verify_test_fixture_registration(ROOT)
        (self.root / "oxid.toml").write_text(
            '[test-fixtures]\n"compiler/native/tests/fixtures/late_type_error.ox" = true\n')
        with only_compiler_registration(), self.assertRaisesRegex(RuntimeError, "exactly match"):
            verify_repo.verify_test_fixture_registration(self.root)

    def test_exact_data_attributes_preserve_cases_and_body_bytes_across_hosts(self):
        attributes = (ROOT / ".gitattributes").read_text().splitlines()
        expected = [data.CASES_JSON[0], *(row[0] for row in data.FIXTURE_INPUTS)]
        for name in expected:
            self.assertEqual(attributes.count(name + " -text"), 1)
        self.assertFalse(any(line.startswith("compiler/") and "*" in line for line in attributes))

    def test_acceptance_authenticates_data_before_creating_evidence_or_invoking_a_process(self):
        verifier = load_python("scalar_registration_acceptance", PROJECT + "/tests/verify.py")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "evidence"
            args = type("Arguments", (), {"output": output})()
            with patch.object(verifier.fixture_data, "load_cases", side_effect=RuntimeError("unreadable fixture")), \
                    patch.object(verifier, "run") as native, patch.object(verifier.driver, "build") as build:
                with self.assertRaisesRegex(RuntimeError, "unreadable fixture"):
                    verifier.verify(args)
            native.assert_not_called()
            build.assert_not_called()
            self.assertFalse(output.exists())

    def test_absent_optional_data_is_empty_but_required_acceptance_cannot_disappear(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.assertEqual(data.fixture_data_sources(root), set())
            with self.assertRaisesRegex(RuntimeError, "unreadable"):
                data.load_cases(root)


def load_tests(loader, tests, pattern):
    # The exact existing driver controls are mandatory in normal script discovery.
    # They test framing/publication, never fake successful Oxid compilation.
    protocol = load_python("scalar_registration_protocol", PROJECT + "/tests/test_protocol.py")
    tests.addTests(loader.loadTestsFromModule(protocol))
    diagnostics = load_python("scalar_runtime_diagnostics", PROJECT + "/tests/test_runtime_diagnostics.py")
    tests.addTests(loader.loadTestsFromModule(diagnostics))
    return tests


if __name__ == "__main__":
    unittest.main()
