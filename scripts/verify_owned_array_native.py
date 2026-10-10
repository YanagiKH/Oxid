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
# These current tests share the historical module prefix but have dedicated CI
# gates. Discovery remains closed; producer execution retains its original roster.
SLICE_ROSTER = (
    PREFIX + "slices::native_slices_acyclic_phi_and_rhs_failure_use_source_free_llvm",
    PREFIX + "slices::native_slices_checkpoint_and_mutation_use_source_free_llvm",
    PREFIX + "slices::native_slices_signed_bounds_and_fuel_use_source_free_llvm",
)
COMPOSITION_ROSTER = (
    PREFIX + "composition::native_composition_projected_array_bounds_fuel_and_rhs_precedence",
    PREFIX + "composition::native_composition_raw_empty_sentinels_source_free",
    PREFIX + "composition::native_composition_raw_pilot_source_free_every_fuel",
    PREFIX + "composition::native_composition_source_free_depth_sixty_four",
    PREFIX + "composition::native_composition_source_free_pilot_sentinels_effects_and_phi",
)
PROJECTED_SLICE_ROSTER = (
    PREFIX + "projected_slices::native_projected_slices_source_free_mutation_every_fuel",
    PREFIX + "projected_slices::native_projected_slices_source_free_mutation_metadata_and_forwarding",
    PREFIX + "projected_slices::native_projected_slices_source_free_signed_bounds_and_fuel",
)
ENUM_ROSTER = (
    PREFIX + "enums::native_enums_source_free_all_payloads_orders_sites_loops_and_every_fuel",
    PREFIX + "enums::native_enums_source_free_all_transfer_faults_and_no_partial_write",
    PREFIX + "enums::native_enums_source_free_changed_variants_and_later_owned_input_boundary",
    PREFIX + "enums::native_enums_source_free_discard_and_inactive_moved_uninitialized_poison",
    PREFIX + "enums::native_enums_source_free_guards_precede_every_binding_and_transfer_effect",
    PREFIX + "enums::native_enums_source_free_invalid_dispatch_consume_and_active_bytes",
)
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


def strict_json(data):
    def pairs(items):
        value = {}
        for key, item in items:
            require(key not in value, "duplicate JSON key: " + key)
            value[key] = item
        return value
    return json.loads(data, object_pairs_hook=pairs)


def read_json(path):
    return strict_json(stable_bytes(path)[0])


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


def admit_discovery(data):
    """Reject drift in the broad current prefix before selecting the frozen 16."""
    names = []
    footer = False
    for line in data.decode("utf-8").splitlines():
        if not line:
            continue
        if line == "33 tests, 0 benchmarks" and not footer:
            footer = True
        else:
            require(not footer and line.endswith(": test"), "unknown or misplaced discovery line")
            names.append(line[:-6])
    require(footer, "missing discovery footer")
    exact_inventory(names, (*ROSTER, *SLICE_ROSTER, *COMPOSITION_ROSTER, *PROJECTED_SLICE_ROSTER, *ENUM_ROSTER), "current ignored prefix roster")
    return names


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
    prefixes = ("src/", "native/", "compiler/", "stdlib/", "rfcs/", ".cargo/", BINDING + "/",
                "tests/fixtures/fixed_array_unit2d_independent/",
                "tests/qualification/lexer_reservation_current/",
                "tests/fixtures/typed_project_source_binding_byte_storage_v1/",
                "tests/qualification/unit2_u8_current/",
                "tests/fixtures/typed_project_unit2_independent/",
                "fixtures/typed-streaming-lexer/")
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
                  "scripts/preserve_unit3_ci_evidence.py", "docs/architecture/fixed-array-unit2e-native-ci.md",
                  "scripts/replay_fixed_array_unit2d.py", "scripts/replay_fixed_array_unit2d_current.py",
                  "scripts/replay_unit2d_tool_capture.py",
                  "scripts/test_replay_fixed_array_unit2d.py",
                  "scripts/build_streaming_lexer.py", "scripts/build_hir_producers_v2.py",
                  "fixtures/typed-frontend-v2/sources.json",
                  "tests/fixtures/producer_diagnostic/provenance.json"))
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
            write_json(output / "independent-llvm-tools.json", {name: tools[name]["path"] for name in INDEPENDENT_TOOLS})
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
                run_child([str(copied), PREFIX, "--list", "--ignored", "--format", "pretty", "--color", "never"],
                          root / "discovery", cwd=repo, environment=child_env, selection=selection,
                          check=profile_boundary, timeout=60)
                admit_discovery((root / "discovery/stdout").read_bytes())
                run_child([str(copied), *ROSTER, "--exact", "--list", "--ignored", "--format", "pretty", "--color", "never"],
                          root / "list", cwd=repo, environment=child_env, selection=selection,
                          check=profile_boundary, timeout=60)
                roster = admit_list((root / "list/stdout").read_bytes())
                write_json(root / "roster.json", {"profile": profile, "binding": binding, "names": roster,
                                                 "test_binary": binary_record})
                run_child([str(copied), *ROSTER, "--exact", "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"],
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
                          "command_receipts": {name: file_record(root / name / "command.json") for name in ("build", "discovery", "list", "run")}}
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
    require(read_json(root / "independent-llvm-tools.json") == {name: toolchains["all_tools"][name]["path"] for name in INDEPENDENT_TOOLS},
            "independent four-tool map differs from admitted tool selection")
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
        commands = {name: command_admission(directory / name, toolchains, expected_environment, expected_cwd=repository) for name in ("build", "discovery", "list", "run")}
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
        require(commands["discovery"]["argv"] == [copied, PREFIX, "--list", "--ignored", "--format", "pretty", "--color", "never"], "discovery invocation differs")
        admit_discovery((directory / "discovery/stdout").read_bytes())
        require(commands["list"]["argv"] == [copied, *ROSTER, "--exact", "--list", "--ignored", "--format", "pretty", "--color", "never"], "roster invocation differs")
        require(commands["run"]["argv"] == [copied, *ROSTER, "--exact", "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"], "exact-roster invocation differs")
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


def compressed_record(stream, before):
    stream.seek(0)
    digest = hashlib.sha256()
    remaining = before.st_size
    while remaining:
        chunk = stream.read(min(1024 * 1024, remaining))
        require(bool(chunk), "archive truncated during digest")
        remaining -= len(chunk)
        digest.update(chunk)
    require(not stream.read(1), "archive grew during digest")
    stream.seek(0)
    return {"bytes": before.st_size, "mode": stat.S_IMODE(before.st_mode), "sha256": digest.hexdigest()}


def verify_archive(path, entries):
    # Readback and compressed-byte digest share one anchored file descriptor.
    # A replacement during either operation fails the stable_file exit check.
    with stable_file(path) as (compressed, before):
        with tarfile.open(fileobj=compressed, mode="r:gz") as archive:
            names = archive.getnames()
            exact_inventory(names, list(entries), "archive members")
            for member in archive:
                expected = entries[member.name]
                kind = expected.get("kind", "file")
                require(member.mode == expected["mode"], "archive mode mismatch")
                if kind == "directory":
                    require(member.isdir(), "archive directory type mismatch")
                    continue
                if kind == "symlink":
                    require(member.issym() and member.linkname == expected["target"], "archive symlink target mismatch")
                    continue
                require(kind == "file" and member.isfile(), "nonregular archive member")
                digest = hashlib.sha256()
                with archive.extractfile(member) as handle:
                    while chunk := handle.read(1024 * 1024):
                        digest.update(chunk)
                expected = entries[member.name]
                require(member.size == expected["bytes"] and member.mode == expected["mode"]
                        and digest.hexdigest() == expected["sha256"], "archive member checksum mismatch")
        record = compressed_record(compressed, before)
    return record


def write_archive(snapshot, destination, entries):
    # Fixed gzip and tar metadata make identical captured bytes reproducible.
    with destination.open("xb") as raw, gzip.GzipFile(fileobj=raw, filename="", mode="wb", compresslevel=6, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as archive:
            for name, row in sorted(entries.items()):
                info = tarfile.TarInfo(name)
                info.mode = row["mode"]
                if row.get("kind") == "directory":
                    info.type = tarfile.DIRTYPE
                    archive.addfile(info)
                elif row.get("kind") == "symlink":
                    info.type, info.linkname = tarfile.SYMTYPE, row["target"]
                    archive.addfile(info)
                else:
                    info.size = row["bytes"]
                    with stable_file(snapshot / name) as (handle, before):
                        require(before.st_size == row["bytes"] and stat.S_IMODE(before.st_mode) == row["mode"],
                                "private captured snapshot changed before archive")
                        archive.addfile(info, handle)


def compact_member(name):
    """Explicit audit roster, never a blanket cache or size exclusion."""
    if name == "membership.json":
        return True
    require(name.startswith("producer/"), "unexpected compact evidence namespace")
    relative = name.removeprefix("producer/")
    fixed = {"invocation.json", "result.json", "failure.json", "evidence-manifest.json",
             "source/identity.json", "toolchains.json", "independent-llvm-tools.json", "source-preflight/prepared.json",
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
        if len(parts) == 3 and parts[1] in ("build", "discovery", "list", "run"):
            return parts[2] in ("command.json", "stdout", "stderr")
        return False
    return False


def package_evidence(root, output, ci, upstream_outcome, *, independent=None, _observe=lambda phase, name: None):
    root = Path(os.path.abspath(root))
    independent = {} if independent is None else independent
    extra_roots = tuple(value["root"] for value in independent.values())
    output = safe_output(output, (root, *extra_roots))
    output.mkdir(parents=True)
    issues = []
    summary = {"status": "INCOMPLETE", "producer_only": True, "complete_unit2e_qualification": False,
               "ci": ci, "producer_step_outcome": upstream_outcome, "errors": issues}
    validation = None
    independent_validation = {}
    try:
        validation = verify_producer(root, ci, upstream_outcome)
    except (Exception, KeyboardInterrupt) as exc:
        issues.append(f"producer admission: {type(exc).__name__}: {exc}")
    if independent:
        summary["producer_only"] = False
        summary["independent_step_outcomes"] = {profile: value["outcome"] for profile, value in independent.items()}
        if set(independent) != set(PROFILES):
            issues.append("both independent profile roots and outcomes are required")
        for profile, value in independent.items():
            try:
                require(validation is not None, "producer admission unavailable for independent source/tool join")
                independent_validation[profile] = verify_independent_invocation(value["root"], root, profile, ci, value["outcome"], validation)
            except (Exception, KeyboardInterrupt) as exc:
                issues.append(f"independent {profile} admission: {type(exc).__name__}: {exc}")
        summary["independent"] = independent_validation
        summary["independent_exclusions"] = ["evidence/temporary (frozen runner seal exclusion)",
            "source (materialized checkout; source.tar, original-source and prepared/source-binding bodies retained)",
            "target only after the required copied marked test ELF is verified; otherwise surviving partial target retained"]
    try:
        with tempfile.TemporaryDirectory(prefix="unit2e-snapshot-", dir=output) as temporary, contextlib.ExitStack() as anchors:
            snapshot = Path(temporary)
            def hold_root(path):
                try:
                    anchors.enter_context(anchored_directory(path))
                except (OSError, AdmissionError) as exc:
                    issues.append(f"export ancestor binding: {type(exc).__name__}: {exc}")
            hold_root(root)
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
            for profile, value in independent.items():
                independent_root = value["root"]
                for owned_root in (independent_root, independent_root / "evidence", independent_root / "inputs"):
                    hold_root(owned_root)
                selected_independent = independent_export_inventory(independent_root, issues)
                for name, expected in selected_independent.items():
                    destination = snapshot / "independent" / profile / name
                    try:
                        row = capture_independent_member(independent_root / name, destination, expected)
                        entries["independent/" + profile + "/" + name] = row
                    except (OSError, AdmissionError) as exc:
                        issues.append(f"independent capture {profile}/{name}: {type(exc).__name__}: {exc}")
                if independent_export_inventory(independent_root, issues) != selected_independent:
                    issues.append("independent " + profile + " export inventory changed during capture")
                invocation_root = independent_root.with_name(profile + "-invocation")
                hold_root(invocation_root)
                selected_invocation = snapshot_inventory(invocation_root, issues)
                for name, expected in selected_invocation.items():
                    destination = snapshot / "independent" / (profile + "-invocation") / name
                    try:
                        row = capture_independent_member(invocation_root / name, destination,
                              {"identity": expected, "record": {"kind": "file", "mode": stat.S_IMODE(expected[2])}})
                        entries["independent/" + profile + "-invocation/" + name] = row
                    except (OSError, AdmissionError) as exc:
                        issues.append(f"independent invocation capture {profile}/{name}: {type(exc).__name__}: {exc}")
                if snapshot_inventory(invocation_root, issues) != selected_invocation:
                    issues.append("independent " + profile + " invocation changed during capture")
            if validation is not None and not issues:
                try:
                    require(verify_producer(root, ci, upstream_outcome) == validation, "producer changed during snapshot")
                    for profile, value in independent.items():
                        require(verify_independent_invocation(value["root"], root, profile, ci, value["outcome"], validation)
                                == independent_validation[profile], "independent admission changed during snapshot")
                except (Exception, KeyboardInterrupt) as exc:
                    issues.append(f"terminal revalidation: {type(exc).__name__}: {exc}")
            if validation is not None and not issues:
                # Original admission plus unchanged metadata must also match the
                # copied byte manifest; no reopening of live files builds the tar.
                recorded = read_json(snapshot / "producer/evidence-manifest.json")
                for name, expected in recorded.items():
                    if entries.get("producer/" + name) != expected:
                        issues.append("copied evidence does not match terminal hashes: " + name)
                for profile, observed in independent_validation.items():
                    try:
                        verify_independent_snapshot(snapshot, entries, profile, observed)
                    except (Exception, KeyboardInterrupt) as exc:
                        issues.append(f"independent copied {profile} admission: {type(exc).__name__}: {exc}")
                if not issues:
                    summary.update(validation, status="COMPLETE")
                    summary["complete_unit2e_qualification"] = set(independent_validation) == set(PROFILES)
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
            compact = {name: row for name, row in entries.items()
                       if (name == "membership.json" or name.startswith("producer/")) and compact_member(name)}
            compact.update({name: row for name, row in entries.items() if independent_compact_member(name)})
            compact_manifest = json_bytes({"status": summary["status"], "members": compact,
                "full_only_members": sorted(set(entries) - set(compact)),
                "full_only_scope": "binary, IR and insertion/captured-input payloads are indexed, not replayed by compact audit"})
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
        summary["complete_unit2e_qualification"] = False
    # Keep the standalone index compact; the complete membership is in the tar.
    summary.pop("members", None)
    write_json(output / INDEX_NAME, summary)
    return 0 if summary["status"] == "COMPLETE" else 1



# Frozen independent runner interface. This reader never executes its phases or
# changes its original state/seals; the one supervised full invocation does that.
import csv
import importlib.util

INDEPENDENT_TOOLS = ("llvm-as", "opt", "clang", "ld.lld")
INDEPENDENT_PHASES = ("prepare", "build", "ordinary", "native", "physical", "verify")
INDEPENDENT_SEALS = tuple("phase-" + phase + "-artifacts.json" for phase in INDEPENDENT_PHASES[1:])
INDEPENDENT_RUNNER_SHA = "f76aa8a1b418adfc25487a240a9a2d9e9e76c893e04a27bfd7aaa73a48777d9b"
INDEPENDENT_INPUT_SHA = "3a904929eb7572bad6430f7ab0ea81c3f68625856621041fa2673e4629e1e290"
INDEPENDENT_FIXTURE = "tests/fixtures/fixed_array_unit2d_independent"


def independent_schema(repo):
    path = repo / "scripts/replay_fixed_array_unit2d.py"
    data, _ = stable_bytes(path)
    require(sha256(data) == INDEPENDENT_RUNNER_SHA, "independent runner differs from frozen interface")
    adapter = repo / "scripts/replay_fixed_array_unit2d_current.py"
    data, _ = stable_bytes(adapter)
    spec = importlib.util.spec_from_file_location("unit2e_current_replay_schema", adapter)
    module = importlib.util.module_from_spec(spec)
    exec(compile(data, str(adapter), "exec"), module.__dict__)
    return module.load_runner(path)


def relative_member(value):
    path = Path(value)
    require(isinstance(value, str) and value and not path.is_absolute() and str(path) == value
            and all(part not in (".", "..") for part in path.parts) and "\\" not in value,
            "unsafe evidence member")
    return path


def independent_links(root):
    return {**{"logged-tools/" + name: str(root / "evidence/replay_unit2d_tool_capture.py") for name in INDEPENDENT_TOOLS},
            **{"frozen-loggers/" + name: str(root / "inputs/tools/native_tool_logger.py") for name in INDEPENDENT_TOOLS}}


def validate_independent_seal(seal, run_root):
    """One frozen member/link vocabulary for live and downloaded evidence."""
    require(isinstance(seal, list) and bool(seal), "independent final seal is empty or not a list")
    rows = {}
    for row in seal:
        name = str(relative_member(row["path"]))
        require(name not in rows and name not in INDEPENDENT_SEALS
                and name != "temporary" and not name.startswith("temporary/"), "invalid independent sealed member")
        kind = row.get("kind")
        if kind == "file":
            require(set(row) == {"path", "kind", "bytes", "sha256", "executable"}
                    and type(row["bytes"]) is int and row["bytes"] >= 0 and type(row["executable"]) is bool
                    and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]) is not None, "invalid independent file seal")
        elif kind == "directory":
            require(set(row) == {"path", "kind"}, "invalid independent directory seal")
        else:
            require(kind == "symlink" and set(row) == {"path", "kind", "target"}, "invalid independent link seal")
        rows[name] = row
    links = {name: row["target"] for name, row in rows.items() if row["kind"] == "symlink"}
    require(links == independent_links(run_root), "independent logger link roster or raw targets differ")
    return rows


def validate_independent_full_closure(members, prefix, state, state_sha, binding, seal):
    """Bidirectional exact join of full members to state, seals and input rows."""
    sealed = validate_independent_seal(seal, Path(binding["run_root"]))
    actual = {name.removeprefix(prefix): row for name, row in members.items() if name.startswith(prefix)}
    for name in actual:
        relative_member(name)
    expected_names = {"state.json"} | {"evidence/" + name for name in sealed} | {"evidence/" + name for name in INDEPENDENT_SEALS}
    inputs = {}
    for row in binding["inputs"]:
        name = str(relative_member(row["path"]))
        require(name not in inputs, "duplicate independent input binding")
        inputs[name] = row
        expected_names.add("inputs/" + name)
        expected_names.update("inputs/" + str(parent) for parent in Path(name).parents if str(parent) != ".")
    require(set(actual) == expected_names, "independent full/seal/input member roster differs")
    require(actual["state.json"].get("kind") == "file" and actual["state.json"]["sha256"] == state_sha,
            "independent full state identity differs")
    require(set(state["seals"]) == set(INDEPENDENT_SEALS), "independent full phase roster differs")
    for name in INDEPENDENT_SEALS:
        require(actual["evidence/" + name].get("kind") == "file"
                and actual["evidence/" + name]["sha256"] == state["seals"][name], "independent full phase seal identity differs")
    for name, expected in sealed.items():
        row = actual["evidence/" + name]
        require(row.get("kind") == expected["kind"], "independent full/seal member type differs")
        if expected["kind"] == "file":
            require(row["sha256"] == expected["sha256"] and row["bytes"] == expected["bytes"]
                    and bool(row["mode"] & 0o111) == expected["executable"], "independent full/seal file identity differs")
        elif expected["kind"] == "symlink":
            require(row["target"] == expected["target"], "independent full/seal link identity differs")
    for name, expected in inputs.items():
        row = actual["inputs/" + name]
        require(row.get("kind") == "file" and row["sha256"] == expected["sha256"] and row["bytes"] == expected["bytes"]
                and bool(row["mode"] & 0o111) == expected["executable"], "independent full/input file identity differs")
    for name in expected_names:
        if name.startswith("inputs/") and name.removeprefix("inputs/") not in inputs:
            require(actual[name].get("kind") == "directory", "independent full/input ancestor type differs")


def validate_independent_launcher_closure(members, prefix, result, manifest, closure, final_seal):
    """The original launcher body and its declared closure must join exactly."""
    actual = {name.removeprefix(prefix): row for name, row in members.items() if name.startswith(prefix)}
    require(set(actual) == set(manifest) | {"evidence-manifest.json"}, "independent launcher full/member roster differs")
    for name, expected in manifest.items():
        relative_member(name)
        row = actual[name]
        require(row.get("kind") == "file" and {key: row[key] for key in ("bytes", "mode", "sha256")} == expected,
                "independent original launcher member identity differs")
    require(actual["closure-manifest.json"]["sha256"] == result["closure_manifest_sha256"]
            and closure == final_seal, "independent original launcher closure digest/body differs")


@contextlib.contextmanager
def anchored_directory(path):
    """Keep each ancestor's directory identity bound throughout a traversal."""
    path = no_symlink_path(path)
    descriptor = os.open(path.anchor, os.O_RDONLY | os.O_DIRECTORY)
    ancestors = []
    try:
        for part in path.parts[1:]:
            child = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor)
            ancestors.append((descriptor, part, os.fstat(child)))
            descriptor = child
        yield descriptor
        for parent, part, before in ancestors:
            after = os.stat(part, dir_fd=parent, follow_symlinks=False)
            require((before.st_dev, before.st_ino, stat.S_IFMT(before.st_mode))
                    == (after.st_dev, after.st_ino, stat.S_IFMT(after.st_mode)),
                    "independent traversal ancestor identity changed")
    finally:
        os.close(descriptor)
        for parent, _, _ in ancestors:
            os.close(parent)


def independent_inventory(root):
    """Exact frozen seal vocabulary, with anchored directory/file identities."""
    evidence = no_symlink_path(root / "evidence")
    allowed = independent_links(root)
    rows = []
    def walk(fd, prefix):
        before_directory = os.fstat(fd)
        with os.scandir(fd) as children:
            names = sorted(entry.name for entry in children)
        for name in names:
            relative = prefix + name
            if relative == "temporary" or (not prefix and name in INDEPENDENT_SEALS):
                continue
            before = os.stat(name, dir_fd=fd, follow_symlinks=False)
            if stat.S_ISLNK(before.st_mode):
                target = os.readlink(name, dir_fd=fd)
                require(allowed.get(relative) == target, "unsupported independent symlink: " + relative)
                rows.append({"path": relative, "kind": "symlink", "target": target})
            elif stat.S_ISDIR(before.st_mode):
                child = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                try:
                    require(stat_identity(os.fstat(child)) == stat_identity(before), "independent directory changed on open")
                    rows.append({"path": relative, "kind": "directory"})
                    walk(child, relative + "/")
                finally:
                    os.close(child)
            else:
                require(stat.S_ISREG(before.st_mode), "unsupported independent evidence type")
                record = file_record(evidence / relative)
                rows.append({"path": relative, "kind": "file", "bytes": record["bytes"],
                             "executable": bool(record["mode"] & 0o111), "sha256": record["sha256"]})
            require(stat_identity(os.stat(name, dir_fd=fd, follow_symlinks=False)) == stat_identity(before),
                    "independent member changed during inventory")
        require(stat_identity(os.fstat(fd)) == stat_identity(before_directory), "independent directory changed during inventory")
    with anchored_directory(evidence) as fd:
        walk(fd, "")
    return sorted(rows, key=lambda row: Path(row["path"]).parts)


def independent_regular_manifest(root):
    with anchored_directory(root):
        inventory = regular_inventory(root)
    return [{"path": name, "bytes": row["bytes"], "executable": bool(row["mode"] & 0o111), "sha256": row["sha256"]}
            for name, row in sorted(inventory.items(), key=lambda item: Path(item[0]).parts)]


def independent_terminal_identity(root, state, state_sha, inputs, evidence):
    # Seals and inputs intentionally sit outside the final evidence-inventory
    # comparison, so they require their own terminal membership/hash checks.
    require(file_record(root / "state.json")["sha256"] == state_sha, "independent state changed at terminal admission")
    for name in INDEPENDENT_SEALS:
        require(file_record(root / "evidence" / name)["sha256"] == state["seals"][name],
                "independent phase seal changed at terminal admission")
    require(independent_regular_manifest(root / "inputs") == inputs,
            "independent frozen input membership or bytes changed at terminal admission")
    require(independent_inventory(root) == evidence, "independent evidence changed at terminal admission")


def validate_independent_capture(row, root, inventory):
    """Join captured compiler-input references to original declared blob bytes."""
    require(isinstance(row.get("snapshots"), list), "independent capture lacks snapshot records")
    for captured in row["snapshots"]:
        checksum = captured.get("sha256")
        require(isinstance(checksum, str) and re.fullmatch(r"[0-9a-f]{64}", checksum) is not None
                and type(captured.get("bytes")) is int and captured["bytes"] >= 0,
                "invalid independent captured-input identity")
        relative = "tool-captures/blobs/" + checksum
        require(captured.get("blob") == str(root / "evidence" / relative), "independent captured-input blob path differs")
        blob = inventory.get(relative)
        require(blob is not None and blob.get("kind") == "file" and blob.get("sha256") == checksum
                and blob.get("bytes") == captured["bytes"], "independent captured-input blob hash/length is missing or different")


def independent_admission_identity(state_sha, state, binary_row, verified, outcomes):
    """One original admission record, shared by hosted and downloaded readers."""
    return {"verified": verified, "state_sha256": state_sha,
            "source_binding_sha256": state["source_binding_sha256"], "test_binary": binary_row,
            "named_test_outcomes": outcomes, "phase_seals": state["seals"]}


def admit_independent_git_commands(commands, checkout, head, root, schema):
    arguments = [("rev-parse", "--show-toplevel"), ("status", "--porcelain=v1", "--untracked-files=all"),
        ("rev-parse", "HEAD"), ("rev-parse", head + "^{commit}"), ("rev-parse", head + "^{tree}"),
        ("ls-tree", "-r", "-z", head), ("archive", "--format=tar", "--output=" + str(root / "evidence/source.tar"), head),
        ("status", "--porcelain=v1", "--untracked-files=all"), ("rev-parse", "HEAD")]
    selected = [row for row in commands if row["label"].startswith("git-") or row["argv"][0] == "git"]
    require(len(selected) == len(arguments), "independent Git command roster differs")
    for row, args in zip(selected, arguments):
        require(row["label"] == "git-" + args[0] and row["argv"] == schema.git_argv(checkout, *args)
                and row["cwd"] == str(root), "independent exact-checkout Git invocation differs")


def independent_body(root, *, head, tree, profile, tools, schema, checkout):
    """Read actual terminal bodies and their sealed closure; no corpus execution."""
    root = no_symlink_path(root)
    evidence = root / "evidence"
    state_data, _ = stable_bytes(root / "state.json")
    state_sha = sha256(state_data)
    state = strict_json(state_data)
    require(state.get("schema") == 1 and state.get("status") == "passed"
            and not state.get("active") and state.get("completed") == list(INDEPENDENT_PHASES),
            "independent replay did not finish all phases")
    require(set(state["seals"]) == set(INDEPENDENT_SEALS), "independent seal roster differs")
    for name, expected in state["seals"].items():
        require(file_record(evidence / name)["sha256"] == expected, "independent phase seal drift")
    sealed = read_json(evidence / "phase-verify-artifacts.json")
    validate_independent_seal(sealed, root)
    actual = independent_inventory(root)
    require(actual == sealed, "independent sealed evidence membership or bytes changed")
    by_name = {row["path"]: row for row in actual}
    require(len(by_name) == len(actual), "duplicate independent evidence members")
    def body(name):
        row = by_name[name]
        data, _ = stable_bytes(evidence / relative_member(name))
        require(row["kind"] == "file" and len(data) == row["bytes"] and sha256(data) == row["sha256"],
                "independent body differs from its sealed member: " + name)
        return data
    def document(name):
        return strict_json(body(name))
    def digest(name):
        return by_name[name]["sha256"]
    binding = document("source-binding.json")
    require(digest("source-binding.json") == state["source_binding_sha256"], "independent source-binding drift")
    require(binding["schema"] == 1 and binding["commit"] == binding["checkout_head"] == head
            and binding["tree"] == tree and binding["profile"] == profile and binding["run_root"] == str(root)
            and binding["input_checkout"] == str(checkout)
            and binding["checkout_clean"] is True and binding["runner_sha256"] == INDEPENDENT_RUNNER_SHA,
            "independent source/head/profile/run identity differs")
    schema.assert_current_module_binding(binding)
    content = {key: value for key, value in binding.items() if key not in ("content_id", "marker", "prepared")}
    content_id = sha256(json.dumps(content, sort_keys=True, separators=(",", ":")).encode())
    require(binding["content_id"] == content_id and binding["marker"] == "independent_unit2d_replay_marker_" + content_id,
            "independent source marker identity differs")
    require(digest("original-source.json") == binding["original_manifest_sha256"]
            and digest("source.tar") == binding["archive_sha256"], "independent original-source identities drift")
    inputs = document("input-manifest.json")
    require(digest("input-manifest.json") == binding["input_manifest_sha256"] == binding["original_input_manifest_sha256"] == INDEPENDENT_INPUT_SHA,
            "independent frozen input manifest differs")
    input_rows = independent_regular_manifest(root / "inputs")
    require(input_rows == binding["inputs"], "independent retained input membership differs")
    require(len(inputs["files"]) == len({row["path"] for row in inputs["files"]}), "duplicate frozen input entries")
    require({row["path"]: (row["bytes"], row["sha256"]) for row in input_rows}
            == {row["path"]: (row["bytes"], row["sha256"]) for row in inputs["files"]}, "independent frozen input bytes changed")
    input_map = {row["path"]: row for row in inputs["files"]}
    def input_body(name):
        data, _ = stable_bytes(root / "inputs" / relative_member(name))
        require(len(data) == input_map[name]["bytes"] and sha256(data) == input_map[name]["sha256"],
                "independent input body changed: " + name)
        return data
    def input_document(name):
        return strict_json(input_body(name))
    qualification = input_document("qualification-v2.json")
    require(document("expectation-provenance.json") == schema.PROVENANCE, "independent provenance body changed")
    require(digest("replay_unit2d_tool_capture.py") == binding["capture_runner_sha256"], "independent capture runner changed")
    selected = {name: tools["all_tools"][name]["path"] for name in INDEPENDENT_TOOLS}
    hashes = {name: tools["all_tools"][name]["sha256"] for name in INDEPENDENT_TOOLS}
    require(state["tools"]["trusted"] == selected and state["tools"]["hashes"] == hashes
            and document("trusted-tools.json") == selected and document("tool-hashes.json") == hashes,
            "independent tool map or selected tool bytes differ")
    require(state["tools"]["rust_bin"] == str(Path(tools["all_tools"]["rustc"]["path"]).parent)
            and state["tools"]["rust_hashes"] == {name: tools["all_tools"][name]["sha256"] for name in ("cargo", "rustc")}
            and state["tools"]["cargo_home"] == tools["effective_environment"]["CARGO_HOME"], "independent Rust/cache selection differs")
    binary = document("binary.json")
    require(digest("binary.json") == state["binary_binding_sha256"] and binary["source_binding_sha256"] == state["source_binding_sha256"],
            "independent marked-binary binding drift")
    require(binary["relative_path"] == "evidence/bin/oxid-unit2d-tests", "independent required binary path differs")
    binary_row = file_record(root / binary["relative_path"])
    require(binary_row["sha256"] == binary["sha256"] and binary_row["bytes"] == binary["bytes"], "independent copied binary changed")
    elf_proof(*stable_bytes(root / binary["relative_path"]))
    artifact = binary["cargo_artifact"]
    require(artifact["reason"] == "compiler-artifact" and artifact["target"]["name"] == "oxid"
            and artifact["profile"]["test"] is True and artifact["fresh"] is False
            and root / "target" / profile in Path(artifact["executable"]).parents, "independent Cargo artifact/profile differs")
    verified = document("verified.json")
    require(verified["status"] == "passed" and verified["source_commit"] == head and verified["source_tree"] == tree
            and verified["profile"] == profile and verified["source_binding_sha256"] == state["source_binding_sha256"]
            and verified["test_binary_sha256"] == binary["sha256"] and verified["historical_binary_is_comparison_only"] is True
            and verified["fixed_inventory_totals_are_not_semantic_oracles"] is True,
            "independent verifier result does not join original source/binary")
    inventory = document("test-inventory.json")
    schema.assert_inventory(set(inventory["all"]), set(inventory["ignored"]), binding["marker"])
    expected_names = schema.ORDINARY + [schema.PREFIX + binding["marker"]] + schema.NATIVE + [schema.PHYSICAL]
    exact_inventory(inventory["scope"], expected_names, "independent named-test scope")
    outcomes, command_labels = [], []
    commands = sorted(name for name in by_name if name.startswith("commands/") and name.endswith(".json"))
    for name in commands:
        row = document(name)
        require(row.get("exit") == row.get("expected_exit") == 0 and "error" not in row, "independent original command failed")
        command_labels.append(row["label"])
        for stream in ("stdout", "stderr"):
            require(row[stream] == Path(name).stem + "." + stream, "independent command stream attribution differs")
            relative = "commands/" + str(relative_member(row[stream]))
            require(relative in by_name and by_name[relative]["kind"] == "file", "independent command stream missing")
        if "--exact" in row["argv"]:
            index = row["argv"].index("--exact")
            name = row["argv"][index + 1]
            require(name in expected_names, "unexpected independent test command")
            expected_argv = [str(root / binary["relative_path"]), "--exact", name, "--nocapture", "--test-threads=1"]
            if name in schema.NATIVE + [schema.PHYSICAL]:
                expected_argv.append("--ignored")
            require(row["argv"] == expected_argv and row["cwd"] == str(root / "source"), "independent named-test invocation differs")
            stdout = body("commands/" + row["stdout"]).decode()
            schema.assert_one_pass(stdout, name)
            require(len(re.findall(r"^test .+ \.\.\. ok$", stdout, re.M)) == 1, "duplicate independent test completion")
            if name == schema.PREFIX + binding["marker"]:
                require(("OXID_UNIT2D_SOURCE_BINDING=" + content_id).encode() in body("commands/" + row["stderr"]),
                        "independent marker stderr does not bind source")
            outcomes.append(name)
    expected_labels = ["git-rev-parse", "git-status", "git-rev-parse", "git-rev-parse", "git-rev-parse",
                       "git-ls-tree", "git-archive", "git-status", "git-rev-parse"]
    expected_labels += [name + "-version" for name in ("rustc", "cargo", *INDEPENDENT_TOOLS)]
    expected_labels += ["cargo-build", "test-inventory", "ignored-inventory", binding["marker"]]
    expected_labels += [name.rsplit("::", 1)[-1] for name in schema.ORDINARY + schema.NATIVE]
    expected_labels += ["storage-text-tests", "prepare-physical", "verify-physical", schema.PHYSICAL.rsplit("::", 1)[-1]]
    require(command_labels == expected_labels, "independent original command roster/order differs")
    admit_independent_git_commands([document(name) for name in commands], checkout, head, root, schema)
    exact_inventory(outcomes, expected_names, "independent executed named tests")
    for label in ("cargo-build", "test-inventory", "ignored-inventory", "prepare-physical", "verify-physical", "storage-text-tests"):
        require(command_labels.count(label) == 1, "independent required original command missing or duplicated")
    manifest = document("artifact-manifest.json")
    elf_rows = {Path(row["path"]).stem: row for row in manifest["elf"]}
    require(len(elf_rows) == len(manifest["elf"]), "duplicate independent ELF manifest rows")
    actual_elf = {name for name in by_name if name.startswith("native/") and name.endswith(".elf")}
    require(actual_elf == {row["path"] for row in elf_rows.values()}, "independent ELF membership differs")
    for row in manifest["elf"]:
        actual_row = by_name[str(relative_member(row["path"]))]
        require(actual_row["sha256"] == row["sha256"] and actual_row["bytes"] == row["bytes"], "independent ELF bytes drift")
        elf_proof(*stable_bytes(evidence / row["path"]))
    require({"native/" + row["path"] for row in manifest["llvm"]}
            == {name for name in by_name if name.startswith("native/") and name.endswith(".ll")}, "independent LLVM membership differs")
    for row in manifest["llvm"]:
        actual_row = by_name["native/" + str(relative_member(row["path"]))]
        require(actual_row["sha256"] == row["sha256"] and actual_row["bytes"] == row["bytes"], "independent LLVM bytes drift")
    execution_rows = document("execution-binding.json")
    expected_executions = {name for name in by_name if name.startswith("executions/") and name.endswith(".json")}
    exact_inventory(["executions/" + row["receipt"] for row in execution_rows], list(expected_executions), "independent execution receipts")
    for row in execution_rows:
        receipt = str(relative_member(row["receipt"]))
        original = document("executions/" + receipt)
        require(digest("executions/" + receipt) == row["receipt_sha256"] and original["env_clear"] is True
                and original["PATH"] == str(Path(original["cwd"]) / "no-tools") and type(original["status"]) is int
                and row["status"] == original["status"] and row["binary"] == elf_rows[Path(original["binary"]).name],
                "independent source-free execution binding differs")
        for stream in ("stdout", "stderr"):
            require(digest("executions/" + str(Path(receipt).with_suffix("." + stream))) == row[stream + "_sha256"], "independent execution stream drift")
    tool_receipts = [name for name in by_name if name.startswith("tool-receipts/") and name.endswith(".json")]
    captures = [name for name in by_name if name.startswith("tool-captures/") and name.endswith("/receipt.json")]
    require(len(captures) == len(tool_receipts), "independent captured/original LLVM command roster differs")
    for name in tool_receipts + captures:
        row = document(name)
        require(row["tool"] in INDEPENDENT_TOOLS and row["trusted_target"] == selected[row["tool"]]
                and row["exit"] == 0 and not row.get("error"), "independent LLVM command used wrong tool or failed")
        if name in captures:
            validate_independent_capture(row, root, by_name)
        parent = Path(name).parent
        for stream in ("stdout", "stderr"):
            leaf = row[stream] if name in tool_receipts else stream
            require((parent / relative_member(leaf)).as_posix() in by_name, "independent LLVM original stream missing")
    schema.assert_execution_inventory(len(actual_elf), len(execution_rows), len(tool_receipts), qualification)
    require(verified["ordinary"] == 8 and verified["native_families"] == 9
            and verified["elf_artifacts"] == len(actual_elf) and verified["source_free_executions"] == len(execution_rows),
            "independent verifier coverage differs from original member bodies")
    old_ir = schema.compare_old_ir(evidence / "old-ir", input_document("expectations/old-ir-manifest.json"),
                                  input_body("expectations/old-ir-inventory.tsv"))
    require(document("old-ir-comparison.json") == old_ir, "independent old-IR comparison body differs")
    harness = input_body("expectations/physical-harness.tsv")
    require(body("physical/harness.tsv") == harness
            and body("physical/manifest.tsv") == harness, "independent physical expected manifest differs")
    physical = []
    for row in csv.DictReader(io.StringIO(harness.decode()), delimiter="\t"):
        stem = evidence / "executions" / ("physical-" + row["case"])
        actual_result = document(stem.with_suffix(".json").relative_to(evidence).as_posix())
        actual_result.update(stdout=body(stem.with_suffix(".stdout").relative_to(evidence).as_posix()), stderr=body(stem.with_suffix(".stderr").relative_to(evidence).as_posix()))
        schema.compare_result(actual_result, {"status": int(row["status"]), "stdout": bytes.fromhex(row["stdout_hex"]),
                                              "stderr": bytes.fromhex(row["stderr_hex"])}, row["case"])
        physical.append({"case": row["case"], "status": actual_result["status"]})
    require(document("physical-comparison.json") == physical and len(physical) == 29
            and sum(row["status"] == 1 for row in physical) == 11, "independent physical comparison body differs")
    require(document("physical/input-binding.json")["source_commit"] == head
            and document("physical/harness-receipt.json")["source_commit"] == head,
            "independent physical source binding differs")
    structure = document("structure.json")
    sidecars = [Path(name).name for name in by_name if name.startswith("executions/") and name.endswith(".structure.tsv")]
    schema.assert_structure_inventory(sidecars, inputs["replay_inventories"], len(structure["bounds"]), len(structure["phis"]))
    require(structure["bounds_sites"] == len(structure["bounds"]) and structure["pointer_phis"] == len(structure["phis"]),
            "independent structure body/count mismatch")
    supplement = input_document("expectations/supplement-v1.json")
    cases = [{**supplement["positive_shared_alias"], "name": "shared-array-aliases"}, *supplement["extreme_cases"]]
    for case in cases:
        stem = evidence / "executions" / case["name"]
        actual_result = document(stem.with_suffix(".json").relative_to(evidence).as_posix())
        actual_result.update(stdout=body(stem.with_suffix(".stdout").relative_to(evidence).as_posix()), stderr=body(stem.with_suffix(".stderr").relative_to(evidence).as_posix()))
        schema.compare_result(actual_result, {"status": case["status"], "stdout": bytes.fromhex(case["stdout_hex"]),
                                              "stderr": bytes.fromhex(case["stderr_hex"])}, case["name"])
    require(document("supplement-comparison.json") == {
        "provenance": "source-frozen semantic expected outputs from expectations/supplement-v1.json",
        "cases": [case["name"] for case in cases], "exact_status_stdout_stderr_match": True}, "independent supplement comparison differs")
    independent_terminal_identity(root, state, state_sha, input_rows, actual)
    return {**independent_admission_identity(state_sha, state, binary_row, verified, outcomes), "closure": actual}



def independent_command(repo, root, profile, tools, producer):
    rust_bin = Path(tools["all_tools"]["rustc"]["path"]).parent
    require(Path(tools["all_tools"]["cargo"]["path"]).parent == rust_bin, "independent Cargo/rustc directories differ")
    return [tools["all_tools"]["python"]["path"], "-B", str(repo / "scripts/replay_fixed_array_unit2d_current.py"),
            "--repo", str(repo), "--commit", read_json(producer / "invocation.json")["ci"]["expected_head"],
            "--output", str(root), "--rust-bin", str(rust_bin), "--trusted-tools", str(producer / "independent-llvm-tools.json"),
            "--profile", profile, "--cargo-home", tools["effective_environment"]["CARGO_HOME"]]


def execute_independent(args):
    producer = no_symlink_path(args.producer_root)
    ci = ci_identity(args.expected_head, args.event_sha)
    invocation = read_json(producer / "invocation.json")
    repo = Path(invocation["repository_path"])
    tools = read_json(producer / "toolchains.json")
    output = safe_output(args.output, (repo, producer, Path(tools["effective_environment"]["CARGO_HOME"])))
    require(output.name == args.profile, "independent output leaf must equal its profile")
    capture = safe_output(output.with_name(args.profile + "-invocation"), (repo, producer, output))
    capture.mkdir(parents=True)
    binding = {"ci": ci, "profile": args.profile, "producer_invocation_id": invocation["invocation_id"],
               "producer_result_sha256": file_record(producer / "result.json")["sha256"],
               "source": read_json(producer / "result.json")["binding"], "output_root": str(output),
               "trusted_map_sha256": file_record(producer / "independent-llvm-tools.json")["sha256"]}
    result = {"status": "FAIL", "binding": binding}
    code = 1
    try:
        admitted = verify_producer(producer, ci, "success")
        require(binding["source"] == admitted["binding"], "producer binding changed before independent admission")
        configs = check_cargo_config(output / "source", Path(tools["effective_environment"]["CARGO_HOME"]),
                                     read_json(producer / "source/identity.json")["files"])
        binding["external_cargo_config"] = [row for row in configs if row["category"] != "repository"]
        write_json(capture / "invocation.json", binding)
        schema = independent_schema(repo)
        def boundary():
            current = verify_producer(producer, ci, "success")
            require(current == admitted, "producer changed around independent invocation")
            configs = check_cargo_config(output / "source", Path(tools["effective_environment"]["CARGO_HOME"]),
                                         read_json(producer / "source/identity.json")["files"])
            require([row for row in configs if row["category"] != "repository"] == binding["external_cargo_config"],
                    "independent Cargo config discovery changed")
        with tempfile.TemporaryDirectory(prefix="unit2e-independent-") as temporary:
            environment = {**tools["effective_environment"], "TMPDIR": temporary}
            run_child(independent_command(repo, output, args.profile, tools, producer), capture / "command",
                      cwd=repo, environment=environment, selection=tools, check=boundary, timeout=5400)
            observed = independent_body(output, head=args.expected_head, tree=admitted["binding"]["tree"],
                                        profile=args.profile, tools=tools, schema=schema, checkout=repo)
        write_json(capture / "closure-manifest.json", observed.pop("closure"))
        result.update(status="PASS", exit_code=0, admission=observed,
                      closure_manifest_sha256=file_record(capture / "closure-manifest.json")["sha256"],
                      command_sha256=file_record(capture / "command/command.json")["sha256"])
        code = 0
    except (Exception, KeyboardInterrupt) as exc:
        code = getattr(exc, "code", 130 if isinstance(exc, KeyboardInterrupt) else 1)
        result.update(exit_code=code, error=f"{type(exc).__name__}: {exc}")
    finally:
        try:
            if not (capture / "invocation.json").exists():
                write_json(capture / "invocation.json", binding)
            write_json(capture / ("result.json" if code == 0 else "failure.json"), result)
            write_json(capture / "evidence-manifest.json", regular_inventory(capture))
        except (Exception, KeyboardInterrupt) as exc:
            print(f"independent invocation evidence sealing failed: {type(exc).__name__}: {exc}", file=sys.stderr)
            code = code or 1
    return code


def verify_independent_invocation(root, producer, profile, ci, outcome, producer_admission):
    require(outcome == "success", "independent " + profile + " upstream step did not succeed: " + outcome)
    capture = root.with_name(profile + "-invocation")
    require(not (capture / "failure.json").exists(), "independent invocation contains failure metadata")
    result = read_json(capture / "result.json")
    invocation = read_json(producer / "invocation.json")
    expected = {"ci": ci, "profile": profile, "producer_invocation_id": invocation["invocation_id"],
                "producer_result_sha256": file_record(producer / "result.json")["sha256"],
                "source": producer_admission["binding"], "output_root": str(root),
                "trusted_map_sha256": file_record(producer / "independent-llvm-tools.json")["sha256"]}
    source_files = read_json(producer / "source/identity.json")["files"]
    tools = read_json(producer / "toolchains.json")
    configs = check_cargo_config(root / "source", Path(tools["effective_environment"]["CARGO_HOME"]), source_files)
    expected["external_cargo_config"] = [row for row in configs if row["category"] != "repository"]
    require(result["status"] == "PASS" and result["exit_code"] == 0 and result["binding"] == expected
            and read_json(capture / "invocation.json") == expected, "independent invocation run/profile/source binding differs")
    tools = read_json(producer / "toolchains.json")
    repo = Path(invocation["repository_path"])
    command = command_admission(capture / "command", tools,
                                expected_argv=independent_command(repo, root, profile, tools, producer), expected_cwd=repo)
    environment = dict(command["environment"])
    require(Path(environment.pop("TMPDIR")).is_absolute(), "independent temporary directory must be absolute")
    expected_environment = dict(tools["effective_environment"])
    expected_environment.pop("TMPDIR")
    require(environment == expected_environment, "independent launcher environment drift")
    require(file_record(capture / "command/command.json")["sha256"] == result["command_sha256"], "independent original command receipt changed")
    require(file_record(capture / "closure-manifest.json")["sha256"] == result["closure_manifest_sha256"], "independent captured final seal changed")
    observed = independent_body(root, head=ci["expected_head"], tree=producer_admission["binding"]["tree"],
                                profile=profile, tools=tools, schema=independent_schema(repo), checkout=repo)
    require(read_json(capture / "closure-manifest.json") == observed.pop("closure"), "independent closure differs from original launcher admission")
    require(observed == result["admission"], "independent terminal bodies changed after original admission")
    evidence = regular_inventory(capture)
    evidence.pop("evidence-manifest.json")
    require(evidence == read_json(capture / "evidence-manifest.json"), "independent launcher evidence changed")
    state = read_json(root / "state.json")
    require(state["tools"]["trusted_map_path"] == str(producer / "independent-llvm-tools.json")
            and state["tools"]["trusted_map_sha256"] == expected["trusted_map_sha256"], "independent selected map lacks producer binding")
    observed["launcher_result_sha256"] = file_record(capture / "result.json")["sha256"]
    observed["launcher_evidence_manifest_sha256"] = file_record(capture / "evidence-manifest.json")["sha256"]
    return observed


def independent_compact_member(name):
    # Exact original receipt bodies from both logger layers; blobs and executable/
    # IR payloads remain explicitly named by full-only membership, never pruned.
    parts = Path(name).parts
    if len(parts) < 3 or parts[0] != "independent":
        return False
    if parts[1].endswith("-invocation"):
        return True
    relative = Path(*parts[2:]).as_posix()
    if relative == "state.json" or relative.startswith("inputs/"):
        return True
    if not relative.startswith("evidence/"):
        return False
    sub = Path(relative.removeprefix("evidence/"))
    if len(sub.parts) == 1:
        return sub.suffix == ".json"
    if sub.parts[0] in ("commands", "executions", "tool-receipts"):
        return True
    if sub.parts[0] == "tool-captures":
        return len(sub.parts) == 3 and sub.parts[1] != "blobs" and sub.parts[2] in ("receipt.json", "stdout", "stderr")
    if sub.parts[0] == "physical":
        return len(sub.parts) == 2 and sub.name in ("input-binding.json", "harness-receipt.json", "harness.tsv", "manifest.tsv", "SHA256SUMS")
    return False


def verify_independent_snapshot(snapshot, entries, profile, observed):
    prefix = "independent/" + profile + "/"
    state = read_json(snapshot / prefix / "state.json")
    binding = read_json(snapshot / prefix / "evidence/source-binding.json")
    seal = read_json(snapshot / prefix / "evidence/phase-verify-artifacts.json")
    require(state["seals"] == observed["phase_seals"], "copied independent phase identity differs from admission")
    validate_independent_full_closure(entries, prefix, state, observed["state_sha256"], binding, seal)
    launch = "independent/" + profile + "-invocation/"
    result = read_json(snapshot / launch / "result.json")
    require(entries[launch + "result.json"]["sha256"] == observed["launcher_result_sha256"]
            and entries[launch + "evidence-manifest.json"]["sha256"] == observed["launcher_evidence_manifest_sha256"],
            "copied independent launcher terminal identity differs")
    validate_independent_launcher_closure(entries, launch, result,
        read_json(snapshot / launch / "evidence-manifest.json"), read_json(snapshot / launch / "closure-manifest.json"), seal)


def capture_independent_member(path, destination, expected):
    no_symlink_path(path.parent)
    require(stat_identity(path.lstat()) == expected["identity"], "independent member changed before capture")
    row = expected["record"]
    if row["kind"] == "file":
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open("xb") as handle:
            captured = capture_file(path, handle)
        destination.chmod(captured["mode"])
        row = {**row, **captured}
    elif row["kind"] == "symlink":
        require(os.readlink(path) == row["target"], "independent symlink target changed")
    else:
        require(row["kind"] == "directory" and path.is_dir() and not path.is_symlink(), "independent directory changed")
    require(stat_identity(path.lstat()) == expected["identity"], "independent member changed during capture")
    return row


def independent_export_inventory(root, issues):
    """Frozen evidence+input/state closure; preserve partial target if binary unverified."""
    result = {}
    def add_tree(directory, prefix, excluded=(), links=None):
        if not directory.exists():
            issues.append("missing independent evidence directory: " + str(directory))
            return
        links = {} if links is None else links
        try:
            no_symlink_path(directory)
        except (OSError, AdmissionError) as exc:
            issues.append(str(exc))
            return
        def failed(error):
            issues.append(f"independent inventory: {type(error).__name__}: {error}")
        for parent, directories, files in os.walk(directory, followlinks=False, onerror=failed):
            for leaf in list(directories) + files:
                path = Path(parent) / leaf
                relative = path.relative_to(directory).as_posix()
                if relative in excluded:
                    if leaf in directories:
                        directories.remove(leaf)
                    continue
                name = prefix + relative
                try:
                    value = path.lstat()
                    mode = stat.S_IMODE(value.st_mode)
                    if stat.S_ISLNK(value.st_mode):
                        target = os.readlink(path)
                        require(links.get(relative) == target, "unsupported independent export symlink")
                        if leaf in directories:
                            directories.remove(leaf)
                        row = {"kind": "symlink", "target": target, "mode": mode}
                    elif stat.S_ISDIR(value.st_mode):
                        no_symlink_path(path)
                        row = {"kind": "directory", "mode": mode}
                    else:
                        require(stat.S_ISREG(value.st_mode), "unsupported independent export member type")
                        no_symlink_path(path)
                        row = {"kind": "file", "mode": mode}
                    result[name] = {"identity": stat_identity(value), "record": row}
                except (OSError, AdmissionError) as exc:
                    failed(exc)
    try:
        no_symlink_path(root)
        value = (root / "state.json").lstat()
        require(stat.S_ISREG(value.st_mode), "independent state is not regular")
        result["state.json"] = {"identity": stat_identity(value), "record": {"kind": "file", "mode": stat.S_IMODE(value.st_mode)}}
    except (OSError, AdmissionError) as exc:
        issues.append(f"independent state: {type(exc).__name__}: {exc}")
    add_tree(root / "evidence", "evidence/", excluded=("temporary",), links=independent_links(root))
    add_tree(root / "inputs", "inputs/")
    try:
        binary = read_json(root / "evidence/binary.json")
        require(binary["relative_path"] == "evidence/bin/oxid-unit2d-tests", "copied independent binary path differs")
        row = file_record(root / binary["relative_path"])
        require(row["sha256"] == binary["sha256"] and row["bytes"] == binary["bytes"], "copied independent binary unverified")
        state = read_json(root / "state.json")
        require(file_record(root / "evidence/binary.json")["sha256"] == state["binary_binding_sha256"]
                and file_record(root / "evidence/source-binding.json")["sha256"] == state["source_binding_sha256"]
                and binary["source_binding_sha256"] == state["source_binding_sha256"], "copied independent binary/source join unverified")
        elf_proof(*stable_bytes(root / binary["relative_path"]))
    except (OSError, ValueError, TypeError, KeyError) as exc:
        # A failed build without the required copied binary cannot silently
        # discard its surviving target artifacts as a qualified cache exclusion.
        if (root / "target").exists():
            issues.append("retaining partial independent target because copied binary was not verified: " + str(exc))
            add_tree(root / "target", "target/")
    return result



def audit_compact(index_path, archive_path, expected_head, event_sha):
    """Audit downloaded bodies/commitments; never reopen hosted absolute paths."""
    index = read_json(index_path)
    require(index["status"] == "COMPLETE" and index["ci"]["expected_head"] == expected_head
            and index["ci"]["event_sha"] == event_sha and index["producer_step_outcome"] == "success",
            "compact index is incomplete or belongs to another head/event")
    bodies, modes = {}, {}
    with stable_file(archive_path) as (compressed, before):
        require(before.st_size <= COMPACT_LIMIT, "compact archive exceeds supported transport size")
        downloaded = compressed_record(compressed, before)
        require(all(downloaded[key] == index["compact"][key] for key in ("bytes", "sha256")),
                "downloaded compact archive differs from original index")
        with tarfile.open(fileobj=compressed, mode="r:gz") as archive:
            total = 0
            for member in archive:
                relative_member(member.name)
                require(member.name not in bodies and (member.isfile() or member.isdir()), "unsupported or duplicate compact member")
                total += member.size
                require(total <= 256 * 1024 * 1024, "compact decoded body budget exceeded")
                modes[member.name] = member.mode
                bodies[member.name] = archive.extractfile(member).read() if member.isfile() else None
        require(compressed_record(compressed, before) == downloaded, "compact compressed bytes changed while parsing bodies")
    def document(name):
        require(name in bodies and bodies[name] is not None, "required compact body missing: " + name)
        return strict_json(bodies[name])
    compact = document("compact-membership.json")
    full = document("membership.json")
    require(compact["status"] == full["status"] == "COMPLETE", "compact/full manifest is incomplete")
    require(set(bodies) == set(compact["members"]) | {"compact-membership.json"}, "compact body roster differs from manifest")
    for name, row in compact["members"].items():
        require(modes[name] == row["mode"], "compact member mode mismatch")
        if row.get("kind") == "directory":
            require(bodies[name] is None, "compact directory type mismatch")
        else:
            require(bodies[name] is not None and len(bodies[name]) == row["bytes"] and sha256(bodies[name]) == row["sha256"],
                    "compact original body hash mismatch")
        if name != "membership.json":
            require(full["members"].get(name) == row, "compact/full original member identity differs")
    expected = {name for name in full["members"]
                if ((name.startswith("producer/") and compact_member(name)) or independent_compact_member(name))}
    require(set(compact["members"]) == expected | {"membership.json"}, "compact required-body classification differs")
    require(compact["full_only_members"] == sorted(set(full["members"]) - expected), "full-only omission roster differs")
    prefixes = ["producer/"]
    if index["complete_unit2e_qualification"]:
        prefixes += ["independent/" + profile + suffix for profile in PROFILES for suffix in ("/", "-invocation/")]
    for name in full["members"]:
        relative_member(name)
        require(any(name.startswith(prefix) for prefix in prefixes), "unsupported full evidence namespace")
    producer_members = {name.removeprefix("producer/"): row for name, row in full["members"].items() if name.startswith("producer/")}
    producer_members.pop("evidence-manifest.json")
    require(producer_members == document("producer/evidence-manifest.json"), "producer full/evidence member roster differs")
    ci = index["ci"]
    invocation = document("producer/invocation.json")
    terminal = document("producer/result.json")
    source = document("producer/source/identity.json")
    tools = document("producer/toolchains.json")
    binding = terminal["binding"]
    require(invocation["ci"] == terminal["ci"] == ci and invocation["profiles"] == list(PROFILES)
            and terminal["status"] == "PASS" and terminal["exit_code"] == 0
            and terminal["invocation_id"] == invocation["invocation_id"], "producer compact run tuple differs")
    require(binding["head"] == source["head"] == expected_head and binding["tree"] == source["tree"]
            and binding["source_identity_sha256"] == sha256(bodies["producer/source/identity.json"])
            and binding["toolchains_sha256"] == sha256(bodies["producer/toolchains.json"]), "producer compact source/tool binding differs")
    tool_map = {name: tools["all_tools"][name]["path"] for name in INDEPENDENT_TOOLS}
    require(document("producer/independent-llvm-tools.json") == tool_map, "compact selected-tool map differs")
    for name in LLVM_TOOLS:
        require(tools["tools"][name] == tools["all_tools"][name]
                and tools["tools"][name]["path"] == str(Path(tools["llvm_directory"]) / name), "compact LLVM selection differs")
        validate_llvm_version(bodies["producer/tools/" + name + "/stdout"] + bodies["producer/tools/" + name + "/stderr"])
    validate_rust_version(bodies["producer/tools/rustc/stdout"])
    def command(prefix, selected_directory):
        row = document(prefix + "/command.json")
        require(row["status"] == "PASS" and row["exit_code"] == row["failure_code"] == 0
                and row["cleanup"]["stopped"] and not row["cleanup"]["surviving_group_detected"], "compact original child failed")
        require(row["environment"].get("OXID_LLVM_BIN") == selected_directory
                and row["unset_controls"] == list(UNSET_CONTROLS)
                and all(name not in row["environment"] for name in UNSET_CONTROLS), "compact command environment differs")
        for stream in ("stdout", "stderr"):
            data = bodies[prefix + "/" + stream]
            require(row[stream]["sha256"] == sha256(data) and row[stream]["bytes"] == len(data), "compact original command log mismatch")
        return row
    command("producer/preflight-command", tools["llvm_directory"])
    prepared = document("producer/source-preflight/prepared.json")
    require(prepared["status"] == "verified-inputs" and prepared["semantic_pass"] is False
            and prepared["current_source_sha256"] == source["current_source_sha256"] == binding["current_source_sha256"],
            "producer compact source preflight identity differs")
    current_source_path = "producer/source/inputs/" + BINDING + "/current-source.json"
    require(sha256(bodies[current_source_path]) == source["current_source_sha256"], "producer compact current-source body differs")
    current_source = document(current_source_path)
    require(current_source["reviewed_source_head"] == source["reviewed_source_head"]
            and current_source["source_only_tree"] == source["source_only_tree"], "producer compact current-source provenance differs")
    for name in tools["all_tools"]:
        command("producer/tools/" + name, tools["llvm_directory"])
    command("producer/tools/target-libdir", tools["llvm_directory"])
    for profile in PROFILES:
        prefix = "producer/" + profile
        result = document(prefix + "/result.json")
        require(result["profile"] == profile and result["status"] == "PASS" and result["binding"] == binding,
                "producer compact profile binding differs")
        require(terminal["profiles"][profile]["status"] == "PASS"
                and terminal["profiles"][profile]["result"]["sha256"] == sha256(bodies[prefix + "/result.json"]), "producer terminal profile result differs")
        commands = {}
        for name in ("build", "discovery", "list", "run"):
            commands[name] = command(prefix + "/" + name, tools["llvm_directory"])
            require(result["command_receipts"][name]["sha256"] == sha256(bodies[prefix + "/" + name + "/command.json"]), "producer command receipt binding differs")
        original_root = Path(invocation["output_root"]) / profile
        copied_path = str(original_root / "bin/oxid-test")
        expected_build = [tools["all_tools"]["cargo"]["path"], "test", "--locked", "--offline", "--bin", "oxid", "--no-run", "--message-format=json", "--jobs", "2"]
        if profile == "release":
            expected_build.append("--release")
        require(commands["build"]["argv"] == expected_build
                and commands["discovery"]["argv"] == [copied_path, PREFIX, "--list", "--ignored", "--format", "pretty", "--color", "never"]
                and commands["list"]["argv"] == [copied_path, *ROSTER, "--exact", "--list", "--ignored", "--format", "pretty", "--color", "never"]
                and commands["run"]["argv"] == [copied_path, *ROSTER, "--exact", "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"],
                "producer compact original profile invocations differ")
        for row in commands.values():
            require(row["cwd"] == invocation["repository_path"]
                    and row["environment"].get("OXID_OWNED_NATIVE_EVIDENCE") == str(original_root / "artifacts"),
                    "producer compact profile working directory/evidence root differs")
        cargo_messages = [strict_json(line) for line in bodies[prefix + "/build/stdout"].splitlines()]
        build_paths = [row["executable"] for row in cargo_messages if row.get("reason") == "compiler-artifact"
                       and row.get("target", {}).get("name") == "oxid" and row.get("profile", {}).get("test") is True and row.get("executable")]
        require(len(build_paths) == 1 and result["build_binary"] == {"path": build_paths[0], **result["test_binary"]}
                and Path(tools["effective_environment"]["CARGO_TARGET_DIR"]) / profile in Path(build_paths[0]).parents,
                "producer compact Cargo/copied-binary provenance differs")
        require(result["outcomes"] == admit_stdout(bodies[prefix + "/run/stdout"])
                and result["families"] == admit_stderr(bodies[prefix + "/run/stderr"]), "producer compact named results differ")
        admit_discovery(bodies[prefix + "/discovery/stdout"])
        require(document(prefix + "/roster.json")["names"] == admit_list(bodies[prefix + "/list/stdout"]), "producer compact roster differs")
        require(full["members"][prefix + "/bin/oxid-test"] == result["test_binary"], "producer full-only binary commitment differs")
        manifest = document(prefix + "/artifact-manifest.json")
        require(sha256(bodies[prefix + "/artifact-manifest.json"]) == result["artifact_manifest_sha256"], "producer artifact manifest body differs")
        for name, row in manifest.items():
            require(full["members"][prefix + "/artifacts/" + name] == {key: row[key] for key in ("bytes", "mode", "sha256")},
                    "producer full-only artifact commitment differs")
    if index["complete_unit2e_qualification"]:
        require(index["independent_step_outcomes"] == {profile: "success" for profile in PROFILES}, "independent upstream steps did not both succeed")
        require(set(index["independent"]) == set(PROFILES), "independent index admission roster differs")
        schema = independent_schema(Path(__file__).resolve().parents[1])
        for profile in PROFILES:
            prefix = "independent/" + profile + "/"
            launch = "independent/" + profile + "-invocation/"
            state = document(prefix + "state.json")
            source_binding = document(prefix + "evidence/source-binding.json")
            schema.assert_current_module_binding(source_binding)
            binary = document(prefix + "evidence/binary.json")
            verified = document(prefix + "evidence/verified.json")
            result = document(launch + "result.json")
            require(state["schema"] == 1 and state["status"] == "passed" and not state.get("active")
                    and state["completed"] == list(INDEPENDENT_PHASES) and set(state["seals"]) == set(INDEPENDENT_SEALS),
                    "independent compact phase completion differs")
            require(source_binding["commit"] == source_binding["checkout_head"] == verified["source_commit"] == expected_head
                    and source_binding["tree"] == verified["source_tree"] == binding["tree"]
                    and source_binding["input_checkout"] == invocation["repository_path"]
                    and source_binding["profile"] == verified["profile"] == profile and verified["status"] == "passed",
                    "independent compact source/profile binding differs")
            source_digest = sha256(bodies[prefix + "evidence/source-binding.json"])
            require(source_digest == state["source_binding_sha256"] == binary["source_binding_sha256"] == verified["source_binding_sha256"],
                    "independent compact source receipt join differs")
            require(state["binary_binding_sha256"] == sha256(bodies[prefix + "evidence/binary.json"])
                    and binary["sha256"] == verified["test_binary_sha256"], "independent compact binary receipt join differs")
            full_binary = full["members"][prefix + "evidence/bin/oxid-unit2d-tests"]
            require(full_binary["sha256"] == binary["sha256"] and full_binary["bytes"] == binary["bytes"], "independent full-only binary commitment differs")
            require(state["tools"]["trusted"] == tool_map and document(prefix + "evidence/trusted-tools.json") == tool_map
                    and state["tools"]["hashes"] == {name: tools["all_tools"][name]["sha256"] for name in INDEPENDENT_TOOLS},
                    "independent compact tool selection differs")
            require(result["status"] == "PASS" and result["exit_code"] == 0 and result["binding"]["ci"] == ci
                    and result["binding"]["profile"] == profile and result["binding"]["source"] == binding
                    and result["binding"]["producer_invocation_id"] == invocation["invocation_id"]
                    and result["binding"] == document(launch + "invocation.json"), "independent compact launcher tuple differs")
            command(launch + "command", tools["llvm_directory"])
            require(result["command_sha256"] == sha256(bodies[launch + "command/command.json"]), "independent compact original launch hash differs")
            for name, expected_digest in state["seals"].items():
                require(sha256(bodies[prefix + "evidence/" + name]) == expected_digest, "independent compact phase seal differs")
            final_seal = document(prefix + "evidence/phase-verify-artifacts.json")
            validate_independent_full_closure(full["members"], prefix, state, result["admission"]["state_sha256"], source_binding, final_seal)
            validate_independent_launcher_closure(full["members"], launch, result,
                document(launch + "evidence-manifest.json"), document(launch + "closure-manifest.json"), final_seal)
            require(sha256(bodies[prefix + "evidence/input-manifest.json"]) == INDEPENDENT_INPUT_SHA, "independent compact frozen input manifest differs")
            input_manifest = document(prefix + "evidence/input-manifest.json")
            for row in input_manifest["files"]:
                data = bodies[prefix + "inputs/" + row["path"]]
                require(len(data) == row["bytes"] and sha256(data) == row["sha256"], "independent compact input body differs")
            expected_names = schema.ORDINARY + [schema.PREFIX + source_binding["marker"]] + schema.NATIVE + [schema.PHYSICAL]
            inventory = document(prefix + "evidence/test-inventory.json")
            exact_inventory(inventory["scope"], expected_names, "independent compact test scope")
            outcomes = []
            for name in bodies:
                if not name.startswith(prefix + "evidence/commands/") or not name.endswith(".json"):
                    continue
                row = document(name)
                require(row["exit"] == row["expected_exit"] == 0 and "error" not in row, "independent compact original command failed")
                if "--exact" in row["argv"]:
                    test = row["argv"][row["argv"].index("--exact") + 1]
                    schema.assert_one_pass(bodies[prefix + "evidence/commands/" + row["stdout"]].decode(), test)
                    outcomes.append(test)
            exact_inventory(outcomes, expected_names, "independent compact executed tests")
            admit_independent_git_commands([document(name) for name in sorted(bodies)
                if name.startswith(prefix + "evidence/commands/") and name.endswith(".json")],
                source_binding["input_checkout"], expected_head, Path(source_binding["run_root"]), schema)
            observed = independent_admission_identity(sha256(bodies[prefix + "state.json"]), state,
                {key: full_binary[key] for key in ("bytes", "mode", "sha256")}, verified, outcomes)
            require(observed == result["admission"], "independent compact terminal bodies differ from original launcher admission")
            require(index["independent"][profile] == {**observed,
                "launcher_result_sha256": sha256(bodies[launch + "result.json"]),
                "launcher_evidence_manifest_sha256": sha256(bodies[launch + "evidence-manifest.json"])},
                "independent compact original admission differs from index")
            schema.assert_inventory(set(inventory["all"]), set(inventory["ignored"]), source_binding["marker"])
            require(document(prefix + "evidence/expectation-provenance.json") == schema.PROVENANCE,
                    "independent compact provenance body differs")
            artifacts = document(prefix + "evidence/artifact-manifest.json")
            native = {Path(row["path"]).stem: row for row in artifacts["elf"]}
            executions = document(prefix + "evidence/execution-binding.json")
            execution_names = [name for name in bodies if name.startswith(prefix + "evidence/executions/") and name.endswith(".json")]
            exact_inventory([prefix + "evidence/executions/" + row["receipt"] for row in executions], execution_names,
                            "independent compact execution body roster")
            for row in executions:
                name = prefix + "evidence/executions/" + row["receipt"]
                original = document(name)
                require(sha256(bodies[name]) == row["receipt_sha256"] and original["env_clear"] is True
                        and original["PATH"] == str(Path(original["cwd"]) / "no-tools")
                        and type(original["status"]) is int and original["status"] == row["status"]
                        and row["binary"] == native[Path(original["binary"]).name], "independent compact execution/binary join differs")
                for stream in ("stdout", "stderr"):
                    require(sha256(bodies[str(Path(name).with_suffix("." + stream))]) == row[stream + "_sha256"],
                            "independent compact runtime stream differs")
            tool_names = [name for name in bodies if name.startswith(prefix + "evidence/tool-receipts/") and name.endswith(".json")]
            capture_names = [name for name in bodies if name.startswith(prefix + "evidence/tool-captures/") and name.endswith("/receipt.json")]
            require(len(tool_names) == len(capture_names), "independent compact LLVM logger layers differ")
            capture_inventory = {name.removeprefix(prefix + "evidence/"): row for name, row in full["members"].items()
                                 if name.startswith(prefix + "evidence/tool-captures/blobs/")}
            for name in tool_names + capture_names:
                original = document(name)
                require(original["exit"] == 0 and not original.get("error") and original["tool"] in INDEPENDENT_TOOLS
                        and original["trusted_target"] == tool_map[original["tool"]], "independent compact LLVM receipt differs")
                for stream in ("stdout", "stderr"):
                    leaf = original[stream] if name in tool_names else stream
                    require(str(Path(name).parent / relative_member(leaf)) in bodies, "independent compact LLVM stream missing")
                if name in capture_names:
                    validate_independent_capture(original, Path(source_binding["run_root"]), capture_inventory)
            qualification = document(prefix + "inputs/qualification-v2.json")
            schema.assert_execution_inventory(len(native), len(executions), len(tool_names), qualification)
            require(verified["ordinary"] == 8 and verified["native_families"] == 9
                    and verified["elf_artifacts"] == len(native) and verified["source_free_executions"] == len(executions),
                    "independent compact coverage differs from original bodies")
            for row in artifacts["elf"]:
                committed = full["members"][prefix + "evidence/" + row["path"]]
                require(committed["sha256"] == row["sha256"] and committed["bytes"] == row["bytes"],
                        "full-only independent ELF commitment differs")
            for row in artifacts["llvm"]:
                committed = full["members"][prefix + "evidence/native/" + row["path"]]
                require(committed["sha256"] == row["sha256"] and committed["bytes"] == row["bytes"],
                        "full-only independent LLVM commitment differs")
            harness = bodies[prefix + "inputs/expectations/physical-harness.tsv"]
            require(bodies[prefix + "evidence/physical/harness.tsv"] == bodies[prefix + "evidence/physical/manifest.tsv"] == harness,
                    "independent compact physical expectations differ")
            physical = []
            for row in csv.DictReader(io.StringIO(harness.decode()), delimiter="\t"):
                stem = prefix + "evidence/executions/physical-" + row["case"]
                original = document(stem + ".json")
                schema.compare_result({"status": original["status"], "stdout": bodies[stem + ".stdout"], "stderr": bodies[stem + ".stderr"]},
                    {"status": int(row["status"]), "stdout": bytes.fromhex(row["stdout_hex"]), "stderr": bytes.fromhex(row["stderr_hex"])}, row["case"])
                physical.append({"case": row["case"], "status": original["status"]})
            require(document(prefix + "evidence/physical-comparison.json") == physical and len(physical) == 29,
                    "independent compact physical result body differs")
            require(document(prefix + "evidence/physical/input-binding.json")["source_commit"] == expected_head
                    and document(prefix + "evidence/physical/harness-receipt.json")["source_commit"] == expected_head,
                    "independent compact physical source differs")
            structure = document(prefix + "evidence/structure.json")
            sidecars = [Path(name).name for name in bodies if name.startswith(prefix + "evidence/executions/") and name.endswith(".structure.tsv")]
            schema.assert_structure_inventory(sidecars, input_manifest["replay_inventories"], len(structure["bounds"]), len(structure["phis"]))
            require(structure["bounds_sites"] == len(structure["bounds"]) and structure["pointer_phis"] == len(structure["phis"]),
                    "independent compact structural body differs")
            old_manifest = document(prefix + "inputs/expectations/old-ir-manifest.json")
            for row in old_manifest:
                committed = full["members"][prefix + "evidence/" + row["path"]]
                require(committed["bytes"] == row["length"] and committed["sha256"] == row["sha256"], "independent old-IR baseline commitment differs")
            old = document(prefix + "evidence/old-ir-comparison.json")
            require(old == {"files": len(old_manifest), "modules": sum(row["path"].endswith(".ll") for row in old_manifest),
                            "unique_modules": len({row["sha256"] for row in old_manifest if row["path"].endswith(".ll")}),
                            "inventory_sha256": sha256(bodies[prefix + "inputs/expectations/old-ir-inventory.tsv"]),
                            "current_inventory_sha256": sha256(schema.old_ir_resource_inventory(bodies[prefix + "inputs/expectations/old-ir-inventory.tsv"])),
                            "resource_successor": schema.OLD_IR_RESOURCE_SUCCESSOR}, "independent old-IR comparison body differs")
            current_inventory = schema.old_ir_resource_inventory(bodies[prefix + "inputs/expectations/old-ir-inventory.tsv"])
            current_commitment = full["members"][prefix + "evidence/old-ir/inventory.tsv"]
            require(current_commitment["bytes"] == len(current_inventory) and current_commitment["sha256"] == sha256(current_inventory),
                    "independent current physical TSV commitment differs")
            supplement = document(prefix + "inputs/expectations/supplement-v1.json")
            cases = [{**supplement["positive_shared_alias"], "name": "shared-array-aliases"}, *supplement["extreme_cases"]]
            for case in cases:
                stem = prefix + "evidence/executions/" + case["name"]
                original = document(stem + ".json")
                schema.compare_result({"status": original["status"], "stdout": bodies[stem + ".stdout"], "stderr": bodies[stem + ".stderr"]},
                    {"status": case["status"], "stdout": bytes.fromhex(case["stdout_hex"]), "stderr": bytes.fromhex(case["stderr_hex"])}, case["name"])
            require(document(prefix + "evidence/supplement-comparison.json")["cases"] == [case["name"] for case in cases],
                    "independent compact supplement comparison body differs")
    require(file_record(archive_path)["sha256"] == index["compact"]["sha256"], "compact archive changed during audit")
    return {"status": "PASS", "head": expected_head, "event_sha": event_sha,
            "complete_unit2e_qualification": index["complete_unit2e_qualification"],
            "selected_body_members": len(compact["members"]), "full_only_members": len(compact["full_only_members"]),
            "scope": "downloaded original-body and identity-commitment consistency only; full-only binaries/IR were not retrieved or replayed"}


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
    package.add_argument("--independent-root", type=Path)
    package.add_argument("--independent-debug-outcome")
    package.add_argument("--independent-release-outcome")
    independent = actions.add_parser("independent")
    independent.add_argument("--producer-root", required=True, type=Path)
    independent.add_argument("--output", required=True, type=Path)
    independent.add_argument("--profile", required=True, choices=PROFILES)
    audit = actions.add_parser("audit-compact")
    audit.add_argument("--index", required=True, type=Path)
    audit.add_argument("--archive", required=True, type=Path)
    for action in (run, package, independent, audit):
        action.add_argument("--expected-head", required=True)
        action.add_argument("--event-sha", required=True)
    args = parser.parse_args()
    try:
        require(sys.flags.optimize == 0 and os.environ.get("PYTHONOPTIMIZE", "0") in ("", "0"), "optimized Python is unsupported")
        if args.action == "audit-compact":
            result = audit_compact(args.index, args.archive, args.expected_head, args.event_sha)
            print(json.dumps(result, sort_keys=True))
            return 0
        if args.action == "run":
            return execute_run(args)
        if args.action == "independent":
            return execute_independent(args)
        selected = None
        if args.independent_root is not None:
            selected = {profile: {"root": no_symlink_path(args.independent_root / profile),
                         "outcome": getattr(args, "independent_" + profile + "_outcome") or ""} for profile in PROFILES}
        return package_evidence(args.producer_root, args.output,
                                ci_identity(args.expected_head, args.event_sha), args.producer_step_outcome,
                                independent=selected)
    except (Exception, KeyboardInterrupt) as exc:
        print(f"Unit2E producer boundary failed: {type(exc).__name__}: {exc}", file=sys.stderr)
        return getattr(exc, "code", 1)


if __name__ == "__main__":
    raise SystemExit(main())
