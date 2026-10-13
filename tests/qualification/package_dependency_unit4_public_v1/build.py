"""Live producer-owned public builds; importing this module starts no child.

Builds require real prepared source, producer-captured machine admission and
owned supervised probes. Serialized envelopes alone cannot establish origin.
All runtime and four-host qualification admission remains NotReady. The pure
inspectors are supporting checks, never alternative admission entrypoints.
"""
import hashlib
import os
import stat
import time
import weakref
import platform
import sys
import types
from pathlib import Path

sys.dont_write_bytecode = True
# Resolve our sibling physically, never through sys.path or a caller module.
_binding_path = Path(__file__).absolute().with_name('binding.py')
_BINDING_SHA256 = '1cd256d81ec99b297f5b774c387151fc348985c4eecc988f9894511086733292'
_binding_body = _binding_path.read_bytes()
if hashlib.sha256(_binding_body).hexdigest() != _BINDING_SHA256:
    raise ValueError('public source binding helper changed')
binding = types.ModuleType('package_unit4_public_source_binding')
binding.__file__ = str(_binding_path)
exec(compile(_binding_body, str(_binding_path), 'exec'), binding.__dict__)
Rejected, NotReady, need = binding.Rejected, binding.NotReady, binding.need
PROFILES = ('debug', 'release')
HOSTS = {
    'Linux x86_64': 'x86_64-unknown-linux-gnu',
    'macOS x86_64': 'x86_64-apple-darwin',
    'macOS arm64': 'aarch64-apple-darwin',
    'Windows x86_64': 'x86_64-pc-windows-msvc',
}
MAX_STREAM_BYTES = 8 * 1024 * 1024
BUILD_TIMEOUT_SECONDS = 1800
ROLE_STAGE_TIMEOUT_SECONDS = 3600
# This describes obligations, not an authority or caller-selectable interface.
MACHINE_REQUIREMENTS = (
    'producer-authenticated real-host tool and dependency authority',
    'actual measured host and Rust 1.99.0 verbose identity',
    'authenticated cargo, rustc, linker, sysroot and process-tree helper bodies',
    'closed Cargo dependency cache and configuration inventories before and after',
    'clean environment with explicit CARGO_HOME and RUSTUP_HOME',
    'Windows x64 linker identity and help controls on Windows',
    'fresh target/output roots and shared invocation nonce captured before children',
    'supervised full stdout/stderr, command, cwd, limits, process window and cleanup',
    'exact Cargo package_id, manifest_path, bin target src_path and profile',
    'fresh emitted executable and copied executable identity/stat/host ABI',
    'source/context/tool/config/dependency preservation before and after each profile',
    'two distinct debug/release binaries and exact paired role linkage',
)


def recipe(profile):
    need(profile in PROFILES, 'unsupported build profile')
    return ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline'] + (
        ['--release'] if profile == 'release' else [])


def metadata_recipe():
    return ['cargo', 'metadata', '--format-version', '1', '--no-deps', '--locked', '--offline']


def measured_host():
    aliases = {'Darwin': 'macOS', 'AMD64': 'x86_64', 'aarch64': 'arm64'}
    return ' '.join(aliases.get(x, x) for x in (platform.system(), platform.machine()))


def _canonical_future(path):
    path = Path(path)
    need(path.is_absolute() and path.resolve() == path, 'canonical absolute output path required')
    need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink output ancestry forbidden')
    need(not path.exists(), 'fresh target/output root required')
    return path


def _overlaps(a, b):
    return a == b or a in b.parents or b in a.parents



def public_paths(run_root, role):
    need(role in binding.ROLE_MAPS, 'unsupported public build role')
    root = Path(run_root)
    need(root.is_absolute(), 'absolute producer root required')
    return root / 'targets' / ('public-' + role), root / 'public-build' / role


def build_plan(context, role, target, output):
    """Plan only: admits real source, creates no directories and starts no child."""
    source = binding.source_binding(context, role)
    document = context.document()
    target, output = _canonical_future(target), _canonical_future(output)
    need(not _overlaps(target, output), 'target and output must be distinct disjoint roots')
    run_root = Path(source['invocation']['run_root'])
    need((target, output) == public_paths(run_root, role), 'exact dedicated public target/output roots required')
    roots = [Path(v['path']) for v in document['views'].values()]
    roots.append(Path(document['checkout']['path']))
    for path in (target, output):
        need(all(not _overlaps(path, root) for root in roots),
             'build artifacts must stay outside checkout and every sealed view')
    host = measured_host()
    need(host in HOSTS, 'unsupported actual build host')
    return {
        'schema': 'package-dependency-unit4-public-build-plan-v1',
        'status': 'NotReady', 'execution_qualified': False,
        'compiler_invocations': 0, 'native_executions': 0,
        'source_binding': source, 'host': host, 'rust_host': HOSTS[host],
        'target': str(target), 'output': str(output),
        'metadata_argv': metadata_recipe(),
        'profiles': {p: {'argv': recipe(p), 'timeout_seconds': BUILD_TIMEOUT_SECONDS,
                         'stream_limit_bytes': MAX_STREAM_BYTES,
                         'emitted_binary': str(target / p / ('oxid.exe' if host.startswith('Windows') else 'oxid')),
                         'copied_binary': str(output / ('oxid-' + p + ('.exe' if host.startswith('Windows') else '')))}
                     for p in PROFILES},
        'environment_requirements': {'CARGO_INCREMENTAL': '0', 'CARGO_BUILD_JOBS': '2',
                                     'CARGO_TARGET_DIR': str(target)},
        'role_stage_timeout_seconds': ROLE_STAGE_TIMEOUT_SECONDS,
        'required_machine_evidence': list(MACHINE_REQUIREMENTS),
    }


def inspect_rust_identity(verbose, host):
    """Check a claim's shape, not its provenance or executable authority."""
    need(host in HOSTS and isinstance(verbose, str), 'actual required host')
    lines = verbose.splitlines()
    need(lines and lines[0].startswith('rustc 1.99.0 '), 'Rust 1.99.0 required')
    need([x for x in lines if x.startswith('host: ')] == ['host: ' + HOSTS[host]], 'Rust host mismatch')
    need([x for x in lines if x.startswith('release: ')] == ['release: 1.99.0'], 'Rust release mismatch')
    return True


def inspect_cargo_metadata(metadata, source_root, target_root, source_files):
    """Check separately captured no-deps metadata without altering build flags."""
    source_root, target_root = Path(source_root), Path(target_root)
    need(source_root.is_absolute() and target_root.is_absolute(), 'absolute Cargo roots')
    need(metadata['version'] == 1 and metadata['resolve'] is None, 'exact no-deps Cargo metadata')
    packages = metadata['packages']
    need(isinstance(packages, list) and len(packages) == 1, 'exact single compiler package')
    package = packages[0]
    package_id = 'path+' + source_root.as_uri() + '#oxid@0.9.0'
    need(package['id'] == package_id, 'actual Cargo package_id')
    need(package['name'] == 'oxid' and package['version'] == '0.9.0' and package['edition'] == '2021',
         'exact compiler package')
    need(package['manifest_path'] == str(source_root / 'Cargo.toml'), 'actual Cargo manifest_path')
    targets = package['targets']
    expected_bin = {'kind': ['bin'], 'crate_types': ['bin'], 'name': 'oxid',
                    'src_path': str(source_root / 'src/cli.rs'), 'edition': '2021',
                    'doc': True, 'doctest': False, 'test': True}
    expected_build = {'kind': ['custom-build'], 'crate_types': ['bin'], 'name': 'build-script-build',
                      'src_path': str(source_root / 'build.rs'), 'edition': '2021',
                      'doc': False, 'doctest': False, 'test': False}
    need(binding.sha(binding.canonical(source_files)) in {v[0] for v in binding.ROLE_MAPS.values()},
         'metadata requires exact admitted public role map')
    expected_targets = [expected_bin, expected_build]
    for row in source_files:
        path = Path(row['path'])
        if len(path.parts) == 2 and path.parts[0] == 'tests' and path.suffix == '.rs':
            expected_targets.append({'kind': ['test'], 'crate_types': ['bin'], 'name': path.stem,
                                     'src_path': str(source_root / path), 'edition': '2021',
                                     'doc': False, 'doctest': False, 'test': True})
    need(sorted(targets, key=lambda x: x['name']) == sorted(expected_targets, key=lambda x: x['name']),
         'exact Cargo bin target src_path and complete discovered target roster')
    need(metadata['workspace_members'] == [package_id] and metadata['workspace_default_members'] == [package_id],
         'exact compiler workspace membership')
    need(metadata['workspace_root'] == str(source_root), 'actual Cargo workspace root')
    need(metadata['target_directory'] == str(target_root), 'external explicit Cargo target root')
    return True


def inspect_process(record, argv, cwd, environment, invocation_start, window_end, timeout_seconds):
    """Pure envelope check; only the later trusted supervisor can admit it."""
    need(type(invocation_start) is int and type(window_end) is int and 0 < invocation_start <= window_end,
         'valid producer window')
    need(record['argv'] == argv and record['cwd'] == str(cwd), 'exact process command/cwd')
    need(record['environment'] == environment, 'exact cleaned process environment')
    need(type(record['pid']) is int and record['pid'] > 0, 'real child pid required')
    need(type(record['status']) is int and record['status'] == 0, 'successful process status')
    need(record['timed_out'] is False and record['stream_limit_exceeded'] is False,
         'incomplete process evidence')
    need(type(record['stream_limit_bytes']) is int and record['stream_limit_bytes'] == MAX_STREAM_BYTES,
         'exact stream bound')
    need(type(record['timeout_seconds']) is int and record['timeout_seconds'] == timeout_seconds,
         'exact process timeout')
    start, end = record['started_ns'], record['completed_ns']
    need(type(start) is int and type(end) is int and invocation_start <= start <= end <= window_end,
         'process outside fresh invocation window')
    need(end - start <= timeout_seconds * 10**9, 'successful process exceeded timeout')
    need(record['cleanup_complete'] is True, 'complete child/process-tree cleanup evidence required')
    for stream in ('stdout', 'stderr'):
        value = record[stream]
        need(isinstance(value, str), 'complete UTF-8 stream required')
        raw = value.encode()
        need(len(raw) <= MAX_STREAM_BYTES and record[stream + '_sha256'] == hashlib.sha256(raw).hexdigest(),
             'stream body/hash/limit mismatch')
    return True


def inspect_artifact(record, expected_path, started_ns, completed_ns, host, invocation=None):
    """Read actual emitted bytes/stat; never treats this alone as build proof."""
    path = Path(expected_path)
    identity, raw = _read_file(path, invocation, header_only=True)
    need(record == identity, 'emitted artifact identity/stat mismatch')
    actual = path.stat()
    need(type(started_ns) is int and type(completed_ns) is int and
         0 < started_ns <= actual.st_mtime_ns <= completed_ns, 'stale emitted binary')
    need(host in HOSTS, 'unsupported artifact host')
    if host == 'Linux x86_64':
        need(len(raw) >= 64 and raw[:7] == b'\x7fELF\x02\x01\x01' and
             int.from_bytes(raw[16:18], 'little') in (2, 3) and int.from_bytes(raw[18:20], 'little') == 62 and
             int.from_bytes(raw[20:24], 'little') == 1 and int.from_bytes(raw[52:54], 'little') == 64,
             'expected executable ELF64 x86_64 header')
    elif host.startswith('Windows'):
        need(len(raw) >= 64 and raw[:2] == b'MZ', 'expected PE executable')
        offset = int.from_bytes(raw[60:64], 'little')
        need(offset + 26 <= len(raw) and raw[offset:offset + 4] == b'PE\0\0' and
             raw[offset + 4:offset + 6] == b'\x64\x86' and raw[offset + 24:offset + 26] == b'\x0b\x02',
             'expected PE32+ x86_64 binary')
    else:
        cpu = 0x01000007 if host == 'macOS x86_64' else 0x0100000c
        need(len(raw) >= 32 and raw[:4] == b'\xcf\xfa\xed\xfe' and
             int.from_bytes(raw[4:8], 'little') == cpu and int.from_bytes(raw[12:16], 'little') == 2,
             'expected native executable Mach-O64 binary')
    return True



_RESULTS = weakref.WeakKeyDictionary()
_BUILD_BODY_SHA256 = hashlib.sha256(Path(__file__).absolute().read_bytes()).hexdigest()
_ACTIVE_DEADLINES = weakref.WeakKeyDictionary()


class BuildResult:
    """Opaque live result; callers cannot manufacture it from result metadata."""
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('BuildResult is issued only by build')

    def document(self):
        return binding.decode(_result(self)['raw'])


def _result(result):
    need(type(result) is BuildResult and result in _RESULTS, 'live public build producer result required')
    return _RESULTS[result]


def _tick(invocation):
    if invocation is not None:
        binding.preparation_api().check_deadline(invocation)
        deadline = _ACTIVE_DEADLINES.get(invocation)
        need(deadline is None or time.monotonic() <= deadline, 'public role-stage deadline')


def _read_file(path, invocation=None, *, header_only=False, maximum=1024 ** 3):
    """Bounded streaming identity, checked ancestry and stable before/after stat."""
    _tick(invocation)
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=True) == path and
         not any(p.is_symlink() for p in (path, *path.parents)), 'canonical regular evidence path')
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= maximum, 'regular evidence byte bound')
    keys = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
    digest, captured, total = hashlib.sha256(), bytearray(), 0
    with path.open('rb') as stream:
        while True:
            _tick(invocation)
            raw = stream.read(1024 * 1024)
            if not raw:
                break
            total += len(raw)
            need(total <= maximum, 'evidence grew beyond byte bound')
            digest.update(raw)
            if not header_only or not captured:
                captured.extend(raw)
    _tick(invocation)
    after = path.stat()
    need(all(getattr(before, k) == getattr(after, k) for k in keys) and total == before.st_size,
         'evidence changed while being read')
    return {'path': str(path), 'bytes': total, 'sha256': digest.hexdigest(),
            'stat': {k: getattr(after, k) for k in keys}}, bytes(captured)


def _identity(path, with_stat=False, invocation=None):
    value, _ = _read_file(path, invocation, header_only=True)
    if not with_stat:
        del value['stat']
    return value


def _json_file(path, invocation):
    return binding.decode(_read_file(path, invocation, maximum=128 * 1024 ** 2)[1])


def _write(path, value, invocation=None, maximum=128 * 1024 ** 2):
    # Never replace an existing carrier, including a symlink.
    _tick(invocation)
    raw = binding.canonical(value) + b'\n'
    need(len(raw) <= maximum, 'written carrier byte bound')
    _tick(invocation)
    with Path(path).open('xb') as stream:
        for offset in range(0, len(raw), 1024 * 1024):
            _tick(invocation)
            stream.write(raw[offset:offset + 1024 * 1024])
    return _identity(path, invocation=invocation)



def _closed_names(root, expected, invocation):
    observed = []
    for path in Path(root).iterdir():
        _tick(invocation)
        observed.append(path.name)
        need(len(observed) <= len(expected), 'extra evidence member')
    need(set(observed) == set(expected), 'closed evidence membership changed')


def _consumer_identity(invocation):
    root = Path(__file__).absolute().parent
    manifest_path = root / 'package-manifest.json'
    manifest = _json_file(manifest_path, invocation)
    need(manifest['schema'] == 'oxid-package-dependency-unit4-public-package-v1', 'public package schema')
    names = ['binding.py', 'build.py', 'hosted.py', 'runtime.py', 'test_binding.py', 'test_build.py', 'test_hosted.py', 'test_runtime.py']
    need([r['path'] for r in manifest['files']] == names, 'exact public consumer roster')
    _closed_names(root, names + ['package-manifest.json'], invocation)
    for row in manifest['files']:
        raw = _read_file(root / row['path'], invocation, maximum=4 * 1024 ** 2)[1]
        need(row == {'path': row['path'], 'bytes': len(raw), 'sha256': binding.sha(raw)}, 'public consumer body drift')
        need(not (root / row['path']).stat().st_mode & 0o111, 'public helper executable mode changed')
    observed = {r['path']: r['sha256'] for r in manifest['files']}
    need(observed['binding.py'] == _BINDING_SHA256 and observed['build.py'] == _BUILD_BODY_SHA256,
         'loaded public helper body drift')
    return _identity(manifest_path, invocation=invocation)

def _preservation(context, role, machine):
    _tick(context.invocation())
    source = binding.source_binding(context, role)
    binding.machine_api().verify_machine(machine, context.invocation())
    need(machine.invocation() is context.invocation(), 'source/machine producer identity mismatch')
    document = machine.document()
    need(document['snapshot']['host']['name'] == measured_host(), 'actual host changed')
    inspect_rust_identity(document['rustc_verbose'], measured_host())
    return {'source': source, 'machine_sha256': binding.sha(binding.canonical(document)),
            'consumer': _consumer_identity(context.invocation())}


def _probe_check(record, context, argv, source_root, environment, timeout, operation_kind='probe'):
    api = binding.preparation_api()
    document = api.verify_probe(record, context.invocation(), argv=argv, cwd=source_root,
                                environment=environment, timeout_seconds=timeout,
                                stream_limit_bytes=MAX_STREAM_BYTES, operation_kind=operation_kind)
    observed = dict(document)
    for name in ('stdout', 'stderr'):
        raw = getattr(record, name)()
        need(len(raw) == document[name]['bytes'] and binding.sha(raw) == document[name]['sha256'],
             'owned stream body differs')
        observed[name] = raw.decode('utf-8', errors='strict')
        observed[name + '_sha256'] = document[name]['sha256']
    inspect_process(observed, argv, source_root, environment,
                    context.invocation().document()['started_ns'], time.time_ns(), timeout)
    return document


def _copy_binary(source, destination, invocation):
    before = _identity(source, with_stat=True, invocation=invocation)
    need(os.name == 'nt' or before['stat']['st_mode'] & 0o111, 'emitted compiler is not executable')
    total = 0
    with Path(source).open('rb') as incoming, Path(destination).open('xb') as outgoing:
        while True:
            _tick(invocation)
            raw = incoming.read(1024 * 1024)
            if not raw:
                break
            total += len(raw)
            need(total <= before['bytes'], 'emitted executable grew during copying')
            outgoing.write(raw)
    _tick(invocation)
    Path(destination).chmod(stat.S_IMODE(before['stat']['st_mode']))
    need(_identity(source, with_stat=True, invocation=invocation) == before, 'emitted compiler changed during copying')
    copied = _identity(destination, with_stat=True, invocation=invocation)
    need((copied['bytes'], copied['sha256']) == (before['bytes'], before['sha256']), 'copied compiler bytes differ')
    return before, copied


def build(context, role=None, target=None, output=None, machine=None, **unsupported):
    """Build both public profiles using fixed admitted inputs and owned children.

    This callable is implemented but no execution is performed by installation
    or tests. Calling it requires a separately authorized execution release.
    """
    need(not unsupported, 'unsupported build override')
    # Reject caller dictionaries before loading or probing any tool.
    binding.context_api().verify_context(context)
    binding.machine_api().verify_machine(machine, context.invocation())
    plan = build_plan(context, role, target, output)
    target, output = Path(plan['target']), Path(plan['output'])
    source_root = Path(plan['source_binding']['view']['path'])
    before = _preservation(context, role, machine)
    need(before['source'] == plan['source_binding'], 'plan source changed')
    started_ns, started_clock = time.time_ns(), time.monotonic()
    invocation = context.invocation()
    # Only these dedicated parents are created, after source/machine admission.
    for parent in (target.parent, output.parent):
        _tick(invocation)
        parent.mkdir(exist_ok=True)
        need(parent.is_dir() and parent.resolve(strict=True) == parent and not parent.is_symlink(),
             'dedicated build parent changed')
    target.mkdir()
    output.mkdir()
    attempt = {'schema': 'package-dependency-unit4-public-build-attempt-v1',
               'invocation': invocation.document(), 'role': role, 'started_ns': started_ns,
               'plan': plan, 'operations': [], 'profiles': {}, 'status': 'building',
               'execution_qualified': False}
    probes = {}
    _ACTIVE_DEADLINES[invocation] = started_clock + ROLE_STAGE_TIMEOUT_SECONDS
    try:
        plan_row = _write(output / 'build-plan.json', plan, invocation)
        source_row = _write(output / 'source-binding.json', before['source'], invocation)
        machine_row = _write(output / 'machine-capture.json', machine.document(), invocation)
        env = machine.environment(role, 'debug', target)
        for key, expected in plan['environment_requirements'].items():
            need(env.get(key) == expected, 'retained public environment changed')
        need(_preservation(context, role, machine) == before, 'pre-metadata source/machine/consumer drift')
        probe = binding.preparation_api().owned_probe(invocation, metadata_recipe(), source_root, env,
                                                      timeout_seconds=30, stream_limit_bytes=MAX_STREAM_BYTES)
        probes['metadata'] = probe
        attempt['operations'].append(probe.document())
        metadata_record = _probe_check(probe, context, metadata_recipe(), source_root, env, 30)
        inspect_cargo_metadata(binding.decode(probe.stdout()), source_root, target, before['source']['view']['files'])
        need(_preservation(context, role, machine) == before, 'metadata changed source/machine inputs')
        for profile in PROFILES:
            need(time.monotonic() - started_clock + BUILD_TIMEOUT_SECONDS <= ROLE_STAGE_TIMEOUT_SECONDS,
                 'role-stage deadline leaves no complete profile window')
            snapshot = _preservation(context, role, machine)
            need(snapshot == before, 'pre-profile source/machine input drift')
            env = machine.environment(role, profile, target)
            for key, expected in plan['environment_requirements'].items():
                need(env.get(key) == expected, 'retained public environment changed')
            emitted = Path(plan['profiles'][profile]['emitted_binary'])
            need(not emitted.exists() and not emitted.is_symlink(), 'profile executable must not preexist its build')
            probe = binding.preparation_api().owned_probe(invocation, recipe(profile), source_root, env,
                                                          timeout_seconds=BUILD_TIMEOUT_SECONDS,
                                                          stream_limit_bytes=MAX_STREAM_BYTES, operation_kind='public-build')
            probes[profile] = probe
            attempt['operations'].append(probe.document())
            process = _probe_check(probe, context, recipe(profile), source_root, env, BUILD_TIMEOUT_SECONDS, 'public-build')
            after = _preservation(context, role, machine)
            need(after == snapshot, 'profile changed source/machine inputs')
            emitted_row = _identity(emitted, with_stat=True, invocation=invocation)
            inspect_artifact(emitted_row, emitted, process['started_ns'], process['completed_ns'], plan['host'], invocation)
            original, copied = _copy_binary(emitted, plan['profiles'][profile]['copied_binary'], invocation)
            need(original == emitted_row, 'compiler changed after artifact inspection')
            attempt['profiles'][profile] = {'profile': profile, 'process': process,
                                           'source_before': snapshot['source']['context_sha256'],
                                           'source_after': after['source']['context_sha256'],
                                           'machine_before': snapshot['machine_sha256'],
                                           'machine_after': after['machine_sha256'],
                                           'emitted_binary': emitted_row, 'binary': copied,
                                           'cargo_profile': 'dev' if profile == 'debug' else 'release'}
        need(time.monotonic() - started_clock <= ROLE_STAGE_TIMEOUT_SECONDS, 'public role-stage timeout')
        need(attempt['profiles']['debug']['binary']['sha256'] != attempt['profiles']['release']['binary']['sha256'],
             'debug/release compiler identities must be distinct')
        need(_preservation(context, role, machine) == before, 'post-build source/machine drift')
        document = {'schema': 'package-dependency-unit4-public-built-binding-v1',
                    'kind': binding.KINDS[role], 'role': role,
                    'status': 'built-runtime-NotReady', 'execution_qualified': False,
                    'invocation': invocation.document(), 'source_binding': before['source'],
                    'plan': plan_row, 'source_carrier': source_row, 'machine_carrier': machine_row,
                    'machine_sha256': before['machine_sha256'], 'consumer': before['consumer'], 'host': plan['host'],
                    'started_ns': started_ns, 'completed_ns': time.time_ns(),
                    'role_stage_timeout_seconds': ROLE_STAGE_TIMEOUT_SECONDS,
                    'metadata_process': metadata_record, 'profiles': attempt['profiles']}
        path = output / 'candidate-binding.json'
        receipt = _write(path, document, invocation)
        result = object.__new__(BuildResult)
        _RESULTS[result] = {'raw': binding.canonical(document), 'context': context, 'machine': machine,
                            'probes': probes, 'receipt': receipt, 'plan': plan}
        read_build_binding(result)
        return result
    except BaseException as error:
        # Original owned operations remain intact even if rechecks or collectors
        # fail. Only one bounded best-effort supplementary report is attempted.
        attempt['status'] = 'failed'
        attempt['completed_ns'] = time.time_ns()
        attempt['error_type'] = type(error).__name__
        attempt['error'] = str(error)[:8192]
        try:
            _tick(invocation)
            attempt['failure_preservation'] = _preservation(context, role, machine)
            attempt['preservation_status'] = 'checked'
        except BaseException as preservation_error:
            attempt['preservation_status'] = 'unavailable'
            attempt['preservation_error'] = (type(preservation_error).__name__ + ': ' + str(preservation_error))[:8192]
        try:
            _write(output / 'failed-attempt.json', attempt, maximum=1024 ** 2)
        except BaseException:
            pass
        raise
    finally:
        _ACTIVE_DEADLINES.pop(invocation, None)


def read_build_binding(result, path=None):
    """Recheck serialized bytes only against the original live producer result."""
    owned = _result(result)
    document = result.document()
    context, machine, plan = owned['context'], owned['machine'], owned['plan']
    invocation = context.invocation()
    _tick(invocation)
    receipt = owned['receipt']
    need(path is None or str(Path(path)) == receipt['path'], 'build receipt relocation is not an admission')
    need(_identity(receipt['path'], invocation=invocation) == receipt and _json_file(receipt['path'], invocation) == document,
         'build receipt bytes changed')
    current = _preservation(context, document['role'], machine)
    need(current['source'] == document['source_binding'] and current['machine_sha256'] == document['machine_sha256'] and
         current['consumer'] == document['consumer'],
         'build source/machine context changed')
    need(document['invocation'] == context.invocation().document(), 'mixed build invocation')
    for row, expected in ((document['plan'], plan), (document['source_carrier'], current['source']),
                          (document['machine_carrier'], machine.document())):
        need(_identity(row['path'], invocation=invocation) == row and _json_file(row['path'], invocation) == expected,
             'build input carrier drift')
    output, target = Path(plan['output']), Path(plan['target'])
    expected_names = {'candidate-binding.json', 'build-plan.json', 'source-binding.json', 'machine-capture.json'}
    expected_names.update(Path(plan['profiles'][p]['copied_binary']).name for p in PROFILES)
    _closed_names(output, expected_names, invocation)
    source_root = Path(current['source']['view']['path'])
    metadata = owned['probes']['metadata']
    checked = _probe_check(metadata, context, metadata_recipe(), source_root,
                           machine.environment(document['role'], 'debug', target), 30)
    need(checked == document['metadata_process'], 'metadata process changed')
    inspect_cargo_metadata(binding.decode(metadata.stdout()), source_root, target, current['source']['view']['files'])
    need(set(document['profiles']) == set(PROFILES), 'complete exact build profiles')
    need(context.invocation().document()['started_ns'] <= document['started_ns'] <= document['completed_ns'],
         'build outside invocation window')
    need(document['completed_ns'] - document['started_ns'] <= ROLE_STAGE_TIMEOUT_SECONDS * 10**9,
         'build role-stage envelope')
    previous_completed = document['metadata_process']['completed_ns']
    need(document['started_ns'] <= document['metadata_process']['started_ns'] <= previous_completed <= document['completed_ns'],
         'metadata outside original build window')
    for profile in PROFILES:
        row = document['profiles'][profile]
        checked = _probe_check(owned['probes'][profile], context, recipe(profile), source_root,
                               machine.environment(document['role'], profile, target), BUILD_TIMEOUT_SECONDS, 'public-build')
        need(checked == row['process'], 'profile process changed')
        need(previous_completed <= checked['started_ns'] <= checked['completed_ns'] <= document['completed_ns'],
             'profile outside original build window or reordered')
        previous_completed = checked['completed_ns']
        need(row['source_before'] == row['source_after'] == current['source']['context_sha256'] and
             row['machine_before'] == row['machine_after'] == current['machine_sha256'], 'input preservation claim changed')
        emitted, copied = row['emitted_binary'], row['binary']
        inspect_artifact(emitted, plan['profiles'][profile]['emitted_binary'], checked['started_ns'],
                         checked['completed_ns'], document['host'], invocation)
        need(_identity(plan['profiles'][profile]['copied_binary'], with_stat=True, invocation=invocation) == copied,
             'copied compiler identity/stat/path changed')
        need((emitted['bytes'], emitted['sha256']) == (copied['bytes'], copied['sha256']), 'copied/emitted binary mismatch')
    need(document['profiles']['debug']['binary']['sha256'] != document['profiles']['release']['binary']['sha256'],
         'distinct profiles required')
    return binding.decode(binding.canonical(document))


def runtime_inputs(result):
    """Verified live provenance for the separately authenticated runtime owner."""
    document = read_build_binding(result)
    owned = _result(result)
    return {'context': owned['context'], 'machine': owned['machine'],
            'invocation': owned['context'].invocation(), 'role': document['role'],
            'metadata_probe': owned['probes']['metadata'],
            'build_probes': {p: owned['probes'][p] for p in PROFILES},
            'emitted_binaries': {p: document['profiles'][p]['emitted_binary'] for p in PROFILES},
            'copied_binaries': {p: document['profiles'][p]['binary'] for p in PROFILES}}


def collect(*args, **kwargs):
    return binding.runtime_admission(*args, **kwargs)
