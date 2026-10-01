# LLVM scalar native preview: local validation

Historical scalar-only evidence; superseded for arithmetic admission by
[the checked-i32 native report](native-arithmetic-validation.md). Counts and
rejection tests below describe the predecessor snapshot, not the current scope.

Scope: experimental candidate based on `a09318cb005075eeb6f56b423ec2fd43379a3e50`.
This is local evidence for the [native contract](../../spec/native-preview.md),
not independent review, remote CI, release certification, or completion of M3.
The original legacy 0.9 scope is unchanged. Merged reference arithmetic has broader
support; native operation admission remains an explicit scalar-copy allowlist.

## Environment and dependencies

Local run: 2026-10-01, Linux x86_64, Debian 13, glibc 2.41
(`2.41-12+deb13u3`), GCC 14.2.0 (`14.2.0-19`), Rust 1.98.1, Cargo 1.98.1.
Clang, opt and LLD report exactly 19.1.7. Clang/LLVM are optional external native
compile-time dependencies. Generated artifacts are dynamically linked ELF PIE;
readelf reports exactly one NEEDED library, `libc.so.6`, plus the system loader.
Oxid/Rust/LLVM/Python runtimes are not required by the generated executable.

The locally qualified packages were downloaded from `https://deb.debian.org/debian/`
after verifying trixie's InRelease signatures with the installed Debian archive
keyring. The Packages.xz size and SHA-256 were checked against the signed Release;
each archive was checked against that authenticated index before workspace-local
extraction. No package installation hooks or global system configuration were
used. Qualified package version for every row: `1:19.1.7-3+b1`.

| Package | Archive SHA-256 |
| --- | --- |
| clang-19 | bbb74458d782b7792b61b0eed92746df5e6d033778c045cf8c2b3835b7c4efc4 |
| libclang-common-19-dev | 4018e40e2d4144fe38c45822e68cca63c1bf6c7872c3073ebb262ad4b2f9b8bc |
| libclang-cpp19 | a7d2b6df51050834f6ea5c0d5e34869c2298b0bcdaa91eff17ba3db2cd545c47 |
| libllvm19 | db0d614d61345ca710fb73e75105f1ec6e38898fa7b3aa99fa7eb7d8b0a11d57 |
| lld-19 | 756a805e8811d527dd2b5417f016c9342b8631fa9c1ddcd56265c13ae33bf3a4 |
| llvm-19 | 0b1b7263f4f45e261099240fdbf00fc14b42b88410fd829e041bf83ea45866cb |

The compiler validates tool-reported upstream versions, not package signatures
or these hashes. Tool installations and the host C toolchain remain trusted.
CI declares a separate Debian trixie Linux job with these six package versions;
a workflow declaration is not evidence that the remote job has already run.

## Test surfaces

- `src/frontend/oir/native.rs`: admission recurrence, inclusive cap predicates,
  private numeric symbols/ABI, no undef/poison/overflow flags
- `tests/typed_native.rs`: public CLI requirements and summaries, malformed UTF-8,
  OXBC, bad syntax/type/range, entry checks, unused/mutual/constant-false recursion,
  count/argument/local/depth/fuel limits, missing tools, version mismatch,
  verifier/linker failure cleanup, no compile-time artifact execution, existing
  files/symlinks, syntactic directory output paths before any tool,
  checked-arithmetic OIR rejection (including unused code and unchosen branches),
  and a simulated output publication race
- `tests/edition_boundary.rs`: default/explicit legacy behavior remains unchanged;
  unapproved typed operations/options still fail before host effects
- `scripts/verify_native_preview.py`: mandatory real toolchain gate, 57 compiled
  source cases compared with reference execution and independent Python results;
  all bool/unit/i32 outputs, MIN/MAX, copies, source-order-independent calls,
  branches/joins, discarded calls, mixed signatures, library-like source names,
  and source/output paths with whitespace, quotes, semicolons and newline

Every real case invokes opt verification, Clang IR/object compilation, separate
runtime C compilation, and LLD linking through the public compiler command.
Every artifact is executed in a cleaned environment with a 1 MiB stack, checked
for x86_64 ELF PIE and libc-only dependencies, and tested for `/dev/full` and
closed-pipe exit 74. The maximal representative chain has 32 frames and 64
arguments. A repeated build is byte-identical on this exact host/toolchain.
The same C adapter is also linked with controlled write/signal wrappers to verify
partial writes, EINTR retry, zero progress, EIO, and signal-setup failure.
These are bounded local tests, not a portability or all-program stack theorem.

## Reproduction commands

Install the disclosed tools using a trusted distribution source and set
`OXID_LLVM_BIN=/path/to/llvm-19/bin` if versioned PATH executables are not present.
A workspace-extracted toolchain may also need its library directory on
`LD_LIBRARY_PATH`; this is only needed by the compiler tools, not generated code.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features --locked
python3 -m unittest discover -s scripts -p 'test_*.py' -v
cargo build --release --locked
python3 scripts/verify_native_preview.py target/release/oxid
python3 scripts/verify_i32_literals.py target/release/oxid
python3 scripts/verify_repo.py target/release/oxid
```

The directory-output regression was demonstrated failing before the fix: a
trailing slash passed path normalization and reached Clang. The fix rejects slash,
dot and dot-dot directory endings before any tool or output creation.

The initial native-route regression was first shown failing on the predecessor because typed compile
was unavailable and emitted check-summary. With this implementation the public
native route passes and produces compile-summary. Missing LLVM tools do not
silently skip the real integration gate. Ordinary cargo tests need no LLVM;
controlled tool doubles only test the process/publication failure boundaries.

## Local results

- Formatting and Clippy with warnings denied: passed
- Cargo all-target/all-feature tests: 283 passed (201 binary unit tests and 82 integration tests; corrected count)
- Metadata validator unit tests: 7 passed
- Release compiler build: passed
- Real native differential/artifact gate: 57 cases passed, plus printer failure injection
- Checked i32 arithmetic Python oracle: 4,100 debug/release invocations passed across 1,025 sources
- Existing i32 literal Python oracle: 1,670 invocations passed
- Full repository verifier: 121 sources and 67 runnable programs passed
- Workflow YAML: parsed locally; new remote native job has not run

## Residual limits

No non-Linux-x86_64 native target, container execution, Windows/macOS native
compilation, exhaustive fuzz campaign, sanitizer run, hostile-tool sandbox,
performance benchmark, ownership proof or independent code review is claimed.
OOM, reduced host stack limits, signals/abrupt termination, filesystem outages,
output blocking and tool installation compromise remain host failures. The
runtime can leave partial output on an I/O failure. General recursion and checked
native arithmetic are deliberately rejected rather than approximated.

Primary implementation references: [LLVM 19 language reference](https://releases.llvm.org/19.1.0/docs/LangRef.html),
[opt command guide](https://releases.llvm.org/19.1.0/docs/CommandGuide/opt.html),
and [Clang command reference](https://releases.llvm.org/19.1.0/tools/clang/docs/ClangCommandLineReference.html).
