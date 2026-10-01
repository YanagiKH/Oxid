# Verified straight-line OIR local validation

Date: 2026-10-01. Scope: experimental check-only typed-HIR lowering to typed
locals, assignments, direct-call normal continuations and returns, followed by
mandatory independent structural verification. No completed roadmap milestone,
execution engine, native backend, ownership or memory-safety claim is made.

## Snapshot and host

- Base commit: `1e44c9dcbec251812f863f1286f2061e787eb2b2`
- Base tree: `31326715ae5d04477ecc926899630032839c8aba`
- Snapshot: base plus this increment's uncommitted source, tests and documentation;
  this report does not claim a publication commit or remote CI result
- Code/test manifest SHA-256:
  `25ec7d12d13a9bae85528271994daa8b7db529b9e16322baf061ce5ee02d2982`
- The manifest maps all `src/**/*.rs` and `tests/*.rs` relative paths, in sorted
  order, to SHA-256 hashes, encoded as Python `json.dumps(map, indent=2) + "\n"`.
  Documentation is outside this code/test digest
- Host: Linux x86_64, `x86_64-unknown-linux-gnu`
- Rust: `rustc 1.98.1 (48a229cea 2026-09-01)`; Python 3.12.14
- No dependency, toolchain, CLI-option, legacy-syntax or artifact-format changes

## Fresh integrated checks

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | Formatting passes |
| `rustfmt --check --edition 2021 src/legacy/syntax.rs` | 0 | Included legacy module checked explicitly |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | No warnings |
| `cargo test --all-targets --all-features --locked` | 0 | 175 Rust tests pass |
| `python3 -m unittest discover -s scripts -p 'test_*.py' -v` | 0 | 7 metadata tests pass |
| `cargo build --release --locked` | 0 | Release executable builds |
| `python3 scripts/verify_repo.py target/release/oxid` | 0 | 121 sources, 67 runnable programs, 15 feature entries |
| `cargo test --release --bin oxid frontend::oir --locked` | 0 | All 55 OIR tests pass with release checks active |
| `git diff --check` | 0 | No whitespace errors |

Debug counts: 131 unit tests, 12 edition-boundary tests, 15 typed-frontend tests,
1 generated-document test, 8 legacy-module tests and 8 legacy-semantic tests.
The previous 117-test suite is retained. New tests comprise 55 OIR cases, two
immutable typed-view cases and one driver diagnostic/status case. No fixture was
added to recursive legacy `.ox` discovery and no existing test was weakened.

## Production boundary and source semantics

The actual driver obtains `TypedProgram` from resolution/type checking, then
calls `oir::lower_and_verify`; its function count comes only from the verified
witness. Source-to-OIR tests exercise that same entry point. Raw-OIR fixtures
exercise the independent verifier without trusting a preceding type checker.

Exact assertions inspect nested `left`, `right`, then `combine` calls, argument
slots, result destinations, continuation IDs and the final return. Other fixtures
cover every accepted expression/statement kind, inferred/annotated locals,
local-to-local copies, discarded bool/unit calls, explicit/synthesized unit,
forward calls, direct/mutual recursion and empty files. Repeated lowering yields
identical internal IR. Two bodies reuse numeric local IDs with different types.

Provenance checks assert exact source slices for function/binding names, groups,
copy uses, arguments, full lets/returns, call continuations and bare-return unit
synthesis. The fixture uses Unicode comments, CRLF and a nonzero source-file ID.
Verifier span tests reject missing files, reversed/out-of-range offsets and both
endpoints inside multibyte scalars, while retaining empty EOF spans. Malformed
origins safely become null diagnostic locations rather than reaching the renderer.

A supplemental release CLI check processed a 135,060-byte source with 15,000
flat calls. An independent JSON decode found exactly the existing schema-1
successful summary with two functions, empty stderr and no extra project files.
This is a bounded regression exercise, not a compiler-performance benchmark.

## Malformed IR and resource evidence

Rejection cases cover missing/duplicate/out-of-order identities, parameter
counts/kinds, empty blocks, bad entries/continuations, missing terminators, all
operand/destination/target references, assignment/copy/call/return type errors,
wrong arity, uninitialized reads and self-copy/call-destination reads. Additional
cases reject parameter overwrites and repeat writes by assignments or calls,
late definitions, self/two-block cycles, valid unreachable blocks and malformed
unreachable blocks. A malformed forward callee is rejected before its signature
is used. A valid backward-indexed acyclic chain confirms control-flow order is
independent of block-table order. Builder tests reject double-close and append
after closure; two terminators cannot be represented in one raw block.

A 20,000-call raw chain passes; late invalid references and cycles fail. A
99,999-call chain reaches exactly 100,000 locals and blocks and passes. Checked
accounting tests cover each limit, limit + 1, overflow and unchanged counters on
failure without huge allocations; raw oversized locals/blocks/assignments/call
arguments are also rejected. Code inspection confirms linear passes with only
one local-initialization and one visited-block bitmap per function. No recursive
CFG traversal or per-block local-state cloning is used.

Lowering preflights exact aggregate expansion before any OIR allocation. Locals
correspond to counted parser parameters, expressions, lets or bare returns;
assignments to non-call expressions, lets or bare returns; blocks to functions
plus calls. The existing 100,000-node limit therefore bounds each IR total.
These limits preserve the current source subset but do not guarantee host
allocation success or provide an OS sandbox.

## Red/green record and diagnostic compatibility

The first actual-source-to-OIR case failed because a deliberate placeholder
lowerer produced zero functions. The missing-terminator case failed because a
placeholder verifier accepted the malformed program. Before implementing checks,
40 malformed-IR tests plus that source case all ran and failed for their intended
behavior against the stubs. An earlier test-authoring compile error was corrected
before recording that behavioral red run. Those 41 cases then passed with the
real lowerer/verifier; subsequent supplemental cases first ran green and are not
represented as test-first evidence.

The driver test preserves ordinary exit 1, maps E0500 internal failures to exit 2,
and asserts the failed schema-1 summary with null functions. OIR error tests
exercise safe human/JSON diagnostics, including invalid origins. Existing public
source errors, success records, E0302/E0303, unsupported `if` E0101, edition routing
and no-side-effect boundaries remain covered by the unchanged integration suites.

## Limits and deferred work

Only this Linux host was run locally. Windows, macOS Intel/ARM64, Linux ARM64,
installation and Docker were not rerun here; prior CI and configured jobs are not
new evidence. Publication CI requires separate checks on the published commit.

The result proves structural intraprocedural return completeness assuming each
call returns normally. It does not prove program termination, executable call
safety, source-to-IR equivalence from span bounds, user-recursion stack safety,
memory safety, ownership or a production-ready language. General branches,
scopes, joins/dataflow, numbers, execution, LLVM and typed artifacts remain
separate increments. See [RFC 0002](../../rfcs/0002-verified-straight-line-oir.md).
