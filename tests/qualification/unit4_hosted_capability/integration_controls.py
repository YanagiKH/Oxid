#!/usr/bin/env python3
"""Bounded real-join predicate controls using explicitly synthetic transport data.

Reads retained real observations; never modifies them or invokes the compiler.
The synthetic root/shard identity data exercise protocol rejection, not an actual
root or non-root execution claim. Only source-only control results are saved.
"""
import sys
sys.dont_write_bytecode = True
import argparse
import copy
import json
import os
from pathlib import Path
import pwd
import tempfile
import types
from unittest import mock
import hosted


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--adapter', required=True)
    parser.add_argument('--main-root', required=True)
    parser.add_argument('--ordinary-binding', required=True)
    parser.add_argument('--observer-binding', required=True)
    parser.add_argument('--expected-head', required=True)
    parser.add_argument('--contracts', required=True)
    parser.add_argument('--amendment-root', required=True)
    parser.add_argument('--trap-controls', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--copy-contract-package', help='Also exercise the copied-input raw join using synthetic path projections')
    args = parser.parse_args()
    c, r, runner, _, _ = hosted.modules(args.adapter)
    real = Path(args.main_root)
    original_rows = {section: hosted.read_rows(real, section) for section in ('public', 'original', 'predecessors', 'lifecycle', 'guards')}
    account = pwd.getpwnam('nobody')
    controls = []
    with tempfile.TemporaryDirectory(prefix='oxid-join-controls-') as temp:
        root = Path(temp)
        main_root, shard_root, output = root / 'main', root / 'shard', root / 'controller'
        for directory in (main_root, shard_root, output): directory.mkdir()
        for name in ('comparison.json', 'input-integrity.json', 'tools.json'):
            (main_root / name).write_bytes((real / name).read_bytes())
        report = c.load(main_root / 'comparison.json')
        report.update(status='INCOMPLETE', synthetic_control_only=True)
        c.save(main_root / 'comparison.json', report)
        old_rows = {section: list(rows) for section, rows in original_rows.items()}
        shard_rows = {'public': [], 'lifecycle': []}
        def identity(got, uid, gid, groups, unsupported):
            result = copy.deepcopy(got)
            proof = result['capability_proof']
            proof.update(effective_uid=uid, effective_gid=gid, groups=groups, read_open_succeeded=unsupported)
            if unsupported:
                result.update(executed=False, status='unsupported-capability', reason=c.CAPABILITY_REASON)
                for field in ('process', 'trace', 'tool_invocations', 'emitted_llvm', 'native_execution', 'binary_stat_after'):
                    result.pop(field, None)
            return result
        for section in ('public', 'lifecycle'):
            for index, got in enumerate(original_rows[section]):
                if got.get('required_capabilities') != [hosted.CAPABILITY] or not got['executed']: continue
                gap = identity(got, 0, 0, [], True)
                child = identity(got, account.pw_uid, account.pw_gid, [], False)
                if section == 'lifecycle':
                    original_baseline = c.load(got['ordinary_receipt']['path'])
                    for role, result, uid, gid, unavailable in (('gap', gap, 0, 0, True), ('child', child, account.pw_uid, account.pw_gid, False)):
                        baseline = identity(original_baseline, uid, gid, [], unavailable)
                        path = root / (role + '-' + str(index) + '.json')
                        c.save(path, baseline)
                        result['ordinary_receipt'] = c.binding(path)
                old_rows[section][index] = gap
                shard_rows[section].append(child)
        c.need({k: len(v) for k, v in shard_rows.items()} == {'public': 6, 'lifecycle': 6}, 'control fixture exact twelve domain')
        child_report = {'status': 'PARTIAL_CAPABILITY_SHARD', 'complete_qualification': False, 'synthetic_control_only': True,
            'expected_head': args.expected_head, 'effective_uid': account.pw_uid, 'effective_gid': account.pw_gid, 'supplementary_groups': [],
            'adapter': runner.adapter_identity(), 'hosted_controller_sha256': c.sha(Path(hosted.__file__).read_bytes()),
            'ordinary_binding': c.binding(args.ordinary_binding), 'observer_binding': c.binding(args.observer_binding),
            'process_group_policy': {'identity': 'outer-owned-session-shared-nested-v1', 'worker_pid': 12345, 'process_group': 12345, 'session_id': 12345, 'nested_new_sessions': False},
            'readonly_transport': None}
        child_report.update({k: report[k] for k in c.EFFECTIVE_FIELDS})
        c.save(shard_root / 'comparison.json', child_report)
        c.save(output / 'child-process.json', {'pid': 12345, 'status': 0, 'timed_out': False, 'stream_limit_exceeded': False, 'synthetic_control_only': True})
        tools = {}
        for tool in ('clang', 'opt', 'ld.lld'):
            controls_root = Path(args.trap_controls)
            tools[tool] = {'executable': c.binding(controls_root / 'no-tool-traps' / tool),
                          'control_log': c.binding(controls_root / (tool + '-trap-control.jsonl'))}
        c.save(shard_root / 'tools.json', {'tools': tools})
        join_args = types.SimpleNamespace(adapter=args.adapter, main_root=str(main_root), ordinary_binding=args.ordinary_binding,
            observer_binding=args.observer_binding, expected_head=args.expected_head, contracts=args.contracts,
            amendment_root=args.amendment_root, existing_user='nobody', out=str(output))
        if args.copy_contract_package:
            copy_args = types.SimpleNamespace(adapter=args.adapter, ordinary_binding=args.ordinary_binding,
                observer_binding=args.observer_binding, expected_head=args.expected_head,
                contract_package=args.copy_contract_package, amendment_root=args.amendment_root,
                trap_binding=str(Path(args.trap_controls) / 'no-tool-trap-build.json'))
            copied_root = root / 'copied-inputs'
            copied_root.mkdir()
            plan = hosted.prepare_copy_view(copy_args, copied_root, c, r)
            child_report['readonly_transport'] = plan['ledger']
            child_report['ordinary_binding'] = plan['views']['ordinary']
            child_report['observer_binding'] = plan['views']['observer']
            configs = {role: r.candidate_binding(bound['path']) for role, bound in plan['views'].items()}
            def projected(item, cfg):
                item['candidate_binding_sha256'] = cfg['_sha256']
                item['binary'] = copy.deepcopy(cfg['binaries'][item['profile']])
                item['binary_stat_before'] = item['binary_stat_after'] = r.file_identity(item['binary']['path'])
                item['process']['argv'][0] = item['binary']['path']
            for section, items in shard_rows.items():
                for index, item in enumerate(items):
                    projected(item, configs['ordinary' if section == 'public' else 'observer'])
                    if section == 'lifecycle':
                        baseline = c.load(item['ordinary_receipt']['path'])
                        projected(baseline, configs['ordinary'])
                        baseline_path = root / ('copied-baseline-' + str(index) + '.json')
                        c.save(baseline_path, baseline)
                        item['ordinary_receipt'] = c.binding(baseline_path)
            c.save(shard_root / 'comparison.json', child_report)
        active_main, active_shard = old_rows, shard_rows
        def read_rows(path, section):
            return active_main[section] if Path(path) == main_root else active_shard[section]
        with mock.patch.object(hosted, 'read_rows', side_effect=read_rows):
            positive = hosted.raw_join(join_args, shard_root)
            c.need(len(positive['raw_replacements']) == 12 and all(s['unsupported_capability'] == 0 for s in positive['sections'].values()), 'synthetic positive protocol control')
            # Never save the synthetic returned qualification object as evidence.
            controls.append({'control': 'synthetic-twelve-key-positive', 'status': 'CONTROL_ACCEPTED_ONLY'})
            if args.copy_contract_package:
                for boundary in (copied_root / 'readonly', copied_root):
                    alias_target = boundary.with_name(boundary.name + '-alias-target')
                    boundary.rename(alias_target)
                    boundary.symlink_to(alias_target, target_is_directory=True)
                    try:
                        try:
                            hosted.raw_join(join_args, shard_root)
                        except c.Reject as error:
                            c.need(str(error) == 'readonly transport physical directory boundaries', 'alias rejected for an unrelated reason')
                            controls.append({'control': boundary.name + '-renamed-directory-alias', 'status': 'REJECTED', 'reason': str(error)})
                        else:
                            raise c.Reject('raw_join accepted aliased transport boundary')
                    finally:
                        boundary.unlink()
                        alias_target.rename(boundary)
            def bad_baseline(m,s,h):
                binding=s['lifecycle'][0]['ordinary_receipt']
                baseline=c.load(binding['path'])
                baseline['status']='fail'
                changed=root/'failed-child-baseline.json'
                c.save(changed,baseline)
                s['lifecycle'][0]['ordinary_receipt']=c.binding(changed)
            def bad_gap_baseline(m, s, h, execution):
                item = next(row for row in m['lifecycle'] if row['status'] == 'unsupported-capability')
                baseline = c.load(item['ordinary_receipt']['path'])
                if execution:
                    baseline['process'] = {'status': 0}
                else:
                    baseline['capability_proof']['read_open_succeeded'] = False
                path = root / ('bad-gap-execution.json' if execution else 'bad-gap-proof.json')
                c.save(path, baseline)
                item['ordinary_receipt'] = c.binding(path)
            mutations = [
                ('executed-ordinary-gap', lambda m,s,h: bad_gap_baseline(m,s,h,True)),
                ('false-ordinary-gap-proof', lambda m,s,h: bad_gap_baseline(m,s,h,False)),
                ('stale-source', lambda m,s,h: s['public'][0]['source_files'][0].__setitem__('sha256','0'*64)),
                ('missing-source', lambda m,s,h: s['public'][0].pop('source_files')),
                ('stale-binary', lambda m,s,h: s['public'][0]['binary'].__setitem__('sha256','0'*64)),
                ('stale-head', lambda m,s,h: h.__setitem__('expected_head','0'*40)),
                ('stale-config-binding', lambda m,s,h: h['ordinary_binding'].__setitem__('sha256','0'*64)),
                ('missing-effective-identity', lambda m,s,h: h.pop('effective_contract_descriptor_sha256')),
                ('stale-effective-identity', lambda m,s,h: h.__setitem__('effective_contract_descriptor_sha256','0'*64)),
                ('missing-lifecycle-ordinary', lambda m,s,h: s['lifecycle'][0].pop('ordinary_receipt')),
                ('failed-original-row', lambda m,s,h: next(x for x in m['public'] if x['executed']).__setitem__('status','fail')),
                ('correct-count-wrong-key', lambda m,s,h: s['public'][0].__setitem__('key','wrong-capability-key')),
                ('fabricated-uid', lambda m,s,h: h.__setitem__('effective_uid',0)),
                ('fabricated-groups', lambda m,s,h: h.__setitem__('supplementary_groups',[0])),
                ('fabricated-proof-identity', lambda m,s,h: s['public'][0]['capability_proof'].__setitem__('effective_uid',0)),
                ('summary-only-shard', lambda m,s,h: s['public'].clear()),
                ('missing-worker-group', lambda m,s,h: h.pop('process_group_policy')),
                ('new-nested-session', lambda m,s,h: h['process_group_policy'].__setitem__('nested_new_sessions',True)),
                ('wrong-public-gid', lambda m,s,h: s['public'][0]['capability_proof'].__setitem__('effective_gid', 0)),
                ('missing-public-gid', lambda m,s,h: s['public'][0]['capability_proof'].pop('effective_gid')),
                ('failed-child-baseline', bad_baseline),
                ('missing-capability-errno', lambda m,s,h: s['public'][0]['capability_proof'].pop('error_errno')),
                ('incorrect-capability-errno', lambda m,s,h: s['public'][0]['capability_proof'].__setitem__('error_errno', 2)),
                ('duplicate-public-row', lambda m,s,h: s['public'].append(copy.deepcopy(s['public'][0]))),
                ('missing-lifecycle-row', lambda m,s,h: s['lifecycle'].pop()),
                ('failed-public-row', lambda m,s,h: s['public'][0].__setitem__('status','fail')),
                ('wrong-lifecycle-gid', lambda m,s,h: s['lifecycle'][0]['capability_proof'].__setitem__('effective_gid',0)),
            ]
            for name, mutate in mutations:
                # Keep copied metadata bounded: only public/lifecycle records can mutate.
                active_main = {**old_rows, 'public': copy.deepcopy(old_rows['public']), 'lifecycle': copy.deepcopy(old_rows['lifecycle'])}
                active_shard = copy.deepcopy(shard_rows)
                header = copy.deepcopy(child_report)
                mutate(active_main, active_shard, header)
                c.save(shard_root / 'comparison.json', header)
                try:
                    hosted.raw_join(join_args, shard_root)
                except (c.Reject, KeyError, TypeError, ValueError, OSError) as error:
                    controls.append({'control': name, 'status': 'REJECTED', 'reason': str(error)[:300]})
                else:
                    raise c.Reject('integrated raw_join accepted mutation: ' + name)
                print(name, 'rejected', flush=True)
    c.save(args.out, {'status': 'SOURCE_ONLY_INTEGRATED_CONTROLS', 'synthetic_protocol_data': True,
                     'compiler_invocations': 0, 'uid_changes': 0, 'actual_root_branch_qualification': False, 'copied_input_protocol': bool(args.copy_contract_package),
                     'controls': controls, 'controller': c.binding(hosted.__file__)})
    print('Integrated raw_join controls completed; no actual root/shard qualification claim')

if __name__ == '__main__':
    main()
