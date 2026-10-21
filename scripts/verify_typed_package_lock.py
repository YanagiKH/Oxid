#!/usr/bin/env python3
"""Exercise typed dependency locking on the real three-package application.

Uses Python's TOML reader and byte-count oracle, then changes dependencies,
relocates the checkout and checks preservation of lockfiles/native output.
This qualifies local dependency inputs, not reproducible native machine code.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

from verify_typed_package_app import APPLICATION, ROOT, Runner, digest, require, save


def framed(value):
    return len(value).to_bytes(8, "little") + value


def reference_source_checksum(project, location, entry):
    # The application libraries each have one source file at this point. This
    # explicit input roster is independent of the compiler's source discovery.
    require(entry == "src/lib.ox", "application library entry unexpectedly changed")
    relative = entry.encode()
    data = (framed(b"oxid-typed-local-source-v1") + framed(relative)
            + (1).to_bytes(8, "little") + b"F" + framed(relative)
            + framed((project / location / entry).read_bytes()))
    return "sha256:" + hashlib.sha256(data).hexdigest()


class LockRunner(Runner):
    def rejected(self, name, words, project):
        result = self.command(name, [self.compiler, *words, "--message-format=json"],
                              project, failure=True)
        rows = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
        require(result.returncode == 1 and not result.stderr
                and any(row.get("kind") == "diagnostic" and row.get("severity") == "error"
                        for row in rows)
                and any(row.get("kind", "").endswith("-summary")
                        and row.get("success") is False for row in rows),
                name + ": expected structured rejection")
        return result

    def inspect_lock(self, project):
        lock = tomllib.loads((project / "oxid.lock").read_text())
        require(lock.get("version") == 2 and lock.get("kind") == "typed-local-v1",
                "lock must identify the separate typed local schema")
        packages = lock.get("package", [])
        by_path = {row.get("path"): row for row in packages}
        require(len(packages) == 3 and set(by_path) == {".", "packages/counts", "packages/report"},
                "diamond graph must retain exactly three canonical package identities")
        ids = {row["id"] for row in packages}
        require(len(ids) == 3 and lock.get("root") == by_path["."]["id"], "invalid root identity")
        for location, row in by_path.items():
            identity = hashlib.sha256(framed(b"oxid-typed-local-package-v1")
                                      + framed(location.encode())).hexdigest()
            require(row["id"] == identity, "package identity does not bind its relative canonical location")
            require(row["manifest"] == digest(project / location / "oxid.toml"),
                    "manifest checksum does not bind the actual raw bytes")
            require(row["edition"] == "typed-preview", "wrong locked edition")
            require(row["sources"] == "root" if location == "." else
                    row["sources"] == reference_source_checksum(project, location, row["entry"]),
                    "application sources must stay editable; dependency sources must be pinned")
        edges = {(row["owner"], row["alias"], row["package"])
                 for row in lock.get("dependency", [])}
        root, counts, report = (by_path[path]["id"] for path in (".", "packages/counts", "packages/report"))
        require(len(lock.get("dependency", [])) == 3
                and edges == {(root, "counts", counts), (root, "report", report), (report, "counts", counts)},
                "lock must bind both aliases to the same shared counts package")
        self.report["packages"] = packages
        self.report["edges"] = lock["dependency"]

    def check(self, native):
        project = self.output / "application"
        shutil.copytree(APPLICATION, project, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
        lock = project / "oxid.lock"
        self.command("plain-check", [self.compiler, "check"], project)
        require(not lock.exists(), "ordinary check unexpectedly created a lock")
        self.rejected("missing-lock", ["check", "--locked"], project)
        require(not lock.exists(), "failed locked check created a lock")
        self.command("create-offline", [self.compiler, "lock", "--offline"], project)
        self.inspect_lock(project)
        original = lock.read_bytes()
        self.command("repeat-lock", [self.compiler, "lock", "--locked"], project)
        require(lock.read_bytes() == original, "matching lock was rewritten")
        tested = self.command("frozen-tests", [self.compiler, "test", "--frozen", "--message-format=json"], project)
        rows = [json.loads(line) for line in tested.stdout.splitlines() if line.strip()]
        require(len(rows) == 2 and rows[0].get("kind") == "test-result"
                and rows[0].get("name") == "counts" and rows[0].get("success") is True
                and rows[0].get("exit_status") == 0
                and rows[1].get("kind") == "test-summary" and rows[1].get("success") is True
                and rows[1].get("passed") == 1
                and rows[1].get("failed") == 0 and not tested.stderr,
                "declared tests did not retain the locked dependency graph")
        self.cases("frozen-interpreted", [self.compiler, "run", "--frozen"], project)

        relocated = self.output / "relocated application"
        project.rename(relocated)
        project, lock = relocated, relocated / "oxid.lock"
        self.command("relocated-check", [self.compiler, "check", "--frozen"], project, env={"PATH": ""})
        require(lock.read_bytes() == original, "relocation changed the lock")
        (project / "packages/counts/README.md").write_text("Not part of the loaded source closure.\n")
        with (project / "src/main.ox").open("a") as stream:
            stream.write("\n// Application edits are intentionally outside the dependency checksum.\n")
        self.command("editable-application", [self.compiler, "check", "--frozen"], project)
        executable = project / ".oxid/bin/byte-count"
        if native:
            self.command("frozen-build", [self.compiler, "build", "--frozen"], project)
            require(executable.read_bytes().startswith(b"\x7fELF"), "native output must be ELF")
            self.report["initial_executable_sha256"] = digest(executable)
            self.cases("frozen-native", [executable], project)

        dependency = project / "packages/counts/src/lib.ox"
        source = dependency.read_text()
        marker = "totals.lines = totals.lines + 1;"
        require(source.count(marker) == 1, "application edit location changed")
        dependency.write_text(source.replace(marker, "totals.lines = totals.lines + 2;"))
        for name, words in (("changed-locked", ["check", "--locked"]),
                            ("changed-default", ["check"]),
                            ("changed-test", ["test", "--frozen"]),
                            ("changed-lock", ["lock"]),
                            ("offline-update-conflict", ["lock", "--update", "--offline"])):
            self.rejected(name, words, project)
            require(lock.read_bytes() == original, name + ": rejected operation modified lock")
        rejected = self.command("changed-run", [self.compiler, "run", "--frozen"], project,
                                payload=b"a\nb\n", failure=True)
        require(rejected.returncode == 1 and not rejected.stdout
                and rejected.stderr.startswith(b"error[E0001] (project): typed lock is out of date:"),
                "changed dependency must fail with the stale-lock diagnostic before execution")
        require(lock.read_bytes() == original, "rejected run changed the lock")
        if native:
            self.rejected("changed-build", ["build", "--frozen"], project)
            require(digest(executable) == self.report["initial_executable_sha256"],
                    "lock rejection replaced the last successful executable")
            self.cases("preserved-native", [executable], project)
        self.command("accept-update", [self.compiler, "lock", "--update"], project)
        require(lock.read_bytes() != original, "explicit update failed to bind changed dependency")
        self.inspect_lock(project)
        self.cases("updated-interpreted", [self.compiler, "run", "--frozen"], project, multiplier=2)
        accepted = lock.read_bytes()
        self.report["initial_lock_sha256"] = hashlib.sha256(original).hexdigest()
        self.report["updated_lock_sha256"] = hashlib.sha256(accepted).hexdigest()
        if native:
            self.command("updated-build", [self.compiler, "build", "--frozen"], project)
            self.report["updated_executable_sha256"] = digest(executable)
            require(digest(executable) != self.report["initial_executable_sha256"],
                    "accepted dependency change did not reach native output")
            self.cases("updated-native", [executable], project, multiplier=2)

        for name, contents in (("legacy", b"version = 1\n"),
                               ("unknown", b'version = 2\nkind = "future-schema"\n'),
                               ("truncated", accepted[:len(accepted) // 2])):
            lock.write_bytes(contents)
            self.rejected(name + "-update", ["lock", "--update"], project)
            require(lock.read_bytes() == contents, name + ": incompatible lock was overwritten")
        lock.write_bytes(accepted)
        self.command("restored-lock", [self.compiler, "check", "--frozen"], project)

        # Source directories do not inherit the legacy resolver's tree-ignore
        # rules: `target` and `.oxid` can contain real dependency entry/modules.
        manifest = project / "packages/counts/oxid.toml"
        original_manifest = manifest.read_text()
        for name in ("target", ".oxid"):
            prefix = name.lstrip(".")
            directory = project / "packages/counts" / name / "source"
            directory.mkdir(parents=True)
            entry = directory / "lib.ox"
            entry.write_text(dependency.read_text() + "\nmod extra;\n")
            extra = directory / "extra.ox"
            extra.write_text("fn value() -> i32 { return 7; }\n")
            manifest.write_text(original_manifest + f'entry = "{name}/source/lib.ox"\n')
            self.command(prefix + "-source-update", [self.compiler, "lock", "--update"], project)
            self.command(prefix + "-source-check", [self.compiler, "check", "--frozen"], project)
            pinned = lock.read_bytes()
            extra.write_text("fn value() -> i32 { return 8; }\n")
            self.rejected(prefix + "-module-change", ["check", "--frozen"], project)
            require(lock.read_bytes() == pinned, "closure mutation changed lock without update")


def verify(args):
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink(), "output directory must be new")
    require(output != ROOT and ROOT not in output.parents, "output must be outside the repository")
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(len(compilers) == len(set(compilers)), "compiler paths must be distinct")
    require(all(path.is_file() and os.access(path, os.X_OK) for path in compilers), "compiler must be executable")
    output.mkdir(parents=True)
    sources = {path.relative_to(APPLICATION).as_posix(): digest(path)
               for path in sorted(APPLICATION.rglob("*"))
               if path.is_file() and ".oxid" not in path.parts and path.name != "oxid.lock"}
    report = {"schema": "oxid-typed-package-lock-application-v1", "status": "running",
              "scope": "local dependency input locking; not native artifact reproducibility or v1 acceptance",
              "native": args.native, "source_sha256": sources,
              "controller_sha256": digest(Path(__file__)), "compilers": []}
    try:
        for index, compiler in enumerate(compilers):
            directory = output / f"compiler-{index}"
            directory.mkdir()
            row = {"path": str(compiler), "sha256": digest(compiler), "status": "running"}
            report["compilers"].append(row)
            LockRunner(directory, compiler, row).check(args.native)
            require(digest(compiler) == row["sha256"], "compiler changed during verification")
            row["status"] = "passed"
        require(all(digest(APPLICATION / name) == sha for name, sha in sources.items()),
                "application sources changed during verification")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "native": args.native,
                          "output": str(output)}))
        return 0
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    try:
        return verify(args)
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
