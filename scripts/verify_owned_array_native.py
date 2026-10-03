#!/usr/bin/env python3
"""Admit and preserve the existing owned-native prefix, without a semantic oracle.

Producer-only Unit2E boundary. Independent replay integration is a separate step.
All synthetic controls live in test_owned_array_native.py; no CLI bypass exists.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import stat
import struct
import contextlib
import io


PREFIX = "frontend::oir::owned::native::tests::"
PROFILES = ("debug", "release")
ROSTER = ('frontend::oir::owned::native::tests::arrays::native_array_continuations_use_source_free_elf_on_both_paths_and_every_fuel',
 'frontend::oir::owned::native::tests::arrays::native_array_effects_use_source_free_elf_and_every_fuel',
 'frontend::oir::owned::native::tests::arrays::native_arrays_all_195_small_cases_use_source_free_elf_and_every_fuel',
 'frontend::oir::owned::native::tests::arrays::native_arrays_extreme_payloads_and_production_guarded_wrappers_use_real_llvm',
 'frontend::oir::owned::native::tests::arrays::native_arrays_maximum_width_39_cases_use_source_free_elf_at_charge_boundaries',
 'frontend::oir::owned::native::tests::arrays::native_arrays_nine_transfer_inputs_use_source_free_elf_and_every_fuel',
 'frontend::oir::owned::native::tests::heldout_review::review_heldout_call_result_merge_real_llvm',
 'frontend::oir::owned::native::tests::heldout_review::review_heldout_interleaved_abi_real_llvm',
 'frontend::oir::owned::native::tests::native_owned_actual_llvm_guard_dominance_and_failure_order',
 'frontend::oir::owned::native::tests::native_owned_batch_every_budget_uses_real_llvm',
 'frontend::oir::owned::native::tests::native_owned_batch_write_failure_every_budget_uses_real_llvm',
 'frontend::oir::owned::native::tests::native_owned_expanded_boundary_uses_real_llvm_and_measures_stack',
 'frontend::oir::owned::native::tests::native_owned_extended_storage_every_budget_uses_real_llvm',
 'frontend::oir::owned::native::tests::native_owned_merge_and_depth_boundaries_use_real_llvm',
 'frontend::oir::owned::native::tests::native_owned_tiny_fixtures_use_real_llvm',
 'frontend::oir::owned::native::tests::source_resources::source_native_actual_slot_and_cell_boundaries_use_real_llvm')
FAMILIES = ({'additional_guarded_modules': 195,
  'compiled_elfs': 390,
  'distinct_inputs': 195,
  'elf_processes': 5037,
  'name': 'frontend::oir::owned::native::tests::arrays::native_arrays_all_195_small_cases_use_source_free_elf_and_every_fuel',
  'summary_label': 'array small core',
  'reference_comparisons_derived': 4842},
 {'additional_guarded_modules': 39,
  'compiled_elfs': 78,
  'distinct_inputs': 39,
  'elf_processes': 309,
  'name': 'frontend::oir::owned::native::tests::arrays::native_arrays_maximum_width_39_cases_use_source_free_elf_at_charge_boundaries',
  'summary_label': 'array maximum width',
  'reference_comparisons_derived': 270},
 {'additional_guarded_modules': 9,
  'compiled_elfs': 18,
  'distinct_inputs': 9,
  'elf_processes': 1083,
  'name': 'frontend::oir::owned::native::tests::arrays::native_arrays_nine_transfer_inputs_use_source_free_elf_and_every_fuel',
  'summary_label': 'array transfers',
  'reference_comparisons_derived': 1074},
 {'additional_guarded_modules': 8,
  'compiled_elfs': 16,
  'distinct_inputs': 8,
  'elf_processes': 288,
  'name': 'frontend::oir::owned::native::tests::arrays::native_array_continuations_use_source_free_elf_on_both_paths_and_every_fuel',
  'summary_label': 'array continuations',
  'reference_comparisons_derived': 280},
 {'additional_guarded_modules': 4,
  'compiled_elfs': 8,
  'distinct_inputs': 4,
  'elf_processes': 229,
  'name': 'frontend::oir::owned::native::tests::arrays::native_array_effects_use_source_free_elf_and_every_fuel',
  'summary_label': 'array effects',
  'reference_comparisons_derived': 225},
 {'additional_guarded_modules': 4,
  'compiled_elfs': 14,
  'distinct_inputs': 4,
  'elf_processes': 112,
  'name': 'frontend::oir::owned::native::tests::arrays::native_arrays_extreme_payloads_and_production_guarded_wrappers_use_real_llvm',
  'summary_label': 'array extreme payloads and guarded wrappers',
  'reference_comparisons_derived': 102})
MULTILINE_NAME = 'frontend::oir::owned::native::tests::source_resources::source_native_actual_slot_and_cell_boundaries_use_real_llvm'
MULTILINE_PAYLOADS = ('source-slots256: source witness -> LLVM19.1.7 -> source-free ELF; stdout=(), stderr=empty, status=0', 'source-cells8192: source witness -> LLVM19.1.7 -> source-free ELF; stdout=(), stderr=empty, status=0')


class AdmissionError(ValueError):
    """Captured evidence did not meet the producer boundary."""


def require(condition, message):
    if not condition:
        raise AdmissionError(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def read_json(path):
    def pairs(items):
        value = {}
        for key, item in items:
            require(key not in value, "duplicate JSON key: " + key)
            value[key] = item
        return value
    return json.loads(stable_bytes(path)[0], object_pairs_hook=pairs)


def write_json(path, value):
    with Path(path).open("xb") as handle:
        handle.write(json_bytes(value))


def stat_identity(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def no_symlink_path(path):
    path = Path(os.path.abspath(path))
    for component in reversed((path, *path.parents)):
        require(not component.is_symlink(), "symlink path: " + str(component))
    return path


@contextlib.contextmanager
def stable_file(path):
    """Anchor every ancestor; verify membership and identity after bounded reads."""
    path = Path(os.path.abspath(path))
    fd = os.open(path.anchor, os.O_RDONLY | os.O_DIRECTORY)
    parents = []
    try:
        for part in path.parts[1:-1]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            parents.append((fd, part, os.fstat(child)))
            fd = child
        before = os.stat(path.name, dir_fd=fd, follow_symlinks=False)
        require(stat.S_ISREG(before.st_mode), "not a regular file: " + str(path))
        descriptor = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
        with os.fdopen(descriptor, "rb") as handle:
            require(stat_identity(os.fstat(handle.fileno())) == stat_identity(before), "file changed on open")
            yield handle, before
            require(stat_identity(os.fstat(handle.fileno())) == stat_identity(before), "file changed during capture")
        require(stat_identity(os.stat(path.name, dir_fd=fd, follow_symlinks=False)) == stat_identity(before),
                "file replaced during capture")
        for parent, part, value in parents:
            now = os.stat(part, dir_fd=parent, follow_symlinks=False)
            require((now.st_dev, now.st_ino, now.st_mode) == (value.st_dev, value.st_ino, value.st_mode),
                    "ancestor changed during capture")
    finally:
        os.close(fd)
        for parent, _, _ in parents:
            os.close(parent)


def capture_file(path, output=None):
    with stable_file(path) as (handle, before):
        digest = hashlib.sha256()
        remaining = before.st_size
        while remaining:
            chunk = handle.read(min(1024 * 1024, remaining))
            require(bool(chunk), "file truncated during capture")
            remaining -= len(chunk)
            digest.update(chunk)
            if output is not None:
                output.write(chunk)
        require(not handle.read(1), "file grew during capture")
        row = {"bytes": before.st_size, "mode": stat.S_IMODE(before.st_mode), "sha256": digest.hexdigest()}
    return row


def stable_bytes(path):
    output = io.BytesIO()
    row = capture_file(path, output)
    return output.getvalue(), row["mode"]


def file_record(path):
    return capture_file(path)


def exact_inventory(actual, expected, label):
    require(bool(expected) and len(expected) == len(set(expected)), label + ": invalid expected inventory")
    require(len(actual) == len(set(actual)) and set(actual) == set(expected),
            label + ": missing, extra, or duplicate members")


def admit_list(data):
    lines = data.decode("utf-8").splitlines()
    names = []
    footer = False
    for line in lines:
        if not line:
            continue
        if line == "16 tests, 0 benchmarks" and not footer:
            footer = True
        else:
            require(not footer and line.endswith(": test"), "unknown or misplaced listing line")
            names.append(line[:-6])
    require(footer, "missing listing footer")
    exact_inventory(names, ROSTER, "ignored prefix roster")
    exact_inventory([name for name in names if name.startswith(PREFIX + "arrays::")],
                    [row["name"] for row in FAMILIES], "array roster")
    return names


def admit_stdout(data):
    """Attribute every completion, preserving original UTF-8 byte and line spans."""
    raw = data.splitlines(keepends=True)
    lines = [line.decode("utf-8").removesuffix("\n") for line in raw]
    offsets, position = [], 0
    for line in raw:
        offsets.append(position)
        position += len(line)
    outcomes = []
    started = finished = False
    index = 0
    while index < len(lines):
        line = lines[index]
        if not line:
            index += 1
            continue
        if not started:
            require(line == "running 16 tests", "missing exact running header")
            started = True
            index += 1
            continue
        require(not finished, "unexpected stdout after footer")
        if line.startswith("test result:"):
            require(len(outcomes) == len(ROSTER), "footer before complete attributed outcomes")
            require(re.fullmatch(r"test result: ok\. 16 passed; 0 failed; 0 ignored; 0 measured; \d+ filtered out; finished in \d+(?:\.\d+)?s", line),
                    "incorrect result footer")
            finished = True
            index += 1
            continue
        first = index
        special = "test " + MULTILINE_NAME + " ... " + MULTILINE_PAYLOADS[0]
        if line == special:
            require(lines[index:index + 3] == [special, MULTILINE_PAYLOADS[1], "ok"],
                    "incomplete or misattributed source-resource result")
            name = MULTILINE_NAME
            index += 3
        else:
            match = re.fullmatch(r"test (\S+) \.\.\. ok", line)
            require(match is not None, "unknown or failed stdout result: " + line)
            name = match.group(1)
            require(name != MULTILINE_NAME, "source-resource result lacks exact payloads")
            index += 1
        require(name in ROSTER and name not in {row["name"] for row in outcomes},
                "unexpected or duplicate result name: " + name)
        outcomes.append({"name": name, "status": "PASS", "line_start": first + 1,
                         "line_end": index, "byte_start": offsets[first],
                         "byte_end": offsets[index] if index < len(offsets) else len(data)})
    require(started and finished, "truncated stdout")
    exact_inventory([row["name"] for row in outcomes], ROSTER, "outcomes")
    return outcomes


def family_summary(row):
    inputs = str(row["distinct_inputs"]) + " input cases"
    if row["summary_label"] == "array extreme payloads and guarded wrappers":
        inputs = "4 new input cases plus 2 reused core inputs"
    result = (f'{row["summary_label"]}: {inputs}, {row["compiled_elfs"]} compiled artifacts, '
              f'{row["elf_processes"]} ELF executions; {row["additional_guarded_modules"]} '
              'additional preserved guarded production modules')
    if row["summary_label"] == "array effects":
        result += ". Intermediate effect traces are reference observations; native execution proves final results and exact failure diagnostics"
    return result


def admit_stderr(data):
    lines = data.decode("utf-8").splitlines()
    for row in FAMILIES:
        selected = [line for line in lines if line.startswith(row["summary_label"] + ":")]
        require(selected == [family_summary(row)], "missing, duplicate, or incorrect family summary: " + row["summary_label"])
    return [{**row, "execution_count_kind": "producer test assertion and summary",
             "reference_count_kind": "derived from source-bound control flow"} for row in FAMILIES]


def expected_array_members():
    result = {}
    for family, count in (("core", 195), ("max", 39), ("transfer", 9),
                          ("phi", 8), ("effect", 4), ("extreme", 4)):
        for index in range(count):
            stem = f"array-{family}-{index}"
            for path in ("acyclic", "fuel-argv"):
                for extension in ("ll", "elf"):
                    result[f"{stem}-{path}.{extension}"] = {"family": family, "role": "compiled-" + extension}
            result[stem + "-guarded-production.ll"] = {"family": family, "role": "original-guarded-ll"}
    for index, fuels in ((0, (23, 24, 1000000)), (1, (14, 15, 1000000))):
        for fuel in fuels:
            for extension in ("ll", "elf"):
                result[f"array-production-{index}-{fuel}.{extension}"] = {"family": "extreme", "role": "compiled-" + extension}
    return result


def elf_proof(data, mode):
    require(len(data) >= 64 and data[:7] == b"\x7fELF\x02\x01\x01", "not ELF64 little-endian version 1")
    kind, machine, version = struct.unpack_from("<HHI", data, 16)
    require(kind in (2, 3) and machine == 62 and version == 1 and struct.unpack_from("<H", data, 52)[0] == 64,
            "not an x86_64 executable ELF header")
    require(mode & 0o111 != 0, "ELF lacks executable mode")
    return {"class": "ELF64", "byte_order": "little-endian", "machine": "x86_64", "type": kind}


def traversal_error(error):
    raise error


def regular_inventory(root):
    root = no_symlink_path(root)
    require(root.is_dir(), "missing evidence directory: " + str(root))
    result = {}
    for directory, directories, files in os.walk(root, followlinks=False, onerror=traversal_error):
        for name in directories:
            require(not (Path(directory) / name).is_symlink(), "symlink evidence directory")
        for name in files:
            path = Path(directory) / name
            result[path.relative_to(root).as_posix()] = file_record(path)
    return result


def admit_artifacts(root, expected=None):
    expected = expected_array_members() if expected is None else expected
    inventory = regular_inventory(root)
    arrays = [name for name in inventory if Path(name).name.startswith("array-")]
    # Directory entries with array names cannot disappear from the member check.
    for directory, directories, _ in os.walk(root, followlinks=False, onerror=traversal_error):
        require(not any(name.startswith("array-") for name in directories), "array member is a directory")
    exact_inventory(arrays, list(expected), "array artifact filenames")
    result = {}
    for name, record in inventory.items():
        role = expected.get(name, {"family": "inherited", "role": "inherited"})
        record = {**record, **role}
        if name in expected:
            data, mode = stable_bytes(Path(root) / name)
            require(record == {**record, "bytes": len(data), "mode": mode, "sha256": sha256(data)}, "artifact changed")
            if name.endswith(".elf"):
                record["elf"] = elf_proof(data, mode)
            else:
                require(bool(data) and bool(data.decode("utf-8").strip()), "empty LLVM module")
        result[name] = record
    require(regular_inventory(root) == inventory, "artifact membership or bytes changed")
    return result


# The controller below uses only owned POSIX groups. It never searches processes
# by name, inherits arbitrary compiler flags, or reads Cargo cache credentials.
import argparse
import ctypes
import platform
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid

LLVM_TOOLS = ("llvm-as", "opt", "clang", "ld.lld", "llvm-readobj")
UNSET_CONTROLS = ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER",
                  "RUSTC_WORKSPACE_WRAPPER", "LD_PRELOAD", "LD_LIBRARY_PATH",
                  "CCC_OVERRIDE_OPTIONS", "CLANG_CONFIG_FILE_SYSTEM_DIR",
                  "CLANG_CONFIG_FILE_USER_DIR", "PYTHONOPTIMIZE")
RUST_RELEASE = "1.99.0"
RUST_COMMIT = "b940084d7eb6a299eb4bfeb8e34901bc051e7ac4"
RUST_HOST = "x86_64-unknown-linux-gnu"
BINDING = "tests/fixtures/typed_project_source_binding"
CI_FIELDS = {"repository": "GITHUB_REPOSITORY", "event_name": "GITHUB_EVENT_NAME",
             "run_id": "GITHUB_RUN_ID", "run_attempt": "GITHUB_RUN_ATTEMPT",
             "job_id": "GITHUB_JOB", "workflow_ref": "GITHUB_WORKFLOW_REF",
             "workflow_sha": "GITHUB_WORKFLOW_SHA", "runner_os": "RUNNER_OS",
             "runner_arch": "RUNNER_ARCH"}


class ChildFailure(AdmissionError):
    def __init__(self, message, code=1):
        super().__init__(message)
        self.code = code


def group_alive(pgid):
    try:
        os.killpg(pgid, 0)
        return True
    except ProcessLookupError:
        return False


def reap_group(pgid):
    while True:
        try:
            pid, _ = os.waitpid(-pgid, os.WNOHANG)
        except ChildProcessError:
            return
        if not pid:
            return


def stop_group(process):
    """Stop and reap only descendants in our new session's process group."""
    survived = group_alive(process.pid)
    for sig, limit in ((signal.SIGTERM, 0.5), (signal.SIGKILL, 2.0)):
        if not group_alive(process.pid):
            break
        os.killpg(process.pid, sig)
        deadline = time.monotonic() + limit
        while time.monotonic() < deadline:
            process.poll()
            if process.returncode is not None:
                reap_group(process.pid)
            if not group_alive(process.pid):
                break
            time.sleep(0.01)
    process.wait(timeout=1)
    reap_group(process.pid)
    return {"surviving_group_detected": survived, "stopped": not group_alive(process.pid)}


def tool_identity(path):
    path = Path(os.path.abspath(path))  # Preserve the invocation basename (ld.lld).
    require(path.is_file() and os.access(path, os.X_OK), "missing executable tool: " + str(path))
    target = path.resolve(strict=True)
    return {"path": str(path), "target": str(target), **file_record(target)}


def verify_tool_identity(row):
    require(tool_identity(row["path"]) == row, "tool selection or bytes changed: " + row["path"])


def validate_environment(environment, selection):
    directory = selection["llvm_directory"]
    require(Path(directory).is_absolute() and str(Path(directory).resolve(strict=True)) == directory,
            "LLVM selection must be a resolved absolute directory")
    require(environment.get("OXID_LLVM_BIN") == directory, "missing or changed exact OXID_LLVM_BIN")
    require(all(name not in environment for name in UNSET_CONTROLS), "unsupported injected child control")
    require(not any(name.startswith("CLANG_CONFIG_FILE_") for name in environment), "unsupported clang config")
    for name in LLVM_TOOLS:
        row = selection["tools"][name]
        require(row["path"] == str(Path(directory) / name), "wrong LLVM invocation path")
        verify_tool_identity(row)


def child_environment(selection, *, cargo_home, home, rustc, cargo, cc, cxx, ar, target, temporary,
                      evidence=None):
    require(bool(home) and Path(home).is_absolute(), "HOME must retain its actual user-home meaning")
    directories = [str(Path(path).parent) for path in (cargo, rustc, cc, cxx, ar)]
    directories += [selection["llvm_directory"], "/usr/bin", "/bin"]
    environment = {"PATH": ":".join(dict.fromkeys(directories)), "HOME": home,
                   "CARGO_HOME": str(cargo_home), "RUSTC": str(rustc), "CC": str(cc),
                   "CXX": str(cxx), "AR": str(ar), "CARGO_TARGET_DIR": str(target),
                   "CARGO_BUILD_JOBS": "2", "CARGO_INCREMENTAL": "0", "CARGO_TERM_COLOR": "never",
                   "RUST_BACKTRACE": "0", "LC_ALL": "C", "TZ": "UTC", "TMPDIR": str(temporary),
                   "PYTHONDONTWRITEBYTECODE": "1", "OXID_LLVM_BIN": selection["llvm_directory"]}
    if evidence is not None:
        environment["OXID_OWNED_NATIVE_EVIDENCE"] = str(evidence)
    validate_environment(environment, selection)
    return environment


def run_child(argv, directory, *, cwd, environment, selection=None, timeout=1800,
              check=None, _after_launch=lambda process: None):
    """Retain separate complete logs; seal them only after every owned exit path."""
    require(platform.system() == "Linux", "producer process controller requires Linux")
    require(timeout > 0, "child timeout must be positive")
    directory = no_symlink_path(directory)
    directory.mkdir(parents=True, exist_ok=False)
    row = {"argv": [str(arg) for arg in argv], "cwd": str(cwd), "environment": dict(environment),
           "unset_controls": list(UNSET_CONTROLS), "timeout_seconds": timeout,
           "status": "NOT_RUN", "exit_code": None, "failure_code": 1}
    process = None
    error = None
    failed_code = 0
    started = time.monotonic()
    libc = ctypes.CDLL(None, use_errno=True)
    old_subreaper = ctypes.c_int()
    require(libc.prctl(37, ctypes.byref(old_subreaper), 0, 0, 0) == 0, "cannot query child subreaper")
    require(libc.prctl(36, 1, 0, 0, 0) == 0, "cannot enable owned-descendant reaping")
    def interrupt(signum, _frame):
        raise ChildFailure("controller interrupted by signal " + str(signum), 128 + signum)
    handlers = {sig: signal.signal(sig, interrupt) for sig in (signal.SIGINT, signal.SIGTERM)}
    try:
        with (directory / "stdout").open("xb") as stdout, (directory / "stderr").open("xb") as stderr:
            try:
                if selection is not None:
                    validate_environment(environment, selection)
                if check:
                    check()
                process = subprocess.Popen(row["argv"], cwd=cwd, env=environment,
                                           stdout=stdout, stderr=stderr, start_new_session=True)
                row["status"] = "RUNNING"
                _after_launch(process)
                try:
                    row["exit_code"] = process.wait(timeout=timeout)
                except subprocess.TimeoutExpired as exc:
                    row["status"] = "TIMEOUT"
                    raise ChildFailure("child timed out", 124) from exc
                if row["exit_code"]:
                    raise ChildFailure("child exited unsuccessfully", row["exit_code"] if row["exit_code"] > 0 else 128 - row["exit_code"])
                require(not group_alive(process.pid), "child left surviving owned descendants")
                if check:
                    check()
                row["status"] = "PASS"
            except (Exception, KeyboardInterrupt) as exc:
                error = f"{type(exc).__name__}: {exc}"
                failed_code = getattr(exc, "code", 130 if isinstance(exc, KeyboardInterrupt) else 1)
                if row["status"] != "TIMEOUT":
                    row["status"] = "FAIL"
            finally:
                # Ignore repeated termination only during bounded teardown.
                for sig in handlers:
                    signal.signal(sig, signal.SIG_IGN)
                if process is not None:
                    original = process.poll()
                    if row["exit_code"] is None and original is not None:
                        row["exit_code"] = original
                    try:
                        row["cleanup"] = stop_group(process)
                        row["cleanup_exit_code"] = process.returncode
                        if row["cleanup"]["surviving_group_detected"] or not row["cleanup"]["stopped"]:
                            row["status"] = "FAIL" if row["status"] != "TIMEOUT" else "TIMEOUT"
                            failed_code = failed_code or 1
                            error = error or "owned descendants survived direct-child termination"
                    except Exception as exc:
                        row["cleanup"] = {"stopped": False, "error": str(exc)}
                        row["status"] = "FAIL"
                        failed_code = failed_code or 1
                        error = error or "owned process cleanup failed"
                else:
                    row["cleanup"] = {"surviving_group_detected": False, "stopped": True, "launched": False}
                stdout.flush()
                stderr.flush()
        row.update(elapsed_seconds=time.monotonic() - started, error=error,
                   failure_code=failed_code, stdout=file_record(directory / "stdout"),
                   stderr=file_record(directory / "stderr"))
        write_json(directory / "command.json", row)
    except (Exception, KeyboardInterrupt) as exc:
        raise ChildFailure((error + "; " if error else "") + "command evidence sealing failed: " + str(exc),
                           failed_code or getattr(exc, "code", 1)) from exc
    finally:
        libc.prctl(36, old_subreaper.value, 0, 0, 0)
        for sig, handler in handlers.items():
            signal.signal(sig, handler)
    if failed_code:
        raise ChildFailure(error or "child failed", failed_code)
    return row


def validate_rust_version(data):
    values = dict(line.split(": ", 1) for line in data.decode("utf-8").splitlines() if ": " in line)
    require(values.get("release") == RUST_RELEASE and values.get("commit-hash") == RUST_COMMIT
            and values.get("host") == RUST_HOST, "Rust release, full commit, or host differs from reviewed pin")


def validate_llvm_version(data):
    require(re.search(rb"(?:LLVM|clang|LLD)(?: version)? 19\.1\.7(?:\s|$)", data) is not None,
            "LLVM version differs from reviewed 19.1.7 pin")


def check_cargo_config(repo, cargo_home, tracked):
    """Metadata only for unsupported config: contents and credentials stay unread."""
    locations = [(repo / ".cargo" / leaf, "repository") for leaf in ("config", "config.toml")]
    locations += [(parent / ".cargo" / leaf, "ancestor") for parent in repo.parents for leaf in ("config", "config.toml")]
    locations += [(cargo_home / leaf, "cargo-home") for leaf in ("config", "config.toml")]
    result = []
    for path, category in locations:
        no_symlink_path(path)
        try:
            value = path.lstat()
        except FileNotFoundError:
            result.append({"path": str(path), "category": category, "present": False})
            continue
        require(category == "repository" and path.relative_to(repo).as_posix() in tracked
                and stat.S_ISREG(value.st_mode), "unsupported Cargo configuration present: " + category + " " + str(path))
        result.append({"path": str(path), "category": category, "present": True,
                       "identity": list(stat_identity(value))})
    return result


def git_bytes(repo, *arguments):
    # Read-only metadata children use the same owned-group cleanup. Their short
    # transient output is not claimed as build/native execution evidence.
    with tempfile.TemporaryDirectory(prefix="unit2e-git-") as temporary:
        directory = Path(temporary) / "command"
        run_child(["/usr/bin/git", "-c", "safe.directory=" + str(repo), "-C", str(repo), *arguments],
                  directory, cwd=repo, environment={"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"],
                                                    "LC_ALL": "C", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"},
                  timeout=60)
        return (directory / "stdout").read_bytes()


def source_identity(repo, expected_head):
    require(re.fullmatch(r"[0-9a-f]{40}", expected_head) is not None, "expected HEAD must be a full Git object identity")
    head = git_bytes(repo, "rev-parse", "HEAD").decode().strip()
    require(head == expected_head, "checkout HEAD differs from expected head")
    tree = git_bytes(repo, "rev-parse", "HEAD^{tree}").decode().strip()
    entries = {}
    for item in git_bytes(repo, "ls-tree", "-rz", "HEAD").split(b"\0"):
        if not item:
            continue
        metadata, name = item.split(b"\t", 1)
        mode, kind, oid = metadata.decode().split()
        entries[name.decode("utf-8")] = {"git_mode": mode, "type": kind, "git_blob": oid}
    # Reject tracked Cargo credentials before Git status or a source snapshot
    # could hash their contents. ls-tree reads only tree membership metadata.
    require(not any(Path(name).name in ("credentials", "credentials.toml") and ".cargo" in Path(name).parts
                    for name in entries), "tracked Cargo credentials are unsupported")
    require(not git_bytes(repo, "status", "--porcelain=v1", "--untracked-files=no"), "tracked repository changes")
    return {"head": head, "tree": tree}, entries


def source_members(repo, tracked):
    current = read_json(repo / BINDING / "current-source.json")
    authority = read_json(repo / BINDING / "authority.json")
    names = {row["path"] for row in current["files"]}
    names.update(row["path"] for row in authority["repository_inputs"])
    prefixes = ("src/", "native/", "compiler/", "stdlib/", "rfcs/", ".cargo/", BINDING + "/")
    selected_names = {name for name in tracked if name.startswith(prefixes)}
    actual_names = set()
    for prefix in prefixes:
        directory = repo / prefix
        no_symlink_path(directory)
        if directory.exists():
            for parent, directories, files in os.walk(directory, followlinks=False, onerror=traversal_error):
                for leaf in directories + files:
                    require(not (Path(parent) / leaf).is_symlink(), "symlink compiler/source input")
                actual_names.update((Path(parent) / leaf).relative_to(repo).as_posix() for leaf in files)
    require(actual_names == selected_names, "compiler/source membership differs from committed inputs")
    names.update(selected_names)
    names.update(("Cargo.toml", "Cargo.lock", "build.rs", ".github/workflows/ci.yml",
                  "scripts/verify_owned_array_native.py", "scripts/test_owned_array_native.py",
                  "scripts/verify_owned_source_native.py", "scripts/verify_owned_source.py",
                  "scripts/preserve_unit3_ci_evidence.py", "docs/architecture/fixed-array-unit2e-native-ci.md"))
    require(not any(Path(name).name in ("credentials", "credentials.toml") for name in names),
            "credential files cannot be source or authority inputs")
    return current, sorted(names)


def capture_sources(repo, tracked):
    current, names = source_members(repo, tracked)
    rows = {}
    for name in names:
        require(name in tracked and tracked[name]["type"] == "blob" and tracked[name]["git_mode"] in ("100644", "100755"),
                "source input is not a tracked regular blob: " + name)
        data, mode = stable_bytes(repo / name)
        git_blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
        require(git_blob == tracked[name]["git_blob"], "source bytes differ from committed blob: " + name)
        require(bool(mode & 0o111) == (tracked[name]["git_mode"] == "100755"), "source executable mode differs from Git")
        rows[name] = {"bytes": len(data), "sha256": sha256(data), "mode": mode, "git_blob": git_blob}
    return {"files": rows, "reviewed_source_head": current["reviewed_source_head"],
            "source_only_tree": current["source_only_tree"],
            "current_source_sha256": rows[BINDING + "/current-source.json"]["sha256"]}


def verify_sources(repo, expected, inventory):
    identity, tracked = source_identity(repo, expected["head"])
    require(identity == expected, "repository HEAD/tree drift")
    require(capture_sources(repo, tracked) == inventory, "compiler/build/source authority inventory drift")


def ci_identity(expected_head, event_sha):
    require(re.fullmatch(r"[0-9a-f]{40}", event_sha) is not None, "event SHA must be a full Git identity")
    result = {name: os.environ.get(variable, "") for name, variable in CI_FIELDS.items()}
    match = re.fullmatch(r"refs/pull/(\d+)/(?:merge|head)", os.environ.get("GITHUB_REF", ""))
    result.update(expected_head=expected_head, event_sha=event_sha,
                  pr_number=int(match.group(1)) if match else None)
    return result


def safe_output(output, inputs):
    output = no_symlink_path(output)
    for item in inputs:
        item = no_symlink_path(item)
        require(output != item and output not in item.parents and item not in output.parents,
                "output overlaps an input/cache root")
    require(not output.exists(), "output must be fresh; existing evidence is preserved")
    return output


def built_test_binary(data, target):
    candidates = []
    for line in data.decode("utf-8").splitlines():
        item = json.loads(line)
        if (item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "oxid"
                and item.get("profile", {}).get("test") is True and item.get("executable")):
            candidates.append(Path(item["executable"]))
    require(len(candidates) == 1, "Cargo must identify exactly one oxid test binary")
    binary = no_symlink_path(candidates[0])
    require(binary.is_absolute() and target in binary.parents, "Cargo binary outside selected target directory")
    data, mode = stable_bytes(binary)
    elf_proof(data, mode)
    return binary, data, mode


def execute_run(args):
    require(sys.flags.optimize == 0 and os.environ.get("PYTHONOPTIMIZE", "0") in ("", "0"), "optimized Python is unsupported")
    require(platform.system() == "Linux" and platform.machine() == "x86_64", "producer requires Linux x86_64")
    repo = no_symlink_path(args.repo).resolve(strict=True)
    cargo_home = no_symlink_path(args.cargo_home).resolve(strict=True)
    output = safe_output(args.output, (repo, cargo_home))
    output.mkdir(parents=True)
    profiles = PROFILES if args.profile == "both" else (args.profile,)
    invocation = {"schema": "oxid-unit2e-producer-v1", "invocation_id": uuid.uuid4().hex,
                  "repository_path": str(repo), "output_root": str(output), "profiles": list(profiles),
                  "ci": ci_identity(args.expected_head, args.event_sha)}
    terminal = {"status": "RUNNING", "producer_only": True, "invocation_id": invocation["invocation_id"],
                "ci": invocation["ci"], "profiles": {profile: {"status": "NOT_RUN"} for profile in PROFILES}}
    write_json(output / "invocation.json", invocation)
    failure_code = 0
    active_profile = None
    try:
        identity, tracked = source_identity(repo, args.expected_head)
        configs = check_cargo_config(repo, cargo_home, tracked)
        source = capture_sources(repo, tracked)
        (output / "source/inputs").mkdir(parents=True)
        for name, row in source["files"].items():
            data, mode = stable_bytes(repo / name)
            require(sha256(data) == row["sha256"], "source changed before retention")
            destination = output / "source/inputs" / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(data)
            destination.chmod(mode)
        write_json(output / "source/identity.json", {**identity, **source, "cargo_config": configs})
        require(args.llvm_bin.is_absolute(), "--llvm-bin must select an absolute directory")
        llvm_directory = str(Path(args.llvm_bin).resolve(strict=True))
        require(Path(llvm_directory).is_dir(), "LLVM selection is not a directory")
        tools = {name: tool_identity(Path(llvm_directory) / name) for name in LLVM_TOOLS}
        tools.update({"cargo": tool_identity(args.cargo), "rustc": tool_identity(args.rustc),
                      "python": tool_identity(sys.executable)})
        for name, leaf in (("cc", "cc"), ("cxx", "c++"), ("ar", "ar")):
            selected = shutil.which(leaf, path="/usr/bin:/bin")
            require(selected is not None, "missing host build tool: " + leaf)
            tools[name] = tool_identity(selected)
        selection = {"llvm_directory": llvm_directory, "tools": {name: tools[name] for name in LLVM_TOOLS}}
        target = no_symlink_path(repo / "target")
        def boundary():
            verify_sources(repo, identity, source)
            require(check_cargo_config(repo, cargo_home, tracked) == configs, "Cargo config discovery changed")
            for row in tools.values():
                verify_tool_identity(row)
        with tempfile.TemporaryDirectory(prefix="unit2e-native-") as temporary:
            environment = child_environment(selection, cargo_home=cargo_home, home=os.environ["HOME"],
                cargo=tools["cargo"]["path"], rustc=tools["rustc"]["path"], cc=tools["cc"]["path"],
                cxx=tools["cxx"]["path"], ar=tools["ar"]["path"], target=target, temporary=temporary)
            run_child([tools["python"]["path"], "-B", str(repo / BINDING / "run.py"), "preflight",
                       "--repo", str(repo), "--output", str(output / "source-preflight")],
                      output / "preflight-command", cwd=repo, environment=environment,
                      selection=selection, check=boundary, timeout=180)
            prepared = read_json(output / "source-preflight/prepared.json")
            require(prepared.get("status") == "verified-inputs" and prepared.get("semantic_pass") is False
                    and prepared.get("current_source_sha256") == source["current_source_sha256"], "official source preflight failed admission")
            versions = {}
            for name, row in tools.items():
                arguments = [row["path"], "--version"] + (["--verbose"] if name == "rustc" else [])
                run_child(arguments, output / "tools" / name, cwd=repo, environment=environment,
                          selection=selection, check=boundary, timeout=60)
                stdout = (output / "tools" / name / "stdout").read_bytes()
                stderr = (output / "tools" / name / "stderr").read_bytes()
                if name in LLVM_TOOLS:
                    validate_llvm_version(stdout + stderr)
                if name == "rustc":
                    validate_rust_version(stdout)
                versions[name] = file_record(output / "tools" / name / "command.json")
            run_child([tools["rustc"]["path"], "--print", "target-libdir"], output / "tools/target-libdir",
                      cwd=repo, environment=environment, selection=selection, check=boundary, timeout=60)
            library_root = Path((output / "tools/target-libdir/stdout").read_text().strip())
            require(library_root.is_absolute(), "rustc standard library directory must be absolute")
            rlibs = list(library_root.glob("libstd-*.rlib"))
            shared = list(library_root.glob("libstd-*.so"))
            require(len(rlibs) == 1 and len(shared) <= 1, "ambiguous selected Rust standard library")
            require(not shared or shared[0].stem == rlibs[0].stem, "selected Rust standard library names do not match")
            stdlib = [{"path": str(path.resolve(strict=True)), **file_record(path.resolve(strict=True))} for path in rlibs + shared]
            toolchains = {**selection, "all_tools": tools, "version_receipts": versions,
                          "stdlib": stdlib, "effective_environment": environment, "unset_controls": list(UNSET_CONTROLS)}
            write_json(output / "toolchains.json", toolchains)
            binding = {**identity, "invocation_id": invocation["invocation_id"],
                       "source_identity_sha256": file_record(output / "source/identity.json")["sha256"],
                       "current_source_sha256": source["current_source_sha256"],
                       "toolchains_sha256": file_record(output / "toolchains.json")["sha256"]}
            terminal["binding"] = binding
            for profile in profiles:
                active_profile = profile
                root = output / profile
                root.mkdir()
                terminal["profiles"][profile] = {"status": "RUNNING"}
                (root / "artifacts").mkdir()
                child_env = {**environment, "OXID_OWNED_NATIVE_EVIDENCE": str(root / "artifacts")}
                argv = [tools["cargo"]["path"], "test", "--locked", "--offline", "--bin", "oxid", "--no-run", "--message-format=json", "--jobs", "2"]
                if profile == "release":
                    argv.append("--release")
                run_child(argv, root / "build", cwd=repo, environment=child_env,
                          selection=selection, check=boundary, timeout=1800)
                binary, data, mode = built_test_binary((root / "build/stdout").read_bytes(), target / profile)
                (root / "bin").mkdir()
                copied = root / "bin/oxid-test"
                copied.write_bytes(data)
                copied.chmod(mode)
                binary_record = file_record(copied)
                require(file_record(binary) == binary_record, "build executable changed during copy")
                def profile_boundary():
                    boundary()
                    require(file_record(copied) == binary_record, "copied test executable drift")
                    for row in stdlib:
                        require(file_record(row["path"]) == {k: row[k] for k in ("bytes", "mode", "sha256")}, "selected standard library drift")
                run_child([str(copied), PREFIX, "--list", "--ignored", "--format", "terse", "--color", "never"],
                          root / "list", cwd=repo, environment=child_env, selection=selection,
                          check=profile_boundary, timeout=60)
                roster = admit_list((root / "list/stdout").read_bytes())
                write_json(root / "roster.json", {"profile": profile, "binding": binding, "names": roster,
                                                 "test_binary": binary_record})
                run_child([str(copied), PREFIX, "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"],
                          root / "run", cwd=repo, environment=child_env, selection=selection,
                          check=profile_boundary, timeout=3600)
                outcomes = admit_stdout((root / "run/stdout").read_bytes())
                families = admit_stderr((root / "run/stderr").read_bytes())
                artifacts = admit_artifacts(root / "artifacts")
                write_json(root / "artifact-manifest.json", artifacts)
                profile_boundary()
                result = {"status": "PASS", "profile": profile, "binding": binding, "test_binary": binary_record,
                          "build_binary": {"path": str(binary), **binary_record},
                          "outcomes": outcomes, "families": families, "array_artifact_files": 1307,
                          "elf_executions_asserted": 7058, "reference_comparisons_derived": 6793,
                          "artifact_manifest_sha256": file_record(root / "artifact-manifest.json")["sha256"],
                          "command_receipts": {name: file_record(root / name / "command.json") for name in ("build", "list", "run")}}
                write_json(root / "result.json", result)
                terminal["profiles"][profile] = {"status": "PASS", "result": file_record(root / "result.json")}
                active_profile = None
            boundary()
        terminal.update(status="PASS", exit_code=0)
    except (Exception, KeyboardInterrupt) as exc:
        failure_code = getattr(exc, "code", 130 if isinstance(exc, KeyboardInterrupt) else 1)
        terminal.update(status="FAIL", exit_code=failure_code, error=f"{type(exc).__name__}: {exc}")
        if active_profile is not None:
            terminal["profiles"][active_profile] = {"status": "FAIL", "error": terminal["error"]}
            write_json(output / active_profile / "failure.json", terminal["profiles"][active_profile])
    finally:
        try:
            write_json(output / ("result.json" if terminal["status"] == "PASS" else "failure.json"), terminal)
            write_json(output / "evidence-manifest.json", regular_inventory(output))
        except (Exception, KeyboardInterrupt) as exc:
            print(f"producer evidence sealing failed: {type(exc).__name__}: {exc}", file=sys.stderr)
            failure_code = failure_code or 1
    return failure_code


import tarfile
import gzip

INDEX_NAME = "fixed-array-unit2e-index.json"
ARCHIVE_NAME = "fixed-array-unit2e-evidence.tar.gz"
COMPACT_NAME = "fixed-array-unit2e-compact.tar.gz"
COMPACT_LIMIT = 32 * 1024 * 1024


def command_admission(root, selection, expected_environment=None, expected_argv=None, expected_cwd=None):
    row = read_json(root / "command.json")
    require(row["status"] == "PASS" and row["exit_code"] == 0 and row["failure_code"] == 0,
            "unsuccessful original command: " + str(root))
    require(row["cleanup"]["stopped"] and not row["cleanup"]["surviving_group_detected"], "incomplete owned-group cleanup")
    require(row["unset_controls"] == list(UNSET_CONTROLS), "command unset-control receipt changed")
    # Package does not execute versions again; selection bytes are checked by the
    # caller, while this receipt binds the environment actually passed to Popen.
    require(row["environment"].get("OXID_LLVM_BIN") == selection["llvm_directory"], "command LLVM directory binding changed")
    require(all(name not in row["environment"] for name in UNSET_CONTROLS)
            and not any(name.startswith("CLANG_CONFIG_FILE_") for name in row["environment"]), "command runtime unset state changed")
    if expected_environment is not None:
        require(row["environment"] == expected_environment, "effective command environment drift")
    if expected_argv is not None:
        require(row["argv"] == expected_argv, "original command invocation drift")
    if expected_cwd is not None:
        require(row["cwd"] == str(expected_cwd), "original command working directory drift")
    for stream in ("stdout", "stderr"):
        require(file_record(root / stream) == row[stream], "original command log drift")
    return row


def verify_producer(root, ci, upstream_outcome):
    require(upstream_outcome == "success", "upstream producer step did not succeed: " + upstream_outcome)
    invocation = read_json(root / "invocation.json")
    require(invocation["schema"] == "oxid-unit2e-producer-v1" and invocation["ci"] == ci, "stale invocation run/head/event tuple")
    require(invocation["profiles"] == list(PROFILES), "both profiles are required for CI packaging")
    require(str(root) == invocation["output_root"], "wrong producer evidence root")
    terminal = read_json(root / "result.json")
    require(not (root / "failure.json").exists() and terminal["status"] == "PASS" and terminal["exit_code"] == 0,
            "missing successful terminal producer receipt")
    require(terminal["invocation_id"] == invocation["invocation_id"] and terminal["ci"] == ci, "terminal invocation drift")
    source = read_json(root / "source/identity.json")
    identity = {key: source[key] for key in ("head", "tree")}
    inventory = {key: source[key] for key in ("files", "reviewed_source_head", "source_only_tree", "current_source_sha256")}
    require(identity["head"] == ci["expected_head"], "source HEAD differs from run head")
    verify_sources(Path(invocation["repository_path"]), identity, inventory)
    for name, expected in inventory["files"].items():
        require(file_record(root / "source/inputs" / name) == {k: expected[k] for k in ("bytes", "mode", "sha256")}, "retained source input drift")
    toolchains = read_json(root / "toolchains.json")
    require(set(toolchains["tools"]) == set(LLVM_TOOLS)
            and set(toolchains["all_tools"]) == set((*LLVM_TOOLS, "cargo", "rustc", "python", "cc", "cxx", "ar")),
            "tool identity roster differs")
    require(all(toolchains["tools"][name] == toolchains["all_tools"][name] for name in LLVM_TOOLS),
            "LLVM identities differ between selection and full receipts")
    require(toolchains["unset_controls"] == list(UNSET_CONTROLS), "toolchain runtime recipe drift")
    validate_environment(toolchains["effective_environment"], toolchains)
    for row in toolchains["all_tools"].values():
        verify_tool_identity(row)
    libraries = [Path(row["path"]) for row in toolchains["stdlib"]]
    rlibs = [path for path in libraries if path.name.startswith("libstd-") and path.suffix == ".rlib"]
    shared = [path for path in libraries if path.name.startswith("libstd-") and path.suffix == ".so"]
    require(len(rlibs) == 1 and len(shared) <= 1 and len(libraries) == len(rlibs) + len(shared)
            and (not shared or shared[0].stem == rlibs[0].stem), "selected standard library receipt roster differs")
    for row in toolchains["stdlib"]:
        require(file_record(row["path"]) == {k: row[k] for k in ("bytes", "mode", "sha256")}, "selected standard library drift")
    require(check_cargo_config(Path(invocation["repository_path"]), Path(toolchains["effective_environment"]["CARGO_HOME"]), inventory["files"]) == source["cargo_config"], "Cargo config receipt drift")
    prepared = read_json(root / "source-preflight/prepared.json")
    require(prepared["status"] == "verified-inputs" and prepared["semantic_pass"] is False
            and prepared["current_source_sha256"] == inventory["current_source_sha256"], "source preflight identity drift")
    environment = toolchains["effective_environment"]
    repository = Path(invocation["repository_path"])
    command_admission(root / "preflight-command", toolchains, environment,
                      [toolchains["all_tools"]["python"]["path"], "-B", str(repository / BINDING / "run.py"),
                       "preflight", "--repo", str(repository), "--output", str(root / "source-preflight")], repository)
    for name in toolchains["all_tools"]:
        argv = [toolchains["all_tools"][name]["path"], "--version"] + (["--verbose"] if name == "rustc" else [])
        command_admission(root / "tools" / name, toolchains, environment, argv, repository)
        require(file_record(root / "tools" / name / "command.json") == toolchains["version_receipts"][name], "tool version receipt drift")
        data = (root / "tools" / name / "stdout").read_bytes()
        if name in LLVM_TOOLS:
            validate_llvm_version(data + (root / "tools" / name / "stderr").read_bytes())
        if name == "rustc":
            validate_rust_version(data)
    command_admission(root / "tools/target-libdir", toolchains, environment,
                      [toolchains["all_tools"]["rustc"]["path"], "--print", "target-libdir"], repository)
    library_directory = Path((root / "tools/target-libdir/stdout").read_text().strip()).resolve(strict=True)
    require(all(path.parent == library_directory for path in libraries), "standard libraries differ from admitted rustc directory")
    binding = {**identity, "invocation_id": invocation["invocation_id"],
               "source_identity_sha256": file_record(root / "source/identity.json")["sha256"],
               "current_source_sha256": inventory["current_source_sha256"],
               "toolchains_sha256": file_record(root / "toolchains.json")["sha256"]}
    require(terminal["binding"] == binding, "terminal source/tool binding changed")
    profiles = {}
    require(set(terminal["profiles"]) == set(PROFILES), "terminal profile roster differs")
    for profile in PROFILES:
        directory = root / profile
        result = read_json(directory / "result.json")
        require(not (directory / "failure.json").exists(), "profile has failure evidence")
        require(terminal["profiles"][profile] == {"status": "PASS", "result": file_record(directory / "result.json")}, "profile result receipt drift")
        require(result["status"] == "PASS" and result["profile"] == profile and result["binding"] == binding,
                "stale source/run/profile binding")
        binary = file_record(directory / "bin/oxid-test")
        require(binary == result["test_binary"], "copied test binary drift")
        elf_proof(*stable_bytes(directory / "bin/oxid-test"))
        expected_environment = {**environment, "OXID_OWNED_NATIVE_EVIDENCE": str(directory / "artifacts")}
        commands = {name: command_admission(directory / name, toolchains, expected_environment, expected_cwd=repository) for name in ("build", "list", "run")}
        for name in commands:
            require(file_record(directory / name / "command.json") == result["command_receipts"][name], "profile command receipt drift")
        expected_build = [toolchains["all_tools"]["cargo"]["path"], "test", "--locked", "--offline", "--bin", "oxid", "--no-run", "--message-format=json", "--jobs", "2"]
        if profile == "release":
            expected_build.append("--release")
        require(commands["build"]["argv"] == expected_build, "profile build invocation differs")
        messages = [json.loads(line) for line in (directory / "build/stdout").read_text().splitlines()]
        built_paths = [item["executable"] for item in messages if item.get("reason") == "compiler-artifact"
                       and item.get("target", {}).get("name") == "oxid"
                       and item.get("profile", {}).get("test") is True and item.get("executable")]
        require(len(built_paths) == 1 and result["build_binary"] == {"path": built_paths[0], **binary},
                "copied test binary lacks exact Cargo artifact provenance")
        require(Path(environment["CARGO_TARGET_DIR"]) / profile in Path(built_paths[0]).parents,
                "Cargo artifact outside selected profile target")
        copied = str(directory / "bin/oxid-test")
        require(commands["list"]["argv"] == [copied, PREFIX, "--list", "--ignored", "--format", "terse", "--color", "never"], "roster invocation differs")
        require(commands["run"]["argv"] == [copied, PREFIX, "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"], "broad-prefix invocation differs")
        roster = read_json(directory / "roster.json")
        require(roster == {"profile": profile, "binding": binding, "names": admit_list((directory / "list/stdout").read_bytes()), "test_binary": binary}, "stale roster receipt")
        require(result["outcomes"] == admit_stdout((directory / "run/stdout").read_bytes()), "attributed stdout outcome drift")
        require(result["families"] == admit_stderr((directory / "run/stderr").read_bytes()), "stderr family receipt drift")
        require(result["array_artifact_files"] == 1307 and result["elf_executions_asserted"] == 7058
                and result["reference_comparisons_derived"] == 6793, "producer count/label drift")
        require(file_record(directory / "artifact-manifest.json")["sha256"] == result["artifact_manifest_sha256"], "artifact manifest drift")
        require(read_json(directory / "artifact-manifest.json") == admit_artifacts(directory / "artifacts"), "artifact members or bytes changed")
        profiles[profile] = {"status": "PASS", "result": file_record(directory / "result.json"),
                             "artifact_manifest_sha256": result["artifact_manifest_sha256"],
                             "families": result["families"], "array_artifact_files": 1307,
                             "elf_executions_asserted": 7058, "reference_comparisons_derived": 6793}
    expected = read_json(root / "evidence-manifest.json")
    actual = regular_inventory(root)
    actual.pop("evidence-manifest.json")
    require(actual == expected, "terminal evidence membership or bytes changed")
    return {"ci": ci, "binding": binding, "profiles": profiles}


def snapshot_inventory(root, issues):
    """Collect recoverable members; every traversal failure makes export incomplete."""
    rows = {}
    try:
        root = no_symlink_path(root)
        require(root.is_dir(), "producer output missing")
    except (OSError, AdmissionError) as exc:
        issues.append(str(exc))
        return rows
    def failed(error):
        issues.append(f"inventory: {type(error).__name__}: {error}")
    for directory, directories, files in os.walk(root, followlinks=False, onerror=failed):
        for name in list(directories):
            path = Path(directory) / name
            if path.is_symlink():
                issues.append("symlink evidence directory: " + str(path))
                directories.remove(name)
        for name in files:
            path = Path(directory) / name
            try:
                no_symlink_path(path)
                value = path.lstat()
                require(stat.S_ISREG(value.st_mode), "nonregular evidence member: " + str(path))
                rows[path.relative_to(root).as_posix()] = stat_identity(value)
            except (OSError, AdmissionError) as exc:
                failed(exc)
    return rows


def verify_archive(path, entries):
    # Readback and compressed-byte digest share one anchored file descriptor.
    # A replacement during either operation fails the stable_file exit check.
    with stable_file(path) as (compressed, before):
        with tarfile.open(fileobj=compressed, mode="r:gz") as archive:
            names = archive.getnames()
            exact_inventory(names, list(entries), "archive members")
            for member in archive:
                require(member.isfile(), "nonregular archive member")
                digest = hashlib.sha256()
                with archive.extractfile(member) as handle:
                    while chunk := handle.read(1024 * 1024):
                        digest.update(chunk)
                expected = entries[member.name]
                require(member.size == expected["bytes"] and member.mode == expected["mode"]
                        and digest.hexdigest() == expected["sha256"], "archive member checksum mismatch")
        compressed.seek(0)
        digest = hashlib.sha256()
        remaining = before.st_size
        while remaining:
            chunk = compressed.read(min(1024 * 1024, remaining))
            require(bool(chunk), "archive truncated during digest")
            remaining -= len(chunk)
            digest.update(chunk)
        require(not compressed.read(1), "archive grew during digest")
        record = {"bytes": before.st_size, "mode": stat.S_IMODE(before.st_mode), "sha256": digest.hexdigest()}
    return record


def write_archive(snapshot, destination, entries):
    # Fixed gzip and tar metadata make identical captured bytes reproducible.
    with destination.open("xb") as raw, gzip.GzipFile(fileobj=raw, filename="", mode="wb", compresslevel=6, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for name, row in sorted(entries.items()):
                info = tarfile.TarInfo(name)
                info.size, info.mode = row["bytes"], row["mode"]
                with (snapshot / name).open("rb") as handle:
                    archive.addfile(info, handle)


def compact_member(name):
    """Explicit audit roster, never a blanket cache or size exclusion."""
    if name == "membership.json":
        return True
    require(name.startswith("producer/"), "unexpected compact evidence namespace")
    relative = name.removeprefix("producer/")
    fixed = {"invocation.json", "result.json", "failure.json", "evidence-manifest.json",
             "source/identity.json", "toolchains.json", "source-preflight/prepared.json",
             "source-preflight/failure.json", "source-preflight/plan.json"}
    fixed.update("source/inputs/" + BINDING + "/" + leaf
                 for leaf in ("authority.json", "package-manifest.json", "current-source.json"))
    if relative in fixed:
        return True
    parts = Path(relative).parts
    if len(parts) == 2 and parts[0] == "preflight-command":
        return parts[1] in ("command.json", "stdout", "stderr")
    if len(parts) == 3 and parts[0] == "tools":
        return parts[2] in ("command.json", "stdout", "stderr")
    if parts[0] in PROFILES:
        if len(parts) == 2:
            return parts[1] in ("result.json", "failure.json", "roster.json", "artifact-manifest.json")
        if len(parts) == 3 and parts[1] in ("build", "list", "run"):
            return parts[2] in ("command.json", "stdout", "stderr")
        return False
    return False


def package_evidence(root, output, ci, upstream_outcome, *, _observe=lambda phase, name: None):
    root = Path(os.path.abspath(root))
    output = safe_output(output, (root,))
    output.mkdir(parents=True)
    issues = []
    summary = {"status": "INCOMPLETE", "producer_only": True, "complete_unit2e_qualification": False,
               "ci": ci, "producer_step_outcome": upstream_outcome, "errors": issues}
    validation = None
    try:
        validation = verify_producer(root, ci, upstream_outcome)
    except (Exception, KeyboardInterrupt) as exc:
        issues.append(f"producer admission: {type(exc).__name__}: {exc}")
    try:
        with tempfile.TemporaryDirectory(prefix="unit2e-snapshot-", dir=output) as temporary:
            snapshot = Path(temporary)
            selected = snapshot_inventory(root, issues)
            _observe("inventory", "")
            entries = {}
            for name, expected in selected.items():
                destination = snapshot / "producer" / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                try:
                    require(stat_identity((root / name).lstat()) == expected, "member changed before copy: " + name)
                    _observe("capture", name)
                    with destination.open("xb") as handle:
                        row = capture_file(root / name, handle)
                    _observe("after-capture", name)
                    require(stat_identity((root / name).lstat()) == expected, "member changed during copy: " + name)
                    destination.chmod(row["mode"])
                    entries["producer/" + name] = row
                except (OSError, AdmissionError) as exc:
                    issues.append(f"capture {name}: {type(exc).__name__}: {exc}")
            final = snapshot_inventory(root, issues)
            if final != selected:
                issues.append("evidence membership or identity changed during snapshot")
            if validation is not None and not issues:
                try:
                    require(verify_producer(root, ci, upstream_outcome) == validation, "producer changed during snapshot")
                except (Exception, KeyboardInterrupt) as exc:
                    issues.append(f"terminal revalidation: {type(exc).__name__}: {exc}")
            if validation is not None and not issues:
                # Original admission plus unchanged metadata must also match the
                # copied byte manifest; no reopening of live files builds the tar.
                recorded = read_json(snapshot / "producer/evidence-manifest.json")
                for name, expected in recorded.items():
                    if entries.get("producer/" + name) != expected:
                        issues.append("copied evidence does not match terminal hashes: " + name)
                if not issues:
                    summary.update(validation, status="COMPLETE")
            summary["members"] = entries
            details = json_bytes({"status": summary["status"], "errors": issues, "members": entries})
            (snapshot / "membership.json").write_bytes(details)
            entries["membership.json"] = file_record(snapshot / "membership.json")
            archive_path = output / ARCHIVE_NAME
            _observe("archive", "")
            write_archive(snapshot, archive_path, entries)
            _observe("verify-archive", "")
            archive_record = verify_archive(archive_path, entries)
            checksum = archive_record["sha256"]
            (output / (ARCHIVE_NAME + ".sha256")).write_text(checksum + "  " + ARCHIVE_NAME + "\n")
            summary["archive_sha256"] = checksum
            compact = {name: row for name, row in entries.items() if compact_member(name)}
            compact_manifest = json_bytes({"status": summary["status"], "members": compact})
            (snapshot / "compact-membership.json").write_bytes(compact_manifest)
            compact["compact-membership.json"] = file_record(snapshot / "compact-membership.json")
            write_archive(snapshot, output / COMPACT_NAME, compact)
            compact_record = verify_archive(output / COMPACT_NAME, compact)
            require(compact_record["bytes"] <= COMPACT_LIMIT, "compact transport exceeds 32 MiB; full evidence is preserved without pruning")
            (output / (COMPACT_NAME + ".sha256")).write_text(compact_record["sha256"] + "  " + COMPACT_NAME + "\n")
            summary["compact"] = {**compact_record, "member_names": sorted(compact), "limit_bytes": COMPACT_LIMIT}
            require(file_record(archive_path) == archive_record and file_record(output / COMPACT_NAME) == compact_record,
                    "verified archive changed before terminal packaging")
    except (Exception, KeyboardInterrupt) as exc:
        issues.append(f"preservation: {type(exc).__name__}: {exc}")
        summary["status"] = "INCOMPLETE"
    if issues:
        summary["status"] = "INCOMPLETE"
    # Keep the standalone index compact; the complete membership is in the tar.
    summary.pop("members", None)
    write_json(output / INDEX_NAME, summary)
    return 0 if summary["status"] == "COMPLETE" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    run = actions.add_parser("run")
    run.add_argument("--repo", required=True, type=Path)
    run.add_argument("--output", required=True, type=Path)
    run.add_argument("--cargo", required=True)
    run.add_argument("--rustc", required=True)
    run.add_argument("--cargo-home", required=True, type=Path)
    run.add_argument("--llvm-bin", required=True, type=Path)
    run.add_argument("--profile", choices=(*PROFILES, "both"), default="both")
    package = actions.add_parser("package")
    package.add_argument("--producer-root", required=True, type=Path)
    package.add_argument("--output", required=True, type=Path)
    package.add_argument("--producer-step-outcome", required=True)
    for action in (run, package):
        action.add_argument("--expected-head", required=True)
        action.add_argument("--event-sha", required=True)
    args = parser.parse_args()
    try:
        require(sys.flags.optimize == 0 and os.environ.get("PYTHONOPTIMIZE", "0") in ("", "0"), "optimized Python is unsupported")
        if args.action == "run":
            return execute_run(args)
        return package_evidence(args.producer_root, args.output,
                                ci_identity(args.expected_head, args.event_sha), args.producer_step_outcome)
    except (Exception, KeyboardInterrupt) as exc:
        print(f"Unit2E producer boundary failed: {type(exc).__name__}: {exc}", file=sys.stderr)
        return getattr(exc, "code", 1)


if __name__ == "__main__":
    raise SystemExit(main())
