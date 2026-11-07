#!/usr/bin/env python3
"""Verify real byte-count workspace workflows against independent byte/lock oracles.

Copies the existing three-package application outside the repository. Native
mode checks object reuse, failure preservation and source-free ELF deployment.
This is R1 workspace regression evidence, not v1 or reproducible-artifact proof.
"""
import linux_native_target as native_target
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

from verify_typed_package_app import APPLICATION, CASES, ROOT, Runner, digest, require, save
from verify_typed_package_lock import framed, reference_source_checksum

LOCATIONS = (".", "packages/counts", "packages/report")
WORKSPACE = ('\n[workspace]\nedition = "typed-preview"\n'
             'members = ["packages/counts", "packages/report"]\n'
             'default-members = ["."]\n')


def identity(location):
    return hashlib.sha256(framed(b"oxid-typed-local-package-v1")
                          + framed(location.encode())).hexdigest()


def inspect_lock(project, lock):
    """Independent application roster: never trust compiler-reported identities."""
    require(set(lock) == {"version", "kind", "manifest", "member", "package", "dependency"}
            and lock["version"] == 3 and lock["kind"] == "typed-workspace-v1",
            "expected exact typed workspace v3 schema")
    require(lock["manifest"] == digest(project / "oxid.toml"),
            "workspace manifest hash must bind its raw bytes")
    ids = {path: identity(path) for path in LOCATIONS}
    require(lock["member"] == [{"package": value} for value in sorted(ids.values())],
            "workspace lock must contain every canonical member exactly once")
    packages = lock["package"]
    require(len(packages) == len(LOCATIONS), "workspace diamond must have three packages")
    require([row["id"] for row in packages] == sorted(ids.values()),
            "workspace package identities must be sorted and deduplicated")
    by_path = {row["path"]: row for row in packages}
    require(set(by_path) == set(LOCATIONS), "workspace package path roster changed")
    for location, row in by_path.items():
        manifest = project / location / "oxid.toml"
        metadata = tomllib.loads(manifest.read_text())["project"]
        entry = "src/main.ox" if location == "." else "src/lib.ox"
        wanted = {"id": ids[location], "path": location, "name": metadata["name"],
                  "version": metadata["version"], "edition": "typed-preview",
                  "entry": entry, "manifest": digest(manifest),
                  "sources": "root" if location == "." else
                  reference_source_checksum(project, location, entry)}
        require(row == wanted, "package metadata or retained source checksum mismatch: " + location)
    expected_edges = sorted(((ids["."], "counts", ids["packages/counts"]),
                             (ids["."], "report", ids["packages/report"]),
                             (ids["packages/report"], "counts", ids["packages/counts"])))
    require(lock["dependency"] == [{"owner": owner, "alias": alias, "package": package}
                                   for owner, alias, package in expected_edges],
            "workspace edges must retain the actual shared counts identity")
    return lock


def oracle_controls(project, valid):
    """A valid compiler lock must not make the independent oracle accept drift."""
    mutations = {}
    wrong = copy.deepcopy(valid)
    wrong["member"].pop()
    mutations["missing-unselected-member"] = wrong
    wrong = copy.deepcopy(valid)
    wrong["manifest"] = "0" * 64
    mutations["wrong-workspace-manifest"] = wrong
    for label, field, value in (("dependency-marked-editable", "sources", "root"),
                                ("wrong-source-bytes", "sources", "sha256:" + "0" * 64),
                                ("wrong-canonical-identity", "id", "0" * 64),
                                ("wrong-member-manifest", "manifest", "0" * 64)):
        wrong = copy.deepcopy(valid)
        next(row for row in wrong["package"] if row["path"] == "packages/counts")[field] = value
        mutations[label] = wrong
    wrong = copy.deepcopy(valid)
    wrong["dependency"] = [row for row in wrong["dependency"]
                           if row["owner"] != identity("packages/report")]
    mutations["missing-diamond-edge"] = wrong
    for name, mutation in mutations.items():
        try:
            inspect_lock(project, mutation)
        except (KeyError, TypeError, ValueError):
            continue
        raise ValueError("lock oracle accepted mutation: " + name)
    return sorted(mutations)


def records(result):
    require(not result.stderr, "JSON command unexpectedly wrote stderr")
    return [json.loads(line) for line in result.stdout.splitlines() if line.strip()]


def workspace_summary(rows, operation, total, failed=0):
    expected = {"schema_version": 1, "edition": "typed-preview", "kind": "workspace-summary",
                "operation": operation, "success": failed == 0, "total": total, "failed": failed}
    require(rows and rows[-1] == expected, operation + ": incorrect workspace summary")


class WorkspaceRunner(Runner):
    def command(self, *args, **kwargs):
        result = super().command(*args, **kwargs)
        for stream in ("stdin", "stdout", "stderr"):
            self.commands[-1][stream + "_bytes"] = (
                self.output / self.commands[-1]["name"] / (stream + ".bin")).stat().st_size
        save(self.output / self.commands[-1]["name"] / "command.json", self.commands[-1])
        return result

    def rejected(self, name, words, project, message=None, code=None):
        result = self.command(name, [self.compiler, *words, "--message-format=json"],
                              project, failure=True)
        rows = records(result)
        diagnostics = [row for row in rows if row.get("kind") == "diagnostic"
                       and row.get("severity") == "error"]
        require(result.returncode == 1 and diagnostics
                and any(row.get("kind", "").endswith("-summary")
                        and row.get("success") is False for row in rows),
                name + ": expected structured rejection")
        require(not any(row.get("kind") in ("workspace-member", "build-cache", "test-result")
                        or row.get("kind") == "compile-summary" and row.get("success")
                        for row in rows), name + ": rejection followed partial work")
        if message:
            require(any(message in row.get("message", "") for row in diagnostics),
                    name + ": wrong rejection reason")
        if code:
            require(any(row.get("code") == code for row in diagnostics), name + ": wrong diagnostic code")

    def retained_inputs(self, project, name):
        destination = self.output / (name + "-inputs")
        destination.mkdir()
        inventory = {}
        for path in sorted(project.rglob("*")):
            if not path.is_file() or ".oxid" in path.parts or path.name == "oxid.lock":
                continue
            relative = path.relative_to(project)
            retained = destination / relative
            retained.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, retained)
            inventory[relative.as_posix()] = {"sha256": digest(retained), "bytes": retained.stat().st_size}
        self.report.setdefault("inputs", {})[name] = inventory
        save(destination / "inventory.json", inventory)

    def retain_lock(self, project, name):
        raw = (project / "oxid.lock").read_bytes()
        parsed = inspect_lock(project, tomllib.loads(raw.decode()))
        destination = self.output / (name + "-oxid.lock")
        destination.write_bytes(raw)
        self.report.setdefault("locks", {})[name] = {"sha256": digest(destination), "bytes": len(raw),
                                                     "parsed": parsed}
        return raw, parsed

    def tests(self, name, project):
        result = self.command(name, [self.compiler, "test", "--workspace", "--frozen",
                                     "--message-format=json"], project)
        rows = records(result)
        workspace_summary(rows, "test", 1)
        require(len(rows) == 2 and rows[0] == {
            "schema_version": 1, "edition": "typed-preview", "kind": "test-result", "member": ".",
            "name": "counts", "success": True, "exit_status": 0, "stdout": "", "stderr": ""},
            "workspace tests did not execute the declared root test through its actual graph")

    def build(self, name, project, cache):
        result = self.command(name, [self.compiler, "build", "--workspace", "--frozen", "--incremental",
                                     "--message-format=json"], project)
        rows = records(result)
        workspace_summary(rows, "build", 3)
        members = [row for row in rows if row.get("kind") == "workspace-member"]
        require(members == [{"schema_version": 1, "edition": "typed-preview", "kind": "workspace-member",
                             "member": path, "status": "ok" if path == "." else "checked-library"}
                            for path in LOCATIONS], "workspace binary/library build statuses differ")
        cache_rows = [row for row in rows if row.get("kind") == "build-cache"]
        require(cache_rows == [{"schema_version": 1, "edition": "typed-preview", "kind": "build-cache",
                               "stage": "native-object", "status": cache}], "wrong native object reuse outcome")
        compiled = [row for row in rows if row.get("kind") == "compile-summary"]
        require(len(rows) == 8 and len(compiled) == 1 and compiled[0].get("success") is True
                and len([row for row in rows if row.get("kind") == "check-summary"
                         and row.get("success") is True]) == 2, "workspace did not compile one binary and check two libraries")
        for location in LOCATIONS[1:]:
            require(not (project / location / ".oxid").exists(), "library check created a native artifact/cache")
        executable = project / ".oxid/bin/byte-count"
        header = executable.read_bytes()[:64]
        require(len(header) == 64 and header[:7] == b"\x7fELF\x02\x01\x01"
                and header[16:20] == native_target.elf_identity(3), "native output must be a host ELF PIE")
        return executable

    def check(self, native):
        project = self.output / "workspace"
        shutil.copytree(APPLICATION, project, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
        with (project / "oxid.toml").open("a") as stream:
            stream.write(WORKSPACE)
        for location in LOCATIONS[1:]:
            manifest = project / location / "oxid.toml"
            text = manifest.read_text()
            require(text.count("[project]") == 1, "application manifest project table changed")
            manifest.write_text(text.replace("[project]", '[project]\nkind = "lib"', 1))
        self.retained_inputs(project, "initial")
        checked = self.command("plain-workspace-check", [self.compiler, "check", "--workspace",
                                                         "--message-format=json"], project)
        rows = records(checked)
        workspace_summary(rows, "check", 3)
        require(len([row for row in rows if row.get("kind") == "workspace-member"
                     and row.get("status") == "ok"]) == 3, "workspace check missed a member")
        lock = project / "oxid.lock"
        require(not lock.exists(), "ordinary workspace check created a lock")
        self.rejected("missing-shared-lock", ["check", "--locked", "-p", "."], project,
                      message="requires oxid.lock")
        self.command("create-complete-lock", [self.compiler, "lock", "--offline", "-p", "."], project)
        initial, parsed = self.retain_lock(project, "initial")
        self.report["rejected_oracle_mutations"] = oracle_controls(project, parsed)
        self.tests("frozen-workspace-tests", project)
        self.cases("default-interpreted", [self.compiler, "run", "--frozen"], project)
        self.command("selected-library", [self.compiler, "check", "-p", "packages/counts", "--frozen"], project)
        manifest = project / "packages/report/oxid.toml"
        original_manifest = manifest.read_bytes()
        manifest.write_bytes(b'[project]\nname = "unterminated\n')
        self.rejected("malformed-unselected-member", ["check", "-p", "packages/counts", "--frozen"], project,
                      message="invalid manifest value")
        require(lock.read_bytes() == initial, "malformed unselected member changed the shared lock")
        manifest.write_bytes(original_manifest)

        executable = project / ".oxid/bin/byte-count"
        if native:
            self.build("native-cold", project, "miss")
            self.report["cold_executable_sha256"] = digest(executable)
            self.build("native-warm", project, "hit")
            self.report["initial_executable_sha256"] = digest(executable)
            self.cases("native", [executable], project)
        source = project / "src/main.ox"
        original_source = source.read_bytes()
        source.write_bytes(original_source + b"\nfn broken(\n")
        self.rejected("invalid-source-before-build", ["build", "--workspace", "--frozen"], project, code="E0100")
        require(lock.read_bytes() == initial, "invalid source build changed the shared lock")
        if native:
            require(digest(executable) == self.report["initial_executable_sha256"], "invalid source build replaced output")
        else:
            require(not (project / ".oxid").exists(), "source rejection touched native output storage")
        source.write_bytes(original_source)

        dependency = project / "packages/counts/src/lib.ox"
        original = dependency.read_text()
        marker = "totals.lines = totals.lines + 1;"
        require(original.count(marker) == 1, "application dependency edit location changed")
        dependency.write_text(original.replace(marker, "totals.lines = totals.lines + 2;"))
        self.rejected("selected-library-cannot-hide-drift", ["check", "-p", "packages/counts", "--frozen"], project,
                      message="lock is out of date")
        self.rejected("stale-lock-before-build", ["build", "--workspace", "--frozen"], project,
                      message="lock is out of date")
        require(lock.read_bytes() == initial, "stale input rejection rewrote the shared lock")
        if native:
            require(digest(executable) == self.report["initial_executable_sha256"], "stale lock build replaced output")
        self.command("explicit-update", [self.compiler, "lock", "--update", "-p", "packages/counts"], project)
        updated, _ = self.retain_lock(project, "updated")
        require(updated != initial, "explicit update did not bind edited source")
        self.cases("updated-interpreted", [self.compiler, "run", "--frozen"], project, multiplier=2)
        # The root test stays editable under the shared dependency lock.
        test = project / "tests/counts.ox"
        text = test.read_text()
        require(text.count("line_count(&totals) != 2") == 3, "application test edit location changed")
        test.write_text(text.replace("line_count(&totals) != 2", "line_count(&totals) != 4"))
        self.tests("updated-workspace-tests", project)
        require(lock.read_bytes() == updated, "editing a declared root test changed the shared lock")
        self.retained_inputs(project, "updated")
        if native:
            self.build("native-updated", project, "miss")
            rebuilt = digest(executable)
            require(rebuilt != self.report["initial_executable_sha256"], "updated dependency did not change native output")
            self.report["updated_executable_sha256"] = rebuilt
            self.cases("updated-native", [executable], project, multiplier=2)
            deployment = self.output / "deployment"
            deployment.mkdir()
            deployed = deployment / "byte-count"
            shutil.copy2(executable, deployed)
            shutil.rmtree(project)
            require(not project.exists() and list(deployment.iterdir()) == [deployed],
                    "deployment must contain only the ELF after copied source removal")
            self.cases("deployed", [deployed], deployment, multiplier=2, env={"PATH": ""})
            require(digest(deployed) == rebuilt, "deployed executable changed")
            self.report["deployment"] = {"sha256": digest(deployed), "source_tree_removed": True,
                                         "path_empty": True, "cases": len(CASES)}


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
    controllers = {name: digest(ROOT / "scripts" / name) for name in
                   (Path(__file__).name, "verify_typed_package_app.py", "verify_typed_package_lock.py")}
    report = {"schema": "oxid-typed-workspace-application-v1", "status": "running",
              "scope": "R1 real workspace regression; not v1 acceptance or native reproducibility",
              "native": args.native, "source_sha256": sources, "controllers_sha256": controllers,
              "cases_per_execution": len(CASES), "compilers": []}
    try:
        for index, compiler in enumerate(compilers):
            directory = output / f"compiler-{index}"
            directory.mkdir()
            retained = directory / "oxid"
            shutil.copy2(compiler, retained)
            row = {"path": str(compiler), "retained": str(retained), "sha256": digest(retained),
                   "profile": compiler.parent.name, "status": "running"}
            require(digest(compiler) == row["sha256"], "compiler changed while retaining it")
            report["compilers"].append(row)
            WorkspaceRunner(directory, retained, row).check(args.native)
            require(digest(retained) == row["sha256"], "retained compiler changed during verification")
            row["status"] = "passed"
        require(all(digest(APPLICATION / name) == sha for name, sha in sources.items()),
                "repository application changed during verification")
        require(all(digest(ROOT / "scripts" / name) == sha for name, sha in controllers.items()),
                "oracle implementation changed during verification")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "native": args.native,
                          "cases_per_execution": len(CASES), "output": str(output)}))
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
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
