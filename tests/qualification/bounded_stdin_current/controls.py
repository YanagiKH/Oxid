"""Fixed actual-input controls. No Rust builds, discovery expansion, or shell calls."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import resource
import signal
import stat
import struct
import subprocess

RAW_TEST = "frontend::oir::owned::builtin_input_tests::builtin_input_subprocess_child"
SOURCE_TEST = "frontend::oir::owned::source::builtin_source_tests::builtin_source_input_subprocess_child"
ELF_ENV = {"PATH": "/no-tools", "LC_ALL": "C"}
MAX_STREAM = 16 * 1024 * 1024
MAX_INPUT = 1025
MAX_ARTIFACT = 512 * 1024 * 1024
HELPERS = Path(__file__).resolve().parent

# name, capacity, bytes, status, checksum, unread, stdin kind
RAW_CASES = (
    ("zero", 0, b"abc", -1, 0, b"abc", "pipe"),
    ("empty", 3, b"", 0, -21, b"", "pipe"),
    ("prefix", 3, b"A", 1, 51, b"", "file"),
    ("exact", 3, b"ABC", -1, 198, b"", "file"),
    ("excess", 3, b"ABCD", -1, 198, b"D", "pipe"),
    ("byte-values", 3, bytes((0, 128, 255, 9)), -1, 383, b"\t", "pipe"),
    ("error-empty", 3, b"", -2, -21, b"", "nonblocking"),
    ("error-prefix", 3, b"A", -2, -21, b"", "nonblocking"),
    ("limit128", 129, b"x" * 128, 128, 15353, b"", "file"),
    ("limit129", 129, b"x" * 129, -1, 15480, b"", "pipe"),
    ("limit130", 129, b"x" * 129 + b"Z", -1, 15480, b"Z", "pipe"),
    ("maximum", 1024, b"\xff" * 1024 + b"Z", -1, 261120, b"Z", "pipe"),
)
SENTINEL, FULL, PREFIX = (-7, -7, -7), (65, 66, 67), (65, -7, -7)
# name, input, fuel, result (None = fuel failure), consumed, failure buffer,
# remaining fuel, nonblocking, synthetic EINTR. These are independent literals.
FUEL_CASES = (
    ("base-short", b"ABC", 72, None, 0, SENTINEL, 6, False, False),
    ("before-first", b"ABC", 73, None, 0, SENTINEL, 0, False, False),
    ("after-one", b"ABC", 74, None, 1, SENTINEL, 0, False, False),
    ("after-two", b"ABC", 75, None, 2, SENTINEL, 0, False, False),
    ("full-before-return", b"ABC", 76, None, 3, FULL, 0, False, False),
    ("return-one-short", b"ABC", 81, None, 3, FULL, 5, False, False),
    ("after-return", b"ABC", 82, None, 3, FULL, 0, False, False),
    ("full-one-short", b"ABC", 114, None, 3, None, 9, False, False),
    ("full-exact", b"ABC", 115, -1, 3, None, 0, False, False),
    ("before-eof", b"A", 74, None, 1, SENTINEL, 0, False, False),
    ("eof-before-return", b"A", 75, None, 1, PREFIX, 0, False, False),
    ("eof-exact", b"A", 113, 1, 1, None, 0, False, False),
    ("error-before-return", b"A", 75, None, 1, SENTINEL, 0, True, False),
    ("error-exact", b"A", 115, -2, 1, None, 0, True, False),
    ("retry-one", b"ABC", 74, None, 0, SENTINEL, 0, False, True),
    ("retry-two", b"ABC", 75, None, 0, SENTINEL, 0, False, True),
    ("retry-byte-one", b"ABC", 76, None, 1, SENTINEL, 0, False, True),
    ("retry-byte-two", b"ABC", 77, None, 2, SENTINEL, 0, False, True),
    ("retry-full-before-return", b"ABC", 78, None, 3, FULL, 0, False, True),
    ("retry-full-one-short", b"ABC", 116, None, 3, None, 9, False, True),
    ("retry-full-exact", b"ABC", 117, -1, 3, None, 0, False, True),
    ("retry-eof-before-return", b"A", 77, None, 1, PREFIX, 0, False, True),
    ("retry-eof-exact", b"A", 115, 1, 1, None, 0, False, True),
    ("retry-error-before-return", b"A", 77, None, 1, SENTINEL, 0, True, True),
    ("retry-error-exact", b"A", 117, -2, 1, None, 0, True, True),
)
SHAPE_CASES = (
    ("empty", b"", 0, -21, b"", "pipe"),
    ("prefix", b"A", 1, 51, b"", "pipe"),
    ("full-excess", b"ABCD", -1, 198, b"D", "pipe"),
    ("error-prefix", b"A", -2, -21, b"", "nonblocking"),
    ("bytes", bytes((0, 128, 255, 9)), -1, 383, b"\t", "pipe"),
)
SOURCE_CASES = (
    ("empty", b"", -21, b"", "pipe"),
    ("prefix", b"A", 1051, b"", "pipe"),
    ("full-excess", b"ABCZ", 10198, b"Z", "pipe"),
    ("byte-values", bytes((0, 128, 255, 9)), 10383, b"\t", "pipe"),
    ("error-empty", b"", -10021, b"", "nonblocking"),
    ("error-prefix", b"A", -10021, b"", "nonblocking"),
)
ROSTER = {"raw": 24, "fuel": 25, "shapes": 15, "private-source": 6, "public-source": 7}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def no_symlinks(path):
    path = Path(os.path.abspath(path))
    for part in (path, *path.parents):
        require(not part.is_symlink(), "symlink path: " + str(part))
    return path


def fresh(path):
    path = no_symlinks(path)
    path.mkdir(parents=True, exist_ok=False)
    return path


def read_file(path, limit=MAX_ARTIFACT):
    path = no_symlinks(path)
    info = path.stat()
    require(stat.S_ISREG(info.st_mode) and info.st_size <= limit,
            "missing, nonregular, or oversized file: " + str(path))
    data = path.read_bytes()
    require(len(data) <= limit, "file grew past cap: " + str(path))
    return data


def identity(path):
    data = read_file(path)
    return {"path": str(Path(path).absolute()), "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest()}


def save_json(path, value):
    with no_symlinks(path).open("x") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def child_env(base, capacity=None, mode=None, fuel=None, shape=None):
    env = {k: v for k, v in base.items() if k not in ("LD_PRELOAD", "LD_AUDIT")
           and not k.startswith(("OXID_RAW_STDIN_", "OXID_PRIVATE_SOURCE_"))}
    for key, value in (("CAPACITY", capacity), ("MODE", mode), ("FUEL", fuel), ("SHAPE", shape)):
        if value is not None:
            env["OXID_RAW_STDIN_" + key] = str(value)
    return env


def test_command(unit, name):
    return [str(unit), name, "--exact", "--nocapture", "--test-threads=1", "--color=never"]


def admit_listing(data, name):
    require([line for line in data.decode().splitlines() if line] ==
            [name + ": test", "1 test, 0 benchmarks"], "missing or ambiguous exact nonignored child")


def admit_child(result, name, required_markers):
    require(result.returncode == 0, "child did not return success")
    lines = [line for line in result.stdout.decode().splitlines() if line]
    require(lines and lines.pop(0) == "running 1 test", "child did not execute exactly one test")
    require(lines and re.fullmatch(
        r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; \d+ filtered out; finished in \d+(?:\.\d+)?s",
        lines.pop()), "missing exact successful child summary")
    prefix = "test " + name + " ... "
    require(lines and lines[0].startswith(prefix), "wrong selected child")
    lines[0] = lines[0][len(prefix):]
    require(lines[-1] == "ok", "child completion missing")
    lines.pop()
    markers = {}
    for line in lines:
        require(re.fullmatch(r"OXID_(?:RAW|SOURCE)_STDIN_[A-Z_]+=[^\r\n]*", line),
                "unexpected child output: " + line)
        key, value = line.split("=", 1)
        require(key not in markers, "duplicate child marker")
        markers[key] = value
    require(set(required_markers) <= markers.keys(), "missing child execution marker")
    return markers


def _limits():
    # Per-file cap includes streams and emitted native files; no sandbox claim.
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_STREAM, MAX_STREAM))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def run(directory, argv, env, data=b"", kind="pipe", timeout=30):
    """Parent owns stdin lifetime and unread observation; always retain receipts."""
    require(kind in ("pipe", "file", "nonblocking", "directory"), "unknown stdin kind")
    require(len(data) <= MAX_INPUT and (kind != "directory" or not data), "invalid input size")
    directory = fresh(directory)
    cwd = fresh(directory / "empty-cwd")
    (directory / "input.bin").write_bytes(data)
    read_fd = write_fd = None
    error = None
    code = None
    remaining = b""
    args = [str(arg) for arg in argv]
    try:
        if kind == "file":
            read_fd = os.open(directory / "input.bin", os.O_RDONLY)
        elif kind == "directory":
            read_fd = os.open(cwd, os.O_RDONLY | os.O_DIRECTORY)
        else:
            read_fd, write_fd = os.pipe()
            require(os.write(write_fd, data) == len(data), "short bounded input write")
            if kind == "nonblocking":
                os.set_blocking(read_fd, False)
            else:
                os.close(write_fd)
                write_fd = None
        with (directory / "stdout").open("xb") as stdout, (directory / "stderr").open("xb") as stderr:
            process = subprocess.Popen(args, stdin=read_fd, stdout=stdout, stderr=stderr,
                cwd=cwd, env=env, start_new_session=True, preexec_fn=_limits)
            try:
                code = process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                code = process.wait()
                raise
        if write_fd is not None:
            os.close(write_fd)
            write_fd = None
        if kind != "directory":
            os.set_blocking(read_fd, False)
            while True:
                part = os.read(read_fd, MAX_INPUT + 1)
                if not part:
                    break
                remaining += part
                require(len(remaining) <= MAX_INPUT, "unread input exceeds cap")
    except (OSError, subprocess.TimeoutExpired, ValueError) as caught:
        error = str(caught)
    finally:
        for fd in (read_fd, write_fd):
            if fd is not None:
                os.close(fd)
        (directory / "remaining.bin").write_bytes(remaining)
        save_json(directory / "receipt.json", {
            "argv": args, "cwd": str(cwd), "status": code, "error": error,
            "stdin_kind": kind, "input": identity(directory / "input.bin"),
            "remaining": identity(directory / "remaining.bin"), "timeout_seconds": timeout,
            "stream_file_cap": MAX_STREAM, "environment": {
                "mode": "cleared-runtime" if env == ELF_ENV or set(env) == {*ELF_ENV, "LD_PRELOAD"} else "inherited-scoped",
                "values": {k: v for k, v in env.items() if k.startswith(("OXID_", "GIT_"))
                    or k in ("PATH", "LC_ALL", "LD_PRELOAD", "LD_LIBRARY_PATH", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")},
            },
        })
    require(error is None, "process failure: " + str(error))
    stdout = read_file(directory / "stdout", MAX_STREAM)
    stderr = read_file(directory / "stderr", MAX_STREAM)
    require(len(stdout) < MAX_STREAM and len(stderr) < MAX_STREAM, "stream cap reached")
    return subprocess.CompletedProcess(args, code, stdout, stderr), remaining


def native_success(result, value):
    require((result.returncode, result.stdout, result.stderr) == (0, f"{value}\n".encode(), b""),
            "wrong native result, status, or streams")


def calibration_expected(mode):
    # Fixed probe sequence: fd 0/count 0 and 2 never spend the retry quota.
    single = {
        "clean": ((1, 0, "435a"), (1, 0, "445a"), (1, 0, "455a"), (0, 0, "a55a"), (0, 0, "a55a"), (0, 0, "a55a")),
        "injected": ((-1, 4, "a55a"), (-1, 4, "a55a"), (1, 0, "435a"), (1, 0, "445a"), (1, 0, "455a"), (0, 0, "a55a")),
        "empty-clean": ((0, 0, "a55a"),) * 6,
    }[mode]
    rows = [("other-1", 1, 1, 0, "785a"), ("stdin-zero-before", 0, 0, 0, "a55a"),
            ("stdin-two", 2, 0 if mode == "empty-clean" else 2, 0, "a55a" if mode == "empty-clean" else "4142"),
            ("stdin-one-1", 1, *single[0]), ("other-2", 1, 1, 0, "795a"),
            ("stdin-zero-between", 0, 0, 0, "a55a"), ("stdin-one-2", 1, *single[1]),
            ("other-3", 1, 1, 0, "7a5a"),
            *((f"stdin-one-{n + 1}", 1, *single[n]) for n in range(2, 6)),
            ("other-eof", 1, 0, 0, "a55a"), ("other-invalid", 1, -1, 9, "a55a")]
    return "".join(f"{label} count={count} result={value} errno={error} buffer={buffer}\n"
                   for label, count, value, error, buffer in rows).encode()


def calibrate(root, probe, shim):
    root = fresh(root)
    other = root / "other-fd.bin"
    other.write_bytes(b"xyz")
    for mode in ("clean", "injected", "empty-clean"):
        env = dict(ELF_ENV)
        if mode == "injected":
            env["LD_PRELOAD"] = str(shim)
        result, remaining = run(root / mode, [probe, other], env,
            b"" if mode == "empty-clean" else b"ABCDE", "file")
        require((result.returncode, result.stdout, result.stderr, remaining) ==
                (0, calibration_expected(mode), b"", b""), "retry calibration failed: " + mode)
    save_json(root / "result.json", {"status": "passed", "probe": identity(probe),
        "shim": identity(shim), "synthetic_eintr_count": 2,
        "scope": "fd0/count1 per-thread startup preload; other fd/count unchanged; no real signals"})


class Suite:
    def __init__(self, root, unit, cli, base_env, shim):
        self.root, self.unit, self.cli = root, unit, cli
        self.env, self.shim = base_env, shim
        self.artifacts = fresh(root / "artifacts")
        self.programs, self.rows, self.artifact_ids = {}, [], []

    def program(self, capacity, mode, fuel=None, shape=None):
        key = (capacity, mode, fuel, shape)
        if key not in self.programs:
            target = fresh(self.artifacts / ("raw-%d-%s-%s-%s" % key))
            env = child_env(self.env, capacity, mode, fuel, shape)
            env["OXID_RAW_STDIN_NATIVE_DIR"] = str(target)
            result, _ = run(target / "build", test_command(self.unit, RAW_TEST), env, timeout=120)
            markers = admit_child(result, RAW_TEST, ("OXID_RAW_STDIN_NATIVE_READY",))
            require(markers == {"OXID_RAW_STDIN_NATIVE_READY": "1"} and not result.stderr,
                    "wrong raw native build markers")
            self.record_artifact(target, {"capacity": capacity, "mode": mode, "fuel": fuel, "shape": shape})
            self.programs[key] = target / "program"
        return self.programs[key]

    def record_artifact(self, target, configuration):
        require(read_file(target / "program", MAX_STREAM).startswith(b"\x7fELF"), "not an ELF")
        row = {"configuration": configuration, "elf": identity(target / "program"),
               "ir": identity(target / "program.ll")}
        if (target / "main.ox").exists():
            row["source"] = identity(target / "main.ox")
        self.artifact_ids.append(row)
        save_json(target / "artifacts.json", row)

    def pair(self, group, name, data, kind, expected, unread, *, capacity=3, mode="status", shape=None):
        target = fresh(self.root / (group + "-" + name))
        program = self.program(capacity, mode, shape=shape)
        ref, left = run(target / "reference", test_command(self.unit, RAW_TEST),
                        child_env(self.env, capacity, mode, shape=shape), data, kind)
        markers = admit_child(ref, RAW_TEST, ("OXID_RAW_STDIN_RESULT", "OXID_RAW_STDIN_REMAINING_FUEL"))
        require(set(markers) == {"OXID_RAW_STDIN_RESULT", "OXID_RAW_STDIN_REMAINING_FUEL"}
                and int(markers["OXID_RAW_STDIN_RESULT"]) == expected and not ref.stderr and left == unread,
                group + ": wrong reference result, input effect, or stderr")
        native, left = run(target / "native", [program], ELF_ENV, data, kind)
        native_success(native, expected)
        require(left == unread, "wrong native unread bytes")
        self.rows.append({"group": group, "name": name, "expected": expected, "passed": True})

    def raw_and_shapes(self):
        for name, capacity, data, status, checksum, unread, kind in RAW_CASES:
            for mode, expected in (("status", status), ("checksum", checksum)):
                self.pair("raw", name + "-" + mode, data, kind, expected, unread, capacity=capacity, mode=mode)
        for shape, mode in (("projected", "checksum"), ("forwarded", "status"), ("forwarded", "checksum")):
            for name, data, status, checksum, unread, kind in SHAPE_CASES:
                expected = status if mode == "status" else checksum + (123456000 if shape == "projected" else 0)
                self.pair("shapes", shape + "-" + mode + "-" + name, data, kind, expected, unread, mode=mode, shape=shape)

    def fuel(self):
        for name, data, fuel, expected, consumed, buffer, remaining_fuel, nonblocking, injected in FUEL_CASES:
            target = fresh(self.root / ("fuel-" + name))
            program = self.program(3, "status", fuel=fuel)
            env = child_env(self.env, 3, "status", fuel)
            native_env = dict(ELF_ENV)
            if injected:
                env["LD_PRELOAD"] = native_env["LD_PRELOAD"] = str(self.shim)
            kind = "nonblocking" if nonblocking else "pipe"
            ref, left = run(target / "reference", test_command(self.unit, RAW_TEST), env, data, kind)
            markers = admit_child(ref, RAW_TEST, ("OXID_RAW_STDIN_REMAINING_FUEL",))
            allowed = {"OXID_RAW_STDIN_REMAINING_FUEL", "OXID_RAW_STDIN_FAILURE", "OXID_RAW_STDIN_BUFFER"} if expected is None else {
                "OXID_RAW_STDIN_REMAINING_FUEL", "OXID_RAW_STDIN_RESULT"}
            require(set(markers) <= allowed, "unexpected fuel observation marker")
            require(left == data[consumed:] and int(markers["OXID_RAW_STDIN_REMAINING_FUEL"]) == remaining_fuel,
                    "wrong reference fuel/input effects")
            if expected is None:
                require(markers.get("OXID_RAW_STDIN_FAILURE") == "fuel" and b"E0601" in ref.stderr,
                        "missing reference fuel failure")
                if buffer is not None:
                    require(markers.get("OXID_RAW_STDIN_BUFFER") == struct.pack("<iii", *buffer).hex(),
                            "wrong reference commit boundary buffer")
            else:
                require(markers.get("OXID_RAW_STDIN_RESULT") == str(expected) and not ref.stderr,
                        "wrong successful reference fuel boundary")
            native, left = run(target / "native", [program], native_env, data, kind)
            require(left == data[consumed:], "wrong native fuel input effects")
            if expected is None:
                require((native.returncode, native.stdout, native.stderr) == (1, b"", ref.stderr),
                        "native/reference fuel diagnostic mismatch")
            else:
                native_success(native, expected)
            self.rows.append({"group": "fuel", "name": name, "fuel": fuel, "synthetic_eintr": injected,
                "expected": expected, "consumed": consumed, "reference_failure_buffer": buffer,
                "remaining_fuel": remaining_fuel, "passed": True})

    def sources(self):
        target = fresh(self.artifacts / "private-source")
        env = child_env(self.env)
        env["OXID_PRIVATE_SOURCE_STDIN"] = "1"
        build_env = dict(env, OXID_PRIVATE_SOURCE_NATIVE_DIR=str(target))
        result, _ = run(target / "build", test_command(self.unit, SOURCE_TEST), build_env, timeout=120)
        markers = admit_child(result, SOURCE_TEST, ("OXID_SOURCE_STDIN_RESULT", "OXID_SOURCE_STDIN_NATIVE_READY"))
        require(markers == {"OXID_SOURCE_STDIN_RESULT": "-21", "OXID_SOURCE_STDIN_NATIVE_READY": "1"}
                and not result.stderr, "wrong private source build result")
        self.record_artifact(target, {"source": "private parsed-source"})
        public = fresh(self.artifacts / "public-source")
        source = public / "main.ox"
        source.write_bytes(read_file(HELPERS / "smoke.ox"))
        require(read_file(target / "main.ox").strip() == source.read_bytes().strip(),
                "public and private smoke source differ")
        elf = public / "program"
        result, unread = run(public / "compile", [self.cli, "compile", source,
            "--edition=typed-preview", "--backend=llvm", "--output=" + str(elf), "--message-format=json"],
            child_env(self.env), b"COMPILE-MUST-NOT-READ", timeout=120)
        require(result.returncode == 0 and not result.stderr and unread == b"COMPILE-MUST-NOT-READ"
            and json.loads(result.stdout) == {"schema_version": 1, "edition": "typed-preview",
                "kind": "compile-summary", "success": True, "errors": 0, "output": str(elf)},
            "public compilation failed or consumed stdin")
        require(read_file(elf, MAX_STREAM).startswith(b"\x7fELF"), "public compile did not emit ELF")
        # The public CLI does not expose IR. Retain source and ELF; private/raw
        # native APIs retain exact emitted IR independently.
        self.artifact_ids.append({"configuration": {"source": "public CLI"},
            "source": identity(source), "elf": identity(elf), "ir": None})
        for group, reference, reference_env, program in (
            ("private-source", test_command(self.unit, SOURCE_TEST), env, target / "program"),
            ("public-source", [self.cli, "run", source, "--edition=typed-preview"], child_env(self.env), elf),
        ):
            cases = SOURCE_CASES + ((("directory-error", b"", -10021, b"", "directory"),)
                                    if group == "public-source" else ())
            for name, data, expected, unread, kind in cases:
                case = fresh(self.root / (group + "-" + name))
                ref, left = run(case / "reference", reference, reference_env, data, kind)
                if group == "private-source":
                    markers = admit_child(ref, SOURCE_TEST, ("OXID_SOURCE_STDIN_RESULT",))
                    require(markers == {"OXID_SOURCE_STDIN_RESULT": str(expected)} and not ref.stderr,
                            "wrong private parsed-source result")
                else:
                    native_success(ref, expected)
                require(left == unread, "wrong source reference input effects")
                native, left = run(case / "native", [program], ELF_ENV, data, kind)
                native_success(native, expected)
                require(left == unread, "wrong source native input effects")
                self.rows.append({"group": group, "name": name, "expected": expected, "passed": True})

    def finish(self):
        counts = {group: sum(row["group"] == group for row in self.rows) for group in ROSTER}
        require(counts == ROSTER and len(self.rows) == 77, "fixed paired case roster changed")
        require(len(self.programs) == 25 and len(self.artifact_ids) == 27,
                "fixed native artifact reuse count changed")
        for row in self.artifact_ids:
            for field in ("elf", "ir", "source"):
                if row.get(field):
                    require(identity(Path(row[field]["path"])) == row[field], "artifact changed during execution")
        save_json(self.root / "report.json", {"status": "passed", "counts": counts, "paired_cases": 77,
            "native_artifact_count": 27,
            "native_artifacts": self.artifact_ids, "cases": self.rows,
            "limits": ["Synthetic EINTR is not real signal delivery", "Cleared cwd/env is not filesystem isolation",
                "Closed fd 0 is excluded: Rust startup may reopen it; real EAGAIN and EISDIR exercise IoError",
                "Failure buffer snapshots are reference observations; native effects use unread bytes and diagnostics",
                "Public CLI compilation exposes no IR; raw/private-source artifacts retain IR"]})
