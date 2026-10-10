"""Focused pure admission controls. Never call a successful materializing admit."""
import copy
import os
from pathlib import Path
import tempfile
import unittest
import sys
sys.dont_write_bytecode=True
import checkout as c

# Installed default is this repository. Staging must explicitly name a fixture.
REPO=Path(os.environ.get('OXID_PACKAGE_CHECKOUT_TEST_REPOSITORY',str(Path(__file__).resolve().parents[3]))).resolve()

class AdmissionControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.context,cls.refs=c.capture(REPO)
        cls.seal,cls.layout,cls.raw=c.anchored_data()
    def validate(self,after=None,refs=None,seal=None,layout=None,raw=None):
        return c.verify_current(self.context['current_inputs'] if after is None else after,self.refs if refs is None else refs,self.seal if seal is None else seal,self.layout if layout is None else layout,self.raw if raw is None else raw)
    def test_new_inputs_separate_from_old(self):
        value=self.validate()
        self.assertEqual(value['status'],'NotReady')
        self.assertFalse(value['execution_qualified'])
        self.assertEqual(value['predecessor_preflight'],'not-run')
        self.assertEqual(len(self.refs),243)
        self.assertEqual(len(value['current_inputs']),376)
        self.assertNotEqual(value['current_inputs']['src/main.rs'],value['predecessor_inputs']['src/main.rs'])
    def test_old_main_replay_rejected(self):
        after=dict(self.context['current_inputs']);after['src/main.rs']=self.context['predecessor_inputs']['src/main.rs']
        with self.assertRaises(c.Reject):self.validate(after=after)
    def test_main_and_nonmain_fixture_accounting_mutations(self):
        names=['src/main.rs','src/frontend/lexer.rs',c.ACCOUNTING[0],next(n for n in self.context['current_inputs'] if n.startswith('tests/fixtures/'))]
        for name in names:
            with self.subTest(name=name):
                after=dict(self.context['current_inputs']);after[name]+=b' '
                with self.assertRaises(c.Reject):self.validate(after=after)
    def test_missing_extra_selected(self):
        for kind in ('missing','extra'):
            after=dict(self.context['current_inputs'])
            if kind=='missing':after.pop('src/main.rs')
            else:after['src/extra.rs']=b''
            with self.subTest(kind=kind),self.assertRaises(c.Reject):self.validate(after=after)
    def test_reference_body_mutation(self):
        for name in (c.SOURCE+'current-source.json',c.SAVED+'run.py',c.HELPERS+'current.py'):
            refs=dict(self.refs);refs[name]+=b' '
            with self.subTest(name=name),self.assertRaises(c.Reject):self.validate(refs=refs)
    def test_missing_extra_reference(self):
        refs=dict(self.refs);refs.pop(next(iter(refs)))
        with self.assertRaises(c.Reject):self.validate(refs=refs)
        refs=dict(self.refs);refs['extra']=b''
        with self.assertRaises(c.Reject):self.validate(refs=refs)
    def test_pinned_data_mutation(self):
        for name in self.raw:
            raw=dict(self.raw);raw[name]+=b' '
            with self.subTest(name=name),self.assertRaises(c.Reject):self.validate(raw=raw)
    def test_arbitrary_seal_layout_rejected(self):
        seal=copy.deepcopy(self.seal);seal['source_head']='0'*40
        with self.assertRaises(c.Reject):self.validate(seal=seal)
        layout=copy.deepcopy(self.layout);layout['files'][0]['sha256']='0'*64
        with self.assertRaises(c.Reject):self.validate(layout=layout)
    def test_execution_notready(self):
        with self.assertRaises(c.NotReady):c.execution_context(self.context)
    def test_missing_repo_rejects_before_output_write(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);output=root/'output'
            with self.assertRaises(c.Reject):c.admit(root/'absent',output)
            self.assertFalse(output.exists())
    def test_existing_output_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            output=Path(directory);(output/'sentinel').write_bytes(b'untouched')
            with self.assertRaises(c.Reject):c.admit(REPO,output)
            self.assertEqual(list(output.iterdir()),[output/'sentinel'])
            self.assertEqual((output/'sentinel').read_bytes(),b'untouched')
    def test_checkout_overlap_rejects_before_write(self):
        output=REPO/'package-checkout-forbidden-output'
        self.assertFalse(output.exists())
        with self.assertRaises(c.Reject):c.admit(REPO,output)
        self.assertFalse(output.exists())
    def test_closed_membership_rejections(self):
        import os
        for kind in ('empty-directory','dangling-symlink','fifo','executable'):
            with self.subTest(kind=kind),tempfile.TemporaryDirectory() as directory:
                root=Path(directory);p=root/'extra'
                if kind=='empty-directory':p.mkdir()
                elif kind=='dangling-symlink':p.symlink_to(root/'absent')
                elif kind=='fifo':os.mkfifo(p)
                else:p.write_bytes(b'');p.chmod(0o755)
                with self.assertRaises(c.Reject):c.closed(root,set())
    def test_synthetic_post_preflight_drift_is_rejected(self):
        # Synthetic post-worker capture only: no worker or materialization runs.
        changed=copy.deepcopy(self.context)
        changed['current_inputs']['src/main.rs']+=b' '
        with self.assertRaises(c.Reject):c.require_unchanged_capture(self.context,self.refs,changed,self.refs)
        changed_refs=dict(self.refs);changed_refs[next(iter(changed_refs))]+=b' '
        with self.assertRaises(c.Reject):c.require_unchanged_capture(self.context,self.refs,self.context,changed_refs)
        c.require_unchanged_capture(self.context,self.refs,self.context,self.refs)
    def test_no_git_execution_dependency(self):
        import ast
        tree=ast.parse(Path(c.__file__).read_text())
        self.assertFalse(any(isinstance(n,ast.Attribute) and n.attr in ('git','blob','roster') for n in ast.walk(tree)))
        self.assertNotIn('git rev-parse',Path(c.__file__).read_text())

if __name__=='__main__':unittest.main()
