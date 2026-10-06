"""Fixed stdout expectations and actual subprocess controls; no Rust builds."""
from __future__ import annotations

import fcntl
import json
import os
from pathlib import Path
import re
import signal
import subprocess

import verify_bounded_enum_native as enum_gate
import verify_bounded_stdin_native as stdin_gate

c = stdin_gate.controls
require = c.require
HELPERS = Path(__file__).resolve().parent
RAW = 'frontend::oir::owned::builtin_output_process_tests::builtin_output_subprocess_child'
SOURCE = 'frontend::oir::owned::source::output_source_tests::output_source_subprocess_child'
PARENTS = (
    ('raw', 'frontend::oir::owned::builtin_output_process_tests::subprocess::builtin_output_process_reference_bytes_and_statuses'),
    ('source', 'frontend::oir::owned::source::output_source_tests::subprocess::output_source_reference_bytes_and_statuses'),
)
RANGE = b'error[E0600] (oir-run): process main must return a status in 0..255\n  --> raw-owned-consumers.ox:1:1\n'
RAW_CASES = (
    ('abc', 0, b'ABC', b''), ('invalid-last-256', 1, b'', b''),
    ('invalid-last-negative', 1, b'', b''), ('empty', 0, b'', b''),
    ('projected', 0, b'ABC', b''), ('forwarded', 0, b'ABC', b''),
    ('status-0', 0, b'', b''), ('status-37', 37, b'', b''),
    ('status-negative', 1, b'', RANGE), ('status-256', 1, b'', RANGE),
)
# Independently authored fuel ledger and its append-only projected correction
# are retained in fuel-oracle.md. These literals are never calibrated from runs.
FUEL_CASES = (
    ('abc', 81, b'', 1), ('abc', 82, b'', 1), ('abc', 83, b'A', 1),
    ('abc', 84, b'AB', 1), ('abc', 85, b'ABC', 1), ('abc', 91, b'ABC', 1),
    ('abc', 122, b'ABC', 1), ('abc', 123, b'ABC', 0),
    ('empty', 106, b'', 1), ('empty', 107, b'', 0),
    ('invalid-last-256', 120, b'', 1), ('invalid-last-256', 121, b'', 1),
    ('projected', 128, b'', 1), ('projected', 129, b'', 1),
    ('projected', 130, b'A', 1), ('projected', 131, b'AB', 1),
    ('projected', 132, b'ABC', 1), ('projected', 178, b'ABC', 1), ('projected', 179, b'ABC', 0),
    ('forwarded', 125, b'', 1), ('forwarded', 126, b'', 1),
    ('forwarded', 127, b'A', 1), ('forwarded', 128, b'AB', 1),
    ('forwarded', 129, b'ABC', 1), ('forwarded', 183, b'ABC', 1), ('forwarded', 184, b'ABC', 0),
    ('status-37', 3, b'', 1), ('status-37', 4, b'', 37),
)
SOURCE_CASES = (
    ('abc', 0, b'ABC'), ('invalid-last-256', 1, b''), ('invalid-last-negative', 1, b''),
    ('empty', 0, b''), ('projected', 0, b'ABC'), ('forwarded', 0, b'ABC'),
    ('mixed-zero-input', 0, b'ABC'), ('status-only', 7, b''), ('module-root', 0, b'ABC'),
    ('status-0', 0, b''), ('status-37', 37, b''),
)
REAL_CASES = (
    ('abc', 'closed-output', 2, b''), ('abc', 'partial-output', 3, b'A'),
    ('empty', 'closed-output', 0, b''), ('invalid-last-256', 'closed-output', 1, b''),
    ('status-256', 'closed-error', 74, b''),
)
INJECTED_CASES = (
    ('retry-first', 'abc', 'two-eintr', 85, 1, b'A', 'fuel'),
    ('retry-return', 'abc', 'two-eintr', 124, 1, b'ABC', 'fuel'),
    ('retry-complete', 'abc', 'two-eintr', 125, 0, b'ABC', 'empty'),
    ('zero', 'abc', 'zero', None, 2, b'', 'empty'),
    ('setup', 'abc', 'setup-failure', None, 74, b'', 'empty'),
    ('stderr-progress', 'status-256', 'diagnostic-short-eintr', None, 1, b'', 'range'),
    ('stderr-error', 'status-256', 'diagnostic-short-error', None, 74, b'', 'one'),
    ('identity-miss', 'abc', 'zero', None, 0, b'ABC', 'empty'),
)


def clean_env(base):
    return {key: value for key, value in c.child_env(base).items()
            if key not in ('LD_PRELOAD', 'LD_AUDIT', 'OXID_OUTPUT_FAULT', 'OXID_PATH')
            and not key.startswith(('OXID_RAW_STDOUT_', 'OXID_SOURCE_STDOUT_', 'OXID_FAULT_', 'OXID_SETUP_'))}


def selected_env(base, kind, value, fuel=None, native=None):
    env = clean_env(base)
    prefix = 'OXID_RAW_STDOUT_' if kind == 'raw' else 'OXID_SOURCE_STDOUT_'
    env[prefix + ('CASE' if kind == 'raw' else 'PATH')] = str(value)
    if fuel is not None:
        env[prefix + 'FUEL'] = str(fuel)
    if native is not None:
        env[prefix + 'NATIVE_DIR'] = str(native)
    return env


def admit_listing(data, names):
    require(names and len(names) == len(set(names)), 'empty or duplicate expected roster')
    count = len(names)
    summary = f'{count} test' + ('' if count == 1 else 's') + ', 0 benchmarks'
    lines = [line for line in data.decode().splitlines() if line]
    require(len(lines) == count + 1 and lines[-1] == summary
            and sorted(lines[:-1]) == sorted(name + ': test' for name in names),
            'missing, duplicate, or unexpected exact test discovery')


def admit_selected_harness(data, name):
    # Selected children exit directly after fd redirection or emission. A normal
    # libtest completion marker here would conceal a zero/effect-free invocation.
    require(data == ('\nrunning 1 test\ntest ' + name + ' ... ').encode(),
            'selected child did not enter exactly the expected direct-exit test')


def admit_effect(result, status, stdout, diagnostic='empty'):
    require(result.returncode == status and result.stdout == stdout,
            f'process status/bytes differ: {(result.returncode, result.stdout)!r}, expected {(status, stdout)!r}')
    error = result.stderr
    if isinstance(diagnostic, bytes):
        require(error == diagnostic, 'exact diagnostic differs')
    elif diagnostic == 'empty':
        require(not error, 'unexpected stderr')
    elif diagnostic == 'fuel':
        require(error.startswith(b'error[E0601] (oir-run): ') and error.endswith(b'\n')
                and error.count(b'error[') == 1 and b'-summary' not in error, 'missing fuel diagnostic')
    elif diagnostic == 'range':
        require(error == RANGE, 'range diagnostic differs')
    elif diagnostic == 'one':
        require(error == b'e', 'partial stderr must retain exactly its accepted prefix')
    else:
        raise ValueError('unknown diagnostic expectation')


def _bind(env, prefix, label, fd, mismatch=False):
    info = os.fstat(fd)
    env[prefix + label + '_DEV'] = str(info.st_dev)
    env[prefix + label + '_INO'] = str(info.st_ino + int(mismatch))


def capture(directory, argv, env, *, child=None, mode='ordinary', shim=None,
            fault=None, setup=None, ordinal=0, mismatch=False, data=b'', timeout=20):
    """Retain separate harness/program bytes, real pipes, exact fd-bound faults."""
    require(mode in ('ordinary', 'closed-output', 'partial-output', 'closed-error'), 'unknown pipe mode')
    require(len(data) <= c.MAX_INPUT, 'oversized process input')
    directory = c.fresh(directory)
    cwd = c.fresh(directory / 'empty-cwd')
    (directory / 'input.bin').write_bytes(data)
    env = dict(env)
    argv = [str(value) for value in argv]
    status = None
    error = None
    reads, writes = [], []
    prefix = b''
    outbytes = errbytes = tracebytes = b''
    try:
        with (directory / 'stdout').open('x+b') as out, (directory / 'stderr').open('x+b') as err, \
             (directory / 'harness.stdout').open('x+b') as harness, (directory / 'trace').open('x+b') as trace, \
             (directory / 'input.bin').open('rb') as input_file:
            stdout, stderr = out.fileno(), err.fileno()
            if mode in ('closed-output', 'partial-output', 'closed-error'):
                rd, wr = os.pipe()
                writes.append(wr)
                if mode == 'closed-error':
                    os.close(rd)
                    stderr = wr
                else:
                    stdout = wr
                    if mode == 'closed-output':
                        os.close(rd)
                    else:
                        reads.append(rd)
                        os.set_blocking(wr, False)
                        capacity = fcntl.fcntl(wr, fcntl.F_GETPIPE_SZ)
                        prefix = b'x' * (capacity - 1)
                        require(os.write(wr, prefix) == len(prefix), 'short real-pipe setup write')
            inherited = []
            if child:
                env['OXID_RAW_STDOUT_FD'] = str(stdout)
                inherited.append(stdout)
            if shim:
                env.update(LD_PRELOAD=str(shim), OXID_OUTPUT_FAULT=fault)
                _bind(env, 'OXID_FAULT_', 'STDOUT', stdout, mismatch)
                _bind(env, 'OXID_FAULT_', 'STDERR', stderr)
            if setup:
                require(not child and not shim, 'setup-ordinal control requires public process')
                env.update(LD_PRELOAD=str(setup), OXID_SETUP_FAIL_ORDINAL=str(ordinal))
                info = os.stat(argv[0])
                env.update(OXID_SETUP_EXE_DEV=str(info.st_dev), OXID_SETUP_EXE_INO=str(info.st_ino))
                _bind(env, 'OXID_SETUP_', 'STDOUT', stdout, mismatch)
                _bind(env, 'OXID_SETUP_', 'STDERR', stderr)
                _bind(env, 'OXID_SETUP_', 'TRACE', trace.fileno())
                env['OXID_SETUP_TRACE_FD'] = str(trace.fileno())
                inherited.append(trace.fileno())
            process = subprocess.Popen(argv, stdin=input_file, stdout=harness if child else stdout,
                stderr=stderr, cwd=cwd, env=env, pass_fds=tuple(inherited),
                start_new_session=True, preexec_fn=c._limits)
            try:
                status = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                status = process.wait()
                raise
            for fd in writes:
                os.close(fd)
            writes.clear()
            out.seek(0)
            outbytes = out.read(c.MAX_STREAM + 1)
            if reads:
                raw = b''
                while block := os.read(reads[0], 8192):
                    raw += block
                    require(len(raw) <= c.MAX_STREAM, 'pipe capture exceeds cap')
                require(raw.startswith(prefix), 'real pipe prefix changed')
                (directory / 'pipe-capture').write_bytes(raw)
                outbytes = raw[len(prefix):]
                out.seek(0)
                out.write(outbytes)
            err.seek(0)
            errbytes = err.read(c.MAX_STREAM + 1)
            trace.seek(0)
            tracebytes = trace.read(64)
            require(len(outbytes) <= c.MAX_STREAM and len(errbytes) <= c.MAX_STREAM, 'capture exceeds cap')
            if child:
                harness.seek(0)
                admit_selected_harness(harness.read(c.MAX_STREAM), child)
    except Exception as caught:
        error = str(caught)
    finally:
        for fd in reads + writes:
            os.close(fd)
        c.save_json(directory / 'receipt.json', {'argv': argv, 'cwd': str(cwd), 'status': status,
            'error': error, 'mode': mode, 'kind': 'injected-syscall-return' if shim or setup else 'real-process',
            'environment': {'mode': 'selected-child' if child else ('cleared-runtime' if env.get('PATH') == '/no-tools' else 'public-cli'),
                'configured': {key: value for key, value in env.items() if key.startswith('OXID_') or key == 'LD_PRELOAD'}},
            'executable': c.identity(Path(argv[0])),
            'streams': [c.identity(directory / name) for name in ('input.bin', 'stdout', 'stderr', 'harness.stdout', 'trace')],
            'trace_ascii': tracebytes.decode('ascii')})
    require(error is None, 'process failure: ' + str(error))
    return subprocess.CompletedProcess(argv, status, outbytes, errbytes), tracebytes


def build_instrumentation(root, clang, env):
    directory = c.fresh(root / 'instrumentation')
    products = {}
    for name, source, shared in (('faults', 'faults.c', True), ('setup', 'setup_ordinal.c', True), ('calibration', 'calibration.c', False)):
        target = directory / (name + ('.so' if shared else ''))
        flags = ['-std=c11', '-O2', '-Wall', '-Wextra', '-Werror']
        if shared:
            flags += ['-fPIC', '-shared', '-Wl,-z,defs', '-Wl,-z,relro,-z,now']
        result, _ = c.run(directory / ('build-' + name), [clang, *flags, HELPERS / source, '-o', target, '-ldl'], env, timeout=120)
        require((result.returncode, result.stdout, result.stderr) == (0, b'', b''), 'instrumentation build failed')
        products[name] = target
    result, _ = capture(directory / 'calibration-two-eintr', [products['calibration']], c.ELF_ENV,
                        shim=products['faults'], fault='two-eintr')
    admit_effect(result, 0, b'Z', b'C')
    # Same executable and scripted mode, mismatched output identity: no injected
    # first interruption, so the independent probe returns its literal status 92.
    result, _ = capture(directory / 'calibration-identity-miss', [products['calibration']], c.ELF_ENV,
                        shim=products['faults'], fault='two-eintr', mismatch=True)
    admit_effect(result, 92, b'Z', b'C')
    c.save_json(directory / 'identities.json', {name: c.identity(path) for name, path in products.items()})
    return products


class Suite:
    def __init__(self, root, unit, cli, env, instrumentation):
        self.root, self.unit, self.cli, self.env = root, unit, cli, clean_env(env)
        self.instrumentation = instrumentation
        self.artifacts = {}
        self.public_identities = {}
        self.source_identities = []
        self.counts = {}
        self.rows = []
        self.sources_root = c.fresh(root / 'source-fixtures')

    def record(self, group, label):
        self.counts[group] = self.counts.get(group, 0) + 1
        self.rows.append({'group': group, 'case': label})

    def selection(self):
        roster = json.loads(c.read_file(HELPERS / 'roster.json'))
        require([len(v) for v in roster['modules'].values()] == [9, 6, 10, 13, 8, 7]
                and len(roster['named']) == 16, 'reviewed test roster changed')
        names = []
        for index, (prefix, expected) in enumerate(roster['modules'].items()):
            result, _ = c.run(self.root / f'module-discovery-{index}', [self.unit, prefix, '--list', '--color=never'], self.env)
            require(result.returncode == 0 and not result.stderr, 'module discovery failed')
            admit_listing(result.stdout, expected)
            names.extend(expected)
        for index, name in enumerate(roster['named'] + [RAW, PARENTS[0][1]]):
            result, _ = c.run(self.root / f'exact-discovery-{index}', [self.unit, name, '--exact', '--list', '--color=never'], self.env)
            require(result.returncode == 0 and not result.stderr, 'exact discovery failed')
            admit_listing(result.stdout, [name])
            names.append(name)
        require(len(names) == len(set(names)) == 71, 'duplicate or incomplete selected roster')
        parents = {name: kind for kind, name in PARENTS}
        for index, name in enumerate(names):
            env = clean_env(self.env)
            argv = c.test_command(self.unit, name)
            if name in parents:
                argv.append('--ignored')
                env['OXID_' + parents[name].upper() + '_STDOUT_EVIDENCE_DIR'] = str(self.root / (parents[name] + '-parent-evidence'))
            result, remaining = c.run(self.root / f'selected-test-{index:02}', argv, env, b'UNCHANGED', timeout=180)
            require(result.returncode == 0 and not result.stderr, 'selected unit proof failed: ' + name)
            enum_gate.admit_execution(result.stdout, name)
            if name in (RAW, SOURCE):
                require(remaining == b'UNCHANGED', 'inert child consumed stdin')
            self.record('selected-tests', name)
        c.save_json(self.root / 'selected-tests.json', {'count': len(names), 'names': names,
            'reference_only_parents': list(parents), 'inert_children': [RAW, SOURCE]})

    def emit(self, kind, value, fuel=None):
        key = (kind, str(value), fuel)
        if key not in self.artifacts:
            directory = c.fresh(self.root / f'emission-{len(self.artifacts):03}')
            artifact = c.fresh(directory / 'artifact')
            child = RAW if kind == 'raw' else SOURCE
            env = selected_env(self.env, kind, value, fuel, artifact)
            result, _ = c.run(directory / 'build', c.test_command(self.unit, child), env, timeout=120)
            require(result.returncode == 0 and not result.stderr, 'private native emission failed')
            admit_selected_harness(result.stdout, child)
            binary = artifact / 'program'
            require(c.read_file(binary).startswith(b'\x7fELF'), 'emitted native program is not ELF')
            identities = [c.identity(artifact / name) for name in ('program.ll', 'program')]
            c.save_json(directory / 'artifacts.json', {'kind': kind, 'value': str(value), 'fuel': fuel,
                'artifacts': identities, 'compiler_unit': c.identity(self.unit)})
            self.artifacts[key] = (binary, identities)
        return self.artifacts[key][0]

    def pair(self, group, label, kind, value, status, stdout, diagnostic='empty', fuel=None,
             mode='ordinary', fault=None, mismatch=False):
        directory = c.fresh(self.root / (group + '-' + label))
        child = RAW if kind == 'raw' else SOURCE
        binary = self.emit(kind, value, fuel)
        outputs = []
        for backend in ('reference', 'native'):
            argv = c.test_command(self.unit, child) if backend == 'reference' else [binary]
            env = selected_env(self.env, kind, value, fuel) if backend == 'reference' else c.ELF_ENV
            result, _ = capture(directory / backend, argv, env, child=child if backend == 'reference' else None,
                mode=mode, shim=self.instrumentation['faults'] if fault else None, fault=fault, mismatch=mismatch)
            admit_effect(result, status, stdout, diagnostic)
            outputs.append(result.stderr)
        require(outputs[0] == outputs[1], 'reference/native stderr differs')
        self.record(group, label)

    def raw(self):
        for case, status, stdout, stderr in RAW_CASES:
            self.pair('raw', case, 'raw', case, status, stdout, stderr)
        for case, fuel, stdout, status in FUEL_CASES:
            semantic_invalid = case == 'invalid-last-256' and fuel == 121
            diagnostic = 'fuel' if status == 1 and not semantic_invalid else 'empty'
            self.pair('fuel', case + '-' + str(fuel), 'raw', case, status, stdout, diagnostic, fuel)
        for case, mode, status, stdout in REAL_CASES:
            self.pair('real-pipe', case + '-' + mode, 'raw', case, status, stdout, mode=mode)
        for name, case, fault, fuel, status, stdout, diagnostic in INJECTED_CASES:
            self.pair('injected', name, 'raw', case, status, stdout, diagnostic, fuel,
                      fault=fault, mismatch=name == 'identity-miss')

    def fixture(self, name, source=None):
        directory = c.fresh(self.sources_root / name)
        path = directory / 'main.ox'
        if source is None:
            source = c.read_file(HELPERS / (name + '.txt')).decode()
        path.write_text(source)
        if name == 'module-root':
            (directory / 'child.ox').write_bytes(c.read_file(HELPERS / 'module-child.txt'))
        self.source_identities.extend(c.identity(item) for item in sorted(directory.iterdir()))
        return path

    def sources(self):
        for name, status, stdout in SOURCE_CASES:
            source = f'fn main()->i32{{return {status};}}' if name.startswith('status-') and name != 'status-only' else None
            path = self.fixture(name, source)
            self.pair('parsed-source', name, 'source', path, status, stdout)

    def cli_run(self, label, argv, *, status=0, stdout=None, diagnostic=None, mode='ordinary'):
        result, _ = capture(self.root / ('public-' + label), [self.cli, *argv], self.env, mode=mode)
        require(result.returncode == status, 'public CLI status differs: ' + label)
        if stdout is not None:
            require(result.stdout == stdout, 'public CLI bytes differ: ' + label)
        if diagnostic is None:
            require(not result.stderr, 'unexpected public stderr: ' + label)
        else:
            require(result.stderr.startswith(('error[' + diagnostic + '] ').encode())
                    and b'-summary' not in result.stderr, 'public process diagnostic differs: ' + label)
        self.record('public-cli', label)
        return result

    def public(self):
        # Exact literal source artifact bytes, scalar statuses, original main,
        # ordinary calls to main, portable status imports, and unused inventory.
        tiny = self.fixture('public-tiny', 'use std::io::write_stdout as emit; use std::io::WriteStatus as Written; fn main()->i32{let bytes=[0,65,255];let written=emit(&bytes);match written{Written::Complete=>{return 39;},Written::InvalidInput=>{return 2;},Written::IoError(n)=>{return n+3;},}}')
        self.public_pair('tiny', tiny, 39, bytes((0, 65, 255)))
        after_output = self.fixture('after-output-range', c.read_file(HELPERS / 'abc.txt').decode().replace('return 0;', 'return 256;'))
        self.public_pair('after-output-range', after_output, 1, b'ABC', diagnostic='E0600')
        tiny_elf = self.public_artifacts['tiny']
        for status in (0, 37, 255, -256, 256):
            name = 'scalar-' + str(status)
            source = self.fixture(name, f'fn main()->i32{{return {status};}}')
            self.public_pair(name, source, status if 0 <= status <= 255 else 1, b'',
                             diagnostic='E0600' if status < 0 or status > 255 else None)
            self.cli_run(name + '-result', ['run', source, '--edition=typed-preview'], stdout=f'{status}\n'.encode())
        called = self.fixture('called-main', 'fn main()->i32{return 7;} fn other()->i32{return main();}')
        self.public_pair('called-main', called, 7, b'')
        portable = self.fixture('portable-status', 'use std::io::WriteStatus; fn main()->i32{return 7;}')
        self.cli_run('portable-status', ['run', portable, '--edition=typed-preview'], stdout=b'7\n')
        for index, text in enumerate((
            'use std::io::write_stdout; fn main()->i32{return 0;}',
            'use std::io::write_stdout as unused; fn main()->i32{return 0;}',
            'use std::io::write_stdout; use std::io::WriteStatus; fn helper(bytes:&[i32])->WriteStatus{return write_stdout(&*bytes);} fn main()->i32{return 0;}',
        )):
            source = self.fixture('denial-' + str(index), text)
            for operation in ('run', 'compile'):
                args = [operation, source, '--edition=typed-preview', '--message-format=json']
                forbidden = self.root / f'forbidden-{index}'
                if operation == 'compile':
                    args += ['--backend=llvm', '--output=' + str(forbidden)]
                result = self.cli_run(f'default-denial-{index}-{operation}', args, status=1)
                require(b'"code":"E0609","stage":"oir-owned-run"' in result.stdout
                        and b'native-toolchain' not in result.stdout and not forbidden.exists(),
                        'default complete inventory denial missing')
        missing = self.sources_root / 'absent.ox'
        early = (
            ['run', missing, '--edition=typed-preview', '--entry-mode=process', '--message-format=json'],
            ['run', missing, '--edition=typed-preview', '--message-format=json', '--entry-mode'],
            ['run', missing, '--edition=typed-preview', '--entry-mode=unknown', '--message-format=json'],
            ['run', missing, '--edition=typed-preview', '--entry-mode=process', '--entry-mode=result'],
            ['run', missing, '--edition=legacy-0.9', '--entry-mode=process'],
            ['run', missing, '--edition=typed-preview', '--unknown', '--entry-mode=process'],
        )
        for index, args in enumerate(early):
            self.cli_run('early-' + str(index), args, status=1, stdout=b'', diagnostic='E0001')
        for operation in ('check', 'fmt'):
            result = self.cli_run(operation + '-process-rejected', [operation, missing, '--edition=typed-preview',
                '--entry-mode=process', '--message-format=json'], status=1)
            require(b'"code":"E0001"' in result.stdout and b'-summary' in result.stdout, 'nonexecuting option restriction missing')
        for index, text in enumerate(('fn helper()->i32{return 1;}', 'fn main(n:i32)->i32{return n;}',
                                      'fn main()->bool{return false;}')):
            source = self.fixture('invalid-entry-' + str(index), text)
            self.cli_run('entry-' + str(index), ['run', source, '--edition=typed-preview', '--entry-mode=process'],
                         status=1, stdout=b'', diagnostic='E0600')
        self.cli_run('missing-source', ['run', missing, '--edition=typed-preview', '--entry-mode=process'],
                     status=1, stdout=b'', diagnostic='E0002')
        self.public_effects(tiny, tiny_elf)

    def public_pair(self, name, source, status, stdout, diagnostic=None):
        if not hasattr(self, 'public_artifacts'):
            self.public_artifacts = {}
        reference = self.cli_run(name + '-reference', ['run', source, '--edition=typed-preview', '--entry-mode=process'],
                     status=status, stdout=stdout, diagnostic=diagnostic)
        binary = self.root / ('public-' + name + '-program')
        result = self.cli_run(name + '-compile', ['compile', source, '--edition=typed-preview',
            '--entry-mode=process', '--backend=llvm', '--output=' + str(binary), '--message-format=json'])
        require(b'"kind":"compile-summary","success":true' in result.stdout
                and b'run-summary' not in result.stdout, 'native JSON must be an effect-free build report')
        require(c.read_file(binary).startswith(b'\x7fELF'), 'public native artifact is not ELF')
        self.public_artifacts[name] = binary
        before = c.identity(binary)
        self.public_identities[name] = before
        result, _ = capture(self.root / ('public-' + name + '-native'), [binary], c.ELF_ENV)
        require((result.returncode, result.stdout) == (status, stdout), 'public emitted ELF status/bytes differ')
        if diagnostic:
            require(result.stderr.startswith(('error[' + diagnostic + '] ').encode()), 'native scalar diagnostic missing')
        else:
            require(not result.stderr, 'native public stderr')
        require(result.stderr == reference.stderr, 'public reference/native exact stderr differs')
        require(c.identity(binary) == before, 'public ELF changed during execution')
        c.save_json(self.root / ('public-' + name + '-identity.json'), before)
        self.record('public-native', name)

    def public_effects(self, tiny, binary):
        setup = self.instrumentation['setup']
        reference = [self.cli, 'run', tiny, '--edition=typed-preview', '--entry-mode=process']
        for backend, argv, env, calibration, ordinal in (
            ('reference', reference, self.env, b'FFFF', 2), ('native', [binary], c.ELF_ENV, b'F', 1)):
            result, trace = capture(self.root / ('public-setup-calibration-' + backend), argv, env, setup=setup)
            admit_effect(result, 39, bytes((0, 65, 255)))
            require(trace == calibration, 'unexpected process setup ordinal calibration')
            result, trace = capture(self.root / ('public-setup-failure-' + backend), argv, env, setup=setup, ordinal=ordinal)
            admit_effect(result, 74, b'')
            require(trace == b'F' * (ordinal - 1) + b'X', 'startup must forward before process setup failure')
            result, _ = capture(self.root / ('public-closed-output-' + backend), argv, env, mode='closed-output')
            admit_effect(result, 3, b'')
            bad = self.sources_root / 'scalar-256/main.ox'
            bad_args = [self.cli, 'run', bad, '--edition=typed-preview', '--entry-mode=process'] if backend == 'reference' else [self.public_artifacts['scalar-256']]
            result, _ = capture(self.root / ('public-closed-error-' + backend), bad_args, env, mode='closed-error')
            admit_effect(result, 74, b'')
            result, _ = capture(self.root / ('public-partial-error-' + backend), bad_args, env,
                                shim=self.instrumentation['faults'], fault='diagnostic-short-error')
            admit_effect(result, 74, b'', 'one')
            self.record('public-effects', backend)
        result, trace = capture(self.root / 'public-setup-identity-miss', reference, self.env,
                                setup=setup, ordinal=2, mismatch=True)
        admit_effect(result, 39, bytes((0, 65, 255)))
        require(not trace, 'mismatched setup identity was injected')
        # A FIFO with no writer blocks source loading. Explicit failed setup must
        # finish before touching it; the control proves this is the live gateway.
        fifo = self.sources_root / 'unread-source.ox'
        os.mkfifo(fifo, 0o600)
        argv = [self.cli, 'run', fifo, '--edition=typed-preview', '--entry-mode=process']
        try:
            capture(self.root / 'public-fifo-control', argv, self.env, timeout=0.25)
        except ValueError:
            receipt = json.loads(c.read_file(self.root / 'public-fifo-control/receipt.json'))
            require(receipt['status'] == -9 and 'timed out' in receipt['error']
                    and not c.read_file(self.root / 'public-fifo-control/stdout')
                    and not c.read_file(self.root / 'public-fifo-control/stderr'), 'FIFO control failed for wrong reason')
        else:
            raise ValueError('unwritten source FIFO unexpectedly completed')
        result, trace = capture(self.root / 'public-setup-before-load', argv, self.env, setup=setup, ordinal=2)
        admit_effect(result, 74, b'')
        require(trace == b'FX', 'setup failure was not before source load')
        self.record('public-effects', 'identity-and-preload')

    def finish(self):
        expected = {'selected-tests': 71, 'raw': 10, 'fuel': 28, 'real-pipe': 5, 'injected': 8,
                    'parsed-source': 11, 'public-cli': 40, 'public-native': 8, 'public-effects': 3}
        require(self.counts == expected, f'exact stdout execution roster differs: {self.counts}')
        for binary, identities in self.artifacts.values():
            require([c.identity(binary.with_name(name)) for name in ('program.ll', 'program')] == identities,
                    'private emitted artifacts changed during execution')
        require([c.identity(Path(row['path'])) for row in self.source_identities] == self.source_identities,
                'materialized source fixtures changed during execution')
        require(all(c.identity(Path(row['path'])) == row for row in self.public_identities.values()),
                'public emitted artifacts changed during execution')
        c.save_json(self.root / 'stdout-cases.json', {'counts': self.counts, 'cases': self.rows,
            'native_artifact_count': len(self.artifacts), 'source_fixtures': self.source_identities,
            'scope': 'paired rows execute both the actual reference child and the separately emitted native ELF'})
