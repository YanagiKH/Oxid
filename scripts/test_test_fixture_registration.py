"""Keep ordinary CLI test classification bound to the frozen data inventory."""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import verify_repo


class TestFixtureRegistrationTests(unittest.TestCase):
    def test_published_registration_matches_validated_source_data(self):
        verify_repo.verify_test_fixture_registration(verify_repo.ROOT)

    def test_missing_extra_and_non_boolean_entries_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            expected = {root / "tests/fixtures/data.ox"}
            documents = (
                "",
                "[test-fixtures]\n",
                '[test-fixtures]\n"tests/fixtures/data.ox" = false\n',
                '[test-fixtures]\n"tests/fixtures/data.ox" = "true"\n',
                '[test-fixtures]\n"tests/fixtures/data.ox" = 1\n',
                '[test-fixtures]\n"tests/fixtures/data.ox" = true\n'
                '"tests/fixtures/neighbor.ox" = true\n',
                '[test-fixtures]\n"tests/fixtures/neighbor.ox" = true\n',
            )
            for document in documents:
                with self.subTest(document=document):
                    (root / "oxid.toml").write_text(document, encoding="utf-8")
                    with patch.object(verify_repo, "fixture_data_sources", return_value=expected):
                        with self.assertRaisesRegex(RuntimeError, "exactly match"):
                            verify_repo.verify_test_fixture_registration(root)

    def test_paths_are_relative_to_the_verifier_root(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "oxid.toml").write_text(
                '[test-fixtures]\n"tests/fixtures/data.ox" = true\n', encoding="utf-8")
            with patch.object(verify_repo, "fixture_data_sources",
                              return_value={root / "tests/fixtures/data.ox"}):
                verify_repo.verify_test_fixture_registration(root)

    def test_unreadable_or_duplicate_registration_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(verify_repo, "fixture_data_sources", return_value=set()):
                with self.assertRaisesRegex(RuntimeError, "unreadable test fixture"):
                    verify_repo.verify_test_fixture_registration(root)
                (root / "oxid.toml").write_text(
                    '[test-fixtures]\n"tests/data.ox" = true\n"tests/data.ox" = true\n',
                    encoding="utf-8")
                with self.assertRaisesRegex(RuntimeError, "unreadable test fixture"):
                    verify_repo.verify_test_fixture_registration(root)


if __name__ == "__main__":
    unittest.main()
