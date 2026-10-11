"""Owned installed-dpkg staging for the Linux public CI domain only.

No process runs at import. A selected-library receipt alone can never issue
StagedRuntime. The original stager and its receipt are used without modification.
"""
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import signal
import stat
import sys
import threading
import time
import types
import weakref

COMMON_SHA256 = '017a0b034ac120b246d2f9b88600cc3515534fe97c3e033b73a9f363e7b76e3d'
COMMON_KEY = '_oxid_package_dependency_unit4_ci_common_v1'
HERE = Path(__file__).resolve().parent
STAGER = 'tests/fixtures/typed_project_unit3_independent/portable/native-v1/stage_llvm_runtime.py'
HELPER = 'tests/fixtures/typed_project_unit3_independent/portable/common.py'
TREE = 'tests/qualification/unit4_public_v3/process_tree.py'
PINS = {STAGER: '055dd46f2c24f3a0496486ec89570ef533d7a016a5bc85d834447e8a59204def',
        HELPER: '7b8b6d476c66eb7c2e833dff9dfe7eb42308bf8a67fd38a9a501f3b490eb7297',
        TREE: '6b53900e18b33a26c5949bf8905f742e9abe5b67e74748585bbb24b6e4091454'}
SOURCE_ROOT = Path('/usr/lib/x86_64-linux-gnu')
PACKAGE_VERSION = '1:19.1.7-3+b1'
FILES = {'libLLVM.so.19.1': {'package': 'libllvm19', 'bytes': 129673080,
         'sha256': '3523f50f635d2a1ea47518a392451f2854b92823ceaf1345fb806099dd1a3b8a'},
         'libclang-cpp.so.19.1': {'package': 'libclang-cpp19', 'bytes': 71043800,
         'sha256': '0108c687d2f500d77687984c46c9fad663782603f5c623e3c14224c155b4fd21'}}
PACKAGES = {'libclang-cpp19': 'libclang-cpp19', 'libllvm19': 'libllvm19:amd64'}
ALIAS, ALIAS_TARGET = 'libLLVM-19.so', 'libLLVM.so.19.1'
STREAM_LIMIT = 1024 ** 2
FILE_LIMIT = 256 * 1024 ** 2
OUTPUT_LIMIT = 512 * 1024 ** 2
STAGE_SECONDS = 600
STAGER_SECONDS = 180
READ_SECONDS = 30
PROCESS_LIMIT = 8
RSS_LIMIT = 2 * 1024 ** 3
_ISSUED = weakref.WeakKeyDictionary()
_USED_CONTEXTS = {}
_TREE_OWNER = None


class Rejected(ValueError):
    pass


class Cancelled(BaseException):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def _common():
    owner = globals().get('_COMMON_OWNER')
    need(owner is not None and type(COMMON_SHA256) is str and
         re.fullmatch('[0-9a-f]{64}', COMMON_SHA256), 'authenticated CI sibling owner required')
    path = HERE / 'common.py'
    raw = _read(path, 2 * 1024 ** 2)
    need(hashlib.sha256(raw).hexdigest() == COMMON_SHA256 and
         sys.modules.get(COMMON_KEY) is owner and owner.__file__ == str(path) and
         getattr(owner, '__source_sha256__', None) == COMMON_SHA256,
         'staging common body/owner changed')
    owner.owner()
    owner.owns_sibling(sys.modules.get(__name__))
    return owner


def _path(path, exists=True):
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=exists) == path and
         not any(p.is_symlink() for p in (path, *path.parents)), 'canonical staging path required')
    return path


def _stat(info):
    return tuple(getattr(info, key) for key in ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns'))


def _write_bytes(path, raw):
    path = _path(path, False)
    need(type(raw) is bytes and len(raw) <= STREAM_LIMIT and not os.path.lexists(path) and path.parent.is_dir(),
         'fresh bounded staging stream required')
    with path.open('xb') as stream:
        stream.write(raw)


def _identity(path, limit=FILE_LIMIT, tick=lambda: None):
    path = _path(path)
    before = path.lstat()
    need(stat.S_ISREG(before.st_mode) and 0 <= before.st_size <= limit, 'bounded regular staging file required')
    digest, count = hashlib.sha256(), 0
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        need(_stat(opened) == _stat(before), 'staging file changed while opening')
        while True:
            tick()
            chunk = stream.read(min(1024 ** 2, limit - count + 1))
            if not chunk:
                break
            count += len(chunk)
            need(count <= limit and count <= before.st_size, 'growing staging file')
            digest.update(chunk)
    need(_stat(path.lstat()) == _stat(before) and count == before.st_size, 'staging file drift')
    return {'path': str(path), 'bytes': count, 'sha256': digest.hexdigest()}


def _read(path, limit=STREAM_LIMIT):
    row = _identity(path, limit)
    raw = Path(path).read_bytes()
    need(len(raw) == row['bytes'] and hashlib.sha256(raw).hexdigest() == row['sha256'], 'staging read drift')
    return raw


def _error(error):
    return {'type': type(error).__name__, 'message': str(error)[:4096]}


def _alias(root):
    _path(root)
    alias = root / ALIAS
    need(alias.is_symlink() and os.readlink(alias) == ALIAS_TARGET and
         alias.resolve(strict=True) == root / ALIAS_TARGET and not (root / ALIAS_TARGET).is_symlink(),
         'exact installed LLVM alias required')
    info = alias.lstat()
    return {'path': str(alias), 'kind': 'symlink', 'link': ALIAS_TARGET,
            'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode}


def _libraries(root, tick):
    result = {}
    for name, expected in sorted(FILES.items()):
        actual = _identity(root / name, tick=tick)
        need({k: actual[k] for k in ('bytes', 'sha256')} ==
             {k: expected[k] for k in ('bytes', 'sha256')}, 'exact pinned LLVM library required')
        result[name] = actual
    result[ALIAS] = _alias(root)
    return result


def _dependencies(repository, machine_api, machine, tick):
    result = {}
    for label, relative in (('stager', STAGER), ('common', HELPER), ('process_tree', TREE)):
        result[label] = _identity(repository / relative, tick=tick)
        need(result[label]['sha256'] == PINS[relative], 'exact original staging helper closure required')
    host = machine.document()['snapshot']['host']
    need(host['name'] == 'Linux x86_64', 'Linux machine required for installed-dpkg staging')
    result['python'] = machine_api._route(Path(host['python']['selected']))
    need(result['python'] == host['python'] and result['python']['resolved'] == sys.executable,
         'original canonical machine interpreter required')
    result['dpkg_query'] = machine_api._route(Path('/usr/bin/dpkg-query'))
    result['readelf'] = machine_api._route(Path('/usr/bin/readelf'))
    for name in ('dpkg_query', 'readelf'):
        need(os.access(result[name]['resolved'], os.X_OK), 'executable installed staging tool required')
    metadata = [Path('/var/lib/dpkg/status')]
    for selector in sorted(PACKAGES.values()):
        metadata.extend(Path('/var/lib/dpkg/info') / (selector + suffix) for suffix in ('.list', '.md5sums'))
    result['dpkg_metadata'] = [_identity(path, 64 * 1024 ** 2, tick) for path in metadata]
    updates = _path('/var/lib/dpkg/updates')
    need(updates.is_dir() and not any(updates.iterdir()), 'pending dpkg updates are not admitted')
    return result


def _tree_module(repository):
    global _TREE_OWNER
    path = repository / TREE
    raw = _read(path, 1024 ** 2)
    need(hashlib.sha256(raw).hexdigest() == PINS[TREE], 'original ProcessTree body required')
    if _TREE_OWNER is None:
        key = '_oxid_package_unit4_ci_staging_process_tree_v1'
        need(key not in sys.modules, 'preoccupied staging ProcessTree owner')
        module = types.ModuleType(key)
        module.__file__ = str(path)
        module.__source_sha256__ = PINS[TREE]
        sys.modules[key] = module
        try:
            exec(compile(raw, str(path), 'exec'), module.__dict__)
        except BaseException:
            if sys.modules.get(key) is module:
                del sys.modules[key]
            raise
        _TREE_OWNER = module
    need(sys.modules.get(_TREE_OWNER.__name__) is _TREE_OWNER and _TREE_OWNER.__file__ == str(path) and
         _TREE_OWNER.__source_sha256__ == PINS[TREE], 'staging ProcessTree owner changed')
    return _TREE_OWNER


def _environment(root):
    return {'PATH': '/usr/bin', 'HOME': str(root / 'home'), 'TMPDIR': str(root / 'tmp'),
            'LANG': 'C', 'LC_ALL': 'C', 'PYTHONDONTWRITEBYTECODE': '1',
            'PYTHONNOUSERSITE': '1', 'PYTHONOPTIMIZE': '0', 'DPKG_COLORS': 'never'}


def _members(group):
    rows = []
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            raw = (path / 'stat').read_text()
        except (FileNotFoundError, ProcessLookupError):
            continue
        fields = raw[raw.rfind(')') + 2:].split()
        need(len(fields) >= 22, 'invalid Linux process metadata')
        if int(fields[2]) == group:
            rows.append({'pid': int(path.name), 'state': fields[0],
                         'rss_bytes': int(fields[21]) * os.sysconf('SC_PAGE_SIZE')})
    return rows


def _scan_output(root, *, hashes=False, tick=lambda: None):
    """The only symlink allowed anywhere is the original LLVM alias."""
    rows, total, paths, pending = [], 0, [], [root]
    while pending:
        parent = pending.pop()
        with os.scandir(parent) as entries:
            for entry in entries:
                tick()
                need(len(paths) < 256, 'staging output member bound')
                path = parent / entry.name
                paths.append(path)
                if entry.is_dir(follow_symlinks=False):
                    pending.append(path)
    for path in sorted(paths):
        tick()
        need(len(rows) < 256, 'staging output member bound')
        name = path.relative_to(root).as_posix()
        info = path.lstat()
        if stat.S_ISDIR(info.st_mode):
            row = {'path': name, 'kind': 'directory', 'mode': stat.S_IMODE(info.st_mode)}
        elif stat.S_ISLNK(info.st_mode):
            need(name == 'runtime/libraries/' + ALIAS, 'unregistered staging output symlink')
            _alias(path.parent)
            row = {'path': name, 'kind': 'symlink', 'link': ALIAS_TARGET}
        else:
            need(stat.S_ISREG(info.st_mode) and info.st_size <= FILE_LIMIT, 'special or oversized staging output')
            total += info.st_size
            need(total <= OUTPUT_LIMIT, 'staging total output bound')
            row = {'path': name, 'kind': 'file', 'mode': stat.S_IMODE(info.st_mode), 'bytes': info.st_size}
            if hashes:
                row['sha256'] = _identity(path, tick=tick)['sha256']
        rows.append(row)
    return rows, total


def _closed_output(manifest, operations):
    files = {'runtime/llvm-runtime-stage.json', *('runtime/libraries/' + name for name in FILES)}
    directories = {'commands', 'home', 'tmp', 'runtime', 'runtime/libraries'}
    for operation in operations:
        prefix = 'commands/' + operation['name']
        directories.add(prefix)
        files.update(prefix + '/' + name for name in ('stdout', 'stderr', 'process.json'))
    expected = {(name, 'directory') for name in directories} | {(name, 'file') for name in files}
    expected.add(('runtime/libraries/' + ALIAS, 'symlink'))
    need({(row['path'], row['kind']) for row in manifest} == expected and len(manifest) == len(expected),
         'exact fixed staging output catalogue required')


def _process_policy(timeout):
    need(timeout in (READ_SECONDS, STAGER_SECONDS), 'fixed staging process deadline required')
    return {'wall_seconds': timeout, 'stream_bytes': STREAM_LIMIT,
            'file_bytes': FILE_LIMIT, 'output_bytes': OUTPUT_LIMIT,
            'linux_processes': PROCESS_LIMIT, 'linux_rss_bytes': RSS_LIMIT,
            'ownership': 'new-posix-process-group', 'cleanup_grace_seconds': 10}


def _supervise(common, tree_class, argv, root, environment, timeout, tick):
    """Own the actual process group and preserve bounded streams on every exit."""
    started = time.time_ns()
    deadline = time.monotonic() + timeout
    streams = {'stdout': bytearray(), 'stderr': bytearray()}
    events = queue.Queue(maxsize=8)
    readers, closed = [], set()
    tree = None
    first_error = None
    cleanup_errors = []
    status = pid = None
    previous = {}
    finalizing = False
    timed_out = exceeded = False
    peaks = {'processes': 0, 'rss_bytes': 0, 'output_bytes': 0}

    def cancel(signum, frame):
        if finalizing:
            return
        handler = previous.get(signum)
        if callable(handler):
            handler(signum, frame)
        raise Cancelled('installed-dpkg staging cancelled by ' + signal.Signals(signum).name)

    def reader(pipe, name):
        try:
            while True:
                chunk = os.read(pipe.fileno(), 65536)
                events.put((name, chunk))
                if not chunk:
                    break
        except BaseException as error:
            events.put(('error', _error(error)))
            events.put((name, b''))
        finally:
            pipe.close()

    def consume(name, data):
        nonlocal exceeded
        if name == 'error':
            raise Rejected('staging stream read failure: ' + str(data))
        if not data:
            closed.add(name)
        else:
            room = STREAM_LIMIT - len(streams[name])
            streams[name].extend(data[:room])
            exceeded = exceeded or len(data) > room

    try:
        need(threading.current_thread() is threading.main_thread(), 'staging signal-owning main thread required')
        for signum in (signal.SIGINT, signal.SIGTERM):
            previous[signum] = signal.getsignal(signum)
            signal.signal(signum, cancel)
        tick()
        tree = tree_class(argv, str(root), environment, new_session=True)
        pid = tree.child.pid
        need(type(pid) is int and pid > 0 and tree.group == pid, 'owned new Linux process group required')
        for name in streams:
            worker = threading.Thread(target=reader, args=(getattr(tree.child, name), name), daemon=True)
            worker.start()
            readers.append(worker)
        while len(closed) != 2 or tree.child.poll() is None:
            tick()
            if time.monotonic() >= deadline:
                timed_out = True
                break
            members = _members(pid)
            rss = sum(row['rss_bytes'] for row in members)
            peaks['processes'] = max(peaks['processes'], len(members))
            peaks['rss_bytes'] = max(peaks['rss_bytes'], rss)
            need(len(members) <= PROCESS_LIMIT and rss <= RSS_LIMIT, 'staging process/RSS limit')
            _, size = _scan_output(root)
            peaks['output_bytes'] = max(peaks['output_bytes'], size)
            try:
                consume(*events.get(timeout=.05))
            except queue.Empty:
                continue
            if exceeded:
                break
        if not timed_out and not exceeded:
            status = tree.child.wait(timeout=max(.01, deadline - time.monotonic()))
    except BaseException as error:
        first_error = error
    finally:
        finalizing = True
        if tree is not None:
            try:
                tree.close()
                status = tree.child.wait(timeout=5)
            except BaseException as error:
                cleanup_errors.append(_error(error))
                try:
                    tree.child.kill()
                    status = tree.child.wait(timeout=2)
                except BaseException as secondary:
                    cleanup_errors.append(_error(secondary))
        until = time.monotonic() + 1
        while time.monotonic() < until:
            try:
                consume(*events.get(timeout=.02))
            except queue.Empty:
                if not any(worker.is_alive() for worker in readers):
                    break
            except BaseException as error:
                first_error = first_error or error
        for worker in readers:
            worker.join(timeout=.02)
        if any(worker.is_alive() for worker in readers):
            cleanup_errors.append({'type': 'CleanupFailure', 'message': 'staging pipe reader remains'})
        if pid is not None:
            try:
                need(not any(row['state'] != 'Z' for row in _members(pid)), 'staging owned descendant remains')
            except BaseException as error:
                cleanup_errors.append(_error(error))
    for signum, handler in previous.items():
        signal.signal(signum, handler)
    record = {'argv': list(argv), 'cwd': str(root), 'environment': dict(environment),
              'pid': pid, 'started_ns': started, 'completed_ns': time.time_ns(), 'status': status,
              'timed_out': timed_out, 'stream_limit_exceeded': exceeded,
              'cleanup_complete': not cleanup_errors, 'cleanup_errors': cleanup_errors,
              'failure': _error(first_error) if first_error is not None else None,
              'observed_peaks': peaks,
              'policy': _process_policy(timeout)}
    return record, {key: bytes(value) for key, value in streams.items()}, first_error


def _queries(dependencies, *, after=False):
    commands = []
    executable = dependencies['dpkg_query']['selected']
    for package, selector in sorted(PACKAGES.items()):
        prefix = ('after-' if after else 'before-') + package
        commands.append((prefix + '-status', [executable, '--show', '--showformat',
            '${binary:Package}\t${Version}\t${Architecture}\t${db:Status-Status}\n', selector]))
        commands.append((prefix + '-listfiles', [executable, '--listfiles', selector]))
        names = sorted(name for name, row in FILES.items() if row['package'] == package)
        if package == 'libllvm19':
            names.append(ALIAS)
        for name in sorted(names):
            commands.append((prefix + '-owner-' + name, [executable, '--search', str(SOURCE_ROOT / name)]))
    return commands


def _verify_query_streams(operations):
    by_name = {row['name']: row for row in operations}
    for after in (False, True):
        for package, selector in sorted(PACKAGES.items()):
            prefix = ('after-' if after else 'before-') + package
            status = _read(by_name[prefix + '-status']['stdout']['path']).decode('utf-8')
            need(status == f'{selector}\t{PACKAGE_VERSION}\tamd64\tinstalled\n', 'installed dpkg status differs')
            owned = _read(by_name[prefix + '-listfiles']['stdout']['path']).decode('utf-8').splitlines()
            names = sorted(name for name, row in FILES.items() if row['package'] == package)
            if package == 'libllvm19':
                names.append(ALIAS)
            for name in names:
                path = str(SOURCE_ROOT / name)
                need(owned.count(path) == 1, 'dpkg-owned library path missing or repeated')
                owner = _read(by_name[prefix + '-owner-' + name]['stdout']['path']).decode('utf-8')
                need(owner == f'{selector}: {path}\n', 'dpkg library owner differs')


def _expected_packages():
    result = []
    for package, selector in sorted(PACKAGES.items()):
        names = [name for name, row in FILES.items() if row['package'] == package]
        if package == 'libllvm19':
            names.append(ALIAS)
        result.append({'package': package, 'binary_package': selector, 'version': PACKAGE_VERSION,
                       'architecture': 'amd64', 'status': 'installed',
                       'owned_paths': sorted(str(SOURCE_ROOT / name) for name in names)})
    return result


def _receipt(common, receipt, dependencies, root):
    keys = {'schema', 'status', 'assertion_mode', 'helper', 'source_directory', 'library_directory',
            'packages', 'content', 'content_sha256', 'unique_target_bytes', 'dependency_scope'}
    need(type(receipt) is dict and set(receipt) == keys and
         receipt['schema'] == 'oxid-unit3-portable-v1-llvm-runtime-stage' and receipt['status'] == 'verified',
         'unchanged complete original staging receipt required')
    need(receipt['helper'] == dependencies['stager'] and receipt['source_directory'] == str(SOURCE_ROOT) and
         receipt['library_directory'] == str(root / 'runtime/libraries') and
         receipt['packages'] == _expected_packages(), 'stager helper/directory/package provenance differs')
    expected = [{'path': name, 'kind': 'file', 'soname': name, **row} for name, row in FILES.items()]
    expected.append({'path': ALIAS, 'kind': 'symlink', 'link': ALIAS_TARGET, 'package': 'libllvm19'})
    content = {'package_version': PACKAGE_VERSION, 'architecture': 'amd64',
               'entries': sorted(expected, key=lambda row: row['path'])}
    # The original helper has its own compact ASCII JSON encoding.
    digest = hashlib.sha256(json.dumps(content, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    need(receipt['content'] == content and receipt['content_sha256'] == digest and
         receipt['unique_target_bytes'] == sum(row['bytes'] for row in FILES.values()), 'original stage content differs')
    need(receipt['assertion_mode'] == {'__debug__': True, 'optimize': 0,
         'PYTHONOPTIMIZE': '0', 'python': sys.version}, 'original stager assertion/interpreter identity differs')
    need(receipt['dependency_scope'] == 'Pinned LLVM override libraries only; ordinary host loader/libc, C++ and other system dependencies remain required.',
         'unchanged original runtime dependency scope required')


class StagedRuntime:
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('StagedRuntime is issued only by the live installed-dpkg staging owner')

    def __setattr__(self, *args):
        raise TypeError('immutable original staging capability')

    def document(self):
        return _common().decode(_entry(self)['raw'])


def _entry(record):
    need(type(record) is StagedRuntime and record in _ISSUED, 'original live staging capability required')
    return _ISSUED[record]


def stage_runtime(ci_context, invocation, machine, fresh_output):
    common = _common()
    ci = common.verify_ci(ci_context)
    common.require_activation(ci_context)
    need(ci['domain'] == 'linux-public' and common.invocation(ci_context) is invocation,
         'original Linux public CI invocation required')
    api = common.apis(ci_context)
    api['machine'].verify_machine(machine, invocation)
    need(machine.invocation() is invocation and os.name == 'posix' and sys.platform.startswith('linux') and
         os.geteuid() == 0 and sys.dont_write_bytecode and not sys.flags.optimize,
         'actual canonical root Linux staging owner required')
    root = _path(fresh_output, False)
    need(root == Path(ci['outer_root']) / 'staging' and root.parent.is_dir() and
         not os.path.lexists(root) and Path(ci['producer_root']) not in root.parents,
         'fresh fixed staging root outside shared producer output required')
    need(ci_context not in _USED_CONTEXTS, 'staging is once-only for the original CI context')
    _USED_CONTEXTS[ci_context] = True
    started = time.time_ns()
    deadline = time.monotonic() + STAGE_SECONDS
    def tick():
        need(time.monotonic() < deadline, 'whole staging deadline exhausted')
        api['preparation'].check_deadline(invocation)
    repository = Path(machine.document()['snapshot']['checkout']['path'])
    dependencies = _dependencies(repository, api['machine'], machine, tick)
    libraries = _libraries(SOURCE_ROOT, tick)
    tree = _tree_module(repository)
    environment = _environment(root)
    root.mkdir(mode=0o700)
    for name in ('commands', 'home', 'tmp'):
        (root / name).mkdir(mode=0o700)
    operations = []
    first_error = None
    preservation = {}

    def command(name, argv, seconds):
        tick()
        directory = root / 'commands' / name
        directory.mkdir()
        remaining = deadline - time.monotonic()
        need(remaining >= seconds, 'complete staging operation must fit remaining reservation')
        process, streams, error = _supervise(common, tree.ProcessTree, argv, root, environment, seconds, tick)
        process['name'] = name
        operations.append(process)
        if error is None and not (process['status'] == 0 and not process['timed_out'] and
                not process['stream_limit_exceeded'] and process['cleanup_complete'] and
                process['failure'] is None and not streams['stderr']):
            error = Rejected('owned staging process failed; original streams and status retained')
        try:
            for stream, raw in streams.items():
                path = directory / stream
                _write_bytes(path, raw)
                process[stream] = _identity(path, STREAM_LIMIT)
            common.write_new(directory / 'process.json', process)
        except BaseException as capture_error:
            process['preservation_error'] = _error(capture_error)
            if error is not None:
                raise error from capture_error
            raise
        if error is not None:
            raise error

    try:
        for name, argv in _queries(dependencies):
            command(name, argv, READ_SECONDS)
        command('stage-installed-llvm', [dependencies['python']['resolved'], '-I', '-S', '-B',
            dependencies['stager']['path'], '--output', str(root / 'runtime')], STAGER_SECONDS)
        for name, argv in _queries(dependencies, after=True):
            command(name, argv, READ_SECONDS)
        _verify_query_streams(operations)
        receipt_path = root / 'runtime/llvm-runtime-stage.json'
        original_receipt = common.decode(_read(receipt_path))
        _receipt(common, original_receipt, dependencies, root)
        _libraries(root / 'runtime/libraries', tick)
        need(sorted(path.name for path in (root / 'runtime/libraries').iterdir()) == sorted([*FILES, ALIAS]) and
             sorted(path.name for path in (root / 'runtime').iterdir()) == ['libraries', 'llvm-runtime-stage.json'],
             'closed fresh staged runtime output required')
        summary = common.decode(_read(next(row for row in operations if row['name'] == 'stage-installed-llvm')['stdout']['path']))
        need(summary == {'status': 'verified', 'library_directory': original_receipt['library_directory'],
                         'content_sha256': original_receipt['content_sha256']}, 'actual stager stdout differs')
        need(_dependencies(repository, api['machine'], machine, tick) == dependencies and
             _libraries(SOURCE_ROOT, tick) == libraries, 'staging input/tool/dpkg/library drift')
        common.verify_ci(ci_context)
        api['machine'].verify_machine(machine, invocation)
    except BaseException as error:
        first_error = error
    finally:
        # Failure evidence is independent of the shared invocation's poisoned state.
        preserve_until = time.monotonic() + 120
        def preserve_tick():
            need(time.monotonic() < preserve_until, 'staging failure preservation deadline')
        for name, action in (
            ('dependencies', lambda: need(_dependencies(repository, api['machine'], machine, preserve_tick) == dependencies,
                                          'staging dependency drift')),
            ('source_libraries', lambda: need(_libraries(SOURCE_ROOT, preserve_tick) == libraries, 'source library drift'))):
            try:
                action()
                preservation[name] = {'status': 'verified'}
            except BaseException as error:
                preservation[name] = {'status': 'failed', 'failure': _error(error)}
                first_error = first_error or error
        if first_error is not None:
            failure = {'schema': 'oxid-package-unit4-ci-staging-failure-v1', 'ci': ci,
                       'invocation': invocation.document(), 'started_ns': started, 'completed_ns': time.time_ns(),
                       'failure': _error(first_error), 'operations': operations, 'preservation': preservation,
                       'status': 'FAILED', 'staging_qualified': False}
            try:
                common.write_new(root / 'staging-failure.json', failure)
            except BaseException as capture_error:
                print(json.dumps({'failure': failure['failure'], 'preservation_failure': _error(capture_error)},
                                 sort_keys=True), file=sys.stderr)
    if first_error is not None:
        raise first_error
    record = None
    try:
        manifest, _ = _scan_output(root, hashes=True, tick=tick)
        _closed_output(manifest, operations)
        document = {'schema': 'oxid-package-unit4-ci-staging-v1', 'ci': ci,
                    'invocation': invocation.document(), 'machine_sha256': common.sha(common.canonical(machine.document())),
                    'root': str(root), 'library_root': str(root / 'runtime/libraries'),
                    'runtime_receipt': _identity(receipt_path), 'dependencies': dependencies,
                    'source_libraries': libraries, 'operations': operations, 'original_receipt': original_receipt,
                    'output_manifest': manifest, 'started_ns': started, 'completed_ns': time.time_ns()}
        record = object.__new__(StagedRuntime)
        _ISSUED[record] = {'raw': common.canonical(document), 'ci': ci_context, 'invocation': invocation,
                           'machine': machine, 'repository': repository}
        try:
            verify_staging(record, ci_context, invocation)
        except BaseException:
            del _ISSUED[record]
            raise
        return record
    except BaseException as error:
        if record is not None:
            _ISSUED.pop(record, None)
        failure = {'schema': 'oxid-package-unit4-ci-staging-failure-v1', 'ci': ci,
                   'invocation': invocation.document(), 'started_ns': started, 'completed_ns': time.time_ns(),
                   'failure': _error(error), 'operations': operations, 'preservation': preservation,
                   'status': 'FAILED', 'staging_qualified': False}
        try:
            common.write_new(root / 'staging-failure.json', failure)
        except BaseException as capture_error:
            print(json.dumps({'failure': failure['failure'], 'preservation_failure': _error(capture_error)},
                             sort_keys=True), file=sys.stderr)
        raise


def verify_staging(record, ci_context, invocation):
    common = _common()
    owner = _entry(record)
    need(owner['ci'] is ci_context and owner['invocation'] is invocation and
         common.invocation(ci_context) is invocation, 'foreign CI staging capability')
    document = common.decode(owner['raw'])
    need(document['ci'] == common.verify_ci(ci_context) and document['invocation'] == invocation.document(),
         'staging CI/invocation identity drift')
    api = common.apis(ci_context)
    machine = owner['machine']
    api['machine'].verify_machine(machine, invocation)
    need(machine.invocation() is invocation and document['machine_sha256'] == common.sha(common.canonical(machine.document())),
         'staging machine identity drift')
    deadline = time.monotonic() + 120
    def tick():
        need(time.monotonic() < deadline, 'staging terminal readmission deadline')
        api['preparation'].check_deadline(invocation)
    root = _path(document['root'])
    need(root == Path(document['ci']['outer_root']) / 'staging' and document['ci']['domain'] == 'linux-public',
         'fixed Linux public staging root required')
    dependencies = _dependencies(owner['repository'], api['machine'], machine, tick)
    need(dependencies == document['dependencies'] and _libraries(SOURCE_ROOT, tick) == document['source_libraries'],
         'staging producer inputs changed')
    _libraries(root / 'runtime/libraries', tick)
    need(_identity(document['runtime_receipt']['path']) == document['runtime_receipt'], 'original staging receipt changed')
    receipt = common.decode(_read(document['runtime_receipt']['path']))
    need(receipt == document['original_receipt'], 'original receipt body drift')
    _receipt(common, receipt, dependencies, root)
    expected = _queries(dependencies) + [('stage-installed-llvm', [dependencies['python']['resolved'], '-I', '-S', '-B',
        dependencies['stager']['path'], '--output', str(root / 'runtime')])] + _queries(dependencies, after=True)
    need([row['name'] for row in document['operations']] == [name for name, _ in expected],
         'complete ordered original staging process catalogue required')
    last = document['started_ns']
    for operation, (name, argv) in zip(document['operations'], expected):
        need(operation['argv'] == argv and operation['cwd'] == str(root) and
             operation['environment'] == _environment(root) and operation['status'] == 0 and
             operation['cleanup_complete'] is True and not operation['cleanup_errors'] and
             operation['failure'] is None and operation['timed_out'] is False and
             operation['stream_limit_exceeded'] is False and type(operation['pid']) is int and operation['pid'] > 0 and
             last <= operation['started_ns'] < operation['completed_ns'] <= document['completed_ns'],
             'original live staging operation differs or failed')
        seconds = STAGER_SECONDS if name == 'stage-installed-llvm' else READ_SECONDS
        need(operation['policy'] == _process_policy(seconds) and
             operation['completed_ns'] - operation['started_ns'] <= (seconds + 10) * 10 ** 9 and
             set(operation['observed_peaks']) == {'processes', 'rss_bytes', 'output_bytes'} and
             all(type(value) is int and value >= 0 for value in operation['observed_peaks'].values()) and
             operation['observed_peaks']['processes'] <= PROCESS_LIMIT and
             operation['observed_peaks']['rss_bytes'] <= RSS_LIMIT and
             operation['observed_peaks']['output_bytes'] <= OUTPUT_LIMIT,
             'fixed live staging limits differ or exceeded')
        last = operation['completed_ns']
        for stream in ('stdout', 'stderr'):
            need(operation[stream]['path'] == str(root / 'commands' / name / stream) and
                 _identity(operation[stream]['path'], STREAM_LIMIT) == operation[stream], 'staging raw stream drift')
        need(operation['stderr']['bytes'] == 0 and common.decode(_read(root / 'commands' / name / 'process.json')) == operation,
             'staging command carrier or stderr differs')
    _verify_query_streams(document['operations'])
    manifest, _ = _scan_output(root, hashes=True, tick=tick)
    _closed_output(manifest, document['operations'])
    need(manifest == document['output_manifest'], 'closed staging output membership/body drift')
    return document
