"""Retained PR64/PR65 controls and current text controls."""
import types
from pathlib import Path
import unittest
REPO=Path(__file__).absolute().parents[3]

def load_tests(loader, tests, pattern):
    path=REPO/'tests/qualification/stdlib_feature_storage_activation_v3/adapters.py'
    module=types.ModuleType('text_test_adapters');module.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module.suite(REPO,loader)

if __name__=='__main__':unittest.main()
