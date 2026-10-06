"""Keep whole-project examples explicit without hiding unrelated source checks."""
import unittest
import shutil
import tempfile
from pathlib import Path
from unittest.mock import patch
import verify_repo

SCANNER_MEMBERS = (
    'tests/fixtures/bounded_enum_scanner/main.ox',
    'tests/fixtures/bounded_enum_scanner/scanner.ox',
)
EXPRESSION_MEMBERS = (
    'fixtures/typed-expression-samples/main.ox',
    'fixtures/typed-expression-samples/arena.ox',
    'fixtures/typed-expression-samples/scanner.ox',
    'fixtures/typed-expression-samples/parser.ox',
    'fixtures/typed-expression-samples/evaluator.ox',
)

class ProjectRegistrationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        data = Path('tests/fixtures/fixed_array_source_unit3')
        shutil.copytree(verify_repo.ROOT / data, self.root / data)
        self.data_sources = list(verify_repo.fixture_data_sources(self.root))
        self.members = [self.root / p for p in verify_repo.TYPED_SOURCE_FILES]
        self.members += [self.root / p for ps in verify_repo.TYPED_PROJECTS.values() for p in ps]

    def assert_expression_addition(self, checks, entries, count):
        self.assertEqual(verify_repo.TYPED_PROJECTS[EXPRESSION_MEMBERS[0]], EXPRESSION_MEMBERS)
        expression_members = {self.root / name for name in EXPRESSION_MEMBERS}
        expression_entry = self.root / EXPRESSION_MEMBERS[0]
        self.assertEqual([row for row in checks if row[0] in expression_members], [(expression_entry, True)])
        self.assertEqual([entry for entry in entries if entry in expression_members], [expression_entry])
        # Subtract only the five named members and their one typed root check/run.
        predecessor_checks = [row for row in checks if row[0] not in expression_members]
        predecessor_entries = [entry for entry in entries if entry not in expression_members]
        predecessor_count = count - len(EXPRESSION_MEMBERS)
        self.assertEqual((len(predecessor_checks), len(predecessor_entries), predecessor_count), (7, 6, 15))
        return predecessor_checks, predecessor_entries, predecessor_count

    def assert_scanner_addition(self, checks, entries, count):
        checks, entries, count = self.assert_expression_addition(checks, entries, count)
        self.assertEqual(verify_repo.TYPED_PROJECTS[SCANNER_MEMBERS[0]], SCANNER_MEMBERS)
        scanner_members = {self.root / name for name in SCANNER_MEMBERS}
        scanner_entry = self.root / SCANNER_MEMBERS[0]
        self.assertEqual([row for row in checks if row[0] in scanner_members], [(scanner_entry, True)])
        self.assertEqual([entry for entry in entries if entry in scanner_members], [scanner_entry])
        # Preserve the pre-scanner totals, excluding only its two named members
        # and single typed entry check/run. Each caller adds one legacy neighbor.
        predecessor_checks = [row for row in checks if row[0] not in scanner_members]
        predecessor_entries = [entry for entry in entries if entry not in scanner_members]
        self.assertEqual((len(predecessor_checks), len(predecessor_entries), count - len(SCANNER_MEMBERS)),
                         (6, 5, 13))

    def test_children_use_real_entry_and_unrelated_sources_stay_legacy(self):
        extra = self.root / 'fixtures/typed-project-batch/unregistered.ox'
        checks, entries, count = verify_repo.source_plan(sorted(self.members + self.data_sources + [extra]), self.root)
        self.assert_scanner_addition(checks, entries, count)
        self.assertIn((extra, False), checks)
        self.assertIn((self.root / 'fixtures/typed-project-batch/main.ox', True), checks)
        self.assertNotIn(self.root / 'fixtures/typed-project-batch/jobs.ox', [p for p, _ in checks])
        self.assertIn((self.root / 'fixtures/typed-array-samples/main.ox', True), checks)
        for name in ('stats.ox', 'samples.ox'):
            self.assertNotIn(self.root / 'fixtures/typed-array-samples' / name, [p for p, _ in checks])
        self.assertIn((self.root / 'fixtures/typed-slice-samples/main.ox', True), checks)
        for name in ('buffers.ox', 'stats.ox'):
            self.assertNotIn(self.root / 'fixtures/typed-slice-samples' / name, [p for p, _ in checks])

    def test_unregistered_slice_sample_member_stays_legacy(self):
        extra = self.root / 'fixtures/typed-slice-samples/unregistered.ox'
        checks, entries, count = verify_repo.source_plan(sorted(self.members + self.data_sources + [extra]), self.root)
        self.assertIn((extra, False), checks)
        self.assertNotIn(extra, entries)
        self.assert_scanner_addition(checks, entries, count)

    def test_composition_members_are_exact_and_unregistered_sources_stay_legacy(self):
        extra = self.root / 'fixtures/typed-record-composition-samples/unregistered.ox'
        checks, entries, count = verify_repo.source_plan(sorted(self.members + self.data_sources + [extra]), self.root)
        self.assertIn((extra, False), checks)
        self.assertNotIn(extra, entries)
        self.assert_scanner_addition(checks, entries, count)
        self.assertIn((self.root / 'fixtures/typed-record-composition-samples/main.ox', True), checks)
        for name in ('model.ox', 'ops.ox'):
            self.assertNotIn(self.root / 'fixtures/typed-record-composition-samples' / name, [p for p, _ in checks])

    def test_unregistered_scanner_member_stays_legacy(self):
        extra = self.root / 'tests/fixtures/bounded_enum_scanner/unregistered.ox'
        checks, entries, count = verify_repo.source_plan(sorted(self.members + self.data_sources + [extra]), self.root)
        self.assertIn((extra, False), checks)
        self.assertNotIn(extra, entries)
        self.assert_scanner_addition(checks, entries, count)

    def test_unregistered_expression_member_stays_legacy(self):
        extra = self.root / 'fixtures/typed-expression-samples/unregistered.ox'
        checks, entries, count = verify_repo.source_plan(sorted(self.members + self.data_sources + [extra]), self.root)
        self.assertIn((extra, False), checks)
        self.assertNotIn(extra, entries)
        self.assert_scanner_addition(checks, entries, count)

    def test_missing_member_fails_before_commands(self):
        for member in self.members:
            with self.subTest(member=member):
                with self.assertRaisesRegex(RuntimeError, 'missing'):
                    verify_repo.source_plan([p for p in self.members if p != member] + self.data_sources, self.root)

    def test_invalid_or_overlapping_inventory_is_rejected(self):
        bad = [
            {'main.ox': ('child.ox',)},
            {'main.ox': ('main.ox', 'main.ox')},
            {'main.ox': ('main.ox', verify_repo.TYPED_SOURCE_FILES[0])},
        ]
        for inventory in bad:
            with self.subTest(inventory=inventory), patch.object(verify_repo, 'TYPED_PROJECTS', inventory):
                with self.assertRaises(RuntimeError):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

if __name__ == '__main__':
    unittest.main()
