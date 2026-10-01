# Checked i32 arithmetic validation

Date: 2026-10-01. Scope: ordinary i32 binary addition, subtraction and
multiplication in experimental typed-preview checking/reference execution.
[RFC 0006](../../rfcs/0006-checked-i32-arithmetic.md) and the
[preview specification](../../spec/typed-preview.md) define the contract.
No native support, ownership, full numeric system, Rust compatibility, stable
edition or completed M2/M3 milestone is claimed.

## Snapshot and host

- Base commit: `53edb751c57a3b5c2ddda4f0fe468e35287ee440`
- Base tree: `bfd4e79d223fb6a4f91f993eee34fabe1f778a41`
- Candidate: base plus this increment; no merge/publication/remote-CI claim
- Code/test manifest SHA-256: `6bd549b77d1c7d497c2f839e943f57373bc78af8aee9202cdb7fae361e836db4`
- Manifest consists of all `src/**/*.rs`, `tests/*.rs`, and
  `scripts/verify_i32_arithmetic.py`, sorted relative paths mapped to SHA-256,
  encoded as Python `json.dumps(map, indent=2) + "\n"`. Documentation and CI
  configuration are outside this code/test digest
- Actual host: Linux x86_64, x86_64-unknown-linux-gnu
- Rust 1.98.1 (48a229cea 2026-09-01); Python 3.12.14; one Cargo build job
- No Cargo dependency, legacy runtime/OXBC, default edition or README changes

## Acceptance-to-evidence map

| Contract | Evidence |
| --- | --- |
| Precedence, grouping, same-level left associativity | Public `typed_arithmetic` cases; hand-derived oracle expressions |
| MIN/MAX and adjacent overflow, negative products, multiplication threshold | CLI boundaries and full 13 × 13 × 3 bigint edge matrix |
| Runtime overflow independent of host profile | Identical debug/release JSON and exits for 1,025 sources; explicit checked integer operations |
| Full-file check without arithmetic evaluation | Overflowing expressions pass check; type/literal errors in dead code still fail |
| Left-to-right evaluation, exactly-once calls, first error | Source call/return event trace; left-overflow/right-recursion and reversed tests |
| No algebraic shortcut/reassociation | MAX + 1 - 1 and zero-times-overflow tests fail at inner/first operator |
| Branch selection, discarded arithmetic, isolated recursive frames | Chosen/dead arm cases, unit/bool main, discarded overflow, recursive arithmetic |
| Strict types and unchanged unary/spelling rules | Both operand positions, bool/unit combinations, existing i32 negative corpus |
| Exact provenance through every representation | Unicode/CRLF AST/HIR/OIR span checks; CLI/oracle operator byte and scalar-column checks |
| Mandatory raw-IR validation | Wrong operand/destination types and IDs, self/undefined/forward reads, bypassed call definitions, invalid operator/operand origins |
| Fuel before computation and no stale state | Exact eight-fuel successful source, six-fuel pending assignment, seven-fuel overflow; repeated invocations |
| Host-stack and compiler bounds | 64-term accepted/65-term rejected flat chains; 20,000-term hostile input; mixed call/group/binary height boundaries; prior resource suite |
| Compatibility | Entire predecessor/legacy/edition/source/repository gates; unchanged legacy/runtime bytes |

The Python oracle evaluates arbitrary-precision mathematical operations and checks
the i32 range at each independently generated expression node. It is separate
from production parsing, HIR/OIR lowering, Rust checked operations and legacy f64.
It compares both successful integer JSON values and the first overflowing
operator's exact location. Its 1,025 cases comprise 507 complete edge pairs,
200 seeded full-range pairs with calls, 300 seeded structured trees, eight
hand-derived precedence/spelling cases and ten branch-selection cases. They yield
602 exact successful results and 423 overflows. Both check and run are compared
across two binaries: 4,100 timeout-bounded subprocess invocations. No generated
source is added to repository `.ox` discovery.

## Verification record

All commands below were run against the integrated local candidate. Every
listed command exited 0. CI now runs the bigint/profile-parity oracle after the
Linux optimized build; this configuration is not a remote CI result.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `rustfmt --check --edition 2021 src/legacy/syntax.rs` | Pass |
| `cargo clippy --all-targets --all-features -- -D warnings` | No warnings |
| `cargo test --all-targets --all-features --locked` | 268 Rust tests pass |
| `cargo test --release --all-targets --all-features --locked` | Same 268 Rust tests pass optimized |
| `cargo build --release --locked` | Optimized binary builds |
| `python3 -m unittest discover -s scripts -p 'test_*.py' -v` | 7 tests pass |
| `python3 scripts/verify_feature_status.py` | 19 experimental/inventory entries valid |
| `python3 scripts/verify_i32_literals.py target/release/oxid` | Predecessor oracle: 1,670 invocations pass |
| `python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid` | 4,100 invocations, 1,025 cases, identical profiles |
| `python3 scripts/verify_repo.py target/release/oxid` | 121 sources, 67 runnable programs, 19 inventory entries pass |
| `git diff --check` | Pass |

Rust totals in each profile: 198 unit, 12 edition-boundary, one generated-document,
eight legacy-module, eight legacy-semantic, six typed-arithmetic, eight
scalar-execution, 19 typed-frontend and eight typed-i32. All 255 predecessor tests
remain; 13 tests are added.

## Resource and compatibility details

CheckedI32 has two fixed operands and one operator origin. It adds no dynamic
executor scratch, scalar representation or frame-state fields. Runtime slot,
frame and argument storage layouts are unchanged from the literal increment.
Each new binary expression adds one compiler node, one OIR temporary and one
assignment, retaining the same aggregate preflight formulas. Arithmetic assignment
costs one fuel, charged before reads/checking; both operand subtrees retain their
own costs. A three-slot `return 1 + 2;` program costs exactly eight fuel including
root/return. Six fuel stops before its arithmetic assignment; seven fuel permits
an overflow error or stops before a successful return. Overflow never initializes
the destination and returning an error discards the private activation stack.

Total expression-tree height is bounded to 64 in addition to the previous parser
recursion bound. Height tracking adds one usize per expression during parsing,
at most 800,000 bytes on the tested 64-bit host excluding Vec capacity. This
bounds recursive resolution even for flat left-associative trees without changing
previously accepted non-arithmetic source. Existing compile/runtime limits and
failure precedence remain; they are not a host allocation-success, whole-process
memory bound, security guarantee or OS sandbox.

## Development evidence and limits

Unchanged main passed all 255 baseline Rust tests. The initial five public
arithmetic test groups then failed for the intended unsupported-operator behavior
(0/5, exit 101). After the narrow vertical slice all five passed. Raw-IR,
exact-fuel, oracle and the additional grammar/depth group were added after that
slice; they are not represented as individually red-first.

Only three obsolete arithmetic rejection cases (+, -, *) were removed from the
literal predecessor's negative list; its unary-minus, unary-plus, literal range,
spelling, division, remainder, comparison and cast regressions remain.

Remote CI, Windows/macOS/Linux ARM64, installation/container, sanitizer,
hardware, security review and native machine-code behavior are not established
by these local tests. Native admission/implementation is a separate increment;
broader arithmetic, wrapping APIs, ownership and roadmap work remain deferred.
