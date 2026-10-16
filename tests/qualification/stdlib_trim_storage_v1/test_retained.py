"""Retained mechanical algebra and closed read/import/association refusals."""
import copy
import io
import importlib.util
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
sys.path.insert(0,str(HERE))
import retained as r


class RetainedControls(unittest.TestCase):
    def test_complete_retained_algebra_and_hosted_controls(self):
        f=r.Fixture(REPO);f.authenticate_all()
        # Global unittest discovery may already have loaded another common.
        # Load the actual current admission module privately, without replacing it.
        calls=[]
        def verify_current():
            calls.append(True)
            common.verify_package(REPO,common.read(common.HERE/'inputs.json'))
        poison=types.ModuleType('common')
        def rejected_ambient(*args,**kwargs):
            raise AssertionError('ambient common admission was called')
        poison.verify_package=rejected_ambient
        with patch.dict(sys.modules,{'common':poison}):
            spec=importlib.util.spec_from_file_location('trim_current_admission',REPO/r.U/'common.py')
            common=importlib.util.module_from_spec(spec)
            spec.loader.exec_module(common)
            suite=unittest.TestSuite([f.scheduling_suite(verify_current),f.hosted_suite()])
            result=unittest.TextTestRunner(stream=io.StringIO()).run(suite)
            self.assertIs(sys.modules['common'],poison)
            manifest=common.read(common.HERE/'inputs.json')
            bad={**manifest,'files':[row for row in manifest['files'] if row['path']!=r.ROOT+'retained.py']}
            with self.assertRaises(common.Reject):common.verify_package(REPO,bad)
        self.assertTrue(result.wasSuccessful(),result.errors or result.failures)
        self.assertEqual(result.testsRun,7)
        self.assertEqual(calls,[True])
        self.assertEqual(f.scheduling_outputs(),f.scheduling_outputs())

    def test_readthrough_live_and_archive_and_missing_refuse(self):
        original=r.Fixture(REPO)
        live='tests/qualification/package_json_current_v1/source-seal-v1.json'
        archived='tests/fixtures/typed_project_source_binding/run.py'
        for name in (live,archived):
            for change in ('changed','missing','symlink','hardlink','executable'):
                with self.subTest(name=name,change=change),tempfile.TemporaryDirectory() as directory:
                    root=Path(directory)
                    manifest=root/r.ROOT/'mechanical-inputs-v1.json';manifest.parent.mkdir(parents=True)
                    manifest.write_bytes((REPO/r.ROOT/'mechanical-inputs-v1.json').read_bytes())
                    row=original.rows[name];target=root/row['storage'];target.parent.mkdir(parents=True,exist_ok=True)
                    body=original.read(name)
                    if change!='missing':target.write_bytes(body)
                    if change=='changed':target.write_bytes(body[:-1]+b' ')
                    if change=='symlink':
                        target.unlink();other=root/'other';other.write_bytes(body);target.symlink_to(other)
                    if change=='hardlink':os.link(target,root/'other')
                    if change=='executable':target.chmod(0o755)
                    # A good live body must not rescue an archived dependency.
                    if name!=row['storage']:
                        fallback=root/name;fallback.parent.mkdir(parents=True,exist_ok=True);fallback.write_bytes(body)
                    f=r.Fixture(root)
                    with self.assertRaises((ValueError,OSError)):f.read(name)

    def test_manifest_membership_and_commit_association_refuse(self):
        raw=(REPO/r.ROOT/'mechanical-inputs-v1.json').read_bytes()
        for mutation in ('omit','extra','storage','commit'):
            value=json.loads(raw)
            if mutation=='omit':value['rows'].pop()
            if mutation=='extra':value['rows'].append(value['rows'][0])
            if mutation=='storage':value['rows'][0]['storage']='another/body'
            if mutation=='commit':value['git_rows'][0]['commit']=value['baseline']
            with tempfile.TemporaryDirectory() as directory:
                path=Path(directory)/r.ROOT/'mechanical-inputs-v1.json';path.parent.mkdir(parents=True)
                path.write_text(json.dumps(value))
                with self.assertRaises(ValueError):r.Fixture(Path(directory))
        f=r.Fixture(REPO)
        with self.assertRaises(ValueError):f.git_read('8debaa96c0238e3376567fa2c76517fda9937037','.github/workflows/ci.yml')
        with self.assertRaises(ValueError):f.git_read(r.BASE,'unlisted')
        value=json.loads(raw);value['git_rows'][0]['commit']=value['baseline']
        f.git_rows={row['path']:row for row in value['git_rows']}
        with self.assertRaisesRegex(ValueError,'per-blob commit'):f._git_proof(value)

    def test_unlisted_read_code_and_ambient_ci_refuse(self):
        f=r.Fixture(REPO)
        with self.assertRaises(ValueError):f.read('stdlib/strings.ox')
        with self.assertRaises(ValueError):f.read_path(REPO/'../outside')
        with self.assertRaises(ValueError):f.module('unlisted')
        with self.assertRaises(ValueError):f._import('unlisted')
        with self.assertRaises(ValueError):r.exact('anchor anchor','anchor','new')
        with self.assertRaises(ValueError):r.exact('absent','anchor','new')
        poison=types.ModuleType('ci')
        def poisoned():raise AssertionError('ambient ci executed')
        poison.scheduling=poisoned
        before=sys.modules.get('ci');sys.modules['ci']=poison
        try:
            self.assertIsNot(f.module('ci'),poison)
            f.assert_scheduling_outputs()
            self.assertIs(sys.modules['ci'],poison)
        finally:
            if before is None:sys.modules.pop('ci',None)
            else:sys.modules['ci']=before
        # Actual additional IO through the adapter is denied, not merely observed.
        with self.assertRaisesRegex(ValueError,'unlisted retained read'):
            f.read('tests/qualification/unlisted-input.py')

    def test_changed_executable_helper_refuses_before_execution(self):
        f=r.Fixture(REPO);name=r.HELPERS['ci'];row=f.rows[name]
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);manifest=root/r.ROOT/'mechanical-inputs-v1.json';manifest.parent.mkdir(parents=True)
            manifest.write_bytes((REPO/r.ROOT/'mechanical-inputs-v1.json').read_bytes())
            target=root/row['storage'];target.parent.mkdir(parents=True,exist_ok=True)
            raw=f.read(name);target.write_bytes(raw.replace(b'import copy',b'import evil'))
            with self.assertRaises(ValueError):r.Fixture(root).module('ci')


if __name__=='__main__':unittest.main()
