#!/usr/bin/env python3
"""Real checked-i32 LLVM O0 / reference / independent Python-bigint gate.

Usage: verify_native_arithmetic.py <oxid-binary> [<other-profile-binary> ...]
Requires the same mandatory LLVM 19.1.7/Linux x86_64 tools as native-preview.
The Python model is shared with the independent reference arithmetic oracle;
no production parser, lowering, evaluator or machine arithmetic supplies answers.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

from verify_i32_arithmetic import MIN, MAX, corpus


def command(args, **kwargs):
    return subprocess.run(args, capture_output=True, timeout=30, **kwargs)


def cases():
    prefix = "// 雪\r\nfn id(x: i32) -> i32 { return x; } fn main() -> i32 { return "
    for expression, value, error in corpus():
        yield prefix + expression + "; }", "i32", value, None if error is None else len(prefix) + error
    for chosen in (True, False):
        for value in (MIN, -1, 0, 1, MAX):
            arms = [f"return id({value}) + 0;", "return 2147483647 + 1;"]
            if not chosen:
                arms.reverse()
            yield (
                "fn id(x: i32) -> i32 { return x; } "
                f"fn main() -> i32 {{ if {str(chosen).lower()} {{ {arms[0]} }} else {{ {arms[1]} }} }}",
                "i32", value, None,
            )
    # Origin must follow execution order, which deliberately differs from source
    # declaration order. Helpers fail at distinct operators/lines.
    helpers = """// 🦀\r
fn later() -> i32 { return -2147483648 - 1; }
fn first() -> i32 { return 2147483647 + 1; }
fn take(a: i32, b: i32) -> i32 { return b; }
"""
    for body, marker in [
        ("return first() + later();", "+ 1"),
        ("return later() + first();", "- 1"),
        ("return take(first(), later());", "+ 1"),
        ("return take(later(), first());", "- 1"),
        ("return take(3, first());", "+ 1"),
        ("first(); return later();", "+ 1"),
        ("let ignored = first(); return 42;", "+ 1"),
        ("0 * first(); return later();", "+ 1"),
        ("if true { return first(); } else { return later(); }", "+ 1"),
        ("if false { return first(); } else { return later(); }", "- 1"),
    ]:
        source = helpers + f"fn main() -> i32 {{ {body} }}"
        yield source, "i32", None, source.index(marker)
    for result_type, return_text, value in [("bool", "true", True), ("()", "()", None)]:
        for statement in ["2147483647 + 1;", "let ignored = -2147483648 * -1;", "fail();"]:
            source = f"fn fail() -> () {{ 2147483647 + 1; return; }} fn main() -> {result_type} {{ {statement} return {return_text}; }}"
            error = source.index("+ 1") if statement == "fail();" else source.index("+ 1", source.index("fn main")) if "+" in statement else source.index("* -1")
            yield source, result_type, None, error
        source = f"fn main() -> {result_type} {{ 46340 * 46340; -2147483648 + 0; return {return_text}; }}"
        yield source, result_type, value, None
    source = "fn unused() -> i32 { return 2147483647 + 1; } fn main() -> i32 { return 42; }"
    yield source, "i32", 42, None
    # An overflow must not be folded away by an algebraic identity or a dead
    # result, and arithmetic in condition calls must finish before branching.
    for expression, marker in [
        ("2147483647 + 1 - 1", "+ 1"),
        ("0 * (2147483647 + 1)", "+ 1"),
        ("(2147483647 + 1) * 0", "+ 1"),
        ("-2147483648 * -1", "* -1"),
        ("(2147483647 + 1) - (-2147483648 - 1)", "+ 1"),
        ("1 + (-2147483648 - 1)", "- 1"),
    ]:
        source = f"fn main() -> i32 {{ return {expression}; }}"
        yield source, "i32", None, source.index(marker)
    source = "fn condition() -> bool { 2147483647 + 1; return true; } fn main() -> () { if condition() {} else {} return; }"
    yield source, "()", None, source.index("+ 1")


def verify_adapter(root):
    """Use the real C adapter with controlled writes and noreturn exit capture."""
    llvm_bin = os.environ.get("OXID_LLVM_BIN")
    clang = str(Path(llvm_bin) / "clang") if llvm_bin else shutil.which("clang-19")
    lld = str(Path(llvm_bin) / "ld.lld") if llvm_bin else shutil.which("ld.lld-19")
    assert clang and lld, "LLVM 19.1.7 tools are required"
    harness = root / "overflow-adapter-test.c"
    harness.write_text(r'''
#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
_Noreturn void __oxid_overflow(const char *, uint64_t);
static const char expected[] = "error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> input.ox:2:38\n";
static int mode, calls, wrong_fd;
static size_t used;
static char captured[sizeof expected];
ssize_t __wrap_write(int fd, const void *bytes, size_t n) {
    if (fd != STDERR_FILENO) wrong_fd = 1;
    if (mode == 1 && calls++ == 0) { errno = EINTR; return -1; }
    if (mode == 2) return 0;
    if (mode == 3) { errno = EIO; return -1; }
    if (!n || used >= sizeof captured) return -1;
    captured[used++] = *(const char *)bytes;
    return 1;
}
void (*__wrap_signal(int sig, void (*handler)(int)))(int) {
    if (sig != SIGPIPE || handler != SIG_IGN) wrong_fd = 1;
    return mode == 4 ? SIG_ERR : SIG_DFL;
}
void (*__wrap___sysv_signal(int sig, void (*handler)(int)))(int) {
    return __wrap_signal(sig, handler);
}
_Noreturn void __wrap__exit(int status) {
    if (wrong_fd || status != (mode >= 2 ? 74 : 1)) _Exit(91);
    if (mode < 2 && (used != sizeof expected - 1 || memcmp(captured, expected, used))) _Exit(92);
    if (mode >= 2 && used != 0) _Exit(93);
    _Exit(0);
}
int main(int argc, char **argv) {
    if (argc != 2) return 90;
    mode = atoi(argv[1]);
    __oxid_overflow(expected, sizeof expected - 1);
}
''')
    runtime = Path(__file__).resolve().parents[1] / "native" / "typed_preview.c"
    output = root / "overflow-adapter-test"
    built = command([clang, "--no-default-config", "--target=x86_64-unknown-linux-gnu", f"--ld-path={lld}", "-std=c11", "-Wall", "-Wextra", "-Werror", str(harness), str(runtime), "-Wl,--wrap=write", "-Wl,--wrap=signal", "-Wl,--wrap=__sysv_signal", "-Wl,--wrap=_exit", "-o", str(output)])
    assert built.returncode == 0, built
    for mode in range(5):
        result = command([str(output), str(mode)])
        assert (result.returncode, result.stdout, result.stderr) == (0, b"", b""), (mode, result)


def verify(binaries):
    binaries = [str(Path(binary).resolve()) for binary in binaries]
    count = {"source_cases": 0, "binaries": len(binaries), "compiled_artifacts": 0, "success_cases": 0, "overflow_cases": 0}
    with tempfile.TemporaryDirectory(prefix="oxid-native-arithmetic-") as directory:
        root = Path(directory)
        # Exercise diagnostic and LLVM data escaping, UTF-8, CRLF and columns.
        name = 'source 雪;\'"\\\n\t\x1b.ox'
        path = root / name
        clean_env = {"PATH": "/no-tools", "LC_ALL": "C"}
        tested_io = False
        for number, (source, result_type, value, error) in enumerate(cases()):
            path.write_bytes(source.encode())
            previous = None
            previous_artifact = None
            for profile, binary in enumerate(binaries):
                reference = command([binary, "run", name, "--edition=typed-preview"], cwd=root)
                json_run = command([binary, "run", name, "--edition=typed-preview", "--message-format=json"], cwd=root)
                records = [json.loads(line) for line in json_run.stdout.splitlines()]
                assert not json_run.stderr
                if error is None:
                    expected_text = "()" if result_type == "()" else str(value).lower()
                    expected = (0, (expected_text + "\n").encode(), b"")
                    assert len(records) == 1 and records[0]["success"], (source, records)
                    expected_result = {"type": "unit"} if result_type == "()" else {"type": result_type, "value": value}
                    assert records[0]["result"] == expected_result, (source, records)
                else:
                    assert len(records) == 2 and not records[1]["success"] and records[1]["result"] is None, (source, records)
                    assert records[0]["code"] == "E0604" and records[0]["stage"] == "oir-run", (source, records)
                    primary = records[0]["primary"]
                    offset = len(source[:error].encode())
                    assert (primary["start"], primary["end"]) == (offset, offset + 1), (source, error, records)
                    assert primary["line"] == source[:error].count("\n") + 1
                    assert primary["column"] == len(source[:error].rsplit("\n", 1)[-1]) + 1
                    expected = (1, b"", reference.stderr)
                    assert reference.stderr.startswith(b"error[E0604] (oir-run): checked i32 arithmetic overflow\n  --> ")
                    # No raw path controls; only the two rendered line endings.
                    assert reference.stderr.count(b"\n") == 2 and b"\x1b" not in reference.stderr
                assert (reference.returncode, reference.stdout, reference.stderr) == expected, (source, reference)
                assert json_run.returncode == expected[0]
                if previous is not None:
                    assert (expected, records) == previous, (source, "compiler-profile mismatch")
                previous = expected, records
                output = root / f"native-{number}-{profile}"
                compiled = command([binary, "compile", name, "--edition=typed-preview", "--backend=llvm", "--output", str(output), "--message-format=json"], cwd=root)
                assert compiled.returncode == 0 and compiled.stderr == b"", (source, compiled)
                summary, = [json.loads(line) for line in compiled.stdout.splitlines()]
                assert summary["kind"] == "compile-summary" and summary["success"] and summary["output"] == str(output)
                assert not list(root.glob(".oxid-native-*"))
                # Artifact execution has no source file, interpreter, LLVM path,
                # LD_LIBRARY_PATH, Python path or compiler environment available.
                path.unlink()
                native = command([str(output)], cwd=root, env=clean_env)
                path.write_bytes(source.encode())
                assert (native.returncode, native.stdout, native.stderr) == expected, (source, native, expected)
                artifact = output.read_bytes()
                assert artifact[:4] == b"\x7fELF"
                digest = hashlib.sha256(artifact).digest()
                if previous_artifact is not None:
                    assert digest == previous_artifact, (source, "artifact profile mismatch")
                previous_artifact = digest
                if error is not None and not tested_io:
                    dynamic = command(["readelf", "-d", str(output)])
                    needed = [line for line in dynamic.stdout.splitlines() if b"(NEEDED)" in line]
                    assert dynamic.returncode == 0 and len(needed) == 1 and b"[libc.so.6]" in needed[0], dynamic
                    with open("/dev/full", "wb") as full:
                        failed = subprocess.run([str(output)], stdout=subprocess.PIPE, stderr=full, env=clean_env, timeout=10)
                    assert failed.returncode == 74 and failed.stdout == b"", failed
                    read_fd, write_fd = os.pipe()
                    os.close(read_fd)
                    try:
                        broken = subprocess.run([str(output)], stdout=subprocess.PIPE, stderr=write_fd, env=clean_env, timeout=10)
                    finally:
                        os.close(write_fd)
                    assert broken.returncode == 74 and broken.stdout == b"", broken
                    tested_io = True
                output.unlink()
                count["compiled_artifacts"] += 1
            count["source_cases"] += 1
            count["success_cases" if error is None else "overflow_cases"] += 1
        assert tested_io
        verify_adapter(root)
    print(json.dumps(count, sort_keys=True))
    print("native checked i32 O0: Python/reference/native parity, first-error origins, compiler-profile artifact parity, standalone ELF, stderr failures and injected adapter failures: PASS")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    verify(sys.argv[1:])
