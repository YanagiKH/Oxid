"""Closed current Unit2 control-artifact retention; no compiler execution."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
WORKFLOW = REPO / '.github/workflows/ci.yml'
PREFIX = '${{ runner.temp }}/typed-project-unit2-results/observer-adapter-controls/'
PATTERNS = tuple(PREFIX + '*.' + suffix for suffix in ('json', 'stdout', 'stderr'))
MEMBERS = frozenset(
    [profile + '-' + kind + '.json' for profile in ('debug', 'release') for kind in ('commands', 'receipt')]
    + [profile + '-' + phase + '.' + stream for profile in ('debug', 'release')
       for phase in ('list', 'run') for stream in ('stdout', 'stderr')])


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def retention_patterns(workflow):
    jobs = re.findall(r'^  quality-and-repository:\n(.*?)(?=^  [A-Za-z0-9_-]+:|\Z)', workflow, re.M | re.S)
    require(len(jobs) == 1, 'exact quality job required')
    steps = re.findall(r'^      - name: Retain compact typed-project qualification evidence\n(.*?)(?=^      - |\Z)', jobs[0], re.M | re.S)
    require(len(steps) == 1, 'exact Unit2 upload step required')
    step = steps[0]
    for expected in ('        if: always()\n', '        uses: actions/upload-artifact@v4\n',
                     '          name: typed-project-unit2-evidence\n'):
        require(step.count(expected) == 1, 'Unit2 evidence must upload on every terminal state')
    blocks = re.findall(r'^          path: \|\n((?:            [^\n]+\n)+)', step, re.M)
    require(len(blocks) == 1, 'exact Unit2 path list required')
    paths = [line.strip() for line in blocks[0].splitlines()]
    retained = [path for path in paths if 'observer-adapter-controls' in path]
    require(len(retained) == 3 and set(retained) == set(PATTERNS), 'exact three observer-control retention globs required')
    return tuple(retained)


def selected_members(directory, patterns):
    require(all(pattern.startswith(PREFIX) for pattern in patterns), 'unexpected observer-control root')
    return {p.relative_to(directory).as_posix() for pattern in patterns
            for p in directory.glob(pattern[len(PREFIX):]) if p.is_file()}


def check_members(directory):
    require(directory.is_dir() and not directory.is_symlink(), 'missing or aliased observer controls')
    entries = list(directory.rglob('*'))
    require(all(p.is_file() and not p.is_symlink() for p in entries), 'nonregular observer control member')
    require({p.relative_to(directory).as_posix() for p in entries} == MEMBERS, 'observer control member roster differs')


def verify_actual_bodies(unit2_root, workflow):
    """Validate retention of already executed controls; never rerun or infer execution."""
    patterns = retention_patterns(workflow)
    directory = unit2_root / 'observer-adapter-controls'
    check_members(directory)
    require(selected_members(directory, patterns) == MEMBERS, 'upload omits observer control bodies')
    outer = json.loads((unit2_root / 'result.json').read_bytes())
    require(outer['status'] == 'passed-current-unit2' and outer['observer_control_tests_per_profile'] == 4,
            'completed current Unit2 result required for full-roster audit')
    rows = outer['observer_control_receipts']
    expected = {'observer-adapter-controls/' + p + '-receipt.json' for p in ('debug', 'release')}
    require(len(rows) == 2 and {r['path'] for r in rows} == expected, 'wrong observer receipt roster')

    def checked(base, bound, expected_path):
        require(bound['path'] == expected_path, 'wrong observer control artifact path')
        data = (base / expected_path).read_bytes()
        require(len(data) == bound['bytes'] and digest(data) == bound['sha256'], 'observer control artifact bytes changed')
        return data

    run = unit2_root / 'unit2/run'
    child_bytes = (run / 'result.json').read_bytes()
    require(digest(child_bytes) == outer['child_result_sha256'], 'original Unit2 result changed')
    child = json.loads(child_bytes)
    require(child['status'] == 'passed' and child['profiles'] == ['debug', 'release'], 'original Unit2 profiles/status differ')
    require(len(child['receipts']) == 2 and {row['profile'] for row in child['receipts']} == {'debug', 'release'},
            'original Unit2 receipt roster differs')
    for profile in ('debug', 'release'):
        name = 'observer-adapter-controls/' + profile + '-receipt.json'
        row = next(r for r in rows if r['path'] == name)
        receipt = json.loads(checked(unit2_root, row, name))
        require(receipt['profile'] == profile and receipt['status'] == 'passed', 'wrong observer control profile/status')
        original_bound = receipt['original_unit2_receipt']
        original_bytes = checked(run, original_bound, profile + '-receipt.json')
        original = json.loads(original_bytes)
        child_bound = next(row for row in child['receipts'] if row['profile'] == profile)
        require(digest(original_bytes) == child_bound['sha256'], 'control receipt differs from original Unit2 result')
        require(original['profile'] == profile and original['status'] == 'passed' and
                original['invocation_id'] == child['invocation_id'], 'original Unit2 profile/invocation differs')
        require(receipt['source_inputs_sha256'] == original['source_inputs_sha256'] ==
                child['source_inputs_sha256'] == outer['current_source_sha256'], 'control/current source identity differs')
        require(original['package_inputs_sha256'] == child['package_inputs_sha256'] ==
                outer['unit2_package_inputs_sha256'], 'original Unit2 package identity differs')
        # These are recorded hosted identities only. Never open either absolute
        # binary path or any source path while auditing a downloaded capsule.
        require(receipt['binary']['path'] == original['binary'] and
                receipt['binary']['sha256'] == original['binary_sha256'], 'control/original binary identity differs')
        assembly = receipt['assembly']
        checked(run, assembly, 'assembly.json')
        originals = [row for row in child['evidence'] if row['path'] == 'assembly.json']
        require(originals == [assembly], 'control/original source assembly differs')
        commands = json.loads(checked(directory, receipt['commands'], profile + '-commands.json'))
        require(len(commands) == 2, 'wrong observer control command count')
        for command, phase in zip(commands, ('list', 'run')):
            require(command['exit_status'] == 0 and command['timed_out'] is False, 'failed observer control command')
            require(command['argv'][0] == receipt['binary']['path'], 'control command uses a different binary')
            for stream in ('stdout', 'stderr'):
                checked(directory, command[stream], profile + '-' + phase + '.' + stream)
    return {'scope': 'retention audit of existing receipts; zero compiler/test executions',
            'members': sorted(MEMBERS), 'files': [
                {'path': name, 'bytes': len(data := (directory / name).read_bytes()), 'sha256': digest(data)}
                for name in sorted(MEMBERS)]}


class Unit2ObserverArtifactRetentionTests(unittest.TestCase):
    def setUp(self):
        self.workflow = WORKFLOW.read_text(encoding='utf-8')

    def test_quality_upload_retains_exact_observer_control_extensions(self):
        self.assertEqual(set(retention_patterns(self.workflow)), set(PATTERNS))

    def test_old_missing_duplicate_broadened_or_success_only_upload_rejects(self):
        for pattern in PATTERNS:
            with self.subTest(missing=pattern), self.assertRaises(ValueError):
                retention_patterns(self.workflow.replace('            ' + pattern + '\n', ''))
        mutations = (
            self.workflow.replace('            ' + PATTERNS[0] + '\n', ('            ' + PATTERNS[0] + '\n') * 2),
            self.workflow.replace(PATTERNS[0], PREFIX + '**'),
            self.workflow.replace('        if: always()\n', '        if: success()\n'),
            self.workflow.replace('            ' + PATTERNS[0] + '\n', '').replace('            ' + PATTERNS[1] + '\n', '').replace('            ' + PATTERNS[2] + '\n', ''),
        )
        for workflow in mutations:
            with self.subTest(workflow_sha=digest(workflow.encode())), self.assertRaises(ValueError):
                retention_patterns(workflow)

    def test_exact_twelve_members_are_selected_and_omission_or_extra_rejects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in MEMBERS:
                (root / name).write_bytes(b'bounded retention fixture\n')
            check_members(root)
            self.assertEqual(selected_members(root, retention_patterns(self.workflow)), MEMBERS)
            self.assertEqual(len(MEMBERS), 12)
            for name in MEMBERS:
                (root / name).unlink()
                with self.subTest(missing=name), self.assertRaisesRegex(ValueError, 'roster differs'):
                    check_members(root)
                (root / name).write_bytes(b'bounded retention fixture\n')
            for name in ('extra.json', 'compiler', 'source.rs', 'extra.stdout'):
                (root / name).write_bytes(b'extra\n')
                with self.subTest(extra=name), self.assertRaisesRegex(ValueError, 'roster differs'):
                    check_members(root)
                (root / name).unlink()

    def synthetic_capsule(self, root):
        directory = root / 'observer-adapter-controls'
        directory.mkdir()
        run = root / 'unit2/run'
        run.mkdir(parents=True)
        def write(base, name, value):
            data = (json.dumps(value, sort_keys=True) + '\n').encode() if not isinstance(value, bytes) else value
            (base / name).write_bytes(data)
            return {'path': name, 'bytes': len(data), 'sha256': digest(data)}
        assembly = write(run, 'assembly.json', {'files': [{'path': 'src/never-opened.rs', 'sha256': 'c' * 64, 'bytes': 10}]})
        receipts, originals = [], []
        for profile in ('debug', 'release'):
            binary = '/nonexistent/hosted/unit2/run/target/' + profile + '/never-opened'
            original = write(run, profile + '-receipt.json', {'profile': profile, 'status': 'passed',
                'invocation_id': 'synthetic-retention-only', 'source_inputs_sha256': 'a' * 64,
                'package_inputs_sha256': 'b' * 64, 'binary': binary, 'binary_sha256': 'd' * 64})
            originals.append({'profile': profile, 'sha256': original['sha256']})
            commands = []
            for phase in ('list', 'run'):
                commands.append({'exit_status': 0, 'timed_out': False, 'argv': [binary, phase],
                    **{stream: write(directory, profile + '-' + phase + '.' + stream, b'synthetic retention stream\n')
                       for stream in ('stdout', 'stderr')}})
            command = write(directory, profile + '-commands.json', commands)
            bound = write(directory, profile + '-receipt.json', {'profile': profile, 'status': 'passed', 'commands': command,
                'original_unit2_receipt': original, 'source_inputs_sha256': 'a' * 64, 'assembly': assembly,
                'binary': {'path': binary, 'sha256': 'd' * 64, 'bytes': 123}})
            receipts.append({**bound, 'path': 'observer-adapter-controls/' + bound['path']})
        child = write(run, 'result.json', {'status': 'passed', 'profiles': ['debug', 'release'],
            'invocation_id': 'synthetic-retention-only', 'source_inputs_sha256': 'a' * 64,
            'package_inputs_sha256': 'b' * 64, 'receipts': originals, 'evidence': [assembly]})
        write(root, 'result.json', {'status': 'passed-current-unit2', 'observer_control_tests_per_profile': 4,
            'observer_control_receipts': receipts, 'child_result_sha256': child['sha256'],
            'current_source_sha256': 'a' * 64, 'unit2_package_inputs_sha256': 'b' * 64})
        return directory

    def test_relocated_receipt_chain_requires_bodies_without_opening_hosted_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = self.synthetic_capsule(root)
            # No source tree or executable exists in this relocated capsule.
            report = verify_actual_bodies(root, self.workflow)
            self.assertEqual(len(report['files']), 12)
            for name in MEMBERS:
                path = directory / name
                original = path.read_bytes()
                path.write_bytes(original + b'changed')
                with self.subTest(changed=name), self.assertRaisesRegex(ValueError, 'artifact bytes changed'):
                    verify_actual_bodies(root, self.workflow)
                path.write_bytes(original)

    def test_coherent_control_rehash_cannot_change_original_binary_or_source_join(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = self.synthetic_capsule(root)
            receipt = directory / 'debug-receipt.json'
            before = receipt.read_bytes()
            outer = json.loads((root / 'result.json').read_bytes())
            for field in ('binary', 'source_inputs_sha256'):
                changed = json.loads(before)
                if field == 'binary':
                    changed[field]['sha256'] = 'e' * 64
                else:
                    changed[field] = 'e' * 64
                data = (json.dumps(changed, sort_keys=True) + '\n').encode()
                receipt.write_bytes(data)
                outer['observer_control_receipts'][0].update(bytes=len(data), sha256=digest(data))
                (root / 'result.json').write_text(json.dumps(outer))
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'identity differs'):
                    verify_actual_bodies(root, self.workflow)
                receipt.write_bytes(before)

    def test_upload_selection_does_not_include_binaries_sources_or_nested_bodies(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in MEMBERS | {'compiler', 'source.rs'}:
                (root / name).write_bytes(b'fixture\n')
            (root / 'nested').mkdir()
            (root / 'nested/extra.json').write_bytes(b'{}\n')
            self.assertEqual(selected_members(root, retention_patterns(self.workflow)), MEMBERS)
            with self.assertRaisesRegex(ValueError, 'nonregular'):
                check_members(root)


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == 'audit':
        parser = argparse.ArgumentParser(description='Audit downloaded Unit2 control bodies; never open hosted binary/source paths')
        parser.add_argument('--unit2-root', type=Path, required=True)
        parser.add_argument('--workflow', type=Path, default=WORKFLOW)
        args = parser.parse_args(sys.argv[2:])
        print(json.dumps(verify_actual_bodies(args.unit2_root, args.workflow.read_text(encoding='utf-8')), indent=2, sort_keys=True))
    else:
        unittest.main()
