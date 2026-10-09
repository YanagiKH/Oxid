"""Keep whole-project examples explicit without hiding unrelated source checks."""
import unittest
import re
import shutil
import tempfile
from pathlib import Path
from unittest.mock import patch
import verify_repo

STREAMING_ROOT = "fixtures/typed-streaming-lexer/main.ox"
STREAMING_FILES = tuple("fixtures/typed-streaming-lexer/" + name + ".ox"
                        for name in ("main", "data", "frame", "keywords", "scanner"))

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
LEXER_MAIN_ENTRY = "fixtures/typed-lexer-samples/main.ox"
LEXER_ADMISSION_ENTRY = "fixtures/typed-lexer-samples/admission.ox"
LEXER_SHARED_MEMBERS = (
    "fixtures/typed-lexer-samples/tape.ox",
    "fixtures/typed-lexer-samples/transcript.ox",
)
LEXER_MEMBERS = (LEXER_MAIN_ENTRY,) + LEXER_SHARED_MEMBERS + (
    "fixtures/typed-lexer-samples/lexer.ox",
    "fixtures/typed-lexer-samples/keywords.ox",
)
LEXER_ADDED_FILES = LEXER_MEMBERS + (LEXER_ADMISSION_ENTRY,)
LEXER_CORE_ADDED_FILES = (
    "fixtures/typed-lexer-samples/lexer_core.ox",
    "fixtures/typed-lexer-samples/buffers.ox",
)
PARSER_ADMISSION_ENTRY = "fixtures/typed-lexer-samples/parser_admission.ox"
PARSER_ADMISSION_ADDED_FILES = (
    PARSER_ADMISSION_ENTRY,
    "fixtures/typed-lexer-samples/parser_banks.ox",
    "fixtures/typed-lexer-samples/parser_probe_output.ox",
)
PARSER_LEXER_SHARED_MEMBERS = (
    "fixtures/typed-lexer-samples/buffers.ox",
    "fixtures/typed-lexer-samples/lexer_core.ox",
    "fixtures/typed-lexer-samples/keywords.ox",
)
PARSER_MAIN_ENTRY = "fixtures/typed-lexer-samples/parser_main.ox"
PARSER_ADDED_FILES = (
    PARSER_MAIN_ENTRY,
    "fixtures/typed-lexer-samples/parser_state.ox",
    "fixtures/typed-lexer-samples/parser_signature.ox",
    "fixtures/typed-lexer-samples/parser_atom.ox",
    "fixtures/typed-lexer-samples/parser_call.ox",
    "fixtures/typed-lexer-samples/parser_expression.ox",
    "fixtures/typed-lexer-samples/parser_statement.ox",
    "fixtures/typed-lexer-samples/parser_control.ox",
    "fixtures/typed-lexer-samples/parser_driver.ox",
    "fixtures/typed-lexer-samples/parser_output.ox",
)
PARSER_MEMBERS = (PARSER_MAIN_ENTRY,) + PARSER_LEXER_SHARED_MEMBERS + PARSER_ADDED_FILES[1:]

STATIC_ROOTS = tuple("fixtures/typed-lexer-samples/" + name + ".ox" for name in (
    "static_main resolver_main typed_main ast_consumer_probe ast_static_main").split())
STATIC_ADDED_FILES = tuple("fixtures/typed-lexer-samples/" + name + ".ox" for name in (
    "ast_block ast_consumer_probe ast_expression ast_input ast_output ast_statement "
    "ast_static_main ast_validate resolver_driver resolver_main resolver_names resolver_output "
    "static_column static_common static_main static_output static_probe static_state "
    "typed_diagnostic typed_driver typed_expression typed_main typed_output typed_statement").split())


class ProjectRegistrationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for data in (Path('tests/fixtures/fixed_array_source_unit3'),
                     Path('tests/qualification/bounded_u8_current')):
            shutil.copytree(verify_repo.ROOT / data, self.root / data)
        self.data_sources = list(verify_repo.fixture_data_sources(self.root))
        self.assertEqual(len(self.data_sources), 188)
        u8_root = self.root / 'tests/qualification/bounded_u8_current/oracle'
        u8_expected = {u8_root / ('pairs-' + mode + '-' + format(index, '02d') + '.ox')
                       for mode in ('owned', 'scalar') for index in range(32)}
        u8_expected |= {u8_root / 'roundtrip-owned.ox', u8_root / 'roundtrip-scalar.ox'}
        self.assertEqual({path for path in self.data_sources if u8_root in path.parents}, u8_expected)
        self.assertEqual(len(set(self.data_sources) - u8_expected), 122)
        self.members = [self.root / p for p in verify_repo.TYPED_SOURCE_FILES]
        self.members += [self.root / p for ps in verify_repo.TYPED_PROJECTS.values() for p in ps]
        self.members += [self.root / p for p in verify_repo.TYPED_CHECK_ONLY_FILES]
        self.members = sorted(set(self.members))

    def assert_static_addition(self, checks, entries, count):
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_PROJECTS[0], STREAMING_ROOT)
        self.assertEqual(verify_repo.TYPED_PROJECTS[STREAMING_ROOT], STREAMING_FILES)
        streaming = {self.root / name for name in STREAMING_FILES}
        self.assertEqual([row for row in checks if row[0] in streaming], [(self.root / STREAMING_ROOT, True)])
        self.assertFalse(any(entry in streaming for entry in entries))
        checks = [row for row in checks if row[0] not in streaming]
        count -= len(STREAMING_FILES)
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_PROJECTS[-5:], STATIC_ROOTS)
        added = {self.root / name for name in STATIC_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in added],
                         [(self.root / root, True) for root in STATIC_ROOTS])
        self.assertFalse(any(entry in added for entry in entries))
        return [row for row in checks if row[0] not in added], entries, count - len(STATIC_ADDED_FILES)

    def assert_parser_addition(self, checks, entries, count):
        checks, entries, count = self.assert_static_addition(checks, entries, count)
        self.assertEqual(verify_repo.TYPED_CHECK_ONLY_PROJECTS[1:-5],
                         (ARTIFACT_MAIN_ENTRY, ARTIFACT_LOAD_ENTRY, LEXER_MAIN_ENTRY,
                          LEXER_ADMISSION_ENTRY, PARSER_ADMISSION_ENTRY, PARSER_MAIN_ENTRY))
        self.assertEqual(verify_repo.TYPED_PROJECTS[PARSER_MAIN_ENTRY], PARSER_MEMBERS)
        added = {self.root / name for name in PARSER_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in added],
                         [(self.root / PARSER_MAIN_ENTRY, True)])
        self.assertFalse(any(entry in added for entry in entries))
        # Only the root and nine parser modules are new. The three shared
        # lexer modules already belong to the synthetic carrier inventory.
        return [row for row in checks if row[0] not in added], entries, count - len(PARSER_ADDED_FILES)

    def assert_synthetic_carrier_addition(self, checks, entries, count):
        checks, entries, count = self.assert_parser_addition(checks, entries, count)
        added_names = LEXER_CORE_ADDED_FILES + PARSER_ADMISSION_ADDED_FILES
        added = {self.root / name for name in added_names}
        self.assertEqual([row for row in checks if row[0] in added],
                         [(self.root / PARSER_ADMISSION_ENTRY, True)])
        self.assertFalse(any(entry in added for entry in entries))
        self.assertEqual(verify_repo.TYPED_PROJECTS[PARSER_ADMISSION_ENTRY],
                         (PARSER_ADMISSION_ENTRY,) + PARSER_LEXER_SHARED_MEMBERS + PARSER_ADMISSION_ADDED_FILES[1:])
        self.assertEqual(verify_repo.TYPED_PROJECTS[LEXER_MAIN_ENTRY], LEXER_MEMBERS + LEXER_CORE_ADDED_FILES)
        self.assertEqual(verify_repo.TYPED_PROJECTS[LEXER_ADMISSION_ENTRY],
                         (LEXER_ADMISSION_ENTRY,) + LEXER_SHARED_MEMBERS + (LEXER_CORE_ADDED_FILES[1],))
        # The extraction adds two modules; the synthetic carrier adds three.
        # Neither changes the original lexer roots or their ordinary run roster.
        return [row for row in checks if row[0] not in added], entries, count - len(added_names)

    def assert_lexer_addition(self, checks, entries, count):
        checks, entries, count = self.assert_synthetic_carrier_addition(checks, entries, count)
        lexer_added = {self.root / name for name in LEXER_ADDED_FILES}
        self.assertEqual([row for row in checks if row[0] in lexer_added],
                         [(self.root / LEXER_MAIN_ENTRY, True), (self.root / LEXER_ADMISSION_ENTRY, True)])
        self.assertFalse(any(entry in lexer_added for entry in entries))
        return [row for row in checks if row[0] not in lexer_added], entries, count - len(LEXER_ADDED_FILES)

    def assert_artifact_addition(self, checks, entries, count):
        checks, entries, count = self.assert_lexer_addition(checks, entries, count)
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
        self.assertEqual({pair: members for pair, members in verify_repo.TYPED_PROJECT_SHARED_MEMBERS.items()
                          if not pair.intersection(STATIC_ROOTS)}, {
            frozenset((LEXER_MAIN_ENTRY, LEXER_ADMISSION_ENTRY)):
                LEXER_SHARED_MEMBERS + (LEXER_CORE_ADDED_FILES[1],),
            frozenset((LEXER_MAIN_ENTRY, PARSER_ADMISSION_ENTRY)): PARSER_LEXER_SHARED_MEMBERS,
            frozenset((LEXER_ADMISSION_ENTRY, PARSER_ADMISSION_ENTRY)): (LEXER_CORE_ADDED_FILES[1],),
            frozenset((LEXER_MAIN_ENTRY, PARSER_MAIN_ENTRY)): PARSER_LEXER_SHARED_MEMBERS,
            frozenset((LEXER_ADMISSION_ENTRY, PARSER_MAIN_ENTRY)): (LEXER_CORE_ADDED_FILES[1],),
            frozenset((PARSER_ADMISSION_ENTRY, PARSER_MAIN_ENTRY)): PARSER_LEXER_SHARED_MEMBERS,
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

    def test_unregistered_lexer_member_stays_legacy(self):
        extra = self.root / 'fixtures/typed-lexer-samples/unregistered.ox'
        checks, entries, count = verify_repo.source_plan(sorted(self.members + self.data_sources + [extra]), self.root)
        self.assertIn((extra, False), checks)
        self.assertNotIn(extra, entries)
        self.assert_scanner_addition(checks, entries, count)

    def test_lexer_projects_cannot_share_an_unregistered_module(self):
        inventory = dict(verify_repo.TYPED_PROJECTS)
        inventory[LEXER_ADMISSION_ENTRY] += (LEXER_MEMBERS[-1],)
        with patch.object(verify_repo, 'TYPED_PROJECTS', inventory):
            with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_synthetic_carrier_cannot_import_the_tape_or_legacy_wrapper(self):
        for extra in (LEXER_SHARED_MEMBERS[0], 'fixtures/typed-lexer-samples/lexer.ox'):
            inventory = dict(verify_repo.TYPED_PROJECTS)
            inventory[PARSER_ADMISSION_ENTRY] += (extra,)
            with self.subTest(extra=extra), patch.object(verify_repo, 'TYPED_PROJECTS', inventory):
                with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_parser_inventory_matches_declared_module_closure(self):
        # Derive the actual closure from declarations rather than the registry.
        # The fixture uses sibling modules, including several on one line.
        pending = [verify_repo.ROOT / PARSER_MAIN_ENTRY]
        declared = set()
        while pending:
            source = pending.pop()
            relative = source.relative_to(verify_repo.ROOT).as_posix()
            if relative in declared:
                continue
            declared.add(relative)
            modules = re.findall(r'\bmod\s+([A-Za-z_]\w*)\s*;', source.read_text(encoding='utf-8'))
            pending.extend(source.with_name(module + '.ox') for module in modules)
        self.assertEqual(declared, set(PARSER_MEMBERS))
        self.assertEqual(set(verify_repo.TYPED_PROJECTS[PARSER_MAIN_ENTRY]), declared)

    def test_static_roots_match_exact_flat_closures_and_added_inventory(self):
        historical = {member for root in (LEXER_MAIN_ENTRY, LEXER_ADMISSION_ENTRY,
                                         PARSER_ADMISSION_ENTRY, PARSER_MAIN_ENTRY)
                      for member in verify_repo.TYPED_PROJECTS[root]}
        static_members = set()
        for entry, size in zip(STATIC_ROOTS, (18, 20, 23, 21, 21), strict=True):
            with self.subTest(entry=entry):
                root = verify_repo.ROOT / entry
                children = re.findall(r'\bmod\s+([A-Za-z_]\w*)\s*;', root.read_text())
                declared = (entry,) + tuple(root.with_name(name + '.ox').relative_to(verify_repo.ROOT).as_posix()
                                             for name in children)
                self.assertEqual(len(declared), size)
                self.assertEqual(len(set(declared)), size)
                self.assertEqual(verify_repo.TYPED_PROJECTS[entry], declared)
                for child in declared[1:]:
                    self.assertNotRegex((verify_repo.ROOT / child).read_text(), r'\bmod\s+')
                static_members.update(declared)
        self.assertEqual(static_members - historical, set(STATIC_ADDED_FILES))
        self.assertEqual(len(STATIC_ADDED_FILES), 24)
        admitted = set(verify_repo.TYPED_PROJECTS[PARSER_MAIN_ENTRY]) | set(verify_repo.TYPED_PROJECTS[STATIC_ROOTS[-1]])
        self.assertEqual(len(admitted), 30)

    def test_static_shared_pairs_are_exact_and_required(self):
        expected = {}
        for entry in STATIC_ROOTS:
            for other in verify_repo.TYPED_PROJECTS:
                pair = frozenset((entry, other))
                shared = set(verify_repo.TYPED_PROJECTS[entry]) & set(verify_repo.TYPED_PROJECTS[other])
                if len(pair) == 2 and shared:
                    expected[pair] = shared
        registered = {pair: set(members) for pair, members in verify_repo.TYPED_PROJECT_SHARED_MEMBERS.items()
                      if pair.intersection(STATIC_ROOTS)}
        self.assertEqual(len(expected), 30)
        self.assertEqual(registered, expected)
        for pair in expected:
            for changed in (None, ('fixtures/typed-lexer-samples/unregistered.ox',)):
                inventory = dict(verify_repo.TYPED_PROJECT_SHARED_MEMBERS)
                if changed is None:
                    del inventory[pair]
                else:
                    inventory[pair] = changed
                with self.subTest(pair=pair, changed=changed), \
                        patch.object(verify_repo, 'TYPED_PROJECT_SHARED_MEMBERS', inventory):
                    with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                        verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_static_consumer_cannot_admit_a_parser_sibling(self):
        inventory = dict(verify_repo.TYPED_PROJECTS)
        inventory[STATIC_ROOTS[-1]] += ('fixtures/typed-lexer-samples/parser_output.ox',)
        with patch.object(verify_repo, 'TYPED_PROJECTS', inventory):
            with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_parser_cannot_import_synthetic_carrier_or_legacy_modules(self):
        for extra in LEXER_SHARED_MEMBERS + ('fixtures/typed-lexer-samples/lexer.ox',) + PARSER_ADMISSION_ADDED_FILES:
            inventory = dict(verify_repo.TYPED_PROJECTS)
            inventory[PARSER_MAIN_ENTRY] += (extra,)
            with self.subTest(extra=extra), patch.object(verify_repo, 'TYPED_PROJECTS', inventory):
                with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                    verify_repo.source_plan(self.members + self.data_sources, self.root)

    def test_parser_requires_each_exact_shared_pair(self):
        for other in (LEXER_MAIN_ENTRY, LEXER_ADMISSION_ENTRY, PARSER_ADMISSION_ENTRY):
            pair = frozenset((PARSER_MAIN_ENTRY, other))
            for shared in (None, ('fixtures/typed-lexer-samples/parser_state.ox',)):
                inventory = dict(verify_repo.TYPED_PROJECT_SHARED_MEMBERS)
                if shared is None:
                    del inventory[pair]
                else:
                    inventory[pair] = shared
                with self.subTest(other=other, shared=shared), \
                        patch.object(verify_repo, 'TYPED_PROJECT_SHARED_MEMBERS', inventory):
                    with self.assertRaisesRegex(RuntimeError, 'overlapping typed source inventories'):
                        verify_repo.source_plan(self.members + self.data_sources, self.root)

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
