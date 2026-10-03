#!/usr/bin/env python3
"""Unchanged compact evidence capsules and separately retained full artifacts."""
import sys
sys.dont_write_bytecode = True
import argparse
import os
from pathlib import Path
import shutil
import tarfile
import common as q


def parser_inputs(root):
    """Closed comparison inputs, including actual binaries but excluding caches."""
    root = Path(root)
    binaries = set()
    for profile in q.PROFILES:
        for role in ('build-', 'build-control-'):
            receipt = q.read(root / (role + profile) / 'result/build-receipt.json')
            binaries.add(Path(receipt['binary']['path']))
    files = set(binaries)
    for directory, subdirs, names in os.walk(root):
        subdirs[:] = [name for name in subdirs if name not in ('target', 'rust-bin', 'cargo-home', 'home')]
        for name in names:
            path = Path(directory) / name
            if path == root / 'comparison.json':
                continue
            files.add(path)
    return [q.identity(path) for path in sorted(files, key=str)]


def verify_parser_seal(seal, resolver):
    """Derive and verify the complete parser comparison input closure."""
    q.need(seal['schema'] == 'oxid-unit4-parser-comparison-seal-v2' and seal['status'] == 'pass', 'parser replay seal absent/failed')
    q.need(seal['before'] == seal['after'] and seal['before'], 'parser inputs changed during comparison')
    paths = [row['path'] for row in seal['before']]
    q.need(paths == sorted(set(paths)), 'duplicate/unordered parser replay inputs')
    index = {row['path']: row for row in seal['before']}
    omitted = {row['path']: row for row in seal['full_archive_only']}
    q.need(len(omitted) == len(seal['full_archive_only']) and set(omitted) <= set(index), 'parser omitted identity inventory')
    q.need(all(record == index[path] for path, record in omitted.items()), 'omitted parser identity differs from seal')
    required, allowed_omissions = {}, {}

    def bound(record, omit=False):
        q.need(set(record) == {'path', 'bytes', 'sha256'}, 'parser artifact identity fields')
        q.need(record['path'] in index and index[record['path']] == record, 'consumed parser input absent or changed in seal')
        required[record['path']] = record
        if omit:
            allowed_omissions[record['path']] = record
            return None
        q.need(record['path'] not in omitted, 'raw parser input mislabeled as full-archive-only')
        return resolver(record)

    def read_bound(record):
        return q.loads(bound(record))

    result = q.loads(resolver(seal['comparison']))
    session = read_bound(result['session'])
    root = Path(session['root'])
    q.need(root.is_absolute() and '..' not in root.parts and str(root) == session['root'], 'noncanonical parser session root')
    q.need(result['session']['path'] == str(root / 'session.json'), 'parser session path differs')

    def named(*parts):
        path = str(root.joinpath(*parts))
        q.need(path in index, 'missing parser closure member: ' + path)
        return index[path]

    def invocation(record):
        value = read_bound(record)
        for stream in ('stdout', 'stderr'):
            bound(value[stream])
        return value

    authority_path = q.HERE.parents[2] / q.PARSER / 'authority.json'
    q.need(q.identity(authority_path)['sha256'] == '02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0', 'unapproved parser closure authority')
    authority = q.read(authority_path)
    q.need(session['authority_sha256'] == '02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0', 'stale parser closure authority')
    for subdirectory, field in (('source', 'derived_files'), ('control-source', 'control_derived_files'), ('helpers', 'helper_files')):
        for row in authority[field]:
            record = {'path': str(root / subdirectory / q.relative(row['path'])), 'bytes': row['bytes'], 'sha256': row['sha256']}
            bound(record, omit=subdirectory != 'helpers')
    for key in ('overlay', 'control_overlay'):
        bound(session[key])
    for directory in ('source', 'control-source'):
        record = named(directory, 'observer-source-manifest.json')
        q.need(record['sha256'] == authority['helper_manifest_sha256'] and
               bound(record) == q.canonical(authority['helper_files']), 'generated observer helper manifest identity')
    for key in ('prepare_invocation', 'control_prepare_invocation'):
        invocation(session[key])
    binaries = []
    contract_roots = set()
    for profile in q.PROFILES:
        for control in (False, True):
            base = ('build-control-' if control else 'build-') + profile
            envelope = read_bound(named(base, 'portable-build.json'))
            q.need(envelope['session'] == result['session'] and envelope['profile'] == profile and envelope['control'] is control, 'sealed parser build role/session')
            invocation(envelope['invocation'])
            build = read_bound(envelope['receipt'])
            q.need(build['profile'] == profile and build['control'] is control, 'sealed parser build role')
            bound(build['overlay_manifest'])
            for stream in ('stdout', 'stderr'):
                bound(build[stream])
            binary = build['binary']
            expected_parent = root / base / 'result/target' / authority['recipe']['target'] / profile / 'deps'
            q.need(Path(binary['path']).parent == expected_parent and __import__('re').fullmatch(r'oxid-[0-9a-f]+', Path(binary['path']).name), 'sealed parser executable path/profile')
            bound(binary, omit=True)
            binaries.append(binary['path'])
        envelope = read_bound(named('collect-' + profile, 'portable-collection.json'))
        q.need(envelope['session'] == result['session'] and envelope['profile'] == profile, 'sealed collection profile/session')
        contract_roots.add(envelope['contract_dir'])
        invocation(envelope['invocation'])
        manifest = read_bound(envelope['manifest'])
        q.need(manifest['case_count'] == len(manifest['case_receipts']) == 248 and manifest['observation_count'] == 319, 'sealed parser collection incomplete')
        bound(manifest['authority_checkpoint'])
        bound(manifest['build_receipt'])
        bound(manifest['driver'])
        bound(manifest['normalizer'])
        bound(manifest['observations'])
        for stream in ('test-roster.stdout', 'test-roster.stderr'):
            bound(named('collect-' + profile, 'result', stream))
        seen = set()
        for case in manifest['case_receipts']:
            case_id = q.relative(case['case_id']).as_posix()
            q.need('/' not in case_id and case_id not in seen, 'duplicate/unsafe sealed case')
            seen.add(case_id)
            for key in ('raw', 'stdout', 'stderr'):
                bound(case[key])
            request = read_bound(case['request'])
            bound(request['source'])
            q.need(read_bound(named('collect-' + profile, 'result', case_id, 'receipt.json')) == case, 'sealed case receipt differs from manifest')
        passive = read_bound(named('passivity-' + profile, 'portable-passivity.json'))
        q.need(passive['session'] == result['session'] and passive['profile'] == profile, 'sealed passivity profile/session')
        invocation(passive['invocation'])
        report = read_bound(passive['report'])
        q.need(len(report['results']) == 3, 'sealed passivity roster incomplete')
        bound(report['instrumented']); bound(report['control']); bound(report['authority_checkpoint'])
        for case in report['results']:
            bound(case['source'])
            q.need(len(case['receipts']) == 2, 'sealed passivity pair incomplete')
            for process in case['receipts']:
                for key in ('raw', 'stdout', 'stderr'):
                    bound(process[key])
    q.need(len(set(binaries)) == 4, 'sealed four executable inventory')
    q.need(index == required, 'missing/extra complete parser comparison closure')
    q.need(omitted == allowed_omissions, 'parser omissions must be exact compiler-source and four binary identities')
    command = q.loads(resolver(seal['command']))
    q.need(command['name'] == '13-parser-comparison' and command['status'] == 0 and not command['timed_out'] and not command['stream_limit_exceeded'], 'parser actual comparison command failed')
    q.need(len(contract_roots) == 1, 'parser comparison contract roots differ')
    expected_tail = [session['adapter']['path'], 'compare', '--session', result['session']['path'], '--contract-dir', next(iter(contract_roots))]
    q.need(command['argv'] == [session['host']['python_executable'], '-B', *expected_tail] and seal['argv_tail'] == expected_tail and command['cwd'] == str(root.parent), 'parser comparison exact command/cwd binding')
    q.need(seal['before_finished_ns'] <= command['started_ns'] <= command['completed_ns'] <= seal['after_started_ns'] <= seal['after_finished_ns'], 'parser snapshot/comparison ordering')
    streams = {name: resolver(record) for name, record in command['streams'].items()}
    q.need(set(streams) == {'stdout', 'stderr'} and q.loads(streams['stdout']) == result and streams['stderr'] == b'', 'parser command output differs from sealed comparison')
    q.need(all(q.sha(raw) == command[name + '_sha256'] for name, raw in streams.items()), 'parser comparison stream hash')
    return {'sealed_members': len(index), 'required_binaries': binaries, 'full_archive_only': len(omitted)}


def parser_full_only(row, root):
    relative = Path(row['path']).relative_to(root)
    return relative.parts[0] in ('source', 'control-source') and relative.name not in ('overlay-manifest.json', 'observer-source-manifest.json') or 'target' in relative.parts


class Capsule:
    def __init__(self, output):
        self.root = q.fresh(output)
        self.entries = {}

    def add(self, path, role):
        bound = q.identity(path)
        original = bound['path']
        if original in self.entries:
            q.need(self.entries[original]['sha256'] == bound['sha256'], 'evidence changed while packaging')
            return
        name = 'members/' + str(len(self.entries)).zfill(5)
        destination = self.root / name
        destination.parent.mkdir(exist_ok=True)
        shutil.copyfile(path, destination)
        q.verify(destination, bound)
        self.entries[original] = {**bound, 'member': name, 'role': role}

    def bound(self, record, role):
        q.verify(record['path'], record)
        self.add(record['path'], role)

    def tree(self, root, role):
        for path in sorted(Path(root).rglob('*')):
            if path.is_file():
                self.add(path, role)

    def finish(self, metadata):
        q.save(self.root / 'capsule.json', {'schema': 'oxid-unit4-compact-capsule-v1', **metadata,
               'members': sorted(self.entries.values(), key=lambda row: row['member']),
               'replay_boundary': 'Unchanged raw source/stream/observer receipts and exact identities are included. Compiler/native executables, selected LLVM libraries, Rust sysroots and reconstructable compiler source views require the separately retained full artifact and exact published authorities. This compact capsule does not establish binary replay or cryptographic proof of execution.'})


def compact(output, destination):
    output = Path(output).resolve()
    capsule = Capsule(destination)
    driver = q.read(output / 'driver.json')
    q.need(driver['status'] in ('pass', 'LOCAL_ONLY_PASS'), 'failed/incomplete host cannot produce capsule')
    plan = q.read(output / 'plan.json')
    final = q.read(output / 'public-finalization.json')
    for name in ('driver.json', 'plan.json', 'path-map.json', 'public-finalization.json'):
        capsule.add(output / name, 'host-control')
    capsule.tree(output / 'commands', 'command')
    for role in ('ordinary', 'observer'):
        directory = output / (role + '-build')
        for path in directory.iterdir():
            if path.name.endswith(('.json', '.stdout', '.stderr')):
                capsule.add(path, 'public-build')
        cfg = q.read(directory / 'candidate-binding.json')
        capsule.bound(cfg['source_manifest'], 'compiler-source-manifest')
    capsule.add(output / 'observer-source/prepared.json', 'observer-preparation')
    capsule.tree(output / 'qualified-rows', 'qualified-public-raw')
    for section in q.SECTIONS:
        for path in (output / 'public' / (section + '-roster.json'), output / 'public' / section / 'observations.jsonl.gz', output / 'public' / section / 'comparison.json'):
            capsule.add(path, 'public-raw')
    for name in ('comparison.json', 'input-integrity.json', 'tools.json'):
        capsule.add(output / 'public' / name, 'public-control')
    if (output / 'public/no-tool-trap-build.json').is_file():
        capsule.add(output / 'public/no-tool-trap-build.json', 'no-native-build')
    for bound in final['ordinary_counterparts']:
        capsule.bound(bound, 'ordinary-lifecycle-raw')
    tools = q.read(output / 'public/tools.json')
    for tool in tools['tools'].values():
        if 'control_log' in tool:
            capsule.bound(tool['control_log'], 'no-native-control')
    if tools['selected_runtime']:
        capsule.bound(tools['selected_runtime']['receipt'], 'llvm-runtime')
    if final['hosted_result']:
        capsule.bound(final['hosted_result'], 'hosted-join')
        joined = q.read(final['hosted_result']['path'])
        capsule.bound(joined['child_process'], 'hosted-controller-command')
        capsule.bound(joined['shard_report'], 'hosted-shard')
        shard_root = Path(joined['shard_report']['path']).parent
        for section in ('public', 'lifecycle'):
            capsule.add(shard_root / section / 'observations.jsonl.gz', 'hosted-shard-raw')
        capsule.add(shard_root / 'tools.json', 'hosted-shard-tools')
        shard = q.read(joined['shard_report']['path'])
        for name in ('ordinary_binding', 'observer_binding'):
            capsule.bound(shard[name], 'hosted-binding')
        if shard.get('readonly_transport'):
            capsule.bound(shard['readonly_transport'], 'hosted-readonly-ledger')
    if plan['parser_required']:
        seal = q.read(output / 'parser-comparison-seal.json')
        verify_parser_seal(seal, lambda bound: (q.verify(bound['path'], bound), Path(bound['path']).read_bytes())[1])
        capsule.add(output / 'parser-comparison-seal.json', 'parser-seal')
        omitted = {r['path'] for r in seal['full_archive_only']}
        for row in seal['before']:
            if row['path'] not in omitted:
                capsule.bound(row, 'parser-raw')
        capsule.bound(seal['comparison'], 'parser-comparison')
    capsule.finish({'status': driver['status'], 'plan': q.identity(output / 'plan.json'), 'driver': q.identity(output / 'driver.json'),
                    'public_finalization': q.identity(output / 'public-finalization.json'),
                    'parser_seal': q.identity(output / 'parser-comparison-seal.json') if plan['parser_required'] else None,
                    'provenance': driver['provenance'], 'host': plan['measured_host']['name'], 'ci': plan['ci']})
    return capsule.root


class ReadCapsule:
    def __init__(self, root):
        self.root = Path(root)
        self.manifest = q.read(self.root / 'capsule.json')
        q.need(self.manifest['schema'] == 'oxid-unit4-compact-capsule-v1' and self.manifest['status'] == 'pass', 'unqualified compact capsule')
        rows = self.manifest['members']
        names = [row['member'] for row in rows]
        originals = [row['path'] for row in rows]
        q.need(rows and len(set(names)) == len(names) and len(set(originals)) == len(originals), 'duplicate/empty compact member inventory')
        actual = {path.relative_to(self.root).as_posix() for path in self.root.rglob('*') if path.is_file()}
        q.need(actual == set(names) | {'capsule.json'}, 'missing/extra compact member')
        self.mapping = {row['path']: row for row in rows}
        for row in rows:
            q.verify(self.root / q.relative(row['member']), row)

    def path(self, bound):
        row = self.mapping[bound['path']]
        q.need(all(row[key] == bound[key] for key in ('bytes', 'sha256')), 'stale/altered sealed receipt')
        return self.root / q.relative(row['member'])

    def raw(self, bound):
        path = self.path(bound)
        q.verify(path, bound)
        return path.read_bytes()

    def json(self, bound):
        return q.loads(self.raw(bound))

    def named(self, original):
        return self.mapping[str(original)]


def full_archive(output, path):
    """Keep executable artifacts; exclude only regenerable compiler caches."""
    output = Path(output).resolve()
    binaries = set()
    parser_root = output / 'parser'
    for profile in q.PROFILES:
        for role in ('build-', 'build-control-'):
            recipe = parser_root / (role + profile) / 'result/build-receipt.json'
            if recipe.is_file():
                binaries.add(Path(q.read(recipe)['binary']['path']))
    selected = set(binaries)
    for directory, subdirs, names in os.walk(output):
        subdirs[:] = [name for name in subdirs if not name.startswith('cache-target-') and name not in ('target', 'rust-bin', 'cargo-home', 'home')]
        for name in names:
            selected.add(Path(directory) / name)
    with tarfile.open(path, 'w:gz', compresslevel=6) as archive:
        for item in sorted(selected):
            archive.add(item, arcname=item.relative_to(output).as_posix(), recursive=False)
    return q.identity(path)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    parser.add_argument('--archives', required=True)
    args = parser.parse_args()
    archives = q.fresh(args.archives)
    output = Path(args.output).resolve()
    status = {'schema': 'oxid-unit4-evidence-export-v1', 'qualification_status': 'missing', 'archives': {}}
    try:
        q.need(output.is_dir(), 'qualification output absent')
        driver = q.read(output / 'driver.json') if (output / 'driver.json').is_file() else {}
        status['qualification_status'] = driver.get('status', 'incomplete')
        try:
            if driver.get('status') in ('pass', 'LOCAL_ONLY_PASS'):
                capsule = compact(output, archives / 'capsule')
                # xz preserves all raw JSON while keeping the complete capsule <32MiB.
                archive = archives / 'compact.tar.xz'
                with tarfile.open(archive, 'w:xz', preset=6) as sink:
                    for path in sorted(capsule.rglob('*')):
                        if path.is_file():
                            sink.add(path, arcname=path.relative_to(capsule).as_posix(), recursive=False)
                q.need(archive.stat().st_size < q.COMPACT_LIMIT, 'compact capsule exceeds32MiB')
                status['archives']['compact'] = q.identity(archive)
        finally:
            status['archives']['full'] = full_archive(output, archives / 'full-evidence.tar.gz')
        status['status'] = 'exported'
    except BaseException as error:
        status.update(status='failed', failure=str(error)[:2000])
        q.save(archives / 'export.json', status)
        raise
    q.save(archives / 'export.json', status)
    q.need(status['qualification_status'] in ('pass', 'LOCAL_ONLY_PASS'), 'evidence preserved; qualification remains failed/incomplete')


if __name__ == '__main__':
    try:
        main()
    except (q.Reject, OSError, KeyError, TypeError, ValueError) as error:
        print('Unit4 evidence: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
