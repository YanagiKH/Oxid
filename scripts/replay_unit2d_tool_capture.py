#!/usr/bin/env python3
"""Keep native compiler inputs/outputs around the frozen transparent logger.

Installed under each LLVM tool's name by replay_fixed_array_unit2d.py. This is
test tooling, never a production compiler boundary. It does not alter arguments.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def main():
    name = Path(sys.argv[0]).name
    root = Path(os.environ["OXID_UNIT2D_CAPTURE_ROOT"])
    run = root / (str(time.time_ns()) + "-" + str(os.getpid()) + "-" + name)
    run.mkdir(parents=True, exist_ok=False)
    trusted = json.loads(Path(os.environ["OXID_UNIT2D_TRUSTED_TOOLS"]).read_text())
    hashes = json.loads(Path(os.environ["OXID_UNIT2D_TOOL_HASHES"]).read_text())
    target = Path(trusted[name])
    if hashlib.sha256(target.read_bytes()).hexdigest() != hashes[name]:
        (run / "receipt.json").write_text(json.dumps(dict(
            tool=name, argv=sys.argv, trusted_target=str(target), cwd=os.getcwd(),
            exit=None, error="trusted tool content changed before invocation", snapshots=[]), indent=2) + "\n")
        raise RuntimeError("trusted tool changed: " + name)
    paths = set()
    temporary = Path(os.environ["TMPDIR"]).resolve()
    for arg in sys.argv[1:]:
        if not arg.startswith("-"):
            path = Path(arg).absolute()
            if path.suffix in (".ll", ".bc", ".c", ".o") and path.resolve().is_relative_to(temporary):
                paths.add(path)
    cwd = Path.cwd()
    # The native workspace is removed by the existing Rust harness. Preserve its
    # complete compiler material before the frozen logger returns to that harness.
    # Retain only explicitly named compiler-workspace inputs/outputs. In
    # particular do not copy linker system libraries, executables or config files.
    snapshots = []

    def snapshot(when):
        for path in sorted(paths):
            if path.is_file() and not path.is_symlink():
                data = path.read_bytes()
                digest = hashlib.sha256(data).hexdigest()
                stored = root / "blobs" / digest
                stored.parent.mkdir(exist_ok=True)
                try:
                    with stored.open("xb") as stream:
                        stream.write(data)
                    stored.chmod(path.stat().st_mode & 0o777)
                except FileExistsError:
                    if hashlib.sha256(stored.read_bytes()).hexdigest() != digest:
                        raise RuntimeError("capture blob changed: " + str(stored))
                snapshots.append(dict(when=when, path=str(path), sha256=digest,
                                      bytes=len(data), blob=str(stored)))

    result = None
    error = None
    start = time.monotonic()
    try:
        snapshot("before")
        logger = Path(os.environ["OXID_UNIT2D_FROZEN_LOGGERS"]) / name
        result = subprocess.run([sys.executable, str(logger), *sys.argv[1:]],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        (run / "stdout").write_bytes(result.stdout)
        (run / "stderr").write_bytes(result.stderr)
        snapshot("after")
    except BaseException as exc:
        error = repr(exc)
        raise
    finally:
        (run / "receipt.json").write_text(json.dumps(dict(
            tool=name, argv=sys.argv, trusted_target=str(target), cwd=str(cwd),
            exit=None if result is None else result.returncode,
            seconds=time.monotonic()-start, error=error, snapshots=snapshots), indent=2) + "\n")
    sys.stdout.buffer.write(result.stdout)
    sys.stderr.buffer.write(result.stderr)
    # Preserve signal termination rather than turning it into a modulo exit code.
    if result.returncode < 0:
        import signal
        signal.signal(-result.returncode, signal.SIG_DFL)
        os.kill(os.getpid(), -result.returncode)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
