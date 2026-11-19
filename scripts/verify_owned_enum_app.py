#!/usr/bin/env python3
"""Run frame-filter against an independent binary protocol oracle.

Requires Linux x86_64 or ARM64 and LLVM 19.1.7. Retains exact command inputs, outputs,
compiler/tool identities and untouched ELF files. Reproducibility comparisons
use the same SDK/environment and relative layout, separately for O0 and O2.
"""
import linux_native_target as native_target
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
import tomllib

from verify_typed_package_app import ROOT, digest, require, save
from verify_typed_package_lock import framed, reference_source_checksum

APPLICATION = ROOT / "apps/frame-filter"
MAX_CAPTURE = 8 * 1024 * 1024


def record(tag, first, second):
    return bytes((tag, first, second, (tag + first + second) % 256))


def expected(data):
    """Protocol definition over byte strings, independent of Oxid code/types."""
    output = bytearray()
    for start in range(0, len(data), 4):
        frame = data[start:start + 4]
        if len(frame) != 4:
            return 66, bytes(output)
        tag, first, second, checksum = frame
        if sum(frame[:3]) % 256 != checksum:
            return 65, bytes(output)
        if tag not in (0, 1, 2):
            return 64, bytes(output)
        if tag == 1:
            output.extend(record(tag, min(first, second), max(first, second)))
        elif tag == 2:
            output.extend(frame)
    return 0, bytes(output)


PREFIX = record(1, 255, 0) + record(2, 0, 255)
CASES = (
    ("empty", b""), ("padding", record(0, 255, 1)),
    ("pair-zero", record(1, 0, 0)), ("pair-max", record(1, 255, 255)),
    ("pair-reversed", record(1, 255, 0)), ("pair-equal", record(1, 128, 128)),
    ("raw-nul-ff", record(2, 0, 255)), ("mixed", PREFIX + record(0, 42, 13) + record(1, 9, 3)),
    ("all-bytes", b"".join(record(2, n, 255 - n) for n in range(256))),
    ("all-pair-boundaries", b"".join(record(1, a, b) for a in (0, 1, 127, 128, 254, 255)
                                    for b in (0, 1, 127, 128, 254, 255))),
    ("unknown-tag", record(255, 7, 9)), ("bad-checksum", bytes((1, 9, 3, 12))),
    ("checksum-before-tag", bytes((255, 7, 9, 0))),
    ("bad-padding", bytes((0, 0, 0, 1))),
    ("truncated-one", b"\x01"), ("truncated-two", b"\x02\xff"),
    ("truncated-three", b"\xff\x00\x00"),
    ("prefix-before-tag", PREFIX + record(3, 0, 0) + record(2, 9, 9)),
    ("prefix-before-checksum", PREFIX + bytes((1, 0, 0, 0)) + record(2, 9, 9)),
    ("prefix-before-truncation", PREFIX + b"\x02\x00\xff"),
    ("bounded-batch", (record(0, 10, 20) + record(1, 250, 5) + record(2, 0, 10)) * 64),
)

FAULT_SOURCE = """use crate::codec::decode;
use crate::codec::Decoded;
use crate::codec::Content;
use crate::output::emit;
use std::io::read_stdin;
use std::io::ReadStatus;
fn spin()->i32{while true {}return 0;}
fn main()->i32{
    let mut input=[0];let read=read_stdin(&mut input);
    match read{ReadStatus::Full=>{},ReadStatus::Eof(n)=>{return 66;},ReadStatus::IoError=>{return 74;},}
    let selected=input[0];
    let decoded=decode([2,7,8,17]);
    match decoded{
        Decoded::Item(content)=>{match content{
            Content::Bytes(bytes)=>{
                let mut values=bytes;
                let sent=emit(Content::Bytes([0,255]));if sent!=0{return sent;}
                if selected==0{return values[2];}
                if selected==1{values[2]=2147483647+1;return 99;}
                if selected==2{return emit(Content::Bytes([65,256]));}
                if selected==3{let skipped=false&&(values[2]==0);if !skipped{return 0;}return 99;}
                return spin();
            },
            Content::Pair(pair)=>{return 98;},Content::Skip=>{return 97;},
        }},
        Decoded::Rejected(code)=>{return code;},
    }
}
"""


def diagnostic(marker, code, stage, message):
    prefix = FAULT_SOURCE[:FAULT_SOURCE.index(marker)]
    line, column = prefix.count("\n") + 1, len(prefix.rsplit("\n", 1)[-1]) + 1
    return f"error[{code}] ({stage}): {message}\n  --> src/main.ox:{line}:{column}\n".encode()


def tree_hashes(path):
    return {file.relative_to(path).as_posix(): digest(file) for file in sorted(path.rglob("*"))
            if file.is_file() and ".oxid" not in file.relative_to(path).parts}


class Oracle:
    def __init__(self, output, environment, report):
        self.output, self.environment, self.report = output, environment, report
        self.commands = report["commands"] = []
        for name in ("commands", "compilers", "artifacts", "scratch"):
            (output / name).mkdir()
        self.spy = output / "tools-spy"
        self.spy.mkdir()
        self.marker = output / "tool-invoked"
        for tool in ("clang", "opt", "ld.lld"):
            path = self.spy / tool
            path.write_text('#!/bin/sh\nprintf invoked > "$OXID_APP_SPY_MARKER"\nexit 91\n')
            path.chmod(0o755)

    def input_capture_limit(self):
        return MAX_CAPTURE

    def command(self, label, argv, cwd, payload=b"", status=0, stdout=None, stderr=b"",
                overrides=None, closed_stdin=False, directory_stdin=False, full_stdout=False):
        folder = self.output / "commands" / f"{len(self.commands):04d}-{label}"
        folder.mkdir()
        (folder / "stdin.bin").write_bytes(payload)
        row = {"label": label, "argv": [str(value) for value in argv], "cwd": str(cwd),
               "environment_overrides": overrides or {}, "expected_status": status,
               "closed_stdin": closed_stdin, "directory_stdin": directory_stdin,
               "stdout_sink": "/dev/full" if full_stdout else "capture",
               "receipt": str(folder), "status": "running"}
        if stdout is not None:
            row["expected_stdout_sha256"] = hashlib.sha256(stdout).hexdigest()
        row["expected_stderr_sha256"] = hashlib.sha256(stderr).hexdigest()
        self.commands.append(row)
        started = time.monotonic()
        directory_fd = None
        try:
            if directory_stdin:
                directory_fd = os.open(cwd, os.O_RDONLY | os.O_DIRECTORY)
            with (folder / "stdin.bin").open("rb") as source, (folder / "stdout.bin").open("wb") as capture, \
                    (folder / "stderr.bin").open("wb") as errors, open("/dev/full", "wb") as full:
                result = subprocess.run(row["argv"], cwd=cwd,
                                        env={**self.environment, **(overrides or {})},
                                        stdin=directory_fd if directory_stdin else source,
                                        stdout=full if full_stdout else capture, stderr=errors,
                                        preexec_fn=(lambda: os.close(0)) if closed_stdin else None,
                                        timeout=120)
            row["exit_status"] = result.returncode
            for stream in ("stdin", "stdout", "stderr"):
                path = folder / (stream + ".bin")
                row[stream + "_bytes"], row[stream + "_sha256"] = path.stat().st_size, digest(path)
                limit = self.input_capture_limit() if stream == "stdin" else MAX_CAPTURE
                require(path.stat().st_size <= limit, "capture exceeds bound")
            result.stdout = (folder / "stdout.bin").read_bytes()
            result.stderr = (folder / "stderr.bin").read_bytes()
            require(result.returncode == status and (stdout is None or result.stdout == stdout)
                    and result.stderr == stderr, label + ": status/bytes differ; inspect " + str(folder))
            row["status"] = "passed"
            return result, len(self.commands) - 1
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            row.update(status="failed", error=str(error))
            raise
        finally:
            if directory_fd is not None:
                os.close(directory_fd)
            row["elapsed_seconds"] = time.monotonic() - started
            save(folder / "command.json", row)

    def cases(self, label, argv, cwd, overrides=None):
        indexes = []
        for name, data in CASES:
            status, output = expected(data)
            _, index = self.command(label + "-" + name, argv, cwd, data, status, output, overrides=overrides)
            indexes.append(index)
        _, index = self.command(label + "-directory-input", argv, cwd, status=74, stdout=b"",
                                overrides=overrides, directory_stdin=True)
        indexes.append(index)
        if len(argv) == 1:
            # Rust's own process startup repairs missing standard descriptors;
            # a closed-fd oracle applies to the generated C-runtime ELF only.
            _, index = self.command(label + "-closed-input", argv, cwd, status=74, stdout=b"",
                                    overrides=overrides, closed_stdin=True)
            indexes.append(index)
        for name, data, status in (("write-before-later-error", PREFIX + record(3, 0, 0), 75),
                                   ("invalid-before-write", record(3, 0, 0), 64),
                                   ("padding-never-writes", record(0, 0, 0), 0)):
            _, index = self.command(label + "-" + name, argv, cwd, data, status, b"",
                                    overrides=overrides, full_stdout=True)
            indexes.append(index)
        return indexes

    def inspect_lock(self, app):
        lock = tomllib.loads((app / "oxid.lock").read_text())
        require(lock.get("version") == 2 and lock.get("kind") == "typed-local-v1", "wrong lock schema")
        rows = lock.get("package", [])
        by_path = {row.get("path"): row for row in rows}
        require(len(rows) == 3 and set(by_path) == {".", "packages/codec", "packages/output"},
                "diamond did not retain three package identities")
        for path, row in by_path.items():
            identity = hashlib.sha256(framed(b"oxid-typed-local-package-v1") + framed(path.encode())).hexdigest()
            require(row["id"] == identity and row["manifest"] == digest(app / path / "oxid.toml"),
                    "package identity or manifest bytes differ")
            require(row["sources"] == ("root" if path == "." else
                    reference_source_checksum(app, path, row["entry"])), "dependency source checksum differs")
        app_id, codec_id, output_id = (by_path[path]["id"] for path in (".", "packages/codec", "packages/output"))
        edges = {(row["owner"], row["alias"], row["package"]) for row in lock.get("dependency", [])}
        require(lock["root"] == app_id and len(lock.get("dependency", [])) == 3
                and edges == {(app_id, "codec", codec_id), (app_id, "output", output_id), (output_id, "codec", codec_id)},
                "package aliases lost their shared nominal identity")
        self.report["locks"].append({"path": str(app / "oxid.lock"), "sha256": digest(app / "oxid.lock"),
                                     "packages": rows, "dependencies": lock["dependency"]})

    def build(self, compiler, app, profile, label):
        result, index = self.command(label, [compiler, "build", "--frozen", "--reproducible",
                                            "--profile=" + profile, "--message-format=json"], app)
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        require(len(rows) == 1 and rows[0].get("kind") == "compile-summary"
                and rows[0].get("success") is True, "native build did not succeed")
        artifact = self.output / "artifacts" / label
        shutil.copyfile(app / ".oxid/bin/frame-filter", artifact)
        artifact.chmod(0o555)
        body = artifact.read_bytes()
        require(body[:7] == b"\x7fELF\x02\x01\x01" and body[16:20] == native_target.elf_identity(3), "not host PIE ELF")
        require(str(self.output).encode() not in body and str(app).encode() not in body,
                "reproducible ELF embeds an absolute fixture path")
        require(all(name.encode() in body for name in ("src/main.ox", "packages/codec/src/lib.ox",
                                                       "packages/output/src/lib.ox")), "logical source names missing")
        row = {"label": label, "profile": profile, "command": index, "artifact": str(artifact),
               "sha256": digest(artifact), "bytes": len(body)}
        self.report["builds"].append(row)
        return artifact, row

    def reject(self, compiler, app, profile, label, code=None, message=None):
        executable, lock = app / ".oxid/bin/frame-filter", app / "oxid.lock"
        previous, locked = executable.read_bytes(), lock.read_bytes()
        before_cache = tree_hashes(app / ".oxid/cache")
        result, index = self.command(label, [compiler, "build", "--frozen", "--profile=" + profile,
                                            "--message-format=json"], app, status=1,
                                     overrides={"OXID_LLVM_BIN": str(self.spy), "OXID_APP_SPY_MARKER": str(self.marker)})
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        diagnostics = [row for row in rows if row.get("kind") == "diagnostic"]
        require(any((code is None or row.get("code") == code)
                    and (message is None or message in row.get("message", "")) for row in diagnostics),
                label + ": wrong preflight failure")
        require(any(row.get("kind", "").endswith("-summary") and row.get("success") is False for row in rows)
                and not any(row.get("kind") == "build-cache" for row in rows), "partial build success on rejection")
        require(not self.marker.exists() and executable.read_bytes() == previous and lock.read_bytes() == locked
                and tree_hashes(app / ".oxid/cache") == before_cache, "failed build changed output/lock/cache or invoked tools")
        self.report["controls"].append({"label": label, "command": index, "profile": profile})

    def controls(self, compiler, app, prefix):
        main, codec = app / "src/main.ox", app / "packages/codec/src/lib.ox"
        original, library, locked = main.read_bytes(), codec.read_bytes(), (app / "oxid.lock").read_bytes()
        borrow = "use crate::codec::Pair;fn take(a:&Pair,b:Pair)->i32{return 0;}"
        head = "use crate::codec::decode;use crate::codec::Decoded;use crate::codec::Content;"
        pair = "fn main()->i32{let value=decode([1,7,8,16]);match value{Decoded::Item(content)=>{match content{Content::Pair(pair)=>{return take(&pair,pair);},Content::Bytes(bytes)=>{return 0;},Content::Skip=>{return 0;},}},Decoded::Rejected(code)=>{return code;},}}"
        negatives = (
            ("parse", b"fn main(\n", "E0100"),
            ("nested-move", (head + "fn take(value:Decoded)->i32{return 0;}fn main()->i32{let value=decode([2,7,8,17]);let first=take(value);return take(value);}").encode(), "E0310"),
            ("array-payload-move", (head + "fn main()->i32{let value=decode([2,7,8,17]);match value{Decoded::Item(content)=>{match content{Content::Bytes(bytes)=>{let moved=bytes;return bytes[0];},Content::Pair(pair)=>{return 0;},Content::Skip=>{return 0;},}},Decoded::Rejected(code)=>{return code;},}}").encode(), "E0310"),
            ("borrow-before-move", (head + borrow + pair).encode(), "E0311"),
        )
        try:
            for name, changed, code in negatives:
                main.write_bytes(changed)
                for profile in ("dev", "release"):
                    self.reject(compiler, app, profile, prefix + "-" + name + "-" + profile, code)
            main.write_bytes(original)
            codec.write_bytes(library + b"\n// dependency input drift\n")
            self.reject(compiler, app, "release", prefix + "-stale-lock", message="out of date")
            codec.write_bytes(library.replace(b"pub struct Pair", b"struct Pair", 1))
            self.command(prefix + "-lock-private-type", [compiler, "lock", "--update"], app)
            self.reject(compiler, app, "release", prefix + "-private-payload-type", "E0207")
        finally:
            main.write_bytes(original)
            codec.write_bytes(library)
            (app / "oxid.lock").write_bytes(locked)

        # Identical package display names and source bytes at different roots
        # must not make their nominal enum/record identities interchangeable.
        mirror = app / "packages/mirror"
        output_manifest = app / "packages/output/oxid.toml"
        output_original = output_manifest.read_bytes()
        shutil.copytree(app / "packages/codec", mirror)
        try:
            output_manifest.write_bytes(output_original.replace(b'codec = "../codec"', b'codec = "../mirror"'))
            self.command(prefix + "-lock-distinct-root", [compiler, "lock", "--update"], app)
            self.reject(compiler, app, "release", prefix + "-distinct-nominal-root", "E0300", "call argument type")
        finally:
            output_manifest.write_bytes(output_original)
            (app / "oxid.lock").write_bytes(locked)
            shutil.rmtree(mirror)

    def faults(self, compiler, app, profile, label):
        path = app / "src/main.ox"
        original = path.read_bytes()
        try:
            path.write_text(FAULT_SOURCE)
            artifact, row = self.build(compiler, app, profile, label)
            cases = (
                (0, 1, diagnostic("values[2];}", "E0606", "oir-owned-run", "array index out of bounds")),
                (1, 1, diagnostic("+1;return 99", "E0604", "oir-run", "checked i32 arithmetic overflow")),
                (2, 75, b""), (3, 0, b""),
                (4, 1, diagnostic("while true", "E0601", "oir-run", "execution fuel exhausted")),
            )
            row["fault_cases"] = []
            for selected, status, stderr in cases:
                _, index = self.command(label + "-" + str(selected), [artifact], self.output,
                                        bytes((selected,)), status, record(2, 0, 255), stderr)
                row["fault_cases"].append(index)
        finally:
            path.write_bytes(original)


def verify(args):
    require(native_target.host() is not None, "requires Linux x86_64 or ARM64")
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink() and ROOT not in output.parents,
            "output must be a new directory outside the repository")
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(compilers and len(set(compilers)) == len(compilers)
            and all(path.is_file() and os.access(path, os.X_OK) for path in compilers), "invalid compiler paths")
    llvm = args.llvm_bin.resolve(strict=True)
    output.mkdir(parents=True)
    overrides = {"PATH": str(llvm) + ":/usr/bin:/bin", "OXID_LLVM_BIN": str(llvm), "LANG": "C.UTF-8",
                 "LC_ALL": "C.UTF-8", "TZ": "UTC", "SOURCE_DATE_EPOCH": "0", "TMPDIR": str(output / "scratch")}
    if "LD_LIBRARY_PATH" in os.environ:
        overrides["LD_LIBRARY_PATH"] = os.environ["LD_LIBRARY_PATH"]
    report = {"schema": "oxid-owned-enum-application-v1", "status": "running",
              "scope": "Linux x86_64 or ARM64 bounded native; same SDK/environment and relative layout; no speed claim",
              "environment": overrides, "sources": tree_hashes(APPLICATION), "compilers": [], "tools": [],
              "controllers_sha256": {name: digest(ROOT / "scripts" / name) for name in
                                     ("verify_owned_enum_app.py", "verify_typed_package_app.py", "verify_typed_package_lock.py")},
              "locks": [], "builds": [], "controls": [], "deployments": []}
    oracle = Oracle(output, {**os.environ, **overrides}, report)
    baseline = {}
    try:
        for name in ("clang", "opt", "ld.lld"):
            invocation = llvm / name
            physical = invocation.resolve(strict=True)
            result, index = oracle.command("version-" + name, [invocation, "--version"], output)
            require(b"19.1.7" in result.stdout, "requires actual LLVM 19.1.7")
            report["tools"].append({"name": name, "path": str(physical), "sha256": digest(physical), "command": index})
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
            oracle.command(prefix + "-lock", [retained, "lock", "--offline"], first)
            oracle.inspect_lock(first)
            oracle.command(prefix + "-check", [retained, "check", "--frozen", "--message-format=json"], first)
            tested, _ = oracle.command(prefix + "-test", [retained, "test", "--frozen", "--message-format=json"], first)
            rows = [json.loads(line) for line in tested.stdout.splitlines()]
            require(len(rows) == 2 and rows[0].get("kind") == "test-result" and rows[0].get("name") == "codec"
                    and rows[0].get("success") is True and rows[0].get("exit_status") == 0
                    and rows[1].get("passed") == 1 and rows[1].get("failed") == 0, "declared test did not pass")
            oracle.cases(prefix + "-run", [retained, "run", "--frozen"], first)
            shutil.copytree(first, relocated, ignore=shutil.ignore_patterns(".oxid"))
            sources = tree_hashes(first)
            deployed = []
            for profile in ("dev", "release"):
                original, original_row = oracle.build(retained, first, profile, prefix + "-" + profile + "-first")
                original_row["cases"] = oracle.cases(original_row["label"], [original], output)
                moved, moved_row = oracle.build(retained, relocated, profile, prefix + "-" + profile + "-relocated")
                moved_row["cases"] = oracle.cases(moved_row["label"], [moved], output)
                require(original.read_bytes() == moved.read_bytes(), "relocation changed raw ELF bytes")
                prior = baseline.setdefault(profile, digest(original))
                require(digest(original) == prior, "compiler build profile changed raw generated ELF bytes")
                deployed.append(moved)
                oracle.faults(retained, first, profile, prefix + "-" + profile + "-faults")
            # Failed builds must preserve the last working application, not a
            # deliberate runtime fault probe. Restore that retained ELF first.
            shutil.copyfile(deployed[-1], first / ".oxid/bin/frame-filter")
            oracle.controls(retained, first, prefix)
            oracle.cases(prefix + "-preserved", [first / ".oxid/bin/frame-filter"], output)
            require(tree_hashes(first) == sources and tree_hashes(relocated) == sources, "source/lock inputs changed")
            shutil.rmtree(first)
            shutil.rmtree(relocated)
            for artifact in deployed:
                indexes = oracle.cases(artifact.name + "-deployed", [artifact], output, overrides={"PATH": ""})
                report["deployments"].append({"artifact": str(artifact), "sha256": digest(artifact), "cases": indexes,
                                               "source_trees_removed": not first.exists() and not relocated.exists()})
            require(digest(compiler) == before == digest(retained), "compiler changed during verification")
        require(tree_hashes(APPLICATION) == report["sources"], "repository application changed")
        require(all(digest(Path(row["path"])) == row["sha256"] for row in report["tools"]), "tool changed")
        require(all(digest(ROOT / "scripts" / name) == sha for name, sha in report["controllers_sha256"].items()),
                "controller changed")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "builds": len(report["builds"]),
                          "commands": len(report["commands"]), "output": str(output)}))
        return 0
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--llvm-bin", required=True, type=Path)
    try:
        return verify(parser.parse_args())
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
