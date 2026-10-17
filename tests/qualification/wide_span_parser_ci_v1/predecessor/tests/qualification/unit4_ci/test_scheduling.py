"""Focused scheduling controls; no compiler or historical proof regeneration."""
import copy
import io
import json
import os
import shutil
import subprocess
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
sys.dont_write_bytecode = True
import common as q
import scheduling as s
import scheduling_derivation as d
import derive_scheduling as activation


class SchedulingControls(unittest.TestCase):
    def setUp(self):
        self.groups = s.roster(s.checked_groups())
        self.identity = dict(head='a'*40, tree='b'*40, event_sha='c'*40,
                             run='123', attempt='1', roster_sha256='d'*64)
        self.jobs = {'unit4-integration': {'result': 'success'}}
        self.receipts = [dict(schema='oxid-unit4-integration-shard-v1', identity=self.identity.copy(),
                              host=host, **{'shard': name}, ids=ids.copy(), started=ids.copy(),
                              passed=ids.copy(), status='pass')
                         for host in q.HOSTS for name, ids in self.groups.items()]

    def check(self, receipts=None, jobs=None):
        s.validate_receipts(self.groups, self.receipts if receipts is None else receipts,
                            self.identity, self.jobs if jobs is None else jobs)

    def test_actual_discovery_includes_imported_inherited_identity_and_whole_class(self):
        classes = s.roster(s.checked_classes())
        self.assertEqual((len(classes), sum(map(len, classes.values()))), (23, 174))
        imported = {k: v for k, v in classes.items() if k.startswith('test_source_identity.')}
        self.assertEqual((len(imported), sum(map(len, imported.values()))), (3, 37))
        ids = [i for tests in self.groups.values() for i in tests]
        self.assertEqual(len(ids), len(set(ids)))
        for name, cases in classes.items():
            self.assertTrue(all(i.rsplit('.', 1)[0] == name for i in cases))
        self.assertEqual(len(classes['test_integration.ComparisonSealControls']), 32)
        self.assertEqual({k: len(v) for k, v in self.groups.items()},
                         {'comparison-seal': 32, 'remaining-controls': 142})
        self.assertEqual(self.groups['comparison-seal'], classes['test_integration.ComparisonSealControls'])
        self.assertEqual(self.groups['remaining-controls'], [test for name, tests in classes.items()
                         if name != 'test_integration.ComparisonSealControls' for test in tests])
        self.check()

    def test_missing_duplicate_unknown_shard_and_test_are_rejected(self):
        mutations = [self.receipts[:-1], self.receipts + [self.receipts[0]]]
        for field, value in [('shard', 'unknown'), ('host', 'foreign'), ('status', 'skip'),
                             ('status', 'fail'), ('schema', 'unknown')]:
            changed = copy.deepcopy(self.receipts); changed[0][field] = value; mutations.append(changed)
        for field in ('ids', 'started', 'passed'):
            for transform in (lambda v: v[:-1], lambda v: v + v[:1], lambda v: ['unknown'] + v[1:],
                              lambda v: list(reversed(v))):
                changed = copy.deepcopy(self.receipts)
                changed[0][field] = transform(changed[0][field]); mutations.append(changed)
        for changed in mutations:
            with self.subTest(receipt=changed[0]), self.assertRaises(q.Reject): self.check(changed)

    def test_stale_head_tree_run_attempt_event_and_roster_fail(self):
        for field in self.identity:
            changed = copy.deepcopy(self.receipts); changed[0]['identity'][field] = 'stale'
            with self.subTest(field=field), self.assertRaises(q.Reject): self.check(changed)

    def test_upstream_failure_skip_cancel_missing_extra_rejected(self):
        for result in ('failure', 'skipped', 'cancelled', 'pending', None):
            with self.subTest(result=result), self.assertRaises(q.Reject):
                self.check(jobs={'unit4-integration': {'result': result}})
        for jobs in ({}, {**self.jobs, 'extra': {'result': 'success'}}):
            with self.assertRaises(q.Reject): self.check(jobs=jobs)

    def test_receipt_unknown_missing_fields_and_missing_identity_fail(self):
        for mutate in (lambda r: r.pop('passed'), lambda r: r.update(extra=True),
                       lambda r: r['identity'].pop('tree')):
            changed = copy.deepcopy(self.receipts); mutate(changed[0])
            with self.assertRaises(q.Reject): self.check(changed)

    def test_real_unittest_outcomes_and_class_fixture_are_serial(self):
        calls = []
        class Serial(unittest.TestCase):
            @classmethod
            def setUpClass(cls): calls.append('setup')
            def test_a(self): calls.append('a')
            def test_b(self): calls.append('b')
            @classmethod
            def tearDownClass(cls): calls.append('teardown')
        tests = list(s.flatten(unittest.TestLoader().loadTestsFromTestCase(Serial)))
        ids = [t.id() for t in tests]
        result = unittest.TextTestRunner(stream=io.StringIO(), resultclass=s.Result).run(unittest.TestSuite(tests))
        self.assertTrue(s.accepted(result, ids)); self.assertEqual(calls, ['setup','a','b','teardown'])
        class Outcomes(unittest.TestCase):
            def test_skip(self): self.skipTest('no')
            @unittest.expectedFailure
            def test_expected(self): self.fail('expected')
            def test_fail(self): self.fail('fail')
            def test_error(self): raise ValueError('error')
            def test_subtest(self):
                with self.subTest(x=1): self.fail('subtest')
        for method in unittest.defaultTestLoader.getTestCaseNames(Outcomes):
            case = Outcomes(method)
            result = unittest.TextTestRunner(stream=io.StringIO(), resultclass=s.Result).run(unittest.TestSuite([case]))
            self.assertFalse(s.accepted(result, [case.id()]))
        class FixtureFailure(Serial):
            @classmethod
            def setUpClass(cls): raise RuntimeError('fixture failed')
        result = unittest.TextTestRunner(stream=io.StringIO(), resultclass=s.Result).run(
            unittest.TestLoader().loadTestsFromTestCase(FixtureFailure))
        self.assertFalse(s.accepted(result, ids))

    def test_runner_preserves_failure_log_receipt_and_occupied_output(self):
        class Failing(unittest.TestCase):
            def test_fail(self):
                print('retained stdout detail')
                self.fail('retained assertion detail')
        case = Failing('test_fail')
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(output=Path(directory)/'shard', shard='fixture', host='Linux x86_64')
            with patch.object(s, 'checked_groups', return_value={'fixture': [case]}), patch.object(
                    s, 'context', return_value=self.identity), patch.object(q, 'measured_host',
                    return_value={'name': args.host}):
                with self.assertRaises(q.Reject): s.run(args)
            receipt = q.read(args.output/'receipt.json')
            self.assertEqual(receipt['status'], 'fail')
            self.assertEqual(receipt['started'], [case.id()])
            log = (args.output/'unittest.log').read_text()
            self.assertIn('retained stdout detail', log)
            self.assertIn('retained assertion detail', log)
            with self.assertRaises(q.Reject): s.run(args)
            self.assertEqual((args.output/'unittest.log').read_text(), log)

    def test_filesystem_join_requires_exact_artifact_roster_and_preserves_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root/'artifacts'; artifacts.mkdir()
            for receipt in self.receipts:
                path = artifacts/s.artifact_name(self.identity, receipt['host'], receipt['shard'])
                path.mkdir(); q.save(path/'receipt.json', receipt); (path/'unittest.log').write_text('complete log')
            args = SimpleNamespace(artifacts=artifacts, output=root/'join', job_results=json.dumps(self.jobs))
            with patch.object(s, 'context', return_value=self.identity): s.join(args)
            self.assertEqual(q.read(args.output/'integration.json')['status'], 'pass')
            (artifacts/'unknown').mkdir(); args.output = root/'failed-join'
            with patch.object(s, 'context', return_value=self.identity), self.assertRaises(q.Reject): s.join(args)
            result = q.read(args.output/'integration.json')
            self.assertEqual(result['status'], 'fail'); self.assertIn('artifact roster', result['error'])

    def test_output_symlink_ancestor_and_occupied_paths_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root/'target'; target.mkdir()
            link = root/'link'; link.symlink_to(target, target_is_directory=True)
            for operation in (s.run, s.join):
                for output in (link/'new', target, root/'dangling'):
                    if output.name == 'dangling' and not output.is_symlink():
                        output.symlink_to(root/'missing', target_is_directory=True)
                    with self.subTest(operation=operation.__name__, output=output), self.assertRaises(q.Reject):
                        operation(SimpleNamespace(output=output))
            self.assertEqual(list(target.iterdir()), [])

    def test_join_and_verify_reject_symlink_and_hardlink_receipts(self):
        for kind in ('symlink', 'hardlink', 'ancestor'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                artifacts = root/'artifacts'; artifacts.mkdir()
                for receipt in self.receipts:
                    path = artifacts/s.artifact_name(self.identity, receipt['host'], receipt['shard'])
                    path.mkdir(); q.save(path/'receipt.json', receipt); (path/'unittest.log').write_text('log')
                receipt_path = next(artifacts.iterdir())/'receipt.json'
                saved = root/'saved.json'; receipt_path.rename(saved)
                if kind == 'symlink': receipt_path.symlink_to(saved)
                elif kind == 'hardlink': os.link(saved, receipt_path)
                else:
                    saved.rename(receipt_path)
                    link = root/'artifact-link'; link.symlink_to(artifacts, target_is_directory=True)
                    artifacts = link
                args = SimpleNamespace(artifacts=artifacts, output=root/'joined', job_results=json.dumps(self.jobs))
                with patch.object(s, 'context', return_value=self.identity), self.assertRaises(q.Reject): s.join(args)
                self.assertEqual(q.read(args.output/'integration.json')['status'], 'fail')
                approval = root/'approval'; approval.mkdir()
                report = {'schema': 'oxid-unit4-integration-join-v1', 'status': 'pass',
                          'identity': self.identity, 'shards': 8, 'tests': 696}
                original = root/'original.json'; q.save(original, report)
                if kind == 'symlink': (approval/'integration.json').symlink_to(original)
                elif kind == 'hardlink': os.link(original, approval/'integration.json')
                else:
                    q.save(approval/'integration.json', report)
                    link = root/'approval-link'; link.symlink_to(approval, target_is_directory=True)
                    approval = link
                with patch.object(s, 'context', return_value=self.identity), self.assertRaises(q.Reject):
                    s.verify(SimpleNamespace(artifacts=approval))

    def test_runner_rejects_foreign_host_and_retains_admission_error(self):
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(output=Path(directory)/'shard', shard=next(iter(self.groups)), host='Windows x86_64')
            with patch.object(q, 'measured_host', return_value={'name': 'Linux x86_64'}), self.assertRaises(q.Reject):
                s.run(args)
            self.assertEqual(q.read(args.output/'receipt.json')['status'], 'fail')
            self.assertIn('wrong actual host', (args.output/'unittest.log').read_text())

    def test_formal_gate_rejects_prior_attempt_join_even_when_needs_succeeded(self):
        with tempfile.TemporaryDirectory() as directory:
            args = SimpleNamespace(artifacts=Path(directory))
            good = {'schema': 'oxid-unit4-integration-join-v1', 'status': 'pass',
                    'identity': self.identity, 'shards': 8, 'tests': 696}
            path = args.artifacts/'integration.json'
            with patch.object(s, 'context', return_value=self.identity):
                with self.assertRaises(q.Reject): s.verify(args)
                q.save(path, good); s.verify(args)
                for field in self.identity:
                    stale = copy.deepcopy(good); stale['identity'][field] = 'previous'
                    q.save(path, stale)
                    with self.subTest(field=field), self.assertRaises(q.Reject): s.verify(args)
                for field, value in [('status','fail'), ('shards',7), ('tests',695), ('schema','other')]:
                    stale = copy.deepcopy(good); stale[field] = value; q.save(path, stale)
                    with self.subTest(field=field), self.assertRaises(q.Reject): s.verify(args)

    def test_discovery_drift_is_rejected(self):
        with patch.object(s, 'discover', return_value={}):
            with self.assertRaises(q.Reject): s.checked_groups()

    def test_named_derivation_outputs_and_immutable_history(self):
        sys.path.insert(0, str(activation.REPO / 'tests/qualification/stdlib_trim_storage_v1'))
        from retained import Fixture
        fixture = Fixture(activation.REPO)
        suite = fixture.scheduling_suite(lambda: q.verify_package(activation.REPO, q.read(q.HERE/'inputs.json')))
        case = next(case for case in suite if case._testMethodName == 'test_named_derivation_outputs_and_immutable_history')
        case.debug()

    def test_original_consumer_generator_emits_named_workflow(self):
        sys.path.insert(0, str(activation.REPO / 'tests/qualification/stdlib_trim_storage_v1'))
        from retained import Fixture
        fixture = Fixture(activation.REPO)
        suite = fixture.scheduling_suite(lambda: q.verify_package(activation.REPO, q.read(q.HERE/'inputs.json')))
        case = next(case for case in suite if case._testMethodName == 'test_original_consumer_generator_emits_named_workflow')
        case.debug()

    def test_budget_entrypoint_preserves_gate_and_refuses_other_dispatches(self):
        import gate_scheduling as budget
        self.assertIs(budget._gate.prepare_parser, budget.prepare_parser)
        raw = (q.HERE/'gate.py').read_bytes()
        self.assertEqual(raw, activation.base('tests/qualification/unit4_ci/gate.py'))
        for name, timeout in [('other-stage',300), ('08-parser-prepare',299), ('08-parser-prepare',600)]:
            def dispatch(args, repo, output, driver):
                driver.stage(name, ['unchanged'], timeout)
            with patch.object(budget, '_prepare_parser', side_effect=dispatch), self.assertRaises(q.Reject):
                budget.prepare_parser(None, None, None, SimpleNamespace(stage=lambda *args: None))

    def test_workflow_refuses_any_unreviewed_predecessor(self):
        raw = activation.base('.github/workflows/ci.yml')
        for changed in (raw+b'\n', raw.replace(b'timeout-minutes: 100', b'timeout-minutes: 101'),
                        raw.replace(b'  unit4-linux:\n', b'')):
            with self.assertRaises(ValueError): d.workflow(changed)
        with self.assertRaises(ValueError): d.exact('anchor anchor', 'anchor', 'new')
        with self.assertRaises(ValueError): d.exact('missing', 'anchor', 'new')

    def test_transitive_dependency_and_capsule_steps_are_preserved(self):
        before = activation.base('.github/workflows/ci.yml').decode()
        after = d.workflow(before.encode()).decode()
        for job in ('unit4-linux', 'unit4-portable-hosts'):
            self.assertIn('  '+job+':\n    needs: unit4-integration-join\n', after)
        self.assertIn('needs: [unit4-linux, unit4-portable-hosts]', after)
        self.assertIn('  unit4-integration-join:\n    needs: unit4-integration\n    if: always()', after)
        self.assertIn('      fail-fast: false\n', after)
        self.assertIn('timeout-minutes: ${{ matrix.shard.minutes }}', after)
        self.assertIn('[{"name": "comparison-seal", "minutes": 150}, {"name": "remaining-controls", "minutes": 100}]', after)
        for job, end in [('unit4-linux', 'unit4-portable-hosts'), ('unit4-portable-hosts', 'unit4-qualification')]:
            original = before.split('  '+job+':\n')[1].split('  '+end+':\n')[0]
            new = after.split('  '+job+':\n')[1].split('  '+end+':\n')[0]
            original = original.replace('      - name: Verify Unit4 integration failure controls\n        run: python3 -B tests/qualification/unit4_ci/test_integration.py\n','')
            original = original.replace('      - name: Verify actual-host integration controls\n        run: python -B tests/qualification/unit4_ci/test_integration.py\n','')
            steps = d.DEPENDENCY_STEPS.replace('PYTHON', 'python3' if job == 'unit4-linux' else 'python')
            self.assertEqual(new.count(steps), 1)
            self.assertEqual(new.replace(steps, '').replace('unit4_ci/gate_scheduling.py host', 'unit4_ci/gate.py host'),
                             '    needs: unit4-integration-join\n'+original)

    def test_173_obligations_retained_and_only_three_relocated(self):
        package = activation.REPO/activation.PACKAGE
        old = q.read(package/'package-json-direct-consumer-dispositions-v1.json')
        new = q.read(package/activation.ci.NAME)
        self.assertEqual(len(new['entries']), 173)
        moved = []
        for a, b in zip(old['entries'], new['entries']):
            if a != b:
                moved.append(a['id'])
                for k in a.keys() - {'activation','active_command','active_command_sha256'}:
                    self.assertEqual(a[k], b[k])
        self.assertEqual(moved, ['ci.yml:L62','ci.yml:L988','ci.yml:L997','ci.yml:L1071'])
        for entries in (old['entries'][:-1], old['entries']+[old['entries'][0]], old['entries'][:-1]+[old['entries'][0]]):
            bad = copy.deepcopy(old); bad['entries'] = entries
            with self.assertRaises(ValueError): d.disposition(bad)
        args = [(activation.REPO/'tests/qualification/published_formatter_current/published-direct-consumer-dispositions-v1.json').read_bytes(),
                (package/'predecessor-v1/.github/workflows/ci.yml').read_bytes(),
                (activation.REPO/'.github/workflows/ci.yml').read_bytes(),
                (activation.REPO/'tests/fixtures/typed_project_source_binding/current-source.json').read_bytes()]
        activation.ci.validate(*args, (package/activation.ci.NAME).read_bytes())
        for index in range(4):
            changed = args.copy(); changed[index] += b'\n'
            with self.assertRaises(ValueError): activation.ci.validate(*changed, (package/activation.ci.NAME).read_bytes())
        with self.assertRaises(ValueError): activation.ci.validate(*args, (package/activation.ci.NAME).read_bytes()+b'\n')


class CheckoutSchedulingControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name)
        cls.repo = cls.root/'snapshot'
        subprocess.run(['git', 'clone', '--quiet', '--shared', str(activation.REPO), str(cls.repo)], check=True)
        # New files must already belong to the committed candidate snapshot.
        # Also copy working-tree edits to paths tracked by that snapshot.
        changed = q.git(activation.REPO, 'diff', '--name-only', 'HEAD').splitlines()
        for name in changed:
            shutil.copyfile(activation.REPO/name, cls.repo/name)
        q.git(cls.repo, 'add', '-u')
        q.git(cls.repo, '-c', 'user.name=Scheduling test', '-c', 'user.email=test@invalid',
              'commit', '--allow-empty', '-m', 'Isolated scheduling regression snapshot')
        cls.head = q.git(cls.repo, 'rev-parse', 'HEAD')
        # A shallow source's fetched predecessor may be unreachable from HEAD.
        # Publish it only in the local fixture remote, so upload-pack can serve it.
        q.git(cls.repo, 'fetch', '--no-tags', '--depth=1', str(activation.REPO), d.BASE)
        q.git(cls.repo, 'update-ref', 'refs/heads/scheduling-predecessor', d.BASE)

    def command(self, repo, code, *arguments):
        return subprocess.run([sys.executable, '-B', '-c', code, *arguments], cwd=repo,
                              env={**os.environ, 'GITHUB_RUN_ID': '123', 'GITHUB_RUN_ATTEMPT': '1'},
                              capture_output=True, text=True, timeout=180)

    def test_real_depth_one_checkout_fetches_only_fixed_verified_predecessor(self):
        repo = self.root/'shallow'
        subprocess.run(['git', 'clone', '--quiet', '--depth=1', self.repo.as_uri(), str(repo)], check=True)
        self.assertEqual(q.git(repo, 'rev-parse', '--is-shallow-repository'), 'true')
        absent = subprocess.run(['git', 'cat-file', '-e', d.BASE], cwd=repo, capture_output=True)
        self.assertNotEqual(absent.returncode, 0)
        fetched = subprocess.run([sys.executable, '-B', 'tests/qualification/unit4_ci/derive_scheduling.py',
                                  '--fetch-base'], cwd=repo, capture_output=True, text=True, timeout=60)
        self.assertEqual(fetched.returncode, 0, fetched.stderr)
        self.assertEqual(q.git(repo, 'rev-parse', d.BASE+'^{tree}'), d.BASE_TREE)
        self.assertEqual(q.git(repo, 'rev-parse', '--is-shallow-repository'), 'true')
        self.assertEqual(q.git(repo, 'rev-list', '--count', '--all'), '1')
        checked = self.command(repo, "import sys; sys.path.insert(0, 'tests/qualification/unit4_ci'); "
            "from pathlib import Path; sys.path.insert(0, 'tests/qualification/stdlib_trim_storage_v1'); "
            "from retained import Fixture; Fixture(Path.cwd()).assert_scheduling_outputs(); "
            "import types; p=Path('tests/qualification/stdlib_text_case_v1/activate.py').absolute(); m=types.ModuleType('text_checkout_activation'); m.__file__=str(p); exec(compile(p.read_bytes(),str(p),'exec'),m.__dict__); m.base.Baseline(Path.cwd()).assert_outputs(); assert all((m.REPO/n).read_bytes()==body for n,body in m.generate().items())")
        self.assertEqual(checked.returncode, 0, checked.stderr)
        # A wrong expected tree must fail even if fetch itself succeeds.
        refused = self.command(repo, "import sys; sys.path.insert(0, 'tests/qualification/unit4_ci'); "
            "import derive_scheduling as a; a.d.BASE_TREE='0'*40; a.ensure_base()")
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn('unreviewed scheduling predecessor tree', refused.stderr)
        workflow = (repo/'.github/workflows/ci.yml').read_text()
        self.assertEqual(workflow.count('derive_scheduling.py --fetch-base'), 1)
        self.assertLess(workflow.index('derive_scheduling.py --fetch-base'), workflow.index('test_scheduling.py'))
        self.assertNotIn('fetch-depth: 0', workflow)

    def test_post_teardown_real_admission_rejects_source_roster_and_head_changes(self):
        code = r"""
import sys, unittest
from pathlib import Path
from types import SimpleNamespace
sys.path.insert(0, 'tests/qualification/unit4_ci')
import scheduling as s
import common as q
repo = Path.cwd()
mode, head, output = sys.argv[1:]
class Mutation(unittest.TestCase):
    def test_pass(self): pass
    @classmethod
    def tearDownClass(cls):
        if mode in ('documentation', 'source', 'roster'):
            path = (repo/'src/main.rs' if mode == 'source' else
                    q.HERE/('SCHEDULING.md' if mode == 'documentation' else 'integration-roster-v1.json'))
            path.write_bytes(path.read_bytes()+b'\n')
        elif mode == 'head':
            q.git(repo, '-c', 'user.name=Scheduling test', '-c', 'user.email=test@invalid',
                  'commit', '--allow-empty', '-m', 'Changed head during teardown')
case = Mutation('test_pass')
s.checked_groups = lambda: {'fixture': [case]}
s.run(SimpleNamespace(repo=repo, head=head, event_sha=head, host=q.measured_host()['name'],
                      shard='fixture', output=Path(output)))
"""
        for mode in ('clean', 'documentation', 'source', 'roster', 'head'):
            with self.subTest(mode=mode):
                repo = self.root/('admission-'+mode)
                subprocess.run(['git', 'clone', '--quiet', '--shared', str(self.repo), str(repo)], check=True)
                output = self.root/('result-'+mode)
                result = self.command(repo, code, mode, self.head, str(output))
                receipt = q.read(output/'receipt.json')
                self.assertEqual(receipt['started'], receipt['passed'])
                self.assertEqual(len(receipt['passed']), 1)
                self.assertEqual(receipt['status'], 'pass' if mode == 'clean' else 'fail')
                self.assertEqual(result.returncode == 0, mode == 'clean', result.stderr)
                if mode != 'clean':
                    self.assertIn('Traceback', (output/'unittest.log').read_text())


if __name__ == '__main__': unittest.main(verbosity=2)
