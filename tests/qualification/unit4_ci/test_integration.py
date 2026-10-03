#!/usr/bin/env python3
"""Bounded admission/transport controls, never synthetic qualification claims."""
import sys
sys.dont_write_bytecode = True
import copy
import io
import json
from pathlib import Path
import tarfile
import tempfile
import subprocess
import unittest
from types import SimpleNamespace
from unittest.mock import patch
import common as q
import gate
import join
from evidence import Capsule, ReadCapsule, verify_parser_seal

REPO = Path(__file__).resolve().parents[3]


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
            self.assertIn('run: git -c core.autocrlf=false -c core.eol=lf checkout-index --all --force', workflow)


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
    authority = q.read(REPO / q.PARSER / 'authority.json')
    overlays = {kind: put((kind, 'overlay-manifest.json'), {'scope': 'synthetic-control-only'}) for kind in ('source', 'control-source')}
    for directory in ('source', 'control-source'):
        put((directory, 'observer-source-manifest.json'), authority['helper_files'])
    for directory, field in (('source', 'derived_files'), ('control-source', 'control_derived_files')):
        for row in authority[field]:
            record = {'path': str(root / directory / row['path']), 'bytes': row['bytes'], 'sha256': row['sha256']}
            records[record['path']] = record; omitted.append(record)
    for row in authority['helper_files']:
        put(('helpers', row['path']), (REPO / q.PARSER / 'frozen/helpers' / row['path']).read_bytes())
    session = put(('session.json',), {'root': str(root), 'authority_sha256': q.identity(REPO / q.PARSER / 'authority.json')['sha256'],
                  'adapter': q.identity(REPO / q.PARSER / 'portable.py'), 'host': {'python_executable': sys.executable},
                  'overlay': overlays['source'], 'control_overlay': overlays['control-source'],
                  'prepare_invocation': process('prepare'), 'control_prepare_invocation': process('prepare-control')})
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
    result_value = {'status': 'pass', 'session': session, 'scope': 'SYNTHETIC_TRANSPORT_CONTROL_ONLY'}
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
