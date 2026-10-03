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
    return json.loads(Path(path).read_bytes(), object_pairs_hook=pairs)


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


def stable_bytes(path):
    """Anchor every ancestor with directory descriptors; never follow symlinks."""
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
            data = handle.read(before.st_size + 1)
            require(len(data) == before.st_size, "file changed size during capture")
            require(stat_identity(os.fstat(handle.fileno())) == stat_identity(before), "file changed during capture")
        require(stat_identity(os.stat(path.name, dir_fd=fd, follow_symlinks=False)) == stat_identity(before),
                "file replaced during capture")
        for parent, part, value in parents:
            now = os.stat(part, dir_fd=parent, follow_symlinks=False)
            require((now.st_dev, now.st_ino, now.st_mode) == (value.st_dev, value.st_ino, value.st_mode),
                    "ancestor changed during capture")
        return data, stat.S_IMODE(before.st_mode)
    finally:
        os.close(fd)
        for parent, _, _ in parents:
            os.close(parent)


def file_record(path):
    data, mode = stable_bytes(path)
    return {"bytes": len(data), "mode": mode, "sha256": sha256(data)}


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
