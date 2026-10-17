#!/usr/bin/env python3
"""Qualify the experimental overlay with both exact-head retained HIR compilers."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def need(ok, message):
    if not ok:
        raise ValueError(message)


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, timeout=60).decode().strip()


def admitted_profiles(receipt_path, head, tree):
    receipt_path = Path(receipt_path)
    need(not receipt_path.is_symlink(), 'aliased compiler receipt')
    data = json.loads(receipt_path.read_bytes())
    need(data.get('complete') is True, 'incomplete compiler qualification')
    need(data.get('source') == {'head': head, 'tree': tree}
         and data.get('expected_head') == head, 'stale compiler source')
    need(set(data.get('profiles', {})) == {'debug', 'release'}, 'missing/extra compiler profile')
    result = {}
    for profile, row in data['profiles'].items():
        path = receipt_path.parent / (profile + '-compiler')
        need(row.get('complete') is True and row.get('source') == data['source'], 'incomplete/stale profile')
        need(Path(row['retained_executable']) == path and not path.is_symlink()
             and path.is_file(), 'unexpected retained compiler path')
        need(sha(path) == row['executable_sha256'], 'retained compiler identity changed')
        artifact = row['cargo_artifact']
        need(artifact.get('target', {}).get('name') == 'oxid'
             and artifact.get('target', {}).get('kind') == ['bin']
             and artifact.get('profile', {}).get('test') is False
             and artifact['profile'].get('opt_level') == ('0' if profile == 'debug' else '3'),
             'incorrect ordinary compiler profile')
        result[profile] = path
    return result


def validate_report(report, compiler, harness, head):
    need(report.get('passed') is True, 'wide parser qualification incomplete')
    need(report.get('source_head') == head and report.get('compiler_sha256') == sha(compiler)
         and report.get('harness_sha256') == sha(harness), 'wide parser report identity mismatch')
    roster_path = ROOT / 'tests/fixtures/bounded_typed_parser/cases.json'
    roster = json.loads(roster_path.read_bytes())
    names = [row['name'] for row in roster['cases']]
    names += [label + '-' + str(n) for n in (254, 255, 256) for label in ('function', 'diagnostic')]
    names += ['empty', 'non-ascii', 'token-capacity']
    comparisons = report.get('comparisons', [])
    need([row.get('name') for row in comparisons] == names and report.get('roster_sha256') == sha(roster_path),
         'incomplete or stale differential roster')
    need(comparisons and all(set(row['modes']) == {'baseline-reference', 'candidate-reference',
             'baseline-native', 'candidate-native'} for row in comparisons), 'missing reference/native comparisons')
    controls = report.get('controls', [])
    pairs = [(r['mode'], r['execution']) for r in controls]
    need(len(pairs) == 44 and set(pairs) == {(i, mode) for i in range(22)
             for mode in ('reference', 'native')}, 'incomplete wide carrier controls')
    need(set(report.get('binary_sha256', {})) == {'baseline', 'candidate', 'controls'}, 'missing native binaries')
    need(set(report.get('canonical_observers', {})) == {'parser', 'lexer'}, 'missing independent observers')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--receipt', type=Path, required=True)
    parser.add_argument('--llvm-bin', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.absolute()
    need(not output.is_relative_to(ROOT), 'evidence must be outside checkout')
    output.mkdir(parents=True, exist_ok=False)
    report = {'schema': 'oxid-wide-span-parser-ci-v1', 'complete': False, 'profiles': {}}
    def save():
        (output / 'receipt.json').write_text(json.dumps(report, indent=2) + '\n')
    save()
    try:
        head, tree = git('rev-parse', 'HEAD'), git('rev-parse', 'HEAD^{tree}')
        need(not git('status', '--porcelain=v1', '--untracked-files=all'), 'dirty compiler checkout')
        profiles = admitted_profiles(args.receipt.absolute(), head, tree)
        report.update(source={'head': head, 'tree': tree}, compiler_receipt_sha256=sha(args.receipt))
        harness = ROOT / 'scripts/verify_wide_span_parser.py'
        for profile in ('debug', 'release'):
            command = [sys.executable, '-B', str(harness), '--compiler', str(profiles[profile]),
                       '--llvm-bin', str(args.llvm_bin.resolve(strict=True)), '--output', str(output / profile)]
            started = time.monotonic()
            row = {'argv': command, 'complete': False}
            report['profiles'][profile] = row
            save()
            try:
                child = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=1200)
                stdout, stderr, status = child.stdout, child.stderr, child.returncode
            except subprocess.TimeoutExpired as error:
                stdout, stderr, status = error.stdout or b'', error.stderr or b'', 'timeout'
            (output / (profile + '.stdout')).write_bytes(stdout)
            (output / (profile + '.stderr')).write_bytes(stderr)
            row.update(status=status, elapsed_seconds=time.monotonic()-started)
            save()
            need(status == 0, profile + ' qualification failed')
            detail = json.loads((output / profile / 'report.json').read_bytes())
            validate_report(detail, profiles[profile], harness, head)
            row.update(complete=True, report_sha256=sha(output / profile / 'report.json'))
            save()
        need(admitted_profiles(args.receipt.absolute(), head, tree) == profiles, 'compiler receipts changed')
        need(report['compiler_receipt_sha256'] == sha(args.receipt), 'compiler receipt changed')
        need((git('rev-parse', 'HEAD'), git('rev-parse', 'HEAD^{tree}')) == (head, tree)
             and not git('status', '--porcelain=v1', '--untracked-files=all'), 'source changed')
        report['complete'] = True
        save()
    except Exception as error:
        report['error'] = str(error)
        save()
        raise
    print(output / 'receipt.json')


if __name__ == '__main__':
    main()
