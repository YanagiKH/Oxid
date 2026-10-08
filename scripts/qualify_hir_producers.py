#!/usr/bin/env python3
"""Linux x86_64 opt-in qualification; preserves all artifacts in a new directory.

Authentic Oxid-built parser/static success is separate from controlled C transport
faults. Requires a built compiler, LLVM 19 tools, and cc. Does not build Rust,
change source bindings, install tools, or establish hostile-process containment.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time


def digest(data):
    return hashlib.sha256(data).hexdigest()


class Qualification:
    def __init__(self, compiler, llvm_bin, output, cc):
        self.repo = Path(__file__).resolve().parents[1]
        self.compiler = compiler.resolve(strict=True)
        self.root = output.resolve()
        self.root.mkdir()  # Never overwrite an earlier run, including failures.
        self.snapshots = self.root / "snapshots"
        self.snapshots.mkdir()
        self.env = dict(os.environ, TMPDIR=str(self.snapshots),
                        OXID_LLVM_BIN=str(llvm_bin.resolve(strict=True)))
        self.cc = cc
        self.receipts = []
        self.harness_hash = digest(Path(__file__).read_bytes())
        self.source_head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=self.repo).decode().strip()
        self.source = (self.repo / "tests/fixtures/checked_hir_import/rich-source.txt").read_bytes()
        self.main = self.root / "main.ox"
        self.main.write_bytes(self.source)

    def call(self, name, args, data=None, status=0, timeout=120):
        command = [str(arg) for arg in args]
        start = time.monotonic()
        try:
            result = subprocess.run(command, cwd=self.repo, env=self.env, input=data,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    timeout=timeout)
        except subprocess.TimeoutExpired as error:
            # Preserve partial failure evidence. Synthetic helper alarms provide
            # an independent 15-second lifetime ceiling, including fork children.
            stdout, stderr = error.stdout or b"", error.stderr or b""
            (self.root / (name + ".stdout")).write_bytes(stdout)
            (self.root / (name + ".stderr")).write_bytes(stderr)
            self.receipts.append(dict(name=name, argv=command, status="outer-timeout",
                                     elapsed_seconds=time.monotonic() - start,
                                     stdout_sha256=digest(stdout), stderr_sha256=digest(stderr)))
            (self.root / "receipts.json").write_text(json.dumps(self.receipts, indent=2) + "\n")
            raise
        (self.root / (name + ".stdout")).write_bytes(result.stdout)
        (self.root / (name + ".stderr")).write_bytes(result.stderr)
        self.receipts.append(dict(name=name, argv=command, status=result.returncode,
                                 elapsed_seconds=time.monotonic() - start,
                                 stdout_sha256=digest(result.stdout),
                                 stderr_sha256=digest(result.stderr)))
        (self.root / "receipts.json").write_text(json.dumps(self.receipts, indent=2) + "\n")
        assert result.returncode == status, (name, result.returncode, result.stdout, result.stderr)
        return result

    def command(self, op, source, option, value, output=None, json_mode=True):
        args = [self.compiler, op, source, "--edition=typed-preview", option, value]
        if json_mode:
            args.append("--message-format=json")
        if op == "compile":
            args += ["--backend=llvm", "--output", output]
        return args

    def manifest(self, bundle):
        hashes = {role: digest((bundle / role).read_bytes()) for role in ("parser", "static")}
        (bundle / "manifest.txt").write_text("OXID-HIR-PRODUCERS-1\n" + "".join(
            role + " " + value + "\n" for role, value in hashes.items()))
        return hashes

    def authentic(self):
        self.bundle = self.root / "authentic-bundle"
        self.bundle.mkdir()
        for role, source in (("parser", "parser_main.ox"), ("static", "ast_static_main.ox")):
            self.call("build-" + role, [self.compiler, "compile",
                      self.repo / "fixtures/typed-lexer-samples" / source,
                      "--edition=typed-preview", "--backend=llvm", "--entry-mode=process",
                      "--output", self.bundle / role])
        hashes = self.manifest(self.bundle)
        self.opa = self.call("manual-parser", [self.bundle / "parser"], self.source).stdout
        self.ast1 = b"AST1" + bytes([len(self.source)]) + self.source + self.opa
        self.wire = self.call("manual-static", [self.bundle / "static"], self.ast1).stdout
        assert self.wire == (self.repo / "tests/fixtures/checked_hir_import/rich-success.bin").read_bytes()
        for op in ("check", "run", "compile"):
            result = self.call(op, self.command(op, self.main, "--experimental-hir-producers",
                               self.bundle, self.root / "program.elf"))
            rows = [json.loads(line) for line in result.stdout.splitlines()]
            records = [row for row in rows if row.get("kind") == "hir-producer"]
            assert len(records) == 2
            for role, record, input_data, output in zip(
                    ("parser", "static"), records, (self.source, self.ast1), (self.opa, self.wire)):
                assert record["role"] == role and record["executable_sha256"] == hashes[role]
                assert record["input_sha256"] == digest(input_data)
                assert record["input_written"] == len(input_data)
                assert record["captured_stdout_sha256"] == digest(output)
                assert record["captured_stdout_bytes"] == len(output)
                assert record["captured_stderr_bytes"] == 0
                assert record["spawned"] and record["leader_reaped"]
                assert record["stop"] == "exited" and record["exit_status"] == 0
                assert record["signal"] is None
            assert rows[-1]["success"] is True and not result.stderr
            assert not list(self.snapshots.iterdir())
            self.call("manual-import-" + op, self.command(op, self.main,
                      "--experimental-hir-import", self.root / "manual-static.stdout",
                      self.root / "manual-program.elf"))
        assert (self.root / "program.elf").read_bytes() == (self.root / "manual-program.elf").read_bytes()
        assert self.call("native", [self.root / "program.elf"]).stdout == b"1\n"
        text = self.call("text-run", self.command("run", self.main,
                         "--experimental-hir-producers", self.bundle, json_mode=False))
        assert text.stdout == b"1\n"
        print("Authentic rebuilt Oxid producer replay and manual native identity: PASS", flush=True)

    def helper(self, path, role, mode):
        data = self.opa if role == "parser" else self.wire
        if mode == "malformed":
            data = b"BAD!" + data[4:]
        elif mode == "short":
            data = data[:-1]
        elif mode == "oversize":
            data += b"X"
        elif mode == "semantic":
            data = data[:1575] + bytes([data[1575] ^ 1]) + data[1576:]
        pidfile = self.root / (path.parent.name + "-" + role + ".pid")
        source = ("#include <unistd.h>\n#include <stdio.h>\n#include <stdlib.h>\n#include <signal.h>\n"
                  "static unsigned char output[]={" + ",".join(map(str, data)) + "};\n"
                  "int main(){signal(SIGALRM,SIG_DFL);alarm(15);char buf[2048];"
                  "while(read(0,buf,sizeof buf)>0){};")
        if mode in ("timeout", "descendant"):
            source += "FILE*f=fopen(" + json.dumps(str(pidfile)) + ',"w");if(!f)return 91;'
            if mode == "descendant":
                source += ("pid_t p=fork();if(p<0)return 92;if(p==0){alarm(15);fclose(f);"
                           "for(;;)pause();}fprintf(f,\"%d\",p);fclose(f);")
            else:
                source += 'fprintf(f,"%d",getpid());fclose(f);for(;;)pause();'
        if mode == "stderr":
            source += 'write(2,"bad",3);'
        elif mode == "stderr-overflow":
            source += "char err[4097]={0};write(2,err,sizeof err);"
        source += "write(1,output,sizeof output);return " + ("17" if mode == "status" else "0") + ";}"
        path.with_suffix(".c").write_text(source)
        self.call("helper-" + path.parent.name + "-" + role,
                  [self.cc, "-O0", path.with_suffix(".c"), "-o", path])
        return pidfile

    def probe(self, name, bundle, source, record_count, stop=None, ops=("check", "run", "compile"), diagnostic=None):
        source_path = self.root / (name + ".ox")
        source_path.write_bytes(source)
        cases = [(op, True) for op in ops]
        if "compile" in ops:
            cases.append(("compile", False))
        for op, preexisting in cases:
            label = name + "-" + op + ("" if preexisting else "-new-output")
            target = self.root / (label + ".elf")
            if preexisting:
                target.write_bytes(b"KEEP-EXISTING-OUTPUT")
            result = self.call(label, self.command(op, source_path,
                               "--experimental-hir-producers", bundle, target), status=1, timeout=12)
            rows = [json.loads(line) for line in result.stdout.splitlines()]
            records = [row for row in rows if row.get("kind") == "hir-producer"]
            assert len(records) == record_count, (name, op, records)
            if preexisting:
                assert target.read_bytes() == b"KEEP-EXISTING-OUTPUT", (name, op, "output overwritten")
            else:
                assert not target.exists(), (name, op, "new artifact published")
            if diagnostic:
                assert any(row.get("code") == diagnostic for row in rows), (label, rows)
            assert rows[-1]["success"] is False
            assert not list(self.snapshots.iterdir()), (name, "workspace leaked")
            for record in records:
                assert record["leader_reaped"] and record["spawned"]
                assert record["executable_sha256"] == digest((bundle / record["role"]).read_bytes())
            if stop:
                assert records[-1]["stop"] == stop, (name, op, records)
            if records:
                assert records[0]["input_sha256"] == digest(source)
                assert records[0]["input_written"] == len(source)
            if len(records) == 2:
                assert records[1]["input_sha256"] == digest(b"AST1" + bytes([len(source)]) + source + self.opa)
            self.probes += 1
            print(label, "PASS", flush=True)

    def negatives(self):
        self.probes = 0
        for role in ("parser", "static"):
            for mode in ("malformed", "short", "status", "stderr", "oversize",
                         "stderr-overflow", "timeout", "descendant"):
                name = role + "-" + mode
                bundle = self.root / name
                bundle.mkdir()
                for selected in ("parser", "static"):
                    pid = self.helper(bundle / selected, selected, mode if selected == role else "valid")
                    if selected == role:
                        pidfile = pid
                self.manifest(bundle)
                stop = {"oversize": "stdout-limit", "stderr-overflow": "stderr-limit",
                        "timeout": "deadline", "descendant": "deadline"}.get(mode, "exited")
                ops = ("check",) if mode in ("timeout", "descendant") else ("check", "run", "compile")
                self.probe(name, bundle, self.source, 1 if role == "parser" else 2, stop, ops)
                if mode in ("timeout", "descendant"):
                    pid = int(pidfile.read_text())
                    proc = Path("/proc") / str(pid) / "stat"
                    assert not proc.exists() or proc.read_text().split(") ", 1)[1].startswith("Z"), ("alive", pid)
        self.probe("source-oversize", self.bundle, self.source + b" " * (129 - len(self.source)), 0)
        self.probe("source-nonascii", self.bundle, self.source + b"//\xc3\xa9", 0)
        bad = self.root / "bad-identity"
        bad.mkdir()
        for role in ("parser", "static"):
            (bad / role).write_bytes((self.bundle / role).read_bytes())
            (bad / role).chmod(0o755)
        hashes = self.manifest(bad)
        manifest = bad / "manifest.txt"
        manifest.write_text(manifest.read_text().replace(hashes["parser"], "0" * 64))
        self.probe("identity-mismatch", bad, self.source, 0)
        link = self.root / "symlink-bundle"
        link.symlink_to(self.bundle, target_is_directory=True)
        self.probe("symlink-root", link, self.source, 0)
        # These complete-size, success-status observations pass process framing,
        # then must fail the independent checked importer, without fallback.
        for name, mode, source in (
                ("static-semantic", "semantic", self.source),
                ("stale-source", "valid", self.source.replace(b"f(1)", b"f(2)"))):
            assert name != "stale-source" or source != self.source
            bundle = self.root / name
            bundle.mkdir()
            self.helper(bundle / "parser", "parser", "valid")
            self.helper(bundle / "static", "static", mode)
            self.manifest(bundle)
            self.probe(name, bundle, source, 2, "exited", diagnostic="E0702")
        assert self.probes == 76

    def finish(self):
        summary = dict(result="passed", negative_cli_probes=self.probes,
                       compiler_sha256=digest(self.compiler.read_bytes()),
                       harness_sha256=self.harness_hash, source_head=self.source_head,
                       source_sha256=digest(self.source), wire_sha256=digest(self.wire),
                       native_sha256=digest((self.root / "program.elf").read_bytes()),
                       files={str(p.relative_to(self.root)): digest(p.read_bytes())
                              for p in self.root.rglob("*") if p.is_file()})
        (self.root / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True, help="new evidence directory")
    parser.add_argument("--cc", default="cc", help="C compiler executable, no shell syntax")
    args = parser.parse_args()
    if not __debug__:
        parser.error("assertions must remain enabled; do not use python -O")
    run = Qualification(args.compiler, args.llvm_bin, args.output, args.cc)
    run.authentic()
    run.negatives()
    run.finish()


if __name__ == "__main__":
    main()
