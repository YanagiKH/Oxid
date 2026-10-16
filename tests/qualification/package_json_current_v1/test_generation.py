"""Focused source-only controls. No Rust build or execution qualification."""
import copy
from contextlib import nullcontext
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import stat
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch, MagicMock

sys.dont_write_bytecode = True
HERE = Path(__file__).absolute().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))
import generation as g
import parser as transport
import predecessor
import derive_consumers as consumers


class AdapterTimeoutControls(unittest.TestCase):
    CASES = {
        'test_original_preparation_and_exact_transformations':
            ('tests/qualification/lexer_reservation_current/test_preparation.py', 300),
        'test_current_unit1_runner_and_correspondence':
            ('tests/qualification/lexer_reservation_current/test_unit1_current.py', 300),
        'test_current_public_lifecycle_composition':
            ('tests/qualification/unit4_public_v3/test_lexer_reservation.py', 300),
        'test_current_parser_token_and_reserve_composition':
            ('tests/qualification/unit4_parser_current/test_lexer_reservation.py', 900),
        'test_current_parser_lexer_phase_source_controls':
            ('tests/qualification/unit4_parser_current/test_lexer_phase.py', 300),
    }

    def setUp(self):
        self.module = types.ModuleType('timeout_dispatch_controls')
        path = REPO / consumers.ADAPTER_TEST
        self.module.__file__ = str(path)
        exec(compile(path.read_bytes(), str(path), 'exec'), self.module.__dict__)
        self.case = self.module.CurrentLexerAdapterSuites(
            'test_current_parser_token_and_reserve_composition')

    def test_exact_five_dispatches_preserve_argv_and_only_one_extended_timeout(self):
        cls = self.module.CurrentLexerAdapterSuites
        self.assertEqual(set(unittest.defaultTestLoader.getTestCaseNames(cls)), set(self.CASES))
        for name, (relative, timeout) in self.CASES.items():
            with self.subTest(method=name), patch.object(self.module.subprocess, 'run',
                    return_value=types.SimpleNamespace(returncode=0, stdout='', stderr='')) as run:
                getattr(cls(name), name)()
                run.assert_called_once()
                args, kwargs = run.call_args
                self.assertEqual(args, ([sys.executable, '-B', str(REPO / relative), '-v'],))
                self.assertEqual(kwargs, {'cwd': REPO, 'capture_output': True, 'text': True,
                    'timeout': timeout, 'env': {**self.module.os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}})

    def test_nonzero_exit_keeps_both_streams_and_fails(self):
        with patch.object(self.module.subprocess, 'run', return_value=types.SimpleNamespace(
                returncode=7, stdout='retained stdout', stderr='retained stderr')) as run:
            with self.assertRaisesRegex(AssertionError, 'retained stdoutretained stderr'):
                self.case.test_current_parser_token_and_reserve_composition()
            self.assertEqual(run.call_args.kwargs['timeout'], 900)

    def test_timeout_exception_and_partial_streams_propagate_unchanged(self):
        failure = subprocess.TimeoutExpired(['parser-control'], 900,
            output='partial stdout', stderr='partial stderr')
        with patch.object(self.module.subprocess, 'run', side_effect=failure) as run:
            with self.assertRaises(subprocess.TimeoutExpired) as caught:
                self.case.test_current_parser_token_and_reserve_composition()
            self.assertIs(caught.exception, failure)
            self.assertEqual((failure.output, failure.stderr), ('partial stdout', 'partial stderr'))
            self.assertEqual(run.call_args.kwargs['timeout'], 900)

    def test_finite_derivation_matches_live_and_rejects_anchor_drift(self):
        old = (HERE / 'predecessor-v1' / consumers.ADAPTER_TEST).read_bytes()
        self.assertEqual(consumers.adapter_timeout_test(old), (REPO / consumers.ADAPTER_TEST).read_bytes())
        for before in (b'def run_source_controls(self, relative):', b'text=True, timeout=300,',
                b"self.run_source_controls('tests/qualification/unit4_parser_current/test_lexer_reservation.py')"):
            for changed in (old.replace(before, b'changed anchor'), old + before):
                with self.subTest(anchor=before, duplicate=changed.endswith(before)), self.assertRaises(ValueError):
                    consumers.adapter_timeout_test(changed)


class RecordViewTimeoutControls(unittest.TestCase):
    def setUp(self):
        self.module = types.ModuleType('record_view_timeout_controls')
        path = REPO / consumers.RECORD_VIEW_TEST
        self.module.__file__ = str(path)
        exec(compile(path.read_bytes(), str(path), 'exec'), self.module.__dict__)
        self.case = self.module.RuntimeViewPackageControls(
            'test_exact_seven_case_parser_amendment_controls')

    def test_only_seven_case_dispatch_gets_120_seconds(self):
        module = self.module
        with patch.object(module.subprocess, 'run', return_value=types.SimpleNamespace(
                returncode=0, stdout='', stderr='Ran 7 tests')) as run:
            self.case.test_exact_seven_case_parser_amendment_controls()
            run.assert_called_once_with([sys.executable, '-B', str(REPO /
                'tests/qualification/unit4_parser_current/test_record_composition_amendment.py')],
                capture_output=True, text=True, timeout=120)
        with patch.object(module.observer_controls, 'admitted_composition_view',
                return_value=nullcontext((REPO, {}))), \
             patch.object(module.subprocess, 'run', side_effect=[
                 types.SimpleNamespace(returncode=0, stdout='', stderr='Ran ' + str(count) + ' tests')
                 for count in (15, 15, 4, 4)]) as run:
            self.case.test_runtime_view_and_transfer_hook_under_normal_and_optimized_python()
            self.assertEqual(run.call_count, 4)
            for call, (script, optimized) in zip(run.call_args_list,
                    [(script, optimized) for script in ('test_runtime_view.py', 'test_transfer_hook.py')
                     for optimized in (False, True)]):
                self.assertEqual(call.args, (module.observer_controls.frozen_controls_argv(
                    module.PACKAGE / script, REPO, optimized),))
                self.assertEqual(call.kwargs, {'capture_output': True, 'text': True, 'timeout': 30})

    def test_nonzero_exit_and_wrong_case_count_still_fail(self):
        for code, stderr, expected in ((9, 'child stderr', 'child stdoutchild stderr'),
                                       (0, 'Ran 6 tests', 'Ran 7 tests')):
            with self.subTest(code=code), patch.object(self.module.subprocess, 'run',
                    return_value=types.SimpleNamespace(returncode=code, stdout='child stdout', stderr=stderr)):
                with self.assertRaisesRegex(AssertionError, expected):
                    self.case.test_exact_seven_case_parser_amendment_controls()

    def test_timeout_and_partial_output_propagate_unchanged(self):
        failure = subprocess.TimeoutExpired(['amendment-controls'], 120,
            output=b'partial stdout', stderr=b'partial stderr')
        with patch.object(self.module.subprocess, 'run', side_effect=failure) as run:
            with self.assertRaises(subprocess.TimeoutExpired) as caught:
                self.case.test_exact_seven_case_parser_amendment_controls()
            self.assertIs(caught.exception, failure)
            self.assertEqual((failure.output, failure.stderr), (b'partial stdout', b'partial stderr'))
            self.assertEqual(run.call_args.kwargs['timeout'], 120)

    def test_finite_derivation_preserves_other_controls_and_rejects_drift(self):
        old = (HERE / 'predecessor-v1' / consumers.RECORD_VIEW_TEST).read_bytes()
        live = (REPO / consumers.RECORD_VIEW_TEST).read_bytes()
        self.assertEqual(consumers.record_view_timeout_test(old), live)
        self.assertEqual(live.replace(b'timeout=120', b'timeout=30'), old)
        for changed in (old.replace(b'timeout=30', b'timeout=31'), old + old):
            with self.assertRaises(ValueError):
                consumers.record_view_timeout_test(changed)


class AcceptedViewControls(unittest.TestCase):
    def test_portable_exact_membership(self):
        bodies = {'nested/deeper/file.txt': b'content'}
        for flavor in (PurePosixPath, PureWindowsPath):
            with self.subTest(flavor=flavor.__name__), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                target = root / 'nested/deeper/file.txt'
                target.parent.mkdir(parents=True)
                target.write_bytes(bodies['nested/deeper/file.txt'])
                with patch.object(predecessor, 'Path', flavor):
                    predecessor.check(root, bodies)
                    for kind in ('file', 'directory', 'missing', 'changed'):
                        extra = root / 'nested/unexpected'
                        if kind == 'file': extra.write_bytes(b'extra')
                        elif kind == 'directory': extra.mkdir()
                        elif kind == 'missing': target.unlink()
                        else: target.write_bytes(b'changed')
                        with self.subTest(mutation=kind), self.assertRaises(ValueError):
                            predecessor.check(root, bodies)
                        if kind == 'file': extra.unlink()
                        elif kind == 'directory': extra.rmdir()
                        else: target.write_bytes(bodies['nested/deeper/file.txt'])
                    predecessor.check(root, bodies)

    def test_directory_and_file_symlinks_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            outside = root / 'outside'
            outside.mkdir()
            (outside / 'file.txt').write_bytes(b'content')
            view = root / 'view'
            view.mkdir()
            for directory_link in (False, True):
                link = view / ('nested' if directory_link else 'file.txt')
                target = outside if directory_link else outside / 'file.txt'
                try:
                    link.symlink_to(target, target_is_directory=directory_link)
                except OSError as error:
                    self.skipTest('symlink creation unavailable: ' + str(error))
                bodies = {'nested/file.txt' if directory_link else 'file.txt': b'content'}
                try:
                    with self.assertRaises(ValueError): predecessor.check(view, bodies)
                finally:
                    link.unlink()

    def test_windows_reparse_entry_rejected_without_following(self):
        with tempfile.TemporaryDirectory() as directory:
            for mode in (stat.S_IFDIR, stat.S_IFREG):
                entry = MagicMock()
                entry.name = 'nested'
                entry.stat.return_value = types.SimpleNamespace(st_mode=mode, st_file_attributes=0x400)
                entries = MagicMock()
                entries.__enter__.return_value = iter([entry])
                with patch.object(predecessor.os, 'scandir', return_value=entries):
                    with self.assertRaisesRegex(ValueError, 'accepted view reparse point'):
                        predecessor.check(Path(directory), {'nested/file.txt': b'content'})
                entry.stat.assert_called_once_with(follow_symlinks=False)


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
