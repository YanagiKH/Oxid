"""Mechanical hosted-bridge controls; no fabricated execution qualification."""
import ast
import copy
import json
from pathlib import Path
import sys
import types
import unittest
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
sys.path.insert(0,str(HERE))
import generation as g
import derive_hosted as h


def methods(raw):
    text=raw.decode();result={}
    for node in ast.parse(text).body:
        if isinstance(node,ast.FunctionDef):result[node.name]=ast.get_source_segment(text,node)
        elif isinstance(node,ast.ClassDef):
            for f in node.body:
                if isinstance(f,ast.FunctionDef):result[node.name+'.'+f.name]=ast.get_source_segment(text,f)
    return result


class HostedBridgeControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.original=g.regular(REPO,g.ROOT+'predecessor-v1/'+h.FACADE)
        cls.dispatcher=g.identity(g.PACKAGE+'run.py',g.regular(REPO,g.PACKAGE+'run.py'))
        cls.seal=json.loads(g.regular(REPO,g.ROOT+'source-seal-v1.json'))
        cls.body,cls.receipt=h.derive(cls.original,cls.dispatcher,cls.seal)
        cls.module=types.ModuleType('package_json_hosted_unit_controls');cls.module.__file__=str(REPO/h.NAME)
        exec(compile(cls.body,cls.module.__file__,'exec'),cls.module.__dict__)

    def test_exact_named_outputs_and_original_git_blob(self):
        self.assertEqual(self.body,g.regular(REPO,h.NAME))
        self.assertEqual(h.facade(self.body),g.regular(REPO,h.FACADE))
        self.assertEqual(self.receipt,json.loads(g.regular(REPO,g.ROOT+'source-proof/hosted-control-derivation-v1.json')))
        tree=g.verify_git(json.loads(g.regular(REPO,g.ROOT+'source-proof/accepted-main.json')),g.MAIN,'refs/heads/main',3417)
        self.assertEqual(g.gitid('blob',self.original),tree[h.FACADE]['sha'])

    def test_all_old_predicates_and_test_factories_except_four_explicit_seams_are_exact(self):
        before,after=methods(self.original),methods(self.body)
        changed={n for n in before if before[n]!=after[n]}
        self.assertEqual(changed,{'ExistingUnit2Evidence.__init__','ExistingUnit2Evidence.flat',
                                  'ExistingUnit2Evidence._export','SourceControls.run'})
        self.assertEqual(self.module.SourceControls().countTestCases(),92)
        self.assertEqual({k:len(v) for k,v in self.module._CONFIG['test_rosters'].items()},
                         {'source_unit':37,'source_integration':15,'view':14,'ci':20,'public_original':6})
        self.assertEqual(self.module.SOURCE_HEAD,g.FORMATTTER_HEAD)
        self.assertEqual(self.module.ACTUAL_REPOSITORY,REPO)

    def test_complete_run_gate_and_actual_verifier_remain_before_export(self):
        before,after=methods(self.original),methods(self.body)
        init=after['ExistingUnit2Evidence.__init__']
        for predicate in ('result["semantic_pass"] is True','result["direct_current_execution"] is True',
                          'result["status"] == "passed-current-unit2"','command["exit_status"] == 0',
                          'source/controller binding differs','old completed receipt hashes disagree',
                          'old compatibility command/seam is not the existing full-run layout'):
            self.assertIn(predicate,before['ExistingUnit2Evidence.__init__']);self.assertIn(predicate,init)
        self.assertLess(init.index('current_runtime.verify_unit2_result(self.root, current_capture, seam, False)'),
                        init.index('self.fd = os.open'))
        self.assertEqual(self.module.ExistingUnit2Evidence.flat('index.json'),'source-controls-package-json-v1-index.json')
        self.assertEqual(before['ExistingUnit2Evidence.preserve'],after['ExistingUnit2Evidence.preserve'])
        self.assertEqual(before['ExistingUnit2Evidence.create'],after['ExistingUnit2Evidence.create'])
        run=after['SourceControls.run']
        self.assertLess(run.index('self.accepted_scope.__exit__(None, None, None)'),run.index('write_new(self.evidence / \"complete.json\"'))
        self.assertIn('separate_from_nested_view_effect_audit',run)

    def test_receipt_association_rejects_missing_relabel_and_bool_alias(self):
        # These are unit boundary records only; they are not complete results
        # and never pass ExistingUnit2Evidence or the retained full verifier.
        expected={'package_json_source_provenance':{'source_checkpoint':g.CHECKPOINT,
                   'source_only':True,'execution_qualified':False},
                  'accepted_formatter_receipt':{'historical_source_head':g.FORMATTTER_HEAD}}
        self.module.verify_successor_receipt(copy.deepcopy(expected),expected)
        for mutation in ('missing','relabel','bool'):
            value=copy.deepcopy(expected)
            if mutation=='missing':del value['accepted_formatter_receipt']
            elif mutation=='relabel':value['package_json_source_provenance']['source_checkpoint']=g.MAIN
            else:value['package_json_source_provenance']['execution_qualified']=0
            with self.subTest(mutation=mutation),self.assertRaises(ValueError):
                self.module.verify_successor_receipt(value,expected)

    def test_original_cannot_be_coherently_rewritten_for_generation(self):
        with self.assertRaises(ValueError):h.derive(self.original+b'\n',self.dispatcher,self.seal)


if __name__=='__main__':unittest.main()
