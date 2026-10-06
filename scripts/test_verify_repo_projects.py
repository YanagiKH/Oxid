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
STDIN_ENTRY = 'fixtures/typed-expression-samples/stdin.ox'
STACK_MAIN_ENTRY = 'fixtures/typed-expression-samples/stack_main.ox'
STACK_STDIN_ENTRY = 'fixtures/typed-expression-samples/stack_stdin.ox'
ARTIFACT_MAIN_ENTRY = "fixtures/typed-expression-samples/artifact_main.ox"
ARTIFACT_LOAD_ENTRY = "fixtures/typed-expression-samples/artifact_load.ox"
ARTIFACT_ADDED_FILES = (
    ARTIFACT_MAIN_ENTRY,
    "fixtures/typed-expression-samples/artifact_writer.ox",
    ARTIFACT_LOAD_ENTRY,
    "fixtures/typed-expression-samples/artifact_reader.ox",
)
STACK_ADDED_FILES = (
    STACK_MAIN_ENTRY,
    'fixtures/typed-expression-samples/stack_code.ox',
    'fixtures/typed-expression-samples/lowering.ox',
    STACK_STDIN_ENTRY,
)
STACK_MEMBERS = STACK_ADDED_FILES[:-1] + EXPRESSION_MEMBERS[1:]

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
        self.members += [self.root / p for p in verify_repo.TYPED_CHECK_ONLY_FILES]
        self.members = sorted(set(self.members))

    def assert_artifact_addition(self, checks, entries, count):
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_PROJECTS,
                         (ARTIFACT_MAIN_ENTRY, ARTIFACT_LOAD_ENTRY))
        added = {self.root / name for name in ARTIFACT_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in added],
                         [(self.root / ARTIFACT_MAIN_ENTRY, True), (self.root / ARTIFACT_LOAD_ENTRY, True)])
        self.assertFalse(any(entry in added for entry in entries))
        self.assertEqual(verify_repo.TYPED_PROJECTS[ARTIFACT_MAIN_ENTRY],
                         ARTIFACT_ADDED_FILES[:2] + EXPRESSION_MEMBERS[1:] + STACK_ADDED_FILES[1:3])
        self.assertEqual(verify_repo.TYPED_PROJECTS[ARTIFACT_LOAD_ENTRY], ARTIFACT_ADDED_FILES[2:])
        return [row for row in checks if row[0] not in added], entries, count - len(ARTIFACT_ADDED_FILES)

    def assert_stack_addition(self, checks, entries, count):
        checks, entries, count = self.assert_artifact_addition(checks, entries, count)
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_FILES, (STDIN_ENTRY, STACK_STDIN_ENTRY))
        self.assertEqual(verify_repo.TYPED_PROJECTS[STACK_MAIN_ENTRY], STACK_MEMBERS)
        self.assertEqual(verify_repo.TYPED_PROJECT_SHARED_MEMBERS, {
            frozenset((EXPRESSION_MEMBERS[0], STACK_MAIN_ENTRY)): EXPRESSION_MEMBERS[1:],
            frozenset((EXPRESSION_MEMBERS[0], ARTIFACT_MAIN_ENTRY)): EXPRESSION_MEMBERS[1:],
            frozenset((STACK_MAIN_ENTRY, ARTIFACT_MAIN_ENTRY)): EXPRESSION_MEMBERS[1:] + STACK_ADDED_FILES[1:3],
        })
        added = {self.root / name for name in STACK_ADDED_FILES}
        stack_entry = self.root / STACK_MAIN_ENTRY
        stack_stdin = self.root / STACK_STDIN_ENTRY
        self.assertEqual([row for row in checks if row[0] in added],
                         [(stack_entry, True), (stack_stdin, True)])
        self.assertEqual([entry for entry in entries if entry in added], [stack_entry])
        self.assertFalse(any(STACK_STDIN_ENTRY in members for members in verify_repo.TYPED_PROJECTS.values()))
        for shared in EXPRESSION_MEMBERS[1:]:
            self.assertNotIn(self.root / shared, [path for path, _ in checks])
            self.assertNotIn(self.root / shared, entries)
        # Remove only the four named additions, including two checks and one
        # run. Shared expression members contribute no second inventory count.
        predecessor_checks = [row for row in checks if row[0] not in added]
        predecessor_entries = [entry for entry in entries if entry not in added]
        predecessor_count = count - len(STACK_ADDED_FILES)
        self.assertEqual((len(predecessor_checks), len(predecessor_entries), predecessor_count), (9, 7, 21))
        return predecessor_checks, predecessor_entries, predecessor_count

    def assert_stdin_addition(self, checks, entries, count):
        checks, entries, count = self.assert_stack_addition(checks, entries, count)
        stdin_entry = self.root / STDIN_ENTRY
        self.assertEqual([row for row in checks if row[0] == stdin_entry], [(stdin_entry, True)])
        self.assertNotIn(stdin_entry, entries)
        self.assertFalse(any(STDIN_ENTRY in members for members in verify_repo.TYPED_PROJECTS.values()))
        # Remove only the new check-only root; the old runnable roster is intact.
        predecessor_checks = [row for row in checks if row[0] != stdin_entry]
        self.assertEqual((len(predecessor_checks), len(entries), count - 1), (8, 7, 20))
        return predecessor_checks, entries, count - 1

    def assert_expression_addition(self, checks, entries, count):
        checks, entries, count = self.assert_stdin_addition(checks, entries, count)
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
            {'main.ox': ('main.ox', STDIN_ENTRY)},
        ]
        for inventory in bad:
            with self.subTest(inventory=inventory), patch.object(verify_repo, 'TYPED_PROJECTS', inventory), \
                    patch.object(verify_repo, 'TYPED_PROJECT_SHARED_MEMBERS', {}), \
                    patch.object(verify_repo, 'TYPED_CHECK_ONLY_PROJECTS', ()):
                with self.assertRaises(RuntimeError):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_explicit_sharing_is_independent_of_project_order(self):
        sources = self.members + self.data_sources
        expected = verify_repo.source_plan(sources, self.root)
        with patch.object(verify_repo, 'TYPED_PROJECTS', dict(reversed(list(verify_repo.TYPED_PROJECTS.items())))):
            checks, entries, count = verify_repo.source_plan(sources, self.root)
        self.assertEqual(set(checks), set(expected[0]))
        self.assertEqual(set(entries), set(expected[1]))
        self.assertEqual(count, expected[2])
        self.assertEqual(count, len(self.members))

    def test_only_the_exact_named_project_pair_may_share_modules(self):
        extra = 'fixtures/typed-expression-samples/unregistered_main.ox'
        inventory = dict(verify_repo.TYPED_PROJECTS)
        inventory[extra] = (extra,) + EXPRESSION_MEMBERS[1:]
        with patch.object(verify_repo, 'TYPED_PROJECTS', inventory):
            with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                verify_repo.source_plan(self.members + self.data_sources + [self.root / extra], self.root)

    def test_wider_or_incomplete_shared_members_are_rejected(self):
        for members in (STACK_MEMBERS + (EXPRESSION_MEMBERS[0],),
                        STACK_MEMBERS + ('fixtures/typed-record-composition-samples/model.ox',),
                        STACK_MEMBERS[:-1]):
            inventory = dict(verify_repo.TYPED_PROJECTS)
            inventory[STACK_MAIN_ENTRY] = members
            with self.subTest(members=members), patch.object(verify_repo, 'TYPED_PROJECTS', inventory):
                with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_shared_inventory_cannot_name_roots_or_missing_projects(self):
        inventories = (
            {frozenset((EXPRESSION_MEMBERS[0], STACK_MAIN_ENTRY)): (STACK_MAIN_ENTRY,)},
            {frozenset((EXPRESSION_MEMBERS[0], 'missing.ox')): EXPRESSION_MEMBERS[1:]},
        )
        for inventory in inventories:
            with self.subTest(inventory=inventory), patch.object(verify_repo, 'TYPED_PROJECT_SHARED_MEMBERS', inventory):
                with self.assertRaisesRegex(RuntimeError, 'invalid explicit typed project sharing inventory'):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_invalid_check_only_project_inventory_is_rejected(self):
        for inventory in ((ARTIFACT_MAIN_ENTRY, ARTIFACT_MAIN_ENTRY), ('missing.ox',)):
            with self.subTest(inventory=inventory), patch.object(verify_repo, 'TYPED_CHECK_ONLY_PROJECTS', inventory):
                with self.assertRaisesRegex(RuntimeError, 'invalid explicit check-only typed project inventory'):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_overlapping_check_only_inventory_is_rejected(self):
        inventories = (
            (STDIN_ENTRY, STDIN_ENTRY),
            (STDIN_ENTRY, verify_repo.TYPED_SOURCE_FILES[0]),
            (STDIN_ENTRY, EXPRESSION_MEMBERS[1]),
            (STDIN_ENTRY, STACK_MAIN_ENTRY),
            (STDIN_ENTRY, STACK_MEMBERS[1]),
        )
        for inventory in inventories:
            with self.subTest(inventory=inventory), patch.object(verify_repo, 'TYPED_CHECK_ONLY_FILES', inventory):
                with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

if __name__ == '__main__':
    unittest.main()
