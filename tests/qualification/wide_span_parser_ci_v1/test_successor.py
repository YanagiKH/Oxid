"""Focused CI-only derivation controls; no compiler or remote operations."""
import copy
import json
from pathlib import Path
import re
import sys
import tempfile
import types
import unittest
from unittest.mock import patch
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
p=HERE/'successor.py';v=types.ModuleType('wide_ci_test_view');v.__file__=str(p)
exec(compile(p.read_bytes(),str(p),'exec'),v.__dict__)

class SuccessorControls(unittest.TestCase):
    def test_exact_insertion_preserves_all_jobs_steps_and_budget(self):
        old=v.predecessor('.github/workflows/ci.yml');new=v.workflow(old)
        self.assertEqual(new.replace(v.STEP.encode(),b''),old)
        self.assertEqual(new.count(v.STEP.encode()),1)
        self.assertEqual(re.findall(rb'^  [a-z][a-z0-9-]*:',old,re.M),
                         re.findall(rb'^  [a-z][a-z0-9-]*:',new,re.M))
        job=new.split(b'  local-hir-producers:\n')[1].split(b'\n  streaming-lexical-provider:')[0]
        self.assertIn(b'timeout-minutes: 40',job)
        self.assertLess(job.index(b'scripts/run_hir_producer_ci.py'),job.index(b'scripts/run_wide_span_parser_ci.py'))
        self.assertLess(job.index(b'scripts/run_wide_span_parser_ci.py'),job.index(b'if: always()'))
        self.assertEqual(v.verify_workflow(),old)

    def test_unreviewed_predecessor_and_unreviewed_successor_are_rejected(self):
        old=v.predecessor('.github/workflows/ci.yml')
        for raw in [old+b'\n',old.replace(b'timeout-minutes: 40',b'timeout-minutes: 41'),old.replace(v.ANCHOR.encode(),b'')]:
            with self.subTest(raw=raw[-30:]),self.assertRaises(ValueError):v.workflow(raw)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);path=root/'.github/workflows/ci.yml';path.parent.mkdir(parents=True)
            for body in [old,v.workflow(old)+b'\n',v.workflow(old).replace(b'--receipt ',b'--ignore-receipt ')]:
                path.write_bytes(body)
                with patch.object(v,'predecessor',return_value=old),self.assertRaises(ValueError):v.verify_workflow(root)

    def test_historical_read_mapping_is_exactly_one_path(self):
        # All other requests are delegated unchanged, even workflow-like names.
        r=v.fixture();regular=r.__class__.__init__.__globals__['regular']
        self.assertEqual(regular(v.REPO,'.github/workflows/ci.yml',71965),v.predecessor('.github/workflows/ci.yml'))
        with self.assertRaises(ValueError):regular(v.REPO,'.github/workflows/ci.yml',71966)
        for name in ['.github/workflows/ci.yml.other','other/.github/workflows/ci.yml']:
            with self.assertRaises(OSError):regular(v.REPO,name,71965)
        with patch.object(v,'predecessor',return_value=b'x'*71965):
            with self.assertRaises(ValueError):regular(v.REPO,'.github/workflows/ci.yml',71965)

    def test_full_predecessor_generators_and_all_current_outputs(self):
        v.fixture().assert_scheduling_outputs()
        self.assertEqual(len(v.check_predecessor_outputs()),26)
        v.verify_inventory()

    def test_archive_members_are_pinned_and_historical_namespaces_stay_closed(self):
        rows=v.rows();self.assertEqual(len(rows),7)
        for name,row in rows.items():
            self.assertEqual(v.sha(v.predecessor(name)),row['sha256'])
        manifest=json.loads((v.REPO/v.U/'inputs.json').read_bytes())
        old=json.loads(v.predecessor(v.U+'inputs.json'))
        self.assertEqual(manifest['closed_roots'],old['closed_roots']+[v.ROOT.rstrip('/'),'fixtures/typed-parser-wide-span',v.STORAGE.rstrip('/')])
        prior={r['path']:r for r in old['files']};current={r['path']:r for r in manifest['files']}
        generated=v.storage().generate()
        changed={'scripts/test_lexer_current_ci.py','tests/qualification/package_json_current_v1/test_hosted.py'}
        changed.update(n for n,b in generated.items() if n in prior and v.sha(b)!=prior[n]['sha256'])
        self.assertEqual({n for n in prior if prior[n]!=current[n]},changed)
        self.assertTrue(set(prior)<=set(current))
        self.assertEqual(manifest['wide_span_ci']['execution_qualified'],False)

if __name__=='__main__':unittest.main(verbosity=2)
