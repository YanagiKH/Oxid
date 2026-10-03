#!/usr/bin/env python3
"""Occupied-output and deliberate post-link publication controls for v3."""
import sys
sys.dont_write_bytecode = True
import copy
import json
import os
from pathlib import Path
from contracts import need, sha, binding, receipt_identity, save, verify, load, FREEZE_SHA
import runtime as rt
import compare

def output_state(output, target=None):
    return {'kind': 'symlink' if output.is_symlink() else 'regular', 'sha256': sha(output.read_bytes()),
            'identity': rt.file_identity(output, follow=False), 'link_target': os.readlink(output) if output.is_symlink() else None,
            'target_sha256': sha(target.read_bytes()) if target else None,
            'target_identity': rt.file_identity(target) if target else None}

def substitution(text, root, output):
    return text.replace('${FIXTURE_ROOT}', str(root)).replace('${OUTPUT}', str(output))

def check_guard(row, got, cfg, contracts, sources, tools):
    from run import adapter_identity
    need(all(got[k] == v for k, v in receipt_identity(row).items()), 'guard tuple identity')
    need(got['candidate_binding_sha256'] == cfg['_sha256'] and got['freeze_sha256'] == FREEZE_SHA and got['adapter'] == adapter_identity(), 'guard source/binding identity')
    need(got['binary'] == cfg['binaries'][row['profile']] and got['binary_stat_before'] == got['binary_stat_after'] == cfg['_binary_stats'][row['profile']], 'guard binary')
    wanted = [{'path': k, 'bytes': len(v), 'sha256': sha(v)} for k, v in sorted(sources.items())]
    need(got['source_files'] == wanted and rt.source_map(got['directory_before']) == wanted, 'guard sources')
    need(got['directory_after'] == got['directory_before'], 'guard source/unrelated mutation')
    need(got['native_workspaces_after'] == [], 'guard scratch cleanup')
    p = got['process']
    expected_argv = [cfg['binaries'][row['profile']]['path']] + [substitution(a, got['fixture_root'], got['output']) for a in row['argv_template'][1:]]
    need(p['argv'] == expected_argv and p['cwd'] == got['fixture_directory'], 'guard argv/cwd')
    need(not p['timed_out'] and not p['stream_limit_exceeded'], 'guard incomplete process')
    for stream in ('stdout', 'stderr'):
        need(sha(p[stream].encode()) == p[stream + '_sha256'], 'guard stream hash')
    case = row['case']
    if row['group'] == 'occupied':
        need(got['output_before'] == got['output_after'] and got['output_before']['kind'] == case['output_kind'], 'occupied output/target identity mutation')
        need(p['status'] == case['frozen_envelope']['exit_status'], 'occupied status')
        for stream in ('stdout', 'stderr'):
            need(p[stream] == substitution(case['frozen_envelope'][stream], got['fixture_root'], got['output']), 'occupied literal ' + stream)
        rt.check_tools(got['tool_invocations'], tools, False, got['emitted_llvm'])
    else:
        need(got['output_initially_absent'] and p['status'] == case['expected_status'], 'race initial/status')
        summary, diagnostics = compare.envelope(p, 'compile')
        need(summary == case['summary'], 'race complete summary')
        exact_summary = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'compile-summary', 'success': False, 'errors': 1, 'output': None}
        need(p['stdout'].splitlines(keepends=True)[-1] == json.dumps(exact_summary, separators=(',', ':')) + '\n', 'race exact summary line')
        compare.diagnostic_vector(diagnostics, [case['diagnostic_projection']])
        compare.valid_origins(diagnostics, sources)
        rt.check_tools(got['tool_invocations'], tools, True, got['emitted_llvm'])
        calls = got['tool_invocations']
        injected = [call for call in calls if 'race' in call]
        need(len(injected) == 1, 'race injection count')
        final = injected[0]
        need(final['tool'] == 'clang' and final['status'] == 0, 'race successful link')
        race = final['race']
        need(final['completed_ns'] <= race['linked_verified_ns'] <= race['absent_verified_ns'] <= race['created_ns'] <= race['closed_ns'] <= race['readback_verified_ns'] <= final['wrapper_return_ready_ns'] <= p['completed_ns'], 'race causal order')
        need(race['created_exclusively'] and race['output'] == got['output'], 'race exclusive competitor')
        need(race['sha256'] == case['payload_sha256'] == got['output_after']['sha256'], 'race competitor bytes')
        need(got['output_after']['kind'] == 'regular' and race['file_identity'] == got['output_after']['identity'], 'race competitor inode preservation')
        need(race['linked_elf_sha256'] == got['linked_artifact']['sha256'] and got['linked_artifact_is_elf'] and got['linked_artifact_executed'] is False, 'race linked artifact')
        direct = sorted([c for c in calls if c['wrapper_ppid'] == p['pid']], key=lambda c: c['started_ns'])
        need(len(direct) == 7 and direct[-1] is final, 'race direct phase inventory')
        names = [c['tool'] for c in direct]
        need(names == ['clang', 'opt', 'ld.lld', 'opt', 'clang', 'clang', 'clang'], 'race tool phase order')
        need(all(c['argv'] == ['--version'] for c in direct[:3]), 'race version argv')
        need(direct[3]['argv'] == ['-passes=verify', '-disable-output', 'program.ll'], 'race verify argv')
        nested = [c for c in calls if c['wrapper_ppid'] != p['pid']]
        need(nested and all(c['tool'] == 'ld.lld' and c['wrapper_ppid'] == final['real_tool_pid'] and final['started_ns'] <= c['started_ns'] <= c['wrapper_return_ready_ns'] <= final['completed_ns'] for c in nested), 'real nested linker evidence')
        need(any('program.o' in c['argv'] and 'runtime.o' in c['argv'] for c in nested), 'real object linking')

def execute(row, config, evidence, fixture, fixture_root, output, target, sources, env, before_output=None):
    from run import adapter_identity
    evidence.mkdir(parents=True, exist_ok=True)
    binary = config['binaries'][row['profile']]
    log = evidence / 'tools.jsonl'
    trace = evidence / 'lifecycle.jsonl'
    per_env = {**env, 'UNIT4_TOOL_LOG': str(log), 'UNIT4_TOOL_EVIDENCE': str(evidence),
               'UNIT4_ADAPTER_ROOT': str(Path(__file__).resolve().parent)}
    if config['kind'] == 'unit4-public-v3-lifecycle-observer':
        per_env['UNIT4_LIFECYCLE_LOG'] = str(trace)
    if row['group'] == 'race':
        per_env['UNIT4_RACE_OUTPUT'] = str(output)
    got = {**receipt_identity(row), 'executed': True, 'status': 'fail', 'candidate_binding_sha256': config['_sha256'],
           'freeze_sha256': FREEZE_SHA, 'adapter': adapter_identity(), 'binary': binary, 'binary_stat_before': rt.file_identity(binary['path']),
           'source_files': [{'path': k, 'bytes': len(v), 'sha256': sha(v)} for k, v in sorted(sources.items())],
           'directory_before': rt.inventory(fixture), 'fixture_directory': str(fixture), 'fixture_root': str(fixture_root),
           'output': str(output), 'output_before': before_output, 'output_initially_absent': not os.path.lexists(output)}
    need(got['binary_stat_before'] == config['_binary_stats'][row['profile']], 'guard stale binary before invocation')
    argv = [binary['path']] + [substitution(a, fixture_root, output) for a in row['argv_template'][1:]]
    got['process'] = rt.process(argv, fixture, per_env)
    got['binary_stat_after'] = rt.file_identity(binary['path'])
    # Race output is explicitly permitted; retain complete directory change too.
    got['complete_directory_after'] = rt.inventory(fixture)
    got['directory_after'] = [r for r in got['complete_directory_after'] if not (row['group'] == 'race' and r['path'] == 'out.bin')]
    got['output_after'] = output_state(output, target) if os.path.lexists(output) else None
    got['tool_invocations'] = rt.read_events(log)
    got['trace'] = rt.read_events(trace)
    got['native_workspaces_after'] = [str(p.relative_to(evidence.parent)) for p in evidence.parent.rglob('.oxid-native-*')]
    got['emitted_llvm'] = binding(evidence / 'program.ll') if (evidence / 'program.ll').exists() else None
    if row['group'] == 'race':
        linked = evidence / 'linked-before-publication.elf'
        got['linked_artifact'] = binding(linked) if linked.exists() else None
        got['linked_artifact_is_elf'] = linked.exists() and linked.read_bytes()[:6] == b'\x7fELF\x02\x01'
        got['linked_artifact_executed'] = False
        need([r['path'] for r in got['complete_directory_after'] if r not in got['directory_before']] == row['case']['expected_directory_delta'], 'race complete directory delta')
    for stream in ('stdout', 'stderr'):
        (evidence / stream).write_text(got['process'][stream])
    return got

def run_guards(rows, output_root, cfg, observer_cfg, contracts, env, tools):
    from run import skipped
    receipts = []
    for index, row in enumerate(rows):
        if row['scope'] != 'execute':
            receipts.append(skipped(row))
            continue
        case = row['case']
        source_case = contracts.cases[case['source_case_id']]
        sources = contracts.source_bytes(source_case)
        evidence = output_root / ('%03d' % index)
        fixture_root = evidence / 'fixtures'
        fixture = fixture_root / case['source_case_id']
        rt.materialize(sources, fixture)
        target = evidence / 'preserved-target' if row['group'] == 'occupied' else None
        output = evidence / 'occupied-output' if row['group'] == 'occupied' else fixture / 'out.bin'
        if target:
            target.write_bytes(b'Oxid Unit3 no-clobber sentinel\x00\xff\n')
            if case['output_kind'] == 'symlink':
                output.symlink_to(target.name)
            else:
                output.write_bytes(b'Oxid Unit3 occupied output sentinel\x00\xfe\n')
        before = output_state(output, target) if target else None
        got = {**receipt_identity(row), 'executed': True, 'status': 'fail'}
        try:
            ordinary = execute(row, cfg, evidence / 'ordinary', fixture, fixture_root, output, target, sources, env, before)
            check_guard(row, ordinary, cfg, contracts, sources, tools)
            ordinary['status'] = 'pass'
            save(evidence / 'ordinary' / 'receipt.json', ordinary)
            if row['group'] == 'race':
                # Remove only this run's proven generated competitor, after saving
                # its bytes and identity. Both runs start with the frozen absent state.
                (evidence / 'ordinary' / 'competitor.bytes').write_bytes(output.read_bytes())
                output.unlink()
            got = execute(row, observer_cfg, evidence / 'observer', fixture, fixture_root, output, target, sources, env, before)
            check_guard(row, got, observer_cfg, contracts, sources, tools)
            for field in ('status', 'stdout', 'stderr'):
                need(got['process'][field] == ordinary['process'][field], 'guard observer passivity ' + field)
            need(got['directory_after'] == ordinary['directory_after'], 'guard observer source passivity')
            expected = next(c for c in contracts.tables['lifecycle']['public_cases'] if c['id'] == case['source_case_id'])
            need(expected['expected_lifecycle'] == case['expected_lifecycle'], 'guard declared lifecycle binding')
            expected = copy.deepcopy(expected)
            if row['group'] == 'occupied':
                expected['file_counts'] = {str(fixture / name): value for name, value in expected['file_counts'].items()}
            got['logical_lifecycle'] = compare.lifecycle(got['trace'], expected, contracts.tables['lifecycle']['counter_order'], contracts.tables['lifecycle']['file_counter_order'])
            got['ordinary_receipt'] = binding(evidence / 'ordinary' / 'receipt.json')
            got['status'] = 'pass'
        except (Exception,) as error:
            got.update(status='fail', failure=str(error))
        save(evidence / 'receipt.json', got)
        receipts.append(got)
    return receipts

def verify_complete(row, got, observer_cfg, contracts, sources, tools, ordinary_cfg):
    if not got['executed']:
        return
    check_guard(row, got, observer_cfg, contracts, sources, tools)
    bound = got['ordinary_receipt']
    verify(bound['path'], bound)
    ordinary = load(bound['path'])
    check_guard(row, ordinary, ordinary_cfg, contracts, sources, tools)
    need(all(got['process'][field] == ordinary['process'][field] for field in ('status', 'stdout', 'stderr')), 'saved guard passivity envelope')
    need(got['directory_after'] == ordinary['directory_after'], 'saved guard passivity directory')
    expected = copy.deepcopy(next(c for c in contracts.tables['lifecycle']['public_cases'] if c['id'] == row['case']['source_case_id']))
    if row['group'] == 'occupied':
        expected['file_counts'] = {str(Path(got['fixture_directory']) / name): value for name, value in expected['file_counts'].items()}
    compare.lifecycle(got['trace'], expected, contracts.tables['lifecycle']['counter_order'], contracts.tables['lifecycle']['file_counter_order'])
    need('native_execution' not in got, 'guard orphan successful-native receipt')
