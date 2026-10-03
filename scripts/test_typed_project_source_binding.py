"""Source-view admission controls; no compiler build or semantic expectation."""
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

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

    def test_current_and_archived_views_are_distinct_and_exact(self):
        captured = binding.preflight(self.repo, self.package)
        self.assertEqual(len(captured["inputs"]), 120)
        self.assertEqual(len(captured["archived"]), 117)
        self.assertNotEqual(captured["inputs"]["src/frontend/driver.rs"], captured["archived"]["src/frontend/driver.rs"])
        output = self.root / "archive"
        output.mkdir()
        receipt = binding.prepare_archived(output, captured)
        self.assertEqual(receipt["compiler_executions"], 0)
        self.assertFalse(receipt["semantic_pass"])
        binding.check_entries(output / "archived-selected", captured["selected"]["files"], exact=True)

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
        self.assertEqual(changes, [binding.RESOURCE])
        modified = (root / binding.RESOURCE).read_bytes()
        self.assertEqual(modified.replace(binding.NEW_SEAM, binding.OLD_SEAM),
                         self.captured["historical_bytes"][binding.RESOURCE])
        self.assertEqual((Path(seam["compatibility_runner"])).read_bytes(), self.captured["references"][binding.COMPAT])

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
