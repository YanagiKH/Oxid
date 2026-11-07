#!/usr/bin/env python3
"""Exercise finite owned-record vectors through public debug/release Oxid CLIs.

This gate qualifies its retained public workflow only. The existing registry
compatibility controller owns the inherited 527-input source-policy replay.
Native execution is limited to an actual Linux64 host and LLVM 19.1.7.
"""
import argparse
from dataclasses import dataclass
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time

import linux_native_target


ROOT = Path(__file__).resolve().parents[1]
DATA = Path("scripts/owned_record_vector_acceptance")
EXAMPLE = Path("examples/owned-record-vectors")
POLICY_COMMIT = "af0fe5e1fbaf4fb49b62e894a53ad4b5ed307ee7"
POLICY = "scripts/verify_registry_compatibility.py"
POLICY_REFERENCES = (POLICY, "scripts/verify_scalar_library_compatibility.py",
                     "scripts/replay_scalar_library_source_policy.py")
MAX_STREAM = 4 * 1024 * 1024
TIMEOUT = 180
ENV_NAMES = ("PATH", "LANG", "LC_ALL", "TZ", "SOURCE_DATE_EPOCH", "LD_LIBRARY_PATH",
             "TMPDIR", "OXID_LLVM_BIN", "OXID_CACHE_DIR")


@dataclass(frozen=True)
class Case:
    name: str
    source: str
    result: int | None
    native: bool = False
    hosted_only: bool = False
    heap_limit: int | None = None
    runtime_code: str | None = None
    diagnostics: tuple[tuple[str, str], ...] = ()


CASES = (
    Case("bytebuf", "bytebuf.ox", 58, native=True),
    Case("enum16", "enum16.ox", 100, native=True),
    Case("files", "files.ox", 58, native=True),
    Case("file-error", "files.ox", -19, native=True),
    Case("failed-reserve", "failed_reserve.ox", 79, native=True, hosted_only=True, heap_limit=512),
    Case("fault-pop", "fault_pop.ox", None, native=True, runtime_code="E0606"),
    Case("fault-push-file", "fault_push_file.ox", None, native=True, runtime_code="E0606"),
    Case("zero-key", "zero_key.ox", 0, native=True),
    Case("unused-template", "unused_template.ox", 0, native=True),
    Case("unused-push", "unused_push.ox", 0, native=True),
    Case("unused-pop", "unused_pop.ox", 0, native=True),
    Case("empty-new", "empty_new.ox", 0, native=True),
    Case("aliases", "aliases.ox", 42, native=True),
    Case("generic-record", "generic_record.ox", 42, native=True),
    Case("package", "package_main.ox", 42, native=True),
    Case("double-move", "double_move.ox", None, diagnostics=(("ownership", "E0310"),)),
    Case("conflicting-borrows", "conflicting_borrows.ox", None,
         diagnostics=(("ownership", "E0311"),)),
    # Candidate collection failure preserves the original ordinary route error.
    Case("copy-operation", "copy_operation.ox", None, diagnostics=(("resolve", "E0205"),)),
    Case("index-read", "index_read.ox", None, diagnostics=(("generics", "E0300"),)),
    Case("nested-vector", "nested_vector.ox", None, diagnostics=(("generics", "E0300"),)),
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def save(path, value):
    Path(path).write_bytes(encoded(value))


def identity(path):
    return {"path": str(path), "bytes": path.stat().st_size, "sha256": digest(path)}


def regular(root, relative):
    path = Path(relative)
    require(not path.is_absolute() and path.parts and ".." not in path.parts,
            "input path must remain relative")
    selected = root / path
    require(all(not item.is_symlink() for item in (selected, *selected.parents)),
            "symlink source input: " + str(relative))
    require(selected.is_file(), "missing source input: " + str(relative))
    return selected


def input_paths():
    sources = {str(DATA / case.source) for case in CASES}
    sources.add(str(DATA / "package_model.ox"))
    sources.update(str(EXAMPLE / name) for name in ("oxid.toml", "src/main.ox", "tests/messages.ox"))
    sources.update(POLICY_REFERENCES)
    sources.update(("scripts/verify_owned_record_vectors.py", "scripts/test_owned_record_vector_verifier.py",
                    "scripts/linux_native_target.py", "scripts/test_registry_compatibility.py"))
    return sorted(sources)


def json_records(result, summary, success):
    require(not result.stderr, "JSON command wrote unexpected stderr")
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    require(rows and all(type(row) is dict and row.get("schema_version") == 1 for row in rows),
            "invalid public CLI record schema")
    require(rows[-1].get("kind") == summary and rows[-1].get("success") is success,
            "wrong public CLI summary")
    return rows


def refused(result, case, summary="check-summary"):
    require(result.returncode == 1, case.name + ": negative source unexpectedly succeeded")
    rows = json_records(result, summary, False)
    errors = rows[:-1]
    require(errors and all(row.get("kind") == "diagnostic" and row.get("severity") == "error"
                           and (row.get("stage"), row.get("code")) in case.diagnostics
                           for row in errors), case.name + ": wrong public refusal code/stage")
    return [(row["stage"], row["code"]) for row in errors]


def outcome(result, case):
    if case.runtime_code:
        require(result.returncode == 1 and result.stdout == b"", case.name + ": wrong failure output/status")
        codes = re.findall(rb"error\[(E[0-9]{4})\]", result.stderr)
        require(codes == [case.runtime_code.encode()] and b"internal compiler error" not in result.stderr,
                case.name + ": wrong runtime diagnostic")
    else:
        expected = (str(case.result) + "\n").encode()
        require(result.returncode == 0 and result.stdout == expected and not result.stderr,
                case.name + ": independent result oracle differs")


def manifest(name, policy, heap_limit=None, package=False):
    require(policy in ("bounded", "hosted") and (heap_limit is None or policy == "hosted"),
            "heap setting requires hosted execution")
    text = (f'[project]\nname = "{name}"\nedition = "typed-preview"\nentry = "src/main.ox"\n'
            '[build]\nentry-mode = "result"\nreproducible = true\n'
            f'[execution]\npolicy = "{policy}"\n')
    if heap_limit is not None:
        text += f"heap-limit = {heap_limit}\n"
    if package:
        text += '\n[dependencies]\nmodel = "packages/model"\n'
    return text


def source_tree(path):
    return {file.relative_to(path).as_posix(): digest(file) for file in sorted(path.rglob("*"))
            if file.is_file() and (file.suffix == ".ox" or file.name in ("oxid.toml", "oxid.lock"))
            and ".oxid" not in file.relative_to(path).parts}


def cache_tree(path):
    return {file.relative_to(path).as_posix(): digest(file) for file in sorted(path.rglob("*"))
            if file.is_file()}


class Driver:
    def __init__(self, output, environment, report, *, repo=None):
        self.output, self.environment, self.report = output, environment, report
        self.repo = Path(ROOT if repo is None else repo).resolve(strict=True)
        self.commands = report["commands"] = []
        for name in ("commands", "projects", "artifacts", "deployments", "retained-sources", "scratch"):
            (output / name).mkdir()

    def command(self, name, argv, cwd, status=0, overrides=None):
        folder = self.output / "commands" / f"{len(self.commands):04d}-{name}"
        folder.mkdir()
        (folder / "stdin.bin").write_bytes(b"")
        environment = {**self.environment, **(overrides or {})}
        require(set(environment) <= set(ENV_NAMES), "environment recording requires an allowlisted name")
        arguments = [str(item) for item in argv]
        if arguments[:1] == ["git"]:
            # The isolated child intentionally has no HOME/global Git config.
            # Trust only this selected checkout, on this Git command.
            arguments[1:1] = ["-c", "safe.directory=" + str(self.repo)]
        row = {"name": name, "argv": arguments, "cwd": str(cwd),
               "environment": environment, "expected_status": status, "status": "running"}
        self.commands.append(row)
        child, started = None, time.monotonic()
        try:
            with (folder / "stdout.bin").open("xb") as stdout, (folder / "stderr.bin").open("xb") as stderr:
                child = subprocess.Popen(row["argv"], cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                         stdout=stdout, stderr=stderr, start_new_session=True)
                while child.poll() is None:
                    require(time.monotonic() - started < TIMEOUT, name + ": command deadline exceeded")
                    require(stdout.tell() <= MAX_STREAM and stderr.tell() <= MAX_STREAM,
                            name + ": command transcript allowance exceeded")
                    time.sleep(0.05)
                result = subprocess.CompletedProcess(row["argv"], child.wait())
            result.stdout = (folder / "stdout.bin").read_bytes()
            result.stderr = (folder / "stderr.bin").read_bytes()
            require(len(result.stdout) <= MAX_STREAM and len(result.stderr) <= MAX_STREAM,
                    name + ": final command transcript allowance exceeded")
            row["exit_status"] = result.returncode
            require(result.returncode == status, name + ": unexpected status; inspect " + str(folder))
            row["status"] = "passed"
            return result
        except BaseException as error:
            row.update(status="failed", error=repr(error))
            raise
        finally:
            if child is not None and child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait(timeout=2)
            row["seconds"] = time.monotonic() - started
            row["streams"] = {name: identity(folder / (name + ".bin")) for name in ("stdin", "stdout", "stderr")}
            save(folder / "command.json", row)

    def materialize(self, base, case, policy):
        app = base / policy / case.name
        (app / "src").mkdir(parents=True)
        body = regular(ROOT, DATA / case.source).read_bytes()
        if case.name == "file-error":
            require(body.count(b'b"popped.bin"') == 1, "file-error edit anchor changed")
            body = body.replace(b'b"popped.bin"', b'b"missing-parent/popped.bin"', 1)
        (app / "src/main.ox").write_bytes(body)
        (app / "oxid.toml").write_text(manifest(case.name, policy, case.heap_limit, case.name == "package"))
        if case.name == "package":
            library = app / "packages/model"
            (library / "src").mkdir(parents=True)
            (library / "src/lib.ox").write_bytes(regular(ROOT, DATA / "package_model.ox").read_bytes())
            (library / "oxid.toml").write_text('[project]\nname = "model"\nedition = "typed-preview"\n'
                                               'kind = "lib"\nentry = "src/lib.ox"\n')
        return app

    def check_case(self, compiler, label, app, case):
        result = self.command(label + "-check", [compiler, "check", "--message-format=json"], app,
                              status=1 if case.diagnostics else 0)
        if case.diagnostics:
            return {"diagnostics": refused(result, case)}
        rows = json_records(result, "check-summary", True)
        require(len(rows) == 1, case.name + ": positive source produced a diagnostic")
        self.command(label + "-lock", [compiler, "lock", "--offline"], app)
        original = source_tree(app)
        result = self.command(label + "-run", [compiler, "run", "--frozen"], app,
                              status=1 if case.runtime_code else 0)
        outcome(result, case)
        require(source_tree(app) == original, case.name + ": execution mutated source/lock")
        if case.name == "files":
            require(all((app / name).is_file() for name in ("popped.bin", "truncated.bin")),
                    "File workflow did not create both expected files")
        if case.name == "file-error":
            require(not (app / "missing-parent").exists() and not (app / "truncated.bin").exists(),
                    "failed File open produced a later side effect")
        if case.name == "fault-push-file":
            require((app / "failure-file.bin").is_file(), "runtime failure did not reach its real File input")
        return {"sources": original, "runtime_code": case.runtime_code, "expected_result": case.result}

    def build(self, compiler, label, app, case, policy, profile):
        result = self.command(label + "-build-" + profile,
                              [compiler, "build", "--frozen", "--reproducible", "--profile=" + profile,
                               "--debug-info" if profile == "dev" else "--no-debug-info", "--message-format=json"], app)
        rows = json_records(result, "compile-summary", True)
        require(all(row.get("kind") != "diagnostic" for row in rows), "native build emitted a diagnostic")
        artifact = self.output / "artifacts" / (label + "-" + profile)
        shutil.copyfile(app / ".oxid/bin" / case.name, artifact)
        artifact.chmod(0o555)
        body = artifact.read_bytes()
        require(body[:7] == b"\x7fELF\x02\x01\x01"
                and body[16:20] == linux_native_target.elf_identity(3), "not an actual host PIE ELF")
        row = {**identity(artifact), "case": case.name, "policy": policy, "profile": profile,
               "optimization": "O0" if profile == "dev" else "O2", "debug_info": profile == "dev"}
        self.report["builds"].append(row)
        outcome(self.command(label + "-native-" + profile, [artifact], app,
                             status=1 if case.runtime_code else 0), case)
        return artifact

    def failed_build_preserves(self, compiler, label, app, profile):
        main = app / "src/main.ox"
        original = main.read_bytes()
        executable, lock = app / ".oxid/bin/bytebuf", app / "oxid.lock"
        before = (digest(executable), digest(lock), cache_tree(app / ".oxid/cache"),
                  cache_tree(self.output / "cache"))
        try:
            main.write_bytes(regular(ROOT, DATA / "double_move.ox").read_bytes())
            result = self.command(label + "-rejected-build-" + profile,
                                  [compiler, "build", "--frozen", "--profile=" + profile, "--message-format=json"],
                                  app, status=1)
            refused(result, next(case for case in CASES if case.name == "double-move"), "compile-summary")
            require((digest(executable), digest(lock), cache_tree(app / ".oxid/cache"),
                     cache_tree(self.output / "cache")) == before,
                    "failed source build changed the successful executable/lock/cache")
        finally:
            main.write_bytes(original)
        outcome(self.command(label + "-preserved-native-" + profile, [executable], app), CASES[0])

    def existing_example(self, compiler, label, base):
        app = base / "example"
        (app / "src").mkdir(parents=True)
        (app / "tests").mkdir()
        for name in ("oxid.toml", "src/main.ox", "tests/messages.ox"):
            (app / name).write_bytes(regular(ROOT, EXAMPLE / name).read_bytes())
        self.command(label + "-example-lock", [compiler, "lock", "--offline"], app)
        result = self.command(label + "-example-check", [compiler, "check", "--frozen", "--message-format=json"], app)
        json_records(result, "check-summary", True)
        result = self.command(label + "-example-tests", [compiler, "test", "--frozen"], app)
        require(result.stdout == b"" and result.stderr ==
                b'test "messages": ok (exit status: 0)\ntests: 1 passed; 0 failed\n',
                "declared example tests did not pass exactly")
        result = self.command(label + "-example-run", [compiler, "run", "--frozen"], app)
        require(result.stdout == b"42\n" and not result.stderr, "existing example oracle differs")
        # The direct typed-file adapter must reach the same genuine project AST.
        self.command(label + "-file-check", [compiler, "check", "src/main.ox", "--edition=typed-preview",
                                             "--message-format=json"], app)
        result = self.command(label + "-file-run", [compiler, "run", "src/main.ox", "--edition=typed-preview"], app)
        require(result.stdout == b"42\n" and not result.stderr, "explicit file adapter differs")
        return source_tree(app)


def verify(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo = args.repo.resolve(strict=True)
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_owned_record_vectors.py",
            "run the controller from the selected repository")
    output = args.output.resolve()
    require(not output.exists() and not output.is_symlink(), "output must be new")
    require(output != repo and repo not in output.parents, "output must be outside the repository")
    target = linux_native_target.required_host()
    compilers = {"debug": args.oxid_debug.resolve(strict=True), "release": args.oxid_release.resolve(strict=True)}
    require(len(set(compilers.values())) == 2 and all(path.is_file() and os.access(path, os.X_OK)
                                                    for path in compilers.values()), "distinct executable compiler paths required")
    llvm = args.llvm_bin.resolve(strict=True) if args.llvm_bin else None
    require(not args.native or llvm is not None, "native mode requires --llvm-bin")
    output.mkdir(parents=True)
    environment = {"PATH": (str(llvm) + ":" if llvm else "") + "/usr/bin:/bin", "LANG": "C.UTF-8",
                   "LC_ALL": "C.UTF-8", "TZ": "UTC", "SOURCE_DATE_EPOCH": "0",
                   "TMPDIR": str(output / "scratch"), "OXID_CACHE_DIR": str(output / "cache")}
    if llvm:
        environment["OXID_LLVM_BIN"] = str(llvm)
    if "LD_LIBRARY_PATH" in os.environ:
        environment["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    report = {"schema": "oxid-owned-record-vector-public-acceptance-v1", "status": "running",
              "scope": "finite non-Copy record vectors; eight ordinary operations; public CLI acceptance only",
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "native_requested": args.native, "host_target": target.triple,
              "windows_qualified": False, "macos_qualified": False, "g2_complete": False,
              "inherited_compatibility_qualified_by_this_gate": False,
              "allocator_cleanup_qualified_by_this_gate": False, "fd_leak_freedom_qualified_by_this_gate": False,
              "source_policy_reference": {"commit": POLICY_COMMIT, "controller": POLICY,
                                          "inherited_input_members": 527, "executed_by_this_gate": False},
              "compilers": {}, "tools": [], "builds": [], "deployments": [], "cases": []}
    driver = Driver(output, environment, report, repo=repo)
    try:
        head = driver.command("head", ["git", "--no-replace-objects", "rev-parse", "HEAD"], repo).stdout.decode().strip()
        require(head == args.expected_head and re.fullmatch(r"[0-9a-f]{40}", head), "checkout does not match --expected-head")
        report["checkout_head"] = head
        status = driver.command("checkout-status", ["git", "--no-replace-objects", "status", "--porcelain=v1", "--untracked-files=normal"], repo).stdout
        report["checkout_source_kind"] = "worktree" if status else "committed"
        report["compiler_source_provenance_verified_by_this_gate"] = False
        pins = {name: identity(regular(repo, name)) for name in input_paths()}
        report["inputs"] = pins
        policy_pins = {}
        for index, name in enumerate(POLICY_REFERENCES):
            frozen = driver.command("frozen-policy-" + str(index),
                                    ["git", "--no-replace-objects", "show", POLICY_COMMIT + ":" + name], repo).stdout
            require(frozen == regular(repo, name).read_bytes(), "frozen inherited policy changed: " + name)
            policy_pins[name] = hashlib.sha256(frozen).hexdigest()
        report["source_policy_reference"]["sha256"] = policy_pins
        if args.retained_result:
            # A reference is useful evidence linkage; it is not artifact-chain verification.
            report["retained_result_reference"] = identity(args.retained_result.resolve(strict=True))
        if args.native:
            for tool in ("clang", "opt", "ld.lld"):
                path = (llvm / tool).resolve(strict=True)
                result = driver.command("tool-" + tool, [llvm / tool, "--version"], output)
                require(b"19.1.7" in result.stdout + result.stderr, "requires actual LLVM 19.1.7: " + tool)
                report["tools"].append(identity(path))
        for name, compiler in compilers.items():
            compiler_pin = identity(compiler)
            report["compilers"][name] = compiler_pin
            base = output / "projects" / name
            base.mkdir()
            example_pin = driver.existing_example(compiler, name, base)
            pending = []
            for case in CASES:
                for policy in (("hosted",) if case.hosted_only else ("bounded", "hosted")):
                    app = driver.materialize(base, case, policy)
                    label = name + "-" + policy + "-" + case.name
                    observed = driver.check_case(compiler, label, app, case)
                    report["cases"].append({"compiler": name, "case": case.name, "policy": policy, **observed})
                    if args.native and case.native and not case.diagnostics:
                        for profile in ("dev", "release"):
                            artifact = driver.build(compiler, label, app, case, policy, profile)
                            pending.append((artifact, identity(artifact), case, policy, profile))
                            if case.name == "bytebuf":
                                driver.failed_build_preserves(compiler, label, app, profile)
            require(source_tree(base / "example") == example_pin, "existing example inputs changed")
            before = source_tree(base)
            unavailable = output / "retained-sources" / name
            base.rename(unavailable)
            try:
                require(not base.exists(), "source project paths remain available")
                for artifact, artifact_pin, case, policy, profile in pending:
                    require(identity(artifact) == artifact_pin, "generated artifact changed before deployment")
                    cwd = output / "deployments" / artifact.name
                    cwd.mkdir()
                    result = driver.command(artifact.name + "-source-unavailable", [artifact], cwd,
                                            status=1 if case.runtime_code else 0,
                                            overrides={"PATH": "", "OXID_LLVM_BIN": ""})
                    outcome(result, case)
                    require(identity(artifact) == artifact_pin, "generated artifact changed during deployment")
                    report["deployments"].append({"artifact": identity(artifact), "case": case.name,
                                                   "policy": policy, "profile": profile,
                                                   "source_project_paths_unavailable": not base.exists(),
                                                   "path_empty": True})
            finally:
                unavailable.rename(base)
            require(source_tree(base) == before, "retained source inputs changed during deployment")
            require(identity(compiler) == compiler_pin, "compiler changed during acceptance")
        require(all(identity(regular(repo, name)) == value for name, value in pins.items()), "acceptance inputs changed")
        require(all(identity(Path(row["path"])) == row for row in report["tools"]), "native tool changed")
        require(driver.command("head-after", ["git", "--no-replace-objects", "rev-parse", "HEAD"], repo).stdout.decode().strip() == head,
                "checkout changed during acceptance")
        report.update(status="passed", public_workflow_pass=True)
        print(json.dumps({"status": "passed", "native": args.native, "commands": len(report["commands"]),
                          "builds": len(report["builds"]), "output": str(output)}))
        return 0
    except BaseException as error:
        report.update(status="failed", public_workflow_pass=False, error=repr(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--oxid-debug", type=Path, required=True)
    parser.add_argument("--oxid-release", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path)
    parser.add_argument("--native", action="store_true")
    parser.add_argument("--retained-result", type=Path)
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
