"""Source-view admission controls; no compiler build or semantic expectation."""
from contextlib import ExitStack
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
PACKAGE = REPO / "tests/fixtures/typed_project_source_binding"
spec = importlib.util.spec_from_file_location("source_binding", PACKAGE / "run.py")
binding = importlib.util.module_from_spec(spec)
spec.loader.exec_module(binding)


class SourceBindingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.captured = binding.preflight(REPO)

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        inputs = dict(self.captured["inputs"])
        inputs.update(self.captured["references"])
        inputs.update({binding.U2 + "/" + name: data for name, data in self.captured["historical_bytes"].items()})
        binding.materialize(self.repo, inputs)
        package_inputs = dict(self.captured["package_bytes"])
        package_inputs["package-manifest.json"] = self.captured["package_manifest"]
        self.package = self.root / "package"
        binding.materialize(self.package, package_inputs)

    def rejects(self, text):
        with self.assertRaisesRegex(binding.BindingError, text):
            binding.preflight(self.repo, self.package)

    def rehash_package(self):
        """Model coherent metadata tampering without replacing the trusted helper."""
        manifest = binding.read_json(self.package / "package-manifest.json")
        manifest["files"] = [binding.entry(row["path"], (self.package / row["path"]).read_bytes())
                             for row in manifest["files"]]
        binding.write_json(self.package / "package-manifest.json", manifest)

    def rejects_before_materialization(self, text):
        output = self.root / ("rejected-" + binding.uuid.uuid4().hex)
        sentinel = self.root / "tool-ran"
        tool = self.root / "cargo"
        tool.write_text("#!/bin/sh\ntouch '" + str(sentinel) + "'\nexit 0\n")
        tool.chmod(0o755)
        result = subprocess.run([sys.executable, "-B", str(self.package / "run.py"), "run-unit2",
                                 "--repo", str(self.repo), "--output", str(output),
                                 "--cargo", str(tool), "--rustc", str(tool)],
                                capture_output=True, text=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(text, result.stderr)
        failure = binding.read_json(output / "failure.json")
        self.assertEqual(failure["compiler_executions"], 0)
        self.assertFalse(failure["semantic_pass"])
        self.assertFalse(sentinel.exists())
        for name in ("plan.json", "compatibility", "archived-selected", "result.json"):
            self.assertFalse((output / name).exists(), name)

    def test_current_and_archived_views_are_distinct_and_exact(self):
        captured = binding.preflight(self.repo, self.package)
        self.assertEqual(len(captured["inputs"]), 237)
        self.assertEqual(len(captured["slices_inputs"]), 188)
        self.assertEqual(len(captured["division_inputs"]), 185)
        self.assertEqual(len(captured["combined_inputs"]), 185)
        self.assertEqual(len(captured["formatter_inputs"]), 133)
        self.assertEqual(len(captured["predecessor_inputs"]), 129)
        self.assertEqual(len(captured["archived"]), 117)
        self.assertNotEqual(captured["inputs"]["src/frontend/driver.rs"], captured["archived"]["src/frontend/driver.rs"])
        output = self.root / "archive"
        output.mkdir()
        receipt = binding.prepare_archived(output, captured)
        self.assertEqual(receipt["compiler_executions"], 0)
        self.assertFalse(receipt["semantic_pass"])
        self.assertEqual(receipt["division_inverse_touched"], list(binding.DIVISION_PATHS))
        self.assertEqual(receipt["division_inverse_patch_sha256"], binding.DIVISION_PATCH_SHA)
        self.assertEqual(receipt["combined_source_sha256"], binding.COMBINED_SOURCE_SHA)
        self.assertEqual(receipt["slices_inverse_touched"], list(binding.SLICES_PATHS))
        self.assertEqual(receipt["slices_inverse_patch_sha256"], binding.SLICES_PATCH_SHA)
        self.assertEqual(receipt["division_source_sha256"], binding.DIVISION_SOURCE_SHA)
        binding.check_entries(output / "archived-selected", captured["selected"]["files"], exact=True)


    def test_enum_inverse_restores_exact_projected_predecessor(self):
        restored, touched = binding.inverse_enum_patch(
            self.captured["inputs"], self.captured["package_bytes"]["enum-transition.patch"])
        self.assertEqual(restored, self.captured["projected_inputs"])
        binding.check_bytes(restored, self.captured["projected_source"]["files"])
        self.assertEqual(touched, list(binding.ENUM_PATHS))
        self.assertEqual((len(touched), len(restored), len(binding.ENUM_ADDITIONS)), (102, 201, 36))
        self.assertEqual(set(self.captured["inputs"]) - set(restored), set(binding.ENUM_ADDITIONS))
        self.assertEqual(len([p for p in self.captured["inputs"] if p.startswith(("src/", "native/"))]), 179)
        self.assertEqual(self.captured["current"]["reviewed_source_head"],
                         "78651228b8233ec2cc8a4e28c2fd1e23fdcb40cd")
        self.assertEqual(self.captured["current"]["source_only_tree"],
                         "4970ee660f670cfcb23f42a9cb182a4ca7996388")
        self.assertEqual(binding.digest(self.captured["package_bytes"]["projected-source.json"]),
                         "850555bcc78b355029ed2ff0a4a094762f0ea4c0c5bcf5f728d30bbbcc213304")

    def test_enum_forward_patch_recreates_every_current_input(self):
        source = self.root / "forward-projected"
        binding.materialize(source, self.captured["projected_inputs"])
        patch_path = self.package / "enum-transition.patch"
        for extra in (['--check'], []):
            result = subprocess.run(['git', '-c', 'core.autocrlf=false', '-c', 'core.eol=lf',
                                     'apply', *extra, str(patch_path)], cwd=source,
                                    capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
        binding.check_entries(source, self.captured["current"]["files"], exact=True)

    def test_enum_members_required_byte_and_mode_bound(self):
        for name in binding.ENUM_PATHS:
            with self.subTest(name=name):
                source = self.repo / name
                raw = source.read_bytes()
                source.write_bytes(raw + b"// mutation\n")
                with patch.object(binding, "inverse_enum_patch", side_effect=AssertionError("inverse ran")):
                    self.rejects("changed input")
                source.write_bytes(raw)
        for name in binding.ENUM_ADDITIONS:
            source = self.repo / name
            raw = source.read_bytes()
            source.unlink()
            self.rejects("missing regular input")
            source.write_bytes(raw)
        if sys.platform != "win32":
            source = self.repo / binding.ENUM_SCANNER_PATHS[0]
            source.chmod(0o755)
            self.rejects("changed input mode")

    def test_enum_authority_coherent_rehash_rejects(self):
        authority = binding.read_json(self.package / "enum-authority.json")
        authority["transition_paths"] = authority["transition_paths"][:-1]
        binding.write_json(self.package / "enum-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale enum authority")

    def test_projected_predecessor_manifest_coherent_rehash_rejects(self):
        source = self.package / "projected-source.json"
        value = binding.read_json(source)
        value["files"].pop()
        binding.write_json(source, value)
        self.rehash_package()
        self.rejects_before_materialization("unapproved projected source manifest")

    def test_enum_patch_changed_or_missing_rejects(self):
        source = self.package / "enum-transition.patch"
        source.write_bytes(source.read_bytes() + b"\n")
        self.rehash_package()
        self.rejects_before_materialization("wrong transition patch")
        source.unlink()
        self.rejects("missing or extra adapter member")

    def test_enum_inverse_context_and_double_application_reject(self):
        original = self.captured["package_bytes"]["enum-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        self.assertEqual(len(sections), 102)
        for name, section in zip(binding.ENUM_PATHS, sections):
            with self.subTest(name=name):
                inputs = dict(self.captured["inputs"])
                hunk = next(line for line in section.splitlines() if line.startswith(b"@@ "))
                offset = max(int(hunk.split(b" +", 1)[1].split(b" ", 1)[0].split(b",", 1)[0]) - 1, 0)
                lines = inputs[name].splitlines(keepends=True)
                lines[offset] = b"X" + lines[offset]
                inputs[name] = b"".join(lines)
                with self.assertRaises(binding.BindingError):
                    binding.inverse_enum_patch(inputs, original)
        with self.assertRaises(binding.BindingError):
            binding.inverse_enum_patch(self.captured["projected_inputs"], original)

    def test_enum_inverse_scope_order_duplicates_unknown_and_extra_tail_reject(self):
        original = self.captured["package_bytes"]["enum-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        unknown = sections[0].replace(binding.ENUM_PATHS[0].encode(), b"src/unknown.rs")
        for changed in (b"".join(reversed(sections)), b"".join(sections[:-1]),
                        original + sections[0], original + unknown, original + b"unexpected tail\n"):
            with self.subTest(sha=binding.digest(changed)), self.assertRaises(binding.BindingError):
                binding.apply_inverse_patch(self.captured["inputs"], changed, binding.digest(changed),
                                            len(changed), binding.ENUM_PATHS)

    def test_enum_scanner_closure_is_separate_and_ordered(self):
        closure = self.captured["enum_authority"]["scanner_include_closure"]
        self.assertEqual([row["includer"]["path"] for row in closure], list(binding.ENUM_SCANNER_INCLUDERS))
        self.assertEqual([row["ordered_references"] for row in closure], [list(binding.ENUM_SCANNER_PATHS)] * 2)
        self.assertEqual(sum(len(row["ordered_references"]) for row in closure), 4)
        self.assertEqual(len({name for row in closure for name in row["ordered_references"]}), 2)
        self.assertEqual((self.captured["current"]["compile_time_fixture_members"],
                          self.captured["current"]["compile_time_fixture_references"]), (42, 47))

    def test_enum_observer_adapter_is_exact_reversible_and_fail_closed(self):
        prior = self.captured["borrowed_observer"]
        current = binding.adapt_enum_unit2_observer(prior)
        self.assertEqual(current, self.captured["observer"])
        self.assertEqual(len(binding.ENUM_OBSERVER_SEAMS), 3)
        self.assertIn(b'AggregateTy::Enum(_) => panic!("current Unit2 observer excludes enum projection")', current)
        self.assertIn(b'BorrowedTy::Exact(AggregateTy::Enum(_)) => panic!("current Unit2 observer excludes enum projection")', current)
        self.assertEqual(current.count(b"fn current_unit2_aggregate_adapter_"), 6)
        restored = current
        for old, new in reversed(binding.ENUM_OBSERVER_SEAMS):
            self.assertEqual(restored.count(new), 1)
            restored = restored.replace(new, old)
        self.assertEqual(restored, prior)
        for changed in (prior + b"\n", prior[:-1], current):
            with self.assertRaisesRegex(binding.BindingError, "wrong predecessor enum Unit2 observer"):
                binding.adapt_enum_unit2_observer(changed)
        for index, (old, new) in enumerate(binding.ENUM_OBSERVER_SEAMS):
            seams = binding.ENUM_OBSERVER_SEAMS
            with patch.object(binding, "ENUM_OBSERVER_SEAMS", seams[:index] + ((old, new + b"// mutation"),) + seams[index + 1:]):
                with self.assertRaisesRegex(binding.BindingError, "wrong derived enum Unit2 observer"):
                    binding.adapt_enum_unit2_observer(prior)

    def test_enum_index_resource_preserves_frozen_bytes_and_exact_four_control_scope(self):
        original = self.captured["historical_bytes"][binding.INDEX_RESOURCE]
        current = binding.adapt_enum_index_resource(original)
        self.assertEqual(current, self.captured["index_resource"])
        self.assertEqual(len(binding.ENUM_INDEX_RESOURCE_SEAMS), 17)
        restored = current
        for old, new in reversed(binding.ENUM_INDEX_RESOURCE_SEAMS):
            self.assertEqual(restored.count(new), 1)
            restored = restored.replace(new, old)
        self.assertEqual(restored, original)
        import re
        def functions(raw):
            return {match[1]: match[0] for match in re.finditer(
                rb'#\[test\]\nfn (reviewer_[a-z_]+)\(\) \{.*?(?=\n#\[test\]|\Z)', raw, re.S)}
        old_functions, new_functions = functions(original), functions(current)
        self.assertEqual(set(old_functions), set(new_functions))
        self.assertEqual([name.decode() for name in old_functions if old_functions[name] != new_functions[name]],
                         list(binding.ENUM_INDEX_RESOURCE_CONTROLS))
        self.assertEqual(len(self.captured["index_resource_authority"]["preserved_test_names"]), 21)
        self.assertIn(b'fn reviewer_fourteen_actual_index_reserve_failures()', current)
        self.assertIn(b'for fail in 1..=16 {', current)
        for unchanged in (b'index.row_lengths(),[4,4,2,1,1,2,1,1,1,1]',
                          b'assert!(FIXED_SCRATCH<=4096)', b'ast_payload,28800',
                          b'plan().build_work,168', b'assert_eq!(count,usize::MAX)'):
            if unchanged in original:
                self.assertIn(unchanged, current)

    def test_enum_index_resource_derivation_matches_independent_prescribed_rows(self):
        derivation = self.captured["index_resource_authority"]["representation_derivation"]
        self.assertEqual(derivation["nine_payloads_after"], [68,120,136,184,180,288,272,340,380])
        self.assertEqual((derivation["module_row_bytes"], derivation["enum_row_bytes"],
                          derivation["variant_row_bytes"], derivation["index_header_bytes"],
                          derivation["fixed_scratch_bytes"]), (68,20,12,344,4088))
        self.assertEqual(derivation["unchanged_function_work"], {"preflight":34,"mandatory_build":168,"admission":202})
        current = self.captured["index_resource"]
        self.assertIn(b'("index enums",0,20),("index variants",0,12),("index modules",2,68)', current)
        self.assertIn(b'al.trace.iter().take(12)', current)
        self.assertIn(b'al.trace.iter().skip(12)', current)
        self.assertIn(b'let retained=120+size_of::<DeclarationIndex', current)

    def test_enum_index_resource_coherent_authority_mutations_reject_before_materialization(self):
        name = self.package / "enum-resource-authority.json"
        original = name.read_bytes()
        for field, value in (("logical_resource_tests", 20), ("changed_controls", []),
                             ("source_dependencies", []), ("preserved_test_names", [])):
            with self.subTest(field=field):
                authority = binding.read_json(name)
                authority[field] = value
                binding.write_json(name, authority)
                self.rehash_package()
                self.rejects_before_materialization("stale enum index resource authority")
                name.write_bytes(original)
        self.rehash_package()

    def test_enum_index_resource_rejects_source_tail_unknown_seams_and_reapplication(self):
        original = self.captured["historical_bytes"][binding.INDEX_RESOURCE]
        for changed in (original + b"\n", original[:-1], self.captured["index_resource"]):
            with self.assertRaisesRegex(binding.BindingError, "wrong original enum index resource"):
                binding.adapt_enum_index_resource(changed)
        seams = binding.ENUM_INDEX_RESOURCE_SEAMS
        for changed in (seams[:-1], seams + (seams[0],)):
            with patch.object(binding, "ENUM_INDEX_RESOURCE_SEAMS", changed):
                with self.assertRaisesRegex(binding.BindingError, "wrong enum index resource substitution count"):
                    binding.adapt_enum_index_resource(original)
        for index, (old, new) in enumerate(seams):
            for changed in ((b"unknown seam", new), (old, new + b"// altered")):
                with self.subTest(index=index), patch.object(binding, "ENUM_INDEX_RESOURCE_SEAMS", seams[:index] + (changed,) + seams[index+1:]):
                    with self.assertRaisesRegex(binding.BindingError, "enum index resource seam drift|wrong derived enum index resource"):
                        binding.adapt_enum_index_resource(original)

    def test_enum_semantic_comparator_is_exact_reversible_and_retains_frozen_report(self):
        original = self.captured["historical_bytes"][binding.UNIT2_COMPARATOR]
        current = binding.adapt_enum_unit2_comparator(original)
        self.assertEqual(current, self.captured["unit2_comparator"])
        restored = current
        for old, new in reversed(binding.UNIT2_COMPARATOR_SEAMS):
            self.assertEqual(restored.count(new), 1)
            restored = restored.replace(new, old)
        self.assertEqual(restored, original)
        compile(current, "current-unit2-comparator", "exec")
        self.assertIn(b'compare_before_enum_enabled_qualified_values.py', current)
        self.assertIn(b"historical_report['counts']=={'match':3599,'mismatch':4}", current)
        self.assertIn(b"report['historical_semantic_comparison']=historical_binding", current)
        output = self.root / "semantic-package"
        output.mkdir()
        seam = binding.prepare_unit2(output, self.captured)
        prepared = Path(seam["resource_package_root"])
        self.assertEqual((prepared / binding.UNIT2_FROZEN_COMPARATOR).read_bytes(), original)
        self.assertEqual((prepared / binding.UNIT2_COMPARATOR).read_bytes(), current)
        self.assertEqual((prepared / "semantic/corpus.jsonl.gz").read_bytes(),
                         self.captured["historical_bytes"]["semantic/corpus.jsonl.gz"])
        self.assertEqual(seam["semantic_amendment"]["cases"], [
            "parser/bare-relative-prefix", "parser/qualified-function-value",
            "parser/self-relative-prefix", "parser/super-relative-prefix"])

    def test_enum_semantic_helper_or_descriptor_coherent_changes_reject(self):
        for name in (binding.SEMANTIC_HELPER, binding.SEMANTIC_DESCRIPTOR):
            with self.subTest(name=name):
                path = self.package / name
                old = path.read_bytes()
                path.write_bytes(old + b"\n")
                self.rehash_package()
                self.rejects_before_materialization("unapproved enum semantic amendment")
                path.write_bytes(old)
        self.rehash_package()

    def test_enum_semantic_comparator_rejects_wrong_original_and_seams(self):
        original = self.captured["historical_bytes"][binding.UNIT2_COMPARATOR]
        for changed in (original + b"\n", self.captured["unit2_comparator"]):
            with self.assertRaisesRegex(binding.BindingError, "wrong original Unit2 comparator"):
                binding.adapt_enum_unit2_comparator(changed)
        seams = binding.UNIT2_COMPARATOR_SEAMS
        for changed in (seams[:-1], seams + (seams[0],)):
            with patch.object(binding, "UNIT2_COMPARATOR_SEAMS", changed):
                with self.assertRaisesRegex(binding.BindingError, "wrong enum Unit2 comparator substitution count"):
                    binding.adapt_enum_unit2_comparator(original)
        for index, (old, new) in enumerate(seams):
            with patch.object(binding, "UNIT2_COMPARATOR_SEAMS", seams[:index] + ((old, new + b"# altered\n"),) + seams[index+1:]):
                with self.assertRaisesRegex(binding.BindingError, "wrong derived enum Unit2 comparator"):
                    binding.adapt_enum_unit2_comparator(original)

    def test_enum_resource_adapter_is_exact_reversible(self):
        prior = self.captured["combined_resource"]
        current = self.captured["enum_resource"]
        self.assertEqual(current.replace(binding.ENUM_RESOURCE_SEAM, binding.NEW_SEAM), prior)
        self.assertEqual(current.count(binding.ENUM_RESOURCE_SEAM), 1)
        self.assertIn(b"enums:EnumSyntaxPolicy::Closed,storage:enums::SyntaxStorage::default()", current)
        self.assertEqual(self.captured["enum_authority"]["resource_adapter"]["derived"],
                         binding.entry(binding.RESOURCE, current))
        self.assertEqual(binding.adapt_stdin_parser_resource(current), self.captured["resource"])

    def test_projected_inverse_restores_exact_unary(self):
        restored, touched = binding.inverse_projected_patch(
            self.captured["projected_inputs"], self.captured["package_bytes"]["projected-transition.patch"])
        self.assertEqual(restored, self.captured["unary_inputs"])
        self.assertEqual(touched, list(binding.PROJECTED_PATHS))
        self.assertEqual(len(touched), 40)
        self.assertEqual(len(binding.PROJECTED_ADDITIONS), 2)
        self.assertEqual(len(restored), 199)
        self.assertEqual(self.captured["projected_source"]["reviewed_source_head"],
                         "052ad52ac876c01b91701132cffb466689b24d01")
        self.assertEqual(self.captured["projected_source"]["source_only_tree"],
                         "a573ca3d279bc3e14ad6da1bd84cae9917d0fe50")

    def test_projected_members_required_and_byte_bound(self):
        for name in binding.PROJECTED_PATHS:
            with self.subTest(name=name):
                source = self.repo / name
                raw = source.read_bytes()
                source.write_bytes(raw + b"// mutation\n")
                with patch.object(binding, "inverse_projected_patch", side_effect=AssertionError("inverse ran")):
                    self.rejects("changed input")
                source.write_bytes(raw)
        for name in binding.PROJECTED_ADDITIONS:
            source = self.repo / name
            raw = source.read_bytes()
            source.unlink()
            self.rejects("missing regular input")
            source.write_bytes(raw)

    def test_projected_authority_coherent_tampering_rejects(self):
        authority = binding.read_json(self.package / "projected-authority.json")
        authority["transition_paths"] = []
        binding.write_json(self.package / "projected-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale projected authority")

    def test_unary_predecessor_manifest_coherent_tampering_rejects(self):
        current = binding.read_json(self.package / "unary-source.json")
        current["files"] = current["files"][:-1]
        binding.write_json(self.package / "unary-source.json", current)
        self.rehash_package()
        self.rejects_before_materialization("unapproved unary source manifest")

    def test_projected_patch_changed_or_missing_rejects(self):
        source = self.package / "projected-transition.patch"
        source.write_bytes(source.read_bytes() + b"\n")
        self.rehash_package()
        self.rejects_before_materialization("wrong transition patch")
        source.unlink()
        self.rejects("missing or extra adapter member")

    def test_projected_inverse_context_and_double_application_reject(self):
        original = self.captured["package_bytes"]["projected-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        self.assertEqual(len(sections), 40)
        for name, section in zip(binding.PROJECTED_PATHS, sections):
            with self.subTest(name=name):
                inputs = dict(self.captured["projected_inputs"])
                hunk = next(line for line in section.splitlines() if line.startswith(b"@@ "))
                offset = max(int(hunk.split(b" +", 1)[1].split(b" ", 1)[0].split(b",", 1)[0]) - 1, 0)
                lines = inputs[name].splitlines(keepends=True)
                lines[offset] = b"X" + lines[offset]
                inputs[name] = b"".join(lines)
                with self.assertRaises(binding.BindingError):
                    binding.inverse_projected_patch(inputs, original)
        with self.assertRaises(binding.BindingError):
            binding.inverse_projected_patch(self.captured["unary_inputs"], original)

    def test_projected_inverse_scope_order_and_duplicates_reject(self):
        original = self.captured["package_bytes"]["projected-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        for changed, expected in ((b"".join(reversed(sections)), "wrong transition scope"),
                                  (b"".join(sections[:-1]), "wrong transition scope"),
                                  (original + sections[0], "duplicate transition member")):
            with self.subTest(sha=binding.digest(changed)), self.assertRaisesRegex(binding.BindingError, expected):
                binding.apply_inverse_patch(self.captured["projected_inputs"], changed, binding.digest(changed),
                                            len(changed), binding.PROJECTED_PATHS)

    def test_unary_inverse_restores_exact_composition(self):
        restored, touched = binding.inverse_unary_patch(
            self.captured["unary_inputs"], self.captured["package_bytes"]["unary-transition.patch"])
        self.assertEqual(restored, self.captured["composition_inputs"])
        self.assertEqual(touched, list(binding.UNARY_PATHS))
        self.assertEqual(len(touched), 25)
        self.assertEqual(len(binding.UNARY_ADDITIONS), 3)
        self.assertEqual(len(restored), 196)
        self.assertEqual(self.captured["unary_source"]["reviewed_source_head"],
                         "bf48512acf86e2d23c28b6b9b16de3be3d127051")
        self.assertEqual(self.captured["unary_source"]["source_only_tree"],
                         "715d047f37db8b7658bda688ff4e5961609193f8")

    def test_unary_members_required_and_byte_bound(self):
        for name in binding.UNARY_PATHS:
            with self.subTest(name=name):
                source = self.repo / name
                raw = source.read_bytes()
                source.write_bytes(raw + b"// mutation\n")
                with patch.object(binding, "inverse_unary_patch", side_effect=AssertionError("inverse ran")):
                    self.rejects("changed input")
                source.write_bytes(raw)
        for name in binding.UNARY_ADDITIONS:
            source = self.repo / name
            raw = source.read_bytes()
            source.unlink()
            self.rejects("missing regular input")
            source.write_bytes(raw)

    def test_unary_authority_coherent_tampering_rejects(self):
        authority = binding.read_json(self.package / "unary-authority.json")
        authority["transition_paths"] = []
        binding.write_json(self.package / "unary-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale unary authority")

    def test_composition_predecessor_manifest_coherent_tampering_rejects(self):
        current = binding.read_json(self.package / "composition-source.json")
        current["files"] = current["files"][:-1]
        binding.write_json(self.package / "composition-source.json", current)
        self.rehash_package()
        self.rejects_before_materialization("unapproved composition source manifest")

    def test_unary_patch_changed_or_missing_rejects(self):
        source = self.package / "unary-transition.patch"
        source.write_bytes(source.read_bytes() + b"\n")
        self.rehash_package()
        self.rejects_before_materialization("wrong transition patch")
        source.unlink()
        self.rejects("missing or extra adapter member")

    def test_unary_inverse_context_and_double_application_reject(self):
        original = self.captured["package_bytes"]["unary-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        self.assertEqual(len(sections), 25)
        for name, section in zip(binding.UNARY_PATHS, sections):
            with self.subTest(name=name):
                inputs = dict(self.captured["unary_inputs"])
                hunk = next(line for line in section.splitlines() if line.startswith(b"@@ "))
                offset = max(int(hunk.split(b" +", 1)[1].split(b" ", 1)[0].split(b",", 1)[0]) - 1, 0)
                lines = inputs[name].splitlines(keepends=True)
                lines[offset] = b"X" + lines[offset]
                inputs[name] = b"".join(lines)
                with self.assertRaises(binding.BindingError):
                    binding.inverse_unary_patch(inputs, original)
        with self.assertRaises(binding.BindingError):
            binding.inverse_unary_patch(self.captured["composition_inputs"], original)

    def test_unary_inverse_scope_order_and_duplicates_reject(self):
        original = self.captured["package_bytes"]["unary-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        for changed, expected in ((b"".join(reversed(sections)), "wrong transition scope"),
                                  (b"".join(sections[:-1]), "wrong transition scope"),
                                  (original + sections[0], "duplicate transition member")):
            with self.subTest(sha=binding.digest(changed)), self.assertRaisesRegex(binding.BindingError, expected):
                binding.apply_inverse_patch(self.captured["unary_inputs"], changed, binding.digest(changed),
                                            len(changed), binding.UNARY_PATHS)

    def test_composition_inverse_restores_exact_slice_predecessor(self):
        captured = self.captured
        restored, touched = binding.inverse_composition_patch(
            captured["composition_inputs"], captured["package_bytes"]["composition-transition.patch"])
        self.assertEqual(restored, captured["slices_inputs"])
        self.assertEqual(touched, list(binding.COMPOSITION_PATHS))
        self.assertEqual(len(touched), 40)
        self.assertEqual(len(set(touched)), 40)
        self.assertEqual(len(binding.COMPOSITION_ADDITIONS), 8)
        self.assertEqual(set(captured["composition_inputs"]) - set(restored), set(binding.COMPOSITION_ADDITIONS))
        binding.check_bytes(restored, captured["slices_source"]["files"])
        self.assertEqual(binding.digest(captured["package_bytes"]["slices-source.json"]),
                         "f3fcde4169957c850dfe14491b0ddc4fcc6e75ac0ba81fccb4b3ebe9041c6660")
        self.assertEqual(captured["composition_source"]["reviewed_source_head"],
                         "8ae66ef5543bcb1251868b84ea38a82c2649a3a4")
        self.assertEqual(captured["composition_source"]["source_only_tree"],
                         "f6b7dee8bac4ebcc27ad020db9940344c5e4ae41")
        self.assertEqual(len([n for n in captured["composition_inputs"] if n.startswith(("src/", "native/"))]), 140)

    def test_composition_members_are_required_and_byte_bound(self):
        for name in binding.COMPOSITION_PATHS:
            with self.subTest(name=name):
                source = self.repo / name
                raw = source.read_bytes()
                source.write_bytes(raw + b"// changed composition member\n")
                with patch.object(binding, "inverse_composition_patch", side_effect=AssertionError("inverse ran")):
                    self.rejects("changed input")
                source.write_bytes(raw)

    def test_composition_additions_cannot_be_omitted(self):
        for name in binding.COMPOSITION_ADDITIONS:
            with self.subTest(name=name):
                source = self.repo / name
                raw = source.read_bytes()
                source.unlink()
                self.rejects("missing regular input")
                source.write_bytes(raw)

    def test_composition_manifest_coherent_tampering_rejects(self):
        current = binding.read_json(self.package / "current-source.json")
        current["files"] = current["files"][:-1]
        binding.write_json(self.package / "current-source.json", current)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_composition_authority_coherent_tampering_rejects(self):
        authority = binding.read_json(self.package / "composition-authority.json")
        authority["public_sample_closure"]["references"] = 2
        binding.write_json(self.package / "composition-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale composition authority")

    def test_composition_patch_changed_or_missing_rejects(self):
        source = self.package / "composition-transition.patch"
        original = source.read_bytes()
        source.write_bytes(original + b"\n")
        self.rehash_package()
        self.rejects_before_materialization("wrong transition patch")
        source.unlink()
        self.rejects("missing or extra adapter member")

    def test_composition_inverse_requires_each_exact_context(self):
        original = self.captured["package_bytes"]["composition-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        self.assertEqual(len(sections), 40)
        for name, section in zip(binding.COMPOSITION_PATHS, sections):
            with self.subTest(name=name):
                inputs = dict(self.captured["composition_inputs"])
                hunk = next(line for line in section.splitlines() if line.startswith(b"@@ "))
                offset = max(int(hunk.split(b" +", 1)[1].split(b" ", 1)[0].split(b",", 1)[0]) - 1, 0)
                lines = inputs[name].splitlines(keepends=True)
                lines[offset] = b"X" + lines[offset]
                inputs[name] = b"".join(lines)
                with self.assertRaisesRegex(binding.BindingError, "transition current context differs"):
                    binding.inverse_composition_patch(inputs, original)

    def test_composition_inverse_scope_order_and_duplicate_controls(self):
        original = self.captured["package_bytes"]["composition-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        for changed, expected in ((b"".join(reversed(sections)), "wrong transition scope"),
                                  (b"".join(sections[:-1]), "wrong transition scope"),
                                  (original + sections[0], "duplicate transition member")):
            with self.subTest(sha=binding.digest(changed)), self.assertRaisesRegex(binding.BindingError, expected):
                binding.apply_inverse_patch(self.captured["composition_inputs"], changed, binding.digest(changed),
                                            len(changed), binding.COMPOSITION_PATHS)

    def test_division_successor_restores_every_combined_input_before_older_stages(self):
        captured = self.captured
        restored, touched = binding.inverse_division_patch(
            captured["division_inputs"], captured["package_bytes"]["division-transition.patch"])
        self.assertEqual(touched, list(binding.DIVISION_PATHS))
        self.assertEqual(len(touched), 14)
        self.assertEqual(len(set(touched)), 14)
        self.assertEqual(len(restored), 185)
        self.assertEqual(set(restored), set(captured["division_inputs"]))
        self.assertEqual(restored, captured["combined_inputs"])
        binding.check_bytes(restored, captured["combined_source"]["files"])
        self.assertEqual([name for name in restored if restored[name] != captured["division_inputs"][name]],
                         list(binding.DIVISION_PATHS))
        self.assertEqual(captured["division_source"]["reviewed_source_head"],
                         "2c46521caa902b2afb88ef6b7bae58b9a1382776")
        self.assertEqual(captured["division_source"]["source_only_tree"],
                         "7a74bf86edb53469dbcfd7839a8d3717a0d59a9a")
        self.assertEqual(captured["division_source"]["division_base_head"],
                         "8a3b8683d911bdabfcdc7ca7d3ba867f6235ded3")
        self.assertEqual(binding.digest(captured["package_bytes"]["combined-source.json"]),
                         "221524ad3faf7ea8e8b336cf8497a2eb7e2fbe476a1e829f510b2ee98dc82487")
        self.assertEqual(len(captured["package_bytes"]["combined-source.json"]), 35021)
        self.assertEqual(binding.digest(captured["package_bytes"]["combined-authority.json"]),
                         "f28aae703e7f3c1010d91e2f53a4f66a9f728fe5248edf1f4832f12f59d90670")
        self.assertEqual(binding.digest(captured["package_bytes"]["division-transition.patch"]),
                         "65319908325ce79bd46fb6014b0392b697e3a9d16447562d213dd882a0e2efb1")
        self.assertEqual(len(captured["package_bytes"]["division-transition.patch"]), 49895)
        self.assertEqual(captured["division_authority"]["added_source_paths"], [])
        self.assertEqual(captured["division_authority"]["removed_source_paths"], [])
        compiler = [name for name in captured["division_inputs"] if name.startswith(("src/", "native/"))]
        self.assertEqual(len(compiler), 133)
        self.assertEqual(len(compiler) + 3, 136)
        self.assertEqual(captured["division_authority"]["current_input_git_modes"],
                         [{"path": name, "mode": "100644"} for name in captured["division_inputs"]])
        formatter, _ = binding.inverse_combined_patch(restored, captured["package_bytes"]["combined-transition.patch"])
        predecessor, _ = binding.inverse_formatter_patch(formatter, captured["package_bytes"]["formatter-transition.patch"])
        archive, _ = binding.inverse_patch(predecessor, captured["package_bytes"]["source-transition.patch"])
        for extra in captured["authority"]["inverse_only_inputs"]:
            self.assertEqual(binding.entry(extra["path"], archive.pop(extra["path"])), extra)
        binding.check_bytes(archive, captured["selected"]["files"])

    def test_division_stage_precedes_all_historical_reconstruction(self):
        calls = []
        def record(name, original):
            def wrapper(*args):
                calls.append(name)
                return original(*args)
            return wrapper
        names = ("inverse_enum_patch", "inverse_projected_patch", "inverse_unary_patch", "inverse_composition_patch", "inverse_slices_patch", "inverse_division_patch", "inverse_combined_patch", "inverse_formatter_patch", "inverse_patch")
        with ExitStack() as stack:
            for name in names:
                stack.enter_context(patch.object(binding, name, side_effect=record(name, getattr(binding, name))))
            binding.preflight(self.repo, self.package)
        self.assertEqual(calls, list(names))

    def test_slices_successor_restores_exact_division_before_older_stages(self):
        captured = self.captured
        restored, touched = binding.inverse_slices_patch(
            captured["slices_inputs"], captured["package_bytes"]["slices-transition.patch"])
        self.assertEqual(touched, list(binding.SLICES_PATHS))
        self.assertEqual(len(touched), 47)
        self.assertEqual(len(set(touched)), 47)
        self.assertEqual(restored, captured["division_inputs"])
        self.assertEqual(set(captured["slices_inputs"]) - set(restored), set(binding.SLICES_ADDITIONS))
        self.assertEqual(len(binding.SLICES_ADDITIONS), 3)
        self.assertEqual(len(restored), 185)
        binding.check_bytes(restored, captured["division_source"]["files"])
        self.assertEqual([name for name in captured["slices_inputs"]
                          if captured["slices_inputs"][name] != restored.get(name)], list(binding.SLICES_PATHS))
        self.assertEqual(captured["slices_source"]["reviewed_source_head"],
                         "03aead9755b1dd6aaec2b4b165ee3881a7a1f7b7")
        self.assertEqual(captured["slices_source"]["source_only_tree"],
                         "450f016ed57bc3d960e0857bb8253e71a8aa718a")
        self.assertEqual(captured["slices_source"]["slices_base_head"],
                         "c5798a232ebdacaf720d580007ee8d760957a081")
        self.assertEqual(binding.entry("division-source.json", captured["package_bytes"]["division-source.json"]),
                         {"path": "division-source.json", "bytes": 35161,
                          "sha256": "d3f3d2c8dc254bdb2b86381325a943925a39fde0eb2b89a10d1de8e6bfbd7f33"})
        compiler = [name for name in captured["slices_inputs"] if name.startswith(("src/", "native/"))]
        self.assertEqual(len(compiler), 136)
        self.assertEqual(len(compiler) + 3, 139)
        self.assertEqual(captured["slices_authority"]["current_input_git_modes"],
                         [{"path": name, "mode": "100644"} for name in captured["slices_inputs"]])
        for row in captured["slices_authority"]["transition_inputs"]:
            self.assertEqual(row["before"] is None, row["path"] in binding.SLICES_ADDITIONS)
            self.assertEqual(row["after"]["mode"], "100644")

    def test_each_slices_source_is_required_and_byte_bound(self):
        for path in binding.SLICES_PATHS:
            with self.subTest(path=path):
                source = self.repo / path
                original = source.read_bytes()
                source.unlink()
                self.rejects("missing regular input")
                source.write_bytes(original + b"// changed slices source\n")
                self.rejects("changed input")
                source.write_bytes(original)

    def test_each_slices_addition_omission_rejects_before_materialization(self):
        for path in binding.SLICES_ADDITIONS:
            with self.subTest(path=path):
                source = self.repo / path
                original = source.read_bytes()
                source.unlink()
                self.rejects_before_materialization("missing regular input")
                source.write_bytes(original)

    def test_slices_changed_source_and_mode_reject_before_materialization(self):
        source = self.repo / binding.SLICES_ADDITIONS[0]
        original = source.read_bytes()
        source.write_bytes(original + b"// changed slices native tests\n")
        self.rejects_before_materialization("changed input")
        source.write_bytes(original)
        source.chmod(0o755)
        self.rejects_before_materialization("changed input mode")

    def test_coherently_rehashed_slices_source_rejects_before_reconstruction(self):
        name = "src/frontend/oir/owned_types.rs"
        source = self.repo / name
        source.write_bytes(source.read_bytes() + b"// coherent borrowed type change\n")
        current = binding.read_json(self.package / "current-source.json")
        current["files"] = [binding.entry(name, source.read_bytes()) if row["path"] == name else row
                            for row in current["files"]]
        binding.write_json(self.package / "current-source.json", current)
        authority = binding.read_json(self.package / "slices-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        authority["current_source_bytes"] = (self.package / "current-source.json").stat().st_size
        binding.write_json(self.package / "slices-authority.json", authority)
        self.rehash_package()
        with patch.object(binding, "inverse_slices_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("unapproved current source manifest")
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_omitted_slices_source_cannot_relax_membership(self):
        name = binding.SLICES_ADDITIONS[0]
        (self.repo / name).unlink()
        current = binding.read_json(self.package / "current-source.json")
        current["files"] = [row for row in current["files"] if row["path"] != name]
        binding.write_json(self.package / "current-source.json", current)
        authority = binding.read_json(self.package / "slices-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        authority["current_source_bytes"] = (self.package / "current-source.json").stat().st_size
        authority["added_source_paths"].remove(name)
        authority["current_source_members"] -= 1
        binding.write_json(self.package / "slices-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_changed_division_manifest_rejects_before_reconstruction(self):
        source = self.package / "division-source.json"
        source.write_bytes(source.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "slices-authority.json")
        authority["division_source_sha256"] = binding.digest(source.read_bytes())
        authority["division_source_bytes"] = source.stat().st_size
        binding.write_json(self.package / "slices-authority.json", authority)
        self.rehash_package()
        with patch.object(binding, "inverse_slices_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("unapproved division source manifest")
        self.rejects_before_materialization("unapproved division source manifest")

    def test_missing_slices_patch_rejects_before_materialization(self):
        (self.package / "slices-transition.patch").unlink()
        self.rejects_before_materialization("missing or extra adapter member")

    def test_rehashed_slices_patch_rejects_before_reconstruction(self):
        source = self.package / "slices-transition.patch"
        source.write_bytes(source.read_bytes() + b"\n")
        self.rehash_package()
        with patch.object(binding, "inverse_slices_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("wrong slices transition patch")
        self.rejects_before_materialization("wrong slices transition patch")

    def test_coherently_rehashed_slices_patch_rejects_before_materialization(self):
        source = self.package / "slices-transition.patch"
        source.write_bytes(source.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "slices-authority.json")
        authority["transition_patch_sha256"] = binding.digest(source.read_bytes())
        authority["transition_patch_bytes"] = source.stat().st_size
        binding.write_json(self.package / "slices-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale slices authority")

    def test_slices_checkpoint_scope_modes_and_recipe_are_pinned(self):
        original = (self.package / "slices-authority.json").read_bytes()
        for field, value in (("base_head", "0" * 40), ("source_only_tree", "0" * 40),
                              ("reviewed_source_head", "0" * 40), ("recipe", "unreviewed"),
                              ("transition_touched_paths", list(reversed(binding.SLICES_PATHS))),
                              ("transition_touched_paths", list(binding.SLICES_PATHS[:-1])),
                              ("added_source_paths", []), ("removed_source_paths", [binding.SLICES_PATHS[0]]),
                              ("compiler_bodies", 140), ("current_source_members", 189),
                              ("division_source_members", 184), ("current_input_git_modes", []),
                              ("transition_inputs", []), ("unit2_observer_adapter", {}),
                              ("compile_time_fixture_derivation", {})):
            with self.subTest(field=field, value=value):
                authority = binding.json.loads(original)
                authority[field] = value
                binding.write_json(self.package / "slices-authority.json", authority)
                self.rehash_package()
                with patch.object(binding, "inverse_slices_patch", side_effect=AssertionError("reconstruction started")):
                    self.rejects("stale slices authority")

    def test_slices_inverse_requires_exact_patch_and_each_current_context(self):
        original = self.captured["package_bytes"]["slices-transition.patch"]
        with self.assertRaisesRegex(binding.BindingError, "wrong transition patch"):
            binding.inverse_slices_patch(self.captured["slices_inputs"], original + b"\n")
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        self.assertEqual(len(sections), 47)
        for path, section in zip(binding.SLICES_PATHS, sections):
            with self.subTest(path=path):
                inputs = dict(self.captured["slices_inputs"])
                first_hunk = next(line for line in section.splitlines() if line.startswith(b"@@ "))
                offset = max(int(first_hunk.split(b" +", 1)[1].split(b" ", 1)[0].split(b",", 1)[0]) - 1, 0)
                lines = inputs[path].splitlines(keepends=True)
                lines[offset] = b"X" + lines[offset]
                inputs[path] = b"".join(lines)
                with self.assertRaisesRegex(binding.BindingError, "transition current context differs"):
                    binding.inverse_slices_patch(inputs, original)

    def test_slices_inverse_rejects_reordered_missing_and_duplicate_paths(self):
        original = self.captured["package_bytes"]["slices-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        for changed, expected in ((b"".join(reversed(sections)), "wrong transition scope"),
                                  (b"".join(sections[:-1]), "wrong transition scope"),
                                  (original + sections[0], "duplicate transition member")):
            with self.subTest(sha=binding.digest(changed)), self.assertRaisesRegex(binding.BindingError, expected):
                binding.apply_inverse_patch(self.captured["slices_inputs"], changed, binding.digest(changed),
                                            len(changed), binding.SLICES_PATHS)

    def test_compile_time_fixture_predecessor_and_current_pins_remain_distinct(self):
        before = self.captured["combined_inputs"][binding.COMPILE_FIXTURE_SOURCE]
        current = self.captured["inputs"][binding.COMPILE_FIXTURE_SOURCE]
        self.assertEqual(binding.digest(before),
                         "10687b76ac4c048d21467653b55a4c322ac011209f6d1ebd503ce2fc778810cc")
        self.assertEqual(len(before), 44176)
        self.assertEqual(binding.compile_fixture_paths(before, combined=True), binding.compile_fixture_paths(current))
        self.assertEqual(binding.re.findall(binding.COMPILE_FIXTURE_PATTERN, before),
                         binding.re.findall(binding.COMPILE_FIXTURE_PATTERN, current))
        for source, combined in ((before, False), (current, True)):
            with self.assertRaisesRegex(binding.BindingError, "wrong compile-time fixture includer"):
                binding.compile_fixture_paths(source, combined=combined)

    def test_each_division_source_is_required_and_byte_bound(self):
        for path in binding.DIVISION_PATHS:
            with self.subTest(path=path):
                source = self.repo / path
                original = source.read_bytes()
                source.unlink()
                self.rejects("missing regular input")
                source.write_bytes(original + b"// changed division source\n")
                self.rejects("changed input")
                source.write_bytes(original)

    def test_division_source_mutation_rejects_before_materialization(self):
        source = self.repo / "src/frontend/oir/native.rs"
        source.write_bytes(source.read_bytes() + b"// changed division lowering\n")
        self.rejects_before_materialization("changed input")

    def test_division_source_omission_rejects_before_materialization(self):
        (self.repo / "src/frontend/oir/arithmetic_tests.rs").unlink()
        self.rejects_before_materialization("missing regular input")

    def test_current_source_mode_changes_reject_before_materialization(self):
        source = self.repo / "src/frontend/oir/owned/native.rs"
        source.chmod(0o755)
        self.rejects_before_materialization("changed input mode")

    def test_unchanged_current_input_modes_are_also_bound(self):
        for name in ("Cargo.toml", "src/frontend/driver.rs", binding.COMBINED_FIXTURE_ADDITIONS[0]):
            with self.subTest(path=name):
                source = self.repo / name
                source.chmod(0o755)
                self.rejects("changed input mode")
                source.chmod(0o644)

    def test_coherently_rehashed_division_source_rejects_before_reconstruction(self):
        name = "src/frontend/oir/execute.rs"
        source = self.repo / name
        source.write_bytes(source.read_bytes() + b"// coherent division change\n")
        current = binding.read_json(self.package / "current-source.json")
        current["files"] = [binding.entry(name, source.read_bytes()) if row["path"] == name else row
                            for row in current["files"]]
        binding.write_json(self.package / "current-source.json", current)
        authority = binding.read_json(self.package / "division-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        authority["current_source_bytes"] = (self.package / "current-source.json").stat().st_size
        binding.write_json(self.package / "division-authority.json", authority)
        self.rehash_package()
        with patch.object(binding, "inverse_division_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("unapproved current source manifest")
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_changed_combined_manifest_rejects_before_reconstruction(self):
        source = self.package / "combined-source.json"
        source.write_bytes(source.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "division-authority.json")
        authority["combined_source_sha256"] = binding.digest(source.read_bytes())
        authority["combined_source_bytes"] = source.stat().st_size
        binding.write_json(self.package / "division-authority.json", authority)
        self.rehash_package()
        with patch.object(binding, "inverse_division_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("unapproved combined source manifest")
        self.rejects_before_materialization("unapproved combined source manifest")

    def test_missing_division_patch_rejects_before_materialization(self):
        (self.package / "division-transition.patch").unlink()
        self.rejects_before_materialization("missing or extra adapter member")

    def test_rehashed_division_patch_rejects_before_reconstruction(self):
        source = self.package / "division-transition.patch"
        source.write_bytes(source.read_bytes() + b"\n")
        self.rehash_package()
        with patch.object(binding, "inverse_division_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("wrong division transition patch")
        self.rejects_before_materialization("wrong division transition patch")

    def test_coherently_rehashed_division_patch_rejects_before_materialization(self):
        source = self.package / "division-transition.patch"
        source.write_bytes(source.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "division-authority.json")
        authority["transition_patch_sha256"] = binding.digest(source.read_bytes())
        authority["transition_patch_bytes"] = source.stat().st_size
        binding.write_json(self.package / "division-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale division authority")

    def test_division_checkpoint_scope_modes_and_recipe_are_pinned(self):
        original = (self.package / "division-authority.json").read_bytes()
        for field, value in (("base_head", "0" * 40), ("source_only_tree", "0" * 40),
                              ("reviewed_source_head", "0" * 40), ("recipe", "unreviewed"),
                              ("transition_touched_paths", list(reversed(binding.DIVISION_PATHS))),
                              ("transition_touched_paths", list(binding.DIVISION_PATHS[:-1])),
                              ("added_source_paths", [binding.DIVISION_PATHS[0]]),
                              ("removed_source_paths", [binding.DIVISION_PATHS[0]]),
                              ("compiler_bodies", 137), ("current_source_members", 186),
                              ("combined_source_members", 184), ("current_input_git_modes", []),
                              ("transition_inputs", [])):
            with self.subTest(field=field, value=value):
                authority = binding.json.loads(original)
                authority[field] = value
                binding.write_json(self.package / "division-authority.json", authority)
                self.rehash_package()
                with patch.object(binding, "inverse_division_patch", side_effect=AssertionError("reconstruction started")):
                    self.rejects("stale division authority")

    def test_division_inverse_requires_exact_patch_and_each_current_context(self):
        original = self.captured["package_bytes"]["division-transition.patch"]
        with self.assertRaisesRegex(binding.BindingError, "wrong transition patch"):
            binding.inverse_division_patch(self.captured["division_inputs"], original + b"\n")
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        self.assertEqual(len(sections), 14)
        for path, section in zip(binding.DIVISION_PATHS, sections):
            with self.subTest(path=path):
                inputs = dict(self.captured["division_inputs"])
                first_hunk = next(line for line in section.splitlines() if line.startswith(b"@@ "))
                offset = max(int(first_hunk.split(b" +", 1)[1].split(b" ", 1)[0].split(b",", 1)[0]) - 1, 0)
                lines = inputs[path].splitlines(keepends=True)
                lines[offset] = b"X" + lines[offset]
                inputs[path] = b"".join(lines)
                with self.assertRaisesRegex(binding.BindingError, "transition current context differs"):
                    binding.inverse_division_patch(inputs, original)

    def test_division_inverse_rejects_reordered_missing_and_duplicate_paths(self):
        original = self.captured["package_bytes"]["division-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        for changed, expected in ((b"".join(reversed(sections)), "wrong transition scope"),
                                  (b"".join(sections[:-1]), "wrong transition scope"),
                                  (original + sections[0], "duplicate transition member")):
            with self.subTest(sha=binding.digest(changed)), self.assertRaisesRegex(binding.BindingError, expected):
                binding.apply_inverse_patch(self.captured["division_inputs"], changed, binding.digest(changed),
                                            len(changed), binding.DIVISION_PATHS)

    def test_cumulative_array_transition_restores_the_published_archive(self):
        captured = binding.preflight(self.repo, self.package)
        self.assertEqual(len(captured["touched"]), 56)
        self.assertEqual(len(set(captured["touched"])), 56)
        delta = captured["authority"]["transition_source_delta"]
        self.assertEqual(captured["touched"][9:], delta["paths"])
        self.assertEqual(len(delta["paths"]), 47)
        self.assertFalse(set(captured["touched"][:9]) & set(delta["paths"]))
        additions = binding.EXTRA - {"src/frontend/parser/activation_tests.rs",
                                   "tests/typed_frontend.rs", "tests/typed_project_dispatch.rs"}
        self.assertEqual(len(additions), 9)
        for path in delta["paths"]:
            if path in additions:
                self.assertNotIn(path, captured["archived"])
            else:
                self.assertNotEqual(captured["inputs"][path], captured["archived"][path])
        patch = captured["package_bytes"]["source-transition.patch"]
        self.assertEqual(binding.digest(patch[:28881]),
                         "04f0588360aac12b96cd69a34b282329ea696eb69d7b979c8ffc385b7a42aab8")
        self.assertEqual(binding.digest(patch[28881:]), delta["sha256"])
        restored, _ = binding.inverse_patch(captured["predecessor_inputs"], patch)
        extra = captured["authority"]["inverse_only_inputs"][0]
        self.assertEqual(binding.entry(extra["path"], restored.pop(extra["path"])), extra)
        binding.check_bytes(restored, captured["selected"]["files"])

    def test_formatter_successor_restores_exact_predecessor_and_archive(self):
        captured = self.captured
        predecessor = binding.read_json(self.package / "predecessor-source.json")
        restored, touched = binding.inverse_formatter_patch(
            captured["formatter_inputs"], captured["package_bytes"]["formatter-transition.patch"])
        self.assertEqual(touched, list(binding.FORMATTER_PATHS))
        self.assertEqual(len(touched), 8)
        self.assertEqual(len(set(touched)), 8)
        binding.check_bytes(restored, predecessor["files"])
        self.assertEqual(restored, captured["predecessor_inputs"])
        self.assertEqual(set(captured["formatter_inputs"]) - set(restored), set(binding.FORMATTER_ADDITIONS))
        self.assertEqual(len(binding.FORMATTER_ADDITIONS), 4)
        self.assertEqual(binding.digest(captured["package_bytes"]["predecessor-source.json"]),
                         "7c3de8673eca2bf2267251a9b3235a123bcefb1538785f3400a1fa0d073c5bb8")
        self.assertEqual(binding.digest(captured["package_bytes"]["authority.json"]),
                         "73e3f96fa48bf3d478c923681108bb119bce4d5eb8441f763d72c29a596e09ce")
        self.assertEqual(binding.digest(captured["package_bytes"]["source-transition.patch"]),
                         "63055a4b1a2cb63ce6a160a53e5c8131c4c288c198cd9af6ea421b5c2931fc18")
        archived, _ = binding.inverse_patch(restored, captured["package_bytes"]["source-transition.patch"])
        for extra in captured["authority"]["inverse_only_inputs"]:
            self.assertEqual(binding.entry(extra["path"], archived.pop(extra["path"])), extra)
        binding.check_bytes(archived, captured["selected"]["files"])
        self.assertEqual(captured["formatter_source"]["reviewed_source_head"],
                         "8a08a2908b2ceb73c80112e6ddd82e2dbda91976")
        self.assertEqual(captured["formatter_source"]["source_only_tree"],
                         "afa181dab5aa3341ceae4f7b882a81a635e08fa0")

    def test_combined_successor_restores_exact_formatter_predecessor_and_archive(self):
        captured = self.captured
        restored, touched = binding.inverse_combined_patch(
            captured["combined_inputs"], captured["package_bytes"]["combined-transition.patch"])
        self.assertEqual(touched, list(binding.COMBINED_PATHS))
        self.assertEqual(len(touched), 80)
        self.assertEqual(len(set(touched)), 80)
        self.assertEqual(len(binding.COMBINED_ADDITIONS), 52)
        self.assertEqual(len(binding.COMBINED_SOURCE_ADDITIONS), 10)
        self.assertEqual(len(binding.COMBINED_FIXTURE_ADDITIONS), 42)
        self.assertEqual(len(restored), 133)
        binding.check_bytes(restored, captured["formatter_source"]["files"])
        self.assertEqual(restored, captured["formatter_inputs"])
        self.assertEqual(set(captured["combined_inputs"]) - set(restored), set(binding.COMBINED_ADDITIONS))
        for path in binding.COMBINED_PATHS:
            if path not in binding.COMBINED_ADDITIONS:
                self.assertNotEqual(captured["combined_inputs"][path], restored[path])
        self.assertEqual(captured["combined_source"]["reviewed_source_head"],
                         "a5fb98b4f1ad2fa95ee6e4637f4e9d7700cbe909")
        self.assertEqual(captured["combined_source"]["source_only_tree"],
                         "b30b0628c45e4a308bbb0ae5b35122794cd7ac12")
        self.assertEqual(captured["combined_source"]["combined_base_head"],
                         "595f681c2a906d686ddea90c65d060cff97e0a75")
        self.assertEqual(binding.digest(captured["package_bytes"]["formatter-source.json"]),
                         "69d89c46f23a99f7dc20911a4054cde7d97a98352d3fc1349e63ee7949ffcf06")
        self.assertEqual(binding.digest(captured["package_bytes"]["formatter-authority.json"]),
                         "f060dd4e264a7261517f496176d9d3def438a1e151616e313972db0545f9b4d2")
        self.assertEqual(binding.digest(captured["package_bytes"]["formatter-transition.patch"]),
                         "8e3bb083c6fbf8846a99476a57a80cb19c7163e5ebbabbd7f3e0305f9ac752b4")
        self.assertEqual(binding.digest(captured["package_bytes"]["combined-transition.patch"]),
                         "8ef58e282f1e37a7c04e653222fb74cb40f144aaa4364723773a608df2182683")
        self.assertEqual(len(captured["package_bytes"]["combined-transition.patch"]), 376315)
        compiler = [name for name in captured["combined_inputs"] if name.startswith(("src/", "native/"))]
        self.assertEqual(len(compiler), 133)
        self.assertEqual(len(compiler) + 3, 136)
        retained = {name: data for name, data in captured["combined_inputs"].items()
                    if name not in compiler and name not in binding.COMBINED_FIXTURE_ADDITIONS}
        self.assertEqual(list(sorted(retained)), list(binding.RETAINED_NON_SOURCE_PATHS))
        self.assertEqual(len(retained), 10)
        self.assertEqual(retained, {name: restored[name] for name in retained})

    def test_each_combined_source_is_required_and_byte_bound(self):
        for path in binding.COMBINED_PATHS:
            with self.subTest(path=path):
                source = self.repo / path
                original = source.read_bytes()
                source.unlink()
                self.rejects("missing regular input")
                source.write_bytes(original + b"// changed combined source\n")
                self.rejects("changed input")
                source.write_bytes(original)

    def test_each_combined_addition_omission_rejects_before_materialization(self):
        for index, path in enumerate(binding.COMBINED_ADDITIONS):
            with self.subTest(path=path):
                source = self.repo / path
                original = source.read_bytes()
                source.unlink()
                # The helper deliberately requires a fresh output for each probe.
                output = self.root / "rejected"
                if output.exists():
                    output.rename(self.root / ("previous-rejection-" + str(index)))
                self.rejects_before_materialization("missing regular input")
                source.write_bytes(original)

    def test_changed_combined_source_rejects_before_materialization(self):
        source = self.repo / "src/frontend/parser/arrays.rs"
        source.write_bytes(source.read_bytes() + b"// changed\n")
        self.rejects_before_materialization("changed input")

    def test_coherently_rehashed_combined_source_rejects_before_reconstruction(self):
        path = "src/frontend/oir/owned/source/array_pipeline.rs"
        source = self.repo / path
        source.write_bytes(source.read_bytes() + b"// coherent change\n")
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [binding.entry(path, source.read_bytes()) if row["path"] == path else row
                             for row in manifest["files"]]
        binding.write_json(self.package / "current-source.json", manifest)
        authority = binding.read_json(self.package / "combined-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        authority["current_source_bytes"] = (self.package / "current-source.json").stat().st_size
        binding.write_json(self.package / "combined-authority.json", authority)
        self.rehash_package()
        with patch.object(binding, "inverse_combined_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("unapproved current source manifest")
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_omitted_combined_source_cannot_relax_membership(self):
        path = binding.COMBINED_ADDITIONS[0]
        (self.repo / path).unlink()
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [row for row in manifest["files"] if row["path"] != path]
        binding.write_json(self.package / "current-source.json", manifest)
        authority = binding.read_json(self.package / "combined-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        authority["current_source_bytes"] = (self.package / "current-source.json").stat().st_size
        authority["added_source_paths"].remove(path)
        authority["current_source_members"] -= 1
        binding.write_json(self.package / "combined-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_changed_formatter_manifest_is_rejected(self):
        path = self.package / "formatter-source.json"
        path.write_bytes(path.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "combined-authority.json")
        authority["formatter_source_sha256"] = binding.digest(path.read_bytes())
        authority["formatter_source_bytes"] = path.stat().st_size
        binding.write_json(self.package / "combined-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("unapproved formatter source manifest")

    def test_missing_combined_patch_rejects_before_materialization(self):
        (self.package / "combined-transition.patch").unlink()
        self.rejects_before_materialization("missing or extra adapter member")

    def test_rehashed_combined_patch_rejects_before_reconstruction(self):
        path = self.package / "combined-transition.patch"
        path.write_bytes(path.read_bytes() + b"\n")
        self.rehash_package()
        with patch.object(binding, "inverse_combined_patch", side_effect=AssertionError("reconstruction started")):
            self.rejects("wrong combined transition patch")
        self.rejects_before_materialization("wrong combined transition patch")

    def test_coherently_rehashed_combined_patch_rejects_before_materialization(self):
        path = self.package / "combined-transition.patch"
        path.write_bytes(path.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "combined-authority.json")
        authority["transition_patch_sha256"] = binding.digest(path.read_bytes())
        authority["transition_patch_bytes"] = path.stat().st_size
        binding.write_json(self.package / "combined-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale combined authority")

    def test_combined_checkpoint_scope_and_recipe_metadata_are_pinned(self):
        original = (self.package / "combined-authority.json").read_bytes()
        for field, replacement in (("base_head", "0" * 40), ("source_only_tree", "0" * 40),
                                   ("reviewed_source_head", "0" * 40), ("recipe", "unreviewed"),
                                   ("transition_touched_paths", list(reversed(binding.COMBINED_PATHS))),
                                   ("transition_touched_paths", list(binding.COMBINED_PATHS[:-1])),
                                   ("added_source_paths", list(binding.COMBINED_SOURCE_ADDITIONS[:-1])),
                                   ("retained_non_source_paths", list(binding.RETAINED_NON_SOURCE_PATHS[:-1])),
                                   ("compiler_bodies", 135), ("current_source_members", 184)):
            with self.subTest(field=field, replacement=replacement):
                authority = binding.json.loads(original)
                authority[field] = replacement
                binding.write_json(self.package / "combined-authority.json", authority)
                self.rehash_package()
                with patch.object(binding, "inverse_combined_patch", side_effect=AssertionError("reconstruction started")):
                    self.rejects("stale combined authority")
        (self.package / "combined-authority.json").write_bytes(original)
        self.rehash_package()

    def test_combined_inverse_requires_exact_patch_and_each_current_context(self):
        original = self.captured["package_bytes"]["combined-transition.patch"]
        with self.assertRaisesRegex(binding.BindingError, "wrong transition patch"):
            binding.inverse_combined_patch(self.captured["combined_inputs"], original + b"\n")
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        self.assertEqual(len(sections), 80)
        for path, section in zip(binding.COMBINED_PATHS, sections):
            with self.subTest(path=path):
                inputs = dict(self.captured["combined_inputs"])
                if not inputs[path]:
                    inputs[path] = b"unexpected empty-file bytes"
                    with self.assertRaisesRegex(binding.BindingError, "invalid empty transition addition"):
                        binding.inverse_combined_patch(inputs, original)
                    continue
                first_hunk = next(line for line in section.splitlines() if line.startswith(b"@@ "))
                offset = max(int(first_hunk.split(b" +", 1)[1].split(b" ", 1)[0].split(b",", 1)[0]) - 1, 0)
                lines = inputs[path].splitlines(keepends=True)
                lines[offset] = b"X" + lines[offset]
                inputs[path] = b"".join(lines)
                with self.assertRaisesRegex(binding.BindingError, "transition current context differs"):
                    binding.inverse_combined_patch(inputs, original)

    def test_combined_inverse_rejects_reordered_missing_and_duplicate_paths(self):
        original = self.captured["package_bytes"]["combined-transition.patch"]
        sections = [b"diff --git " + item for item in original.split(b"diff --git ")[1:]]
        for changed, expected in ((b"".join(reversed(sections)), "wrong transition scope"),
                                  (b"".join(sections[:-1]), "wrong transition scope"),
                                  (original + sections[0], "duplicate transition member")):
            with self.subTest(sha=binding.digest(changed)), self.assertRaisesRegex(binding.BindingError, expected):
                binding.apply_inverse_patch(self.captured["combined_inputs"], changed, binding.digest(changed),
                                            len(changed), binding.COMBINED_PATHS)

    def test_current_resource_derives_through_unchanged_predecessor_authority(self):
        original = self.captured["historical_bytes"][binding.RESOURCE]
        predecessor = original.replace(binding.OLD_SEAM, binding.PREDECESSOR_SEAM)
        current = predecessor.replace(binding.PREDECESSOR_SEAM, binding.NEW_SEAM)
        self.assertEqual(binding.entry(binding.RESOURCE, predecessor),
                         self.captured["authority"]["derived_resource"])
        self.assertEqual(predecessor, self.captured["predecessor_resource"])
        self.assertEqual(binding.entry(binding.RESOURCE, current),
                         self.captured["combined_authority"]["derived_resource"])
        self.assertEqual(current, self.captured["combined_resource"])
        self.assertEqual(current.replace(binding.NEW_SEAM, binding.OLD_SEAM), original)
        self.assertEqual(current.count(b"arrays:ArraySyntaxPolicy::Closed"), 1)
        self.assertEqual(binding.digest(current),
                         "7c3b0d1cc06124be9a432525acdad2bf622f1061c6f474fc8fa049ce267e360f")
        self.assertEqual(len(current), 2007)

    def test_coherently_changed_combined_resource_authority_is_rejected(self):
        authority = binding.read_json(self.package / "combined-authority.json")
        authority["derived_resource"]["sha256"] = "0" * 64
        binding.write_json(self.package / "combined-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale combined authority")

    def test_changed_predecessor_resource_authority_is_still_rejected(self):
        authority = binding.read_json(self.package / "authority.json")
        authority["derived_resource"]["sha256"] = "0" * 64
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("derived predecessor resource drift")

    def test_resource_seam_changes_are_rejected(self):
        for name, replacement in (("PREDECESSOR_SEAM", binding.PREDECESSOR_SEAM + b" "),
                                   ("NEW_SEAM", binding.NEW_SEAM.replace(b"Closed", b"Candidate"))):
            with self.subTest(name=name), patch.object(binding, name, replacement):
                self.rejects("derived predecessor resource drift|derived combined resource drift")

    def test_compile_time_fixture_closure_is_exact_and_identity_bound(self):
        source = self.captured["inputs"][binding.COMPILE_FIXTURE_SOURCE]
        paths = binding.compile_fixture_paths(source)
        references = binding.re.findall(binding.COMPILE_FIXTURE_PATTERN, source)
        self.assertEqual(source.count(b"include_str!"), 47)
        self.assertEqual(len(references), 47)
        self.assertEqual(len(set(references)), 42)
        self.assertEqual(paths, list(binding.COMBINED_FIXTURE_ADDITIONS))
        self.assertEqual(binding.digest(source),
                         "93dd962176ccf43c4bfd67b5fd1d138b651f4e06e3a3c7ffd25691c09f3a5dab")
        self.assertEqual(len(source), 44194)
        self.assertEqual(self.captured["slices_authority"]["compile_time_fixture_derivation"]["source"],
                         binding.entry(binding.COMPILE_FIXTURE_SOURCE, source))
        self.assertEqual(self.captured["combined_authority"]["added_fixture_paths"], paths)
        self.assertEqual(self.captured["combined_authority"]["added_input_paths"], list(binding.COMBINED_ADDITIONS))
        self.assertTrue(all(name in self.captured["inputs"] for name in paths))
        self.assertTrue(all(name not in self.captured["formatter_inputs"] for name in paths))
        self.assertTrue(all(name not in self.captured["predecessor_inputs"] for name in paths))
        self.assertTrue(all(name not in self.captured["archived"] for name in paths))

    def test_changed_compile_time_fixture_rejects_before_materialization(self):
        source = self.repo / binding.COMBINED_FIXTURE_ADDITIONS[0]
        source.write_bytes(source.read_bytes() + b"// changed compile-time fixture\n")
        self.rejects_before_materialization("changed input")

    def test_coherently_omitted_compile_time_fixture_rejects_before_materialization(self):
        path = binding.COMBINED_FIXTURE_ADDITIONS[0]
        (self.repo / path).unlink()
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [row for row in manifest["files"] if row["path"] != path]
        manifest["compile_time_fixture_members"] -= 1
        binding.write_json(self.package / "current-source.json", manifest)
        authority = binding.read_json(self.package / "combined-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        authority["current_source_bytes"] = (self.package / "current-source.json").stat().st_size
        authority["added_fixture_paths"].remove(path)
        authority["added_input_paths"].remove(path)
        authority["compile_time_fixture_derivation"]["unique_fixture_inputs"] -= 1
        authority["current_source_members"] -= 1
        binding.write_json(self.package / "combined-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_changed_compile_time_fixture_rejects_before_materialization(self):
        path = binding.COMBINED_FIXTURE_ADDITIONS[0]
        source = self.repo / path
        source.write_bytes(source.read_bytes() + b"// coherent fixture change\n")
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [binding.entry(path, source.read_bytes()) if row["path"] == path else row
                             for row in manifest["files"]]
        binding.write_json(self.package / "current-source.json", manifest)
        authority = binding.read_json(self.package / "combined-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        authority["current_source_bytes"] = (self.package / "current-source.json").stat().st_size
        binding.write_json(self.package / "combined-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_compile_time_fixture_includer_changes_are_rejected(self):
        original = self.captured["inputs"][binding.COMPILE_FIXTURE_SOURCE]
        for changed in (original + b"\n", original.replace(b"include_str!", b"include_bytes!", 1),
                        original.replace(b"guard-empty/main.ox", b"unlisted/main.ox", 1)):
            with self.subTest(sha=binding.digest(changed)), self.assertRaisesRegex(
                    binding.BindingError, "wrong compile-time fixture includer"):
                binding.compile_fixture_paths(changed)

    def test_compile_time_fixture_literal_derivation_rejects_reference_drift(self):
        original = self.captured["inputs"][binding.COMPILE_FIXTURE_SOURCE]
        reference = binding.re.search(binding.COMPILE_FIXTURE_PATTERN, original).group()
        for changed in (original.replace(reference, b"", 1), original + reference,
                        original.replace(b"guard-empty/main.ox", b"unlisted/main.ox", 1),
                        original.replace(b'CARGO_MANIFEST_DIR', b'CARGO_OTHER_DIR', 1)):
            with self.subTest(sha=binding.digest(changed)), patch.object(
                    binding, "COMPILE_FIXTURE_SOURCE_SHA", binding.digest(changed)), patch.object(
                    binding, "COMPILE_FIXTURE_SOURCE_BYTES", len(changed)):
                with self.assertRaisesRegex(binding.BindingError, "wrong literal compile-time fixture references"):
                    binding.compile_fixture_paths(changed)

    def test_compile_time_fixture_derivation_rejects_generic_roster_changes(self):
        original = self.captured["inputs"][binding.COMPILE_FIXTURE_SOURCE]
        for changed in (binding.COMBINED_FIXTURE_ADDITIONS[:-1],
                        tuple(reversed(binding.COMBINED_FIXTURE_ADDITIONS)),
                        binding.COMBINED_FIXTURE_ADDITIONS + ("tests/arbitrary-asset.txt",)):
            with self.subTest(count=len(changed)), patch.object(binding, "COMBINED_FIXTURE_ADDITIONS", changed):
                with self.assertRaisesRegex(binding.BindingError, "wrong compile-time fixture path roster"):
                    binding.compile_fixture_paths(original)

    def test_coherently_changed_compile_time_fixture_authority_is_rejected(self):
        original = (self.package / "combined-authority.json").read_bytes()
        for field, value in (("include_str_references", 46), ("unique_fixture_inputs", 41),
                             ("literal_pattern", ".*"), ("ordered_references_sha256", "0" * 64)):
            with self.subTest(field=field):
                authority = binding.json.loads(original)
                authority["compile_time_fixture_derivation"][field] = value
                binding.write_json(self.package / "combined-authority.json", authority)
                self.rehash_package()
                self.rejects("stale combined authority")
        (self.package / "combined-authority.json").write_bytes(original)
        self.rehash_package()

    def test_empty_compile_time_fixture_addition_reverses_only_exact_empty_blob(self):
        path = "tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox"
        original = self.captured["package_bytes"]["combined-transition.patch"]
        section = next(b"diff --git " + item for item in original.split(b"diff --git ")[1:]
                       if item.startswith(("a/" + path + " ").encode()))
        self.assertEqual(self.captured["inputs"][path], b"")
        self.assertEqual(section.splitlines()[-1], b"index 0000000..e69de29")
        self.assertNotIn(b"@@", section)
        self.assertEqual(binding.apply_inverse_patch({path: b""}, section, binding.digest(section),
                                                    len(section), (path,)), ({}, [path]))
        for changed_section, data in ((section, b"\n"),
                                       (section.replace(b"e69de29", b"1111111"), b""),
                                       (section.replace(b"new file mode 100644\n", b""), b"")):
            with self.subTest(sha=binding.digest(changed_section), data=data):
                with self.assertRaisesRegex(binding.BindingError, "invalid empty transition addition"):
                    binding.apply_inverse_patch({path: data}, changed_section, binding.digest(changed_section),
                                                len(changed_section), (path,))

    def test_each_formatter_source_is_required_and_byte_bound(self):
        for path in binding.FORMATTER_PATHS:
            with self.subTest(path=path):
                source = self.repo / path
                original = source.read_bytes()
                source.unlink()
                self.rejects("missing regular input")
                source.write_bytes(original + b"// changed formatter source\n")
                self.rejects("changed input")
                source.write_bytes(original)

    def test_formatter_addition_omission_rejects_before_materialization(self):
        (self.repo / binding.FORMATTER_ADDITIONS[0]).unlink()
        self.rejects_before_materialization("missing regular input")

    def test_coherently_rehashed_formatter_manifest_cannot_replace_source_authority(self):
        path = binding.FORMATTER_ADDITIONS[0]
        source = self.repo / path
        source.write_bytes(source.read_bytes() + b"// coherent formatter mutation\n")
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [binding.entry(path, source.read_bytes()) if row["path"] == path else row
                             for row in manifest["files"]]
        binding.write_json(self.package / "current-source.json", manifest)
        authority = binding.read_json(self.package / "formatter-authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        binding.write_json(self.package / "formatter-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_omitted_formatter_source_cannot_relax_membership(self):
        path = binding.FORMATTER_ADDITIONS[0]
        (self.repo / path).unlink()
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [row for row in manifest["files"] if row["path"] != path]
        binding.write_json(self.package / "current-source.json", manifest)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_missing_formatter_patch_rejects_before_materialization(self):
        (self.package / "formatter-transition.patch").unlink()
        self.rejects_before_materialization("missing or extra adapter member")

    def test_coherently_rehashed_formatter_patch_rejects_before_materialization(self):
        path = self.package / "formatter-transition.patch"
        path.write_bytes(path.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "formatter-authority.json")
        authority["transition_patch_sha256"] = binding.digest(path.read_bytes())
        authority["transition_patch_bytes"] = path.stat().st_size
        binding.write_json(self.package / "formatter-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale formatter authority")

    def test_formatter_inverse_rejects_changed_patch_and_each_changed_current_context(self):
        original = self.captured["package_bytes"]["formatter-transition.patch"]
        with self.assertRaisesRegex(binding.BindingError, "wrong transition patch"):
            binding.inverse_formatter_patch(self.captured["formatter_inputs"], original + b"\n")
        for path in binding.FORMATTER_PATHS:
            with self.subTest(path=path):
                inputs = dict(self.captured["formatter_inputs"])
                # Mutate a byte consumed by a hunk, even for a late-file change.
                marker = b"mod format;" if path.endswith("/mod.rs") else (
                    b"pub(super) fn into_single_text" if path.endswith("/source.rs") else (
                        b"Route::FormatError" if path.endswith("/driver.rs") else (
                            b"TypedFormat" if path.endswith("/options.rs") else inputs[path][:20])))
                self.assertIn(marker, inputs[path])
                inputs[path] = inputs[path].replace(marker, b"X" + marker[1:], 1)
                with self.assertRaisesRegex(binding.BindingError, "transition current context differs"):
                    binding.inverse_formatter_patch(inputs, original)

    def test_coherently_changed_predecessor_manifest_is_rejected(self):
        path = self.package / "predecessor-source.json"
        path.write_bytes(path.read_bytes() + b"\n")
        self.rehash_package()
        self.rejects_before_materialization("unapproved predecessor source manifest")

    def test_formatter_checkpoint_scope_and_recipe_metadata_are_pinned(self):
        original = (self.package / "formatter-authority.json").read_bytes()
        for field, replacement in (("base_head", "0" * 40), ("source_only_tree", "0" * 40),
                                   ("reviewed_source_head", "0" * 40), ("recipe", "unreviewed"),
                                   ("transition_touched_paths", list(binding.FORMATTER_PATHS[:-1]))):
            with self.subTest(field=field):
                authority = binding.json.loads(original)
                authority[field] = replacement
                binding.write_json(self.package / "formatter-authority.json", authority)
                self.rehash_package()
                self.rejects("stale formatter authority")
        (self.package / "formatter-authority.json").write_bytes(original)
        self.rehash_package()

    def test_every_array_addition_is_required(self):
        additions = binding.EXTRA - {"src/frontend/parser/activation_tests.rs",
                                   "tests/typed_frontend.rs", "tests/typed_project_dispatch.rs"}
        for path in sorted(additions):
            with self.subTest(path=path):
                source = self.repo / path
                original = source.read_bytes()
                source.unlink()
                self.rejects("missing regular input")
                source.write_bytes(original)

    def test_missing_groundwork_member_rejects_before_materialization(self):
        (self.repo / "src/frontend/oir/owned_types/array_tests.rs").unlink()
        self.rejects_before_materialization("missing regular input")

    def test_changed_groundwork_member_rejects_before_materialization(self):
        path = self.repo / "src/frontend/oir/owned_types/array_tests.rs"
        path.write_bytes(path.read_bytes() + b"// changed\n")
        self.rejects_before_materialization("changed input")

    def test_coherently_rehashed_groundwork_source_rejects_before_reconstruction(self):
        path = "src/frontend/oir/owned_types/array_tests.rs"
        source = self.repo / path
        source.write_bytes(source.read_bytes() + b"// coherent change\n")
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [binding.entry(path, source.read_bytes()) if row["path"] == path else row
                             for row in manifest["files"]]
        binding.write_json(self.package / "current-source.json", manifest)
        authority = binding.read_json(self.package / "authority.json")
        authority["current_source_sha256"] = binding.digest((self.package / "current-source.json").read_bytes())
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_coherently_omitted_groundwork_member_rejects_before_reconstruction(self):
        path = "src/frontend/oir/owned_types/array_tests.rs"
        (self.repo / path).unlink()
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["files"] = [row for row in manifest["files"] if row["path"] != path]
        binding.write_json(self.package / "current-source.json", manifest)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_stale_checkpoint_metadata_rejects_before_reconstruction(self):
        manifest = binding.read_json(self.package / "current-source.json")
        manifest["reviewed_source_head"] = "0" * 40
        binding.write_json(self.package / "current-source.json", manifest)
        self.rehash_package()
        self.rejects_before_materialization("unapproved current source manifest")

    def test_missing_patch_rejects_before_materialization(self):
        (self.package / "source-transition.patch").unlink()
        self.rejects_before_materialization("missing or extra adapter member")

    def test_coherently_rehashed_patch_rejects_before_materialization(self):
        path = self.package / "source-transition.patch"
        path.write_bytes(path.read_bytes() + b"\n")
        authority = binding.read_json(self.package / "authority.json")
        authority["transition_patch_sha256"] = binding.digest(path.read_bytes())
        authority["transition_patch_bytes"] = path.stat().st_size
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale transition authority")

    def test_stale_transition_scope_rejects_before_materialization(self):
        authority = binding.read_json(self.package / "authority.json")
        authority["transition_touched_paths"].pop()
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale transition authority")

    def test_coherently_rewritten_transition_base_rejects(self):
        authority = binding.read_json(self.package / "authority.json")
        authority["transition_source_delta"]["base_head"] = "0" * 40
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale source delta authority")

    def test_coherently_rewritten_transition_recipe_rejects(self):
        authority = binding.read_json(self.package / "authority.json")
        authority["transition_source_delta"]["recipe"] = "unreviewed transformation"
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale source delta authority")

    def test_groundwork_inverse_requires_exact_current_context(self):
        inputs = dict(self.captured["predecessor_inputs"])
        path = "src/frontend/oir/owned_types/array_tests.rs"
        inputs[path] = b"X" + inputs[path][1:]
        with self.assertRaisesRegex(binding.BindingError, "transition current context differs"):
            binding.inverse_patch(inputs, self.captured["package_bytes"]["source-transition.patch"])

    def test_missing_source_fails(self):
        (self.repo / "src/frontend/driver.rs").unlink()
        self.rejects("missing regular input")

    def test_changed_source_fails(self):
        source = self.repo / "src/frontend/driver.rs"
        source.write_bytes(b"X" + source.read_bytes()[1:])
        self.rejects("changed input")

    def test_extra_source_fails(self):
        (self.repo / "src/unlisted.rs").write_bytes(b"// no admission\n")
        self.rejects("missing or extra compiler source")

    def test_symlink_source_fails(self):
        source = self.repo / "src/frontend/driver.rs"
        source.rename(source.with_suffix(".original"))
        source.symlink_to(source.with_suffix(".original").name)
        self.rejects("symlink input")

    def test_extra_package_fails(self):
        (self.package / "unlisted.json").write_bytes(b"{}")
        self.rejects("missing or extra adapter member")

    def test_changed_published_manifest_fails(self):
        source = self.repo / binding.U3 / "manifests/core-v1.json"
        source.write_bytes(source.read_bytes() + b"\n")
        self.rejects("changed input")

    def test_changed_compatibility_runner_fails(self):
        (self.repo / binding.COMPAT).write_bytes(b"raise SystemExit(0)\n")
        self.rejects("changed input")

    def test_changed_current_manifest_fails(self):
        path = self.package / "current-source.json"
        value = binding.read_json(path)
        value["files"].pop()
        binding.write_json(path, value)
        self.rejects("changed input")

    def test_changed_patch_fails(self):
        patch = self.captured["package_bytes"]["source-transition.patch"]
        with self.assertRaisesRegex(binding.BindingError, "wrong transition patch"):
            binding.inverse_patch(self.captured["inputs"], patch + b"\n")

    def test_current_unit2_package_has_exact_declared_adapters(self):
        output = self.root / "unit2"
        output.mkdir()
        seam = binding.prepare_unit2(output, self.captured)
        root = Path(seam["resource_package_root"])
        changes = [name for name, data in self.captured["historical_bytes"].items()
                   if (root / name).read_bytes() != data]
        self.assertEqual(sorted(changes), sorted([binding.RESOURCE, binding.INDEX_RESOURCE, binding.OBSERVER, binding.UNIT2_COMPARATOR]))
        self.assertEqual(seam["resource_package_changes"],
                         [binding.RESOURCE, binding.INDEX_RESOURCE, binding.OBSERVER, binding.UNIT2_COMPARATOR,
                          binding.UNIT2_FROZEN_COMPARATOR, binding.UNIT2_SEMANTIC_HELPER,
                          binding.UNIT2_SEMANTIC_DESCRIPTOR, "package-inputs.json"])
        self.assertEqual(seam["observer_adapter"], self.captured["enum_authority"]["unit2_observer_adapter"])
        self.assertEqual((root / binding.OBSERVER).read_bytes(), self.captured["observer"])
        modified = (root / binding.RESOURCE).read_bytes()
        self.assertEqual(modified.replace(binding.STDIN_RESOURCE_SEAM, binding.OLD_SEAM),
                         self.captured["historical_bytes"][binding.RESOURCE])
        self.assertEqual(seam["resource_enum_predecessor"],
                         self.captured["enum_authority"]["resource_adapter"]["derived"])
        self.assertEqual(seam["resource_adapter"], self.captured["stdin_authority"]["resource_adapter"])
        self.assertEqual(seam["resource_after"], binding.entry(binding.RESOURCE, modified))
        self.assertEqual((Path(seam["compatibility_runner"])).read_bytes(), self.captured["references"][binding.COMPAT])

    def test_current_observer_substitutions_are_exact_and_reversible(self):
        original = self.captured["historical_bytes"][binding.OBSERVER]
        adapted = binding.adapt_unit2_observer(original)
        self.assertEqual(len(binding.OBSERVER_SEAMS), 4)
        self.assertEqual(binding.digest(original), binding.OBSERVER_ORIGINAL_SHA)
        self.assertEqual(binding.digest(adapted), binding.OBSERVER_DERIVED_SHA)
        restored = adapted
        for old, new in reversed(binding.OBSERVER_SEAMS):
            self.assertEqual(restored.count(new), 1)
            restored = restored.replace(new, old)
        self.assertEqual(restored, original)
        # These compiled controls are required in the isolated observer; this
        # bounded test checks admitted bytes, never claims they executed.
        for name in ("preserves_scalar_and_record_json", "denies_owned_array",
                     "denies_shared_array", "denies_exclusive_array"):
            self.assertIn(("fn current_unit2_aggregate_adapter_" + name + "()").encode(), adapted)
        self.assertEqual(adapted.count(b'#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]'), 3)

    def test_wrong_original_observer_is_rejected(self):
        original = self.captured["historical_bytes"][binding.OBSERVER]
        for data in (original + b"\n", original[:-1], original.replace(b"record.0", b"record.1", 1)):
            with self.subTest(sha=binding.digest(data)), self.assertRaisesRegex(binding.BindingError, "wrong original Unit2 observer"):
                binding.adapt_unit2_observer(data)

    def test_borrowed_observer_successor_is_exact_and_reversible(self):
        original = self.captured["historical_bytes"][binding.OBSERVER]
        aggregate = binding.adapt_unit2_observer(original)
        borrowed = binding.adapt_borrowed_unit2_observer(aggregate)
        self.assertEqual(aggregate, self.captured["aggregate_observer"])
        self.assertEqual(borrowed, self.captured["borrowed_observer"])
        self.assertEqual(len(binding.BORROWED_OBSERVER_SEAMS), 7)
        self.assertEqual(len(borrowed), 17039)
        self.assertEqual(binding.digest(borrowed),
                         "abe639a07549c67db03df2e1d549173c327056f42e49c87795032a5266c2883b")
        restored = borrowed
        for old, new in reversed(binding.BORROWED_OBSERVER_SEAMS):
            self.assertEqual(restored.count(new), 1)
            restored = restored.replace(new, old)
        self.assertEqual(restored, aggregate)
        for old, new in reversed(binding.OBSERVER_SEAMS):
            restored = restored.replace(new, old)
        self.assertEqual(restored, original)
        self.assertNotIn(b"ParameterTy::Reference { aggregate", borrowed)
        self.assertIn(b"BorrowedTy::Exact(AggregateTy::Record(record)) => record.0", borrowed)
        self.assertIn(b'BorrowedTy::Exact(AggregateTy::FixedArray(_)) => panic!("current Unit2 observer excludes fixed-array projection")', borrowed)
        self.assertIn(b'BorrowedTy::ScalarSlice(_) => panic!("current Unit2 observer excludes scalar-slice projection")', borrowed)
        # The owner helper and projection are retained exactly, without borrowed wrapping.
        self.assertIn(binding.OBSERVER_SEAMS[2][1], borrowed)
        owner_helper = aggregate.split(b"fn current_unit2_record_ordinal", 1)[1].split(b"\n#[test]", 1)[0]
        self.assertIn(b"fn current_unit2_record_ordinal" + owner_helper, borrowed)
        self.assertEqual(borrowed.count(b"fn current_unit2_aggregate_adapter_"), 6)
        self.assertEqual(borrowed.count(b'#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]'), 3)
        self.assertEqual(borrowed.count(b'#[should_panic(expected = "current Unit2 observer excludes scalar-slice projection")]'), 2)
        for name in binding.OBSERVER_CONTROL_NAMES:
            self.assertIn(("fn " + name.rsplit("::", 1)[1] + "()").encode(), borrowed)

    def test_predecessor_observer_is_verified_before_borrowed_derivation(self):
        calls = []
        def record(name, original):
            def wrapper(*args):
                calls.append(name)
                return original(*args)
            return wrapper
        names = ("adapt_unit2_observer", "adapt_borrowed_unit2_observer")
        with ExitStack() as stack:
            for name in names:
                stack.enter_context(patch.object(binding, name, side_effect=record(name, getattr(binding, name))))
            binding.preflight(self.repo, self.package)
        self.assertEqual(calls, list(names))
        authority = binding.read_json(self.package / "authority.json")
        authority["unit2_observer_adapter"]["derived"]["sha256"] = "0" * 64
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        with patch.object(binding, "adapt_borrowed_unit2_observer", side_effect=AssertionError("borrowed derivation started")):
            self.rejects("stale Unit2 observer adapter authority")

    def test_borrowed_observer_requires_exact_predecessor_and_seven_seams(self):
        aggregate = self.captured["aggregate_observer"]
        for changed in (aggregate + b"\n", aggregate[:-1], self.captured["historical_bytes"][binding.OBSERVER]):
            with self.assertRaisesRegex(binding.BindingError, "wrong predecessor Unit2 observer"):
                binding.adapt_borrowed_unit2_observer(changed)
        seams = binding.BORROWED_OBSERVER_SEAMS
        for changed in (seams[:-1], seams + (seams[0],)):
            with patch.object(binding, "BORROWED_OBSERVER_SEAMS", changed):
                with self.assertRaisesRegex(binding.BindingError, "wrong borrowed Unit2 observer substitution count"):
                    binding.adapt_borrowed_unit2_observer(aggregate)

    def test_borrowed_observer_projection_changes_are_rejected(self):
        seams = binding.BORROWED_OBSERVER_SEAMS
        for at, (old, new) in enumerate(seams):
            for changed in ((b"missing borrowed seam", new), (old, new + b"// altered")):
                with self.subTest(at=at), patch.object(binding, "BORROWED_OBSERVER_SEAMS", seams[:at] + (changed,) + seams[at + 1:]):
                    with self.assertRaisesRegex(binding.BindingError, "borrowed Unit2 observer seam drift|wrong derived borrowed Unit2 observer"):
                        binding.adapt_borrowed_unit2_observer(self.captured["aggregate_observer"])

    def test_coherently_rehashed_borrowed_observer_authority_is_rejected(self):
        authority = binding.read_json(self.package / "slices-authority.json")
        authority["unit2_observer_adapter"]["derived"]["sha256"] = "0" * 64
        binding.write_json(self.package / "slices-authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale slices authority")

    def test_missing_or_extra_observer_substitution_is_rejected(self):
        seams = binding.OBSERVER_SEAMS
        for changed in (seams[:-1], seams + (seams[0],)):
            with self.subTest(count=len(changed)), patch.object(binding, "OBSERVER_SEAMS", changed):
                with self.assertRaisesRegex(binding.BindingError, "wrong Unit2 observer substitution count"):
                    binding.adapt_unit2_observer(self.captured["historical_bytes"][binding.OBSERVER])

    def test_observer_seam_or_projection_change_is_rejected(self):
        seams = binding.OBSERVER_SEAMS
        variants = [(b"missing seam", seams[0][1]), (seams[0][0], seams[0][1] + b"// altered")]
        for replacement in variants:
            with self.subTest(replacement=replacement), patch.object(binding, "OBSERVER_SEAMS", (replacement,) + seams[1:]):
                with self.assertRaisesRegex(binding.BindingError, "observer seam drift|wrong derived Unit2 observer"):
                    binding.adapt_unit2_observer(self.captured["historical_bytes"][binding.OBSERVER])

    def test_coherently_rehashed_observer_authority_is_rejected(self):
        authority = binding.read_json(self.package / "authority.json")
        authority["unit2_observer_adapter"]["derived"]["sha256"] = "0" * 64
        binding.write_json(self.package / "authority.json", authority)
        self.rehash_package()
        self.rejects_before_materialization("stale Unit2 observer adapter authority")

    def observer_protocol(self):
        spec = importlib.util.spec_from_file_location("_observer_control_test_protocol", REPO / binding.U2 / "protocol.py")
        protocol = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(protocol)
        return protocol

    def observer_control_streams(self):
        names = binding.OBSERVER_CONTROL_NAMES
        listing = "\n".join(name + ": test" for name in names) + "\n\n6 tests, 0 benchmarks\n"
        results = [name + (" - should panic" if "_denies_" in name else "") for name in names]
        execution = "running 6 tests\n" + "\n".join("test " + name + " ... ok" for name in results)
        execution += "\n\ntest result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 44 filtered out; finished in 0.00s\n"
        return listing, execution

    def test_observer_control_protocol_rejects_incomplete_results(self):
        listing, execution = self.observer_control_streams()
        protocol = self.observer_protocol()
        self.assertEqual(binding.verify_observer_control_output(protocol, listing, execution)["passed"], 6)
        for changed_listing, changed_execution in (
                (listing.replace(binding.OBSERVER_CONTROL_NAMES[0] + ": test\n", ""), execution),
                (listing + "unexpected: test\n", execution),
                (listing, execution.replace(" ... ok", " ... ignored", 1)),
                (listing, execution.replace(" ... ok", " ... FAILED", 1)),
                (listing, execution.replace("6 passed", "0 passed")),
                (listing, execution.replace(" - should panic", "", 1))):
            with self.subTest(listing=changed_listing, execution=changed_execution), self.assertRaises(ValueError):
                binding.verify_observer_control_output(protocol, changed_listing, changed_execution)

    def test_observer_control_protocol_rejects_complete_old_four_test_roster(self):
        listing, execution = self.observer_control_streams()
        for name in binding.OBSERVER_CONTROL_NAMES[-2:]:
            listing = listing.replace(name + ": test\n", "")
            execution = execution.replace("test " + name + " - should panic ... ok\n", "")
        listing = listing.replace("6 tests", "4 tests")
        execution = execution.replace("6 tests", "4 tests").replace("6 passed", "4 passed")
        with self.assertRaises(ValueError):
            binding.verify_observer_control_output(self.observer_protocol(), listing, execution)

    def synthetic_observer_controls(self, fail=False):
        """Exercise orchestration with synthetic stream producers; no Rust execution claim."""
        output = self.root / "synthetic-observer-controls"
        output.mkdir()
        seam = binding.prepare_unit2(output, self.captured)
        run = output / "unit2/run"
        source = run / "source"
        binding.materialize(source, {"synthetic-source.txt": b"synthetic orchestration only\n"})
        binding.write_json(run / "assembly.json", {"files": [binding.entry("synthetic-source.txt", (source / "synthetic-source.txt").read_bytes())]})
        listing, execution = self.observer_control_streams()
        script = ("#!" + sys.executable + "\nimport sys\n"
                  + "print(" + repr(listing) + " if '--list' in sys.argv else " + repr(execution) + ", end='')\n"
                  + ("sys.stderr.write('synthetic control failure\\n')\nraise SystemExit(7 if '--list' not in sys.argv else 0)\n" if fail else ""))
        for profile in ("debug", "release"):
            binary = run / "target" / profile / "synthetic-test-producer"
            binary.parent.mkdir(parents=True)
            binary.write_text(script)
            binary.chmod(0o755)
            binding.write_json(run / (profile + "-receipt.json"), {"binary": str(binary), "binary_sha256": binding.digest(binary.read_bytes())})
        return output, seam

    @unittest.skipIf(sys.platform == "win32", "synthetic executable uses a POSIX shebang")
    def test_observer_controls_bind_both_profiles_and_keep_streams(self):
        output, seam = self.synthetic_observer_controls()
        receipt = binding.run_unit2_observer_controls(self.repo, output, self.captured, seam)
        self.assertEqual(receipt["observer_adapter_version"], binding.ENUM_OBSERVER_ADAPTER_VERSION)
        self.assertEqual(receipt["observer_adapter_version"], seam["observer_adapter"]["version"])
        self.assertEqual(receipt["observer_control_tests_per_profile"], 6)
        self.assertEqual(receipt["test_function_executions"], 56)
        self.assertEqual(len(receipt["observer_control_receipts"]), 2)
        for row in receipt["observer_control_receipts"]:
            report = binding.read_json(output / row["path"])
            self.assertEqual(report["tests"], list(binding.OBSERVER_CONTROL_NAMES))
            self.assertEqual(report["observer_adapter"], seam["observer_adapter"])
            self.assertEqual(report["observer_adapter"]["version"], receipt["observer_adapter_version"])
            self.assertEqual(report["source_inputs_sha256"], binding.CURRENT_SOURCE_SHA)
            commands = binding.read_json(output / "observer-adapter-controls" / report["commands"]["path"])
            self.assertEqual(len(commands), 2)
            for command in commands:
                self.assertEqual(command["exit_status"], 0)
                self.assertFalse(command["timed_out"])
                binding.check_entries(output / "observer-adapter-controls", [command["stdout"], command["stderr"]])

    @unittest.skipIf(sys.platform == "win32", "synthetic executable uses a POSIX shebang")
    def test_observer_control_failure_keeps_evidence(self):
        output, seam = self.synthetic_observer_controls(fail=True)
        with self.assertRaisesRegex(binding.BindingError, "observer controls failed"):
            binding.run_unit2_observer_controls(self.repo, output, self.captured, seam)
        commands = binding.read_json(output / "observer-adapter-controls/debug-commands.json")
        self.assertEqual(commands[-1]["exit_status"], 7)
        self.assertEqual((output / "observer-adapter-controls/debug-run.stderr").read_text(), "synthetic control failure\n")
        self.assertFalse((output / "observer-adapter-controls/debug-receipt.json").exists())
        self.assertFalse((output / "observer-adapter-controls/release-commands.json").exists())

    def test_preflight_retains_zero_execution_metadata(self):
        output = self.root / "preflight"
        result = subprocess.run([sys.executable, "-B", str(self.package / "run.py"), "preflight",
                                 "--repo", str(self.repo), "--output", str(output)],
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        prepared = binding.read_json(output / "prepared.json")
        self.assertEqual(prepared["compiler_executions"], 0)
        self.assertFalse(prepared["semantic_pass"])
        self.assertFalse((output / "result.json").exists())
        self.assertEqual(prepared["plan_sha256"], binding.digest((output / "plan.json").read_bytes()))
        plan = binding.read_json(output / "plan.json")
        self.assertEqual(plan["slices_source_members"], 188)
        self.assertEqual(plan["composition_authority_sha256"], binding.COMPOSITION_AUTHORITY_SHA)
        self.assertEqual(plan["slices_source_sha256"], binding.SLICES_SOURCE_SHA)
        self.assertEqual((plan["current_source_members"], plan["division_source_members"], plan["combined_source_members"],
                          plan["formatter_source_members"],
                          plan["predecessor_source_members"], plan["archive_members"]),
                         (237, 185, 185, 133, 129, 117))
        self.assertEqual((plan["compile_time_fixture_members"], plan["compile_time_fixture_references"]), (42, 47))
        self.assertEqual(plan["unit2_current_observer_controls_per_profile"], 6)
        self.assertEqual(prepared["slices_authority_sha256"], binding.SLICES_AUTHORITY_SHA)
        self.assertEqual(prepared["division_source_sha256"], binding.DIVISION_SOURCE_SHA)
        self.assertEqual(prepared["division_authority_sha256"], binding.DIVISION_AUTHORITY_SHA)
        self.assertEqual(prepared["combined_source_sha256"], binding.COMBINED_SOURCE_SHA)
        self.assertEqual(prepared["combined_authority_sha256"], binding.COMBINED_AUTHORITY_SHA)
        self.assertEqual(prepared["formatter_source_sha256"], binding.FORMATTER_SOURCE_SHA)
        self.assertEqual(prepared["current_source_sha256"], binding.CURRENT_SOURCE_SHA)
        self.assertEqual(prepared["formatter_authority_sha256"], binding.FORMATTER_AUTHORITY_SHA)
        self.assertEqual(prepared["predecessor_source_sha256"], binding.PREDECESSOR_SOURCE_SHA)

    def test_failed_preflight_retains_failure_without_executing_tools(self):
        (self.repo / "src/frontend/driver.rs").unlink()
        output = self.root / "failed"
        sentinel = self.root / "tool-ran"
        tool = self.root / "cargo"
        tool.write_text("#!/bin/sh\ntouch '" + str(sentinel) + "'\nexit 0\n")
        tool.chmod(0o755)
        result = subprocess.run([sys.executable, "-B", str(self.package / "run.py"), "run-unit2",
                                 "--repo", str(self.repo), "--output", str(output), "--cargo", str(tool),
                                 "--rustc", str(tool)], capture_output=True, text=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        failure = binding.read_json(output / "failure.json")
        self.assertEqual(failure["status"], "failed")
        self.assertEqual(failure["compiler_executions"], 0)
        self.assertFalse(sentinel.exists())
        self.assertFalse((output / "compatibility").exists())
        self.assertFalse((output / "result.json").exists())

    def test_occupied_output_preserves_previous_evidence(self):
        output = self.root / "occupied"
        output.mkdir()
        (output / "prepared.json").write_bytes(b"previous run")
        result = subprocess.run([sys.executable, "-B", str(self.package / "run.py"), "preflight",
                                 "--repo", str(self.repo), "--output", str(output)], capture_output=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((output / "prepared.json").read_bytes(), b"previous run")

    def test_optimized_python_fails_before_materialization(self):
        output = self.root / "optimized"
        result = subprocess.run([sys.executable, "-O", "-B", str(self.package / "run.py"), "prepare-archived",
                                 "--repo", str(self.repo), "--output", str(output)], capture_output=True, timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((output / "archived-selected").exists())
        self.assertEqual(binding.read_json(output / "failure.json")["compiler_executions"], 0)

    def test_stale_post_admission_source_fails(self):
        captured = binding.preflight(self.repo, self.package)
        source = self.repo / "src/frontend/driver.rs"
        source.write_bytes(source.read_bytes() + b"\n")
        with self.assertRaisesRegex(binding.BindingError, "changed input"):
            binding.assert_unchanged(self.repo, captured, self.package)

    def synthetic_preparation(self):
        """Untrusted metadata fixture only; never describes executed tests."""
        output = self.root / "synthetic"
        output.mkdir()
        seam = binding.prepare_unit2(output, self.captured)
        compatibility = output / "unit2"
        run = compatibility / "run"
        run.mkdir(parents=True)
        inputs = dict(self.captured["historical_bytes"])
        inputs[binding.RESOURCE] = self.captured["resource"]
        inputs[binding.INDEX_RESOURCE] = self.captured["index_resource"]
        inputs[binding.UNIT2_COMPARATOR] = self.captured["unit2_comparator"]
        inputs[binding.UNIT2_FROZEN_COMPARATOR] = self.captured["historical_bytes"][binding.UNIT2_COMPARATOR]
        inputs[binding.UNIT2_SEMANTIC_HELPER] = self.captured["package_bytes"][binding.SEMANTIC_HELPER]
        inputs[binding.UNIT2_SEMANTIC_DESCRIPTOR] = self.captured["package_bytes"][binding.SEMANTIC_DESCRIPTOR]
        inputs[binding.OBSERVER] = self.captured["observer"]
        inputs["source-inputs.json"] = self.captured["package_bytes"]["current-source.json"]
        manifest = {**self.captured["historical"], "files": [binding.entry(n, d) for n, d in sorted(inputs.items())]}
        inputs["package-inputs.json"] = binding.encoded(manifest)
        binding.materialize(compatibility / "derived-package", inputs)
        child = {"invocation_id": "synthetic-zero-execution", "status": "prepared", "compiler_executions": 0,
                 "source_inputs_sha256": binding.digest(inputs["source-inputs.json"]),
                 "package_inputs_sha256": binding.digest(inputs["package-inputs.json"])}
        outer = {"status": "prepared", "compiler_executions": 0,
                 "historical_package_inputs_sha256": seam["resource_package_inputs_sha256"],
                 "derived_package_inputs_sha256": child["package_inputs_sha256"],
                 "child_invocation_id": child["invocation_id"],
                 "source_inputs_sha256": child["source_inputs_sha256"]}
        binding.write_json(run / "invocation.json", child)
        self.write_synthetic(compatibility, child, outer, "prepared.json")
        return output, seam, child, outer

    def write_synthetic(self, compatibility, child, outer, artifact):
        binding.write_json(compatibility / "run" / artifact, child)
        outer["child_result_sha256"] = binding.digest((compatibility / "run" / artifact).read_bytes())
        binding.write_json(compatibility / artifact, outer)

    def test_stale_child_invocation_is_rejected(self):
        output, seam, child, outer = self.synthetic_preparation()
        child["invocation_id"] = "stale-other-run"
        self.write_synthetic(output / "unit2", child, outer, "prepared.json")
        with self.assertRaisesRegex(binding.BindingError, "stale child invocation"):
            binding.verify_unit2_result(output, self.captured, seam, True)

    def test_stale_child_source_is_rejected(self):
        output, seam, child, outer = self.synthetic_preparation()
        child["source_inputs_sha256"] = "0" * 64
        self.write_synthetic(output / "unit2", child, outer, "prepared.json")
        with self.assertRaisesRegex(binding.BindingError, "stale child binding"):
            binding.verify_unit2_result(output, self.captured, seam, True)

    def test_zero_execution_pass_is_rejected(self):
        output, seam, child, outer = self.synthetic_preparation()
        child.update(status="passed", profiles=["debug", "release"], semantic_cases_per_profile=0,
                     resource_tests_per_profile=0, receipts=[])
        outer["status"] = "passed"
        self.write_synthetic(output / "unit2", child, outer, "result.json")
        with self.assertRaisesRegex(binding.BindingError, "zero or partial Unit2 result"):
            binding.verify_unit2_result(output, self.captured, seam, False)

    def test_preparation_cannot_be_relabelled_pass(self):
        output, seam, child, outer = self.synthetic_preparation()
        child["status"] = outer["status"] = "passed"
        self.write_synthetic(output / "unit2", child, outer, "prepared.json")
        with self.assertRaisesRegex(binding.BindingError, "preparation claimed execution"):
            binding.verify_unit2_result(output, self.captured, seam, True)

    def test_modified_derived_package_is_rejected(self):
        output, seam, child, outer = self.synthetic_preparation()
        (output / "unit2/derived-package/semantic/compare.py").write_bytes(b"raise SystemExit(0)\n")
        with self.assertRaisesRegex(binding.BindingError, "changed input"):
            binding.verify_unit2_result(output, self.captured, seam, True)


if __name__ == "__main__":
    unittest.main()
