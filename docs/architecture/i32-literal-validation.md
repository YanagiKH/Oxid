# Exact i32 literal validation

Date: 2026-10-01. Scope: literal-only ASCII decimal i32 constants, immutable
copies, parameters/results and bounded scalar execution through the existing
verified pipeline. [RFC 0005](../../rfcs/0005-exact-i32-literals.md) and the
[preview specification](../../spec/typed-preview.md) define the provisional
contract. No arithmetic overflow mode, native compilation, ownership,
Rust compatibility, stable edition or completed M2/M3 milestone is claimed.

## Snapshot and environment

- Base commit: `fe4737bd52c9cae8d3a2c7ee8f5616a67fd613c7`
- Base tree: `481f4f9191f5e4336536f77f3f70e4c1e90b5c15`
- Candidate: base plus this increment; no merge/publication/remote-CI claim
- Code/test manifest SHA-256: `3c0d1122cc086f78666012ae6726faa5bb96ff1b0695c96c821d69515cfd3726`
- Manifest: all `src/**/*.rs`, `tests/*.rs`, and
  `scripts/verify_i32_literals.py`, sorted relative paths mapped to file SHA-256,
  encoded as Python `json.dumps(map, indent=2) + "\n"`. Documentation and CI YAML
  are outside that code/test digest
- Host: Linux x86_64, `x86_64-unknown-linux-gnu`
- Rust: `rustc 1.98.1 (48a229cea 2026-09-01)`; Python 3.12.14
- Cargo builds use one job; no dependency/toolchain/legacy-runtime/OXBC changes
- README files and layout are unchanged; fixtures remain embedded or generated
  only in temporary directories, outside repository `.ox` discovery

## Verification record

The final integrated local commands are recorded below. The repository workflow
also runs the Python oracle after its optimized Linux build; that future gate
is not a claim that remote CI already passed.

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | Formatting passes |
| `rustfmt --check --edition 2021 src/legacy/syntax.rs` | 0 | Included legacy syntax checked explicitly |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | No warnings |
| `cargo test --all-targets --all-features --locked` | 0 | 255 Rust tests pass |
| `cargo build --release --locked` | 0 | Optimized standalone binary builds |
| `cargo test --release --bin oxid frontend::oir --locked` | 0 | 103 OIR tests pass optimized |
| `cargo test --release --test typed_i32 --test typed_execution --locked` | 0 | 16 public scalar-run tests pass optimized |
| `python3 -m unittest discover -s scripts -p 'test_*.py' -v` | 0 | 7 metadata tests pass |
| `python3 scripts/verify_i32_literals.py target/release/oxid` | 0 | 1,670 invocations pass: 502 literal, 13 spelling and 320 source cases |
| `python3 scripts/verify_repo.py target/release/oxid` | 0 | 121 sources, 67 runnable programs, 18 feature entries |
| `git diff --check` | 0 | No whitespace errors |

Debug counts: 191 unit, 12 edition-boundary, 1 generated-document, 8 legacy-module,
8 legacy-semantic, 8 typed-execution, 19 typed-frontend and 8 typed-i32 tests.
All 238 predecessor tests remain, with 17 added. One predecessor unsupported
numeric fixture now uses a decimal-point spelling instead of an out-of-range
integer: exact integer range failures deliberately move to E0203/resolve.
The new suite preserves that large-integer rejection and its exact source span.

## Requirement-to-evidence map

| Contract or risk | Evidence |
| --- | --- |
| MIN/MAX, adjacent values, zero/sign canonicalization, long zeroes | Public `typed_i32` tests; Python arbitrary-precision expected values |
| Exact conversion, never f64 or integer wrapping | Checked signed accumulation; MIN/MAX ± 1 and 2^53/2^53+1 range tests; integer JSON assertions |
| Grammar rejection before magnitude conversion | Suffix/radix/separator/float/exponent/non-ASCII candidates including an overlong-magnitude suffix; literal-only-minus and cast tests |
| Full source provenance | AST digits/sign, HIR value/type, OIR local/assignment exact signed spans; Unicode comments and CRLF range diagnostics; EOF and diagnostic caps |
| Default i32 and strict context | Annotated/inferred lets, copies, parameters, return values; bool/unit/i32 mismatch and integer condition tests, wrong arity and unknown widths |
| Entry/result contract | Main before/after helpers; repeated exact text/JSON, negative and zero success exit 0; predecessor bool/unit/error summaries retained |
| Mandatory verifier | Raw constant destination, copy, argument, call result, return and branch-type corruptions; invalid UTF-8/overflowed origins; exact aggregate-local cap |
| Source equivalence | Independent structured-source values and ordered call/branch/return traces for 28 i32 tables, plus retained 68 bool/unit model fixtures |
| Complete-file check | Range errors in unchosen branches and unused functions precede execution |
| Resource behavior and isolated activations | Exact four-fuel source, every prior boundary, i32 recursive frame isolation, exact frame/slot caps and 256-argument values/order/charges |
| Compatibility | Previously legal `as` and `i32` identifiers remain usable; full legacy/edition/document/repository suites and unchanged legacy/README bytes |

The standalone Python oracle passed on both debug and optimized binaries: 502 literal cases,
13 invalid-spelling cases and 320 independently selected source programs, each
checked and run, for 1,670 timeout-bounded CLI invocations. It calculates numeric
expectations with Python integers and selection with a Python table, not Oxid's
resolver, OIR dispatcher or legacy f64 evaluator. JSON type is checked as an
integer (not bool or float). The separate Rust structured-source model evaluates
name environments and blocks before printing source, then compares source-level
call/branch/return traces with the verified runner. Seven rotations of boundary
values cover all four boolean input combinations. The model does not use OIR
slots or production lowering helpers to calculate its expectations.

## Storage and work contract

Measured on this host: `Scalar` = 8 bytes, `Option<Scalar>` = 8 bytes, `Frame` =
96 bytes, `Resume` = 40 bytes. Scalar/slot storage grew from the predecessor's
1-byte representation. The maximum frame-header vector is 1024 × 96 = 98,304
bytes, live slot storage is 200,000 × 8 = 1,600,000 bytes, and argument scratch
is 256 × 8 = 2,048 bytes: approximately 1,700,352 bytes plus small headers,
counters and allocator overhead. Compiled IR/source, compiler/verifier scratch,
test observations and host allocator behavior are excluded. These measurements
are not a portable ABI or a hard process-memory/host-OOM guarantee.

No largest-function-times-frame-cap preallocation is introduced. Counts remain
1,000,000 fuel, 1,024 frames and 200,000 live slots. Every i32 is one slot and
constant assignment costs one. The one-slot signed-MIN source costs exactly
four: root allocation two, literal assignment one, return one. Fuel two fails
at the complete signed literal; fuel three fails at its return. The 256-i32-
argument source costs exactly 1,031 and uses 514 live slots, matching its bool
counterpart. A 50,000-slot recursive activation admits four frames; 50,001
admits three before the next allocation fails. Source reads, allocation success,
compiler work and output remain outside execution fuel. No sandbox, security,
termination or memory-safety certification follows.

## Development evidence and limits

The initial six public CLI groups were observed red against the unchanged
bool/unit predecessor, then green after the exact-i32 vertical slice. Their
first harness version waited before draining pipes and timed out on a long
predecessor diagnostic; concurrent draining corrected that harness issue, and
the complete six-group baseline was rerun red (0/6 passing) for behavioral
reasons. Raw-IR, layout and oracle tests were added after the initial vertical
slice; they are not represented as individually red-first.

Review found an avoidable compatibility regression when an initial cast-rejection
implementation reserved `as`. A new test was recorded red, then green after
retaining ordinary identifier syntax and recognizing unsupported casts only in
already-invalid parser positions. An EOF test's hand-entered offset was corrected
to the original source byte length; production diagnostics were unchanged.
Final integrated commands above follow those corrections.

Local results do not establish Windows/macOS/Linux ARM64, installation/container,
remote CI, sanitizer, native-code, hardware or security results. All broader
numeric operations, ownership, native backend, typed artifact and roadmap work
remain separate. The legacy dynamic numeric behavior is unchanged.
