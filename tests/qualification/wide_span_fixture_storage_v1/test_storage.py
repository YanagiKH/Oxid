"""Fixed fixture-data relocation and real current source-view controls."""
import ast
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]

def load(path, **scope):
    module=types.ModuleType('fixture_test_'+path.stem);module.__file__=str(path)
    module.__dict__.update(scope)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module

v=load(HERE/'successor.py')

def helper():
    g=load(REPO/v.Q/'generation.py')
    raw=v.derive.derive(g.regular(REPO,v.derive.PREVIOUS)).decode()
    raw=v.exact(raw,'import generation as g\n','')
    m=types.ModuleType('fixture_test_helper');m.__file__=str(HERE/'predecessor.py');m.g=g
    exec(compile(raw,m.__file__,'exec'),m.__dict__)
    return m,g

class FixtureStorageControls(unittest.TestCase):
    def test_fixed_derivation_and_only_capture_changes(self):
        original=(REPO/v.derive.PREVIOUS).read_bytes()
        first=v.derive.derive(original);self.assertEqual(first,v.derive.derive(original))
        self.assertEqual(first,(HERE/'predecessor.py').read_bytes())
        def methods(raw):
            text=raw.decode();return {n.name:ast.get_source_segment(text,n) for n in ast.parse(text).body if isinstance(n,ast.FunctionDef)}
        old,new=methods(original),methods(first)
        self.assertEqual({n for n in old if old[n]!=new[n]},{'capture'})
        with self.assertRaises(ValueError):v.derive.derive(original+b'\n')

    def test_fixed_archive_and_coherent_rehash_refusals(self):
        p,g=helper();row=p._FIXTURE_ROW;body=g.regular(REPO,p._FIXTURE_ARCHIVE)
        for mutation in ('missing','wrong','oversize','symlink','hardlink','executable','directory'):
            with self.subTest(mutation=mutation),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);target=root/p._FIXTURE_ARCHIVE;target.parent.mkdir(parents=True)
                live=root/p._FIXTURE_TEST;live.parent.mkdir();live.write_bytes(body)
                real=root/'real';real.write_bytes(body)
                if mutation=='wrong':target.write_bytes(b'X'+body[1:])
                elif mutation=='oversize':target.write_bytes(body+b'\n')
                elif mutation=='symlink':target.symlink_to(real)
                elif mutation=='hardlink':os.link(real,target)
                elif mutation=='executable':target.write_bytes(body);target.chmod(0o755)
                elif mutation=='directory':target.mkdir()
                with self.assertRaises((ValueError,OSError)):p._fixture_archive(root,copy.deepcopy(row))
        for key,value in (('read_from',p._FIXTURE_ARCHIVE),('bytes',True),('bytes',float(row['bytes'])),('mode','100755'),('git_blob','0'*40),('sha256','0'*64),('extra',0)):
            with self.subTest(key=key),self.assertRaises(ValueError):p._fixture_archive(REPO,{**row,key:value})
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);path=root/p._FIXTURE_ARCHIVE;path.parent.mkdir(parents=True)
            bad=body+b'\n';path.write_bytes(bad)
            with self.assertRaises(ValueError):p._fixture_archive(root,{**row,**g.identity(p._FIXTURE_TEST,bad),'git_blob':g.gitid('blob',bad)})

    def test_exact_original_membership_and_live_test_derivation(self):
        self.assertEqual(len(v.names(REPO)),14)
        with self.assertRaises(ValueError):v.read_original('unlisted',REPO)
        p,g=helper();raw=v.read_original(p._FIXTURE_TEST,REPO)
        self.assertEqual(raw,g.regular(REPO,p._FIXTURE_ARCHIVE))
        derived=load(HERE/'test_derivation.py').derive(raw)
        self.assertEqual(derived,(REPO/p._FIXTURE_TEST).read_bytes())
        self.assertNotEqual(derived,raw)
        for marker in (b'(209, 143, 75, 79, 8)',b'len(predecessor_commands), 205',
                       b'7bba06dbfb7fa7cbe33ed94eb0201224cb3633f0048d621de30a7630adcce1b4',
                       b'38a388157d6bef824ab927a6f06021349e4a6e4a87ada8edd6ffea61873493ed',
                       b'ff2cfd1a71b69c54062c03ea03928945ef3a00eb83c513fd96b05360897afe9e'):
            self.assertIn(marker,raw);self.assertIn(marker,derived)

    def test_actual_dispatcher_preserves_entire_historical_view(self):
        runtime=load(REPO/v.S/'run.py');capture=runtime.preflight(REPO)
        self.assertEqual(len(capture['inputs']),376)
        p=runtime._modules['predecessor'];g=runtime._modules['generation']
        bodies=p.capture(REPO,capture['package_json_predecessor']['inputs'],runtime._SOURCE_SEAL['predecessor_view'])
        self.assertEqual((len(bodies),sum(map(len,bodies.values()))),(2724,78507317))
        identities=g.encoded([g.identity(n,raw) for n,raw in sorted(bodies.items())])
        self.assertEqual(g.sha(identities),'e70b8b790756e0fb9e0cd1341779c51646b964afcf9e2eade2bd9480716b1882')
        self.assertEqual(bodies[p._FIXTURE_TEST],v.read_original(p._FIXTURE_TEST,REPO))
        self.assertNotEqual(bodies[p._FIXTURE_TEST],(REPO/p._FIXTURE_TEST).read_bytes())
        with self.assertRaisesRegex(ValueError,'fixture-data test cannot be restored source'):
            p.capture(REPO,{p._FIXTURE_TEST:bodies[p._FIXTURE_TEST]},runtime._SOURCE_SEAL['predecessor_view'])

if __name__=='__main__':unittest.main()
