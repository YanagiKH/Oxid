"""Retained PR64 hosted mechanical tests; active bootstrap tested separately."""
import sys
from pathlib import Path
import unittest
sys.dont_write_bytecode = True
REPO = Path(__file__).absolute().parents[3]
sys.path.insert(0, str(REPO / 'tests/qualification/stdlib_trim_storage_v1'))
from retained import Fixture

def load_tests(loader, tests, pattern):
    import importlib.util
    suite = Fixture(REPO).hosted_suite()
    for name in ('test_storage', 'test_retained', 'test_active'):
        path = REPO / 'tests/qualification/stdlib_trim_storage_v1' / (name + '.py')
        spec = importlib.util.spec_from_file_location('trim_' + name, path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        suite.addTests(loader.loadTestsFromModule(module))
    return suite

if __name__ == '__main__':
    unittest.main()
