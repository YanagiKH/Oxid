#!/usr/bin/env python3
"""Reject coherent rewritten current/observer manifests without running a compiler."""
import sys
sys.dont_write_bytecode = True
import argparse
import copy
import tempfile
from pathlib import Path
from contracts import Contracts, Reject, need, load, save, binding, sha, receipt_identity, check_inventory
from runtime import candidate_binding

def check(original, modified_path, message):
    cfg = candidate_binding(original)
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        source = root / 'source'
        manifest = load(cfg['source_manifest']['path'])
        for row in manifest['files']:
            path = source / row['path']
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((Path(cfg['source_root']) / row['path']).read_bytes())
        path = source / modified_path
        path.write_bytes(path.read_bytes() + b'\n// unauthorized coherent source change\n')
        row = next(row for row in manifest['files'] if row['path'] == modified_path)
        row.update(bytes=path.stat().st_size, sha256=sha(path.read_bytes()))
        manifest_path = root / 'rewritten-source.json'
        save(manifest_path, manifest)
        cfg['source_root'] = str(source)
        cfg['source_manifest'] = binding(manifest_path)
        cfg.pop('_sha256', None)
        candidate_path = root / 'rewritten-candidate.json'
        save(candidate_path, cfg)
        try:
            candidate_binding(candidate_path)
        except Reject as error:
            need(message in str(error), 'coherent manifest rejected for wrong reason: ' + str(error))
            return {'control': modified_path, 'status': 'REJECTED', 'reason': str(error), 'compiler_invocations': 0}
        raise Reject('coherent unauthorized source manifest accepted')

def roster_controls(contracts):
    from run import aggregate, skipped, negative_controls
    results = []
    for actual_host in ('Linux x86_64', 'macOS x86_64', 'macOS arm64', 'Windows x86_64'):
        for section in ('public', 'original', 'predecessors', 'lifecycle', 'guards'):
            rows = contracts.roster(section, actual_host)
            local = sum(row['scope'] == 'execute' for row in rows)
            if local == 0:
                receipts = [skipped(row) for row in rows]
                result = aggregate(rows, receipts)
                need(result['status'] == 'NOT_APPLICABLE_THIS_HOST', 'empty actual domain must be nonapplicable')
                controls = negative_controls(rows, receipts, lambda row, got: None)
                results.append({'host': actual_host, 'section': section, 'local_tuples': local, 'status': result['status'], 'controls': controls})
            else:
                receipts = [skipped(row) if row['scope'] != 'execute' else {**receipt_identity(row), 'executed': False, 'status': 'excluded-host', 'reason': 'host outside frozen required_hosts'} for row in rows]
                try:
                    aggregate(rows, receipts)
                except Reject:
                    results.append({'host': actual_host, 'section': section, 'local_tuples': local, 'all_false_skips': 'REJECTED'})
                else:
                    raise Reject('nonempty required local domain accepted zero execution')
    for host_name in ('macOS x86_64', 'macOS arm64', 'Windows x86_64'):
        got = {row['section']: row['local_tuples'] for row in results if row['host'] == host_name}
        need(got == {'public': 130, 'original': 248, 'predecessors': 0, 'lifecycle': 154, 'guards': 0}, 'frozen non-Linux local-domain inventory')
    return results

def amendment_controls(base_root, path_map, amendment_root):
    from contracts import EFFECTIVE_FIELDS
    base = Contracts(base_root, path_map)
    raw_before = (Path(base_root) / 'public-cli-contract-replacement-v3.json').read_bytes()
    amended = Contracts(base_root, path_map, amendment_root)
    rows = amended.roster('public')
    row = next(r for r in rows if r['scope'] == 'execute')
    good = {**receipt_identity(row), 'executed': True, 'status': 'pass'}
    check_inventory([row], [good])
    controls = []
    def rejected(name, operation):
        try:
            operation()
        except (Reject, KeyError, ValueError) as error:
            controls.append({'control': name, 'status': 'REJECTED', 'reason': str(error)})
        else:
            raise Reject('amendment control accepted: ' + name)
    rejected('duplicate-application', lambda: amended.apply_amendment(amendment_root))
    for field in EFFECTIVE_FIELDS:
        bad = copy.deepcopy(good)
        bad.pop(field)
        rejected('missing-' + field, lambda bad=bad: check_inventory([row], [bad]))
    bad = copy.deepcopy(good)
    bad['ordered_amendment_sha256'] *= 2
    rejected('duplicate-receipt-amendment', lambda: check_inventory([row], [bad]))
    changed_case = copy.deepcopy(base)
    changed_case.tables['public']['inherited_negative_and_route_cases'][13]['expected_public_projection']['diagnostic_cap'] = 99
    rejected('unlisted-case-change', lambda: changed_case.apply_amendment(amendment_root))
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        (root / 'public-cli-contract-replacement-v3.json').write_bytes(raw_before)
        source = root / 'sources/host-malformed-root-first/main.ox'
        source.parent.mkdir(parents=True)
        source.write_bytes(b'changed source')
        changed_source = copy.deepcopy(base)
        changed_source.root = root
        rejected('changed-source', lambda: changed_source.apply_amendment(amendment_root))
    need((Path(base_root) / 'public-cli-contract-replacement-v3.json').read_bytes() == raw_before, 'base public document was rewritten')
    return {'effective_identity': amended.effective_identity, 'controls': controls, 'base_bytes_preserved': True, 'compiler_invocations': 0}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--ordinary-binding', required=True)
    parser.add_argument('--observer-binding', required=True)
    parser.add_argument('--out', required=True)
    parser.add_argument('--contracts', required=True)
    parser.add_argument('--path-map', required=True)
    parser.add_argument('--amendment-root')
    args = parser.parse_args()
    results = [check(args.ordinary_binding, 'src/frontend/driver.rs', 'unapproved current compiler source authority'),
               check(args.observer_binding, 'src/frontend/lifecycle_observer.rs', 'unapproved lifecycle observer bodies')]
    rosters = roster_controls(Contracts(args.contracts, load(args.path_map), args.amendment_root))
    amendment = amendment_controls(args.contracts, load(args.path_map), args.amendment_root) if args.amendment_root else None
    save(args.out, {'controls': results, 'actual_host_roster_controls': rosters, 'amendment': amendment, 'compiler_invocations': 0})
    print('Rejected both coherent source/observer forgeries')

if __name__ == '__main__':
    main()
