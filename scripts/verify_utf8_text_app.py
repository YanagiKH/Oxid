#!/usr/bin/env python3
"""Compare the UTF-8 library application with Python's strict Unicode decoder.

Retain exact inputs, outputs, commands, locks and raw executables. Native mode
checks both optimization profiles, object reuse, relocation and execution after
removing all application/library sources. These are scoped application checks.
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

from verify_owned_enum_app import Oracle, ROOT, MAX_CAPTURE, digest, require, save, tree_hashes
from verify_typed_package_lock import framed, reference_source_checksum

APP = Path("apps/utf8-normalize")
LIBRARY = Path("packages/utf8_text")
LOCATIONS = ("app", "contracts", "../../packages/utf8_text")
CONTROLLERS = ("verify_utf8_text_app.py", "verify_owned_enum_app.py",
               "verify_typed_package_app.py", "verify_typed_package_lock.py")
LONG_INPUT = b"a\r\nb\r\x00" * 2049 + "\U0001f44b".encode()


def expected(payload):
    try:
        text = payload.decode("utf-8", errors="strict")
    except UnicodeDecodeError:
        return 65, b""
    return 0, text.replace("\r\n", "\n").replace("\r", "\n").encode("utf-8")


def cases():
    edges = (0, 1, 127, 128, 2047, 2048, 55295, 57344, 65535, 65536, 1114111)
    encoded = [chr(value).encode() for value in edges]
    rows = [("empty", b""), ("ascii", bytes(range(128))),
            ("scalar-edges", b"".join(encoded)),
            ("line-endings", b"\r\r\n\n\r\x00a\r\nb\rc\n\r"),
            ("bom-combining", "\ufeffe\u0301\u00e9\u200d\U0001f44b\ufe0f".encode()),
            ("noncharacters", "".join(chr(plane * 65536 + tail)
                                      for plane in range(17) for tail in (65534, 65535)).encode()),
            ("long-mixed", LONG_INPUT)]
    rows += [("octet-" + str(value), bytes((value,))) for value in range(256)]
    for size in (1023, 1024, 1025, 4095, 4096, 4097, 8199, 65536):
        rows.append(("ascii-length-" + str(size), b"A" * size))
    for offset in (1021, 1022, 1023, 1024, 2047, 8191):
        rows.append(("split-scalar-" + str(offset), b"A" * offset + b"".join(encoded)))
        rows.append(("split-crlf-" + str(offset), b"A" * offset + b"\r\nZ"))
    for size in (8199, 65536):
        rows.append(("late-invalid-" + str(size), b"A\r\n" + b"a" * size + b"\xed\xa0\x80"))
    for number, raw in enumerate(encoded):
        if len(raw) < 2:
            continue
        for end in range(1, len(raw)):
            rows.append((f"truncated-{number}-{end}", b"A" + raw[:end]))
        for offset in range(len(raw)):
            for value in (0, 127, 128, 191, 192, 194, 223, 224, 237, 239, 240, 244, 245, 255):
                changed = raw[:offset] + bytes((value,)) + raw[offset + 1:]
                rows.append((f"mutated-{number}-{offset}-{value}", b"A" + changed + b"\r\n"))
    unique = {}
    for label, payload in rows:
        unique.setdefault(payload, label)
    return tuple((label, payload) for payload, label in unique.items())


CASES = cases()


def unicode_chunks():
    """Cover every Unicode scalar once, without splitting an encoded scalar."""
    for start in range(0, 0x110000, 16384):
        payload = "".join(chr(value) for value in range(start, min(start + 16384, 0x110000))
                          if not 0xD800 <= value <= 0xDFFF).encode("utf-8")
        if payload:
            yield "all-scalars-" + format(start, "x"), payload


def identity(location):
    return hashlib.sha256(framed(b"oxid-typed-local-package-v1") + framed(location.encode())).hexdigest()


def inspect_lock(project, lock):
    require(set(lock) == {"version", "kind", "manifest", "member", "package", "dependency"}
            and lock["version"] == 3 and lock["kind"] == "typed-workspace-v1", "wrong lock schema")
    require(lock["manifest"] == digest(project / "oxid.toml"), "workspace manifest hash differs")
    ids = {location: identity(location) for location in LOCATIONS}
    require(lock["member"] == [{"package": value} for value in sorted(ids[p] for p in LOCATIONS[:2])],
            "workspace member roster differs")
    require(len(lock["package"]) == 3 and [row["id"] for row in lock["package"]] == sorted(ids.values()),
            "package identity/order differs")
    by_path = {row["path"]: row for row in lock["package"]}
    require(set(by_path) == set(LOCATIONS), "package path roster differs")
    for location in LOCATIONS:
        manifest = project / location / "oxid.toml"
        metadata = tomllib.loads(manifest.read_text())["project"]
        entry = "src/lib.ox" if location == LOCATIONS[2] else "src/main.ox"
        require(by_path[location] == {
            "id": ids[location], "path": location, "name": metadata["name"],
            "version": metadata["version"], "edition": "typed-preview", "entry": entry,
            "manifest": digest(manifest), "sources": reference_source_checksum(project, location, entry)
            if location == LOCATIONS[2] else "root"}, "package/source identity differs: " + location)
    edges = sorted((ids[member], "text", ids[LOCATIONS[2]]) for member in LOCATIONS[:2])
    require(lock["dependency"] == [{"owner": owner, "alias": alias, "package": package}
                                   for owner, alias, package in edges], "shared library identity differs")
    return lock


def lock_controls(project, valid):
    mutations = {}
    for name, field, value in (("root", "sources", "root"), ("checksum", "sources", "sha256:" + "0" * 64),
                                ("identity", "id", "0" * 64), ("manifest", "manifest", "0" * 64)):
        changed = copy.deepcopy(valid)
        next(row for row in changed["package"] if row["path"] == LOCATIONS[2])[field] = value
        mutations[name] = changed
    changed = copy.deepcopy(valid)
    changed["member"].append({"package": identity(LOCATIONS[2])})
    mutations["library-is-not-workspace-member"] = changed
    changed = copy.deepcopy(valid)
    changed["dependency"].pop()
    mutations["missing-shared-edge"] = changed
    for name, changed in mutations.items():
        try:
            inspect_lock(project, changed)
        except (ValueError, KeyError, TypeError):
            continue
        raise ValueError("lock oracle accepted mutation: " + name)
    return sorted(mutations)


def records(result):
    require(not result.stderr, "JSON command wrote stderr")
    return [json.loads(line) for line in result.stdout.splitlines()]


def summary(rows, operation, total):
    require(rows and rows[-1] == {"schema_version": 1, "edition": "typed-preview",
            "kind": "workspace-summary", "operation": operation, "success": True,
            "total": total, "failed": 0}, "incorrect workspace summary")


class TextOracle(Oracle):
    def input_capture_limit(self):
        return MAX_CAPTURE + 1

    def no_tools(self):
        return {"OXID_LLVM_BIN": str(self.spy), "OXID_APP_SPY_MARKER": str(self.marker)}

    def cases(self, label, argv, cwd, overrides=None, complete_unicode=False):
        indexes = []
        for name, payload in CASES:
            status, output = expected(payload)
            _, index = self.command(label + "-" + name, argv, cwd, payload, status, output, overrides=overrides)
            indexes.append(index)
        if complete_unicode:
            for name, payload in unicode_chunks():
                status, output = expected(payload)
                require(status == 0, "Unicode scalar corpus construction failed")
                _, index = self.command(label + "-" + name, argv, cwd, payload, 0, output, overrides=overrides)
                indexes.append(index)
            # Ownership transfer permits the largest current growth boundary
            # without allocating a second copy of the validated text.
            boundary = b"A" * (8 * 1024 * 1024)
            _, index = self.command(label + "-no-copy-boundary", argv, cwd, boundary,
                                    0, boundary, overrides=overrides)
            indexes.append(index)
            # Growing beyond that capacity still needs staged replacement.
            _, index = self.command(label + "-memory-refusal", argv, cwd, boundary + b"A",
                                    71, b"", overrides=overrides)
            indexes.append(index)
        _, index = self.command(label + "-directory-input", argv, cwd, status=74, stdout=b"",
                                overrides=overrides, directory_stdin=True)
        indexes.append(index)
        if len(argv) == 1:
            _, index = self.command(label + "-closed-input", argv, cwd, status=74, stdout=b"",
                                    overrides=overrides, closed_stdin=True)
            indexes.append(index)
        for name, payload, status in (("full-output", LONG_INPUT, 74), ("empty-full-output", b"", 0),
                                       ("invalid-full-output", b"hello\n\xff", 65)):
            _, index = self.command(label + "-" + name, argv, cwd, payload, status, b"",
                                    overrides=overrides, full_stdout=True)
            indexes.append(index)
        indexes.append(self.partial_write(label, argv, cwd, overrides))
        return indexes

    def partial_write(self, label, argv, cwd, overrides):
        folder = self.output / "commands" / f"{len(self.commands):04d}-{label}-partial-output"
        folder.mkdir()
        (folder / "stdin.bin").write_bytes(LONG_INPUT)
        row = {"label": label + "-partial-output", "argv": [str(p) for p in argv], "cwd": str(cwd),
               "environment_overrides": overrides or {}, "receipt": str(folder), "status": "running",
               "expected_status": 74, "stdout_sink": "nonblocking-pipe", "pipe_capacity": 4096,
               "expected_stdout_sha256": hashlib.sha256(expected(LONG_INPUT)[1][:4096]).hexdigest(),
               "expected_stderr_sha256": hashlib.sha256(b"").hexdigest()}
        self.commands.append(row)
        reader, writer = os.pipe()
        started = time.monotonic()
        try:
            require(fcntl.fcntl(writer, fcntl.F_SETPIPE_SZ, 4096) == 4096, "requires a 4096-byte pipe")
            os.set_blocking(writer, False)
            with (folder / "stdin.bin").open("rb") as source, (folder / "stderr.bin").open("wb") as errors:
                result = subprocess.run(row["argv"], cwd=cwd, env={**self.environment, **(overrides or {})},
                                        stdin=source, stdout=writer, stderr=errors, timeout=120)
            os.close(writer)
            writer = None
            payload = os.read(reader, 8192)
            require(os.read(reader, 1) == b"", "unexpected pipe tail")
            (folder / "stdout.bin").write_bytes(payload)
            row["exit_status"] = result.returncode
            for stream in ("stdin", "stdout", "stderr"):
                path = folder / (stream + ".bin")
                row[stream + "_bytes"], row[stream + "_sha256"] = path.stat().st_size, digest(path)
            require(result.returncode == 74 and payload == expected(LONG_INPUT)[1][:4096]
                    and not (folder / "stderr.bin").read_bytes(), "partial output differs")
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

    def lock(self, compiler, project, label):
        self.command(label + "-lock", [compiler, "lock", "--offline", "--message-format=json"],
                     project, overrides=self.no_tools())
        parsed = inspect_lock(project, tomllib.loads((project / "oxid.lock").read_text()))
        retained = self.output / (label + "-oxid.lock")
        shutil.copyfile(project / "oxid.lock", retained)
        self.report["locks"].append({"retained": str(retained), "sha256": digest(retained),
                                     "parsed": parsed, "rejected_mutations": lock_controls(project, parsed)})

    def build(self, compiler, project, profile, label, cache=None, member="app"):
        argv = [compiler, "build", "-p", member, "--frozen", "--reproducible",
                "--profile=" + profile, "--message-format=json"]
        if cache is not None:
            argv.append("--incremental")
        result, index = self.command(label, argv, project)
        rows = records(result)
        summary(rows, "build", 1)
        require([row for row in rows if row.get("kind") == "workspace-member"] == [{
            "schema_version": 1, "edition": "typed-preview", "kind": "workspace-member",
            "member": member, "status": "ok"}], "build selected wrong member")
        require(len([row for row in rows if row.get("kind") == "compile-summary"
                     and row.get("success") is True]) == 1, "expected one successful native link")
        require([row for row in rows if row.get("kind") == "build-cache"] == ([] if cache is None else [{
            "schema_version": 1, "edition": "typed-preview", "kind": "build-cache",
            "stage": "native-object", "status": cache}]), "cache outcome differs")
        binary_name = "utf8-normalize" if member == "app" else "utf8-text-contracts"
        artifact = self.output / "artifacts" / label
        shutil.copyfile(project / member / ".oxid/bin" / binary_name, artifact)
        artifact.chmod(0o555)
        body = artifact.read_bytes()
        require(body[:7] == b"\x7fELF\x02\x01\x01" and body[16:20] == native_target.elf_identity(3),
                "expected a host PIE ELF")
        require(str(project).encode() not in body and str(self.output).encode() not in body,
                "reproducible output contains an absolute project path")
        self.report["builds"].append({"label": label, "member": member, "profile": profile,
                                       "optimization": "O0" if profile == "dev" else "O2",
                                       "command": index, "cache": cache, "artifact": str(artifact),
                                       "sha256": digest(artifact), "bytes": len(body)})
        return artifact

    def controls(self, compiler, project, label, native):
        main = project / "app/src/main.ox"
        library = project / LOCATIONS[2] / "src/lib.ox"
        originals = {path: path.read_bytes() for path in (main, library)}
        locked = (project / "oxid.lock").read_bytes()
        before = tree_hashes(project / "app/.oxid")
        probes = (
            ("moved-text", "use crate::text::new_text;use crate::text::byte_len;"
             "fn main()->i32{let t=new_text();let moved=t;return byte_len(&t);}", "E0310", None),
            ("moved-input", "use alloc::bytes::new;use alloc::bytes::len;use crate::text::from_bytes;"
             "fn main()->i32{let b=new();let moved=from_bytes(b);return len(&b);}", "E0310", None),
            ("moved-into-bytes", "use crate::text::new_text;use crate::text::into_bytes;"
             "use crate::text::byte_len;fn main()->i32{let t=new_text();let bytes=into_bytes(t);"
             "return byte_len(&t);}", "E0310", None),
            ("private-owner", "use crate::text::new_text;fn main()->i32{let t=new_text();"
             "let b=t.bytes;return 0;}", None, "private"),
            ("stale-library", None, "E0001", "lock is out of date"),
        )
        try:
            for name, source, code, message in probes:
                main.write_bytes(originals[main])
                library.write_bytes(originals[library])
                if source is None:
                    library.write_bytes(originals[library] + b"\n// Changed dependency bytes.\n")
                else:
                    main.write_text(source)
                argv = [compiler, "build" if native else "check", "-p", "app", "--frozen",
                        "--message-format=json"]
                if native:
                    argv.append("--profile=release")
                result, index = self.command(label + "-" + name, argv, project, status=1,
                                             overrides=self.no_tools())
                rows = records(result)
                require(any(row.get("kind") == "diagnostic" and (code is None or row.get("code") == code)
                            and (message is None or message in row.get("message", "")) for row in rows),
                        "incorrect ownership/privacy/lock refusal: " + name)
                require(not any(row.get("kind") in ("build-cache", "test-result")
                                or row.get("kind") == "compile-summary" and row.get("success") for row in rows),
                        "refusal followed native work")
                require(not self.marker.exists() and tree_hashes(project / "app/.oxid") == before
                        and (project / "oxid.lock").read_bytes() == locked, "refusal changed outputs or invoked tools")
                self.report["controls"].append({"name": name, "command": index, "native": native,
                                                "preserved_output_files": before})
        finally:
            for path, raw in originals.items():
                path.write_bytes(raw)


def verify(args):
    require(native_target.host() is not None, "requires Linux x86_64 or ARM64")
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink() and ROOT not in output.parents and output != ROOT,
            "output must be a new directory outside the repository")
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(compilers and len(compilers) == len(set(compilers)) and all(os.access(p, os.X_OK) for p in compilers),
            "invalid compiler paths")
    require(args.reference_only or args.llvm_bin is not None, "native mode requires --llvm-bin")
    llvm = None if args.reference_only else args.llvm_bin.resolve(strict=True)
    output.mkdir(parents=True)
    environment = {"PATH": (str(llvm) + ":" if llvm else "") + "/usr/bin:/bin", "LANG": "C.UTF-8",
                   "LC_ALL": "C.UTF-8", "TZ": "UTC", "SOURCE_DATE_EPOCH": "0", "TMPDIR": str(output / "scratch")}
    if llvm:
        environment["OXID_LLVM_BIN"] = str(llvm)
    if "LD_LIBRARY_PATH" in os.environ:
        environment["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    report = {"schema": "oxid-utf8-text-application-v1", "status": "running",
              "reference_only": args.reference_only, "scope": "Strict UTF-8, newline conversion and real fd errors",
              "core_cases": len(CASES), "full_v1_qualification": False, "environment": environment,
              "sources": {str(path): tree_hashes(ROOT / path) for path in (APP, LIBRARY)},
              "controllers_sha256": {name: digest(ROOT / "scripts" / name) for name in CONTROLLERS},
              "compilers": [], "tools": [], "locks": [], "builds": [], "controls": [], "deployments": []}
    oracle = TextOracle(output, {**os.environ, **environment}, report)
    baseline = {}
    try:
        if llvm:
            for name in ("clang", "opt", "ld.lld"):
                tool = (llvm / name).resolve(strict=True)
                identity_before = digest(tool)
                result, index = oracle.command("version-" + name, [llvm / name, "--version"], output)
                with tool.open("rb") as stream:
                    require(stream.read(4) == b"\x7fELF", "native cache checks need original ELF tools")
                require(b"19.1.7" in result.stdout and digest(tool) == identity_before, "wrong LLVM tool/version")
                report["tools"].append({"name": name, "path": str(tool), "sha256": identity_before, "command": index})
        for number, compiler in enumerate(compilers):
            prefix = "compiler-" + str(number)
            retained = output / "compilers" / prefix
            compiler_hash = digest(compiler)
            shutil.copyfile(compiler, retained)
            retained.chmod(0o555)
            require(digest(retained) == compiler_hash == digest(compiler), "compiler changed during retention")
            report["compilers"].append({"source": str(compiler), "retained": str(retained), "sha256": compiler_hash})
            first_root, moved_root = output / (prefix + "-source"), output / (prefix + "-longer-移動後")
            for path in (APP, LIBRARY):
                shutil.copytree(ROOT / path, first_root / path, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
            project = first_root / APP
            oracle.lock(retained, project, prefix)
            result, _ = oracle.command(prefix + "-check", [retained, "check", "--workspace", "--frozen",
                                       "--message-format=json"], project, overrides=oracle.no_tools())
            summary(records(result), "check", 2)
            result, _ = oracle.command(prefix + "-contracts", [retained, "test", "--frozen", "--message-format=json"],
                                       project / "contracts", overrides=oracle.no_tools())
            rows = records(result)
            summary(rows, "test", 1)
            require(len(rows) == 2 and rows[0].get("kind") == "test-result" and rows[0].get("success") is True
                    and rows[0].get("name") == "utf8" and rows[0].get("stdout") == rows[0].get("stderr") == "",
                    "API checks did not pass through oxid test")
            oracle.cases(prefix + "-reference", [retained, "run", "--frozen"], project / "app", oracle.no_tools())
            require(not oracle.marker.exists(), "reference commands invoked native tools")
            source_hashes = tree_hashes(first_root)
            deployed = []
            if llvm:
                shutil.copytree(first_root, moved_root, ignore=shutil.ignore_patterns(".oxid"))
                for profile in ("dev", "release"):
                    cache = project / "app/.oxid"
                    if cache.exists():
                        shutil.rmtree(cache)
                    label = prefix + "-" + profile
                    clean = oracle.build(retained, project, profile, label + "-clean")
                    oracle.cases(label + "-native", [clean], output)
                    cold = oracle.build(retained, project, profile, label + "-cold", "miss")
                    (project / "app/.oxid/bin/utf8-normalize").unlink()
                    warm = oracle.build(retained, project, profile, label + "-warm", "hit")
                    moved = oracle.build(retained, moved_root / APP, profile, label + "-relocated")
                    require(clean.read_bytes() == cold.read_bytes() == warm.read_bytes() == moved.read_bytes(),
                            "raw ELF differs across clean/cache/relocation in " + profile)
                    require(digest(clean) == baseline.setdefault(profile, digest(clean)), "Rust compiler profile changed output")
                    status, value = expected(LONG_INPUT)
                    for binary in (cold, warm, moved):
                        oracle.command(binary.name + "-smoke", [binary], output, LONG_INPUT, status, value)
                    contract = oracle.build(retained, project, profile, label + "-api", member="contracts")
                    oracle.command(label + "-api-run", [contract], output, stdout=b"0\n")
                    deployed.append((profile, moved, contract))
            oracle.controls(retained, project, prefix, bool(llvm))
            require(tree_hashes(first_root) == source_hashes, "source/lock inputs changed")
            if moved_root.exists():
                require(tree_hashes(moved_root) == source_hashes, "relocated inputs changed")
            shutil.rmtree(first_root)
            if moved_root.exists():
                shutil.rmtree(moved_root)
            inherited = oracle.environment
            try:
                oracle.environment = {}
                for profile, binary, contract in deployed:
                    empty = output / (prefix + "-" + profile + "-empty")
                    empty.mkdir()
                    indexes = oracle.cases(binary.name + "-deployed", [binary], empty, complete_unicode=True)
                    oracle.command(contract.name + "-deployed", [contract], empty, stdout=b"0\n")
                    require(not list(empty.iterdir()), "deployed program created files")
                    report["deployments"].append({"artifact": str(binary), "sha256": digest(binary), "profile": profile,
                        "cases": indexes, "environment": {}, "removed_source_roots": [str(first_root), str(moved_root)],
                        "source_trees_removed": not first_root.exists() and not moved_root.exists(),
                        "all_unicode_scalar_values": 1112064})
            finally:
                oracle.environment = inherited
            require(digest(compiler) == compiler_hash == digest(retained), "compiler changed during checks")
        require({str(path): tree_hashes(ROOT / path) for path in (APP, LIBRARY)} == report["sources"], "sources changed")
        require(all(digest(ROOT / "scripts" / name) == value for name, value in report["controllers_sha256"].items()),
                "controller changed during checks")
        require(all(digest(Path(row["path"])) == row["sha256"] for row in report["tools"]), "tool changed")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "reference_only": args.reference_only, "core_cases": len(CASES),
                          "commands": len(report["commands"]), "builds": len(report["builds"]), "output": str(output)}))
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
