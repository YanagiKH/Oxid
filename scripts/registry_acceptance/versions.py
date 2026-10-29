#!/usr/bin/env python3
"""Verify exact-version registry identity and rollback through the public CLI.

This acceptance helper has no compiler substitute. It publishes real
Oxid libraries and keeps every fixture, command transcript and failure. Expected
values are independent constants; lock files use the standard TOML decoder.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import time
import tomllib

MAX_STREAM = 8 * 1024 * 1024
TIMEOUT = 90


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value.encode("utf-8") if isinstance(value, str) else value)


def inventory(root):
    """Record real regular files without following links or reading outside root."""
    require(root.is_dir() and not root.is_symlink(), "inventory root must be an ordinary directory")
    files = {}
    total = 0
    for directory, directories, names in os.walk(root, followlinks=False):
        for name in directories:
            require(not Path(directory, name).is_symlink(), "unexpected directory link in fixture")
        for name in names:
            path = Path(directory, name)
            mode = path.lstat().st_mode
            require(stat.S_ISREG(mode), "unexpected nonregular fixture input: " + str(path))
            total += path.stat().st_size
            require(total <= 64 * 1024 * 1024 and len(files) < 4096, "fixture inventory exceeded its bounds")
            files[str(path.relative_to(root))] = {"bytes": path.stat().st_size, "sha256": sha(path), "mode": stat.S_IMODE(mode)}
    return files


def preserve_files(root, previous):
    for relative, expected in previous.items():
        path = root / relative
        require(path.is_file() and not path.is_symlink(), "previous cache file disappeared or became a link: " + relative)
        actual = {"bytes": path.stat().st_size, "sha256": sha(path), "mode": stat.S_IMODE(path.stat().st_mode)}
        require(actual == expected, "previous immutable cache file changed: " + relative)


def library_manifest(version):
    return ('[project]\nname = "numbers"\nversion = "' + version
            + '"\nedition = "typed-preview"\nkind = "lib"\nentry = "src/lib.ox"\n')


def library_source(value):
    return ('pub struct Token { pub value: i32 }\n'
            'pub fn make() -> Token { return Token { value: ' + str(value) + ' }; }\n'
            'pub fn read(value: &Token) -> i32 { return value.value; }\n')


def dependency(source, version):
    return '{ registry = "' + source + '", package = "numbers", version = "' + version + '" }'


def app_manifest(name, dependencies):
    return ('[project]\nname = "' + name + '"\nversion = "0.1.0"\nedition = "typed-preview"\nentry = "src/main.ox"\n'
            '[build]\nentry-mode = "result"\n[execution]\npolicy = "hosted"\n[dependencies]\n'
            + ''.join(alias + ' = ' + value + '\n' for alias, value in dependencies.items()))


MULTI_SOURCE = '''fn main() -> i32 {
    let original = crate::old::make();
    let updated = crate::upgraded::make();
    let elsewhere = crate::foreign::make();
    let first = crate::old_alias::read(&original);
    let second = crate::upgraded::read(&updated);
    let third = crate::foreign::read(&elsewhere);
    if first != 41 { return -1; }
    if second != 42 { return -2; }
    if third != 41 { return -3; }
    return first + second + third;
}
'''
UPGRADE_SOURCE = '''fn main() -> i32 {
    let token = crate::numbers::make();
    return crate::numbers::read(&token);
}
'''


class Run:
    def __init__(self, compiler, output, report):
        self.compiler, self.output, self.report = compiler, output, report
        self.commands = report["commands"] = []

    def command(self, name, args, cwd, expected_exit=0, expected_stdout=None):
        directory = self.output / "commands" / name
        directory.mkdir(parents=True)
        argv = [str(self.compiler), *map(str, args)]
        before = {str(p.relative_to(cwd)): sha(p) for p in [cwd / "oxid.toml", cwd / "src/main.ox", cwd / "src/lib.ox", cwd / "oxid.lock"] if p.is_file()}
        row = {"name": name, "argv": argv, "cwd": str(cwd), "expected_exit": expected_exit,
               "input_sha256_before": before, "status": "running"}
        self.commands.append(row)
        save(self.output / "result.json", self.report)
        started = time.monotonic()
        proc = None
        try:
            with (directory / "stdout.bin").open("wb") as stdout, (directory / "stderr.bin").open("wb") as stderr:
                proc = subprocess.Popen(argv, cwd=cwd, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr, start_new_session=True)
                reason = None
                while proc.poll() is None:
                    if time.monotonic() - started > TIMEOUT:
                        reason = "command deadline exceeded"
                    elif stdout.tell() > MAX_STREAM or stderr.tell() > MAX_STREAM:
                        reason = "command transcript exceeded its allowance"
                    if reason:
                        os.killpg(proc.pid, signal.SIGTERM)
                        try:
                            proc.wait(timeout=2)
                        except subprocess.TimeoutExpired:
                            os.killpg(proc.pid, signal.SIGKILL)
                            proc.wait(timeout=2)
                        raise ValueError(name + ": " + reason)
                    time.sleep(0.02)
                code = proc.wait()
            output = (directory / "stdout.bin").read_bytes()
            error = (directory / "stderr.bin").read_bytes()
            require(len(output) <= MAX_STREAM and len(error) <= MAX_STREAM, name + ": oversized command output")
            row.update(exit_code=code, stdout_sha256=sha(directory / "stdout.bin"), stderr_sha256=sha(directory / "stderr.bin"),
                       stdout_bytes=len(output), stderr_bytes=len(error))
            require(code == expected_exit, name + ": unexpected exit; inspect retained raw streams")
            require(not error, name + ": unexpected stderr; inspect retained raw stream")
            require(expected_stdout is None or output == expected_stdout, name + ": independent stdout mismatch")
            row["status"] = "passed"
            return output
        except BaseException as error:
            row.update(status="failed", error=repr(error))
            if proc is not None and proc.poll() is None:
                os.killpg(proc.pid, signal.SIGKILL)
                proc.wait(timeout=2)
            raise
        finally:
            row["seconds"] = time.monotonic() - started
            row["input_sha256_after"] = {str(p.relative_to(cwd)): sha(p) for p in [cwd / "oxid.toml", cwd / "src/main.ox", cwd / "src/lib.ox", cwd / "oxid.lock"] if p.is_file()}
            for stream in ["stdout", "stderr"]:
                path = directory / (stream + ".bin")
                if path.exists():
                    row[stream + "_bytes"] = path.stat().st_size
                    row[stream + "_sha256"] = sha(path)
            save(directory / "command.json", row)
            save(self.output / "result.json", self.report)

    def check(self, name, app, expected_exit=0):
        output = self.command(name, ["check", "--message-format=json"], app, expected_exit)
        rows = [json.loads(line) for line in output.splitlines() if line.strip()]
        summaries = [row for row in rows if row.get("kind") == "check-summary"]
        require(len(summaries) == 1 and summaries[0].get("success") is (expected_exit == 0), name + ": missing or inconsistent check summary")
        diagnostics = [row for row in rows if row.get("kind") == "diagnostic"]
        if expected_exit:
            require(diagnostics and all(row.get("severity") == "error" for row in diagnostics), name + ": missing static error")
            allowed_stages = {"type", "generics", "ownership", "borrow", "declarations"}
            require(any(str(row.get("code", "")).startswith(("E03", "E05")) and row.get("stage") in allowed_stages for row in diagnostics),
                    name + ": expected static nominal-type rejection, not a missing source/tool/project failure")
        else:
            require(not diagnostics, name + ": unexpected diagnostics")
        return rows


def lock_data(app):
    value = tomllib.loads((app / "oxid.lock").read_text(encoding="utf-8"))
    require(value.get("version") == 4 and value.get("kind") == "typed-registry-project-v1", "expected a registry v4 project lock")
    return value


def pins(value):
    rows = [row for row in value.get("package", []) if row.get("registry")]
    result = {}
    for row in rows:
        key = (row["registry"], row["name"], row["version"])
        require(key not in result, "duplicate registry tuple in lock")
        require(re.fullmatch(r"[a-f0-9]{64}", row["id"]) is not None, "invalid registry package ID")
        result[key] = row
    require(len({row["id"] for row in rows}) == len(rows), "distinct registry tuples share an ID")
    return result


def verify(args):
    compiler = args.compiler.resolve(strict=True)
    require(compiler.is_file() and os.access(compiler, os.X_OK), "compiler must be an executable file")
    output = args.output.absolute()
    require(not output.exists() and not output.is_symlink(), "output must be a fresh directory")
    output.mkdir(parents=True)
    report = {"schema": "oxid-private-registry-version-identity-v1", "status": "running",
              "scope": "exact version, alias and source nominal identity; explicit upgrade and byte-exact rollback",
              "compiler": {"path": str(compiler), "sha256": sha(compiler)}, "controller_sha256": sha(Path(__file__)),
              "commands": []}
    runner = Run(compiler, output, report)
    try:
        registries = output / "registries"
        registries.mkdir()
        first, second = registries / "first", registries / "second"
        runner.command("init-first", ["registry", "init", first], output)
        runner.command("init-second", ["registry", "init", second], output)
        uri_first, uri_second = first.as_uri(), second.as_uri()
        authors = output / "authors"
        for version, number in [("1.2.0", 41), ("1.3.0", 42)]:
            author = authors / version
            write(author / "oxid.toml", library_manifest(version))
            write(author / "src/lib.ox", library_source(number))
            runner.command("author-lock-" + version, ["lock"], author)
            runner.command("publish-first-" + version, ["publish", "--registry", uri_first], author)
            if version == "1.2.0":
                runner.command("publish-second-1.2.0", ["publish", "--registry", uri_second], author)
        a = first / "packages/numbers/1.2.0/artifact.tar"
        b = second / "packages/numbers/1.2.0/artifact.tar"
        require(a.is_file() and b.is_file() and a.stat().st_size == b.stat().st_size and sha(a) == sha(b),
                "same exact library source/version must yield identical artifacts in the two registries")
        # Compare actual bytes, in addition to independent streaming SHA-256.
        require(a.read_bytes() == b.read_bytes(), "registry artifact bytes differ")
        report["published_artifacts"] = {"first_1_2": {"sha256": sha(a), "bytes": a.stat().st_size},
                                          "second_1_2": {"sha256": sha(b), "bytes": b.stat().st_size},
                                          "first_1_3": {"sha256": sha(first / "packages/numbers/1.3.0/artifact.tar")}}
        report["publisher_sources"] = inventory(authors)
        app = output / "identity-app"
        write(app / "oxid.toml", app_manifest("registry-identity", {
            "old": dependency(uri_first, "1.2.0"), "old_alias": dependency(uri_first, "1.2.0"),
            "upgraded": dependency(uri_first, "1.3.0"), "foreign": dependency(uri_second, "1.2.0")}))
        write(app / "src/main.ox", MULTI_SOURCE)
        runner.check("identity-check", app)
        runner.command("identity-lock", ["lock"], app)
        runner.command("identity-run", ["run", "--frozen"], app, expected_stdout=b"124\n")
        original_lock = (app / "oxid.lock").read_bytes()
        original_manifest = (app / "oxid.toml").read_bytes()
        value = lock_data(app)
        actual = pins(value)
        wanted = {(uri_first, "numbers", "1.2.0"), (uri_first, "numbers", "1.3.0"), (uri_second, "numbers", "1.2.0")}
        require(set(actual) == wanted, "v4 lock must contain exactly the three requested registry identities")
        require(len(value["package"]) == 4, "identity graph must contain exactly its root and three registry packages")
        edges = {row["alias"]: row for row in value.get("dependency", []) if row["owner"] == value["root"]}
        require(set(edges) == {"old", "old_alias", "upgraded", "foreign"} and len(value["dependency"]) == 4, "unexpected identity graph edges")
        old_id = actual[(uri_first, "numbers", "1.2.0")]["id"]
        require(edges["old"]["package"] == edges["old_alias"]["package"] == old_id, "aliases of one exact tuple must share nominal identity")
        require(edges["upgraded"]["package"] == actual[(uri_first, "numbers", "1.3.0")]["id"], "version edge differs from requested tuple")
        require(edges["foreign"]["package"] == actual[(uri_second, "numbers", "1.2.0")]["id"], "source edge differs from requested tuple")
        report["identity_lock"] = value
        cache = app / ".oxid/registry"
        cache_before = inventory(cache)
        report["identity_cache_before"] = cache_before
        for other in ["upgraded", "foreign"]:
            source = ('fn main() -> i32 { let original = crate::old::make(); return crate::' + other + '::read(&original); }\n')
            write(app / "src/main.ox", source)
            try:
                rows = runner.check("reject-cross-" + other, app, expected_exit=1)
                report.setdefault("nominal_rejections", {})[other] = rows
                require((app / "oxid.lock").read_bytes() == original_lock and (app / "oxid.toml").read_bytes() == original_manifest,
                        "static nominal rejection changed the existing lock or manifest")
                require(inventory(cache) == cache_before, "static nominal rejection changed immutable registry cache")
            finally:
                write(app / "src/main.ox", MULTI_SOURCE)
        upgrade = output / "upgrade-app"
        old_manifest = app_manifest("registry-upgrade", {"numbers": dependency(uri_first, "1.2.0")}).encode()
        write(upgrade / "oxid.toml", old_manifest)
        write(upgrade / "src/main.ox", UPGRADE_SOURCE)
        runner.command("upgrade-initial-lock", ["lock"], upgrade)
        runner.command("upgrade-initial-run", ["run", "--frozen"], upgrade, expected_stdout=b"41\n")
        old_lock = (upgrade / "oxid.lock").read_bytes()
        old_data = lock_data(upgrade)
        require(set(pins(old_data)) == {(uri_first, "numbers", "1.2.0")}, "initial upgrade lock must pin 1.2.0")
        old_cache = inventory(upgrade / ".oxid/registry")
        write(output / "retained/upgrade-1.2.0-oxid.toml", old_manifest)
        write(output / "retained/upgrade-1.2.0-oxid.lock", old_lock)
        write(upgrade / "oxid.toml", app_manifest("registry-upgrade", {"numbers": dependency(uri_first, "1.3.0")}))
        runner.command("upgrade-lock-update", ["lock", "--update"], upgrade)
        new_lock = (upgrade / "oxid.lock").read_bytes()
        require(new_lock != old_lock, "explicit version upgrade did not update lock bytes")
        new_data = lock_data(upgrade)
        require(set(pins(new_data)) == {(uri_first, "numbers", "1.3.0")}, "updated lock must pin only reachable version 1.3.0")
        preserve_files(upgrade / ".oxid/registry", old_cache)
        runner.command("upgrade-new-run", ["run", "--frozen"], upgrade, expected_stdout=b"42\n")
        unavailable = []
        try:
            for registry in [first, second]:
                moved = registry.with_name(registry.name + "-unavailable")
                registry.rename(moved)
                unavailable.append((registry, moved))
            runner.command("identity-registry-unavailable-run", ["run", "--frozen"], app, expected_stdout=b"124\n")
            runner.command("upgrade-registry-unavailable-run", ["run", "--frozen"], upgrade, expected_stdout=b"42\n")
            write(upgrade / "oxid.toml", old_manifest)
            write(upgrade / "oxid.lock", old_lock)
            runner.command("rollback-registry-unavailable-run", ["run", "--frozen"], upgrade, expected_stdout=b"41\n")
            require((upgrade / "oxid.toml").read_bytes() == old_manifest and (upgrade / "oxid.lock").read_bytes() == old_lock,
                    "rollback failed to preserve exact original manifest and lock bytes")
            preserve_files(upgrade / ".oxid/registry", old_cache)
        finally:
            for registry, moved in reversed(unavailable):
                moved.rename(registry)
        require(inventory(cache) == cache_before, "identity application's cached releases changed")
        require(inventory(authors) == report["publisher_sources"], "publisher source/lock bytes changed after publication")
        require(sha(compiler) == report["compiler"]["sha256"], "compiler changed during verification")
        report["upgrade_and_rollback"] = {"initial_lock": old_data, "upgraded_lock": new_data,
                                             "initial_lock_sha256": hashlib.sha256(old_lock).hexdigest(),
                                             "upgraded_lock_sha256": hashlib.sha256(new_lock).hexdigest(),
                                             "original_manifest_and_lock_restored_exactly": True,
                                             "old_cached_files_preserved": True, "registry_roots_absent_during_final_runs": True}
        report["final_sources"] = {"identity_app": {"manifest": sha(app / "oxid.toml"), "main": sha(app / "src/main.ox")},
                                     "upgrade_app": {"manifest": sha(upgrade / "oxid.toml"), "main": sha(upgrade / "src/main.ox")}}
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "commands": len(runner.commands), "registry_identities": 3,
                          "independent_result": 124, "upgrade_result": 42, "rollback_result": 41}))
        return 0
    except BaseException as error:
        report.update(status="failed", error=repr(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        return verify(args)
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
