#!/usr/bin/env python3
"""Staged independent owned-source qualification, with at most two workers.

Model mode is safe before source activation. Production mode exercises exact
provided compiler binaries through ordinary check/run/compile, real ELF output
and source-free execution. It cannot report full qualification without separate
candidate trace, exact-budget/LLVM-store and resource evidence. No public fuel
flag or production source-admission switch is introduced by this harness.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import math
import os
import re
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time

import owned_source_model as model


@dataclass(frozen=True)
class Job:
    name: str
    command: tuple[str, ...]
    summary: str


def digest(path):
    with Path(path).open("rb") as stream: return hashlib.file_digest(stream, "sha256").hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def strict_json_loads(text):
    def pairs(items):
        value = {}
        for key, child in items:
            if key in value: raise ValueError('duplicate JSON object key: ' + key)
            value[key] = child
        return value
    def nonfinite(value): raise ValueError('non-finite JSON scalar: ' + value)
    def finite_float(value):
        result = float(value)
        if not math.isfinite(result): raise ValueError('non-finite JSON exponent: ' + value)
        return result
    return json.loads(text, object_pairs_hook=pairs, parse_constant=nonfinite, parse_float=finite_float)


def freeze(directory):
    """Freeze source and facts before any compiler process or raw observation."""
    directory = Path(directory)
    if (directory / "manifest.json").exists():
        # Frozen evidence is immutable. A revised model needs a new directory;
        # never rewrite expectations after observing a candidate trace.
        return verify_frozen(directory)
    directory.mkdir(parents=True, exist_ok=True)
    expectations = []
    for case in model.corpus():
        item = model.case_expectation(case)
        source = item.pop("source")
        source_path, expectation_path = directory / (case.identifier + ".ox"), directory / (case.identifier + ".json")
        source_path.write_bytes(source.encode())
        write_json(expectation_path, item)
        expectations.append(item)
    manifest = model.coverage_manifest(expectations)
    manifest["files"] = [{"id": item["id"], "source_sha256": item["source_sha256"],
                           "expectation_sha256": digest(directory / (item["id"] + ".json"))} for item in expectations]
    manifest["model_sha256"] = digest(Path(model.__file__))
    write_json(directory / "manifest.json", manifest)
    return manifest


def verify_frozen(directory):
    directory = Path(directory)
    manifest = strict_json_loads((directory / "manifest.json").read_text())
    if manifest["model_sha256"] != digest(Path(model.__file__)):
        raise ValueError("model changed after freeze; regenerate before observing compiler output")
    cases = list(model.corpus())
    identifiers = [case.identifier for case in cases]
    files = manifest.get('files')
    if type(files) is not list or any(type(entry) is not dict for entry in files):
        raise ValueError('invalid frozen file inventory')
    actual = [entry.get('id') for entry in files]
    if actual != identifiers or len(set(actual)) != len(identifiers):
        raise ValueError('frozen corpus must contain the complete unique ordered model inventory')
    if type(manifest.get('unique_source_programs')) is not int or manifest['unique_source_programs'] != len(identifiers):
        raise ValueError('frozen corpus count does not match independent model inventory')
    if {p.name for p in directory.glob('*.ox')} != {name+'.ox' for name in identifiers}:
        raise ValueError('frozen source directory inventory mismatch')
    if {p.name for p in directory.glob('*.json')} != {name+'.json' for name in identifiers}|{'manifest.json'}:
        raise ValueError('frozen expectation directory inventory mismatch')
    for case, entry in zip(cases, files):
        expected = model.case_expectation(case)
        source = expected.pop('source').encode()
        encoded = (json.dumps(expected, indent=2, sort_keys=True)+'\n').encode()
        if hashlib.sha256(source).hexdigest() != entry['source_sha256'] or hashlib.sha256(encoded).hexdigest() != entry['expectation_sha256']:
            raise ValueError('frozen source/expectation differs from independent model: ' + entry['id'])
        if digest(directory / (entry["id"] + ".ox")) != entry["source_sha256"]:
            raise ValueError("frozen source changed: " + entry["id"])
        if digest(directory / (entry["id"] + ".json")) != entry["expectation_sha256"]:
            raise ValueError("frozen expectation changed: " + entry["id"])
    return manifest


def signal_group(process, sig):
    try:
        os.killpg(process.pid, sig)
        return True
    except ProcessLookupError: return False


def stop_groups(processes):
    live = [process for process in processes if signal_group(process, signal.SIGTERM)]
    deadline = time.monotonic() + 1
    while live and time.monotonic() < deadline:
        for process in live: process.poll()
        live = [process for process in live if signal_group(process, 0)]
        if live: time.sleep(.01)
    for process in live: signal_group(process, signal.SIGKILL)
    for process in processes: process.wait()


def has_terminal_summary(path, summary):
    with path.open("rb") as stream:
        stream.seek(0, os.SEEK_END)
        stream.seek(max(0, stream.tell() - 8192))
        tail = stream.read()
    return tail.endswith(b"\n") and bool(tail.splitlines()) and tail.splitlines()[-1] == summary.encode()


def run_jobs(jobs_to_run, evidence, *, jobs=1, timeout=1800, output=None):
    """Bounded process sessions, checked status, ordered full logs, fail closed.

    Profiles are worker processes, each issuing one command at a time. Thus at
    most two compiler/native subprocesses can execute simultaneously. On failure,
    pending jobs are NOT RUN and every remaining process group is cleaned up.
    """
    if os.name != "posix": raise ValueError("qualification requires POSIX process groups")
    if jobs not in (1, 2) or not math.isfinite(timeout) or timeout <= 0:
        raise ValueError("jobs must be 1 or 2; timeout must be finite and positive")
    if not jobs_to_run or len({job.name for job in jobs_to_run}) != len(jobs_to_run):
        raise ValueError("jobs require nonempty unique names")
    evidence = Path(evidence)
    evidence.mkdir(parents=True, exist_ok=True)
    output = sys.stdout.buffer if output is None else output
    interrupted, active, owned, status = [0], {}, [], {}
    def cancelled(signum, _frame):
        if not interrupted[0]: interrupted[0] = signum
    old = {sig: signal.signal(sig, cancelled) for sig in (signal.SIGINT, signal.SIGTERM)}
    logs = [evidence / f"{index:02d}-{job.name}.log" for index, job in enumerate(jobs_to_run)]
    next_job, failure = 0, 0
    try:
        while next_job < len(jobs_to_run) or active:
            if interrupted[0]: failure = 128 + interrupted[0]; break
            # Observe every existing worker before launching any replacement.
            for index, (process, started) in list(active.items()):
                code = process.poll()
                if code is None and time.monotonic() - started >= timeout:
                    status[index] = "TIMEOUT"; failure = 124; break
                if code is not None:
                    del active[index]
                    if code:
                        status[index] = f"FAIL (exit {code})"; failure = code if code > 0 else 128 - code
                    elif not has_terminal_summary(logs[index], jobs_to_run[index].summary):
                        status[index] = "FAIL (missing checked terminal summary)"; failure = 1
                    elif signal_group(process, 0):
                        status[index] = "FAIL (running descendants after terminal status)"; failure = 1
                    else:
                        status[index] = "PASS"; owned.remove(process)
                    if failure: break
            if failure: break
            while next_job < len(jobs_to_run) and len(active) < jobs and not interrupted[0]:
                index, next_job = next_job, next_job + 1
                with logs[index].open("wb") as log:
                    process = subprocess.Popen(jobs_to_run[index].command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                active[index] = process, time.monotonic()
                owned.append(process)
            if active: time.sleep(.01)
    except (OSError, ValueError) as error:
        failure = 1
        output.write(("source harness orchestration failure: " + str(error) + "\n").encode())
    finally:
        try:
            stop_groups(owned)
            failure = failure or (128 + interrupted[0] if interrupted[0] else 0)
            results = []
            for index, job in enumerate(jobs_to_run):
                outcome = status.get(index, "CANCELLED" if index < next_job else "NOT RUN")
                results.append({"name": job.name, "status": outcome, "log": str(logs[index])})
                output.write(("=== " + job.name + " ===\n").encode())
                if logs[index].exists():
                    with logs[index].open("rb") as log: shutil.copyfileobj(log, output, 65536)
                output.write(("\n" + job.name + ": " + outcome + "\n").encode())
            passed = sum(result["status"] == "PASS" for result in results)
            failure = failure or int(passed != len(jobs_to_run))
            write_json(evidence / "worker-status.json", {"exit": failure, "passed": passed, "jobs": jobs, "workers": results})
            output.write(f"owned-source workers: {passed}/{len(jobs_to_run)} passed; jobs={jobs}; exit={failure}\n".encode())
            output.flush()
        finally:
            for sig, handler in old.items(): signal.signal(sig, handler)
    return failure


def parse_records(completed, operation=None):
    if completed.stderr: raise ValueError("JSON command wrote unexpected stderr")
    try: records = [strict_json_loads(line) for line in completed.stdout.splitlines()]
    except (ValueError, UnicodeDecodeError) as error: raise ValueError("invalid JSON output") from error
    if not records or any(type(record) is not dict for record in records):
        raise ValueError("JSON records must be objects ending in one command summary")
    allowed = {"check-summary", "run-summary", "compile-summary"}
    summaries = [index for index, record in enumerate(records) if record.get("kind") in allowed]
    if summaries != [len(records) - 1] or any(record.get("kind") != "diagnostic" for record in records[:-1]):
        raise ValueError("require exactly one terminal command summary after diagnostic records only")
    summary, diagnostics = records[-1], records[:-1]
    if operation is not None and summary["kind"] != operation + "-summary":
        raise ValueError("terminal summary belongs to the wrong command")
    if any(type(record.get("schema_version")) is not int or record["schema_version"] != 1 for record in records):
        raise ValueError("unexpected JSON schema version or scalar type")
    if type(summary.get("success")) is not bool:
        raise ValueError("summary success must be a JSON boolean")
    if summary.get("edition") != "typed-preview": raise ValueError("summary did not use the requested typed-preview edition")
    if type(summary.get("errors")) is not int or summary["errors"] != len(diagnostics):
        raise ValueError("summary error count must exactly match diagnostics")
    if summary["success"] and diagnostics: raise ValueError("successful summary cannot contain error diagnostics")
    if not summary["success"] and not diagnostics: raise ValueError("failed summary must contain an error diagnostic")
    if summary["success"] != (completed.returncode == 0): raise ValueError("summary and terminal exit disagree")
    for diagnostic in diagnostics:
        if type(diagnostic.get("code")) is not str or type(diagnostic.get("stage")) is not str:
            raise ValueError("diagnostic code/stage must be JSON strings")
        primary = diagnostic.get("primary")
        if primary is not None:
            if type(primary) is not dict or any(type(primary.get(key)) is not int for key in ("start", "end")):
                raise ValueError("diagnostic byte offsets must be JSON integers")
            if not 0 <= primary["start"] <= primary["end"]: raise ValueError("invalid diagnostic byte range")
            if any(key in primary and (type(primary[key]) is not int or primary[key] < 1) for key in ("line", "column")):
                raise ValueError("diagnostic human coordinates must be positive JSON integers")
    payload = {"check-summary": "functions", "run-summary": "result", "compile-summary": "output"}[summary["kind"]]
    if payload not in summary: raise ValueError("summary is missing its command payload")
    value = summary[payload]
    if not summary["success"]:
        if value is not None: raise ValueError("failed summary payload must be null")
    elif payload == "functions":
        if type(value) is not int or value < 0: raise ValueError("function count must be a nonnegative JSON integer")
    elif payload == "output":
        if type(value) is not str or not value: raise ValueError("compile output must be a nonempty JSON string")
    else:
        if type(value) is not dict: raise ValueError("successful run result must be a scalar object")
        ty = value.get("type")
        if ty == "unit":
            if set(value) != {"type"}: raise ValueError("unit result has no value field")
        elif ty in ("bool", "i32"):
            if set(value) != {"type", "value"} or type(value["value"]) is not (bool if ty == "bool" else int):
                raise ValueError("run result uses an incorrect JSON scalar type")
            if ty == "i32" and not model.MIN_I32 <= value["value"] <= model.MAX_I32:
                raise ValueError("run result is outside i32")
        else: raise ValueError("unknown run result type")
    return records


def match_diagnostic(records, expected, *, stage=None, code=None, interval=False):
    errors = [record for record in records if record.get("code")]
    if not errors: raise ValueError("missing expected diagnostic")
    error = errors[0]
    if error["code"] != (code or expected["code"]) or error["stage"] != (stage or expected["stage"]):
        raise ValueError(f"diagnostic stage/code mismatch: {error.get('stage')}/{error.get('code')}")
    def span(value):
        if value is None: return None
        if type(value) is not dict or any(type(value.get(key)) is not int for key in ("start", "end")):
            raise ValueError("diagnostic span must contain exact JSON integers")
        if not 0 <= value["start"] <= value["end"]: raise ValueError("invalid diagnostic span")
        return [value["start"], value["end"]]
    primary = span(error.get("primary"))
    related = [span(label.get("span")) for label in error.get("secondary", [])]
    selected = {"stage": error["stage"], "code": error["code"], "span": primary, "related": related}
    alternatives = expected.get("diagnostic_alternatives")
    if alternatives:
        if len(errors) != 1: raise ValueError("alias ownership verification must select exactly one diagnostic")
        allowed = [{"stage": item["stage"], "code": item["code"], "span": item["span"],
                    "related": [item["cause_span"], item["declaration_span"]]} for item in alternatives]
        if selected not in allowed:
            raise ValueError("diagnostic is not an exact source-defined conflict/cause/declaration alternative")
        return selected
    if "span" in expected:
        wanted = expected["span"]
        if wanted is None:
            if primary is not None: raise ValueError("diagnostic unexpectedly fabricated a primary origin")
        else:
            if primary is None: raise ValueError("diagnostic lost primary origin")
            valid = wanted[0] <= primary[0] <= primary[1] <= wanted[1] if interval else primary == wanted
            if not valid: raise ValueError(f"diagnostic origin mismatch: {primary}, expected {wanted}")
    return selected


def scalar_text(ty, value):
    if ty == "()": return b"()\n"
    if ty == "bool": return b"true\n" if value else b"false\n"
    return (str(value) + "\n").encode()


def verify_text_result(completed, item, source):
    """Each human/ELF result is checked against the frozen oracle independently."""
    expected = item["expected"]
    failure = expected.get("runtime_failure")
    if not failure:
        wanted = (0, scalar_text(expected["result_type"], expected["result"]), b"")
        if (completed.returncode, completed.stdout, completed.stderr) != wanted:
            raise ValueError("text scalar result disagrees with frozen expectation")
        return
    code = failure["code"]
    if code not in ("E0600", "E0601", "E0604"):
        raise ValueError("incomplete/unsupported runtime expectation: " + code)
    if completed.returncode != 1 or completed.stdout:
        raise ValueError("expected runtime failure must exit 1 with empty stdout")
    prefix = f"error[{code}] (oir-run): ".encode()
    if not completed.stderr.startswith(prefix):
        raise ValueError("runtime failure text has the wrong code/stage")
    origin = failure["origin"]
    location = b""
    if origin is not None:
        start, _ = item["origins"][origin]
        before = source.read_bytes()[:start].decode("utf-8")
        line, column = before.count("\n") + 1, len(before.rsplit("\n", 1)[-1]) + 1
        location = f"  --> {source.name}:{line}:{column}\n".encode()
        if location not in completed.stderr: raise ValueError("runtime text lost the exact source location")
    if code in ("E0601", "E0604"):
        message = "execution fuel exhausted" if code == "E0601" else "checked i32 arithmetic overflow"
        if completed.stderr != prefix + message.encode() + b"\n" + location:
            raise ValueError("runtime failure text disagrees with the fixed scalar contract")


def profile_worker(binary, profile, corpus_dir, evidence):
    """Actual ordinary CLI only. Test-only facades are never counted here."""
    binary, corpus_dir, evidence = Path(binary).resolve(), Path(corpus_dir).resolve(), Path(evidence).resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    manifest = verify_frozen(corpus_dir)
    before = digest(binary)
    counts = {"cli_invocations": 0, "native_executions": 0, "unique_elf_artifacts": 0, "source_programs": 0}
    artifacts, diagnostic_selections = [], {}
    def command(args, *, cwd, environment=None, timeout=30):
        counts["cli_invocations"] += int(args[0] == str(binary))
        number = counts["cli_invocations"] + counts["native_executions"]
        stem = evidence / f"command-{number:05d}"
        try:
            result = subprocess.run(args, cwd=cwd, env=environment, capture_output=True, timeout=timeout)
        except subprocess.TimeoutExpired as error:
            stem.with_suffix(".stdout").write_bytes(error.stdout or b"")
            stem.with_suffix(".stderr").write_bytes(error.stderr or b"")
            write_json(stem.with_suffix(".json"), {"argv": args, "cwd": str(cwd), "status": "TIMEOUT", "timeout_seconds": timeout})
            raise
        # Keep the complete ordered stdout/stderr bytes, even for a failed command.
        stem.with_suffix(".stdout").write_bytes(result.stdout)
        stem.with_suffix(".stderr").write_bytes(result.stderr)
        write_json(stem.with_suffix(".json"), {"argv": args, "cwd": str(cwd), "returncode": result.returncode})
        return result
    for entry in manifest["files"]:
        identifier = entry["id"]
        item = strict_json_loads((corpus_dir / (identifier + ".json")).read_text())
        case_dir = evidence / identifier
        case_dir.mkdir(exist_ok=True)
        source = case_dir / ("source 雪.ox" if identifier == "utf8-crlf-borrow-origin" else "source.ox")
        source.write_bytes((corpus_dir / (identifier + ".ox")).read_bytes())
        expected, output = item["expected"], case_dir / "native.elf"
        def invoke(operation, json_output=True, missing=False):
            args = [str(binary), operation, source.name, "--edition=typed-preview"]
            if json_output: args.append("--message-format=json")
            if operation == "compile": args += ["--backend=llvm", "--output", str(output)]
            env = dict(os.environ)
            if missing: env["OXID_LLVM_BIN"] = str(case_dir / "missing-tools")
            completed = command(args, cwd=case_dir, environment=env)
            if digest(source) != item["source_sha256"]:
                raise ValueError("source changed while the compiler was observing it")
            return completed
        if expected["status"] == "reject":
            for operation in ("check", "run", "compile"):
                sentinel = b"SOURCE-ERROR-NO-CLOBBER\n"
                if operation == "compile": output.write_bytes(sentinel)
                result = invoke(operation, missing=operation == "compile")
                if result.returncode != 1: raise ValueError(f"{identifier}: expected ordinary source failure, got {result.returncode}")
                records = parse_records(result, operation)
                failure_field = {"check": "functions", "run": "result", "compile": "output"}[operation]
                if failure_field not in records[-1] or records[-1][failure_field] is not None:
                    raise ValueError("failed command must have a null summary payload")
                selected = match_diagnostic(records, expected, interval=expected.get("span_match") == "within-authored-region")
                if expected.get("diagnostic_alternatives"):
                    if identifier in diagnostic_selections and diagnostic_selections[identifier] != selected:
                        raise ValueError("permitted ownership diagnostic selection changed between commands")
                    diagnostic_selections[identifier] = selected
                if operation == "compile" and output.read_bytes() != sentinel: raise ValueError("early source failure clobbered output")
        else:
            checked = invoke("check")
            if checked.returncode: raise ValueError(f"{identifier}: valid source rejected at check")
            check_records = parse_records(checked, "check")
            if check_records[-1].get("functions") != item["function_count"]:
                raise ValueError("check function count must exclude struct declarations")
            ran = invoke("run")
            records = parse_records(ran, "run")
            runtime_failure = expected.get("runtime_failure")
            if runtime_failure:
                if ran.returncode != 1: raise ValueError("expected runtime/entry failure")
                wanted = {"stage": "oir-run", "code": runtime_failure["code"],
                          "span": item["origins"].get(runtime_failure["origin"]) if runtime_failure["origin"] else None}
                match_diagnostic(records, wanted)
                if "result" not in records[-1] or records[-1]["result"] is not None:
                    raise ValueError("failed reference run must have a null result")
            else:
                if ran.returncode: raise ValueError("unexpected run failure")
                ty = "unit" if expected["result_type"] == "()" else expected["result_type"]
                wanted_result = {"type": "unit"} if ty == "unit" else {"type": ty, "value": expected["result"]}
                if model.correspondence_differences(wanted_result, records[-1].get("result")):
                    raise ValueError(f"{identifier}: scalar JSON result mismatch")
            human = invoke("run", False)
            verify_text_result(human, item, source)
            if runtime_failure and runtime_failure["code"] == "E0600":
                sentinel = b"ENTRY-ERROR-NO-CLOBBER\n"; output.write_bytes(sentinel)
                compiled = invoke("compile", missing=True)
                if compiled.returncode != 1: raise ValueError("owned entry must be rejected before native tools")
                match_diagnostic(parse_records(compiled, "compile"), {"stage": "native-admission", "code": "E0700", "span": item["origins"].get(runtime_failure["origin"])})
                if output.read_bytes() != sentinel: raise ValueError("entry error clobbered output")
            else:
                compiled = invoke("compile")
                if compiled.returncode: raise ValueError(f"{identifier}: native compilation failed")
                parse_records(compiled, "compile")
                if not output.is_file() or output.read_bytes()[:4] != b"\x7fELF": raise ValueError("compiler did not produce an ELF")
                hidden = case_dir / "source.hidden"
                source.rename(hidden)
                try:
                    counts["native_executions"] += 1
                    executed = command([str(output)], cwd=case_dir, environment={"PATH": "/nonexistent", "LC_ALL": "C"}, timeout=10)
                finally: hidden.rename(source)
                verify_text_result(executed, item, source)
                artifacts.append({"id": identifier, "sha256": digest(output), "path": str(output), "source_sha256": item["source_sha256"]})
                counts["unique_elf_artifacts"] += 1
        if list(case_dir.glob(".oxid-native-*")): raise ValueError("compiler left temporary native output")
        counts["source_programs"] += 1
        print(f"{profile} {identifier}: PASS", flush=True)
    if digest(binary) != before: raise ValueError("compiler changed during profile qualification")
    result = {"profile": profile, "binary": str(binary), "compiler_sha256": before, "counts": counts, "elf": artifacts, "diagnostic_selections": diagnostic_selections,
              "evidence_kind": "actual-production-cli-and-source-free-ELF", "llvm_text_gate": "PENDING",
              "exact_fuel_and_failure_before_store": "PENDING", "full_qualification": False}
    write_json(evidence / "profile-result.json", result)
    print(f"owned-source {profile}: CLI/ELF PASS", flush=True)


class RawInspector:
    def __init__(self, expected, source, raw, checked):
        self.expected, self.facts, self.origins = expected, expected['facts'], expected['origins']
        self.source, self.raw, self.checked = source.encode(), raw, checked
        self.issues, self.checks = [], 0
        self.record_ids, self.function_ids = {}, {}
        self.functions, self.slots, self.calls, self.owners = {}, {}, {}, {}
        self.local_defs = {}

    def same(self, want, actual, label):
        self.checks += 1
        if not self.equal(want, actual):
            self.issues.append({'fact': label, 'expected': want, 'observed': actual})

    @classmethod
    def equal(cls, want, actual):
        if type(want) is not type(actual): return False
        if isinstance(want, dict):
            return want.keys() == actual.keys() and all(cls.equal(value, actual[key]) for key, value in want.items())
        if isinstance(want, (list, tuple)):
            return len(want) == len(actual) and all(cls.equal(a, b) for a, b in zip(want, actual))
        return want == actual

    def source_nodes(self):
        def visit(value, name):
            if hasattr(value, 'tag') and hasattr(value, 'key'):
                yield name, value
                yield from visit(value.data, name)
            elif isinstance(value, dict):
                for child in value.values(): yield from visit(child, name)
            elif isinstance(value, (list, tuple)):
                for child in value: yield from visit(child, name)
        for function in self.checked.program.functions:
            yield from visit(function.body, function.name)

    def unique(self, rows, predicate, label):
        found = [row for row in rows if predicate(row)]
        self.same(1, len(found), label + '.count')
        return found[0] if len(found) == 1 else None

    def origin(self, key): return self.origins[key]

    def text(self, span):
        start, end = span
        if type(start) is not int or type(end) is not int or not 0 <= start <= end <= len(self.source):
            raise ValueError('invalid raw observation origin')
        return self.source[start:end].decode()

    def record_name(self, identity):
        record = next((record for record in self.raw['records'] if record['id'] == identity), None)
        return self.text(record['span']) if record else '<unknown raw record>'

    def ty(self, value):
        if value['mode'] == 'scalar': return value['type']
        record = self.record_name(value['record'])
        if value['mode'] == 'owned': return record
        if value['mode'] == 'shared': return '&' + record
        if value['mode'] == 'exclusive': return '&mut ' + record
        return '<unknown raw type>'

    def field_id(self, value):
        record = self.record_ids.get(value['record'])
        return f'{record}.field.{value["index"]}' if record else '<unknown nominal field>'

    def operations(self, function):
        for block in function['blocks']:
            for event in block['instructions'] + [block['terminator']]:
                yield block['id'], event

    def op(self, function, operation, span, extra=lambda _op: True):
        rows = [{'block': block, **event} for block, event in self.operations(function)]
        event = self.unique(rows, lambda event: event['instruction']['operation'] == operation and event['span'] == span and extra(event['instruction']), operation + str(span))
        if event and 'charge_span' in event: self.same(span, event['charge_span'], operation + str(span) + '.charge-origin')
        return event

    def scalar(self, function, identity):
        return next((value for value in function['locals'] if value['id'] == identity), None)

    def operand(self, function, value, span, label):
        local = self.scalar(function, value['local'])
        self.same(span, value['span'], label + '.operand-span')
        self.same(span, local['span'] if local else None, label + '.snapshot-definition-span')
        self.same('temporary', local['kind'] if local else None, label + '.snapshot-class')

    def slot(self, function, binding):
        rows = []
        for owner in function['owners']:
            rows.append({'space': 'owner', 'id': owner['id'], 'span': owner['span'], 'type': self.record_name(owner['record']),
                         'mutable': owner['kind'].get('mutable', False), 'class': owner['kind']['class'], 'position': owner['kind'].get('position')})
        for ref in function['references']:
            rows.append({'space': 'reference', 'id': ref['id'], 'span': ref['span'], 'type': ('&mut ' if ref['mode'] == 'exclusive' else '&') + self.record_name(ref['record']),
                         'mutable': False, 'class': 'parameter', 'position': ref['position']})
        for value in function['locals']:
            if value['kind'] in ('binding', 'parameter'):
                parameters = [p for p in function['parameters'] if p['mode'] == 'scalar' and p['id'] == value['id']]
                rows.append({'space': 'scalar', 'id': value['id'], 'span': value['span'], 'type': value['type'], 'mutable': False,
                             'class': 'parameter' if parameters else 'local', 'position': parameters[0]['position'] if parameters else None})
        for place in function['places']:
            rows.append({'space': 'place', 'id': place['id'], 'span': place['span'], 'type': place['type'], 'mutable': True, 'class': 'local', 'position': None})
        return self.unique(rows, lambda row: row['span'] == binding['span'], 'binding.' + binding['identity'])

    def inspect(self):
        self.same(self.expected['source_sha256'], hashlib.sha256(self.source).hexdigest(), 'source identity')
        self.same(len(self.facts['records']), len(self.raw['records']), 'record declaration count')
        for record in self.facts['records']:
            observed = self.unique(self.raw['records'], lambda row: row['span'] == record['span'], record['identity'])
            if not observed: continue
            self.record_ids[observed['id']] = record['identity']
            self.same(record['name'], self.text(observed['span']), record['identity'] + '.name')
            self.same(len(record['fields']), len(observed['fields']), record['identity'] + '.field-count')
            for position, (field, actual) in enumerate(zip(record['fields'], observed['fields'])):
                self.same(field['span'], actual['span'], field['identity'] + '.span')
                self.same(field['type'], actual['type'], field['identity'] + '.type')
                self.same({'record': observed['id'], 'index': position}, actual['id'], field['identity'] + '.nominal-identity')
        function_origins = {self.text(span): span for key, span in self.origins.items() if re.fullmatch(r'function\.\d+\.name', key)}
        self.same(len(function_origins), len(self.raw['functions']), 'function count')
        for name, span in function_origins.items():
            function = self.unique(self.raw['functions'], lambda row: row['span'] == span, 'function.' + name)
            if function:
                self.functions[name] = function; self.function_ids[function['id']] = name
        for name, function in self.functions.items():
            expected_fn = self.checked.functions[name]
            self.same(expected_fn.result, self.ty(function['result']), name + '.result-type')
            bindings = [row for row in self.facts['bindings'] if row['function'] == name]
            parameters = sorted((row for row in bindings if row['position'] is not None), key=lambda row: row['position'])
            self.same(len(parameters), len(function['parameters']), name + '.parameter-count')
            for expected, actual in zip(parameters, function['parameters']):
                for key in ('position', 'span'): self.same(expected[key], actual[key], name + '.parameter.' + key)
                self.same(expected['type'], self.ty(actual['type']), name + '.parameter-type')
                mode = 'reference' if model.ref_type(expected['type']) else 'scalar' if expected['type'] in model.SCALARS else 'owned'
                self.same(mode, actual['mode'], name + '.parameter-mode')
            for binding in bindings:
                actual = self.slot(function, binding)
                if actual:
                    self.slots[name, binding['identity']] = actual
                    for key in ('type', 'mutable', 'class', 'position'):
                        self.same(binding[key], actual[key], binding['identity'] + '.' + key)
            # Call identities are correlated solely through frozen source call spans.
            wanted_calls = [call for call in self.facts['calls'] if call['function'] == name]
            self.same(len(wanted_calls), len(function['calls']), name + '.call-count')
            for call in wanted_calls:
                actual = self.unique(function['calls'], lambda row: row['span'] == call['span'], call['identity'])
                if actual: self.calls[name, call['identity']] = actual
            # Every owned slot comes from a frozen canonical source template.
            owner_rows = self.expected['template_frames'][name]['owners']
            self.same(len(owner_rows), len(function['owners']), name + '.owner-count')
            for owner in owner_rows:
                site, kind = owner['site'], owner['kind']
                raw_class = {'staged-argument': 'staging', 'call-result': 'call_result'}.get(kind, kind)
                extra = {}
                if site.startswith('temporary:'): span = self.origin(site[len('temporary:'):])
                elif site.startswith('staged:'):
                    _, call_key, position = site.split(':'); position = int(position)
                    expected_call = next(call for call in wanted_calls if call['identity'] == call_key)
                    span = expected_call['arguments'][position]['span']
                    if (name, call_key) in self.calls: extra = {'call': self.calls[name, call_key]['id'], 'argument': position}
                elif site.startswith('result:'):
                    call_key = site[len('result:'):]; span = self.origin(call_key)
                    if (name, call_key) in self.calls: extra = {'call': self.calls[name, call_key]['id']}
                else:
                    binding = next(binding for binding in bindings if binding['identity'] == site); span = binding['span']
                    if kind == 'parameter': extra = {'position': binding['position']}
                actual = self.unique(function['owners'], lambda row: row['span'] == span and row['kind']['class'] == raw_class, name + '.' + site)
                if actual:
                    self.owners[name, site] = actual
                    self.same(owner['record'], self.record_name(actual['record']), name + '.' + site + '.record')
                    for key, value in extra.items(): self.same(value, actual['kind'].get(key), name + '.' + site + '.' + key)
            # Build actual scalar definition control points, without producer helpers.
            for block in function['blocks']:
                if block['merge'] is not None:
                    merge = block['merge']; self.local_defs[name, merge['destination']] = ('merge', block['id'], -1)
                for event in block['instructions']:
                    instruction = event['instruction']
                    destination = instruction.get('destination') if instruction['operation'] == 'ReadField' else instruction.get('scalar', {}).get('destination')
                    if destination is not None: self.local_defs[name, destination] = ('instruction', block['id'], event['position'])
                term = block['terminator']['instruction']
                if term['operation'] == 'Invoke':
                    call = next(call for call in function['calls'] if call['id'] == term['call'])
                    if call['result']['mode'] == 'scalar': self.local_defs[name, call['result']['id']] = ('call', term['continuation'], -1)
        self.inspect_operations()
        return {'checks': self.checks, 'issues': self.issues,
                'scope': 'source/raw correspondence only; no witness creation or independent execution certification'}

    def owner_id(self, name, key): return self.owners.get((name, key), {}).get('id')
    def binding_id(self, name, key): return self.slots.get((name, key), {}).get('id')

    def check_base(self, name, expected_binding, actual, label):
        slot = self.slots.get((name, expected_binding))
        self.same({'mode': slot['space'], 'id': slot['id']} if slot else None, actual, label)

    def inspect_operations(self):
        nodes = {node.key: node for _, node in self.source_nodes()}
        operators = {'+':'Add', '-':'Subtract', '*':'Multiply', '==':'Equal', '!=':'NotEqual',
                     '<':'Less', '<=':'LessEqual', '>':'Greater', '>=':'GreaterEqual'}
        for name, node in self.source_nodes():
            if self.checked.types.get(node.key) not in ('i32','bool','()') or node.tag not in ('int','bool','unit','group','unary','binary'): continue
            if node.tag == 'binary' and node.data['op'] in ('&&','||'): continue
            function = self.functions[name]; span = self.origin(node.key)
            event = self.op(function, 'Scalar', span)
            if not event: continue
            scalar = event['instruction'].get('scalar', {})
            self.same('Assign', scalar.get('kind'), node.key + '.scalar-expression-assign')
            actual = scalar.get('value', {})
            if node.tag in ('int','bool','unit'):
                want = {'kind':{'int':'I32','bool':'Bool','unit':'Unit'}[node.tag]}
                if node.tag != 'unit': want['value'] = node.data['value']
                self.same(want, actual, node.key + '.scalar-literal')
            elif node.tag in ('group','unary'):
                self.same('Copy' if node.tag == 'group' else 'NotBool', actual.get('kind'), node.key + '.scalar-operator')
                operand = actual.get('operand')
                if operand: self.operand(function, operand, self.origin(node.data['value'].key), node.key + '.operand')
                else: self.same(True, False, node.key + '.missing-operand')
                if node.tag == 'unary': self.same(self.origin(node.key+'.operator'), actual.get('operator_span'), node.key + '.operator-origin')
            else:
                self.same('CheckedI32' if node.data['op'] in ('+','-','*') else 'CompareScalar', actual.get('kind'), node.key + '.scalar-kind')
                self.same(operators[node.data['op']], actual.get('operator'), node.key + '.scalar-operator')
                self.same(self.origin(node.key+'.operator'), actual.get('operator_span'), node.key + '.operator-origin')
                for side,key in (('left','lhs'),('right','rhs')):
                    if actual.get(side): self.operand(function, actual[side], self.origin(node.data[key].key), node.key + '.' + side)
                    else: self.same(True, False, node.key + '.missing-' + side)
        for name, node in self.source_nodes():
            if node.tag != 'name' or self.checked.types.get(node.key) not in ('i32', 'bool', '()'): continue
            function = self.functions[name]
            span = self.origin(node.key)
            event = self.op(function, 'Scalar', span)
            if not event: continue
            scalar = event['instruction'].get('scalar', {})
            slot = self.slots.get((name, self.checked.references[node.key]))
            self.same('Assign', scalar.get('kind'), node.key + '.scalar-name-assign')
            value = scalar.get('value', {})
            if slot and slot['space'] == 'place':
                self.same('Load', value.get('kind'), node.key + '.scalar-name-load')
                self.same(slot['id'], value.get('place'), node.key + '.scalar-name-binding')
            else:
                self.same('Copy', value.get('kind'), node.key + '.scalar-name-copy')
                self.same(slot['id'] if slot else None, value.get('operand', {}).get('local'), node.key + '.scalar-name-binding')
            local = self.scalar(function, scalar.get('destination'))
            self.same(span, local['span'] if local else None, node.key + '.scalar-name-origin')
        for projection in self.facts['projections']:
            name = projection['function']; function = self.functions[name]
            event = self.op(function, 'ReadField', projection['span'])
            if not event: continue
            raw = event['instruction']
            self.check_base(name, projection['base_binding'], raw['base'], projection['expression'] + '.base')
            self.same(projection['field_identity'], self.field_id(raw['field']), projection['expression'] + '.field')
            local = self.scalar(function, raw['destination'])
            self.same(projection['result_type'], local['type'] if local else None, projection['expression'] + '.result-type')
            self.same(projection['span'], local['span'] if local else None, projection['expression'] + '.result-span')
            self.same(projection['span'], (event.get('origins') or {}).get('primary'), projection['expression'] + '.diagnostic-primary')
        for literal in self.facts['literals']:
            name = literal['function']; function = self.functions[name]
            event = self.op(function, 'Construct', literal['span'])
            if not event: continue
            raw = event['instruction']
            self.same(self.owner_id(name, literal['result_owner']), raw['destination'], literal['expression'] + '.owner')
            self.same([field['field_identity'] for field in literal['written_fields']], [self.field_id(field['field']) for field in raw['fields']], literal['expression'] + '.written-field-order')
            for wanted, actual in zip(literal['written_fields'], raw['fields']):
                self.operand(function, actual['value'], wanted['value_span'], literal['expression'] + '.value')
        for move in self.facts['transfers']:
            binding = next(binding for binding in self.facts['bindings'] if binding['identity'] == move['source_binding'])
            name = binding['function']; function = self.functions[name]
            event = self.op(function, 'MoveInitialize', move['charge'])
            if not event: continue
            raw = event['instruction']
            self.same(self.binding_id(name, move['source_binding']), raw['source'], move['destination'] + '.source')
            self.same(self.owner_id(name, move['destination']), raw['destination'], move['destination'] + '.destination')
            for key in ('primary', 'cause'): self.same(move[key], (event.get('origins') or {}).get(key), move['destination'] + '.' + key)
        for store in self.facts['stores']:
            name = store['function']; function = self.functions[name]
            if store['kind'] == 'write':
                event = self.op(function, 'WriteField', store['charge_span'])
                if event:
                    raw = event['instruction']; self.check_base(name, store['target_binding'], raw['base'], store['statement'] + '.base')
                    self.same(store['projection']['field_identity'], self.field_id(raw['field']), store['statement'] + '.field')
                    self.operand(function, raw['value'], store['value_span'], store['statement'] + '.rhs')
            elif store['source_owner']:
                event = self.op(function, 'MoveInitialize' if store['kind'] == 'let' else 'Replace', store['charge_span'])
                if event:
                    raw = event['instruction']; self.same(self.binding_id(name, store['target_binding']), raw['destination'], store['statement'] + '.target')
                    self.same(self.owner_id(name, store['source_owner']), raw['source'], store['statement'] + '.rhs')
            else:
                target = self.slots.get((name, store['target_binding']))
                scalar_kind = ('Initialize' if target and target['space'] == 'place' else 'Assign') if store['kind'] == 'let' else 'Store'
                event = self.op(function, 'Scalar', store['charge_span'], lambda raw: raw['scalar']['kind'] == scalar_kind)
                if event:
                    raw = event['instruction']['scalar']
                    self.same(target['id'] if target else None, raw.get('place', raw.get('destination')), store['statement'] + '.target')
                    value = raw['value']
                    if scalar_kind == 'Assign': value = value['operand'] if 'operand' in value else value.get('value', value)
                    self.operand(function, value, store['value_span'], store['statement'] + '.rhs')
            # A let target names its declaration; it is not an assignment-error primary.
            # Scalar embedded let origins retain the full source statement (§5.3).
            if event and store['kind'] != 'let': self.same(store['target_span'], (event.get('origins') or {}).get('primary'), store['statement'] + '.target-origin')
        for call in self.facts['calls']:
            name = call['function']; function = self.functions[name]; actual = self.calls.get((name, call['identity']))
            if not actual: continue
            self.same(call['callee'], self.function_ids.get(actual['callee']), call['identity'] + '.callee')
            parent = None
            if call['parent']:
                parent_call = self.calls.get((name, call['parent']['call']))
                parent = {'call': parent_call['id'], 'argument': call['parent']['argument']} if parent_call else None
            self.same(parent, actual['parent'], call['identity'] + '.parent')
            self.same(len(call['arguments']), len(actual['arguments']), call['identity'] + '.argument-count')
            for wanted, argument in zip(call['arguments'], actual['arguments']):
                position = wanted['position']; self.same(position, argument['position'], call['identity'] + '.argument-position')
                borrow = wanted['mode'] in ('shared', 'exclusive')
                self.same('borrow' if borrow else wanted['mode'], argument['mode'], call['identity'] + '.argument-mode')
                operation = 'PrepareBorrow' if borrow else 'PrepareOwned' if wanted['mode'] == 'owned' else 'PrepareScalar'
                event = self.op(function, operation, wanted['span'], lambda raw: raw['call'] == actual['id'] and raw['argument'] == position)
                if not event: continue
                raw = event['instruction']
                if borrow:
                    loan = next(loan for loan in function['loans'] if loan['id'] == raw['loan'])
                    self.same(argument['id'], raw['loan'], call['identity'] + '.loan-identity')
                    self.same(wanted['mode'], loan['mode'], call['identity'] + '.loan-mode')
                    self.same(wanted['span'], loan['span'], call['identity'] + '.loan-origin')
                    arg_node = next(key for key, span in self.origins.items() if re.fullmatch(r'n\d+', key) and span == wanted['span'])
                    self.check_base(name, self.checked.references[arg_node], loan['authority'], call['identity'] + '.borrow-authority')
                elif wanted['mode'] == 'scalar':
                    self.operand(function, raw['value'], wanted['span'], call['identity'] + '.scalar-snapshot')
                    definition = self.local_defs.get((name, raw['value']['local']))
                    valid = bool(definition and definition[1] == event['block'] and
                                 ((definition[0] == 'instruction' and definition[2] + 1 == event['position']) or
                                  (definition[0] in ('call', 'merge') and event['position'] == 0)))
                    self.same(True, valid, call['identity'] + '.immediate-scalar-preparation')
                else:
                    argument_node = next((node for node in nodes.values() if self.origin(node.key) == wanted['span']), None)
                    while argument_node is not None and argument_node.tag == 'group': argument_node = argument_node.data['value']
                    site = None if argument_node is None else ('result:' if argument_node.tag == 'call' else 'temporary:') + argument_node.key
                    self.same(self.owner_id(name, site), raw['source'], call['identity'] + '.owned-result-identity')
            result = actual['result']
            self.same('scalar' if call['result_owner'] is None else 'owned', result['mode'], call['identity'] + '.result-mode')
            if call['result_owner'] is not None: self.same(self.owner_id(name, call['result_owner']), result['id'], call['identity'] + '.result-owner')


def exact_span(value, source_bytes, nullable=False):
    if nullable and value is None: return
    if type(value) is not list or len(value) != 2 or any(type(x) is not int for x in value):
        raise ValueError('receipt span must use two exact JSON integers')
    if not 0 <= value[0] <= value[1] <= source_bytes: raise ValueError('receipt span outside original source')


def validate_raw_variants(raw, source_bytes):
    """Neutral observation schema, including every required operation operand.

    This validates JSON representation and authority-table references. It does
    not replace raw ownership/flow verification or create a sealed witness.
    """
    def fields(value, names):
        if type(value) is not dict or set(value) != set(names.split()): raise ValueError('invalid raw variant fields: '+names)
    def array(value):
        if type(value) is not list: raise ValueError('raw serialized arrays must be JSON arrays')
    def identity(value, rows):
        if type(value) is not int or not 0 <= value < len(rows): raise ValueError('raw identity outside authority table')
    def field(value):
        fields(value,'record index');identity(value['record'],raw['records']);identity(value['index'],raw['records'][value['record']]['fields'])
    array(raw['records']); array(raw['functions'])
    for record in raw['records']:
        fields(record,'id span fields')
        array(record['fields'])
        for position,decl in enumerate(record['fields']):
            fields(decl,'id type span');field(decl['id'])
            if decl['id'] != {'record':record['id'],'index':position} or decl['type'] not in model.SCALARS: raise ValueError('invalid nominal field declaration')
    for fn in raw['functions']:
        fields(fn,'id span result entry parameters locals places owners references loans calls blocks')
        for key in ('parameters','locals','places','owners','references','loans','calls','blocks'): array(fn[key])
        identity(fn['entry'],fn['blocks'])
        def ty(value):
            if type(value) is not dict: raise ValueError('invalid raw value type')
            if value.get('mode') == 'scalar':
                fields(value,'mode type')
                if value['type'] not in model.SCALARS: raise ValueError('unknown raw scalar type')
            elif value.get('mode') in ('owned','shared','exclusive'):
                fields(value,'mode record');identity(value['record'],raw['records'])
            else:raise ValueError('unknown raw value type')
        ty(fn['result'])
        if fn['result']['mode'] not in ('scalar','owned'):raise ValueError('raw function result cannot be a reference')
        def operand(value):
            fields(value,'local span');identity(value['local'],fn['locals']);exact_span(value['span'],source_bytes)
        def base(value):
            fields(value,'mode id')
            if value['mode'] not in ('owner','reference'):raise ValueError('unknown raw place-base mode')
            identity(value['id'],fn['owners' if value['mode']=='owner' else 'references'])
        parameter_slots = set()
        for position,param in enumerate(fn['parameters']):
            fields(param,'position mode id span type');ty(param['type'])
            if type(param['position']) is not int or param['position'] != position:raise ValueError('invalid original parameter position')
            space={'scalar':'locals','owned':'owners','reference':'references'}.get(param['mode'])
            if space is None:raise ValueError('unknown raw parameter mode')
            identity(param['id'],fn[space])
            slot = fn[space][param['id']]
            if (space,param['id']) in parameter_slots:raise ValueError('duplicate parameter authority slot')
            parameter_slots.add((space,param['id']))
            if param['span'] != slot['span']:raise ValueError('parameter span differs from authoritative declaration')
            if param['mode'] == 'scalar':
                if param['type'] != {'mode':'scalar','type':slot['type']} or slot['kind'] != 'parameter':
                    raise ValueError('scalar parameter mode/type/declaration mismatch')
            elif param['mode'] == 'owned':
                if param['type'] != {'mode':'owned','record':slot['record']} or slot['kind'] != {'class':'parameter','position':position}:
                    raise ValueError('owned parameter mode/type/position mismatch')
            elif param['type'] != {'mode':slot['mode'],'record':slot['record']} or slot['position'] != position:
                raise ValueError('reference parameter mode/type/position mismatch')
        for local in fn['locals']:
            fields(local,'id type span kind')
            if local['type'] not in model.SCALARS or local['kind'] not in ('temporary','binding','parameter'):raise ValueError('invalid raw local declaration')
        for place in fn['places']:
            fields(place,'id type span')
            if place['type'] not in model.SCALARS:raise ValueError('invalid raw place type')
        for owner in fn['owners']:
            fields(owner,'id record span kind');identity(owner['record'],raw['records']);kind=owner['kind'];tag=kind.get('class') if type(kind) is dict else None
            names={'temporary':'class','local':'class mutable','parameter':'class position','staging':'class call argument','call_result':'class call'}.get(tag)
            if names is None:raise ValueError('unknown raw owner class')
            fields(kind,names)
            if 'call' in kind:identity(kind['call'],fn['calls'])
        for ref in fn['references']:
            fields(ref,'id record mode position span');identity(ref['record'],raw['records'])
            if ref['mode'] not in ('shared','exclusive'):raise ValueError('unknown reference mode')
        for loan in fn['loans']:
            fields(loan,'id call argument authority mode record span');identity(loan['call'],fn['calls']);identity(loan['record'],raw['records']);base(loan['authority'])
            if loan['mode'] not in ('shared','exclusive'):raise ValueError('unknown loan mode')
        for call in fn['calls']:
            fields(call,'id callee span parent arguments result');identity(call['callee'],raw['functions'])
            array(call['arguments'])
            if call['parent'] is not None:
                fields(call['parent'],'call argument');identity(call['parent']['call'],fn['calls'])
            for position,arg in enumerate(call['arguments']):
                fields(arg,'position mode id')
                if arg['position']!=position:raise ValueError('invalid call argument position')
                if arg['mode']=='scalar':
                    if arg['id'] is not None:raise ValueError('scalar call descriptor has no owner or loan ID')
                elif arg['mode'] in ('owned','borrow'):identity(arg['id'],fn['owners' if arg['mode']=='owned' else 'loans'])
                else:raise ValueError('unknown call argument mode')
            result=call['result'];fields(result,'mode id')
            if result['mode'] not in ('scalar','owned'):raise ValueError('unknown call result mode')
            identity(result['id'],fn['locals' if result['mode']=='scalar' else 'owners'])
        def scalar_value(value):
            if type(value) is not dict:raise ValueError('invalid scalar value')
            kind=value.get('kind')
            schema={'I32':'kind value','Bool':'kind value','Unit':'kind','Copy':'kind operand','Load':'kind place span',
                    'NotBool':'kind operand operator_span','CheckedI32':'kind operator left right operator_span','CompareScalar':'kind operator left right operator_span'}
            if kind not in schema:raise ValueError('unknown scalar value kind')
            fields(value,schema[kind])
            if kind=='I32' and (type(value['value']) is not int or not model.MIN_I32 <= value['value'] <= model.MAX_I32):raise ValueError('raw constant outside i32')
            if kind=='Bool' and type(value['value']) is not bool:raise ValueError('raw boolean constant type')
            if kind in ('Copy','NotBool'):operand(value['operand'])
            if kind=='Load':identity(value['place'],fn['places'])
            if kind in ('CheckedI32','CompareScalar'):
                allowed={'Add','Subtract','Multiply'} if kind=='CheckedI32' else {'Equal','NotEqual','Less','LessEqual','Greater','GreaterEqual'}
                if value['operator'] not in allowed:raise ValueError('unknown scalar operator')
                operand(value['left']);operand(value['right'])
        def instruction(value):
            if type(value) is not dict:raise ValueError('invalid raw instruction')
            op=value.get('operation');schema={
                'Scalar':'operation scalar','ReadField':'operation destination base field','WriteField':'operation base field value',
                'StorageLive':'operation owner','StorageEnd':'operation owner','Discard':'operation owner','ReturnOwned':'operation owner',
                'Construct':'operation destination fields','MoveInitialize':'operation destination source','Replace':'operation destination source',
                'OpenCall':'operation call','PrepareScalar':'operation call argument value','PrepareOwned':'operation call argument source',
                'PrepareBorrow':'operation call argument loan','Invoke':'operation call continuation','ReturnScalar':'operation value',
                'Goto':'operation target','Branch':'operation condition then_block else_block'}
            if op not in schema:raise ValueError('unknown raw operation')
            fields(value,schema[op])
            if op=='Scalar':
                scalar=value['scalar'];kind=scalar.get('kind') if type(scalar) is dict else None
                names={'Assign':'kind destination value span','Initialize':'kind place place_span value span','Store':'kind place place_span operator_span value span'}.get(kind)
                if names is None:raise ValueError('unknown scalar instruction')
                fields(scalar,names)
                if kind=='Assign':identity(scalar['destination'],fn['locals']);scalar_value(scalar['value'])
                else:identity(scalar['place'],fn['places']);operand(scalar['value'])
            if 'owner' in value:identity(value['owner'],fn['owners'])
            if 'source' in value:identity(value['source'],fn['owners'])
            if 'destination' in value:identity(value['destination'],fn['locals' if op=='ReadField' else 'owners'])
            if 'base' in value:base(value['base'])
            if 'field' in value:field(value['field'])
            if op=='Construct':
                if type(value['fields']) is not list:raise ValueError('invalid constructor fields')
                for initializer in value['fields']:fields(initializer,'field value');field(initializer['field']);operand(initializer['value'])
            if op in ('WriteField','PrepareScalar','ReturnScalar'):operand(value['value'])
            if op=='Branch':operand(value['condition'])
            for key in ('target','then_block','else_block','continuation'):
                if key in value:identity(value[key],fn['blocks'])
            if 'call' in value:
                identity(value['call'],fn['calls'])
                if 'argument' in value:identity(value['argument'],fn['calls'][value['call']]['arguments'])
            if 'loan' in value:identity(value['loan'],fn['loans'])
        for block in fn['blocks']:
            fields(block,'id span merge instructions terminator')
            array(block['instructions'])
            merge=block['merge']
            if merge is not None:
                fields(merge,'operation destination span operator_span incoming');identity(merge['destination'],fn['locals'])
                array(merge['incoming'])
                for incoming in merge['incoming']:fields(incoming,'predecessor value');identity(incoming['predecessor'],fn['blocks']);operand(incoming['value'])
            for position,event in enumerate(block['instructions']+[block['terminator']]):
                required={'position','span','origins','instruction'}
                if position < len(block['instructions']): required.add('charge_span')
                if type(event) is not dict or set(event) != required:raise ValueError('invalid raw event fields')
                if type(event['position']) is not int or event['position']!=position:raise ValueError('invalid raw event position')
                if event['origins'] is not None:fields(event['origins'],'primary cause')
                instruction(event['instruction'])



def validate_candidate_receipt(observed, item, source):
    """Reject incomplete or type-confused observations before comparing semantics."""
    if type(observed) is not dict or observed.get('id') != item['id']: raise ValueError('candidate receipt identity mismatch')
    check = observed.get('check')
    if type(check) is not dict or set(check) != {'accepted', 'diagnostics'} or type(check['accepted']) is not bool or type(check['diagnostics']) is not list:
        raise ValueError('invalid candidate check protocol')
    if check['accepted'] != (len(check['diagnostics']) == 0): raise ValueError('candidate check outcome and diagnostics disagree')
    for diagnostic in check['diagnostics']:
        if type(diagnostic) is not dict or set(diagnostic) != {'stage','code','span','related'}:
            raise ValueError('invalid candidate diagnostic fields')
        if any(type(diagnostic[k]) is not str for k in ('stage','code')) or type(diagnostic['related']) is not list:
            raise ValueError('invalid candidate diagnostic scalar types')
        exact_span(diagnostic['span'], len(source), True)
        for span in diagnostic['related']: exact_span(span, len(source))
    if not check['accepted']:
        if any(k in observed for k in ('reference','frames','raw_view','charge_events')):
            raise ValueError('rejected source must not expose sealed execution observations')
        return
    reference = observed.get('reference')
    if type(reference) is not dict or set(reference) != {'result_type','result','failure'}:
        raise ValueError('candidate reference result missing or malformed')
    failure = reference['failure']
    if failure is not None:
        if type(failure) is not dict or set(failure) != {'code','span'} or type(failure['code']) is not str:
            raise ValueError('invalid candidate runtime failure')
        exact_span(failure['span'], len(source), True)
        if reference['result_type'] is not None or reference['result'] is not None: raise ValueError('failed candidate result must be null')
    else:
        ty = reference['result_type']; value = reference['result']
        if ty == '()': valid = value is None
        elif ty == 'bool': valid = type(value) is bool
        elif ty == 'i32': valid = type(value) is int and model.MIN_I32 <= value <= model.MAX_I32
        else: valid = False
        if not valid: raise ValueError('candidate scalar result type mismatch')
    frames = observed.get('frames')
    if type(frames) is not dict: raise ValueError('missing frame census')
    for frame in frames.values():
        if type(frame) is not dict or set(frame) != set('SAPORLCX') or any(type(v) is not int or v < 0 for v in frame.values()):
            raise ValueError('invalid frame census schema')
    if failure is None or failure['code'] != 'E0600':
        if type(observed.get('charge_events')) is not list: raise ValueError('missing actual charge events')
    for charge in observed.get('charge_events', []):
        if type(charge) is not dict or set(charge) != {'cost','span'} or type(charge['cost']) is not int or charge['cost'] <= 0:
            raise ValueError('invalid charge event')
        exact_span(charge['span'], len(source))
    raw = observed.get('raw_view')
    if type(raw) is not dict or set(raw) != {'records','functions'} or any(type(raw[k]) is not list for k in raw):
        raise ValueError('missing raw authority view')
    integer_keys = {'argument','call','callee','continuation','destination','else_block','entry','index','loan','local','owner','place','position','record','source','target','then_block','predecessor'}
    statements = {'Scalar','StorageLive','StorageEnd','Construct','MoveInitialize','Discard','Replace','ReadField','WriteField','OpenCall','PrepareScalar','PrepareOwned','PrepareBorrow'}
    terminators = {'Goto','Branch','Invoke','ReturnScalar','ReturnOwned'}
    def scan(value):
        if type(value) is dict:
            for key, child in value.items():
                if key == 'span' or key.endswith('_span') or key in ('primary','cause'):
                    exact_span(child, len(source))
                elif key in integer_keys and (type(child) is not int or child < 0):
                    raise ValueError('raw authority identity must use exact nonnegative integers: ' + key)
                elif key == 'id' and child is not None:
                    if type(child) is not dict and (type(child) is not int or child < 0):
                        raise ValueError('raw ID must use a nonnegative integer or nominal field identity')
                elif key == 'operation' and child not in statements|terminators|{'BoolMerge'}:
                    raise ValueError('unknown raw operation')
                elif key == 'mutable' and type(child) is not bool: raise ValueError('raw mutability must be boolean')
                elif key == 'value' and value.get('kind') in ('I32','Bool'):
                    if type(child) is not (int if value['kind'] == 'I32' else bool): raise ValueError('raw scalar constant type mismatch')
                scan(child)
        elif type(value) is list:
            for child in value: scan(child)
    scan(raw)
    def identities(rows, label):
        if type(rows) is not list or any(type(row) is not dict for row in rows): raise ValueError('invalid raw declaration array: '+label)
        ids = [row.get('id') for row in rows]
        if any(type(identity) is not int for identity in ids) or ids != list(range(len(rows))):
            raise ValueError('raw declaration IDs must match their authority-table positions: '+label)
    identities(raw['records'],'records'); identities(raw['functions'],'functions')
    for function in raw['functions']:
        for key in ('locals','places','owners','references','loans','calls','blocks'): identities(function[key],key)
        for block in function['blocks']:
            if block['terminator']['instruction']['operation'] not in terminators: raise ValueError('non-terminator in raw terminator slot')
            if any(event['instruction']['operation'] not in statements for event in block['instructions']): raise ValueError('non-statement in raw instruction slot')
            if block['merge'] is not None and block['merge'].get('operation') != 'BoolMerge': raise ValueError('invalid raw merge operation')
    validate_raw_variants(raw, len(source))


def candidate_diagnostics(observed):
    return [{'stage': d['stage'], 'code': d['code'],
             'primary': None if d['span'] is None else dict(zip(('start','end'), d['span'])),
             'secondary': [{'span': dict(zip(('start','end'), span))} for span in d['related']]}
            for d in observed['check']['diagnostics']]


def compare_candidate(item, source, observed, checked=None):
    validate_candidate_receipt(observed, item, source.encode())
    expected = item['expected']
    selected = None
    if expected['status'] == 'reject':
        if not observed['check']['accepted']:
            selected = match_diagnostic(candidate_diagnostics(observed), expected)
        return {'checks': 1, 'issues': [] if not observed['check']['accepted'] else
                [{'fact':'check.accepted', 'expected':False, 'observed':True}], 'selected_diagnostic': selected}
    inspector = RawInspector(item, source, observed.get('raw_view'), checked)
    inspector.same(True, observed['check']['accepted'], 'check.accepted')
    if not observed['check']['accepted']:
        return {'checks': inspector.checks, 'issues': inspector.issues, 'selected_diagnostic': None}
    failure = expected.get('runtime_failure')
    wanted = {'result_type': None if failure else expected['result_type'], 'result': None if failure else expected['result'],
              'failure': None if not failure else {'code': failure['code'], 'span': item['origins'].get(failure['origin'])}}
    inspector.same(wanted, observed['reference'], 'reference')
    frames = {name: {k: v[k] for k in 'SAPORLCX'} for name,v in item['template_frames'].items()}
    inspector.same(frames, observed['frames'], 'frames')
    charges = [{'cost': c['cost'], 'span': item['origins'][c['origin']]} for c in item.get('schedule', {}).get('items', [])]
    inspector.same(charges, observed.get('charge_events', []), 'charge-events')
    report = inspector.inspect()
    return {**report, 'selected_diagnostic': None, 'charge_events': len(charges)}


def verify_collection_manifest(frozen, observed, collection_path):
    """Bind original source, compiler and actual receipt bytes before comparison."""
    frozen, observed, collection_path = Path(frozen), Path(observed), Path(collection_path)
    manifest = verify_frozen(frozen)
    collection = strict_json_loads(collection_path.read_text())
    if type(collection.get('exit_code')) is not int or collection['exit_code'] != 0: raise ValueError('candidate collector did not finish successfully')
    if collection.get('source_manifest_sha256') != digest(frozen/'manifest.json'): raise ValueError('collection uses a different source freeze')
    source_files = {row['id']+'.ox': row['source_sha256'] for row in manifest['files']}
    receipt_files = {row['id']+'.json': digest(observed/(row['id']+'.json')) for row in manifest['files']}
    # The first and repeated collectors used these two explicit key spellings.
    # Accept either complete representation, never conflicting aliases.
    def inventory(first, second):
        keys = [key for key in (first,second) if key in collection]
        if len(keys) != 1: raise ValueError('missing or ambiguous collection inventory')
        return collection[keys[0]]
    if inventory('source_files','sources') != source_files: raise ValueError('candidate original source identity mismatch')
    if inventory('receipt_files','receipts') != receipt_files: raise ValueError('candidate receipt digest/inventory mismatch')
    if {p.name for p in observed.glob('*.json')} != set(receipt_files): raise ValueError('unexpected or missing receipt files')
    command = collection.get('command')
    if type(command) is not list or not command or any(type(x) is not str for x in command): raise ValueError('missing collector command identity')
    if digest(command[0]) != collection.get('binary_sha256'): raise ValueError('collector binary identity changed')
    return manifest, collection


def compare_candidate_collection(frozen, observed, collection_path, evidence):
    manifest, collection = verify_collection_manifest(frozen, observed, collection_path)
    cases = {case.identifier: case for case in model.corpus()}
    reports, selections = [], {}
    for row in manifest['files']:
        identifier = row['id']; item = strict_json_loads((Path(frozen)/(identifier+'.json')).read_text())
        receipt = strict_json_loads((Path(observed)/(identifier+'.json')).read_text())
        source = (Path(frozen)/(identifier+'.ox')).read_bytes().decode()
        checked = model.NamesTypes(cases[identifier].program).check() if item['expected']['status'] == 'accept' else None
        try: report = compare_candidate(item, source, receipt, checked)
        except (ValueError, KeyError, TypeError, StopIteration) as error:
            report = {'checks':0, 'issues':[{'protocol_or_diagnostic_error':str(error)}], 'selected_diagnostic':None}
        if item['expected'].get('diagnostic_alternatives'): selections[identifier] = report['selected_diagnostic']
        reports.append({'id':identifier, **report})
    result = {'scope':'actual private candidate source/check/reference and raw correspondence; not production/native qualification',
              'model_sha256':digest(Path(model.__file__)), 'harness_sha256':digest(Path(__file__)),
              'frozen_manifest_sha256':digest(Path(frozen)/'manifest.json'), 'collection_manifest_sha256':digest(collection_path),
              'compiler_sha256':collection['binary_sha256'], 'documents':len(reports),
              'fact_checks':sum(r['checks'] for r in reports), 'charge_events':sum(r.get('charge_events',0) for r in reports),
              'diagnostic_selections':selections, 'mismatches':[r for r in reports if r['issues']], 'full_qualification':False}
    write_json(Path(evidence)/'candidate-comparison.json', result)
    return result


def tokens(text):
    result, offset = [], 0
    pattern = re.compile(r'[A-Za-z_][A-Za-z0-9_]*|[0-9]+|->|&&|\|\||==|!=|<=|>=|[^\s]')
    while offset < len(text):
        if text[offset].isspace(): offset += 1; continue
        if text.startswith('//', offset):
            end = text.find('\n', offset + 2); offset = len(text) if end < 0 else end + 1; continue
        if text.startswith('/*', offset):
            end = text.find('*/', offset + 2)
            if end < 0: raise ValueError('unterminated original comment')
            offset = end + 2; continue
        token = pattern.match(text, offset)
        result.append((token.group(), len(text[:token.start()].encode()), len(text[:token.end()].encode())))
        offset = token.end()
    return result


def original_expectation(identifier, program, source):
    canonical = model.case_expectation(model.Case(identifier, 'producer-mutation-original', program))
    left, right = tokens(canonical['source']), tokens(source)
    if [x[0] for x in left] != [x[0] for x in right]: raise ValueError('tagged specification and supplied source tokens differ: ' + identifier)
    starts = {a[1]: b[1] for a, b in zip(left, right)}
    ends = {a[2]: b[2] for a, b in zip(left, right)}
    def remap(value):
        if type(value) is list:
            if len(value) == 2 and all(type(x) is int for x in value) and tuple(value) in span_pairs:
                return [starts[value[0]], ends[value[1]]]
            return [remap(item) for item in value]
        if type(value) is dict: return {key: remap(item) for key, item in value.items()}
        return value
    span_pairs = {tuple(span) for span in canonical['origins'].values()}
    value = remap(canonical)
    value['source'] = source
    value['source_sha256'] = hashlib.sha256(source.encode()).hexdigest()
    if 'facts' in value: value['facts']['source_sha256'] = value['source_sha256']
    value['original_layout_alignment'] = {'method': 'independent canonical and original token streams identical', 'tokens': len(left)}
    return value


def decode_source_program(value):
    """Deserialize only this oracle's tagged source domain, never compiler AST/OIR."""
    def decode(node):
        if type(node) is dict:
            if set(node) == {'key','tag','data'}: return model.Node(node['key'], node['tag'], decode(node['data']))
            return {key:decode(child) for key,child in node.items()}
        if type(node) is list: return tuple(decode(child) for child in node)
        return node
    records = tuple(model.Record(r['name'], tuple(tuple(f) for f in r['fields'])) for r in value['records'])
    functions = tuple(model.Function(f['name'], tuple(tuple(p) for p in f['parameters']), f['result'], decode(f['body']), f['inline']) for f in value['functions'])
    return model.Program(records, functions, value['preamble'], value['newline'], tuple(tuple(p) for p in value['order']))


def compare_translation_controls(path):
    data = strict_json_loads(Path(path).read_text()); reports=[]
    for row in data['cases']:
        encoded = row['receipt_json'].encode()
        if hashlib.sha256(encoded).hexdigest() != row['receipt_sha256'] or row['release_receipt_sha256'] != row['receipt_sha256']:
            raise ValueError('translation receipt bytes/profile identities changed')
        item, source = row['expected'], row['source']
        if hashlib.sha256(source.encode()).hexdigest() != item['source_sha256']: raise ValueError('translation original source changed')
        program = decode_source_program(row['program'])
        predicted = original_expectation(row['id'], program, source); predicted.pop('source')
        predicted = strict_json_loads(json.dumps(predicted))  # JSON arrays encode source-model tuples.
        if not RawInspector.equal(item, predicted): raise ValueError('frozen translation expectation changed from independent source specification')
        checked = model.NamesTypes(program).check()
        receipt = strict_json_loads(encoded)
        original = compare_candidate(item, source, receipt['original'], checked)
        if original['issues']: raise ValueError('translation baseline mismatch: ' + row['id'])
        # Outside the alias-family exact-label contract, this frozen cause is a
        # required exact label. Preserve additional observed declaration evidence,
        # without claiming it was independently predicted before this collection.
        related = item['expected'].get('related', [])
        if related:
            actual = receipt['original']['check']['diagnostics'][0]['related']
            for cause in related:
                if sum(RawInspector.equal(cause, label) for label in actual) != 1:
                    raise ValueError('translation baseline lost its frozen cause label')
        if receipt.get('raw_valid') is not True or receipt.get('witness') != 'verified::verify_owned returned a sealed witness':
            raise ValueError('translation mutant lacks actual sealed-verifier success')
        mutant = receipt['mutant']; validate_candidate_receipt(mutant, item, source.encode())
        if mutant['check']['accepted'] is not True: raise ValueError('translation mutant is malformed/rejected, not raw-valid')
        difference = RawInspector(item, source, mutant['raw_view'], checked).inspect()
        if not difference['issues']: raise ValueError('raw-valid source mistranslation escaped correspondence: ' + row['id'])
        reports.append({'id':row['id'], 'original_outcome':receipt['original']['check'],
                        'original_reference':receipt['original'].get('reference'),
                        'selected_original_diagnostic':original['selected_diagnostic'],
                        'required_frozen_related':related, 'raw_valid':True, 'witness':True,
                        'mutant_reference':mutant['reference'], 'failed_facts':difference['issues'],
                        'receipt_sha256':row['receipt_sha256'], 'debug_release_identical':True})
    return {'scope':'Actual test-only producer mutations rechecked against frozen source facts; no production execution or malformed-raw claim',
            'cases':reports, 'mutations':len(reports), 'unique_original_sources':len({r['expected']['source_sha256'] for r in data['cases']}),
            'fixture_sha256':digest(path), 'model_sha256':digest(Path(model.__file__)), 'harness_sha256':digest(Path(__file__))}


def expected_budget_row(item, budget):
    result = model.TemplateScheduler.at_budget(item['schedule'], item['dynamic'], budget)
    failure = result['failure']
    stores=[]
    for event in result['committed_writes']:
        store={'kind':event['kind'], 'span':item['origins'][event['origin']]}
        for key in ('field','value','fields'):
            if key in event: store[key]=event[key]
        stores.append(store)
    return {'budget':budget, 'reference':{'result_type':None if failure else item['expected']['result_type'],
            'result':result['result'], 'failure':None if not failure else {'code':failure['code'],'span':item['origins'][failure['origin']]}},
            'committed_stores':stores}


def native_probe_expectations(path):
    data=strict_json_loads(Path(path).read_text())
    if data['model_sha256'] != digest(Path(model.__file__)): raise ValueError('native source probe model identity changed')
    result=[]
    for row in data['cases']:
        item=model.case_expectation(model.Case(row['id'],'native-source-probe',decode_source_program(row['program'])))
        source=item.pop('source')
        if source != row['source'] or item['source_sha256'] != row['source_sha256']: raise ValueError('native probe original source changed')
        item['native_budget_expectations']=[expected_budget_row(item,budget) for budget in range(item['schedule']['total_fuel']+1)]
        encoded=(json.dumps(item,indent=2,sort_keys=True)+'\n').encode()
        if hashlib.sha256(encoded).hexdigest() != row['expectation_sha256']: raise ValueError('native probe expectation differs from pre-observation freeze')
        result.append((source,strict_json_loads(encoded)))
    return result


def compare_budget_receipts(item, rows, *, require_stores=False):
    """Exact complete budget inventory; absent store observations remain PENDING.

    A semantic store view must be independently mapped from actual instrumented
    native sites/reference events. This helper never fabricates missing events.
    """
    if type(rows) is not list: raise ValueError('budget receipts must be an ordered array')
    total=item['schedule']['total_fuel']
    if len(rows) != total+1: raise ValueError('incomplete budget inventory')
    issues=[]; missing_stores=0
    for budget,row in enumerate(rows):
        if type(row) is not dict or type(row.get('budget')) is not int or row['budget'] != budget:
            raise ValueError('budget inventory must contain each exact integer in order once')
        expected=expected_budget_row(item,budget)
        # Initial collector used a flat reference row. Normalize representation,
        # retaining all original scalar values for strict comparison.
        reference=row.get('reference', {key:row.get(key) for key in ('result_type','result','failure')})
        if not RawInspector.equal(expected['reference'],reference):
            issues.append({'budget':budget,'fact':'reference','expected':expected['reference'],'observed':reference})
        if 'committed_stores' not in row:
            missing_stores+=1
        elif not RawInspector.equal(expected['committed_stores'],row['committed_stores']):
            issues.append({'budget':budget,'fact':'committed-store-prefix','expected':expected['committed_stores'],'observed':row['committed_stores']})
    if require_stores and missing_stores: raise ValueError('missing actual committed store observations')
    store_mismatch = any(issue['fact'] == 'committed-store-prefix' for issue in issues)
    return {'budgets':len(rows),'issues':issues,'store_prefix_gate':'MISMATCH' if store_mismatch else 'PENDING' if missing_stores else 'MATCH',
            'missing_store_rows':missing_stores,'full_qualification':False}


QUALIFICATION_PENDING = (
    'exact debug/release ordinary CLI and source-free ELF',
    'source-bound candidate receipts and deterministic diagnostic selections',
    'reviewed raw-correspondence inspector and actual sealed-valid translation controls',
    'all independently frozen native budget outcomes and exact error origins',
    'ordinary-body byte identity for test-only fuel wrappers',
    'every native store guard and overflow failure-edge audit plus actual store probes',
    'inclusive and one-over source/IR/runtime/native resource boundaries',
    'exact artifact review and existing source/native compatibility suites',
)



def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("debug_binary", nargs="?")
    parser.add_argument("release_binary", nargs="?")
    parser.add_argument("--mode", choices=("model", "freeze", "production", "cli", "candidate"), default=None)
    parser.add_argument("--evidence", type=Path, default=Path("target/owned-source-evidence"))
    parser.add_argument("--frozen", type=Path)
    parser.add_argument("--candidate-observations", type=Path)
    parser.add_argument("--collection-manifest", type=Path)
    parser.add_argument("--jobs", type=int, choices=(1, 2), default=1)
    parser.add_argument("--timeout", type=float, default=1800)
    parser.add_argument("--worker-profile", choices=("debug", "release"), help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    if args.mode is None: args.mode = "production" if args.debug_binary or args.release_binary else "model"
    if sys.flags.optimize: parser.error("Python assertions must remain enabled")
    try:
        if args.worker_profile:
            profile_worker(args.debug_binary, args.worker_profile, args.frozen, args.evidence)
            return 0
        args.evidence.mkdir(parents=True, exist_ok=True)
        frozen = args.frozen or args.evidence / "frozen"
        manifest = verify_frozen(frozen) if args.frozen else freeze(frozen)
        if args.mode in ("model", "freeze"):
            print(f"owned-source model: {manifest['unique_source_programs']} independent cases; actual CLI/native runs=0; production source qualification PENDING: PASS")
            return 0
        if args.mode == "candidate":
            if not args.candidate_observations: raise ValueError("candidate mode requires immutable independently collected observations")
            if not args.collection_manifest: raise ValueError("candidate mode requires the immutable collection manifest")
            result = compare_candidate_collection(frozen, args.candidate_observations, args.collection_manifest, args.evidence)
            if result['mismatches']: raise ValueError(f"{len(result['mismatches'])} candidate receipt mismatches")
            print(f"owned-source candidate receipts: {result['documents']} actual original-source observations: PASS (production/native remains unqualified)")
            return 0
        if not args.debug_binary or not args.release_binary: raise ValueError("production mode requires both exact compiler binaries")
        binaries = [str(Path(value).resolve()) for value in (args.debug_binary, args.release_binary)]
        for binary in binaries:
            if not os.path.isfile(binary) or not os.access(binary, os.X_OK): raise ValueError("compiler missing or not executable: " + binary)
        before = [digest(binary) for binary in binaries]
        jobs = [Job(profile, (sys.executable, str(Path(__file__).resolve()), binary, "--worker-profile", profile,
                      "--frozen", str(frozen.resolve()), "--evidence", str((args.evidence / profile).resolve())),
                    f"owned-source {profile}: CLI/ELF PASS") for profile, binary in zip(("debug", "release"), binaries)]
        code = run_jobs(jobs, args.evidence / "logs", jobs=args.jobs, timeout=args.timeout)
        if [digest(binary) for binary in binaries] != before: raise ValueError("compiler identities changed")
        if code: return code
        results = [strict_json_loads((args.evidence / profile / "profile-result.json").read_text()) for profile in ("debug", "release")]
        if results[0]["diagnostic_selections"] != results[1]["diagnostic_selections"]:
            raise ValueError("debug/release ownership diagnostic selection mismatch")
        if [(item["id"], item["sha256"]) for item in results[0]["elf"]] != [(item["id"], item["sha256"]) for item in results[1]["elf"]]:
            raise ValueError("debug/release ELF mismatch")
        write_json(args.evidence / "production-cli-result.json", {"status": "CLI_ELF_PASS" if args.mode == "cli" else "PARTIAL", "profiles": results,
                   "full_qualification": False, "model_sha256":digest(Path(model.__file__)), "harness_sha256":digest(Path(__file__)),
                   "frozen_manifest_sha256":digest(frozen/'manifest.json') if (frozen/'manifest.json').exists() else None,
                   "pending": ["real LLVM text and failure-before-store proof", "exact source fuel sweeps", "resource boundaries and held-out review"]})
        if args.mode == "cli":
            print("owned-source CLI/ELF comparison: PASS (full source qualification requires the separate native, resource and review gates)")
            return 0
        print("owned-source production CLI/ELF checks passed; full source qualification remains PARTIAL (missing later gates)")
        return 2
    except (OSError, ValueError, AssertionError, subprocess.TimeoutExpired) as error:
        print("owned-source: FAIL: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
