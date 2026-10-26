"""Keep CI entry points explicit without adding a YAML dependency.

These source-contract checks intentionally follow the workflows' block style;
actionlint remains the YAML and GitHub Actions syntax validator.
"""
from pathlib import Path
import re
import unittest


REPO = Path(__file__).resolve().parents[1]


def block(source, key, indent=0):
    """Return exactly one block at the requested indentation level."""
    prefix = ' ' * indent
    matches = re.findall(
        rf'^{prefix}{re.escape(key)}:(.*?)(?=^\S|^ {{1,{indent}}}\S|\Z)'
        if indent else rf'^{re.escape(key)}:(.*?)(?=^\S|\Z)',
        source, re.MULTILINE | re.DOTALL,
    )
    if len(matches) != 1:
        raise AssertionError(f'expected one {key} block, found {len(matches)}')
    return matches[0].strip()


class CiTriggerPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ci = (REPO / '.github/workflows/ci.yml').read_text(encoding='utf-8')
        cls.release = (REPO / '.github/workflows/release.yml').read_text(encoding='utf-8')
        cls.historical = (REPO / '.github/workflows/historical-qualification.yml').read_text(encoding='utf-8')

    def test_push_runs_main_and_all_tags_only(self):
        self.assertEqual(block(self.ci, 'push', 2),
                         "branches: [main]\n    tags: ['**']")

    def test_pull_requests_manual_reusable_and_weekly_entry_points(self):
        for event in ('pull_request', 'workflow_dispatch', 'workflow_call'):
            with self.subTest(event=event):
                self.assertEqual(block(self.ci, event, 2), '')
        self.assertEqual(block(self.ci, 'schedule', 2), '- cron: "17 3 * * 1"')
        events = re.findall(r'^(?:  )?([a-z_]+):', block(self.ci, 'on'), re.MULTILINE)
        self.assertEqual(events, ['push', 'pull_request', 'workflow_dispatch', 'workflow_call', 'schedule'])

    def test_no_workflow_or_job_concurrency_cancellation(self):
        for workflow in (self.ci, self.release, self.historical):
            self.assertNotRegex(workflow, r'(?m)^\s*(?:concurrency|cancel-in-progress):')

    def test_release_requires_readiness_then_full_reusable_ci_before_packaging(self):
        readiness = block(self.release, 'readiness', 2)
        self.assertIn('scripts/check_release_gates.py --release-tag "$RELEASE_TAG"', readiness)
        self.assertIn("if: github.ref_type == 'tag'", readiness)
        self.assertNotRegex(readiness, r'(?m)^    if:|continue-on-error:')
        verify = block(self.release, 'verify', 2)
        self.assertEqual(re.findall(r'^(?:    )?needs: (.+)$', verify, re.MULTILINE), ['readiness'])
        self.assertIn('uses: ./.github/workflows/ci.yml', verify)
        self.assertNotRegex(verify, r'(?m)^    if:|continue-on-error:')
        package = block(self.release, 'package', 2)
        self.assertEqual(re.findall(r'^    needs: (.+)$', package, re.MULTILINE), ['verify'])
        self.assertNotRegex(package, r'(?m)^    if:')

    def test_current_discovery_and_historical_replay_are_both_mandatory(self):
        current = block(self.ci, 'quality-and-repository', 2)
        self.assertIn('python3 -B scripts/run_current_script_tests.py', current)
        self.assertIn('scripts/verify_local_references_compatibility.py', current)
        self.assertIn('scripts/verify_floats.py', current)
        self.assertIn('scripts/verify_float_roots.py', current)
        self.assertIn('scripts/verify_characters.py', current)
        self.assertIn('scripts/verify_float_vector.py', current)
        self.assertIn('scripts/verify_matrices.py', current)
        self.assertIn('scripts/verify_i64.py', current)
        self.assertIn('scripts/verify_integer_widths.py', current)
        self.assertIn('scripts/verify_byte_integers.py', current)
        self.assertIn('scripts/verify_integer128.py', current)
        self.assertIn('scripts/verify_integer_bits.py', current)
        self.assertNotRegex(current, r'(?m)^    if:|continue-on-error:')

        replay = block(self.ci, 'historical-qualification', 2)
        self.assertIn('uses: ./.github/workflows/historical-qualification.yml', replay)
        self.assertNotRegex(replay, r'(?m)^    if:|continue-on-error:')
        historical = block(self.historical, 'quality-and-repository', 2)
        self.assertIn('ref: 9ff8ef90a26f003f4616838dfc186fa3bfb6845c', historical)
        self.assertIn('historical-script-controller/run_historical_script_tests.py', historical)
        self.assertIn('--archive "$RUNNER_TEMP/historical-script-controller/typed-project-unit2-evidence.zip"', historical)
        self.assertIn('name: historical-script-replay', historical)
        self.assertNotRegex(historical, r'(?m)^    if:|continue-on-error:')

        predecessor = block(self.ci, 'source-policy-predecessor', 2)
        self.assertIn('ref: 2e3fe21f76d0027d2b24cb5c7f02ba38ad354809', predecessor)
        self.assertIn('scripts/run_predecessor_script_tests.py', predecessor)
        self.assertIn('source-policy-replay.py" --repo .', predecessor)
        self.assertIn('name: byte-buffer-source-policy-predecessor', predecessor)
        self.assertNotRegex(predecessor, r'(?m)^    if:|continue-on-error:')

        hosted = block(self.ci, 'hosted-source-policy-predecessor', 2)
        self.assertIn('ref: 559a6e6aa0b2ab2022d8fa37546ac62aeac9954e', hosted)
        self.assertIn('scripts/run_hosted_predecessor_tests.py', hosted)
        self.assertIn('hosted-source-policy-replay.py" --repo .', hosted)
        self.assertIn('name: hosted-source-policy-predecessor', hosted)
        self.assertNotRegex(hosted, r'(?m)^    if:|continue-on-error:')

        transfer = block(self.ci, 'buffer-transfer-source-policy-predecessor', 2)
        self.assertIn('ref: c45d519127d314f522762c35879c0c485994ea42', transfer)
        self.assertIn('scripts/run_buffer_transfer_predecessor_tests.py', transfer)
        self.assertIn('buffer-transfer-source-policy-replay.py" --repo .', transfer)
        self.assertIn('name: buffer-transfer-source-policy-predecessor', transfer)
        self.assertNotRegex(transfer, r'(?m)^    if:|continue-on-error:')

        literals = block(self.ci, 'byte-string-source-policy-predecessor', 2)
        self.assertIn('ref: 22d04a015a444edcb600a6011cdb0e2c8529cb34', literals)
        self.assertIn('scripts/run_byte_string_predecessor_tests.py', literals)
        self.assertIn('byte-string-source-policy-replay.py" --repo .', literals)
        self.assertIn('name: byte-string-source-policy-predecessor', literals)
        self.assertNotRegex(literals, r'(?m)^    if:|continue-on-error:')

        arm64 = block(self.ci, 'arm64-host-source-policy-predecessor', 2)
        self.assertIn('ref: 18314e95d6afa3d8a4da7970dbddc828f4f6cbac', arm64)
        self.assertIn('fetch-depth: 0', arm64)
        self.assertIn('scripts/run_arm64_host_predecessor_tests.py', arm64)
        self.assertIn('arm64-host-source-policy-replay.py" --repo .', arm64)
        self.assertIn('name: arm64-host-source-policy-predecessor', arm64)
        self.assertNotRegex(arm64, r'(?m)^    if:|continue-on-error:')

    def test_historical_provenance_reads_are_authorized_without_event_spoofing(self):
        for workflow in (self.ci, self.release, self.historical):
            self.assertIn('actions: read', block(workflow, 'permissions'))
        self.assertNotRegex(self.historical, r'(?m)^\s*GITHUB_SHA:|GITHUB_SHA=|GITHUB_EVENT_PATH=')


if __name__ == '__main__':
    unittest.main()
