# Fixed scalar arrays: public-route validation

Status: experimental, local public-route evidence. This is not a stable feature,
M2 completion, a complete v1.0 qualification, or a replacement for historical
staged reports. The change starts from PR29 commit
`c9d97cc610cdc712cea21eee281ec1f12f480d79` (source tree
`638bfae8bfc6995bf982b66349595918dd288082`). Hosted CI applies only to the exact
published head it tests; publication and final CI status are recorded by the PR.
Array frontend implementation bytes match activation commit
`9a09e5fb856a51548a1ee443c6fa27214b317f8c`; the final local test source additionally
asserts the pilot's sequence-check insertion marker occurs exactly once. Its
SHA256 is recorded below.

## Production path and preserved boundaries

Only `ProjectSources::load_typed` enables the bounded array parser policy. The
ordinary closed `parse_counted` interface remains available to its existing
callers. Original-file and linked-project ownership selectors already recognize
array syntax, including unused child declarations; both reach the ordinary
owned resolution, typing, lowering, source association and sealed verifier.

Production raw admission now accepts valid array carriers/operations only after
the complete authoritative shape, CFG, initialization, ownership, whole-loan and
permission checks. The previous carrier-inventory verification charges remain;
no source, verifier, runtime or native ceiling was increased. Malformed
unreachable array operations still reject before a witness can escape.
Observation-only typed values and the private `ArrayConsumer` admission remain
immutable: their tests still reject promotion to `SourceProgram` with E0500.

The activation removed the old whole-feature rejection, not the scalar frontend
routing guard or the independent validators. Old tests asserting that every raw
array must fail were updated to require valid-array admission and exact
malformed-ID/type/binding rejection. Historical evidence and archived fixtures
remain records of their original source identities.

A full ordinary-suite run caught an unintended scalar-diagnostic regression in
the previously dormant parser: `(x) = 2` became E0101 at `(x)`. Activation now refines
only array-shaped assignment targets. Scalar/record invalid targets keep E0100,
the existing statement-terminator message, at `=`; the pre-existing mutable suite
is unchanged.
A new four-program public check/run/compile test asserts those exact origins.

## Direct public source evidence

`tests/typed_fixed_arrays.rs` launches the real `oxid` binary against ordinary
source files. It uses no candidate parser, observer or private consumer selector.
Its current **12 ordinary tests** cover:

- One end-to-end check/run smoke case and 13 literal result oracles for bool,
  i32, unit, empty arrays, moves/replacement/self-moves, borrowing/reborrowing,
  RHS-before-index snapshots, left-to-right literal effects, loops/continue,
  short-circuiting and the ordinary record field named `len`
- 19 negative programs through check/run/compile, with exact error code, stage
  and byte origin, including narrow empty-literal inference, identity mismatch,
  immutable writes, moved zero/unit arrays, moves during an index expression,
  active-loan length reads and shared-reference writes
- Excluded grammar, decimal length restrictions, missing punctuation and
  257/1024-element successful literals versus a 1025-element parse failure; the
  source-valid 1024-element case still rejects native compilation with E0700
  before tools because the native slot ceiling is narrower
- Eight executed bounds/arithmetic failures with literal expected human
  diagnostics, including a Unicode comment, complete indexed-store origins,
  empty unit-array indexing and RHS failure before index evaluation
- Array-returning/nonzero-argument/missing mains accepted by check but rejected
  by run/native admission; default and explicit legacy dynamic-array controls
- Three-module structural identity, unused-child routing and exact child-file
  ownership origins, and original-root-main requirements
- The complete sample sequence `[5,7,0,9,4,0,0,0]`, count 5, sum 25 and result
  5325, checked independently rather than relying only on a weighted checksum
- E0601 exhaustion of an indexed-access loop and the scalar diagnostic regression

Two explicit native tests additionally exercise **23 real public CLI
compilations and source-free ELF executions per Rust profile**. The first uses
13 literal output oracles and eight exact runtime-error oracles. The second
checks the three-module sequence/pilot and reference/native E0601 parity.
The array source files are removed before executing each native binary; programs
run with a cleared environment. This does not remove their host libc/ELF-loader
dependency. The fuel-parity case is a consumer comparison, not a separately
hand-counted fuel oracle.

## Commands and local scope

The local run uses Linux x86_64, Rust 1.99.0 (`b940084d7`, 2026-09-28), and
LLVM/Clang/LLD 19.1.7. Native emission remains O0, independently of whether Rust
builds the compiler in debug or release mode.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --locked --test typed_fixed_arrays -- --include-ignored
cargo test --release --locked --test typed_fixed_arrays -- --include-ignored
cargo build --release --locked
python3 scripts/verify_feature_status.py
```

The two `--include-ignored` runs require the supported native host and pinned
LLVM toolchain. Ordinary portable test runs intentionally do not launch those
native gates. The local result totals are recorded after final execution below;
no other host or optimization level is qualified by this report.

Local results on 2026-10-04 UTC:

- Formatting, all-target Clippy with warnings denied, release build and feature
  inventory validation passed
- The array-activation `cargo test --all-targets --all-features --locked` run:
  **958 passed, 0 failed,
  31 ignored**; this is the ordinary Rust suite, not a total of all historical
  qualification packages
- Public array integration including both native gates: **14 passed, 0 failed**
  in debug and **14 passed, 0 failed** in release; each includes the 23 actual
  source-file CLI compilations/source-free native executions described above
- Both existing immutable observation/consumer-admission fence tests passed;
  the unchanged mutable public suite passed all 11 tests
- The checked-in three-module fixture itself checks as nine functions and runs
  to `5325`; the augmented integration version also checks every final element

Exact final test source SHA256: `4edd98bf943e3b00e6ae3370b26cd091f3d0ab87d38d8e228832ed1215676955`.
The reviewed fixture-discovery change was subsequently composed with this
activation, preserving its `src/main.rs` bytes and registering only the three
new sample files as one typed project. That combined ordinary Rust run passed
**972 tests, 0 failed, 31 ignored**; both 14-test public/native profile runs passed
again. The final added native-limit assertion was also exercised in both profiles.
Seven relevant Python registration tests passed. Release repository verification
passed **128 language sources, 124 checks and 70 runnable programs**, including
seven typed source members and three typed entry runs. Its 122 source-only data
files were manifest/body validated only, not executed as language tests.

The historical private source-consumer/native evidence remains separate from
these newly exercised public routes. No current hosted-CI status is asserted here.

The current [language contract](../../spec/typed-preview.md#fixed-scalar-arrays),
[native restrictions](../../spec/native-preview.md) and
[RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md) remain authoritative.
