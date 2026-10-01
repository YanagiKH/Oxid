# Native checked i32 arithmetic: local validation

Scope: experimental extension of scalar-native PR #11, based on merged main
`20047f72c920cfe0cc6851b5c0ecd8347635728a`, whose
`ee0176119fab8287326094c4e3256719ee74ea88` tree is identical to the reviewed PR. The
[native contract](../../spec/native-preview.md) and
[RFC 0008](../../rfcs/0008-native-checked-i32.md) bound the claim. This report is
local Linux x86_64 evidence, not independent-review sign-off, remote CI, production
certification, a new stable ABI or completion of M2/M3. Legacy behavior is unchanged.

## Environment

2026-10-01, Linux x86_64, Rust/Cargo 1.98.1, Debian 13/glibc 2.41, with the same
LLVM/Clang/LLD 19.1.7 packages and host C environment qualified in the
[scalar predecessor report](native-preview-validation.md). Both debug and release
Oxid compilers still invoke the native backend at explicit `-O0`. No `-O2`, LTO,
cross-target or non-Linux native result is claimed. Generated executables depend
on libc and the ELF loader, with no Oxid/Rust/LLVM/Python runtime dependency.

## Tests and independent expectations

- `src/frontend/oir/native.rs`: signed-overflow intrinsic selection, overflow
  branches before exposing values, numeric local/global names, no unchecked
  arithmetic flags, exact reference human-diagnostic bytes, and escaping of UTF-8,
  quotes, backslashes and control characters. Admission counts an arithmetic
  assignment once; `return 1 + 2;` needs exactly eight inclusive reference fuel.
  Dead branches and three repeated helper calls retain conservative costs
- `tests/typed_native.rs`: arithmetic now reaches tool selection instead of E0700,
  including unused functions, unchosen overflowing branches and direct overflow.
  The test was demonstrated failing against the scalar predecessor before the
  allowlist change. Existing missing/bad tools, recursion/resource rejection,
  no-clobber race/symlink/path handling and failure cleanup tests remain required
- `scripts/verify_native_arithmetic.py`: independent Python arbitrary-precision
  expression model shared with the reference arithmetic oracle; per-node range
  checks and source-order first-error positions. It covers all 13-by-13 boundary
  pairs for each of three operators, 200 random call-wrapped operand pairs,
  300 generated expression trees, precedence/associativity, MIN/MAX neighbors,
  branch choices, unused overflow, unit/bool results, discarded expressions and
  calls, argument/operand order, conditions that overflow, and identities that
  must not erase intermediate overflow

For each source and each supplied compiler build, the new script checks reference
human output and JSON against Python results/operator byte offsets, compiles with
real opt/Clang/LLD, deletes the source temporarily, and executes with a cleaned
environment whose PATH has no tools. Native stdout/stderr/exit must exactly match
reference human behavior. Debug/release JSON and complete native artifact hashes
must agree. Filenames contain Unicode, shell metacharacters, quotes, backslash,
newline, tab and ESC; source contains Unicode and CRLF. Each artifact is ELF;
a representative overflow artifact is checked for a single libc dependency.

Real stderr `/dev/full` and broken-pipe tests require status 74 and empty stdout.
The actual C overflow adapter is separately linked to controlled write/signal/
exit wrappers covering one-byte partial writes, EINTR, zero progress, EIO and
signal-setup failure. The wrappers verify stderr selection, bytes and exit status.
The scalar predecessor's 57 real-artifact cases continue covering stdout I/O,
ELF PIE/libc, 32 frames/64 parameters at a 1 MiB stack, and repeat builds.

## Reproduction

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
python3 -m unittest discover -s scripts -p 'test_*.py' -v
cargo build --release --locked
python3 scripts/verify_native_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_preview.py target/release/oxid
python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_i32_literals.py target/release/oxid
python3 scripts/verify_repo.py target/release/oxid
```

Set `OXID_LLVM_BIN` and, for the workspace-extracted toolchain only, its LLVM
`LD_LIBRARY_PATH` as in the predecessor report. Missing LLVM tools fail the real
native gates rather than silently skipping. Ordinary Cargo tests require no LLVM.
The pinned native CI job also invokes the new differential script with both
compiler profiles; workflow configuration is not evidence of remote execution.

## Local results

- Formatting and Clippy with warnings denied: passed
- Complete Cargo all-target/all-feature suite: 286 passed in debug and 286 in
  release (204 binary unit tests and 82 integration tests in each profile)
- Metadata unit tests: 7 passed; feature inventory: 19 entries valid
- Release compiler build: passed
- New real native arithmetic gate: 1,051 source cases, 605 successful results and
  446 first-overflow results; 2,102 LLVM artifacts across both compiler profiles
  passed Python/reference/native comparison and paired SHA-256 artifact equality
- Overflow adapter: all five controlled write/signal/exit modes passed; real
  stderr `/dev/full` and broken-pipe cases returned 74 with empty stdout
- Scalar predecessor gate: 57 real compiled cases plus printer fault injection passed
- Reference arithmetic oracle: 4,100 debug/release invocations over 1,025 sources passed
- Exact literal oracle: 1,670 invocations passed
- Full repository verifier: 121 sources and 67 runnable programs passed
- Workflow YAML parsed locally; this increment's remote CI has not run

For count reconciliation, `--list` on every untouched predecessor test binary
reported 201 unit + 82 integration = 283 tests; the candidate adds three unit
tests and retains every integration test (one admission regression is renamed
and updated). The predecessor report's earlier 293 headline was a counting error,
not removed coverage. Both actual complete candidate execution logs report 286.

## Residual limits

No optimizations above O0, host portability, sanitizer/fuzz campaign, performance
superiority, hostile-tool isolation, ownership proof, native recursion, destruction/
unwinding contract or recoverable exceptions are claimed. Diagnostic source paths
are fixed compile-time data and may be visible in the executable. Compiler memory,
LLVM/tool execution, stack-byte guarantees, output blocking, host signals and
allocation/I/O failure remain outside the reference-fuel model. A diagnostic I/O
failure can leave partial stderr. Independent review and remote CI remain separate.
