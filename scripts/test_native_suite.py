"""Exercise the dispatcher with real, isolated tiny subprocesses, never LLVM."""
import errno
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

from verify_native_suite import Job, build_jobs, run_suite
import verify_native_suite


WORKER = r'''
import fcntl, json, os, pathlib, signal, subprocess, sys, time
root, name, mode = pathlib.Path(sys.argv[1]), sys.argv[2], sys.argv[3]
with (root / "events").open("a") as f:
    fcntl.flock(f, fcntl.LOCK_EX)
    f.write(json.dumps(["start", name, os.getpid()]) + "\n")
    f.flush()
print(name + " stdout", flush=True)
print(name + " stderr", file=sys.stderr, flush=True)
if mode in ("wait", "orphan"):
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
    child = subprocess.Popen([sys.executable, "-c", "import pathlib,signal,sys,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); pathlib.Path(sys.argv[1]).touch(); time.sleep(60)", str(root / (name + ".ready"))])
    while not (root / (name + ".ready")).exists(): time.sleep(.01)
    (root / (name + ".pids")).write_text(str(os.getpid()) + " " + str(child.pid))
    if mode == "wait": time.sleep(60)
if mode == "fail":
    while not (root / "first.pids").exists(): time.sleep(.01)
    print(name + ": PASS")
    sys.exit(23)
if mode == "tool":
    subprocess.run([str(root / "missing-llvm-tool")], check=True)
if mode == "signal":
    os.kill(os.getpid(), signal.SIGTERM)
if mode == "empty":
    sys.exit(0)
if mode == "large":
    print("x" * 200000)
if mode == "overlap":
    while len((root / "events").read_text().splitlines()) < 2: time.sleep(.01)
    time.sleep(.15 if name == "first" else .03)
with (root / "events").open("a") as f:
    fcntl.flock(f, fcntl.LOCK_EX)
    f.write(json.dumps(["end", name, os.getpid()]) + "\n")
    f.flush()
print(name + ": PASS")
'''


@unittest.skipUnless(os.name == "posix", "native LLVM suite requires POSIX process groups")
class NativeSuiteTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="oxid-suite-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.worker = self.root / "worker.py"
        self.worker.write_text(WORKER)
        self.output = io.BytesIO()

    def job(self, name, mode="success"):
        return Job(name, (sys.executable, str(self.worker), str(self.root), name, mode), name.encode())

    def run_jobs(self, jobs, workers=2, timeout=5):
        return run_suite(jobs, jobs=workers, timeout=timeout, output=self.output)

    def assert_stopped(self, path):
        for pid in map(int, path.read_text().split()):
            # A reparented zombie has stopped; this test cannot reap another parent’s child.
            for _ in range(100):
                status = Path(f"/proc/{pid}/stat")
                try:
                    if status.read_text().split()[2] == "Z":
                        break
                except (FileNotFoundError, ProcessLookupError):
                    break
                time.sleep(.01)
            else:
                self.fail(f"worker descendant {pid} remains running")

    def test_stopped_probe_accepts_process_disappearing_during_stat_read(self):
        with mock.patch.object(Path, "read_text", side_effect=[
                "123456", ProcessLookupError(errno.ESRCH, "process disappeared")]):
            self.assert_stopped(self.root / "simulated.pids")

    def test_stopped_probe_propagates_unrelated_io_failures(self):
        for error in [PermissionError(errno.EACCES, "denied"), OSError(errno.EIO, "I/O failure")]:
            with self.subTest(error=type(error).__name__):
                with mock.patch.object(Path, "read_text", side_effect=["123456", error]):
                    with self.assertRaises(type(error)) as caught:
                        self.assert_stopped(self.root / "simulated.pids")
                self.assertIs(caught.exception, error)

    def test_exact_seven_commands_and_profile_arguments(self):
        jobs = build_jobs(Path("/repo/scripts"), "/debug", "/release")
        self.assertEqual(len(jobs), 7)
        scripts = {Path(job.command[1]).name: job.command[2:] for job in jobs}
        self.assertEqual(scripts, {
            "verify_native_preview.py": ("/release",),
            "verify_native_arithmetic.py": ("/debug", "/release"),
            "verify_scalar_comparisons.py": ("/debug", "/release"),
            "verify_boolean_logic.py": ("/debug", "/release", "--expectation-amendment", "checked-unary-negation-v1"),
            "verify_mutable_locals.py": ("/debug", "/release"),
            "verify_while_loops.py": ("/debug", "/release"),
            "verify_loop_control.py": ("/debug", "/release"),
        })
        self.assertTrue(all(job.command[0] == sys.executable for job in jobs))

    def test_all_jobs_run_once_bounded_to_two_with_ordered_complete_logs(self):
        jobs = [self.job("first", "overlap"), self.job("second", "overlap")]
        jobs += [self.job(str(i), "large") for i in range(5)]
        self.assertEqual(self.run_jobs(jobs), 0)
        events = [json.loads(line) for line in (self.root / "events").read_text().splitlines()]
        active, peak = set(), 0
        for action, name, _ in events:
            if action == "start":
                self.assertNotIn(name, active)
                active.add(name)
                peak = max(peak, len(active))
            else:
                active.remove(name)
        self.assertEqual(peak, 2)
        self.assertFalse(active)
        self.assertEqual(len(events), 14)
        self.assertEqual(next(name for action, name, _ in events if action == "end"), "second")
        text = self.output.getvalue()
        positions = [text.index(("=== " + job.name + " ===").encode()) for job in jobs]
        self.assertEqual(positions, sorted(positions))
        for job in jobs:
            self.assertIn((job.name + " stdout\n" + job.name + " stderr\n").encode(), text)
        self.assertEqual(text.count(b"x" * 200000), 5)
        self.assertIn(b"7/7 passed", text)

    def test_scratch_creation_failure_keeps_signal_handlers(self):
        previous = {sig: signal.getsignal(sig) for sig in (signal.SIGINT, signal.SIGTERM)}
        with mock.patch.object(verify_native_suite.tempfile, "TemporaryDirectory", side_effect=OSError("no space")):
            with self.assertRaises(OSError):
                self.run_jobs([self.job("x")])
        self.assertEqual(previous, {sig: signal.getsignal(sig) for sig in previous})
        context = mock.MagicMock()
        context.__enter__.side_effect = OSError("cannot enter scratch")
        with mock.patch.object(verify_native_suite.tempfile, "TemporaryDirectory", return_value=context):
            with self.assertRaises(OSError):
                self.run_jobs([self.job("x")])
        self.assertEqual(previous, {sig: signal.getsignal(sig) for sig in previous})

    def test_successful_reaped_groups_are_not_signalled_in_cleanup(self):
        with mock.patch.object(verify_native_suite, "_signal_group", wraps=verify_native_suite._signal_group) as send:
            self.assertEqual(self.run_jobs([self.job("a"), self.job("b")]), 0)
        self.assertEqual([call.args[1] for call in send.call_args_list], [0, 0])

    def test_one_worker_is_serial(self):
        self.assertEqual(self.run_jobs([self.job("a"), self.job("b")], workers=1), 0)
        events = [json.loads(line)[:2] for line in (self.root / "events").read_text().splitlines()]
        self.assertEqual(events, [["start", "a"], ["end", "a"], ["start", "b"], ["end", "b"]])

    def test_failure_preserves_code_stops_sibling_tree_and_reports_unstarted(self):
        self.assertEqual(self.run_jobs([self.job("first", "wait"), self.job("bad", "fail"), self.job("unstarted")]), 23)
        self.assert_stopped(self.root / "first.pids")
        self.assertIn(b"unstarted: NOT RUN", self.output.getvalue())
        self.assertNotIn(b'"unstarted"', (self.root / "events").read_bytes())

    def test_timeout_stops_process_group(self):
        self.assertEqual(self.run_jobs([self.job("first", "wait")], timeout=.3), 124)
        self.assert_stopped(self.root / "first.pids")
        self.assertIn(b"TIMEOUT", self.output.getvalue())

    def test_missing_tool_and_missing_worker_fail(self):
        self.assertNotEqual(self.run_jobs([self.job("missing", "tool")]), 0)
        self.assertIn(b"FileNotFoundError", self.output.getvalue())
        self.assertEqual(self.run_jobs([Job("absent", ("/nonexistent/oxid-worker",), b"absent")]), 1)
        self.assertIn(b"could not start", self.output.getvalue())

    def test_zero_exit_without_final_summary_is_failure(self):
        self.assertEqual(self.run_jobs([self.job("truncated", "empty")]), 1)
        self.assertIn(b"missing final PASS summary", self.output.getvalue())

    def test_successful_worker_cannot_leave_running_descendants(self):
        self.assertEqual(self.run_jobs([self.job("orphan", "orphan")]), 1)
        self.assertIn(b"left running descendants", self.output.getvalue())
        self.assert_stopped(self.root / "orphan.pids")

    def test_cli_preflight_and_compiler_hash_guard(self):
        scripts = self.root / "scripts"
        scripts.mkdir()
        runner = scripts / "verify_native_suite.py"
        runner.write_bytes(Path(verify_native_suite.__file__).read_bytes())
        binary = self.root / "oxid"
        binary.write_text("fixture compiler")
        binary.chmod(0o755)
        command = [sys.executable, str(runner), str(binary), str(binary)]
        result = subprocess.run(command, capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"oracle script is missing", result.stderr)
        for job in build_jobs(scripts, str(binary), str(binary)):
            Path(job.command[1]).write_text(f"print({job.pass_prefix.decode()!r} + ': fixture: PASS')\n")
        self.assertEqual(subprocess.run(command, capture_output=True).returncode, 0)
        script = scripts / "verify_scalar_comparisons.py"
        script.write_text("from pathlib import Path\n" +
                          f"Path({str(binary)!r}).write_text('changed compiler')\n" + script.read_text())
        result = subprocess.run(command, capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"compiler binaries changed", result.stderr)
        result = subprocess.run(command, capture_output=True, env=dict(os.environ, PYTHONOPTIMIZE="1"))
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"assertions must be enabled", result.stderr)
        binary.unlink()
        result = subprocess.run(command, capture_output=True)
        self.assertEqual(result.returncode, 1)
        self.assertIn(b"compiler is missing", result.stderr)

    def test_child_signal_becomes_shell_exit_code(self):
        self.assertEqual(self.run_jobs([self.job("signalled", "signal")]), 128 + signal.SIGTERM)

    def test_invalid_limits_and_empty_suite_are_rejected(self):
        for workers in (0, 3):
            with self.assertRaises(ValueError): self.run_jobs([self.job("x")], workers=workers)
        for timeout in (0, -1, float("inf"), float("nan")):
            with self.assertRaises(ValueError): self.run_jobs([self.job("x")], timeout=timeout)
        with self.assertRaises(ValueError): self.run_jobs([])

    def test_interruptions_stop_entire_process_group(self):
        for sig in (signal.SIGINT, signal.SIGTERM):
            with self.subTest(signal=sig):
                marker = self.root / "first.pids"
                marker.unlink(missing_ok=True)
                (self.root / "first.ready").unlink(missing_ok=True)
                harness = self.root / "harness.py"
                harness.write_text(
                    "import sys\nfrom verify_native_suite import Job, run_suite\n"
                    f"job = Job('first', {self.job('first', 'wait').command!r}, b'first')\n"
                    "sys.exit(run_suite([job], jobs=1, timeout=20))\n")
                env = dict(os.environ, PYTHONPATH=str(Path(__file__).resolve().parent))
                process = subprocess.Popen([sys.executable, str(harness)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env)
                try:
                    deadline = time.monotonic() + 5
                    while not marker.exists() and time.monotonic() < deadline:
                        time.sleep(.01)
                    self.assertTrue(marker.exists())
                    process.send_signal(sig)
                    stdout, stderr = process.communicate(timeout=5)
                    self.assertEqual(process.returncode, 128 + sig, stderr)
                    self.assertIn(b"interrupted", stdout)
                    self.assert_stopped(marker)
                finally:
                    if process.poll() is None:
                        process.kill()
                    process.wait()


if __name__ == "__main__":
    unittest.main()
