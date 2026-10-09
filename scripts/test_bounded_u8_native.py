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
        self.assertEqual(len(roster), 115)
        self.assertTrue(set(gate.IGNORED) <= set(roster))
        self.assertEqual(sum(value[1] for value in gate.IGNORED.values()), 91)
        data = '\n'.join(name + ': test' for name in roster) + '\n\n115 tests, 0 benchmarks\n'
        gate.admit_listing(data.encode(), roster)
        for bad in (data.replace(roster[0], roster[1]), data.replace('115 tests', '114 tests'), data.replace(roster[0] + ': test\n', '')):
            with self.assertRaises(ValueError):
                gate.admit_listing(bad.encode(), roster)

    def test_zero_or_skipped_execution_is_not_a_pass(self):
        good = b'test result: ok. 111 passed; 0 failed; 0 ignored; 0 measured; 1234 filtered out; finished in 1.2s\n'
        gate.admit_execution(good, 111)
        for bad in (good.replace(b'111 passed', b'0 passed'), good.replace(b'0 ignored', b'4 ignored'),
                    good.replace(b'0 failed', b'1 failed'), good + good):
            with self.assertRaises(ValueError):
                gate.admit_execution(bad, 111)

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


if __name__ == '__main__':
    unittest.main()
