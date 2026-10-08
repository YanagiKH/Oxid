"""Fast admission controls use synthetic evidence, never component-use claims."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import qualify_streaming_lexer as gate
from qualify_streaming_lexer_controls import expected_receipts as controls_recipe
from run_streaming_lexer_ci import Runner


def save(path,value):path.write_text(json.dumps(value)+"\n")


def fixture(root):
    expected=dict(source_head='a'*40,compiler_sha256={'debug':'b'*64,'release':'c'*64},
                  harness_sha256='d'*64,lexical_source_manifest_sha256='e'*64,v2_source_manifest_sha256='f'*64)
    receipts=[]
    for name in gate.expected_receipts():
        row=dict(name=name,status=0,argv=['synthetic',name])
        for stream in ('stdout','stderr'):
            data=b'';(root/(name+'.'+stream)).write_bytes(data);row[stream+'_sha256']=gate.digest(data)
        receipts.append(row)
    save(root/'receipts.json',receipts)
    save(root/'module-receipts.json',[{'synthetic':i}for i in range(82)])
    controls={}
    for profile in ('debug','release'):
        directory=root/(profile+'-controls');directory.mkdir();rows=[]
        for name,status in controls_recipe():
            row=dict(name=name,status=status,argv=['synthetic',name])
            for stream in ('stdout','stderr'):
                data=b'';(directory/(name+'.'+stream)).write_bytes(data);row[stream+'_sha256']=gate.digest(data)
            rows.append(row)
        save(directory/'receipts.json',rows)
        controls[profile]=dict(status='passed',receipts=111,compiler_sha256=expected['compiler_sha256'][profile],
                               receipts_sha256=gate.digest((directory/'receipts.json').read_bytes()))
        save(directory/'summary.json',controls[profile])
    summary=dict(expected,schema_version=1,status='passed',component_use_only=True,module_invocations=82,
                 corpus_cases=623,exact_diagnostics=69,controls=controls,
                 receipts_sha256=gate.digest((root/'receipts.json').read_bytes()),
                 modules_sha256=gate.digest((root/'module-receipts.json').read_bytes()))
    save(root/'summary.json',summary)
    return expected,summary,receipts


class EvidenceControls(unittest.TestCase):
    def test_exact_recipes_and_corpus(self):
        self.assertEqual(len(gate.expected_receipts()),737)
        self.assertEqual(len(set(gate.expected_receipts())),737)
        self.assertEqual(len(controls_recipe()),111)
        cases=list(gate.split_cases())
        self.assertEqual(len(cases),623)
        self.assertEqual(len({(source,limit)for _,source,limit in cases}),623)

    def test_complete_synthetic_metadata_is_admitted(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);expected,summary,_=fixture(root)
            self.assertEqual(gate.validate_summary(root,expected),summary)

    def test_wrong_identity_counts_and_status_rejected(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);expected,original,_=fixture(root)
            changes=[(key,'wrong')for key in expected]+[('module_invocations',81),('module_invocations',True),
                     ('corpus_cases',622),('exact_diagnostics',68),('status','pending'),('component_use_only',False)]
            for key,value in changes:
                summary=copy.deepcopy(original);summary[key]=value;save(root/'summary.json',summary)
                with self.subTest(key=key),self.assertRaises(RuntimeError):gate.validate_summary(root,expected)

    def test_retained_stream_tampering_rejected(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);expected,_,rows=fixture(root)
            (root/(rows[0]['name']+'.stdout')).write_bytes(b'changed')
            with self.assertRaisesRegex(RuntimeError,'stream identity'):gate.validate_summary(root,expected)

    def test_coherent_recipe_and_failure_status_tampering_rejected(self):
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);expected,summary,rows=fixture(root)
            rows[0]['name']='omitted-build';save(root/'receipts.json',rows)
            summary['receipts_sha256']=gate.digest((root/'receipts.json').read_bytes());save(root/'summary.json',summary)
            with self.assertRaisesRegex(RuntimeError,'command recipe'):gate.validate_summary(root,expected)
        with tempfile.TemporaryDirectory()as tmp:
            root=Path(tmp);expected,summary,_=fixture(root);directory=root/'debug-controls'
            rows=json.loads((directory/'receipts.json').read_bytes())
            next(row for row in rows if row['status']==1)['status']=0
            save(directory/'receipts.json',rows)
            summary['controls']['debug']['receipts_sha256']=gate.digest((directory/'receipts.json').read_bytes())
            save(directory/'summary.json',summary['controls']['debug']);save(root/'summary.json',summary)
            with self.assertRaisesRegex(RuntimeError,'failure-control recipe'):gate.validate_summary(root,expected)

    def test_runner_keeps_failed_command_evidence(self):
        with tempfile.TemporaryDirectory()as tmp:
            args=SimpleNamespace(output=Path(tmp)/'evidence',expected_head='a'*40,event_sha='b'*40)
            runner=Runner(args)
            with patch('run_hir_producer_ci.subprocess.run',return_value=subprocess.CompletedProcess(['bad'],7,b'out',b'err')):
                with self.assertRaises(RuntimeError):runner.call('bad',['bad'])
            receipt=json.loads((args.output/'receipt.json').read_bytes())
            self.assertFalse(receipt['complete']);self.assertEqual(receipt['commands'][0]['status'],7)
            self.assertEqual((args.output/'bad.stdout').read_bytes(),b'out')


if __name__=='__main__':unittest.main()
