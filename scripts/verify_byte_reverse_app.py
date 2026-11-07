#!/usr/bin/env python3
"""Check byte-reverse against Python's independent bytes[::-1] oracle.

Reference mode needs no native tools. Native mode requires Linux x86_64 or ARM64 and
LLVM 19.1.7, retaining untouched ELF files and exact command streams. Raw ELF
equality is checked within each optimization profile, under one SDK/environment
and the same relative source layout; this is not a performance measurement.
"""
import linux_native_target as native_target
import argparse
import copy
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time
import tomllib

from verify_owned_enum_app import Oracle, ROOT, digest, require, save, tree_hashes
from verify_typed_package_lock import framed, reference_source_checksum

APPLICATION = ROOT / "apps/byte-reverse"
LOCATIONS = ("app", "binary", "reverse")
CONTROLLERS = ("verify_byte_reverse_app.py", "verify_owned_enum_app.py",
               "verify_typed_package_app.py", "verify_typed_package_lock.py")
MAXIMUM_INPUT = 8199
BOUNDARIES = (0, 1, 7, 8, 9, 255, 256, 257, 511, 512, 513,
              1023, 1024, 1025, 2047, 2048, 2049, 4095, 4096, 4097,
              8191, 8192, 8193, MAXIMUM_INPUT)
CASES = tuple((f"length-{size}", bytes((index * 73 + 19) % 256 for index in range(size)))
              for size in BOUNDARIES) + (
    ("nul-high-octets", bytes((0, 128, 255))),
    ("all-octets", bytes(range(256))),
    ("only-nul", bytes(1025)),
    ("only-high-octets", bytes(range(128, 256)) * 9),
    ("line-endings", b"\x00\xff\r\nfirst\nsecond\r\x80\x00"),
)
LONG_INPUT = dict(CASES)["length-8199"]


def identity(location):
    return hashlib.sha256(framed(b"oxid-typed-local-package-v1")
                          + framed(location.encode())).hexdigest()


def inspect_lock(project, lock):
    require(set(lock) == {"version", "kind", "manifest", "member", "package", "dependency"}
            and lock["version"] == 3 and lock["kind"] == "typed-workspace-v1",
            "expected the complete v3 workspace lock")
    require(lock["manifest"] == digest(project / "oxid.toml"), "workspace manifest hash differs")
    ids = {path: identity(path) for path in LOCATIONS}
    require(lock["member"] == [{"package": value} for value in sorted(ids.values())],
            "lock does not contain exactly the three workspace members")
    packages = lock["package"]
    require(len(packages) == 3 and [row["id"] for row in packages] == sorted(ids.values()),
            "package identities must be sorted and deduplicated")
    by_path = {row["path"]: row for row in packages}
    require(set(by_path) == set(LOCATIONS), "package path roster differs")
    for location, row in by_path.items():
        manifest = project / location / "oxid.toml"
        metadata = tomllib.loads(manifest.read_text())["project"]
        entry = "src/main.ox" if location == "app" else "src/lib.ox"
        wanted = {"id": ids[location], "path": location, "name": metadata["name"],
                  "version": metadata["version"], "edition": "typed-preview", "entry": entry,
                  "manifest": digest(manifest), "sources": "root" if location == "app" else
                  reference_source_checksum(project, location, entry)}
        require(row == wanted, "package metadata/source bytes differ: " + location)
    edges = sorted(((ids["app"], "binary", ids["binary"]),
                    (ids["app"], "reversal", ids["reverse"])))
    require(lock["dependency"] == [{"owner": owner, "alias": alias, "package": package}
                                   for owner, alias, package in edges],
            "lock does not preserve both local dependency aliases")
    return lock


def lock_oracle_controls(project, valid):
    mutations = {}
    wrong = copy.deepcopy(valid)
    wrong["member"].pop()
    mutations["missing-library-member"] = wrong
    wrong = copy.deepcopy(valid)
    wrong["manifest"] = "0" * 64
    mutations["wrong-workspace-bytes"] = wrong
    for label, field, value in (("wrong-library-source", "sources", "sha256:" + "0" * 64),
                                ("editable-library", "sources", "root"),
                                ("wrong-identity", "id", "0" * 64)):
        wrong = copy.deepcopy(valid)
        next(row for row in wrong["package"] if row["path"] == "binary")[field] = value
        mutations[label] = wrong
    wrong = copy.deepcopy(valid)
    wrong["dependency"][0]["alias"] = "missing"
    mutations["wrong-dependency-alias"] = wrong
    for label, mutation in mutations.items():
        try:
            inspect_lock(project, mutation)
        except (KeyError, TypeError, ValueError):
            continue
        raise ValueError("independent lock oracle accepted " + label)
    return sorted(mutations)


def records(result):
    require(not result.stderr, "JSON command wrote unexpected stderr")
    return [json.loads(line) for line in result.stdout.splitlines()]


def workspace_summary(rows, operation):
    require(rows and rows[-1] == {
        "schema_version": 1, "edition": "typed-preview", "kind": "workspace-summary",
        "operation": operation, "success": True, "total": 3, "failed": 0},
        operation + ": incorrect workspace summary")


class ReverseOracle(Oracle):
    def no_tools(self):
        return {"OXID_LLVM_BIN": str(self.spy), "OXID_APP_SPY_MARKER": str(self.marker)}

    def cases(self, label, argv, cwd, overrides=None):
        indexes = []
        for name, payload in CASES:
            _, index = self.command(label + "-" + name, argv, cwd, payload, 0,
                                    payload[::-1], overrides=overrides)
            indexes.append(index)
        _, index = self.command(label + "-directory-input", argv, cwd, status=74, stdout=b"",
                                overrides=overrides, directory_stdin=True)
        indexes.append(index)
        if len(argv) == 1:
            # Rust startup repairs missing standard fds before `oxid run`;
            # the closed-fd case therefore belongs to the generated ELF.
            _, index = self.command(label + "-closed-input", argv, cwd, status=74, stdout=b"",
                                    overrides=overrides, closed_stdin=True)
            indexes.append(index)
        for name, payload, status in (("full-output", LONG_INPUT, 74), ("empty-full-output", b"", 0)):
            _, index = self.command(label + "-" + name, argv, cwd, payload, status, b"",
                                    overrides=overrides, full_stdout=True)
            indexes.append(index)
        indexes.append(self.partial_write(label + "-partial-write", argv, cwd, overrides))
        return indexes

    def partial_write(self, label, argv, cwd, overrides):
        """Force real progress followed by EAGAIN without a scripted I/O layer."""
        folder = self.output / "commands" / f"{len(self.commands):04d}-{label}"
        folder.mkdir()
        (folder / "stdin.bin").write_bytes(LONG_INPUT)
        (folder / "stdout.bin").write_bytes(b"")
        row = {"label": label, "argv": [str(value) for value in argv], "cwd": str(cwd),
               "environment_overrides": overrides or {}, "expected_status": 74,
               "stdout_sink": "nonblocking-pipe", "pipe_capacity": 4096,
               "expected_stdout_sha256": hashlib.sha256(LONG_INPUT[::-1][:4096]).hexdigest(),
               "expected_stderr_sha256": hashlib.sha256(b"").hexdigest(),
               "receipt": str(folder), "status": "running"}
        self.commands.append(row)
        reader, writer = os.pipe()
        started = time.monotonic()
        try:
            require(fcntl.fcntl(writer, fcntl.F_SETPIPE_SZ, 4096) == 4096,
                    "partial-write oracle requires a 4096-byte pipe")
            os.set_blocking(writer, False)
            with (folder / "stdin.bin").open("rb") as source, (folder / "stderr.bin").open("wb") as errors:
                result = subprocess.run(row["argv"], cwd=cwd,
                                        env={**self.environment, **(overrides or {})},
                                        stdin=source, stdout=writer, stderr=errors, timeout=120)
            row["exit_status"] = result.returncode
            os.close(writer)
            writer = None
            # The child has exited and our write end is closed; reading cannot
            # wait for more data. No reader drains the pipe during execution.
            data = os.read(reader, 8192)
            require(os.read(reader, 1) == b"", "unexpected pipe tail")
            (folder / "stdout.bin").write_bytes(data)
            for stream in ("stdin", "stdout", "stderr"):
                path = folder / (stream + ".bin")
                row[stream + "_bytes"], row[stream + "_sha256"] = path.stat().st_size, digest(path)
            require(result.returncode == 74 and data == LONG_INPUT[::-1][:4096]
                    and not (folder / "stderr.bin").read_bytes(),
                    label + ": partial progress/status differ; inspect " + str(folder))
            row["status"] = "passed"
            return len(self.commands) - 1
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            row.update(status="failed", error=str(error))
            raise
        finally:
            os.close(reader)
            if writer is not None:
                os.close(writer)
            row["elapsed_seconds"] = time.monotonic() - started
            save(folder / "command.json", row)

    def retain_lock(self, project, label):
        lock = project / "oxid.lock"
        parsed = inspect_lock(project, tomllib.loads(lock.read_text()))
        retained = self.output / (label + "-oxid.lock")
        shutil.copyfile(lock, retained)
        self.report["locks"].append({"path": str(lock), "retained": str(retained),
                                     "sha256": digest(retained), "bytes": retained.stat().st_size,
                                     "parsed": parsed,
                                     "rejected_oracle_mutations": lock_oracle_controls(project, parsed)})

    def build(self, compiler, project, profile, label, cache=None):
        argv = [compiler, "build", "--workspace", "--frozen", "--reproducible",
                "--profile=" + profile, "--message-format=json"]
        if cache is not None:
            argv.append("--incremental")
        result, index = self.command(label, argv, project)
        rows = records(result)
        workspace_summary(rows, "build")
        members = [row for row in rows if row.get("kind") == "workspace-member"]
        require(members == [{"schema_version": 1, "edition": "typed-preview", "kind": "workspace-member",
                             "member": path, "status": "ok" if path == "app" else "checked-library"}
                            for path in LOCATIONS], "workspace binary/library statuses differ")
        cache_rows = [row for row in rows if row.get("kind") == "build-cache"]
        require(cache_rows == ([] if cache is None else [{
            "schema_version": 1, "edition": "typed-preview", "kind": "build-cache",
            "stage": "native-object", "status": cache}]), "wrong native object reuse outcome")
        require(len(rows) == (7 if cache is None else 8)
                and len([row for row in rows if row.get("kind") == "compile-summary"
                         and row.get("success") is True]) == 1
                and len([row for row in rows if row.get("kind") == "check-summary"
                         and row.get("success") is True]) == 2,
                "workspace must compile one binary and check two libraries")
        for location in LOCATIONS[1:]:
            require(not (project / location / ".oxid").exists(), "library check created native outputs")
        artifact = self.output / "artifacts" / label
        shutil.copyfile(project / "app/.oxid/bin/byte-reverse", artifact)
        artifact.chmod(0o555)
        body = artifact.read_bytes()
        require(body[:7] == b"\x7fELF\x02\x01\x01" and body[16:20] == native_target.elf_identity(3),
                "output is not a host PIE ELF")
        require(str(self.output).encode() not in body and str(project).encode() not in body,
                "reproducible ELF embeds an absolute fixture path")
        require(all(name.encode() in body for name in
                    ("app/src/main.ox", "binary/src/lib.ox", "reverse/src/lib.ox")),
                "logical source names are missing")
        row = {"label": label, "profile": profile, "optimization": "O0" if profile == "dev" else "O2",
               "command": index, "cache": cache, "artifact": str(artifact),
               "sha256": digest(artifact), "bytes": len(body)}
        self.report["builds"].append(row)
        row["cases"] = self.cases(label, [artifact], self.output)
        return artifact

    def reject(self, compiler, project, label, native, code=None, message=None, selected=False):
        locked = (project / "oxid.lock").read_bytes()
        before = tree_hashes(project / "app/.oxid")
        argv = [compiler, "build" if native else "check", "--frozen", "--message-format=json"]
        argv.extend(["-p", "app"] if selected else ["--workspace"])
        if native:
            argv.append("--profile=release")
        result, index = self.command(label, argv, project, status=1, overrides=self.no_tools())
        rows = records(result)
        diagnostics = [row for row in rows if row.get("kind") == "diagnostic"]
        require(any((code is None or row.get("code") == code)
                    and (message is None or message in row.get("message", "")) for row in diagnostics),
                label + ": incorrect refusal")
        require(any(row.get("kind", "").endswith("-summary") and row.get("success") is False for row in rows)
                and not any(row.get("kind") in ("build-cache", "test-result")
                            or row.get("kind") == "compile-summary" and row.get("success") for row in rows),
                "rejection followed native work or execution")
        members = [row for row in rows if row.get("kind") == "workspace-member"]
        if not native and code in ("E0310", "E0311"):
            # Workspace check intentionally reports independent semantic
            # results. Build preflight must reject before any publication.
            require([(row["member"], row["status"]) for row in members]
                    == [("app", "failed"), ("binary", "ok"), ("reverse", "ok")]
                    and rows[-1].get("kind") == "workspace-summary"
                    and rows[-1].get("failed") == 1, "incorrect check failure aggregation")
        else:
            require(not members, "preflight refusal followed partial workspace work")
        require(not self.marker.exists() and (project / "oxid.lock").read_bytes() == locked
                and tree_hashes(project / "app/.oxid") == before,
                "refusal changed an output/cache/lock or invoked tools")
        self.report["controls"].append({"label": label, "command": index,
                                        "native": native, "preserved_output_files": before})

    def controls(self, compiler, project, prefix, native):
        main, library, manifest = (project / "app/src/main.ox", project / "reverse/src/lib.ox",
                                   project / "reverse/oxid.toml")
        original = {path: path.read_bytes() for path in (main, library, manifest)}
        try:
            for label, body, code in (
                    ("parse", "fn main(\n", "E0100"),
                    ("moved-buffer", "use alloc::bytes::new;use alloc::bytes::len;"
                     "fn main()->i32{let b=new();let moved=b;return len(&b);}", "E0310"),
                    ("conflicting-view", "use alloc::bytes::new;use alloc::bytes::try_reserve;"
                     "use alloc::bytes::try_extend;fn main()->i32{let mut b=new();"
                     "try_reserve(&mut b,32);try_extend(&mut b,&b);return 0;}", "E0311")):
                main.write_text(body)
                self.reject(compiler, project, prefix + "-" + label, native, code=code)
            main.write_bytes(original[main])
            library.write_bytes(original[library] + b"\n// Dependency byte drift must invalidate the lock.\n")
            self.reject(compiler, project, prefix + "-stale-dependency", native,
                        message="lock is out of date", selected=True)
            library.write_bytes(original[library])
            manifest.write_bytes(b'[project]\nname = "unterminated\n')
            self.reject(compiler, project, prefix + "-malformed-unselected-member", native,
                        message="invalid manifest value", selected=True)
        finally:
            for path, raw in original.items():
                path.write_bytes(raw)

    def failed_link(self, compiler, project, prefix, llvm):
        tools = self.output / (prefix + "-failed-link-tools")
        tools.mkdir()
        for name in ("clang", "opt"):
            (tools / name).symlink_to(llvm / name)
        linker = tools / "ld.lld"
        linker.write_text('#!/bin/sh\nif [ "$1" = --version ]; then printf "%s\\n" "LLD 19.1.7"; exit 0; fi\n'
                          'printf invoked > "$OXID_APP_SPY_MARKER"\n'
                          'printf "%s\\n" "forced byte-reverse link failure" >&2\nexit 9\n')
        linker.chmod(0o755)
        marker = self.output / (prefix + "-link-invoked")
        before = tree_hashes(project / "app/.oxid")
        locked = (project / "oxid.lock").read_bytes()
        result, index = self.command(prefix + "-failed-link",
                                     [compiler, "build", "--workspace", "--frozen", "--profile=dev",
                                      "--reproducible", "--message-format=json"], project, status=1,
                                     overrides={"OXID_LLVM_BIN": str(tools), "OXID_APP_SPY_MARKER": str(marker)})
        rows = records(result)
        require(any(row.get("kind") == "diagnostic"
                    and "forced byte-reverse link failure" in row.get("message", "") for row in rows)
                and any(row.get("kind") == "compile-summary" and row.get("success") is False for row in rows),
                "failed linker did not propagate its actual error")
        require(marker.read_bytes() == b"invoked" and tree_hashes(project / "app/.oxid") == before
                and (project / "oxid.lock").read_bytes() == locked,
                "failed link changed the previous executable, cache or lock")
        self.report["controls"].append({"label": prefix + "-failed-link", "command": index,
                                        "native": True, "preserved_output_files": before})


def verify(args):
    require(native_target.host() is not None, "requires Linux x86_64 or ARM64")
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink() and output != ROOT and ROOT not in output.parents,
            "output must be a new directory outside the repository")
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(compilers and len(set(compilers)) == len(compilers)
            and all(path.is_file() and os.access(path, os.X_OK) for path in compilers), "invalid compiler paths")
    require(args.reference_only or args.llvm_bin is not None, "native verification requires --llvm-bin")
    llvm = args.llvm_bin.resolve(strict=True) if not args.reference_only else None
    output.mkdir(parents=True)
    overrides = {"PATH": (str(llvm) + ":" if llvm else "") + "/usr/bin:/bin", "LANG": "C.UTF-8",
                 "LC_ALL": "C.UTF-8", "TZ": "UTC", "SOURCE_DATE_EPOCH": "0", "TMPDIR": str(output / "scratch")}
    if llvm:
        overrides["OXID_LLVM_BIN"] = str(llvm)
    if "LD_LIBRARY_PATH" in os.environ:
        overrides["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    report = {"schema": "oxid-byte-reverse-application-v1", "status": "running",
              "reference_only": args.reference_only, "maximum_input_bytes": MAXIMUM_INPUT,
              "scope": "independent byte reversal and real fd errors; capacity/allocation injection belongs to runtime tests",
              "environment": overrides, "sources": tree_hashes(APPLICATION), "compilers": [], "tools": [],
              "controllers_sha256": {name: digest(ROOT / "scripts" / name) for name in CONTROLLERS},
              "locks": [], "builds": [], "controls": [], "deployments": [], "references": []}
    oracle = ReverseOracle(output, {**os.environ, **overrides}, report)
    baseline = {}
    try:
        if llvm:
            for name in ("clang", "opt", "ld.lld"):
                invocation = llvm / name
                physical = invocation.resolve(strict=True)
                before = digest(physical)
                result, index = oracle.command("version-" + name, [invocation, "--version"], output)
                with physical.open("rb") as stream:
                    magic = stream.read(4)
                require(b"19.1.7" in result.stdout and magic == b"\x7fELF",
                        "native/cache oracle requires actual LLVM 19.1.7 ELF tools")
                require(digest(physical) == before, "tool changed during version check")
                report["tools"].append({"name": name, "path": str(physical), "sha256": before, "command": index})
        for number, compiler in enumerate(compilers):
            prefix = "compiler-" + str(number)
            retained = output / "compilers" / prefix
            before = digest(compiler)
            shutil.copyfile(compiler, retained)
            retained.chmod(0o555)
            require(digest(retained) == before == digest(compiler), "compiler changed during retention")
            report["compilers"].append({"source": str(compiler), "retained": str(retained), "sha256": before})
            first, relocated = output / (prefix + "-source"), output / (prefix + "-relocated-longer-路徑")
            shutil.copytree(APPLICATION, first, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
            oracle.command(prefix + "-lock", [retained, "lock", "--offline", "--message-format=json"],
                           first, overrides=oracle.no_tools())
            oracle.retain_lock(first, prefix)
            checked, _ = oracle.command(prefix + "-check",
                                        [retained, "check", "--workspace", "--frozen", "--message-format=json"],
                                        first, overrides=oracle.no_tools())
            checked_rows = records(checked)
            workspace_summary(checked_rows, "check")
            require(len([row for row in checked_rows if row.get("kind") == "workspace-member"
                         and row.get("status") == "ok"]) == 3, "workspace check missed a member")
            indexes = oracle.cases(prefix + "-run", [retained, "run", "--frozen"], first / "app",
                                   overrides=oracle.no_tools())
            report["references"].append({"compiler": str(retained), "cases": indexes})
            require(not oracle.marker.exists(), "reference commands invoked native tools")
            sources = tree_hashes(first)
            deployed = []
            if llvm:
                shutil.copytree(first, relocated, ignore=shutil.ignore_patterns(".oxid"))
                for profile in ("dev", "release"):
                    cache_root = first / "app/.oxid"
                    if cache_root.exists():
                        shutil.rmtree(cache_root)
                    label = prefix + "-" + profile
                    clean = oracle.build(retained, first, profile, label + "-clean")
                    cold = oracle.build(retained, first, profile, label + "-cache-cold", "miss")
                    (first / "app/.oxid/bin/byte-reverse").unlink()
                    warm = oracle.build(retained, first, profile, label + "-cache-warm", "hit")
                    moved = oracle.build(retained, relocated, profile, label + "-relocated")
                    require(clean.read_bytes() == cold.read_bytes() == warm.read_bytes() == moved.read_bytes(),
                            "clean/cache/relocation changed raw ELF bytes within " + profile)
                    require(digest(clean) == baseline.setdefault(profile, digest(clean)),
                            "Rust compiler build profile changed generated ELF bytes")
                    deployed.append((profile, moved))
                oracle.controls(retained, first, prefix, True)
                oracle.failed_link(retained, first, prefix, llvm)
                oracle.cases(prefix + "-preserved", [first / "app/.oxid/bin/byte-reverse"], output)
                require(tree_hashes(relocated) == sources, "relocated source/lock inputs changed")
            else:
                oracle.controls(retained, first, prefix, False)
                require(not any((first / member / ".oxid").exists() for member in LOCATIONS),
                        "reference verification created native output storage")
            require(tree_hashes(first) == sources, "source/lock inputs changed")
            shutil.rmtree(first)
            if relocated.exists():
                shutil.rmtree(relocated)
            for profile, artifact in deployed:
                indexes = oracle.cases(artifact.name + "-deployed", [artifact], output, overrides={"PATH": ""})
                report["deployments"].append({"artifact": str(artifact), "sha256": digest(artifact),
                                               "profile": profile, "cases": indexes,
                                               "removed_source_roots": [str(first), str(relocated)],
                                               "source_trees_removed": not first.exists() and not relocated.exists()})
            require(digest(compiler) == before == digest(retained), "compiler changed during verification")
        require(tree_hashes(APPLICATION) == report["sources"], "repository application changed")
        require(all(digest(Path(row["path"])) == row["sha256"] for row in report["tools"]), "tool changed")
        require(all(digest(ROOT / "scripts" / name) == sha for name, sha in report["controllers_sha256"].items()),
                "controller changed")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "reference_only": args.reference_only,
                          "maximum_input_bytes": MAXIMUM_INPUT, "compilers": len(compilers),
                          "builds": len(report["builds"]), "commands": len(report["commands"]), "output": str(output)}))
        return 0
    except (OSError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--llvm-bin", type=Path)
    parser.add_argument("--reference-only", action="store_true")
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
