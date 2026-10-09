"""Fast fail-closed controls for the RFC0030 hosted gate and integer oracle."""
import copy
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import run_bounded_u8_corpus as corpus
import verify_bounded_u8_native as gate


class CorpusAdmissionTests(unittest.TestCase):
    def test_exact_frozen_roster_and_independent_complete_coverage(self):
        corpus.admit_oracle()
        for route in ('scalar', 'owned'):
            bitmap = bytearray(corpus.KEY_COUNT)
            for batch in range(32):
                source = (corpus.ORACLE / f'pairs-{route}-{batch:02}.ox').read_text()
                adapted, seals = corpus.partition_helpers(source, route, batch)
                self.assertIn('if a<0{return a;}if b<0{return b;}return a+b;', adapted)
                for seal in seals:
                    body = corpus.partition_body(seal['left'], seal['partition'], *seal['right_range'])
                    self.assertEqual(source.count(body), 1)
                    self.assertEqual(adapted.count(body), 1)
                    self.assertEqual(corpus.sha(body.encode()), seal['unchanged_original_body_sha256'])
                corpus.mark_coverage(bitmap, seals)
            self.assertEqual(sum(bitmap), 393216)
            self.assertTrue(all(bitmap))
            self.assertEqual(corpus.sha(bitmap), '005f2f6fb9bcaf75851803e4a9a9c8f664b706d8b824dadbc1d312c7558e8bcc')
        self.assertEqual(corpus.expected_stream(), '56bb1aecf742b802ce2024075d6049ea3d6fa8d73f06bdb8cc94f3725d69e350')

    def test_mutated_or_missing_inputs_do_not_admit(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'oracle'
            shutil.copytree(corpus.ORACLE, root)
            path = root / 'pairs-scalar-00.ox'
            original = path.read_bytes()
            path.write_bytes(original.replace(b'!= false', b'!= true', 1))
            with self.assertRaisesRegex(ValueError, 'source changed'):
                corpus.admit_oracle(root)
            path.write_bytes(original)
            extra = root / 'extra.ox'
            extra.write_bytes(b'')
            with self.assertRaisesRegex(ValueError, 'membership'):
                corpus.admit_oracle(root)
            extra.unlink()
            (root / 'roundtrip-owned.ox').unlink()
            with self.assertRaisesRegex(ValueError, 'membership'):
                corpus.admit_oracle(root)

    def test_rehashed_manifest_and_altered_partition_expectation_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'coverage.json').write_text('{}\n')
            with self.assertRaisesRegex(ValueError, 'coverage authority'):
                corpus.admit_oracle(root)
        original = (corpus.ORACLE / 'pairs-scalar-00.ox').read_text()
        for wrong in (original.replace('!= false', '!= true', 1), original.replace('checked + 6', 'checked + 7', 1),
                      original.replace('j0_1 < 1', 'j0_1 < 2', 1)):
            with self.assertRaisesRegex(ValueError, 'independent integer'):
                corpus.partition_helpers(wrong, 'scalar', 0)

    def test_duplicate_operator_missing_operator_and_out_of_range_keys_reject(self):
        _, seals = corpus.partition_helpers((corpus.ORACLE / 'pairs-owned-00.ox').read_text(), 'owned', 0)
        bitmap = bytearray(corpus.KEY_COUNT)
        corpus.mark_coverage(bitmap, seals)
        with self.assertRaisesRegex(ValueError, 'duplicate'):
            corpus.mark_coverage(bitmap, seals)
        for key, value in (('operators', ['==']), ('left', 256), ('right_range', [0, 257])):
            altered = copy.deepcopy(seals)
            altered[0][key] = value
            with self.assertRaises(ValueError):
                corpus.mark_coverage(bytearray(corpus.KEY_COUNT), altered)
        self.assertFalse(all(bitmap))  # A partial batch cannot stand for full coverage.

    def test_failed_timed_out_and_unlaunchable_commands_keep_original_evidence(self):
        failures = [subprocess.CompletedProcess(['tool'], 7, b'original out', b'original err'),
                    subprocess.TimeoutExpired(['tool'], 1, output=b'partial out', stderr=b'partial err'),
                    FileNotFoundError('missing tool')]
        for result in failures:
            with self.subTest(result=result), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                options = {'side_effect': result} if isinstance(result, Exception) else {'return_value': result}
                with patch.object(corpus.subprocess, 'run', **options), self.assertRaises(ValueError):
                    corpus.invoke(root, 'failed', ['tool'], cwd=root, env={}, timeout=1)
                row = json.loads((root / 'failed.json').read_text())
                self.assertNotEqual(row['status'], 0)
                self.assertEqual(row['stdout_sha256'], corpus.sha((root / 'failed.stdout').read_bytes()))
                self.assertEqual(row['stderr_sha256'], corpus.sha((root / 'failed.stderr').read_bytes()))
                self.assertIsNotNone(row['error']) if isinstance(result, Exception) else self.assertIsNone(row['error'])


class NativeGateTests(unittest.TestCase):
    def test_exact_roster_includes_all_four_ignored_tests(self):
        repo = Path(__file__).resolve().parents[1]
        roster = json.loads((repo / gate.PACKAGE / 'tests.json').read_text())
        self.assertEqual(len(roster), 117)
        initial_path = repo / gate.PACKAGE / 'tests-u8-initial.json'
        self.assertEqual(gate.identity(initial_path)['sha256'], gate.INITIAL_ROSTER_SHA)
        initial = json.loads(initial_path.read_text())
        self.assertEqual(len(initial), 115)
        gate.admit_byte_storage_predecessors(repo, initial)
        self.assertEqual(roster, gate.current_roster(initial))
        self.assertEqual(set(roster) - set(initial) - set(gate.CROSS_HOST_TESTS),
                         set(gate.BYTE_STORAGE_RENAMES.values()))
        self.assertEqual(set(initial) - set(roster), set(gate.BYTE_STORAGE_RENAMES))
        self.assertTrue(set(gate.IGNORED) <= set(roster))
        self.assertEqual(sum(value[1] for value in gate.IGNORED.values()), 91)
        data = '\n'.join(name + ': test' for name in roster) + '\n\n117 tests, 0 benchmarks\n'
        gate.admit_listing(data.encode(), roster)
        for bad in (data.replace(roster[0], roster[1]), data.replace('117 tests', '116 tests'), data.replace(roster[0] + ': test\n', '')):
            with self.assertRaises(ValueError):
                gate.admit_listing(bad.encode(), roster)

    def test_zero_or_skipped_execution_is_not_a_pass(self):
        good = b'test result: ok. 113 passed; 0 failed; 0 ignored; 0 measured; 1234 filtered out; finished in 1.2s\n'
        gate.admit_execution(good, 113)
        for bad in (good.replace(b'113 passed', b'0 passed'), good.replace(b'0 ignored', b'4 ignored'),
                    good.replace(b'0 failed', b'1 failed'), good + good):
            with self.assertRaises(ValueError):
                gate.admit_execution(bad, 113)

    def test_empty_or_duplicate_selection_cannot_admit_its_own_summary(self):
        for names, data in (([], b'0 tests, 0 benchmarks\n'),
                            (['same', 'same'], b'same: test\nsame: test\n2 tests, 0 benchmarks\n')):
            with self.subTest(names=names), self.assertRaises(ValueError):
                gate.admit_listing(data, names)
        with self.assertRaises(ValueError):
            gate.admit_execution(
                b'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 117 filtered out; finished in 0.00s\n', 0)

    def test_native_stream_retention_is_mandatory(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for suffix in ('.elf', '.ll', '.stdout', '.stderr', '.status'):
                (root / ('case' + suffix)).write_bytes(b'\x7fELF' if suffix == '.elf' else b'')
            self.assertEqual(len(gate.native_artifacts(root, 'owned', 1)), 5)
            with self.assertRaises(ValueError):
                gate.native_artifacts(root, 'owned', 2)
            (root / 'case.status').unlink()
            with self.assertRaisesRegex(ValueError, 'missing owned'):
                gate.native_artifacts(root, 'owned', 1)

    def test_provider_summary_requires_complete_exact_compiler_and_source(self):
        parities, refusals, captures = gate.provider.expected_results()
        summary = dict(schema='rfc0030-frozen-provider-successor-1', status='passed', compiler_sha256='a',
            source_head='b', source_tree='c', recipe=gate.provider.recipe(), legacy_parities=parities,
            changed_domain_refusals=refusals,
            captures=[dict(version=v, name=n, source_sha256=s, kind=k) for v, n, s, k in captures],
            frozen_protocols=['OPA1/AST1/STF1', 'OPA2/AST2/STF2'], caps=[128, 255],
            frame_bytes=[1559, 2607, 1575], invalid_source_llvm_invocations=0)
        gate.validate_provider_summary(summary, {'sha256': 'a'}, 'b', 'c')
        for key, value in (('compiler_sha256', 'wrong'), ('source_head', 'wrong'), ('legacy_parities', parities[:-1]),
                           ('captures', []), ('changed_domain_refusals', refusals + refusals[:1]), ('caps', [256, 256]),
                           ('invalid_source_llvm_invocations', 1)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                gate.validate_provider_summary(dict(summary, **{key: value}), {'sha256': 'a'}, 'b', 'c')

    def test_workflow_prescribes_gate_with_evidence_and_unchanged_real_stager(self):
        repo = Path(__file__).resolve().parents[1]
        workflow = (repo / '.github/workflows/ci.yml').read_text()
        invocation = workflow.split('python3 -B scripts/verify_bounded_u8_native.py', 1)[1].split('\n      - name:', 1)[0]
        self.assertIn('--build-evidence "$RUNNER_TEMP/bounded-enum-native"', invocation)
        self.assertIn('--expected-head "$UNIT2E_EXPECTED_HEAD"', invocation)
        self.assertIn('PYTHONDONTWRITEBYTECODE: "1"', workflow)
        self.assertIn('name: bounded-u8-native-evidence', workflow)
        self.assertEqual(corpus.sha((repo / gate.STAGER).read_bytes()),
                         '055dd46f2c24f3a0496486ec89570ef533d7a016a5bc85d834447e8a59204def')


class RosterSelectionTests(unittest.TestCase):
    def setUp(self):
        self.repo = Path(__file__).resolve().parents[1]
        self.roster = json.loads((self.repo / gate.PACKAGE / 'tests.json').read_text())
        self.normal = [name for name in self.roster if name not in gate.IGNORED]
        self.collision = ('frontend::oir::source::association::u8_tests::'
                          'byte_storage_inherited_auth_walk_counters_refuse_overflow_and_mismatch')
        # Include both the actual RFC0031 collision and names containing a full
        # admitted name, so a multi-filter substring selector also fails.
        self.available = self.roster + [self.collision, self.normal[0] + '_later',
                                        'later::' + next(iter(gate.IGNORED))]
        self.ignored = set(gate.IGNORED)

    @staticmethod
    def listing(names):
        return ('\n'.join(name + ': test' for name in names)
                + f'\n\n{len(names)} tests, 0 benchmarks\n').encode()

    def invoke(self, directory, label, argv, **kwargs):
        filters = [arg for arg in argv[1:] if not arg.startswith('--')]
        selected = [name for name in self.available if
                    (name in filters if '--exact' in argv else any(part in name for part in filters))]
        if '--ignored' in argv:
            selected = [name for name in selected if name in self.ignored]
        if '--list' in argv:
            return self.listing(selected), b''
        ignored = len(set(selected) & self.ignored)
        return (f'test result: ok. {len(selected) - ignored} passed; 0 failed; {ignored} ignored; '
                f'0 measured; {len(self.available) - len(selected)} filtered out; finished in 0.01s\n').encode(), b''

    def run_selection(self):
        gate.run_roster_tests(Path('evidence'), Path('unit'), self.roster, self.repo, {})

    def test_discovery_and_execution_use_exact_frozen_names_despite_collisions(self):
        with patch.object(corpus, 'invoke', side_effect=self.invoke) as invoke:
            self.run_selection()
        self.assertEqual(invoke.call_count, 3)
        for call, names, flags in zip(invoke.call_args_list,
                (self.roster, self.roster, self.normal),
                (['--list', '--color=never'], ['--list', '--ignored', '--color=never'],
                 ['--nocapture', '--test-threads=1', '--color=never'])):
            self.assertEqual(call.args[2], [Path('unit'), '--exact', *names, *flags])
        self.assertEqual(len(self.normal), 113)

    def test_invalid_selection_is_rejected_before_invoking_the_binary(self):
        for roster in ([], self.roster + self.roster[:1], self.normal, list(gate.IGNORED)):
            with self.subTest(roster=roster), patch.object(corpus, 'invoke') as invoke:
                with self.assertRaises(ValueError):
                    gate.run_roster_tests(Path('evidence'), Path('unit'), roster, self.repo, {})
                invoke.assert_not_called()

    def test_missing_duplicate_unexpected_or_zero_discovery_fails_closed(self):
        for label, names in (
                ('missing', self.roster[1:]), ('duplicate', [self.roster[1], *self.roster[1:]]),
                ('unexpected', [self.collision, *self.roster[1:]]), ('zero', [])):
            with self.subTest(label=label), patch.object(corpus, 'invoke',
                    return_value=(self.listing(names), b'')) as invoke:
                with self.assertRaises(ValueError):
                    self.run_selection()
                self.assertEqual(invoke.call_count, 1)

    def test_missing_extra_or_zero_ignored_discovery_fails_before_execution(self):
        for ignored in (set(list(gate.IGNORED)[1:]), set(gate.IGNORED) | {self.normal[0]}, set()):
            self.ignored = ignored
            with self.subTest(ignored=ignored), patch.object(corpus, 'invoke', side_effect=self.invoke) as invoke:
                with self.assertRaises(ValueError):
                    self.run_selection()
                self.assertEqual(invoke.call_count, 2)

    def test_discovery_stderr_fails_before_execution(self):
        for failure in ('all-discovery', 'ignored-discovery'):
            def invoke(directory, label, argv, **kwargs):
                stdout, stderr = self.invoke(directory, label, argv, **kwargs)
                return stdout, b'unexpected' if label == failure else stderr
            with self.subTest(failure=failure), patch.object(corpus, 'invoke', side_effect=invoke) as mocked:
                with self.assertRaisesRegex(ValueError, 'discovery stderr'):
                    self.run_selection()
                self.assertEqual(mocked.call_count, 1 if failure == 'all-discovery' else 2)

    def test_zero_or_ignored_normal_execution_fails_closed(self):
        for passed, ignored in ((0, 0), (112, 1)):
            def invoke(directory, label, argv, **kwargs):
                if label == 'resources-and-semantics':
                    return (f'test result: ok. {passed} passed; 0 failed; {ignored} ignored; '
                            '0 measured; 5 filtered out; finished in 0.01s\n').encode(), b''
                return self.invoke(directory, label, argv, **kwargs)
            with self.subTest(passed=passed, ignored=ignored), patch.object(corpus, 'invoke', side_effect=invoke):
                with self.assertRaises(ValueError):
                    self.run_selection()


if __name__ == '__main__':
    unittest.main()
