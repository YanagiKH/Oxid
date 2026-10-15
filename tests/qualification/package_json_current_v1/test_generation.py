"""Focused source-only controls. No Rust build or execution qualification."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import types
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).absolute().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))
import generation as g
import parser as transport


class SourceControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.previous = g.regular(REPO, g.PACKAGE + g.PREDECESSOR_NAME)
        cls.inputs = {r['path']: g.regular(REPO, r['path']) for r in json.loads(cls.previous)['files']}
        cls.evidence = {n: g.regular(REPO, g.ROOT + 'source-proof/' + n + '.json') for n in g.READBACKS}
        cls.patch = g.regular(REPO, g.ROOT + 'generated/' + g.PATCH_NAME)
        cls.runner = g.regular(REPO, 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py')
        cls.outputs, cls.restored = cls.generate()
        cls.admitted = {'current': json.loads(cls.outputs[g.CURRENT_NAME]), 'inputs': cls.inputs,
                        'predecessor_inputs': cls.restored, 'artifacts': cls.outputs}
        cls.old = json.loads(g.regular(REPO, 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json'))
        cls.old['current'] = json.loads(g.regular(REPO, g.ROOT+'predecessor-v1/tests/qualification/unit4_parser_current/authority.json'))
        cls.old['current_source'] = json.loads(cls.previous)
        cls.inserted = json.loads(g.regular(REPO, 'tests/qualification/unit4_public_v3/published-lifecycle-v1.json'))['inserted_roster']
        cls.dispatcher = g.identity(g.PACKAGE+'run.py', g.regular(REPO, g.PACKAGE+'run.py'))

    @classmethod
    def generate(cls, **changes):
        arguments = dict(previous_raw=cls.previous, inputs=cls.inputs, patch=cls.patch,
                         evidence=cls.evidence, runner=cls.runner)
        arguments.update(changes)
        return g.generate(**arguments)

    def test_actual_git_objects_and_all_376_inverse_bodies(self):
        before, after = g.readbacks(self.evidence)
        for n, raw in self.restored.items():
            self.assertEqual(g.gitid('blob', raw), before[n]['sha'])
            self.assertEqual(g.gitid('blob', self.inputs[n]), after[n]['sha'])
        self.assertEqual([n for n in self.inputs if self.inputs[n] != self.restored[n]], list(g.PATHS))
        self.assertEqual(sum(self.inputs[n] == self.restored[n] for n in self.inputs), 374)
        for n, raw in self.outputs.items():
            self.assertEqual(raw, g.regular(REPO, g.ROOT+'generated/'+n))
        self.assertFalse(json.loads(self.outputs[g.PROOF_NAME])['execution_qualified'])

    def test_missing_either_source_or_third_changed_source(self):
        for n in (*g.PATHS, 'src/frontend/lexer.rs'):
            changed = dict(self.inputs)
            if n in g.PATHS: del changed[n]
            else: changed[n] += b'\n'
            with self.subTest(path=n), self.assertRaises(ValueError): self.generate(inputs=changed)

    def test_empty_duplicate_unordered_rosters(self):
        rows = json.loads(self.previous)['files']
        for changed in ([], rows+rows[-1:], list(reversed(rows))):
            with self.assertRaises(ValueError): g.rows_map(changed)
        with self.assertRaises(ValueError): self.generate(inputs=dict(reversed(list(self.inputs.items()))))

    def test_unapproved_complete_data_body(self):
        changed = dict(self.inputs);changed['src/runtime/data.rs'] += b'\n'
        with self.assertRaises(ValueError): self.generate(inputs=changed)

    def test_coherent_data_rehash_cannot_replace_git_blob_or_inverse(self):
        changed = dict(self.inputs);changed['src/runtime/data.rs'] += b'\n// coherent negative\n'
        previous = json.loads(self.previous)
        previous['files'] = [g.identity(r['path'], changed[r['path']]) if r['path']=='src/runtime/data.rs' else r for r in previous['files']]
        with self.assertRaises(ValueError): self.generate(previous_raw=g.encoded(previous), inputs=changed)
        with self.assertRaises(ValueError): self.generate(inputs=changed, patch=self.patch+b'\n')
        evidence = dict(self.evidence);record=json.loads(evidence['source-checkpoint'])
        next(r for r in record['entries'] if r['path']=='src/runtime/data.rs')['sha']=g.gitid('blob',changed['src/runtime/data.rs'])
        evidence['source-checkpoint']=g.encoded(record)
        with self.assertRaises(ValueError): self.generate(inputs=changed,evidence=evidence)
        with self.assertRaises(ValueError): g.verify_git(record,g.CHECKPOINT,'refs/heads/KH/alias-unicode-source-v1',3418)

    def test_inverse_missing_extra_or_wrong_body(self):
        for patch in (b'', self.patch+b'\n', self.patch.replace(b'Unicode',b'UnicodX',1)):
            if patch == self.patch: continue
            with self.assertRaises(ValueError): self.generate(patch=patch)
        with self.assertRaises(ValueError): self.generate(runner=self.runner+b'\n')

    def test_readback_required_and_commit_relabel_rejected(self):
        with self.assertRaises(ValueError): self.generate(evidence={})
        record=json.loads(self.evidence['source-checkpoint']);record['raw_commit_utf8']+='\n'
        with self.assertRaises(ValueError):g.verify_git(record,g.CHECKPOINT,'refs/heads/KH/alias-unicode-source-v1',3418)
        record=json.loads(self.evidence['source-checkpoint']);record['commit']=g.MAIN['head']
        with self.assertRaises(ValueError):g.verify_git(record,g.CHECKPOINT,'refs/heads/KH/alias-unicode-source-v1',3418)

    def test_v1_manifest_modification_and_checkpoint_relabel(self):
        for key,value in [('reviewed_source_head',g.CHECKPOINT['head']),('purpose','changed')]:
            previous=json.loads(self.previous);previous[key]=value
            with self.assertRaises(ValueError):self.generate(previous_raw=g.encoded(previous))

    def test_v1_artifacts_byte_identical_to_accepted_git(self):
        before,_=g.readbacks(self.evidence)
        for n,row in before.items():
            if n.startswith('tests/qualification/published_formatter_current/'):
                self.assertEqual(g.gitid('blob',g.regular(REPO,n)),row['sha'],n)
        for name in (g.PREDECESSOR_NAME,'published-formatter-authority-v1.json','published-formatter-transition-v1.patch','published-main-source-proof-v1.json'):
            n=g.PACKAGE+name;self.assertEqual(g.gitid('blob',g.regular(REPO,n)),before[n]['sha'])

    def test_data_symlink_and_executable_mode_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory); path=root/'src/runtime/data.rs';path.parent.mkdir(parents=True)
            real=root/'real';real.write_bytes(self.inputs['src/runtime/data.rs']);path.symlink_to(real)
            with self.assertRaises(ValueError):g.regular(root,'src/runtime/data.rs')
            path.unlink();path.write_bytes(real.read_bytes());path.chmod(0o755)
            with self.assertRaises(ValueError):g.regular(root,'src/runtime/data.rs')

    def test_materialized_data_current_not_predecessor_and_no_tamper(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for n in g.PATHS:
                p=root/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(self.inputs[n])
            g.verify_materialized(root,self.inputs,self.restored)
            for raw in (self.restored['src/runtime/data.rs'],self.inputs['src/runtime/data.rs']+b'\n'):
                (root/'src/runtime/data.rs').write_bytes(raw)
                with self.assertRaises(ValueError):g.verify_materialized(root,self.inputs,self.restored)

    def test_parser_actual_cardinalities_and_complete_inverse(self):
        result=transport.transport(self.old,self.admitted,self.dispatcher,self.inserted)
        active=result['current']
        self.assertEqual([len(active[k]) for k in ('source_delta','current_base_files','current_derived_files','current_control_derived_files')],[342,539,542,542])
        self.assertEqual(active['published_main_parser'],self.old['current']['published_main_parser'])
        self.assertEqual([r['path'] for r in active[transport.KEY]['source_provenance']['source_deltas']],list(g.PATHS))
        self.assertEqual(g.encoded(active),g.regular(REPO,'tests/qualification/unit4_parser_current/authority.json'))

    def test_data_absent_from_instrumentation_control_observer_accounting(self):
        for field in ('instrumentation','control_instrumentation'):
            old=copy.deepcopy(self.old);old[field].append({'path':'src/runtime/data.rs'})
            with self.assertRaises(ValueError):transport.ordinary_sources(old,self.inserted,g.ACCOUNTING_PATHS)
        with self.assertRaises(ValueError):transport.ordinary_sources(self.old,self.inserted+[{'path':'src/runtime/data.rs'}],g.ACCOUNTING_PATHS)
        with self.assertRaises(ValueError):transport.ordinary_sources(self.old,self.inserted,(*g.ACCOUNTING_PATHS,'src/runtime/data.rs'))

    def test_parser_rejects_rehashed_historical_authority(self):
        old=copy.deepcopy(self.old);old['current']['reviewed_source_head']=g.CHECKPOINT['head']
        with self.assertRaises(ValueError):transport.transport(old,self.admitted,self.dispatcher,self.inserted)


if __name__ == '__main__':
    unittest.main()
