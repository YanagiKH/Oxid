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
        self.assertEqual(list(privacy.ARRAY_COMPILE_TIME_FIXTURES), sorted(expected))
        self.assertEqual(sum('/contracts-v2/' in name for name in expected), 38)
        self.assertEqual(sum('/typing-contracts-v1/' in name for name in expected), 4)

    def test_exact_enum_compile_time_fixture_roster(self):
        self.assertEqual(privacy.ENUM_COMPILE_TIME_FIXTURES, (
            "tests/fixtures/bounded_enum_scanner/main.ox",
            "tests/fixtures/bounded_enum_scanner/scanner.ox",
        ))

    def test_exact_hir_compile_time_fixture_roster(self):
        stems = ('public', 'rich', 'scalar-arithmetic', 'scalar-assignment',
                 'scalar-boolean', 'scalar-comparison', 'scalar-loop', 'scalar-unit',
                 'synthetic-division', 'synthetic-overflow')
        expected = tuple(f'tests/fixtures/checked_hir_import/{stem}-{suffix}'
                         for stem in stems for suffix in ('source.txt', 'success.bin'))
        self.assertEqual(privacy.HIR_IMPORT_COMPILE_TIME_FIXTURES, expected)
        self.assertEqual(len(privacy.COMPILE_TIME_FIXTURES), 74)
        self.assertEqual(len(set(privacy.COMPILE_TIME_FIXTURES)), 74)

    def test_exact_v2_compile_time_fixture_roster(self):
        expected = (
            "tests/fixtures/checked_hir_import_v2/source-255.txt",
            "tests/fixtures/checked_hir_import_v2/success-255.bin",
        )
        self.assertEqual(privacy.HIR_IMPORT_V2_COMPILE_TIME_FIXTURES, expected)
        includer = ROOT / "src/frontend/oir/source/hir_import/emit_resource_tests.rs"
        references = re.findall(r'"(/tests/fixtures/checked_hir_import_v2/[^"\n]+)"', includer.read_text())
        self.assertEqual(tuple(name.lstrip('/') for name in references), expected)
        self.assertEqual(len((ROOT / expected[0]).read_bytes()), 255)
        self.assertEqual(len((ROOT / expected[1]).read_bytes()), 2607)

    def test_exact_producer_diagnostic_compile_time_fixture_roster(self):
        expected = tuple(f"tests/fixtures/producer_diagnostic/{stem}{suffix}"
                         for stem in ("duplicate", "end255", "multiple", "unknown-type")
                         for suffix in ("-source.txt", ".bin"))
        self.assertEqual(privacy.PRODUCER_DIAGNOSTIC_COMPILE_TIME_FIXTURES, expected)
        includer = ROOT / "src/frontend/oir/source/hir_import/producer_diagnostic/tests.rs"
        references = re.findall(r'"(/tests/fixtures/producer_diagnostic/[^"\n]+)"', includer.read_text())
        self.assertEqual(set(name.lstrip('/') for name in references), set(expected))
        self.assertEqual(len(references), 8)
        for name in expected:
            self.assertEqual((ROOT / name).stat().st_size if name.endswith('.bin') else len((ROOT / name).read_bytes()),
                             1575 if name.endswith('.bin') else {"duplicate":31, "end255":255, "multiple":48, "unknown-type":35}[Path(name).name.removesuffix('-source.txt')])

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
                dynamic = re.findall(
                    r'include(?:_str|_bytes)?!\(\s*concat!\(\s*env!\("CARGO_MANIFEST_DIR"\),\s*"(/tests/fixtures/checked_hir_import/(?:scalar-)?)",\s*\$name,\s*"(-source\.txt|-success\.bin)"\s*\)\s*\)',
                    text)
                self.assertEqual(len(literal) + len(rooted) + len(dynamic),
                                 len(re.findall(r'include(?:_str|_bytes)?!\(', text)), source)
                for name, base in [(name, source.parent) for name in literal] + [(name.lstrip('/'), checkout) for name in rooted]:
                    target = (base / name).resolve()
                    self.assertTrue(target.is_relative_to(checkout))
                    self.assertTrue(target.is_file(), target)
                # Only the two existing closed import macro shapes are allowed.
                # Check literal call sites against the explicit copied roster;
                # no directory expansion or compiler-derived fact generator.
                for prefix, suffix in dynamic:
                    macro = 'scalar_fixture' if prefix.endswith('scalar-') else '(?:capture|fixture)'
                    names = re.findall(macro + r'!\(\s*"([^"\n]+)"', text)
                    self.assertTrue(names, source)
                    for name in names:
                        relative = prefix.lstrip('/') + name + suffix
                        self.assertIn(relative, privacy.HIR_IMPORT_COMPILE_TIME_FIXTURES)
                        self.assertTrue((checkout / relative).is_file(), relative)
                count += len(literal) + len(rooted) + len(dynamic)
            # 61 predecessor includes, 60 original import includes/macro
            # definitions, two v2 endpoints and ten diagnostic-test include sites.
            # Real cfg(test)
            # compilation checks their expansion without expanding probe scope.
            self.assertEqual(count, 133)

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
