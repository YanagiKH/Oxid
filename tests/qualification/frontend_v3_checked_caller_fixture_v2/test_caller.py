"""Focused local fixture controls. Never invokes Oxid, producers or compilers."""
from dataclasses import replace
import errno
import hashlib
import json
import os
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import caller as c
import fixture_owner as ownerlib
import physical_checks as pc


def file_row(path, body):
    return c.Row(path, 'file', 0o600, pc.ExpectedFile(0o600, len(body), hashlib.sha256(body).hexdigest()))


# Expected constants exist before any fixture creation or candidate observation.
SOURCE_BYTES = b'fixed helper body\n'
SHARED_BYTES = b'fixed shared body\n'
SOURCE_ROWS = (c.Row((), 'directory', 0o700), file_row(('helper.py',), SOURCE_BYTES))
OUTER_ROWS = (
    c.Row((), 'directory', 0o700),
    c.Row(('empty',), 'directory', 0o700),
    file_row(('sibling',), b'unconsumed sibling\n'),
    c.Row(('wide-span',), 'directory', 0o700),
    file_row(('wide-span', 'raw.stdout'), b'fixed raw\n'),
    file_row(('wide-span', 'shared'), SHARED_BYTES),
    file_row(('wide-span', 'source.ox'), b'fixed source, not executable\n'),
)
BODIES = {('helper.py',): SOURCE_BYTES, ('sibling',): b'unconsumed sibling\n',
          ('wide-span', 'raw.stdout'): b'fixed raw\n', ('wide-span', 'shared'): SHARED_BYTES,
          ('wide-span', 'source.ox'): b'fixed source, not executable\n'}


def leaves(error):
    if isinstance(error, BaseExceptionGroup):
        return [leaf for child in error.exceptions for leaf in leaves(child)]
    return [error]


class CallerControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='oxid-fixture-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source, self.evidence = self.root / 'source', self.root / 'evidence'
        for root, rows in ((self.source, SOURCE_ROWS), (self.evidence, OUTER_ROWS)):
            for row in rows:
                path = root.joinpath(*row.path)
                if row.kind == 'directory':
                    path.mkdir()
                else:
                    path.write_bytes(BODIES[row.path])
                path.chmod(row.mode)
        self.owner = ownerlib.create_owner(self.root)
        self.addCleanup(ownerlib._owners.pop, self.owner.ownership_token, None)
        pins = json.loads(Path(c.__file__).with_name('fixed-inputs.json').read_text())
        self.fixture = c.FixtureV2('oxid-v3-local-fixture-v2', str(self.source), str(self.evidence),
            str(self.evidence / 'wide-span'), SOURCE_ROWS, OUTER_ROWS, (('missing',),), pins['physical_checks_sha256'])
        self.calls, self.capabilities = [], []

    def binding(self, source=None, raw=None):
        def source_probe(reads):
            self.calls.append('source'); self.capabilities.append(reads)
            self.assertEqual(reads.read_source(('helper.py',)), SOURCE_BYTES)
            self.assertEqual(reads.read_wide(('source.ox',)), BODIES[('wide-span', 'source.ox')])
            self.assertEqual(reads.read_wide(('shared',)), SHARED_BYTES)
            return c.SOURCE_OUTCOME
        def raw_probe(reads):
            self.calls.append('raw'); self.capabilities.append(reads)
            self.assertEqual(reads.read_outer(('wide-span', 'raw.stdout')), b'fixed raw\n')
            self.assertEqual(reads.read_wide(('shared',)), SHARED_BYTES)
            return c.RAW_OUTCOME
        binding = c._trusted_binding(source or source_probe, raw or raw_probe)
        self.addCleanup(c._bindings.pop, id(binding), None)
        return binding

    def invoke_error(self, binding=None, fixture=None):
        try:
            c.inspect_fixture_pair(fixture or self.fixture, binding or self.binding(), self.owner)
        except BaseException as error:
            self.assertEqual(error.fixture_result.state, 'CLOSED')
            self.assertIs(error.fixture_result.qualified, False)
            self.assertIs(error.fixture_result.fixture_only, True)
            return error
        self.fail('fixture unexpectedly succeeded')

    def test_success_one_actual_view_same_dispatch_and_no_core_fallback(self):
        views, phases, reads = [], [], []
        native = pc.read_regular
        def observe(phase, **values):
            phases.append(phase)
            if phase == 'view_admitted':
                views.append(values['view'])
        def checked(anchor, parts, expected):
            reads.append((anchor.path, parts, expected))
            return native(anchor, parts, expected)
        original_support = c._support
        def support(fixture):
            v5, core = original_support(fixture)
            core.regular = lambda path: self.fail('retained reopening reader called')
            return v5, core
        with patch.object(c, '_observe', observe), patch.object(pc, 'read_regular', checked), patch.object(c, '_support', support):
            result = c.inspect_fixture_pair(self.fixture, self.binding(), self.owner)
        self.assertEqual(self.calls, ['source', 'raw'])
        self.assertIs(self.capabilities[0], self.capabilities[1])
        self.assertEqual(len(views), 1)
        self.assertEqual(phases.count('view_construct'), 1)
        self.assertEqual(len(reads), 15)  # five initial + five probe + five final
        self.assertEqual(tuple(x[1] for x in result.probe_outcomes), (c.SOURCE_OUTCOME, c.RAW_OUTCOME))
        self.assertTrue(all(x.status == 'passed' for x in result.finalization))
        self.assertEqual(result.state, 'CLOSED')
        self.assertIs(result.qualified, False); self.assertIs(result.fixture_only, True)
        self.assertFalse(Path(self.owner.owned_root).exists())
        self.assertTrue(self.source.is_dir() and self.evidence.is_dir())

    def test_native_adapter_exact_membership_and_no_fallback(self):
        def observe(phase, **values):
            if phase == 'view_admitted':
                native = values['view']._native_read
                for unknown in (self.source / 'unknown', self.evidence / 'wide-span2' / 'shared'):
                    with self.assertRaises(pc.PhysicalReject): native(unknown)
                with self.assertRaises(pc.PhysicalReject): native(str(self.source / 'helper.py'))
        with patch.object(c, '_observe', observe):
            c.inspect_fixture_pair(self.fixture, self.binding(), self.owner)

    def test_escaped_bound_method_revoked_during_all_finalization_and_after_return(self):
        saved, phases = [], []
        def source(reads):
            saved.append(reads.read_source)
            self.assertEqual({x for x in dir(reads) if not x.startswith('_')},
                             {'read_source', 'read_wide', 'read_outer'})
            for invalid in (['helper.py'], ('..',), (), ('missing',)):
                with self.assertRaises((pc.PhysicalReject, ValueError)): reads.read_source(invalid)
            return c.SOURCE_OUTCOME
        def observe(phase, **values):
            if phase in ('revoke','layout','extra_metadata','source_final','evidence_final','resource_close','cleanup'):
                phases.append(phase)
                with self.assertRaisesRegex(pc.PhysicalReject, 'revoked'): saved[0](('helper.py',))
        with patch.object(c, '_observe', observe):
            c.inspect_fixture_pair(self.fixture, self.binding(source=source), self.owner)
        with self.assertRaisesRegex(pc.PhysicalReject, 'revoked'): saved[0](('helper.py',))
        self.assertEqual(phases, ['revoke','layout','extra_metadata','source_final','evidence_final','resource_close','cleanup'])

    def test_source_failure_skips_raw_and_preserves_original(self):
        original = RuntimeError('source failed')
        def source(reads): raise original
        error = self.invoke_error(self.binding(source=source))
        self.assertEqual(error.fixture_failures, (('source_probe', original),))
        self.assertEqual(self.calls, [])

    def test_raw_failure_preserves_source_outcome(self):
        original = RuntimeError('raw failed')
        def raw(reads): raise original
        error = self.invoke_error(self.binding(raw=raw))
        self.assertEqual(error.fixture_failures, (('raw_probe', original),))
        self.assertEqual(error.fixture_result.probe_outcomes, (('source_probe', c.SOURCE_OUTCOME),))

    def test_fixed_outcome_required(self):
        def source(reads): return replace(c.SOURCE_OUTCOME)
        error = self.invoke_error(self.binding(source=source))
        self.assertEqual(error.fixture_failures[0][0], 'source_probe')
        self.assertIn('fixed immutable', str(leaves(error)))

    def test_unregistered_binding_rejects_before_probe(self):
        trusted = self.binding()
        unregistered = c.FixtureBindingV2(trusted.schema, trusted.source_probe, trusted.raw_probe)
        self.invoke_error(unregistered)
        self.assertEqual(self.calls, [])

    def test_fixture_subclass_rejects_without_owner_cleanup(self):
        class Substitute(c.FixtureV2): pass
        substitute = Substitute(**vars(self.fixture))
        self.invoke_error(fixture=substitute)
        self.assertEqual(self.calls, [])
        self.assertTrue(Path(self.owner.owned_root).exists())

    def test_partial_initial_failure_still_seals_both_scopes(self):
        original = OSError('initial body refused')
        actual = pc.read_regular
        visits = []
        def reader(anchor, parts, expected):
            visits.append((anchor.path, parts))
            if len(visits) == 1: raise original
            return actual(anchor, parts, expected)
        with patch.object(pc, 'read_regular', reader):
            error = self.invoke_error()
        self.assertIn(original, leaves(error))
        self.assertEqual(len(visits), 10)
        self.assertEqual(self.calls, [])
        results = {r.phase: r.status for r in error.fixture_result.finalization}
        self.assertEqual(results['layout'], 'unavailable')
        self.assertEqual(results['source_final'], 'passed')
        self.assertEqual(results['evidence_final'], 'passed')

    def test_failure_before_scope_establishment_keeps_independent_scope(self):
        malformed = replace(self.fixture, outer_rows=(OUTER_ROWS[1],))
        error = self.invoke_error(fixture=malformed)
        results = {r.phase: r.status for r in error.fixture_result.finalization}
        self.assertEqual(results['source_final'], 'passed')
        self.assertEqual(results['evidence_final'], 'unavailable')
        self.assertEqual(error.fixture_failures[0][0], 'admission_evidence')

    def test_malformed_absence_keeps_independent_source_scope(self):
        error = self.invoke_error(fixture=replace(self.fixture, known_absent=[]))
        results = {r.phase: r.status for r in error.fixture_result.finalization}
        self.assertEqual(results['source_final'], 'passed')
        self.assertEqual(results['evidence_final'], 'unavailable')

    def test_partial_evidence_initial_failure_still_attempts_every_final_body(self):
        actual = pc.read_regular
        visits = []
        failure = OSError('evidence body refused')
        def reader(anchor, parts, expected):
            visits.append((anchor.path, parts))
            if len(visits) == 3: raise failure
            return actual(anchor, parts, expected)
        with patch.object(pc, 'read_regular', reader): error = self.invoke_error()
        self.assertIn(failure, leaves(error))
        self.assertEqual(len(visits), 10)
        self.assertEqual(self.calls, [])
        self.assertEqual({r.phase:r.status for r in error.fixture_result.finalization}['evidence_final'], 'passed')

        fresh = CallerControls(methodName=self._testMethodName)
        fresh.setUp()
        try:
            target = fresh.evidence / 'wide-span' / 'raw.stdout'
            missing_inode = target.stat().st_ino
            sibling_id = pc.file_identity((fresh.evidence / 'sibling').stat())
            original = OSError('initial metadata identity refused')
            failed, visits, snapshots, body_reads = [], [], [], []
            actual_identity, actual_reader, actual_read = pc.file_identity, pc.read_regular, os.read
            def identity(info):
                if info.st_ino == missing_inode and not failed:
                    failed.append(original)
                    raise original
                return actual_identity(info)
            def reader(anchor, parts, expected):
                visits.append((anchor.path, parts))
                if anchor.path == str(fresh.evidence):
                    snapshots.append(anchor.files)
                return actual_reader(anchor, parts, expected)
            def read(fd, count):
                if os.fstat(fd).st_ino == missing_inode:
                    body_reads.append(count)
                return actual_read(fd, count)
            with patch.object(pc, 'file_identity', identity), patch.object(pc, 'read_regular', reader), \
                    patch.object(pc.os, 'read', read):
                error = fresh.invoke_error()
            self.assertIn(original, leaves(error))
            self.assertEqual(error.fixture_failures[0][0], 'admission_evidence')
            self.assertEqual(fresh.calls, [])
            self.assertEqual(len(visits), 10)
            self.assertEqual(len(snapshots), 8)
            self.assertTrue(all(value == ((('sibling',), sibling_id),) for value in snapshots))
            self.assertEqual(body_reads, [])
            self.assertIn('file baseline was not established', str(leaves(error)))
            statuses = {r.phase:r.status for r in error.fixture_result.finalization}
            self.assertEqual(statuses['source_final'], 'passed')
            self.assertEqual(statuses['evidence_final'], 'failed')
            self.assertEqual(statuses['cleanup'], 'passed')
            self.assertEqual(statuses['resource_close'], 'passed')
            self.assertFalse(Path(fresh.owner.owned_root).exists())
        finally:
            fresh.doCleanups()

    def test_view_construction_failure_zero_admitted_views_with_full_finalization(self):
        views = []
        original = RuntimeError('construct refused')
        def observe(phase, **values):
            if phase == 'view_construct': raise original
            if phase == 'view_admitted': views.append(values['view'])
        with patch.object(c, '_observe', observe): error = self.invoke_error()
        self.assertEqual(views, [])
        statuses = {r.phase: r.status for r in error.fixture_result.finalization}
        self.assertEqual(statuses['layout'], 'unavailable')
        self.assertEqual(statuses['source_final'], 'passed'); self.assertEqual(statuses['evidence_final'], 'passed')

    def test_full_outer_unconsumed_body_mutation_is_detected(self):
        def raw(reads):
            (self.evidence / 'sibling').write_bytes(b'X' * len(BODIES[('sibling',)]))
            return c.RAW_OUTCOME
        error = self.invoke_error(self.binding(raw=raw))
        self.assertIn('evidence_final', [label for label, _ in error.fixture_failures])

        for scope in ('source', 'evidence'):
            for stage in ('view_construct', 'view_admitted', 'extra_metadata',
                          'initial_before', 'initial_after', 'final_before', 'final_after'):
                with self.subTest(scope=scope, stage=stage):
                    fresh = CallerControls(methodName=self._testMethodName)
                    fresh.setUp()
                    try:
                        root = fresh.source if scope == 'source' else fresh.evidence
                        parts = ('helper.py',) if scope == 'source' else ('sibling',)
                        target = root.joinpath(*parts)
                        original_id = pc.file_identity(target.stat())
                        replacement = fresh.root / 'replacement'
                        replacement.write_bytes(BODIES[parts]); replacement.chmod(0o600)
                        replacement_id = pc.file_identity(replacement.stat())
                        events, visits, replacement_reads, snapshots = [], [], [], []
                        active = [None]
                        swapped = [False]
                        actual_reader, actual_read = pc.read_regular, os.read
                        def swap():
                            os.replace(replacement, target)
                            swapped[0] = True
                        def observe(phase, **values):
                            active[0] = phase
                            events.append(phase)
                            if phase == stage:
                                swap()
                        def reader(anchor, path, expected):
                            visits.append((anchor.path, path))
                            match = anchor.path == str(root) and path == parts
                            if match:
                                snapshots.append(dict(anchor.files).get(parts))
                            phase = scope + ('_initial' if stage.startswith('initial_') else '_final')
                            inject = match and active[0] == phase and not swapped[0]
                            if inject and stage.endswith('_before'):
                                swap()
                            body = actual_reader(anchor, path, expected)
                            if inject and stage.endswith('_after'):
                                swap()
                            return body
                        def read(fd, count):
                            info = os.fstat(fd)
                            if (info.st_dev, info.st_ino) == replacement_id[:2]:
                                replacement_reads.append((fd, count))
                            return actual_read(fd, count)
                        with patch.object(c, '_observe', observe), patch.object(pc, 'read_regular', reader), \
                                patch.object(pc.os, 'read', read):
                            error = fresh.invoke_error()
                        self.assertTrue(swapped[0])
                        self.assertNotEqual(original_id[:2], replacement_id[:2])
                        self.assertEqual(replacement_reads, [])
                        self.assertTrue(snapshots)
                        self.assertTrue(all(identity == original_id for identity in snapshots))
                        expected_visits = 10 if stage.startswith('initial_') else 15
                        if scope == 'source' and stage in ('view_construct', 'view_admitted'):
                            expected_visits = 11 if stage == 'view_construct' else 10
                        self.assertEqual(len(visits), expected_visits)
                        self.assertEqual(visits[-5:], [(str(fresh.source), ('helper.py',))] +
                                         [(str(fresh.evidence), row.path) for row in OUTER_ROWS if row.kind == 'file'])
                        statuses = {r.phase:r.status for r in error.fixture_result.finalization}
                        self.assertEqual(statuses[scope + '_final'], 'failed')
                        self.assertEqual(statuses[('evidence' if scope == 'source' else 'source') + '_final'], 'passed')
                        self.assertEqual(statuses['cleanup'], 'passed')
                        self.assertEqual(statuses['resource_close'], 'passed')
                        self.assertEqual(events.count('cleanup'), 1)
                        self.assertEqual(events[-1], 'cleanup')
                        self.assertEqual(error.fixture_result.state, 'CLOSED')
                        self.assertFalse(Path(fresh.owner.owned_root).exists())
                        self.assertEqual(target.read_bytes(), BODIES[parts])
                        self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o600)
                        if stage.startswith('initial_'):
                            self.assertEqual(fresh.calls, [])
                            self.assertEqual(error.fixture_failures[0][0], 'admission_' + scope)
                    finally:
                        fresh.doCleanups()

    def test_membership_and_known_absence_mutations_detected(self):
        def raw(reads):
            (self.evidence / 'missing').write_bytes(b'extra')
            return c.RAW_OUTCOME
        error = self.invoke_error(self.binding(raw=raw))
        labels = [label for label, _ in error.fixture_failures]
        self.assertIn('layout', labels); self.assertIn('extra_metadata', labels); self.assertIn('evidence_final', labels)

    def test_initial_hardlink_rejects_before_consumers(self):
        os.link(self.source / 'helper.py', self.root / 'external-link')
        error = self.invoke_error()
        self.assertEqual(self.calls, [])
        self.assertIn('link count', str(leaves(error)))

    def test_nested_directory_replacement_never_reads_replacement_body(self):
        actual = os.read
        replacement_inode = []
        replacement_reads = []
        def source(reads):
            wide = self.evidence / 'wide-span'
            wide.rename(self.evidence / 'old-wide')
            wide.mkdir(mode=0o700)
            for row in OUTER_ROWS:
                if row.path[:1] == ('wide-span',) and row.kind == 'file':
                    path = self.evidence.joinpath(*row.path)
                    path.write_bytes(BODIES[row.path]); path.chmod(row.mode)
                    replacement_inode.append(path.stat().st_ino)
            return c.SOURCE_OUTCOME
        def read(fd, count):
            if os.fstat(fd).st_ino in replacement_inode:
                replacement_reads.append((fd, count))
            return actual(fd, count)
        with patch.object(pc.os, 'read', read):
            error = self.invoke_error(self.binding(source=source))
        self.assertTrue(error.fixture_failures)
        self.assertEqual(replacement_reads, [])

    def test_cancellation_same_object_after_cleanup_and_closed(self):
        cancellation = c.FixtureCancellation('stop')
        def source(reads): raise cancellation
        error = self.invoke_error(self.binding(source=source))
        self.assertIs(error, cancellation)
        self.assertFalse(Path(self.owner.owned_root).exists())
        self.assertIsNone(error.__cause__)

    def test_preservation_cancellation_with_ordered_other_failures(self):
        cancellation = c.FixtureCancellation('preservation stop')
        primary = ValueError('probe failure')
        cleanup = OSError('cleanup failure')
        events = []
        def source(reads): raise primary
        def observe(phase, **values):
            if phase in ('revoke','layout','extra_metadata','source_final','evidence_final','resource_close','cleanup'):
                events.append(phase)
            if phase == 'source_final': raise cancellation
        with patch.object(c, '_observe', observe), patch.object(ownerlib, 'cleanup', side_effect=cleanup):
            error = self.invoke_error(self.binding(source=source))
        self.assertIs(error, cancellation)
        self.assertEqual(error.fixture_failures, (('source_probe', primary), ('source_final', cancellation), ('cleanup', cleanup)))
        self.assertEqual(leaves(error.__cause__), [primary, cleanup])
        self.assertEqual(events, ['revoke','layout','extra_metadata','source_final','evidence_final','resource_close','cleanup'])

    def test_first_cancellation_identity_wins_over_later_cancellation(self):
        first, later = KeyboardInterrupt('first'), SystemExit(7)
        def source(reads): raise first
        def observe(phase, **values):
            if phase == 'evidence_final': raise later
        with patch.object(c, '_observe', observe): error = self.invoke_error(self.binding(source=source))
        self.assertIs(error, first)
        self.assertEqual(leaves(error.__cause__), [later])

    def test_anchor_resources_close_before_cleanup_even_when_close_reports_error(self):
        actual = os.close
        sessions, close_errors = [], []
        def observe(phase, **values):
            if phase == 'resource_close': sessions.append(values['session'])
        def close(fd):
            actual(fd)
            if sessions and sessions[-1].state == 'REVOKED' and len(close_errors) < 2:
                error = OSError('anchor close reported failure'); close_errors.append(error); raise error
        with patch.object(c, '_observe', observe), patch.object(c.os, 'close', close): error = self.invoke_error()
        self.assertEqual(len(close_errors), 2)
        for original in close_errors: self.assertIn(original, leaves(error))
        self.assertIsNone(sessions[-1].view)
        self.assertFalse(Path(self.owner.owned_root).exists())

    def test_all_finalization_failures_do_not_skip_actual_cleanup_or_closure(self):
        errors = {name: RuntimeError(name) for name in ('revoke','layout','extra_metadata','source_final','evidence_final','resource_close','cleanup')}
        def observe(phase, **values):
            if phase in errors: raise errors[phase]
        with patch.object(c, '_observe', observe): error = self.invoke_error()
        self.assertEqual(error.fixture_failures, tuple(errors.items()))
        self.assertFalse(Path(self.owner.owned_root).exists())
        self.assertEqual(error.fixture_result.state, 'CLOSED')

    def test_replaced_owned_root_untouched_and_cleanup_once_last(self):
        owned = Path(self.owner.owned_root)
        old = self.root / 'original-owned'
        events = []
        actual = ownerlib.cleanup
        def raw(reads):
            owned.rename(old)
            owned.mkdir(mode=0o700)
            (owned / 'sentinel').write_bytes(b'untouched')
            return c.RAW_OUTCOME
        def observe(phase, **values): events.append(phase)
        with patch.object(c, '_observe', observe), patch.object(ownerlib, 'cleanup', wraps=actual) as cleanup:
            error = self.invoke_error(self.binding(raw=raw))
        self.assertEqual(cleanup.call_count, 1)
        self.assertEqual(events[-1], 'cleanup')
        self.assertEqual((owned / 'sentinel').read_bytes(), b'untouched')
        self.assertTrue(old.exists())
        self.assertEqual(error.fixture_failures[-1][0], 'cleanup')

    def test_ownership_physical_ancestor_aliases_reject_before_cleanup(self):
        roots = (str(self.source), str(self.evidence), self.owner.owned_root)
        identities = tuple(pc.directory_identity(os.stat(root)) for root in roots)
        actual_open = pc.open_anchor
        # All six ordered pairs, including owner-under-source and its inverse.
        # Synthetic chains retain distinct leaf IDs and lexically disjoint roots.
        for ancestor in range(3):
            for descendant in range(3):
                if ancestor == descendant:
                    continue
                with self.subTest(ancestor=ancestor, descendant=descendant):
                    acquired = []
                    def open_anchor(path):
                        anchor = actual_open(path)
                        acquired.append(anchor.fd)
                        if path == roots[descendant]:
                            anchor = replace(anchor, chain=anchor.chain[:-1] +
                                (('/synthetic-alias', identities[ancestor]), anchor.chain[-1]))
                        return anchor
                    with patch.object(pc, 'open_anchor', open_anchor), \
                            patch.object(ownerlib, 'cleanup', wraps=ownerlib.cleanup) as cleanup:
                        error = self.invoke_error()
                    self.assertIn('physically overlapping roots', str(leaves(error)))
                    self.assertEqual(self.calls, [])
                    self.assertEqual(cleanup.call_count, 0)
                    statuses = {r.phase: r.status for r in error.fixture_result.finalization}
                    self.assertEqual(statuses['cleanup'], 'unavailable')
                    self.assertTrue(Path(self.owner.owned_root).is_dir())
                    self.assertEqual(list(Path(self.owner.owned_root).iterdir()), [])
                    self.assertEqual(len(acquired), 3)
                    for fd in acquired:
                        with self.assertRaises(OSError): os.fstat(fd)

    def test_ownership_shared_ordinary_ancestors_pass_and_close_anchors(self):
        actual_open = pc.open_anchor
        acquired = []
        def open_anchor(path):
            anchor = actual_open(path)
            acquired.append(anchor)
            return anchor
        with patch.object(pc, 'open_anchor', open_anchor):
            ownerlib.validate(self.owner, str(self.source), str(self.evidence))
        self.assertEqual(len(acquired), 3)
        self.assertEqual(len({anchor.identity[:2] for anchor in acquired}), 3)
        self.assertTrue(all(anchor.chain[:-1] == acquired[0].chain[:-1] for anchor in acquired))
        for anchor in acquired:
            with self.assertRaises(OSError): os.fstat(anchor.fd)

    def test_ownership_partial_anchor_failure_closes_acquired_descriptors(self):
        actual_open = pc.open_anchor
        for fail_at in (0, 1, 2):
            with self.subTest(fail_at=fail_at):
                acquired = []
                original = OSError('ownership anchor unavailable')
                def open_anchor(path):
                    if len(acquired) == fail_at:
                        raise original
                    anchor = actual_open(path)
                    acquired.append(anchor.fd)
                    return anchor
                with patch.object(pc, 'open_anchor', open_anchor), \
                        patch.object(ownerlib, 'cleanup', wraps=ownerlib.cleanup) as cleanup:
                    error = self.invoke_error()
                self.assertIn(original, leaves(error))
                self.assertEqual(cleanup.call_count, 0)
                self.assertEqual(self.calls, [])
                self.assertEqual(len(acquired), fail_at)
                self.assertTrue(Path(self.owner.owned_root).is_dir())
                self.assertEqual({r.phase:r.status for r in error.fixture_result.finalization}['cleanup'], 'unavailable')
                for fd in acquired:
                    with self.assertRaises(OSError): os.fstat(fd)

    def test_ownership_close_failure_still_closes_remaining_anchors(self):
        actual_open, actual_close = pc.open_anchor, os.close
        acquired, closed = [], []
        original = OSError('validation close reported failure')
        def open_anchor(path):
            anchor = actual_open(path)
            acquired.append(anchor.fd)
            return anchor
        def close(fd):
            actual_close(fd)
            if len(acquired) == 3:
                closed.append(fd)
                if len(closed) == 1:
                    raise original
        with patch.object(pc, 'open_anchor', open_anchor), patch.object(ownerlib.os, 'close', close), \
                self.assertRaises(BaseExceptionGroup) as caught:
            ownerlib.validate(self.owner, str(self.source), str(self.evidence))
        self.assertIn(original, leaves(caught.exception))
        self.assertEqual(closed, list(reversed(acquired)))
        for fd in acquired:
            with self.assertRaises(OSError): os.fstat(fd)

    def test_cleanup_parent_open_failure_retains_original_and_leftover_path(self):
        original = OSError('cleanup parent unavailable')
        with patch.object(pc, 'open_anchor', side_effect=original) as opened, \
                patch.object(ownerlib.os, 'rmdir') as remove, \
                self.assertRaises(BaseExceptionGroup) as caught:
            ownerlib.cleanup(self.owner)
        self.assertEqual(leaves(caught.exception), [original])
        self.assertIs(leaves(caught.exception)[0], original)
        self.assertIn('Fixture cleanup target (harness must inspect any leftover): ' + self.owner.owned_root,
                      original.__notes__)
        opened.assert_called_once_with(str(Path(self.owner.owned_root).parent))
        self.assertEqual(remove.call_count, 0)
        self.assertTrue(Path(self.owner.owned_root).is_dir())

    def test_invalid_owner_never_deletes(self):
        forged = replace(self.owner)
        with self.assertRaises(BaseExceptionGroup) as caught:
            c.inspect_fixture_pair(self.fixture, self.binding(), forged)
        self.assertTrue(Path(self.owner.owned_root).exists())
        cleanup = next(r for r in caught.exception.fixture_result.finalization if r.phase == 'cleanup')
        self.assertEqual(cleanup.status, 'unavailable')


class PhysicalReaderControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='oxid-reader-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.body = getattr(self, 'fixture_body', bytes(range(251)) * 600)
        self.path = self.root / 'body'
        self.path.write_bytes(self.body); self.path.chmod(0o600)
        self.expected = pc.ExpectedFile(0o600, len(self.body), hashlib.sha256(self.body).hexdigest())
        self.anchor = pc.open_anchor(str(self.root))
        self.anchor = replace(self.anchor, files=((('body',), pc.file_identity(self.path.stat())),))
        self.addCleanup(os.close, self.anchor.fd)

    def read(self, expected=None):
        return pc.read_regular(self.anchor, ('body',), expected or self.expected)

    def test_same_fd_and_expected_plus_one_bound(self):
        reads, opened, closed, observations = [], [], [], []
        actual_read, actual_open, actual_close = os.read, os.open, os.close
        def read(fd, count): reads.append((fd, count)); return actual_read(fd, count)
        def opened_file(*args, **kwargs):
            fd = actual_open(*args, **kwargs); opened.append(fd); return fd
        def close(fd): closed.append(fd); return actual_close(fd)
        with patch.object(pc, 'supported'), patch.object(pc.os, 'read', read), patch.object(pc.os, 'open', opened_file), patch.object(pc.os, 'close', close), \
                patch.object(pc, '_observe', lambda phase, **kw: observations.append(phase)):
            self.assertEqual(self.read(), self.body)
        self.assertEqual(len(opened), 1); self.assertEqual(closed, opened)
        self.assertEqual({fd for fd, _ in reads}, set(opened))
        self.assertEqual([count for _, count in reads], [65536,65536,len(self.body)-131072,1])
        self.assertEqual(sum(count for _, count in reads), len(self.body) + 1)
        self.assertEqual(observations, ['pre','fd_before','fd_after','post'])
        with self.assertRaises(OSError): os.fstat(opened[0])

    def test_nlink_required_at_all_four_observations(self):
        actual_stat, actual_fstat, actual_read = os.stat, os.fstat, os.read
        for stage in ('pre','fd_before','fd_after','post'):
            with self.subTest(stage=stage):
                calls = {'stat':0,'fstat':0,'read':0}
                def corrupt(info):
                    keys = ('st_dev','st_ino','st_mode','st_nlink','st_uid','st_gid','st_size','st_mtime_ns','st_ctime_ns')
                    values = {key:getattr(info,key) for key in keys}; values['st_nlink']=2
                    return SimpleNamespace(**values)
                def name_stat(name, *args, **kwargs):
                    info = actual_stat(name, *args, **kwargs)
                    if name == 'body':
                        calls['stat'] += 1
                        if (stage == 'pre' and calls['stat']==1) or (stage == 'post' and calls['stat']==2): return corrupt(info)
                    return info
                def fd_stat(fd):
                    info = actual_fstat(fd)
                    if stat.S_ISREG(info.st_mode):
                        calls['fstat'] += 1
                        if (stage == 'fd_before' and calls['fstat']==1) or (stage == 'fd_after' and calls['fstat']==2): return corrupt(info)
                    return info
                def read(fd, count): calls['read'] += 1; return actual_read(fd,count)
                # Patching os.stat changes function identity; platform support is
                # separately tested, while these controls inject observations.
                with patch.object(pc, 'supported'), patch.object(pc.os,'stat',name_stat), patch.object(pc.os,'fstat',fd_stat), \
                        patch.object(pc.os,'read',read), self.assertRaises(BaseExceptionGroup) as caught:
                    self.read()
                self.assertIn('link count', str(leaves(caught.exception)))
                if stage in ('pre','fd_before'): self.assertEqual(calls['read'],0)

    def test_read_failures_still_attempt_fd_name_ancestor_checks_and_close(self):
        actual_read, actual_close = os.read, os.close
        for mode in ('early','extra','eagain','hash','cancel','close'):
            with self.subTest(mode=mode):
                observations, closed = [], []
                cancellation = c.FixtureCancellation('reader cancellation')
                close_error = OSError('close failed after close')
                def read(fd,count):
                    if mode=='early':return b''
                    if mode=='eagain':raise BlockingIOError(errno.EAGAIN,'not ready')
                    if mode=='cancel':raise cancellation
                    if mode=='extra' and count==1:return b'x'
                    return actual_read(fd,count)
                def close(fd):
                    closed.append(fd);actual_close(fd)
                    if mode=='close':raise close_error
                expected = replace(self.expected,sha256='0'*64) if mode=='hash' else self.expected
                with patch.object(pc.os,'read',read), patch.object(pc.os,'close',close), \
                        patch.object(pc,'_observe',lambda phase,**kw:observations.append(phase)), \
                        self.assertRaises(BaseExceptionGroup) as caught:
                    self.read(expected)
                self.assertIn('fd_after',observations);self.assertIn('post',observations)
                self.assertEqual(len(closed),1)
                with self.assertRaises(OSError):os.fstat(closed[0])
                if mode=='cancel':self.assertIn(cancellation,leaves(caught.exception))
                if mode=='close':self.assertIn(close_error,leaves(caught.exception))

    def test_size_mismatch_refuses_before_open_or_body(self):
        with patch.object(pc.os,'read',side_effect=AssertionError('body read')) as read, \
                patch.object(pc.os,'open',side_effect=AssertionError('file open')) as opened, self.assertRaises(BaseExceptionGroup):
            self.read(replace(self.expected,bytes=len(self.body)-1))
        self.assertEqual(read.call_count, 0)
        self.assertEqual(opened.call_count, 0)

        baseline = dict(self.anchor.files)[('body',)]
        for changed_field in range(10):
            with self.subTest(changed_field=changed_field):
                altered = list(baseline)
                if changed_field < 9:
                    altered[changed_field] += 1
                files = ((('body',), tuple(altered)),) if changed_field < 9 else ()
                anchor = replace(self.anchor, files=files)
                observations = []
                with patch.object(pc.os, 'read', wraps=os.read) as read, \
                        patch.object(pc, '_observe', lambda phase, **kw: observations.append(phase)), \
                        self.assertRaises(BaseExceptionGroup) as caught:
                    pc.read_regular(anchor, ('body',), self.expected)
                self.assertEqual(read.call_count, 0)
                self.assertEqual(observations, ['pre', 'post'])
                self.assertIn('admitted file identity changed' if files else 'file baseline was not established',
                              str(leaves(caught.exception)))
        cancellation = c.FixtureCancellation('postcheck cancellation after baseline refusal')
        def observe(phase, **kw):
            if phase == 'post':
                raise cancellation
        with patch.object(pc, '_observe', observe), self.assertRaises(BaseExceptionGroup) as caught:
            pc.read_regular(replace(self.anchor, files=()), ('body',), self.expected)
        self.assertIn(cancellation, leaves(caught.exception))
        self.assertIn('file baseline was not established', str(leaves(caught.exception)))

    def test_missing_platform_features_reject(self):
        with patch.object(pc.sys,'platform','win32'),self.assertRaises(pc.PhysicalReject):pc.supported()
        with patch.object(pc.os,'supports_fd',set()),self.assertRaises(pc.PhysicalReject):pc.supported()

    def test_empty_file_reads_only_one_eof_sentinel(self):
        fresh = PhysicalReaderControls(methodName=self._testMethodName)
        fresh.fixture_body = b''
        fresh.setUp()
        try:
            actual = os.read
            counts = []
            def read(fd,count):counts.append(count);return actual(fd,count)
            with patch.object(pc.os,'read',read):
                self.assertEqual(fresh.read(pc.ExpectedFile(0o600,0,hashlib.sha256(b'').hexdigest())),b'')
            self.assertEqual(counts,[1])
        finally:
            fresh.doCleanups()

    def test_partial_anchor_traversal_closes_every_opened_fd(self):
        actual_open,actual_close=os.open,os.close
        opened,closed=[],[]
        failure=OSError('directory open failure')
        def opened_file(path,*args,**kwargs):
            if path=='unavailable':raise failure
            fd=actual_open(path,*args,**kwargs);opened.append(fd);return fd
        def close(fd):closed.append(fd);return actual_close(fd)
        (self.root/'unavailable').mkdir()
        with patch.object(pc,'supported'),patch.object(pc.os,'open',opened_file),patch.object(pc.os,'close',close), \
                self.assertRaises(BaseExceptionGroup) as caught:
            pc.open_anchor(str(self.root/'unavailable'))
        self.assertIn(failure,leaves(caught.exception))
        self.assertEqual(closed,list(reversed(opened)))
        for fd in opened:
            with self.assertRaises(OSError):os.fstat(fd)

    def test_symlink_and_fifo_rejected_before_body_read(self):
        self.path.unlink()
        for kind in ('link','fifo'):
            with self.subTest(kind=kind):
                if kind=='link':self.path.symlink_to(self.root/'missing')
                else:os.mkfifo(self.path,0o600)
                with patch.object(pc.os,'read',side_effect=AssertionError('body read')) as read, self.assertRaises(BaseExceptionGroup):self.read()
                self.assertEqual(read.call_count,0)
                self.path.unlink()


class SupportPackagingControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='oxid-support-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / 'support').mkdir()
        self.path = self.root / 'support' / 'module.py'
        self.raw = b'value = "captured bytes"\n'
        self.path.write_bytes(self.raw)
        self.digest = hashlib.sha256(self.raw).hexdigest()
        self.name = '_fixture_packaging_control'
        patched = patch.object(c, '__file__', str(self.root / 'caller.py'))
        patched.start(); self.addCleanup(patched.stop)

    def load(self, relative='support/module.py'):
        return c._load_pinned(relative, self.digest, self.name)

    def test_missing_dependency_has_no_execution_or_fallback(self):
        self.path.unlink()
        with patch('builtins.compile') as compile_source, self.assertRaises(FileNotFoundError):
            self.load()
        self.assertEqual(compile_source.call_count, 0)
        self.assertNotIn(self.name, c.sys.modules)

    def test_altered_dependency_rejected_before_compile(self):
        self.path.write_bytes(b'raise RuntimeError("must not execute")\n')
        with patch('builtins.compile') as compile_source, self.assertRaisesRegex(pc.PhysicalReject, 'digest differs'):
            self.load()
        self.assertEqual(compile_source.call_count, 0)
        self.assertNotIn(self.name, c.sys.modules)

    def test_noncanonical_spellings_rejected_before_filesystem_access(self):
        for relative in ('/support/module.py', './support/module.py', 'support/./module.py',
                         'support/../support/module.py', '../module.py', 'support//module.py',
                         'support/module.py/', 'support\\module.py', 'C:/module.py', '',
                         'support/%2e%2e/module.py', None, Path('support/module.py')):
            with self.subTest(relative=relative):
                with patch.object(Path, 'lstat') as lstat, patch.object(Path, 'read_bytes') as read, \
                        self.assertRaises(pc.PhysicalReject):
                    self.load(relative)
                self.assertEqual(lstat.call_count, 0)
                self.assertEqual(read.call_count, 0)

    def test_file_directory_symlinks_and_hardlinks_rejected(self):
        (self.root / 'alias').symlink_to(self.root / 'support', target_is_directory=True)
        (self.root / 'support' / 'link.py').symlink_to(self.path)
        for relative in ('alias/module.py', 'support/link.py'):
            with self.subTest(relative=relative), patch.object(Path, 'read_bytes') as read, \
                    self.assertRaises(pc.PhysicalReject):
                self.load(relative)
            self.assertEqual(read.call_count, 0)
        os.link(self.path, self.root / 'hardlink.py')
        with patch.object(Path, 'read_bytes') as read, self.assertRaises(pc.PhysicalReject):
            self.load()
        self.assertEqual(read.call_count, 0)

    def test_exact_captured_bytes_compiled_once_from_fixture_directory(self):
        actual_compile = compile
        captured = []
        def compile_source(raw, filename, mode):
            captured.append(raw)
            self.path.write_bytes(b'raise RuntimeError("replacement must not execute")\n')
            return actual_compile(raw, filename, mode)
        with patch.object(Path, 'read_bytes', autospec=True, side_effect=lambda path: self.raw) as read, \
                patch('builtins.compile', compile_source):
            module = self.load()
        self.assertEqual(module.value, 'captured bytes')
        self.assertEqual(captured, [self.raw])
        read.assert_called_once_with(self.path)
        self.assertEqual(module.__file__, str(self.path))
        self.assertNotIn(self.name, c.sys.modules)


if __name__ == '__main__':
    unittest.main(verbosity=2)
