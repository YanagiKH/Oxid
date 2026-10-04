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
        for workflow in (self.ci, self.release):
            self.assertNotRegex(workflow, r'(?m)^\s*(?:concurrency|cancel-in-progress):')

    def test_release_packages_depend_on_full_reusable_ci(self):
        self.assertEqual(block(self.release, 'verify', 2),
                         'uses: ./.github/workflows/ci.yml')
        package = block(self.release, 'package', 2)
        self.assertEqual(re.findall(r'^    needs: (.+)$', package, re.MULTILINE), ['verify'])
        self.assertNotRegex(package, r'(?m)^    if:')

    def test_regression_is_in_existing_script_discovery(self):
        self.assertIn("python3 -B -m unittest discover -s scripts -p 'test_*.py' -v", self.ci)


if __name__ == '__main__':
    unittest.main()
