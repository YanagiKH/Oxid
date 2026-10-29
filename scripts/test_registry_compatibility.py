"""Closed registry source admission; inherited default predicates remain exact."""
import argparse
import ast
import hashlib
import inspect
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_registry_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]
SDK_TRANSPORT_PIN = (3657, "73ed97d799025f258631fee2f8874f4ceb83fccfde5537a4a01cb8984b200224")
SDK_PREDECESSOR_TRANSPORT = b'          $PSNativeCommandArgumentPassing = \'Legacy\'\n          $settings = & $env:ComSpec /d /s /c "`"$devcmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set"\n          if ($LASTEXITCODE -ne 0) { throw \'Visual Studio environment initialization failed\' }\n'
REGISTRY_NATIVE_STEP = b'      - name: Verify private registry publication offline builds and deployment\n        run: |\n          python3 -B scripts/verify_typed_registry.py target/debug/oxid target/release/oxid \\\n            --output "$RUNNER_TEMP/current-typed-registry" --native\n'
REGISTRY_NATIVE_EVIDENCE = b'      - name: Retain private registry evidence\n        if: always()\n        uses: actions/upload-artifact@v4\n        with:\n          name: current-typed-registry\n          path: ${{ runner.temp }}/current-typed-registry\n          include-hidden-files: true\n          if-no-files-found: error\n          retention-days: 14\n'
PREEXISTING_NATIVE_LIBRARY_STEP = b'      - name: Verify returning native libraries from C Rust and Python\n        env:\n          TMPDIR: ${{ runner.temp }}/current-native-libraries\n          OXID_TEST_KEEP_ARTIFACTS: \'1\'\n        run: |\n          mkdir -p "$TMPDIR"\n          cargo test --locked --test native_library actual_same_library -- --ignored --nocapture\n          cargo test --release --locked --test native_library actual_same_library -- --ignored --nocapture\n'
PREEXISTING_NATIVE_LIBRARY_EVIDENCE = b'            ${{ runner.temp }}/current-native-libraries\n'


def git_body(commit, name):
    return subprocess.check_output(["git", "--no-replace-objects", "show", commit + ":" + name], cwd=ROOT)


def baseline_names():
    body = (ROOT / gate.BINDING / "current-source.json").read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class RegistryAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-private-registry-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// fixed R2 addition\n" for name in gate.PREDECESSOR_ADDED}}
        for name in gate.UNCHANGED_INPUTS:
            self.parent[name] = git_body(gate.PREDECESSOR, name)
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed private-registry input\n"
                             for name in gate.REGISTRY_CHANGED | gate.REGISTRY_ADDED})
        self.changed = "src/project_registry.rs"
        self.current[self.changed] = b"#[test]\nfn first(){}\n#[test]\nfn second(){}\n"
        for name, body in self.current.items():
            self.put(name, body)
        for name, value in (
            ("REVIEWED_INPUTS", {name: (len(self.current[name]), gate.sha(self.current[name]))
                                 for name in gate.REGISTRY_CHANGED | gate.REGISTRY_ADDED}),
            ("REVIEWED_DIRECTIVES", {name: gate.source_directives(name, self.current[name])
                                     for name in gate.REGISTRY_CHANGED | gate.REGISTRY_ADDED}),
        ):
            mocked = patch.object(gate, name, value)
            mocked.start()
            self.addCleanup(mocked.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_exact_scalar_library_roster_and_only_five_registry_modules_are_added(self):
        inputs = self.capture()
        self.assertEqual(len(self.baseline), 376)
        self.assertEqual(len(self.parent), 522)
        self.assertEqual(len(inputs), 527)
        self.assertEqual(gate.REGISTRY_ADDED, {
            "src/project_cli/registry.rs", "src/project_package/archive/library.rs",
            "src/project_registry.rs", "src/project_registry/resources.rs", "src/project_registry/tests.rs",
        })
        self.assertEqual(gate.REGISTRY_CHANGED, {
            "src/cli.rs", "src/main.rs", "src/project_cli.rs", "src/project_cli/documentation.rs",
            "src/project_cli/editor.rs",
            "src/project_cli/package.rs", "src/project_cli/workspace.rs", "src/project_lock.rs",
            "src/project_package.rs", "src/project_package/archive.rs",
        })
        self.assertEqual(set(inputs), set(self.parent) | gate.REGISTRY_ADDED)
        for name in set(self.parent) - gate.REGISTRY_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_manifest_binds_all_59_policies_and_keeps_every_claim_false(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual(manifest["source_policy_input_members"], 59)
        self.assertEqual(manifest["predecessor_compiler_input_members"], 522)
        self.assertEqual(manifest["source_policy_parent"], "8f2c29eaffabbd90c591bbdc440ae6e23acf1353")
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.current))
        self.assertEqual(len(manifest["files"]), 527)
        self.assertEqual(manifest["registry_pins_provisional"], gate.PINS_PROVISIONAL)
        claims = {key: value for key, value in manifest.items() if key.endswith("qualified_by_this_gate")}
        inherited = gate.library.current_manifest(self.parent, self.baseline, {}, self.parent)
        self.assertLessEqual({key for key in inherited if key.endswith("qualified_by_this_gate")}, set(claims))
        for name in ("copy_record_vectors_semantics", "windows_module_sources_semantics",
                     "windows_activation", "qualified_module_completion_semantics",
                     "copy_record_debug_semantics", "record_field_completion_semantics",
                     "file_seek_semantics", "seekable_streaming_datasets_semantics",
                     "scalar_native_library_semantics", "general_ffi_semantics"):
            self.assertIs(claims[name + "_qualified_by_this_gate"], False)
        self.assertTrue(claims)
        for key, value in claims.items():
            self.assertIs(value, False, key)
        for name in gate.UNCHANGED_INPUTS:
            self.assertEqual(manifest["retained_protected_inputs"][name], gate.identity(name, self.parent[name]))

    def test_windows_platform_doctor_PE_and_debug_claims_stay_false_after_pin_approval(self):
        inputs = self.capture()
        with patch.object(gate, "PINS_PROVISIONAL", False):
            manifest = gate.current_manifest(inputs, self.baseline, {}, self.parent)
        self.assertIs(manifest["registry_pins_provisional"], False)
        for key in (
            "windows_native_semantics_qualified_by_this_gate",
            "win32_native_doctor_semantics_qualified_by_this_gate",
            "windows_native_platform_semantics_qualified_by_this_gate",
            "windows_native_debug_semantics_qualified_by_this_gate",
            "msvc_pe_execution_qualified_by_this_gate",
            "copy_record_debug_semantics_qualified_by_this_gate",
            "qualified_module_completion_semantics_qualified_by_this_gate",
            "record_field_completion_semantics_qualified_by_this_gate",
            "file_seek_semantics_qualified_by_this_gate",
            "seekable_streaming_datasets_semantics_qualified_by_this_gate",
            "scalar_native_library_semantics_qualified_by_this_gate",
            "general_ffi_semantics_qualified_by_this_gate",
        ):
            self.assertIs(manifest[key], False, key)
        self.assertIs(manifest["debug_information_enabled_for_retained_claim"], False)

    def test_source_hash_and_length_are_both_required(self):
        self.put(self.changed, self.current[self.changed] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()
        self.put(self.changed, self.current[self.changed])
        length, digest = gate.REVIEWED_INPUTS[self.changed]
        gate.REVIEWED_INPUTS[self.changed] = (length + 1, digest)
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()

    def test_each_new_library_module_requires_its_hash_and_ordered_compile_time_inputs(self):
        self.assertEqual(self.capture(), self.current)
        for name in sorted(gate.REGISTRY_ADDED):
            original = self.current[name]
            changed = original + b"#[cfg(any())]\nfn altered_library(){}\n"
            self.put(name, changed)
            with self.subTest(name=name, seam="hash"), self.assertRaisesRegex(
                    ValueError, "does not match reviewed input"):
                self.capture()
            with patch.dict(gate.REVIEWED_INPUTS, {name: (len(changed), gate.sha(changed))}):
                with self.subTest(name=name, seam="directive"), self.assertRaisesRegex(
                        ValueError, "directive changed"):
                    self.capture()
            self.put(name, original)
        self.assertEqual(self.capture(), self.current)

    def test_ordered_repeated_compile_time_directives_stay_complete(self):
        original = gate.REVIEWED_DIRECTIVES[self.changed]
        self.assertEqual(original["attributes"], ((b"#[test]", 2),))
        for attributes in (((b"#[test]", 1),), ((b"#[test]", 3),),
                           ((b"#[ignore]", 1), (b"#[test]", 1)),
                           ((b"#[test]", 1), (b"#[ignore]", 1))):
            gate.REVIEWED_DIRECTIVES[self.changed] = {**original, "attributes": attributes}
            with self.subTest(attributes=attributes), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        for field in ("macros", "c_preprocessor", "legacy", "build_script"):
            gate.REVIEWED_DIRECTIVES[self.changed] = {**original, field: ((b"unreviewed", 1),)}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        gate.REVIEWED_DIRECTIVES[self.changed] = original

    def test_reviewed_pin_and_directive_membership_must_be_exact(self):
        pin = gate.REVIEWED_INPUTS.pop(self.changed)
        with self.assertRaisesRegex(ValueError, "reviewed private-registry input membership"):
            self.capture()
        gate.REVIEWED_INPUTS[self.changed] = pin
        directives = gate.REVIEWED_DIRECTIVES.pop(self.changed)
        with self.assertRaisesRegex(ValueError, "directive membership"):
            self.capture()
        gate.REVIEWED_DIRECTIVES[self.changed] = directives
        gate.REVIEWED_DIRECTIVES["src/unreviewed.rs"] = directives
        with self.assertRaisesRegex(ValueError, "directive membership"):
            self.capture()

    def test_unchanged_source_stays_closed(self):
        for name in ("src/frontend/ast.rs", "src/frontend/source.rs",
                     "src/frontend/oir/source/association.rs", "native/typed_preview.c"):
            self.put(name, self.parent[name] + b"// changed\n")
            with self.subTest(name=name), self.assertRaisesRegex(
                    ValueError, "outside private-registry boundary"):
                self.capture()
            self.put(name, self.parent[name])

    def test_already_accepted_windows_dispatch_bytes_cannot_change(self):
        name, = gate.UNCHANGED_INPUTS
        self.put(name, self.parent[name] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "outside private-registry boundary"):
            self.capture()

    def test_protected_member_cannot_enter_generic_reviewed_delta(self):
        name, = gate.UNCHANGED_INPUTS
        with patch.object(gate, "REGISTRY_CHANGED", gate.REGISTRY_CHANGED | {name}):
            with self.assertRaisesRegex(ValueError, "protected input cannot enter"):
                self.capture()

    def test_frontend_and_native_cannot_enter_even_with_replaced_review_pins(self):
        for name in ("src/frontend/ast.rs", "native/typed_preview.c"):
            body = self.current[name]
            with patch.object(gate, "REGISTRY_CHANGED", gate.REGISTRY_CHANGED | {name}), \
                    patch.dict(gate.REVIEWED_INPUTS, {name: (len(body), gate.sha(body))}), \
                    patch.dict(gate.REVIEWED_DIRECTIVES, {name: gate.source_directives(name, body)}):
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "inherited frontend or native"):
                    self.capture()

    def test_extra_and_missing_compiler_members_are_refused(self):
        extra = self.root / "src/unreviewed.rs"
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        (self.root / sorted(gate.REGISTRY_ADDED)[0]).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()

    def test_empty_unlisted_directory_is_refused(self):
        (self.root / "src/unreviewed").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_parent_member_loss_cannot_lower_the_roster(self):
        name = sorted(gate.PREDECESSOR_ADDED)[0]
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, "scalar-library compiler input membership"):
            self.capture()
        with patch.object(gate, "PREDECESSOR_ADDED", gate.PREDECESSOR_ADDED - {name}):
            with self.assertRaisesRegex(ValueError, "scalar-library compiler input membership"):
                self.capture()

    def test_source_symlink_hardlink_and_executable_are_refused(self):
        path = self.root / self.changed
        alias = self.root / "aliased-source"
        path.rename(alias)
        path.symlink_to(alias)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()
        path.unlink()
        path.write_bytes(self.current[self.changed])
        alias.unlink()
        os.link(path, alias)
        with self.assertRaisesRegex(ValueError, "linked"):
            self.capture()
        alias.unlink()
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "executable"):
            self.capture()


class FrozenScalarLibraryReplay(unittest.TestCase):
    def restore_reviewed_sdk_transport(self, current):
        start = b"          $sdkStart = [System.Diagnostics.ProcessStartInfo]::new()\n"
        end = (b"          $names = @('Path', 'INCLUDE', 'LIB', 'LIBPATH', "
               b"'VCINSTALLDIR', 'VCToolsInstallDir',\n")
        self.assertEqual(current.count(start), 1)
        self.assertEqual(current.count(end), 1)
        self.assertEqual(current.count(SDK_PREDECESSOR_TRANSPORT), 0)
        begin, finish = current.index(start), current.index(end)
        self.assertLess(begin, finish)
        windows = re.search(rb"^  current-windows-native:.*?(?=^  [A-Za-z0-9_-]+:|\Z)",
                            current, flags=re.M | re.S)
        self.assertIsNotNone(windows)
        step = re.search(rb"^      - name: Initialize the installed x64 Visual Studio and Windows SDK\n.*?(?=^      -|\Z)",
                         windows.group(), flags=re.M | re.S)
        self.assertIsNotNone(step)
        self.assertEqual(step.group().count(start), 1)
        self.assertEqual(step.group().count(end), 1)
        body = current[begin:finish]
        self.assertEqual((len(body), gate.sha(body)), SDK_TRANSPORT_PIN)
        return current[:begin] + SDK_PREDECESSOR_TRANSPORT + current[finish:]

    def test_reviewed_sdk_transport_normalizes_only_its_exact_bounded_process_block(self):
        current = gate.regular(ROOT, ".github/workflows/ci.yml")
        restored = self.restore_reviewed_sdk_transport(current)
        self.assertEqual(restored.count(SDK_PREDECESSOR_TRANSPORT), 1)
        parent = git_body(gate.PREDECESSOR, ".github/workflows/ci.yml")
        self.assertEqual(parent.count(SDK_PREDECESSOR_TRANSPORT), 1)
        # The 34-test predecessor is immutable in this successor. The separate
        # PR159 35-test normalizer is never substituted for its fixed replay.
        source = gate.regular(ROOT, "scripts/test_scalar_library_compatibility.py")
        self.assertEqual(gate.sha(source), "9315dbe8332a349b88f617a7a55cc0b6b0795fbb4952885ad8f3e2fd4f5429fa")
        self.assertEqual(source, git_body(gate.PREDECESSOR, "scripts/test_scalar_library_compatibility.py"))
        methods = [node for node in ast.walk(ast.parse(source))
                   if isinstance(node, ast.FunctionDef) and node.name.startswith("test_")]
        self.assertEqual(len(methods), 34)

    def test_sdk_normalization_rejects_transport_caps_encoding_exit_and_cleanup_mutations(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        changes = (
            (b"Elapsed.TotalSeconds -ge 120", b"Elapsed.TotalSeconds -ge 0"),
            (b"$sdkOut.Length + $count -gt 1048576", b"$sdkOut.Length + $count -gt 2147483647"),
            (b"$sdkErr.Length + $count -gt 65536", b"$sdkErr.Length + $count -gt 2147483647"),
            (b"$outBuffer = [char[]]::new(4096)", b"$outBuffer = [char[]]::new(1048576)"),
            (b"$errBuffer = [char[]]::new(4096)", b"$errBuffer = [char[]]::new(1048576)"),
            (b"$sdkStart.RedirectStandardInput = $true", b"$sdkStart.RedirectStandardInput = $false"),
            (b"$sdkStart.RedirectStandardError = $true", b"$sdkStart.RedirectStandardError = $false"),
            (b"$sdkStart.StandardOutputEncoding = [System.Text.Encoding]::Unicode",
             b"$sdkStart.StandardOutputEncoding = [System.Text.Encoding]::UTF8"),
            (b"$sdkStart.StandardErrorEncoding = [System.Text.Encoding]::Unicode",
             b"$sdkStart.StandardErrorEncoding = [System.Text.Encoding]::UTF8"),
            (b"$sdkStart.UseShellExecute = $false", b"$sdkStart.UseShellExecute = $true"),
            (b"$sdkStart.Arguments = '/d /u /s /c", b"$sdkStart.Arguments = '/d /s /c"),
            (b' -no_logo -arch=x64 -host_arch=x64 >nul', b' "-no_logo" -arch=x64 -host_arch=x64 >nul'),
            (b' -no_logo -arch=x64 -host_arch=x64 >nul', b' -no_logo "-arch=x64" -host_arch=x64 >nul'),
            (b' -no_logo -arch=x64 -host_arch=x64 >nul', b' -no_logo -arch=x64 "-host_arch=x64" >nul'),
            (b"$sdk.StandardInput.Close()", b"# removed input close"),
            (b"if ($sdk.ExitCode -ne 0)", b"if ($false)"),
            (b"Min(256, $sdkErrorText.Length)", b"Min(65536, $sdkErrorText.Length)"),
            (b"$settings = $sdkOut.ToString() -split '\\r?\\n'", b"$settings = @()"),
            (b"$sdk.Kill($true)", b"$sdk.Kill($false)"),
            (b"$sdk.WaitForExit(5000)", b"$sdk.WaitForExit(0)"),
            (b"$sdk.Dispose()", b"# removed dispose"),
        )
        regular = gate.regular
        for index, (before, after) in enumerate(changes):
            self.assertEqual(current.count(before), 1, index)
            changed = current.replace(before, after, 1)
            with self.subTest(case=index), self.assertRaises(AssertionError):
                self.restore_reviewed_sdk_transport(changed)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(full_comparison=index), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_CI_changes_only_the_gate_replay_job_and_exact_registry_native_step()

    def test_sdk_normalization_cannot_hide_extra_blocks_allowlist_or_surrounding_workflow_edits(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        start = b"          $sdkStart = [System.Diagnostics.ProcessStartInfo]::new()\n"
        end = (b"          $names = @('Path', 'INCLUDE', 'LIB', 'LIBPATH', "
               b"'VCINSTALLDIR', 'VCToolsInstallDir',\n")
        block = current[current.index(start):current.index(end)]
        regular = gate.regular
        outside = (current.replace(end, end.replace(b"'LIBPATH',", b"'UNTRUSTED',"), 1),
                   current.replace(b"'VCToolsVersion', 'WindowsSdkDir'", b"'UNTRUSTED', 'WindowsSdkDir'", 1),
                   current + b"\n# unrelated SDK workflow change\n")
        for changed in (current.replace(block, block * 2, 1),
                        current.replace(block, b"", 1),
                        current.replace(block, block + b"          # hidden extra transport\n", 1)):
            with self.subTest(block_mutation=gate.sha(changed)), self.assertRaises(AssertionError):
                self.restore_reviewed_sdk_transport(changed)
        for index, changed in enumerate(outside):
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(outside=index), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_CI_changes_only_the_gate_replay_job_and_exact_registry_native_step()

    def test_registry_native_acceptance_requires_both_current_compilers_and_retained_evidence(self):
        current = gate.regular(ROOT, ".github/workflows/ci.yml")
        job = re.search(rb"^  native-numeric-and-packages:.*?(?=^  [A-Za-z0-9_-]+:|\Z)",
                        current, flags=re.M | re.S)
        self.assertIsNotNone(job)
        job = job.group()
        self.assertNotRegex(job, rb"(?m)^    (?:if|needs):|continue-on-error:")
        self.assertEqual(current.count(REGISTRY_NATIVE_STEP), 1)
        self.assertEqual(job.count(REGISTRY_NATIVE_STEP), 1)
        self.assertEqual(job.count(b"          cargo build --locked\n"), 1)
        self.assertEqual(job.count(b"          cargo build --release --locked\n"), 1)
        self.assertLess(job.index(b"          cargo build --locked\n"), job.index(REGISTRY_NATIVE_STEP))
        self.assertLess(job.index(b"          cargo build --release --locked\n"), job.index(REGISTRY_NATIVE_STEP))
        self.assertIn(b"      - name: Retain native numeric and package evidence\n", job)
        self.assertLess(job.index(REGISTRY_NATIVE_STEP),
                        job.index(b"      - name: Retain native numeric and package evidence\n"))
        upload = re.search(rb"^      - name: Retain private registry evidence\n.*?(?=^      -|\Z)",
                           job, flags=re.M | re.S)
        self.assertIsNotNone(upload)
        self.assertEqual(current.count(REGISTRY_NATIVE_EVIDENCE), 1)
        self.assertEqual(upload.group().count(REGISTRY_NATIVE_EVIDENCE), 1)
        self.assertIn(b"        if: always()\n", upload.group())
        self.assertIn(b"        uses: actions/upload-artifact@v4\n", upload.group())
        self.assertIn(b"          include-hidden-files: true\n", upload.group())
        self.assertIn(b"          if-no-files-found: error\n", upload.group())
        self.assertIn(b"          retention-days: 14\n", upload.group())

    def test_registry_native_CI_refuses_lost_profiles_optional_execution_and_missing_artifacts(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        marker = b"      - name: Verify private registry publication offline builds and deployment\n"
        command = b"python3 -B scripts/verify_typed_registry.py target/debug/oxid target/release/oxid"
        output = b'            --output "$RUNNER_TEMP/current-typed-registry" --native\n'
        debug = b"target/debug/oxid"
        release = b"target/release/oxid"
        mutations = [
            ("missing-step", REGISTRY_NATIVE_STEP, b""),
            ("duplicate-step", REGISTRY_NATIVE_STEP, REGISTRY_NATIVE_STEP * 2),
            ("wrong-verifier", command, command.replace(b"verify_typed_registry.py", b"other_registry.py")),
            ("missing-debug", command, command.replace(debug + b" ", b"")),
            ("missing-release", command, command.replace(b" " + release, b"")),
            ("duplicated-debug", command, command.replace(release, debug)),
            ("duplicated-release", command, command.replace(debug, release)),
            ("not-native", output, output.replace(b" --native", b"")),
            ("reference-only", output, output.replace(b" --native", b" --reference-only")),
            ("unretained-output", output, output.replace(b"current-typed-registry", b"unretained")),
            ("soft-failed-command", output, output.replace(b" --native", b" --native || true")),
            ("conditional-step", marker, marker + b"        if: false\n"),
            ("soft-failed-step", marker, marker + b"        continue-on-error: true\n"),
            ("conditional-job", b"  native-numeric-and-packages:\n",
             b"  native-numeric-and-packages:\n    if: false\n"),
            ("missing-evidence", REGISTRY_NATIVE_EVIDENCE, b""),
            ("duplicate-evidence", REGISTRY_NATIVE_EVIDENCE, REGISTRY_NATIVE_EVIDENCE * 2),
            ("unretained-evidence", REGISTRY_NATIVE_EVIDENCE,
             REGISTRY_NATIVE_EVIDENCE.replace(b"current-typed-registry", b"unretained")),
        ]
        native = re.search(rb"^  native-numeric-and-packages:.*?(?=^  [A-Za-z0-9_-]+:|\Z)",
                           current, flags=re.M | re.S)
        self.assertIsNotNone(native)
        native = native.group()
        for label, before, after in (
            ("missing-debug-build", b"          cargo build --locked\n", b""),
            ("missing-release-build", b"          cargo build --release --locked\n", b""),
        ):
            self.assertEqual(native.count(before), 1, label)
            mutations.append((label, native, native.replace(before, after, 1)))
        for label, before, after in (
            ("success-only-artifact", b"        if: always()\n", b"        if: success()\n"),
            ("missing-artifact-is-soft", b"          if-no-files-found: error\n",
             b"          if-no-files-found: warn\n"),
            ("lost-hidden-cache-files", b"          include-hidden-files: true\n", b""),
            ("lost-retention", b"          retention-days: 14\n", b""),
        ):
            self.assertEqual(REGISTRY_NATIVE_EVIDENCE.count(before), 1, label)
            mutations.append((label, REGISTRY_NATIVE_EVIDENCE,
                              REGISTRY_NATIVE_EVIDENCE.replace(before, after, 1)))
        regular = gate.regular
        for label, before, after in mutations:
            self.assertEqual(current.count(before), 1, label)
            changed = current.replace(before, after, 1)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(case=label), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_registry_native_acceptance_requires_both_current_compilers_and_retained_evidence()
                with self.assertRaises(AssertionError):
                    self.test_CI_changes_only_the_gate_replay_job_and_exact_registry_native_step()

    def test_preexisting_G7_native_library_step_and_artifact_remain_exact(self):
        current = gate.regular(ROOT, ".github/workflows/ci.yml")
        parent = git_body(gate.PREDECESSOR, ".github/workflows/ci.yml")
        self.assertEqual(len(PREEXISTING_NATIVE_LIBRARY_STEP), 434)
        self.assertEqual(len(PREEXISTING_NATIVE_LIBRARY_EVIDENCE), 56)
        for body in (current, parent):
            self.assertEqual(body.count(PREEXISTING_NATIVE_LIBRARY_STEP), 1)
            self.assertEqual(body.count(PREEXISTING_NATIVE_LIBRARY_EVIDENCE), 1)

    def test_CI_changes_only_the_gate_replay_job_and_exact_registry_native_step(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        pattern = rb"^  scalar-library-source-policy-predecessor:.*?(?=^  [A-Za-z0-9_-]+:|\Z)"
        jobs = re.findall(pattern, current, flags=re.M | re.S)
        self.assertEqual(len(jobs), 1)
        self.assertEqual((len(jobs[0]), gate.sha(jobs[0])),
                         (1075, "99b03e1e2b32f3a9890492b43a1716a34e953919d18d43c1f0eb27321e5a05ac"))
        self.assertEqual(current.count(jobs[0]), 1)
        restored = current.replace(jobs[0], b"", 1)
        self.assertEqual(restored.count(REGISTRY_NATIVE_STEP), 1)
        self.assertEqual(restored.count(REGISTRY_NATIVE_EVIDENCE), 1)
        restored = restored.replace(REGISTRY_NATIVE_STEP, b"", 1).replace(REGISTRY_NATIVE_EVIDENCE, b"", 1)
        restored = self.restore_reviewed_sdk_transport(restored)
        entrypoint = b"scripts/verify_registry_compatibility.py"
        self.assertEqual(restored.count(entrypoint), 1)
        self.assertEqual(restored.replace(entrypoint, b"scripts/verify_scalar_library_compatibility.py", 1),
                         git_body(gate.PREDECESSOR, name))

    def test_CI_boundary_refuses_unrelated_unquoted_or_quoted_jobs(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        anchor = b"  macos-native:\n"
        self.assertEqual(current.count(anchor), 1)
        regular = gate.regular
        for heading in (b"unrelated-job", b"'unrelated-job'", b'"unrelated-job"'):
            extra = b"  " + heading + b":\n    runs-on: ubuntu-latest\n    steps:\n      - run: true\n\n"
            changed = current.replace(anchor, extra + anchor, 1)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(heading=heading), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_CI_changes_only_the_gate_replay_job_and_exact_registry_native_step()

    def test_CI_boundary_refuses_lost_replay_inputs_execution_and_evidence(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        match = re.search(rb"^  scalar-library-source-policy-predecessor:.*?(?=^  [A-Za-z0-9_-]+:|\Z)",
                          current, flags=re.M | re.S)
        self.assertIsNotNone(match)
        original = match.group()
        command = (b'      - run: python3 -B "$RUNNER_TEMP/scalar-library-source-policy-replay.py" '
                   b'--repo . --output "$RUNNER_TEMP/scalar-library-source-policy-replay"\n')
        cases = [
            original.replace(b"    runs-on: ubuntu-latest\n", b"    if: false\n    runs-on: ubuntu-latest\n", 1),
            original.replace(b"    steps:\n", b"    continue-on-error: true\n    steps:\n", 1),
            original.replace(gate.PREDECESSOR.encode(), b"0" * 40, 1),
            original.replace(b"          fetch-depth: 0\n", b"", 1),
            original.replace(b"replay_scalar_library_source_policy.py", b"other_policy.py", 1),
            original.replace(command, b"", 1),
            original.replace(command, command + command, 1),
            original.replace(b"        if: always()\n", b"        if: success()\n", 1),
            original.replace(b"          if-no-files-found: error\n", b"          if-no-files-found: warn\n", 1),
            original.replace(b"          retention-days: 14\n", b"", 1),
            original.replace(b"          path: ${{ runner.temp }}/scalar-library-source-policy-replay\n", b"", 1),
        ]
        regular = gate.regular
        for index, job in enumerate(cases):
            self.assertNotEqual(job, original)
            changed = current.replace(original, job, 1)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(case=index), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_CI_changes_only_the_gate_replay_job_and_exact_registry_native_step()

    def test_existing_native_library_commands_test_selector_and_host_guard_stay_exact(self):
        name = "tests/native_library.rs"
        current = gate.regular(ROOT, name)
        self.assertEqual(current, git_body(gate.PREDECESSOR, name))
        selector = b"actual_same_library_is_called_by_c_rust_python_with_returning_errors_and_concurrent_calls"
        self.assertEqual(re.findall(rb"#\[test\]\s*#\[ignore[^\]]*\]\s*fn (actual_same_library\w*)\s*\(", current),
                         [selector])
        self.assertEqual(current.count(b"fn " + selector + b"("), 1)
        self.assertRegex(current,
            rb'#\[cfg\(all\(\s*target_os = "linux",\s*target_pointer_width = "64",\s*'
            rb'target_endian = "little",\s*any\(target_arch = "x86_64", target_arch = "aarch64"\)\s*'
            rb'\)\)\]\s*mod linux \{')

    def test_current_native_test_binding_refuses_selector_attributes_and_host_changes(self):
        name = "tests/native_library.rs"
        current = gate.regular(ROOT, name)
        selector = b"actual_same_library_is_called_by_c_rust_python_with_returning_errors_and_concurrent_calls"
        mutations = (
            current.replace(b"fn " + selector, b"fn renamed_native_client", 1),
            current.replace(b"    #[test]\n", b"", 1),
            current.replace(b"    #[ignore", b"    #[allow", 1),
            current.replace(b'target_os = "linux"', b'target_os = "windows"', 1),
            current + b"\nfn " + selector + b"() {}\n",
        )
        regular = gate.regular
        for index, changed in enumerate(mutations):
            self.assertNotEqual(changed, current)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(case=index), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_existing_native_library_commands_test_selector_and_host_guard_stay_exact()

    def test_reviewed_inputs_bind_only_the_integrated_reachable_registry_checkpoint(self):
        frozen = "5a8d74ad33dafccba39aa23870ad78ca16aa9c28"
        subprocess.check_call(["git", "--no-replace-objects", "merge-base", "--is-ancestor",
                               frozen, "HEAD"], cwd=ROOT)
        self.assertEqual(len(gate.REGISTRY_CHANGED), 10)
        self.assertEqual(len(gate.REGISTRY_ADDED), 5)
        self.assertEqual(set(gate.REVIEWED_INPUTS), gate.REGISTRY_CHANGED | gate.REGISTRY_ADDED)
        self.assertEqual(set(gate.REVIEWED_DIRECTIVES), set(gate.REVIEWED_INPUTS))
        for name, pin in gate.REVIEWED_INPUTS.items():
            body = git_body(frozen, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
            self.assertEqual((len(body), gate.sha(body)), pin, name)
            self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name], name)

    def test_all_59_predecessor_policies_and_adapters_are_exact(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 59)
        self.assertEqual(gate.PREDECESSOR_POLICIES, {
            **gate.library.PREDECESSOR_POLICIES,
            "scripts/verify_scalar_library_compatibility.py": gate.PREDECESSOR_POLICY_PIN[1],
        })
        for name, expected in gate.PREDECESSOR_POLICIES.items():
            body = git_body(gate.PREDECESSOR, name)
            self.assertEqual(gate.sha(body), expected, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
        old = git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY)
        self.assertEqual((len(old), gate.sha(old)), gate.PREDECESSOR_POLICY_PIN)

    def test_policy_authentication_refuses_current_and_shared_parent_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            parent, current = Path(directory) / "parent", Path(directory) / "current"
            for name in gate.PREDECESSOR_POLICIES:
                body = git_body(gate.PREDECESSOR, name)
                for root in (parent, current):
                    (root / name).parent.mkdir(parents=True, exist_ok=True)
                    (root / name).write_bytes(body)
            gate.authenticate_policies(current, parent)
            for name in (gate.PREDECESSOR_POLICY, gate.library.OBSERVER_ADAPTER,
                         "scripts/record_vector_dispatch_activation.py"):
                original = (current / name).read_bytes()
                (current / name).write_bytes(original + b"# changed\n")
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "policy changed"):
                    gate.authenticate_policies(current, parent)
                (parent / name).write_bytes(original + b"# changed\n")
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "wrong pinned predecessor"):
                    gate.authenticate_policies(current, parent)
                (current / name).write_bytes(original)
                (parent / name).write_bytes(original)

    def test_pinned_policy_is_not_executed_after_a_hash_or_mode_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / gate.PREDECESSOR_POLICY
            path.parent.mkdir(parents=True)
            body = git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY)
            path.write_bytes(body + b"# changed\n")
            with patch.object(gate, "exec", create=True) as execute:
                with self.assertRaisesRegex(ValueError, "changed pinned scalar-library policy"):
                    gate.load_predecessor(root)
                execute.assert_not_called()
            path.write_bytes(body)
            path.chmod(0o755)
            with patch.object(gate, "exec", create=True) as execute:
                with self.assertRaisesRegex(ValueError, "unsafe pinned scalar-library policy"):
                    gate.load_predecessor(root)
                execute.assert_not_called()

    def test_parent_blob_mode_and_identity_bind_the_fixed_qualified_predecessor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            body = b"// parent source\n"
            (root / "source.rs").write_bytes(body)
            oid = hashlib.sha1(b"blob " + str(len(body)).encode() + b"\0" + body,
                               usedforsecurity=False).hexdigest()
            row = f"100644 blob {oid} {len(body)}\tsource.rs\0".encode()
            with patch.object(gate, "git", return_value=row) as git:
                self.assertEqual(gate.capture_parent(ROOT, root, {"source.rs"}), {"source.rs": body})
            git.assert_called_once_with(ROOT, "ls-tree", "-rzl", "--full-tree", gate.PREDECESSOR)
            for changed in (row.replace(b"100644", b"100755"), row.replace(oid.encode(), b"0" * 40),
                            row.replace(str(len(body)).encode(), str(len(body) + 1).encode(), 1)):
                with patch.object(gate, "git", return_value=changed), self.assertRaisesRegex(ValueError, "pinned Git blob"):
                    gate.capture_parent(ROOT, root, {"source.rs"})

    def test_both_complete_retained_execution_blocks_are_unchanged(self):
        def replay(body):
            tree = ast.parse(body)
            run = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "run")
            return [node for node in ast.walk(run) if isinstance(node, ast.If)
                    and ast.unparse(node.test) == "not args.prepare_only"]
        path = Path(gate.__file__).resolve()
        self.assertEqual(path, ROOT / "scripts/verify_registry_compatibility.py")
        current = replay(path.read_bytes())
        predecessor = replay(git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY))
        self.assertEqual(len(current), 2)
        self.assertEqual(len(predecessor), 2)
        self.assertEqual([ast.dump(node, include_attributes=False) for node in current],
                         [ast.dump(node, include_attributes=False) for node in predecessor])
        self.assertGreater(len(current[0].body), 10)
        self.assertIn("run_record_vector_observer_controls", ast.unparse(current[1]))

    def test_retained_block_comparison_reads_and_rejects_mutated_successor_blocks(self):
        current_path = Path(gate.__file__).resolve()
        original = current_path.read_bytes()
        read_bytes = Path.read_bytes
        for index in (0, 1):
            parsed = ast.parse(original)
            run = next(node for node in parsed.body if isinstance(node, ast.FunctionDef) and node.name == "run")
            blocks = [node for node in ast.walk(run) if isinstance(node, ast.If)
                      and ast.unparse(node.test) == "not args.prepare_only"]
            self.assertEqual(len(blocks), 2)
            blocks[index].body.pop()
            changed = ast.unparse(parsed).encode()
            def selected(path):
                return changed if path.resolve() == current_path else read_bytes(path)
            with self.subTest(block=index), patch.object(Path, "read_bytes", new=selected):
                with self.assertRaises(AssertionError):
                    self.test_both_complete_retained_execution_blocks_are_unchanged()

    def test_all_frontend_native_carriers_witnesses_layouts_and_caps_stay_exact(self):
        rows = subprocess.check_output(["git", "--no-replace-objects", "ls-tree", "-rz", "--name-only",
                                        gate.PREDECESSOR, "--", "src/frontend", "native"], cwd=ROOT)
        names = [name.decode() for name in rows.split(b"\0") if name]
        self.assertGreater(len(names), 350)
        for name in names:
            self.assertNotIn(name, gate.REGISTRY_CHANGED | gate.REGISTRY_ADDED)
            self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name), name)

    def test_both_narrow_observer_closures_and_all_frozen_wrappers_stay_unchanged(self):
        for kind, count in (("parser", 40), ("static", 41)):
            name = "scripts/build_typed_" + kind + "_observer.py"
            body = gate.regular(ROOT, name)
            self.assertEqual(body, git_body(gate.PREDECESSOR, name), name)
            definitions = {node.targets[0].id: ast.literal_eval(node.value)
                           for node in ast.parse(body).body if isinstance(node, ast.Assign)
                           and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
                           and node.targets[0].id in ("SOURCE_FILES", "WRAPPER_FILES")}
            sources = {"src/frontend/" + path for path in definitions["SOURCE_FILES"]}
            self.assertEqual(len(sources), count)
            self.assertFalse(sources & (gate.REGISTRY_CHANGED | gate.REGISTRY_ADDED))
            for source in sources:
                self.assertEqual(gate.regular(ROOT, source), git_body(gate.PREDECESSOR, source), source)
            for wrapper, _ in definitions["WRAPPER_FILES"]:
                name = "tests/fixtures/bounded_typed_" + kind + "/observer/" + wrapper
                self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name), name)

    def test_every_inherited_control_function_and_resource_preparation_is_unmodified(self):
        for name in (
            "run_byte_buffer_observer_controls", "run_file_observer_controls", "run_i64_observer_controls",
            "run_integer_width_observer_controls", "run_integer128_observer_controls",
            "run_float_observer_controls", "run_character_observer_controls", "run_vector_observer_controls",
            "run_record_vector_observer_controls", "authenticate_index_resource_adapter",
            "authenticate_parser_resource_adapter", "verify_current_observer", "verify_materialized",
        ):
            self.assertIs(getattr(gate, name), getattr(gate.library, name), name)
        self.assertIn("return library.prepare_current_observer", inspect.getsource(gate.prepare_current_observer))
        with patch.object(gate.library, "prepare_current_observer", return_value=("selected", "seam")) as prepare:
            self.assertEqual(gate.prepare_current_observer(1, 2, 3, 4, 5, 6), ("selected", "seam"))
        prepare.assert_called_once_with(1, 2, 3, 4, 5, 6)

    def test_provisional_pins_refuse_qualification_before_any_process(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result"
            args = argparse.Namespace(repo=ROOT, output=output, prepare_only=False)
            with patch.object(gate, "PINS_PROVISIONAL", True), patch.object(gate.subprocess, "run") as run:
                self.assertEqual(gate.run(args), 1)
            run.assert_not_called()
            result = json.loads((output / "failure.json").read_text())
            self.assertEqual(result["status"], "failed")
            self.assertIs(result["semantic_pass"], False)
            self.assertEqual(result["compiler_executions"], 0)
            self.assertIn("prepare-only", result["error"])
            self.assertFalse((output / "result.json").exists())
            self.assertFalse((output / "prepared.json").exists())


