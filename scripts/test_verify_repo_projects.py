"""Keep whole-project examples explicit without hiding unrelated source checks."""
import unittest
import shutil
import tempfile
from pathlib import Path
from unittest.mock import patch
import verify_repo

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

    def test_children_use_real_entry_and_unrelated_sources_stay_legacy(self):
        extra = self.root / 'fixtures/typed-project-batch/unregistered.ox'
        checks, entries, count = verify_repo.source_plan(sorted(self.members + self.data_sources + [extra]), self.root)
        self.assertEqual(count, 4)
        self.assertEqual(len(entries), 2)
        self.assertIn((extra, False), checks)
        self.assertIn((self.root / 'fixtures/typed-project-batch/main.ox', True), checks)
        self.assertNotIn(self.root / 'fixtures/typed-project-batch/jobs.ox', [p for p, _ in checks])
        self.assertEqual(len(checks), 3)

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
