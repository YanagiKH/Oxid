"""Independent exact-run five-domain join over retained raw observations.

The expected identity always comes from the join job's original JoinContext.
Transport adapters are read-only and never register a consumer capability.
"""
import argparse
import ast
import base64
import builtins
from collections import Counter
import copy
import hashlib
import json
import os
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
import stat
import sys
import time
import types

sys.dont_write_bytecode = True
COMMON_SHA256 = '017a0b034ac120b246d2f9b88600cc3515534fe97c3e033b73a9f363e7b76e3d'
COMMON_KEY = '_oxid_package_dependency_unit4_ci_common_v1'
_COMMON = None
_EVIDENCE = None
JOIN_SCHEMA = 'oxid-package-unit4-ci-join-v1'
DOMAINS = ('linux-public', 'linux-parser', 'macos-x86_64-public', 'macos-arm64-public', 'windows-x86_64-public')
PUBLIC_HOSTS = ('Linux x86_64', 'macOS x86_64', 'macOS arm64', 'Windows x86_64')
PROFILES = ('debug', 'release')
SECTIONS = ('public', 'original', 'predecessors', 'lifecycle', 'guards')
RUN_FIELDS = ('repository', 'run_id', 'run_attempt', 'workflow', 'event_name')


class Rejected(ValueError):
    pass


def need(value, message):
    if not value: raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def same(actual, expected, message):
    need(actual == expected, message)


def _common():
    global _COMMON
    path = Path(__file__).resolve().with_name('common.py')
    need(type(COMMON_SHA256) is str and re.fullmatch('[0-9a-f]{64}', COMMON_SHA256), 'unsealed join common body')
    need(path.resolve(strict=True) == path and not any(p.is_symlink() for p in (path, *path.parents)) and
         stat.S_ISREG(path.stat().st_mode) and path.stat().st_size < 2 * 1024 ** 2, 'canonical bounded common body')
    raw = path.read_bytes()
    same(sha(raw), COMMON_SHA256, 'join common body changed')
    owner = globals().get('_COMMON_OWNER') or _COMMON
    if owner is None:
        need(COMMON_KEY not in sys.modules, 'preoccupied common module slot')
        owner = types.ModuleType(COMMON_KEY)
        owner.__file__, owner.__source_sha256__ = str(path), COMMON_SHA256
        sys.modules[COMMON_KEY] = owner
        try: exec(compile(raw, str(path), 'exec'), owner.__dict__)
        except BaseException:
            if sys.modules.get(COMMON_KEY) is owner: del sys.modules[COMMON_KEY]
            raise
    need(type(owner) is types.ModuleType and sys.modules.get(COMMON_KEY) is owner and
         owner.__file__ == str(path) and owner.__source_sha256__ == COMMON_SHA256, 'foreign common owner')
    if globals().get('_COMMON_OWNER') is not None:
        owner.owns_sibling(sys.modules.get(__name__))
    owner.owner()
    need(_COMMON is None or _COMMON is owner, 'common owner replacement')
    _COMMON = owner
    return owner


def _evidence():
    global _EVIDENCE
    if _EVIDENCE is None: _EVIDENCE = _common().load_sibling('evidence')
    return _EVIDENCE


def _child(root, *parts):
    return _evidence().child(root, *parts)


def _identity(row):
    return {key: row[key] for key in ('path', 'bytes', 'sha256')}


def _body_identity(a, b, message):
    same({k: a[k] for k in ('bytes', 'sha256')}, {k: b[k] for k in ('bytes', 'sha256')}, message)


def stage_catalogue(domain):
    need(domain in DOMAINS, 'unknown stage domain')
    stages = ['capture-current-source', 'prepare-source', 'capture-machine']
    if domain == 'linux-parser':
        stages += ['admit-parser-source', 'prepare-parser-layout']
        stages += ['build-parser-' + role + '-' + p for role in ('observed', 'control') for p in PROFILES]
        stages += ['capture-parser-runtime-inputs', 'admit-complete-parser-runtime',
                   'complete-seven-parser-stages', 'verify-complete-parser-runtime', 'terminal-parser-readmission']
    else:
        stages += ['build-public-ordinary', 'build-public-lifecycle']
        if domain == 'linux-public': stages += ['stage-installed-dpkg-runtime', 'verify-fresh-staging']
        stages += ['capture-public-runtime-tools', 'admit-public-runtime', 'collect-complete-public', 'verify-public-collection']
        if domain == 'linux-public':
            stages += ['prepare-existing-nonroot-transport', 'execute-twelve-hosted-replacements',
                       'join-twelve-hosted-replacements', 'verify-public-hosted-join']
        stages += ['terminal-public-readmission']
        if domain == 'linux-public': stages += ['terminal-staging-readmission']
    return stages + ['terminal-source-readmission', 'terminal-machine-readmission', 'terminal-ci-readmission']


def _provenance(q, expected, host):
    need(set(host) == {'schema', 'ci_context', 'policy', 'invocation', 'source_context', 'preparation', 'machine',
                      'public', 'parser', 'staging', 'status', 'ci_runtime_proof',
                      'terminal_readmission', 'qualification_boundary', 'journal'}, 'closed host evidence schema')
    need(host['schema'] == 'oxid-package-unit4-ci-host-v1' and host['status'] == 'TERMINAL_LIVE_SEALED' and
         host['ci_runtime_proof'] is True, 'local/old/unsealed host result cannot qualify')
    ci = host['ci_context']
    domain = ci['domain']
    need(domain in expected['required_domains'] == list(DOMAINS), 'exact expected five-domain scope')
    same(host['policy'], q.host_policy(domain), 'fixed domain policy')
    for field in ('checkout_head', 'checkout_tree', 'event_sha', 'current_source_checkpoint', 'activation'):
        same(ci[field], expected[field], 'domain differs from trusted join ' + field)
    for field in ('workflow', 'package_identity', 'inputs_identity', 'source_manifest_identity'):
        _body_identity(ci[field], expected[field], 'domain differs from trusted join ' + field)
    same({k: ci['ci'][k] for k in RUN_FIELDS}, {k: expected['ci'][k] for k in RUN_FIELDS}, 'stale/foreign CI run or attempt')
    same(ci['ci']['job'], q.JOBS[domain], 'wrong actual producing job')
    same(ci['scope'], host['policy']['scope'], 'domain source scope')
    same(ci['host']['name'], host['policy']['host'], 'domain measured host')
    need(ci['host']['pointer_width'] == 64, '64-bit actual host required')
    if host['policy']['root_required']: same(ci['host']['effective_uid'], 0, 'genuine Linux root primary required')
    for field, flag in (('public', 'public_required'), ('parser', 'parser_required'), ('staging', 'staging_required')):
        need((host[field] is not None) is host['policy'][flag], 'missing/foreign domain result: ' + field)
    invocation = host['invocation']
    need(set(invocation) == {'nonce', 'started_ns', 'run_root'} and re.fullmatch('[0-9a-f]{32}', invocation['nonce']) and
         type(invocation['started_ns']) is int and ci['capture_started_ns'] <= ci['capture_finished_ns'] <= invocation['started_ns'],
         'fresh invocation identity/window')
    same(invocation['run_root'], ci['producer_root'], 'producer root differs from admitted CI root')
    same(ci['producer_root'], _child(ci['outer_root'], 'producer'), 'fixed nested producer route')
    need(re.fullmatch('[0-9a-f]{32}', ci['ci_nonce']), 'fresh CI nonce')
    return domain


def _source(q, expected, t, api):
    host, source = t.host, t.host['source_context']
    ci, inv = host['ci_context'], host['invocation']
    need(source['schema'] == 'oxid-package-unit4-admitted-source-context-v1' and source['status'] == 'NotReady' and
         source['execution_qualified'] is False, 'source context qualification relabelled')
    same(source['scope'], host['policy']['scope'], 'source context scope')
    same(source['invocation'], inv, 'source context invocation')
    for field, expected_key in (('head', 'checkout_head'), ('tree', 'checkout_tree'), ('event_sha', 'event_sha')):
        same(source['checkout'][field], expected[expected_key], 'source checkout/event identity')
    same(source['checkout']['path'], ci['repository_path'], 'actual producer checkout route')
    _body_identity(source['current_source_manifest'], expected['source_manifest_identity'], 'current source authority')
    same(source['source_checkpoint'], expected['current_source_checkpoint'], 'separate immutable source checkpoint')
    repo = Path(expected['repository_path'])
    source_root = repo / 'tests/qualification/package_dependency_unit4_source_v1'
    if host['policy']['public_required']:
        authority = q.decode(q.read(source_root / 'public-source-successor-authority.json'))
        maps = {'ordinary': authority['maps']['new_ordinary376'], 'lifecycle': authority['maps']['new_lifecycle377']}
        roles = {'ordinary': 'public/ordinary', 'lifecycle': 'public/lifecycle'}
    else:
        authority = q.decode(q.read(source_root / 'parser-source-successor-authority.json'))
        maps = {'parser_observed': authority['maps']['current_derived_files'],
                'parser_control': authority['maps']['current_control_derived_files']}
        roles = {'parser_observed': 'parser/observed', 'parser_control': 'parser/control'}
    same(set(source['views']), set(maps), 'source role inventory')
    preparation = source['preparation']
    same(preparation['root'], _child(inv['run_root'], 'source-preparation'), 'fresh source preparation root')
    report = t.json(preparation['report'])
    need(report['status'] == 'NotReady' and report['execution_qualified'] is False, 'preparation remains nonqualifying')
    same(report['scope'], source['scope'], 'preparation exact scope')
    for role, rows in maps.items():
        view = source['views'][role]
        same(view['files'], rows, 'exact authenticated role source map')
        same(view['map_sha256'], sha(canonical(rows)), 'source role map hash')
        same(view['path'], _child(preparation['root'], roles[role]), 'prepared role route')
        need(len(view['metadata']) == 1 and view['metadata'][0]['path'] == 'package-preparation-overlay.json',
             'exact preparation metadata')
        for row in view['metadata']:
            bound = dict(row, path=_child(view['path'], row['path']))
            t.raw(bound)
        for row in rows:
            bound = dict(row, path=_child(view['path'], row['path']))
            same(t.named(bound['path']), bound, 'prepared source transport inventory')
            if row['path'] == 'candidate-source-manifest.json': t.raw(bound)
    preparation_record = host['preparation']
    same(set(preparation_record), {'config', 'plan', 'provenance'}, 'complete original preparation metadata')
    same(t.json(_child(inv['run_root'], 'preparation-config.json')), preparation_record['config'], 'actual preparation config body')
    same(sha(api['context'].serial(preparation_record['plan'])), preparation['plan_sha256'], 'complete preparation plan identity')
    same(sha(canonical(preparation_record['provenance'])), preparation['provenance_sha256'], 'original live preparation provenance identity')
    same(preparation_record['provenance']['invocation'], inv, 'actual preparation invocation')
    same(preparation_record['provenance']['repositories_before'], preparation_record['provenance']['repositories_after'], 'preparation repository preservation')
    status = t.json(_child(inv['run_root'], 'preparation-status.json'))
    need(status['success'] is True and status['inputs_preserved'] is True and status['repository_poststate_checked'] is True and status['failure'] is None and status['preservation_error'] is None, 'preparation terminal preservation')
    for row in report['files']:
        bound = dict(row, path=_child(preparation['root'], row['path']))
        same(t.named(bound['path']), bound, 'complete predecessor/preparation inventory')
        if bound['path'] in t.mapping: t.raw(bound)
    required_proof = 'public_admission' if source['scope'] == 'public' else 'predecessor_session'
    t.raw(preparation[required_proof])
    # All preparation files, including predecessor transitions, are transported
    # unless that exact path is one of the authenticated role source bodies.
    full, caches, _ = _evidence()._classification(host, t)
    actual_omitted = {r['identity']['path']: {'identity': r['identity'], 'role': r['role']}
                      for r in t.seal['inventory']['files'] if r['storage'] == 'full-only'}
    same(actual_omitted, full, 'exact role/path omission inventory')
    actual_caches = {r['path']: r['role'] for r in t.seal['inventory']['excluded_regenerable_caches']}
    same(actual_caches, caches, 'exact regenerable cache root inventory')
    for row in t.seal['inventory']['excluded_regenerable_caches']:
        root = _evidence().original_path(row['path'])
        same(row['retained_executables'], sorted(p for p in full if root in _evidence().original_path(p).parents),
             'cache exclusion swallowed emitted executable')
    machine = host['machine']
    same(machine['snapshot']['invocation'], inv, 'machine invocation')
    same(machine['snapshot']['checkout'], source['checkout'], 'machine source checkout')
    same(machine['snapshot']['host']['name'], host['policy']['host'], 'machine actual host')
    same(set(machine['snapshot']['source_roles']), set(maps), 'machine source scope')
    api['build'].inspect_rust_identity(machine['rustc_verbose'], host['policy']['host'])


def _journal(t):
    value = t.json(t.host['journal'])
    need(set(value) == {'schema', 'ci', 'invocation', 'stages', 'started_ns', 'completed_ns'} and
         value['schema'] == 'oxid-package-unit4-ci-stages-v1', 'closed gate stage journal')
    same(value['ci'], t.host['ci_context'], 'stage journal CI association')
    same(value['invocation'], t.host['invocation'], 'stage journal invocation association')
    same([r['name'] for r in value['stages']], stage_catalogue(t.host['ci_context']['domain']),
         'missing/reordered/mixed old CI stage catalogue')
    previous = value['started_ns']
    for row in value['stages']:
        need(set(row) == {'name', 'started_ns', 'completed', 'completed_ns'} and row['completed'] is True and
             type(row['started_ns']) is int and type(row['completed_ns']) is int and
             previous <= row['started_ns'] <= row['completed_ns'] <= value['completed_ns'],
             'failed/partial/reordered producer stage')
        previous = row['completed_ns']
    need(value['completed_ns'] <= t.seal['started_ns'], 'journal written after seal')


def _process(row, invocation, t=None):
    need(type(row['pid']) is int and row['pid'] > 0 and type(row['status']) is int and
         type(row['started_ns']) is int and type(row['completed_ns']) is int and
         invocation['started_ns'] <= row['started_ns'] <= row['completed_ns'], 'actual process window')
    need(row['timed_out'] is False and row['stream_limit_exceeded'] is False and
         row['cleanup_complete'] is True and row['cleanup_error'] is None and row['failure'] is None,
         'incomplete or poisoned owned process')
    same(row['invocation_nonce'], invocation['nonce'], 'process invocation nonce')
    need(type(row['timeout_seconds']) in (int, float) and row['timeout_seconds'] > 0 and
         row['completed_ns'] - row['started_ns'] <= (row['timeout_seconds'] + 30) * 10**9,
         'process timeout bound')
    if t is not None:
        for name in ('stdout', 'stderr'):
            raw = t.raw(row[name])
            need(len(raw) <= row['stream_limit_bytes'], 'process stream bound')
    return row['pid'], row['started_ns']


def _operations(t):
    t.used_operations = set()
    prefix = _child(t.host['invocation']['run_root'], 'operations')
    root = _evidence().original_path(prefix)
    records = {}
    by_number = {}
    for path in t.index:
        p = _evidence().original_path(path)
        if p.parent.parent == root and p.name == 'result.json':
            number = p.parent.name
            need(re.fullmatch('[0-9]{4,}', number), 'operation directory index')
            row = t.json(path)
            need(row['schema'] == 'oxid-owned-probe-v1', 'original owned process schema')
            key = _process(row, t.host['invocation'], t)
            need(row['completed_ns'] <= t.seal['started_ns'], 'owned operation extends beyond terminal seal')
            need(key not in records and number not in by_number, 'reused owned process identity')
            command = t.json(str(p.parent / 'command.json'))
            same(command, {k: row[k] for k in ('argv', 'cwd', 'environment', 'timeout_seconds', 'stream_limit_bytes', 'operation_kind')},
                 'original process command body')
            records[key] = row
            by_number[number] = row
    need(records and sorted(by_number) == ['%04d' % x for x in range(1, len(records) + 1)],
         'complete contiguous owned process evidence')
    same(set(_relative_files(t, prefix)), {number + '/' + name for number in by_number for name in ('command.json', 'result.json', 'stdout', 'stderr')},
         'exact complete owned operation raw file catalogue')
    same(_relative_directories(t, prefix), set(by_number), 'exact owned operation directory catalogue')
    for number, row in by_number.items():
        for name in ('stdout', 'stderr'): same(row[name]['path'], _child(prefix, number, name), 'fixed original owned stream route')
    previous = t.host['invocation']['started_ns']
    for row in (by_number[k] for k in sorted(by_number)):
        need(previous <= row['started_ns'], 'owned processes overlap/reorder')
        previous = row['completed_ns']
    return records


def _same_owned(row, operations, t, *, text_streams=False):
    key = row['pid'], row['started_ns']
    need(key in operations, 'unbound process observation')
    original = copy.deepcopy(operations[key])
    if text_streams:
        for stream in ('stdout', 'stderr'):
            raw = t.raw(original[stream])
            original[stream], original[stream + '_sha256'] = raw.decode('utf-8'), sha(raw)
    same(row, original, 'raw process differs from exact owned operation')
    t.used_operations.add(key)
    return key


def _public_builds(t, api, operations):
    public, source, machine = t.host['public'], t.host['source_context'], t.host['machine']
    configs = {}
    same(set(public['builds']), {'ordinary', 'lifecycle'}, 'both public build roles')
    for role in ('ordinary', 'lifecycle'):
        record = public['builds'][role]
        build = record['document']
        same(t.json(record['binding']), build, 'actual public build binding body')
        need(build['schema'] == 'package-dependency-unit4-public-built-binding-v1' and
             build['kind'] == api['binding'].KINDS[role] and build['role'] == role and
             build['status'] == 'built-runtime-NotReady' and build['execution_qualified'] is False,
             'current public build schema/role')
        same(build['invocation'], t.host['invocation'], 'public build invocation')
        same(build['host'], t.host['policy']['host'], 'public build actual host')
        same(build['machine_sha256'], sha(canonical(machine)), 'public build captured machine')
        bound = build['source_binding']
        same(bound['context_sha256'], sha(canonical(source)), 'public source context digest')
        same(bound['view'], source['views'][role], 'public source role view')
        same(bound['checkout'], source['checkout'], 'public current checkout')
        same(bound['source_manifest'], source['current_source_manifest'], 'public new source identity')
        same(bound['retained_semantic_basis']['source_manifest'], source['predecessor_source_manifest'], 'retained semantic identity')
        same(t.json(build['source_carrier']), bound, 'public source carrier body')
        same(t.json(build['machine_carrier']), machine, 'public machine carrier body')
        plan = t.json(build['plan'])
        same(plan['source_binding'], bound, 'public exact source build plan')
        same(plan['target'], _child(t.host['invocation']['run_root'], 'targets', 'public-' + role), 'public fixed target')
        same(plan['output'], _child(t.host['invocation']['run_root'], 'public-build', role), 'public fixed build output')
        _same_owned(build['metadata_process'], operations, t)
        same(build['metadata_process']['argv'], api['build'].metadata_recipe(), 'actual Cargo metadata command')
        same(build['metadata_process']['cwd'], bound['view']['path'], 'actual Cargo metadata source root')
        same(build['metadata_process']['environment'], dict(machine['snapshot']['environment'], CARGO_TARGET_DIR=plan['target']),
             'actual exact Cargo metadata environment')
        need(build['metadata_process']['operation_kind'] == 'probe' and build['metadata_process']['timeout_seconds'] == 30 and
             build['metadata_process']['stream_limit_bytes'] == api['build'].MAX_STREAM_BYTES, 'actual Cargo metadata fixed limits')
        metadata_checker = _transport_function(api['build'], 'inspect_cargo_metadata',
            {'Path': _remote_path, 'need': api['build'].need, 'binding': api['binding']})
        metadata_checker(t.json(build['metadata_process']['stdout']), _remote_path(bound['view']['path']),
                         _remote_path(plan['target']), bound['view']['files'])
        need(build['metadata_process']['status'] == 0, 'Cargo metadata failed')
        same(set(build['profiles']), set(PROFILES), 'complete paired public profiles')
        previous = build['metadata_process']['completed_ns']
        for profile in PROFILES:
            row = build['profiles'][profile]
            process = row['process']
            _same_owned(process, operations, t)
            same(process['argv'], api['build'].recipe(profile), 'public build command')
            need(process['status'] == 0 and process['operation_kind'] == 'public-build' and
                 previous <= process['started_ns'] <= process['completed_ns'] <= build['completed_ns'],
                 'public profile build failure/order')
            previous = process['completed_ns']
            same(row['source_before'], row['source_after'], 'public build source preservation')
            same(row['source_before'], bound['context_sha256'], 'public build source association')
            same(row['machine_before'], row['machine_after'], 'public build machine preservation')
            same(row['machine_before'], build['machine_sha256'], 'public build machine association')
            env = process['environment']
            same(env, dict(machine['snapshot']['environment'], CARGO_TARGET_DIR=plan['target']), 'actual exact public build environment')
            same(process['cwd'], bound['view']['path'], 'actual public build source cwd')
            need(process['timeout_seconds'] == api['build'].BUILD_TIMEOUT_SECONDS and
                 process['stream_limit_bytes'] == api['build'].MAX_STREAM_BYTES, 'actual public build fixed limits')
            need(env['CARGO_INCREMENTAL'] == '0' and env['CARGO_BUILD_JOBS'] == '2' and env['CARGO_TARGET_DIR'] == plan['target'],
                 'public resource recipe')
            for key, name in (('binary', 'copied_binary'), ('emitted_binary', 'emitted_binary')):
                binary = row[key]
                same(binary['path'], plan['profiles'][profile][name], 'public exact executable path')
                same(t.named(binary['path']), _identity(binary), 'public executable inventory')
                need(binary['stat']['st_size'] == binary['bytes'] and process['started_ns'] <=
                     binary['stat']['st_mtime_ns'] <= (process['completed_ns'] if key == 'emitted_binary' else build['completed_ns']) + 2 * 10**9,
                     'public emitted artifact creation window')
            _body_identity(row['binary'], row['emitted_binary'], 'public copied/emitted binary identity')
        need(build['profiles']['debug']['binary']['sha256'] != build['profiles']['release']['binary']['sha256'],
             'public debug/release binary reuse')
        cfg = {'kind': build['kind'], 'role': role, '_sha256': record['binding']['sha256'],
               'binaries': {p: _identity(build['profiles'][p]['binary']) for p in PROFILES},
               '_binary_stats': {p: build['profiles'][p]['binary']['stat'] for p in PROFILES}, 'build_binding': build,
               '_execution_context': {**public['execution_context'], 'role': role},
               '_consumer': {'schema': 'package-unit4-public-consumer-v2', 'bytes': build['consumer']['bytes'], 'sha256': build['consumer']['sha256']}}
        same(public['configs'][role], cfg, 'public runtime configuration derived from real builds')
        configs[role] = cfg
    return configs


def _raw_public_row(t, got, operations, *, hosted_processes=None, expected_case=None, row=None):
    if not got['executed']: return None
    process = got['process']
    if hosted_processes is None:
        _public_process_environment(t, got, expected_case, row)
        key = _same_owned(process, operations, t, text_streams=True)
        need(process['operation_kind'] == 'public-runtime' and process['timeout_seconds'] == 120 and
             process['stream_limit_bytes'] == 8 * 1024 ** 2, 'actual public compiler operation recipe')
    else:
        key = process['pid'], process['started_ns']
        same(hosted_processes.get(key), process, 'hosted raw process not in exact child roster')
    evidence = (_evidence().original_path(process['environment']['UNIT4_TOOL_EVIDENCE']) if 'fixture_root' in got
                else _evidence().original_path(got['fixture_directory']).parent)
    same(t.json(str(evidence / 'process.json')), process, 'saved raw process document')
    for stream in ('stdout', 'stderr'):
        same(t.raw(str(evidence / stream)), process[stream].encode(), 'raw public saved stream')
    for filename, field in (('tools.jsonl', 'tool_invocations'), ('lifecycle.jsonl', 'trace')):
        path = str(evidence / filename)
        events = t.rows(path) if path in t.mapping else []
        same(events, got[field], 'raw public event stream')
    if got.get('emitted_llvm'): t.raw(got['emitted_llvm'])
    if got.get('native_execution'):
        for name in ('original_elf', 'copied_elf'):
            same(t.named(got['native_execution'][name]['path']), got['native_execution'][name], 'native exact full-only artifact')
        native = got['native_execution']['process']
        _same_owned(native, operations, t, text_streams=True)
        need(native['operation_kind'] == 'public-native' and native['timeout_seconds'] == 120 and
             native['stream_limit_bytes'] == 8 * 1024 ** 2, 'actual public native operation recipe')
    return key


def _public_contracts(t, api, expected):
    public = t.host['public']
    helpers = api['public_runtime'].retained_helpers(Path(expected['repository_path']))
    paths = public['input_paths']
    raw_map = t.json(paths['path_map'])
    mapping = {logical: str(t.local_path(physical)) for logical, physical in raw_map.items()}
    root = t.local_path(paths['contract_root'])
    repo = Path(expected['repository_path'])
    amendment = repo / 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3'
    # The original Contracts constructor validates all frozen bodies and semantic
    # amendments. Only its explicitly supported path map is relocated.
    contracts = helpers['contracts'].Contracts(root, mapping, t.local_path(paths['amendment_root']))
    source = repo / 'tests/fixtures/typed_project_source_binding'
    predecessors = helpers['predecessors'].Predecessors(contracts, source_manifest=source / 'current-source.json', amendment_root=source)
    same(contracts.effective_identity, public['contracts']['effective_identity'], 'effective contract authority')
    same(contracts.tables, public['contracts']['tables'], 'exact transported public semantic tables')
    return helpers, contracts, predecessors


def _hosted(t, api, contracts, configs, operations):
    public, data = t.host['public'], t.host['public']['hosted']
    if data is None: return {}, {}, None, None
    hosted = api['hosted']
    _runtime_tool_operations(t, data['runtime_tools'], operations, api['machine'])
    report, ledger, worker = data['report'], data['ledger'], data['worker_process']
    same(t.json(data['report_binding']), report, 'actual hosted shard report')
    same(t.json(data['ledger_binding']), ledger, 'actual readonly ledger')
    need(ledger['schema'] == 'package-dependency-unit4-public-hosted-transport-v1' and ledger['status'] == 'NotReady' and
         ledger['execution_qualified'] is False, 'readonly carrier never qualifies')
    same(ledger['invocation'], t.host['invocation'], 'readonly invocation')
    entries = ledger['entries']
    logical = [row['logical_path'] for row in entries]
    need(logical == sorted(set(logical)) and 0 < len(entries) <= hosted.MAX_LEDGER_MEMBERS and
         sum(row['bytes'] for row in entries) <= hosted.MAX_LEDGER_BYTES, 'complete bounded readonly ledger')
    mapping = {}
    for row in entries:
        need(row['mode'] in (0o444, 0o555) and row['roles'] == sorted(set(row['roles'])) and row['roles'], 'readonly permissions/roles')
        same(row['physical_path'], str(hosted.namespace_path(ledger['root'], row['logical_path'])), 'readonly exact physical route')
        same(t.named(row['physical_path']), {'path': row['physical_path'], 'bytes': row['bytes'], 'sha256': row['sha256']},
             'readonly actual file identity')
        mapping[row['logical_path']] = row
    _same_owned(worker, operations, t, text_streams=True)
    argv = hosted.worker_argv(ledger['controller']['setpriv']['path'], ledger['controller']['python']['path'],
        hosted.namespace_path(ledger['root'], ledger['controller']['helper']), data['ledger_binding']['path'],
        data['ledger_binding']['sha256'], ledger['output'], ledger['identity']['effective_uid'], ledger['identity']['effective_gid'])
    hosted.inspect_worker_process(worker, argv, ledger['output'], hosted.outer_environment(Path(ledger['output'])), t.host['invocation'])
    need(report['status'] == 'PARTIAL_CAPABILITY_SHARD' and report['execution_qualified'] is False and
         report['global_host_qualification'] is False and report['complete_qualification'] is False,
         'hosted child cannot claim global qualification')
    identity = report['identity']
    need(identity['effective_uid'] != 0 and identity['effective_gid'] == ledger['identity']['effective_gid'] and
         identity['supplementary_groups'] == [], 'actual existing nonroot child identity/groups')
    for key, value in ledger['identity'].items(): same(identity[key], value, 'hosted pre-existing identity')
    records = {(row['pid'], row['started_ns']): row for row in report['processes']}
    need(len(report['processes']) == len(records) == 21, 'exact hosted process roster')
    for row in records.values(): hosted.inspect_nested_process(row, worker, identity)
    raw = report['raw_result']
    need(raw['schema'] == 'package-unit4-public-capability-shard-v1' and raw['status'] == 'PARTIAL_CAPABILITY_SHARD' and
         raw['complete_qualification'] is False, 'actual raw capability shard')
    rows = api['public_runtime'].capability_rows(contracts)
    same(report['tuple_keys'], {s: [r['key'] for r in values] for s, values in rows.items()}, 'exact hosted tuple set')
    same(set(raw['sections']), {'public', 'lifecycle'}, 'hosted section set')
    same(set(raw['ordinary_counterparts']), {r['key'] for r in rows['lifecycle']}, 'hosted ordinary counterparts')
    shard_configs = report['cfgs']
    for role, original in configs.items():
        cfg = shard_configs[role]
        same(cfg['build_binding'], original['build_binding'], 'hosted copied build source')
        same(cfg['transport']['logical_binding_sha256'], original['_sha256'], 'hosted logical build')
        same(cfg['transport']['logical_binaries'], original['binaries'], 'hosted copied binaries')
        same(cfg['transport']['readonly_ledger'], data['ledger_binding'], 'hosted ledger binding')
        for profile, binary in cfg['binaries'].items():
            row = mapping[original['binaries'][profile]['path']]
            same(binary, {'path': row['physical_path'], 'bytes': row['bytes'], 'sha256': row['sha256']}, 'exact readonly copied executable')
    need(len(raw['sections']['public']) == len(raw['sections']['lifecycle']) == 6, 'twelve actual replacements')
    for name, tool in report['tools'].items():
        need(name in ('clang', 'opt', 'ld.lld') and tool['control']['status'] == 121,
             'hosted actual trap control')
        same(t.rows(tool['control_log']), [{'unexpected_native_tool': True}], 'hosted trap raw log')
    _hosted_raw_closure(t, api, contracts, data, configs, records)
    return raw['sections'], records, shard_configs, report['tools']


def _public_replay(t, api, expected, operations):
    runtime = api['public_runtime']
    public = t.host['public']
    configs = _public_builds(t, api, operations)
    _runtime_tool_operations(t, public['runtime_tools'], operations, api['machine'])
    helpers, contracts, predecessors = _public_contracts(t, api, expected)
    _generator(t, helpers, contracts, operations)
    same(t.json(public['primary_binding']), public['primary'], 'primary original collection body')
    same(t.json(public['terminal_binding']), public['document'], 'original terminal collection body')
    same(public['document']['execution_context'], public['execution_context'], 'public terminal runtime context')
    same(public['execution_context']['invocation'], t.host['invocation'], 'public runtime invocation')
    same(public['execution_context']['checkout'], t.host['source_context']['checkout'], 'public runtime checkout')
    tools_doc = public['runtime_tools']
    tools = tools_doc['tools']
    same(set(tools), {'clang', 'opt', 'ld.lld'}, 'complete public tools')
    if t.host['policy']['hosted_required']:
        need(tools_doc['mode'] == 'real-native' and public['primary']['status'] == 'INCOMPLETE_LOCAL_CAPABILITY',
             'actual Linux root branch and staged native tools required')
    else:
        need(tools_doc['mode'] == 'no-native-traps' and public['primary']['status'] == 'COMPLETE_LOCAL_OBSERVATIONS',
             'portable original trap path')
        for tool in tools.values():
            need(tool['control']['status'] == 121 and tool['control']['stdout'] == '' and
                 tool['control']['stderr'] == 'unexpected native tool invocation\n', 'actual portable native trap')
            same(t.rows(tool['control_log']), [{'unexpected_native_tool': True}], 'portable trap raw event')
    shards, child_processes, shard_configs, shard_tools = _hosted(t, api, contracts, configs, operations)
    keys, used, replacements, counts = set(), set(), {}, {}
    for section in SECTIONS:
        roster = contracts.roster(section, t.host['policy']['host'])
        same(public['rosters'][section], roster, 'frozen host roster')
        values = t.rows(_child(public['collection_root'], section, 'observations.jsonl.gz'), compressed=True)
        _public_section_closure(t, api, helpers, contracts, predecessors, section, roster, values)
        helpers['contracts'].check_inventory(roster, values)
        same({k: public['primary']['reports'][section][k] for k in runtime.aggregate(roster, values)}, runtime.aggregate(roster, values), 'primary report from raw rows')
        merged = []
        shard_index = {r['key']: r for r in shards.get(section, [])}
        replacements[section] = []
        for index, (row, base) in enumerate(zip(roster, values)):
            got, cfgs, selected_tools, child_records = base, configs, tools, None
            if base['status'] == 'unsupported-capability':
                sources = helpers['run'].tuple_sources(row, contracts, predecessors)
                if section == 'lifecycle':
                    baseline = t.json(base['ordinary_receipt'])
                    replay = copy.deepcopy(base)
                    runtime.lifecycle_compare(row, baseline, replay, configs['ordinary'], configs['lifecycle'], contracts, sources, tools)
                    same(replay, base, 'root primary incomplete observation changed')
                else:
                    runtime.core_compare(section, row, base, configs['ordinary'], contracts, sources, tools, predecessors)
                need(base['key'] in shard_index, 'missing genuine root gap replacement')
                got, cfgs, selected_tools, child_records = shard_index[base['key']], shard_configs, shard_tools, child_processes
                replacements[section].append(base['key'])
                api['hosted'].require_denial_proof(got, public['hosted']['report']['identity'])
            elif base['key'] in shard_index:
                raise Rejected('hosted shard replaced a primary executed/nonlocal row')
            merged.append(got)
            if row['scope'] != 'execute':
                same(got, runtime._skipped(row, configs['ordinary']), 'exact unavailable/excluded row')
                continue
            need(got['executed'] is True and got['status'] == 'pass' and row['key'] not in keys, 'actual unique required public tuple')
            keys.add(row['key'])
            case = {'section': section, 'index': index, 'variant': 'observer' if section in ('lifecycle', 'guards') else 'ordinary'}
            key = _raw_public_row(t, got, operations, hosted_processes=child_records, expected_case=case, row=row)
            need(key not in used, 'public compiler process reused across tuples')
            used.add(key)
            if section == 'guards':
                sources = contracts.source_bytes(contracts.cases[row['case']['source_case_id']])
                baseline = t.json(got['ordinary_receipt'])
                for observation, role in ((baseline, 'ordinary'), (got, 'lifecycle')):
                    runtime.check_guard(row, observation, cfgs[role], contracts, sources, selected_tools)
                _raw_public_row(t, baseline, operations, expected_case=dict(case, variant='ordinary'), row=row)
                need(all(got['process'][k] == baseline['process'][k] for k in ('status', 'stdout', 'stderr')) and
                     got['directory_after'] == baseline['directory_after'], 'guard ordinary/observer passivity')
                expected_case = copy.deepcopy(next(c for c in contracts.tables['lifecycle']['public_cases'] if c['id'] == row['case']['source_case_id']))
                if row['group'] == 'occupied':
                    expected_case['file_counts'] = {_child(got['fixture_directory'], name): n for name, n in expected_case['file_counts'].items()}
                logical = helpers['compare'].lifecycle(got['trace'], expected_case,
                    contracts.tables['lifecycle']['counter_order'], contracts.tables['lifecycle']['file_counter_order'])
                same(got['logical_lifecycle'], logical, 'guard derived lifecycle differs')
            else:
                sources = helpers['run'].tuple_sources(row, contracts, predecessors)
                if section == 'lifecycle':
                    baseline = t.json(got['ordinary_receipt'])
                    _raw_public_row(t, baseline, operations, hosted_processes=child_records, expected_case=dict(case, variant='ordinary'), row=row)
                    if child_records is not None: api['hosted'].require_denial_proof(baseline, public['hosted']['report']['identity'])
                    replay = copy.deepcopy(got)
                    runtime.lifecycle_compare(row, baseline, replay, cfgs['ordinary'], cfgs['lifecycle'], contracts, sources, selected_tools)
                    same(replay, got, 'saved derived lifecycle differs')
                else:
                    runtime.core_compare(section, row, got, cfgs['ordinary'], contracts, sources, selected_tools, predecessors)
            if section != 'predecessors': need(not any(k.startswith('qualified_paths_') for k in got), 'extra semantic amendment')
        replacements[section].sort()
        same(set(shard_index), set(replacements[section]), 'extra/missing replacement')
        counts[section] = runtime.aggregate(roster, merged)
        need(counts[section]['unsupported_capability'] == 0, 'incomplete capability coverage')
        same({k: public['document']['reports'][section][k] for k in counts[section]}, counts[section], 'terminal report independently reconstructed')
        if section == 'predecessors':
            path = _child(public['collection_root'], section, 'observations.jsonl.gz')
            history = runtime.predecessor_history(roster, values, predecessors,
                contracts.tables['public']['new_policy_diagnostic_bounds'], t.named(path))
            same(t.json(_child(public['collection_root'], section, 'historical-comparison.json')), history,
                 'unadapted predecessor semantic outcome')
            authority_path = _child(public['collection_root'], 'qualified-paths-amendment.json')
            same(t.json(authority_path), predecessors.qualified_paths_amendment, 'exact public semantic amendment')
            report = t.json(_child(public['collection_root'], section, 'comparison.json'))
            expected_fields = runtime.predecessor_report_fields(predecessors, history, t.named(authority_path),
                t.named(_child(public['collection_root'], section, 'historical-comparison.json')))
            for key, value in expected_fields.items(): same(report[key], value, 'predecessor amended report')
    collection_members = _relative_files(t, public['collection_root'])
    same({name for name in collection_members if '/' not in name.replace('\\', '/')},
         {'input-integrity.json', 'tools.json', 'qualified-paths-amendment.json'}, 'exact public collection root files')
    same(t.json(_child(public['collection_root'], 'tools.json')), tools_doc, 'actual selected tools raw body')
    if t.host['policy']['hosted_required']:
        same(public['document']['replaced_capability_keys'], replacements, 'exact twelve original gap replacement closure')
        need(sum(map(len, replacements.values())) == 12 and public['document']['primary_receipts_unchanged'] is True,
             'primary gaps cannot be rewritten')
    expected_keys = {row['key'] for section in SECTIONS for row in contracts.roster(section, t.host['policy']['host']) if row['scope'] == 'execute'}
    same(keys, expected_keys, 'complete actual host public tuple set')
    return {'counts': counts, 'keys': sorted(keys), 'replacements': replacements}


def _parser_authority(parser):
    """Reproduce only the reviewed recipe projection; issue no live authority."""
    manifest = parser._manifest()
    old = parser._module('unit4_ci_retained_parser_transport', parser.REPOSITORY / parser.OLD_PATH,
                         parser._record_at(manifest['retained_portable']))
    historical = parser.load(parser._record_at(manifest['historical_authority']))
    active = parser.load(parser._record_at(manifest['retained_authority']))
    source = parser.load(parser._record_at(manifest['source_authority']))
    candidate = parser._record_at(manifest['candidate_manifest'])
    original = copy.deepcopy(active)
    for field, rows in source['maps'].items():
        need(field in ('current_base_files', 'current_derived_files', 'current_control_derived_files'), 'exact parser map fields')
        active[field] = copy.deepcopy(rows)
    active['current_candidate_source_manifest_sha256'] = sha(candidate)
    active['current_source_manifest'] = copy.deepcopy(source['current_source_manifest'])
    active['reviewed_source_head'] = source['source_checkpoint']['head']
    active['source_only_tree'] = source['source_checkpoint']['tree']
    same({k for k in active if active[k] != original[k]}, set(manifest['projection_fields']), 'exact parser recipe projection')
    return old, dict(historical, current=active), manifest, candidate


def _parser_module(parser, t, old, recipe, manifest, builds, admissions):
    """Separate transport namespace, exact derived semantic code, explicit I/O.

    No retained module/global or canonical live consumer registry is modified.
    The only injected admission has a transport name and cannot create handles.
    """
    root = parser.REPOSITORY / parser.FROZEN_PATH / 'frozen/comparator'
    source, proof = parser.derive_comparator(parser.file_bytes(root / 'comparator.py'))
    decoder = types.ModuleType('unit4_ci_parser_transport_debug_decoder')
    decoder.__file__ = str(root / 'debug_decoder.py')
    exec(compile(parser.file_bytes(root / 'debug_decoder.py'), decoder.__file__, 'exec'), decoder.__dict__)
    original_import = builtins.__import__
    def isolated_import(name, globals=None, locals=None, fromlist=(), level=0):
        if name == 'debug_decoder' and level == 0: return decoder
        return original_import(name, globals, locals, fromlist, level)
    comparator = types.ModuleType('unit4_ci_parser_transport_comparator')
    comparator.__file__ = str(root / 'comparator.py')
    comparator.__dict__['__builtins__'] = {**vars(builtins), '__import__': isolated_import}
    exec(compile(source, comparator.__file__ + ':transport', 'exec'), comparator.__dict__)
    def admission(path, contract, approval):
        need(approval is admissions and contract is admissions['contract'], 'closed transported parser admission')
        original = str(path)
        profile = next((p for p in PROFILES if original == _child(t.host['parser']['root'], 'collect-' + p, 'result/execution-manifest.json')), None)
        need(profile is not None, 'exact transported collection manifest')
        value = t.json(original)
        same(value['schema'], 'oxid-unit4-parser-execution-v1', 'original collection schema')
        same(value['status'], 'collected', 'complete actual collection')
        same(value['contract_decoded_sha256'], recipe['contract_sha256'], 'original parser contract')
        same(value['package_freeze_sha256'], recipe['freeze_sha256'], 'original parser freeze')
        same(value['profile'], profile, 'parser collection profile')
        same(value['authority_checkpoint'], t.host['parser']['session_binding'], 'parser actual session checkpoint')
        same(value['build_receipt'], builds[(profile, False)][1]['receipt'], 'parser actual observed build receipt')
        same(value['host_runtime'], t.host['parser']['session']['host'], 'parser actual runtime host')
        for name, filename in (('driver', 'run.py'), ('normalizer', 'parse_debug.py')):
            same(value[name], t.named(_child(t.host['parser']['root'], 'helpers', filename)), 'original raw normalizer/helper identity')
            t.raw(value[name])
        expected_ids = [c['id'] for c in contract['cases']]
        same(value['requested_case_ids'], expected_ids, 'whole frozen parser roster')
        same([r['case_id'] for r in value['case_receipts']], expected_ids, 'actual exact ordered parser cases')
        for row in value['case_receipts']:
            case = row['case_id']
            nonce = row['execution_id']
            need(nonce not in admissions['nonces'], 'parser cross-profile/process nonce reuse')
            admissions['nonces'].add(nonce)
            directory = _child(t.host['parser']['root'], 'collect-' + profile, 'result', case)
            for key, filename in (('request', 'request.json'), ('raw', 'raw.json'), ('stdout', 'stdout.txt'), ('stderr', 'stderr.txt')):
                same(row[key], t.named(_child(directory, filename)), 'parser fixed raw member path')
            same(t.json(_child(directory, 'receipt.json')), row, 'parser actual case receipt body')
            request = t.json(row['request'])
            same(request['source'], t.named(_child(directory, 'source.ox')), 'parser raw source identity')
        return value, value['host_runtime'], builds[(profile, False)][0], profile
    # These bindings are created only on this separate transport module. Its
    # source-derived semantic suffix is unchanged; original modules stay intact.
    comparator.package_source_admission = admission
    comparator.artifact = t.raw
    runtime = parser._runtime_api()
    sources, derivation = runtime.derive_functions(parser._record_at(manifest['retained_portable']))
    same(derivation, manifest['runtime_derivations'], 'exact parser runtime derivation proof')
    derived = types.ModuleType('unit4_ci_parser_transport_semantics')
    derived.__dict__.update({name: getattr(old, name) for name in
        ('require', 'same', 'safe_relative', 'load', 'sha', 'phase_module', 'u8_policy_module',
         'phase_native_identity', 'verify_map', 'Rejected')})
    derived.__dict__.update(Path=Path, re=re, copy=copy, REPOSITORY=parser.REPOSITORY,
        CHECKPOINT=parser.CHECKPOINT, AUTHORITY_SHA=parser.AUTHORITY_SHA,
        PHASE_SCOPES=old.PHASE_SCOPES, PHASE_PROFILES=old.PHASE_PROFILES, PHASE_ROLES=old.PHASE_ROLES,
        ENUM_AST_SCHEMA_ADAPTER=copy.deepcopy(old.ENUM_AST_SCHEMA_ADAPTER), artifact=t.raw,
        identity=lambda path: t.named(str(path)),
        package_phase_authority=lambda value: same(value, recipe, 'exact parser transported phase authority'),
        package_build_finished_ns=lambda profile, control: builds[(profile, control)][1]['helper_probe']['completed_ns'],
        comparator=lambda: (comparator, proof))
    for name in ('phase_control_binding', 'phase_scope', 'phase_evidence', 'project_enum_observations'):
        exec(compile(sources[name], str(Path(__file__).resolve()) + ':' + name, 'exec'), derived.__dict__)
    return comparator, proof, derived


def _parser_builds(t, api, operations, recipe, candidate):
    parser, data = api['parser'], t.host['parser']
    session, root, source = data['session'], data['root'], t.host['source_context']
    same(t.json(data['session_binding']), session, 'actual parser checkpoint body')
    same(data['session_binding']['path'], _child(root, parser.CHECKPOINT), 'exact successor checkpoint path')
    need(session['schema'] == parser.SESSION_SCHEMA and session['root'] == root and
         session['host']['os'] == 'linux' and session['host']['architecture'] == 'x86_64' and
         session['host']['python_pointer_width'] == 64, 'new parser session actual Linux host')
    same(root, _child(t.host['invocation']['run_root'], 'parser'), 'fixed fresh parser root')
    same(session['context'], source, 'parser original source context')
    same(session['preparation'], source['preparation'], 'parser original preparation')
    same(t.json(session['shared_context']), source, 'parser retained shared-context body')
    same(session['context_sha256'], sha(parser.serial(source)), 'parser exact context serialization')
    same(session['authority']['sha256'], parser.AUTHORITY_SHA, 'parser consumer authority')
    for role, dirname in (('parser_observed', 'source'), ('parser_control', 'control-source')):
        view = source['views'][role]
        for row in view['files']:
            path = _child(root, dirname, row['path'])
            same(t.named(path), dict(row, path=path), 'parser copied exact role source map')
        same(t.raw(_child(root, dirname, 'candidate-source-manifest.json')), candidate,
             'both generated parser candidate manifests retained')
        helper_manifest = t.raw(_child(root, dirname, 'observer-source-manifest.json'))
        same(helper_manifest, canonical(recipe['helper_files']), 'generated parser helper manifest exact bytes')
        for row in view['metadata']:
            path = _child(root, 'preparation-metadata', role, row['path'])
            same(t.named(path), dict(row, path=path), 'parser separate preparation metadata inventory')
            t.raw(path)
    builds, binaries = {}, set()
    for record in data['builds']:
        outer, native = record['document'], record['native']
        same(t.json(record['binding']), outer, 'original parser build envelope body')
        same(t.json(outer['receipt']), native, 'original parser native build receipt body')
        key = outer['profile'], outer['control']
        need(key in {(p, c) for p in PROFILES for c in (False, True)} and key not in builds,
             'exact parser role/profile build inventory')
        profile, control = key
        need(outer['schema'] == 'oxid-package-source-parser-build-v1' and outer['status'] == 'built' and
             outer['execution_qualified'] is False, 'new parser build transport schema')
        same(outer['session'], data['session_binding'], 'parser build actual session')
        same(outer['invocation'], t.host['invocation'], 'parser build original invocation')
        same(outer['machine_sha256'], sha(parser.serial(t.host['machine'])), 'parser build original machine')
        same(outer['authority'], session['authority'], 'parser build authority')
        same(outer['adapter'], session['adapter'], 'parser build adapter')
        same(outer['output'], _child(root, ('build-control-' if control else 'build-') + profile), 'fixed parser build root')
        same(outer['source'], _child(root, 'control-source' if control else 'source'), 'fixed parser source role')
        environment = dict(t.host['machine']['snapshot']['environment'], CARGO_TARGET_DIR=_child(outer['output'], 'result/target'))
        environment.update(recipe['recipe']['build_environment'])
        environment.update(HOME=_child(outer['output'], 'home'), CARGO_HOME=_child(outer['output'], 'cargo-home'),
                           PATH=_child(outer['output'], 'rust-bin') + ':/usr/bin:/bin', PYTHONDONTWRITEBYTECODE='1',
                           PYTHONNOUSERSITE='1', PYTHONOPTIMIZE='0')
        same(outer['environment'], environment, 'exact original parser build environment')
        for process, kind, seconds in ((outer['sysroot_probe'], 'probe', parser.PROBE_TIMEOUT_SECONDS),
                                       (outer['helper_probe'], 'parser-helper', parser.BUILD_TIMEOUT_SECONDS)):
            _same_owned(process, operations, t)
            need(process['status'] == 0 and process['operation_kind'] == kind and process['timeout_seconds'] == seconds and
                 process['stream_limit_bytes'] == parser.BUILD_STREAM_LIMIT, 'parser build exact process recipe/limits')
            same(process['environment'], environment, 'actual parser build clean environment')
            same(process['cwd'], root, 'actual parser build cwd')
        same(outer['sysroot_probe']['argv'], [_child(outer['output'], 'rust-bin/rustc'), '--print', 'sysroot'], 'actual narrow-view sysroot command')
        same(t.raw(outer['sysroot_probe']['stdout']), (t.host['machine']['snapshot']['rust']['root'] + '\n').encode(), 'actual parser sysroot')
        same(t.raw(outer['sysroot_probe']['stderr']), b'', 'actual parser sysroot stderr')
        helper = outer['helper_probe']
        same(helper['argv'], [session['host']['python_executable'], '-B', _child(root, 'helpers/build.py'),
             '--overlay', _child(outer['source'], 'overlay-manifest.json'), '--profile', profile,
             '--output', _child(outer['output'], 'result')], 'actual parser helper argv')
        same(t.json(helper['stdout']), native, 'actual helper stdout/native receipt')
        same(t.raw(helper['stderr']), b'', 'actual parser helper stderr')
        need(session['prepared_ns'] <= outer['sysroot_probe']['started_ns'] <= outer['sysroot_probe']['completed_ns'] <=
             helper['started_ns'] <= helper['completed_ns'], 'parser preparation/sysroot/build order')
        need(native['schema'] == 'oxid-unit4-parser-build-v1' and native['profile'] == profile and native['control'] is control and
             native['status'] == 'built' and native['exit_code'] == 0 and native['target'] == recipe['recipe']['target'] and
             native['rustc_version'] == recipe['recipe']['rustc_verbose'], 'actual frozen parser build recipe')
        same(native['candidate_source_manifest_sha256'], recipe['current']['current_candidate_source_manifest_sha256'], 'actual candidate source manifest')
        same(native['observer_source_sha256'], recipe['helper_manifest_sha256'], 'actual parser observer helper manifest')
        same(native['overlay_manifest'], session['control_overlay' if control else 'overlay'], 'actual parser overlay')
        overlay = t.json(native['overlay_manifest'])
        same(overlay['files'], source['views']['parser_control' if control else 'parser_observed']['files'], 'exact parser overlay body map')
        same(native['binary'], outer['binary'], 'parser copied/native binary identity')
        same(t.named(native['binary']['path']), native['binary'], 'parser binary full evidence inventory')
        need(native['binary']['sha256'] not in binaries, 'parser binary reused across roles/profiles')
        binaries.add(native['binary']['sha256'])
        rows = t.rows(native['stdout'])
        emitted = [r for r in rows if r.get('reason') == 'compiler-artifact' and r.get('executable')]
        need(len(emitted) == 1 and emitted[0]['fresh'] is False and emitted[0]['executable'] == native['binary']['path'] and
             emitted[0]['profile']['test'] is True and emitted[0]['target']['name'] == 'oxid', 'actual fresh parser Cargo artifact')
        same([r for r in rows if r.get('reason') == 'build-finished'], [{'reason': 'build-finished', 'success': True}], 'complete Cargo success')
        t.raw(native['stderr'])
        builds[key] = native, outer
    same(set(builds), {(p, c) for p in PROFILES for c in (False, True)}, 'all four parser builds before runtime')
    return builds


def _parser_envelope(t, profile, kind, builds, operations, old):
    data, root = t.host['parser'], t.host['parser']['root']
    directory = _child(root, ('collect-' if kind == 'collection' else 'passivity-') + profile)
    envelope = t.json(_child(directory, 'package-source-' + kind + '.json'))
    same(envelope['schema'], 'oxid-package-source-parser-' + kind + '-v1', 'exact successor parser envelope')
    same(envelope['session'], data['session_binding'], 'parser stage actual checkpoint')
    same(envelope['profile'], profile, 'parser exact stage profile')
    process = envelope['owned_process']
    _same_owned(process, operations, t)
    need(process['status'] == 0 and process['started_ns'] >= max(b[1]['helper_probe']['completed_ns'] for b in builds.values()),
         'parser stage must follow all four actual builds')
    helper = 'run.py' if kind == 'collection' else 'passivity.py'
    argv = [data['session']['host']['python_executable'], '-B', _child(root, 'helpers', helper)]
    if kind == 'collection':
        argv += ['--build-receipt', builds[(profile, False)][1]['receipt']['path'], '--contract-dir', data['contract_root']]
    else:
        argv += ['--instrumented', builds[(profile, False)][1]['receipt']['path'],
                 '--control', builds[(profile, True)][1]['receipt']['path']]
    argv += ['--checkpoint', data['session_binding']['path'], '--output', _child(directory, 'result')]
    need(process['operation_kind'] == ('parser-collect' if kind == 'collection' else 'parser-passivity') and
         process['timeout_seconds'] == (1800 if kind == 'collection' else 900) and
         process['stream_limit_bytes'] == 8 * 1024 ** 2, 'original exact parser helper process limits')
    same(process['argv'], argv, 'original exact parser runtime helper command')
    same(process['cwd'], root, 'original exact parser runtime helper cwd')
    same(process['environment'], old.minimal_env(_remote_path(root) / 'home'), 'original exact parser runtime helper environment')
    key, filename = ('manifest', 'execution-manifest.json') if kind == 'collection' else ('report', 'report.json')
    same(envelope[key], t.named(_child(directory, 'result', filename)), 'original exact parser runtime output')
    same(envelope['invocation'], t.named(_child(directory, 'invocation.json')), 'original exact parser process projection path')
    if kind == 'collection': same(envelope['contract_dir'], data['contract_root'], 'original parser collection contract route')
    wanted = ({'status': 'collected', 'cases': 248, 'observations': 319, 'manifest': envelope[key]['path']}
              if kind == 'collection' else {'status': 'pass', 'pairs': 6, 'report': envelope[key]['path']})
    same(t.json(process['stdout']), wanted, 'actual frozen parser helper stdout')
    same(t.raw(process['stderr']), b'', 'actual frozen parser helper stderr')
    invocation = t.json(envelope['invocation'])
    same(invocation['owned_process'], process, 'parser projection original complete process')
    same(invocation['argv'], process['argv'], 'parser projected command unchanged')
    same(invocation['cwd'], process['cwd'], 'parser projected cwd unchanged')
    same(invocation['environment'], process['environment'], 'parser projected environment unchanged')
    same(invocation['exit_code'], process['status'], 'parser projected exit unchanged')
    same((invocation['started_ns'], invocation['finished_ns']), (process['started_ns'], process['completed_ns']),
         'parser actual projection process window')
    same(invocation['host'], data['session']['host'], 'parser actual projected host')
    for name in ('stdout', 'stderr'):
        same(invocation[name], t.named(_child(directory, 'driver.' + name)), 'exact projected parser stream route')
        same(t.raw(invocation[name]), t.raw(process[name]), 'parser raw stream copied unchanged')
    return envelope, invocation


def _passivity(t, profile, envelope, recipe, builds, nonces):
    report = t.json(envelope['report'])
    same(report['schema'], 'oxid-unit4-parser-passivity-v1', 'original passivity report schema')
    same(report['status'], 'pass', 'ordinary passivity failed')
    same(report['scope'], 'three hand-prescribed ordinary controls in both modes; no seam equivalence or full-corpus assertion',
         'unchanged passivity scope')
    same(report['instrumented'], builds[(profile, False)][1]['receipt'], 'actual observed passivity binary')
    same(report['control'], builds[(profile, True)][1]['receipt'], 'actual control passivity binary')
    same(report['authority_checkpoint'], t.host['parser']['session_binding'], 'actual passivity checkpoint')
    prescribed = recipe['recipe']['passivity']
    same([r['case'] for r in report['results']], [name for name, _ in prescribed['cases']], 'exact ordinary passivity roster')
    pairs = 0
    for row, (name, source) in zip(report['results'], prescribed['cases']):
        result_root = _child(t.host['parser']['root'], 'passivity-' + profile, 'result')
        same(row['source'], t.named(_child(result_root, name + '.ox')), 'exact ordinary passivity source route')
        same(t.raw(row['source']), source.encode(), 'exact passivity source')
        same(row['pairs'], 2, 'both ordinary passivity modes')
        same(row['equal_fields'], prescribed['compare_fields'], 'unchanged passivity compared fields')
        need(len(row['receipts']) == 2, 'both ordinary passivity role observations')
        observations = []
        for index, process in enumerate(row['receipts']):
            binary = builds[(profile, bool(index))][0]['binary']
            same(process['argv'], [binary['path'], recipe['recipe']['entrypoint'], '--exact', '--ignored', '--nocapture', '--test-threads=1'],
                 'actual ordinary passivity command')
            same(process['exit_code'], 0, 'passivity actual process exit')
            work = _child(result_root, name + ('-control' if index else '-instrumented'))
            for field, filename in (('raw', 'raw.json'), ('stdout', 'stdout.txt'), ('stderr', 'stderr.txt')):
                same(process[field], t.named(_child(work, filename)), 'exact original passivity raw route')
            raw = t.json(process['raw'])
            need(raw['schema'] == 'oxid-unit4-parser-raw-v1' and raw['case_id'] == name and raw['source_utf8'] == source and
                 raw['display_path'] == name + '.ox', 'passivity raw source/case/schema')
            nonce = raw['nonce']
            need(type(nonce) is str and re.fullmatch('[0-9a-f]{32}', nonce) and nonce not in nonces, 'passivity nonce reuse')
            nonces.add(nonce)
            stdout = t.raw(process['stdout']).decode()
            need(stdout.count('UNIT4_EXECUTED ' + nonce) == 1 and
                 re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;', stdout), 'actual passivity execution witness')
            t.raw(process['stderr'])
            same([x['mode'] for x in raw['observations']], prescribed['modes'], 'exact passivity mode roster')
            for mode_index, observed in enumerate(raw['observations']):
                need(observed['executed'] is True and observed['mode_execution_index'] == mode_index and
                     observed['runtime_os'] == 'linux' and observed['runtime_architecture'] == 'x86_64' and
                     observed['pointer_width'] == 64, 'actual complete passivity mode')
            observations.append(raw['observations'])
        for left, right in zip(*observations):
            for field in prescribed['compare_fields']: same(left[field], right[field], 'ordinary passivity semantic field ' + field)
            if left['result'] == 'ok': same(left['nodes_admitted'], right['nodes_admitted'], 'successful passivity node ledger')
            pairs += 1
    same(pairs, prescribed['pairs_per_profile'], 'all six ordinary passivity pairs')
    return {'profile': profile, 'pairs': pairs, 'report': envelope['report'], 'scope': prescribed['scope']}


def _parser_replay(t, api, operations):
    parser, data = api['parser'], t.host['parser']
    old, recipe, manifest, candidate = _parser_authority(parser)
    same(data['recipe'], recipe, 'parser recipe independently reconstructed from trusted fixed authorities')
    report = data['report']
    need(report['schema'] == 'oxid-package-source-parser-runtime-v1' and report['status'] == 'NotReady' and
         report['execution_qualified'] is False and report['local_semantics_status'] == 'pass', 'original local parser boundary')
    same(report['stages'], list(_evidence().PARSER_SEQUENCE), 'complete seven parser stages')
    builds = _parser_builds(t, api, operations, recipe, candidate)
    _parser_inputs(t, api)
    admissions = {'nonces': set()}
    comparator, proof, derived = _parser_module(parser, t, old, recipe, manifest, builds, admissions)
    contract = comparator.load_contract(t.local_path(data['contract_root']))
    admissions['contract'] = contract
    _parser_runtime_closure(t, parser._runtime_api(), old, recipe, contract)
    need(len(contract['cases']) == 248 and sum(1 + ('relation_to_original' in c['expected']) for c in contract['cases']) == 319,
         'unchanged whole parser corpus')
    result = t.json(report['comparison'])
    need(result['schema'] == 'oxid-package-source-parser-comparison-v1' and result['status'] == 'NotReady' and
         result['execution_qualified'] is False and result['local_semantics_status'] == 'pass', 'current parser comparison schema/boundary')
    same(result['session'], data['session_binding'], 'actual compared parser session')
    same(result['source_manifest'], t.host['source_context']['current_source_manifest'], 'actual compared source identity')
    same(result['authority_sha256'], parser.AUTHORITY_SHA, 'actual compared parser authority')
    same(result['comparator_derivation'], proof, 'exact unchanged comparator predicate derivation')
    same(result['runtime_derivations'], manifest['runtime_derivations'], 'exact parser runtime derivation proof')
    rows, identities, passive = [], {}, []
    previous = max(v[1]['helper_probe']['completed_ns'] for v in builds.values())
    for profile in PROFILES:
        envelope, invocation = _parser_envelope(t, profile, 'passivity', builds, operations, old)
        need(previous <= invocation['started_ns'], 'parser actual stage order')
        previous = invocation['finished_ns']
        passive.append(_passivity(t, profile, envelope, recipe, builds, admissions['nonces']))
        envelope, invocation = _parser_envelope(t, profile, 'collection', builds, operations, old)
        need(previous <= invocation['started_ns'], 'parser actual collection follows passivity')
        previous = invocation['finished_ns']
        actual, observations, bound = comparator.execution_manifest(
            _child(data['root'], 'collect-' + profile, 'result/execution-manifest.json'), contract, admissions)
        same(actual, profile, 'parser complete profile order')
        need(len(observations) == 319, 'complete parser profile observations')
        rows.extend(observations); identities[profile] = bound
    need(len(rows) == 638 and sum(v['pairs'] for v in passive) == 12, 'all parser observations/passivity pairs')
    same(result['ordinary_passivity'], passive, 'parser passivity outcome reconstructed')
    effective, receipt = comparator.admit_contract_amendment(contract, t.local_path(data['contract_root']), old.effective_authority(recipe))
    unadapted = comparator.compare_effective_rows(effective, receipt, rows, identities)
    projected, projection = derived.project_enum_observations(recipe, rows)
    frozen = comparator.compare_effective_rows(effective, receipt, projected, identities)
    current, current_receipt = old.current_parser_contract(recipe, effective, receipt)
    amended = comparator.compare_rows(current, projected, identities)
    amended.update(current_receipt)
    same(result['enum_structural_projection'], projection, 'exact original/raw enum projection receipt')
    for field, value in (('unadapted_frozen_comparison', unadapted), ('frozen_comparison', frozen), ('current_comparison', amended)):
        same(result[field], value, 'independent complete raw semantic comparison: ' + field)
        same(result[field + '_canonical_sha256'], comparator.sha(comparator.canonical(value)), 'semantic comparison body hash')
    same(amended['status'], 'pass', 'current full parser semantic comparison failed')
    controls = t.json(result['u8_policy_controls'])
    same(controls['session'], data['session_binding'], 'u8 actual session')
    same(controls['closed_policy'], recipe['current']['u8_closed_policy'], 'unchanged u8 closed policy')
    policy = old.u8_policy_module(recipe)
    policy_result = policy.verify(old, controls, t.raw, {k: v[0]['binary'] for k, v in builds.items()}, data['session'])
    for row in controls['cases']:
        if row['case'] != 'enabled-public':
            need(row['nonce'] not in admissions['nonces'], 'u8 reused earlier parser nonce')
            admissions['nonces'].add(row['nonce'])
    phase = derived.phase_evidence(recipe, Path(data['session_binding']['path']), resolve=t.raw,
                                   identities=t.index, builds={k: v[0] for k, v in builds.items()})
    same(result['lexer_phase_envelopes'], phase['envelopes'], 'exact independently reconstructed six lexical envelopes')
    same(result['lexer_phase_controls'], phase['probe_index'], 'complete independent54 lexical probes')
    _parser_process_catalogue(t, old, recipe, builds, operations, derived)

    return {'observations': 638, 'passivity_pairs': 12, 'builds': 4, 'lexical_processes': 524,
            'lexical_envelopes': 6, 'lexical_probes': 54, 'u8': policy_result,
            'comparison': report['comparison'], 'current_status': amended['status'],
            'unadapted_frozen_status': unadapted['status'], 'projected_frozen_status': frozen['status']}


def _staging(t, expected):
    q = _common()
    staging = q.load_sibling('staging')
    data = t.host['staging']
    if data is None: return None
    need(data['schema'] == 'oxid-package-unit4-ci-staging-v1', 'new live CI staging envelope')
    same(data['ci'], t.host['ci_context'], 'staging actual CI identity')
    same(data['invocation'], t.host['invocation'], 'staging actual invocation')
    same(data['machine_sha256'], q.sha(q.canonical(t.host['machine'])), 'staging original machine')
    same(data['root'], _child(t.host['ci_context']['outer_root'], 'staging'), 'fresh fixed staging root')
    same(data['library_root'], _child(data['root'], 'runtime/libraries'), 'fresh fixed staged library route')
    same(t.json(data['runtime_receipt']), data['original_receipt'], 'original unchanged stage receipt bytes')
    receipt = data['original_receipt']
    need(receipt['schema'] == 'oxid-unit3-portable-v1-llvm-runtime-stage' and receipt['status'] == 'verified', 'original installed-dpkg stage receipt')
    same(receipt['packages'], staging._expected_packages(), 'exact installed dpkg package claims')
    same(receipt['helper'], data['dependencies']['stager'], 'original staging helper')
    # Its dependency inventory supplies the authenticated path; never guess an
    # alternative helper after a mismatch.
    for field, relative in (('stager', staging.STAGER), ('common', staging.HELPER), ('process_tree', staging.TREE)):
        row = data['dependencies'][field]
        same(row['path'], _child(t.host['ci_context']['repository_path'], relative), 'staging fixed helper path')
        _body_identity(row, q.identity(Path(expected['repository_path']) / relative), 'staging fixed helper body')
    commands = staging._queries(data['dependencies']) + [('stage-installed-llvm',
        [data['dependencies']['python']['resolved'], '-I', '-S', '-B', data['dependencies']['stager']['path'],
         '--output', _child(data['root'], 'runtime')])] + staging._queries(data['dependencies'], after=True)
    same([r['name'] for r in data['operations']], [n for n, _ in commands], 'all fifteen ordered dpkg/stager operations')
    previous = data['started_ns']
    for row, (name, argv) in zip(data['operations'], commands):
        same(row['argv'], argv, 'actual staging command')
        same(row['cwd'], data['root'], 'actual staging cwd')
        same(row['environment'], staging._environment(Path(data['root'])), 'actual clean staging environment')
        need(type(row['pid']) is int and row['pid'] > 0 and row['status'] == 0 and
             row['cleanup_complete'] is True and row['cleanup_errors'] == [] and row['failure'] is None and
             row['timed_out'] is False and row['stream_limit_exceeded'] is False and
             previous <= row['started_ns'] < row['completed_ns'] <= data['completed_ns'], 'staging actual success/cleanup/window')
        _staging_process_limits(t, staging, data, row, name)
        previous = row['completed_ns']
        same(t.json(_child(data['root'], 'commands', name, 'process.json')), row, 'actual staging process carrier')
        same(t.raw(row['stderr']), b'', 'staging stderr')
        t.raw(row['stdout'])
    by_name = {r['name']: r for r in data['operations']}
    for after in (False, True):
        for package, selector in sorted(staging.PACKAGES.items()):
            prefix = ('after-' if after else 'before-') + package
            same(t.raw(by_name[prefix + '-status']['stdout']).decode(),
                 f'{selector}\t{staging.PACKAGE_VERSION}\tamd64\tinstalled\n', 'actual dpkg installed status')
            owned = t.raw(by_name[prefix + '-listfiles']['stdout']).decode().splitlines()
            names = [name for name, row in staging.FILES.items() if row['package'] == package]
            if package == 'libllvm19': names.append(staging.ALIAS)
            for name in names:
                path = str(staging.SOURCE_ROOT / name)
                need(owned.count(path) == 1, 'actual dpkg-owned library membership')
                same(t.raw(by_name[prefix + '-owner-' + name]['stdout']).decode(), f'{selector}: {path}\n', 'actual dpkg library owner')
    for name, row in staging.FILES.items():
        bound = t.named(_child(data['library_root'], name))
        _body_identity(bound, row, 'fresh staged exact LLVM library')
    same(receipt['library_directory'], data['library_root'], 'actual original fresh staging destination')
    entries = [{'path': name, 'kind': 'file', 'soname': name, **row} for name, row in staging.FILES.items()]
    entries.append({'path': staging.ALIAS, 'kind': 'symlink', 'link': staging.ALIAS_TARGET, 'package': 'libllvm19'})
    content = {'package_version': staging.PACKAGE_VERSION, 'architecture': 'amd64', 'entries': sorted(entries, key=lambda r: r['path'])}
    same(receipt['content'], content, 'original unchanged stage content')
    same(receipt['content_sha256'], sha(canonical(content)), 'original stage content identity')
    _staging_closure(t, staging, data)
    selected = t.host['public']['runtime_tools']['selected_runtime']
    same(selected['receipt'], data['runtime_receipt'], 'public tools consume this freshly staged receipt')
    need(t.host['public']['runtime_tools']['receipt_producer_authenticated'] is False and t.host['public']['runtime_tools']['hosted_staging_admitted'] is False,
         'old content-only staging flags cannot be relabelled')
    return {'fresh_installed_dpkg_staging': True, 'operations': len(commands), 'receipt': data['runtime_receipt']}


def verify_transport(expected_context, transport):
    with _evidence().budget(time.monotonic() + 2700):
        return _verify_transport(expected_context, transport)


def _verify_transport(expected_context, transport):
    q = _common()
    expected = q.verify_join_context(expected_context)
    q.require_activation(expected_context)
    domain = _provenance(q, expected, transport.host)
    transport.expected_context = expected_context
    api = q.apis(expected_context)
    _source(q, expected, transport, api)
    _journal(transport)
    operations = _operations(transport)
    _operation_policies(transport, api, operations)
    result = {'domain': domain, 'host': transport.host['policy']['host'], 'nonce': transport.host['invocation']['nonce'],
              'ci_nonce': transport.host['ci_context']['ci_nonce'], 'public': None, 'parser': None, 'staging': None}
    _preparation_machine_operations(transport, api, operations)
    if transport.host['policy']['public_required']: result['public'] = _public_replay(transport, api, expected, operations)
    if transport.host['policy']['parser_required']: result['parser'] = _parser_replay(transport, api, operations)
    if transport.host['policy']['staging_required']: result['staging'] = _staging(transport, expected)
    same(transport.used_operations, set(operations), 'unconsumed or unrecognized owned operation')
    _evidence().tick()
    same(q.verify_join_context(expected_context), expected, 'trusted join admission changed during semantic readback')
    result['status'] = 'transport-verified'
    return result


def join(expected_context, capsule_paths):
    """Always return a structured final outcome; success requires all conjunctions."""
    result = {'schema': JOIN_SCHEMA, 'status': 'failed', 'qualification_complete': False,
              'domains': {}, 'failure': None, 'ci_runtime_proof': False}
    transports = []
    phase = None
    try:
        evidence = _evidence()
        phase = evidence.budget(time.monotonic() + 2700)
        phase.__enter__()
        q = _common()
        expected = q.verify_join_context(expected_context)
        result['expected'] = expected
        q.require_activation(expected_context)
        need(type(capsule_paths) in (list, tuple) and len(capsule_paths) == len(DOMAINS), 'exactly five domain capsules required')
        jobs = expected['upstream_job_results']
        same(set(jobs), set(q.JOBS.values()), 'exact three upstream jobs required')
        need(all(value == 'success' for value in jobs.values()), 'upstream job failed/cancelled/skipped')
        same(expected['required_domains'], list(DOMAINS), 'trusted expected exact domain set')
        nonces, ci_nonces, public_hosts, all_keys = set(), set(), set(), set()
        for path in capsule_paths:
            transport = q.load_sibling('evidence').read_capsule(path, expected_context)
            transports.append(transport)
            row = transport.verification
            domain = row['domain']
            need(domain not in result['domains'], 'duplicate actual domain capsule')
            need(row['nonce'] not in nonces and row['ci_nonce'] not in ci_nonces, 'reused domain producer/CI nonce')
            nonces.add(row['nonce']); ci_nonces.add(row['ci_nonce'])
            if row['public'] is not None:
                need(row['host'] not in public_hosts, 'duplicate public host')
                public_hosts.add(row['host'])
                keys = set(row['public']['keys'])
                need(not keys.intersection(all_keys), 'cross-host tuple reuse')
                all_keys.update(keys)
            result['domains'][domain] = row
        same(set(result['domains']), set(DOMAINS), 'missing/extra actual domain')
        same(public_hosts, set(PUBLIC_HOSTS), 'exact four public hosts required')
        need(result['domains']['linux-parser']['parser']['current_status'] == 'pass' and
             result['domains']['linux-public']['staging']['fresh_installed_dpkg_staging'] is True and
             sum(map(len, result['domains']['linux-public']['public']['replacements'].values())) == 12,
             'Linux full parser/fresh staging/root replacement conjunction')
        same(q.verify_join_context(expected_context), expected, 'terminal join CI readmission changed')
        result.update(status='pass', qualification_complete=True, ci_runtime_proof=True,
                      public_host_count=4, domain_count=5, public_tuple_count=len(all_keys),
                      qualification_boundary='Exact-run current-source Unit4 CI only; other mandatory repository gates remain separate.')
    except BaseException as error:
        result['failure'] = {'type': type(error).__name__, 'message': str(error)[:8192]}
    finally:
        for transport in transports:
            try: transport.close()
            except BaseException as error:
                failure = {'type': type(error).__name__, 'message': str(error)[:8192]}
                result.setdefault('preservation_failures', []).append(failure)
                result.update(status='failed', qualification_complete=False, ci_runtime_proof=False)
                if result['failure'] is None: result['failure'] = failure
        if phase is not None:
            try: phase.__exit__(None, None, None)
            except BaseException as error:
                result.update(status='failed', qualification_complete=False, ci_runtime_proof=False, failure={'type': type(error).__name__, 'message': str(error)[:8192]})
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', required=True)
    parser.add_argument('--expected-head', required=True)
    parser.add_argument('--event-sha', required=True)
    parser.add_argument('--capsule', action='append', default=[])
    parser.add_argument('--output', required=True)
    args = parser.parse_args(argv)
    q = _common()
    try:
        context = q.expected_join(args.repository, expected_head=args.expected_head, event_sha=args.event_sha)
        result = join(context, args.capsule)
    except BaseException as error:
        result = {'schema': JOIN_SCHEMA, 'status': 'failed', 'qualification_complete': False, 'ci_runtime_proof': False,
                  'failure': {'type': type(error).__name__, 'message': str(error)[:8192]}}
    q.write_new(Path(args.output), result)
    print(json.dumps(result, sort_keys=True))
    return 0 if result['status'] == 'pass' else 1



def _relative_files(t, root):
    parent = _evidence().original_path(root)
    return {str(_evidence().original_path(path).relative_to(parent)): path
            for path in t.index if parent in _evidence().original_path(path).parents}


def _public_section_closure(t, api, helpers, contracts, predecessors, section, roster, receipts):
    """Exact original per-case file grammar, with actual retained source bytes."""
    runtime = api['public_runtime']
    root = t.host['public']['collection_root']
    state = {'collection_root': _evidence().original_path(root), 'contracts': contracts, 'helpers': helpers, 'predecessors': predecessors}
    required = {section + '/' + name for name in ('roster.json', 'observations.jsonl.gz', 'comparison.json')}
    optional, links = set(), set()
    if section == 'predecessors': required.add(section + '/historical-comparison.json')
    same(t.json(_child(root, section, 'roster.json')), [helpers['contracts'].receipt_identity(r) for r in roster],
         'actual complete raw roster file')
    same(t.json(_child(root, section, 'comparison.json')), t.host['public']['primary']['reports'][section],
         'actual primary comparison report file')
    for index, (row, got) in enumerate(zip(roster, receipts)):
        _evidence().tick()
        if row['scope'] != 'execute': continue
        current = _transport_case_roster(runtime, t)(state, section, index, row, got)
        for destination, values in zip((required, optional, links), current): destination.update(values)
        prefix = section + '/' + (('%03d' if section == 'guards' else '%05d') % index)
        same(t.json(_child(root, prefix, 'receipt.json')), got, 'actual indexed public receipt')
        sources = runtime._row_sources(state, section, row)
        values = [got]
        if got.get('ordinary_receipt'):
            same(got['ordinary_receipt']['path'], _child(root, prefix, 'ordinary/receipt.json'), 'exact ordinary counterpart path')
            values.append(t.json(got['ordinary_receipt']))
        for value in values:
            role = 'observer' if value is got and section in ('lifecycle', 'guards') else 'ordinary'
            fixture = (_child(root, prefix, 'fixtures', row['case']['source_case_id']) if section == 'guards' else
                       _child(root, prefix, role, 'fixture'))
            same(value['fixture_directory'], fixture, 'exact indexed public fixture route')
            for name, raw in sources.items():
                same(t.raw(_child(fixture, name)), raw, 'transported actual fixture differs from frozen derived source')
            same(t.json(_child(root, prefix, role, 'receipt.json')), value, 'actual ordinary/observer receipt body')
            if section == 'guards':
                same(value['fixture_root'], _child(root, prefix, 'fixtures'), 'exact guard fixture root')
                same(value['output'], _child(fixture, 'out.bin') if row['group'] == 'race' else _child(root, prefix, 'occupied-output'),
                     'exact guard target output route')
                if row['group'] == 'race':
                    linked = value['linked_artifact']
                    same(linked['path'], _child(root, prefix, role, 'linked-before-publication.elf'), 'exact retained guard linked artifact')
                    same(t.named(linked['path']), linked, 'guard retained linked artifact identity')
                    if role == 'ordinary':
                        same(sha(t.raw(_child(root, prefix, role, 'competitor.bytes'))), value['output_after']['sha256'],
                             'guard ordinary competitor actual bytes')
                    else:
                        same(sha(t.raw(value['output'])), value['output_after']['sha256'], 'guard final competitor actual bytes')
                else:
                    target = _child(root, prefix, 'preserved-target')
                    target_bytes = t.raw(target)
                    same(target_bytes, b'Oxid Unit3 no-clobber sentinel\x00\xff\n', 'original occupied target sentinel')
                    if value['output_after']['kind'] == 'symlink':
                        same(sha(target_bytes), value['output_after']['target_sha256'], 'occupied guard target actual bytes')
                    else:
                        same(t.raw(value['output']), b'Oxid Unit3 occupied output sentinel\x00\xfe\n', 'original occupied output sentinel')
                        same(sha(t.raw(value['output'])), value['output_after']['sha256'], 'occupied guard output actual bytes')
    actual = {section + '/' + name.replace('\\', '/') for name in _relative_files(t, _child(root, section))}
    observed_links = {str(_evidence().original_path(r['path']).relative_to(_evidence().original_path(root))).replace('\\', '/')
                      for r in t.seal['inventory']['links'] if _evidence().original_path(_child(root, section)) in _evidence().original_path(r['path']).parents}
    need(required <= actual <= required | optional and observed_links == links, 'exact public raw case/member closure')
    directory_names = {str(parent) for name in actual | observed_links for parent in PurePosixPath(name).parents
                       if str(parent) not in ('.', section)}
    same({section + '/' + name for name in _relative_directories(t, _child(root, section))},
         directory_names, 'exact public raw directory closure')
    for link in t.seal['inventory']['links']:
        if link['path'] in {_child(root, name) for name in links}:
            same(link['target'], 'preserved-target', 'exact occupied-output alias')


def _generator(t, helpers, contracts, operations):
    public = t.host['public']
    generator = public['primary']['generator']
    root = _child(public['root'], 'original-generator')
    same(t.json(_child(root, 'verification.json')), generator, 'actual original generator receipt')
    need(generator['status'] == 'SOURCE_IDENTITIES_VERIFIED' and generator['compiler_invocations'] == 0,
         'original complete generator identity')
    same(generator['execution_context'], public['execution_context'], 'generator source runtime context')
    _same_owned(generator['owned_process'], operations, t)
    process = generator['owned_process']
    need(process['status'] == 0 and process['operation_kind'] == 'generator' and process['timeout_seconds'] == 60 and
         process['stream_limit_bytes'] == 8 * 1024 ** 2 and t.raw(process['stderr']) == b'', 'actual generator operation/limits')
    same(process['argv'], [t.host['ci_context']['host']['executable'], _child(root, 'generate_fixtures.py')], 'exact original generator command')
    same(process['cwd'], root, 'exact original generator cwd')
    same(process['environment'], public['runtime_tools']['environment'], 'exact original generator environment')
    expected = helpers['predecessors'].authority_path(contracts, '/typed_project_unit1_compatibility/suite-intent.json')
    raw_expected = expected.read_bytes()
    intent = json.loads(raw_expected)
    same(generator['sources'], [{k: r[k] for k in ('path', 'bytes', 'sha256')} for r in intent['cases']], 'complete35 generated source identities')
    raw = t.raw(generator['manifest_newline_transport']['raw_generated'])
    projected = helpers['predecessors'].original_manifest_transport(raw, raw_expected,
        'nt' if t.host['policy']['host'].startswith('Windows') else 'posix')
    same(t.raw(generator['manifest_newline_transport']['frozen_newline_projection']), projected, 'exact generated manifest newline transport')
    expected_files = {'verification.json', 'generate_fixtures.py', 'suite-intent.json', 'suite-intent.frozen-newline.json'}
    for row in intent['cases']:
        path = _child(root, row['path'])
        same(t.named(path), {'path': path, 'bytes': row['bytes'], 'sha256': row['sha256']}, 'actual complete generated fixture')
        t.raw(path); expected_files.add(row['path'])
    generator_source = helpers['predecessors'].authority_path(contracts, '/typed_project_unit1_compatibility/generate_fixtures.py')
    same(t.raw(_child(root, 'generate_fixtures.py')), generator_source.read_bytes(), 'unchanged generated source helper')
    same({p.replace('\\', '/') for p in _relative_files(t, root)}, expected_files, 'exact original generator output closure')


def _runtime_tool_operations(t, document, operations, machine_api):
    same(document['invocation'], t.host['invocation'], 'runtime setup original invocation')
    same(document['machine_sha256'], sha(canonical(t.host['machine'])), 'runtime setup original captured machine')
    need(document['execution_qualified'] is False and document['qualification_admitted'] is False and
         document['receipt_producer_authenticated'] is False and document['hosted_staging_admitted'] is False,
         'runtime setup retained nonqualifying boundary')
    _runtime_tool_environment(t, document, machine_api)
    root = document['output_root']
    same(root, _child(t.host['invocation']['run_root'], 'public-runtime/tools', document['purpose']), 'fixed runtime tool output')
    records, projected = document['probe_records'], document['probes']
    expected = []
    if document['mode'] == 'real-native':
        for name in ('clang', 'opt', 'ld.lld'):
            expected.append(([_child(document['input_snapshot']['native']['root'], name), '--version'], 'runtime-tool-version', 0))
    else:
        binary = _child(root, 'no-tool-trap.exe' if t.host['policy']['host'].startswith('Windows') else 'no-tool-trap')
        expected.append(([t.host['machine']['snapshot']['rust']['tools']['rustc']['resolved'], '--edition=2021', '-C', 'opt-level=0',
                          document['trap_build']['source']['path'], '-o', binary], 'runtime-trap-build', 0))
        for name in ('clang', 'opt', 'ld.lld'):
            expected.append(([_child(root, 'no-tool-traps', name), '--version'], 'runtime-trap-control', 121))
    need(document['started_ns'] <= document['completed_ns'] <= t.seal['started_ns'], 'runtime setup terminal window')
    need(len(records) == len(projected) == len(expected), 'exact runtime setup process catalogue')
    for row, projection, (argv, kind, status) in zip(records, projected, expected):
        _same_owned(row, operations, t)
        _same_owned(projection, operations, t, text_streams=True)
        same((row['argv'], row['operation_kind'], row['status'], row['cwd']), (argv, kind, status, root), 'runtime setup exact actual operation')
        need(document['started_ns'] <= row['started_ns'] <= row['completed_ns'] <= document['completed_ns'], 'runtime setup actual process window')
        environment = dict(document['input_snapshot']['environment'])
        if kind == 'runtime-trap-control':
            environment.update(OXID_LLVM_BIN=_child(root, 'no-tool-traps'), UNIT4_TOOL_LOG=_child(root, argv[0].replace('\\', '/').rsplit('/', 1)[-1] + '-trap-control.jsonl'))
        same(row['environment'], environment, 'exact runtime setup clean process environment')
        seconds = {'runtime-tool-version': machine_api.PROBE_TIMEOUT, 'runtime-trap-build': machine_api.TRAP_BUILD_TIMEOUT,
                   'runtime-trap-control': machine_api.TRAP_CONTROL_TIMEOUT}[kind]
        need(row['timeout_seconds'] == seconds and row['stream_limit_bytes'] == machine_api.STREAM_LIMIT, 'exact runtime setup process limits')
        if kind == 'runtime-tool-version':
            marker = {'clang': 'clang version 19.1.7', 'opt': 'LLVM version 19.1.7', 'ld.lld': 'LLD 19.1.7'}[argv[0].rsplit('/', 1)[-1]]
            need(re.search(re.escape(marker) + r'(?=$|[ \(\r\n])', projection['stdout']) and projection['stderr'] == '', 'actual exact LLVM version marker')
    if document['mode'] == 'real-native':
        wanted_files = {'tool-wrappers/' + name for name in ('clang', 'opt', 'ld.lld')}
    else:
        wanted_files = {'no-tool-trap.exe' if t.host['policy']['host'].startswith('Windows') else 'no-tool-trap', 'no-tool-trap-build.json'}
        wanted_files |= {'no-tool-traps/' + name for name in ('clang', 'opt', 'ld.lld')}
        wanted_files |= {name + '-trap-control.jsonl' for name in ('clang', 'opt', 'ld.lld')}
        if t.host['policy']['host'].startswith('Windows'): wanted_files |= {'no-tool-traps/' + name + '.exe' for name in ('clang', 'opt', 'ld.lld')}
    files = {r['path']: r for r in document['output_snapshot']['rows'] if r['kind'] == 'file'}
    same(set(files), wanted_files, 'prescribed runtime wrapper/trap output catalogue')
    actual = _relative_files(t, root)
    same({k.replace('\\', '/') for k in actual}, set(files), 'runtime tools exact raw output closure')
    same(set(_relative_directories(t, root)), {r['path'].replace('\\', '/') for r in document['output_snapshot']['rows'] if r['kind'] == 'directory' and r['path'] != '.'},
         'runtime tools exact directory closure')
    for name, row in files.items():
        _body_identity(t.named(_child(root, name)), row['route']['body'], 'actual runtime wrapper/trap bytes')


def _preparation_machine_operations(t, api, operations):
    preparation = t.host['preparation']
    config, provenance = preparation['config'], preparation['provenance']
    worker = provenance['operation']
    _same_owned(worker, operations, t)
    need(worker['status'] == 0, 'original source preparation process failed')
    expected = provenance['expected']
    root = t.host['invocation']['run_root']
    same(config['run_root'], root, 'actual preparation owned root')
    same(config['current_repo'], t.host['ci_context']['repository_path'], 'actual preparation current checkout')
    same(config['scope'], t.host['policy']['scope'], 'actual complete preparation scope')
    same(config['python'], t.host['ci_context']['host']['executable'], 'actual preparation Python')
    same(provenance['config_sha256'], sha(canonical(config)), 'preparation complete config identity')
    same(provenance['plan_sha256'], sha(canonical(preparation['plan'])), 'preparation complete plan identity')
    same(provenance['report'], t.host['source_context']['preparation']['report'], 'preparation original report identity')
    helper = _child(config['current_repo'], 'tests/qualification/package_dependency_unit4_current/prepare_worker.py')
    argv = [config['python'], '-B', helper, '--config', _child(root, 'preparation-config.json')]
    if t.host['policy']['host'] == 'Linux x86_64': argv[2:2] = ['-c', api['preparation']._LIMIT_LAUNCHER]
    same(worker['argv'], argv, 'exact original preparation worker command')
    env = {'PATH': expected['git_environment']['PATH'], 'HOME': _child(root, 'home'),
           'TMPDIR': _child(root, 'tmp'), 'TEMP': _child(root, 'tmp'), 'TMP': _child(root, 'tmp'),
           'PYTHONDONTWRITEBYTECODE': '1', 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8'}
    if t.host['policy']['host'].startswith('Windows'):
        env.update({k: expected['git_environment'][k] for k in ('SYSTEMROOT', 'WINDIR')})
    same(worker['environment'], env, 'exact original preparation worker environment')
    need(worker['cwd'] == root and worker['operation_kind'] == 'probe' and worker['timeout_seconds'] == 180 and
         worker['stream_limit_bytes'] == 8 * 1024 ** 2 and t.raw(worker['stderr']) == b'', 'exact successful preparation worker bounds')
    report = t.json(provenance['report'])
    same(t.json(worker['stdout']), {k: report[k] for k in ('schema', 'status', 'execution_qualified')} | {'report': provenance['report']['path']},
         'actual preparation worker stdout report')
    git = expected['git']
    null = 'nul' if t.host['policy']['host'].startswith('Windows') else '/dev/null'
    wanted = []
    for before in (True, False):
        for repo in config['repositories']:
            for field, tail in (('head', ['rev-parse', 'HEAD']), ('tree', ['rev-parse', 'HEAD^{tree}']), ('index', ['ls-files', '--stage', '-z'])):
                argv = [git, '--no-optional-locks', '-c', 'core.hooksPath=' + null, '-c', 'core.fsmonitor=false', '-C', repo['path'], *tail]
                wanted.append((before, argv, repo['path'], field, repo['identity'][field]))
        if config['scope'] != 'public':
            argv = [git, '--no-optional-locks', '-C', config['historical_repo'], 'ls-files', '--stage', '-z']
            wanted.append((before, argv, config['historical_repo'], 'historical-index', config['historical_selected']))
    for before, argv, cwd, field, value in wanted:
        candidates = [r for k, r in operations.items() if k not in t.used_operations and r['argv'] == argv and r['cwd'] == cwd and
                      (r['completed_ns'] <= worker['started_ns'] if before else r['started_ns'] >= worker['completed_ns'])]
        need(len(candidates) == 1, 'exact source-preparation Git operation occurrence')
        row = candidates[0]
        _same_owned(row, operations, t)
        same(row['environment'], expected['git_environment'], 'original preparation Git environment')
        need(row['status'] == 0 and row['operation_kind'] == 'probe' and row['timeout_seconds'] == 10 and row['stream_limit_bytes'] == 1024 ** 2 and
             t.raw(row['stderr']) == b'', 'actual bounded preparation Git success')
        raw = t.raw(row['stdout'])
        if field in ('head', 'tree'): same(raw.decode().strip(), value, 'actual original repository Git identity')
        elif field == 'index': same(sha(raw), value, 'actual original repository index identity')
        else:
            records = [x for x in raw.split(b'\0') if x]
            need(len(records) == 800, 'complete actual historical index')
            names = [r.split(b'\t', 1)[1].decode() for r in records]
            need(len(set(names)) == 800 and all(name in names for name in value) and len(value) == 283,
                 'actual historical selected index membership')
    machine = t.host['machine']
    snapshot = machine['snapshot']
    records, projected = machine['probe_records'], machine['probes']
    commands = []
    if machine['tool_discovery'] == 'captured-rustup-resolver':
        commands += [[snapshot['resolver']['resolved'], 'which', '--toolchain', '1.99.0', name] for name in ('cargo', 'rustc')]
    else: same(machine['tool_discovery'], 'producer-direct', 'closed actual Rust discovery route')
    commands += [[snapshot['rust']['tools']['rustc']['resolved'], '-Vv'], [snapshot['rust']['tools']['cargo']['resolved'], '-Vv'],
                 [snapshot['rust']['tools']['rustc']['resolved'], '--print', 'sysroot']]
    if t.host['policy']['host'].startswith('Windows'):
        commands.append([snapshot['native']['tools']['linker']['resolved'], '/?'])
    else:
        commands += [[snapshot['native']['tools'][name]['resolved'], '--version'] for name in ('cc', 'cxx', 'ar')]
    need(len(records) == len(projected) == len(commands), 'exact actual machine probe catalogue')
    for row, projection, argv in zip(records, projected, commands):
        _same_owned(row, operations, t)
        same(row['argv'], argv, 'actual machine probe command')
        environment = machine['resolver_snapshot']['environment'] if argv[1:3] == ['which', '--toolchain'] else snapshot['environment']
        same(row['environment'], environment, 'actual machine probe original clean environment')
        need(row['timeout_seconds'] == api['machine'].PROBE_TIMEOUT and row['stream_limit_bytes'] == api['machine'].STREAM_LIMIT,
             'actual machine probe fixed limits')
        expected_projection = copy.deepcopy(row)
        for stream in ('stdout', 'stderr'): expected_projection[stream] = t.raw(row[stream]).decode()
        same(projection, expected_projection, 'actual machine raw stream projection')
        need(row['operation_kind'] == 'probe' and row['cwd'] == config['current_repo'], 'machine captured probe route')
        if argv[-1] == '/?':
            api['machine']._windows_help(projection)
        else:
            need(row['status'] == 0 and projection['stderr'] == '' and projection['stdout'], 'machine probe actual success')
        need(machine['started_ns'] <= row['started_ns'] <= row['completed_ns'] <= machine['completed_ns'], 'machine actual capture window')


def _parser_process_catalogue(t, old, recipe, builds, operations, derived):
    data = t.host['parser']
    expected, order = {}, []
    for profile in PROFILES:
        for kind in ('passivity', 'collection'):
            envelope = t.json(_child(data['root'], ('passivity-' if kind == 'passivity' else 'collect-') + profile,
                                     'package-source-' + kind + '.json'))
            process = envelope['owned_process']; key = (process['pid'], process['started_ns'])
            expected[key] = process; order.append(process)
    controls = t.json(t.json(data['report']['comparison'])['u8_policy_controls'])
    for row in controls['cases']:
        projected = t.json(row['invocation']); process = projected['owned_process']
        _same_owned(process, operations, t)
        same((projected['argv'], projected['cwd'], projected['environment'], projected['exit_code']),
             (process['argv'], process['cwd'], process['environment'], process['status']), 'actual u8 process projection')
        need(process['status'] == 0 and process['operation_kind'] == 'parser-u8' and
             process['timeout_seconds'] == 120 and process['stream_limit_bytes'] == 8 * 1024 ** 2, 'actual u8 operation success/limits')
        for stream in ('stdout', 'stderr'): same(t.raw(projected[stream]), t.raw(process[stream]), 'actual u8 stream projection')
        key = (process['pid'], process['started_ns']); need(key not in expected, 'reused actual u8 process'); expected[key] = process; order.append(process)
    driver = old.phase_module(recipe, controls=True)
    for probe, profile, role in driver.roster():
        _evidence().tick()
        directory = _child(data['root'], driver.process_dir(dict(probe=probe, profile=profile, role=role)))
        request = t.json(_child(directory, 'request.json'))
        receipt = t.json(_child(directory, 'receipt.json'))
        same(request['expected'], driver.expected(probe, profile, role), 'command-derived actual phase expected status')
        argv, cwd, env = driver.inputs(data['root'], request)
        matches = [r for r in data['processes'] if r['argv'] == argv and r['cwd'] == cwd and r['environment'] == env and
                   r['started_ns'] == receipt['started_ns'] and r['completed_ns'] == receipt['finished_ns']]
        need(len(matches) == 1, 'unique original owned phase process')
        process = matches[0]
        _same_owned(process, operations, t)
        same(process['status'], driver.expected(probe, profile, role)['exit_code'], 'actual phase command-specific exit')
        need(process['operation_kind'] == 'parser-phase' and process['timeout_seconds'] == 30 and process['stream_limit_bytes'] == 4096,
             'phase actual operation kind/fixed limits')
        for stream in ('stdout', 'stderr'): same(t.raw(receipt[stream]), t.raw(process[stream]), 'phase actual raw stream projection')
        key = (process['pid'], process['started_ns']); need(key not in expected, 'reused original phase process'); expected[key] = process; order.append(process)
    need(len(data['processes']) == len(expected), 'no duplicate parser process rows')
    same({(r['pid'], r['started_ns']): r for r in data['processes']}, expected, 'complete exact parser owned-process roster')
    previous = max(v[1]['helper_probe']['completed_ns'] for v in builds.values())
    for row in order:
        need(previous <= row['started_ns'] <= row['completed_ns'] <= t.seal['started_ns'], 'actual complete parser stage/process order')
        previous = row['completed_ns']


def _hosted_raw_closure(t, api, contracts, data, configs, processes):
    hosted, public = api['hosted'], t.host['public']
    ledger, report = data['ledger'], data['report']
    rows = api['public_runtime'].capability_rows(contracts)
    sources = {row['key']: contracts.source_bytes(contracts.cases[row['case_id']]) for values in rows.values() for row in values}
    same(ledger['capability_rows'], rows, 'original readonly exact capability roster')
    same(ledger['capability_sources_hex'], {key: {name: raw.hex() for name, raw in values.items()} for key, values in sources.items()},
         'readonly original capability source bytes')
    same(ledger['cfgs'], configs, 'original readonly exact live compiler configurations')
    expected_outer = {**public['execution_context'], 'hosted_transport': {
        'readonly_ledger': data['ledger_binding'], 'logical_builds': {role: cfg['_sha256'] for role, cfg in configs.items()},
        'identity': report['identity'], 'qualification_claim': False}}
    same(report['execution_context'], expected_outer, 'hosted complete source/build/session context')
    same(report['raw_result']['execution_context'], expected_outer, 'hosted raw shard complete context')
    for role, cfg in report['cfgs'].items():
        same(cfg['_execution_context'], {**expected_outer, 'role': role}, 'hosted observed exact runtime context')
        same(cfg['_consumer'], configs[role]['_consumer'], 'hosted exact original consumer')
    hosted.verify_output_roster(report['output_inventory'], rows, sources)
    files = {r['path']: r for r in report['output_inventory'] if r['kind'] == 'file'}
    actual = {p.replace('\\', '/') for p in _relative_files(t, ledger['output'])}
    same(actual, set(files) | {'shard.json'}, 'complete original hosted output membership')
    for name, row in files.items(): _body_identity(t.named(_child(ledger['output'], name)), row, 'actual hosted raw body identity')
    same(t.json(_child(ledger['output'], 'tools.json')), report['tools'], 'actual hosted trap document')
    environment = hosted._expected_child_environment(hosted.outer_environment(Path(ledger['output'])), ledger['output'])
    same(report['environment'], environment, 'actual exact clean child environment')
    inputs = {'output': ledger['output'], 'environment': environment, 'capability_rows': rows,
              'cfgs': report['cfgs'], 'helpers': {'runtime': api['public_runtime'].rt},
              'adapter_root': str(hosted.namespace_path(ledger['root'],
                  _child(t.host['ci_context']['repository_path'], 'tests/qualification/unit4_public_v3')))}
    commands = hosted._worker_operations(inputs)
    previous = data['worker_process']['started_ns']
    for command, process in zip(commands, report['processes']):
        same({k: process[k] for k in ('argv', 'cwd', 'environment')},
             {k: command[k] for k in ('argv', 'cwd', 'environment')}, 'actual ordered child command')
        need(process['requested_timeout_seconds'] == command['timeout_seconds'] and
             0 < process['timeout_seconds'] <= command['timeout_seconds'] and
             previous <= process['started_ns'], 'actual bounded ordered child operation')
        if command['kind'] == 'trap-control': same(process['status'], 121, 'actual child trap status')
        previous = process['completed_ns']
    used = set()
    for section in ('public', 'lifecycle'):
        values = report['raw_result']['sections'][section]
        same([r['key'] for r in values], [r['key'] for r in rows[section]], 'actual ordered twelve child rows')
        for index, (row, got) in enumerate(zip(rows[section], values)):
            case = _child(ledger['output'], section, str(index))
            same(t.json(_child(case, 'receipt.json')), got, 'actual indexed child raw receipt')
            observations = [('ordinary', got)] if section == 'public' else [
                ('ordinary', report['raw_result']['ordinary_counterparts'][row['key']]), ('observer', got)]
            for role, observation in observations:
                path = _child(case, role, 'receipt.json')
                same(t.json(path), observation, 'actual child ordinary/observer receipt')
                for name, raw in sources[row['key']].items():
                    same(t.raw(_child(case, role, 'fixture', name)), raw, 'actual child copied fixture bytes')
                hosted.require_denial_proof(observation, report['identity'])
                key = observation['process']['pid'], observation['process']['started_ns']
                need(key not in used and processes[key] == observation['process'], 'actual unique child compiler process')
                used.add(key)
    need(len(used) == 18 and set(processes) - used == {(p['pid'], p['started_ns']) for p in report['processes'][:3]},
         'all eighteen actual child compiler observations plus exact three trap controls')
    _readonly_members(t, api, contracts, data, configs)


def _readonly_members(t, api, contracts, data, configs):
    """Derive the original logical readonly copy roster without reading old paths."""
    expected = {}
    def add(row, role):
        row = _identity(row)
        entry = expected.setdefault(row['path'], {'identity': row, 'roles': set()})
        same(entry['identity'], row, 'readonly duplicated logical body differs')
        entry['roles'].add(role)
    q = _common(); local_repo = Path(q.verify_join_context(t.expected_context)['repository_path'])
    original_repo = t.host['ci_context']['repository_path']
    def installed(path, role):
        original = _child(original_repo, Path(path).relative_to(local_repo).as_posix())
        add(dict(q.identity(path), path=original), role)
    for row in api['public_runtime'].retained_helper_inputs(local_repo): installed(Path(row['path']), 'retained-helper')
    for name in api['hosted'].RETAINED_HOSTED:
        installed(local_repo / 'tests/qualification/unit4_hosted_capability' / name, 'retained-hosted-authority')
    consumer_root = local_repo / 'tests/qualification/package_dependency_unit4_public_v1'
    package = q.decode(q.read(consumer_root / 'package-manifest.json'))
    installed(consumer_root / 'package-manifest.json', 'current-consumer-manifest')
    for row in package['files']: installed(consumer_root / row['path'], 'current-consumer')
    for role, cfg in configs.items():
        source = cfg['build_binding']['source_binding']
        for row in source['view']['files']: add(dict(row, path=_child(source['view']['path'], row['path'])), role + '-source')
        add(t.named(_child(source['view']['path'], 'package-preparation-overlay.json')), role + '-overlay')
        for key in ('plan', 'source_carrier', 'machine_carrier'): add(cfg['build_binding'][key], role + '-' + key)
        add(t.host['public']['builds'][role]['binding'], role + '-build-binding')
        for profile, binary in cfg['binaries'].items(): add(binary, role + '-binary-' + profile)
        row = source['source_manifest']; path = row['path']
        if not (path.startswith('/') or re.match('[A-Za-z]:', path)): path = _child(original_repo, path)
        add(dict(row, path=path), role + '-source-manifest')
    public = t.host['public']
    root = public['input_paths']['contract_root']
    add(t.named(_child(root, 'PUBLIC-FREEZE-v3.json')), 'contract-freeze')
    for row in public['contracts']['verified']: add(row, 'contract-authority')
    for case in contracts.cases.values():
        for row in case['source_files']: add(t.named(_child(root, case['source_root'], row['path'])), 'contract-fixture')
    for name in ('public-cli-contract-replacement-v3.json', 'original-compatibility-contract-replacement-v3.json.gz',
                 'predecessor-public-projections-replacement-v3.json.gz', 'lifecycle-contract-replacement-v3.json.gz',
                 'output-guards-contract-replacement-v3.json', 'authority-bindings-replacement-v3.json',
                 'pilot-semantic-derivations-v3.json'):
        add(t.named(_child(root, name)), 'contract-document')
    for key in ('source', 'binary'): add(data['ledger']['trap_build'][key], 'capability-trap-' + key)
    actual = {r['logical_path']: r for r in data['ledger']['entries']}
    same(set(actual), set(expected), 'exact authenticated readonly logical input membership')
    for name, value in expected.items():
        row = actual[name]
        _body_identity(row, value['identity'], 'readonly actual copied input bytes')
        same(row['roles'], sorted(value['roles']), 'readonly exact input role catalogue')


def _remote_path(value):
    text = str(value)
    return PureWindowsPath(text) if re.match(r'^[A-Za-z]:[\\/]', text) else PurePosixPath(text)


def _transport_function(owner, name, namespace):
    """Compile one unchanged authenticated pure function in a new I/O namespace."""
    raw = Path(owner.__file__).read_bytes()
    same(sha(raw), owner.__source_sha256__, 'transport predicate source body changed')
    tree = ast.parse(raw)
    nodes = [n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name]
    need(len(nodes) == 1, 'unique named transport predicate')
    node = nodes[0]
    source = b''.join(raw.splitlines(keepends=True)[node.lineno - 1:node.end_lineno]) + b'\n'
    isolated = dict(namespace)
    exec(compile(source, owner.__file__ + ':read-only-transport:' + name, 'exec'), isolated)
    return isolated[name]


def _transport_case_roster(runtime, transport):
    if not hasattr(transport, '_case_roster'):
        transport._case_roster = _transport_function(runtime, '_case_evidence_roster',
            {'load': lambda path: transport.json(str(path)), '_row_sources': runtime._row_sources, 'Path': _remote_path, 'need': need})
    return transport._case_roster



def _relative_directories(t, root):
    parent = _evidence().original_path(root)
    return {str(_evidence().original_path(path).relative_to(parent)).replace('\\', '/')
            for path in t.seal['inventory']['directories'] if parent in _evidence().original_path(path).parents}


def _parser_inputs(t, api):
    document, machine = t.host['parser']['runtime_inputs'], t.host['machine']
    same(set(document), {'schema', 'scope', 'execution_qualified', 'binary_or_case_admission',
         'runtime_native_admitted', 'native_llvm_qualified', 'hosted_staging_admitted', 'hermetic',
         'invocation', 'machine_sha256', 'machine_consumer', 'environment', 'loader_scope', 'limitations'},
         'exact original parser runtime input carrier')
    same(document['schema'], 'oxid-package-unit4-parser-runtime-inputs-v1', 'parser runtime input schema')
    same(document['scope'], 'recorded-local-parser-runtime-environment-only', 'original parser runtime scope')
    for field in ('execution_qualified', 'binary_or_case_admission', 'runtime_native_admitted',
                  'native_llvm_qualified', 'hosted_staging_admitted', 'hermetic'):
        need(document[field] is False, 'original parser runtime claim cannot be promoted')
    same(document['invocation'], t.host['invocation'], 'actual parser runtime invocation')
    same(document['machine_sha256'], sha(canonical(machine)), 'actual parser runtime machine')
    same(document['environment'], machine['snapshot']['environment'], 'actual captured parser clean environment')
    owner = api['machine']
    _body_identity(document['machine_consumer'], _common().identity(Path(owner.__file__)), 'actual authenticated machine consumer')
    same(document['machine_consumer']['path'], _child(t.host['ci_context']['repository_path'],
         'tests/qualification/package_dependency_unit4_machine_v1/machine.py'), 'fixed original parser machine owner')
    same(document['loader_scope'], 'Previously captured local Linux loader, system libraries, cache/config and Rust inputs.',
         'original parser loader boundary')
    same(document['limitations'], ['No new ELF dependency-resolution or hermetic loader claim is established.',
         'Only the parser consumer can bind the four live BuildResults and existing cases.'], 'original parser runtime limitations')


def _parser_runtime_closure(t, runtime, old, recipe, contract):
    # This original function is purely a recipe-to-members calculation.
    names, directories = runtime._full_roster({'old': old, 'recipe': recipe, 'contract': contract})
    root, sealed = t.host['parser']['root'], t.host['parser']['sealed_runtime']
    same([row['path'] for row in sealed], [_child(root, name) for name in sorted(names)],
         'all and only original sealed parser runtime identities')
    for row in sealed: same(t.named(row['path']), row, 'actual sealed parser raw identity')
    tops = {PurePosixPath(name).parts[0] for name in names}
    actual = {name.replace('\\', '/') for name in _relative_files(t, root)
              if PurePosixPath(name.replace('\\', '/')).parts[0] in tops}
    same(actual, names, 'complete original parser raw file closure')
    actual_dirs = {name for name in _relative_directories(t, root) if PurePosixPath(name).parts[0] in tops}
    same(actual_dirs, directories, 'complete original parser raw directory closure')
    for row in t.seal['inventory']['links']:
        parent = _evidence().original_path(root)
        path = _evidence().original_path(row['path'])
        if parent in path.parents:
            need(PurePosixPath(path.relative_to(parent).as_posix()).parts[0] not in tops,
                 'parser runtime raw evidence cannot contain symlinks')


def _staging_process_limits(t, staging, data, row, name):
    seconds = staging.STAGER_SECONDS if name == 'stage-installed-llvm' else staging.READ_SECONDS
    same(row['policy'], staging._process_policy(seconds), 'original exact staging process resource policy')
    need(row['completed_ns'] - row['started_ns'] <= (seconds + 10) * 10 ** 9 and
         set(row['observed_peaks']) == {'processes', 'rss_bytes', 'output_bytes'} and
         all(type(value) is int and value >= 0 for value in row['observed_peaks'].values()) and
         row['observed_peaks']['processes'] <= staging.PROCESS_LIMIT and
         row['observed_peaks']['rss_bytes'] <= staging.RSS_LIMIT and
         row['observed_peaks']['output_bytes'] <= staging.OUTPUT_LIMIT, 'actual bounded staging process peaks/duration')
    for stream in ('stdout', 'stderr'):
        same(row[stream]['path'], _child(data['root'], 'commands', name, stream), 'fixed actual staging stream route')
        need(len(t.raw(row[stream])) <= staging.STREAM_LIMIT, 'bounded actual staging stream')


def _staging_closure(t, staging, data):
    need(t.host['invocation']['started_ns'] <= data['started_ns'] < data['completed_ns'] <= t.seal['started_ns'] and
         data['completed_ns'] - data['started_ns'] <= staging.STAGE_SECONDS * 10 ** 9, 'staging terminal phase window')
    dependencies = data['dependencies']
    same(set(dependencies), {'stager', 'common', 'process_tree', 'python', 'dpkg_query', 'readelf', 'dpkg_metadata'},
         'complete original staging dependency carrier')
    same(dependencies['python'], t.host['machine']['snapshot']['host']['python'], 'actual original staging Python route')
    for field, path in (('dpkg_query', '/usr/bin/dpkg-query'), ('readelf', '/usr/bin/readelf')):
        same(dependencies[field]['selected'], path, 'fixed actual staging tool route')
    wanted = ['/var/lib/dpkg/status']
    for selector in sorted(staging.PACKAGES.values()):
        wanted.extend('/var/lib/dpkg/info/' + selector + suffix for suffix in ('.list', '.md5sums'))
    same([row['path'] for row in dependencies['dpkg_metadata']], wanted, 'exact original dpkg metadata catalogue')
    for row in dependencies['dpkg_metadata']: t.raw(row)
    source = data['source_libraries']
    same(set(source), {*staging.FILES, staging.ALIAS}, 'exact original installed library roster')
    for name, row in staging.FILES.items():
        same(source[name]['path'], str(staging.SOURCE_ROOT / name), 'fixed installed library route')
        _body_identity(source[name], row, 'exact original installed library body')
    alias = source[staging.ALIAS]
    need(set(alias) == {'path', 'kind', 'link', 'device', 'inode', 'mode'} and
         alias['path'] == str(staging.SOURCE_ROOT / staging.ALIAS) and alias['kind'] == 'symlink' and
         alias['link'] == staging.ALIAS_TARGET and stat.S_ISLNK(alias['mode']), 'exact original installed LLVM alias')
    # Replay the unchanged original receipt predicate with the producer Python
    # version; join's interpreter is deliberately not the producer's identity.
    check = _transport_function(staging, '_receipt', dict(need=need, SOURCE_ROOT=staging.SOURCE_ROOT,
        _expected_packages=staging._expected_packages, FILES=staging.FILES, ALIAS=staging.ALIAS,
        ALIAS_TARGET=staging.ALIAS_TARGET, PACKAGE_VERSION=staging.PACKAGE_VERSION,
        hashlib=hashlib, json=json, sys=types.SimpleNamespace(version=t.host['ci_context']['host']['python'])))
    check(None, data['original_receipt'], dependencies, _remote_path(data['root']))
    stager = next(row for row in data['operations'] if row['name'] == 'stage-installed-llvm')
    same(t.json(stager['stdout']), {'status': 'verified', 'library_directory': data['library_root'],
         'content_sha256': data['original_receipt']['content_sha256']}, 'actual original stager stdout')
    staging._closed_output(data['output_manifest'], data['operations'])
    files = {row['path']: row for row in data['output_manifest'] if row['kind'] == 'file'}
    same(set(_relative_files(t, data['root'])), set(files), 'exact actual staged file catalogue')
    same(_relative_directories(t, data['root']), {row['path'] for row in data['output_manifest'] if row['kind'] == 'directory'},
         'exact actual staged directory catalogue')
    for name, row in files.items(): _body_identity(t.named(_child(data['root'], name)), row, 'actual staged output bytes')
    parent = _evidence().original_path(data['root'])
    links = [row for row in t.seal['inventory']['links'] if parent in _evidence().original_path(row['path']).parents]
    same(links, [{'path': _child(data['library_root'], staging.ALIAS), 'target': staging.ALIAS_TARGET,
         'resolved': _child(data['library_root'], staging.ALIAS_TARGET), 'role': 'staged-llvm-alias'}], 'exact actual staged alias route')


def _runtime_tool_environment(t, document, machine_api):
    snapshot = t.host['machine']['snapshot']
    inputs = document['input_snapshot']
    env = dict(snapshot['environment'])
    need(not any(key in env for key in ('LD_PRELOAD', 'DYLD_INSERT_LIBRARIES', 'PYTHONPATH',
         'UNIT4_TOOL_LOG', 'UNIT4_TOOL_EVIDENCE', 'UNIT4_RACE_OUTPUT', 'UNIT4_LIFECYCLE_LOG')),
         'unadmitted inherited runtime environment override')
    root = _child(t.host['ci_context']['repository_path'], machine_api.PUBLIC_RUNTIME_PATH)
    same(inputs['helpers']['root'], root, 'fixed original runtime helper root')
    same(set(inputs['helpers']['files']), set(machine_api.RUNTIME_HELPERS), 'fixed runtime helper roster')
    for name, digest in machine_api.RUNTIME_HELPERS.items():
        row = inputs['helpers']['files'][name]
        same((row['path'], row['sha256']), (_child(root, name), digest), 'fixed runtime helper original body')
    env['UNIT4_ADAPTER_ROOT'] = root
    native = document['purpose'] == 'main' and t.host['policy']['host'] == 'Linux x86_64'
    same(inputs['mode'], 'real-native' if native else 'no-native-traps', 'host/purpose-derived runtime mode')
    same(document['mode'], inputs['mode'], 'actual runtime mode')
    same(document['host'], t.host['policy']['host'], 'actual runtime tools host')
    if native:
        need(inputs['native'] is not None, 'actual Linux native input capture required')
        interpreter = snapshot['host']['python']
        env['PATH'] = str(_remote_path(interpreter['selected']).parent) + ':' + env['PATH']
        same(inputs['native']['wrapper_python']['resolved'], interpreter['resolved'], 'original wrapper Python selection')
        env.update(UNIT4_REAL_LLVM=inputs['native']['root'],
                   LD_LIBRARY_PATH=t.host['staging']['library_root'])
    else: same(inputs['native'], None, 'no native overrides in original trap environment')
    same(inputs['environment'], env, 'independently reconstructed runtime input environment')
    wrappers = _child(document['output_root'], 'tool-wrappers' if native else 'no-tool-traps')
    env['OXID_LLVM_BIN'] = wrappers
    same(document['environment'], env, 'independently reconstructed final runtime environment')


def _public_process_environment(t, got, expected_case, row):
    need(type(expected_case) is dict and set(expected_case) == {'section', 'index', 'variant'} and row is not None,
         'independent indexed public tuple required')
    section, index, variant = (expected_case[k] for k in ('section', 'index', 'variant'))
    need(section in SECTIONS and type(index) is int and index >= 0 and
         variant in ('ordinary', 'observer') and (variant == 'ordinary' or section in ('lifecycle', 'guards')),
         'exact independently prescribed public role')
    prefix = ('%03d' if section == 'guards' else '%05d') % index
    directory = _child(t.host['public']['collection_root'], section, prefix, variant)
    env = dict(t.host['public']['runtime_tools']['environment'])
    env.update(UNIT4_TOOL_LOG=_child(directory, 'tools.jsonl'), UNIT4_TOOL_EVIDENCE=directory,
               UNIT4_ADAPTER_ROOT=_child(t.host['ci_context']['repository_path'], 'tests/qualification/unit4_public_v3'))
    if variant == 'observer': env['UNIT4_LIFECYCLE_LOG'] = _child(directory, 'lifecycle.jsonl')
    if section == 'guards' and row['group'] == 'race':
        env['UNIT4_RACE_OUTPUT'] = _child(t.host['public']['collection_root'], section, prefix,
                                         'fixtures', row['case']['source_case_id'], 'out.bin')
    same(got['process']['runtime_case'], expected_case, 'original process exact frozen case/variant')
    same(got['process']['environment'], env, 'independent original compiler environment')
    if got.get('native_execution'):
        native = got['native_execution']; process = native['process']
        same(process['runtime_case'], expected_case, 'source-free native exact compile tuple association')
        same(process['environment'], env, 'source-free native environment equals its actual compiler environment')
        same(process['argv'], [_child(directory, 'native-source-free/program')], 'source-free native exact retained executable')
        same(process['cwd'], _child(directory, 'native-source-free'), 'source-free native exact retained cwd')
        need(got['process']['completed_ns'] <= process['started_ns'], 'native execution must follow associated compiler')


def _operation_policies(t, api, operations):
    preparation = api['preparation']
    transient = _transport_function(preparation, '_transient_target', {'Path': _remote_path, 'need': need})
    worker = t.host['preparation']['provenance']['operation']
    worker_key = worker['pid'], worker['started_ns']
    for key, row in operations.items():
        _evidence().tick()
        kind = row['operation_kind']; is_prepare = key == worker_key
        compiled = kind in ('public-build', 'parser-build', 'parser-helper', 'runtime-trap-build')
        registration = row['native_workspace']
        if registration is not None:
            need(not is_prepare and kind == 'public-runtime' and t.host['policy']['host'] == 'Linux x86_64' and
                 registration['source']['sha256'] == preparation.NATIVE_SOURCE_SHA and registration['pid'] == row['pid'] and
                 registration['invocation_nonce'] == t.host['invocation']['nonce'], 'fixed native workspace resource authority')
        target = transient(kind, row['argv'], row['cwd'], row['environment'], t.host['invocation']['run_root'])
        policy = {'processes': 5 if registration is not None else (16 if compiled and not is_prepare else 4),
                  'rss_bytes': (8 if compiled and not is_prepare else 2) * 1024 ** 3,
                  'output_bytes': (128 * 1024 ** 2 if is_prepare else 6 * 1024 ** 3),
                  'file_bytes': (64 * 1024 ** 2 if is_prepare else 2 * 1024 ** 3),
                  'process_memory_enforced_on': 'linux',
                  'all_hosts': ['wall', 'stream', 'output_file', 'output_total', 'owned_tree_cleanup'],
                  'transient_roots': [] if target is None else [str(target)]}
        monitored = kind in ('public-runtime', 'public-native')
        if monitored: policy['output_measurement'] = 'sampled ledger; see public_output_ledger, no continuous output peak'
        same(row['operation_policy'], policy, 'exact original owned operation resource policy')
        prescribed = ({'wall_seconds': row['timeout_seconds'], 'stream_bytes': row['stream_limit_bytes'],
            'output_bytes': 128 * 1024 ** 2, 'file_bytes': 64 * 1024 ** 2, 'linux_group_processes': 4,
            'linux_group_rss_bytes': 2 * 1024 ** 3, 'linux_child_cpu_seconds': 120,
            'linux_child_address_space_bytes': 1024 ** 3} if is_prepare else None)
        same(row['preparation_resource_policy'], prescribed, 'exact original preparation resource policy')
        peaks = row['observed_peaks']
        need(set(peaks) == {'processes', 'rss_bytes', 'output_bytes'} and
             all(type(peaks[k]) is int and 0 <= peaks[k] <= policy[k] for k in ('processes', 'rss_bytes')),
             'actual bounded owned process/resource peaks')
        need(peaks['output_bytes'] is None if monitored else type(peaks['output_bytes']) is int and
             0 <= peaks['output_bytes'] <= policy['output_bytes'], 'original output peak claim/bound')
        if monitored:
            ledger = row['public_output_ledger']
            need(ledger['schema'] == 'public-output-sampled-ledger-v1' and
                 ledger['continuous_peak_claim'] is False and ledger['body_cache_authority'] is False and
                 ledger['invocation_poisoned'] is False and ledger['max_completed_audit_gap'] <= 5 and
                 ledger['full_audit_count'] > 0 and
                 all(type(ledger[k]) is int and 0 <= ledger[k] <= policy['output_bytes'] for k in
                     ('full_observed_peak_bytes', 'active_accounted_peak_bytes', 'sealed_bytes')),
                 'original complete bounded sampled public output ledger')
        else: same(row['public_output_ledger'], None, 'nonpublic operation cannot claim sampled public ledger')
        same(row['public_output_failure_preservation'], None, 'no hidden failed public output preservation')

if __name__ == '__main__':
    raise SystemExit(main())
