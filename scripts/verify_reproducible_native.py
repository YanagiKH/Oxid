#!/usr/bin/env python3
"""Compare untouched native ELF bytes across clean/cache/relocated builds.

Requires Linux x86_64 and actual LLVM 19.1.7 plus its SDK. All comparisons use
one captured, explicitly recorded environment and preserve relative source
layout. This is a bounded R1 application regression, not a cross-SDK/OS claim.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time

from verify_typed_package_app import APPLICATION, CASES, ROOT, digest, expected, require, save

APP = "app Ω"
COUNTS = "共享 μ"
REPORT = "報告"
LABELS = ("clean-1", "clean-2", "cache-cold", "cache-warm", "relocated-clean",
          "relocated-transferred-cache", "relocated-cache-warm")
MAX_CAPTURE = 8 * 1024 * 1024


def tree_hashes(root, exclude_outputs=False):
    return {path.relative_to(root).as_posix(): {"bytes": path.stat().st_size, "sha256": digest(path)}
            for path in sorted(root.rglob("*")) if path.is_file()
            and not (exclude_outputs and ".oxid" in path.relative_to(root).parts)}


def prepare_layout(destination, workspace):
    app = destination / APP
    shutil.copytree(APPLICATION, app, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
    for location, name, entry in (("counts", COUNTS, "數量.ox"), ("report", REPORT, "報告.ox")):
        (app / "packages" / location).rename(destination / name)
        library = destination / name
        path = library / "oxid.toml"
        text = path.read_text()
        # Display names intentionally collide. Canonical roots and dependency
        # edges, including the app/report -> counts diamond, must stay distinct.
        lines = [line if not line.startswith("name = ") else 'name = "shared"' for line in text.splitlines()]
        text = "\n".join(lines).replace("[project]", '[project]\nkind = "lib"\nentry = "src/' + entry + '"', 1)
        text = text.replace('counts = "../counts"', 'counts = "../' + COUNTS + '"')
        path.write_text(text + "\n")
        (library / "src/lib.ox").rename(library / "src" / entry)
    (app / "packages").rmdir()
    path = app / "oxid.toml"
    text = path.read_text().replace('entry = "src/main.ox"', 'entry = "src/主程式.ox"')
    text = text.replace('counts = "packages/counts"', 'counts = "../' + COUNTS + '"')
    text = text.replace('report = "packages/report"', 'report = "../' + REPORT + '"')
    path.write_text(text)
    (app / "src/main.ox").rename(app / "src/主程式.ox")
    if workspace:
        (destination / "oxid.toml").write_text(
            '[workspace]\nedition = "typed-preview"\nmembers = ["' + APP + '", "' + COUNTS + '", "' + REPORT
            + '"]\ndefault-members = ["' + APP + '"]\n')


class Oracle:
    def __init__(self, output, environment, report):
        self.output, self.environment, self.report = output, environment, report
        self.commands = report["commands"] = []
        (output / "commands").mkdir()

    def command(self, label, argv, cwd, payload=b"", failure=False, overrides=None):
        directory = self.output / "commands" / f"{len(self.commands):04d}-{label}"
        directory.mkdir()
        (directory / "stdin.bin").write_bytes(payload)
        row = {"label": label, "argv": [str(value) for value in argv], "cwd": str(cwd),
               "environment_overrides": overrides or {}, "expected_failure": failure,
               "receipt": str(directory), "status": "running"}
        self.commands.append(row)
        started = time.monotonic()
        try:
            with (directory / "stdin.bin").open("rb") as stdin, (directory / "stdout.bin").open("wb") as stdout, \
                    (directory / "stderr.bin").open("wb") as stderr:
                result = subprocess.run(row["argv"], cwd=cwd, env={**self.environment, **(overrides or {})},
                                        stdin=stdin, stdout=stdout, stderr=stderr, timeout=120)
            row["exit_status"] = result.returncode
            for stream in ("stdin", "stdout", "stderr"):
                path = directory / (stream + ".bin")
                row[stream + "_bytes"] = path.stat().st_size
                row[stream + "_sha256"] = digest(path)
                require(path.stat().st_size <= MAX_CAPTURE, "command output exceeded retained capture limit")
            result.stdout = (directory / "stdout.bin").read_bytes()
            result.stderr = (directory / "stderr.bin").read_bytes()
            require(result.returncode == (1 if failure else 0), label + ": unexpected status; inspect " + str(directory))
            row["status"] = "passed"
            return result, len(self.commands) - 1
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            row.update(status="failed", error=str(error))
            raise
        finally:
            row["elapsed_seconds"] = time.monotonic() - started
            save(directory / "command.json", row)

    def cases(self, label, artifact, overrides=None):
        indexes = []
        for name, payload in CASES:
            result, index = self.command(label + "-" + name, [artifact], self.output, payload, overrides=overrides)
            require(result.stdout == expected(payload) and not result.stderr, label + "-" + name + ": byte oracle mismatch")
            indexes.append(index)
        return indexes

    def build(self, compiler, layout, workspace, variant, profile, label, incremental=False, cache=None, nested=False):
        app = layout / APP
        args = [compiler, "build", "--reproducible", "--frozen", "--message-format=json"]
        if workspace:
            args.append("--workspace")
        if incremental:
            args.append("--incremental")
        result, index = self.command(profile + "-" + variant + "-" + label, args, app / "src" if nested else app)
        require(not result.stderr, "build unexpectedly wrote stderr")
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        compiled = [row for row in rows if row.get("kind") == "compile-summary"]
        require(len(compiled) == 1 and compiled[0].get("success") is True, "build did not produce exactly one binary")
        events = [row for row in rows if row.get("kind") == "build-cache"]
        if incremental:
            require(len(events) == 1 and events[0].get("stage") == "native-object"
                    and events[0].get("status") in ("hit", "miss"), "native object cache was not exercised")
            if cache:
                require(events[0]["status"] == cache, "unexpected same-root native object reuse status")
        else:
            require(not events, "ordinary build unexpectedly reported cache use")
        if workspace:
            members = {row["member"]: row["status"] for row in rows if row.get("kind") == "workspace-member"}
            require(members == {APP: "ok", COUNTS: "checked-library", REPORT: "checked-library"},
                    "workspace build omitted a member or emitted a library binary")
            require(rows[-1].get("kind") == "workspace-summary" and rows[-1].get("success") is True
                    and rows[-1].get("total") == 3, "workspace did not complete all three members")
        executable = app / ".oxid/bin/byte-count"
        retained = self.output / "artifacts" / (profile + "-" + variant + "-" + label)
        shutil.copyfile(executable, retained)
        retained.chmod(0o555)
        body = retained.read_bytes()
        require(body[:7] == b"\x7fELF\x02\x01\x01" and body[16:20] == b"\x03\x00\x3e\x00", "build did not emit x86_64 PIE ELF")
        require(str(layout).encode() not in body and str(self.output).encode() not in body,
                "reproducible ELF contains an absolute source/staging/output root")
        names = ([APP + "/src/主程式.ox", COUNTS + "/src/數量.ox", REPORT + "/src/報告.ox"] if workspace else
                 ["src/主程式.ox", "../" + COUNTS + "/src/數量.ox", "../" + REPORT + "/src/報告.ox"])
        require(all(name.encode() in body for name in names), "ELF omits distinct logical source-path diagnostics")
        row = {"profile": profile, "variant": variant, "label": label, "command": index,
               "artifact": str(retained), "sha256": digest(retained), "bytes": len(body),
               "cache": events[0]["status"] if events else None, "logical_paths": names}
        self.report["builds"].append(row)
        row["cases"] = self.cases(profile + "-" + variant + "-" + label, retained)
        return retained

    def preflight_controls(self, compiler, layout, workspace, profile, variant):
        app = layout / APP
        executable = app / ".oxid/bin/byte-count"
        before = executable.read_bytes()
        cache = tree_hashes(app / ".oxid/cache")
        owner = layout if workspace else app
        lock = (owner / "oxid.lock").read_bytes()
        spy = self.output / "tools-spy"
        marker = self.output / "tools-spy-invoked"
        for label, path, changed in (
                ("parse-refusal", app / "src/主程式.ox", b"fn main(\n"),
                ("stale-lock-refusal", layout / COUNTS / "src/數量.ox", (layout / COUNTS / "src/數量.ox").read_bytes() + b"\n// changed input\n")):
            original = path.read_bytes()
            path.write_bytes(changed)
            try:
                args = [compiler, "build", "--reproducible", "--incremental", "--frozen", "--message-format=json"]
                if workspace:
                    args.append("--workspace")
                result, index = self.command(profile + "-" + variant + "-" + label, args, app, failure=True,
                                             overrides={"OXID_LLVM_BIN": str(spy)})
                rows = [json.loads(line) for line in result.stdout.splitlines()]
                require(not result.stderr and any(row.get("kind") == "diagnostic" for row in rows), "refusal lost structured diagnostics")
                if label == "parse-refusal":
                    require(any(row.get("code") == "E0100" and row.get("stage") == "parse" for row in rows),
                            "syntax control did not fail during parsing")
                else:
                    require(any("out of date" in row.get("message", "") for row in rows),
                            "dependency mutation did not fail existing lock verification")
                require(not any(row.get("kind") in ("build-cache", "workspace-member") for row in rows),
                        "preflight refusal reported partial build work")
                require(not marker.exists(), "preflight refusal invoked a native tool")
                require(executable.read_bytes() == before and tree_hashes(app / ".oxid/cache") == cache
                        and (owner / "oxid.lock").read_bytes() == lock, "preflight refusal changed previous output/cache/lock")
                self.report["controls"].append({"profile": profile, "variant": variant, "label": label, "command": index})
            finally:
                path.write_bytes(original)


def clean(layout):
    outputs = layout / APP / ".oxid"
    if outputs.exists():
        shutil.rmtree(outputs)


def verify(args):
    require(platform.system() == "Linux" and platform.machine() in ("x86_64", "AMD64"), "requires Linux x86_64")
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink() and ROOT not in output.parents and output != ROOT,
            "output must be a new directory outside the repository")
    llvm = args.llvm_bin.resolve(strict=True)
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(len(compilers) == len(set(compilers)) and all(path.is_file() and os.access(path, os.X_OK) for path in compilers),
            "compiler inputs must be distinct executable files")
    environment = {"PATH": str(llvm) + ":/usr/bin:/bin", "OXID_LLVM_BIN": str(llvm), "LANG": "C.UTF-8",
                   "LC_ALL": "C.UTF-8", "TZ": "UTC", "SOURCE_DATE_EPOCH": "0", "TMPDIR": str(output / "scratch")}
    if "LD_LIBRARY_PATH" in os.environ:
        environment["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    sources = tree_hashes(APPLICATION, exclude_outputs=True)
    controllers = {name: digest(ROOT / "scripts" / name) for name in (Path(__file__).name, "verify_typed_package_app.py")}
    output.mkdir(parents=True)
    for name in ("scratch", "artifacts", "compilers", "tools-spy"):
        (output / name).mkdir()
    # Spies are only used for admission failures, never equality/cache builds.
    for tool in ("clang", "opt", "ld.lld"):
        path = output / "tools-spy" / tool
        marker = str(output / "tools-spy-invoked").replace("'", "'\\''")
        path.write_text("#!/bin/sh\nprintf '%s\\n' invoked >> '" + marker + "'\nexit 91\n")
        path.chmod(0o700)
    report = {"schema": "oxid-reproducible-native-application-v1", "status": "running",
              "scope": "bounded Linux x86_64, same recorded toolchain/environment and preserved relative layout; not cross-SDK or v1",
              "environment": environment, "host": platform.uname()._asdict(), "source_inputs": sources,
              "controllers_sha256": controllers, "compilers": [], "tools": [], "layouts": {}, "builds": [], "controls": []}
    report["environment_sha256"] = hashlib.sha256(json.dumps(environment, sort_keys=True).encode()).hexdigest()
    oracle = Oracle(output, environment, report)
    try:
        for index, compiler in enumerate(compilers):
            retained = output / "compilers" / f"oxid-{index}"
            before = digest(compiler)
            shutil.copyfile(compiler, retained)
            retained.chmod(0o555)
            require(digest(compiler) == digest(retained) == before, "compiler changed during retention")
            report["compilers"].append({"profile": f"compiler-{index}", "original_profile": compiler.parent.name,
                                        "source": str(compiler), "retained": str(retained), "sha256": before})
        for tool in ("clang", "opt", "ld.lld"):
            path = llvm / tool
            result, command = oracle.command("version-" + tool, [path, "--version"], output)
            require(b"19.1.7" in result.stdout, "expected actual LLVM 19.1.7 tools")
            report["tools"].append({"name": tool, "invocation": str(path), "physical": str(path.resolve(strict=True)),
                                    "sha256": digest(path.resolve()), "command": command})
        layouts = []
        for workspace in (False, True):
            variant = "workspace" if workspace else "standalone"
            first = output / (variant + "-first")
            relocated = output / (variant + "-much-longer-relocated-root-遷移")
            prepare_layout(first, workspace)
            owner = first if workspace else first / APP
            oracle.command(variant + "-lock", [report["compilers"][0]["retained"], "lock", "--offline"], owner)
            shutil.copytree(first, relocated)
            inputs = tree_hashes(first, exclude_outputs=True)
            require(tree_hashes(relocated, exclude_outputs=True) == inputs, "relocation changed source/manifest/lock bytes")
            report["layouts"][variant] = {"first": str(first), "relocated": str(relocated), "files": inputs}
            layouts.append((variant, workspace, first, relocated))
        for compiler in report["compilers"]:
            profile, binary = compiler["profile"], Path(compiler["retained"])
            for variant, workspace, first, relocated in layouts:
                clean(first)
                oracle.build(binary, first, workspace, variant, profile, "clean-1")
                clean(first)
                for path in first.rglob("*.ox"):
                    os.utime(path, (946684800, 946684800))
                oracle.build(binary, first, workspace, variant, profile, "clean-2", nested=True)
                clean(first)
                oracle.build(binary, first, workspace, variant, profile, "cache-cold", True, "miss")
                (first / APP / ".oxid/bin/byte-count").unlink()
                oracle.build(binary, first, workspace, variant, profile, "cache-warm", True, "hit", nested=True)
                clean(relocated)
                oracle.build(binary, relocated, workspace, variant, profile, "relocated-clean")
                shutil.copytree(first / APP / ".oxid/cache", relocated / APP / ".oxid/cache")
                oracle.build(binary, relocated, workspace, variant, profile, "relocated-transferred-cache", True)
                (relocated / APP / ".oxid/bin/byte-count").unlink()
                oracle.build(binary, relocated, workspace, variant, profile, "relocated-cache-warm", True, "hit")
                rows = [row for row in report["builds"] if row["profile"] == profile and row["variant"] == variant]
                require(tuple(row["label"] for row in rows) == LABELS, "seven-build sequence is incomplete")
                reference = Path(rows[0]["artifact"]).read_bytes()
                require(all(Path(row["artifact"]).read_bytes() == reference for row in rows),
                        "raw linked ELF changed across clean/cache/relocated builds")
                oracle.preflight_controls(binary, relocated, workspace, profile, variant)
            require(digest(binary) == compiler["sha256"], "retained compiler changed")
        for variant, _, first, relocated in layouts:
            rows = [row for row in report["builds"] if row["variant"] == variant]
            reference = Path(rows[0]["artifact"]).read_bytes()
            require(all(Path(row["artifact"]).read_bytes() == reference for row in rows), "Rust compiler profile changed generated ELF bytes")
            require(tree_hashes(first, exclude_outputs=True) == report["layouts"][variant]["files"]
                    and tree_hashes(relocated, exclude_outputs=True) == report["layouts"][variant]["files"], "build changed retained source/lock bytes")
            shutil.rmtree(first)
            shutil.rmtree(relocated)
        report["source_free_deployments"] = []
        for row in report["builds"]:
            if row["label"] == "relocated-cache-warm":
                commands = oracle.cases(row["profile"] + "-" + row["variant"] + "-deployed", Path(row["artifact"]), {"PATH": ""})
                report["source_free_deployments"].append({"artifact": row["artifact"], "sha256": row["sha256"], "commands": commands})
        require(all(digest(Path(row["artifact"])) == row["sha256"] for row in report["builds"]),
                "retained ELF changed during oracle execution or source-free deployment")
        require(tree_hashes(APPLICATION, exclude_outputs=True) == sources, "repository application changed")
        require(all(digest(ROOT / "scripts" / name) == value for name, value in controllers.items()), "oracle controller changed")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "builds": len(report["builds"]),
                          "commands": len(report["commands"]), "output": str(output)}))
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
