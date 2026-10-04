"""Source-view admission controls; no compiler build or semantic expectation."""
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
        output = self.root / "rejected"
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
        self.assertEqual(len(captured["inputs"]), 133)
        self.assertEqual(len(captured["predecessor_inputs"]), 129)
        self.assertEqual(len(captured["archived"]), 117)
        self.assertNotEqual(captured["inputs"]["src/frontend/driver.rs"], captured["archived"]["src/frontend/driver.rs"])
        output = self.root / "archive"
        output.mkdir()
        receipt = binding.prepare_archived(output, captured)
        self.assertEqual(receipt["compiler_executions"], 0)
        self.assertFalse(receipt["semantic_pass"])
        binding.check_entries(output / "archived-selected", captured["selected"]["files"], exact=True)

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
            captured["inputs"], captured["package_bytes"]["formatter-transition.patch"])
        self.assertEqual(touched, list(binding.FORMATTER_PATHS))
        self.assertEqual(len(touched), 8)
        self.assertEqual(len(set(touched)), 8)
        binding.check_bytes(restored, predecessor["files"])
        self.assertEqual(restored, captured["predecessor_inputs"])
        self.assertEqual(set(captured["inputs"]) - set(restored), set(binding.FORMATTER_ADDITIONS))
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
        self.assertEqual(captured["current"]["reviewed_source_head"],
                         "8a08a2908b2ceb73c80112e6ddd82e2dbda91976")
        self.assertEqual(captured["current"]["source_only_tree"],
                         "afa181dab5aa3341ceae4f7b882a81a635e08fa0")

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
            binding.inverse_formatter_patch(self.captured["inputs"], original + b"\n")
        for path in binding.FORMATTER_PATHS:
            with self.subTest(path=path):
                inputs = dict(self.captured["inputs"])
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

    def test_resource_seam_only_adds_false_initializer(self):
        output = self.root / "unit2"
        output.mkdir()
        seam = binding.prepare_unit2(output, self.captured)
        root = Path(seam["resource_package_root"])
        changes = [name for name, data in self.captured["historical_bytes"].items()
                   if (root / name).read_bytes() != data]
        self.assertEqual(sorted(changes), sorted([binding.RESOURCE, binding.OBSERVER]))
        self.assertEqual(seam["resource_package_changes"],
                         [binding.RESOURCE, binding.OBSERVER, "package-inputs.json"])
        self.assertEqual(seam["observer_adapter"], self.captured["authority"]["unit2_observer_adapter"])
        self.assertEqual((root / binding.OBSERVER).read_bytes(), self.captured["observer"])
        modified = (root / binding.RESOURCE).read_bytes()
        self.assertEqual(modified.replace(binding.NEW_SEAM, binding.OLD_SEAM),
                         self.captured["historical_bytes"][binding.RESOURCE])
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
        listing = "\n".join(name + ": test" for name in names) + "\n\n4 tests, 0 benchmarks\n"
        results = [name + (" - should panic" if "_denies_" in name else "") for name in names]
        execution = "running 4 tests\n" + "\n".join("test " + name + " ... ok" for name in results)
        execution += "\n\ntest result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 44 filtered out; finished in 0.00s\n"
        return listing, execution

    def test_observer_control_protocol_rejects_incomplete_results(self):
        listing, execution = self.observer_control_streams()
        protocol = self.observer_protocol()
        self.assertEqual(binding.verify_observer_control_output(protocol, listing, execution)["passed"], 4)
        for changed_listing, changed_execution in (
                (listing.replace(binding.OBSERVER_CONTROL_NAMES[0] + ": test\n", ""), execution),
                (listing + "unexpected: test\n", execution),
                (listing, execution.replace(" ... ok", " ... ignored", 1)),
                (listing, execution.replace(" ... ok", " ... FAILED", 1)),
                (listing, execution.replace("4 passed", "0 passed")),
                (listing, execution.replace(" - should panic", "", 1))):
            with self.subTest(listing=changed_listing, execution=changed_execution), self.assertRaises(ValueError):
                binding.verify_observer_control_output(protocol, changed_listing, changed_execution)

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
        self.assertEqual(receipt["observer_control_tests_per_profile"], 4)
        self.assertEqual(receipt["test_function_executions"], 52)
        self.assertEqual(len(receipt["observer_control_receipts"]), 2)
        for row in receipt["observer_control_receipts"]:
            report = binding.read_json(output / row["path"])
            self.assertEqual(report["tests"], list(binding.OBSERVER_CONTROL_NAMES))
            self.assertEqual(report["observer_adapter"], seam["observer_adapter"])
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
        self.assertEqual((plan["current_source_members"], plan["predecessor_source_members"], plan["archive_members"]),
                         (133, 129, 117))
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
