#!/usr/bin/env python3
"""Execute and replay new public-v3 qualification; historical receipts stay separate."""
import sys
sys.dont_write_bytecode = True
import argparse
import collections
import copy
import gzip
import json
import os
import shutil
import sys
from pathlib import Path
from contracts import (Contracts, Reject, need, sha, load, save, binding, verify, host, NAMES, FREEZE_SHA,
                       receipt_identity, check_inventory, EXCLUDED_REASON, REMOTE_REASON, CAPABILITY_REASON)
import compare
import runtime as rt
from predecessors import Predecessors, original_generator_check

ADAPTER_ROOT = Path(__file__).resolve().parent

PACKAGE_FILES = {'observer-combined-v1.patch', 'README.md', 'authority.py', 'authority_controls.py', 'build.py', 'compare.py', 'contracts.py', 'guards.py', 'no_tool_trap.rs', 'predecessors.py', 'process_tree.py', 'run.py', 'runtime.py', 'selftest.py', 'tool_wrapper.py'}

def adapter_identity():
    paths = sorted(ADAPTER_ROOT.iterdir(), key=lambda path: path.name)
    need({p.name for p in paths} == PACKAGE_FILES and all(p.is_file() and not p.is_symlink() for p in paths), 'adapter package exact membership')
    return [{'path': p.name, 'bytes': p.stat().st_size, 'sha256': sha(p.read_bytes())} for p in paths]

def skipped(row):
    return {**receipt_identity(row), 'executed': False, 'status': row['scope'],
            'reason': EXCLUDED_REASON if row['scope'] == 'excluded-host' else REMOTE_REASON}

def tuple_sources(row, contracts, predecessors):
    case = row['case']
    if row['group'] in ('Unit2', 'Unit3'):
        return predecessors.rows[case['family'], case['id']]['sources']
    if row['group'] == 'heldout':
        return {name: bytes.fromhex(data) for name, data in case['files_hex'].items()}
    return contracts.source_bytes(contracts.cases[row['case_id']])

def collect(row, cfg, evidence, sources, env):
    evidence.mkdir(parents=True)
    fixture = evidence / 'fixture'
    wanted_sources = rt.materialize(sources, fixture)
    got = {**receipt_identity(row), 'executed': True, 'status': 'fail', 'candidate_binding_sha256': cfg['_sha256'],
           'freeze_sha256': FREEZE_SHA, 'adapter': adapter_identity(), 'source_files': wanted_sources,
           'fixture_directory': str(fixture), 'directory_before': rt.inventory(fixture)}
    if row['group'] in ('Unit2', 'Unit3'):
        got['projection_kind'] = row['case']['kind']
        got['expected_authority_line_sha256'] = row['case']['expected_authority_line_sha256']
        got['expected_projection_sha256'] = row['case']['expected_projection_sha256']
        if 'source_line_sha256' in row['case']:
            got['source_line_sha256'] = row['case']['source_line_sha256']
    binary = cfg['binaries'][row['profile']]
    got['binary'] = binary
    got['binary_stat_before'] = rt.file_identity(binary['path'])
    need(got['binary_stat_before'] == cfg['_binary_stats'][row['profile']], 'binary changed before invocation')
    setup = row.get('_filesystem_setup', row['case'].get('filesystem_setup'))
    if setup:
        got['capability_proof'] = rt.capability(fixture, setup)
        if got['capability_proof']['read_open_succeeded']:
            rt.restore_capability(fixture, got['capability_proof'])
            got.update(executed=False, status='unsupported-capability', reason=CAPABILITY_REASON)
            got['directory_after'] = rt.inventory(fixture)
            return got
    log = evidence / 'tools.jsonl'
    trace = evidence / 'lifecycle.jsonl'
    per_env = {**env, 'UNIT4_TOOL_LOG': str(log), 'UNIT4_TOOL_EVIDENCE': str(evidence), 'UNIT4_ADAPTER_ROOT': str(ADAPTER_ROOT)}
    if cfg['kind'] == 'unit4-public-v3-lifecycle-observer':
        per_env['UNIT4_LIFECYCLE_LOG'] = str(trace)
    try:
        got['process'] = rt.process([binary['path']] + row['argv_template'][1:], fixture, per_env)
    finally:
        if setup:
            rt.restore_capability(fixture, got['capability_proof'])
    save(evidence / 'process.json', got['process'])
    for stream in ('stdout', 'stderr'):
        (evidence / stream).write_bytes(got['process'][stream].encode())
    got['tool_invocations'] = rt.read_events(log)
    got['emitted_llvm'] = binding(evidence / 'program.ll') if (evidence / 'program.ll').exists() else None
    got['trace'] = rt.read_events(trace)
    if got['process']['status'] == 0 and row['argv_template'][1] == 'compile':
        got['native_execution'] = rt.native(fixture, evidence, sources, per_env)
    got['directory_after'] = rt.inventory(fixture)
    got['binary_stat_after'] = rt.file_identity(binary['path'])
    for stream in ('stdout', 'stderr'):
        (evidence / stream).write_bytes(got['process'][stream].encode())
    return got

def check_common(row, got, cfg, sources, tools, expected_native=None, native_required=None):
    need(all(got.get(key) == value for key, value in receipt_identity(row).items()), 'tuple identity')
    need(got['freeze_sha256'] == FREEZE_SHA and got['candidate_binding_sha256'] == cfg['_sha256'], 'candidate/contract identity')
    need(got['adapter'] == adapter_identity(), 'adapter implementation identity')
    if not got['executed']:
        need(got['status'] == 'unsupported-capability', 'nonexecuted candidate receipt')
        need(got['directory_before'] == got['directory_after'], 'capability skip source preservation')
        wanted = [{'path': name, 'bytes': len(data), 'sha256': sha(data)} for name, data in sorted(sources.items())]
        need(got['source_files'] == wanted and rt.source_map(got['directory_before']) == wanted, 'capability skip source map')
        proof = got['capability_proof']
        need(proof['source_restored'] and proof['content_before_sha256'] == proof['content_after_sha256'] == sha(sources[proof['target']]), 'capability skipped source bytes')
        return
    need(got['binary'] == cfg['binaries'][row['profile']], 'wrong executable identity')
    need(got['binary_stat_before'] == got['binary_stat_after'] == cfg['_binary_stats'][row['profile']], 'executable mutation')
    wanted = [{'path': name, 'bytes': len(data), 'sha256': sha(data)} for name, data in sorted(sources.items())]
    need(got['source_files'] == wanted and rt.source_map(got['directory_before']) == wanted, 'input source identity')
    p = got['process']
    need(not p['timed_out'] and not p['stream_limit_exceeded'] and p['completed_ns'] >= p['started_ns'], 'process incomplete')
    need(p['argv'] == [got['binary']['path']] + row['argv_template'][1:] and p['cwd'] == got['fixture_directory'], 'invocation argv/cwd')
    for stream in ('stdout', 'stderr'):
        need(sha(p[stream].encode()) == p[stream + '_sha256'], 'process stream bytes/hash')
    native_required = p['status'] == 0 and row['argv_template'][1] == 'compile' if native_required is None else native_required
    after = got['directory_after']
    delta = [r['path'] for r in after if r not in got['directory_before']]
    need(delta == (['out.bin'] if native_required else []), 'unexpected directory delta')
    need(all(r in after for r in got['directory_before']), 'changed/removed source')
    need(rt.source_map(after, exclude=('out.bin',)) == wanted, 'post-run source identity')
    if 'capability_proof' in got:
        proof = got['capability_proof']
        need(not proof['read_open_succeeded'] and proof['source_restored'] and proof['mode_before_candidate'] == proof['mode_after_candidate'] == 0, 'permission control proof')
    rt.check_tools(got['tool_invocations'], tools, native_required, got['emitted_llvm'])
    if native_required:
        need(expected_native is not None and 'native_execution' in got, 'successful compile missing frozen/native envelope')
        rt.check_native(got['native_execution'], expected_native, sources)
    else:
        need('native_execution' not in got, 'orphan native receipt')

def core_compare(section, row, got, cfg, contracts, sources, tools, predecessors):
    policy = contracts.tables['public']['new_policy_diagnostic_bounds']
    expected_native = row['case'].get('native_execution')
    check_common(row, got, cfg, sources, tools, expected_native)
    if not got['executed']:
        return
    if section in ('public', 'original'):
        compare.public(row, got['process'], policy)
        if row['format'] == 'json':
            _, diagnostics = compare.envelope(got['process'], row['argv_template'][1])
            compare.valid_origins(diagnostics, sources)
        if row['group'] == 'literal_cases':
            delta = [r['path'] for r in got['directory_after'] if r not in got['directory_before']]
            need(delta == row['observation']['directory_delta'], 'literal directory delta')
    elif section == 'predecessors':
        need(got['projection_kind'] == row['case']['kind'], 'predecessor projection classification')
        for field in ('expected_authority_line_sha256', 'expected_projection_sha256', 'source_line_sha256'):
            if field in row['case']:
                need(got[field] == row['case'][field], 'predecessor authority identity: ' + field)
        predecessors.compare(row, got['process'], policy)

def lifecycle_compare(row, ordinary, observed, cfg, observer_cfg, contracts, sources, tools):
    if not observed['executed']:
        need(not ordinary['executed'], 'observer capability inconsistent')
        check_common(row, ordinary, cfg, sources, tools)
        check_common(row, observed, observer_cfg, sources, tools)
        return
    case = row['case']
    source_case = contracts.cases.get(row['case_id'])
    native_expected = source_case.get('native_execution') if source_case else None
    for got, config in ((ordinary, cfg), (observed, observer_cfg)):
        check_common(row, got, config, sources, tools, native_expected)
    for field in ('status', 'stdout', 'stderr'):
        need(observed['process'][field] == ordinary['process'][field], 'observer passivity: ' + field)
    need(observed['directory_after'] == ordinary['directory_after'], 'observer directory/ELF passivity')
    if row['group'] == 'public':
        mainrow = next(r for r in contracts.roster('public', row['execution_host']) if r['case_id'] == row['case_id'] and r['observation_index'] == row['observation_index'] and r['profile'] == row['profile'] and r['host'] == row['host'])
        compare.public(mainrow, ordinary['process'], contracts.tables['public']['new_policy_diagnostic_bounds'])
    else:
        need(ordinary['process']['status'] == case['expected_status'], 'heldout expected process status')
    if row['format'] == 'json' and ordinary['process']['stdout']:
        # Invalid CLI combinations can intentionally report only stderr, so only
        # successfully framed typed-preview streams have source origins here.
        if ordinary['process']['stdout'].startswith('{"schema_version":'):
            _, diagnostics = compare.envelope(ordinary['process'], row['argv_template'][1])
            compare.valid_origins(diagnostics, sources)
    table = contracts.tables['lifecycle']
    observed['logical_lifecycle'] = compare.lifecycle(observed['trace'], case, table['counter_order'], table['file_counter_order'])
    need(ordinary['trace'] == [], 'ordinary binary emitted observer events')

def aggregate(rows, receipts):
    counts = check_inventory(rows, receipts)
    failures = [{'key': r['key'], 'failure': r.get('failure', 'predicate failure')} for r in receipts if r['status'] == 'fail']
    need(not failures, json.dumps(failures[:25]))
    return {**counts, 'status': ('NOT_APPLICABLE_THIS_HOST' if counts['required_local'] == 0 else ('INCOMPLETE_LOCAL_CAPABILITY' if counts['unsupported_capability'] else 'QUALIFIED_LOCAL')),
            'global_host_qualification': counts['unavailable_host'] == 0 and counts['unsupported_capability'] == 0}

def negative_controls(rows, receipts, checker):
    aggregate(rows, receipts)
    index = next((i for i, r in enumerate(receipts) if r['executed']), None)
    controls = []
    def run(name, change):
        mutated = copy.deepcopy(receipts)
        change(mutated)
        try:
            aggregate(rows, mutated)
            for row, got in zip(rows, mutated):
                if got['executed']:
                    checker(row, got)
        except (Reject, KeyError, TypeError, ValueError, AssertionError) as error:
            controls.append({'control': name, 'status': 'REJECTED', 'reason': str(error)[:500]})
        else:
            raise Reject('negative control accepted: ' + name)
    if index is None:
        need(not any(row['scope'] == 'execute' for row in rows), 'unexecuted required local domain')
        index = 0
        run('zero', lambda x: x.clear())
        run('missing', lambda x: x.pop(0))
        run('duplicate', lambda x: x.append(copy.deepcopy(x[0])))
        run('extra', lambda x: x.append({**copy.deepcopy(x[0]), 'key': 'extra'}))
        run('fabricated-execution', lambda x: x[0].update(executed=True, status='pass'))
        return controls
    run('zero', lambda x: x.clear())
    run('missing', lambda x: x.pop(index))
    run('duplicate', lambda x: x.append(copy.deepcopy(x[index])))
    run('extra', lambda x: x.append({**copy.deepcopy(x[index]), 'key': 'extra'}))
    for name, field, value in (('false-skip', 'executed', False), ('failed-predicate', 'status', 'fail'), ('wrong-host', 'host', 'other'), ('wrong-profile', 'profile', 'other'), ('wrong-format', 'format', 'other'), ('stale-contract', 'freeze_sha256', '0' * 64), ('stale-candidate', 'candidate_binding_sha256', '0' * 64)):
        run(name, lambda x, k=field, v=value: x[index].__setitem__(k, v))
    run('wrong-argv', lambda x: x[index]['process']['argv'].append('--invented'))
    run('changed-source', lambda x: x[index]['source_files'][0].__setitem__('sha256', '0' * 64))
    run('wrong-stream', lambda x: x[index]['process'].__setitem__('stdout', 'wrong'))
    native_index = next((i for i, r in enumerate(receipts) if r.get('native_execution')), None)
    if native_index is not None:
        run('missing-native', lambda x: x[native_index].pop('native_execution'))
        run('sources-available', lambda x: x[native_index]['native_execution'].__setitem__('sources_unavailable', []))
        run('changed-ELF', lambda x: x[native_index]['native_execution']['copied_elf'].__setitem__('sha256', '0' * 64))
        run('missing-tools', lambda x: x[native_index].__setitem__('tool_invocations', []))
    traced = next((i for i, r in enumerate(receipts) if r.get('trace')), None)
    if traced is not None:
        run('missing-passivity-binding', lambda x: x[traced].pop('ordinary_receipt'))
        run('missing-trace', lambda x: x[traced].__setitem__('trace', []))
        run('duplicate-checker', lambda x: x[traced]['trace'].append({'event': 'checker_attempts', 'subject': 'project'}))
        run('source-reopen', lambda x: x[traced]['trace'].append({'event': 'source_open_attempt', 'subject': 'main.ox'}))
    race_index = next((i for i, r in enumerate(receipts) if r['group'] == 'race' and r['executed']), None)
    if race_index is not None:
        run('changed-competitor', lambda x: x[race_index]['output_after'].__setitem__('sha256', '0' * 64))
        run('missing-race-barrier', lambda x: next(c for c in x[race_index]['tool_invocations'] if 'race' in c).pop('race'))
    return controls

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=('inventory', 'run'))
    parser.add_argument('--contracts', required=True)
    parser.add_argument('--path-map')
    parser.add_argument('--amendment-root')
    parser.add_argument('--binding')
    parser.add_argument('--observer-binding')
    parser.add_argument('--out', required=True)
    parser.add_argument('--sections', default='public,original,predecessors,lifecycle,guards')
    parser.add_argument('--llvm', default=os.environ.get('OXID_LLVM_BIN'))
    parser.add_argument('--libs', default=os.environ.get('LD_LIBRARY_PATH', ''))
    parser.add_argument('--runtime-receipt')
    parser.add_argument('--pilot', type=int)
    args = parser.parse_args()
    need(args.pilot is None or args.pilot > 0, '--pilot must be positive')
    out = Path(args.out).resolve()
    out.mkdir(parents=True, exist_ok=False)
    contracts = Contracts(args.contracts, load(args.path_map) if args.path_map else None, args.amendment_root)
    sections = args.sections.split(',')
    need(set(sections) <= set(NAMES) and len(set(sections)) == len(sections), 'section selection')
    rosters = {name: contracts.roster(name) for name in sections}
    path_map_binding = binding(args.path_map) if args.path_map else None
    save(out / 'input-integrity.json', {'freeze_sha256': FREEZE_SHA, 'verified': contracts.verified, 'adapter': adapter_identity(), 'path_map': path_map_binding, 'effective_authority': contracts.effective_identity})
    for name, rows in rosters.items():
        save(out / (name + '-roster.json'), [receipt_identity(row) for row in rows])
    if args.command == 'inventory':
        report = {name: {'total': len(rows), 'local': sum(r['scope'] == 'execute' for r in rows)} for name, rows in rosters.items()}
        save(out / 'inventory.json', report)
        print(json.dumps(report))
        return 0
    need(args.binding and args.path_map, 'candidate binding and immutable path map required for comparison')
    cfg = rt.candidate_binding(args.binding)
    observer_cfg = rt.candidate_binding(args.observer_binding) if args.observer_binding else None
    need(cfg['kind'] == 'unit4-public-v3-candidate', 'ordinary binding kind')
    if 'lifecycle' in sections or 'guards' in sections:
        need(observer_cfg and observer_cfg['kind'] == 'unit4-public-v3-lifecycle-observer', 'observer binding required')
        need(observer_cfg['base_source_manifest_sha256'] == cfg['source_manifest']['sha256'], 'observer base source binding')
    for config in (cfg, observer_cfg):
        if config:
            config['_binary_stats'] = {p: rt.file_identity(b['path']) for p, b in config['binaries'].items()}
    needs_native = any(row['scope'] == 'execute' and (row['group'] == 'race' or (row['argv_template'][1] == 'compile' and contracts.cases.get(row['case_id'], {}).get('native_execution'))) for rows in rosters.values() for row in rows)
    runtime_binding = None
    if needs_native:
        need(host() == 'Linux x86_64' and args.llvm and args.runtime_receipt, 'native tuples require Linux x86_64, LLVM and selected-runtime receipt')
        runtime_binding = rt.staged_runtime(args.runtime_receipt, args.libs)
        env, tools = rt.setup_tools(out, args.llvm, args.libs)
    else:
        env, tools = rt.setup_no_tool_traps(out)
    save(out / 'tools.json', {'tools': tools, 'selected_runtime': runtime_binding, 'mode': 'real-native' if needs_native else 'no-native-traps'})
    predecessors = Predecessors(contracts) if 'predecessors' in sections else None
    if 'original' in sections:
        original_generator_check(contracts, out / 'original-source-generation', cfg['source_root'])
    results = {}
    for section, complete_rows in rosters.items():
        rows = complete_rows
        if args.pilot:
            wanted = {r['key'] for r in rows if r['scope'] == 'execute'}
            wanted = set(sorted(wanted)[:args.pilot])
            rows = [r for r in rows if r['key'] in wanted]
        section_out = out / section
        section_out.mkdir()
        receipts = []
        ordinary_receipts = {}
        sources_by_key = {}
        if section == 'guards':
            from guards import run_guards
            receipts = run_guards(rows, section_out, cfg, observer_cfg, contracts, env, tools)
        else:
            for index, row in enumerate(rows):
                if row['scope'] != 'execute':
                    receipts.append(skipped(row))
                    continue
                sources = tuple_sources(row, contracts, predecessors)
                if section == 'lifecycle' and row['group'] == 'public':
                    row['_filesystem_setup'] = contracts.cases[row['case_id']].get('filesystem_setup')
                sources_by_key[row['key']] = sources
                evidence = section_out / ('%05d' % index)
                got = {**receipt_identity(row), 'executed': True, 'status': 'fail'}
                try:
                    got = collect(row, cfg, evidence / 'ordinary', sources, env)
                    if section == 'lifecycle':
                        ordinary_receipts[row['key']] = got
                        observed = collect(row, observer_cfg, evidence / 'observer', sources, env)
                        lifecycle_compare(row, got, observed, cfg, observer_cfg, contracts, sources, tools)
                        if got['executed']:
                            got['status'] = 'pass'
                            observed['status'] = 'pass'
                        save(evidence / 'ordinary' / 'receipt.json', got)
                        observed['ordinary_receipt'] = binding(evidence / 'ordinary' / 'receipt.json')
                        got = observed
                    else:
                        core_compare(section, row, got, cfg, contracts, sources, tools, predecessors)
                        if got['executed']:
                            got['status'] = 'pass'
                except (Reject, KeyError, TypeError, ValueError, OSError) as error:
                    got.update(status='fail', failure=str(error))
                    print('FAIL', row['case_id'], row['profile'], str(error)[:400], flush=True)
                save(evidence / 'receipt.json', got)
                receipts.append(got)
                if index and index % 500 == 0:
                    print(section, index, 'tuples', flush=True)
        with gzip.open(section_out / 'observations.jsonl.gz', 'wt') as sink:
            for got in receipts:
                sink.write(json.dumps(got, sort_keys=True) + '\n')
        try:
            report = aggregate(rows, receipts)
            def checker(row, got):
                if section == 'lifecycle':
                    original_binding = got['ordinary_receipt']
                    verify(original_binding['path'], original_binding)
                    lifecycle_compare(row, load(original_binding['path']), got, cfg, observer_cfg, contracts, sources_by_key[row['key']], tools)
                elif section == 'guards':
                    from guards import verify_complete
                    source_case = contracts.cases[row['case']['source_case_id']]
                    verify_complete(row, got, observer_cfg, contracts, contracts.source_bytes(source_case), tools, cfg)
                else:
                    core_compare(section, row, got, cfg, contracts, sources_by_key[row['key']], tools, predecessors)
            report['negative_controls'] = negative_controls(rows, receipts, checker)
            if args.pilot:
                report['status'] = 'PILOT_ONLY_' + report['status']
                report['unexecuted_contract_tuples'] = len(complete_rows) - len(rows)
        except (Reject, KeyError, TypeError, ValueError) as error:
            report = {'status': 'FAIL', 'reason': str(error), 'failures': [{'key': r['key'], 'reason': r.get('failure')} for r in receipts if r['status'] == 'fail']}
        if section == 'predecessors':
            report['executed_projection_kind_counts'] = dict(collections.Counter(r['projection_kind'] for r in receipts if r['executed'] and 'projection_kind' in r))
            report['frozen_case_kind_counts'] = contracts.tables[section]['kind_counts']
            report['projection_boundary'] = 'Complete public check/run projections only where declared. First-error, frontend-prefix and schema/source-only rows remain partial. No private legacy-scalar/raw/event/fuel claim.'
        if contracts.effective_identity:
            report.update(contracts.effective_identity)
        report['contract_identity'] = contracts.tables[section]['identity']
        report['historical_qualification_claim'] = False
        report['private_raw_event_fuel_claim'] = False
        save(section_out / 'comparison.json', report)
        results[section] = report
    rt.candidate_binding(args.binding)
    if args.observer_binding:
        rt.candidate_binding(args.observer_binding)
    if path_map_binding:
        verify(args.path_map, path_map_binding)
    Contracts(args.contracts, load(args.path_map) if args.path_map else None, args.amendment_root)
    if runtime_binding:
        need(rt.staged_runtime(args.runtime_receipt, args.libs) == runtime_binding, 'runtime libraries changed during run')
    for tool in tools.values():
        verify(tool['executable']['path'], tool['executable'])
        verify(tool['wrapper']['path'], tool['wrapper'])
        if 'windows_alias' in tool:
            verify(tool['windows_alias']['path'], tool['windows_alias'])
    overall = {'status': 'FAIL' if any(r['status'] == 'FAIL' for r in results.values()) else ('PILOT_ONLY' if args.pilot else ('INCOMPLETE' if any(r.get('unsupported_capability', 0) for r in results.values()) else 'LOCAL_COMPARISONS_COMPLETE')),
               'freeze_sha256': FREEZE_SHA, 'candidate_binding': binding(args.binding), 'adapter': adapter_identity(), 'sections': results,
               'all_contract_sections_collected': set(sections) == set(NAMES) and not args.pilot,
               'all_selected_local_obligations_executed': not args.pilot and not any(r.get('unsupported_capability', 0) for r in results.values()),
               'global_host_qualification': False, 'historical_receipts_retargeted': False, 'selected_runtime': runtime_binding,
               'build_receipt_boundary': 'Pinned approved compiler/observer source maps and actual tool-produced build receipts are required. Serialized receipts alone are not cryptographic proof of execution.'}
    if contracts.effective_identity:
        overall.update(contracts.effective_identity)
    save(out / 'comparison.json', overall)
    print(json.dumps({k: v for k, v in overall.items() if k != 'sections'}, indent=2))
    return 1 if overall['status'] == 'FAIL' else (2 if overall['status'] == 'INCOMPLETE' else 0)

if __name__ == '__main__':
    try:
        sys.exit(main())
    except (Reject, KeyError, TypeError, ValueError, OSError) as error:
        print('REJECT:', error, file=sys.stderr)
        sys.exit(1)
