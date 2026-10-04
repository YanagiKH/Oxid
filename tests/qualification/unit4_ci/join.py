#!/usr/bin/env python3
"""Join exact-host raw evidence, never summaries or synthetic host skips."""
import sys
sys.dont_write_bytecode = True
import argparse
import base64
from collections import Counter
import copy
import gzip
import json
import os
from pathlib import Path, PureWindowsPath
import re
import common as q
from evidence import ReadCapsule, verify_parser_seal


def provenance_key(value):
    return {key: value[key] for key in ('checkout_head', 'checkout_tree', 'event_sha', 'source_only_tree', 'integration')} | {
        key: {field: value[key][field] for field in ('bytes', 'sha256')} for key in ('source_manifest', 'inputs', 'workflow')}


def original_child(root, *parts):
    path_type = PureWindowsPath if '\\' in str(root) else Path
    return str(path_type(root).joinpath(*parts))


def checked_process(process):
    q.need(process['timed_out'] is False and process['stream_limit_exceeded'] is False, 'incomplete process')
    q.need(process['completed_ns'] >= process['started_ns'], 'invalid process timing')
    for stream in ('stdout', 'stderr'):
        q.need(q.sha(process[stream].encode()) == process[stream + '_sha256'], 'altered process stream')


def final_coverage(contracts, host_keys):
    q.need(set(host_keys) == set(q.HOSTS), 'missing/extra actual host')
    union = set()
    for host in q.HOSTS:
        expected = {row['key'] for section in q.SECTIONS for row in contracts.roster(section, host) if row['scope'] == 'execute'}
        actual = host_keys[host]
        q.need(len(actual) == len(set(actual)) and set(actual) == expected, 'missing/duplicate host profile/domain tuple')
        q.need(not union.intersection(actual), 'cross-host duplicate actual tuple')
        union.update(actual)
    return union


def build_configs(capsule, output, provenance, host):
    configs = {}
    target = {'Linux x86_64': 'x86_64-unknown-linux-gnu', 'Windows x86_64': 'x86_64-pc-windows-msvc',
              'macOS x86_64': 'x86_64-apple-darwin', 'macOS arm64': 'aarch64-apple-darwin'}[host]
    for role, kind in (('ordinary', 'unit4-public-v3-candidate'), ('observer', 'unit4-public-v3-lifecycle-observer')):
        bound = capsule.named(original_child(output, role + '-build', 'candidate-binding.json'))
        cfg = capsule.json(bound)
        q.need(cfg['kind'] == kind and cfg['compiler_head'] == provenance['checkout_head'] and cfg['compiler_head_tree'] == provenance['checkout_tree'] and
               cfg['compiler_source_only_tree'] == provenance['source_only_tree'], 'public build checkout binding')
        q.need(set(cfg['binaries']) == set(cfg['build_receipts']) == set(q.PROFILES), 'missing public build profile')
        manifest = capsule.json(cfg['source_manifest'])
        expected_sha = q.CURRENT_SHA if role == 'ordinary' else '1cea140199a9b84d8233e5410579a509663e06aa06a4a7543eeea42cffbec4a6'
        actual_sha = cfg['source_manifest']['sha256'] if role == 'ordinary' else q.sha(q.canonical(manifest['files']))
        q.need(actual_sha == expected_sha and len(manifest['files']) == (133 if role == 'ordinary' else 134), 'public source authority')
        for profile in q.PROFILES:
            build = capsule.json(cfg['build_receipts'][profile])
            q.need(build['status'] == 0 and build['profile'] == profile and build['source_before'] == build['source_after'] ==
                   build['source_manifest_sha256'] == cfg['source_manifest']['sha256'], 'public build source/profile/status')
            q.need(build['binary'] == cfg['binaries'][profile] and build['argv'] == ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline'] + (['--release'] if profile == 'release' else []), 'public build recipe')
            q.need(build['rustc'].startswith('rustc 1.99.0 ') and '\nhost: ' + target + '\n' in build['rustc'], 'actual public Rust host/version')
            q.need(build['environment']['CARGO_INCREMENTAL'] == '0' and build['environment']['CARGO_BUILD_JOBS'] == '2', 'public build resource recipe')
            for stream in build['streams'].values():
                capsule.raw(stream)
        q.need(cfg['binaries']['debug']['sha256'] != cfg['binaries']['release']['sha256'], 'same public binary reused for two profiles')
        cfg.update(_sha256=bound['sha256'], _binary_stats={})
        configs[role] = cfg
    return configs


def public_join(capsule, contracts, repo, plan):
    c, rt, run, guards, Predecessors = q.public_modules(repo)
    final = capsule.json(capsule.manifest['public_finalization'])
    q.need(final['status'] == 'pass' and final['plan'] == capsule.manifest['plan'], 'stale public finalizer/plan')
    output = str(Path(capsule.manifest['plan']['path']).parent) if plan['measured_host']['name'] != 'Windows x86_64' else str(PureWindowsPath(capsule.manifest['plan']['path']).parent)
    host = plan['measured_host']['name']
    configs = build_configs(capsule, output, plan['provenance'], host)
    q.need(set(final['rows']) == set(final['sections']) == set(q.SECTIONS), 'missing final public domain')
    main_result = capsule.json(final['main_result'])
    q.need(main_result['all_contract_sections_collected'] is True and main_result['adapter'] == run.adapter_identity(), 'main public result incomplete')
    q.need(all(main_result.get(k) == v for k, v in contracts.effective_identity.items()), 'stale public effective authority')
    tool_record = capsule.json(capsule.named(original_child(output, 'public', 'tools.json')))
    tools = tool_record['tools']
    q.need(set(tools) == {'clang', 'opt', 'ld.lld'}, 'tool profile incomplete')
    replacement_records, shards, shard_configs = {}, {}, {}
    if host == 'Linux x86_64':
        q.need(plan['measured_host']['effective_uid'] == 0 and final['hosted_result'] is not None and main_result['status'] == 'INCOMPLETE', 'Linux actual root branch missing')
        hosted = capsule.json(final['hosted_result'])
        q.need(hosted['status'] == 'QUALIFIED_LOCAL_RAW_JOIN' and hosted['expected_head'] == plan['provenance']['checkout_head'] and
               hosted['summary_only_substitution'] is False and len(hosted['raw_replacements']) == 12, 'hosted exact raw closure missing')
        controller = capsule.json(hosted['child_process'])
        checked_process(controller)
        q.need(controller['status'] == 0, 'hosted controller failed')
        shard = capsule.json(hosted['shard_report'])
        q.need(shard['status'] == 'PARTIAL_CAPABILITY_SHARD' and shard['complete_qualification'] is False and shard['effective_uid'] != 0 and
               shard['supplementary_groups'] == [] and shard['expected_head'] == plan['provenance']['checkout_head'], 'actual hosted shard identity')
        group = shard['process_group_policy']
        q.need(group['worker_pid'] == group['process_group'] == group['session_id'] == controller['pid'] and group['nested_new_sessions'] is False, 'hosted process group binding')
        shard_root = str(Path(hosted['shard_report']['path']).parent)
        shard_tools = capsule.json(capsule.named(original_child(shard_root, 'tools.json')))['tools']
        for role in ('ordinary', 'observer'):
            cfg = capsule.json(shard[role + '_binding'])
            original = configs[role]
            q.need(cfg['compiler_head'] == original['compiler_head'] and cfg['compiler_head_tree'] == original['compiler_head_tree'] and
                   cfg['source_manifest']['sha256'] == original['source_manifest']['sha256'], 'hosted copied source/head mismatch')
            for profile in q.PROFILES:
                q.need(all(cfg['binaries'][profile][key] == original['binaries'][profile][key] for key in ('bytes', 'sha256')), 'hosted copied binary mismatch')
            cfg.update(_sha256=shard[role + '_binding']['sha256'], _binary_stats={})
            shard_configs[role] = cfg
        for section in ('public', 'lifecycle'):
            values = q.rows(capsule.path(capsule.named(original_child(shard_root, section, 'observations.jsonl.gz'))))
            q.need(len(values) == 6 and len({row['key'] for row in values}) == 6, 'missing/duplicate hosted section')
            shards[section] = {row['key']: row for row in values}
        replacement_records = {(row['section'], row['key']): row for row in hosted['raw_replacements']}
        q.need(len(replacement_records) == 12, 'duplicate hosted replacements')
        q.need(tool_record['mode'] == 'real-native' and tool_record['selected_runtime'] is not None, 'Linux LLVM stage missing')
        capsule.raw(tool_record['selected_runtime']['receipt'])
    else:
        q.need(final['hosted_result'] is None and main_result['status'] == 'LOCAL_COMPARISONS_COMPLETE' and tool_record['mode'] == 'no-native-traps', 'non-Linux route incomplete')
        for tool in tools.values():
            process = tool['control']
            checked_process(process)
            q.need(process['status'] == 121 and process['stdout'] == '' and process['stderr'] == 'unexpected native tool invocation\n', 'actual no-native trap control')
            q.need([q.loads(line) for line in capsule.raw(tool['control_log']).splitlines()] == [{'unexpected_native_tool': True}], 'actual no-native trap log')
        trap = capsule.json(capsule.named(original_child(output, 'public', 'no-tool-trap-build.json')))
        q.need(trap['build']['status'] == trap['rustc']['status'] == 0 and trap['rustc']['stdout'].startswith('rustc 1.99.0 '), 'actual trap build missing')
        for process in (trap['build'], trap['rustc']):
            checked_process(process)
    predecessors = Predecessors(contracts)
    counts, all_keys, replaced = {}, set(), set()
    for section in q.SECTIONS:
        roster = contracts.roster(section, host)
        q.need(plan['rosters'][section] == [c.receipt_identity(row) for row in roster], 'changed frozen host plan')
        values = q.rows(capsule.path(final['rows'][section]))
        original = q.rows(capsule.path(capsule.named(original_child(output, 'public', section, 'observations.jsonl.gz'))))
        original_index = {row['key']: row for row in original}
        c.check_inventory(roster, original)
        actual_counts = c.check_inventory(roster, values, allow_capability_gap=False)
        q.need(actual_counts == final['sections'][section] and actual_counts['executed'] == actual_counts['required_local'], 'public count/result mismatch')
        actual_index = {row['key']: row for row in values}
        for row in roster:
            got = actual_index[row['key']]
            base = original_index[row['key']]
            row_configs, row_tools = configs, tools
            if base['status'] == 'unsupported-capability':
                key = (section, row['key'])
                q.need(key in replacement_records and got == shards[section][row['key']], 'unbound hosted replacement')
                replacement = replacement_records[key]
                q.need(q.sha(json.dumps(base, sort_keys=True).encode()) == replacement['main_raw_sha256'] and
                       q.sha(json.dumps(got, sort_keys=True).encode()) == replacement['shard_raw_sha256'], 'changed main/shard raw replacement')
                proof = got['capability_proof']
                q.need(proof['effective_uid'] == shard['effective_uid'] and proof['effective_gid'] == shard['effective_gid'] and proof['groups'] == [] and
                       proof['read_open_succeeded'] is False and proof['error_errno'] in (1, 13), 'actual transient denial capability missing')
                row_configs, row_tools = shard_configs, shard_tools
                replaced.add(key)
            else:
                q.need(got == base, 'unapproved changed public raw row')
            if row['scope'] != 'execute':
                continue
            q.need(got['status'] == 'pass' and got['executed'] is True, 'unqualified actual public tuple')
            q.need(row['host'] == host and row['execution_host'] == host, 'foreign-host substitution')
            q.need(row['key'] not in all_keys, 'duplicate domain tuple')
            all_keys.add(row['key'])
            role = 'observer' if section in ('lifecycle', 'guards') else 'ordinary'
            cfg = row_configs[role]
            cfg['_binary_stats'].setdefault(row['profile'], got['binary_stat_before'])
            if section == 'guards':
                source_case = contracts.cases[row['case']['source_case_id']]
                sources = contracts.source_bytes(source_case)
                guards.check_guard(row, got, cfg, contracts, sources, row_tools)
                baseline = capsule.json(got['ordinary_receipt'])
                ordinary = row_configs['ordinary']
                ordinary['_binary_stats'].setdefault(row['profile'], baseline['binary_stat_before'])
                q.need(baseline['executed'] is True and baseline['status'] == 'pass', 'guard ordinary counterpart missing')
                guards.check_guard(row, baseline, ordinary, contracts, sources, row_tools)
                q.need(all(got['process'][field] == baseline['process'][field] for field in ('status', 'stdout', 'stderr')) and got['directory_after'] == baseline['directory_after'], 'guard passivity changed')
                expected = copy.deepcopy(next(case for case in contracts.tables['lifecycle']['public_cases'] if case['id'] == row['case']['source_case_id']))
                if row['group'] == 'occupied':
                    expected['file_counts'] = {str(Path(got['fixture_directory']) / name): value for name, value in expected['file_counts'].items()}
                sys.modules['compare'].lifecycle(got['trace'], expected, contracts.tables['lifecycle']['counter_order'], contracts.tables['lifecycle']['file_counter_order'])
            else:
                sources = run.tuple_sources(row, contracts, predecessors)
                if section == 'lifecycle':
                    baseline = capsule.json(got['ordinary_receipt'])
                    ordinary = row_configs['ordinary']
                    ordinary['_binary_stats'].setdefault(row['profile'], baseline['binary_stat_before'])
                    q.need(baseline['executed'] is True and baseline['status'] == 'pass', 'lifecycle ordinary counterpart missing')
                    run.lifecycle_compare(row, baseline, got, ordinary, cfg, contracts, sources, row_tools)
                else:
                    run.core_compare(section, row, got, cfg, contracts, sources, row_tools, predecessors)
        by_profile = dict(Counter(row['profile'] for row in values if row['executed']))
        q.need(by_profile == plan['counts_from_frozen_rosters'][host][section], 'missing/zero public profile')
        counts[section] = {'executed': actual_counts['executed'], 'profiles': by_profile,
                           'excluded': actual_counts['excluded_host'], 'unavailable': actual_counts['unavailable_host']}
    q.need(replaced == set(replacement_records), 'missing hosted replacement tuple')
    return counts, all_keys


def parser_join(capsule, repo, contract_root, plan):
    seal = capsule.json(capsule.manifest['parser_seal'])
    verify_parser_seal(seal, capsule.raw)
    return parser_records(capsule, repo, contract_root, plan, seal)


def parser_records(capsule, repo, contract_root, plan, seal):
    """Transport predicate only; the caller must first verify its comparison seal."""
    result = capsule.json(seal['comparison'])
    q.need(result['status'] == 'pass' and result['issues'] == [], 'parser comparator failed')
    session = capsule.json(result['session'])
    q.need(session['checkout']['head'] == plan['provenance']['checkout_head'] and session['checkout']['tree'] == plan['provenance']['checkout_tree'] and
           session['checkout']['historical_source_equivalent'] is False and session['checkout']['current_source_bound'] is True, 'parser current checkout identity')
    q.need(session['host']['os'] == 'linux' and session['host']['architecture'] == 'x86_64' and session['host']['python_pointer_width'] == 64, 'parser actual host')
    root = session['root']
    adapter = q.module('_unit4_portable_admission', Path(repo) / q.PARSER / 'portable.py')
    authority = adapter.authority()
    source = authority['current_source']
    compiler = adapter.compiler_map(authority)
    q.need(len(session['checkout']['compiler_files']) == len(compiler) and session['checkout']['compiler_files'] == compiler and
           session['checkout']['current_source_manifest_sha256'] == session['current_source_manifest']['sha256'] == q.CURRENT_SHA and
           session['checkout']['reviewed_source_head'] == source['reviewed_source_head'] and
           session['checkout']['source_only_tree'] == source['source_only_tree'], 'parser exact current source map/checkpoint')
    q.need(result['portable_authority_sha256'] == session['authority_sha256'] == adapter.AUTHORITY_SHA and
           session['adapter']['sha256'] == q.identity(Path(repo) / q.PARSER / 'portable.py')['sha256'], 'stale parser adapter/authority')
    comparator, proof = adapter.comparator()
    contract = comparator.load_contract(contract_root)
    effective, receipt = comparator.admit_contract_amendment(contract, contract_root, adapter.effective_authority(authority))
    q.need(result['derivation'] == proof and all(result.get(key) == value for key, value in receipt.items()), 'parser comparator/effective contract identity')
    sys.path.insert(0, str(Path(repo) / q.PARSER_FROZEN / 'frozen/helpers'))
    normalizer = q.module('_unit4_frozen_collector', Path(repo) / q.PARSER_FROZEN / 'frozen/helpers/run.py')
    expected_case_ids = [case['id'] for case in contract['cases']]
    expected_modes = sum(1 + ('relation_to_original' in case['expected']) for case in contract['cases'])
    q.need(len(expected_case_ids) == 248 and expected_modes == 319, 'parser frozen case/mode roster changed')
    binaries, nonces, total, pairs = set(), set(), 0, 0
    for profile in q.PROFILES:
        builds = []
        for control in (False, True):
            build_root = original_child(root, ('build-control-' if control else 'build-') + profile)
            envelope = capsule.json(capsule.named(original_child(build_root, 'portable-build.json')))
            q.need(envelope['session'] == result['session'] and envelope['profile'] == profile and envelope['control'] is control, 'parser build session/profile/control')
            invocation = capsule.json(envelope['invocation'])
            build = capsule.json(envelope['receipt'])
            q.need(invocation['exit_code'] == 0 and invocation['host'] == session['host'] and
                   session['prepared_ns'] <= invocation['started_ns'] <= invocation['finished_ns'], 'parser actual fresh build invocation')
            q.need(build['profile'] == profile and build['control'] is control and build['status'] == 'built' and build['exit_code'] == 0 and
                   build['rustc_version'] == authority['recipe']['rustc_verbose'] and build['target'] == authority['recipe']['target'], 'parser actual build recipe')
            q.need(build['control'] is control and
                   build['candidate_source_manifest_sha256'] == authority['current']['current_candidate_source_manifest_sha256'] and
                   build['observer_source_sha256'] == authority['helper_manifest_sha256'] and
                   build['overlay_manifest'] == session['control_overlay' if control else 'overlay'], 'parser actual current build source/overlay')
            q.need(build['binary']['sha256'] not in binaries, 'parser binary reused across builds')
            binaries.add(build['binary']['sha256'])
            rows = [q.loads(line) for line in capsule.raw(build['stdout']).splitlines()]
            emitted = [row for row in rows if row.get('reason') == 'compiler-artifact' and row.get('executable')]
            q.need(len(emitted) == 1 and emitted[0]['fresh'] is False and emitted[0]['executable'] == build['binary']['path'], 'missing/fresh-cached compiler artifact')
            q.need([row for row in rows if row.get('reason') == 'build-finished'] == [{'reason': 'build-finished', 'success': True}], 'Cargo did not finish')
            for bound in (invocation['stdout'], invocation['stderr'], build['stderr']):
                capsule.raw(bound)
            builds.append((build, envelope))
        collection = capsule.json(capsule.named(original_child(root, 'collect-' + profile, 'portable-collection.json')))
        q.need(collection['session'] == result['session'] and collection['profile'] == profile, 'parser collection session/profile')
        manifest = capsule.json(collection['manifest'])
        invocation = capsule.json(collection['invocation'])
        q.need(invocation['exit_code'] == 0 and invocation['host'] == session['host'] and manifest['host_runtime'] == session['host'], 'parser collection actual host/exit')
        q.need(manifest['status'] == 'collected' and manifest['profile'] == profile and manifest['build_receipt'] == builds[0][1]['receipt'] and
               manifest['authority_checkpoint'] == result['session'], 'parser actual collection build/session')
        q.need(manifest['requested_case_ids'] == expected_case_ids and [row['case_id'] for row in manifest['case_receipts']] == expected_case_ids,
               'missing/duplicate/out-of-order parser case')
        actual_count = 0
        with capsule.path(manifest['observations']).open(encoding='utf-8') as normalized:
            for case, observation in zip(contract['cases'], manifest['case_receipts']):
                q.need(observation['status'] == 'executed' and observation['exit_code'] == 0, 'parser case not executed')
                nonce = observation['execution_id']
                q.need(re.fullmatch('[0-9a-f]{32}', nonce) is not None and nonce not in nonces, 'parser execution nonce reused')
                nonces.add(nonce)
                stdout = capsule.raw(observation['stdout']).decode()
                capsule.raw(observation['stderr'])
                q.need(stdout.count('UNIT4_EXECUTED ' + nonce) == 1 and re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;', stdout), 'zero-execution parser case')
                request = capsule.json(observation['request'])
                q.need(capsule.raw(request['source']) == base64.b64decode(case['source']['base64'], validate=True), 'parser actual source mismatch')
                raw = capsule.json(observation['raw'])
                derived = normalizer.normalize(raw, case, builds[0][0], nonce, session['host'])
                q.need(observation['observations'] == len(derived), 'parser case mode count')
                for row in derived:
                    line = normalized.readline()
                    q.need(line and q.loads(line) == row, 'changed/summary-only normalized parser row')
                    q.need(row['executed'] is True and row['observation_complete'] is True, 'incomplete parser mode')
                    actual_count += 1
            q.need(normalized.readline() == '', 'extra normalized parser row')
        q.need(manifest['case_count'] == len(expected_case_ids) and manifest['observation_count'] == actual_count == expected_modes, 'parser profile incomplete')
        total += actual_count
        passive = capsule.json(capsule.named(original_child(root, 'passivity-' + profile, 'portable-passivity.json')))
        report = capsule.json(passive['report'])
        q.need(passive['session'] == result['session'] and passive['profile'] == profile and report['status'] == 'pass' and
               report['instrumented'] == builds[0][1]['receipt'] and report['control'] == builds[1][1]['receipt'], 'parser passivity build/session')
        prescribed = authority['recipe']['passivity']
        q.need([row['case'] for row in report['results']] == [name for name, _ in prescribed['cases']], 'missing passivity case')
        for row, (name, source) in zip(report['results'], prescribed['cases']):
            q.need(capsule.raw(row['source']) == source.encode() and row['pairs'] == 2 and len(row['receipts']) == 2, 'passivity source/mode scope')
            observations = []
            for process in row['receipts']:
                q.need(process['exit_code'] == 0, 'passivity process failed')
                raw = capsule.json(process['raw'])
                nonce = raw['nonce']
                q.need(nonce not in nonces and raw['source_utf8'] == source and raw['case_id'] == name, 'passivity raw identity')
                nonces.add(nonce)
                stdout = capsule.raw(process['stdout']).decode()
                q.need(stdout.count('UNIT4_EXECUTED ' + nonce) == 1 and 'test result: ok. 1 passed; 0 failed; 0 ignored;' in stdout, 'zero-execution passivity')
                capsule.raw(process['stderr'])
                q.need([item['mode'] for item in raw['observations']] == prescribed['modes'] and all(item['executed'] is True for item in raw['observations']), 'missing passivity mode')
                observations.append(raw['observations'])
            for left, right in zip(*observations):
                q.need(all(left[field] == right[field] for field in prescribed['compare_fields']), 'passivity changed')
                pairs += 1
    q.need(total == result['observations'] == result['expected_observations'] == 638 and pairs == 12 and len(binaries) == 4, 'parser total incomplete')
    q.need([(row['profile'], row['pairs']) for row in result['ordinary_passivity']] == [('debug', 6), ('release', 6)], 'parser passivity result incomplete')
    return {'actual_observations': total, 'ordinary_passivity_pairs': pairs, 'fresh_builds': len(binaries),
            'comparison': seal['comparison'], 'seal': capsule.manifest['parser_seal'],
            'boundary': 'Exact normalized/raw case and mode transport, actual source/nonce/stream/build identities, and unchanged inputs sealed around fresh same-host semantic comparison. Binary bytes, toolchain replay and original path restoration require the separate full evidence archive.'}


def join_body(args, output, progress):
    repo = Path(args.repo).resolve()
    provenance = q.admit(repo, args.expected_head, args.event_sha)
    progress['provenance'] = provenance
    job_results = q.loads(args.job_results)
    progress['upstream_job_results'] = {str(key)[:100]: value.get('result') for key, value in job_results.items() if isinstance(value, dict)}
    q.need(set(job_results) == {'unit4-linux', 'unit4-portable-hosts'} and all(value['result'] == 'success' for value in job_results.values()), 'required actual-host job failed/skipped/cancelled')
    transport = q.module('_unit4_join_transport', repo / q.TRANSPORT / 'transport.py')
    manifest = transport.materialize(repo / q.TRANSPORT, output / 'contracts')
    mapping = {row['logical_path']: str(output / 'contracts' / row['archive_path']) for row in manifest['members']}
    roots = {key: Path(mapping[value['logical_path']]).parent for key, value in manifest['active_contracts'].items()}
    c = q.public_modules(repo)[0]
    contracts = c.Contracts(roots['public'], mapping, repo / q.AMENDMENT)
    expected_ci = {key: os.environ.get(key) for key in ('GITHUB_RUN_ID', 'GITHUB_RUN_ATTEMPT', 'GITHUB_WORKFLOW', 'GITHUB_EVENT_NAME', 'GITHUB_REPOSITORY')}
    q.need(all(expected_ci.values()), 'final join requires actual workflow/run provenance')
    archives = sorted(Path(args.artifacts).glob('*/compact.tar.xz'))
    progress['available_capsules'] = [q.identity(path) for path in archives]
    q.need(len(archives) == len(q.HOSTS), 'missing/extra actual host artifact')
    hosts, global_keys, host_keys, totals, parser_result = {}, set(), {}, Counter(), None
    for index, archive in enumerate(archives):
        capsule = ReadCapsule(q.unpack(archive, output / ('host-' + str(index))))
        metadata = capsule.manifest
        host = metadata['host']
        progress['available_host_outcomes'][host] = 'checking'
        q.need(host in q.HOSTS and host not in hosts, 'duplicate/unknown actual host')
        q.need(metadata['ci'] == expected_ci, 'stale workflow run/attempt')
        q.need(provenance_key(metadata['provenance']) == provenance_key(provenance), 'stale head/tree/source/component identity')
        plan = capsule.json(metadata['plan'])
        driver = capsule.json(metadata['driver'])
        q.need(driver['status'] == 'pass' and driver['plan'] == metadata['plan'] and driver['public'] == metadata['public_finalization'], 'stale/missing host terminal result')
        q.need(driver['execution_mode'] == 'hosted-required' and driver['provenance'] == metadata['provenance'], 'local-only/stale execution is not hosted qualification')
        q.need(plan['measured_host']['name'] == host and plan['provenance'] == metadata['provenance'] and plan['ci'] == expected_ci and
               plan['required_hosts'] == list(q.HOSTS) and plan['sections'] == list(q.SECTIONS) and plan['profiles'] == list(q.PROFILES), 'host plan identity/domain/profile')
        q.need(plan['effective_authority'] == contracts.effective_identity and plan['parser_required'] is (host == 'Linux x86_64') and
               plan['hosted_root_required'] is (host == 'Linux x86_64'), 'missing mandatory host route')
        expected_counts = {name: {section: dict(Counter(row['profile'] for row in contracts.roster(section, name) if row['scope'] == 'execute'))
                                for section in q.SECTIONS} for name in q.HOSTS}
        q.need(plan['counts_from_frozen_rosters'] == expected_counts, 'changed cross-host plan counts')
        names = ['01-contract-verification', '02-contract-materialization', '03-lifecycle-preparation', '04-build-ordinary', '04-build-observer']
        if host == 'Linux x86_64': names.append('05-stage-llvm')
        names.append('06-public-collection')
        if host == 'Linux x86_64':
            names += ['07-hosted-capability-join', '08-parser-prepare']
            for profile in q.PROFILES:
                names += [prefix + profile for prefix in ('09-parser-build-', '10-parser-control-', '11-parser-passivity-', '12-parser-collect-')]
            names.append('13-parser-comparison')
        commands = [capsule.json(bound) for bound in driver['stages']]
        q.need([command['name'] for command in commands] == names, 'missing/reordered qualification stage')
        for command in commands:
            expected_exit = 2 if host == 'Linux x86_64' and command['name'] == '06-public-collection' else 0
            q.need(command['status'] == expected_exit and command['accepted_exit_codes'] == [expected_exit] and
                   not command['timed_out'] and not command['stream_limit_exceeded'], 'qualification stage exit was swallowed')
            for stream in ('stdout', 'stderr'):
                q.need(q.sha(capsule.raw(command['streams'][stream])) == command[stream + '_sha256'], 'stage command stream changed')
        counts, keys = public_join(capsule, contracts, repo, plan)
        q.need(not global_keys.intersection(keys), 'cross-host duplicate actual tuple')
        global_keys.update(keys)
        host_keys[host] = list(keys)
        hosts[host] = {'capsule': q.identity(archive), 'counts': counts, 'plan': metadata['plan'], 'driver': metadata['driver']}
        progress['available_host_outcomes'][host] = {'status': 'public-rows-verified', 'counts': counts}
        totals.update({section: record['executed'] for section, record in counts.items()})
        if host == 'Linux x86_64':
            q.need(metadata['parser_seal'] is not None, 'Linux parser seal missing')
            parser_result = parser_join(capsule, repo, roots['parser'], plan)
        else:
            q.need(metadata['parser_seal'] is None and driver['parser'] is None, 'unexpected foreign parser claim')
    q.need(global_keys == final_coverage(contracts, host_keys) and parser_result is not None, 'global frozen roster incomplete')
    q.save(output / 'comparison.json', {'schema': 'oxid-unit4-cross-host-qualification-v1', 'status': 'pass',
           'provenance': provenance, 'ci': expected_ci, 'hosts': hosts, 'actual_section_observations': dict(totals),
           'actual_public_observations_total': len(global_keys), 'parser': parser_result,
           'global_host_qualification': True, 'summary_only_substitution': False})
    print(json.dumps({'status': 'pass', 'hosts': len(hosts), 'public_observations': len(global_keys), 'parser_observations': parser_result['actual_observations']}))


def join(args):
    # Create the fresh evidence destination before admission so every rejection
    # has a bounded structured result. Occupied outputs are never overwritten.
    output = q.fresh(args.output)
    progress = {'expected_head': args.expected_head, 'event_sha': args.event_sha,
                'available_host_outcomes': {}, 'available_capsules': []}
    try:
        return join_body(args, output, progress)
    except BaseException as error:
        q.save(output / 'comparison.json', {'schema': 'oxid-unit4-cross-host-qualification-v1', 'status': 'fail',
               'global_host_qualification': False, 'failure_type': type(error).__name__,
               'failure': str(error)[:2000], **progress})
        raise


def main():
    parser = argparse.ArgumentParser()
    for name in ('repo', 'artifacts', 'output', 'expected-head', 'event-sha', 'job-results'):
        parser.add_argument('--' + name, required=True)
    join(parser.parse_args())


if __name__ == '__main__':
    try:
        main()
    except (q.Reject, OSError, KeyError, TypeError, ValueError) as error:
        print('Unit4 final join rejected: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
