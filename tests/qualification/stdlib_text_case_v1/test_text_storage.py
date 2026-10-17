"""Text relocation, retained/current generation and import-isolation controls."""
import ast
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]

def local(name):
    path=HERE/(name+'.py');m=types.ModuleType('text_'+name);m.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),m.__dict__)
    return m

b=local('baseline')

class TextStorageControls(unittest.TestCase):
    def test_exact_derivation_and_unchanged_methods(self):
        baseline=b.Baseline(REPO);d=local('derive');original=baseline.read(d.PREVIOUS)
        first=d.derive(original);self.assertEqual(first,d.derive(original))
        self.assertEqual(first,(HERE/'predecessor.py').read_bytes())
        def methods(raw):
            text=raw.decode();return {n.name:ast.get_source_segment(text,n) for n in ast.parse(text).body if isinstance(n,ast.FunctionDef)}
        old,new=methods(original),methods(first)
        self.assertEqual({n for n in old if old[n]!=new[n]},{'capture'})
        for wrong in (original+b'\n',original.replace(b'_strings_archive',b'_wrong_archive')):
            with self.assertRaises(ValueError):d.derive(wrong)

    def test_text_archive_refusals_and_no_live_fallback(self):
        baseline=b.Baseline(REPO);p=baseline.execute('text_predecessor',b.ROOT+'predecessor.py',(HERE/'predecessor.py').read_bytes())
        body=baseline.g.regular(REPO,p._TEXT_ARCHIVE)
        for mutation in ('missing','wrong','oversize','symlink','hardlink','executable','directory','parent-symlink'):
            with self.subTest(mutation=mutation),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);target=root/p._TEXT_ARCHIVE;target.parent.mkdir(parents=True)
                live=root/p._TEXT;live.parent.mkdir();live.write_bytes(body)
                real=root/'real';real.write_bytes(body)
                if mutation=='wrong':target.write_bytes(b'X'+body[1:])
                elif mutation=='oversize':target.write_bytes(body+b'\n')
                elif mutation=='symlink':target.symlink_to(real)
                elif mutation=='hardlink':os.link(real,target)
                elif mutation=='executable':target.write_bytes(body);target.chmod(0o755)
                elif mutation=='directory':target.mkdir()
                elif mutation=='parent-symlink':
                    target.parent.rmdir();target.parent.symlink_to(live.parent,target_is_directory=True)
                with self.assertRaises((ValueError,OSError)):p._text_archive(root,copy.deepcopy(p._TEXT_ROW))
        row=p._TEXT_ROW
        for key,value in (('path','other.ox'),('read_from',p._TEXT_ARCHIVE),('bytes',float(row['bytes'])),('bytes',True),('mode','100755'),('git_blob','0'*40),('sha256','0'*64),('extra',0)):
            with self.subTest(key=key),self.assertRaises(ValueError):p._text_archive(REPO,{**row,key:value})
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);target=root/p._TEXT_ARCHIVE;target.parent.mkdir(parents=True)
            raw=body+b'\n';target.write_bytes(raw)
            with self.assertRaises(ValueError):p._text_archive(root,{**row,**baseline.g.identity(p._TEXT,raw),'git_blob':b.blob('blob',raw)})
        descriptor=json.loads(baseline.g.regular(REPO,b.Q+'source-seal-v1.json'))['predecessor_view']
        with self.assertRaisesRegex(ValueError,'text cannot be restored source'):p.capture(REPO,{p._TEXT:body},descriptor)

    def test_retained_outputs_and_private_aliases(self):
        poisons={name:types.ModuleType(name) for name in ('activate','derive','retained','generation','common')}
        before=list(sys.path)
        with patch.dict(sys.modules,poisons):
            baseline=b.Baseline(REPO);first=baseline.outputs();self.assertEqual(first,baseline.outputs())
            self.assertEqual(len(first),27)
            for name,module in poisons.items():self.assertIs(sys.modules[name],module)
        self.assertEqual(sys.path,before)
        with self.assertRaisesRegex(ValueError,'unlisted PR65 read'):baseline.read('stdlib/text.ox')
        with self.assertRaisesRegex(ValueError,'unlisted PR65 helper'):baseline.module('common')
        self.assertNotEqual(first['tests/fixtures/typed_project_source_binding/run.py'],(REPO/'tests/fixtures/typed_project_source_binding/run.py').read_bytes())

    def test_current_outputs_and_inventory_are_real(self):
        activation=local('activate');first=activation.generate();self.assertEqual(first,activation.generate())
        for name,body in first.items():self.assertEqual((REPO/name).read_bytes(),body,name)
        common=b.local(REPO/b.U/'common.py','text_current_admission')
        manifest=common.read(common.HERE/'inputs.json');common.verify_package(REPO,manifest)
        names={row['path'] for row in manifest['files']}
        for name in ('stdlib/text.ox','stdlib/strings.ox','tests/stdlib_text.rs',b.ROOT+'archive/stdlib/text.ox.data',b.ROOT+'baseline.py'):
            self.assertIn(name,names)
            if name.startswith(b.ROOT):
                with self.assertRaises(common.Reject):common.verify_package(REPO,{**manifest,'files':[r for r in manifest['files'] if r['path']!=name]})
        with self.assertRaises(common.Reject):common.verify_package(REPO,json.loads(b.Baseline(REPO).read(b.U+'inputs.json')))
        self.assertNotIn(HERE/'archive/stdlib/text.ox.data',list(REPO.rglob('*.ox')))

    def test_fixed_fixture_storage_refuses_changed_and_aliased_bodies(self):
        baseline=b.Baseline(REPO)
        for name in (b.TRIM+'activate.py','tests/fixtures/typed_project_source_binding/run.py'):
            row=baseline.rows[name];raw=baseline.read(name)
            for mutation in ('changed','missing','symlink','hardlink','executable'):
                with self.subTest(name=name,mutation=mutation),tempfile.TemporaryDirectory() as directory:
                    root=Path(directory);target=root/row['storage'];target.parent.mkdir(parents=True)
                    real=root/'real';real.write_bytes(raw)
                    if mutation=='changed':target.write_bytes(raw+b'\n')
                    elif mutation=='symlink':target.symlink_to(real)
                    elif mutation=='hardlink':os.link(real,target)
                    elif mutation=='executable':target.write_bytes(raw);target.chmod(0o755)
                    original=baseline.repo;baseline.repo=root
                    try:
                        with self.assertRaises((ValueError,OSError)):baseline.read(name)
                    finally:baseline.repo=original
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);target=root/b.Q/'generation.py';target.parent.mkdir(parents=True)
            target.symlink_to(REPO/b.Q/'generation.py')
            with self.assertRaises((ValueError,OSError)):b.Baseline(root)

    def test_actual_text_language_classification(self):
        before=list(sys.path)
        try:
            sys.path.insert(0,str(REPO/'scripts'))
            verifier=b.local(REPO/'scripts/verify_repo.py','text_current_repo_verifier')
            archive=HERE/'archive/stdlib/text.ox.data';live=REPO/'stdlib/text.ox'
            neighbor=HERE/'archive/stdlib/unregistered.ox';self.assertFalse(neighbor.exists())
            sources=sorted(path for path in REPO.rglob('*.ox') if '.oxid' not in path.parts and 'target' not in path.parts)
            self.assertIn(live,sources);self.assertNotIn(archive,sources)
            checks,_,_=verifier.source_plan(sources+[neighbor],REPO)
            self.assertIn((live,False),checks);self.assertIn((neighbor,False),checks)
            self.assertFalse(any(path==archive for path,_ in checks))
            self.assertNotIn(archive,verifier.runnable_sources(REPO))
        finally:sys.path[:]=before

    def test_old_hosted_and_lifecycle_reject_current_dispatcher(self):
        old_hosted=b.local(REPO/b.TRIM/'hosted_controls.py','text_old_hosted')
        with self.assertRaisesRegex(ValueError,'successor dispatcher'):old_hosted.bootstrap_successor(REPO)
        published=b.local(REPO/'tests/qualification/unit4_public_v3/published_lifecycle.py','text_published_lifecycle')
        with patch.dict(sys.modules,{'published_lifecycle':published}):
            old_lifecycle=b.local(REPO/b.TRIM/'lifecycle.py','text_old_lifecycle')
            with self.assertRaisesRegex(published.Rejected,'fixed source dispatcher'):old_lifecycle._source()
            current=b.local(HERE/'lifecycle.py','text_current_lifecycle')
            source=current._source()
            self.assertEqual(source._modules['predecessor']._TEXT,'stdlib/text.ox')

    def test_active_text_capture_preserves_both_historical_bodies(self):
        baseline=b.Baseline(REPO);g=baseline.g
        runtime=b.local(REPO/'tests/fixtures/typed_project_source_binding/run.py','text_real_dispatcher')
        capture=runtime.preflight(REPO);self.assertEqual(len(capture['inputs']),376)
        helper=runtime._modules['predecessor'];bodies=helper.capture(REPO,capture['package_json_predecessor']['inputs'],runtime._SOURCE_SEAL['predecessor_view'])
        self.assertEqual((len(bodies),sum(map(len,bodies.values()))),(2724,78507317))
        for logical,archive in ((helper._TEXT,helper._TEXT_ARCHIVE),(helper._STRINGS,helper._ARCHIVE)):
            self.assertEqual(bodies[logical],g.regular(REPO,archive));self.assertNotEqual(bodies[logical],g.regular(REPO,logical))
        old=baseline.execute('old_predecessor',b.TRIM+'predecessor.py',baseline.read(b.TRIM+'predecessor.py'))
        with self.assertRaises(ValueError):old.capture(REPO,capture['package_json_predecessor']['inputs'],runtime._SOURCE_SEAL['predecessor_view'])

if __name__=='__main__':unittest.main()
