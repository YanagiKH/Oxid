"""Run the exact current adapter controls through ordinary Python CI discovery.

Each reviewed helper suite keeps its own module namespace, source pins and test
rosters. These are source-only controls and never compiler qualification passes.
"""
import os
from pathlib import Path
import subprocess
import sys
import unittest

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]


class CurrentLexerAdapterSuites(unittest.TestCase):
    def run_source_controls(self, relative):
        result = subprocess.run([sys.executable, '-B', str(REPO / relative), '-v'],
                                cwd=REPO, capture_output=True, text=True, timeout=180,
                                env={**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'})
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_original_preparation_and_exact_transformations(self):
        self.run_source_controls('tests/qualification/lexer_reservation_current/test_preparation.py')

    def test_current_unit1_runner_and_correspondence(self):
        self.run_source_controls('tests/qualification/lexer_reservation_current/test_unit1_current.py')

    def test_current_public_lifecycle_composition(self):
        self.run_source_controls('tests/qualification/unit4_public_v3/test_lexer_reservation.py')

    def test_current_parser_token_and_reserve_composition(self):
        self.run_source_controls('tests/qualification/unit4_parser_current/test_lexer_reservation.py')

    def test_current_parser_lexer_phase_source_controls(self):
        self.run_source_controls('tests/qualification/unit4_parser_current/test_lexer_phase.py')


if __name__ == '__main__':
    unittest.main()
