#!/usr/bin/env python3
"""Qualify actual PEP 517 wheels, sdists and installed CPython native calls.

This script launches real build tools only when explicitly executed. It has no
compiler substitute or simulated success path. Receipts retain raw command
streams, wheel members, source inventories and positive native-call counts.
"""
from __future__ import annotations

import argparse
import base64
import csv
from email.parser import BytesParser
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import selectors
import shutil
import signal
import stat
import struct
import subprocess
import sys
import sysconfig
import tarfile
import time
import tomllib
import zipfile


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_LIMIT = 4 * 1024 * 1024
FILE_LIMIT = 32 * 1024 * 1024
ARCHIVE_LIMIT = 80 * 1024 * 1024
SOURCE_LIMIT = 8 * 1024 * 1024
BACKEND_FILES = ('pyproject.toml', 'build_backend/oxid_backend.py', 'build_backend/shim.c.in')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def file_identity(path):
    with Path(path).open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return {'path': str(path), 'bytes': Path(path).stat().st_size, 'sha256': digest}


def member_name(name):
    require(isinstance(name, str) and '\\' not in name and ':' not in name
            and not any(ord(character) < 32 or ord(character) == 127 for character in name),
            'unsafe archive member')
    path = PurePosixPath(name)
    require(not path.is_absolute() and all(part not in ('', '.', '..') for part in name.split('/')),
            'archive member escapes its root')
    return path


def actual_tag():
    require(sys.implementation.name == 'cpython' and sys.version_info >= (3, 11)
            and sys.platform == 'linux' and struct.calcsize('P') == 8
            and sys.byteorder == 'little' and not sysconfig.get_config_var('Py_GIL_DISABLED'),
            'qualification requires actual GIL CPython >=3.11 on little-endian Linux64')
    machine = platform.machine()
    require(machine in ('x86_64', 'aarch64') and sysconfig.get_platform() == 'linux-' + machine,
            'qualification requires the actual supported Linux host')
    version = f'cp{sys.version_info.major}{sys.version_info.minor}'
    return f'{version}-{version}-linux_{machine}'


class Evidence:
    def __init__(self, output):
        self.output = output
        self.report = {'schema': 'oxid-python-native-qualification-v1', 'status': 'running',
                       'commands': [], 'wheels': [], 'probes': [], 'native_calls': 0,
                       'abi3_qualified': False, 'manylinux_qualified': False}
        self.save()

    def save(self):
        (self.output / 'result.json').write_text(json.dumps(self.report, indent=2, sort_keys=True) + '\n')

    def command(self, label, argv, cwd, environment, timeout=360, purpose='genuine-qualification'):
        require(argv and all(isinstance(arg, str) and '\0' not in arg for arg in argv), 'invalid argv')
        folder = self.output / 'commands' / f'{len(self.report["commands"]):03d}-{label}'
        folder.mkdir(parents=True)
        row = {'label': label, 'purpose': purpose, 'argv': argv, 'cwd': str(cwd), 'timeout_seconds': timeout,
               'output_limit_per_stream': OUTPUT_LIMIT, 'status': 'running'}
        self.report['commands'].append(row)
        self.save()
        started = time.monotonic()
        child = None
        selector = selectors.DefaultSelector()
        captures = {name: bytearray() for name in ('stdout', 'stderr')}
        streams = {name: (folder / name).open('xb') for name in captures}
        try:
            child = subprocess.Popen(argv, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                     stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            for name, source in (('stdout', child.stdout), ('stderr', child.stderr)):
                os.set_blocking(source.fileno(), False)
                selector.register(source, selectors.EVENT_READ, name)
            while selector.get_map():
                remaining = timeout - (time.monotonic() - started)
                require(remaining > 0, label + ': command deadline exceeded')
                for key, _ in selector.select(min(remaining, 0.1)):
                    data = os.read(key.fileobj.fileno(), 65536)
                    if not data:
                        selector.unregister(key.fileobj)
                        continue
                    saved = captures[key.data]
                    available = OUTPUT_LIMIT - len(saved)
                    streams[key.data].write(data[:available])
                    saved.extend(data[:available])
                    require(len(data) <= available, label + ': command output bound exceeded')
            remaining = timeout - (time.monotonic() - started)
            require(remaining > 0, label + ': command deadline exceeded')
            row['exit_status'] = child.wait(timeout=remaining)
            require(row['exit_status'] == 0, label + ': real command failed; inspect ' + str(folder))
            row['status'] = 'passed'
            return bytes(captures['stdout'])
        except BaseException as error:
            row.update(status='failed', failure=type(error).__name__ + ': ' + str(error))
            if child is not None:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait(timeout=10)
                row['exit_status'] = child.returncode
            raise
        finally:
            selector.close()
            if child is not None:
                child.stdout.close()
                child.stderr.close()
            for name, stream in streams.items():
                stream.close()
                row[name] = file_identity(folder / name)
            row['seconds'] = time.monotonic() - started
            self.save()


def project_config(app):
    document = tomllib.loads((app / 'pyproject.toml').read_text(encoding='utf-8'))
    require(document['build-system'] == {'requires': [], 'build-backend': 'oxid_backend',
                                         'backend-path': ['build_backend']}, 'unexpected PEP 517 backend')
    return {**document['project'], **document['tool']['oxid']}


def snapshot(app, destination):
    config = project_config(app)
    names = (*BACKEND_FILES, *config['sources'])
    require(len(names) <= 256 and len(set(names)) == len(names), 'source inventory is not closed')
    destination.mkdir()
    total, inventory = 0, {}
    for name in names:
        member_name(name)
        source = app / name
        require(all(not part.is_symlink() for part in (source, *source.parents)), 'source links are forbidden')
        info = source.stat()
        require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and info.st_size <= 1024 * 1024,
                'source is not a bounded ordinary file')
        body = source.read_bytes()
        total += len(body)
        require(total <= SOURCE_LIMIT, 'source inventory exceeds 8 MiB')
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(body)
        inventory[name] = {'bytes': len(body), 'sha256': sha(body)}
    return config, inventory


def variant(source, destination, suffix, body, fuel=None, helpers=None, profile=None):
    shutil.copytree(source, destination)
    config = project_config(destination)
    config['name'] += '-' + suffix
    config['module'] += '_' + suffix
    if fuel is not None:
        config['fuel'] = fuel
    if profile is not None:
        config['profile'] = profile
    require(len(config['sources']) == 1, 'qualification variants require the published single-source example')
    (destination / config['source']).write_text(body, encoding='utf-8')
    for name, contents in ({} if helpers is None else helpers).items():
        member_name(name)
        require(name not in config['sources'] and not (destination / name).exists(),
                'qualification helper would replace an existing source')
        path = destination / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding='utf-8')
        config['sources'].append(name)
    parts = ['[build-system]', 'requires = []', 'build-backend = "oxid_backend"',
             'backend-path = ["build_backend"]', '', '[project]']
    for name in ('name', 'version', 'description', 'requires-python'):
        parts.append(name + ' = ' + json.dumps(config[name]))
    parts.extend(('', '[tool.oxid]'))
    for name in ('module', 'source', 'sources', 'export', 'profile', 'fuel'):
        parts.append(name + ' = ' + json.dumps(config[name]))
    (destination / 'pyproject.toml').write_text('\n'.join(parts) + '\n', encoding='utf-8')
    return config


HOOK = r'''
import importlib, json, sys, tomllib
from pathlib import Path
config = tomllib.loads(Path('pyproject.toml').read_text())['build-system']
sys.path[:0] = [str((Path.cwd() / path).resolve()) for path in config['backend-path']]
backend = importlib.import_module(config['build-backend'])
if sys.argv[1] == 'metadata':
    assert backend.get_requires_for_build_wheel() == []
    assert backend.get_requires_for_build_sdist() == []
    name = backend.prepare_metadata_for_build_wheel(sys.argv[2])
else:
    name = backend.build_sdist(sys.argv[2])
print(json.dumps({'hook': sys.argv[1], 'filename': name, 'checks': 3 if sys.argv[1] == 'metadata' else 1}))
'''


def inspect_sdist(path, source_inventory):
    limit = SOURCE_LIMIT + 1024 * 1024
    require(path.stat().st_size <= limit, 'sdist exceeds compressed artifact bound')
    # PAX extension records are parsed inside tarfile before they become visible
    # members. Bound the complete decompressed stream before that parser runs.
    with path.open('rb') as compressed, gzip.GzipFile(fileobj=compressed, mode='rb') as stream:
        expanded = stream.read(limit + 1)
    require(len(expanded) <= limit, 'sdist exceeds decompressed artifact bound')
    found, folded, total = {}, set(), 0
    with tarfile.open(fileobj=io.BytesIO(expanded), mode='r:') as archive:
        for member in archive:
            require(member.isfile() and not member.issym() and not member.islnk(), 'sdist contains a non-file')
            parts = member_name(member.name).parts
            require(len(parts) > 1 and parts[0] == path.name.removesuffix('.tar.gz')
                    and member.size <= 1024 * 1024, 'invalid sdist member')
            name = '/'.join(parts[1:])
            require(name.casefold() not in folded and len(found) < 257, 'sdist duplicates or exceeds its roster')
            folded.add(name.casefold())
            total += member.size
            require(total <= SOURCE_LIMIT + 1024 * 1024, 'sdist uncompressed bound exceeded')
            with archive.extractfile(member) as stream:
                body = stream.read(member.size + 1)
            require(len(body) == member.size, 'sdist member length changed')
            found[name] = {'bytes': len(body), 'sha256': sha(body)}
    require(set(found) == set(source_inventory) | {'PKG-INFO'}, 'sdist source inventory differs')
    require(all(found[name] == identity for name, identity in source_inventory.items()), 'sdist source bytes differ')
    return found


def inspect_wheel(path, config, expected_tag, prepared=None):
    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
            and info.st_size <= ARCHIVE_LIMIT and path.name.endswith('-' + expected_tag + '.whl'),
            'wheel filename does not describe this interpreter/host')
    with path.open('rb') as stream:
        payload = stream.read(ARCHIVE_LIMIT + 1)
    require(len(payload) <= ARCHIVE_LIMIT, 'wheel grew beyond its archive bound')
    preflight_zip_directory(payload)
    files, folded, total = {}, set(), 0
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        rows = archive.infolist()
        require(1 <= len(rows) <= 32, 'wheel central-directory census exceeds its bound')
        # Finish the complete central-directory preflight before reading even
        # the first payload. A later unsafe member cannot bypass this phase.
        for row in rows:
            member_name(row.filename)
            limit = FILE_LIMIT if row.filename.endswith('.so') else 1024 * 1024
            if row.filename.endswith('.h'):
                limit = 64 * 1024
            require(row.filename.casefold() not in folded
                    and row.file_size <= limit and not row.is_dir()
                    and stat.S_IFMT(row.external_attr >> 16) in (0, stat.S_IFREG), 'unsafe wheel member')
            folded.add(row.filename.casefold())
            total += row.file_size
            require(total <= ARCHIVE_LIMIT, 'wheel uncompressed bound exceeded')
        for row in rows:
            files[row.filename] = archive.read(row)
    package = config['module'] + '/'
    private = package + '_oxid_native/'
    distribution = config['name'].replace('-', '_') + '-' + config['version'] + '.dist-info/'
    extension = package + '_native' + sysconfig.get_config_var('EXT_SUFFIX')
    provenance = json.loads(files[private + 'build.json'])
    native = json.loads(files[private + 'metadata.json'])
    library = private + provenance['library_filename']
    expected = {package + '__init__.py', extension, library, private + 'oxid_library.h',
                private + 'metadata.json', private + 'build.json', distribution + 'METADATA',
                distribution + 'WHEEL', distribution + 'RECORD'}
    require(set(files) == expected, 'wheel members are not the exact installed tuple')
    require(provenance['schema'] == 'oxid-python-native-build-v1'
            and provenance['wheel_tag'] == expected_tag
            and provenance['abi3_qualified'] is False and provenance['manylinux_qualified'] is False,
            'provenance claims an unqualified ABI or platform')
    require(isinstance(provenance['commands'], list) and len(provenance['commands']) == 3
            and all(row['exit_code'] == 0 and row['argv'] for row in provenance['commands']),
            'wheel lacks real successful build command receipts')
    require(provenance['library_filename'] == 'liboxid_' + sha(files[library]) + '.so'
            and provenance['library_sha256'] == sha(files[library])
            and provenance['extension_sha256'] == sha(files[extension]), 'native provenance tuple differs')
    require(native['library_sha256'] == sha(files[library])
            and native['header_sha256'] == sha(files[private + 'oxid_library.h'])
            and native['export'] == config['export'] and native['fuel'] == config['fuel']
            and native['profile'] == config['profile'],
            'wheel does not bind the configured native bundle')
    require(files[library].startswith(b'\x7fELF\x02\x01\x01')
            and files[extension].startswith(b'\x7fELF\x02\x01\x01'), 'wheel does not contain real ELF extensions')
    metadata = BytesParser().parsebytes(files[distribution + 'METADATA'])
    wheel = BytesParser().parsebytes(files[distribution + 'WHEEL'])
    require(metadata['Name'] == config['name'] and metadata['Version'] == config['version']
            and metadata['Requires-Python'] == '>=3.11', 'wheel project metadata differs')
    require(wheel['Root-Is-Purelib'] == 'false' and wheel.get_all('Tag') == [expected_tag], 'wheel tag is not exact')
    if prepared is not None:
        for name in ('METADATA', 'WHEEL'):
            require(files[distribution + name] == (prepared / name).read_bytes(), 'metadata hook and wheel differ')
    rows = list(csv.reader(io.StringIO(files[distribution + 'RECORD'].decode('utf-8'))))
    require(len(rows) == len(files) and all(len(row) == 3 for row in rows)
            and len({row[0] for row in rows}) == len(rows), 'RECORD census differs')
    require({row[0] for row in rows} == set(files), 'RECORD omits or adds members')
    for name, digest, size in rows:
        if name == distribution + 'RECORD':
            require(digest == size == '', 'RECORD must have an empty self hash')
        else:
            encoded = base64.urlsafe_b64encode(hashlib.sha256(files[name]).digest()).rstrip(b'=').decode()
            require(digest == 'sha256=' + encoded and size == str(len(files[name])), 'RECORD hash/size differs')
    return {'wheel': file_identity(path), 'tag': expected_tag, 'module': config['module'],
            'export': config['export'][7:], 'library': provenance['library_filename'],
            'members': {name: {'bytes': len(body), 'sha256': sha(body)} for name, body in files.items()},
            'source_manifest': provenance['source_manifest'], 'build_commands': provenance['commands'],
            'native_sources': native['sources'], 'profile': native['profile'],
            'c_compiler': provenance['c_compiler']}


def preflight_zip_directory(payload):
    """Bound directory parsing before ZipFile can allocate its entry objects.

    The producer uses ordinary, uncommented, non-ZIP64 archives without extra
    fields. The entry count in EOCD alone is insufficient: require the actual
    closed directory records to match it before invoking the ZIP parser.
    """
    require(len(payload) >= 22, 'missing ZIP end record')
    signature, disk, directory_disk, disk_count, count, size, offset, comment = struct.unpack_from(
        '<4s4H2IH', payload, len(payload) - 22)
    require(signature == b'PK\x05\x06' and disk == directory_disk == comment == 0
            and disk_count == count and 1 <= count <= 32 and size <= 64 * 1024
            and offset + size == len(payload) - 22,
            'ZIP directory count, extent or format exceeds the closed producer contract')
    end, observed = offset + size, 0
    while offset < end:
        require(observed < count and end - offset >= 46
                and payload[offset:offset + 4] == b'PK\x01\x02', 'invalid ZIP directory census')
        name, extra, annotation = struct.unpack_from('<HHH', payload, offset + 28)
        require(1 <= name <= 1024 and extra == annotation == 0,
                'ZIP directory names or extra records exceed the producer contract')
        offset += 46 + name
        require(offset <= end, 'truncated ZIP directory record')
        observed += 1
    require(offset == end and observed == count, 'ZIP actual and declared directory counts differ')


PROBE = r'''
import concurrent.futures, ctypes, gc, importlib, json, os, pathlib, sys, sysconfig, threading, types
spec = json.loads(sys.argv[1])
checks = calls = 0
def check(value, message):
    global checks
    checks += 1
    if not value: raise AssertionError(message)
def invoke(function, *arguments):
    global calls
    value = function(*arguments)
    calls += 1
    return value
def rejects(kind, function, *arguments):
    try: invoke(function, *arguments)
    except kind: check(True, 'expected native/Python error')
    else: raise AssertionError('expected ' + kind.__name__)
check(sys.implementation.name == 'cpython', 'real CPython required')
prefix = pathlib.Path(sys.prefix).resolve()
site = pathlib.Path(sysconfig.get_path('platlib')).resolve()
# Preload an independently compiled original library globally. ctypes never
# calls either export; all qualification calls go through CPython C functions.
preload = spec['preload']
library = site / preload['module'] / '_oxid_native' / preload['library']
handle = ctypes.CDLL(str(library), mode=os.RTLD_NOW | os.RTLD_GLOBAL)
functions = {}
for row in spec['packages']:
    package = importlib.import_module(row['module'])
    native = importlib.import_module(row['module'] + '._native')
    check(pathlib.Path(native.__file__).resolve().is_relative_to(prefix), 'extension escapes relocated venv')
    check(native.__file__.endswith(sysconfig.get_config_var('EXT_SUFFIX')), 'actual CPython extension suffix required')
    function = getattr(package, row['export'])
    check(function is getattr(native, row['export']) and isinstance(function, types.BuiltinFunctionType), 'Python fallback instead of native function')
    functions[row['module']] = function
main, peer, exhausted, zero, zerolow, wide, modules = [functions[row['module']] for row in spec['packages']]
check(invoke(main, 1250, 3, 2) == 1875, 'pricing result')
check(invoke(peer, 1250, 3, 2) == 1255, 'independent symbol binding')
check(invoke(modules, 1250, 3, 2) == 1875, 'declared helper module source closure')
for arguments in ((), (1, 2), (1, 2, 3, 4)):
    rejects(TypeError, main, *arguments)
for bad in (True, False, None, '1', 1.0, object()):
    rejects(TypeError, main, bad, 3, 2)
for bad in (-(2**31)-1, 2**31, 2**100):
    rejects(OverflowError, main, bad, 3, 2)
rejects(OverflowError, main, 2**31-1, 2, 1)
rejects(ZeroDivisionError, main, 1, 2, 0)
rejects(RuntimeError, exhausted, 1, 2, 1)
check(invoke(zero) == 73, 'zero-arity native binding')
rejects(RuntimeError, zerolow)
check(invoke(wide, *range(1, 65)) == 652, '64-arity weighted register/stack native binding')
rejects(TypeError, wide, *range(1, 64))
ready = threading.Barrier(5)
resume = threading.Event()
active_gc_checks = 0
def worker(index):
    first = main(1250, 3, 2)
    if index < 4:
        ready.wait(timeout=10)
        if not resume.wait(timeout=30): raise AssertionError('lifetime coordination deadline')
    return first, peer(1250, 3, 2)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
    futures = [pool.submit(worker, index) for index in range(128)]
    try:
        ready.wait(timeout=10)
        for _ in range(16):
            # Four futures have completed one genuine native call and remain
            # active at a barrier. This observes GC with live future/module
            # references; it does not claim timed overlap inside the native call.
            check(all(future.running() and not future.done() for future in futures[:4]), 'no active lifetime futures')
            for row in spec['packages']:
                sys.modules.pop(row['module'], None)
                sys.modules.pop(row['module'] + '._native', None)
            gc.collect()
            active_gc_checks += 1
            check(invoke(main, 1250, 3, 2) == 1875 and invoke(peer, 1250, 3, 2) == 1255, 'module/GC lifetime with active futures')
    finally:
        resume.set()
    for future in futures:
        value = future.result(timeout=30)
        calls += 2
        check(value == (1875, 1255), 'concurrent independent native calls')
for row in spec['packages'][:2]:
    package = importlib.import_module(row['module'])
    importlib.reload(package)
    check(invoke(getattr(package, row['export']), 1250, 3, 2) == row['expected'], 'import/reload')
import _xxsubinterpreters as interpreters
for _ in range(3):
    try:
        interpreter = interpreters.create(isolated=False)
    except TypeError as error:
        raise RuntimeError('shared-GIL lifecycle API seam: this host does not accept explicit create(isolated=False); classify the API before qualifying lifecycle support') from error
    try:
        code = 'import sys; sys.path.insert(0, ' + repr(str(site)) + '); import ' + spec['packages'][0]['module'] + ' as package; assert package.' + spec['packages'][0]['export'] + '(1250,3,2)==1875'
        interpreters.run_string(interpreter, code)
        checks += 1
        calls += 1
    finally: interpreters.destroy(interpreter)
check(checks > 0 and calls > 0, 'empty qualification is forbidden')
print(json.dumps({'status': 'passed', 'checks': checks, 'native_calls': calls, 'active_future_gc_checks': active_gc_checks, 'subinterpreter_mode': 'explicit-shared-GIL-isolated-False', 'executable': sys.executable, 'prefix': str(prefix)}))
'''


LOADER_DENIAL = r'''
import gc, importlib, json, pathlib, sys, types
module, export, mode, target, backup = sys.argv[1:]
target, backup = pathlib.Path(target), pathlib.Path(backup)
checks = synthetic_calls = recovery_calls = 0
if mode in ('status4', 'status99'):
    package = importlib.import_module(module)
    function = getattr(package, export)
    assert isinstance(function, types.BuiltinFunctionType)
    try:
        function(1250, 3, 2)
    except SystemError:
        checks += 1
        synthetic_calls += 1
    else:
        raise AssertionError('synthetic invalid/unknown status was accepted')
else:
    try:
        importlib.import_module(module)
    except ImportError:
        checks += 1
    else:
        raise AssertionError('missing, wrong-ABI or missing-symbol private library was imported')
    # Only an already failed initialization is retried in this process. Move
    # whole files; never overwrite or truncate a potentially mapped inode.
    for name in (module, module + '._native'):
        sys.modules.pop(name, None)
    gc.collect()
    if target.exists():
        rejected = target.with_name(target.name + '.rejected-' + mode)
        assert not rejected.exists()
        target.rename(rejected)
    backup.rename(target)
    importlib.invalidate_caches()
    package = importlib.import_module(module)
    function = getattr(package, export)
    assert isinstance(function, types.BuiltinFunctionType)
    assert function(1250, 3, 2) == 1875
    checks += 2
    recovery_calls += 1
print(json.dumps({'status': 'passed', 'mode': mode, 'denial_checks': checks,
                  'synthetic_native_calls': synthetic_calls, 'genuine_recovery_calls': recovery_calls,
                  'counts_as_production_qualification': False}))
'''


def loader_denials(evidence, python, row, cwd, runtime_environment, build_environment):
    """Compile explicit denial fixtures, separately from every genuine wheel.

    Fixture exports are never a wheel producer, runtime fallback, or evidence of
    native production semantics. Their results never enter native_calls.
    """
    folder = evidence.output / 'negative-loader-fixtures'
    folder.mkdir()
    compiler = row['c_compiler']
    require(Path(compiler['path']).resolve(strict=True) == Path(compiler['resolved'])
            and file_identity(compiler['resolved'])['sha256'] == compiler['sha256'],
            'selected Clang identity changed before denial-fixture compilation')
    blocked = {'CC', 'CFLAGS', 'CPPFLAGS', 'LDFLAGS', 'CPATH', 'C_INCLUDE_PATH',
               'CPLUS_INCLUDE_PATH', 'OBJC_INCLUDE_PATH', 'LIBRARY_PATH', 'COMPILER_PATH',
               'GCC_EXEC_PREFIX', 'CCC_OVERRIDE_OPTIONS', 'CLANG_CONFIG_FILE',
               'CLANG_CONFIG_FILE_SYSTEM_DIR', 'CLANG_CONFIG_FILE_USER_DIR', 'SDKROOT',
               'LD_PRELOAD', 'LD_LIBRARY_PATH', 'LD_AUDIT', 'LD_RUN_PATH', 'LD_DEBUG'}
    fixture_environment = {key: value for key, value in build_environment.items() if key not in blocked}
    signature = 'int32_t oxid_library_call_v1(int32_t a,int32_t b,int32_t c,int32_t *out)'
    unused = '(void)a;(void)b;(void)c;(void)out;'
    version = 'uint32_t oxid_library_abi_version(void){return 1;}\n'
    sources = {
        'wrong-abi': '#include <stdint.h>\nuint32_t oxid_library_abi_version(void){return 2;}\n'
                     + signature + '{' + unused + 'return 99;}\n',
        'missing-version': '#include <stdint.h>\n' + signature + '{' + unused + 'return 99;}\n',
        'missing-call': '#include <stdint.h>\n' + version,
        'status4': '#include <stdint.h>\n' + version + signature + '{' + unused + 'return 4;}\n',
        'status99': '#include <stdint.h>\n' + version + signature + '{' + unused + 'return 99;}\n',
    }
    artifacts = {}
    for label, source in sources.items():
        c_file = folder / (label + '.c')
        library = folder / (label + '.so')
        c_file.write_text(source)
        argv = [compiler['path'], '--no-default-config', '--target=' + platform.machine() + '-unknown-linux-gnu',
                '-std=c11', '-shared', '-fPIC', '-O2', '-Wall', '-Wextra', '-Werror',
                str(c_file), '-o', str(library)]
        evidence.command('denial-build-' + label, argv, folder, fixture_environment, 60,
                         purpose='synthetic-negative-fixture-build')
        require(library.stat().st_size <= FILE_LIMIT, 'negative fixture exceeds its bound')
        artifacts[label] = library
    raw = evidence.command('installed-native-path', [str(python), '-I', '-B', '-c',
                           'import json,sysconfig;print(json.dumps(sysconfig.get_path("platlib")))'],
                           cwd, runtime_environment, 30, purpose='installed-path-inspection')
    site = Path(json.loads(raw)).resolve(strict=True)
    prefix = python.parent.parent.resolve(strict=True)
    require(site.is_relative_to(prefix), 'installed package directory escapes relocated venv')
    target = site / row['module'] / '_oxid_native' / row['library']
    expected_hash = row['members'][row['module'] + '/_oxid_native/' + row['library']]['sha256']
    require(file_identity(target)['sha256'] == expected_hash, 'installed genuine library differs before denial probes')
    genuine = folder / 'genuine-original.so'
    shutil.copyfile(target, genuine)
    require(file_identity(genuine)['sha256'] == expected_hash, 'genuine recovery copy differs')
    evidence.report['negative_loader_probes'] = []
    for label in ('missing-file', *sources):
        backup = target.with_name(target.name + '.genuine-preserved')
        staged = target.with_name(target.name + '.denial-staged')
        require(not backup.exists() and not staged.exists(), 'denial swap path already exists')
        # The prior child has completed. Keep the genuine inode intact and
        # publish any denial file by rename, never writing a loaded target.
        target.rename(backup)
        try:
            if label != 'missing-file':
                shutil.copyfile(artifacts[label], staged)
                staged.rename(target)
            raw = evidence.command('loader-denial-' + label,
                                   [str(python), '-I', '-B', '-c', LOADER_DENIAL,
                                    row['module'], row['export'], label, str(target), str(backup)],
                                   cwd, runtime_environment, 60, purpose='synthetic-negative-loader-probe')
            result = json.loads(raw)
            require(result['status'] == 'passed' and result['mode'] == label
                    and result['denial_checks'] > 0 and result['counts_as_production_qualification'] is False,
                    'loader denial did not actually execute')
            evidence.report['negative_loader_probes'].append(result)
        finally:
            # This child has completed or been killed/waited by command(). A
            # failed retry cannot strand the installed package on a fixture.
            if backup.exists():
                if target.exists():
                    target.rename(folder / ('retired-' + label + '.so'))
                backup.rename(target)
            elif not target.exists() or file_identity(target)['sha256'] != expected_hash:
                shutil.copyfile(genuine, staged)
                staged.rename(target)
            if staged.exists():
                staged.unlink()
            require(file_identity(target)['sha256'] == expected_hash, 'failed to restore genuine library bytes')
            evidence.save()
    require(file_identity(compiler['resolved'])['sha256'] == compiler['sha256'],
            'selected Clang changed during denial-fixture compilation')
    evidence.report['negative_fixture_artifacts'] = {
        label: {'source': file_identity(folder / (label + '.c')), 'library': file_identity(path)}
        for label, path in artifacts.items()}


LIFECYCLE_FIXTURE = r'''
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>

/* Synthetic transport fixture, never Oxid producer/semantic evidence. */
static int final_fd = -1;
static int pipe_fd(const char *name) {
    const char *text = getenv(name);
    if (text == NULL || *text == '\0') return -1;
    char *end = NULL;
    errno = 0;
    long value = strtol(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0' || value < 0 || value > INT_MAX) return -1;
    return (int)value;
}
static int send_one(int descriptor, char byte) {
    for (int attempts = 0; attempts < 32; ++attempts) {
        ssize_t count = write(descriptor, &byte, 1);
        if (count == 1) return 0;
        if (count < 0 && errno == EINTR) continue;
        return -1;
    }
    return -1;
}
uint32_t oxid_library_abi_version(void) { return 1; }
int32_t oxid_library_call_v1(int32_t a, int32_t b, int32_t c, int32_t *out) {
    (void)a; (void)b; (void)c;
    int entered = pipe_fd("OXID_LIFECYCLE_ENTERED_FD");
    int gate = pipe_fd("OXID_LIFECYCLE_GATE_FD");
    final_fd = pipe_fd("OXID_LIFECYCLE_CLOSED_FD");
    if (entered < 0 || gate < 0 || final_fd < 0) return 99;
    if (out == NULL) return 4;
    if (send_one(entered, 'E') != 0) return 99;
    for (int attempts = 0; attempts < 32; ++attempts) {
        char release = 0;
        ssize_t count = read(gate, &release, 1);
        if (count < 0 && errno == EINTR) continue;
        if (count != 1 || release != 'G') return 99;
        *out = 1875;
        return 0;
    }
    return 99;
}
__attribute__((destructor)) static void closed(void) {
    if (final_fd >= 0) (void)send_one(final_fd, 'C');
}
'''


LIFECYCLE_PROBE = r'''
import gc, importlib, json, os, select, sys, threading, types, weakref
name, export = sys.argv[1:]
checks = 0
def check(condition, message):
    global checks
    checks += 1
    if not condition: raise AssertionError(message)
entered_read, entered_write = os.pipe()
gate_read, gate_write = os.pipe()
closed_read, closed_write = os.pipe()
descriptors = [entered_read, entered_write, gate_read, gate_write, closed_read, closed_write]
os.environ['OXID_LIFECYCLE_ENTERED_FD'] = str(entered_write)
os.environ['OXID_LIFECYCLE_GATE_FD'] = str(gate_read)
os.environ['OXID_LIFECYCLE_CLOSED_FD'] = str(closed_write)
results, errors, functions = [], [], []
worker = None
try:
    package = importlib.import_module(name)
    native = importlib.import_module(name + '._native')
    function = getattr(package, export)
    check(function is getattr(native, export) and isinstance(function, types.BuiltinFunctionType),
          'lifecycle test must call the genuine produced C extension')
    reference = weakref.ref(native)
    functions.append(function)
    del function
    def invoke():
        try:
            # pop() leaves no control-side callable/second library handle.
            results.append(functions.pop()(1250, 3, 2))
        except BaseException as error:
            errors.append(type(error).__name__ + ': ' + str(error))
    worker = threading.Thread(target=invoke, daemon=True)
    worker.start()
    check(bool(select.select([entered_read], [], [], 10)[0]), 'native entry did not signal before its deadline')
    check(os.read(entered_read, 2) == b'E', 'native entry signal differs')
    for module_name in (name, name + '._native'):
        sys.modules.pop(module_name, None)
    del native, package
    check(functions == [], 'control-side callable reference remains')
    for _ in range(8):
        gc.collect()
        check(worker.is_alive(), 'native call stopped before the gate release')
        check(reference() is not None, 'module died during its actual GIL-free native call')
        check(not select.select([closed_read], [], [], 0)[0], 'private handle closed during its blocked call')
    check(os.write(gate_write, b'G') == 1, 'could not release the native gate')
    worker.join(timeout=10)
    check(not worker.is_alive() and not errors and results == [1875], 'blocked transport call did not return correctly')
    for _ in range(8):
        gc.collect()
        if reference() is None:
            break
    check(reference() is None, 'last owned module reference was not released after the call')
    check(bool(select.select([closed_read], [], [], 5)[0]), 'last module did not close its private library')
    close_events = os.read(closed_read, 16)
    check(close_events == b'C', 'private library destructor count differs from one')
    gc.collect()
    check(not select.select([closed_read], [], [], 0.05)[0], 'private library closed more than once')
    print(json.dumps({'schema': 'oxid-python-synthetic-lifecycle-v1', 'status': 'passed',
                      'checks': checks, 'synthetic_blocked_calls': 1, 'close_events': 1,
                      'entered_before_gc': True, 'closed_while_blocked': False,
                      'module_collected_after_call': True, 'no_secondary_dlopen_handle': True,
                      'oxid_producer_qualified': False, 'oxid_semantics_qualified': False,
                      'genuine_native_calls': 0, 'counts_as_production_qualification': False}))
finally:
    # If any assertion fails, EOF releases a read-blocked native call before
    # descriptors are closed. A daemon thread cannot strand process teardown;
    # the parent also bounds/kills/reaps this whole child process group.
    try:
        os.write(gate_write, b'G')
    except OSError:
        pass
    try:
        os.close(gate_write)
    except OSError:
        pass
    if worker is not None:
        worker.join(timeout=2)
    for descriptor in descriptors:
        try:
            os.close(descriptor)
        except OSError:
            pass
'''


def lifecycle_transport(evidence, python, row, cwd, runtime_environment, build_environment):
    """A deterministic synthetic lifetime oracle; no genuine semantics claim."""
    folder = evidence.output / 'synthetic-lifecycle-transport'
    folder.mkdir()
    compiler = row['c_compiler']
    require(Path(compiler['path']).resolve(strict=True) == Path(compiler['resolved'])
            and file_identity(compiler['resolved'])['sha256'] == compiler['sha256'],
            'selected Clang identity changed before lifecycle fixture compilation')
    blocked = {'CC', 'CFLAGS', 'CPPFLAGS', 'LDFLAGS', 'CPATH', 'C_INCLUDE_PATH',
               'CPLUS_INCLUDE_PATH', 'OBJC_INCLUDE_PATH', 'LIBRARY_PATH', 'COMPILER_PATH',
               'GCC_EXEC_PREFIX', 'CCC_OVERRIDE_OPTIONS', 'CLANG_CONFIG_FILE',
               'CLANG_CONFIG_FILE_SYSTEM_DIR', 'CLANG_CONFIG_FILE_USER_DIR', 'SDKROOT',
               'LD_PRELOAD', 'LD_LIBRARY_PATH', 'LD_AUDIT', 'LD_RUN_PATH', 'LD_DEBUG'}
    fixture_environment = {key: value for key, value in build_environment.items() if key not in blocked}
    c_file, library = folder / 'lifecycle.c', folder / 'lifecycle.so'
    c_file.write_text(LIFECYCLE_FIXTURE)
    argv = [compiler['path'], '--no-default-config', '--target=' + platform.machine() + '-unknown-linux-gnu',
            '-std=c11', '-shared', '-fPIC', '-O2', '-Wall', '-Wextra', '-Werror',
            str(c_file), '-o', str(library)]
    evidence.command('lifecycle-fixture-build', argv, folder, fixture_environment, 60,
                     purpose='synthetic-lifecycle-transport-only')
    require(library.stat().st_size <= FILE_LIMIT, 'lifecycle fixture exceeds its artifact bound')
    raw = evidence.command('lifecycle-installed-native-path', [str(python), '-I', '-B', '-c',
                           'import json,sysconfig;print(json.dumps(sysconfig.get_path("platlib")))'],
                           cwd, runtime_environment, 30, purpose='installed-path-inspection')
    site = Path(json.loads(raw)).resolve(strict=True)
    require(site.is_relative_to(python.parent.parent.resolve(strict=True)), 'lifecycle package escapes relocated venv')
    target = site / row['module'] / '_oxid_native' / row['library']
    expected_hash = row['members'][row['module'] + '/_oxid_native/' + row['library']]['sha256']
    require(file_identity(target)['sha256'] == expected_hash, 'genuine library differs before lifecycle fixture')
    backup = target.with_name(target.name + '.lifecycle-genuine')
    staged = target.with_name(target.name + '.lifecycle-staged')
    require(not backup.exists() and not staged.exists(), 'lifecycle swap path already exists')
    target.rename(backup)
    try:
        shutil.copyfile(library, staged)
        staged.rename(target)
        raw = evidence.command('deterministic-native-lifetime', [str(python), '-I', '-B', '-c',
                               LIFECYCLE_PROBE, row['module'], row['export']],
                               cwd, runtime_environment, 60, purpose='synthetic-lifecycle-transport-only')
        result = json.loads(raw)
        require(result['status'] == 'passed' and result['checks'] > 0
                and result['synthetic_blocked_calls'] == result['close_events'] == 1
                and result['genuine_native_calls'] == 0
                and result['oxid_producer_qualified'] is result['oxid_semantics_qualified'] is False
                and result['counts_as_production_qualification'] is False,
                'synthetic lifecycle transport did not execute its complete separate contract')
        evidence.report['synthetic_lifecycle_transport'] = result
    finally:
        # No child remains active when its mapped fixture inode is retired.
        if target.exists():
            target.rename(folder / 'retired-installed-fixture.so')
        backup.rename(target)
        if staged.exists():
            staged.unlink()
        require(file_identity(target)['sha256'] == expected_hash, 'failed to restore genuine lifecycle target')
        evidence.save()
    evidence.report['synthetic_lifecycle_artifacts'] = {'source': file_identity(c_file), 'library': file_identity(library)}
    require(file_identity(compiler['resolved'])['sha256'] == compiler['sha256'],
            'selected Clang changed during lifecycle fixture compilation')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, default=ROOT / 'apps/python-pricing')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    tag = actual_tag()
    output = args.output.absolute()
    require(not output.exists() and not output.is_symlink() and ROOT not in output.parents,
            'evidence directory must be new and outside this repository')
    output.mkdir(parents=True)
    evidence = Evidence(output)
    try:
        environment = dict(os.environ, PIP_NO_INDEX='1', PIP_DISABLE_PIP_VERSION_CHECK='1',
                           PIP_NO_CACHE_DIR='1', PIP_CONFIG_FILE=os.devnull, PYTHONDONTWRITEBYTECODE='1')
        environment.pop('PYTHONOPTIMIZE', None)
        source = output / 'source'
        config, inventory = snapshot(args.app.resolve(strict=True), source)
        evidence.report.update(tag=tag, interpreter=file_identity(Path(sys.executable).resolve()), source_inventory=inventory)
        metadata = output / 'metadata'
        metadata.mkdir()
        hook_env = dict(environment, OXID=str(output / 'absent-oxid'), OXID_PYTHON_CC=str(output / 'absent-cc'))
        raw = evidence.command('metadata-hooks', [sys.executable, '-I', '-B', '-c', HOOK, 'metadata', str(metadata)], source, hook_env, 30)
        hook = json.loads(raw)
        require(hook['checks'] == 3 and PurePosixPath(hook['filename']).name == hook['filename'], 'metadata hooks did not execute')
        prepared = metadata / hook['filename']
        sdists = output / 'sdists'
        sdists.mkdir()
        raw = evidence.command('sdist-hook', [sys.executable, '-I', '-B', '-c', HOOK, 'sdist', str(sdists)], source, hook_env, 30)
        sdist = sdists / json.loads(raw)['filename']
        evidence.report['sdist'] = {'archive': file_identity(sdist), 'members': inspect_sdist(sdist, inventory)}
        packages = [(source, config)]
        export = config['export'][7:]
        peer = variant(source, output / 'source-peer', 'peer', f'pub fn {export}(a:i32,b:i32,c:i32)->i32{{return a+b+c;}}\n', profile='dev')
        exhausted = variant(source, output / 'source-fuel', 'fuel', (source / config['source']).read_text(), fuel=0)
        zero_source = f'pub fn {export}()->i32{{return 73;}}\n'
        zero = variant(source, output / 'source-zero', 'zero', zero_source, fuel=4)
        zerolow = variant(source, output / 'source-zerolow', 'zerolow', zero_source, fuel=3)
        parameters = ','.join(f'a{i}:i32' for i in range(64))
        wide = variant(source, output / 'source-wide', 'wide', f'pub fn {export}({parameters})->i32{{return a0+a7*2+a8*3+a31*5+a63*7;}}\n')
        modules_source = f'mod math;\nuse crate::math::subtotal as multiply;\npub fn {export}(a:i32,b:i32,c:i32)->i32{{return multiply(a,b)/c;}}\n'
        modules_helper = 'pub fn subtotal(unit:i32,quantity:i32)->i32{return unit*quantity;}\n'
        modules = variant(source, output / 'source-modules', 'modules', modules_source,
                          helpers={'src/math.ox': modules_helper})
        packages.extend((output / ('source-' + suffix), row) for suffix, row in
                        (('peer', peer), ('fuel', exhausted), ('zero', zero), ('zerolow', zerolow),
                         ('wide', wide), ('modules', modules)))
        wheel_rows, wheels = [], []
        for index, (project, row) in enumerate(packages):
            wheelhouse = output / 'wheels' / str(index)
            wheelhouse.mkdir(parents=True)
            evidence.command('pip-wheel-' + str(index), [sys.executable, '-m', 'pip', 'wheel', '--no-deps', '--no-index', str(project), '-w', str(wheelhouse)], output, environment)
            produced = list(wheelhouse.glob('*.whl'))
            require(len(produced) == 1, 'pip must produce exactly one wheel')
            wheel = inspect_wheel(produced[0], row, tag, prepared if index == 0 else None)
            if row['module'] == modules['module']:
                # Names are specified by this public fixture, not inferred
                # from the producer's observed inventory or path heuristic.
                expected_sources = {'pricing.ox': sha(modules_source.encode()),
                                    'math.ox': sha(modules_helper.encode())}
                require(wheel['source_manifest'] == expected_sources
                        and isinstance(wheel['native_sources'], list) and len(wheel['native_sources']) == 2
                        and all(isinstance(item, dict) and set(item) == {'path', 'sha256'}
                                for item in wheel['native_sources'])
                        and {item['path']: item['sha256'] for item in wheel['native_sources']} == expected_sources,
                        'multi-file compiler closure does not match both declared source identities and hashes')
            wheel['expected'] = 1255 if index == 1 else 1875
            wheel_rows.append(wheel)
            wheels.append(produced[0])
        sdist_wheels = output / 'sdist-wheels'
        sdist_wheels.mkdir()
        evidence.command('pip-wheel-sdist', [sys.executable, '-m', 'pip', 'wheel', '--no-deps', '--no-index', str(sdist), '-w', str(sdist_wheels)], output, environment)
        produced = list(sdist_wheels.glob('*.whl'))
        require(len(produced) == 1, 'sdist rebuild must produce one real wheel')
        rebuilt = inspect_wheel(produced[0], config, tag, prepared)
        rebuilt['expected'] = 1875
        require(rebuilt['source_manifest'] == wheel_rows[0]['source_manifest'], 'sdist changed the compiler source closure')
        evidence.report['wheels'] = wheel_rows + [rebuilt]
        venv = output / 'install-venv'
        evidence.command('fresh-venv', [sys.executable, '-m', 'venv', '--copies', str(venv)], output, environment, 90)
        python = venv / 'bin/python'
        evidence.command('install-wheels', [str(python), '-m', 'pip', 'install', '--no-deps', '--no-index', *map(str, wheels)], output, environment, 90)
        for project, _ in packages:
            shutil.rmtree(project)
            require(not project.exists(), 'owned source tree was not withheld')
        relocated = output / 'relocated 安裝 with spaces' / 'venv'
        relocated.parent.mkdir()
        venv.rename(relocated)
        empty = output / 'empty-tools'
        empty.mkdir()
        runtime = {key: value for key, value in environment.items()
                   if not key.upper().startswith(('OXID', 'LLVM', 'CLANG', 'PYTHON', 'LD_', 'DYLD_', 'CARGO', 'RUST'))
                   and key not in ('CC', 'CFLAGS', 'LDFLAGS', 'LIBRARY_PATH')}
        runtime.update(PATH=str(empty), PYTHONDONTWRITEBYTECODE='1')
        python = relocated / 'bin/python'
        for index in (1, 0):
            spec = {'packages': wheel_rows, 'preload': wheel_rows[index]}
            raw = evidence.command('relocated-native-probe-' + str(index), [str(python), '-I', '-B', '-c', PROBE, json.dumps(spec)], empty, runtime, 120)
            result = json.loads(raw)
            require(result['status'] == 'passed' and type(result['checks']) is int and result['checks'] > 0
                    and type(result['native_calls']) is int and result['native_calls'] > 0, 'native probe did not execute actual checks')
            evidence.report['probes'].append(result)
            evidence.report['native_calls'] += result['native_calls']
        evidence.command('install-sdist-wheel', [str(python), '-m', 'pip', 'install', '--no-deps', '--no-index', '--force-reinstall', str(produced[0])], empty, runtime, 90)
        raw = evidence.command('relocated-sdist-native-probe', [str(python), '-I', '-B', '-c', PROBE, json.dumps({'packages': [rebuilt, *wheel_rows[1:]], 'preload': wheel_rows[1]})], empty, runtime, 120)
        result = json.loads(raw)
        require(result['checks'] > 0 and result['native_calls'] > 0 and result['status'] == 'passed', 'empty sdist qualification')
        evidence.report['probes'].append(result)
        evidence.report['native_calls'] += result['native_calls']
        require(evidence.report['native_calls'] > 0 and len(evidence.report['probes']) == 3, 'incomplete native qualification')
        loader_denials(evidence, python, rebuilt, empty, runtime, environment)
        lifecycle_transport(evidence, python, rebuilt, empty, runtime, environment)
        evidence.report['status'] = 'passed'
        evidence.save()
        print(json.dumps({'status': 'passed', 'native_calls': evidence.report['native_calls'], 'output': str(output)}))
        return 0
    except BaseException as error:
        evidence.report.update(status='failed', failure=type(error).__name__ + ': ' + str(error))
        evidence.save()
        raise


if __name__ == '__main__':
    raise SystemExit(main())
