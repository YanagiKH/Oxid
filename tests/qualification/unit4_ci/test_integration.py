#!/usr/bin/env python3
"""Bounded admission/transport controls, never synthetic qualification claims."""
import sys
sys.dont_write_bytecode = True
import copy
import io
import json
from pathlib import Path, PurePosixPath, PureWindowsPath
import shutil
import tarfile
import tempfile
import subprocess
import unittest
from types import SimpleNamespace
from unittest.mock import patch
import common as q
import gate
import join
from evidence import Capsule, ReadCapsule, verify_parser_seal, parser_full_only, full_archive, stage_compact_upload

REPO = Path(__file__).resolve().parents[3]


class WorkflowEnvironmentControls(unittest.TestCase):
    def test_yaml_global_environment_has_unique_keys(self):
        workflow = (REPO / '.github/workflows/ci.yml').read_text()
        def check(text):
            block = text.split('\nenv:\n', 1)[1].split('\njobs:\n', 1)[0]
            rows = [line.strip().split(':', 1) for line in block.splitlines() if line.strip()]
            self.assertTrue(all(len(row) == 2 for row in rows))
            keys = [row[0] for row in rows]
            self.assertEqual(len(keys), len(set(keys)), 'duplicate YAML global env key')
            self.assertEqual(dict(rows)['PYTHONDONTWRITEBYTECODE'].strip(), '"1"')
        check(workflow)
        marker = '  PYTHONDONTWRITEBYTECODE: "1"\n'
        for duplicate in (marker, '  PYTHONDONTWRITEBYTECODE: "0"\n'):
            with self.subTest(duplicate=duplicate), self.assertRaisesRegex(AssertionError, 'duplicate YAML'):
                check(workflow.replace(marker, marker + duplicate))


class GitDiffBatchControls(unittest.TestCase):
    def test_windows_length_unicode_quoting_order_and_duplicates(self):
        # Astral characters count as two UTF-16 units; quoting and backslashes
        # also consume CreateProcess space. No path may disappear at a boundary.
        names = [('nested space/' + '\U0001f600' * 30 + '/file\\"' + str(i)) for i in range(900)]
        names += names[:3]
        with patch.object(q, 'git') as git:
            q.git_diff_paths(Path('C:/checkout with spaces'), 'a' * 40, names)
        self.assertGreater(len(git.call_args_list), 1)
        actual = []
        for call in git.call_args_list:
            repo, *args = call.args
            self.assertEqual(args[:4], ['diff', '--exit-code', 'a' * 40, '--'])
            actual.extend(args[4:])
            command = ['git', '-C', str(repo), *args]
            self.assertLessEqual(len(subprocess.list2cmdline(command).encode('utf-16-le')) // 2 + 1, 16000)
        self.assertEqual(actual, names)

    def test_empty_paths_do_not_diff_the_whole_checkout(self):
        with patch.object(q, 'git') as git:
            q.git_diff_paths(Path('repo'), 'a' * 40, [])
        git.assert_not_called()

    def test_single_oversized_path_fails_closed(self):
        with patch.object(q, 'git') as git, self.assertRaises(q.Reject):
            q.git_diff_paths(Path('repo'), 'a' * 40, ['x' * 16000])
        git.assert_not_called()

    def test_later_batch_failure_propagates_and_stops(self):
        with patch.object(q, 'git', side_effect=['', q.Reject('changed later input')]) as git:
            with self.assertRaisesRegex(q.Reject, 'changed later input'):
                q.git_diff_paths(Path('repo'), 'a' * 40, ['x' * 8000] * 4)
        self.assertEqual(git.call_count, 2)

    def test_real_git_preserves_selected_dirty_and_staged_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            q.git(repo, 'init', '--quiet')
            names = ['first', 'nested space/last']
            (repo / 'nested space').mkdir()
            for name in names + ['unrelated']:
                (repo / name).write_text('original\n')
            q.git(repo, 'add', '.')
            q.git(repo, '-c', 'user.name=Unit4 fixture', '-c', 'user.email=unit4@localhost', 'commit', '-qm', 'diff fixture')
            head = q.git(repo, 'rev-parse', 'HEAD')
            (repo / 'unrelated').write_text('outside selected scope\n')
            q.git_diff_paths(repo, head, names)
            for name in names:
                for staged in (False, True):
                    with self.subTest(name=name, staged=staged):
                        (repo / name).write_text('changed\n')
                        if staged:
                            q.git(repo, 'add', '--', name)
                        with self.assertRaises(q.Reject):
                            q.git_diff_paths(repo, head, names)
                        q.git(repo, 'checkout', head, '--', name)


class FrozenRosterControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temp.name).resolve()
        transport = q.module('_unit4_test_transport', REPO / q.TRANSPORT / 'transport.py')
        manifest = transport.materialize(REPO / q.TRANSPORT, cls.root / 'contracts')
        mapping = {row['logical_path']: str(cls.root / 'contracts' / row['archive_path']) for row in manifest['members']}
        public_root = Path(mapping[manifest['active_contracts']['public']['logical_path']]).parent
        cls.c, cls.rt, cls.collector, _, _ = q.public_modules(REPO)
        cls.contracts = cls.c.Contracts(public_root, mapping, REPO / q.AMENDMENT)
        cls.host_keys = {host: [row['key'] for section in q.SECTIONS for row in cls.contracts.roster(section, host) if row['scope'] == 'execute'] for host in q.HOSTS}

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def test_complete_frozen_scope_derives_counts(self):
        result = join.final_coverage(self.contracts, self.host_keys)
        self.assertEqual(len(result), 10286)
        self.assertEqual([len(self.host_keys[host]) for host in q.HOSTS], [8690, 532, 532, 532])
        counts = {section: sum(row['scope'] == 'execute' for host in q.HOSTS for row in self.contracts.roster(section, host)) for section in q.SECTIONS}
        self.assertEqual(counts, {'public': 678, 'original': 992, 'predecessors': 7814, 'lifecycle': 780, 'guards': 22})

    def test_missing_host(self):
        rows = copy.deepcopy(self.host_keys)
        rows.pop('macOS arm64')
        with self.assertRaises(q.Reject): join.final_coverage(self.contracts, rows)

    def test_wrong_host_substitution(self):
        rows = copy.deepcopy(self.host_keys)
        rows['Windows x86_64'] = rows['macOS x86_64']
        with self.assertRaises(q.Reject): join.final_coverage(self.contracts, rows)

    def test_duplicate_tuple(self):
        rows = copy.deepcopy(self.host_keys)
        rows['Linux x86_64'].append(rows['Linux x86_64'][0])
        with self.assertRaises(q.Reject): join.final_coverage(self.contracts, rows)

    def test_missing_and_zero_domain_and_profile(self):
        for name, predicate in (
            ('missing', lambda parsed: True),
            ('domain', lambda parsed: 'lifecycle' in parsed[0]),
            ('profile', lambda parsed: parsed[4] == 'release'),
            ('zero', lambda parsed: True)):
            with self.subTest(name=name):
                rows = copy.deepcopy(self.host_keys)
                if name == 'missing': rows['Linux x86_64'].pop()
                else: rows['Linux x86_64'] = [key for key in rows['Linux x86_64'] if not predicate(q.loads(key))]
                with self.assertRaises(q.Reject): join.final_coverage(self.contracts, rows)

    def test_exclusions_cannot_be_fabricated_execution(self):
        roster = self.contracts.roster('guards', 'Windows x86_64')
        values = [self.collector.skipped(row) for row in roster]
        self.c.check_inventory(roster, values, allow_capability_gap=False)
        values[0].update(executed=True, status='pass')
        with self.assertRaises(self.c.Reject): self.c.check_inventory(roster, values)

    def test_stale_contract_receipt(self):
        roster = self.contracts.roster('guards', 'Windows x86_64')
        values = [self.collector.skipped(row) for row in roster]
        values[0]['effective_contract_identity'] = 'stale'
        with self.assertRaises(self.c.Reject): self.c.check_inventory(roster, values)

    def test_exact_exit_propagation_preserves_streams(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            script = root / 'fail.py'
            script.write_text("import sys\nprint('retained stdout')\nprint('retained stderr', file=sys.stderr)\nraise SystemExit(7)\n")
            provenance = {'checkout_head': 'a' * 40, 'event_sha': 'b' * 40}
            driver = gate.Driver(REPO, root, provenance, self.rt)
            with patch.object(q, 'admit', return_value=provenance):
                with self.assertRaises(q.Reject): driver.stage('failure-control', [script], 10)
            receipt = q.read(root / 'commands/failure-control/receipt.json')
            self.assertEqual(receipt['status'], 7)
            self.assertEqual((root / 'commands/failure-control/stdout').read_text(), 'retained stdout\n')
            self.assertEqual((root / 'commands/failure-control/stderr').read_text(), 'retained stderr\n')


class ProvenanceOrderingControls(unittest.TestCase):
    def test_exact_posix_strings_order_both_path_flavors_identically(self):
        names = ['common.py', 'DESIGN.md', 'evidence.py', 'gate.py', 'inputs.json', 'join.py', 'README.md', 'test_integration.py']
        roots = [PurePosixPath('/repo'), PureWindowsPath('C:/repo')]
        paths = [[root / 'tests/qualification/unit4_ci' / name for name in names] for root in roots]
        self.assertNotEqual([p.name for p in sorted(paths[0])], [p.name for p in sorted(paths[1])])
        for root, members in zip(roots, paths):
            self.assertEqual([p.name for p in q.relative_path_order(members, root)], sorted(names))
            self.assertEqual([p.relative_to(root).as_posix() for p in q.relative_path_order(members, root)],
                             ['tests/qualification/unit4_ci/' + name for name in sorted(names)])

    def test_producer_order_preserves_exact_case_and_duplicate_members(self):
        for root in (PurePosixPath('/repo'), PureWindowsPath('C:/repo')):
            members = [root / name for name in ('a.py', 'A.py', 'a.py', 'B.py')]
            self.assertEqual([p.name for p in q.relative_path_order(members, root)], ['A.py', 'B.py', 'a.py', 'a.py'])

    def test_strict_reader_still_rejects_order_and_member_identity_changes(self):
        base = {'checkout_head': 'a' * 40, 'checkout_tree': 'b' * 40, 'event_sha': 'c' * 40, 'source_only_tree': 'd' * 40,
                'integration': [{'path': name, 'bytes': i + 1, 'sha256': str(i) * 64} for i, name in enumerate(('DESIGN.md', 'common.py'))],
                **{name: {'path': 'host-relative', 'bytes': 1, 'sha256': 'e' * 64} for name in ('source_manifest', 'inputs', 'workflow')}}
        expected = join.provenance_key(base)
        for mutation in ('order', 'hash', 'missing', 'duplicate'):
            changed = copy.deepcopy(base)
            if mutation == 'order': changed['integration'].reverse()
            elif mutation == 'hash': changed['integration'][0]['sha256'] = 'f' * 64
            elif mutation == 'missing': changed['integration'].pop()
            else: changed['integration'].append(changed['integration'][0])
            with self.subTest(mutation=mutation): self.assertNotEqual(join.provenance_key(changed), expected)


class HostPreparationControls(unittest.TestCase):
    def test_actual_host_preparation_uses_current_overlay_and_stage_defaults(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            runtime = q.public_modules(REPO)[1]
            provenance = {'checkout_head': 'a' * 40, 'event_sha': 'b' * 40}
            driver = gate.Driver(REPO, root, provenance, runtime)
            # Admission has separate exact-commit controls; exercise the actual
            # preparation commands and Driver.stage defaults without a compiler.
            with patch.object(q, 'admit', return_value=provenance), \
                 patch.object(runtime, 'process', wraps=runtime.process) as process:
                gate.prepare_host(SimpleNamespace(), REPO, root, provenance, q.measured_host(), driver)
            receipts = [q.read(record['path']) for record in driver.state['stages']]
            self.assertEqual([row['name'] for row in receipts],
                             ['01-contract-verification', '02-contract-materialization', '03-lifecycle-preparation'])
            self.assertEqual(process.call_count, 3)
            for row in receipts:
                self.assertEqual(row['status'], 0)
                self.assertEqual(row['accepted_exit_codes'], [0])
                self.assertFalse(row['timed_out'])
                self.assertFalse(row['stream_limit_exceeded'])
                self.assertEqual(row['argv'][:2], [sys.executable, '-B'])
            lifecycle = receipts[-1]['argv']
            self.assertEqual(lifecycle[2:4], [str(REPO / q.PUBLIC / 'build.py'), 'prepare-observer'])
            patch_path = Path(lifecycle[lifecycle.index('--observer-patch') + 1])
            self.assertEqual(patch_path, REPO / q.OBSERVER_PATCH)
            patch_identity = q.identity(patch_path)
            self.assertEqual(lifecycle[lifecycle.index('--observer-patch-sha256') + 1], patch_identity['sha256'])
            prepared = q.read(root / 'observer-source/prepared.json')
            self.assertEqual(prepared['compiler_invocations'], 0)
            observer = q.read(root / 'observer-source/observer-source.json')
            self.assertEqual(observer['observer_patch'], patch_identity)
            self.assertEqual(observer['base_source_manifest_sha256'], q.CURRENT_SHA)
            self.assertEqual(len(observer['files']), 364)
            runtime.source_manifest(root / 'observer-source/observer-source.json', root / 'observer-source/source')
            self.assertFalse((root / 'ordinary-build').exists())
            self.assertFalse((root / 'observer-build').exists())


class ObserverPreparationControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temp.name).resolve()
        cls.c, cls.runtime, _, _, _ = q.public_modules(REPO)
        cls.builder = q.module('_unit4_observer_build_controls', REPO / q.PUBLIC / 'build.py')
        cls.config = cls.root / 'controlled-gitconfig'
        cls.config.write_bytes(b'[core]\n autocrlf = true\n eol = crlf\n')
        cls.output = cls.root / 'prepared'
        command = [sys.executable, '-B', str(REPO / q.PUBLIC / 'build.py'), 'prepare-observer',
                   '--source-root', str(REPO), '--manifest', str(REPO / q.SOURCE / 'current-source.json'),
                   '--observer-patch', str(REPO / q.OBSERVER_PATCH), '--observer-patch-sha256', cls.builder.LIFECYCLE_PATCH_SHA,
                   '--out', str(cls.output)]
        env = {**gate.clean_environment(), 'GIT_CONFIG_GLOBAL': str(cls.config), 'GIT_CONFIG_NOSYSTEM': '1'}
        cls.preparation = subprocess.run(command, cwd=cls.root, env=env, capture_output=True, timeout=120)
        if cls.preparation.returncode:
            raise AssertionError(cls.preparation.stderr.decode(errors='replace'))
        cls.manifest = q.read(cls.output / 'observer-source.json')

    @classmethod
    def tearDownClass(cls):
        cls.temp.cleanup()

    def current_build_capsule(self, output, observer_mutation=None):
        # Actual admitted source maps, synthetic protocol envelopes only.
        current = q.read(REPO / q.SOURCE / 'current-source.json')
        observer = copy.deepcopy(self.manifest)
        if observer_mutation is not None: observer_mutation(observer['files'])
        objects = {}
        provenance = {'checkout_head': 'a' * 40, 'checkout_tree': 'b' * 40, 'source_only_tree': current['source_only_tree']}
        for role, kind, manifest in (('ordinary', 'unit4-public-v3-candidate', current),
                                     ('observer', 'unit4-public-v3-lifecycle-observer', observer)):
            manifest_id = {'path': role + '-manifest', 'sha256': q.CURRENT_SHA if role == 'ordinary' else q.sha(q.canonical(manifest))}
            objects[manifest_id['path']] = manifest
            binaries = {profile: {'path': role + '-' + profile, 'bytes': 1, 'sha256': str(i + 1) * 64}
                        for i, profile in enumerate(q.PROFILES)}
            receipts = {}
            for profile in q.PROFILES:
                row = {'path': role + '-' + profile + '-receipt'}
                receipts[profile] = row
                objects[row['path']] = {'status': 0, 'profile': profile, 'source_before': manifest_id['sha256'],
                    'source_after': manifest_id['sha256'], 'source_manifest_sha256': manifest_id['sha256'],
                    'binary': binaries[profile], 'argv': ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline'] + (['--release'] if profile == 'release' else []),
                    'rustc': 'rustc 1.99.0 local-test\nhost: x86_64-unknown-linux-gnu\n',
                    'environment': {'CARGO_INCREMENTAL': '0', 'CARGO_BUILD_JOBS': '2'}, 'streams': {}}
            objects[str(output / (role + '-build') / 'candidate-binding.json')] = {
                'kind': kind, 'compiler_head': provenance['checkout_head'], 'compiler_head_tree': provenance['checkout_tree'],
                'compiler_source_only_tree': provenance['source_only_tree'], 'source_manifest': manifest_id,
                'binaries': binaries, 'build_receipts': receipts}
        capsule = SimpleNamespace(named=lambda path: {'path': path, 'sha256': '9' * 64},
                                  json=lambda row: objects[row['path']], raw=lambda row: b'')
        return capsule, provenance

    def test_final_join_accepts_actual_current_ordinary_and_observer_maps(self):
        for output in (Path('/synthetic'), PureWindowsPath('C:/synthetic')):
            with self.subTest(output=str(output)):
                capsule, provenance = self.current_build_capsule(output)
                configs = join.build_configs(capsule, str(output), provenance, 'Linux x86_64', REPO)
                self.assertEqual(set(configs), {'ordinary', 'observer'})

    def test_synthetic_build_capsule_preserves_both_path_flavors(self):
        for output in (PurePosixPath('/synthetic'), PureWindowsPath('C:/synthetic')):
            with self.subTest(output=str(output)):
                capsule, _ = self.current_build_capsule(output)
                for role in ('ordinary', 'observer'):
                    member = str(output / (role + '-build') / 'candidate-binding.json')
                    self.assertEqual(capsule.json(capsule.named(member))['kind'],
                                     'unit4-public-v3-candidate' if role == 'ordinary' else 'unit4-public-v3-lifecycle-observer')

    def test_final_join_rejects_changed_or_incomplete_current_observer_map(self):
        for mutate in (lambda rows: rows.pop(), lambda rows: rows.reverse(),
                       lambda rows: rows[-1].update(sha256='0' * 64)):
            for output in (Path('/synthetic'), PureWindowsPath('C:/synthetic')):
                with self.subTest(output=str(output)):
                    capsule, provenance = self.current_build_capsule(output, mutate)
                    with self.assertRaisesRegex(q.Reject, 'public source authority'):
                        join.build_configs(capsule, str(output), provenance, 'Linux x86_64', REPO)

    def test_lifecycle_successor_preserves_exact_enum_and_array_lineage(self):
        current = (REPO / q.OBSERVER_PATCH).read_bytes()
        original = (REPO / 'tests/fixtures/typed_project_unit4_independent/components/lifecycle/observer-additive-v1.patch').read_bytes()
        self.assertEqual(self.builder.verify_lifecycle_successor(current), original)
        for changed in (current + b'\n', current.replace(b'ArraySyntaxPolicy', b'OtherSyntaxPolicy'),
                        current.replace(b'parse_attempt', b'other_attempt'),
                        current.replace(b'EnumSyntaxPolicy', b'OtherEnumSyntaxPolicy')):
            with self.subTest(changed=q.sha(changed)), self.assertRaises(Exception):
                self.builder.verify_lifecycle_successor(changed)

    def test_exact_approved_bodies_under_crlf_git_configuration(self):
        self.assertEqual(len(self.manifest['files']), 364)
        self.assertEqual(q.sha(q.canonical(self.manifest['files'])), self.builder.OBSERVER_FILES_SHA)
        for row in self.manifest['files']:
            q.verify(self.output / 'source' / row['path'], row)
        self.assertEqual(self.config.read_bytes(), b'[core]\n autocrlf = true\n eol = crlf\n')
        self.assertEqual(q.read(self.output / 'prepared.json')['compiler_invocations'], 0)

    def test_current_identity_rejects_before_observer_materialization(self):
        for member in ('src/frontend/oir/owned_types/array_tests.rs', 'src/frontend/format.rs',
                       'src/frontend/oir/unary_source_tests.rs', 'src/frontend/parser/enums.rs',
                       'tests/fixtures/bounded_enum_scanner/main.ox',
                       'tests/fixtures/bounded_enum_scanner/scanner.ox'):
            for control in ('changed', 'coherent-changed', 'missing', 'coherent-missing', 'stale-checkpoint'):
                with self.subTest(member=member, control=control), tempfile.TemporaryDirectory() as temporary:
                    root = Path(temporary).resolve()
                    source = root / 'source'
                    manifest = q.read(REPO / q.SOURCE / 'current-source.json')
                    for row in manifest['files']:
                        path = source / row['path']
                        path.parent.mkdir(parents=True, exist_ok=True)
                        path.write_bytes((REPO / row['path']).read_bytes())
                    row = next(row for row in manifest['files'] if row['path'] == member)
                    path = source / row['path']
                    if control.endswith('changed'):
                        path.write_bytes(path.read_bytes() + b'// unauthorized groundwork change\n')
                        if control.startswith('coherent'):
                            row.update(bytes=path.stat().st_size, sha256=q.sha(path.read_bytes()))
                    elif control.endswith('missing'):
                        path.unlink()
                        if control.startswith('coherent'):
                            manifest['files'].remove(row)
                    else:
                        manifest['reviewed_source_head'] = '0' * 40
                    manifest_path = root / 'current-source.json'
                    q.save(manifest_path, manifest)
                    output = root / 'observer'
                    args = SimpleNamespace(source_root=source, manifest=manifest_path, out=output)
                    with patch.object(self.builder.subprocess, 'run', side_effect=AssertionError('patch/tool must not run')):
                        with self.assertRaises((self.c.Reject, OSError)):
                            self.builder.prepare(args)
                    self.assertFalse(output.exists())

    def test_coherently_rehashed_groundwork_observer_rejects_before_compiler(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            shutil.copytree(self.output, root / 'prepared')
            manifest_path = root / 'prepared/observer-source.json'
            manifest = q.read(manifest_path)
            source = root / 'prepared/source'
            row = next(row for row in manifest['files'] if row['path'] == 'src/frontend/oir/owned_types/array_tests.rs')
            path = source / row['path']
            path.write_bytes(path.read_bytes() + b'// unauthorized observer groundwork change\n')
            row.update(bytes=path.stat().st_size, sha256=q.sha(path.read_bytes()))
            q.save(manifest_path, manifest)
            args = SimpleNamespace(source_root=source, manifest=manifest_path, kind='unit4-public-v3-lifecycle-observer')
            with patch.object(self.builder.subprocess, 'check_output', side_effect=AssertionError('compiler must not run')):
                with self.assertRaisesRegex(self.c.Reject, 'unapproved observer build source'):
                    self.builder.build(args)

    def test_native_path_flavors_emit_identical_frozen_component_order(self):
        names = [row['path'] for row in self.manifest['files']]
        native_orders = []
        for root in (PurePosixPath('/source'), PureWindowsPath('C:/source')):
            native = sorted(root / name for name in names)
            native_orders.append([path.relative_to(root).as_posix() for path in native])
            self.assertEqual([path.relative_to(root).as_posix() for path in self.builder.observer_path_order(native, root)], names)
        self.assertNotEqual(*native_orders)
        self.assertNotEqual(names, sorted(names))  # The approved authority uses path components.

    def test_order_preserves_case_and_duplicate_members(self):
        names = ['a.rs', 'A.rs', 'a/B.rs', 'a.rs', 'a/b.rs']
        expected = ['A.rs', 'a/B.rs', 'a/b.rs', 'a.rs', 'a.rs']
        for root in (PurePosixPath('/source'), PureWindowsPath('C:/source')):
            got = self.builder.observer_path_order([root / name for name in names], root)
            self.assertEqual([path.relative_to(root).as_posix() for path in got], expected)

    def test_exact_adapter_identity_is_identical_across_path_flavors(self):
        run = q.public_modules(REPO)[2]
        class ViewedPath:
            def __init__(self, actual, flavor):
                self.actual, self.flavor = actual, flavor(actual.name)
            def __lt__(self, other):
                return self.flavor < other.flavor
            def __getattr__(self, name):
                return getattr(self.actual, name)
        expected = run.adapter_identity()
        results, native_orders = [], []
        for flavor in (PurePosixPath, PureWindowsPath):
            members = [ViewedPath(path, flavor) for path in (REPO / q.PUBLIC).iterdir()]
            native_orders.append([path.name for path in sorted(members)])
            with patch.object(run, 'ADAPTER_ROOT', SimpleNamespace(iterdir=lambda: iter(members))):
                results.append(run.adapter_identity())
        self.assertNotEqual(*native_orders)
        self.assertEqual(results, [expected, expected])
        self.assertEqual(len(expected), 21)
        self.assertEqual([row['path'] for row in expected], sorted(run.PACKAGE_FILES))
        for changed in (expected[:-1], expected + expected[:1], list(reversed(expected))):
            self.assertNotEqual(changed, expected)  # Preserve the strict cross-host list contract.

    def test_changed_payload_and_missing_member_reject_before_compiler(self):
        for control in ('changed-payload', 'coherent-changed-payload', 'missing-member', 'coherent-missing-member', 'windows-order'):
            with self.subTest(control=control), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                shutil.copytree(self.output, root / 'prepared')
                manifest_path = root / 'prepared/observer-source.json'
                manifest = q.read(manifest_path)
                source = root / 'prepared/source'
                row = next(row for row in manifest['files'] if row['path'] == 'src/frontend/lifecycle_observer.rs')
                if control.endswith('changed-payload'):
                    path = source / row['path']
                    path.write_bytes(path.read_bytes() + b'// altered observer payload\n')
                    if control.startswith('coherent'):
                        row.update(bytes=path.stat().st_size, sha256=q.sha(path.read_bytes()))
                elif control.endswith('missing-member'):
                    (source / row['path']).unlink()
                    if control.startswith('coherent'):
                        manifest['files'].remove(row)
                else:
                    manifest['files'].sort(key=lambda row: PureWindowsPath(row['path']))
                q.save(manifest_path, manifest)
                args = SimpleNamespace(source_root=source, manifest=manifest_path, kind='unit4-public-v3-lifecycle-observer')
                with patch.object(self.builder.subprocess, 'check_output', side_effect=AssertionError('compiler must not run')):
                    with self.assertRaises((self.c.Reject, OSError)):
                        self.builder.build(args)


class PackageControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        (self.root / 'package').mkdir()
        (self.root / 'package/member').write_bytes(b'approved')
        row = q.identity(self.root / 'package/member')
        row['path'] = 'package/member'
        self.manifest = {'closed_roots': ['package'], 'files': [row]}

    def tearDown(self): self.temp.cleanup()

    def test_approved_package(self): q.verify_package(self.root, self.manifest)

    def test_coherently_bound_python_cache_is_rejected(self):
        for name in ('package/__pycache__/member.cpython-312.pyc', 'package/member.pyc', 'package/member.pyo'):
            with self.subTest(name=name):
                path = self.root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'generated cache')
                row = q.identity(path)
                row['path'] = name
                manifest = {'closed_roots': ['package'],
                            'files': sorted(self.manifest['files'] + [row], key=lambda r: r['path'])}
                with self.assertRaisesRegex(q.Reject, 'generated Python cache'):
                    q.verify_package(self.root, manifest)
                path.unlink()
                if path.parent.name == '__pycache__': path.parent.rmdir()

    def test_omitted_member(self):
        (self.root / 'package/member').unlink()
        with self.assertRaises(q.Reject): q.verify_package(self.root, self.manifest)

    def test_changed_member(self):
        (self.root / 'package/member').write_bytes(b'changed!')
        with self.assertRaises(q.Reject): q.verify_package(self.root, self.manifest)

    def test_extra_member_and_directory(self):
        extra = self.root / 'package/extra'
        extra.mkdir()
        with self.assertRaises(q.Reject): q.verify_package(self.root, self.manifest)
        extra.rmdir(); extra.write_bytes(b'extra')
        with self.assertRaises(q.Reject): q.verify_package(self.root, self.manifest)

    def test_stale_manifest_identity(self):
        self.manifest['files'][0]['sha256'] = '0' * 64
        with self.assertRaises(q.Reject): q.verify_package(self.root, self.manifest)

    def test_duplicate_manifest_identity(self):
        self.manifest['files'].append(self.manifest['files'][0])
        with self.assertRaises(q.Reject): q.verify_package(self.root, self.manifest)

    def test_symlink_member(self):
        member = self.root / 'package/member'
        member.unlink()
        (self.root / 'outside').write_bytes(b'approved')
        try: member.symlink_to(self.root / 'outside')
        except OSError: self.skipTest('host lacks symlink creation permission; production still rejects symlinks')
        with self.assertRaises(q.Reject): q.verify_package(self.root, self.manifest)


class CheckoutControls(unittest.TestCase):
    def test_scoped_checkout_preserves_committed_lf_crlf_and_binary(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            def git(*args):
                return subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True).stdout
            git('init', '--quiet')
            fixtures = {'lf.txt': b'one\ntwo\n', 'intentional-crlf.txt': b'one\r\ntwo\r\n', 'binary.dat': b'\x00\xff\r\n\x10\n'}
            for name, raw in fixtures.items(): (root / name).write_bytes(raw)
            git('-c', 'core.autocrlf=false', 'add', '.')
            git('-c', 'user.name=Unit4 local fixture', '-c', 'user.email=unit4-fixture@localhost', 'commit', '-qm', 'exact byte fixture')
            git('config', 'core.autocrlf', 'true')
            for name in fixtures: (root / name).write_bytes(b'changed checkout')
            git('-c', 'core.autocrlf=false', '-c', 'core.eol=lf', 'checkout-index', '--all', '--force')
            for name, raw in fixtures.items():
                self.assertEqual(git('show', 'HEAD:' + name), raw)
                self.assertEqual((root / name).read_bytes(), raw)
            self.assertEqual(git('config', 'core.autocrlf').strip(), b'true')
            workflow = (REPO / '.github/workflows/ci.yml').read_text()
            self.assertIn('common.py --restore-checkout --repo "$GITHUB_WORKSPACE"', workflow)


    def test_fresh_prefix_restores_a_stable_crlf_index_without_normalizing_blobs(self):
        import os
        import shutil
        import time
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            repo = root / 'checkout'; repo.mkdir()
            def git(*args):
                return subprocess.run(['git', '-C', str(repo), *args], check=True, capture_output=True).stdout
            fixtures = {'lf.txt': b'one\ntwo\n', 'intentional-crlf.txt': b'one\r\ntwo\r\n', 'binary.dat': b'\x00\xff\r\n\x10\n'}
            git('init', '--quiet')
            for name, raw in fixtures.items(): (repo / name).write_bytes(raw)
            git('-c', 'core.autocrlf=false', 'add', '.')
            git('-c', 'user.name=Unit4 fixture', '-c', 'user.email=unit4@localhost', 'commit', '-qm', 'blob-byte fixture')
            git('config', 'core.autocrlf', 'true')
            for name in fixtures: (repo / name).unlink()
            git('checkout', '--force', 'HEAD')
            self.assertEqual((repo / 'lf.txt').read_bytes(), b'one\r\ntwo\r\n')
            # Make the index stat entry older than the refreshed index, avoiding a racy fresh-checkout case.
            for name in fixtures: os.utime(repo / name, (time.time() - 60, time.time() - 60))
            git('update-index', '--really-refresh')
            sentinel = repo / 'untracked.txt'; sentinel.write_bytes(b'preserve untracked')
            metadata = {p.relative_to(repo).as_posix(): p.read_bytes() for p in (repo / '.git').rglob('*') if p.is_file()}
            result = q.restore_committed_checkout(repo, root / 'restoration', git('rev-parse', 'HEAD').decode().strip())
            self.assertEqual(result['status'], 'restored')
            for name, raw in fixtures.items():
                self.assertEqual((repo / name).read_bytes(), git('show', 'HEAD:' + name))
                self.assertEqual((repo / name).read_bytes(), raw)
            self.assertEqual((repo / 'untracked.txt').read_bytes(), b'preserve untracked')
            self.assertEqual(metadata, {p.relative_to(repo).as_posix(): p.read_bytes() for p in (repo / '.git').rglob('*') if p.is_file()})
            self.assertEqual(git('config', 'core.autocrlf').strip(), b'true')
            restored = {row['path']: row for row in result['files']}
            self.assertNotEqual(restored['lf.txt']['before']['sha256'], restored['lf.txt']['after']['sha256'])
            self.assertEqual(restored['intentional-crlf.txt']['before']['sha256'], restored['intentional-crlf.txt']['after']['sha256'])
            workflow = (REPO / '.github/workflows/ci.yml').read_text()
            stage = workflow.split('- name: Restore and verify tracked committed bytes through a fresh checkout', 1)[1].split('      - name:', 1)[0]
            self.assertIn('--restore-checkout --repo "$GITHUB_WORKSPACE"', stage)
            self.assertIn('--expected-head "$UNIT4_EXPECTED_HEAD"', stage)

    def test_linux_uses_fresh_owned_checkout_and_exact_scoped_trust(self):
        workflow = (REPO / '.github/workflows/ci.yml').read_text()
        linux = workflow.split('  unit4-linux:\n', 1)[1].split('  unit4-portable-hosts:\n', 1)[0]
        self.assertIn('defaults:\n      run:\n        working-directory: unit4-current', linux)
        self.assertIn('working-directory: /\n        run:', linux)
        self.assertIn('mkdir "$GITHUB_WORKSPACE/unit4-current"', linux)
        self.assertLess(linux.index('mkdir "$GITHUB_WORKSPACE/unit4-current"'), linux.index('uses: actions/checkout'))
        self.assertIn('path: unit4-current', linux)
        self.assertIn('set-safe-directory: false', linux)
        self.assertIn('--repo "$GITHUB_WORKSPACE/unit4-current"', linux)
        self.assertEqual(linux.count('git -c safe.directory="$GITHUB_WORKSPACE/unit4-current" '), 3)
        self.assertNotIn('--global', linux)
        self.assertNotIn('safe.directory=*', linux)
        self.assertNotIn('chown', linux)

    def test_scoped_trust_and_clean_git_owned_worktree(self):
        import os
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            checkout = root / 'unit4-current'
            checkout.mkdir()
            def git(*args, env=None):
                return subprocess.run(['git', '-C', str(checkout), *args], capture_output=True, env=env)
            self.assertEqual(git('init', '--quiet').returncode, 0)
            (checkout / 'source').write_bytes(b'committed source\n')
            self.assertEqual(git('add', '.').returncode, 0)
            self.assertEqual(git('-c', 'user.name=Unit4 fixture', '-c', 'user.email=unit4@localhost', 'commit', '-qm', 'owned checkout').returncode, 0)
            head = git('rev-parse', 'HEAD').stdout.strip()
            config_before = (checkout / '.git/config').read_bytes()
            # Git's bounded test switch exercises rejection without changing ownership.
            foreign = {**os.environ, 'GIT_TEST_ASSUME_DIFFERENT_OWNER': '1', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull}
            denied = git('rev-parse', 'HEAD', env=foreign)
            self.assertNotEqual(denied.returncode, 0)
            self.assertIn(b'dubious ownership', denied.stderr)
            self.assertEqual(git('-c', 'safe.directory=' + str(checkout), 'rev-parse', 'HEAD', env=foreign).stdout.strip(), head)
            self.assertNotEqual(git('-c', 'safe.directory=' + str(root / 'other'), 'rev-parse', 'HEAD', env=foreign).returncode, 0)
            self.assertEqual((checkout / '.git/config').read_bytes(), config_before)
            if sys.platform.startswith('linux'):
                # The unchanged parser erases inherited Git trust; the owned checkout still works.
                portable = q.module('_unit4_test_portable_git', REPO / q.PARSER / 'portable.py')
                self.assertEqual(portable.git(checkout, 'rev-parse', 'HEAD').strip(), head)
                historical = root / 'historical'
                self.assertEqual(git('worktree', 'add', '--detach', str(historical), head.decode()).returncode, 0)
                self.assertEqual(portable.git(historical, 'show', 'HEAD:source'), b'committed source\n')

    def test_windows_qualification_uses_native_shell_and_propagates_exit(self):
        workflow = (REPO / '.github/workflows/ci.yml').read_text()
        portable = workflow.split('  unit4-portable-hosts:\n', 1)[1].split('  unit4-qualification:\n', 1)[0]
        mac = portable.split('- name: Execute ordinary, lifecycle and actual no-native trap gates', 1)[1].split('      - name:', 1)[0]
        self.assertIn("if: runner.os != 'Windows'", mac)
        windows = portable.split('- name: Execute Windows gates with the native toolchain search path', 1)[1].split('      - name:', 1)[0]
        self.assertIn("if: runner.os == 'Windows'", windows)
        self.assertIn('shell: pwsh', windows)
        self.assertIn('Microsoft.VisualStudio.Component.VC.Tools.x86.x64', windows)
        self.assertIn('& $unit4_dev_shell -Arch amd64 -HostArch amd64 -SkipAutomaticLocation', windows)
        self.assertIn('python -B tests/qualification/unit4_ci/gate.py host', windows)
        for key in ('GITHUB_WORKSPACE', 'RUNNER_TEMP', 'UNIT4_EXPECTED_HEAD', 'UNIT4_EVENT_SHA', 'UNIT4_HOST'):
            self.assertIn('$env:' + key, windows)
        self.assertTrue(windows.rstrip().endswith('exit $LASTEXITCODE'))


class CheckoutRestorationControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.repo = self.root / 'checkout'; self.repo.mkdir()
        self.git('init', '--quiet')
        self.original = {'nested space/LF.txt': b'one\ntwo\n', 'intentional.txt': b'one\r\ntwo\r\n', 'binary.dat': b'\x00\xff\r\n'}
        for name, raw in self.original.items():
            path = self.repo / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(raw)
        self.git('-c', 'core.autocrlf=false', 'add', '.')
        self.git('-c', 'user.name=Unit4 fixture', '-c', 'user.email=unit4@localhost', 'commit', '-qm', 'restoration control')
        self.head = self.git('rev-parse', 'HEAD').decode().strip()
        (self.repo / 'nested space/LF.txt').write_bytes(b'one\r\ntwo\r\n')
        (self.repo / 'untracked').write_bytes(b'keep')
        self.before = {name: (self.repo / name).read_bytes() for name in self.original}
        self.out = self.root / 'result'

    def git(self, *args):
        return subprocess.run(['git', '-C', str(self.repo), *args], check=True, capture_output=True).stdout

    def tearDown(self): self.temp.cleanup()

    def test_spaced_paths_and_receipt_preserve_all_blob_identities(self):
        result = q.restore_committed_checkout(self.repo, self.out, self.head)
        self.assertEqual(result['status'], 'restored')
        self.assertEqual({row['path'] for row in result['files']}, set(self.original))
        for row in result['files']:
            self.assertEqual(row['before']['sha256'], q.sha(self.before[row['path']]))
            self.assertEqual(row['after']['sha256'], q.sha(self.original[row['path']]))
            self.assertEqual((self.repo / row['path']).read_bytes(), self.original[row['path']])
        self.assertEqual((self.repo / 'untracked').read_bytes(), b'keep')

    def test_changed_index_rejects_before_checkout_writes(self):
        self.git('-c', 'core.autocrlf=false', 'add', 'nested space/LF.txt')
        with self.assertRaises(q.Reject): q.restore_committed_checkout(self.repo, self.out, self.head)
        self.assertFalse(q.read(self.out / 'restoration.json')['writes_started'])
        self.assertEqual({name: (self.repo / name).read_bytes() for name in self.original}, self.before)

    def test_unsupported_git_mode_rejects_before_checkout_writes(self):
        oid = self.git('rev-parse', 'HEAD:intentional.txt').decode().strip()
        self.git('update-index', '--add', '--cacheinfo', '120000,' + oid + ',unsupported-link')
        self.git('-c', 'user.name=Unit4 fixture', '-c', 'user.email=unit4@localhost', 'commit', '-qm', 'unsupported mode control')
        head = self.git('rev-parse', 'HEAD').decode().strip()
        with self.assertRaises(q.Reject): q.restore_committed_checkout(self.repo, self.out, head)
        self.assertFalse(q.read(self.out / 'restoration.json')['writes_started'])
        self.assertEqual({name: (self.repo / name).read_bytes() for name in self.original}, self.before)

    def test_bad_materialized_members_or_bytes_reject_before_writes(self):
        original_run = subprocess.run
        for mutation in ('missing', 'extra', 'directory', 'changed'):
            def run(argv, *args, **kwargs):
                result = original_run(argv, *args, **kwargs)
                if 'checkout-index' in argv:
                    view = Path(next(arg[len('--prefix='):] for arg in argv if arg.startswith('--prefix=')))
                    if mutation == 'missing': (view / 'intentional.txt').unlink()
                    elif mutation == 'extra': (view / 'extra').write_bytes(b'extra')
                    elif mutation == 'directory': (view / 'extra-dir').mkdir()
                    else: (view / 'intentional.txt').write_bytes(b'altered')
                return result
            out = self.root / mutation
            with self.subTest(mutation=mutation), patch.object(q.subprocess, 'run', side_effect=run):
                with self.assertRaises(q.Reject): q.restore_committed_checkout(self.repo, out, self.head)
            self.assertFalse(q.read(out / 'restoration.json')['writes_started'])
            self.assertEqual({name: (self.repo / name).read_bytes() for name in self.original}, self.before)

    def test_unsafe_target_and_inside_output_reject_without_checkout_writes(self):
        with self.assertRaises(q.Reject): q.restore_committed_checkout(self.repo, self.repo / 'must-not-create', self.head)
        self.assertFalse((self.repo / 'must-not-create').exists())
        target = self.repo / 'intentional.txt'; target.unlink(); target.mkdir()
        with self.assertRaises(q.Reject): q.restore_committed_checkout(self.repo, self.out, self.head)
        self.assertFalse(q.read(self.out / 'restoration.json')['writes_started'])
        self.assertTrue(target.is_dir())
        self.assertEqual((self.repo / 'nested space/LF.txt').read_bytes(), self.before['nested space/LF.txt'])

    def test_copy_error_propagates_and_preserves_failure_receipt(self):
        with patch.object(q.shutil, 'copyfile', side_effect=OSError('controlled copy failure')):
            with self.assertRaises(OSError): q.restore_committed_checkout(self.repo, self.out, self.head)
        self.assertEqual(q.read(self.out / 'restoration.json')['status'], 'failed')
        self.assertEqual((self.repo / 'untracked').read_bytes(), b'keep')


class WindowsToolchainControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.tools = self.root / 'MSVC/bin/Hostx64/x64'
        self.tools.mkdir(parents=True)
        for name in ('link.exe', 'cl.exe'): (self.tools / name).write_bytes(b'synthetic fixture: ' + name.encode())
        self.env = {'PATH': str(self.tools), 'LIB': 'library search', 'INCLUDE': 'header search',
                    'VCINSTALLDIR': str(self.root), 'VSINSTALLDIR': str(self.root),
                    'VCToolsInstallDir': str(self.root / 'MSVC'), 'VSCMD_ARG_TGT_ARCH': 'x64', 'VSCMD_ARG_HOST_ARCH': 'x64'}
        self.result = {'status': 0, 'timed_out': False, 'stream_limit_exceeded': False,
                       'argv': [str(self.tools / 'link.exe'), '/?'], 'cwd': str(self.root),
                       'stdout': 'Microsoft (R) Incremental Linker Version 14.51.36260.0\r\nCopyright (C) Microsoft Corporation.  All rights reserved.\r\n\r\n usage: LINK [options] [files] [@commandfile]\r\n\r\n   options:\r\n\r\n      /MACHINE:{X64|X86}\r\n      /OUT:filename\r\n', 'stderr': ''}
        self.calls = []
        def process(argv, cwd, env, timeout):
            self.calls.append((argv, cwd, env, timeout))
            return dict(self.result)
        self.driver = SimpleNamespace(env=self.env, output=self.root, state={},
            runtime=SimpleNamespace(process=process), write=lambda: q.save(self.root / 'driver.json', self.driver.state))
        self.which = patch.object(gate.shutil, 'which', side_effect=lambda name, path: str(self.tools / name))
        self.which.start()

    def tearDown(self):
        self.which.stop()
        self.temp.cleanup()

    def test_clean_environment_retains_only_named_discovery_keys(self):
        supplied = {**self.env, **{key: 'preinstalled-location' for key in gate.WINDOWS_DISCOVERY_KEYS},
                    'UNRELATED_SECRET': 'must not be copied', 'RUSTFLAGS': 'must not be copied'}
        with patch.object(gate, 'os', SimpleNamespace(name='nt', environ=supplied)):
            actual = gate.clean_environment()
        for key in gate.WINDOWS_DISCOVERY_KEYS: self.assertEqual(actual[key], supplied[key])
        self.assertEqual(actual['VSLANG'], '1033')
        self.assertNotIn('UNRELATED_SECRET', actual)
        self.assertNotIn('RUSTFLAGS', actual)
        with patch.object(gate, 'os', SimpleNamespace(name='posix', environ=supplied)):
            nonwindows = gate.clean_environment()
        self.assertFalse(set(gate.WINDOWS_DISCOVERY_KEYS) & nonwindows.keys())
        self.assertNotIn('VSLANG', nonwindows)

    def test_configured_tools_and_linker_receipt(self):
        gate.verify_windows_toolchain(self.driver)
        saved = q.read(self.root / 'driver.json')['windows_toolchain']
        self.assertEqual(saved['status'], 'pass')
        self.assertEqual(saved['tools']['link.exe'], q.identity(self.tools / 'link.exe'))
        self.assertEqual(saved['linker_help'], self.result)
        self.assertEqual(self.calls, [([str(self.tools / 'link.exe'), '/?'], self.root, self.env, 30)])
        self.assertEqual(saved['search_environment_sha256']['PATH'], q.sha(self.env['PATH'].encode()))
        self.assertNotIn('environment', saved)

    def test_wrong_path_tool_is_rejected_before_execution(self):
        other = self.root / 'link.exe'; other.write_bytes(b'GNU-link fixture')
        with patch.object(gate.shutil, 'which', return_value=str(other)):
            with self.assertRaises(q.Reject): gate.verify_windows_toolchain(self.driver)
        self.assertEqual(self.calls, [])
        self.assertEqual(q.read(self.root / 'driver.json')['windows_toolchain']['status'], 'fail')

    def test_unconfigured_or_wrong_architecture_is_rejected(self):
        for key, value in (('VCINSTALLDIR', ''), ('VSCMD_ARG_TGT_ARCH', 'x86'), ('VSCMD_ARG_HOST_ARCH', 'arm64')):
            with self.subTest(key=key), patch.dict(self.env, {key: value}):
                with self.assertRaises(q.Reject): gate.verify_windows_toolchain(self.driver)
        self.assertEqual(self.calls, [])

    def test_gnu_diagnostic_or_nonzero_help_exit_is_rejected(self):
        for change in ({'stdout': "link: extra operand; Try 'link --help'"}, {'status': 7}, {'timed_out': True}, {'stream_limit_exceeded': True}):
            with self.subTest(change=change), patch.dict(self.result, change):
                with self.assertRaises(q.Reject): gate.verify_windows_toolchain(self.driver)
        self.assertEqual(q.read(self.root / 'driver.json')['windows_toolchain']['status'], 'fail')


    def test_observed_help_exit_is_preserved_and_accepted_only_for_help(self):
        self.result['status'] = 1100
        gate.verify_windows_toolchain(self.driver)
        saved = q.read(self.root / 'driver.json')['windows_toolchain']
        self.assertEqual(saved['status'], 'pass')
        self.assertEqual(saved['linker_help']['status'], 1100)
        self.assertEqual(saved['linker_help'], self.result)

    def test_exit_1100_nonhelp_wrong_provenance_and_error_are_rejected(self):
        self.result['status'] = 1100
        for change in (
            {'argv': [str(self.tools / 'link.exe'), '/OUT:program.exe', 'input.obj']},
            {'argv': [str(self.root / 'other-link.exe'), '/?']}, {'cwd': str(self.root / 'other')},
            {'stdout': "link: extra operand; Try 'link --help'"},
            {'stdout': self.result['stdout'] + 'LINK : fatal error LNK1100: bad input\r\n'},
            {'stdout': self.result['stdout'].replace(' usage: LINK', ' missing: LINK')},
            {'stderr': 'LINK : fatal error LNK1100: bad input'}, {'status': 1101},
            {'timed_out': True}, {'stream_limit_exceeded': True}):
            with self.subTest(change=change), patch.dict(self.result, change):
                with self.assertRaises(q.Reject): gate.verify_windows_toolchain(self.driver)
        self.assertEqual(q.read(self.root / 'driver.json')['windows_toolchain']['status'], 'fail')

    def test_exit_1100_never_qualifies_an_actual_build_stage(self):
        self.result['status'] = 1100
        provenance = {'checkout_head': 'a' * 40, 'event_sha': 'b' * 40}
        driver = gate.Driver(REPO, self.root, provenance, self.driver.runtime)
        with patch.object(q, 'admit', return_value=provenance):
            with self.assertRaises(q.Reject): driver.stage('04-build-ordinary', ['synthetic-build-control'], 10)
        self.assertEqual(q.read(self.root / 'commands/04-build-ordinary/receipt.json')['status'], 1100)


class FullArchiveControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.output = self.root / 'output'; self.output.mkdir()
        self.archive = self.root / 'full-evidence.tar.gz'
        q.save(self.output / 'driver.json', {'status': 'fail'})

    def parser_build(self, profile='debug', control=False, status='build-failure', exit_code=101):
        # Frozen helpers/build.py emits no binary on build-failure, including
        # Cargo exit zero when its executable inventory is not exactly one.
        result = self.output / 'parser' / (('build-control-' if control else 'build-') + profile) / 'result'
        result.mkdir(parents=True)
        stdout = result / 'stdout.jsonl'; stdout.write_bytes(b'{"reason":"build-finished"}\n')
        stderr = result / 'stderr.txt'; stderr.write_bytes(b'retained raw build diagnostic\n')
        receipt = {'schema': 'oxid-unit4-parser-build-v1', 'status': status, 'exit_code': exit_code,
                   'profile': profile, 'control': control, 'stdout': q.identity(stdout), 'stderr': q.identity(stderr)}
        target = result / 'target/x86_64-unknown-linux-gnu' / profile / 'deps'
        target.mkdir(parents=True)
        (target / 'regenerable-cache').write_bytes(b'not evidence')
        if status == 'built':
            binary = target / 'oxid-a1b2c3'
            binary.write_bytes(('synthetic executable ' + profile + str(control)).encode())
            receipt['binary'] = q.identity(binary)
        path = result / 'build-receipt.json'
        q.save(path, receipt)
        return path, receipt

    def archive_members(self):
        with tarfile.open(self.archive, 'r:gz') as archive:
            return {member.name: archive.extractfile(member).read() for member in archive.getmembers()}

    def assert_retained(self, members, path):
        self.assertEqual(members[Path(path).relative_to(self.output).as_posix()], Path(path).read_bytes())

    def test_failed_parser_export_preserves_raw_evidence_without_qualifying(self):
        path, receipt = self.parser_build()
        archives = self.root / 'archives'
        process = subprocess.run([sys.executable, '-B', str(REPO / 'tests/qualification/unit4_ci/evidence.py'),
                                  '--output', str(self.output), '--archives', str(archives)],
                                 capture_output=True, timeout=30)
        self.assertEqual(process.returncode, 1)
        self.assertIn(b'evidence preserved; qualification remains failed/incomplete', process.stderr)
        report = q.read(archives / 'export.json')
        self.assertEqual(report['status'], 'exported')
        self.assertEqual(report['qualification_status'], 'fail')
        self.assertEqual(set(report['archives']), {'full'})
        self.assertFalse((archives / 'compact.tar.xz').exists())
        self.assertFalse((archives / 'capsule').exists())
        self.archive = archives / 'full-evidence.tar.gz'
        q.verify(self.archive, report['archives']['full'])
        members = self.archive_members()
        for retained in (path, receipt['stdout']['path'], receipt['stderr']['path'], self.output / 'driver.json'):
            self.assert_retained(members, retained)
        archived = q.loads(members[path.relative_to(self.output).as_posix()])
        self.assertEqual(archived['status'], 'build-failure')
        self.assertNotIn('binary', archived)
        self.assertFalse(any('/target/' in name for name in members))

    def test_zero_exit_failed_receipt_does_not_require_or_invent_binary(self):
        path, receipt = self.parser_build(exit_code=0)
        full_archive(self.output, self.archive)
        members = self.archive_members()
        self.assert_retained(members, path)
        self.assertEqual(q.loads(members[path.relative_to(self.output).as_posix()]), receipt)
        self.assertFalse(any('/target/' in name for name in members))

    def test_successful_binaries_are_retained_for_every_profile_and_role(self):
        receipts = [self.parser_build(profile, control, status='built', exit_code=0)
                    for profile in q.PROFILES for control in (False, True)]
        full_archive(self.output, self.archive)
        members = self.archive_members()
        for path, receipt in receipts:
            for retained in (path, receipt['binary']['path'], receipt['stdout']['path'], receipt['stderr']['path']):
                self.assert_retained(members, retained)
        self.assertEqual(sum('/target/' in name for name in members), 4)
        self.assertEqual(q.read(self.output / 'driver.json')['status'], 'fail')

    def test_earlier_successful_binary_survives_a_later_failed_build(self):
        built_path, built = self.parser_build(status='built', exit_code=0)
        failed_path, failed = self.parser_build(control=True)
        full_archive(self.output, self.archive)
        members = self.archive_members()
        for retained in (built_path, built['binary']['path'], failed_path, failed['stderr']['path']):
            self.assert_retained(members, retained)
        self.assertEqual(sum('/target/' in name for name in members), 1)

    def test_successful_receipt_requires_unchanged_binary_identity(self):
        path, receipt = self.parser_build(status='built', exit_code=0)
        binary = Path(receipt['binary']['path']); original = binary.read_bytes()
        for mutation in ('missing-identity', 'missing-file', 'bytes', 'sha256', 'content'):
            with self.subTest(mutation=mutation):
                binary.write_bytes(original)
                changed = copy.deepcopy(receipt)
                if mutation == 'missing-identity': changed.pop('binary')
                elif mutation == 'missing-file': binary.unlink()
                elif mutation == 'bytes': changed['binary']['bytes'] += 1
                elif mutation == 'sha256': changed['binary']['sha256'] = '0' * 64
                else: binary.write_bytes(b'x' * len(original))
                q.save(path, changed)
                with self.assertRaises(q.Reject): full_archive(self.output, self.archive)
                self.assertFalse(self.archive.exists())


class CompactUploadControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.archives = self.root / 'archives'; self.archives.mkdir()
        self.compact = self.archives / 'compact.tar.xz'; self.compact.write_bytes(b'synthetic compact fixture')
        self.report = {'schema': 'oxid-unit4-evidence-export-v1', 'status': 'exported', 'qualification_status': 'pass',
                       'archives': {'compact': q.identity(self.compact)}}
        q.save(self.archives / 'export.json', self.report)
        self.destination = self.root / 'unit4-host-linux-x86_64'

    def tearDown(self): self.temp.cleanup()

    def test_success_stages_only_exact_verified_members(self):
        (self.archives / 'full-evidence.tar.gz').write_bytes(b'must remain separate')
        (self.archives / 'unrelated').write_bytes(b'must not upload')
        result = stage_compact_upload(self.archives, self.destination)
        self.assertEqual(sorted(result['members']), ['compact.tar.xz', 'export.json'])
        self.assertEqual(sorted(p.name for p in self.destination.iterdir()), ['compact.tar.xz', 'export.json'])
        self.assertEqual((self.destination / 'compact.tar.xz').read_bytes(), self.compact.read_bytes())
        self.assertEqual((self.destination / 'export.json').read_bytes(), (self.archives / 'export.json').read_bytes())
        self.assertEqual(set(self.root.glob('*/compact.tar.xz')), {self.compact, self.destination / 'compact.tar.xz'})

    def test_failed_export_preserves_only_failure_receipt(self):
        self.report.update(status='failed', qualification_status='fail', archives={})
        q.save(self.archives / 'export.json', self.report)
        result = stage_compact_upload(self.archives, self.destination)
        self.assertEqual(list(result['members']), ['export.json'])
        self.assertEqual(q.read(self.destination / 'export.json')['status'], 'failed')

    def test_changed_missing_or_wrong_path_compact_is_rejected(self):
        original = self.compact.read_bytes()
        for mutation in ('changed', 'missing', 'path', 'omitted'):
            with self.subTest(mutation=mutation):
                self.compact.write_bytes(original)
                report = copy.deepcopy(self.report)
                if mutation == 'changed': self.compact.write_bytes(b'changed')
                elif mutation == 'missing': self.compact.unlink()
                elif mutation == 'path': report['archives']['compact']['path'] = str(self.root / 'outside')
                else: report['archives'] = {}
                q.save(self.archives / 'export.json', report)
                with self.assertRaises((q.Reject, OSError)): stage_compact_upload(self.archives, self.destination)
                self.assertFalse(self.destination.exists())

    def test_occupied_directory_is_preserved(self):
        self.destination.mkdir(); (self.destination / 'existing').write_bytes(b'preserve')
        with self.assertRaises(q.Reject): stage_compact_upload(self.archives, self.destination)
        self.assertEqual((self.destination / 'existing').read_bytes(), b'preserve')

    def test_linux_upload_has_one_directory_and_join_stays_strict(self):
        workflow = (REPO / '.github/workflows/ci.yml').read_text()
        block = workflow.split('- name: Upload compact Linux raw evidence and bindings', 1)[1].split('      - name:', 1)[0]
        self.assertIn('path: ${{ runner.temp }}/unit4-linux-compact-upload', block)
        self.assertNotIn('path: |', block)
        stage = workflow.split('- name: Stage the closed compact Linux upload directory', 1)[1].split('      - name:', 1)[0]
        self.assertIn('if: always()', stage)
        self.assertIn('--stage-compact-upload "$RUNNER_TEMP/unit4-linux-compact-upload"', stage)
        self.assertIn(".glob('*/compact.tar.xz')", (REPO / 'tests/qualification/unit4_ci/join.py').read_text())


class CapsuleControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.raw = self.root / 'raw.json'
        q.save(self.raw, {'status': 'executed', 'observations': [1, 2]})
        self.bound = q.identity(self.raw)
        capsule = Capsule(self.root / 'capsule')
        capsule.add(self.raw, 'synthetic-control')
        capsule.finish({'status': 'pass', 'scope': 'synthetic transport control only'})

    def tearDown(self): self.temp.cleanup()

    def test_unchanged_transport(self):
        self.assertEqual(ReadCapsule(self.root / 'capsule').raw(self.bound), self.raw.read_bytes())

    def test_symlink_output_ancestry_requires_resolved_test_root(self):
        target = self.root / 'real-directory'
        target.mkdir()
        alias = self.root / 'directory-alias'
        try: alias.symlink_to(target, target_is_directory=True)
        except OSError: self.skipTest('host lacks symlink creation permission; production still rejects symlinks')
        with self.assertRaisesRegex(q.Reject, 'symlink output ancestry'):
            Capsule(alias / 'capsule')
        self.assertFalse((target / 'capsule').exists())
        capsule = Capsule(alias.resolve() / 'capsule')
        capsule.add(self.raw, 'synthetic-control')
        capsule.finish({'status': 'pass', 'scope': 'synthetic transport control only'})
        self.assertEqual(ReadCapsule(capsule.root).raw(self.bound), self.raw.read_bytes())

    def test_altered_member(self):
        (self.root / 'capsule/members/00000').write_bytes(b'altered')
        with self.assertRaises(q.Reject): ReadCapsule(self.root / 'capsule')

    def test_summary_only(self):
        (self.root / 'capsule/members/00000').unlink()
        with self.assertRaises(q.Reject): ReadCapsule(self.root / 'capsule')

    def test_local_only_capsule_is_unconditionally_ineligible(self):
        path = self.root / 'capsule/capsule.json'
        value = q.read(path); value['status'] = 'LOCAL_ONLY_PASS'; q.save(path, value)
        with self.assertRaises(q.Reject): ReadCapsule(self.root / 'capsule')

    def test_rehashed_member_cannot_satisfy_old_receipt(self):
        path = self.root / 'capsule/members/00000'
        path.write_bytes(b'altered')
        index = q.read(self.root / 'capsule/capsule.json')
        index['members'][0].update({key: q.identity(path)[key] for key in ('bytes', 'sha256')})
        q.save(self.root / 'capsule/capsule.json', index)
        capsule = ReadCapsule(self.root / 'capsule')
        with self.assertRaises(q.Reject): capsule.raw(self.bound)

    def test_archive_traversal_duplicate_and_link(self):
        for variant in ('traversal', 'duplicate', 'symlink'):
            with self.subTest(variant=variant):
                archive = self.root / (variant + '.tar.gz')
                with tarfile.open(archive, 'w:gz') as sink:
                    info = tarfile.TarInfo('../escape' if variant == 'traversal' else 'member')
                    if variant == 'symlink': info.type = tarfile.SYMTYPE; info.linkname = '/tmp/escape'
                    else: info.size = 1
                    sink.addfile(info, io.BytesIO(b'x') if info.isfile() else None)
                    if variant == 'duplicate': sink.addfile(info, io.BytesIO(b'x'))
                with self.assertRaises(q.Reject): q.unpack(archive, self.root / ('out-' + variant))

    def test_head_tree_event_and_source_are_distinct(self):
        value = {'checkout_head': 'a' * 40, 'checkout_tree': 'b' * 40, 'event_sha': 'c' * 40,
                 'source_only_tree': 'd' * 40, 'integration': [], 'source_manifest': {'path': '/one', 'bytes': 1, 'sha256': 'e' * 64},
                 'inputs': {'path': '/two', 'bytes': 2, 'sha256': 'f' * 64}, 'workflow': {'path': '/workflow', 'bytes': 3, 'sha256': '1' * 64}}
        for key in ('checkout_head', 'checkout_tree', 'event_sha', 'source_only_tree'):
            altered = copy.deepcopy(value); altered[key] = '0' * 40
            self.assertNotEqual(join.provenance_key(value), join.provenance_key(altered))
        relocated = copy.deepcopy(value); relocated['source_manifest']['path'] = '/host/two'
        self.assertEqual(join.provenance_key(value), join.provenance_key(relocated))


def synthetic_seal_fixture(root):
    """A complete synthetic transport graph; it cannot pass semantic qualification."""
    root = Path(root) / 'parser'
    data, records, omitted = {}, {}, []
    def put(parts, value, included=True):
        path = root.joinpath(*parts) if isinstance(parts, tuple) else Path(parts)
        raw = value if isinstance(value, bytes) else q.canonical(value)
        record = {'path': str(path), 'bytes': len(raw), 'sha256': q.sha(raw)}
        data[str(path)] = raw
        if included: records[str(path)] = record
        return record
    def process(base):
        return put((base, 'invocation.json'), {'stdout': put((base, 'driver.stdout'), b''), 'stderr': put((base, 'driver.stderr'), b'')})
    adapter = q.module('_unit4_synthetic_current_parser', REPO / q.PARSER / 'portable.py')
    authority = adapter.authority()
    overlays = {kind: put((kind, 'overlay-manifest.json'), adapter.current_overlay(root / kind, authority, kind == 'control-source'))
                for kind in ('source', 'control-source')}
    for directory in ('source', 'control-source'):
        put((directory, 'observer-source-manifest.json'), authority['helper_files'])
    for directory, field in (('source', 'current_derived_files'), ('control-source', 'current_control_derived_files')):
        for row in authority['current'][field]:
            record = {'path': str(root / directory / row['path']), 'bytes': row['bytes'], 'sha256': row['sha256']}
            records[record['path']] = record
            if row['path'] == 'candidate-source-manifest.json':
                put((directory, row['path']), (json.dumps(adapter.current_candidate(authority), indent=2, sort_keys=True) + '\n').encode())
            else:
                omitted.append(record)
    for row in authority['helper_files']:
        put(('helpers', row['path']), (REPO / q.PARSER_FROZEN / 'frozen/helpers' / row['path']).read_bytes())
    transitions = []
    historical_candidate = (json.dumps({'schema': 'oxid-unit4-candidate-source-manifest-v1', 'commit': authority['base_commit'],
                                       'files': authority['original_files']}, indent=2, sort_keys=True) + '\n').encode()
    for control in (False, True):
        directory = 'control-source' if control else 'source'
        preparation = 'prepare-control' if control else 'prepare'
        historical_overlay = {'schema': 'oxid-unit4-observer-overlay-v1', 'base_commit': authority['base_commit'], 'control': control,
                              'source': str(root / directory), 'candidate_source_manifest_sha256': authority['candidate_source_manifest_sha256'],
                              'observer_source_sha256': authority['helper_manifest_sha256'], 'observer_files': authority['helper_files'],
                              'instrumentation': authority['control_instrumentation' if control else 'instrumentation'],
                              'files': authority['control_derived_files' if control else 'derived_files']}
        transitions.append({'control': control, 'historical_candidate': put((preparation, 'historical-candidate-source-manifest.json'), historical_candidate),
                            'historical_overlay': put((preparation, 'historical-overlay-manifest.json'), historical_overlay),
                            'current_candidate': records[str(root / directory / 'candidate-source-manifest.json')],
                            'current_overlay': overlays[directory], 'changes': authority['current']['source_delta']})
    session = put(('session.json',), {'schema': 'oxid-unit4-current-parser-session-v1', 'root': str(root), 'authority_sha256': adapter.AUTHORITY_SHA,
                  'adapter': q.identity(REPO / q.PARSER / 'portable.py'), 'host': {'python_executable': sys.executable},
                  'overlay': overlays['source'], 'control_overlay': overlays['control-source'],
                  'prepare_invocation': process('prepare'), 'control_prepare_invocation': process('prepare-control'),
                  'transition_authority': put(('current-authority.json',), (REPO / q.PARSER / 'authority.json').read_bytes()),
                  'historical_authority': put(('historical-authority.json',), (REPO / q.PARSER_FROZEN / 'authority.json').read_bytes()),
                  'current_source_manifest': put(('current-source-manifest.json',), (REPO / q.SOURCE / 'current-source.json').read_bytes()),
                  'transitions': transitions})
    for profile_index, profile in enumerate(q.PROFILES):
        builds = []
        for control in (False, True):
            base = ('build-control-' if control else 'build-') + profile
            binary = put((base, 'result', 'target', authority['recipe']['target'], profile, 'deps', 'oxid-' + str(10 + profile_index * 2 + control)), b'synthetic executable ' + base.encode())
            omitted.append(binary)
            build = put((base, 'result', 'build-receipt.json'), {'profile': profile, 'control': control, 'binary': binary,
                        'overlay_manifest': overlays['control-source' if control else 'source'],
                        'stdout': put((base, 'result', 'stdout.jsonl'), b''), 'stderr': put((base, 'result', 'stderr.txt'), b'')})
            put((base, 'portable-build.json'), {'profile': profile, 'control': control, 'session': session, 'invocation': process(base), 'receipt': build})
            builds.append(build)
        base = 'collect-' + profile
        cases = []
        for index in range(248):
            case = 'case-' + str(index).zfill(3)
            source = put((base, 'result', case, 'source.ox'), b'synthetic source')
            request = put((base, 'result', case, 'request.json'), {'source': source})
            row = {'case_id': case, 'request': request, 'raw': put((base, 'result', case, 'raw.json'), {'synthetic': True}),
                   'stdout': put((base, 'result', case, 'stdout.txt'), b''), 'stderr': put((base, 'result', case, 'stderr.txt'), b'')}
            put((base, 'result', case, 'receipt.json'), row); cases.append(row)
        manifest = put((base, 'result', 'execution-manifest.json'), {'case_count': 248, 'observation_count': 319, 'case_receipts': cases,
                       'authority_checkpoint': session, 'build_receipt': builds[0], 'driver': records[str(root / 'helpers/run.py')],
                       'normalizer': records[str(root / 'helpers/parse_debug.py')], 'observations': put((base, 'result', 'observations.jsonl'), b'')})
        for name in ('test-roster.stdout', 'test-roster.stderr'): put((base, 'result', name), b'')
        put((base, 'portable-collection.json'), {'session': session, 'profile': profile, 'contract_dir': str(root.parent / 'contracts'), 'manifest': manifest, 'invocation': process(base)})
        base = 'passivity-' + profile
        cases = []
        for name in ('original', 'project', 'malformed'):
            source = put((base, 'result', name + '.ox'), b'synthetic source')
            receipts = [{key: put((base, 'result', name + '-' + role, filename), b'{}' if key == 'raw' else b'')
                         for key, filename in (('raw', 'raw.json'), ('stdout', 'stdout.txt'), ('stderr', 'stderr.txt'))} for role in ('instrumented', 'control')]
            cases.append({'source': source, 'receipts': receipts})
        report = put((base, 'result', 'report.json'), {'results': cases, 'instrumented': builds[0], 'control': builds[1], 'authority_checkpoint': session})
        put((base, 'portable-passivity.json'), {'session': session, 'profile': profile, 'invocation': process(base), 'report': report})
    controls = put(('u8-policy-controls', 'receipt.json'), {'cases': [], 'scope': 'SYNTHETIC_TRANSPORT_CONTROL_ONLY'})
    result_value = {'status': 'pass', 'session': session, 'scope': 'SYNTHETIC_TRANSPORT_CONTROL_ONLY', 'u8_policy_controls': controls}
    result = put(('comparison.json',), result_value, included=False)
    tail = [str(REPO / q.PARSER / 'portable.py'), 'compare', '--session', session['path'], '--contract-dir', str(root.parent / 'contracts')]
    streams = {'stdout': put(root.parent / 'stdout', result_value, included=False), 'stderr': put(root.parent / 'stderr', b'', included=False)}
    command = put(root.parent / 'command.json', {'name': '13-parser-comparison', 'status': 0, 'timed_out': False, 'stream_limit_exceeded': False,
                  'argv': [sys.executable, '-B', *tail], 'cwd': str(root.parent), 'started_ns': 20, 'completed_ns': 30,
                  'streams': streams, 'stdout_sha256': streams['stdout']['sha256'], 'stderr_sha256': streams['stderr']['sha256']}, included=False)
    before = [records[path] for path in sorted(records)]
    return {'schema': 'oxid-unit4-parser-comparison-seal-v2', 'status': 'pass', 'before': before, 'after': copy.deepcopy(before),
            'full_archive_only': omitted, 'comparison': result, 'command': command, 'argv_tail': tail,
            'before_finished_ns': 10, 'after_started_ns': 40, 'after_finished_ns': 50}, data


class ComparisonSealControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.seal, self.data = synthetic_seal_fixture(self.root)

    def tearDown(self): self.temp.cleanup()

    def resolve(self, bound):
        raw = self.data[bound['path']]
        q.need(len(raw) == bound['bytes'] and q.sha(raw) == bound['sha256'], 'synthetic transport identity changed')
        return raw

    def raw_record(self):
        return next(row for row in self.seal['before'] if '/case-000/raw.json' in row['path'].replace('\\', '/'))

    def test_exact_complete_comparison_seal(self):
        report = verify_parser_seal(self.seal, self.resolve)
        self.assertEqual(len(report['required_binaries']), 4)

    def candidate_record(self, control=False):
        directory = 'control-source' if control else 'source'
        return next(row for row in self.seal['before']
                    if Path(row['path']).relative_to(self.root / 'parser').parts == (directory, 'candidate-source-manifest.json'))

    def compact_reader(self):
        destination = self.root / 'capsule'
        capsule = Capsule(destination)
        omitted = {row['path'] for row in self.seal['full_archive_only']}
        # Use the production classification and actual transport, not an identity-only resolver.
        self.assertEqual(omitted, {row['path'] for row in self.seal['before'] if parser_full_only(row, self.root / 'parser')})
        for name, raw in self.data.items():
            if name not in omitted:
                path = Path(name); path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(raw)
                capsule.add(path, 'synthetic-parser-control')
        capsule.finish({'status': 'pass', 'scope': 'synthetic transport control; no semantic qualification'})
        return ReadCapsule(destination)

    def test_actual_compact_reader_retains_all_generated_metadata(self):
        reader = self.compact_reader()
        metadata = ['session.json', 'current-authority.json', 'historical-authority.json', 'current-source-manifest.json']
        for role in ('prepare', 'prepare-control'):
            metadata += [role + '/' + name for name in ('historical-candidate-source-manifest.json', 'historical-overlay-manifest.json')]
        for role in ('source', 'control-source'):
            metadata += [role + '/' + name for name in ('candidate-source-manifest.json', 'overlay-manifest.json', 'observer-source-manifest.json')]
        for name in metadata:
            bound = reader.named(self.root / 'parser' / name)
            self.assertEqual(reader.raw(bound), self.data[bound['path']])
        report = verify_parser_seal(self.seal, reader.raw)
        self.assertEqual(report['full_archive_only'], 1060)
        self.assertEqual(len(metadata), 14)

    def test_current_candidate_missing_from_actual_compact_reader(self):
        for control in (False, True):
            with self.subTest(control=control):
                reader = self.compact_reader()
                bound = self.candidate_record(control)
                entry = reader.mapping[bound['path']]
                (reader.root / entry['member']).unlink()
                manifest = q.read(reader.root / 'capsule.json')
                manifest['members'] = [row for row in manifest['members'] if row['path'] != bound['path']]
                q.save(reader.root / 'capsule.json', manifest)
                reader = ReadCapsule(reader.root)
                with self.assertRaises((q.Reject, KeyError)):
                    verify_parser_seal(self.seal, reader.raw)
                shutil.rmtree(reader.root)

    def test_current_candidate_substitution_in_actual_compact_reader(self):
        for control in (False, True):
            with self.subTest(control=control):
                reader = self.compact_reader()
                bound = self.candidate_record(control)
                entry = reader.mapping[bound['path']]
                path = reader.root / entry['member']; path.write_bytes(path.read_bytes() + b'\n')
                manifest = q.read(reader.root / 'capsule.json')
                row = next(row for row in manifest['members'] if row['path'] == bound['path'])
                row.update({key: q.identity(path)[key] for key in ('bytes', 'sha256')})
                q.save(reader.root / 'capsule.json', manifest)
                reader = ReadCapsule(reader.root)
                with self.assertRaisesRegex(q.Reject, 'stale/altered sealed receipt'):
                    verify_parser_seal(self.seal, reader.raw)
                shutil.rmtree(reader.root)

    def test_coherently_rehashed_current_candidate_is_rejected(self):
        for control in (False, True):
            with self.subTest(control=control):
                self.seal, self.data = synthetic_seal_fixture(self.root)
                record = self.candidate_record(control)
                value = q.loads(self.data[record['path']]); value['files'] = value['files'][:-1]
                self.data[record['path']] = q.canonical(value)
                def rebind(value):
                    if isinstance(value, list):
                        return [rebind(item) for item in value]
                    if isinstance(value, dict):
                        value = {key: rebind(item) for key, item in value.items()}
                        if {'path', 'bytes', 'sha256'} <= set(value) and value['path'] in self.data:
                            raw = self.data[value['path']]; value.update(bytes=len(raw), sha256=q.sha(raw))
                        if 'streams' in value:
                            for name, stream in value['streams'].items():
                                value[name + '_sha256'] = stream['sha256']
                        return value
                    return value
                # Rehash the complete acyclic receipt graph, including comparison command streams.
                for _ in range(20):
                    changed = False
                    for name, raw in list(self.data.items()):
                        try: old = q.loads(raw)
                        except (ValueError, UnicodeError): continue
                        new = rebind(old)
                        if old != new: self.data[name] = q.canonical(new); changed = True
                    if not changed: break
                self.assertFalse(changed, 'synthetic receipt graph did not stabilize')
                self.seal = rebind(self.seal)
                reader = self.compact_reader()
                with self.assertRaisesRegex(q.Reject, 'current candidate transition binding'):
                    verify_parser_seal(self.seal, reader.raw)
                shutil.rmtree(reader.root)

    def test_post_comparison_raw_mutation(self):
        self.data[self.raw_record()['path']] = b'changed observation'
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_stale_comparison_result(self):
        self.data[self.seal['comparison']['path']] = b'stale result'
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_changed_during_comparison(self):
        self.seal['after'][0]['sha256'] = '0' * 64
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_failed_comparison_command(self):
        record = q.loads(self.data[self.seal['command']['path']]); record['status'] = 7
        raw = q.canonical(record); self.data[self.seal['command']['path']] = raw
        self.seal['command'].update(bytes=len(raw), sha256=q.sha(raw))
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_arbitrary_one_member_seal_rejected(self):
        self.seal['before'] = [self.raw_record()]; self.seal['after'] = copy.deepcopy(self.seal['before']); self.seal['full_archive_only'] = []
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_raw_cannot_be_omitted_even_after_rehash(self):
        row = self.raw_record(); self.seal['full_archive_only'].append(copy.deepcopy(row))
        self.data[row['path']] = b'altered raw hidden by omission'
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_omitted_identity_must_equal_seal(self):
        self.seal['full_archive_only'][0] = {**self.seal['full_archive_only'][0], 'sha256': '0' * 64}
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_binary_must_be_in_complete_seal(self):
        binary = next(row for row in self.seal['before'] if '/target/' in row['path'].replace('\\', '/'))
        for name in ('before', 'after', 'full_archive_only'):
            self.seal[name] = [row for row in self.seal[name] if row['path'] != binary['path']]
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_comparison_ordering(self):
        self.seal['before_finished_ns'] = 21
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)

    def test_generated_helper_manifest_cannot_be_omitted(self):
        record = next(row for row in self.seal['before'] if row['path'].endswith('observer-source-manifest.json'))
        self.seal['full_archive_only'].append(copy.deepcopy(record))
        with self.assertRaises(q.Reject): verify_parser_seal(self.seal, self.resolve)


    def test_retained_transition_metadata_cannot_be_omitted(self):
        for suffix in ('current-authority.json', 'historical-authority.json', 'current-source-manifest.json',
                       'prepare/historical-candidate-source-manifest.json', 'prepare/historical-overlay-manifest.json',
                       'prepare-control/historical-candidate-source-manifest.json', 'prepare-control/historical-overlay-manifest.json',
                       'source/candidate-source-manifest.json', 'control-source/candidate-source-manifest.json'):
            with self.subTest(member=suffix):
                self.seal, self.data = synthetic_seal_fixture(self.root)
                row = next(row for row in self.seal['before'] if row['path'].replace('\\', '/').endswith('/' + suffix))
                self.seal['full_archive_only'].append(copy.deepcopy(row))
                with self.assertRaisesRegex(q.Reject, 'raw parser input mislabeled'):
                    verify_parser_seal(self.seal, self.resolve)

    def test_coherently_rehashed_transition_metadata_is_rejected(self):
        for key in ('transition_authority', 'historical_authority', 'current_source_manifest', 'historical_candidate', 'historical_overlay'):
            with self.subTest(key=key):
                self.seal, self.data = synthetic_seal_fixture(self.root)
                comparison = q.loads(self.data[self.seal['comparison']['path']])
                session_binding = comparison['session']
                session = q.loads(self.data[session_binding['path']])
                record = session['transitions'][0][key] if key.startswith('historical_') and key != 'historical_authority' else session[key]
                raw = self.data[record['path']]
                if key == 'historical_overlay':
                    value = q.loads(raw); value['files'] = value['files'][:-1]; raw = q.canonical(value)
                else:
                    raw += b'\n'
                def rebind(record, raw):
                    self.data[record['path']] = raw
                    record.update(bytes=len(raw), sha256=q.sha(raw))
                    for row in self.seal['before']:
                        if row['path'] == record['path']: row.update(record)
                rebind(record, raw)
                rebind(session_binding, q.canonical(session))
                rebind(self.seal['comparison'], q.canonical(comparison))
                self.seal['after'] = copy.deepcopy(self.seal['before'])
                with self.assertRaisesRegex(q.Reject, 'parser transition metadata'):
                    verify_parser_seal(self.seal, self.resolve)


class ParserPreparationBoundaryControls(unittest.TestCase):
    def test_shared_preparation_calls_actual_current_parser(self):
        calls = []
        args = SimpleNamespace(historical_repo='/explicit/historical-parser')
        driver = SimpleNamespace(stage=lambda *values: calls.append(values))
        with patch.object(q, 'git', return_value=q.HISTORICAL_HEAD):
            gate.prepare_parser(args, REPO, Path('/fresh/qualification'), driver)
        self.assertEqual(calls, [('08-parser-prepare', [REPO / q.PARSER / 'portable.py', 'prepare',
                         '--repo', Path(args.historical_repo).resolve(), '--checkout', REPO,
                         '--output', Path('/fresh/qualification/parser')], 300)])

    def test_missing_historical_checkout_cannot_prepare(self):
        with self.assertRaisesRegex(q.Reject, 'explicit historical checkout'):
            gate.prepare_parser(SimpleNamespace(historical_repo=None), REPO, Path('/fresh'), None)

    def test_failed_parser_tail_is_bounded_escaped_and_preserves_full_receipt(self):
        from contextlib import redirect_stderr
        payload = 'prefix ' * 500 + '\n\x1b[31m::error:: marker\n::stop-commands::token\n##[error] end\n'
        for stage in ('08-parser-prepare', 'other-controlled-stage'):
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                script = root / 'failure.py'
                script.write_text('import sys\nsys.stderr.buffer.write(' + repr(payload.encode()) + ')\nraise SystemExit(7)\n')
                provenance = {'checkout_head': 'a' * 40, 'event_sha': 'b' * 40}
                runtime = q.public_modules(REPO)[1]
                driver = gate.Driver(REPO, root, provenance, runtime)
                displayed = io.StringIO()
                with patch.object(q, 'admit', return_value=provenance), redirect_stderr(displayed):
                    with self.assertRaises(q.Reject): driver.stage(stage, [script], 10)
                receipt = q.read(root / 'commands' / stage / 'receipt.json')
                self.assertEqual(receipt['status'], 7)
                self.assertEqual((root / 'commands' / stage / 'stderr').read_text(), payload)
                self.assertEqual(receipt['streams']['stderr']['sha256'], q.sha(payload.encode()))
                text = displayed.getvalue()
                if stage == '08-parser-prepare':
                    prefix = 'Unit4 parser preparation diagnostic: '
                    self.assertTrue(text.startswith(prefix))
                    value = json.loads(text[len(prefix):])
                    self.assertEqual(value['tail_bytes'], 2048)
                    self.assertEqual(value['stderr_bytes'], len(payload.encode()))
                    self.assertIs(value['truncated'], True)
                    self.assertEqual(value['stderr_tail'], payload.encode()[-2048:].decode())
                    for forbidden in ('\x1b', '::error::', '::stop-commands::', '##[error]'):
                        self.assertNotIn(forbidden, text)
                    self.assertEqual(text.count('\n'), 1)
                else:
                    self.assertEqual(text, '')

    def test_historical_parser_session_cannot_enter_current_join(self):
        session = {'checkout': {'head': 'a' * 40, 'tree': 'b' * 40,
                   'historical_source_equivalent': True, 'current_source_bound': False}}
        result = {'status': 'pass', 'issues': [], 'session': 'session'}
        capsule = SimpleNamespace(json=lambda record: result if record == 'result' else session)
        plan = {'provenance': {'checkout_head': 'a' * 40, 'checkout_tree': 'b' * 40}}
        with self.assertRaisesRegex(q.Reject, 'parser current checkout identity'):
            join.parser_records(capsule, REPO, None, plan, {'comparison': 'result'})

    def current_source_session(self):
        source = q.read(REPO / q.SOURCE / 'current-source.json')
        compiler = [row for row in source['files'] if row['path'].startswith(('src/', 'native/'))
                    or row['path'] in ('Cargo.toml', 'Cargo.lock', 'build.rs')]
        self.assertEqual(len(compiler), 278)
        return {'root': '/synthetic/current-parser',
                'host': {'os': 'linux', 'architecture': 'x86_64', 'python_pointer_width': 64},
                'checkout': {'head': 'a' * 40, 'tree': 'b' * 40,
                             'historical_source_equivalent': False, 'current_source_bound': True,
                             'compiler_files': compiler,
                             'current_source_manifest_sha256': q.CURRENT_SHA,
                             'reviewed_source_head': source['reviewed_source_head'],
                             'source_only_tree': source['source_only_tree']},
                'current_source_manifest': {'sha256': q.CURRENT_SHA},
                'authority_sha256': '1' * 64}

    def check_current_source_boundary(self, session, expected_rejection):
        # An intentionally stale comparator authority stops after source admission;
        # this bounded control makes no compiler execution or semantic claim.
        result = {'status': 'pass', 'issues': [], 'session': 'session',
                  'portable_authority_sha256': '0' * 64}
        capsule = SimpleNamespace(json=lambda record: result if record == 'result' else session)
        plan = {'provenance': {'checkout_head': 'a' * 40, 'checkout_tree': 'b' * 40}}
        with self.assertRaisesRegex(q.Reject, expected_rejection):
            join.parser_records(capsule, REPO, None, plan, {'comparison': 'result'})

    def test_exact_current_compiler_map_reaches_comparator_authority_check(self):
        self.check_current_source_boundary(self.current_source_session(), 'stale parser adapter/authority')

    def test_current_compiler_map_and_checkpoint_mutations_reject(self):
        for mutation in ('missing', 'extra', 'path', 'bytes', 'hash', 'order',
                         'manifest', 'reviewed_source_head', 'source_only_tree'):
            with self.subTest(mutation=mutation):
                session = self.current_source_session()
                checkout = session['checkout']
                compiler = checkout['compiler_files']
                if mutation == 'missing': compiler.pop()
                elif mutation == 'extra': compiler.append(copy.deepcopy(compiler[-1]))
                elif mutation == 'path': compiler[-1]['path'] = 'src/unapproved.rs'
                elif mutation == 'bytes': compiler[-1]['bytes'] += 1
                elif mutation == 'hash': compiler[-1]['sha256'] = '0' * 64
                elif mutation == 'order': compiler.reverse()
                elif mutation == 'manifest':
                    checkout['current_source_manifest_sha256'] = '0' * 64
                    session['current_source_manifest']['sha256'] = '0' * 64
                else: checkout[mutation] = '0' * 40
                self.check_current_source_boundary(session, 'parser exact current source map/checkpoint')


class EnumProjectionTransportControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.adapter = q.module('_unit4_enum_transport_controls', REPO / q.PARSER / 'portable.py')
        cls.authority = cls.adapter.authority()

    def test_exact_projection_is_rederived_from_raw_normalized_rows(self):
        rows = [{"ast": None, "diagnostics": [{"code": "E0400"}], "executed": True}]
        original = copy.deepcopy(rows)
        _, receipt = self.adapter.project_enum_observations(self.authority, rows)
        self.assertEqual(join.verify_enum_projection(self.adapter, self.authority, rows, receipt), receipt)
        self.assertEqual(rows, original)

    def test_changed_reordered_missing_or_extra_projection_receipt_rejects(self):
        rows = [{"ast": None, "diagnostics": [{"code": "E0400"}], "executed": True}]
        _, receipt = self.adapter.project_enum_observations(self.authority, rows)
        for mutation in ('hash', 'count', 'indices', 'adapter', 'missing', 'extra'):
            altered = copy.deepcopy(receipt)
            if mutation == 'hash': altered['original_observations_canonical_sha256'] = '0' * 64
            elif mutation == 'count': altered['observations'] += 1
            elif mutation == 'indices': altered['changed_row_indices'] = [0]
            elif mutation == 'adapter': altered['adapter_canonical_sha256'] = '0' * 64
            elif mutation == 'missing': altered.pop('restored_observations_canonical_sha256')
            else: altered['unapproved'] = True
            with self.subTest(mutation=mutation), self.assertRaisesRegex(q.Reject, 'projection receipt differs'):
                join.verify_enum_projection(self.adapter, self.authority, rows, altered)
        changed = copy.deepcopy(rows)
        changed[0]['diagnostics'][0]['code'] = 'E9999'
        with self.assertRaisesRegex(q.Reject, 'projection receipt differs'):
            join.verify_enum_projection(self.adapter, self.authority, changed, receipt)

    def test_projection_refuses_nonhistorical_ast_in_transport(self):
        rows = [{"ast": {"canonical": {"tag": "Program", "enums": ["unapproved"]}}}]
        with self.assertRaisesRegex(q.Reject, 'structural projection rejected'):
            join.verify_enum_projection(self.adapter, self.authority, rows, {})


class EnumSemanticReceiptTransportControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temp.cleanup)
        cls.root = Path(cls.temp.name).resolve()
        transport = q.module('_unit4_semantic_receipt_transport', REPO / q.TRANSPORT / 'transport.py')
        manifest = transport.materialize(REPO / q.TRANSPORT, cls.root / 'contracts')
        mapping = {row['logical_path']: str(cls.root / 'contracts' / row['archive_path']) for row in manifest['members']}
        public_root = Path(mapping[manifest['active_contracts']['public']['logical_path']]).parent
        cls.c, _, cls.collector, _, cls.Predecessors = q.public_modules(REPO)
        cls.contracts = cls.c.Contracts(public_root, mapping, REPO / q.AMENDMENT)
        cls.predecessors = cls.Predecessors(cls.contracts, REPO / q.SOURCE / 'current-source.json', REPO / q.SOURCE)
        roster = cls.contracts.roster('predecessors', 'Linux x86_64')
        ids = cls.predecessors.qualified_paths_amendment['cases']
        cls.amended_rows = [row for row in roster if row['scope'] == 'execute' and row['group'] == 'Unit2' and row['case_id'] in ids]
        cls.unaffected = next(row for row in roster if row['scope'] == 'execute' and row['group'] == 'Unit2' and
                              row['case']['kind'] == 'first-diagnostic-only' and row['case_id'] not in ids)
        cls.policy = cls.contracts.tables['public']['new_policy_diagnostic_bounds']

    def process(self, row, historical=False):
        amended = self.predecessors._qualified_paths.rows.get(row['case_id'])
        if amended:
            first = amended['old_public_expected' if historical else 'current_public_expected']['first_diagnostic']
        else:
            first = self.predecessors.rows['Unit2', row['case_id']]['expected']['first_diagnostic']
        diagnostic = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'diagnostic',
                      'severity': 'error', **first, 'message': 'arbitrary valid diagnostic text',
                      'primary': None, 'secondary': [], 'notes': []}
        summary = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'check-summary',
                   'success': False, 'errors': 1, 'functions': None}
        stdout = ''.join(json.dumps(value) + '\n' for value in (diagnostic, summary))
        return {'status': 1, 'timed_out': False, 'stdout': stdout, 'stderr': '',
                'stdout_sha256': q.sha(stdout.encode()), 'stderr_sha256': q.sha(b'')}

    def setUp(self):
        self.rows = copy.deepcopy(self.amended_rows + [self.unaffected])
        self.values = []
        for row in self.rows:
            value = {**self.c.receipt_identity(row), 'executed': True, 'status': 'pass', 'process': self.process(row)}
            value['projection_kind'] = row['case']['kind']
            for key in ('expected_authority_line_sha256', 'expected_projection_sha256', 'source_line_sha256'):
                if key in row['case']: value[key] = row['case'][key]
            amendment = self.predecessors.qualified_paths_comparison(row, value['process'])
            if amendment is not None:
                value['qualified_paths_comparison'] = amendment
            self.values.append(value)
        self.observations = {'path': '/synthetic/public/predecessors/observations.jsonl.gz', 'bytes': 1, 'sha256': 'a' * 64}
        self.authority_binding = {'path': '/synthetic/public/qualified-paths-amendment.json', 'bytes': 1, 'sha256': 'b' * 64}
        self.history_binding = {'path': '/synthetic/public/predecessors/historical-comparison.json', 'bytes': 1, 'sha256': 'c' * 64}
        self.authority = copy.deepcopy(self.predecessors.qualified_paths_amendment)
        self.history = self.collector.predecessor_history(self.rows, self.values, self.predecessors, self.policy, self.observations)
        self.report = self.collector.predecessor_report_fields(self.predecessors, self.history, self.authority_binding, self.history_binding)

    def verify(self):
        return join.verify_qualified_paths_receipts(self.collector, self.predecessors, self.rows, self.values,
                    self.policy, self.authority, self.report, self.history, self.observations,
                    self.authority_binding, self.history_binding)

    def test_eight_amended_tuples_replay_both_semantics_without_raw_changes(self):
        before = copy.deepcopy((self.rows, self.values, self.predecessors.rows))
        self.assertEqual(len(self.amended_rows), 8)
        self.verify()
        self.assertEqual(len(self.history['mismatches']), 8)
        self.assertEqual(len(self.history['qualified_paths_comparisons']), 8)
        self.assertEqual(self.history['executed'], 9)
        for row, value in zip(self.rows, self.values):
            self.predecessors.compare(row, value['process'], self.policy)
        self.assertEqual((self.rows, self.values, self.predecessors.rows), before)
        self.assertNotIn('qualified_paths_comparison', self.values[-1])

    def test_collector_replay_keeps_frozen_expectation_and_source_identities(self):
        # Only the process/semantic seam is under test; build/native checks have separate controls.
        row, value = self.rows[0], self.values[0]
        sources = self.predecessors.rows['Unit2', row['case_id']]['sources']
        with patch.object(self.collector, 'check_common'):
            self.collector.core_compare('predecessors', row, value, {}, self.contracts, sources, {}, self.predecessors)
            for key in ('expected_authority_line_sha256', 'expected_projection_sha256', 'source_line_sha256'):
                altered = copy.deepcopy(value)
                altered[key] = '0' * 64
                with self.subTest(key=key), self.assertRaises(self.c.Reject):
                    self.collector.core_compare('predecessors', row, altered, {}, self.contracts, sources, {}, self.predecessors)

    def test_missing_extra_and_forged_per_tuple_comparisons_reject(self):
        for mutation in ('missing', 'extra', 'forged-current', 'forged-historical', 'forged-observed',
                         'extra-field', 'namespace', 'boolean-status', 'unaffected', 'duplicate', 'reorder'):
            with self.subTest(mutation=mutation):
                self.setUp()
                value = self.values[0]
                comparison = value['qualified_paths_comparison']
                if mutation == 'missing': value.pop('qualified_paths_comparison')
                elif mutation == 'extra': self.values[-1]['qualified_paths_comparison'] = comparison
                elif mutation == 'forged-current': comparison['current']['expected']['first_diagnostic']['code'] = 'E9999'
                elif mutation == 'forged-historical': comparison['historical']['status'] = 'match'
                elif mutation == 'forged-observed': comparison['observed_projection']['first_diagnostic']['stage'] = 'parse'
                elif mutation == 'extra-field': comparison['unapproved'] = True
                elif mutation == 'namespace': value['qualified_paths_forgery'] = True
                elif mutation == 'boolean-status': comparison['observed_projection']['status'] = True
                elif mutation == 'unaffected': self.rows[0]['case']['id'] = self.unaffected['case_id']
                elif mutation == 'duplicate': self.values.append(copy.deepcopy(value))
                else: self.values.reverse()
                with self.assertRaises((q.Reject, self.c.Reject, KeyError, ValueError)):
                    self.verify()

    def test_authority_and_report_metadata_reject_even_with_forged_matching_summary(self):
        for mutation in ('missing-authority', 'wrong-authority', 'source', 'execution-source', 'swapped-sources', 'extra-authority', 'missing-report',
                         'wrong-report', 'extra-report', 'authority-binding', 'history-binding', 'membership'):
            with self.subTest(mutation=mutation):
                self.setUp()
                if mutation == 'missing-authority': self.authority = None
                elif mutation == 'wrong-authority': self.authority['identity'] = 'unapproved'
                elif mutation == 'source': self.authority['source_manifest']['sha256'] = '0' * 64
                elif mutation == 'execution-source': self.authority['execution_source_manifest']['sha256'] = '0' * 64
                elif mutation == 'swapped-sources':
                    self.authority['source_manifest'], self.authority['execution_source_manifest'] = self.authority['execution_source_manifest'], self.authority['source_manifest']
                elif mutation == 'extra-authority': self.authority['unapproved'] = True
                elif mutation == 'missing-report': self.report.pop('qualified_paths_amendment')
                elif mutation == 'wrong-report': self.report['comparison_basis'] = 'unchanged'
                elif mutation == 'extra-report': self.report['qualified_paths_unapproved'] = True
                elif mutation == 'authority-binding': self.report['qualified_paths_amendment_receipt'] = {**self.authority_binding, 'sha256': '0' * 64}
                elif mutation == 'history-binding': self.report['historical_comparison'] = {**self.history_binding, 'sha256': '0' * 64}
                else: self.report['qualified_paths_amended_keys'].pop()
                if mutation in ('wrong-authority', 'source', 'execution-source', 'swapped-sources', 'extra-authority'):
                    self.report['qualified_paths_amendment'] = copy.deepcopy(self.authority)
                with self.assertRaises(q.Reject): self.verify()

    def test_historical_report_and_rehashed_raw_observation_changes_reject(self):
        for mutation in ('missing', 'extra', 'forged', 'raw', 'old-output', 'extra-unaffected-mismatch'):
            with self.subTest(mutation=mutation):
                self.setUp()
                if mutation == 'missing': self.history['mismatches'].pop()
                elif mutation == 'extra': self.history['mismatches'].append(copy.deepcopy(self.history['mismatches'][0]))
                elif mutation == 'forged': self.history['qualified_paths_comparisons'][0]['comparison']['historical']['status'] = 'match'
                else:
                    index = -1 if mutation == 'extra-unaffected-mismatch' else 0
                    self.values[index]['process'] = self.process(self.rows[index], historical=True)
                    process = self.values[index]['process']
                    if mutation != 'old-output':
                        decoded = [json.loads(line) for line in process['stdout'].splitlines()]
                        decoded[0]['code'] = 'E9999'
                        process['stdout'] = ''.join(json.dumps(value) + '\n' for value in decoded)
                        process['stdout_sha256'] = q.sha(process['stdout'].encode())
                    # Rehashing transport and regenerating attacker-controlled metadata cannot authorize a new outcome.
                    comparison = self.predecessors.qualified_paths_comparison(self.rows[index], process)
                    if comparison is not None: self.values[index]['qualified_paths_comparison'] = comparison
                    self.history = self.collector.predecessor_history(self.rows, self.values, self.predecessors, self.policy, self.observations)
                    self.report = self.collector.predecessor_report_fields(self.predecessors, self.history, self.authority_binding, self.history_binding)
                with self.assertRaises(q.Reject): self.verify()

    def test_host_exclusions_cannot_gain_amendment_metadata(self):
        self.rows = [row for row in self.contracts.roster('predecessors', 'Windows x86_64')
                     if row['group'] == 'Unit2' and row['case_id'] in self.authority['cases']]
        self.values = [self.collector.skipped(row) for row in self.rows]
        self.history = self.collector.predecessor_history(self.rows, self.values, self.predecessors, self.policy, self.observations)
        self.report = self.collector.predecessor_report_fields(self.predecessors, self.history, self.authority_binding, self.history_binding)
        self.verify()
        self.assertEqual(self.history['qualified_paths_comparisons'], [])
        self.values[0]['qualified_paths_comparison'] = {}
        with self.assertRaises(q.Reject): self.verify()

    def test_compact_reader_retains_named_authority_and_historical_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            capsule = Capsule(root / 'capsule')
            source = REPO / q.SOURCE / 'current-source.json'
            authority_path, history_path = root / 'qualified-paths-amendment.json', root / 'historical-comparison.json'
            q.save(authority_path, self.authority); q.save(history_path, self.history)
            for path in (source, authority_path, history_path): capsule.add(path, 'semantic-control')
            capsule.finish({'status': 'pass'})
            reader = ReadCapsule(capsule.root)
            admitted = self.Predecessors(self.contracts, reader.path(q.identity(source)), REPO / q.SOURCE)
            self.assertEqual(reader.json(q.identity(authority_path)), admitted.qualified_paths_amendment)
            self.assertEqual(reader.json(q.identity(history_path)), self.history)
            bound = reader.named(str(history_path))
            reader.path(bound).write_bytes(b'forged historical comparison')
            with self.assertRaises(q.Reject): reader.json(bound)

    def semantic_capsule(self, root, mutation=None):
        public = root / 'public'
        (public / 'predecessors').mkdir(parents=True)
        authority_path = public / 'qualified-paths-amendment.json'
        observations_path = public / 'predecessors/observations.jsonl.gz'
        history_path = public / 'predecessors/historical-comparison.json'
        report_path, main_path = public / 'predecessors/comparison.json', public / 'comparison.json'
        q.save(authority_path, self.authority)
        q.write_rows(observations_path, self.values)
        history = self.collector.predecessor_history(self.rows, self.values, self.predecessors,
                                                    self.policy, self.c.binding(observations_path))
        if mutation is not None and mutation[0] == 'observations': mutation[1](history['observations'])
        q.save(history_path, history)
        report = self.collector.predecessor_report_fields(self.predecessors, history,
                                                          self.c.binding(authority_path), self.c.binding(history_path))
        report_fields = {'authority': 'qualified_paths_amendment_receipt', 'history': 'historical_comparison'}
        if mutation is not None and mutation[0] in report_fields: mutation[1](report[report_fields[mutation[0]]])
        q.save(report_path, report)
        main = {'predecessor_semantic_amendment': self.c.binding(authority_path),
                'sections': {'predecessors': report}}
        if mutation is not None and mutation[0] == 'main': mutation[1](main['predecessor_semantic_amendment'])
        q.save(main_path, main)
        capsule = Capsule(root / 'capsule')
        for path in (authority_path, observations_path, history_path, report_path, main_path):
            capsule.add(path, 'semantic-control')
        capsule.finish({'status': 'pass'})
        return ReadCapsule(capsule.root)

    def verify_semantic_capsule(self, capsule, root):
        main = capsule.json(capsule.named(str(root / 'public/comparison.json')))
        values = q.rows(capsule.path(capsule.named(str(root / 'public/predecessors/observations.jsonl.gz'))))
        return join.verify_qualified_paths_capsule(capsule, str(root), main, self.collector,
                                                  self.predecessors, self.rows, values, self.policy)

    def test_public_semantic_join_replays_actual_capsule_receipt_shapes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            capsule = self.semantic_capsule(root)
            authority_path = root / 'public/qualified-paths-amendment.json'
            self.assertEqual(set(self.c.binding(authority_path)), {'path', 'bytes', 'sha256'})
            self.assertEqual(set(capsule.named(str(authority_path))), {'path', 'bytes', 'sha256', 'member', 'role'})
            result = self.verify_semantic_capsule(capsule, root)
            self.assertEqual(result['qualified_paths_amendment_receipt'], self.c.binding(authority_path))
            self.assertEqual(len(result['qualified_paths_amended_keys']), 8)

    def test_public_semantic_join_rejects_mutated_original_receipts_after_transport(self):
        mutations = {'path': lambda row: row.update(path=row['path'] + '.alias'),
                     'bytes': lambda row: row.update(bytes=row['bytes'] + 1),
                     'sha256': lambda row: row.update(sha256='0' * 64),
                     'missing': lambda row: row.pop('sha256'),
                     'extra-member': lambda row: row.update(member='members/00000'),
                     'extra-role': lambda row: row.update(role='semantic-control'),
                     'extra-original': lambda row: row.update(unapproved=True)}
        for target in ('main', 'authority', 'history', 'observations'):
            for field, mutate in mutations.items():
                with self.subTest(target=target, field=field), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory).resolve()
                    capsule = self.semantic_capsule(root, (target, mutate))
                    with self.assertRaises(q.Reject): self.verify_semantic_capsule(capsule, root)

    def test_public_semantic_join_revalidates_transported_member_bytes(self):
        for name in ('qualified-paths-amendment.json', 'predecessors/comparison.json',
                     'predecessors/historical-comparison.json', 'predecessors/observations.jsonl.gz'):
            with self.subTest(member=name), tempfile.TemporaryDirectory() as directory:
                root = Path(directory).resolve()
                capsule = self.semantic_capsule(root)
                main = capsule.json(capsule.named(str(root / 'public/comparison.json')))
                values = q.rows(capsule.path(capsule.named(str(root / 'public/predecessors/observations.jsonl.gz'))))
                path = capsule.path(capsule.named(str(root / 'public' / name)))
                path.write_bytes(path.read_bytes() + b'\n')
                with self.assertRaises(q.Reject):
                    join.verify_qualified_paths_capsule(capsule, str(root), main, self.collector,
                                                       self.predecessors, self.rows, values, self.policy)
                with self.assertRaises(q.Reject): ReadCapsule(capsule.root)

    def test_public_semantic_join_preserves_transported_host_exclusions(self):
        self.rows = [row for row in self.contracts.roster('predecessors', 'Windows x86_64')
                     if row['group'] == 'Unit2' and row['case_id'] in self.authority['cases']]
        self.values = [self.collector.skipped(row) for row in self.rows]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            capsule = self.semantic_capsule(root)
            result = self.verify_semantic_capsule(capsule, root)
            self.assertEqual(result['qualified_paths_amended_keys'], [])


class FinalFailureControls(unittest.TestCase):
    def test_failed_upstream_has_structured_nonpassing_result(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve() / 'join'
            args = SimpleNamespace(repo=REPO, expected_head='a' * 40, event_sha='b' * 40, output=root,
                                   artifacts=Path(temp).resolve() / 'missing', job_results=json.dumps({'unit4-linux': {'result': 'failure'}, 'unit4-portable-hosts': {'result': 'success'}}))
            with patch.object(q, 'admit', return_value={'checkout_head': 'a' * 40}):
                with self.assertRaises(q.Reject): join.join(args)
            receipt = q.read(root / 'comparison.json')
            self.assertEqual(receipt['status'], 'fail')
            self.assertFalse(receipt['global_host_qualification'])
            self.assertEqual(receipt['upstream_job_results']['unit4-linux'], 'failure')
            self.assertEqual(receipt['expected_head'], 'a' * 40)

    def test_occupied_join_output_is_preserved(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            prior = root / 'comparison.json'; prior.write_bytes(b'preserve existing evidence')
            args = SimpleNamespace(output=root)
            with self.assertRaises(q.Reject): join.join(args)
            self.assertEqual(prior.read_bytes(), b'preserve existing evidence')


if __name__ == '__main__': unittest.main(verbosity=2)
