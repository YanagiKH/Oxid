NAME = 'tests/qualification/lexer_reservation_current/test_current_source.py'
COUNT = 23
DISPATCHER = {'path': 'tests/fixtures/typed_project_source_binding/run.py', 'bytes': 6774, 'sha256': 'a2fbd5c06b82ad5d3ffc74d15a49ecf58efe2a24a6e98cacf328e5ecd45fc184'}
"""Retained controls run only after successor admission, at their genuine paths."""
import hashlib
import importlib.util
from pathlib import Path
import sys
import types
import unittest
sys.dont_write_bytecode = True
ROOT = Path(__file__).absolute().parents[3]


class AcceptedControls(unittest.TestSuite):
    def countTestCases(self):
        return COUNT

    def run(self, result, debug=False):
        # Close any preceding discovery module before entering the retained view.
        self._tearDownPreviousClass(None, result)
        self._handleModuleTearDown(result)
        result._previousTestClass = None
        entered = getattr(result, '_testRunEntered', False)
        result._testRunEntered = True
        try:
            path = ROOT / DISPATCHER['path']
            raw = path.read_bytes()
            if path.is_symlink() or len(raw) != DISPATCHER['bytes'] or hashlib.sha256(raw).hexdigest() != DISPATCHER['sha256']:
                raise RuntimeError('changed independently pinned successor dispatcher')
            source = types.ModuleType('accepted_controls_dispatcher')
            source.__file__ = str(path)
            exec(compile(raw, str(path), 'exec'), source.__dict__)
            captured = source.preflight(ROOT)
            view = source._modules['predecessor']
            with view.view(ROOT, captured['package_json_predecessor']['inputs'],
                           captured['package_json_seal']['predecessor_view']) as root:
                directory = (root / NAME).parent
                aliases = {p.stem for p in directory.glob('*.py')}
                saved = {n: sys.modules[n] for n in aliases if n in sys.modules}
                search = list(sys.path)
                try:
                    for n in aliases:
                        sys.modules.pop(n, None)
                    sys.path.insert(0, str(directory))
                    module_name = 'accepted_' + NAME.replace('/', '_').replace('.', '_')
                    module = view.load(root, NAME, module_name)
                    sys.modules[module_name] = module
                    suite = unittest.defaultTestLoader.loadTestsFromModule(module)
                    if suite.countTestCases() != COUNT:
                        raise RuntimeError('retained test roster changed')
                    try:
                        suite.run(result, debug)
                    finally:
                        suite._tearDownPreviousClass(None, result)
                        suite._handleModuleTearDown(result)
                        result._previousTestClass = None
                finally:
                    sys.path[:] = search
                    for n in aliases:
                        sys.modules.pop(n, None)
                    sys.modules.update(saved)
        except Exception:
            case = unittest.FunctionTestCase(lambda: None, description=NAME + ' accepted-view admission/closure')
            result.startTest(case)
            result.addError(case, sys.exc_info())
            result.stopTest(case)
        finally:
            result._testRunEntered = entered
        return result


def load_tests(loader, tests, pattern):
    return AcceptedControls()


if __name__ == '__main__':
    unittest.main()
