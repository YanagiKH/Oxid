"""Exact PR65 test seams: retained algebra versus actual current admission."""
import ast
from pathlib import Path
import sys
import types
import unittest

HERE=Path(__file__).absolute().parent

def local(name):
    path=HERE/(name+'.py');m=types.ModuleType('text_'+name);m.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),m.__dict__)
    return m


def suite(repo,loader=unittest.defaultTestLoader):
    baseline=local('baseline').Baseline(repo);b=local('baseline');r=baseline.module('retained')
    suite=r.Fixture(repo).hosted_suite()
    for name in ('test_storage','test_retained','test_active'):
        path=b.TRIM+name+'.py';text=baseline.read(path).decode()
        # The original files remain byte-identical. Remove their global path
        # mutation and route helper imports privately through the fixed loader.
        if name=='test_storage':
            text=b.exact(text,"sys.path.insert(0, str(REPO / 'tests/qualification/package_json_current_v1'))\n",'')
            text=b.exact(text,"d = load('trim_storage_derivation', HERE / 'derive.py')","d = _baseline.module('derive')")
            text=b.exact(text,"p = load('trim_storage_predecessor', HERE / 'predecessor.py')",
                "p = _baseline.execute('old_predecessor', _trim+'predecessor.py', _baseline.read(_trim+'predecessor.py'))")
            # Only this full-view integration method needs the active text-aware
            # helper. All old derivation/row/archive negative controls use p.
            tree=ast.parse(text);cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='StorageControls')
            node=next(n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name=='test_complete_source_inverse_and_all_view_bodies')
            segment=ast.get_source_segment(text,node)
            text=b.exact(text,segment,segment.replace('p.','_current.'))
        else:text=b.exact(text,'sys.path.insert(0,str(HERE))\n','')
        if name=='test_active':
            text=b.exact(text,'for name,body in first.items():self.assertEqual((REPO/name).read_bytes(),body,name)',
                'for name,body in first.items():self.assertEqual(_baseline.read(name),body,name)')
            text=b.exact(text,"hosted=load('trim_active_hosted',HERE/'hosted_controls.py')",
                "hosted=load('text_active_hosted',REPO/_text/'hosted_controls.py')")
        current_path=b.ROOT+'predecessor.py'
        current=types.ModuleType('text_runtime_predecessor');current.__file__=str(Path(repo)/current_path)
        current.__dict__['g']=baseline.g
        current_raw=b.exact(baseline.g.regular(repo,current_path).decode(),'import generation as g\n','')
        exec(compile(current_raw,current.__file__,'exec'),current.__dict__)
        for before,after in (('import activate\n',"activate = _baseline.module('activate')\n"),('import retained as r\n',"r = _baseline.module('retained')\n"),('import generation as g\n','g = _baseline.g\n')):
            if before in text:text=b.exact(text,before,after)
        module=types.ModuleType('text_pr65_'+name);module.__file__=str(Path(repo)/path)
        module.__dict__.update(_baseline=baseline,_trim=b.TRIM,_text=b.ROOT,_current=current)
        exec(compile(text,module.__file__,'exec'),module.__dict__)
        suite.addTests(loader.loadTestsFromModule(module))
    suite.addTests(loader.loadTestsFromModule(local('test_text_storage')))
    return RestoringSuite(suite)


class RestoringSuite(unittest.TestSuite):
    def run(self,result,debug=False):
        before=list(sys.path)
        try:return super().run(result,debug)
        finally:sys.path[:]=before
