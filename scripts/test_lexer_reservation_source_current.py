"""Expose retained lexer controls through the authenticated formatter predecessor."""
import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1] / 'tests/qualification/lexer_reservation_current'
sys.path.insert(0, str(ROOT))


def load_tests(loader, tests, pattern):
    spec = importlib.util.spec_from_file_location('lexer_reservation_current_source_controls', ROOT / 'test_current_source.py')
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return loader.loadTestsFromModule(module)


if __name__ == '__main__':
    unittest.main()
