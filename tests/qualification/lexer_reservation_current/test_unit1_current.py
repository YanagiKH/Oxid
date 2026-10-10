#!/usr/bin/env python3
"""Source-only current Unit1 authentication, inverse, and fake-libtest controls.

No Cargo/rustc invocation, real Rust executable, fixture corpus generation, or
compiler qualification occurs. Run normally and with -O; production controllers
continue to refuse optimized Python independently of these explicit guards.
"""
import ast
import copy
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
import unit1_current_support as support
import unit1_derivation as recipe

ROOT = Path(__file__).resolve().parent
PACKAGE = support.original_package()


class AuthenticationControls(unittest.TestCase):
    def test_original_and_current_checkpoints_have_distinct_roles(self):
        old = {'compiler_checkpoint': support.ORIGINAL_CHECKPOINT}
        new = {'compiler_checkpoint': support.CHECKPOINT}
        self.assertNotEqual(support.ORIGINAL_CHECKPOINT, support.CHECKPOINT)
        support.check_compiler_checkpoints(old, new)
        for bad_old, bad_new in ((new, new), (old, old), (new, old),
                                 (dict(compiler_checkpoint='0' * 40), new),
                                 (old, dict(compiler_checkpoint='0' * 40))):
            with self.subTest(original=bad_old, current=bad_new), self.assertRaises(ValueError):
                support.check_compiler_checkpoints(bad_old, bad_new)

    def test_complete_original_and_current_inverse(self):
        verified = support.authenticate()
        self.assertEqual(len(verified['originals']), 16)
        self.assertEqual(len(verified['correspondence']['cases']), 69)
        self.assertEqual(sum(row['adaptation'] != 'unchanged' for row in verified['correspondence']['cases']), 1)
        self.assertEqual(verified['correspondence']['current_controls']['count'], 1)
        self.assertFalse(verified['correspondence']['execution_qualified'])
        for old, new, seams in (
            ('run.py', 'unit1_run_current.py', recipe.RUNNER_SEAMS),
            ('run_reviewer.py', 'unit1_run_reviewer_current.py', recipe.REVIEWER_SEAMS),
        ):
            original = verified['originals'][old]
            derived = verified['derived'][new]
            self.assertEqual(recipe.inverse(derived, seams), original)
            self.assertEqual(recipe.transform(original, seams), derived)
            ast.parse(derived)
            with self.assertRaises(ValueError):
                recipe.transform(derived, seams)
            with self.assertRaises(ValueError):
                recipe.inverse(original, seams)

    def test_every_runner_seam_missing_duplicate_or_altered_rejected(self):
        for old, new, seams in (
            ('run.py', 'unit1_run_current.py', recipe.RUNNER_SEAMS),
            ('run_reviewer.py', 'unit1_run_reviewer_current.py', recipe.REVIEWER_SEAMS),
        ):
            raw = (PACKAGE / old).read_bytes()
            derived = (ROOT / new).read_bytes()
            for before, after in seams:
                with self.subTest(runner=new, seam=before[:50]):
                    for changed in (raw.replace(before, b'', 1), raw + before):
                        with self.assertRaises(ValueError):
                            recipe.transform(changed, seams)
                    for changed in (derived.replace(after, b'', 1), derived + after):
                        with self.assertRaises(ValueError):
                            recipe.inverse(changed, seams)

    def test_original_mutation_extra_missing_modes_and_symlinks_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            target = Path(name) / 'package'
            shutil.copytree(PACKAGE, target)
            for original in support.ORIGINAL_NAMES:
                path = target / original
                raw = path.read_bytes()
                path.write_bytes(raw + b'\n')
                with self.subTest(name=original), self.assertRaises(ValueError):
                    support.authenticate(package=target)
                path.write_bytes(raw)
            extra = target / 'extra'
            extra.mkdir()
            with self.assertRaises(ValueError):
                support.authenticate(package=target)
            extra.rmdir()
            path = target / 'run.py'
            raw = path.read_bytes()
            path.unlink()
            with self.assertRaises(ValueError):
                support.authenticate(package=target)
            path.write_bytes(raw)
            path.chmod(0o755)
            with self.assertRaises(ValueError):
                support.authenticate(package=target)
            path.chmod(0o644)
            path.unlink()
            path.symlink_to(PACKAGE / 'run.py')
            with self.assertRaises(ValueError):
                support.authenticate(package=target)

    def test_every_current_complete_body_and_map_mutation_rejected(self):
        with tempfile.TemporaryDirectory() as name:
            target = Path(name)
            names = [row['path'] for row in json.loads((ROOT / 'unit1_current_bindings.json').read_bytes())['files']]
            names += ['unit1_original_package.json', 'unit1_current_bindings.json']
            for body_name in names:
                shutil.copyfile(ROOT / body_name, target / body_name)
            for body_name in names:
                path = target / body_name
                raw = path.read_bytes()
                path.write_bytes(raw + b'\n')
                with self.subTest(name=body_name), self.assertRaises(ValueError):
                    support.authenticate(current_root=target)
                path.write_bytes(raw)

    def test_staging_changes_only_one_original_body(self):
        with tempfile.TemporaryDirectory() as name:
            q = Path(name)
            for original in support.ORIGINAL_NAMES:
                shutil.copyfile(PACKAGE / original, q / original)
            (q / 'input-source-manifest.json').write_text('{}\n')
            support.stage(q)
            changed = [original for original in support.ORIGINAL_NAMES
                       if (PACKAGE / original).read_bytes() != (q / original).read_bytes()]
            self.assertEqual(changed, ['reviewer_additional.rs'])
            self.assertEqual((q / 'reviewer_additional.rs').read_bytes(),
                             (ROOT / 'unit1_reviewer_additional.current.rs').read_bytes())
            support.check_stage(q)
            with self.assertRaises(ValueError):
                support.stage(q)
            for body_name in support.ORIGINAL_NAMES + ('unit1_lexer_controls.rs', 'unit1_run_reviewer_current.py', 'unit1_correspondence.json'):
                path = q / body_name
                raw = path.read_bytes()
                path.write_bytes(raw + b'\n')
                with self.subTest(name=body_name), self.assertRaises(ValueError):
                    support.check_stage(q)
                path.write_bytes(raw)

    def test_control_is_single_separate_include(self):
        with tempfile.TemporaryDirectory() as name:
            test = Path(name) / 'reviewer_cases.rs'
            original = b'include!("reviewer_additional.rs");\ninclude!("reviewer_v2.rs");\n'
            test.write_bytes(original)
            support.include_control(test)
            self.assertTrue(test.read_bytes().startswith(original))
            self.assertEqual(test.read_bytes().count(b'include!("unit1_lexer_controls.rs");'), 1)
            with self.assertRaises(ValueError):
                support.include_control(test)

    def test_admission_and_roster_guards_precede_build(self):
        runner = (ROOT / 'unit1_run_current.py').read_text()
        self.assertLess(runner.index('current.admit(repo)'), runner.index('shutil.copytree(repo,source'))
        self.assertLess(runner.index('current.stage(q)'), runner.index("tool=checked([args.cargo"))
        self.assertLess(runner.index('current.profile_guard('), runner.index("build=run(argv"))
        self.assertEqual(runner.count('current.profile_guard('), 2)
        self.assertIn("['debug','release'] if args.profile=='both'", runner)
        for preserved in (
            "assert actual==json.loads((PACKAGE/'expected-fixtures.json').read_text())",
            "assert (q/'resource-fixture-manifest.json').read_bytes()==(PACKAGE/'expected-resources.json').read_bytes()",
            "assert digest(binary)==binary_sha",
            "['results'];assert before==after",
            "assert digest(repo/rel)==expected",
            "assert digest(source/rel)==expected",
        ):
            self.assertIn(preserved, runner)

    def test_active_admission_rejects_historical_shape_before_any_operation(self):
        fake = mock.Mock()
        fake.preflight.return_value = {'inputs': {}}
        with mock.patch.object(support, 'load_module', return_value=fake), \
             mock.patch.object(support, 'regular', side_effect=[b'dispatcher', b'package']), \
             mock.patch.object(support, 'sha', side_effect=[support.DISPATCHER_SHA, support.SOURCE_PACKAGE_SHA]), \
             self.assertRaises(ValueError):
            support.admit(PACKAGE.parents[2])
        fake.preflight.assert_called_once()

    def test_changed_trusted_dispatcher_rejected_before_loading(self):
        with mock.patch.object(support, 'regular', return_value=b'wrong dispatcher'), \
             mock.patch.object(support, 'load_module') as load, self.assertRaises(ValueError):
            support.admit(Path('.'))
        load.assert_not_called()

    def test_changed_trusted_package_rejected_before_loading(self):
        with mock.patch.object(support, 'regular', side_effect=[b'dispatcher', b'package']), \
             mock.patch.object(support, 'sha', side_effect=[support.DISPATCHER_SHA, 'wrong']), \
             mock.patch.object(support, 'load_module') as load, self.assertRaises(ValueError):
            support.admit(Path('.'))
        load.assert_not_called()

    def test_changed_source_admission_and_compiled_body_rejected(self):
        with mock.patch.object(support, 'admit', return_value={'changed': True}), self.assertRaises(ValueError):
            support.check_admission(Path('.'), {'changed': False})
        with tempfile.TemporaryDirectory() as name:
            source = Path(name)
            (source / 'x').write_bytes(b'bad')
            with mock.patch.object(support, 'check_admission'), mock.patch.object(support, 'check_stage'), self.assertRaises(ValueError):
                support.profile_guard(source, {}, source, {'x': support.sha(b'good')}, source)


FAKE_LIBTEST = r'''#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
root=Path(__file__).parent
names=json.loads((root/'fake-roster.json').read_text())
mode=os.environ['OXID_CURRENT_UNIT1_FAKE_MODE']
if '--list' in sys.argv:
    if mode=='list-error':raise SystemExit(9)
    if mode=='zero-list':names=[]
    elif mode=='missing-control':names=names[:-1]
    elif mode=='duplicate-list':names[-1]=names[0]
    elif mode=='wrong-list':names[-1]='frontend::project::reviewer_unit1::replacement'
    for name in names:print(name+': test')
    print(f'\n{len(names)} tests, 0 benchmarks')
    raise SystemExit(0)
test=sys.argv[1]
first=test==names[0]
control=test==names[-1]
if mode=='roster-tamper' and first:
    path=root/'expected-test-roster.json';path.write_bytes(path.read_bytes()+b'\n')
if mode=='binary-tamper' and first:
    path=Path(__file__);path.write_bytes(path.read_bytes()+b'\n')
if (mode=='test-error' and first) or (mode=='control-error' and control):raise SystemExit(11)
if mode=='zero-run' and first:
    print('running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 70 filtered out; finished in 0.00s')
    raise SystemExit(0)
if mode=='ignored' and first:
    print(f'running 1 test\ntest {test} ... ignored\n\ntest result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 69 filtered out; finished in 0.00s')
    raise SystemExit(0)
if mode=='wrong-executed' and first:test='frontend::project::reviewer_unit1::wrong_executed'
print(f'running 1 test\ntest {test} ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 69 filtered out; finished in 0.00s')
'''


@unittest.skipUnless(os.name == 'posix', 'Fake libtest uses a POSIX Python script')
class ProtocolControls(unittest.TestCase):
    def run_case(self, mode):
        with tempfile.TemporaryDirectory() as name:
            q = Path(name)
            shutil.copyfile(ROOT / 'unit1_run_reviewer_current.py', q / 'unit1_run_reviewer_current.py')
            shutil.copyfile(PACKAGE / 'expected-test-roster.json', q / 'expected-test-roster.json')
            original = json.loads((q / 'expected-test-roster.json').read_bytes())['tests']
            (q / 'fake-roster.json').write_text(json.dumps(original + [support.CONTROL]))
            binary = q / 'fake-libtest'
            binary.write_text(FAKE_LIBTEST)
            binary.chmod(0o755)
            env = dict(os.environ)
            env['OXID_CURRENT_UNIT1_FAKE_MODE'] = mode
            env['PYTHONDONTWRITEBYTECODE'] = '1'
            env.pop('PYTHONOPTIMIZE', None)
            process = subprocess.run([sys.executable, '-B', str(q / 'unit1_run_reviewer_current.py'),
                                      '--binary', str(binary), '--tag', mode],
                                     capture_output=True, text=True, env=env, timeout=60)
            result = json.loads((q / (mode + '-results.json')).read_bytes())
            return process, result

    def test_success_original69_plus_new1(self):
        process, result = self.run_case('success')
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual((result['passed'], result['total']), (69, 69))
        self.assertEqual((result['current_controls']['passed'], result['current_controls']['total']), (1, 1))
        self.assertEqual((result['all_passed'], result['all_total']), (70, 70))
        support.check_profile_results(result)
        for key in ('total', 'passed', 'expected'):
            bad = copy.deepcopy(result)
            bad['current_controls'][key] = 0
            with self.assertRaises(ValueError):
                support.check_profile_results(bad)
        bad = copy.deepcopy(result)
        bad['tests'][-1]['test'] = support.CONTROL
        with self.assertRaises(ValueError):
            support.check_profile_results(bad)

    def test_zero_missing_duplicate_substitute_and_failed_listing_rejected(self):
        for mode in ('zero-list', 'missing-control', 'duplicate-list', 'wrong-list', 'list-error'):
            with self.subTest(mode=mode):
                process, result = self.run_case(mode)
                self.assertNotEqual(process.returncode, 0)
                self.assertEqual(result['status'], 'failed')
                self.assertEqual(result['total'], 0)
                self.assertEqual(result['current_controls']['total'], 0)

    def test_original_execution_failures_rejected(self):
        for mode in ('zero-run', 'ignored', 'wrong-executed', 'test-error'):
            with self.subTest(mode=mode):
                process, result = self.run_case(mode)
                self.assertNotEqual(process.returncode, 0)
                self.assertEqual(result['status'], 'failed')
                self.assertEqual((result['passed'], result['total']), (68, 69))
                self.assertEqual(result['current_controls']['passed'], 1)

    def test_control_failure_cannot_be_hidden_by_original69(self):
        process, result = self.run_case('control-error')
        self.assertNotEqual(process.returncode, 0)
        self.assertEqual((result['passed'], result['total']), (69, 69))
        self.assertEqual((result['current_controls']['passed'], result['current_controls']['total']), (0, 1))
        self.assertEqual(result['status'], 'failed')
        with self.assertRaises(ValueError):
            support.check_profile_results(result)

    def test_roster_or_executable_changed_during_execution_rejected(self):
        for mode in ('roster-tamper', 'binary-tamper'):
            with self.subTest(mode=mode):
                process, result = self.run_case(mode)
                self.assertNotEqual(process.returncode, 0)
                self.assertEqual(result['status'], 'failed')
                self.assertEqual(result['all_passed'], 70)
                self.assertIn('changed during execution', result['error'])

    def test_production_controllers_reject_optimized_python(self):
        for name in ('unit1_run_current.py', 'unit1_run_reviewer_current.py'):
            process = subprocess.run([sys.executable, '-O', '-B', str(ROOT / name), '--help'],
                                     capture_output=True, text=True, timeout=15)
            self.assertNotEqual(process.returncode, 0)
            self.assertIn('refuses optimized Python', process.stderr)


if __name__ == '__main__':
    unittest.main()
