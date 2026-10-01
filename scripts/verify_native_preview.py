#!/usr/bin/env python3
"""Real LLVM/Clang/LLD differential and artifact gate, never a mocked backend.

Requires Linux x86_64, LLVM 19.1.7 (OXID_LLVM_BIN or versioned PATH tools),
readelf, and the host C development environment. Missing tools are failures.
"""
import hashlib
import json
import os
from pathlib import Path
import random
import resource
import shutil
import subprocess
import sys
import tempfile


def command(args, **kwargs):
    return subprocess.run(args, capture_output=True, timeout=30, **kwargs)


def verify(binary):
    binary = str(Path(binary).resolve())
    rng = random.Random(70319)
    cases = []
    for value in [-(2**31), -1, 0, 1, 2**31-1]:
        cases.append((f"fn main() -> i32 {{ return {value}; }}", f"{value}\n"))
    for value in ["true", "false"]:
        cases.append((f"fn main() -> bool {{ return {value}; }}", value + "\n"))
    cases.append(("fn main() -> () { return; }", "()\n"))
    # Independent Python value selection provides a third oracle beyond OIR run.
    for _ in range(48):
        a, b = rng.randint(-(2**31), 2**31-1), rng.randint(-(2**31), 2**31-1)
        flag, outer = rng.choice([True, False]), rng.choice([True, False])
        boolean = lambda v: str(v).lower()
        text = f"""
fn write(value: i32) -> i32 {{ let copied = value; return copied; }}
fn main() -> i32 {{
  let flag = flip({boolean(not flag)});
  unit_sink(());
  if {boolean(outer)} {{ return choose(flag, write({a}), write({b})); }}
  else {{ let discarded = choose(true, {a}, {b}); }}
  return choose(flip(flag), write({b}), write({a}));
}}
fn unit_sink(value: ()) -> () {{ return value; }}
fn flip(value: bool) -> bool {{ if value {{ return false; }} else {{ return true; }} }}
fn choose(flag: bool, a: i32, b: i32) -> i32 {{ if flag {{ return a; }} else {{ return b; }} }}
"""
        cases.append((text, f"{a if flag else b}\n"))
    # Maximum supported call depth and argument count, with copied locals live
    # across calls. Compiler flags prohibit inlining; run with a 1 MiB stack.
    params = ",".join(f"p{i}: i32" for i in range(64))
    uses = ",".join(f"p{i}" for i in range(64))
    values = ",".join(str(i) for i in range(64))
    chain = f"fn main() -> i32 {{ return f0({values}); }}"
    for i in range(31):
        copies = " ".join(f"let copy{j} = p{j};" for j in range(63))
        tail = "p63" if i == 30 else f"f{i+1}({uses})"
        chain += f"fn f{i}({params}) -> i32 {{ {copies} return {tail}; }}"
    cases.append((chain, "63\n"))
    with tempfile.TemporaryDirectory(prefix="oxid-native-real-") as directory:
        root = Path(directory)
        for i, (source, expected) in enumerate(cases):
            # Shell metacharacters and control bytes are ordinary path bytes.
            path = root / f"source {i};'\"\n.ox"
            output = root / f"native {i};'\"\n"
            path.write_text(source)
            reference = command([binary, "run", str(path), "--edition=typed-preview"])
            assert reference.returncode == 0 and reference.stdout == expected.encode(), reference
            compiled = command([binary, "compile", str(path), "--edition=typed-preview", "--backend=llvm", "--output", str(output), "--message-format=json"])
            assert compiled.returncode == 0, compiled
            summary, = [json.loads(line) for line in compiled.stdout.splitlines()]
            assert summary["kind"] == "compile-summary" and summary["success"] and summary["output"] == str(output)
            assert compiled.stderr == b""
            assert output.read_bytes()[:4] == b"\x7fELF"
            def small_stack():
                resource.setrlimit(resource.RLIMIT_STACK, (1024*1024, 1024*1024))
            native = command([str(output)], preexec_fn=small_stack, env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"})
            assert (native.returncode, native.stdout, native.stderr) == (0, expected.encode(), b""), native
            dynamic = command(["readelf", "-d", str(output)])
            assert dynamic.returncode == 0
            needed = [line for line in dynamic.stdout.splitlines() if b"(NEEDED)" in line]
            assert len(needed) == 1 and b"[libc.so.6]" in needed[0], dynamic.stdout
            header = command(["readelf", "-h", str(output)]).stdout
            assert b"DYN (Position-Independent Executable file)" in header and b"Advanced Micro Devices X86-64" in header
            with open("/dev/full", "wb") as full:
                failed = subprocess.run([str(output)], stdout=full, stderr=subprocess.PIPE, timeout=10)
            assert failed.returncode == 74 and failed.stderr == b""
            read_fd, write_fd = os.pipe()
            os.close(read_fd)
            try:
                broken = subprocess.run([str(output)], stdout=write_fd, stderr=subprocess.PIPE, timeout=10)
            finally:
                os.close(write_fd)
            assert broken.returncode == 74 and broken.stderr == b""
            assert not list(root.glob(".oxid-native-*"))
        # Link-wrap the actual C adapter to deterministically exercise partial
        # writes, EINTR, zero progress, EIO and signal-setup failure.
        harness = root / "printer-test.c"
        harness.write_text(r"""
#include <errno.h>
#include <signal.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
int __oxid_print_i32(int32_t);
int __oxid_print_bool(int32_t);
int __oxid_print_unit(void);
static int mode, calls;
static size_t used;
static char captured[32];
ssize_t __wrap_write(int fd, const void *bytes, size_t n) {
    (void)fd; (void)n;
    if (mode == 1 && calls++ == 0) { errno = EINTR; return -1; }
    if (mode == 2) return 0;
    if (mode == 3) { errno = EIO; return -1; }
    if (used >= sizeof captured) return -1;
    captured[used++] = *(const char *)bytes;
    return 1;
}
void (*__wrap_signal(int sig, void (*handler)(int)))(int) {
    (void)sig; (void)handler;
    return mode == 4 ? SIG_ERR : SIG_DFL;
}
void (*__wrap___sysv_signal(int sig, void (*handler)(int)))(int) {
    return __wrap_signal(sig, handler);
}
int main(void) {
    for (mode = 0; mode <= 4; ++mode) {
        calls = 0; used = 0;
        int result = __oxid_print_i32(INT32_MIN);
        if (mode >= 2) { if (result != 74) return 1; }
        else if (result != 0 || used != 12 || memcmp(captured, "-2147483648\n", 12)) return 2;
    }
    mode = 0; used = 0;
    if (__oxid_print_bool(0) != 0 || used != 6 || memcmp(captured, "false\n", 6)) return 3;
    used = 0;
    if (__oxid_print_bool(1) != 0 || used != 5 || memcmp(captured, "true\n", 5)) return 4;
    used = 0;
    if (__oxid_print_unit() != 0 || used != 3 || memcmp(captured, "()\n", 3)) return 5;
    return 0;
}
""")
        llvm_bin = os.environ.get("OXID_LLVM_BIN")
        clang = str(Path(llvm_bin) / "clang") if llvm_bin else shutil.which("clang-19")
        lld = str(Path(llvm_bin) / "ld.lld") if llvm_bin else shutil.which("ld.lld-19")
        assert clang and lld, "LLVM 19.1.7 tools are required"
        printer = root / "printer-test"
        runtime = Path(__file__).resolve().parents[1] / "native" / "typed_preview.c"
        built = command([clang, "--no-default-config", "--target=x86_64-unknown-linux-gnu", f"--ld-path={lld}", "-std=c11", "-Wall", "-Wextra", "-Werror", str(harness), str(runtime), "-Wl,--wrap=write", "-Wl,--wrap=signal", "-Wl,--wrap=__sysv_signal", "-o", str(printer)])
        assert built.returncode == 0, built
        tested = command([str(printer)])
        assert tested.returncode == 0, tested
        # Same input, flags and exact toolchain yield byte-identical executables.
        second = root / "repeat"
        repeat = command([binary, "compile", str(path), "--edition=typed-preview", "--backend=llvm", "--output", str(second)])
        assert repeat.returncode == 0, repeat
        assert hashlib.sha256(second.read_bytes()).digest() == hashlib.sha256(output.read_bytes()).digest()
        # A success artifact cannot be overwritten, even by recompiling it.
        before = second.read_bytes()
        again = command([binary, "compile", str(path), "--edition=typed-preview", "--backend=llvm", "--output", str(second)])
        assert again.returncode == 1 and second.read_bytes() == before
    print(f"native preview: {len(cases)} real compiled cases; reference/Python parity; 32 frames/64 args at 1 MiB stack; ELF/PIE/libc; output errors; reproducibility: PASS")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: verify_native_preview.py <oxid-binary>")
    verify(sys.argv[1])
