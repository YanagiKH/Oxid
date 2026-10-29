#!/usr/bin/env python3
"""Qualify the owned UTF-8 text package through real debug/release Oxid CLIs.

With --native, use LLVM 19.1.7 and replay copied executables with their original
project and cache paths unavailable. This finite gate does not certify all of
G2, allocator fault injection, other platforms, or the complete v1 release.
"""
import argparse
from dataclasses import dataclass
import datetime
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import sys

import linux_native_target
import verify_owned_record_vectors as public


ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = Path("examples/typed-owned-text")
LIBRARY = Path("packages/utf8_text")
APPLICATION = Path("apps/utf8-normalize")
EXAMPLE_FILES = ("oxid.toml", "README.md", "src/main.ox", "tests/text.ox",
                 "tests/reject/self-append.ox", "tests/reject/private-bytes.ox",
                 "tests/reject/moved-text.ox", "tests/reject/returned-borrow.ox")
LIBRARY_FILES = ("oxid.toml", "README.md", "src/lib.ox")
APPLICATION_FILES = ("oxid.toml", "README.md", "app/oxid.toml", "app/src/main.ox",
                     "contracts/oxid.toml", "contracts/src/main.ox")
CONTROLLERS = ("scripts/verify_owned_text.py", "scripts/test_owned_text_verifier.py",
               "scripts/verify_owned_record_vectors.py", "scripts/linux_native_target.py")
POLICIES = ("bounded", "hosted")
PROFILES = ("dev", "release")


@dataclass(frozen=True)
class Case:
    name: str
    entry: str
    result: int
    workspace: bool = False


CASES = (Case("main", "src/main.ox", 16), Case("text-tests", "tests/text.ox", 0),
         Case("contracts", "contracts/src/main.ox", 0, workspace=True))


@dataclass(frozen=True)
class Rejection:
    name: str
    stage: str
    code: str


REJECTIONS = (Rejection("self-append", "ownership", "E0311"),
              Rejection("private-bytes", "type", "E0206"),
              Rejection("moved-text", "ownership", "E0310"),
              Rejection("returned-borrow", "parse", "E0101"))


def input_paths():
    paths = [str(root / name) for root, names in
             ((EXAMPLE, EXAMPLE_FILES), (LIBRARY, LIBRARY_FILES), (APPLICATION, APPLICATION_FILES))
             for name in names]
    paths.extend(CONTROLLERS)
    public.require(len(paths) == len(set(paths)) == 21, "owned-text input census changed")
    return sorted(paths)


def source_snapshot(root):
    return {file.relative_to(root).as_posix(): public.digest(file)
            for file in sorted(root.rglob("*"))
            if file.is_file() and ".oxid" not in file.relative_to(root).parts
            and (file.suffix == ".ox" or file.name in ("oxid.toml", "oxid.lock", "README.md"))}


def records(result):
    public.require(not result.stderr and result.stdout.endswith(b"\n"), "JSON output framing differs")
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    public.require(rows and all(type(row) is dict and type(row.get("schema_version")) is int
                               and row.get("schema_version") == 1
                               and row.get("edition") == "typed-preview" for row in rows),
                   "unexpected public JSON schema")
    return rows


def successful(result, kind):
    rows = records(result)
    public.require(result.returncode == 0 and len(rows) == 1 and rows[0].get("kind") == kind
                   and rows[0].get("success") is True, "wrong successful public summary")
    return rows[0]


def outcome(result, case):
    public.require(result.returncode == 0 and result.stdout == (str(case.result) + "\n").encode()
                   and not result.stderr, case.name + ": result status or complete output differs")


def refused(result, case, summary):
    rows = records(result)
    public.require(result.returncode == 1 and len(rows) >= 2
                   and rows[-1].get("kind") == summary and rows[-1].get("success") is False,
                   case.name + ": wrong source refusal summary")
    errors = rows[:-1]
    public.require(all(row.get("kind") == "diagnostic" and row.get("severity") == "error"
                       and (row.get("stage"), row.get("code")) == (case.stage, case.code)
                       for row in errors), case.name + ": wrong source diagnostic stage/code")
    return [(row["stage"], row["code"]) for row in errors]


def declared_test(result, workspace=False):
    rows = records(result)
    test = {"schema_version": 1, "edition": "typed-preview", "kind": "test-result",
            "name": "utf8" if workspace else "text", "success": True, "exit_status": 0,
            "stdout": "", "stderr": ""}
    final = {"schema_version": 1, "edition": "typed-preview", "kind": "test-summary",
             "success": True, "passed": 1, "failed": 0}
    if workspace:
        test["member"] = "contracts"
        final = workspace_summary("test", 1)
    counts = ("total", "failed") if workspace else ("passed", "failed")
    public.require(result.returncode == 0 and rows == [test, final]
                   and type(rows[0].get("exit_status")) is int
                   and all(type(rows[-1].get(name)) is int for name in counts),
                   "declared test did not finish exactly")


def workspace_summary(operation, total):
    return {"schema_version": 1, "edition": "typed-preview", "kind": "workspace-summary",
            "operation": operation, "success": True, "total": total, "failed": 0}


def workspace_operation(result, operation, members):
    rows = records(result)
    public.require(result.returncode == 0 and rows[-1] == workspace_summary(operation, len(members))
                   and all(type(rows[-1].get(name)) is int for name in ("total", "failed"))
                   and len(rows) == 2 * len(members) + 1, "wrong workspace operation census")
    for index, member in enumerate(members):
        summary, selected = rows[index * 2:index * 2 + 2]
        public.require(summary.get("kind") == ("compile-summary" if operation == "build" else "check-summary")
                       and summary.get("success") is True, "workspace child did not succeed")
        public.require(selected == {"schema_version": 1, "edition": "typed-preview", "kind": "workspace-member",
                                    "member": member, "status": "ok"}, "workspace selected the wrong member")


def replace_once(body, before, after):
    public.require(body.count(before) == 1, "copied manifest transformation anchor changed")
    return body.replace(before, after, 1)


class Driver(public.Driver):
    def __init__(self, output, environment, report):
        super().__init__(output, environment, report)
        (output / "compilers").mkdir()
        self.spy = output / "no-native-tools"
        self.spy.mkdir()
        self.marker = output / "unexpected-native-tool"
        for name in ("clang", "opt", "ld.lld"):
            path = self.spy / name
            path.write_text("#!/bin/sh\nprintf invoked > " + shlex.quote(str(self.marker)) + "\nexit 91\n")
            path.chmod(0o555)

    def no_tools(self):
        return {"OXID_LLVM_BIN": str(self.spy)}

    def materialize(self, base, case, policy):
        files = APPLICATION_FILES if case.workspace else EXAMPLE_FILES
        relative = APPLICATION if case.workspace else EXAMPLE
        for root, names in ((relative, files), (LIBRARY, LIBRARY_FILES)):
            for name in names:
                destination = base / root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(public.regular(ROOT, root / name).read_bytes())
        app = base / relative
        manifest = app / ("contracts/oxid.toml" if case.workspace else "oxid.toml")
        before = manifest.read_bytes()
        body = before
        changes = []
        if not case.workspace and case.entry != "src/main.ox":
            body = replace_once(body, b'entry = "src/main.ox"', ('entry = "' + case.entry + '"').encode())
            changes.append({"entry": case.entry})
        if policy == "hosted":
            if case.workspace:
                public.require(b"[execution]" not in body, "contracts gained an explicit execution policy")
                body += b'\n[execution]\npolicy = "hosted"\n'
            else:
                body = replace_once(body, b'policy = "bounded"', b'policy = "hosted"')
            changes.append({"policy": "hosted"})
        manifest.write_bytes(body)
        self.report["materializations"].append({"project": str(app), "case": case.name,
            "policy": policy, "manifest": str(manifest), "source_manifest_sha256":
            public.digest(public.regular(ROOT, relative / ("contracts/oxid.toml" if case.workspace else "oxid.toml"))),
            "materialized_manifest_sha256": public.digest(manifest), "changes": changes})
        return app

    def lock(self, compiler, label, app, workspace):
        kind = "workspace-lock-summary" if workspace else "lock-summary"
        for changed in (True, False):
            result = self.command(label + ("-lock" if changed else "-verify-lock"),
                [compiler, "lock", "--offline", "--message-format=json"], app, overrides=self.no_tools())
            row = successful(result, kind)
            expected = {"schema_version": 1, "edition": "typed-preview", "kind": kind,
                        "success": True, "changed": changed}
            if workspace:
                expected["members"] = 2
            public.require(row == expected and type(row.get("changed")) is bool
                           and (not workspace or type(row.get("members")) is int),
                           "offline lock did not write then verify the same graph")
            pin = public.identity(app / "oxid.lock")
            if changed:
                original = pin
                self.report["locks"].append(pin)
            else:
                public.require(pin == original, "unchanged offline lock rewrote its bytes")

    def positive(self, compiler, label, app, case):
        self.lock(compiler, label, app, case.workspace)
        before = source_snapshot(app.parent.parent)
        check = [compiler, "check", "--frozen", "--message-format=json"]
        if case.workspace:
            check.append("--workspace")
        result = self.command(label + "-check", check, app, overrides=self.no_tools())
        if case.workspace:
            workspace_operation(result, "check", ("app", "contracts"))
        else:
            successful(result, "check-summary")
        argv = [compiler, "run", "--frozen"] + (["-p", "contracts"] if case.workspace else [])
        outcome(self.command(label + "-run", argv, app, overrides=self.no_tools()), case)
        if case.name in ("main", "contracts"):
            argv = [compiler, "test", "--frozen", "--message-format=json"]
            if case.workspace:
                argv.extend(("-p", "contracts"))
            declared_test(self.command(label + "-test", argv, app, overrides=self.no_tools()), case.workspace)
            self.report["declared_tests"].append({"label": label, "name": "utf8" if case.workspace else "text"})
        public.require(not self.marker.exists(), "reference workflow invoked a native tool")
        public.require(source_snapshot(app.parent.parent) == before,
                       "reference workflow changed its source/lock inputs")

    def reject(self, compiler, label, app, case, cache):
        before = (source_snapshot(app.parent.parent), public.cache_tree(app / ".oxid"), public.cache_tree(cache))
        result = self.command(label + "-check", [compiler, "check", "--message-format=json"], app,
                              status=1, overrides=self.no_tools())
        diagnostics = refused(result, case, "check-summary")
        for profile in PROFILES:
            result = self.command(label + "-build-" + profile,
                [compiler, "build", "--profile=" + profile, "--message-format=json"], app,
                status=1, overrides=self.no_tools())
            refused(result, case, "compile-summary")
        public.require(not self.marker.exists(), "rejected source reached a native tool")
        public.require((source_snapshot(app.parent.parent), public.cache_tree(app / ".oxid"), public.cache_tree(cache)) == before,
                       "rejected source changed the source/lock/cache state")
        return diagnostics

    def build(self, compiler, label, app, case, policy, profile):
        before = source_snapshot(app.parent.parent)
        argv = [compiler, "build", "--frozen", "--reproducible", "--profile=" + profile,
                "--debug-info" if profile == "dev" else "--no-debug-info", "--message-format=json"]
        if case.workspace:
            argv.extend(("-p", "contracts"))
        result = self.command(label + "-build-" + profile, argv, app)
        if case.workspace:
            workspace_operation(result, "build", ("contracts",))
        else:
            successful(result, "compile-summary")
        produced = app / ("contracts/.oxid/bin/utf8-text-contracts" if case.workspace else ".oxid/bin/owned-text")
        artifact = self.output / "artifacts" / (label + "-" + profile)
        shutil.copyfile(produced, artifact)
        artifact.chmod(0o555)
        with artifact.open("rb") as stream:
            header = stream.read(20)
        public.require(header[:7] == b"\x7fELF\x02\x01\x01"
                       and header[16:20] == linux_native_target.elf_identity(3), "expected an actual host PIE ELF")
        pin = public.identity(artifact)
        self.report["builds"].append({**pin, "case": case.name, "policy": policy, "profile": profile,
                                      "optimization": "O0" if profile == "dev" else "O2"})
        outcome(self.command(label + "-native-" + profile, [artifact], app), case)
        public.require(source_snapshot(app.parent.parent) == before, "native workflow changed source/lock inputs")
        return artifact, pin

    def deploy(self, base, cache, pending, label):
        before, cache_before = source_snapshot(base), public.cache_tree(cache)
        source_archive = self.output / "retained-sources" / label
        cache_archive = self.output / "retained-sources" / (label + "-cache")
        base.rename(source_archive)
        cache.rename(cache_archive)
        inherited = self.environment
        try:
            self.environment = {}
            public.require(not base.exists() and not cache.exists(), "original source/cache paths remain available")
            for artifact, pin, case, policy, profile in pending:
                public.require(not base.exists() and not cache.exists(), "original paths reappeared before deployment")
                public.require(public.identity(artifact) == pin, "artifact changed before copying")
                cwd = self.output / "deployments" / artifact.name
                cwd.mkdir()
                copied = cwd / artifact.name
                shutil.copyfile(artifact, copied)
                copied.chmod(0o555)
                public.require(public.digest(copied) == pin["sha256"], "deployed executable bytes differ")
                outcome(self.command(artifact.name + "-deployed", [copied], cwd), case)
                public.require(not base.exists() and not cache.exists(), "deployed executable recreated original paths")
                public.require(list(cwd.iterdir()) == [copied] and public.digest(copied) == pin["sha256"],
                               "deployed executable changed files")
                self.report["deployments"].append({**public.identity(copied), "case": case.name,
                    "policy": policy, "profile": profile, "environment": {}, "original_source": str(base),
                    "original_cache": str(cache), "source_and_cache_unavailable": not base.exists() and not cache.exists()})
        finally:
            self.environment = inherited
            cache_archive.rename(cache)
            source_archive.rename(base)
        public.require(source_snapshot(base) == before and public.cache_tree(cache) == cache_before,
                       "retained source/cache changed during deployment")


def verify(args):
    public.require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo = args.repo.resolve(strict=True)
    public.require(repo == ROOT and Path(__file__).resolve() == repo / CONTROLLERS[0], "wrong controller checkout")
    public.require(re.fullmatch(r"[0-9a-f]{40}", args.expected_head) is not None, "expected a complete commit identity")
    output = args.output.resolve()
    public.require(not output.exists() and not args.output.is_symlink() and repo != output
                   and repo not in output.parents, "output must be new and outside the repository")
    compilers = {"debug": args.oxid_debug.resolve(strict=True), "release": args.oxid_release.resolve(strict=True)}
    public.require(len(set(compilers.values())) == 2 and all(path.is_file() and os.access(path, os.X_OK)
                   for path in compilers.values()), "distinct real executable compiler paths required")
    target = linux_native_target.required_host()
    llvm = args.llvm_bin.resolve(strict=True) if args.llvm_bin else None
    public.require(not args.native or llvm is not None, "--native requires --llvm-bin")
    output.mkdir(parents=True)
    environment = {"PATH": (str(llvm) + ":" if llvm else "") + "/usr/bin:/bin", "LANG": "C.UTF-8",
                   "LC_ALL": "C.UTF-8", "TZ": "UTC", "SOURCE_DATE_EPOCH": "0", "TMPDIR": str(output / "scratch")}
    if llvm:
        environment["OXID_LLVM_BIN"] = str(llvm)
    if "LD_LIBRARY_PATH" in os.environ:
        environment["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    report = {"schema": "oxid-owned-text-public-acceptance-v1", "status": "running",
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "native_requested": args.native, "host_target": target.triple,
              "scope": "four owned UTF-8 text APIs, imported Text vectors and ordinary package workflows",
              "g2_complete": False, "full_v1_qualification": False, "allocator_fault_injection_qualified": False,
              "compiler_source_provenance_verified": False, "compilers": {}, "tools": [], "tool_invocations": [], "inputs": {},
              "positive_cases": [], "rejections": [], "declared_tests": [], "locks": [], "builds": [],
              "deployments": [], "materializations": []}
    driver = Driver(output, environment, report)
    try:
        result = driver.command("head", ["git", "--no-replace-objects", "rev-parse", "HEAD"], repo)
        public.require(result.stdout == (args.expected_head + "\n").encode() and not result.stderr,
                       "checkout does not match expected head")
        head = args.expected_head
        report["checkout_head"] = head
        pins = {name: public.identity(public.regular(repo, name)) for name in input_paths()}
        report["inputs"] = pins
        for root, names in ((EXAMPLE, EXAMPLE_FILES), (LIBRARY, LIBRARY_FILES), (APPLICATION, APPLICATION_FILES)):
            public.require(set(source_snapshot(repo / root)) - {"oxid.lock"} == set(names),
                           "unexpected source fixture census: " + str(root))
        if args.native:
            for name in ("clang", "opt", "ld.lld"):
                invocation = llvm / name
                path = invocation.resolve(strict=True)
                pin = public.identity(path)
                with path.open("rb") as stream:
                    public.require(stream.read(4) == b"\x7fELF", "native tools must be original ELF executables")
                result = driver.command("version-" + name, [invocation, "--version"], output)
                public.require(b"19.1.7" in result.stdout and not result.stderr, "LLVM 19.1.7 required")
                public.require(invocation.resolve(strict=True) == path and public.identity(path) == pin,
                               "LLVM invocation or executable changed during version check")
                report["tools"].append(pin)
                report["tool_invocations"].append({"path": str(invocation), "resolved_path": str(path)})
        for compiler_name, original_compiler in compilers.items():
            original_pin = public.identity(original_compiler)
            with original_compiler.open("rb") as stream:
                public.require(stream.read(7) == b"\x7fELF\x02\x01\x01", "compiler must be a real Linux64 ELF executable")
            compiler = output / "compilers" / compiler_name
            shutil.copyfile(original_compiler, compiler)
            compiler.chmod(0o555)
            public.require(public.digest(compiler) == original_pin["sha256"], "compiler copy differs")
            report["compilers"][compiler_name] = {"original": original_pin, "retained": public.identity(compiler)}
            base = output / "projects" / compiler_name
            cache = output / (compiler_name + "-cache")
            base.mkdir()
            cache.mkdir()
            driver.environment["OXID_CACHE_DIR"] = str(cache)
            pending = []
            for policy in POLICIES:
                for case in CASES:
                    label = compiler_name + "-" + policy + "-" + case.name
                    app = driver.materialize(base / policy / case.name, case, policy)
                    driver.positive(compiler, label, app, case)
                    report["positive_cases"].append({"compiler": compiler_name, "policy": policy,
                                                     "case": case.name, "expected_result": case.result})
                    if args.native:
                        for profile in PROFILES:
                            artifact, pin = driver.build(compiler, label, app, case, policy, profile)
                            pending.append((artifact, pin, case, policy, profile))
                for rejection in REJECTIONS:
                    label = compiler_name + "-" + policy + "-reject-" + rejection.name
                    case = Case("reject-" + rejection.name, "tests/reject/" + rejection.name + ".ox", 0)
                    app = driver.materialize(base / policy / case.name, case, policy)
                    diagnostics = driver.reject(compiler, label, app, rejection, cache)
                    report["rejections"].append({"compiler": compiler_name, "policy": policy,
                                                 "case": rejection.name, "diagnostics": diagnostics})
            driver.deploy(base, cache, pending, compiler_name)
            public.require(public.identity(original_compiler) == original_pin
                           and public.digest(compiler) == original_pin["sha256"], "compiler changed during acceptance")
        public.require(len(report["positive_cases"]) == 12 and len(report["rejections"]) == 16
                       and len(report["declared_tests"]) == 8, "reference case census differs")
        expected_native = 24 if args.native else 0
        public.require(len(report["builds"]) == len(report["deployments"]) == expected_native,
                       "native/deployment census differs")
        public.require(all(public.identity(public.regular(repo, name)) == pin for name, pin in pins.items()),
                       "source/controller input changed")
        public.require(all(public.identity(Path(pin["path"])) == pin for pin in report["tools"])
                       and all(Path(binding["path"]).resolve(strict=True) == Path(binding["resolved_path"])
                               for binding in report["tool_invocations"]), "LLVM tool or invocation changed")
        result = driver.command("head-after", ["git", "--no-replace-objects", "rev-parse", "HEAD"], repo)
        public.require(result.stdout == (head + "\n").encode() and not result.stderr,
                       "checkout changed during acceptance")
        report.update(status="passed", public_workflow_pass=True)
        print(json.dumps({"status": "passed", "native": args.native, "builds": expected_native,
                          "commands": len(report["commands"]), "output": str(output)}))
        return 0
    except BaseException as error:
        report.update(status="failed", public_workflow_pass=False, error=repr(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        public.save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--oxid-debug", type=Path, required=True)
    parser.add_argument("--oxid-release", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path)
    parser.add_argument("--native", action="store_true")
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
