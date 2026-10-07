# Private checked-HIR Emit native gate

This opt-in, Linux x86_64 test gate consumes the owned artifact from the complete
private checked-HIR Emit leaf. It adds no production entry point, public provider,
default-route change, or qualification-binding update. Normal Rust tests keep the
exporter ignored and never invoke LLVM or a generated executable.

## Contract

The exporter uses the genuine committed rich source/capture and the existing
explicit hand-authored overflow/division controls. No observation is generated
from compiler facts or repaired. Source/OPA/canonical-HIR comparison, candidate
construction, real checking, full STF comparison, lowering, association, independent
OIR verification and bounded native serialization all precede export.

Actual source map/path/text, AST and ordinary checked-program owners leave scope
before either owned artifact is saved. Private and ordinary text must be identical;
the final artifact retains the same capacity, paid-work and single-final-reserve
assertions as the text gate. External processes are outside that private tariff.
The existing ownership, allocation-failure and budget tests remain separate.

The ordinary interpreter is an additional reference, checked against frozen
outcomes: rich returns i32 1; overflow is E0604 at 32..33; division by zero is E0607
at 23..24. The two errors have frozen messages and stage. Complete human diagnostic
bytes are saved while the source map still exists, then compared with native
status/stdout/stderr. Synthetic controls are not producer-correctness evidence.

## Invocation

Build the unit-test binary from a clean commit with the selected Rust toolchain,
retaining a build receipt and the actual executable path from Cargo:

```sh
cargo test --bin oxid --no-run --locked
python3 scripts/verify_checked_hir_import_native.py \
  --test-binary /absolute/path/to/the/oxid-test-executable \
  --llvm-bin /absolute/path/to/llvm-19/bin \
  --evidence /absolute/path/to/a/new/evidence-directory
```

The evidence directory must not exist and must be outside the checkout. Required
trusted tools are LLVM assembler, opt, Clang and LLD 19.1.7. The repository's CI
pins Debian packages to 1:19.1.7-3+b1. Tool hashes and version streams are retained.
A prebuilt test-binary hash is recorded; a current Git head by itself cannot prove
an arbitrary supplied binary was built from that head. Keep the matching build
receipt with the run. The driver rejects dirty checkouts and changed Git identity
or executable bytes across the run.

Each unchanged private and ordinary artifact is separately assembled, verified,
compiled and linked with the existing O0 Linux x86_64 PIE/runtime flags. The driver
checks ELF64/little-endian/x86_64/PIE headers, executes with empty stdin, a clean
environment and PATH=/no-tools, and compares all three process outputs. The run
working directory contains only its executable. This demonstrates source-free
runtime dependencies, not OS filesystem sandboxing. A disposable malformed LLVM
copy must be rejected by opt with a real diagnostic; the original remains intact.

Every command uses argv directly, never a shell. Export, tool and runtime steps
have explicit timeouts; a timeout kills the process group, reaps its leader and fails the gate.
Child-created files have a 64 MiB per-file limit and core dumps are disabled.
Streams are retained on disk, and in-process reads of process streams are capped at 1 MiB. These are
qualification-runner controls, not a process-wide memory/disk or compiler-work
bound. Tool failures, unexpected streams, signals and timeout results never count
as expected language errors. The manifest hashes retained inputs, outputs,
executables, runtime source, driver/exporter and command receipts.

This gate directly exercises the emitted text and external LLVM, not production
CLI routing or its atomic executable-publication behavior. It does not qualify
other hosts, release/debug profiles, public imports or rebased current-source
seals. Hosted verification and those decisions remain separate milestones.
