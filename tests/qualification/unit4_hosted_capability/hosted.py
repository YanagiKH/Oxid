#!/usr/bin/env python3
"""Same-container, raw-receipt join for a transient non-root capability shard.

This controller never changes existing permissions, creates accounts or changes
persistent settings. All compiler and observer sources remain pinned by the
reviewed public adapter. It is separate from the immutable 14-file package.
"""
import sys
sys.dont_write_bytecode = True
import argparse
import copy
import errno
import gzip
import hashlib
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import tempfile

CAPABILITY = 'posix_regular_file_mode_denies_open_for_effective_identity'
TRANSPORT_MANIFEST_SHA = '631bd4dcc5e3c2e2887debddf8a8657af5247fb985a8d6fcc45cbf78b701c687'
TRANSPORT_HELPER_SHA = 'cbeb6634a57685f5b86f8ec3507a083fcf0d22e9afb7f828d19bf559a9e99eb3'


def modules(adapter):
    sys.path.insert(0, str(Path(adapter).resolve()))
    import contracts as c
    import runtime as r
    import run as runner
    import guards
    from predecessors import Predecessors
    runner.adapter_identity()
    return c, r, runner, guards, Predecessors


def namespace_path(root, logical, need):
    path = Path(logical)
    need(path.is_absolute() and '..' not in path.parts, 'transport requires an exact absolute logical path')
    return Path(root) / 'readonly' / str(path).lstrip('/')


def copy_input_inventory(inputs, ordinary_binding, observer_binding, c, physical):
    """Derive every input from original receipts/manifests and fixed packages."""
    from run import PACKAGE_FILES
    c.need(set(inputs) == {'adapter', 'controller', 'contract_package', 'amendment_root', 'trap_binding'}, 'readonly input roots')
    paths = {}
    def add(path, role, bound=None):
        logical = str(Path(path))
        c.need(Path(path).is_absolute() and '..' not in Path(path).parts and logical == str(path), 'noncanonical readonly input')
        target = Path(physical(logical))
        c.need(target.is_file() and not target.is_symlink(), 'missing/nonregular readonly input: ' + logical)
        if bound is not None:
            c.verify(target, bound)
        paths.setdefault(logical, set()).add(role)
        return target
    for role, cfg_binding in (('ordinary', ordinary_binding), ('observer', observer_binding)):
        cfg = c.load(add(cfg_binding['path'], role + '-config', cfg_binding))
        manifest = c.load(add(cfg['source_manifest']['path'], role + '-source-manifest', cfg['source_manifest']))
        names = [row['path'] for row in manifest['files']]
        c.need(len(names) == len(set(names)), 'duplicate original source manifest entry')
        for row in manifest['files']:
            relative = Path(row['path'])
            c.need(not relative.is_absolute() and '..' not in relative.parts, 'unsafe original source member')
            add(Path(cfg['source_root']) / relative, role + '-source', row)
        for profile, binary in cfg['binaries'].items():
            add(binary['path'], role + '-binary-' + profile, binary)
            bound = cfg['build_receipts'][profile]
            recipe = c.load(add(bound['path'], role + '-build-' + profile, bound))
            for stream in recipe['streams'].values():
                add(stream['path'], role + '-build-stream', stream)
        if 'observer_patch' in cfg:
            add(cfg['observer_patch']['path'], 'approved-observer-patch', cfg['observer_patch'])
    packages = (
        (inputs['adapter'], PACKAGE_FILES, 'closed-adapter'),
        (inputs['controller'], {'hosted.py', 'selftest.py', 'README.md'}, 'hosted-controller'),
        (inputs['contract_package'], {'README.md', 'authorities.tar.gz', 'publication-summary.json', 'test_transport.py', 'transport-manifest.json', 'transport.py'}, 'frozen-contract-package'),
    )
    for root, names, role in packages:
        for name in sorted(names):
            add(Path(root) / name, role)
    if inputs['amendment_root']:
        for name in ('AMENDMENT-CHECKPOINT-v1.json', 'README.md', 'build_amendment.py', 'receipt-identity.json', 'validation.json',
                     'artifacts/amendment.json', 'artifacts/effective-contract.json', 'artifacts/source-coordinate-proof.json'):
            add(Path(inputs['amendment_root']) / name, 'source-coordinate-amendment')
    trap = c.load(add(inputs['trap_binding'], 'prebuilt-trap-receipt'))
    add(trap['source']['path'], 'prebuilt-trap-source', trap['source'])
    add(trap['binary']['path'], 'prebuilt-trap-binary', trap['binary'])
    return paths


def prepare_copy_view(args, child_root, c, r):
    """Copy only a closed input ledger; original config/recipe bytes stay intact."""
    configs(args, c, r)
    inputs = {'adapter': str(Path(args.adapter).resolve()), 'controller': str(Path(__file__).resolve().parent),
              'contract_package': str(Path(args.contract_package).resolve()),
              'amendment_root': str(Path(args.amendment_root).resolve()) if args.amendment_root else None,
              'trap_binding': str(Path(args.trap_binding).resolve())}
    ordinary_binding, observer_binding = c.binding(args.ordinary_binding), c.binding(args.observer_binding)
    paths = copy_input_inventory(inputs, ordinary_binding, observer_binding, c, lambda path: path)
    c.need(len(paths) <= 400, 'readonly copy member bound')
    c.need(sum(Path(path).stat().st_size for path in paths) <= 512 * 1024 * 1024, 'readonly copy byte bound')
    entries = []
    for logical in sorted(paths):
        source = Path(logical)
        target = namespace_path(child_root, logical, c.need)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())
        target.chmod(0o555 if source.stat().st_mode & 0o111 else 0o444)
        original = c.binding(source)
        c.verify(target, original)
        entries.append({'logical_path': logical, 'physical_path': str(target), 'bytes': original['bytes'],
                        'sha256': original['sha256'], 'roles': sorted(paths[logical])})
    ledger_path = Path(child_root) / 'readonly-transport.json'
    ledger = {'schema': 'oxid-unit4-capability-readonly-transport-v1', 'root': str(child_root),
              'expected_head': args.expected_head, 'ordinary_original': c.binding(args.ordinary_binding),
              'observer_original': c.binding(args.observer_binding), 'inputs': inputs, 'entries': entries,
              'no_build_invoked': True, 'existing_permissions_changed': False}
    c.save(ledger_path, ledger)
    views = create_projection_views(ledger_path, c, r)
    return {'ledger': c.binding(ledger_path), 'views': views,
            'adapter': str(namespace_path(child_root, Path(args.adapter).resolve(), c.need)),
            'controller': str(namespace_path(child_root, Path(__file__).resolve(), c.need)),
            'contract_package': str(namespace_path(child_root, inputs['contract_package'], c.need)),
            'amendment_root': str(namespace_path(child_root, Path(args.amendment_root).resolve(), c.need)) if args.amendment_root else None,
            'trap_binding': str(namespace_path(child_root, Path(args.trap_binding).resolve(), c.need))}


def create_projection_views(ledger_path, c, r, verify_only=False):
    """Explicit new transport projections, with immutable original receipts bound."""
    ledger = c.load(ledger_path)
    transport_root = Path(ledger['root'])
    readonly = transport_root / 'readonly'
    c.need(transport_root.is_dir() and not transport_root.is_symlink()
           and readonly.is_dir() and not readonly.is_symlink(), 'readonly transport physical directory boundaries')
    c.need(ledger['schema'] == 'oxid-unit4-capability-readonly-transport-v1' and ledger['no_build_invoked'] is True and ledger['existing_permissions_changed'] is False, 'readonly transport schema/scope')
    entries = ledger['entries']
    c.need(len(entries) <= 400 and len({e['logical_path'] for e in entries}) == len(entries), 'readonly transport inventory')
    c.need(sum(e['bytes'] for e in entries) <= 512 * 1024 * 1024, 'readonly transport size bound')
    mapping = {e['logical_path']: e for e in entries}
    for entry in entries:
        c.need(Path(entry['physical_path']) == namespace_path(ledger['root'], entry['logical_path'], c.need), 'readonly exact path mapping')
        c.verify(entry['physical_path'], entry)
    def mapped(path):
        logical = str(Path(path))
        c.need(logical in mapping, 'unlisted readonly transport read: ' + logical)
        return mapping[logical]['physical_path']
    def rebinding(row):
        return {**row, 'path': mapped(row['path'])}
    root = Path(ledger['root']) / 'transport-views'
    if not verify_only:
        root.mkdir(exist_ok=False)
    expected = copy_input_inventory(ledger['inputs'], ledger['ordinary_original'], ledger['observer_original'], c, mapped)
    c.need(set(mapping) == set(expected), 'readonly full original input closure')
    c.need(all(entry['roles'] == sorted(expected[entry['logical_path']]) for entry in entries), 'readonly exact input roles')
    actual_files = set()
    for path in readonly.rglob('*'):
        c.need(not path.is_symlink(), 'readonly transport symlink')
        if path.is_file():
            actual_files.add(str(path))
        else:
            c.need(path.is_dir(), 'readonly transport special member')
    c.need(actual_files == {entry['physical_path'] for entry in entries}, 'readonly exact physical file membership')
    views = {}
    ledger_binding = c.binding(ledger_path)
    for role in ('ordinary', 'observer'):
        original_binding = ledger[role + '_original']
        c.verify(mapped(original_binding['path']), original_binding)
        original = c.load(mapped(original_binding['path']))
        cfg = copy.deepcopy(original)
        cfg['source_root'] = str(namespace_path(ledger['root'], original['source_root'], c.need))
        cfg['source_manifest'] = rebinding(original['source_manifest'])
        cfg['binaries'] = {profile: rebinding(value) for profile, value in original['binaries'].items()}
        cfg['build_receipts'] = {}
        for profile, recipe_binding in original['build_receipts'].items():
            c.verify(mapped(recipe_binding['path']), recipe_binding)
            recipe = c.load(mapped(recipe_binding['path']))
            view = copy.deepcopy(recipe)
            view['binary'] = cfg['binaries'][profile]
            view['streams'] = {stream: rebinding(value) for stream, value in recipe['streams'].items()}
            view['artifact_transport_projection'] = {'kind': 'build-receipt-path-view-v1', 'original_receipt': recipe_binding,
                'readonly_transport': ledger_binding, 'replaced_fields': ['binary.path'] + ['streams.' + name + '.path' for name in sorted(recipe['streams'])],
                'build_reexecuted': False, 'original_cwd_and_recipe_preserved': True}
            path = root / (role + '-' + profile + '-build-view.json')
            if verify_only:
                c.need(c.load(path) == view, 'altered build-receipt transport projection')
            else:
                c.save(path, view)
                path.chmod(0o444)
            cfg['build_receipts'][profile] = c.binding(path)
        if 'observer_patch' in cfg:
            cfg['observer_patch'] = rebinding(cfg['observer_patch'])
        cfg['artifact_transport_projection'] = {'kind': 'candidate-path-view-v1', 'original_binding': original_binding,
            'readonly_transport': ledger_binding, 'source_and_binary_bytes_unchanged': True, 'build_reexecuted': False}
        path = root / (role + '-candidate-view.json')
        if verify_only:
            c.need(c.load(path) == cfg, 'altered candidate transport projection')
        else:
            c.save(path, cfg)
            path.chmod(0o444)
        r.candidate_binding(path)  # Same reviewed semantic/source/build predicate.
        views[role] = c.binding(path)
    return views


def configs(args, c, r):
    ordinary = r.candidate_binding(args.ordinary_binding)
    observer = r.candidate_binding(args.observer_binding)
    c.need(ordinary['kind'] == 'unit4-public-v3-candidate' and observer['kind'] == 'unit4-public-v3-lifecycle-observer', 'binary roles')
    c.need(ordinary['compiler_head'] == observer['compiler_head'] == args.expected_head, 'exact checked-out head mismatch')
    c.need(ordinary['compiler_head_tree'] == observer['compiler_head_tree'], 'head full-tree mismatch')
    c.need(ordinary['source_manifest']['sha256'] == observer['base_source_manifest_sha256'], 'observer base mismatch')
    for cfg in (ordinary, observer):
        cfg['_binary_stats'] = {p: r.file_identity(b['path']) for p, b in cfg['binaries'].items()}
    return ordinary, observer


def selected_rosters(contracts):
    return {section: [row for row in contracts.roster(section) if row['scope'] == 'execute' and row['required_capabilities'] == [CAPABILITY]]
            for section in ('public', 'lifecycle')}


def prebuilt_traps(args, output, c, r):
    receipt = c.load(args.trap_binding)
    mapping = {e['logical_path']: e['physical_path'] for e in c.load(args.transport_ledger)['entries']} if args.transport_ledger else {}
    source_path = mapping.get(receipt['source']['path'], receipt['source']['path'])
    binary_path = mapping.get(receipt['binary']['path'], receipt['binary']['path'])
    c.verify(source_path, receipt['source'])
    c.verify(binary_path, receipt['binary'])
    c.need(receipt['source']['sha256'] == c.sha((Path(args.adapter) / 'no_tool_trap.rs').read_bytes()), 'prebuilt trap source identity')
    expected = ['rustc', '--edition=2021', '-C', 'opt-level=0', receipt['source']['path'], '-o', receipt['binary']['path']]
    c.need(receipt['build']['argv'] == expected and receipt['build']['status'] == 0 and not receipt['build']['timed_out'], 'prebuilt trap recipe')
    c.need(receipt['rustc']['status'] == 0 and receipt['rustc']['stdout'].startswith('rustc 1.99.0 '), 'prebuilt trap Rust identity')
    for p in (receipt['build'], receipt['rustc']):
        c.need(not p['stream_limit_exceeded'], 'prebuilt trap incomplete capture')
        for stream in ('stdout', 'stderr'):
            c.need(c.sha(p[stream].encode()) == p[stream + '_sha256'], 'prebuilt trap stream identity')
    wrappers = output / 'tool-traps'
    wrappers.mkdir()
    env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1', 'PYTHONNOUSERSITE': '1', 'OXID_LLVM_BIN': str(wrappers)}
    for key in ('UNIT4_LIFECYCLE_LOG', 'UNIT4_RACE_OUTPUT', 'LD_LIBRARY_PATH'):
        env.pop(key, None)
    tools = {}
    for name in ('clang', 'opt', 'ld.lld'):
        target = wrappers / name
        shutil.copy2(binary_path, target)
        log = output / (name + '-direct-control.jsonl')
        process = r.process([str(target), '--version'], output, {**env, 'UNIT4_TOOL_LOG': str(log)}, timeout=10)
        c.need(process['status'] == 121 and process['stdout'] == '' and process['stderr'] == 'unexpected native tool invocation\n', 'child direct trap control')
        c.need(r.read_events(log) == [{'unexpected_native_tool': True}], 'child trap logging')
        tools[name] = {'mode': 'no-native-trap', 'executable': c.binding(target), 'wrapper': c.binding(target),
                       'control': process, 'control_log': c.binding(log)}
    c.save(output / 'tools.json', {'tools': tools, 'prebuilt_receipt': c.binding(args.trap_binding), 'rust_builds_in_child': 0})
    return env, tools


def materialize_contracts(args, output, c, r):
    package = Path(args.contract_package).resolve()
    manifest_path = package / 'transport-manifest.json'
    helper = package / 'transport.py'
    c.need(c.sha(manifest_path.read_bytes()) == TRANSPORT_MANIFEST_SHA, 'transport manifest pin')
    c.need(c.sha(helper.read_bytes()) == TRANSPORT_HELPER_SHA, 'transport helper pin')
    destination = output / 'contracts'
    p = r.process([sys.executable, str(helper), 'extract', '--destination', str(destination)], output,
                  {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}, timeout=120)
    c.need(p['status'] == 0 and not p['timed_out'] and not p['stream_limit_exceeded'], 'child contract materialization')
    manifest = c.load(manifest_path)
    path_map = {row['logical_path']: str(destination / row['archive_path']) for row in manifest['members']}
    path_map_path = output / 'path-map.json'
    c.save(path_map_path, path_map)
    public = destination / 'workspace/shared/oxid-reset-recovery-20261003/new-contracts/public-v3'
    c.save(output / 'contract-materialization-process.json', p)
    return c.Contracts(public, path_map, args.amendment_root)


def install_worker_group_policy(r, need):
    """Nested commands stay in the one session/group owned by the controller."""
    need(os.name == 'posix' and os.getpid() == os.getpgrp() == os.getsid(0), 'worker must lead the controller-owned session')
    original = r.process
    def nested(*args, **kwargs):
        kwargs['new_session'] = False
        return original(*args, **kwargs)
    r.process = nested
    return {'identity': 'outer-owned-session-shared-nested-v1', 'worker_pid': os.getpid(),
            'process_group': os.getpgrp(), 'session_id': os.getsid(0), 'nested_new_sessions': False}


def worker(args):
    c, r, runner, _, _ = modules(args.adapter)
    c.need(os.name == 'posix' and os.geteuid() == args.expected_uid != 0 and os.getegid() == args.expected_gid and os.getgroups() == [] and c.host() == 'Linux x86_64', 'capability worker requires exact non-root Linux identity and cleared supplementary groups')
    group_policy = install_worker_group_policy(r, c.need)
    output = Path(args.out).resolve()
    if args.transport_ledger:
        views = create_projection_views(args.transport_ledger, c, r, verify_only=True)
        c.need(views['ordinary']['path'] == args.ordinary_binding and views['observer']['path'] == args.observer_binding, 'worker transport view roles')
    c.need(output.is_dir() and output.stat().st_uid == os.geteuid(), 'worker output must already be a fresh child-owned directory')
    c.need(list(output.iterdir()) == [], 'worker output not fresh')
    ordinary, observer = configs(args, c, r)  # Read-only probe; no chmod/fallback.
    contracts = materialize_contracts(args, output, c, r)
    rosters = selected_rosters(contracts)
    c.need({section: len(rows) for section, rows in rosters.items()} == {'public': 6, 'lifecycle': 6}, 'frozen capability shard cardinality')
    env, tools = prebuilt_traps(args, output, c, r)
    reports = {}
    for section, rows in rosters.items():
        section_out = output / section
        section_out.mkdir()
        receipts = []
        for index, row in enumerate(rows):
            source_case = contracts.cases[row['case_id']]
            sources = contracts.source_bytes(source_case)
            if section == 'lifecycle':
                row['_filesystem_setup'] = source_case['filesystem_setup']
            evidence = section_out / str(index)
            got = runner.collect(row, ordinary, evidence / 'ordinary', sources, env)
            if section == 'public':
                runner.core_compare(section, row, got, ordinary, contracts, sources, tools, None)
            else:
                base = got
                got = runner.collect(row, observer, evidence / 'observer', sources, env)
                runner.lifecycle_compare(row, base, got, ordinary, observer, contracts, sources, tools)
                c.need(base['executed'], 'ordinary capability baseline was unavailable')
                base['status'] = 'pass'
                c.save(evidence / 'ordinary' / 'receipt.json', base)
                got['ordinary_receipt'] = c.binding(evidence / 'ordinary' / 'receipt.json')
            c.need(got['executed'] and got['capability_proof']['read_open_succeeded'] is False,
                   'worker did not demonstrate actual read denial')
            c.need(got['capability_proof']['effective_uid'] == os.geteuid() and got['capability_proof']['groups'] == os.getgroups(), 'worker identity proof')
            got['status'] = 'pass'
            c.save(evidence / 'receipt.json', got)
            receipts.append(got)
        with gzip.open(section_out / 'observations.jsonl.gz', 'wt') as sink:
            for got in receipts:
                sink.write(json.dumps(got, sort_keys=True) + '\n')
        reports[section] = runner.aggregate(rows, receipts)
    configs(args, c, r)
    for tool in tools.values():
        c.verify(tool['executable']['path'], tool['executable'])
    report = {'status': 'PARTIAL_CAPABILITY_SHARD', 'complete_qualification': False, 'hosted_controller_sha256': c.sha(Path(__file__).read_bytes()),
              'effective_uid': os.geteuid(), 'effective_gid': os.getegid(), 'supplementary_groups': os.getgroups(), 'process_group_policy': group_policy,
              'expected_head': args.expected_head, 'ordinary_binding': c.binding(args.ordinary_binding),
              'observer_binding': c.binding(args.observer_binding), 'adapter': runner.adapter_identity(),
              'readonly_transport': c.binding(args.transport_ledger) if args.transport_ledger else None,
              'sections': reports, 'tuple_keys': {section: [row['key'] for row in rows] for section, rows in rosters.items()},
              'scope': 'Only the frozen actual-host capability tuples; never a full qualification result.'}
    if contracts.effective_identity:
        report.update(contracts.effective_identity)
    c.save(output / 'comparison.json', report)
    return 0


def read_rows(root, section):
    with gzip.open(root / section / 'observations.jsonl.gz', 'rt') as stream:
        return [json.loads(line) for line in stream]


def replacement_map(required_keys, candidates, need):
    """Exact replacement domain; never authorize missing or extra tuple records."""
    keys = [got['key'] for got in candidates]
    need(len(keys) == len(set(keys)), 'duplicate shard tuple')
    need(set(keys) == set(required_keys), 'missing/extra capability replacement')
    need(all(got.get('executed') is True and got.get('status') == 'pass' for got in candidates), 'nonexecuted or failed replacement')
    return {got['key']: got for got in candidates}

def require_denial_proof(receipt, identity, need):
    proof = receipt.get('capability_proof', {})
    need(proof.get('read_open_succeeded') is False
         and type(proof.get('effective_uid')) is int and proof['effective_uid'] == identity['effective_uid']
         and type(proof.get('effective_gid')) is int and proof['effective_gid'] == identity['effective_gid']
         and proof.get('groups') == [] and type(proof.get('error_errno')) is int
         and proof['error_errno'] in (errno.EACCES, errno.EPERM), 'replacement read-denial identity/errno')


def raw_join(args, child_root=None):
    c, r, runner, guards, Predecessors = modules(args.adapter)
    ordinary, observer = configs(args, c, r)
    root = Path(args.main_root).resolve()
    main = c.load(root / 'comparison.json')
    c.need(main['all_contract_sections_collected'] is True and main['status'] in ('INCOMPLETE', 'LOCAL_COMPARISONS_COMPLETE'), 'main collection incomplete for unrelated reasons')
    c.need(main['adapter'] == runner.adapter_identity(), 'main collector package identity')
    c.verify(args.ordinary_binding, main['candidate_binding'])
    integrity = c.load(root / 'input-integrity.json')
    c.verify(integrity['path_map']['path'], integrity['path_map'])
    contracts = c.Contracts(args.contracts, c.load(integrity['path_map']['path']), args.amendment_root)
    c.need(integrity.get('effective_authority') == contracts.effective_identity, 'main effective authority mismatch')
    predecessors = Predecessors(contracts, source_manifest=ordinary['source_manifest']['path'],
                                amendment_root=runner.SOURCE_BINDING_ROOT)
    main_tools_record = c.load(root / 'tools.json')
    main_tools = main_tools_record['tools']
    runtime_binding = main_tools_record['selected_runtime']
    if runtime_binding:
        c.need(r.staged_runtime(runtime_binding['receipt']['path'], runtime_binding['library_directory']) == runtime_binding, 'main runtime identity')
    for tool in main_tools.values():
        c.verify(tool['executable']['path'], tool['executable'])
        c.verify(tool['wrapper']['path'], tool['wrapper'])
    child_rows = {}
    child_ordinary, child_observer = ordinary, observer
    child_tools = None
    shard = None
    if child_root:
        child_root = Path(child_root).resolve()
        shard = c.load(child_root / 'comparison.json')
        c.need(shard['status'] == 'PARTIAL_CAPABILITY_SHARD' and shard['complete_qualification'] is False, 'child is not bounded shard')
        account = pwd.getpwnam(args.existing_user)
        c.need(shard['expected_head'] == args.expected_head and shard['effective_uid'] == account.pw_uid != 0 and shard['effective_gid'] == account.pw_gid and shard['supplementary_groups'] == [], 'child head/identity scope')
        if contracts.effective_identity:
            c.need(all(shard.get(k) == v for k, v in contracts.effective_identity.items()), 'child effective authority mismatch')
        group = shard['process_group_policy']
        controller_process = c.load(Path(args.out) / 'child-process.json')
        c.need(group['identity'] == 'outer-owned-session-shared-nested-v1' and group['nested_new_sessions'] is False, 'child nested group policy')
        c.need(group['worker_pid'] == group['process_group'] == group['session_id'] == controller_process['pid'] and controller_process['status'] == 0 and not controller_process['timed_out'] and not controller_process['stream_limit_exceeded'], 'owned child session/terminal binding')
        c.need(shard['adapter'] == runner.adapter_identity(), 'child collector package identity')
        c.need(shard['hosted_controller_sha256'] == c.sha(Path(__file__).read_bytes()), 'child controller source identity')
        if shard.get('readonly_transport'):
            transport_binding = shard['readonly_transport']
            c.verify(transport_binding['path'], transport_binding)
            ledger = c.load(transport_binding['path'])
            c.need(ledger['ordinary_original'] == c.binding(args.ordinary_binding) and ledger['observer_original'] == c.binding(args.observer_binding), 'transport original config bindings')
            c.need(ledger['expected_head'] == args.expected_head, 'transport expected head')
            views = create_projection_views(transport_binding['path'], c, r, verify_only=True)
            c.need(shard['ordinary_binding'] == views['ordinary'] and shard['observer_binding'] == views['observer'], 'shard exact projected bindings')
            child_ordinary = r.candidate_binding(views['ordinary']['path'])
            child_observer = r.candidate_binding(views['observer']['path'])
            for config in (child_ordinary, child_observer):
                config['_binary_stats'] = {profile: r.file_identity(binary['path']) for profile, binary in config['binaries'].items()}
        else:
            c.need(shard['ordinary_binding'] == c.binding(args.ordinary_binding) and shard['observer_binding'] == c.binding(args.observer_binding), 'same-container exact binary bindings')
        child_tools = c.load(child_root / 'tools.json')['tools']
        for tool in child_tools.values():
            c.verify(tool['executable']['path'], tool['executable'])
            c.verify(tool['control_log']['path'], tool['control_log'])
            c.need(r.read_events(Path(tool['control_log']['path'])) == [{'unexpected_native_tool': True}], 'child trap direct evidence')
        child_rows = {section: read_rows(child_root, section) for section in ('public', 'lifecycle')}
    results = {}
    replacements = []
    def verify_row(section, row, got, toolmap, ordinary_cfg=ordinary, observer_cfg=observer, identity=None):
        if row['scope'] != 'execute':
            return
        if section == 'guards':
            source_case = contracts.cases[row['case']['source_case_id']]
            guards.verify_complete(row, got, observer_cfg, contracts, contracts.source_bytes(source_case), toolmap, ordinary_cfg)
            return
        sources = runner.tuple_sources(row, contracts, predecessors)
        if section == 'lifecycle':
            bound = got['ordinary_receipt']
            c.verify(bound['path'], bound)
            baseline = c.load(bound['path'])
            if got['executed']:
                c.need(baseline.get('executed') is True and baseline.get('status') == 'pass', 'ordinary lifecycle counterpart did not execute/pass')
            else:
                c.need(got.get('status') == baseline.get('status') == 'unsupported-capability' and baseline.get('executed') is False
                       and baseline.get('reason') == c.CAPABILITY_REASON, 'ordinary lifecycle counterpart is not the paired capability gap')
                proof = baseline.get('capability_proof', {})
                c.need('process' not in baseline and proof.get('read_open_succeeded') is True
                       and proof.get('mode_before_candidate') == 0 and proof.get('source_restored') is True,
                       'ordinary lifecycle gap has execution or lacks unavailability proof')
            if identity is not None:
                for item in (got, baseline):
                    require_denial_proof(item, identity, c.need)
            runner.lifecycle_compare(row, baseline, got, ordinary_cfg, observer_cfg, contracts, sources, toolmap)
        else:
            runner.core_compare(section, row, got, ordinary_cfg, contracts, sources, toolmap, predecessors)
    for section in ('public', 'original', 'predecessors', 'lifecycle', 'guards'):
        rows = contracts.roster(section)
        originals = read_rows(root, section)
        c.check_inventory(rows, originals)
        by_key = {got['key']: got for got in originals}
        gaps = {got['key'] for got in originals if got['status'] == 'unsupported-capability'}
        candidates = child_rows.get(section, [])
        selected = replacement_map(gaps, candidates, c.need)
        combined = []
        for row in rows:
            old = by_key[row['key']]
            c.need(old['status'] != 'fail', 'main failed predicate is not a capability gap')
            verify_row(section, row, old, main_tools)
            got = old
            if row['key'] in selected:
                got = selected[row['key']]
                c.need(row['required_capabilities'] == [CAPABILITY], 'replacement outside capability scope')
                c.need(got['executed'] and got['status'] == 'pass', 'replacement did not execute/pass')
                require_denial_proof(got, shard, c.need)
                verify_row(section, row, got, child_tools, child_ordinary, child_observer, shard)
                replacements.append({'section': section, 'key': row['key'], 'main_raw_sha256': c.sha(json.dumps(old, sort_keys=True).encode()),
                                     'shard_raw_sha256': c.sha(json.dumps(got, sort_keys=True).encode())})
            combined.append(got)
        results[section] = runner.aggregate(rows, combined)
        c.need(results[section]['unsupported_capability'] == 0, 'unresolved joined capability gap')
    c.need(not child_root or len(replacements) == 12, 'exact frozen twelve replacements required')
    configs(args, c, r)
    result = {'status': 'QUALIFIED_LOCAL_RAW_JOIN', 'hosted_controller_sha256': c.sha(Path(__file__).read_bytes()), 'expected_head': args.expected_head,
            'main_report': c.binding(root / 'comparison.json'), 'shard_report': c.binding(child_root / 'comparison.json') if child_root else None,
            'raw_replacements': replacements, 'sections': results, 'global_host_qualification': False,
            'summary_only_substitution': False, 'same_container_original_receipts_preserved': True, 'readonly_copy_used': bool(shard and shard.get('readonly_transport'))}
    if contracts.effective_identity:
        result.update(contracts.effective_identity)
    return result


def probe(args):
    c, r, runner, _, _ = modules(args.adapter)
    try:
        c.need(os.name == 'posix' and c.host() == 'Linux x86_64' and os.geteuid() == args.expected_uid and os.getegid() == args.expected_gid and os.getgroups() == [], 'transient identity preflight')
        ordinary, observer = configs(args, c, r)
        for path in (Path(args.contract_package) / 'transport.py', Path(args.contract_package) / 'transport-manifest.json'):
            path.read_bytes()
        for path in Path(args.contract_package).iterdir():
            c.need(path.is_file(), 'unexpected contract package member')
            path.read_bytes()
        if args.amendment_root:
            for name in ('artifacts/amendment.json', 'artifacts/effective-contract.json', 'receipt-identity.json'):
                (Path(args.amendment_root) / name).read_bytes()
        receipt = c.load(args.trap_binding)
        for row in (receipt['source'], receipt['binary']):
            c.verify(row['path'], row)
        print(json.dumps({'status': 'READ_ACCESS_VERIFIED', 'effective_uid': os.geteuid(), 'effective_gid': os.getegid(), 'groups': os.getgroups()}))
        return 0
    except PermissionError as error:
        print(json.dumps({'status': 'READ_ACCESS_DENIED', 'path': error.filename, 'effective_uid': os.geteuid(), 'effective_gid': os.getegid(), 'groups': os.getgroups()}))
        return 20


def orchestrate(args):
    c, r, runner, _, _ = modules(args.adapter)
    output = Path(args.out).resolve()
    output.mkdir(parents=True, exist_ok=False)
    root = Path(args.main_root).resolve()
    main = c.load(root / 'comparison.json')
    c.need(main['all_contract_sections_collected'] is True and main['status'] in ('INCOMPLETE', 'LOCAL_COMPARISONS_COMPLETE'), 'main needs non-capability repair first')
    gaps = [got for section in ('public', 'original', 'predecessors', 'lifecycle', 'guards') for got in read_rows(root, section) if got['status'] == 'unsupported-capability']
    if not gaps:
        c.save(output / 'comparison.json', raw_join(args))
        return 0
    c.need(len(gaps) == 12 and os.name == 'posix' and os.geteuid() == 0 and c.host() == 'Linux x86_64', 'transient root-owned Linux twelve-gap controller preconditions')
    account = pwd.getpwnam(args.existing_user)
    c.need(account.pw_uid != 0, 'existing non-root account required')
    setpriv = shutil.which('setpriv')
    c.need(setpriv is not None, 'installed setpriv required; no account/settings workaround')
    # All new ownership changes are restricted to this freshly created directory.
    child_root = Path(tempfile.mkdtemp(prefix='oxid-capability-', dir='/tmp'))
    os.chown(child_root, account.pw_uid, account.pw_gid)
    traps = output / 'prebuilt-traps'
    traps.mkdir()
    r.setup_no_tool_traps(traps)
    trap_binding = traps / 'no-tool-trap-build.json'
    prefix = [setpriv, '--reuid=' + str(account.pw_uid), '--regid=' + str(account.pw_gid), '--clear-groups', '--no-new-privs']
    def command(operation, adapter, controller, ordinary_binding, observer_binding, package, amendment, trap, destination, ledger=None):
        argv = prefix + [sys.executable, controller, operation, '--adapter', adapter, '--ordinary-binding', ordinary_binding,
            '--observer-binding', observer_binding, '--expected-head', args.expected_head, '--contract-package', package,
            '--trap-binding', trap, '--out', destination, '--expected-uid', str(account.pw_uid), '--expected-gid', str(account.pw_gid)]
        if amendment:
            argv += ['--amendment-root', amendment]
        if ledger:
            argv += ['--transport-ledger', ledger]
        return argv
    args.trap_binding = str(trap_binding)
    direct = command('probe', str(Path(args.adapter).resolve()), str(Path(__file__).resolve()),
        str(Path(args.ordinary_binding).resolve()), str(Path(args.observer_binding).resolve()),
        str(Path(args.contract_package).resolve()), str(Path(args.amendment_root).resolve()) if args.amendment_root else None,
        str(trap_binding), str(child_root))
    preflight = r.process(direct, child_root, {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1', 'PYTHONNOUSERSITE': '1'}, timeout=120)
    c.save(output / 'read-access-probe.json', preflight)
    copy_required = preflight['status'] == 20
    if preflight['status'] != 0 and not copy_required:
        # This also covers an inaccessible controller/helper before probe import;
        # copies are permitted only with an explicit opt-in, never a chmod retry.
        c.need(args.copy_if_unreadable and 'Permission denied' in preflight['stderr'], 'read access preflight failed for a non-copyable reason')
        copy_required = True
    c.need(not preflight['timed_out'] and not preflight['stream_limit_exceeded'], 'read preflight incomplete')
    worker_output = child_root / 'observations'
    worker_output.mkdir(mode=0o700)
    os.chown(worker_output, account.pw_uid, account.pw_gid)
    if copy_required:
        c.need(args.copy_if_unreadable, 'readonly copy required but not selected; existing permissions stay unchanged')
        copied = prepare_copy_view(args, child_root, c, r)
        c.save(output / 'readonly-copy-plan.json', copied)
        argv = command('worker', copied['adapter'], copied['controller'], copied['views']['ordinary']['path'], copied['views']['observer']['path'],
            copied['contract_package'], copied['amendment_root'], copied['trap_binding'], str(worker_output), copied['ledger']['path'])
    else:
        c.need(json.loads(preflight['stdout'])['status'] == 'READ_ACCESS_VERIFIED', 'unverified read access result')
        argv = command('worker', str(Path(args.adapter).resolve()), str(Path(__file__).resolve()), str(Path(args.ordinary_binding).resolve()),
            str(Path(args.observer_binding).resolve()), str(Path(args.contract_package).resolve()),
            str(Path(args.amendment_root).resolve()) if args.amendment_root else None, str(trap_binding), str(worker_output))
    p = r.process(argv, worker_output, {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1', 'PYTHONNOUSERSITE': '1'}, timeout=600)
    c.save(output / 'child-process.json', p)
    c.need(p['status'] == 0 and not p['timed_out'] and not p['stream_limit_exceeded'], 'non-root worker failed; existing permissions were not changed')
    result = raw_join(args, worker_output)
    result['transient_child_root'] = str(child_root)
    result['child_process'] = c.binding(output / 'child-process.json')
    c.save(output / 'comparison.json', result)
    # Preserve raw child evidence without rewriting its logical paths or receipts.
    shutil.copytree(child_root, output / 'child-evidence')
    c.save(output / 'child-copy-map.json', {'original_root': str(child_root), 'copied_root': str(output / 'child-evidence'),
                                         'policy': 'Byte-preserving archive copy only; comparisons used original same-container paths.'})
    return 0


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=('worker', 'join', 'probe'))
    parser.add_argument('--adapter', required=True)
    parser.add_argument('--ordinary-binding', required=True)
    parser.add_argument('--observer-binding', required=True)
    parser.add_argument('--expected-head', required=True)
    parser.add_argument('--contract-package', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--trap-binding')
    parser.add_argument('--main-root')
    parser.add_argument('--contracts')
    parser.add_argument('--amendment-root')
    parser.add_argument('--existing-user', default='nobody')
    parser.add_argument('--expected-uid', type=int)
    parser.add_argument('--expected-gid', type=int)
    parser.add_argument('--copy-if-unreadable', action='store_true')
    parser.add_argument('--transport-ledger')
    args = parser.parse_args()
    return worker(args) if args.command == 'worker' else (probe(args) if args.command == 'probe' else orchestrate(args))

if __name__ == '__main__':
    sys.exit(main())
