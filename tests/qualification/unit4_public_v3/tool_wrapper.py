#!/usr/bin/env python3
"""Bounded real-tool observer; optional explicitly marked post-link race."""
import sys
sys.dont_write_bytecode = True
import importlib.util
import json
import os
import pathlib
import shutil
import sys
import time
# Only this versioned implementation is loaded, never a historical mutable helper.
root = pathlib.Path(os.environ['UNIT4_ADAPTER_ROOT'])
sys.path.insert(0, str(root))
from runtime import process, file_identity
from contracts import sha, need
name = pathlib.Path(sys.argv[0]).name
real = pathlib.Path(os.environ['UNIT4_REAL_LLVM']) / name
argv = sys.argv[1:]
evidence = pathlib.Path(os.environ['UNIT4_TOOL_EVIDENCE'])
if name == 'opt' and argv == ['-passes=verify', '-disable-output', 'program.ll']:
    data = pathlib.Path('program.ll').read_bytes()
    need(len(data) <= 8 * 1024 * 1024, 'IR evidence byte limit')
    (evidence / 'program.ll').write_bytes(data)
r = process([str(real)] + argv, pathlib.Path.cwd(), os.environ, limit=2 * 1024 * 1024, new_session=False)
r.update(tool=name, real_path=str(real.resolve()), real_sha256=sha(real.read_bytes()), argv=argv,
         wrapper_pid=os.getpid(), wrapper_ppid=os.getppid(), real_tool_pid=r['pid'])
phase = ['--no-default-config', '--target=x86_64-unknown-linux-gnu', '-pie', '-Wl,-z,noexecstack', '-Wl,-z,relro', '-Wl,-z,now', 'program.o', 'runtime.o', '-o', 'executable', '--ld-path=' + str(pathlib.Path(sys.argv[0]).parent / 'ld.lld')]
if os.environ.get('UNIT4_RACE_OUTPUT') and name == 'clang' and argv == phase and r['status'] == 0:
    linked = pathlib.Path('executable')
    shutil.copy2(linked, evidence / 'linked-before-publication.elf')
    verified = time.time_ns()
    output = pathlib.Path(os.environ['UNIT4_RACE_OUTPUT'])
    need(not os.path.lexists(output), 'race output already occupied')
    absent = time.time_ns()
    payload = b'winner'
    descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    created = time.time_ns()
    try:
        written = os.write(descriptor, payload)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    closed = time.time_ns()
    need(written == len(payload) and output.read_bytes() == payload, 'race payload write')
    r['race'] = {'output': str(output), 'created_exclusively': True, 'bytes': written, 'sha256': sha(payload),
                 'linked_elf_sha256': sha(linked.read_bytes()), 'linked_verified_ns': verified,
                 'absent_verified_ns': absent, 'created_ns': created, 'closed_ns': closed,
                 'readback_verified_ns': time.time_ns(), 'file_identity': file_identity(output)}
r['wrapper_return_ready_ns'] = time.time_ns()
record = (json.dumps(r, sort_keys=True) + '\n').encode()
log = pathlib.Path(os.environ['UNIT4_TOOL_LOG'])
need(not log.exists() or log.stat().st_size + len(record) <= 2 * 1024 * 1024, 'tool log byte limit')
fd = os.open(log, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
try:
    need(os.write(fd, record) == len(record), 'tool log short write')
finally:
    os.close(fd)
sys.stdout.buffer.write(r['stdout'].encode())
sys.stderr.buffer.write(r['stderr'].encode())
sys.exit(r['status'] if not r['timed_out'] and not r['stream_limit_exceeded'] else 125)
