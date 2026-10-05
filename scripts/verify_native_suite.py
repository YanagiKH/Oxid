#!/usr/bin/env python3
"""Run all seven native oracles in one or two isolated processes.

Linux/POSIX only, like the real LLVM gate. Default to serial locally. Each child
keeps its existing command/stack limits, private scratch, corpus and profile
checks. The boolean oracle selects the explicit checked-unary successor.
Never import the oracle modules here: they use preexec_fn internally.
"""
import argparse
from dataclasses import dataclass
import hashlib
import math
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time


@dataclass(frozen=True)
class Job:
    name: str
    command: tuple[str, ...]
    pass_prefix: bytes


def build_jobs(scripts, debug, release):
    # Longest suites first; this is also the deterministic grouped-log order.
    suites = (
        ("scalar_comparisons", b"scalar comparisons O0", (debug, release)),
        ("native_arithmetic", b"native checked i32 O0", (debug, release)),
        ("boolean_logic", b"boolean logic O0",
         (debug, release, "--expectation-amendment", "checked-unary-negation-v1")),
        ("native_preview", b"native preview", (release,)),
        ("mutable_locals", b"mutable locals O0", (debug, release)),
        ("while_loops", b"while loops O0", (debug, release)),
        ("loop_control", b"loop control O0", (debug, release)),
    )
    return [Job(name, (sys.executable, str(scripts / f"verify_{name}.py"), *args), prefix)
            for name, prefix, args in suites]


def _signal_group(process, sig):
    try:
        os.killpg(process.pid, sig)
        return True
    except ProcessLookupError:
        return False


def _cleanup(processes):
    # A failed Python worker can leave a running compiler/linker/native child.
    # Signal its session group even if the worker itself has already exited.
    live = [process for process in processes if _signal_group(process, signal.SIGTERM)]
    deadline = time.monotonic() + 1
    while live and time.monotonic() < deadline:
        for process in live:
            process.poll()
        live = [process for process in live if _signal_group(process, 0)]
        if live:
            time.sleep(.02)
    for process in live:
        _signal_group(process, signal.SIGKILL)
    for process in processes:
        process.wait()


def _has_summary(log, prefix):
    # Read only a bounded tail, regardless of how much a failed tool printed.
    with log.open("rb") as stream:
        stream.seek(0, os.SEEK_END)
        stream.seek(max(0, stream.tell() - 8192))
        tail = stream.read()
    lines = tail.splitlines()
    return (tail.endswith(b"\n") and bool(lines)
            and lines[-1].startswith(prefix + b":") and lines[-1].endswith(b": PASS"))


def run_suite(suites, *, jobs=1, timeout=1800, output=None):
    """Return the first observed failure (signals use 128+signal), or zero.

    Exit 124 is a whole-worker timeout; existing per-command timeouts are
    unchanged. Exit 1 covers launch, missing-summary and orchestration failures.
    Pending work is explicitly NOT RUN on failure; it can never count as a pass.
    """
    if os.name != "posix":
        raise ValueError("native suite requires POSIX process groups")
    if jobs not in (1, 2) or not math.isfinite(timeout) or timeout <= 0:
        raise ValueError("jobs must be 1 or 2 and timeout must be positive and finite")
    if not suites or len({job.name for job in suites}) != len(suites):
        raise ValueError("suite must contain uniquely named jobs")
    output = sys.stdout.buffer if output is None else output
    interrupted = [0]

    def on_signal(signum, _frame):
        # Do not raise between Popen creating a process and recording its handle.
        # Workers have separate sessions and receive cancellation in _cleanup.
        if not interrupted[0]:
            interrupted[0] = signum

    processes, active, statuses = [], {}, {}
    failure, next_job = 0, 0
    with tempfile.TemporaryDirectory(prefix="oxid-native-suite-") as directory:
        previous = {sig: signal.signal(sig, on_signal) for sig in (signal.SIGINT, signal.SIGTERM)}
        logs = [Path(directory) / f"{index}.log" for index in range(len(suites))]
        try:
            while next_job < len(suites) or active:
                if interrupted[0]:
                    failure = 128 + interrupted[0]
                    break
                # Observe ALL active workers before admitting any replacement.
                for index, (process, started) in list(active.items()):
                    code = process.poll()
                    if code is None and time.monotonic() - started >= timeout:
                        statuses[index] = "TIMEOUT"
                        failure = 124
                        break
                    if code is not None:
                        del active[index]
                        if code:
                            statuses[index] = f"FAIL (exit {code})"
                            failure = code if code > 0 else 128 - code
                        elif not _has_summary(logs[index], suites[index].pass_prefix):
                            statuses[index] = "FAIL (missing final PASS summary)"
                            failure = 1
                        elif _signal_group(process, 0):
                            statuses[index] = "FAIL (worker left running descendants)"
                            failure = 1
                        else:
                            statuses[index] = "PASS"
                            # Reaped and no group remains: never signal this historical PID again.
                            processes.remove(process)
                        if failure:
                            break
                if failure:
                    break
                while next_job < len(suites) and len(active) < jobs and not interrupted[0]:
                    index = next_job
                    next_job += 1
                    try:
                        # Files avoid pipe backpressure and unbounded in-memory logs.
                        with logs[index].open("wb") as log:
                            process = subprocess.Popen(suites[index].command, stdout=log,
                                                       stderr=subprocess.STDOUT, start_new_session=True)
                    except OSError as error:
                        statuses[index] = f"FAIL (could not start: {error})"
                        failure = 1
                        break
                    processes.append(process)
                    active[index] = process, time.monotonic()
                if failure:
                    break
                if active:
                    time.sleep(.02)
        except KeyboardInterrupt:
            interrupted[0] = signal.SIGINT
            failure = 130
        except (OSError, ValueError) as error:
            failure = 1
            output.write(f"native suite orchestration failed: {error}\n".encode())
        finally:
            try:
                _cleanup(processes)
                if interrupted[0]:
                    failure = failure or 128 + interrupted[0]
                    output.write(f"native suite interrupted by signal {interrupted[0]}\n".encode())
                for index, job in enumerate(suites):
                    output.write(f"=== {job.name} ===\n".encode())
                    if logs[index].exists():
                        with logs[index].open("rb") as log:
                            shutil.copyfileobj(log, output, length=65536)
                    status = statuses.get(index, "CANCELLED" if index < next_job else "NOT RUN")
                    output.write(f"\n{job.name}: {status}\n".encode())
                passed = sum(status == "PASS" for status in statuses.values())
                failure = failure or (0 if passed == len(suites) else 1)
                output.write(f"native suite: {passed}/{len(suites)} passed; jobs={jobs}; exit={failure}\n".encode())
                output.flush()
            finally:
                for sig, handler in previous.items():
                    signal.signal(sig, handler)
    return failure


def compiler_hashes(binaries):
    hashes = []
    for binary in binaries:
        with Path(binary).open("rb") as stream:
            hashes.append(hashlib.file_digest(stream, "sha256").hexdigest())
    return hashes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("debug_binary")
    parser.add_argument("release_binary")
    parser.add_argument("--jobs", type=int, choices=(1, 2), default=1)
    parser.add_argument("--timeout", type=float, default=1800,
                        help="whole-worker backstop in seconds (default: 1800)")
    args = parser.parse_args()
    if sys.flags.optimize:
        parser.error("Python assertions must be enabled for the native oracles")
    binaries = [str(Path(binary).resolve()) for binary in (args.debug_binary, args.release_binary)]
    suites = build_jobs(Path(__file__).resolve().parent, *binaries)
    try:
        for path in binaries:
            if not Path(path).is_file() or not os.access(path, os.X_OK):
                raise ValueError(f"compiler is missing or not executable: {path}")
        for job in suites:
            if not Path(job.command[1]).is_file():
                raise ValueError(f"oracle script is missing: {job.command[1]}")
        before = compiler_hashes(binaries)
        code = run_suite(suites, jobs=args.jobs, timeout=args.timeout)
        after = compiler_hashes(binaries)
        if before != after:
            print("native suite: FAIL (compiler binaries changed)", file=sys.stderr)
            return code or 1
        print("native suite compiler SHA256: " + " ".join(after))
        return code
    except (OSError, ValueError) as error:
        print(f"native suite: FAIL ({error})", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
