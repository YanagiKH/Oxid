"""Producer-captured local Unit4 build environment. Importing starts no child.

Only capture_machine can issue a ToolAdmission. Serialized evidence and caller
claims cannot reconstruct one. This records relevant local host inputs; it is
neither a hermetic sandbox, a network-isolation claim, nor four-host execution
qualification. Actual capture is dormant until its shared package is sealed.
"""
import hashlib
import contextlib
import contextvars
import io
import json
import os
import platform
import re
import shutil
import stat
import struct
import sys
import tarfile
import threading
import time
import tomllib
import types
import weakref
from pathlib import Path, PurePosixPath

sys.dont_write_bytecode = True
SHARED_MANIFEST_SHA256 = '158851e13a6adffa6909665b9392a5baaa8f4f3e160f1061177400df82aa5bd5'  # Integrator seals the fixed installed package.
SHARED_FILES = ('context.py', 'preparation.py', 'prepare_worker.py',
                'test_context.py', 'test_preparation.py')
SHARED_KEY = '_oxid_package_dependency_unit4_preparation_v1'
MACHINE_KEY = '_oxid_package_dependency_unit4_machine_v1'
FROZEN_PATH = 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json'
FROZEN_SHA256 = '02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0'
SOURCE_SHA256 = 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
HOSTS = {'Linux x86_64': 'x86_64-unknown-linux-gnu',
         'macOS x86_64': 'x86_64-apple-darwin',
         'macOS arm64': 'aarch64-apple-darwin',
         'Windows x86_64': 'x86_64-pc-windows-msvc'}
ROLES = ('ordinary', 'lifecycle', 'parser_observed', 'parser_control')
WINDOWS_KEYS = ('SystemRoot', 'SYSTEMROOT', 'WINDIR', 'COMSPEC', 'PATHEXT',
                'USERPROFILE', 'ProgramFiles', 'ProgramFiles(x86)', 'ProgramW6432',
                'VSINSTALLDIR', 'VCINSTALLDIR', 'VCToolsInstallDir',
                'VSCMD_ARG_TGT_ARCH', 'VSCMD_ARG_HOST_ARCH', 'LIB', 'INCLUDE', 'LIBPATH')
INPUT_KEYS = ('PATH', 'HOME', 'CARGO_HOME', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN',
              'TEMP', 'TMP', 'TMPDIR', 'SDKROOT', 'DEVELOPER_DIR') + WINDOWS_KEYS
MAX_FILE = 1024 ** 3
MAX_TREE_BYTES = 8 * 1024 ** 3
MAX_TREE_MEMBERS = 200000
PROBE_TIMEOUT = 30
STREAM_LIMIT = 8 * 1024 ** 2
CAPTURE_TIMEOUT = 300
VERIFY_TIMEOUT = 120
_ISSUED = weakref.WeakKeyDictionary()
_CHECKPOINT = contextvars.ContextVar('unit4_machine_checkpoint', default=None)


class Rejected(ValueError):
    pass


class NotReady(Rejected):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def _tick():
    checkpoint = _CHECKPOINT.get()
    if checkpoint is not None:
        checkpoint()


@contextlib.contextmanager
def _deadline(api, invocation, seconds):
    deadline = time.monotonic() + seconds
    def checkpoint():
        api.check_deadline(invocation)
        need(time.monotonic() < deadline, 'machine capture/verification phase deadline exceeded')
    token = _CHECKPOINT.set(checkpoint)
    try:
        _tick()
        yield
        _tick()
    finally:
        _CHECKPOINT.reset(token)


class _ArchiveReader(io.BytesIO):
    def read(self, size=-1):
        _tick()
        return super().read(size)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    def nonfinite(_):
        raise Rejected('nonfinite JSON')
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=nonfinite)


def _canonical_path(path, exists=True):
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=exists) == path,
         'canonical absolute path required')
    need(not any(p.is_symlink() for p in (path, *path.parents)), 'unexpected symlink ancestry')
    return path


def _stat(info):
    return {key: getattr(info, key) for key in
            ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')}


def _file(path):
    """Hash actual bytes, with before/after identity, never a supplied digest."""
    _tick()
    path = _canonical_path(path)
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= MAX_FILE, 'bounded regular file required')
    digest = hashlib.sha256()
    size = 0
    with path.open('rb') as stream:
        need(_stat(os.fstat(stream.fileno())) == _stat(before), 'file changed before read')
        while True:
            _tick()
            part = stream.read(1024 ** 2)
            if not part:
                break
            size += len(part)
            need(size <= MAX_FILE, 'file exceeds capture bound')
            digest.update(part)
        need(_stat(os.fstat(stream.fileno())) == _stat(before), 'file changed during read')
    need(_stat(path.stat()) == _stat(before) and size == before.st_size, 'file changed after read')
    return {'path': str(path), 'bytes': size, 'sha256': digest.hexdigest(), 'stat': _stat(before)}


def _body(path, limit=64 * 1024 ** 2):
    row = _file(path)
    need(row['bytes'] <= limit, 'bounded input body required')
    output = bytearray()
    with Path(path).open('rb') as stream:
        while True:
            _tick()
            part = stream.read(min(1024 ** 2, limit + 1 - len(output)))
            if not part:
                break
            output.extend(part)
            need(len(output) <= limit, 'input body grew beyond capture bound')
    raw = bytes(output)
    need(len(raw) == row['bytes'] and sha(raw) == row['sha256'], 'input changed during reread')
    return raw


def _route(path):
    """Record discovery spelling, every link route, and resolved executable body."""
    path = Path(path)
    need(path.is_absolute() and '..' not in path.parts, 'absolute tool route required')
    links = []
    for component in reversed((path, *path.parents)):
        _tick()
        info = component.lstat()
        row = {'path': str(component), 'device': info.st_dev, 'inode': info.st_ino,
               'mode': info.st_mode}
        if stat.S_ISLNK(info.st_mode):
            row['link'] = os.readlink(component)
        else:
            need(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode), 'special route component')
        links.append(row)
    resolved = path.resolve(strict=True)
    return {'selected': str(path), 'resolved': str(resolved), 'route': links, 'body': _file(resolved)}


def _tree(root):
    """Closed bounded roster, including link routes; cycles and special files fail."""
    root = Path(root)
    need(root.is_absolute() and root.is_dir(), 'required input directory is absent')
    rows = []
    total = 0
    stack = [(root, frozenset())]
    while stack:
        _tick()
        path, ancestors = stack.pop()
        resolved = path.resolve(strict=True)
        info = path.lstat()
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            need(resolved not in ancestors, 'directory-link cycle')
            row = {'path': relative, 'kind': 'directory', 'resolved': str(resolved),
                   'device': resolved.stat().st_dev, 'inode': resolved.stat().st_ino,
                   'mode': resolved.stat().st_mode}
            if stat.S_ISLNK(info.st_mode):
                row['link'] = os.readlink(path)
            rows.append(row)
            children = []
            for child in path.iterdir():
                _tick()
                children.append(child)
                need(len(rows) + len(stack) + len(children) <= MAX_TREE_MEMBERS,
                     'input directory member bound')
            children.sort(key=lambda p: p.name, reverse=True)
            stack.extend((p, ancestors | {resolved}) for p in children)
        else:
            need(path.is_file(), 'special tree member')
            route = _route(path)
            total += route['body']['bytes']
            need(total <= MAX_TREE_BYTES, 'input tree byte bound')
            rows.append({'path': relative, 'kind': 'file', 'route': route})
        need(len(rows) + len(stack) <= MAX_TREE_MEMBERS, 'input tree member bound')
    return {'root': str(root), 'rows': sorted(rows, key=lambda row: row['path']), 'bytes': total}


def _shared():
    if SHARED_MANIFEST_SHA256 is None:
        raise NotReady('machine capture awaits the sealed shared supervisor package')
    root = _canonical_path(Path(__file__).absolute().parent.parent / 'package_dependency_unit4_current')
    raw = _body(root / 'package-manifest.json')
    need(sha(raw) == SHARED_MANIFEST_SHA256, 'fixed shared package manifest changed')
    manifest = decode(raw)
    need(manifest['schema'] == 'oxid-package-dependency-unit4-current-package-v1', 'shared package schema')
    rows = manifest['files']
    need([row['path'] for row in rows] == sorted(SHARED_FILES), 'closed shared supervisor roster')
    need(sorted(p.name for p in root.iterdir()) == sorted((*SHARED_FILES, 'package-manifest.json')),
         'shared package physical membership')
    bodies = {}
    for row in rows:
        data = _body(root / row['path'])
        need(row == {'path': row['path'], 'bytes': len(data), 'sha256': sha(data)}, 'shared body drift')
        bodies[row['path']] = data
    path = root / 'preparation.py'
    digest = sha(bodies['preparation.py'])
    module = sys.modules.get(SHARED_KEY)
    if module is not None:
        need(module.__file__ == str(path) and getattr(module, '__source_sha256__', None) == digest,
             'canonical preparation owner was replaced')
        return module
    module = types.ModuleType(SHARED_KEY)
    module.__file__, module.__source_sha256__ = str(path), digest
    sys.modules[SHARED_KEY] = module
    try:
        exec(compile(bodies['preparation.py'], str(path), 'exec'), module.__dict__)
    except BaseException:
        if sys.modules.get(SHARED_KEY) is module:
            del sys.modules[SHARED_KEY]
        raise
    return module


def measured_host():
    uname = platform.uname()
    system = {'Darwin': 'macOS'}.get(uname.system, uname.system)
    machine = {'amd64': 'x86_64', 'aarch64': 'arm64'}.get(uname.machine.lower(), uname.machine.lower())
    name = system + ' ' + machine
    need(name in HOSTS and struct.calcsize('P') == 8, 'unsupported actual Unit4 host')
    return {'name': name, 'target': HOSTS[name], 'pointer_width': 64,
            'uname': {'system': uname.system, 'release': uname.release,
                      'version': uname.version, 'machine': uname.machine},
            'python': _route(Path(sys.executable).absolute()), 'python_version': sys.version}


def _producer_environment():
    return {key: os.environ[key] for key in INPUT_KEYS if key in os.environ}


def _clean_environment(inputs, host):
    """Allowlist only; no inherited compiler wrappers, flags, credentials or config."""
    name = host['name']
    home = inputs.get('HOME') or inputs.get('USERPROFILE')
    need(home and Path(home).is_absolute(), 'producer home is missing')
    home = str(Path(home).resolve(strict=True))
    env = {'HOME': home, 'CARGO_HOME': inputs.get('CARGO_HOME', str(Path(home) / '.cargo')),
           'RUSTUP_HOME': inputs.get('RUSTUP_HOME', str(Path(home) / '.rustup')),
           'RUSTUP_TOOLCHAIN': '1.99.0', 'PYTHONDONTWRITEBYTECODE': '1',
           'PYTHONNOUSERSITE': '1', 'PYTHONOPTIMIZE': '0', 'CARGO_INCREMENTAL': '0',
           'CARGO_BUILD_JOBS': '2', 'CARGO_TERM_COLOR': 'never'}
    env.update({k: inputs[k] for k in ('TEMP', 'TMP', 'TMPDIR') if k in inputs})
    for key in ('CARGO_HOME', 'RUSTUP_HOME'):
        need(Path(env[key]).is_absolute(), 'absolute producer home required')
        env[key] = str(Path(env[key]).resolve(strict=(key == 'CARGO_HOME')))
    if name == 'Windows x86_64':
        env.update({k: inputs[k] for k in WINDOWS_KEYS if k in inputs})
        env['VSLANG'] = '1033'
        env['PATH'] = inputs.get('PATH', '')
    else:
        env.update(LANG='C.UTF-8', LC_ALL='C.UTF-8')
        env['PATH'] = '/usr/bin:/bin' if name.startswith('Linux') else '/usr/bin:/bin:/usr/sbin:/sbin'
        env.update({k: inputs[k] for k in ('SDKROOT', 'DEVELOPER_DIR') if k in inputs})
    need(env['PATH'], 'empty host discovery path')
    return env


def _which(name, path):
    need(all(p and Path(p).is_absolute() for p in path.split(os.pathsep)), 'unsafe executable search path')
    selected = shutil.which(name, path=path)
    need(selected is not None, 'required tool unavailable: ' + name)
    return _route(Path(selected).absolute())


def _config_absence(repo, run_root, env):
    roots = {Path(repo), Path(run_root), Path(env['HOME']), Path(env['CARGO_HOME'])}
    roots.update(Path(run_root) / 'source-preparation' / sub for sub in
                 ('public/ordinary', 'public/lifecycle', 'parser/observed', 'parser/control'))
    candidates = set()
    for root in roots:
        for ancestor in (root, *root.parents):
            candidates.update(ancestor / '.cargo' / name for name in ('config', 'config.toml'))
    candidates.update(Path(env['CARGO_HOME']) / name for name in ('config', 'config.toml'))
    result = []
    for path in sorted(candidates, key=str):
        _tick()
        need(not os.path.lexists(path), 'Cargo configuration is forbidden: ' + str(path))
        for parent in path.parents:
            if os.path.lexists(parent):
                need(not parent.is_symlink(), 'Cargo configuration ancestry is a link')
        result.append({'path': str(path), 'absent': True})
    return result


def _workspace_absence(run_root):
    """Reject ambient ancestor workspaces before Cargo sees any prepared cwd."""
    root = Path(run_root)
    sources = [root / 'source-preparation' / sub for sub in
               ('public/ordinary', 'public/lifecycle', 'parser/observed', 'parser/control')]
    candidates = {ancestor / 'Cargo.toml' for source in sources for ancestor in source.parents}
    result = []
    for path in sorted(candidates, key=str):
        _tick()
        need(not os.path.lexists(path), 'ancestor Cargo workspace is forbidden: ' + str(path))
        for parent in path.parents:
            if os.path.lexists(parent):
                need(not parent.is_symlink(), 'workspace ancestry is a link')
        result.append({'path': str(path), 'absent': True})
    return result


def _source_authority(captured):
    need(captured['current_source_manifest']['sha256'] == SOURCE_SHA256, 'wrong package source authority')
    repo = _canonical_path(captured['checkout']['path'])
    maps = captured['maps']
    need(maps and set(maps) <= set(ROLES), 'actual selected source roles required')
    closures = [{r['path']: r for r in rows if r['path'] in ('Cargo.toml', 'Cargo.lock', 'build.rs')
                 or r['path'].startswith('native/')} for _, rows in sorted(maps.items())]
    wanted = closures[0]
    need(all(value == wanted for value in closures), 'role build-script/native source authorities differ')
    need({'Cargo.toml', 'Cargo.lock', 'build.rs'} <= wanted.keys(), 'missing build source closure')
    observed = []
    for name, expected in sorted(wanted.items()):
        row = _file(repo / name)
        need({k: row[k] for k in ('bytes', 'sha256')} ==
             {k: expected[k] for k in ('bytes', 'sha256')}, 'fixed build-script/native source drift')
        observed.append(row)
    frozen = _body(repo / FROZEN_PATH)
    need(sha(frozen) == FROZEN_SHA256, 'original frozen parser machine authority changed')
    return repo, observed, decode(frozen)


def _dependency_closure(repo, cargo_home):
    """Cargo.lock authenticates crate archives; archives authenticate source bodies."""
    lock = tomllib.loads(_body(repo / 'Cargo.lock').decode())
    need(lock['version'] == 4, 'reviewed Cargo lock format')
    cache_root = Path(cargo_home) / 'registry'
    registry = 'index.crates.io-1949cf8c6b5b557f'
    result = []
    seen = set()
    for package in lock['package']:
        _tick()
        if 'source' not in package:
            need(package['name'] == 'oxid' and package['version'] == '0.9.0', 'unexpected local dependency')
            continue
        need(package['source'] == 'registry+https://github.com/rust-lang/crates.io-index',
             'unreviewed dependency source')
        name, version = package['name'], package['version']
        need(re.fullmatch('[a-zA-Z0-9_-]+', name) and re.fullmatch('[0-9]+(?:[.][0-9]+){2}', version),
             'unsafe locked dependency')
        base = name + '-' + version
        need(base not in seen, 'duplicate locked dependency')
        seen.add(base)
        archive = cache_root / 'cache' / registry / (base + '.crate')
        archive_row = _file(archive)
        need(archive_row['sha256'] == package['checksum'], 'locked crate archive checksum differs')
        source = cache_root / 'src' / registry / base
        expected = {}
        raw = _body(archive)
        with tarfile.open(fileobj=_ArchiveReader(raw), mode='r:gz') as tar:
            for member in tar:
                _tick()
                path = PurePosixPath(member.name)
                need(not path.is_absolute() and '..' not in path.parts and path.parts[0] == base,
                     'unsafe crate archive member')
                need(member.isfile() or member.isdir(), 'crate archive link/special member')
                need(0 <= member.size <= 64 * 1024 ** 2, 'crate member bound')
                if member.isdir():
                    need(member.size == 0, 'crate directory contains hidden body')
                    continue
                relative = PurePosixPath(*path.parts[1:]).as_posix()
                need(relative not in expected and relative not in ('.', ''), 'duplicate crate archive member')
                need(len(expected) < MAX_TREE_MEMBERS, 'crate archive member bound')
                digest = hashlib.sha256()
                size = 0
                stream = tar.extractfile(member)
                while True:
                    _tick()
                    chunk = stream.read(1024 ** 2)
                    if not chunk:
                        break
                    size += len(chunk)
                    need(size <= 64 * 1024 ** 2, 'crate decompression bound')
                    digest.update(chunk)
                need(size == member.size, 'crate member size differs')
                expected[relative] = {'bytes': size, 'sha256': digest.hexdigest()}
        actual = _tree(source)
        files = {r['path']: r['route']['body'] for r in actual['rows'] if r['kind'] == 'file'}
        extra = {'.cargo-ok'}
        if '.cargo-checksum.json' in files:
            extra.add('.cargo-checksum.json')
        need(set(files) == set(expected) | extra, 'closed crate source roster')
        need(_body(source / '.cargo-ok') == b'{"v":1}', 'exact Cargo extraction marker required')
        for relative, expected_row in expected.items():
            need({k: files[relative][k] for k in ('bytes', 'sha256')} == expected_row,
                 'crate source differs from locked archive')
        if '.cargo-checksum.json' in files:
            checksum = decode(_body(source / '.cargo-checksum.json'))
            need(checksum['package'] == package['checksum'], 'Cargo package checksum metadata changed')
            need(checksum['files'] == {n: r['sha256'] for n, r in expected.items()},
                 'Cargo file checksum metadata differs from archive')
        lower = name.lower()
        index_rel = ('1/' + lower if len(lower) == 1 else '2/' + lower if len(lower) == 2 else
                     '3/' + lower[0] + '/' + lower if len(lower) == 3 else lower[:2] + '/' + lower[2:4] + '/' + lower)
        index = cache_root / 'index' / registry
        result.append({'name': name, 'version': version, 'archive': archive_row, 'source': actual,
                       'index': _file(index / '.cache' / index_rel), 'index_config': _file(index / 'config.json')})
    need(result, 'empty locked dependency closure')
    return result


def _native_inputs(host, env):
    """Read qualified native setup before probes. Unsupported setup fails closed."""
    tools, trees, files = {}, [], []
    name = host['name']
    if name == 'Windows x86_64':
        need(all(env.get(k) for k in ('VCINSTALLDIR', 'VSINSTALLDIR', 'VCToolsInstallDir', 'LIB', 'INCLUDE')),
             'Microsoft developer environment incomplete')
        need(all(env.get(k) in ('x64', 'amd64') for k in ('VSCMD_ARG_TGT_ARCH', 'VSCMD_ARG_HOST_ARCH')),
             'Microsoft developer tools must run and target x64')
        directory = Path(env['VCToolsInstallDir']).resolve(strict=True) / 'bin/Hostx64/x64'
        for key, binary in (('cc', 'cl.exe'), ('cxx', 'cl.exe'), ('ar', 'lib.exe'), ('linker', 'link.exe')):
            route = _which(binary, env['PATH'])
            need(Path(route['resolved']) == (directory / binary).resolve(strict=True),
                 'Windows native tool outside selected x64 toolset')
            tools[key] = route
        paths = {directory}
        for key in ('LIB', 'INCLUDE', 'LIBPATH'):
            paths.update(Path(p) for p in env.get(key, '').split(';') if p)
        trees = [_tree(p) for p in sorted(paths, key=str)]
    elif name.startswith('macOS'):
        developer = Path(env.get('DEVELOPER_DIR', '/var/db/xcode_select_link'))
        need(developer.is_dir(), 'qualified Apple developer directory unavailable')
        developer = developer.resolve(strict=True)
        candidates = [developer / 'Toolchains/XcodeDefault.xctoolchain/usr', developer / 'usr']
        roots = [p for p in candidates if (p / 'bin/clang').is_file()]
        need(len(roots) == 1, 'unambiguous qualified Apple toolchain required')
        toolroot = roots[0]
        for key, binary in (('cc', 'clang'), ('cxx', 'clang++'), ('ar', 'ar'), ('linker', 'clang')):
            tools[key] = _route(toolroot / 'bin' / binary)
        sdk_candidates = ([Path(env['SDKROOT'])] if env.get('SDKROOT') else
                          [p for p in (developer / 'SDKs/MacOSX.sdk',
                                       developer / 'Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk') if p.is_dir()])
        need(len(sdk_candidates) == 1, 'unambiguous qualified macOS SDK required')
        env['DEVELOPER_DIR'], env['SDKROOT'] = str(developer), str(sdk_candidates[0].resolve(strict=True))
        trees = [_tree(toolroot), _tree(sdk_candidates[0])]
    else:
        for key, binary in (('cc', 'cc'), ('cxx', 'c++'), ('ar', 'ar'), ('linker', 'cc'),
                            ('as', 'as'), ('ld', 'ld'), ('which', 'which')):
            tools[key] = _which(binary, env['PATH'])
        need(re.search(r'(?:^|-)gcc(?:-[0-9.]+)?$', Path(tools['cc']['resolved']).name),
             'qualified Linux GCC setup required')
        need(re.search(r'(?:^|-)g\+\+(?:-[0-9.]+)?$', Path(tools['cxx']['resolved']).name),
             'qualified Linux G++ setup required')
        trees = [_tree(Path('/usr/lib/gcc')), _tree(Path('/usr/include'))]
        for base in (Path('/usr/lib/x86_64-linux-gnu'), Path('/lib/x86_64-linux-gnu')):
            for pattern in ('crt*.o', 'Scrt*.o', 'libc.so*', 'libm.so*', 'libstdc++.so*', 'libgcc_s.so*',
                            'libpthread.so*', 'libdl.so*', 'librt.so*', 'ld-linux*.so*'):
                files.extend(_route(p) for p in sorted(base.glob(pattern)))
        need(files, 'Linux native runtime search inputs unavailable')
        for path in (Path('/etc/ld.so.cache'), Path('/etc/ld.so.conf')):
            if path.is_file():
                files.append(_route(path))
        if Path('/etc/ld.so.conf.d').is_dir():
            trees.append(_tree(Path('/etc/ld.so.conf.d')))
    env.update(CC=tools['cc']['resolved'], CXX=tools['cxx']['resolved'], AR=tools['ar']['resolved'])
    env['CARGO_TARGET_' + host['target'].upper().replace('-', '_') + '_LINKER'] = tools['linker']['resolved']
    return {'tools': tools, 'trees': trees, 'files': files}


def _rust_inputs(root, host, frozen, parser_required=False):
    root = _canonical_path(root)
    suffix = '.exe' if host['name'] == 'Windows x86_64' else ''
    tools = {name: _route(root / 'bin' / (name + suffix)) for name in ('cargo', 'rustc')}
    library = root / 'lib'
    top = [_route(p) for p in sorted(library.iterdir()) if p.is_file()]
    target = library / 'rustlib' / host['target']
    trees = [_tree(target)]
    if host['name'] == 'Windows x86_64':
        top.extend(_route(p) for p in sorted((root / 'bin').glob('*.dll')))
    parser = None
    if parser_required:
        need(host['name'] == 'Linux x86_64', 'original parser machine authority requires Linux x86_64')
        # This immutable binary map is Linux-only. It is never generalized to
        # the other required public hosts and remains separately labelled.
        selected = {}
        for p in root.rglob('*'):
            _tick()
            relative = p.relative_to(root).as_posix()
            if (relative in ('bin/cargo', 'bin/rustc', 'lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld',
                             'lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld.lld')
                    or relative.startswith(('lib/libLLVM', 'lib/librustc_driver'))
                    or (relative.startswith('lib/rustlib/x86_64-unknown-linux-gnu/lib/')
                        and relative.endswith(('.rlib', '.rmeta', '.so')))):
                row = _file(p)
                selected[relative] = {'path': relative, 'bytes': row['bytes'], 'sha256': row['sha256']}
        need(sorted(selected.values(), key=lambda r: r['path']) == frozen['compiler_files'],
             'original Linux parser toolchain body authority differs')
        parser = {'scope': 'Linux x86_64 parser only', 'compiler_files': frozen['compiler_files'],
                  'recipe': frozen['recipe']}
    return {'root': str(root), 'tools': tools, 'shared_libraries': top, 'sysroot_trees': trees,
            'parser_original_authority': parser}


def _probe(api, invocation, argv, cwd, env, accepted=(0,)):
    before = time.time_ns()
    record = api.owned_probe(invocation, list(argv), str(cwd), dict(env),
                             timeout_seconds=PROBE_TIMEOUT, stream_limit_bytes=STREAM_LIMIT)
    result = api.verify_probe(record, invocation, argv=list(argv), cwd=str(cwd), environment=dict(env),
                              timeout_seconds=PROBE_TIMEOUT, stream_limit_bytes=STREAM_LIMIT)
    after = time.time_ns()
    need(result['argv'] == list(argv) and result['cwd'] == str(cwd) and result['environment'] == env,
         'supervised probe command/environment changed')
    need(type(result['pid']) is int and result['pid'] > 0, 'probe child PID missing')
    need(type(result['status']) is int and result['status'] in accepted, 'probe failed')
    need(result['timed_out'] is False and result['stream_limit_exceeded'] is False
         and result['cleanup_complete'] is True, 'incomplete owned probe')
    need(result['timeout_seconds'] == PROBE_TIMEOUT and result['stream_limit_bytes'] == STREAM_LIMIT,
         'probe limits changed')
    need(type(result['started_ns']) is int and type(result['completed_ns']) is int
         and before <= result['started_ns'] <= result['completed_ns'] <= after,
         'probe outside actual capture window')
    observed = dict(result)
    for stream in ('stdout', 'stderr'):
        raw = getattr(record, stream)()
        need(len(raw) <= STREAM_LIMIT and len(raw) == result[stream]['bytes']
             and sha(raw) == result[stream]['sha256'], 'probe stream differs')
        observed[stream] = raw.decode('utf-8', errors='strict')
    return record, observed


def _windows_help(record):
    need(record['status'] in (0, 1100) and record['stderr'] == '', 'selected Microsoft linker help failed')
    lines = record['stdout'].splitlines()
    need(len(lines) > 7 and re.fullmatch(r'Microsoft \(R\) Incremental Linker Version [0-9]+(?:\.[0-9]+){3}', lines[0]),
         'selected linker did not identify as Microsoft Incremental Linker')
    need(lines[1:7] == ['Copyright (C) Microsoft Corporation.  All rights reserved.', '',
                       ' usage: LINK [options] [files] [@commandfile]', '', '   options:', ''],
         'Microsoft linker exact help header missing')
    need(all(not line or re.match(r'^ {6}(?:/| {6})', line) for line in lines[7:])
         and any(line.startswith('      /OUT:') for line in lines[7:])
         and any(line.startswith('      /MACHINE:') for line in lines[7:])
         and re.search(r'(?i)\bfatal error\b|\berror(?:\s+LNK[0-9]+|\s*:)', record['stdout']) is None,
         'Microsoft linker options absent or diagnostics present')


def _snapshot(api, invocation, toolchain=None, resolver=None, direct_discovery=None):
    # Full machine admission cannot inherit a cheap outer read-scope association.
    inv, captured = api.full_verify_invocation(invocation)
    repo, source, frozen = _source_authority(captured)
    host = measured_host()
    inputs = _producer_environment()
    env = _clean_environment(inputs, host)
    config = _config_absence(repo, inv['run_root'], env)
    workspaces = _workspace_absence(inv['run_root'])
    native = _native_inputs(host, env)
    dependencies = _dependency_closure(repo, env['CARGO_HOME'])
    rust = None
    if toolchain is not None:
        rust = _rust_inputs(toolchain, host, frozen,
                            parser_required=any(role.startswith('parser_') for role in captured['maps']))
        env['PATH'] = str(Path(toolchain) / 'bin') + os.pathsep + env['PATH']
        env['RUSTC'] = rust['tools']['rustc']['resolved']
        for name in ('cargo', 'rustc'):
            need(_which(name, env['PATH'])['resolved'] == rust['tools'][name]['resolved'],
                 'direct Rust tool search path redirected')
    resolver_settings = []
    if resolver is not None:
        need(Path(env['RUSTUP_HOME']).is_dir(), 'installed resolver home is absent')
        need(_route(resolver['selected']) == resolver, 'Rust resolver route/body drift')
        for name in ('settings.toml', 'update-hashes/1.99.0-' + host['target']):
            path = Path(env['RUSTUP_HOME']) / name
            resolver_settings.append(_file(path) if path.exists() else {'path': str(path), 'absent': True})
    if direct_discovery is not None:
        need(set(direct_discovery) == {'cargo', 'rustc'}, 'closed direct tool discovery')
        for name, expected in direct_discovery.items():
            need(_which(name, inputs['PATH']) == expected, 'producer direct Rust discovery route drift')
    return {'invocation': inv, 'checkout': captured['checkout'], 'source': source,
            'source_scope': captured['scope'], 'source_roles': sorted(captured['maps']),
            'frozen_authority': _file(repo / FROZEN_PATH), 'host': host, 'producer_environment': inputs,
            'environment': env, 'configuration_absence': config, 'native': native,
            'ancestor_workspace_absence': workspaces,
            'dependencies': dependencies, 'rust': rust, 'resolver': resolver,
            'direct_discovery': direct_discovery,
            'resolver_settings': resolver_settings}, frozen


class ToolAdmission:
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('ToolAdmission is issued only by capture_machine')

    def document(self):
        return decode(_issued(self)['raw'])

    def invocation(self):
        return _issued(self)['invocation']

    def tool(self, name):
        state = self.document()['snapshot']
        tools = {**state['native']['tools'], **state['rust']['tools']}
        need(name in tools, 'unknown admitted tool')
        return tools[name]['resolved']

    def environment(self, role, profile, target):
        need(role in ROLES and profile in ('debug', 'release'), 'unknown machine build role/profile')
        document = self.document()
        state = document['snapshot']
        need(role in state['source_roles'], 'machine role is outside selected source scope')
        target = _canonical_path(target, exists=False)
        root = Path(state['invocation']['run_root'])
        need(root in target.parents, 'target outside owned invocation')
        repo = Path(state['checkout']['path'])
        preparation = root / 'source-preparation'
        need(repo not in target.parents and target != preparation and preparation not in target.parents
             and target not in preparation.parents, 'target overlaps source authority')
        env = dict(state['environment'], CARGO_TARGET_DIR=str(target))
        if role.startswith('parser_'):
            parser = state['rust']['parser_original_authority']
            need(parser is not None, 'parser original machine authority is Linux-only')
            env.update(parser['recipe']['build_environment'])
        return env


def _issued(admission):
    need(type(admission) is ToolAdmission and admission in _ISSUED, 'producer-issued machine admission required')
    return _ISSUED[admission]


def _runtime_projection(state):
    """Pure minimal metadata projection; never an admission or a file-hash cache."""
    python = state['host']['python']
    return canonical({'checkout': {'path': state['checkout']['path']},
                      'source_roles': list(state['source_roles']),
                      'environment': dict(state['environment']),
                      'host': {'name': state['host']['name'],
                               'python': {key: python[key] for key in ('selected', 'resolved')}}})


def _runtime_state(admission):
    """Read only issuer-owned immutable metadata, retaining its full-carrier tie."""
    owner = _issued(admission)
    need(all(key in owner for key in ('raw_sha256', 'runtime_projection', 'runtime_projection_sha256')),
         'original producer runtime projection required')
    raw = owner['runtime_projection']
    need(type(owner['raw']) is bytes and type(raw) is bytes and len(raw) <= 1024 * 1024 and
         sha(raw) == owner['runtime_projection_sha256'] and sha(owner['raw']) == owner['raw_sha256'],
         'original machine carrier/runtime projection changed')
    return decode(raw)


def _capture_machine(api, invocation):
    started = time.time_ns()
    before, frozen = _snapshot(api, invocation)
    env = before['environment']
    inputs = before['producer_environment']
    # Prefer actual direct tools from the producer's PATH. Rustup shims are
    # resolved only through the captured preinstalled resolver fallback.
    # Neither route accepts a caller-selected path or version claim.
    discovery = inputs.get('PATH', '')
    direct = None
    try:
        candidates = {name: _which(name, discovery) for name in ('cargo', 'rustc')}
        suffix = '.exe' if before['host']['name'].startswith('Windows') else ''
        if (all(Path(row['resolved']).name == name + suffix and Path(row['resolved']).parent.name == 'bin'
                for name, row in candidates.items())
                and Path(candidates['cargo']['resolved']).parent == Path(candidates['rustc']['resolved']).parent):
            direct = candidates
    except Rejected:
        # Missing direct tools are not authority. The separately authenticated
        # resolver must still be present, or this capture fails before probes.
        pass
    resolver = None if direct is not None else _which('rustup', discovery)
    before, frozen = _snapshot(api, invocation, resolver=resolver, direct_discovery=direct)
    probes = []
    live_probes = []
    resolved = {}
    cwd = before['checkout']['path']
    if direct is not None:
        resolved = {name: _route(Path(row['resolved'])) for name, row in direct.items()}
    else:
        for tool in ('cargo', 'rustc'):
            live, p = _probe(api, invocation, [resolver['resolved'], 'which', '--toolchain', '1.99.0', tool], cwd, env)
            need(p['stderr'] == '' and len(p['stdout'].splitlines()) == 1, 'unexpected Rust resolver output')
            selected = Path(p['stdout'].strip())
            selected = _canonical_path(selected)
            need(selected.name == tool + ('.exe' if before['host']['name'].startswith('Windows') else ''),
                 'resolver selected unexpected Rust executable')
            resolved[tool] = _route(selected)
            probes.append(p)
            live_probes.append(live)
    root = Path(resolved['rustc']['resolved']).parent.parent
    need(Path(resolved['cargo']['resolved']).parent.parent == root, 'Cargo and rustc toolchains differ')
    # Resolution itself may not alter any pre-probe authority.
    after_resolution, _ = _snapshot(api, invocation, resolver=resolver, direct_discovery=direct)
    need(after_resolution == before, 'machine inputs changed during Rust resolution')
    state, frozen = _snapshot(api, invocation, toolchain=root, resolver=resolver, direct_discovery=direct)
    need(state['rust']['tools'] == resolved, 'direct Rust inputs changed during capture')
    env = state['environment']
    live, rust_version = _probe(api, invocation, [resolved['rustc']['resolved'], '-Vv'], cwd, env)
    expected = frozen['recipe']['rustc_verbose'].replace('host: ' + frozen['recipe']['target'],
                                                        'host: ' + state['host']['target'])
    need(rust_version['stdout'] == expected and rust_version['stderr'] == '', 'exact Rust 1.99.0 actual-host identity differs')
    probes.append(rust_version)
    live_probes.append(live)
    live, cargo_version = _probe(api, invocation, [resolved['cargo']['resolved'], '-Vv'], cwd, env)
    need(cargo_version['stderr'] == '' and cargo_version['stdout'].startswith('cargo 1.99.0 ')
         and cargo_version['stdout'].splitlines().count('release: 1.99.0') == 1
         and cargo_version['stdout'].splitlines().count('host: ' + state['host']['target']) == 1,
         'Cargo 1.99.0 actual-host identity differs')
    probes.append(cargo_version)
    live_probes.append(live)
    live, sysroot = _probe(api, invocation, [resolved['rustc']['resolved'], '--print', 'sysroot'], cwd, env)
    need(sysroot['stderr'] == '' and sysroot['stdout'].strip() == str(root), 'Rust sysroot differs from captured direct tree')
    probes.append(sysroot)
    live_probes.append(live)
    if state['host']['name'] == 'Windows x86_64':
        linker = state['native']['tools']['linker']['resolved']
        live, help_record = _probe(api, invocation, [linker, '/?'], cwd, env, accepted=(0, 1100))
        _windows_help(help_record)
        probes.append(help_record)
        live_probes.append(live)
    else:
        for tool in ('cc', 'cxx', 'ar'):
            live, p = _probe(api, invocation, [state['native']['tools'][tool]['resolved'], '--version'], cwd, env)
            need(p['stdout'].strip() and p['stderr'] == '', 'native tool version probe missing')
            probes.append(p)
            live_probes.append(live)
    terminal, _ = _snapshot(api, invocation, toolchain=root, resolver=resolver, direct_discovery=direct)
    need(terminal == state, 'machine inputs changed during version/sysroot probes')
    document = {'schema': 'oxid-package-unit4-machine-capture-v1',
                'scope': 'recorded-local-host-environment', 'execution_qualified': False,
                'hermetic': False, 'network_isolated': False, 'reproducibility_claim': False,
                'runtime_native_admitted': False, 'required_public_hosts': list(HOSTS),
                'capture_timeout_seconds': CAPTURE_TIMEOUT, 'verify_timeout_seconds': VERIFY_TIMEOUT,
                'rustc_verbose': rust_version['stdout'],
                'started_ns': started, 'completed_ns': time.time_ns(), 'snapshot': state,
                'resolver_snapshot': before, 'probes': probes,
                'tool_discovery': 'producer-direct' if direct is not None else 'captured-rustup-resolver',
                'probe_records': [p.document() for p in live_probes],
                'limitations': ['Records selected local build inputs and relevant search paths.',
                                'Does not authenticate the operating system or close dynamic-loader trust.',
                                'Before/after rereads detect drift; they are not atomic filesystem isolation.',
                                'Linux frozen compiler bytes apply only to original parser authority.',
                                'No public runtime, parser execution, or four-host CI join is admitted.']}
    admission = object.__new__(ToolAdmission)
    raw, projection = canonical(document), _runtime_projection(state)
    _ISSUED[admission] = {'invocation': invocation, 'raw': raw, 'raw_sha256': sha(raw),
                        'runtime_projection': projection, 'runtime_projection_sha256': sha(projection),
                        'probes': live_probes}
    return admission


def capture_machine(invocation):
    """Capture actual host inputs and fixed owned probes; never runs at import."""
    api = _shared()
    with _deadline(api, invocation, CAPTURE_TIMEOUT):
        return _capture_machine(api, invocation)


def _verify_machine(api, admission, invocation):
    record = _issued(admission)
    need(record['invocation'] is invocation, 'machine admission belongs to another producer invocation')
    document = decode(record['raw'])
    state = document['snapshot']
    need(_runtime_state(admission) == decode(_runtime_projection(state)),
         'runtime projection differs from original full machine carrier')
    current, _ = _snapshot(api, invocation, toolchain=state['rust']['root'], resolver=state['resolver'],
                           direct_discovery=state['direct_discovery'])
    need(current == state, 'captured local machine inputs drifted')
    need(len(record['probes']) == len(document['probes']) == len(document['probe_records']),
         'closed machine probe roster differs')
    for live, expected, envelope in zip(record['probes'], document['probes'], document['probe_records']):
        observed = api.verify_probe(live, invocation, argv=expected['argv'], cwd=expected['cwd'],
                                    environment=expected['environment'], timeout_seconds=PROBE_TIMEOUT,
                                    stream_limit_bytes=STREAM_LIMIT)
        need(observed == envelope, 'machine probe envelope differs')
        observed = dict(observed)
        for name in ('stdout', 'stderr'):
            observed[name] = getattr(live, name)().decode('utf-8', errors='strict')
        need(observed == expected, 'retained machine probe evidence drifted')
    return True


def verify_machine(admission, invocation):
    issued = _issued(admission)
    need(issued['invocation'] is invocation, 'machine admission belongs to another producer invocation')
    api = _shared()
    with _deadline(api, invocation, VERIFY_TIMEOUT):
        return _verify_machine(api, admission, invocation)


class _Supervisor:
    __slots__ = ('_admission', '_invocation')

    def __init__(self, admission, invocation):
        self._admission, self._invocation = admission, invocation

    def run(self, argv, cwd, environment, timeout, stream_limit):
        verify_machine(self._admission, self._invocation)
        result = _shared().owned_probe(self._invocation, argv, str(cwd), environment,
                                       timeout_seconds=timeout, stream_limit_bytes=stream_limit)
        verify_machine(self._admission, self._invocation)
        return result


def supervisor(admission, invocation):
    verify_machine(admission, invocation)
    return _Supervisor(admission, invocation)


def load_machine(*args, **kwargs):
    raise NotReady('serialized machine reports cannot establish fresh producer authority')


# Runtime input capture is deliberately separate from build-machine admission.
# These fixed historical helpers are read, never imported or patched here.
PUBLIC_RUNTIME_PATH = 'tests/qualification/unit4_public_v3'
RUNTIME_HELPERS = {
    'authority.py': '346485ecb8afed13bd784190f57f8e86a4d6d065437c2220e99928ade425aac7',
    'contracts.py': '170515201950aa6018aa176e9d02720d7fdca5f9e52fae123a3a01c71d8a6ac9',
    'no_tool_trap.rs': '6493bd391eb3d8647dd3c33f27b75ff614024070942715bc20acc9732632bda2',
    'process_tree.py': '6b53900e18b33a26c5949bf8905f742e9abe5b67e74748585bbb24b6e4091454',
    'runtime.py': '866f839279f65ff4d8e57a49190202785aa2b4139097156a00aa1e952b3b9db8',
    'tool_wrapper.py': '60382c17f7c51217d2e53354a423666aa644ed678fcce21b7d1507abfd7f16ed',
}
LLVM_CONTENT_SHA256 = '31f2db375a0008638deb261dd8789c5d16a797a2ecd9e7074a7d0752922a5597'
LLVM_TOOLS = ('clang', 'opt', 'ld.lld')
LLVM_MARKERS = ('clang version 19.1.7', 'LLVM version 19.1.7', 'LLD 19.1.7')
RUNTIME_CAPTURE_TIMEOUT = 600
RUNTIME_VERIFY_TIMEOUT = 300
TRAP_BUILD_TIMEOUT = 120
TRAP_CONTROL_TIMEOUT = 10
_RUNTIME_TOOLS = weakref.WeakKeyDictionary()
_RUNTIME_FAILURES = weakref.WeakKeyDictionary()
_PARSER_RUNTIME_INPUTS = weakref.WeakKeyDictionary()


def _binding(path):
    return {k: v for k, v in _file(path).items() if k != 'stat'}


def _runtime_helpers(state):
    need({'ordinary', 'lifecycle'} <= set(state['source_roles']),
         'public runtime tools require both admitted public source roles')
    root = _canonical_path(Path(state['checkout']['path']) / PUBLIC_RUNTIME_PATH)
    rows = {}
    for name, expected in sorted(RUNTIME_HELPERS.items()):
        row = _file(root / name)
        need(row['sha256'] == expected, 'original public runtime helper changed: ' + name)
        rows[name] = row
    return {'root': str(root), 'files': rows, 'machine_consumer': _file(Path(__file__).absolute())}




class _RuntimeReadCheckpoint:
    """One synchronous read call, with no shared or cross-operation body cache.

    Only actual canonical body reads populate this memo. Every alias still gets
    its own route observations, and every reuse checks the full file identity.
    The object is permanently invalid after exit, including exceptional exit.
    """
    def __init__(self):
        self._bodies, self._routes = {}, {}
        self._active = False
        self._closed = False
        self._thread = None

    def __enter__(self):
        need(not self._active and not self._closed, 'runtime read checkpoint cannot be reused')
        self._thread, self._active = threading.get_ident(), True
        return self

    def _live(self):
        need(self._active and not self._closed and self._thread == threading.get_ident(),
             'active same-thread runtime read checkpoint required')
        _tick()

    @staticmethod
    def _copy(row):
        return dict(row, stat=dict(row['stat']))

    def _check_file(self, path, row):
        self._live()
        path = _canonical_path(path)
        before = path.stat()
        need(_stat(before) == row['stat'], 'runtime body stat changed within checkpoint')
        with path.open('rb') as stream:
            need(_stat(os.fstat(stream.fileno())) == row['stat'], 'runtime body descriptor changed within checkpoint')
        need(_canonical_path(path) == path and _stat(path.stat()) == row['stat'],
             'runtime body changed after checkpoint reuse')

    def file(self, path):
        self._live()
        path = _canonical_path(path)
        key = str(path)
        if key not in self._bodies:
            self._bodies[key] = _file(path)
        row = self._bodies[key]
        self._check_file(path, row)
        return self._copy(row)

    def binding(self, path):
        return {key: value for key, value in self.file(path).items() if key != 'stat'}

    def _route_state(self, path):
        self._live()
        path = Path(path)
        need(path.is_absolute() and '..' not in path.parts, 'absolute tool route required')
        links, stats = [], []
        for component in reversed((path, *path.parents)):
            self._live()
            info = component.lstat()
            row = {'path': str(component), 'device': info.st_dev, 'inode': info.st_ino, 'mode': info.st_mode}
            if stat.S_ISLNK(info.st_mode):
                row['link'] = os.readlink(component)
            else:
                need(stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode), 'special route component')
            links.append(row)
            stats.append({'path': str(component), 'stat': _stat(info)})
        resolved = path.resolve(strict=True)
        return {'selected': str(path), 'resolved': str(resolved), 'route': links}, stats

    def route(self, path):
        self._live()
        before, stats = self._route_state(path)
        body = self.file(before['resolved'])
        after, after_stats = self._route_state(path)
        need((after, after_stats) == (before, stats), 'runtime route changed during checkpoint read')
        key = before['selected']
        if key in self._routes:
            need(self._routes[key] == (before, stats), 'runtime route changed within checkpoint')
        self._routes[key] = (before, stats)
        return dict(before, body=body)

    def _verify(self):
        self._live()
        for path, row in self._bodies.items():
            self._check_file(path, row)
        for path, expected in self._routes.items():
            need(self._route_state(path) == expected, 'runtime route changed at checkpoint exit')

    def __exit__(self, error_type, error, traceback):
        try:
            self._verify()
        except BaseException as failure:
            if error is None:
                raise
            if hasattr(error, 'add_note'):
                error.add_note('Runtime read checkpoint exit failed: ' + repr(failure)[:1000])
        finally:
            self._bodies.clear()
            self._routes.clear()
            self._active, self._closed, self._thread = False, True, None
        return False


def _selected_runtime(receipt_path, library_root):
    # This checkpoint never yields to caller code and never invokes a process.
    with _RuntimeReadCheckpoint() as checkpoint:
        return _selected_runtime_body(receipt_path, library_root, checkpoint)


def _selected_runtime_body(receipt_path, library_root, checkpoint):
    """Verify old selected content, without upgrading a receipt into provenance."""
    receipt_path, root = _canonical_path(receipt_path), _canonical_path(library_root)
    need(root.is_dir() and os.pathsep not in str(root),
         'selected runtime must be one explicit library search directory')
    checkpoint.file(receipt_path)
    receipt = decode(_body(receipt_path))
    need(receipt['schema'] in ('oxid-restored-llvm-runtime-v1', 'oxid-unit3-portable-v1-llvm-runtime-stage'),
         'unsupported staged-runtime receipt')
    need(sha(canonical(receipt['content'])) == receipt['content_sha256'] == LLVM_CONTENT_SHA256,
         'original selected LLVM runtime content differs')
    need(type(receipt['library_directory']) is str and
         _canonical_path(receipt['library_directory']) == root,
         'selected runtime receipt directory differs')
    names, selected, routes = [], [], []
    for row in receipt['content']['entries']:
        name = row['path']
        need(type(name) is str and PurePosixPath(name).name == name and name not in ('.', '..')
             and name not in names, 'closed flat selected runtime roster required')
        names.append(name)
        path = root / name
        if row['kind'] == 'symlink':
            need(path.is_symlink() and os.readlink(path) == row['link'], 'selected runtime symlink changed')
            need(path.resolve(strict=True).parent == root, 'selected runtime link escapes its directory')
            selected.append({'path': str(path), 'kind': 'symlink', 'target': row['link']})
        else:
            need(row['kind'] == 'file' and row['soname'] == name and not path.is_symlink(),
                 'selected runtime SONAME/file differs')
            actual = checkpoint.binding(path)
            need(all(actual[k] == row[k] for k in ('bytes', 'sha256')), 'selected runtime bytes changed')
            selected.append(dict(actual, kind='file', soname=row['soname']))
        routes.append(checkpoint.route(path))
    need(sorted(p.name for p in root.iterdir()) == sorted(names), 'selected runtime has extra or missing overrides')
    need(type(receipt['dependency_scope']) is str and len(receipt['dependency_scope']) <= 2048,
         'bounded selected runtime dependency description required')
    # The retained description is historical metadata. The enclosing RuntimeTools
    # scope below is authoritative about what this live producer actually proved.
    carrier = {'receipt': checkpoint.binding(receipt_path), 'library_directory': str(root),
               'content_sha256': LLVM_CONTENT_SHA256, 'selected_libraries': selected,
               'dependency_scope': receipt['dependency_scope']}
    return {'carrier': carrier, 'receipt_file': checkpoint.file(receipt_path), 'library_routes': routes,
            'receipt_producer_authenticated': False, 'hosted_staging_admitted': False}


def _runtime_inputs(admission, purpose, llvm_root, library_root, runtime_receipt):
    state = _runtime_state(admission)
    helpers = _runtime_helpers(state)
    env = dict(state['environment'])
    need(not any(k in env for k in ('LD_PRELOAD', 'DYLD_INSERT_LIBRARIES', 'PYTHONPATH',
                                    'UNIT4_TOOL_LOG', 'UNIT4_TOOL_EVIDENCE', 'UNIT4_RACE_OUTPUT',
                                    'UNIT4_LIFECYCLE_LOG')), 'unexpected inherited runtime override')
    env['UNIT4_ADAPTER_ROOT'] = helpers['root']
    native = purpose == 'main' and state['host']['name'] == 'Linux x86_64'
    if native:
        need(all(value is not None for value in (llvm_root, library_root, runtime_receipt)),
             'Linux public runtime requires explicit LLVM, selected libraries and receipt paths')
        root = _canonical_path(llvm_root)
        need(root.is_dir(), 'LLVM executable directory absent')
        tools = {name: _route(root / name) for name in LLVM_TOOLS}
        runtime = _selected_runtime(runtime_receipt, library_root)
        # The unmodified wrappers use /usr/bin/env python3. Resolve that exact
        # interpreter, rather than silently inheriting an unrelated Python.
        interpreter = state['host']['python']
        env['PATH'] = str(Path(interpreter['selected']).parent) + os.pathsep + env['PATH']
        python = _which('python3', env['PATH'])
        need(python['resolved'] == interpreter['resolved'], 'wrapper Python differs from captured interpreter')
        env.update(UNIT4_REAL_LLVM=str(root), LD_LIBRARY_PATH=str(_canonical_path(library_root)))
        native_inputs = {'root': str(root), 'tools': tools, 'selected_runtime': runtime,
                         'wrapper_python': python, 'wrapper_env': _route(Path('/usr/bin/env'))}
    else:
        need(all(value is None for value in (llvm_root, library_root, runtime_receipt)),
             'no-native traps cannot accept LLVM or library inputs')
        native_inputs = None
    return {'helpers': helpers, 'native': native_inputs, 'environment': env,
            'mode': 'real-native' if native else 'no-native-traps'}


def _runtime_probe(api, invocation, argv, cwd, env, timeout, kind, accepted=(0,), retained=None):
    started = time.time_ns()
    live = api.owned_probe(invocation, list(argv), str(cwd), dict(env), timeout_seconds=timeout,
                           stream_limit_bytes=STREAM_LIMIT, operation_kind=kind)
    if retained is not None:
        retained.append(live)
    envelope = api.verify_probe(live, invocation, argv=list(argv), cwd=str(cwd), environment=dict(env),
                                 timeout_seconds=timeout, stream_limit_bytes=STREAM_LIMIT, operation_kind=kind)
    completed = time.time_ns()
    need(envelope['argv'] == list(argv) and envelope['cwd'] == str(cwd) and envelope['environment'] == env,
         'runtime setup command/environment changed')
    need(type(envelope['pid']) is int and envelope['pid'] > 0 and
         type(envelope['status']) is int and envelope['status'] in accepted, 'runtime setup operation failed')
    need(envelope['timed_out'] is False and envelope['stream_limit_exceeded'] is False and
         envelope['cleanup_complete'] is True and envelope.get('failure') is None,
         'runtime setup owned process incomplete')
    need(envelope['timeout_seconds'] == timeout and envelope['stream_limit_bytes'] == STREAM_LIMIT and
         envelope['operation_kind'] == kind, 'runtime setup exact limits/policy changed')
    need(type(envelope['started_ns']) is int and type(envelope['completed_ns']) is int and
         started <= envelope['started_ns'] <= envelope['completed_ns'] <= completed and
         envelope['completed_ns'] - envelope['started_ns'] <= timeout * 10**9,
         'runtime setup process outside capture window')
    observed = dict(envelope)
    for stream in ('stdout', 'stderr'):
        raw = getattr(live, stream)()
        need(len(raw) <= STREAM_LIMIT and len(raw) == envelope[stream]['bytes'] and
             sha(raw) == envelope[stream]['sha256'], 'runtime setup stream body/hash changed')
        observed[stream], observed[stream + '_sha256'] = raw.decode('utf-8', errors='strict'), sha(raw)
    return live, observed


def _copy_runtime_file(source, destination):
    """Create an immutable-by-readmission copy; never follow/replace a target."""
    source = _canonical_path(source)
    destination = _canonical_path(destination, exists=False)
    need(not os.path.lexists(destination), 'fresh runtime file required')
    raw = _body(source)
    with destination.open('xb') as stream:
        for offset in range(0, len(raw), 1024 ** 2):
            _tick()
            stream.write(raw[offset:offset + 1024 ** 2])
    destination.chmod(0o755)
    need(_binding(destination)['sha256'] == sha(raw), 'runtime copy changed')
    return _binding(destination)


def _trap_artifact(path, host, process):
    row, raw = _file(path), _body(path)
    need(process['started_ns'] <= row['stat']['st_mtime_ns'] <= process['completed_ns'],
         'no-native trap binary predates actual build')
    need(row['stat']['st_mode'] & 0o111, 'trap output is not executable')
    if host == 'Linux x86_64':
        need(len(raw) >= 64 and raw[:7] == b'\x7fELF\x02\x01\x01' and
             int.from_bytes(raw[16:18], 'little') in (2, 3) and int.from_bytes(raw[18:20], 'little') == 62 and
             int.from_bytes(raw[20:24], 'little') == 1 and int.from_bytes(raw[52:54], 'little') == 64,
             'trap output must be ELF64 x86_64')
    elif host == 'Windows x86_64':
        need(len(raw) >= 64 and raw[:2] == b'MZ', 'trap output must be PE')
        offset = int.from_bytes(raw[60:64], 'little')
        need(offset + 26 <= len(raw) and raw[offset:offset + 4] == b'PE\0\0' and
             raw[offset + 4:offset + 6] == b'\x64\x86' and raw[offset + 24:offset + 26] == b'\x0b\x02',
             'trap output must be PE32+ x86_64')
    else:
        cpu = 0x01000007 if host == 'macOS x86_64' else 0x0100000c
        need(host in HOSTS and len(raw) >= 32 and raw[:4] == b'\xcf\xfa\xed\xfe' and
             int.from_bytes(raw[4:8], 'little') == cpu and int.from_bytes(raw[12:16], 'little') == 2,
             'trap output must be actual-host executable Mach-O64')
    return row


class RuntimeTools:
    """Live local setup input capture. Never hosted or qualification authority."""
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('RuntimeTools is issued only by live runtime-tool capture')

    def document(self):
        return decode(_runtime_issued(self)['raw'])

    def invocation(self):
        return _runtime_issued(self)['invocation']

    def machine(self):
        return _runtime_issued(self)['machine']

    def environment(self):
        return self.document()['environment']

    def tools(self):
        return self.document()['tools']

    def mode(self):
        return self.document()['mode']

    def purpose(self):
        return self.document()['purpose']

    def selected_runtime(self):
        return self.document()['selected_runtime']

    def setup_probes(self):
        """Return original opaque records only after selected-input readmission."""
        verify_runtime_inputs(self, self.machine(), self.invocation())
        return tuple(_runtime_issued(self)['probes'])


def _runtime_issued(value):
    need(type(value) is RuntimeTools and value in _RUNTIME_TOOLS, 'live producer RuntimeTools required')
    return _RUNTIME_TOOLS[value]


def _runtime_output(root, mode, host):
    """Compare generated output to a recipe-derived roster, never itself."""
    if mode == 'real-native':
        directories = {'.', 'tool-wrappers'}
        files = {'tool-wrappers/' + name for name in LLVM_TOOLS}
    else:
        need(mode == 'no-native-traps' and host in HOSTS, 'fixed runtime output mode/host')
        directories = {'.', 'no-tool-traps'}
        files = {'no-tool-trap.exe' if host == 'Windows x86_64' else 'no-tool-trap',
                 'no-tool-trap-build.json'}
        files |= {'no-tool-traps/' + name for name in LLVM_TOOLS}
        files |= {name + '-trap-control.jsonl' for name in LLVM_TOOLS}
        if host == 'Windows x86_64':
            files |= {'no-tool-traps/' + name + '.exe' for name in LLVM_TOOLS}
    snapshot = _tree(root)
    need({row['path'] for row in snapshot['rows'] if row['kind'] == 'directory'} == directories and
         {row['path'] for row in snapshot['rows'] if row['kind'] == 'file'} == files,
         'runtime setup output has extra or missing files/directories')
    for row in snapshot['rows']:
        path = Path(root) / row['path']
        need(not path.is_symlink() and path.resolve(strict=True) == path,
             'runtime generated output cannot contain links')
    return snapshot


def _retain_runtime_failure(root, admission, invocation, purpose, started, probes, preservation, error):
    """Bounded terminal diagnostic, without swallowing the original exception."""
    summaries = []
    for probe in probes[:4]:
        try:
            row = probe.document()
            summaries.append({**{key: row.get(key) for key in
                                 ('argv', 'cwd', 'pid', 'status', 'operation_kind', 'started_ns', 'completed_ns',
                                  'cleanup_complete', 'timed_out', 'stream_limit_exceeded', 'stdout', 'stderr')},
                              'owned_record_sha256': sha(canonical(row))})
        except BaseException as failure:
            summaries.append({'status': 'unverified', 'error': repr(failure)[:1000]})
    document = {'schema': 'oxid-package-unit4-runtime-setup-failure-v1', 'status': 'NotReady',
                'execution_qualified': False, 'runtime_native_admitted': False,
                'purpose': purpose, 'output_root': str(root), 'started_ns': started,
                'completed_ns': time.time_ns(), 'original_error': repr(error)[:2000],
                'preservation': preservation, 'completed_owned_probe_records': summaries,
                'limitations': ['Failed operations may terminate before a live ProbeRecord is issued.',
                                'Partial setup output is not an admitted runtime tool capability.']}
    record = {'raw': canonical(document), 'invocation': invocation, 'probes': tuple(probes),
              'receipt_persisted': False}
    _RUNTIME_FAILURES.setdefault(admission, []).append(record)
    # A failed invocation deadline cannot prevent a small, exclusive terminal
    # write. No unbounded read, rehash, process or retry occurs in this path.
    raw = record['raw'] + b'\n'
    need(len(raw) <= 128 * 1024, 'bounded runtime failure receipt')
    need(root.is_dir() and not any(p.is_symlink() for p in (root, *root.parents)),
         'safe runtime failure receipt directory required')
    path = root / 'runtime-setup-failure.json'
    with path.open('xb') as stream:
        stream.write(raw)
    record['receipt_persisted'] = True
    record['receipt'] = {'path': str(path), 'bytes': len(raw), 'sha256': sha(raw)}


def _capture_runtime_tools(admission, purpose, llvm_root=None, library_root=None, runtime_receipt=None):
    owner = _issued(admission)
    need(purpose in ('main', 'capability'), 'fixed runtime-tool purpose required')
    invocation, api = owner['invocation'], _shared()
    with _deadline(api, invocation, RUNTIME_CAPTURE_TIMEOUT):
        _verify_machine(api, admission, invocation)
        started = time.time_ns()
        inputs = _runtime_inputs(admission, purpose, llvm_root, library_root, runtime_receipt)
        state = admission.document()['snapshot']
        root = _canonical_path(Path(state['invocation']['run_root']) / 'public-runtime/tools' / purpose, exists=False)
        need(not os.path.lexists(root), 'fresh dedicated runtime tool root required')
        root.mkdir(parents=True, exist_ok=False)
        env, tools, probes, observations = dict(inputs['environment']), {}, [], []
        trap_build = None

        def preservation():
            _verify_machine(api, admission, invocation)
            need(_runtime_inputs(admission, purpose, llvm_root, library_root, runtime_receipt) == inputs,
                 'runtime tool source/environment/input drift')

        def probe(argv, environment, timeout, kind, accepted=(0,)):
            preservation()
            live, observed = _runtime_probe(api, invocation, argv, root, environment, timeout, kind, accepted, probes)
            observations.append(observed)
            preservation()
            return observed

        try:
            if inputs['mode'] == 'real-native':
                wrappers = root / 'tool-wrappers'
                wrappers.mkdir()
                source = Path(inputs['helpers']['root']) / 'tool_wrapper.py'
                for name, marker in zip(LLVM_TOOLS, LLVM_MARKERS):
                    wrapper = _copy_runtime_file(source, wrappers / name)
                    version = probe([str(Path(inputs['native']['root']) / name), '--version'], env,
                                    PROBE_TIMEOUT, 'runtime-tool-version')
                    need(re.search(re.escape(marker) + r'(?=$|[ \(\r\n])', version['stdout']) is not None and
                         version['stderr'] == '', 'exact LLVM 19.1.7 actual tool identity differs')
                    tools[name] = {'executable': _binding(inputs['native']['tools'][name]['resolved']),
                                   'wrapper': wrapper, 'version': version}
                env['OXID_LLVM_BIN'] = str(wrappers)
            else:
                source = Path(inputs['helpers']['root']) / 'no_tool_trap.rs'
                binary = root / ('no-tool-trap.exe' if state['host']['name'] == 'Windows x86_64' else 'no-tool-trap')
                build = probe([admission.tool('rustc'), '--edition=2021', '-C', 'opt-level=0',
                               str(source), '-o', str(binary)], env, TRAP_BUILD_TIMEOUT, 'runtime-trap-build')
                binary_row = _trap_artifact(binary, state['host']['name'], build)
                wrappers = root / 'no-tool-traps'
                wrappers.mkdir()
                env['OXID_LLVM_BIN'] = str(wrappers)
                for name in LLVM_TOOLS:
                    path = wrappers / name
                    executable = _copy_runtime_file(binary, path)
                    alias = (_copy_runtime_file(binary, wrappers / (name + '.exe'))
                             if state['host']['name'] == 'Windows x86_64' else None)
                    log = root / (name + '-trap-control.jsonl')
                    need(not os.path.lexists(log), 'fresh trap control log required')
                    control = probe([str(path), '--version'], dict(env, UNIT4_TOOL_LOG=str(log)),
                                    TRAP_CONTROL_TIMEOUT, 'runtime-trap-control', accepted=(121,))
                    need(control['stdout'] == '' and control['stderr'] == 'unexpected native tool invocation\n',
                         'original no-tool trap control envelope differs')
                    need(_body(log, 1024) == b'{"unexpected_native_tool":true}\n',
                         'original no-tool trap control did not record exactly one event')
                    tools[name] = {'mode': 'no-native-trap', 'executable': executable, 'wrapper': dict(executable),
                                   'control': control, 'control_log': _binding(log)}
                    if alias is not None:
                        tools[name]['windows_alias'] = alias
                trap_build = {'source': _binding(source), 'binary': binary_row,
                              'rustc_verbose': admission.document()['rustc_verbose'],
                              'machine_sha256': sha(owner['raw']), 'build': build,
                              'purpose': 'Original no-native traps; no LLVM identity or native qualification.'}
                with (root / 'no-tool-trap-build.json').open('xb') as stream:
                    stream.write(canonical(trap_build) + b'\n')
            preservation()
            output = _runtime_output(root, inputs['mode'], state['host']['name'])
            document = {'schema': 'oxid-package-unit4-runtime-tools-v1', 'status': 'captured-local-inputs',
                        'scope': 'recorded-local-public-runtime-tools-only', 'execution_qualified': False,
                        'runtime_native_admitted': False, 'qualification_admitted': False,
                        'receipt_producer_authenticated': False, 'hosted_staging_admitted': False,
                        'hermetic': False, 'network_isolated': False,
                        'invocation': state['invocation'], 'machine_sha256': sha(owner['raw']),
                        'host': state['host']['name'], 'purpose': purpose, 'mode': inputs['mode'],
                        'output_root': str(root), 'environment': env, 'tools': tools, 'input_snapshot': inputs,
                        'selected_runtime': inputs['native']['selected_runtime']['carrier'] if inputs['native'] else None,
                        'trap_build': trap_build, 'output_snapshot': output,
                        'started_ns': started, 'completed_ns': time.time_ns(),
                        'probe_records': [live.document() for live in probes], 'probes': observations,
                        'limitations': ['Live setup captures local input/tool provenance, not public runtime qualification.',
                                        'Selected library content cannot authenticate the old staging receipt producer.',
                                        'Hosted staging and the four-host exact-head CI join require separate live producers.',
                                        'Ordinary host loader/system dependencies are recorded only by the machine snapshot.',
                                        'This is not hermetic or atomic filesystem isolation.']}
            result = object.__new__(RuntimeTools)
            _RUNTIME_TOOLS[result] = {'raw': canonical(document), 'machine': admission, 'invocation': invocation,
                                      'probes': probes, 'paths': (llvm_root, library_root, runtime_receipt)}
            return result
        except BaseException as error:
            preservation_result = {'status': 'unverified'}
            try:
                with _deadline(api, invocation, 30):
                    preservation()
                preservation_result = {'status': 'preserved'}
            except BaseException as failure:
                preservation_result['error'] = repr(failure)[:1000]
            try:
                _retain_runtime_failure(root, admission, invocation, purpose, started, probes,
                                        preservation_result, error)
            except BaseException as failure:
                if hasattr(error, 'add_note'):
                    error.add_note('Runtime setup failure receipt could not be retained: ' + repr(failure)[:1000])
            raise



def capture_runtime_tools(admission, *, llvm_root=None, library_root=None, runtime_receipt=None):
    """Capture the actual-host main tools; Linux cannot select the trap branch."""
    return _capture_runtime_tools(admission, 'main', llvm_root, library_root, runtime_receipt)


def capture_capability_traps(admission):
    """Capture original traps solely for the separate capability controller."""
    return _capture_runtime_tools(admission, 'capability')


def _verify_runtime_tools(value, admission, invocation, full):
    record = _runtime_issued(value)
    need(record['machine'] is admission and record['invocation'] is invocation and
         _issued(admission)['invocation'] is invocation, 'runtime tool live machine/invocation mismatch')
    api = _shared()
    with _deadline(api, invocation, RUNTIME_VERIFY_TIMEOUT):
        if full:
            _verify_machine(api, admission, invocation)
        with api.checked_scope(invocation):
            document = decode(record['raw'])
            need(document['machine_sha256'] == sha(_issued(admission)['raw']), 'runtime machine carrier differs')
            current = _runtime_inputs(admission, document['purpose'], *record['paths'])
            need(current == document['input_snapshot'], 'runtime input/helper/environment drift')
            need(_runtime_output(Path(document['output_root']), document['mode'], document['host']) ==
                 document['output_snapshot'], 'runtime tool output drift')
            need(len(record['probes']) == len(document['probe_records']) == len(document['probes']),
                 'runtime setup live probe roster differs')
            for live, envelope, expected in zip(record['probes'], document['probe_records'], document['probes']):
                observed = api.verify_probe(live, invocation, argv=expected['argv'], cwd=expected['cwd'],
                                            environment=expected['environment'], timeout_seconds=expected['timeout_seconds'],
                                            stream_limit_bytes=STREAM_LIMIT, operation_kind=expected['operation_kind'])
                need(observed == envelope, 'runtime setup retained probe envelope differs')
                observed = dict(observed)
                for stream in ('stdout', 'stderr'):
                    raw = getattr(live, stream)()
                    need(len(raw) == envelope[stream]['bytes'] and sha(raw) == envelope[stream]['sha256'],
                         'runtime retained stream body/hash differs')
                    observed[stream], observed[stream + '_sha256'] = raw.decode('utf-8', errors='strict'), sha(raw)
                need(observed == expected, 'runtime setup retained process differs')
            need(_shared() is api, 'canonical shared owner changed during runtime checkpoint')
        return True


def verify_runtime_tools(value, admission, invocation):
    """Full machine plus selected runtime re-admission at section boundaries."""
    return _verify_runtime_tools(value, admission, invocation, True)


def verify_runtime_inputs(value, admission, invocation):
    """Per-operation selected-input reread, not fresh build-machine admission.

    Consumers must retain full verification at start/end and section boundaries.
    This rereads actual helpers, wrappers, selected LLVM/library routes and every
    retained owned probe; it does not repeat the Cargo/Rust/sysroot tree scan.
    """
    return _verify_runtime_tools(value, admission, invocation, False)


class ParserRuntimeInputs:
    """Machine-only permit; never authority for a parser binary, result or case."""
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('ParserRuntimeInputs is issued only by capture_parser_runtime')

    def document(self):
        return decode(_parser_runtime_issued(self)['raw'])

    def machine(self):
        return _parser_runtime_issued(self)['machine']

    def invocation(self):
        return _parser_runtime_issued(self)['invocation']

    def environment(self):
        return self.document()['environment']


def _parser_runtime_issued(value):
    need(type(value) is ParserRuntimeInputs and value in _PARSER_RUNTIME_INPUTS,
         'live producer parser runtime inputs required')
    return _PARSER_RUNTIME_INPUTS[value]


def _parser_runtime_projection(document):
    """Derive existing parser runtime inputs, never caller-selected authority.

    Expected captured hashes are retained only for comparison. Every selected
    body is read again within each verification checkpoint. Native development
    trees and Cargo dependency reconstruction remain full-boundary obligations.
    """
    state = document['snapshot']
    rust = state['rust']
    authority = rust['parser_original_authority']
    need(state['host']['name'] == 'Linux x86_64' and authority is not None and
         {'parser_observed', 'parser_control'} <= set(state['source_roles']),
         'original Linux parser runtime projection required')
    routes = [*rust['tools'].values(), *rust['shared_libraries']]
    routes.extend(row['route'] for tree in rust['sysroot_trees'] for row in tree['rows']
                  if row['kind'] == 'file')
    by_path = {row['selected']: row for row in routes}
    compiler_routes = []
    for row in authority['compiler_files']:
        path = str(Path(rust['root']) / row['path'])
        need(path in by_path and all(by_path[path]['body'][key] == row[key]
                                    for key in ('bytes', 'sha256')),
             'captured original compiler route/body missing')
        compiler_routes.append(by_path[path])
    native = state['native']
    return canonical({
        **{key: state[key] for key in ('invocation', 'checkout', 'source_scope', 'source_roles',
                                      'host', 'producer_environment', 'environment',
                                      'configuration_absence', 'ancestor_workspace_absence',
                                      'resolver', 'resolver_settings', 'direct_discovery')},
        'rust': {'root': rust['root'], 'tools': rust['tools'],
                 'shared_libraries': rust['shared_libraries'],
                 'compiler_files': authority['compiler_files'], 'compiler_routes': compiler_routes},
        'native': {'tools': native['tools'], 'files': native['files'],
                   'trees': [tree for tree in native['trees'] if tree['root'] == '/etc/ld.so.conf.d']},
        'probes': document['probes'], 'probe_records': document['probe_records']})


def _parser_selected_state(record, admission):
    owner = _issued(admission)
    _runtime_state(admission)  # Also checks the original immutable full carrier.
    need(all(key in record for key in ('raw_sha256', 'selected_raw', 'selected_sha256')),
         'original producer parser runtime projection required')
    raw = record['selected_raw']
    need(type(record['raw']) is bytes and type(raw) is bytes and len(raw) <= 4 * 1024 ** 2 and
         sha(record['raw']) == record['raw_sha256'] and sha(raw) == record['selected_sha256'],
         'parser runtime carrier/projection changed')
    document = decode(record['raw'])
    need(document['machine_sha256'] == sha(owner['raw']), 'parser runtime machine carrier differs')
    return document, decode(raw)


def _parser_native_runtime_paths():
    files = []
    for base in (Path('/usr/lib/x86_64-linux-gnu'), Path('/lib/x86_64-linux-gnu')):
        for pattern in ('crt*.o', 'Scrt*.o', 'libc.so*', 'libm.so*', 'libstdc++.so*', 'libgcc_s.so*',
                        'libpthread.so*', 'libdl.so*', 'librt.so*', 'ld-linux*.so*'):
            files.extend(str(path) for path in sorted(base.glob(pattern)))
    files.extend(str(path) for path in (Path('/etc/ld.so.cache'), Path('/etc/ld.so.conf')) if path.is_file())
    return files


def _parser_loader_trees():
    return [_tree(Path('/etc/ld.so.conf.d'))] if Path('/etc/ld.so.conf.d').is_dir() else []


def _parser_selected_read(state, checkpoint):
    """Reread only previously captured paths; no source capture or operation."""
    need(measured_host() == state['host'], 'parser selected actual host/interpreter drift')
    need(_producer_environment() == state['producer_environment'], 'parser selected producer environment drift')
    env = _clean_environment(state['producer_environment'], state['host'])
    need(_config_absence(state['checkout']['path'], state['invocation']['run_root'], env) ==
         state['configuration_absence'] and
         _workspace_absence(state['invocation']['run_root']) == state['ancestor_workspace_absence'],
         'parser selected ambient configuration drift')

    def route(expected):
        need(checkpoint.route(expected['selected']) == expected, 'parser selected executable/library route or body drift')

    def which(name, path, expected):
        need(all(part and Path(part).is_absolute() for part in path.split(os.pathsep)),
             'unsafe parser selected executable search path')
        selected = shutil.which(name, path=path)
        need(selected is not None and str(Path(selected).absolute()) == expected['selected'],
             'parser selected executable search redirected')
        route(expected)

    route(state['host']['python'])
    for key, name in (('cc', 'cc'), ('cxx', 'c++'), ('ar', 'ar'), ('linker', 'cc'),
                      ('as', 'as'), ('ld', 'ld'), ('which', 'which')):
        which(name, env['PATH'], state['native']['tools'][key])
    env.update(CC=state['native']['tools']['cc']['resolved'], CXX=state['native']['tools']['cxx']['resolved'],
               AR=state['native']['tools']['ar']['resolved'])
    env['CARGO_TARGET_' + state['host']['target'].upper().replace('-', '_') + '_LINKER'] = \
        state['native']['tools']['linker']['resolved']
    rust = state['rust']
    root = _canonical_path(rust['root'])
    env['PATH'] = str(root / 'bin') + os.pathsep + env['PATH']
    env['RUSTC'] = rust['tools']['rustc']['resolved']
    need(env == state['environment'], 'parser selected clean environment drift')
    for name in ('cargo', 'rustc'):
        which(name, env['PATH'], rust['tools'][name])
    shared = [str(path) for path in sorted((root / 'lib').iterdir()) if path.is_file()]
    need(shared == [row['selected'] for row in rust['shared_libraries']], 'parser selected Rust library roster drift')
    for row in rust['shared_libraries']:
        route(row)
    # Match the original Linux-only frozen compiler selector, including the
    # complete original 62-file map, without reconstructing its build trees.
    names = []
    for path in root.rglob('*'):
        _tick()
        relative = path.relative_to(root).as_posix()
        if (relative in ('bin/cargo', 'bin/rustc', 'lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld',
                         'lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld.lld')
                or relative.startswith(('lib/libLLVM', 'lib/librustc_driver'))
                or (relative.startswith('lib/rustlib/x86_64-unknown-linux-gnu/lib/')
                    and relative.endswith(('.rlib', '.rmeta', '.so')))):
            names.append(relative)
    need(sorted(names) == [row['path'] for row in rust['compiler_files']], 'parser original compiler roster drift')
    for row, expected in zip(rust['compiler_files'], rust['compiler_routes']):
        route(expected)
        body = checkpoint.file(root / row['path'])
        need(all(body[key] == row[key] for key in ('bytes', 'sha256')),
             'parser original compiler authority drift')
    need(_parser_native_runtime_paths() == [row['selected'] for row in state['native']['files']],
         'parser native runtime input roster drift')
    for row in state['native']['files']:
        route(row)
    trees = _parser_loader_trees()
    need(trees == state['native']['trees'], 'parser loader configuration tree drift')
    for tree in trees:
        for row in tree['rows']:
            if row['kind'] == 'file':
                route(row['route'])
    if state['resolver'] is not None:
        route(state['resolver'])
    for row in state['resolver_settings']:
        if row.get('absent'):
            need(not os.path.lexists(row['path']), 'parser resolver configuration appeared')
        else:
            need(checkpoint.file(row['path']) == row, 'parser resolver configuration drift')
    if state['direct_discovery'] is not None:
        for name, expected in state['direct_discovery'].items():
            which(name, state['producer_environment']['PATH'], expected)


def capture_parser_runtime(admission, invocation):
    """Re-admit existing Linux machine inputs; launches no additional operation.

    The parser consumer must independently bind and verify all four original live
    BuildResults and its session. This method accepts no binaries or callbacks.
    """
    owner = _issued(admission)
    need(owner['invocation'] is invocation, 'parser runtime machine/invocation mismatch')
    api = _shared()
    with _deadline(api, invocation, RUNTIME_VERIFY_TIMEOUT):
        _verify_machine(api, admission, invocation)
        machine_document = admission.document()
        state = machine_document['snapshot']
        need(state['host']['name'] == 'Linux x86_64' and
             {'parser_observed', 'parser_control'} <= set(state['source_roles']) and
             state['rust']['parser_original_authority'] is not None,
             'original parser runtime machine authority requires both Linux parser roles')
        document = {'schema': 'oxid-package-unit4-parser-runtime-inputs-v1',
                    'scope': 'recorded-local-parser-runtime-environment-only',
                    'execution_qualified': False, 'binary_or_case_admission': False,
                    'runtime_native_admitted': False, 'native_llvm_qualified': False,
                    'hosted_staging_admitted': False, 'hermetic': False,
                    'invocation': state['invocation'], 'machine_sha256': sha(owner['raw']),
                    'machine_consumer': _file(Path(__file__).absolute()),
                    'environment': dict(state['environment']),
                    'loader_scope': 'Previously captured local Linux loader, system libraries, cache/config and Rust inputs.',
                    'limitations': ['No new ELF dependency-resolution or hermetic loader claim is established.',
                                    'Only the parser consumer can bind the four live BuildResults and existing cases.']}
        result = object.__new__(ParserRuntimeInputs)
        raw, selected = canonical(document), _parser_runtime_projection(machine_document)
        _PARSER_RUNTIME_INPUTS[result] = {'raw': raw, 'raw_sha256': sha(raw), 'selected_raw': selected,
                                         'selected_sha256': sha(selected), 'machine': admission, 'invocation': invocation}
        return result


def verify_parser_runtime(value, admission, invocation):
    record = _parser_runtime_issued(value)
    need(record['machine'] is admission and record['invocation'] is invocation and
         _issued(admission)['invocation'] is invocation, 'parser runtime live ownership differs')
    api = _shared()
    with _deadline(api, invocation, RUNTIME_VERIFY_TIMEOUT):
        _verify_machine(api, admission, invocation)
        document, selected = _parser_selected_state(record, admission)
        need(selected == decode(_parser_runtime_projection(admission.document())),
             'parser runtime projection differs from full machine carrier')
        need(document['machine_sha256'] == sha(_issued(admission)['raw']) and
             document['machine_consumer'] == _file(Path(__file__).absolute()), 'parser runtime input/consumer drift')
        return True


def verify_parser_runtime_selected(value, admission, invocation):
    """Per-operation parser inputs only; full phase/terminal checks are mandatory.

    Rehashes the original compiler map and selected executable/runtime-library,
    host, environment and loader configuration inputs. It does not recapture
    sources, reconstruct Cargo dependencies or scan native development trees.
    The parser consumer still owns all four live BuildResults and case checks.
    """
    record = _parser_runtime_issued(value)
    need(record['machine'] is admission and record['invocation'] is invocation and
         _issued(admission)['invocation'] is invocation, 'parser runtime live ownership differs')
    api = _shared()
    with _deadline(api, invocation, RUNTIME_VERIFY_TIMEOUT), api.checked_scope(invocation):
        document, state = _parser_selected_state(record, admission)
        inv, captured = api.verify_invocation(invocation)
        need(inv == state['invocation'] and captured['checkout'] == state['checkout'] and
             captured['scope'] == state['source_scope'] and sorted(captured['maps']) == state['source_roles'],
             'parser selected original context association differs')
        with _RuntimeReadCheckpoint() as checkpoint:
            need(document['machine_consumer'] == checkpoint.file(Path(__file__).absolute()),
                 'parser runtime consumer body drift')
            _parser_selected_read(state, checkpoint)
            probes = _issued(admission)['probes']
            need(len(probes) == len(state['probes']) == len(state['probe_records']),
                 'closed parser machine probe roster differs')
            for live, expected, envelope in zip(probes, state['probes'], state['probe_records']):
                observed = api.verify_probe(live, invocation, argv=expected['argv'], cwd=expected['cwd'],
                                            environment=expected['environment'], timeout_seconds=PROBE_TIMEOUT,
                                            stream_limit_bytes=STREAM_LIMIT)
                need(observed == envelope, 'parser machine probe envelope drift')
                observed = dict(observed)
                for name in ('stdout', 'stderr'):
                    raw = getattr(live, name)()
                    need(len(raw) == envelope[name]['bytes'] and sha(raw) == envelope[name]['sha256'],
                         'parser retained machine probe body drift')
                    observed[name] = raw.decode('utf-8', errors='strict')
                need(observed == expected, 'parser retained machine probe drift')
            # Repeat roster/configuration observations while the same private
            # checkpoint revalidates each route and deduplicates only this call's
            # actual body reads. Nothing is retained for the next operation.
            _parser_selected_read(state, checkpoint)
            need(_shared() is api, 'canonical shared owner changed during parser checkpoint')
        return True
