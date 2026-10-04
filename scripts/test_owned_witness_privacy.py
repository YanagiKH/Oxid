"""Keep privacy-probe checkouts complete without expanding their probe scope."""
from pathlib import Path
import re
import tempfile
import unittest

import verify_owned_witness_privacy as privacy


ROOT = Path(__file__).resolve().parent.parent
ARRAY_TESTS = Path('src/frontend/oir/owned/source/array_types_tests.rs')


class PrivacyCheckoutTests(unittest.TestCase):
    def test_exact_array_compile_time_fixture_roster(self):
        references = re.findall(
            r'include_str!\(concat!\(env!\("CARGO_MANIFEST_DIR"\), "(/tests/fixtures/[^"\n]+)"\)\)',
            (ROOT / ARRAY_TESTS).read_text())
        expected = {name.removeprefix('/') for name in references}
        self.assertEqual(len(references), 47)
        self.assertEqual(len(expected), 42)
        self.assertEqual(list(privacy.COMPILE_TIME_FIXTURES), sorted(expected))
        self.assertEqual(sum('/contracts-v2/' in name for name in expected), 38)
        self.assertEqual(sum('/typing-contracts-v1/' in name for name in expected), 4)

    def test_materialized_checkout_has_exact_fixture_bytes_and_all_includes(self):
        with tempfile.TemporaryDirectory() as directory:
            checkout = Path(directory) / 'checkout'
            privacy.materialize_checkout(ROOT, checkout)
            actual = {path.relative_to(checkout).as_posix()
                      for path in (checkout / 'tests').rglob('*') if path.is_file()}
            self.assertEqual(actual, set(privacy.COMPILE_TIME_FIXTURES))
            for name in actual:
                self.assertEqual((checkout / name).read_bytes(), (ROOT / name).read_bytes())
            empty = checkout / 'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox'
            self.assertEqual(empty.read_bytes(), b'')

            count = 0
            for source in (checkout / 'src').rglob('*.rs'):
                text = source.read_text()
                literal = re.findall(r'include(?:_str|_bytes)?!\(\s*"([^"\n]+)"\s*\)', text)
                rooted = re.findall(
                    r'include(?:_str|_bytes)?!\(\s*concat!\(\s*env!\("CARGO_MANIFEST_DIR"\),\s*"([^"\n]+)"\s*\)\s*\)',
                    text)
                self.assertEqual(len(literal) + len(rooted),
                                 len(re.findall(r'include(?:_str|_bytes)?!\(', text)), source)
                for name, base in [(name, source.parent) for name in literal] + [(name.lstrip('/'), checkout) for name in rooted]:
                    target = (base / name).resolve()
                    self.assertTrue(target.is_relative_to(checkout))
                    self.assertTrue(target.is_file(), target)
                count += len(literal) + len(rooted)
            self.assertEqual(count, 56)

    def test_missing_required_fixture_fails_materialization(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'source'
            root.mkdir()
            for name in ('Cargo.toml', 'Cargo.lock', 'build.rs'):
                (root / name).write_bytes(b'')
            for name in ('src', 'native', 'compiler', 'stdlib', 'rfcs', 'fixtures'):
                (root / name).mkdir()
            with self.assertRaises(FileNotFoundError):
                privacy.materialize_checkout(root, Path(directory) / 'checkout')


if __name__ == '__main__':
    unittest.main()
