"""Keep predecessor controls intact; adapt only named historical byte IO seams."""
from pathlib import Path
import types
import unittest
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
p=HERE/'successor.py';v=types.ModuleType('wide_span_ci_view');v.__file__=str(p)
exec(compile(p.read_bytes(),str(p),'exec'),v.__dict__)

def text_tests():
    path=REPO/v.TEXT/'test_text_storage.py'
    raw=path.read_text()
    raw=v.exact(raw,"b=local('baseline')","b=_view.baseline(REPO)")
    raw=v.exact(raw,"activation=local('activate');first=activation.generate();self.assertEqual(first,activation.generate())",
        "activation=_view.activation(REPO);first=activation.generate();self.assertEqual(first,activation.generate())")
    raw=v.exact(raw,"self.assertEqual((REPO/name).read_bytes(),body,name)",
        "self.assertEqual(_view.predecessor_output(name,REPO),body,name)")
    raw=v.exact(raw,"current=b.local(HERE/'lifecycle.py','text_current_lifecycle')",
        "current=b.local(REPO/'tests/qualification/wide_span_fixture_storage_v1/lifecycle.py','text_current_lifecycle')")
    return v.load(path,raw,_view=v)

def suite(repo=REPO,loader=unittest.defaultTestLoader):
    v.need(Path(repo)==REPO,'explicit adapter repository')
    path=REPO/v.TEXT/'adapters.py';raw=path.read_text()
    raw=v.exact(raw,"baseline=local('baseline').Baseline(repo);b=local('baseline');r=baseline.module('retained')",
        "baseline=_view.baseline(repo).Baseline(repo);b=_view.baseline(repo);r=baseline.module('retained')")
    raw=v.exact(raw,"loader.loadTestsFromModule(local('test_text_storage'))",
        "loader.loadTestsFromModule(_text_tests())")
    raw=v.exact(raw,"current_path=b.ROOT+'predecessor.py'", "current_path='tests/qualification/wide_span_fixture_storage_v1/predecessor.py'")
    raw=v.exact(raw,"REPO/_text/'hosted_controls.py'", "REPO/'tests/qualification/wide_span_fixture_storage_v1/hosted_controls.py'")
    return v.load(path,raw,_view=v,_text_tests=text_tests).suite(repo,loader)
