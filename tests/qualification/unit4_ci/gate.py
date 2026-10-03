#!/usr/bin/env python3
"""Mandatory actual-host Unit4 orchestration. Every stage fails closed."""
import sys
sys.dont_write_bytecode = True
import argparse
from collections import Counter
import os
from pathlib import Path
import re
import shutil
import time
import common as q


def materialized(repo, output):
    transport = q.module('_unit4_contract_transport', repo / q.TRANSPORT / 'transport.py')
    manifest, _ = transport.load(repo / q.TRANSPORT)
    transport.verify_materialized(manifest, output / 'contracts')
    mapping = {row['logical_path']: str(output / 'contracts' / row['archive_path']) for row in manifest['members']}
    roots = {name: Path(mapping[record['logical_path']]).parent for name, record in manifest['active_contracts'].items()}
    q.save(output / 'path-map.json', mapping)
    return roots, mapping


def plan_for(contracts, measured, provenance, ci):
    c = sys.modules['contracts']
    rosters = {section: [c.receipt_identity(row) for row in contracts.roster(section, measured['name'])] for section in q.SECTIONS}
    counts = {host: {section: dict(Counter(row['profile'] for row in contracts.roster(section, host) if row['scope'] == 'execute'))
                     for section in q.SECTIONS} for host in q.HOSTS}
    q.need(all(counts[host][section].get(profile, 0) > 0 for host in q.HOSTS for section in ('public', 'original', 'lifecycle') for profile in q.PROFILES), 'empty mandatory domain/profile')
    q.need(all(not counts[host][section] for host in q.HOSTS[1:] for section in ('predecessors', 'guards')), 'non-Linux scope unexpectedly changed')
    return {'schema': 'oxid-unit4-host-plan-v1', 'provenance': provenance, 'measured_host': measured,
            'ci': ci, 'required_hosts': list(q.HOSTS), 'sections': list(q.SECTIONS), 'profiles': list(q.PROFILES),
            'rosters': rosters, 'counts_from_frozen_rosters': counts, 'effective_authority': contracts.effective_identity,
            'parser_required': measured['name'] == 'Linux x86_64', 'hosted_root_required': measured['name'] == 'Linux x86_64'}


WINDOWS_DISCOVERY_KEYS = ('ProgramFiles', 'ProgramFiles(x86)', 'ProgramW6432', 'VSINSTALLDIR',
                          'VCINSTALLDIR', 'VCToolsInstallDir', 'VSCMD_ARG_TGT_ARCH', 'VSCMD_ARG_HOST_ARCH')


def clean_environment():
    # Never serialize inherited CI credentials or unrelated environment values.
    keys = ('PATH', 'HOME', 'USERPROFILE', 'SYSTEMROOT', 'SystemRoot', 'WINDIR', 'COMSPEC', 'PATHEXT',
            'TEMP', 'TMP', 'TMPDIR', 'SDKROOT', 'DEVELOPER_DIR', 'LIB', 'INCLUDE', 'LIBPATH')
    if os.name == 'nt':
        keys += WINDOWS_DISCOVERY_KEYS
    env = {key: os.environ[key] for key in keys if key in os.environ}
    if os.name == 'nt':
        env['VSLANG'] = '1033'
    env.update(CARGO_HOME=os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')),
               RUSTUP_HOME=os.environ.get('RUSTUP_HOME', str(Path.home() / '.rustup')),
               PYTHONDONTWRITEBYTECODE='1', PYTHONNOUSERSITE='1', PYTHONOPTIMIZE='0',
               CARGO_INCREMENTAL='0', CARGO_BUILD_JOBS='2', CARGO_TERM_COLOR='never')
    if os.name != 'nt':
        env.update(LANG='C.UTF-8', LC_ALL='C.UTF-8')
    return env


def verify_msvc_linker_help(process, selected_linker, cwd):
    """The observed MSVC /? exit 1100 is valid only for this bounded help probe."""
    q.need(process['argv'] == [selected_linker, '/?'] and process['cwd'] == str(cwd), 'Microsoft help receipt command differs from the exact selected-linker probe')
    q.need(process['status'] in (0, 1100) and not process['timed_out'] and not process['stream_limit_exceeded'], 'selected Microsoft linker help invocation failed')
    q.need(process['stderr'] == '', 'Microsoft linker help emitted an error stream')
    lines = process['stdout'].splitlines()
    q.need(len(lines) > 7 and re.fullmatch(r'Microsoft \(R\) Incremental Linker Version [0-9]+(?:\.[0-9]+){3}', lines[0]) is not None,
           'selected linker did not identify as Microsoft Incremental Linker')
    q.need(lines[1:7] == ['Copyright (C) Microsoft Corporation.  All rights reserved.', '',
                         ' usage: LINK [options] [files] [@commandfile]', '', '   options:', ''], 'Microsoft linker help header/usage is missing')
    body = lines[7:]
    q.need(all(not line or re.match(r'^ {6}(?:/| {6})', line) is not None for line in body) and
           any(line.startswith('      /OUT:') for line in body) and any(line.startswith('      /MACHINE:') for line in body) and
           re.search(r'(?i)\bfatal error\b|\berror(?:\s+LNK[0-9]+|\s*:)', process['stdout']) is None,
           'Microsoft linker help options are missing or contain diagnostics')


def verify_windows_toolchain(driver):
    """Bind the preinstalled developer tools selected by the actual cleaned environment."""
    env = driver.env
    record = {'schema': 'oxid-unit4-windows-toolchain-v1', 'status': 'checking',
              'discovery_keys': sorted(key for key in WINDOWS_DISCOVERY_KEYS if key in env),
              'search_environment_sha256': {key: q.sha(env.get(key, '').encode()) for key in ('PATH', 'LIB', 'INCLUDE', 'LIBPATH')}}
    driver.state['windows_toolchain'] = record
    try:
        q.need(all(env.get(key) for key in ('VCINSTALLDIR', 'VSINSTALLDIR', 'VCToolsInstallDir', 'LIB', 'INCLUDE')), 'Microsoft developer environment is incomplete')
        q.need(all(env.get(key) in ('x64', 'amd64') for key in ('VSCMD_ARG_TGT_ARCH', 'VSCMD_ARG_HOST_ARCH')), 'Microsoft developer tools must target and run on x64')
        tools_dir = Path(env['VCToolsInstallDir']).resolve() / 'bin/Hostx64/x64'
        record['expected_tool_directory'] = str(tools_dir)
        record['tools'] = {}
        for name in ('link.exe', 'cl.exe'):
            selected = shutil.which(name, path=env['PATH'])
            q.need(selected is not None, 'Microsoft tool is absent from cleaned PATH: ' + name)
            selected = Path(selected).resolve()
            record['tools'][name] = q.identity(selected)
            q.need(selected == (tools_dir / name).resolve(), 'cleaned PATH selected a tool outside the configured x64 MSVC toolset: ' + name)
        process = driver.runtime.process([record['tools']['link.exe']['path'], '/?'], driver.output, env, timeout=30)
        record['linker_help'] = process
        verify_msvc_linker_help(process, record['tools']['link.exe']['path'], driver.output)
        for bound in record['tools'].values():
            q.verify(bound['path'], bound)
        record['status'] = 'pass'
    except BaseException as error:
        record.update(status='fail', failure=str(error)[:2000])
        raise
    finally:
        driver.write()


class Driver:
    def __init__(self, repo, output, provenance, runtime):
        self.repo, self.output, self.provenance, self.runtime = repo, output, provenance, runtime
        self.env = clean_environment()
        self.start = time.monotonic()
        self.state = {'schema': 'oxid-unit4-driver-v1', 'status': 'running', 'provenance': provenance,
                      'started_ns': time.time_ns(), 'stages': [], 'limits': {'total_seconds': 9000, 'minimum_free_bytes': 2 * 1024**3, 'stream_bytes': 8 * 1024**2}}
        (output / 'commands').mkdir()
        self.write()

    def write(self):
        q.save(self.output / 'driver.json', self.state)

    def stage(self, name, argv, timeout=1800, accepted=(0,)):
        q.need(q.admit(self.repo, self.provenance['checkout_head'], self.provenance['event_sha']) == self.provenance, 'admitted inputs changed between stages')
        q.need(time.monotonic() - self.start < 9000, 'combined qualification deadline exceeded')
        q.need(shutil.disk_usage(self.output).free >= 2 * 1024**3, 'insufficient free qualification disk')
        directory = self.output / 'commands' / name
        directory.mkdir()
        command = [sys.executable, '-B', *map(str, argv)]
        print('Unit4 stage: ' + name, flush=True)
        process = self.runtime.process(command, self.output, self.env, timeout=min(timeout, max(1, 9000 - (time.monotonic() - self.start))))
        streams = {}
        for stream in ('stdout', 'stderr'):
            path = directory / stream
            path.write_bytes(process[stream].encode())
            streams[stream] = q.identity(path)
        receipt = {key: value for key, value in process.items() if key not in ('stdout', 'stderr')}
        receipt.update(name=name, streams=streams, accepted_exit_codes=list(accepted))
        q.save(directory / 'receipt.json', receipt)
        self.state['stages'].append(q.identity(directory / 'receipt.json'))
        self.write()
        q.need(not process['timed_out'] and not process['stream_limit_exceeded'] and process['status'] in accepted,
               name + ' failed; raw command output is preserved')
        return process['status']


def recheck_public(repo, output, contracts, root_result=None, ordinary_path=None, observer_path=None, main=None):
    """Replay the exact public predicates while all original files still exist."""
    c, rt, run, guards, Predecessors = q.public_modules(repo)
    ordinary_path = ordinary_path or output / 'ordinary-build/candidate-binding.json'
    observer_path = observer_path or output / 'observer-build/candidate-binding.json'
    ordinary, observer = rt.candidate_binding(ordinary_path), rt.candidate_binding(observer_path)
    for cfg in (ordinary, observer):
        cfg['_binary_stats'] = {profile: rt.file_identity(binary['path']) for profile, binary in cfg['binaries'].items()}
    main = main or output / 'public'
    result = q.read(main / 'comparison.json')
    q.need(result['all_contract_sections_collected'] is True and result['adapter'] == run.adapter_identity(), 'incomplete public domains')
    tools_record = q.read(main / 'tools.json')
    tools = tools_record['tools']
    for tool in tools.values():
        c.verify(tool['executable']['path'], tool['executable'])
        c.verify(tool['wrapper']['path'], tool['wrapper'])
        if 'control' in tool:
            process = tool['control']
            q.need(process['status'] == 121 and not process['timed_out'] and not process['stream_limit_exceeded'], 'no-native trap control did not execute')
            c.verify(tool['control_log']['path'], tool['control_log'])
            q.need(rt.read_events(Path(tool['control_log']['path'])) == [{'unexpected_native_tool': True}], 'no-native trap control log')
    if c.host() != 'Linux x86_64':
        q.need(tools_record['mode'] == 'no-native-traps' and tools_record['selected_runtime'] is None, 'non-Linux must execute no-native trap route')
    predecessor = Predecessors(contracts)
    qualified, ordinary_counterparts = {}, []
    for section in q.SECTIONS:
        roster = contracts.roster(section)
        actual = q.rows(main / section / 'observations.jsonl.gz')
        c.check_inventory(roster, actual)
        lookup = {row['key']: row for row in roster}
        for got in actual:
            row = lookup[got['key']]
            q.need(got['status'] != 'fail', 'failed public observation')
            if row['scope'] != 'execute':
                continue
            if section == 'guards':
                case = contracts.cases[row['case']['source_case_id']]
                guards.verify_complete(row, got, observer, contracts, contracts.source_bytes(case), tools, ordinary)
                ordinary_counterparts.append(got['ordinary_receipt'])
            else:
                sources = run.tuple_sources(row, contracts, predecessor)
                if section == 'lifecycle':
                    bound = got['ordinary_receipt']
                    c.verify(bound['path'], bound)
                    baseline = c.load(bound['path'])
                    q.need((baseline.get('executed') is True and baseline.get('status') == 'pass') if got['executed'] else
                           (baseline.get('executed') is False and baseline.get('status') == got['status'] == 'unsupported-capability'), 'ordinary lifecycle counterpart failed or missing')
                    run.lifecycle_compare(row, baseline, got, ordinary, observer, contracts, sources, tools)
                    ordinary_counterparts.append(bound)
                else:
                    run.core_compare(section, row, got, ordinary, contracts, sources, tools, predecessor)
        qualified[section] = actual
    if root_result is not None:
        q.need(root_result['status'] == 'QUALIFIED_LOCAL_RAW_JOIN' and root_result['summary_only_substitution'] is False, 'hosted raw join missing')
        q.need(len(root_result['raw_replacements']) == 12 and root_result['shard_report'] is not None, 'real hosted twelve-gap branch absent')
        shard_root = Path(root_result['shard_report']['path']).parent
        shard = q.read(root_result['shard_report']['path'])
        q.verify(root_result['shard_report']['path'], root_result['shard_report'])
        q.need(shard['status'] == 'PARTIAL_CAPABILITY_SHARD' and shard['effective_uid'] != 0 and shard['supplementary_groups'] == [], 'hosted actual identity')
        replacements = {(row['section'], row['key']): row for row in root_result['raw_replacements']}
        q.need(len(replacements) == 12, 'duplicate hosted replacement')
        for section in ('public', 'lifecycle'):
            shards = {row['key']: row for row in q.rows(shard_root / section / 'observations.jsonl.gz')}
            for index, got in enumerate(qualified[section]):
                if got['status'] != 'unsupported-capability':
                    continue
                replacement = replacements[section, got['key']]
                supplied = shards.pop(got['key'])
                q.need(q.sha(__import__('json').dumps(got, sort_keys=True).encode()) == replacement['main_raw_sha256'] and
                       q.sha(__import__('json').dumps(supplied, sort_keys=True).encode()) == replacement['shard_raw_sha256'], 'hosted raw replacement identity')
                qualified[section][index] = supplied
                if section == 'lifecycle':
                    ordinary_counterparts.append(supplied['ordinary_receipt'])
            q.need(not shards, 'extra hosted capability row')
    results = {}
    for section, actual in qualified.items():
        counts = c.check_inventory(contracts.roster(section), actual, allow_capability_gap=False)
        q.need(all(row['status'] == 'pass' for row in actual if row['executed']), 'failed qualified row')
        q.need(counts['executed'] == counts['required_local'] and counts['unsupported_capability'] == 0, 'incomplete qualified domain')
        results[section] = counts
    return qualified, results, ordinary_counterparts


def prepare_host(args, repo, output, provenance, measured, driver):
    transport = repo / q.TRANSPORT / 'transport.py'
    driver.stage('01-contract-verification', [transport, 'verify'], 120)
    driver.stage('02-contract-materialization', [transport, 'extract', '--destination', output / 'contracts'], 120)
    roots, mapping = materialized(repo, output)
    c, _, _, _, _ = q.public_modules(repo)
    contracts = c.Contracts(roots['public'], mapping, repo / q.AMENDMENT)
    ci = {key: os.environ.get(key) for key in ('GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT', 'GITHUB_WORKFLOW', 'GITHUB_EVENT_NAME', 'GITHUB_REPOSITORY')}
    plan = plan_for(contracts, measured, provenance, ci)
    q.save(output / 'plan.json', plan)
    source_root = q.fresh(output / 'current-source')
    for row in q.read(repo / q.SOURCE / 'current-source.json')['files']:
        target = source_root / row['path']
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(repo / row['path'], target)
        q.verify(target, row)
    # The frozen generator separately verifies this unchanged retained package.
    retained = Path('tests/fixtures/typed_project_unit1_compatibility')
    shutil.copytree(repo / retained, source_root / retained)
    driver.stage('03-lifecycle-preparation', [repo / q.PUBLIC / 'build.py', 'prepare-observer', '--source-root', source_root,
                 '--manifest', repo / q.SOURCE / 'current-source.json', '--observer-patch', repo / q.OBSERVER_PATCH,
                 '--observer-patch-sha256', '2d652a3a39240c32f6dce38e6d61710399e028cdecc13b5d0f159c550994de56', '--out', output / 'observer-source'], 120)
    return roots, contracts, plan


def host(args):
    repo = Path(args.repo).resolve()
    provenance = q.admit(repo, args.expected_head, args.event_sha)
    measured = q.measured_host()
    q.need(measured['name'] == args.host and measured['pointer_width'] == 64, 'requested/measured host mismatch')
    linux = measured['name'] == 'Linux x86_64'
    local_only = args.command == 'local-only'
    if linux:
        q.need(local_only or measured['effective_uid'] == 0, 'mandatory Linux gate requires actual root controller branch; use local-only for unqualified local validation')
        q.need(args.historical_repo and args.toolchain and args.cargo_cache, 'Linux parser build inputs missing')
    output = q.fresh(args.output)
    c, rt, _, _, _ = q.public_modules(repo)
    driver = Driver(repo, output, provenance, rt)
    try:
        if measured['name'] == 'Windows x86_64':
            verify_windows_toolchain(driver)
        roots, contracts, plan = prepare_host(args, repo, output, provenance, measured, driver)
        for role, source, manifest, kind in (
            ('ordinary', output / 'current-source', repo / q.SOURCE / 'current-source.json', 'unit4-public-v3-candidate'),
            ('observer', output / 'observer-source/source', output / 'observer-source/observer-source.json', 'unit4-public-v3-lifecycle-observer')):
            driver.stage('04-build-' + role, [repo / q.PUBLIC / 'build.py', 'build', '--source-root', source, '--manifest', manifest,
                         '--out', output / (role + '-build'), '--target', output / ('cache-target-' + role), '--kind', kind,
                         '--compiler-head', provenance['checkout_head'], '--compiler-head-tree', provenance['checkout_tree'],
                         '--compiler-source-only-tree', provenance['source_only_tree']], 3600)
        command = [repo / q.PUBLIC / 'run.py', 'run', '--contracts', roots['public'], '--path-map', output / 'path-map.json',
                   '--amendment-root', repo / q.AMENDMENT, '--binding', output / 'ordinary-build/candidate-binding.json',
                   '--observer-binding', output / 'observer-build/candidate-binding.json', '--out', output / 'public']
        if linux:
            from evidence import parser_inputs, parser_full_only
            if local_only:
                q.need(args.llvm_bin and args.llvm_lib_dir and args.llvm_runtime_receipt, 'local-only LLVM inputs must be explicit')
                driver.stage('05-verify-local-llvm', [q.HERE / 'gate.py', 'verify-local-runtime', '--repo', repo, '--llvm-lib-dir', args.llvm_lib_dir,
                             '--llvm-runtime-receipt', args.llvm_runtime_receipt, '--output', output / 'local-runtime-verification.json',
                             '--expected-head', provenance['checkout_head'], '--event-sha', provenance['event_sha']], 120)
                command += ['--llvm', args.llvm_bin, '--libs', args.llvm_lib_dir, '--runtime-receipt', args.llvm_runtime_receipt]
            else:
                driver.stage('05-stage-llvm', [repo / q.RUNTIME_STAGE, '--output', output / 'llvm-runtime'], 120)
                command += ['--llvm', '/usr/lib/llvm-19/bin', '--libs', output / 'llvm-runtime/libraries', '--runtime-receipt', output / 'llvm-runtime/llvm-runtime-stage.json']
        root_branch = linux and measured['effective_uid'] == 0
        status = driver.stage('06-public-collection', command, 3600, accepted=(2,) if root_branch else (0,))
        root_result = None
        if linux and not local_only:
            q.need(status == (2 if root_branch else 0), 'Linux capability branch exit mismatch')
            driver.stage('07-hosted-capability-join', [repo / q.HOSTED / 'hosted.py', 'join', '--adapter', repo / q.PUBLIC,
                         '--ordinary-binding', output / 'ordinary-build/candidate-binding.json', '--observer-binding', output / 'observer-build/candidate-binding.json',
                         '--expected-head', provenance['checkout_head'], '--contract-package', repo / q.TRANSPORT,
                         '--main-root', output / 'public', '--contracts', roots['public'], '--amendment-root', repo / q.AMENDMENT,
                         '--copy-if-unreadable', '--out', output / 'hosted'], 1200)
            root_result = q.read(output / 'hosted/comparison.json')
        actual, counts, counterparts = recheck_public(repo, output, contracts, root_result)
        qualified = q.fresh(output / 'qualified-rows')
        for section, values in actual.items():
            q.write_rows(qualified / (section + '.jsonl.gz'), values)
        q.save(output / 'public-finalization.json', {'status': 'pass', 'plan': q.identity(output / 'plan.json'), 'sections': counts,
               'rows': {section: q.identity(qualified / (section + '.jsonl.gz')) for section in q.SECTIONS},
               'ordinary_counterparts': counterparts, 'main_result': q.identity(output / 'public/comparison.json'),
               'hosted_result': q.identity(output / 'hosted/comparison.json') if linux and not local_only else None,
               'boundary': 'Fresh same-host full comparator replay, followed by unchanged raw-row transport'})
        if linux:
            parser = repo / q.PARSER / 'portable.py'
            historical = Path(args.historical_repo).resolve()
            q.need(q.git(historical, 'rev-parse', 'HEAD') == q.HISTORICAL_HEAD, 'explicit historical parser Git checkout required')
            session_root = output / 'parser'
            session = session_root / 'session.json'
            driver.stage('08-parser-prepare', [parser, 'prepare', '--repo', historical, '--checkout', repo, '--output', session_root], 300)
            for profile in q.PROFILES:
                build = [parser, 'build', '--session', session, '--profile', profile, '--toolchain', Path(args.toolchain).resolve(), '--cargo-cache', Path(args.cargo_cache).resolve()]
                driver.stage('09-parser-build-' + profile, build, 1800)
                driver.stage('10-parser-control-' + profile, [*build, '--control'], 1800)
                driver.stage('11-parser-passivity-' + profile, [parser, 'passivity', '--session', session, '--profile', profile], 300)
                driver.stage('12-parser-collect-' + profile, [parser, 'collect', '--session', session, '--profile', profile, '--contract-dir', roots['parser']], 1800)
            comparison_argv = [parser, 'compare', '--session', session, '--contract-dir', roots['parser']]
            before = parser_inputs(session_root)
            before_finished_ns = time.time_ns()
            driver.stage('13-parser-comparison', comparison_argv, 1200)
            after_started_ns = time.time_ns()
            after = parser_inputs(session_root)
            after_finished_ns = time.time_ns()
            q.need(before == after, 'parser inputs changed during final comparison')
            result = q.read(session_root / 'comparison.json')
            q.need(result['status'] == 'pass' and result['observations'] == result['expected_observations'] == 638 and
                   [(p['profile'], p['pairs']) for p in result['ordinary_passivity']] == [('debug', 6), ('release', 6)], 'parser incomplete')
            q.save(output / 'parser-comparison-seal.json', {'schema': 'oxid-unit4-parser-comparison-seal-v2', 'status': 'pass',
                   'before': before, 'after': after, 'comparison': q.identity(session_root / 'comparison.json'),
                   'before_finished_ns': before_finished_ns, 'after_started_ns': after_started_ns, 'after_finished_ns': after_finished_ns,
                   'command': q.identity(output / 'commands/13-parser-comparison/receipt.json'), 'argv_tail': list(map(str, comparison_argv)),
                   'full_archive_only': [row for row in before if parser_full_only(row, session_root)]})
        q.need(q.admit(repo, args.expected_head, args.event_sha) == provenance, 'final input binding changed')
        driver.state.update(status='LOCAL_ONLY_PASS' if local_only else 'pass', execution_mode='local-only' if local_only else 'hosted-required',
                            completed_ns=time.time_ns(), plan=q.identity(output / 'plan.json'), public=q.identity(output / 'public-finalization.json'),
                            parser=q.identity(output / 'parser/comparison.json') if linux else None)
        if local_only:
            driver.state['unexecuted_hosted_obligations'] = ['actual root twelve-replacement controller', 'other declared execution hosts']
        driver.write()
    except BaseException as error:
        driver.state.update(status='fail', completed_ns=time.time_ns(), failure=str(error)[:2000])
        driver.write()
        raise


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest='command', required=True)
    host_parser = sub.add_parser('host')
    local_parser = sub.add_parser('local-only')
    for command in (host_parser, local_parser):
        command.add_argument('--host', choices=q.HOSTS, required=True)
        for name in ('historical-repo', 'toolchain', 'cargo-cache'):
            command.add_argument('--' + name)
    for name in ('llvm-bin', 'llvm-lib-dir', 'llvm-runtime-receipt'):
        local_parser.add_argument('--' + name, required=True)
    runtime_parser = sub.add_parser('verify-local-runtime')
    for name in ('repo', 'llvm-lib-dir', 'llvm-runtime-receipt', 'output', 'expected-head', 'event-sha'):
        runtime_parser.add_argument('--' + name, required=True)
    prepare_parser = sub.add_parser('prepare-only')
    prepare_parser.add_argument('--host', choices=q.HOSTS, required=True)
    for command in (host_parser, local_parser, prepare_parser):
        for name in ('repo', 'output', 'expected-head', 'event-sha'):
            command.add_argument('--' + name, required=True)
    args = parser.parse_args()
    if args.command == 'verify-local-runtime':
        q.admit(Path(args.repo).resolve(), args.expected_head, args.event_sha)
        _, runtime, _, _, _ = q.public_modules(Path(args.repo).resolve())
        result = runtime.staged_runtime(args.llvm_runtime_receipt, args.llvm_lib_dir)
        q.need(not Path(args.output).exists(), 'occupied runtime verification')
        q.save(args.output, {'status': 'EXISTING_SELECTED_RUNTIME_VERIFIED_LOCAL_ONLY', 'runtime': result})
    elif args.command in ('host', 'local-only'):
        host(args)
    else:
        repo = Path(args.repo).resolve()
        provenance = q.admit(repo, args.expected_head, args.event_sha)
        measured = q.measured_host()
        q.need(measured['name'] == args.host, 'requested/measured host mismatch')
        output = q.fresh(args.output)
        driver = Driver(repo, output, provenance, q.public_modules(repo)[1])
        prepare_host(args, repo, output, provenance, measured, driver)
        driver.state.update(status='PREPARATION_ONLY', compiler_executions=0)
        driver.write()


if __name__ == '__main__':
    try:
        main()
    except (q.Reject, OSError, KeyError, TypeError, ValueError) as error:
        print('Unit4 gate rejected: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
