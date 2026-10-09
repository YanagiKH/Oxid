#!/usr/bin/env python3
"""Bounded process capture and fresh source/native execution evidence."""
import sys
sys.dont_write_bytecode = True
import copy
import errno
import json
import os
import queue
import threading
import re
import shutil
import signal
import stat
import struct
import subprocess
import time
from pathlib import Path
from contracts import Reject, need, sha, load, save, binding, verify
from authority import CURRENT_SOURCE_SHA, CURRENT_FILES_SHA, LIFECYCLE_PATCH_SHA, OBSERVER_FILES_SHA, LLVM_CONTENT_SHA, CURRENT_SOURCE_MEMBERS, OBSERVER_SOURCE_MEMBERS

MAX_STREAM_BYTES = 8 * 1024 * 1024

def process(argv, cwd, env, timeout=120, limit=MAX_STREAM_BYTES, new_session=True):
    from process_tree import ProcessTree
    start = time.time_ns()
    tree = ProcessTree(argv, cwd, env, new_session=new_session)
    child = tree.child
    streams = {'stdout': bytearray(), 'stderr': bytearray()}
    chunks = queue.Queue(maxsize=8)
    def reader(pipe, name):
        try:
            while True:
                data = os.read(pipe.fileno(), 65536)
                chunks.put((name, data))
                if not data:
                    break
        except OSError as error:
            chunks.put(('reader-error', (name, repr(error))))
            chunks.put((name, b''))
        finally:
            pipe.close()
    readers = [threading.Thread(target=reader, args=(getattr(child, name), name), daemon=True, name='unit4-pipe-reader-' + name) for name in streams]
    for thread in readers:
        thread.start()
    deadline = time.monotonic() + timeout
    timed_out = False
    exceeded = False
    killed = False
    closed = set()
    reader_error = None
    cleanup_deadline = None
    try:
        while len(closed) != 2 or child.poll() is None:
            if time.monotonic() >= deadline:
                timed_out = True
            if (timed_out or exceeded or reader_error) and not killed:
                tree.kill()
                killed = True
                cleanup_deadline = time.monotonic() + 3
            if cleanup_deadline is not None and time.monotonic() > cleanup_deadline:
                raise TimeoutError('bounded subprocess pipe cleanup failed')
            try:
                name, data = chunks.get(timeout=0.05)
            except queue.Empty:
                continue
            if name == 'reader-error':
                reader_error = data
                continue
            if not data:
                closed.add(name)
                continue
            remaining = limit - len(streams[name])
            streams[name].extend(data[:remaining])
            if len(data) > remaining:
                exceeded = True
        status = child.wait()
        for thread in readers:
            thread.join(timeout=1)
        if reader_error:
            raise OSError('subprocess reader failure: ' + repr(reader_error))
    finally:
        tree.close()
    result = {'argv': list(argv), 'cwd': str(cwd), 'pid': child.pid, 'status': status,
              'started_ns': start, 'completed_ns': time.time_ns(), 'timed_out': timed_out,
              'stream_limit_exceeded': exceeded, 'stream_limit_bytes': limit}
    for name, data in streams.items():
        result[name] = bytes(data).decode('utf-8')
        result[name + '_sha256'] = sha(data)
    return result

def file_identity(path, follow=True):
    s = Path(path).stat() if follow else Path(path).lstat()
    return {k: getattr(s, k) for k in ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')}

def inventory(root):
    rows = []
    for path in sorted(Path(root).rglob('*')):
        rel = path.relative_to(root).as_posix()
        if path.is_symlink():
            rows.append({'path': rel, 'kind': 'symlink', 'target': os.readlink(path)})
        elif path.is_dir():
            rows.append({'path': rel, 'kind': 'directory'})
        else:
            data = path.read_bytes()
            rows.append({'path': rel, 'kind': 'file', 'bytes': len(data), 'sha256': sha(data)})
    return rows

def source_map(inv, exclude=()):
    return sorted([{k: row[k] for k in ('path', 'bytes', 'sha256')} for row in inv if row['kind'] == 'file' and row['path'] not in exclude], key=lambda row: row['path'])

def materialize(sources, target):
    target.mkdir(parents=True, exist_ok=False)
    for name, data in sources.items():
        p = Path(name)
        need(not p.is_absolute() and '..' not in p.parts, 'unsafe source name')
        destination = target / p
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
    return [{'path': name, 'bytes': len(data), 'sha256': sha(data)} for name, data in sorted(sources.items())]

def source_manifest(path, root):
    manifest = load(path)
    declared = {row['path'] for row in manifest['files']}
    need(len(declared) == len(manifest['files']), 'duplicate source manifest path')
    for row in manifest['files']:
        verify(Path(root) / row['path'], row)
    # Source-file membership is checked, not just the listed file hashes.
    actual = {p.relative_to(root).as_posix() for base in ('src', 'native') for p in (Path(root) / base).rglob('*') if p.is_file()}
    need(actual == {p for p in declared if p.startswith(('src/', 'native/'))}, 'source manifest membership')
    return binding(path)

def candidate_binding(path):
    cfg = load(path)
    need(all(isinstance(cfg[key], str) and len(cfg[key]) == 40 for key in ('compiler_head', 'compiler_head_tree', 'compiler_source_only_tree')), 'compiler provenance fields')
    need(cfg['schema_version'] == 1 and cfg['kind'] in ('unit4-public-v3-candidate', 'unit4-public-v3-lifecycle-observer'), 'candidate binding kind')
    verify(cfg['source_manifest']['path'], cfg['source_manifest'])
    source_manifest(cfg['source_manifest']['path'], cfg['source_root'])
    manifest = load(cfg['source_manifest']['path'])
    files_sha = sha(json.dumps(manifest['files'], sort_keys=True, separators=(',', ':')).encode())
    if cfg['kind'] == 'unit4-public-v3-candidate':
        need(cfg['source_manifest']['sha256'] == CURRENT_SOURCE_SHA and files_sha == CURRENT_FILES_SHA and len(manifest['files']) == CURRENT_SOURCE_MEMBERS, 'unapproved current compiler source authority')
    else:
        need(files_sha == OBSERVER_FILES_SHA and len(manifest['files']) == OBSERVER_SOURCE_MEMBERS, 'unapproved lifecycle observer bodies')
        need(manifest['base_source_manifest_sha256'] == cfg['base_source_manifest_sha256'] == CURRENT_SOURCE_SHA, 'observer exact approved base')
        need(cfg['observer_patch']['sha256'] == manifest['observer_patch']['sha256'] == LIFECYCLE_PATCH_SHA, 'observer exact approved overlay')
        verify(cfg['observer_patch']['path'], cfg['observer_patch'])
    need(set(cfg['binaries']) == {'debug', 'release'}, 'candidate profiles')
    for profile, binary in cfg['binaries'].items():
        verify(binary['path'], binary)
        receipt_binding = cfg['build_receipts'][profile]
        receipt = load(receipt_binding['path'])
        verify(receipt_binding['path'], receipt_binding)
        need(receipt['source_manifest_sha256'] == cfg['source_manifest']['sha256'], 'build/source manifest binding')
        need(receipt['profile'] == profile and receipt['status'] == 0, 'build profile/status')
        need(receipt['binary'] == binary, 'build/binary binding')
        need(receipt['argv'] == ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline'] + (['--release'] if profile == 'release' else []), 'bound build argv')
        need(receipt['source_before'] == receipt['source_after'] == cfg['source_manifest']['sha256'], 'build source preservation')
        need(receipt['environment']['CARGO_INCREMENTAL'] == '0' and receipt['environment']['CARGO_BUILD_JOBS'] == '2', 'build resource settings')
        for stream in receipt['streams'].values():
            verify(stream['path'], stream)
    cfg['_sha256'] = sha(Path(path).read_bytes())
    return cfg

def capability(fixture, setup):
    target = fixture / setup['target']
    need(target.is_file() and not target.is_symlink(), 'capability target not regular')
    prior_mode = stat.S_IMODE(target.stat().st_mode)
    data = target.read_bytes()
    target.chmod(int(setup['temporary_mode_octal'], 8))
    proof = {'target': setup['target'], 'content_before_sha256': sha(data), 'mode_original': prior_mode,
             'mode_before_candidate': stat.S_IMODE(target.stat().st_mode), 'effective_uid': os.geteuid(),
             'effective_gid': os.getegid(), 'groups': os.getgroups()}
    try:
        with target.open('rb'):
            proof['read_open_succeeded'] = True
    except OSError as error:
        proof['read_open_succeeded'] = False
        proof['error_errno'] = error.errno
        need(error.errno in (errno.EACCES, errno.EPERM), 'mode capability failed for unrelated reason')
    return proof

def restore_capability(fixture, proof):
    target = fixture / proof['target']
    proof['mode_after_candidate'] = stat.S_IMODE(target.stat().st_mode)
    target.chmod(proof['mode_original'])
    proof['content_after_sha256'] = sha(target.read_bytes())
    proof['source_restored'] = proof['content_before_sha256'] == proof['content_after_sha256'] and stat.S_IMODE(target.stat().st_mode) == proof['mode_original']
    need(proof['mode_after_candidate'] == 0 and proof['source_restored'], 'capability source mutation')

def native(fixture, evidence, sources, env):
    original = fixture / 'out.bin'
    data = original.read_bytes()
    need(data[:6] == b'\x7fELF\x02\x01' and len(data) >= 20 and struct.unpack('<H', data[18:20])[0] == 62, 'native output is not x86_64 ELF')
    dest = evidence / 'native-source-free'
    dest.mkdir()
    copied = dest / 'program'
    shutil.copy2(original, copied)
    before = inventory(dest)
    need(before == [{'path': 'program', 'kind': 'file', 'bytes': len(data), 'sha256': sha(data)}], 'native directory not ELF-only')
    result = {'original_elf': binding(original), 'copied_elf': binding(copied), 'directory_before': before}
    source_bytes = {name: (fixture / name).read_bytes() for name in sources}
    modes = {name: stat.S_IMODE((fixture / name).stat().st_mode) for name in sources}
    try:
        for name in source_bytes:
            (fixture / name).unlink()
        result['sources_unavailable'] = [name for name in sources if not os.path.lexists(fixture / name)]
        need(result['sources_unavailable'] == list(sources), 'source-free removal roster')
        result['process'] = process([str(copied)], dest, env)
        result['directory_after'] = inventory(dest)
    finally:
        for name, data in source_bytes.items():
            (fixture / name).write_bytes(data)
            (fixture / name).chmod(modes[name])
        result['restored_sources'] = [{'path': name, 'bytes': len(data), 'sha256': sha((fixture / name).read_bytes())} for name, data in sorted(source_bytes.items())]
        result['sources_restored_exactly'] = all((fixture / name).read_bytes() == data and stat.S_IMODE((fixture / name).stat().st_mode) == modes[name] for name, data in source_bytes.items())
    return result

def check_native(result, expected, sources):
    need(result['original_elf']['sha256'] == result['copied_elf']['sha256'], 'copied ELF identity')
    need(result['sources_unavailable'] == list(sources), 'native source-free roster')
    need(result['sources_restored_exactly'] and result['restored_sources'] == [{'path': k, 'bytes': len(v), 'sha256': sha(v)} for k, v in sorted(sources.items())], 'native source restoration')
    need(result['directory_before'] == result['directory_after'] and len(result['directory_before']) == 1, 'source-free directory mutation')
    p = result['process']
    need(not p['timed_out'] and not p['stream_limit_exceeded'], 'native execution incomplete')
    need(p['argv'] == [result['copied_elf']['path']] and p['cwd'] == str(Path(result['copied_elf']['path']).parent), 'native invocation binding')
    need(p['completed_ns'] >= p['started_ns'], 'native process timing')
    for stream in ('stdout', 'stderr'):
        need(sha(p[stream].encode()) == p[stream + '_sha256'], 'native stream identity')
    for field in ('status', 'stdout', 'stderr'):
        need(p[field] == expected[field], 'native envelope ' + field)

TOOL_LOG_LIMIT = 2 * 1024 * 1024

def read_events(path, limit=TOOL_LOG_LIMIT):
    if not path.exists():
        return []
    data = path.read_bytes()
    need(len(data) <= limit, 'event log byte limit')
    lines = data.splitlines()
    need(len(lines) <= 10000, 'event log row limit')
    return [json.loads(line) for line in lines]

def setup_tools(run, llvm, libs, race=False):
    llvm = Path(llvm).resolve()
    wrappers = run / ('race-wrappers' if race else 'tool-wrappers')
    wrappers.mkdir()
    source = Path(__file__).with_name('tool_wrapper.py')
    for tool in ('clang', 'opt', 'ld.lld'):
        shutil.copy2(source, wrappers / tool)
        (wrappers / tool).chmod(0o755)
    env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1', 'OXID_LLVM_BIN': str(wrappers), 'UNIT4_REAL_LLVM': str(llvm), 'LD_LIBRARY_PATH': str(libs)}
    env.pop('UNIT4_LIFECYCLE_LOG', None)
    tools = {}
    for tool, marker in (('clang', 'clang version 19.1.7'), ('opt', 'LLVM version 19.1.7'), ('ld.lld', 'LLD 19.1.7')):
        p = process([str(llvm / tool), '--version'], run, env)
        need(p['status'] == 0 and re.search(re.escape(marker) + r'(?=$|[ \(\r\n])', p['stdout']) is not None and not p['timed_out'] and not p['stream_limit_exceeded'], 'LLVM 19.1.7 identity')
        tools[tool] = {'executable': binding(llvm / tool), 'wrapper': binding(wrappers / tool), 'version': p}
    return env, tools

def check_tools(calls, tools, expected_native, emitted):
    need(len(calls) <= 32, 'tool invocation upper bound')
    if not expected_native:
        need(calls == [] and emitted is None, 'external tools before admission/denial')
        return
    for call in calls:
        expected = tools[call['tool']]['executable']
        need(call['real_path'] == expected['path'] and call['real_sha256'] == expected['sha256'], 'real tool binding')
        need(call['status'] == 0 and not call['timed_out'] and not call['stream_limit_exceeded'], 'real tool completion')
        for stream in ('stdout', 'stderr'):
            need(sha(call[stream].encode()) == call[stream + '_sha256'], 'tool stream identity')
    need(len(calls) >= 8 and emitted is not None, 'missing LLVM execution/emission')
    need(sum(call['tool'] == 'opt' and call['argv'] == ['-passes=verify', '-disable-output', 'program.ll'] for call in calls) == 1, 'LLVM verification count')
    compiles = [call for call in calls if call['tool'] == 'clang' and '-c' in call['argv']]
    need(len(compiles) == 2 and all('-O0' in call['argv'] for call in compiles), 'O0 compile count')
    need(any(call['tool'] == 'ld.lld' and '--version' not in call['argv'] for call in calls), 'actual link missing')


def staged_runtime(receipt_path, library_path):
    receipt_path = Path(receipt_path).resolve()
    receipt = load(receipt_path)
    need(receipt['schema'] in ('oxid-restored-llvm-runtime-v1', 'oxid-unit3-portable-v1-llvm-runtime-stage'), 'unsupported staged-runtime receipt')
    content_sha = sha(json.dumps(receipt['content'], sort_keys=True, separators=(',', ':')).encode())
    need(content_sha == receipt['content_sha256'] == LLVM_CONTENT_SHA, 'selected LLVM runtime identity')
    root = Path(library_path).resolve()
    need(str(root) == library_path and os.pathsep not in library_path, 'runtime path must be one explicit staged directory')
    need(Path(receipt['library_directory']).resolve() == root, 'runtime receipt directory differs from --libs')
    names = set()
    files = []
    for row in receipt['content']['entries']:
        path = root / row['path']
        names.add(row['path'])
        if row['kind'] == 'symlink':
            need(path.is_symlink() and os.readlink(path) == row['link'], 'staged runtime symlink')
            files.append({'path': str(path), 'kind': 'symlink', 'target': row['link']})
        else:
            need(row['soname'] == row['path'] and not path.is_symlink(), 'staged runtime SONAME/file')
            verify(path, row)
            files.append({**binding(path), 'kind': 'file', 'soname': row['soname']})
    need({path.name for path in root.iterdir()} == names, 'selected staged-runtime directory contains unexpected overrides')
    return {'receipt': binding(receipt_path), 'library_directory': str(root), 'content_sha256': content_sha,
            'selected_libraries': files, 'dependency_scope': receipt['dependency_scope']}


def setup_no_tool_traps(run):
    source = Path(__file__).with_name('no_tool_trap.rs')
    binary = run / ('no-tool-trap.exe' if os.name == 'nt' else 'no-tool-trap')
    env = {**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'}
    for name in ('UNIT4_LIFECYCLE_LOG', 'UNIT4_RACE_OUTPUT'):
        env.pop(name, None)
    version = process(['rustc', '-Vv'], run, env, timeout=30)
    need(version['status'] == 0 and not version['timed_out'], 'Rust required to build source-bound no-tool traps')
    build = process(['rustc', '--edition=2021', '-C', 'opt-level=0', str(source), '-o', str(binary)], run, env, timeout=120)
    need(build['status'] == 0 and not build['timed_out'] and not build['stream_limit_exceeded'], 'no-tool trap build failed')
    wrappers = run / 'no-tool-traps'
    wrappers.mkdir()
    env['OXID_LLVM_BIN'] = str(wrappers)
    tools = {}
    for tool in ('clang', 'opt', 'ld.lld'):
        path = wrappers / tool
        shutil.copy2(binary, path)
        if os.name == 'nt':
            shutil.copy2(binary, wrappers / (tool + '.exe'))
        log = run / (tool + '-trap-control.jsonl')
        p = process([str(path), '--version'], run, {**env, 'UNIT4_TOOL_LOG': str(log)}, timeout=10)
        need(p['status'] == 121 and p['stdout'] == '' and p['stderr'] == 'unexpected native tool invocation\n', 'no-tool trap direct control')
        need(read_events(log) == [{'unexpected_native_tool': True}], 'no-tool trap did not capture invocation')
        tools[tool] = {'mode': 'no-native-trap', 'executable': binding(path), 'wrapper': binding(path), 'control': p, 'control_log': binding(log)}
        if os.name == 'nt':
            tools[tool]['windows_alias'] = binding(wrappers / (tool + '.exe'))
    save(run / 'no-tool-trap-build.json', {'source': binding(source), 'binary': binding(binary), 'rustc': version, 'build': build,
                                        'purpose': 'Trap and reject any native tool call; no LLVM tool or version is invoked.'})
    return env, tools
