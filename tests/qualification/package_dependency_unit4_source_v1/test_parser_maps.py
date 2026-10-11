"""Focused source-only controls; no build, subprocess, or physical preparation."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import unittest

HERE=Path(__file__).resolve().parent
REPO=Path(os.environ.get('OXID_PACKAGE_UNIT4_TEST_REPOSITORY',str(HERE.parents[2])))

def module(name):
    spec=importlib.util.spec_from_file_location('parser_test_'+name,HERE/(name+'.py'))
    result=importlib.util.module_from_spec(spec); spec.loader.exec_module(result)
    return result

class ParserMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=module('parser_maps')
        cls.context=module('source').capture(REPO)
        cls.inputs=cls.api.retained_inputs(REPO)
        cls.outputs=cls.api.derive(cls.context,cls.inputs)

    def reject_context(self, change):
        context=copy.deepcopy(self.context); change(context)
        with self.assertRaises((self.api.Reject,ValueError,KeyError)):
            self.api.derive(context,self.inputs)

    def test_complete_counts_inverse_commutation(self):
        result=self.api.verify(self.outputs,self.context,self.inputs)
        self.assertEqual([len(result['maps'][f]) for f in self.api.FIELDS],[539,542,542])
        self.assertEqual(result['status'],'NotReady')
        self.assertFalse(result['execution_qualified'])
        self.assertFalse(result['physical_preparation_performed'])
        for proof in result['commutation']:
            self.assertTrue(proof['source_then_phase_equals_phase_then_source'])
            self.assertTrue(proof['complete_source_inverse'])
            self.assertTrue(proof['complete_phase_inverse'])
        with self.assertRaises(self.api.NotReady): self.api.execution_context()

    def test_output_determinism_no_input_mutation(self):
        context=copy.deepcopy(self.context); inputs=copy.deepcopy(self.inputs)
        self.assertEqual(self.api.derive(context,inputs),self.outputs)
        self.assertEqual(context,self.context); self.assertEqual(inputs,self.inputs)

    def test_changed_missing_extra_actual_body(self):
        for change in (
            lambda c:c['current_inputs'].__setitem__('src/main.rs',c['current_inputs']['src/main.rs']+b'\n'),
            lambda c:c['current_inputs'].pop('src/main.rs'),
            lambda c:c['current_inputs'].__setitem__('extra.rs',b''),
            lambda c:c['predecessor_inputs'].__setitem__('src/main.rs',c['current_inputs']['src/main.rs']),
            lambda c:c.__setitem__('current_inputs',c['predecessor_inputs']),
        ):
            with self.subTest(change=change): self.reject_context(change)

    def test_source_only_context_flags(self):
        for key,value in (('status','Ready'),('execution_qualified',True),('execution_qualified',0)):
            with self.subTest(key=key,value=value):
                self.reject_context(lambda c:c.__setitem__(key,value))

    def test_wrong_manifest_source_head_tree(self):
        for change in (
            lambda c:c['source_checkpoint'].__setitem__('head','0'*40),
            lambda c:c['current_manifest_binding'].__setitem__('path','wrong-source.json'),
            lambda c:c['source_checkpoint'].__setitem__('tree','0'*40),
            lambda c:c.__setitem__('current_manifest',c['predecessor_manifest']),
            lambda c:c.__setitem__('predecessor_manifest',c['current_manifest']),
            lambda c:c.__setitem__('transition_patch',c['transition_patch']+b'\n'),
        ):
            with self.subTest(change=change): self.reject_context(change)

    def test_each_retained_input_is_pinned(self):
        for name in self.inputs:
            with self.subTest(name=name):
                inputs=dict(self.inputs); inputs[name]+=b'\n'
                with self.assertRaises(self.api.Reject):self.api.derive(self.context,inputs)
        for inputs in ({k:v for k,v in self.inputs.items() if k!=self.api.RUNNER},dict(self.inputs,extra=b'')):
            with self.assertRaises(self.api.Reject):self.api.derive(self.context,inputs)

    def test_duplicate_reordered_missing_extra_maps_reject(self):
        result=json.loads(self.outputs[self.api.OUTPUTS[0]])
        for field in self.api.FIELDS:
            for mutation in ('duplicate','reverse','missing','extra'):
                changed=copy.deepcopy(result); rows=changed['maps'][field]
                if mutation=='duplicate':rows[1]=rows[0]
                elif mutation=='reverse':rows.reverse()
                elif mutation=='missing':rows.pop()
                else:rows.append(rows[0])
                outputs=dict(self.outputs); outputs[self.api.OUTPUTS[0]]=self.api.serial(changed)
                with self.subTest(field=field,mutation=mutation),self.assertRaises(self.api.Reject):
                    self.api.verify(outputs,self.context,self.inputs)

    def test_changed_instrumentation_digest_and_old_substitution(self):
        result=json.loads(self.outputs[self.api.OUTPUTS[0]])
        for mutation in ('instrumentation','digest','old_map','old_authority'):
            outputs=dict(self.outputs); changed=copy.deepcopy(result)
            if mutation=='instrumentation':
                next(r for r in changed['maps']['current_derived_files'] if r['path']=='src/frontend/lexer.rs')['sha256']='0'*64
            elif mutation=='digest':changed['map_sha256']['current_derived_files']='0'*64
            elif mutation=='old_map':changed['maps']['current_derived_files']=json.loads(self.inputs[self.api.DIRECTORY+'authority.json'])['current_derived_files']
            outputs[self.api.OUTPUTS[0]]=self.inputs[self.api.DIRECTORY+'authority.json'] if mutation=='old_authority' else self.api.serial(changed)
            with self.subTest(mutation=mutation),self.assertRaises(self.api.Reject):self.api.verify(outputs,self.context,self.inputs)

    def test_named_generated_outputs_exact(self):
        self.assertEqual({n:(HERE/n).read_bytes() for n in self.api.OUTPUTS},self.outputs)

    def test_old_candidate_manifest_substitution(self):
        active=json.loads(self.inputs[self.api.DIRECTORY+'authority.json'])
        old=json.loads(self.outputs[self.api.OUTPUTS[1]])
        old.update(reviewed_source_head=active['reviewed_source_head'],source_only_tree=active['source_only_tree'],current_source_manifest_sha256=self.api.OLD_SOURCE,files=active['current_base_files'])
        outputs=dict(self.outputs); outputs[self.api.OUTPUTS[1]]=self.api.serial(old)
        with self.assertRaises(self.api.Reject):self.api.verify(outputs,self.context,self.inputs)

    def test_named_candidate_manifest_association(self):
        candidate=json.loads(self.outputs[self.api.OUTPUTS[1]])
        for key in ('reviewed_source_head','source_only_tree','current_source_manifest_sha256'):
            changed=copy.deepcopy(candidate); changed[key]='0'*len(changed[key])
            outputs=dict(self.outputs);outputs[self.api.OUTPUTS[1]]=self.api.serial(changed)
            with self.subTest(key=key),self.assertRaises(self.api.Reject):self.api.verify(outputs,self.context,self.inputs)
        outputs=dict(self.outputs);outputs['candidate-source-manifest.json']=outputs.pop(self.api.OUTPUTS[1])
        with self.assertRaises(self.api.Reject):self.api.verify(outputs,self.context,self.inputs)

if __name__=='__main__': unittest.main()
