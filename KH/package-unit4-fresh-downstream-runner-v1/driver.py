"""Held live-owner diagnostic bootstrap. Import performs no producer actions.

Only the separately reviewed launcher supplies authenticated inputs. Canonical
session APIs issue every genuine owner; this driver never reconstructs one.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import stat
import sys
import time
import traceback
import types


SCHEMA = 'oxid-fresh-downstream-diagnostic-input-v1'
LIMITS = {'aggregate_seconds':21600, 'setup_seconds':2400,
          'selected_public_seconds':5400, 'parser_seconds':12600,
          'work_stop_seconds':20400, 'preservation_seconds':1140,
          'preservation_stop_seconds':21540, 'cleanup_seconds':60,
          'output_bytes':64*1024**2}
CAPACITY = {'expected_growth_bytes': 3221225472, 'expected_growth_inodes': 40960,
            'alarm_bytes': 2281701376, 'alarm_inode_fraction': [1, 5],
            'alarm_cleanup_inodes': 1024}
OWNERSHIP_LIMITS = {'live_pidfds': 128, 'lifetime_identities': 65536}
PIN_NAMES = {'driver', 'worker', 'session', 'python', 'request',
             'installed_admission', 'capacity_launcher', 'capacity_policy'}
KINDS = {'phase', 'call', 'preservation', 'terminal', 'operation'}
FLAGS = {'diagnostic_only': True, 'local_complete': False,
         'execution_qualified': False, 'collection_complete': False,
         'section_complete': False}
RESERVE = 2 * 1024 ** 2  # One MiB driver failure room, one MiB watchdog terminal room.


class Rejected(RuntimeError):
    pass


def need(condition, message):
    if not condition:
        raise Rejected(message)


def safe_error(error):
    try:
        return str(error)[:2048]
    except BaseException:
        return '<exception formatting unavailable>'


def note(primary, label, error):
    try:
        primary.add_note(label + ': ' + safe_error(error))
    except BaseException:
        pass


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'),
                      allow_nan=False).encode()


def decode(raw):
    def pairs(items):
        value = {}
        for key, item in items:
            need(key not in value, 'duplicate JSON key')
            value[key] = item
        return value
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: need(False, 'nonfinite JSON'))


def stat_key(value):
    return tuple(getattr(value, key) for key in
                 ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns'))


def plain(path, *, directory=False):
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=True) == path and
         not any(item.is_symlink() for item in (path, *path.parents)),
         'canonical path without links required')
    mode = path.lstat().st_mode
    need(stat.S_ISDIR(mode) if directory else stat.S_ISREG(mode),
         'wrong input path type')
    return path


def stable_bytes(path, maximum, checkpoint=lambda: None):
    """Reviewed V3 descriptor association, finite reads and safe-close pattern."""
    path = plain(path)
    before = path.lstat()
    need(0 <= before.st_size <= maximum, 'input byte cap')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC)
    primary = None
    try:
        opened = os.fstat(fd)
        need(stat.S_ISREG(opened.st_mode) and stat_key(opened) == stat_key(before),
             'opened input descriptor changed')
        body = bytearray()
        while True:
            checkpoint()
            part = os.read(fd, min(1024 ** 2, maximum + 1 - len(body)))
            if not part:
                break
            body.extend(part)
            need(len(body) <= maximum, 'input grew beyond cap')
        need(stat_key(os.fstat(fd)) == stat_key(before) == stat_key(path.lstat()) and
             len(body) == before.st_size, 'input changed during read')
        plain(path)
        return bytes(body)
    except BaseException as error:
        primary = error
        raise
    finally:
        try:
            os.close(fd)
        except BaseException as error:
            if primary is None:
                raise
            note(primary, 'input descriptor close', error)


def read_pin(pin, checkpoint=lambda: None):
    need(type(pin) is dict and set(pin) == {'path', 'bytes', 'sha256'} and
         type(pin['bytes']) is int and 0 <= pin['bytes'] <= 16 * 1024 ** 2 and
         type(pin['sha256']) is str and len(pin['sha256']) == 64,
         'closed finite source pin')
    body = stable_bytes(pin['path'], pin['bytes'], checkpoint)
    need(len(body) == pin['bytes'] and hashlib.sha256(body).hexdigest() == pin['sha256'],
         'input pin mismatch: ' + pin['path'])
    return body


def load_bytes(pin, body, name):
    need(name not in sys.modules, 'occupied diagnostic module name')
    need(len(body) == pin['bytes'] and hashlib.sha256(body).hexdigest() == pin['sha256'],
         'captured source bytes differ')
    module = types.ModuleType(name)
    module.__file__, module.__source_sha256__ = pin['path'], pin['sha256']
    sys.modules[name] = module
    try:
        exec(compile(body, pin['path'], 'exec'), module.__dict__)
    except BaseException:
        if sys.modules.get(name) is module:
            del sys.modules[name]
        raise
    return module


class Sink:
    """Bounded record-only external output, never a source of runtime authority."""
    def __init__(self, root, limit):
        self.root, self.limit = plain(root, directory=True), limit
        info = self.root.lstat()
        self.identity = (info.st_dev, info.st_ino)
        self.used = 0
        self.terminal = False
        self.files = {}
        need(all(not (self.root / (name + '.jsonl')).exists() for name in KINDS),
             'worker evidence files must be fresh')
        need(not (self.root / 'partial-downstream-diagnostic.json').exists(),
             'partial terminal must be fresh')

    def _check_root(self):
        info = plain(self.root, directory=True).lstat()
        need((info.st_dev, info.st_ino) == self.identity, 'evidence root identity changed')

    def _write(self, name, body, *, exclusive=False, emergency=False):
        self._check_root()
        allowance = self.limit - RESERVE // 2 if emergency else self.limit - RESERVE
        total = 0
        with os.scandir(self.root) as entries:
            for index, entry in enumerate(entries):
                need(index < 32, 'bounded diagnostic output member roster')
                info = entry.stat(follow_symlinks=False)
                need(stat.S_ISREG(info.st_mode), 'regular diagnostic output members required')
                total += info.st_size
        need(len(body) <= (65536 if emergency else 1024 ** 2) and
             self.used + len(body) <= allowance and total + len(body) <= allowance,
             'diagnostic output cap')
        flags = os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC
        first = name not in self.files
        need(not exclusive or first, 'exclusive output already owned')
        flags |= os.O_EXCL if first else os.O_APPEND
        fd = os.open(self.root / name, flags, 0o600)
        primary = None
        try:
            opened = os.fstat(fd)
            need(stat.S_ISREG(opened.st_mode), 'regular diagnostic evidence required')
            observed = (opened.st_dev, opened.st_ino, opened.st_size)
            if first:
                need(opened.st_size == 0, 'new output must be empty')
            else:
                need(observed == self.files[name], 'owned diagnostic output changed')
            offset = 0
            while offset < len(body):
                wrote = os.write(fd, body[offset:])
                need(wrote > 0, 'diagnostic write made no progress')
                offset += wrote
                self.used += wrote
            after = os.fstat(fd)
            route = (self.root / name).lstat()
            need((after.st_dev, after.st_ino, after.st_size) ==
                 (opened.st_dev, opened.st_ino, opened.st_size + len(body)) and
                 stat_key(after) == stat_key(route), 'output descriptor/path changed')
            self.files[name] = (after.st_dev, after.st_ino, after.st_size)
        except BaseException as error:
            primary = error
            raise
        finally:
            try:
                os.close(fd)
            except BaseException as error:
                if primary is None:
                    raise
                note(primary, 'diagnostic output close', error)

    def emit(self, kind, value):
        need(kind in KINDS and type(value) is dict, 'closed diagnostic record kind')
        if kind == 'terminal':
            need(not self.terminal, 'duplicate partial diagnostic terminal')
        need(all(key not in value or value[key] is expected for key, expected in FLAGS.items()),
             'diagnostic cannot claim qualification')
        body = canonical({**value, **FLAGS}) + b'\n'
        self._write(kind + '.jsonl', body)
        if kind == 'terminal':
            self._write('partial-downstream-diagnostic.json', body, exclusive=True)
            self.terminal = True

    def emergency(self, error):
        try:
            frames = traceback.StackSummary.extract(traceback.walk_tb(error.__traceback__),
                                                    limit=24, lookup_lines=False)
            value = {**FLAGS, 'type': type(error).__name__, 'error': safe_error(error),
                     'stack': [{'file': f.filename[:1024], 'function': f.name[:128],
                                'line': f.lineno} for f in frames]}
            body = canonical(value) + b'\n'
            if len(body) > 65536:
                body = canonical({**FLAGS, 'error': 'bounded failure details omitted'}) + b'\n'
        except BaseException:
            body = b'{"diagnostic_only":true,"execution_qualified":false,"error":"failure formatting unavailable"}\n'
        try:
            self._write('driver-failure.jsonl', body, emergency=True)
            return True
        except BaseException:
            return False


def validate_transport(value, driver_body):
    need(type(value) is dict and set(value) == {'manifest', 'request', 'started_monotonic'},
         'captured launcher transport required')
    manifest, request, started = value['manifest'], value['request'], value['started_monotonic']
    need(type(manifest) is dict and set(manifest) ==
         {'schema', 'mode', 'pins', 'installed', 'paths', 'limits', 'capacity',
          'ownership_limits', 'environment'},
         'closed diagnostic input manifest')
    need(manifest['schema'] == SCHEMA and manifest['mode'] == 'fresh-downstream' and
         manifest['limits'] == LIMITS and manifest['capacity'] == CAPACITY and
         manifest['ownership_limits'] == OWNERSHIP_LIMITS,
         'exact approved diagnostic scope/bounds')
    need(set(manifest['pins']) == PIN_NAMES, 'exact diagnostic source pin roster')
    need(type(started) in (int, float) and math.isfinite(started) and
         0 < started <= time.monotonic() < started + LIMITS['setup_seconds'],
         'original aggregate launch clock')
    need(sys.version_info[:3] == (3, 13, 5) and sys.executable == '/usr/bin/python3.13' and
         sys.dont_write_bytecode and not sys.flags.optimize, 'pinned normal Python required')
    need(dict(os.environ) == manifest['environment'], 'exact clean worker environment')
    need(type(manifest['paths']) is dict and set(manifest['paths']) == {'run_root', 'evidence_root'},
         'closed diagnostic paths')
    need(type(manifest['installed']) is dict and set(manifest['installed']) ==
         {'repository', 'head', 'tree'}, 'closed installed source identity')
    need(type(request) is dict and request.get('scope') == 'combined' and
         request.get('expected_head') == request.get('event_sha') == manifest['installed']['head'] and
         request.get('run_root') == manifest['paths']['run_root'], 'exact fresh combined request')
    run = Path(manifest['paths']['run_root'])
    evidence = plain(manifest['paths']['evidence_root'], directory=True)
    repo = plain(manifest['installed']['repository'], directory=True)
    need(run.is_absolute() and run.resolve(strict=False) == run and not os.path.lexists(run),
         'fresh producer root required')
    plain(run.parent, directory=True)
    need(all(a != b and a not in b.parents and b not in a.parents
             for i, a in enumerate((run, evidence, repo)) for b in (run, evidence, repo)[i+1:]),
         'disjoint producer/evidence/source roots')
    need(Path(manifest['pins']['driver']['path']) == Path(__file__) and
         type(driver_body) is bytes and len(driver_body) == manifest['pins']['driver']['bytes'] and
         hashlib.sha256(driver_body).hexdigest() == manifest['pins']['driver']['sha256'],
         'authenticated executing driver bytes')
    need(Path(manifest['pins']['session']['path']) ==
         repo / 'tests/qualification/package_dependency_unit4_session_v1/run.py',
         'installed canonical session route')
    deadline = started + LIMITS['setup_seconds']
    def checkpoint():
        need(time.monotonic() < deadline, 'driver input validation deadline')
    bodies = {name: read_pin(pin, checkpoint) for name, pin in manifest['pins'].items()}
    need(decode(bodies['request']) == request, 'captured request differs from exact pinned bytes')
    admission = decode(bodies['installed_admission'])
    need(admission['schema'] == 'oxid-unit4-installed-entrypoint-local-admission-v1' and
         admission['status'] == 'verified-local-installed-loader-admission' and
         admission['head'] == manifest['installed']['head'] and
         admission['tree'] == manifest['installed']['tree'] and
         admission['installed_committed_files'] == 37 and admission['issued_capabilities'] == 0 and
         admission['producer_executed'] is False and admission['first_error'] is None,
         'passed effects-blocked installed admission required')
    return manifest, request, started, bodies


def main():
    transport = globals().get('_AUTHENTICATED_INPUTS')
    driver_body = globals().get('_AUTHENTICATED_DRIVER_BYTES')
    manifest, request, started, bodies = validate_transport(transport, driver_body)
    sink = Sink(manifest['paths']['evidence_root'], LIMITS['output_bytes'])
    primary = None
    cancellation = None
    previous = {}
    try:
        session = load_bytes(manifest['pins']['session'], bodies['session'],
                             '_oxid_fresh_live_owner_session_v1')
        worker = load_bytes(manifest['pins']['worker'], bodies['worker'],
                            '_oxid_fresh_live_owner_worker_v1')
        cancellation = session._Cancellation()
        for sig in (signal.SIGTERM, signal.SIGINT):
            previous[sig] = signal.getsignal(sig)
            signal.signal(sig, cancellation)
        worker.run(session, request, sink, started_monotonic=started,
                   cancellation=cancellation, installed=manifest['installed'])
        need(sink.terminal, 'worker did not retain distinct partial terminal')
    except BaseException as error:
        primary = error
    finally:
        if cancellation is not None:
            cancellation.finalizing = True
        def final_checkpoint():
            need(time.monotonic() < started + LIMITS['preservation_stop_seconds'],
                 'driver final preservation deadline')
            if primary is None and cancellation is not None and getattr(cancellation, 'owner', None):
                api, invocation = cancellation.owner
                api.check_deadline(invocation)
                api.public_output_tick(invocation)
        checks = []
        for name, pin in manifest['pins'].items():
            try:
                read_pin(pin, final_checkpoint)
                checks.append({'name': name, 'status': 'VERIFIED', 'pin': pin})
            except BaseException as error:
                checks.append({'name': name, 'status': 'UNVERIFIED', 'error': safe_error(error)})
                if primary is None:
                    primary = error
                else:
                    note(primary, 'driver final pin preservation ' + name, error)
        try:
            sink.emit('preservation', {'stage': 'driver-input-pins', 'files': checks,
                                      'monotonic': time.monotonic()})
        except BaseException as error:
            if primary is None:
                primary = error
            else:
                note(primary, 'driver final preservation record', error)
        if cancellation is not None:
            cancellation.sealed = True
        for sig, handler in previous.items():
            try:
                signal.signal(sig, handler)
            except BaseException as error:
                if primary is None:
                    primary = error
                else:
                    note(primary, 'driver signal restoration', error)
    if primary is not None:
        if cancellation is not None:
            try:
                # A stage terminal is not overall success. Late preservation or
                # handler failures still invalidate the original issued owner.
                cancellation.poison()
            except BaseException as error:
                note(primary, 'driver late original-owner poison', error)
        sink.emergency(primary)
        raise primary
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
