"""Unqualified fixture-only caller. Trusted synchronous harness callbacks only."""
from dataclasses import dataclass, replace
import hashlib
import json
import os
from pathlib import Path, PosixPath
import stat
import sys
from types import FunctionType, MappingProxyType, ModuleType, SimpleNamespace
import physical_checks as pc
import fixture_owner as ownership


class FixtureCancellation(BaseException):
    """The one harness-designated cancellation type, in addition to process exits."""


@dataclass(frozen=True)
class Row:
    path: tuple
    kind: str
    mode: int
    expected: object = None


@dataclass(frozen=True)
class FixtureV2:
    schema: str
    source_root: str
    evidence_root: str
    wide_root: str
    source_rows: tuple
    outer_rows: tuple
    known_absent: tuple
    reader_pin: str


@dataclass(frozen=True)
class FixtureBindingV2:
    schema: str
    source_probe: object
    raw_probe: object


@dataclass(frozen=True)
class FixtureOutcome:
    schema: str
    probe: str
    value: str
    fixture_only: bool = True
    qualified: bool = False


SOURCE_OUTCOME = FixtureOutcome('oxid-v3-local-fixture-probe-outcome-v2', 'source', 'fixed-source-ok')
RAW_OUTCOME = FixtureOutcome('oxid-v3-local-fixture-probe-outcome-v2', 'raw', 'fixed-raw-ok')


@dataclass(frozen=True)
class Finalization:
    phase: str
    status: str
    reason: object = None


@dataclass(frozen=True)
class FixtureResult:
    schema: str
    probe_outcomes: tuple
    finalization: tuple
    state: str
    fixture_only: bool = True
    qualified: bool = False


_bindings = {}


def _trusted_binding(source_probe, raw_probe):
    """Reviewed harness registration, not authentication of arbitrary Python."""
    pc.need(type(source_probe) is FunctionType and type(raw_probe) is FunctionType, 'exact harness functions required')
    binding = FixtureBindingV2('oxid-v3-local-fixture-probes-v2', source_probe, raw_probe)
    _bindings[id(binding)] = binding
    return binding


def _observe(phase, **values):
    """Private test instrumentation. No probe receives this function."""


def _support_path(relative):
    # Validate the spelling before pathlib can normalize separators or dots.
    pc.need(type(relative) is str and bool(relative), 'exact relative support path required')
    parts = tuple(relative.split('/'))
    pc.components(parts)
    path = Path(__file__).parent.joinpath(*parts)
    for parent in reversed(path.parents):
        pc.directory_identity(parent.lstat())
    pc.file_identity(path.lstat())
    return path


def _load_pinned(relative, digest, name):
    path = _support_path(relative)
    raw = Path(path).read_bytes()
    pc.need(hashlib.sha256(raw).hexdigest() == digest, 'reviewed support digest differs: ' + name)
    module = ModuleType(name)
    module.__file__ = str(path)
    pc.need(name not in sys.modules, 'support module name already occupied')
    sys.modules[name] = module
    try:
        exec(compile(raw, str(path), 'exec'), module.__dict__)
    finally:
        del sys.modules[name]
    return module


def _support(fixture):
    pins = json.loads(Path(__file__).with_name('fixed-inputs.json').read_text())
    for name, digest in pins['local_support_sha256'].items():
        pc.need(name in ('caller.py', 'physical_checks.py', 'fixture_owner.py', 'test_caller.py'),
                'unknown local test support')
        pc.need(hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest() == digest,
                'local support digest differs: ' + name)
    reader = Path(pc.__file__).read_bytes()
    pc.need(fixture.reader_pin == pins['physical_checks_sha256'] == hashlib.sha256(reader).hexdigest(),
            'physical reader digest differs')
    view = _load_pinned(pins['v5']['path'], pins['v5']['sha256'], '_fixture_reviewed_v5')
    core = _load_pinned(pins['core']['path'], pins['core']['sha256'], '_fixture_reviewed_core')
    return view, core


def _rows(rows):
    pc.need(type(rows) is tuple and bool(rows), 'exact nonempty row tuple required')
    for row in rows:
        pc.need(type(row) is Row, 'exact immutable row required')
        pc.components(row.path)
        pc.need(type(row.mode) is int and 0 <= row.mode <= 0o7777, 'invalid mode')
        pc.need(type(row.kind) is str and row.kind in ('file', 'directory'), 'fixture row kind')
        if row.kind == 'directory':
            pc.need(row.expected is None, 'directory carries body fields')
        else:
            expected = row.expected
            pc.need(type(expected) is pc.ExpectedFile and type(expected.permission_mode) is int and expected.permission_mode == row.mode,
                    'exact expected file required')
            pc.need(type(expected.bytes) is int and expected.bytes >= 0, 'invalid expected size')
            pc.need(type(expected.sha256) is str and len(expected.sha256) == 64 and
                    set(expected.sha256) <= set('0123456789abcdef'), 'invalid expected hash')
    paths = tuple(row.path for row in rows)
    pc.need(paths == tuple(sorted(set(paths))), 'unordered/duplicate rows')
    index = {row.path: row for row in rows}
    pc.need(() in index and index[()].kind == 'directory', 'directory root required')
    for path in paths:
        if path:
            pc.need(path[:-1] in index and index[path[:-1]].kind == 'directory', 'missing directory ancestor')
    return rows


class _Scope:
    def __init__(self, root, rows, anchor):
        self.root, self.rows, self.anchor = root, rows, anchor
        self.directory_ids = {(): anchor.identity}
        self.file_ids = {}
        self.initial = None


def _metadata(scope, *, establish=False):
    pc.check_anchor(scope.anchor)
    index = {row.path: row for row in scope.rows}
    for row in scope.rows:
        path = Path(scope.root).joinpath(*row.path)
        info = path.lstat()
        pc.need(stat.S_IMODE(info.st_mode) == row.mode, 'scope permission mode changed')
        if row.kind == 'directory':
            identity = pc.directory_identity(info)
            if establish:
                scope.directory_ids[row.path] = identity
            if row.path in scope.directory_ids:
                pc.need(identity == scope.directory_ids[row.path], 'scope directory identity changed')
            names = frozenset(p.name for p in path.iterdir())
            expected = frozenset(p[-1] for p in index if p and p[:-1] == row.path)
            pc.need(names == expected, 'scope directory membership changed')
        else:
            identity = pc.file_identity(info)
            pc.need(info.st_size == row.expected.bytes, 'scope file size changed')
            if establish and row.path not in scope.file_ids:
                scope.file_ids[row.path] = identity
            pc.need(row.path in scope.file_ids, 'file baseline was not established')
            pc.need(identity == scope.file_ids[row.path], 'scope file identity changed')
    pc.check_anchor(scope.anchor)


def _seal(scope, *, initial=False):
    errors, bodies = [], []
    try:
        _metadata(scope, establish=initial)
    except BaseException as error:
        errors.append(error)
    if initial:
        scope.anchor = replace(scope.anchor, directories=tuple(sorted(scope.directory_ids.items())),
                               files=tuple(sorted(scope.file_ids.items())))
    # Every expected body is attempted, including when an earlier body failed.
    for row in scope.rows:
        if row.kind == 'file':
            try:
                body = pc.read_regular(scope.anchor, row.path, row.expected)
                bodies.append((row.path, dict(scope.anchor.files)[row.path], hashlib.sha256(body).hexdigest()))
            except BaseException as error:
                errors.append(error)
    try:
        _metadata(scope)
    except BaseException as error:
        errors.append(error)
    pc._finish(errors)
    sealed = tuple(bodies)
    if scope.initial is not None:
        pc.need(sealed == scope.initial, 'final seal differs from initial seal')
    if initial:
        scope.initial = sealed


def _absence(scope, paths):
    pc.check_anchor(scope.anchor)
    for parts in paths:
        try:
            Path(scope.root).joinpath(*parts).lstat()
        except FileNotFoundError:
            continue
        raise pc.PhysicalReject('known-absent name appeared')
    pc.check_anchor(scope.anchor)


class _Session:
    def __init__(self):
        self.state = 'ADMITTING'
        self.view = None
        self.roots = None

    def read(self, root, parts):
        pc.need(self.state == 'CONSUMING', 'fixture capability revoked')
        pc.components(parts)
        pc.need(bool(parts), 'file component required')
        base = self.roots[root]
        return self.view.regular(base + '/' + '/'.join(parts))


class FixtureReads:
    __slots__ = ('__session',)

    def __init__(self, session):
        self.__session = session

    def read_source(self, relative_components):
        return self.__session.read('source', relative_components)

    def read_wide(self, relative_components):
        return self.__session.read('wide', relative_components)

    def read_outer(self, relative_components):
        return self.__session.read('outer', relative_components)


def _construct(viewlib, core, source, evidence, absent):
    mapping = MappingProxyType({str(Path(scope.root).joinpath(*row.path)): (scope.anchor, row.path, row.expected)
        for scope in (source, evidence) for row in scope.rows if row.kind == 'file'})

    def regular(path):
        pc.need(type(path) is PosixPath, 'native adapter requires exact NativePath')
        name = str(path)
        pc.canonical(name)
        pc.need(name in mapping, 'native adapter path not admitted')
        anchor, parts, expected = mapping[name]
        return pc.read_regular(anchor, parts, expected)

    adapter = SimpleNamespace(regular=regular, loads=core.loads, stamp=core.stamp,
                              ancestor=core.ancestor, windows_host=core.windows_host)
    def row_dict(row):
        value = dict(path='/'.join(row.path), kind=row.kind, mode=row.mode)
        if row.kind == 'file':
            value.update(bytes=row.expected.bytes, sha256=row.expected.sha256)
        return value
    source_authority = viewlib.SourceAuthority(source.root, [row_dict(r) for r in source.rows if r.kind == 'file'])
    evidence_authority = viewlib.EvidenceAuthority(evidence.root, [row_dict(r) for r in evidence.rows if r.path])
    _observe('view_construct', source_authority=source_authority, evidence_authority=evidence_authority)
    view = viewlib.FreshReadView(adapter, source_authority=source_authority, evidence_authority=evidence_authority,
                                known_absent=tuple(str(Path(evidence.root).joinpath(*p)) for p in absent))
    return view


def _cancel(error):
    if isinstance(error, (FixtureCancellation, KeyboardInterrupt, SystemExit)):
        return error
    if isinstance(error, BaseExceptionGroup):
        for child in error.exceptions:
            found = _cancel(child)
            if found is not None:
                return found
    return None


def _without(error, omitted):
    if error is omitted:
        return None
    if isinstance(error, BaseExceptionGroup):
        children = [child for item in error.exceptions if (child := _without(item, omitted)) is not None]
        return error.derive(children) if children else None
    return error


def inspect_fixture_pair(fixture, binding, owner):
    session = _Session()
    failures, records, outcomes = [], [], []
    scopes, unavailable = {}, {}
    cleanup_due = False
    active_phase = 'admission'
    def fail(label, error):
        failures.append((label, error))
    def attempt(label, action, reason=None):
        if reason is not None:
            records.append(Finalization(label, 'unavailable', reason))
            return
        try:
            _observe(label, session=session)
        except BaseException as error:
            fail(label, error)
        try:
            action()
        except BaseException as error:
            fail(label, error)
            records.append(Finalization(label, 'failed', error))
        else:
            records.append(Finalization(label, 'failed' if any(p == label for p, _ in failures) else 'passed'))
    try:
        pc.supported()
        pc.need(type(fixture) is FixtureV2 and fixture.schema == 'oxid-v3-local-fixture-v2', 'exact fixture required')
        for path in (fixture.source_root, fixture.evidence_root, fixture.wide_root):
            pc.canonical(path)
        pc.need(fixture.wide_root == fixture.evidence_root + '/wide-span', 'direct wide-span child required')
        ownership.validate(owner, fixture.source_root, fixture.evidence_root)
        cleanup_due = True
        pc.need(type(binding) is FixtureBindingV2 and _bindings.get(id(binding)) is binding and
                binding.schema == 'oxid-v3-local-fixture-probes-v2', 'unregistered harness binding')
        viewlib, core = _support(fixture)
        for label, root, rows in (('source', fixture.source_root, fixture.source_rows),
                                   ('evidence', fixture.evidence_root, fixture.outer_rows)):
            try:
                _observe(label + '_establish', session=session)
                rows = _rows(rows)
                if label == 'evidence':
                    pc.need(type(fixture.known_absent) is tuple, 'exact known-absence tuple required')
                    directories = {row.path for row in rows if row.kind == 'directory'}
                    paths = {row.path for row in rows}
                    pc.need(('wide-span',) in directories, 'wide-span directory required')
                    for parts in fixture.known_absent:
                        pc.components(parts)
                        pc.need(bool(parts) and parts not in paths and parts[:-1] in directories,
                                'invalid known-absence path')
                    pc.need(len(set(fixture.known_absent)) == len(fixture.known_absent), 'duplicate known-absence path')
                anchor = pc.open_anchor(root)
                scope = scopes[label] = _Scope(root, rows, anchor)
                _observe(label + '_initial', session=session)
                _seal(scope, initial=True)
            except BaseException as error:
                fail('admission_' + label, error)
                if label not in scopes:
                    unavailable[label] = error
        if not failures:
            _absence(scopes['evidence'], fixture.known_absent)
            session.view = _construct(viewlib, core, scopes['source'], scopes['evidence'], fixture.known_absent)
            _observe('view_admitted', view=session.view)
            session.roots = dict(source=fixture.source_root, wide=fixture.wide_root, outer=fixture.evidence_root)
            session.state = 'CONSUMING'
            reads = FixtureReads(session)
            for active_phase, probe, expected in (('source_probe', binding.source_probe, SOURCE_OUTCOME),
                                                   ('raw_probe', binding.raw_probe, RAW_OUTCOME)):
                result = probe(reads)
                pc.need(result is expected, 'probe must return its fixed immutable fixture outcome')
                outcomes.append((active_phase, result))
    except BaseException as error:
        fail(active_phase, error)
    finally:
        session.state = 'REVOKED'
        attempt('revoke', lambda: None)
        reason = failures[0][1] if failures else 'no admitted view'
        attempt('layout', lambda: session.view.recheck_layout(), None if session.view is not None else reason)
        def extra():
            errors = []
            for scope in scopes.values():
                try:
                    _metadata(scope)
                except BaseException as error:
                    errors.append(error)
            if 'evidence' in scopes:
                try:
                    _absence(scopes['evidence'], fixture.known_absent)
                except BaseException as error:
                    errors.append(error)
            pc._finish(errors)
        attempt('extra_metadata', extra, None if scopes else reason)
        for label in ('source', 'evidence'):
            attempt(label + '_final', lambda label=label: _seal(scopes[label]),
                    None if label in scopes else unavailable.get(label, reason))
        def close():
            session.view = None
            errors = []
            for scope in scopes.values():
                try:
                    os.close(scope.anchor.fd)
                except BaseException as error:
                    errors.append(error)
            pc._finish(errors)
        attempt('resource_close', close)
        attempt('cleanup', lambda: ownership.cleanup(owner), None if cleanup_due else 'ownership unavailable')
        session.state = 'CLOSED'
        # No external action after cleanup; closure assignment cannot invoke a
        # consumer, instrumentation hook, pathname operation or callback.
        records.append(Finalization('session_close', 'passed'))
    result = FixtureResult('oxid-v3-local-fixture-result-v2', tuple(outcomes), tuple(records), session.state)
    if failures:
        ordered = tuple(failures)
        cancellation = next((c for _, e in failures if (c := _cancel(e)) is not None), None)
        if cancellation is not None:
            cancellation.fixture_failures = ordered
            cancellation.fixture_result = result
            others = [rest for _, error in failures if (rest := _without(error, cancellation)) is not None]
            raise cancellation from (BaseExceptionGroup('other fixture failures', others) if others else None)
        group = BaseExceptionGroup('fixture failures: ' + ', '.join(label for label, _ in failures),
                                   [error for _, error in failures])
        group.fixture_failures = ordered
        group.fixture_result = result
        raise group
    return result
