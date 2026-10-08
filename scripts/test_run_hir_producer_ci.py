"""Admission tests for hosted exact-build producer qualification."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

from run_hir_producer_ci import (Runner, digest, select_executable, validate_v1_summary,
                                 validate_edge_summary, validate_source_manifest)


class BuildReceiptControls(unittest.TestCase):
    def fixture(self, root, profile="debug"):
        executable = root / profile / "oxid"
        executable.parent.mkdir()
        executable.write_bytes(b"selected compiler")
        artifact = dict(reason="compiler-artifact", target=dict(name="oxid", kind=["bin"]),
                        profile=dict(test=False, opt_level="0" if profile == "debug" else "3"),
                        executable=str(executable))
        return executable, artifact

    def data(self, records, success=True):
        return b"\n".join(json.dumps(r).encode() for r in records + [dict(reason="build-finished", success=success)])

    def test_selects_actual_ordinary_non_test_executable(self):
        for profile in ("debug", "release"):
            with self.subTest(profile=profile), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                executable, artifact = self.fixture(root, profile)
                selected, record = select_executable(self.data([artifact]), root, profile)
                self.assertEqual(selected, executable)
                self.assertEqual(record, artifact)

    def test_missing_duplicate_wrong_profile_or_test_artifact_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _, artifact = self.fixture(root)
            cases = [[], [artifact, artifact],
                     [dict(artifact, profile=dict(test=True, opt_level="0"))],
                     [dict(artifact, profile=dict(test=False, opt_level="3"))]]
            for case in cases:
                with self.subTest(case=case), self.assertRaises(RuntimeError):
                    select_executable(self.data(case), root, "debug")
            with self.assertRaises(RuntimeError):
                select_executable(self.data([artifact], success=False), root, "debug")

    def test_outside_path_and_symlink_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            executable, artifact = self.fixture(root)
            outside = root / "outside"
            outside.write_bytes(b"other")
            with self.assertRaises(RuntimeError):
                select_executable(self.data([dict(artifact, executable=str(outside))]), root, "debug")
            executable.unlink()
            executable.symlink_to(outside)
            with self.assertRaises(RuntimeError):
                select_executable(self.data([artifact]), root, "debug")

    def test_failed_and_timed_out_commands_retain_incomplete_evidence(self):
        for result in (subprocess.CompletedProcess(["fake"], 7, b"out", b"err"),
                       subprocess.TimeoutExpired(["fake"], 1, output=b"out", stderr=b"err")):
            with self.subTest(result=result), tempfile.TemporaryDirectory() as tmp:
                runner = Runner.__new__(Runner)
                runner.output = runner.repo = Path(tmp)
                runner.env = {}
                runner.receipt = dict(complete=False, commands=[])
                kwargs = {"side_effect": result} if isinstance(result, Exception) else {"return_value": result}
                with patch("run_hir_producer_ci.subprocess.run", **kwargs):
                    with self.assertRaises(RuntimeError):
                        runner.call("failure", ["fake"], timeout=1)
                receipt = json.loads((runner.output / "receipt.json").read_text())
                self.assertFalse(receipt["complete"])
                self.assertNotEqual(receipt["commands"][0]["status"], 0)
                self.assertEqual((runner.output / "failure.stdout").read_bytes(), b"out")
                self.assertEqual((runner.output / "failure.stderr").read_bytes(), b"err")

    def test_unlaunchable_command_retains_explicit_spawn_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner = Runner.__new__(Runner)
            runner.output = runner.repo = Path(tmp)
            runner.env = {}
            runner.receipt = dict(complete=False, commands=[])
            with patch("run_hir_producer_ci.subprocess.run", side_effect=FileNotFoundError("missing-tool")):
                with self.assertRaises(RuntimeError):
                    runner.call("missing", ["missing-tool"])
            receipt = json.loads((runner.output / "receipt.json").read_text())
            self.assertEqual(receipt["commands"][0]["status"], "spawn-failed")
            self.assertEqual((runner.output / "missing.stdout").read_bytes(), b"")
            self.assertIn(b"missing-tool", (runner.output / "missing.stderr").read_bytes())

    def test_profile_override_is_rejected_before_source_or_build_access(self):
        runner = Runner.__new__(Runner)
        runner.forbidden = ["RUSTFLAGS"]
        with patch.object(runner, "identity") as identity:
            with self.assertRaisesRegex(RuntimeError, "forbids overrides"):
                runner.run()
            identity.assert_not_called()

    def test_v1_summary_keeps_76_refusals_and_exact_build_identities(self):
        source = {'head': 'a' * 40}
        summary = dict(result='passed', negative_cli_probes=76, source_head=source['head'],
                       compiler_sha256='c' * 64, harness_sha256='h' * 64)
        validate_v1_summary(summary, source, 'c' * 64, 'h' * 64)
        for key, value in (('result', 'failed'), ('negative_cli_probes', 75),
                           ('negative_cli_probes', '76'), ('source_head', 'other'),
                           ('compiler_sha256', 'other'), ('harness_sha256', 'other')):
            with self.subTest(key=key), self.assertRaises(RuntimeError):
                validate_v1_summary(dict(summary, **{key: value}), source, 'c' * 64, 'h' * 64)
        with self.assertRaises(RuntimeError):
            validate_v1_summary([], source, 'c' * 64, 'h' * 64)

    def test_reviewed_v2_manifest_pin_rejects_coherent_source_and_manifest_edits(self):
        original = Path(__file__).resolve().parents[1] / 'fixtures/typed-frontend-v2/sources.json'
        validate_source_manifest(original)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            manifest = json.loads(original.read_text())
            source = root / manifest['sources']['source_buffer']['path']
            source.parent.mkdir(parents=True)
            source.write_bytes(b'changed source with a matching manifest hash')
            manifest['sources']['source_buffer']['sha256'] = digest(source)
            changed = root / 'sources.json'
            changed.write_text(json.dumps(manifest, indent=2) + '\n')
            self.assertEqual(changed.stat().st_size, original.stat().st_size)
            self.assertEqual(manifest['sources']['source_buffer']['sha256'], digest(source))
            with self.assertRaisesRegex(RuntimeError, 'reviewed frozen closure'):
                validate_source_manifest(changed)

    def test_manifest_pin_is_checked_before_toolchain_or_build_commands(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner = Runner.__new__(Runner)
            runner.repo = Path(tmp)
            (runner.repo / 'scripts').mkdir()
            (runner.repo / 'scripts/qualify_hir_producers.py').write_bytes(b'harness')
            manifest = runner.repo / 'fixtures/typed-frontend-v2/sources.json'
            manifest.parent.mkdir(parents=True)
            manifest.write_bytes(b'changed manifest')
            runner.forbidden, runner.receipt = [], {}
            with patch.object(runner, 'identity', return_value={'head': 'a' * 40, 'tree': 'b' * 40}), \
                    patch.object(runner, 'call') as call:
                with self.assertRaisesRegex(RuntimeError, 'reviewed frozen closure'):
                    runner.run()
                call.assert_not_called()

    def edge_fixture(self, root):
        (root / 'sources').mkdir()
        (root / 'sources/secondary_boundary.ox').write_bytes(b'synthetic source')
        (root / 'secondary-boundary').write_bytes(b'synthetic executable')
        (root / 'secondary.stdout').write_bytes(b'STF2' + bytes([1, 0, 3, 1, 2, 254, 255, 1, 0, 0, 0, 0]))
        for name in ('probe-error.stdout', 'probe-error.stderr', 'secondary.stderr'):
            (root / name).write_bytes(b'')
        summary = dict(status='passed', actual_parser_probe_io_error=dict(
            unread_bytes=0, syscall='read', fd=0, requested_bytes=1, status=74, parser_sha256='p' * 64),
            synthetic_secondary_span=dict(start=254, end=255,
                executable_sha256=digest(root / 'secondary-boundary'),
                source_sha256=digest(root / 'sources/secondary_boundary.ox'), argv=['/selected-compiler', 'compile']))
        (root / 'summary.json').write_text(json.dumps(summary))
        return summary

    def test_edge_controls_require_bound_parser_eof_probe_and_secondary_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            original = self.edge_fixture(root)
            validate_edge_summary(root, Path('/selected-compiler'), 'p' * 64)
            cases = [('status', None, 'failed'),
                     ('actual_parser_probe_io_error', 'parser_sha256', 'wrong'),
                     ('actual_parser_probe_io_error', 'unread_bytes', False),
                     ('actual_parser_probe_io_error', 'requested_bytes', 2),
                     ('actual_parser_probe_io_error', 'status', 0),
                     ('actual_parser_probe_io_error', 'syscall', 'unknown'),
                     ('synthetic_secondary_span', 'start', 253),
                     ('synthetic_secondary_span', 'end', 254),
                     ('synthetic_secondary_span', 'executable_sha256', 'wrong'),
                     ('synthetic_secondary_span', 'source_sha256', 'wrong'),
                     ('synthetic_secondary_span', 'argv', ['/other-compiler'])]
            for section, key, value in cases:
                with self.subTest(section=section, key=key):
                    changed = copy.deepcopy(original)
                    if key is None:
                        changed[section] = value
                    else:
                        changed[section][key] = value
                    (root / 'summary.json').write_text(json.dumps(changed))
                    with self.assertRaises(RuntimeError):
                        validate_edge_summary(root, Path('/selected-compiler'), 'p' * 64)
            (root / 'summary.json').write_text(json.dumps(original))
            (root / 'secondary.stdout').write_bytes(b'wrong-wire')
            with self.assertRaisesRegex(RuntimeError, 'output mismatch'):
                validate_edge_summary(root, Path('/selected-compiler'), 'p' * 64)

    def test_wrapper_runs_v1_v2_edge_and_diagnostic_recipe_on_each_selected_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runner = Runner.__new__(Runner)
            runner.repo = Path(__file__).resolve().parents[1]
            runner.output, runner.target = root, root / 'target'
            runner.target.mkdir()
            runner.args = SimpleNamespace(cargo='cargo', llvm_bin=Path('/llvm'), cc='cc')
            runner.env, runner.forbidden = {}, []
            runner.receipt = dict(complete=False, commands=[], profiles={})
            initial = dict(head='a' * 40, tree='b' * 40)
            calls = []

            def call(name, argv):
                calls.append((name, argv))
                if name == 'rust-version':
                    return b'release: 1.99.0\n'
                if name.endswith('-version'):
                    return b'19.1.7'
                if name.endswith('-build'):
                    profile = name.removesuffix('-build')
                    _, artifact = self.fixture(runner.target, profile)
                    return self.data([artifact])
                output = Path(argv[-1])
                output.mkdir()
                if '-v2-' not in name and '-diagnostic-' not in name:
                    selected = Path(argv[argv.index('--compiler') + 1])
                    summary = dict(result='passed', negative_cli_probes=76, source_head=initial['head'],
                                   compiler_sha256=digest(selected), harness_sha256=runner.receipt['harness_sha256'])
                else:
                    summary = {}
                (output / 'summary.json').write_text(json.dumps(summary))
                return b'passed'

            with patch.object(runner, 'identity', return_value=initial), patch.object(runner, 'call', side_effect=call), \
                    patch('run_hir_producer_ci.validate_v2_summary', return_value={'executable_sha256': {'parser': 'p' * 64}}) as v2, \
                    patch('run_hir_producer_ci.validate_edge_summary') as edge, \
                    patch('run_hir_producer_ci.validate_diagnostic_summary') as diagnostics:
                runner.run()
            self.assertTrue(runner.receipt['complete'])
            self.assertEqual(v2.call_count, 4)
            self.assertEqual(edge.call_count, 2)
            self.assertEqual(diagnostics.call_count, 2)
            for profile in ('debug', 'release'):
                names = [name for name, _ in calls if name.startswith(profile)]
                self.assertEqual(names, [profile + suffix for suffix in ('-build', '-qualification', '-v2-qualification', '-v2-edge-controls', '-diagnostic-qualification')])
                selected = str((runner.target / profile / 'oxid').resolve())
                for name, argv in calls:
                    if name.startswith(profile) and name != profile + '-build':
                        self.assertEqual(str(argv[argv.index('--compiler') + 1]), selected)
                entry = runner.receipt['profiles'][profile]
                self.assertTrue(entry['complete'])
                for key in ('summary_sha256', 'v2_summary_sha256', 'v2_edge_summary_sha256', 'diagnostic_summary_sha256'):
                    self.assertEqual(len(entry[key]), 64)
            for call in diagnostics.call_args_list:
                evidence, expected = call.args
                self.assertTrue(evidence.name.endswith('-diagnostic-qualification'))
                self.assertEqual(expected['source_head'], initial['head'])
                self.assertEqual(expected['source_tree'], initial['tree'])
                self.assertEqual(expected['roster_sha256'], runner.receipt['diagnostic_inputs']['roster_sha256'])

    def test_workflow_keeps_exact_head_profiles_and_failure_upload(self):
        workflow = (Path(__file__).resolve().parents[1] / ".github/workflows/ci.yml").read_text()
        job = workflow.split("  local-hir-producers:\n", 1)[1]
        self.assertIn("github.event.pull_request.head.sha || github.sha", job)
        self.assertIn("scripts/run_hir_producer_ci.py", job)
        self.assertIn("test_qualify_hir_producers_v2.py", job)
        self.assertIn("python3 -O -B -m unittest", job)
        self.assertIn("v1, v2 and edge controls", job)
        self.assertIn("if: always()", job)
        self.assertIn("cargo fetch --locked", job)
        self.assertNotIn("continue-on-error", job)
        self.assertNotIn("permissions:", job)

    def test_failed_diagnostic_admission_cannot_complete_profile_or_wrapper(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runner = Runner.__new__(Runner)
            runner.repo = Path(__file__).resolve().parents[1]
            runner.output, runner.target = root, root / 'target'
            runner.target.mkdir()
            runner.args = SimpleNamespace(cargo='cargo', llvm_bin=Path('/llvm'), cc='cc')
            runner.env, runner.forbidden = {}, []
            runner.receipt = dict(complete=False, commands=[], profiles={})
            initial = dict(head='a' * 40, tree='b' * 40)

            def call(name, argv):
                if name == 'rust-version':
                    return b'release: 1.99.0\n'
                if name.endswith('-version'):
                    return b'19.1.7'
                if name.endswith('-build'):
                    _, artifact = self.fixture(runner.target, name.removesuffix('-build'))
                    return self.data([artifact])
                output = Path(argv[-1])
                output.mkdir()
                (output / 'summary.json').write_text('{}')
                return b'fake controller evidence'

            with patch.object(runner, 'identity', return_value=initial), \
                    patch.object(runner, 'call', side_effect=call), \
                    patch('run_hir_producer_ci.validate_v1_summary'), \
                    patch('run_hir_producer_ci.validate_v2_summary', return_value={'executable_sha256': {'parser': 'p' * 64}}), \
                    patch('run_hir_producer_ci.validate_edge_summary'), \
                    patch('run_hir_producer_ci.validate_diagnostic_summary', side_effect=RuntimeError('incomplete diagnostics')):
                with self.assertRaisesRegex(RuntimeError, 'incomplete diagnostics'):
                    runner.run()
            self.assertFalse(runner.receipt['complete'])
            self.assertFalse(runner.receipt['profiles']['debug']['complete'])
            self.assertNotIn('release', runner.receipt['profiles'])
            self.assertNotIn('diagnostic_summary_sha256', runner.receipt['profiles']['debug'])


if __name__ == "__main__":
    unittest.main()
