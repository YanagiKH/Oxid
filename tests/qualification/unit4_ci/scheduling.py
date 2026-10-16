"""Closed, class-serial integration shards and exact-run four-host join."""
import argparse
from contextlib import redirect_stdout, redirect_stderr
import traceback
import json
import os
from pathlib import Path
import sys
import unittest
import common as q

ROSTER = Path(__file__).with_name('integration-roster-v1.json')


def flatten(suite):
    for test in suite:
        if isinstance(test, unittest.TestSuite):
            yield from flatten(test)
        else:
            yield test


def discover():
    import test_integration
    loader = unittest.TestLoader()
    tests = list(flatten(loader.loadTestsFromModule(test_integration)))
    q.need(not loader.errors, 'discovery failed')
    ids = [test.id() for test in tests]
    q.need(ids and len(ids) == len(set(ids)), 'duplicate/empty discovery')
    groups = {}
    for test in tests:
        groups.setdefault(test.id().rsplit('.', 1)[0], []).append(test)
    return dict(sorted(groups.items()))


def roster(groups):
    return {name: [test.id() for test in cases] for name, cases in groups.items()}


def checked_classes():
    groups = discover()
    q.need(roster(groups) == q.read(ROSTER), 'closed discovery roster changed')
    return groups


def partition(classes):
    comparison = 'test_integration.ComparisonSealControls'
    q.need(comparison in classes and len(classes) > 1, 'missing comparison/remaining classes')
    return {'comparison-seal': list(classes[comparison]),
            'remaining-controls': [test for name, tests in classes.items() if name != comparison for test in tests]}


def checked_groups():
    return partition(checked_classes())


class Result(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.started = []
        self.passed = []

    def startTest(self, test):
        self.started.append(test.id())
        super().startTest(test)

    def addSuccess(self, test):
        self.passed.append(test.id())
        super().addSuccess(test)


def accepted(result, ids):
    return (result.wasSuccessful() and not result.skipped and not result.expectedFailures
            and not result.unexpectedSuccesses and result.started == ids and result.passed == ids)


def context(args):
    admitted = q.admit(args.repo, args.head, args.event_sha)
    run, attempt = os.environ['GITHUB_RUN_ID'], os.environ['GITHUB_RUN_ATTEMPT']
    q.need(run.isdecimal() and attempt.isdecimal() and int(run) > 0 and int(attempt) > 0,
           'invalid run/attempt')
    return {'head': admitted['checkout_head'], 'tree': admitted['checkout_tree'],
            'event_sha': args.event_sha, 'run': run, 'attempt': attempt,
            'roster_sha256': q.sha(ROSTER.read_bytes())}


def artifact_name(identity, host, shard):
    return 'unit4-controls-' + identity['run'] + '-' + identity['attempt'] + '-' + q.SLUGS[host] + '-' + shard


def validate_receipts(groups, receipts, identity, job_results):
    q.need(set(job_results) == {'unit4-integration'} and
           job_results['unit4-integration']['result'] == 'success', 'controls failed/skipped/cancelled')
    expected = {(host, name) for host in q.HOSTS for name in groups}
    seen = set()
    for receipt in receipts:
        q.need(set(receipt) == {'schema', 'identity', 'host', 'shard', 'ids', 'started', 'passed', 'status'},
               'unknown/missing receipt field')
        key = (receipt['host'], receipt['shard'])
        q.need(key in expected and key not in seen, 'unknown/duplicate shard')
        seen.add(key)
        q.need(receipt['schema'] == 'oxid-unit4-integration-shard-v1' and
               receipt['identity'] == identity and receipt['status'] == 'pass', 'stale/failed shard')
        ids = groups[key[1]]
        q.need(receipt['ids'] == receipt['started'] == receipt['passed'] == ids,
               'missing/duplicate/unknown/skipped test')
    q.need(seen == expected, 'missing shard')


def run(args):
    args.output = q.fresh(args.output)
    receipt = {'schema': 'oxid-unit4-integration-shard-v1', 'status': 'fail',
               'identity': {}, 'host': args.host, 'shard': args.shard,
               'ids': [], 'started': [], 'passed': []}
    with (args.output / 'unittest.log').open('w', encoding='utf-8') as log:
        try:
            with redirect_stdout(log), redirect_stderr(log):
                groups = checked_groups()
                q.need(args.shard in groups, 'unknown group')
                q.need(q.measured_host()['name'] == args.host, 'wrong actual host')
                receipt['identity'] = context(args)
                receipt['ids'] = [test.id() for test in groups[args.shard]]
                # Whole classes remain contiguous; one serial TestSuite/process per group.
                result = unittest.TextTestRunner(stream=log, verbosity=2, resultclass=Result).run(
                    unittest.TestSuite(groups[args.shard]))
                receipt.update(started=result.started, passed=result.passed)
                q.need(accepted(result, receipt['ids']), 'integration controls did not all pass')
                q.need(context(args) == receipt['identity'], 'post-suite identity changed')
                q.need(q.measured_host()['name'] == args.host, 'post-suite host changed')
                receipt['status'] = 'pass'
        except BaseException:
            traceback.print_exc(file=log)
            raise
        finally:
            q.save(args.output / 'receipt.json', receipt)


def receipt_file(path):
    path = q.regular(path)
    q.need(path.stat().st_nlink == 1, 'hardlinked receipt')
    return path


def join(args):
    args.output = q.fresh(args.output)
    report = {'schema': 'oxid-unit4-integration-join-v1', 'status': 'fail'}
    try:
        groups = roster(checked_groups())
        identity = context(args)
        receipts = []
        expected_dirs = {artifact_name(identity, h, c) for h in q.HOSTS for c in groups}
        q.need({p.name for p in args.artifacts.iterdir()} == expected_dirs, 'artifact roster mismatch')
        for directory in sorted(args.artifacts.iterdir()):
            q.need(directory.is_dir() and not directory.is_symlink(), 'unsafe artifact directory')
            q.need({p.name for p in directory.iterdir()} == {'receipt.json', 'unittest.log'},
                   'incomplete/extra artifact files')
            receipt = q.read(receipt_file(directory / 'receipt.json'))
            q.need(directory.name == artifact_name(identity, receipt['host'], receipt['shard']),
                   'artifact name mismatch')
            receipts.append(receipt)
        validate_receipts(groups, receipts, identity, json.loads(args.job_results))
        report.update(status='pass', identity=identity, shards=len(receipts), tests=sum(map(len, groups.values()))*4)
    except BaseException as error:
        report['error'] = str(error)
        report['traceback'] = traceback.format_exc()
        raise
    finally:
        q.save(args.output / 'integration.json', report)


def verify(args):
    """Rerun-failed jobs must not reuse a successful join from an older attempt."""
    groups = roster(checked_groups())
    identity = context(args)
    q.need({p.name for p in args.artifacts.iterdir()} == {'integration.json'}, 'missing/extra integration approval')
    report = q.read(receipt_file(args.artifacts/'integration.json'))
    q.need(report == {'schema': 'oxid-unit4-integration-join-v1', 'status': 'pass',
                      'identity': identity, 'shards': len(groups)*len(q.HOSTS),
                      'tests': sum(map(len, groups.values()))*len(q.HOSTS)},
           'stale/failed/incomplete integration approval')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['run', 'join', 'verify'])
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--head', required=True)
    parser.add_argument('--event-sha', required=True)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--host')
    parser.add_argument('--shard')
    parser.add_argument('--artifacts', type=Path)
    parser.add_argument('--job-results')
    args = parser.parse_args()
    if args.mode != 'verify' and args.output is None:
        parser.error('--output is required for run/join')
    {'run': run, 'join': join, 'verify': verify}[args.mode](args)
