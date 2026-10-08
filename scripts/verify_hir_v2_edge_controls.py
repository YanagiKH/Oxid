#!/usr/bin/env python3
"""Narrow Linux runtime controls: real EOF-probe I/O fault and synthetic span255."""
import argparse
import array
import fcntl
import json
import os
from pathlib import Path
import subprocess
import termios
import time
import tty

import build_hir_producers_v2 as builder


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def verify(compiler, llvm, bundle, output):
    output = output.resolve()
    output.mkdir()
    parser = (bundle / 'parser').resolve(strict=True)
    master, slave = os.openpty()
    tty.setraw(slave)
    process = subprocess.Popen([str(parser)], stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        source = b'fn main()->i32{return 7;}'
        source = b' ' * (255-len(source)) + source
        require(len(source) == 255, 'control source length changed')
        require(os.write(master, source) == 255, 'incomplete PTY write')
        deadline = time.monotonic() + 5
        observed = None
        while time.monotonic() < deadline:
            require(process.poll() is None, 'parser exited before EOF-probe control')
            unread = array.array('i', [0])
            fcntl.ioctl(slave, termios.FIONREAD, unread, True)
            syscall = Path(f'/proc/{process.pid}/syscall').read_text().split()
            # All 255 bytes left the tty input queue, and the process is in a
            # read(0, ..., 1). Closing the master now gives the actual next read
            # EIO rather than EOF. No replacement parser/runtime is used.
            if unread[0] == 0 and len(syscall) >= 4 and syscall[0] == '0' and int(syscall[1], 16) == 0 and int(syscall[3], 16) == 1:
                observed = dict(unread_bytes=unread[0], syscall='read', fd=0, requested_bytes=1)
                break
            time.sleep(0.005)
        require(observed is not None, 'did not establish actual one-byte EOF probe')
        os.close(master)
        master = None
        stdout, stderr = process.communicate(timeout=5)
        (output / 'probe-error.stdout').write_bytes(stdout)
        (output / 'probe-error.stderr').write_bytes(stderr)
        require(process.returncode == 74 and not stdout and not stderr, 'probe I/O error was not a clean74refusal')
        observed.update(status=process.returncode, parser_sha256=builder.digest(parser.read_bytes()))
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        if master is not None:
            os.close(master)
        os.close(slave)
    # This intentionally synthetic caller exercises a secondary span ending255.
    # It is not presented as a diagnostic reachable from accepted source grammar.
    sources = output / 'sources'
    builder.materialize(sources)
    text = '''mod buffers; mod static_column; mod typed_output; mod typed_diagnostic; mod static_state; mod parser_state;
fn main()->i32 {
    let mut state=crate::static_state::State{stack:0,locals:0,value:0};
    let failed=crate::typed_diagnostic::fail(&mut state,3,32832,4194176,1,0,0);
    if failed || state.stack < 0 { return 70; }
    let resolved=crate::buffers::zeros();
    let semantic=crate::buffers::zeros();
    return crate::typed_output::emit(&resolved,&semantic,&state,0,255);
}
'''
    root = sources / 'secondary_boundary.ox'
    root.write_text(text)
    executable = output / 'secondary-boundary'
    env = dict(os.environ, OXID_LLVM_BIN=str(llvm.resolve(strict=True)))
    argv = [str(compiler.resolve(strict=True)), 'compile', str(root), '--edition=typed-preview',
            '--backend=llvm', '--entry-mode=process', '--output', str(executable)]
    compiled = subprocess.run(argv, capture_output=True, env=env, timeout=120)
    (output / 'secondary-build.stdout').write_bytes(compiled.stdout)
    (output / 'secondary-build.stderr').write_bytes(compiled.stderr)
    require(compiled.returncode == 0, 'synthetic diagnostic control build failed')
    result = subprocess.run([str(executable)], capture_output=True, timeout=5)
    (output / 'secondary.stdout').write_bytes(result.stdout)
    (output / 'secondary.stderr').write_bytes(result.stderr)
    require(result.returncode == 0 and not result.stderr and len(result.stdout) == 16, 'synthetic diagnostic output failed')
    require(result.stdout[:16] == b'STF2'+bytes([1,0,3,1,2,254,255,1,0,0,0,0]), 'secondary endpoint255 did not roundtrip')
    report = dict(status='passed', actual_parser_probe_io_error=observed,
                  synthetic_secondary_span=dict(start=254,end=255,executable_sha256=builder.digest(executable.read_bytes()),
                                                source_sha256=builder.digest(text.encode()),argv=argv),
                  limits='Synthetic diagnostic caller is a packing control, not source grammar coverage.')
    (output / 'summary.json').write_text(json.dumps(report, indent=2)+'\n')
    print('Actual parser EOF-probe EIO and synthetic secondary endpoint255: PASS')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['compiler','llvm-bin','bundle','output']:
        parser.add_argument('--'+name, required=True, type=Path)
    args=parser.parse_args()
    verify(args.compiler,args.llvm_bin,args.bundle,args.output)


if __name__=='__main__':
    main()
