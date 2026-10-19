#!/usr/bin/env python3
"""Linux binary CLI acceptance driver (Python stdlib only).

Runs exactly one given executable argv (no shell), feeds it controlled,
segmented stdin and then closes stdin, drains stdout/stderr concurrently
(no pipe deadlock), enforces a hard wall-clock timeout with process-group
cleanup, and records raw streams, exit status and the exact command.

It does NOT know about any repo/worktree and does not judge semantics; it
only records. Optional --expect-* checks are convenience comparisons.

NOTE: stdin segmentation is writer-side only. The driver issues one
os.write() call sequence per segment (with optional delay between
segments) and records each call's return value. The kernel/pipe may merge
or split data arbitrarily on the reader side, so this is NOT a
deterministic short-read generator.

v1.2 (on top of root's 1.1 hardening):
- every post-spawn exception / KeyboardInterrupt goes through one
  try/finally cleanup (stop writer, TERM/KILL process group, bounded reap,
  join threads, close fds never handed to a thread) and still writes
  result.json with driver_exception / driver_interrupted;
- --expect-* files and stdin are read (bounded) BEFORE spawn;
- stdin, per-stream capture and write-call log are memory bounded; going
  over a limit is reported explicitly and never counted as a clean record;
- per stream, "EOF actually read" and "reader thread ended" are separate;
- result.json has a "verdict" block (record_ok + reject_reasons) so an
  upper layer can reject timeout / capture error / incomplete stdin /
  cleanup failure even when the driver exit code is 0.
"""
import argparse, hashlib, json, math, os, signal, subprocess, sys, threading, time

VERSION = "1.2.1-root"

MiB = 1024 * 1024
DEFAULT_MAX_STDIN = 64 * MiB
DEFAULT_MAX_CAPTURE = 64 * MiB
DEFAULT_MAX_WRITE_LOG = 100000

# driver exit codes
RC_OK, RC_EXPECT, RC_USAGE, RC_SPAWN, RC_CLEANUP, RC_LIMIT, RC_EXCEPTION, RC_INTERRUPT = 0, 1, 2, 3, 4, 5, 6, 130


def sha256(b):
    return hashlib.sha256(b).hexdigest()


def parse_segments(spec, total, max_count):
    """spec: comma list of byte sizes, e.g. '1,7,4096'; last size repeats.
    Raises ValueError if the plan would need more than max_count segments."""
    sizes = [int(x) for x in spec.split(",") if x.strip()]
    if not sizes or any(s <= 0 for s in sizes):
        raise ValueError("segment sizes must be positive integers")
    # count first, without materialising the plan
    pos, i, count = 0, 0, 0
    while pos < total and i < len(sizes):
        pos += sizes[i]; i += 1; count += 1
    if pos < total:
        count += -(-(total - pos) // sizes[-1])
    if count > max_count:
        raise ValueError("segment plan needs %d write segments > --max-write-log %d" % (count, max_count))
    out, pos, i = [], 0, 0
    while pos < total:
        n = sizes[min(i, len(sizes) - 1)]
        out.append((pos, min(n, total - pos)))
        pos += n; i += 1
    return out


def group_alive(pgid):
    try:
        os.killpg(pgid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def read_bounded(ap, path, limit, what):
    try:
        with open(path, "rb") as f:
            b = f.read(limit + 1)
    except OSError as e:
        ap.error("cannot read %s %r: %r" % (what, path, e))
    if len(b) > limit:
        ap.error("%s %r is larger than the %d-byte limit (refusing; nothing truncated, nothing spawned)"
                 % (what, path, limit))
    return b


def main(argv=None):
    ap = argparse.ArgumentParser(
        description="Binary CLI acceptance driver. Usage: cli_driver.py [opts] --out DIR -- EXE [ARGS...]")
    src = ap.add_mutually_exclusive_group()
    src.add_argument("--stdin-file", help="binary file to feed as stdin (read as bytes)")
    src.add_argument("--stdin-hex", help="stdin bytes as hex string, e.g. 00 80 ff")
    src.add_argument("--no-stdin", action="store_true", help="give child /dev/null-like closed stdin (empty input, still a pipe closed immediately)")
    ap.add_argument("--segments", default="65536", help="writer segment sizes, comma list, last repeats (default 65536)")
    ap.add_argument("--segment-delay", type=float, default=0.0, help="seconds to sleep between segments")
    ap.add_argument("--timeout", type=float, required=True, help="hard wall-clock timeout in seconds")
    ap.add_argument("--kill-grace", type=float, default=1.0, help="seconds between SIGTERM and SIGKILL to the process group")
    ap.add_argument("--drain-timeout", type=float, default=2.0, help="max seconds to wait for stream EOF after child exit before killing the group")
    ap.add_argument("--cwd", default=None, help="working directory for the child")
    ap.add_argument("--env-clear", action="store_true", help="start child with empty env plus --env entries")
    ap.add_argument("--env", action="append", default=[], help="KEY=VALUE for child env (repeatable)")
    ap.add_argument("--out", required=True, help="output dir for raw artifacts (created exclusively; must be absent)")
    ap.add_argument("--expect-exit", type=int, default=None)
    ap.add_argument("--expect-stdout-file", default=None)
    ap.add_argument("--expect-stderr-file", default=None)
    ap.add_argument("--max-stdin-bytes", type=int, default=DEFAULT_MAX_STDIN,
                    help="refuse (exit 2, before spawn) if stdin input is larger (default %d)" % DEFAULT_MAX_STDIN)
    ap.add_argument("--max-capture-bytes", type=int, default=DEFAULT_MAX_CAPTURE,
                    help="per-stream capture limit for stdout/stderr; exceeding it kills the group and exits 5 (default %d)" % DEFAULT_MAX_CAPTURE)
    ap.add_argument("--max-write-log", type=int, default=DEFAULT_MAX_WRITE_LOG,
                    help="max recorded stdin write() calls; plans needing more segments are refused before spawn, runtime overflow exits 5 (default %d)" % DEFAULT_MAX_WRITE_LOG)
    ap.add_argument("cmd", nargs=argparse.REMAINDER, help="-- EXE [ARGS...]")
    a = ap.parse_args(argv)
    cmd = a.cmd[1:] if a.cmd and a.cmd[0] == "--" else a.cmd
    if not cmd:
        ap.error("missing executable argv after --")
    for name in ("timeout", "kill_grace", "drain_timeout", "segment_delay"):
        value = getattr(a, name)
        if not math.isfinite(value) or value < 0 or (name == "timeout" and value == 0):
            ap.error("time values must be finite and nonnegative; timeout must be positive")
    for kv in a.env:
        if "=" not in kv or not kv.partition("=")[0] or "\x00" in kv:
            ap.error("--env requires a nonempty KEY=VALUE without NUL")
    for name in ("max_stdin_bytes", "max_capture_bytes", "max_write_log"):
        if getattr(a, name) <= 0:
            ap.error("--%s must be a positive integer" % name.replace("_", "-"))

    # ---- everything that can fail cheaply happens BEFORE spawn ----
    if a.stdin_file:
        data = read_bounded(ap, a.stdin_file, a.max_stdin_bytes, "--stdin-file")
    elif a.stdin_hex is not None:
        try:
            data = bytes.fromhex(a.stdin_hex)
        except ValueError as e:
            ap.error("--stdin-hex: %s" % e)
        if len(data) > a.max_stdin_bytes:
            ap.error("--stdin-hex decodes to %d bytes > --max-stdin-bytes %d" % (len(data), a.max_stdin_bytes))
    else:
        data = b""
    try:
        segs = parse_segments(a.segments, len(data), a.max_write_log)
    except ValueError as e:
        ap.error("--segments: %s" % e)
    expected = {}
    for key, path in (("stdout", a.expect_stdout_file), ("stderr", a.expect_stderr_file)):
        if path:
            expected[key] = read_bounded(ap, path, a.max_capture_bytes, "--expect-%s-file" % key)

    try:
        os.makedirs(a.out, exist_ok=False)
    except FileExistsError:
        ap.error("--out must be a new directory")

    env = {} if a.env_clear else dict(os.environ)
    for kv in a.env:
        k, _, v = kv.partition("=")
        env[k] = v

    # Pipes created manually so we control raw fds (unbuffered os.write/os.read).
    in_r, in_w = os.pipe()
    out_r, out_w = os.pipe()
    err_r, err_w = os.pipe()
    t0 = time.monotonic()
    wall_start = time.time()
    # Finish all fallible state/thread construction before creating a child.
    try:
        def ev(msg):
            events.append({"t": round(time.monotonic() - t0, 6), "event": msg})

        # ---- shared state ----
        events = []
        parent_fds = {"in_r": in_r, "out_w": out_w, "err_w": err_w}   # child-side ends still open in parent
        thread_fds = {0: out_r, 1: err_r, 2: in_w}                      # owned by thread i once started
        started = [False, False, False]
        limit_hit = threading.Event()
        stop_writer = threading.Event()
        cap = {n: {"buf": bytearray(), "bytes_read_total": 0, "read_calls": 0, "eof_seen": False,
                   "read_error": None, "overflow": False} for n in ("stdout", "stderr")}
        writes = []
        wstate = {"written": 0, "error": None, "closed": False, "log_dropped": 0}
        timed_out = False
        limit_killed = False
        driver_exception = None
        driver_interrupted = False
        cleanup_errors = []
        leftover = None
        streams_eof = None
        child_end = None

        def reader(fd, name):
            st = cap[name]
            try:
                while True:
                    b = os.read(fd, 65536)
                    if not b:
                        st["eof_seen"] = True
                        break
                    st["read_calls"] += 1
                    st["bytes_read_total"] += len(b)
                    room = a.max_capture_bytes - len(st["buf"])
                    if len(b) > room:
                        if room > 0:
                            st["buf"] += b[:room]
                        if not st["overflow"]:
                            st["overflow"] = True
                            limit_hit.set()
                        # keep draining (discarding) so the child is never blocked on a full pipe
                    else:
                        st["buf"] += b
            except BaseException as error:
                st["read_error"] = repr(error)
            finally:
                try: os.close(fd)
                except OSError: pass

        def writer():
            try:
                for i, (off, n) in enumerate(segs):
                    if stop_writer.is_set():
                        break
                    if i and a.segment_delay > 0 and stop_writer.wait(a.segment_delay):
                        break
                    view = memoryview(data)[off:off + n]
                    while len(view):
                        if stop_writer.is_set():
                            break
                        k = os.write(in_w, view)
                        if k == 0:
                            raise OSError("stdin write made no progress")
                        if len(writes) < a.max_write_log:
                            writes.append({"segment": i, "requested": len(view), "returned": k,
                                           "t": round(time.monotonic() - t0, 6)})
                        else:
                            wstate["log_dropped"] += 1
                        wstate["written"] += k
                        view = view[k:]
            except BrokenPipeError as e:
                wstate["error"] = "EPIPE: " + str(e)
            except BaseException as e:
                wstate["error"] = repr(e)
            finally:
                try:
                    os.close(in_w)
                    wstate["closed"] = True
                except OSError:
                    pass

        def kill_group(why):
            """TERM, bounded wait, KILL, bounded reap. Safe to call repeatedly."""
            for sig, grace in ((signal.SIGTERM, a.kill_grace), (signal.SIGKILL, max(0.1, a.kill_grace))):
                if proc.poll() is not None and not group_alive(pgid):
                    return
                try:
                    os.killpg(pgid, sig)
                    ev("%s: %s to pgid %d" % (why, sig.name, pgid))
                except ProcessLookupError:
                    pass
                end = time.monotonic() + grace
                while time.monotonic() < end:
                    if proc.poll() is not None and not group_alive(pgid):
                        return
                    time.sleep(0.02)
            if proc.poll() is None:
                ev("direct child not reaped within bounded kill wait")

        th = [threading.Thread(target=reader, args=(out_r, "stdout"), daemon=True),
              threading.Thread(target=reader, args=(err_r, "stderr"), daemon=True),
              threading.Thread(target=writer, daemon=True)]
    except BaseException:
        for fd in (in_r, in_w, out_r, out_w, err_r, err_w):
            try: os.close(fd)
            except OSError: pass
        raise

    try:
        proc = subprocess.Popen(cmd, stdin=in_r, stdout=out_w, stderr=err_w,
                                cwd=a.cwd, env=env, start_new_session=True,
                                close_fds=True)
    except BaseException as e:  # OSError, ValueError (NUL in argv), KeyboardInterrupt ...
        for fd in (in_r, in_w, out_r, out_w, err_r, err_w):
            try: os.close(fd)
            except OSError: pass
        res = {"driver_version": VERSION, "argv": cmd, "spawn_error": repr(e),
               "verdict": {"record_ok": False, "reject_reasons": ["spawn_failed"]}}
        with open(os.path.join(a.out, "result.json"), "x") as f:
            json.dump(res, f, indent=2)
        print(json.dumps(res, indent=2))
        return RC_INTERRUPT if isinstance(e, KeyboardInterrupt) else RC_SPAWN
    try:
        pgid = proc.pid  # start_new_session makes the child its group leader
        for fdname in ("in_r", "out_w", "err_w"):
            os.close(parent_fds.pop(fdname))
        for i, t in enumerate(th):
            t.start()
            started[i] = True

        deadline = t0 + a.timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                timed_out = True
                break
            try:
                proc.wait(timeout=min(0.05, remaining))
                break
            except subprocess.TimeoutExpired:
                if limit_hit.is_set():
                    limit_killed = True
                    break
        if timed_out:
            kill_group("timeout")
        elif limit_killed:
            kill_group("capture limit exceeded")
        stop_writer.set()
        child_end = time.monotonic()

        # After the direct child is gone, leftover group members may hold pipes.
        leftover = group_alive(pgid)
        if leftover:
            ev("process group still alive after child exit")
        drain_deadline = time.monotonic() + a.drain_timeout
        for t in th[:2]:
            t.join(max(0.0, drain_deadline - time.monotonic()))
        streams_eof = not any(t.is_alive() for t in th[:2])
        if leftover or not streams_eof:
            kill_group("cleanup")
            for t in th[:2]:
                t.join(2.0)
        th[2].join(2.0)
    except BaseException as e:
        driver_exception = repr(e)
        driver_interrupted = isinstance(e, KeyboardInterrupt)
        try: ev("driver exception after spawn: %r" % (e,))
        except BaseException: pass
    finally:
        # Runs on every path; idempotent when the normal path already cleaned up.
        try:
            stop_writer.set()
            for fdname in list(parent_fds):
                try: os.close(parent_fds.pop(fdname))
                except OSError: pass
            for i in range(3):
                if not started[i] and th[i].ident is None:   # never handed to a running thread
                    try: os.close(thread_fds[i])
                    except OSError: pass
            if proc.poll() is None or group_alive(pgid):
                kill_group("final cleanup")
            for i, t in enumerate(th):
                if started[i]:
                    t.join(2.0)
        except BaseException as e2:
            cleanup_errors.append(repr(e2))
            if isinstance(e2, KeyboardInterrupt):
                driver_interrupted = True
    if child_end is None:
        child_end = time.monotonic()

    group_alive_final = group_alive(pgid)
    readers_ended = {n: (started[i] and not th[i].is_alive()) for i, n in enumerate(("stdout", "stderr"))}
    streams_eof_final = all(readers_ended.values())          # kept for 1.0/1.1 compatibility (= reader threads ended)
    writer_finished = started[2] and not th[2].is_alive()
    rc = proc.returncode
    out_b, err_b = bytes(cap["stdout"]["buf"]), bytes(cap["stderr"]["buf"])
    write_log_overflow = wstate["log_dropped"] > 0

    def stream_rec(n, b):
        st = cap[n]
        return {"bytes": len(b), "sha256": sha256(b), "read_calls": st["read_calls"],
                "bytes_read_total": st["bytes_read_total"],
                "capture_limit_exceeded": st["overflow"],
                "truncated": st["bytes_read_total"] > len(b),
                "eof_seen": st["eof_seen"],
                "reader_thread_ended": readers_ended[n],
                "read_error": st["read_error"]}

    checks = {}
    if a.expect_exit is not None:
        checks["exit"] = (rc is not None and rc >= 0 and rc == a.expect_exit and not timed_out)
    for key, b in (("stdout", out_b), ("stderr", err_b)):
        if key in expected:
            checks[key] = (not cap[key]["overflow"]) and expected[key] == b

    # ---- verdict ----
    cleanup_fail = []
    if group_alive_final: cleanup_fail.append("cleanup_failure:process_group_alive")
    if rc is None: cleanup_fail.append("cleanup_failure:child_not_reaped")
    for n in ("stdout", "stderr"):
        if not readers_ended[n]: cleanup_fail.append("cleanup_failure:reader_not_ended:" + n)
        if cap[n]["read_error"]: cleanup_fail.append("capture_error:read_error:" + n)
    if not writer_finished: cleanup_fail.append("cleanup_failure:writer_not_finished")
    if cleanup_errors: cleanup_fail.append("cleanup_failure:exception_in_cleanup")
    limit_fail = ["capture_error:limit_exceeded:" + n for n in ("stdout", "stderr") if cap[n]["overflow"]]
    if write_log_overflow: limit_fail.append("record_error:write_log_limit_exceeded")
    reasons = []
    if driver_exception: reasons.append("driver_interrupted" if driver_interrupted else "driver_exception")
    reasons += cleanup_fail + limit_fail
    if timed_out: reasons.append("timeout")
    for n in ("stdout", "stderr"):
        if readers_ended[n] and not cap[n]["eof_seen"]:
            reasons.append("capture_error:eof_not_seen:" + n)
    if wstate["written"] != len(data): reasons.append("stdin_incomplete")
    if not wstate["closed"]: reasons.append("stdin_not_closed")
    if wstate["error"] and not wstate["error"].startswith("EPIPE"): reasons.append("stdin_write_error")
    if checks and not all(checks.values()):
        reasons += ["expect_failed:" + k for k, v in checks.items() if not v]

    if driver_interrupted: drc = RC_INTERRUPT
    elif driver_exception: drc = RC_EXCEPTION
    elif cleanup_fail: drc = RC_CLEANUP
    elif limit_fail: drc = RC_LIMIT
    elif checks and not all(checks.values()): drc = RC_EXPECT
    else: drc = RC_OK

    res = {
        "driver_version": VERSION,
        "argv": cmd,
        "cwd": a.cwd or os.getcwd(),
        "env_clear": a.env_clear,
        "env_override_names": [kv.partition("=")[0] for kv in a.env],
        "started_unix": wall_start,
        "child_pid": proc.pid, "pgid": pgid,
        "timeout_s": a.timeout, "kill_grace_s": a.kill_grace, "drain_timeout_s": a.drain_timeout,
        "limits": {"max_stdin_bytes": a.max_stdin_bytes, "max_capture_bytes": a.max_capture_bytes,
                   "max_write_log": a.max_write_log},
        "timed_out": timed_out,
        "capture_limit_killed": limit_killed,
        "exit_code": rc if rc is not None and rc >= 0 else None,
        "term_signal": -rc if rc is not None and rc < 0 else None,
        "direct_child_reaped": rc is not None,
        "child_elapsed_s": round(child_end - t0, 6),
        "total_elapsed_s": round(time.monotonic() - t0, 6),
        "stdin": {"bytes": len(data), "sha256": sha256(data), "written": wstate["written"],
                  "complete": wstate["written"] == len(data), "write_error": wstate["error"],
                  "closed": wstate["closed"], "segments_planned": [n for _, n in segs],
                  "segment_delay_s": a.segment_delay,
                  "write_calls_logged": len(writes), "write_calls_dropped": wstate["log_dropped"],
                  "write_log_limit_exceeded": write_log_overflow, "write_calls": writes,
                  "note": "writer-side segmentation only; reader-side read sizes are not controlled"},
        "stdout": stream_rec("stdout", out_b),
        "stderr": stream_rec("stderr", err_b),
        "streams_eof_before_cleanup": streams_eof,
        "streams_eof_final": streams_eof_final,
        "stream_read_errors": {n: cap[n]["read_error"] for n in ("stdout", "stderr")},
        "writer_finished": writer_finished,
        "leftover_group_after_child_exit": leftover,
        "process_group_alive_final": group_alive_final,
        "driver_exception": driver_exception,
        "driver_interrupted": driver_interrupted,
        "cleanup_errors": cleanup_errors,
        "events": events,
        "checks": checks,
        "verdict": {"driver_exit": drc, "record_ok": drc == RC_OK, "reject_reasons": reasons,
                    "note": "driver_exit 0 only means the run was recorded completely; "
                            "reject_reasons lists everything an acceptance layer should reject on "
                            "(timeout, capture error, incomplete stdin, cleanup failure, ...)"},
    }
    for name, b in (("stdin.bin", data), ("stdout.bin", out_b), ("stderr.bin", err_b)):
        with open(os.path.join(a.out, name), "xb") as f:
            f.write(b)
    with open(os.path.join(a.out, "result.json"), "x") as f:
        json.dump(res, f, indent=2)
    summary = {k: res[k] for k in ("argv", "timed_out", "exit_code", "term_signal", "checks")}
    summary.update({"stdin_written": wstate["written"], "stdin_bytes": len(data),
                    "write_error": wstate["error"], "stdout_bytes": len(out_b),
                    "stderr_bytes": len(err_b), "process_group_alive_final": group_alive_final,
                    "driver_exit": drc, "reject_reasons": reasons})
    print(json.dumps(summary, ensure_ascii=True))
    return drc


if __name__ == "__main__":
    sys.exit(main())
