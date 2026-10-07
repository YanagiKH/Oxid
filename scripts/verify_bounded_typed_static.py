#!/usr/bin/env python3
"""Qualify the bounded parser -> AST1 -> scalar static consumer.

The host frames unchanged parser output or relays its failure. Complete typed
facts / first diagnostics come from the existing canonical static observer and
are compared through the unchanged static_typed_observation projection. This
qualifies a component using an externally qualified compiler, not the compiler,
a provider, self-hosting, or a filesystem sandbox.
"""
import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

import build_typed_lexer_observer as lexer_builder
import build_typed_static_observer as static_builder
from verify_bounded_typed_lexer import digest
from verify_bounded_typed_parser import inventory, observer_identity, require, save


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/bounded_typed_static"
ROSTER_SHA256 = "985c6f5286b86591bdb5945ef13df66bce7b5bbdd2ff4cdb8e8a4cd7742a2df3"
CONTROLS_SHA256 = "6790f03fb0cc09ecd476954ade897a09e90f1d7f9e82bab72f71e6d6686b347a"
IO_SOURCES = {
    "stdout_prefix_eio.c": "12a9dd506a82d466a9af348da7d43159f9313781fa9b05aeaa2e1a8b3a0b26c3",
    "calibrate_stdout_prefix.c": "2499d674ba045ecf980f3b3f283fb8527f9f38d9c2fe60b4fcd5abf075489ce6",
}
RELAYS = {"canonical-036", "canonical-037", "canonical-038", "canonical-039"}
ROOTS = {"parser": "parser_main.ox", "consumer": "ast_static_main.ox"}
MEMBERS = tuple(name + ".ox" for name in (
    "ast_block ast_expression ast_input ast_output ast_statement ast_static_main ast_validate "
    "buffers keywords lexer_core parser_atom parser_call parser_control parser_driver "
    "parser_expression parser_main parser_output parser_signature parser_state parser_statement "
    "resolver_driver resolver_names static_column static_common static_state typed_diagnostic "
    "typed_driver typed_expression typed_output typed_statement").split())
PROJECTIONS = ("parser_ast_observation", "static_resolution_observation", "static_typed_observation")
MALFORMED_NAMES = tuple((
    "unsigned-overflow-column-0-128 unsigned-overflow-column-1-255 unsigned-overflow-column-2-128 "
    "inactive-column-1 out-of-range-kind-active-row reversed-span function-list-cycle "
    "function-list-detaches-second shared-parameter-type cross-function-shared-body "
    "block-statement-role-confusion argument-detaches-sibling function-order-repaired "
    "right-associative-subtraction chained-comparison source-nonascii source-unterminated-comment "
    "source-length129 opa-failure-status row-count129 source-length-mismatch truncated-at-4 "
    "nonzero-sentinel group-child-self-cycle prefix-height call-wrong-callee-token "
    "negative-number-canonical-shape control-after-condition truncated-0 "
    "truncated-final-byte trailing-two-witnesses").split())
IO_NAMES = ("directory-stdin", "integer-sigpipe-default", "integer-sigpipe-ignored",
            "type_error-sigpipe-default", "type_error-sigpipe-ignored")
BASELINE_NAMES = ("integer", "type_error")
# Existing fixed prefix locations: before output, inside OPA1, at its boundary,
# inside the STF1 header, and one byte before the complete output ends.
PREFIXES = {"integer": (0, 1, 1559, 1563, 2606), "type_error": (0, 1, 1559, 1563, 1574)}
INJECTED_NAMES = tuple(name + "-prefix-" + str(prefix) for name in BASELINE_NAMES for prefix in PREFIXES[name])


def load_cases(path):
    require(digest(path) == ROSTER_SHA256, "fixed 81-case authored roster changed")
    manifest = json.loads(path.read_bytes())
    require(set(manifest) == {"scope", "cases"}, "invalid authored case manifest")
    cases = manifest["cases"]
    require(len(cases) == len({case["name"] for case in cases}) == 81,
            "exact nonzero 81-case selection required")
    for case in cases:
        require(set(case) == {"name", "source", "origin"}, "invalid authored case fields")
        source = case["source"].encode("ascii")
        require(len(source) <= 128, "source outside bounded ASCII matrix")
        require(re.fullmatch(r"[A-Za-z0-9_-]+", case["name"]), "invalid case name")
    require(RELAYS <= {case["name"] for case in cases}, "missing fixed parser relay")
    return cases


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def snapshot_sources(destination):
    """Copy just the fixed closure of the two real roots, excluding the probe."""
    source = ROOT / "fixtures/typed-lexer-samples"
    closure, pending = set(), list(ROOTS.values())
    while pending:
        name = pending.pop()
        if name in closure:
            continue
        require(name in MEMBERS, "root imported a module outside the fixed closure")
        closure.add(name)
        pending.extend(module + ".ox" for module in
                       re.findall(r"\bmod\s+(\w+)\s*;", (source / name).read_text()))
    require(closure == set(MEMBERS), "fixed root source closure changed")
    candidate, host = destination / "candidate", destination / "host"
    candidate.mkdir(parents=True)
    host.mkdir()
    for name in MEMBERS:
        shutil.copyfile(source / name, candidate / name)
    for name in PROJECTIONS:
        shutil.copyfile(ROOT / "scripts" / (name + ".py"), host / (name + ".py"))
    shutil.copyfile(FIXTURES / "observer/observe.py", host / "canonical_observe.py")
    shutil.copyfile(FIXTURES / "malformed_controls.py", host / "malformed_controls.py")
    for name, expected in IO_SOURCES.items():
        require(digest(FIXTURES / name) == expected, "fixed I/O calibration source changed: " + name)
        shutil.copyfile(FIXTURES / name, host / name)
    return candidate, host


def check_execution(result, remaining, *, status=0, expected_remaining=b"", empty_output=False):
    require(result.returncode == status, "unexpected process exit status")
    require(not result.stderr, "unexpected process stderr")
    require(remaining == expected_remaining, "unexpected stdin consumption")
    if empty_output:
        require(result.stdout == b"", "refusal / I/O failure emitted stdout")


def run_process(directory, command, data, cwd, *, clear=False, io_kind=None, prefix=None, shim=None):
    """Retain real streams and seekable input consumption, including failures.

    Directory and closed-pipe controls use real kernel EISDIR/EPIPE. Prefix
    controls separately inject fd1-only short writes/EIO using the fixed shim;
    they are not real-kernel partial-write evidence.
    """
    directory.mkdir(parents=True, exist_ok=False)
    argv = [str(part) for part in command]
    require(io_kind in (None, "directory-stdin", "sigpipe-default", "sigpipe-ignored"),
            "unknown I/O control")
    require(prefix is None or (isinstance(prefix, int) and prefix >= 0 and shim is not None and io_kind is None),
            "invalid injected prefix control")
    (directory / "input.bin").write_bytes(data)
    stream = (directory / "input.bin").open("rb", buffering=0)
    input_fd, pipe_fd, directory_fd, output_file = stream.fileno(), None, None, None
    stdout, stderr, remaining = b"", b"", b""
    started = time.monotonic()
    receipt = {"command": argv, "cwd": str(cwd), "cleared_environment": clear,
               "input_sha256": digest(directory / "input.bin"), "input_bytes": len(data),
               "input_mode": "seekable exact bytes", "output_mode": "captured pipe",
               "io_kind": io_kind, "injected_prefix_bytes": prefix, "child_only_environment_overrides": {}}
    try:
        kwargs = {}
        environment = {} if clear else None
        if io_kind == "directory-stdin":
            directory_fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
            input_fd = directory_fd
            receipt["input_mode"] = "directory FD: real read failure EISDIR"
        elif io_kind:
            read_fd, pipe_fd = os.pipe()
            os.close(read_fd)
            policy = signal.SIG_DFL if io_kind == "sigpipe-default" else signal.SIG_IGN
            kwargs = {"preexec_fn": lambda: signal.signal(signal.SIGPIPE, policy),
                      "restore_signals": False}
            receipt["output_mode"] = "real pipe with no readers; zero bytes delivered"
        if prefix is not None:
            environment = {} if clear else os.environ.copy()
            require("LD_PRELOAD" not in environment, "refuse to merge unrelated preload configuration")
            output_file = (directory / "stdout").open("wb", buffering=0)
            target = os.fstat(output_file.fileno())
            overrides = {"LD_PRELOAD": str(shim), "OXID_STDOUT_PREFIX_BYTES": str(prefix),
                         "OXID_STDOUT_TARGET_DEV": str(target.st_dev), "OXID_STDOUT_TARGET_INO": str(target.st_ino)}
            environment.update(overrides)
            receipt["child_only_environment_overrides"] = overrides
            receipt["output_mode"] = "regular file; calibrated fd1-only injected short write/EIO"
        output_fd = output_file.fileno() if output_file is not None else pipe_fd
        result = subprocess.run(argv, stdin=input_fd,
                                stdout=output_fd if output_fd is not None else subprocess.PIPE,
                                stderr=subprocess.PIPE, cwd=cwd, env=environment,
                                timeout=120, check=False, **kwargs)
        stdout, stderr = result.stdout or b"", result.stderr or b""
        receipt["status"] = result.returncode
        return_value = subprocess.CompletedProcess(argv, result.returncode, stdout, stderr)
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b"", error.stderr or b""
        receipt["status"] = "timeout"
        raise
    except Exception as error:
        receipt.update(status="execution-error", error=type(error).__name__ + ": " + str(error))
        raise
    finally:
        remaining = stream.read()
        consumed = len(data) - len(remaining)
        stream.close()
        if directory_fd is not None:
            os.close(directory_fd)
        if pipe_fd is not None:
            os.close(pipe_fd)
        if output_file is not None:
            output_file.close()
            stdout = (directory / "stdout").read_bytes()
        (directory / "stdout").write_bytes(stdout)
        (directory / "stderr").write_bytes(stderr)
        (directory / "remaining-stdin.bin").write_bytes(remaining)
        receipt.update(elapsed_seconds=time.monotonic() - started,
                       input_consumed=None if directory_fd is not None else consumed,
                       remaining_hex=remaining.hex(), stdout_bytes=len(stdout), stderr_bytes=len(stderr),
                       stdout_sha256=digest(directory / "stdout"), stderr_sha256=digest(directory / "stderr"))
        save(directory / "receipt.json", receipt)
    return_value.stdout = stdout
    return return_value, remaining


def expectation(case, canonical):
    """The sole named refusal has its own contract, never diagnostic parity."""
    if case["name"] == "canonical-039":
        require(case["source"] == "struct S{x:i32}" and canonical == {
            "schema": "canonical-static-observation-1", "status": "outside_subset", "family": "record",
            "span": {"file_id": 0, "start": 0, "end": 15}}, "canonical-039 refusal identity changed")
        return {**canonical, "span": {"file_id": 0, "start": 0, "end": 6}}
    require(canonical.get("status") in ("ok", "diagnostic", "lexical_diagnostic"),
            "unclassified canonical status: " + case["name"])
    return canonical


def frame_parser_output(raw, source):
    require(len(source) <= 128 and source.isascii(), "source outside AST1 domain")
    require(len(raw) >= 11 and raw[:4] == b"OPA1", "invalid parser header")
    if raw[4]:
        require(len(raw) == 11, "parser failure retained bytes")
        return None
    require(len(raw) == 1559 and raw[5:8] == b"\0\0\0" and raw[10] == len(source),
            "invalid successful parser framing")
    return b"AST1" + bytes([len(source)]) + source + raw


def completed(report, cases, modes):
    expected = {(case["name"], mode, "parser-relay" if case["name"] in RELAYS else "ast1-pair")
                for case in cases for mode in modes}
    checks = report["checks"]
    require(len(checks) == len(expected)
            and {(c["case"], c["mode"], c["route"]) for c in checks} == expected
            and all(c["passed"] for c in checks), "incomplete or duplicate 77+4 results")
    groups = (("malformed", MALFORMED_NAMES), ("io", IO_NAMES),
              ("baseline", BASELINE_NAMES), ("injected", INJECTED_NAMES))
    for group, names in groups:
        wanted = {(name, mode) for name in names for mode in modes}
        records = report[group]
        require(len(records) == len(wanted)
                and {(c["case"], c["mode"]) for c in records} == wanted
                and all(c["passed"] for c in records), "incomplete " + group + " controls")
    io_pairs = {(group, name) for group, names in groups for name in names} if "native" in modes else set()
    require(len(report["io_pairs"]) == len(io_pairs)
            and {(c["group"], c["case"]) for c in report["io_pairs"]} == io_pairs
            and all(c["passed"] for c in report["io_pairs"]), "incomplete 48 I/O reference/native pairs")
    require(report["calibration"].get("passed") is True, "missing fd1-only injection calibration")
    require(len(report["io_build"]) == 2 and {c["name"] for c in report["io_build"]} == {"shim", "calibrator"}
            and all(c["passed"] for c in report["io_build"]), "incomplete I/O support compilation")
    wanted_pairs = {case["name"] for case in cases} if "native" in modes else set()
    require(len(report["wire_pairs"]) == len(wanted_pairs)
            and {c["case"] for c in report["wire_pairs"]} == wanted_pairs
            and all(c["passed"] for c in report["wire_pairs"]), "incomplete reference/native byte parity")
    setup = {(root, stage) for root in ROOTS for stage in
             (["check", "compile"] if "native" in modes else ["check"])}
    require(len(report["setup"]) == len(setup)
            and {(c["root"], c["stage"]) for c in report["setup"]} == setup
            and all(c["passed"] for c in report["setup"]), "incomplete root setup")
    report["counts"] = {mode: {**dict(Counter(c["route"] for c in checks if c["mode"] == mode)),
                              "pending": 0, "malformed": 31, "io": 5, "baseline": 2,
                              "injected": 10, "io_executions": 48} for mode in modes}


def qualify(compiler, static_observer, lexer_observer, output, native=False):
    cases = load_cases(FIXTURES / "cases.json")
    require(digest(FIXTURES / "malformed_controls.py") == CONTROLS_SHA256,
            "fixed malformed-control lineage changed")
    require(sys.platform.startswith("linux"), "actual qualification requires Linux process I/O and ELF conventions")
    output.mkdir(parents=True, exist_ok=False)
    report = {"passed": False, "native": native, "checks": [], "malformed": [], "io": [],
              "baseline": [], "injected": [], "io_pairs": [], "io_build": [], "calibration": {},
              "wire_pairs": [], "setup": [], "compiler_sha256": digest(compiler),
              "controller_sha256": digest(Path(__file__)), "case_roster_sha256": ROSTER_SHA256,
              "malformed_controls_sha256": CONTROLS_SHA256,
              "scope": "81 fixed sources, complete typed facts / first diagnostic; 77 AST1 routes and four parser relays per mode",
              "compiler_scope": "Externally qualified compiler supplied by caller; compiler build/provenance qualification remains external",
              "execution_scope": "Two independent ELF processes; each native run has empty cwd and cleared environment except recorded injection overrides, with the fixed source snapshot renamed; no filesystem sandbox claim",
              "io_scope": "Existing 48 I/O executions per mode: two transport baselines, 31 retained malformed inputs, one real directory-input error, four zero-delivery closed-pipe failures and ten calibrated fd1-only injected prefixes; injected prefixes are not real-kernel partial-write evidence"}
    save(output / "report.json", report)
    try:
        report["static_observer"] = observer_identity(static_observer, static_builder, "static")
        report["lexer_observer"] = observer_identity(lexer_observer, lexer_builder, "lexer")
        empty, sources = output / "source-free", output / "source"
        empty.mkdir()
        candidate, host = snapshot_sources(sources)
        shutil.copyfile(FIXTURES / "cases.json", output / "cases.json")
        sys.dont_write_bytecode = True
        for name in PROJECTIONS:
            projection = load_module(name, host / (name + ".py"))
        canonical = load_module("bounded_static_canonical", host / "canonical_observe.py")
        controls = load_module("bounded_static_controls", host / "malformed_controls.py")
        fixed = controls.records()
        require(tuple(c["name"] for c in fixed["malformed"]) == MALFORMED_NAMES,
                "fixed malformed selection changed")
        require([c["name"] for c in fixed["output_failure_inputs"]] == ["integer", "type_error"],
                "fixed output failure selection changed")
        report["control_lineage"] = controls.LINEAGE
        report["source_sha256"] = inventory(sources)
        report["binary_sha256"] = {}
        report["io_artifact_sha256"] = {}
        modes = ["reference"] + (["native"] if native else [])
        entries = {kind: candidate / name for kind, name in ROOTS.items()}
        binaries = {kind: output / kind for kind in ROOTS}
        io_support = {"shim": output / "io-support/stdout_prefix_eio.so",
                      "calibrator": output / "io-support/calibrate_stdout_prefix"}
        commands = {mode: {kind: ([compiler, "run", entry, "--edition", "typed-preview", "--entry-mode", "process"]
                                 if mode == "reference" else [binaries[kind]])
                           for kind, entry in entries.items()} for mode in modes}

        def identities():
            require(inventory(sources) == report["source_sha256"], "fixed source snapshot changed")
            require(digest(compiler) == report["compiler_sha256"], "compiler changed")
            require(digest(Path(__file__)) == report["controller_sha256"], "controller changed")
            for kind, expected in report["binary_sha256"].items():
                require(digest(binaries[kind]) == expected, "native executable changed: " + kind)
            for kind, expected in report["io_artifact_sha256"].items():
                require(digest(io_support[kind]) == expected, "I/O support executable changed: " + kind)

        def execute(directory, command, data, mode=None, io_kind=None, prefix=None):
            identities()
            hidden = output / "retained-source"
            if mode == "native":
                require(not any(empty.iterdir()), "native cwd is not empty")
                sources.rename(hidden)
            try:
                return run_process(directory, command, data, empty,
                                   clear=mode != "reference", io_kind=io_kind, prefix=prefix,
                                   shim=io_support["shim"] if prefix is not None else None)
            finally:
                if mode == "native":
                    hidden.rename(sources)

        for kind, entry in entries.items():
            setup = [("check", [compiler, "check", entry, "--edition", "typed-preview"])]
            if native:
                setup.append(("compile", [compiler, "compile", entry, "--edition", "typed-preview",
                                           "--backend", "llvm", "--entry-mode", "process", "--output", binaries[kind]]))
            for stage, command in setup:
                result, remaining = execute(output / "setup" / kind / stage, command,
                                            b"unread setup input", "reference")
                check_execution(result, remaining, expected_remaining=b"unread setup input")
                report["setup"].append({"root": kind, "stage": stage, "passed": True})
                if stage == "compile":
                    require(binaries[kind].read_bytes()[:4] == b"\x7fELF", "expected ELF: " + kind)
                    report["binary_sha256"][kind] = digest(binaries[kind])
                save(output / "report.json", report)

        # Build the exact previously reviewed injection/calibration support.
        cc = shutil.which("cc")
        require(cc is not None, "cc is required for the retained I/O prefix controls")
        (output / "io-support").mkdir()
        for kind, command in (
            ("shim", [cc, "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror",
                      host / "stdout_prefix_eio.c", "-o", io_support["shim"], "-ldl"]),
            ("calibrator", [cc, "-O2", "-Wall", "-Wextra", "-Werror",
                            host / "calibrate_stdout_prefix.c", "-o", io_support["calibrator"]]),
        ):
            result, remaining = execute(output / "io-build" / kind, command, b"unread setup input", "reference")
            check_execution(result, remaining, expected_remaining=b"unread setup input")
            report["io_artifact_sha256"][kind] = digest(io_support[kind])
            report["io_build"].append({"name": kind, "passed": True})
            save(output / "report.json", report)
        calibration = output / "calibration"
        unrelated = calibration / "unrelated-fd.bin"
        result, remaining = execute(calibration, [io_support["calibrator"], unrelated], b"", prefix=3)
        require(result.returncode == 0 and result.stdout == b"abc" and result.stderr == b"diagnostic\n"
                and remaining == b"" and unrelated.read_bytes() == b"side-before\nside-after\n",
                "fd1-only injection calibration failed")
        report["calibration"] = {"passed": True, "scope": "seven-byte fd1 write returns three; next write gets EIO; stderr and unrelated FD unchanged"}
        save(output / "report.json", report)

        # Establish every independent expectation before candidate execution.
        expectations = {}
        for case in cases:
            data = case["source"].encode("ascii")
            directory = output / "canonical" / case["name"]
            identities()
            observed = canonical.observe(static_observer, compiler, data, directory / "static")
            expected = expectation(case, observed)
            save(directory / "expected.json", expected)
            result, remaining = execute(directory / "lexer", [lexer_observer], data)
            check_execution(result, remaining)
            lexical = json.loads(result.stdout)
            require(lexical.get("status") in ("ok", "diagnostic"), "invalid canonical lexical status")
            expectations[case["name"]] = (expected, lexical.get("tokens"))
        save(output / "report.json", report)

        reference_outputs, reference_io = {}, {}

        def record_io(group, name, mode, result, remaining, **metadata):
            observed = (result.returncode, result.stdout, result.stderr, remaining)
            if mode == "reference":
                reference_io[group, name] = observed
            else:
                require(reference_io[group, name] == observed, "I/O status/streams/consumption parity changed: " + name)
                report["io_pairs"].append({"group": group, "case": name, "passed": True})
            report[group].append({"case": name, "mode": mode, "passed": True, **metadata})
            save(output / "report.json", report)

        for mode in modes:
            for case in cases:
                name, data = case["name"], case["source"].encode("ascii")
                directory = output / mode / "cases" / name
                parser, remaining = execute(directory / "parser", commands[mode]["parser"], data, mode)
                check_execution(parser, remaining)
                frame = frame_parser_output(parser.stdout, data)
                require((frame is None) == (name in RELAYS), "fixed parser route changed: " + name)
                expected, tokens = expectations[name]
                if frame is None:
                    raw, route = parser.stdout, "parser-relay"
                else:
                    (directory / "frame.bin").write_bytes(frame)
                    consumer, remaining = execute(directory / "consumer", commands[mode]["consumer"], frame, mode)
                    check_execution(consumer, remaining)
                    raw, route = consumer.stdout, "ast1-pair"
                    require(raw[:1559] == parser.stdout, "consumer changed the parser's OPA1 prefix")
                actual = projection.decode(raw, data, tokens)
                save(directory / "projected.json", actual)
                require(actual == expected, "complete canonical/refusal mismatch: " + mode + "/" + name)
                if mode == "reference":
                    reference_outputs[name] = (parser.stdout, raw)
                else:
                    require(reference_outputs[name] == (parser.stdout, raw), "reference/native wire mismatch: " + name)
                    report["wire_pairs"].append({"case": name, "passed": True})
                report["checks"].append({"case": name, "mode": mode, "route": route,
                                         "status": actual["status"], "passed": True,
                                         "oracle": "named record-keyword refusal at 0..6; no canonical static diagnostic parity"
                                         if name == "canonical-039" else "canonical static observation"})
                save(output / "report.json", report)
            baselines = {}
            for control in fixed["output_failure_inputs"]:
                name, data = control["name"], control["data"]
                result, remaining = execute(output / mode / "baseline" / name, commands[mode]["consumer"], data, mode)
                check_execution(result, remaining)
                opa = data[5 + data[4]:]
                tag = 0 if name == "integer" else 1
                require(result.stdout[:1559] == opa and result.stdout[1559:1564] == b"STF1" + bytes([tag])
                        and len(result.stdout) == PREFIXES[name][-1] + 1, "I/O baseline framing changed")
                baselines[name] = result.stdout
                record_io("baseline", name, mode, result, remaining,
                          scope="ordinary consumer output; transport-prefix oracle only")
            for control in fixed["malformed"]:
                data = control["data"]
                result, remaining = execute(output / mode / "malformed" / control["name"],
                                            commands[mode]["consumer"], data, mode)
                check_execution(result, remaining, status=64, empty_output=True,
                                expected_remaining=data[control["consumed"]:])
                record_io("malformed", control["name"], mode, result, remaining,
                          origin=control["origin"], input_sha256=control["input_sha256"])
            result, remaining = execute(output / mode / "io/directory-stdin", commands[mode]["consumer"],
                                        b"", mode, "directory-stdin")
            check_execution(result, remaining, status=74, empty_output=True)
            record_io("io", "directory-stdin", mode, result, remaining)
            for control in fixed["output_failure_inputs"]:
                for policy in ("default", "ignored"):
                    name = control["name"] + "-sigpipe-" + policy
                    result, remaining = execute(output / mode / "io" / name, commands[mode]["consumer"],
                                                control["data"], mode, "sigpipe-" + policy)
                    check_execution(result, remaining, status=74, empty_output=True)
                    record_io("io", name, mode, result, remaining)
                for prefix in PREFIXES[control["name"]]:
                    name = control["name"] + "-prefix-" + str(prefix)
                    result, remaining = execute(output / mode / "injected" / name, commands[mode]["consumer"],
                                                control["data"], mode, prefix=prefix)
                    check_execution(result, remaining, status=74)
                    require(result.stdout == baselines[control["name"]][:prefix], "injected output was not the exact observed prefix")
                    record_io("injected", name, mode, result, remaining,
                              scope="calibrated injected short write/EIO; no real-kernel partial-write claim")
        identities()
        require(observer_identity(static_observer, static_builder, "static") == report["static_observer"]
                and observer_identity(lexer_observer, lexer_builder, "lexer") == report["lexer_observer"],
                "canonical observers changed")
        completed(report, cases, modes)
        report["passed"] = True
    except Exception as error:
        report["error"] = type(error).__name__ + ": " + str(error)
        raise
    finally:
        save(output / "report.json", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oxid", type=Path, required=True, help="externally built and qualified compiler")
    parser.add_argument("--static-observer", type=Path, required=True)
    parser.add_argument("--lexer-observer", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="fresh evidence directory")
    parser.add_argument("--native", action="store_true", help="also compile and execute both roots with LLVM")
    args = parser.parse_args()
    try:
        report = qualify(args.oxid.resolve(strict=True), args.static_observer.resolve(strict=True),
                         args.lexer_observer.resolve(strict=True), args.output.resolve(), args.native)
    except (OSError, ValueError, AssertionError, subprocess.SubprocessError) as error:
        print("bounded typed static qualification failed: " + str(error), file=sys.stderr)
        return 1
    print(json.dumps({"passed": report["passed"], "counts": report["counts"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
