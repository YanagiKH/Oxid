# Restricted boolean CFG local validation

Date: 2026-10-01. Scope: experimental bool/unit statement if/else, lexical arm
scopes, explicit all-path returns, actual Branch/Goto lowering and an independent
acyclic definition-dominance verifier. This is check-only and provisional;
[RFC 0003](../../rfcs/0003-boolean-branch-cfg.md) records the review boundary.
No completed M1/M2/M3 milestone, execution, native compilation, ownership,
memory-safety, self-hosting, AI or stable-edition claim follows.

## Snapshot and host

- Base commit: `dbd4bfe30bc4c453d7f27ff62e3116b6a0c5925d`
- Base tree: `bd27c396dfa3b37e61caaaaaada978dc11a20a77`
- The work began on the exact predecessor head `6ca44155abdc10d7d3c90e6b86fb42e8647acca1`
  and was moved to the same-tree merged base without changing any working files
- Snapshot: base plus this increment's source, tests and documentation; this
  report does not claim a publication commit or remote CI result
- Code/test manifest SHA-256:
  `25823cc96be3fddb441471c60df0595dc67448c3e1483d2f88a0a248abb18b3d`
- The manifest maps all `src/**/*.rs` and `tests/*.rs` relative paths, sorted,
  to SHA-256 hashes, encoded as Python `json.dumps(map, indent=2) + "\n"`.
  Documentation is outside the code/test digest
- Host: Linux x86_64, `x86_64-unknown-linux-gnu`
- Rust: `rustc 1.98.1 (48a229cea 2026-09-01)`; Python 3.12.14
- No dependency, Cargo/toolchain, runtime, legacy syntax, default edition,
  artifact, numeric, ownership or execution-path change

## Fresh integrated checks

| Command | Exit | Result |
| --- | --- | --- |
| `cargo fmt --all -- --check` | 0 | Formatting passes |
| `rustfmt --check --edition 2021 src/legacy/syntax.rs` | 0 | Included legacy module checked explicitly |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | No warnings |
| `cargo test --all-targets --all-features --locked` | 0 | 214 Rust tests pass |
| `python3 -m unittest discover -s scripts -p 'test_*.py' -v` | 0 | 7 metadata tests pass |
| `cargo build --release --locked` | 0 | Release executable builds |
| `python3 scripts/verify_repo.py target/release/oxid` | 0 | 121 sources, 67 runnable programs, 16 feature entries |
| `cargo test --release --bin oxid frontend::oir --locked` | 0 | All 78 OIR tests pass with release checks active |
| `git diff --check` | 0 | No whitespace errors |

Final debug counts: 166 unit tests, 12 edition-boundary tests, 19 typed-frontend
CLI tests, 1 generated-document test, 8 legacy-module tests and 8 legacy-semantic
tests. The predecessor's 175 tests remain, with 39 added. Its unsupported-if
fixture was intentionally replaced by a still-unsupported while fixture; new
positive/negative branch tests establish the changed source contract. The old
99,999-call boundary fixture now uses MAX_LOCALS rather than the enlarged block
limit, retaining its exact previous size. No fixture entered recursive legacy
`.ox` discovery. Initial Clippy found only a hand-written ceil division in the
wide-tree test; using `usize::div_ceil` fixed it, and the final integrated checks
above were rerun on that edit.

## Requirement-to-evidence map

| Contract or risk | Evidence |
| --- | --- |
| Optional brace-only else, strict bool, no constant pruning | Frontend and CLI positives; exact E0300 condition spans; both literal arms checked; rejected else-if, if-expression, naked blocks and semicolons |
| Lexical scope and no active shadowing | Sibling/closed-child reuse with distinct typed local IDs; E0200 leakage/self-initialization; E0201 original declaration labels |
| All-path explicit returns | Both/one/no returning arms, nested mixed paths, unit/bool cases, E0302 function-brace and E0303 following-statement ranges |
| Complete typed body structure | Arena spans/IDs, lexical expression order, full return-flow tables and corrupt-view boundary tests |
| Actual production CFG | Mandatory lower-and-verify tests; exact Branch/Goto/call IDs; no dead both-return join; real joins reserved before call continuations |
| Source provenance | Exact condition, full-if, arm, closing-brace Goto, synthetic join and return spans, including Unicode/CRLF and nonzero file IDs |
| Raw malformed IR | Branch type/init/targets, Goto IDs, new spans, existing reference/type/signature/terminator checks, cycles and unreachable blocks |
| Definition availability | Arm-only join reads, opposite-arm duplicates, outer overwrites, parameter reads, same-block ordering/self-use, call arguments and bypassed call results |
| Determinism and bounded storage | Repeated lowering/CLI records, independent small-DAG oracle, table permutations, long/wide resource cases and checked dimension arithmetic |
| Legacy/public CLI boundary | Full retained legacy and edition suites, repository verifier, unchanged schema-1 records and no-output/cache checks |

The source graph fixtures assert condition calls before Branch; distinct left,
right and after-join callees on the proper paths; Gotos only from falling-through
arms; and no synthetic unreachable block when both arms return. Nested source
scope and raw dominance are checked separately: a raw continuing-arm definition
can dominate a join even though that source arm's binding cannot escape.

## Independent oracle and resource exercises

The test oracle enumerates all 826 entry-reachable DAGs of one through six nodes
with out-degree at most two in a canonical topological labeling. It compares
73,438 assignment/call availability decisions under identity and reversed block
IDs, including nonzero entries. Expected assignment dominance is obtained by
removing a vertex and checking reachability. Expected call-result availability
is obtained by removing the single normal continuation edge. The oracle does not
call production LCA or interval code. Equal Branch successors have a separate
case because canonical DAG enumeration uses distinct edges.

Raw stress tests include 20,000 diamond/call stages (80,001 blocks and 60,001
locals), a 16,383-node wide branching tree with 8,192 call predecessors to a join,
the retained 99,999-call chain, and exactly 300,000 blocks followed by a plus-one
rejection. Source lowering combines 1,000 distinct bindings with 1,000 branches,
2,000 calls and real joins (5,001 blocks). Dimension tests cover zero, one,
powers of two, maxima, plus-one and checked multiplication overflow without
attempting enormous allocations. The optimized isolated OIR test execution also
ran under a 120-second subprocess timeout. These are resource regressions, not
performance certification or brittle timing benchmarks.

Supplemental production CLI exercises, each repeated and timeout-bounded:

- Release checks of 10,000 empty boolean branches/joins (110,039 source bytes)
  and 15,000 call continuations (135,074 bytes) return exactly the existing
  schema-1 success summary, empty stderr and no extra project files
- A deterministic independent structured-source generator (seed 39172) produces
  500 flow cases: 53 accepted, 294 E0302 missing-path cases and 153 E0303
  unreachable-statement cases, matching its construction-time flow oracle
- 300 malformed mutations also retain deterministic JSON, valid UTF-8 byte spans,
  the 100-diagnostic cap, no panic/timeout, ordinary exit 0/1, and no output files

Lowering preflights exact expansion before allocation. Source node accounting
bounds locals/assignments at 100,000; branch expansion raises only the block cap
to 300,000. The verifier uses flat predecessors with consistent edge multiplicity,
Kahn order, global single-definition records, predecessor-LCA binary lifting and
iterative tree intervals. There is no blocks-by-locals matrix, recursive CFG walk
or quadratic dominator-parent traversal.

Peak scratch is `(B*levels + 7B + E + 1)*size_of::<usize>()` plus
`L*size_of::<Option<Definition>>()` and small vector headers. At B=300,000,
levels=19, E=600,000, L=100,000 the cell count is 8,400,001. Measured type sizes
on this host are 8 and 24 bytes respectively: 69,600,008 bytes, approximately
69.6 MB decimal, excluding raw IR/source storage, allocator overhead and host
allocation failure. Predecessor insertion cursors and reachability/indegree
arrays are dropped before that peak. Only one function's scratch is live at once.

## Red/green and diagnostic compatibility

Before implementation, the new public choose/if/else CLI case failed with
E0101 at the `if` keyword; the same focused Cargo command exited 101 and later
passed in the full CLI suite. An actual-module frontend test harness recorded
10 intended branch/scope/flow failures and one already-passing grammar-rejection
case, then passed after implementation. The initial duplicate-definition ordering
case observed Uninitialized rather than the new global AlreadyInitialized rule;
it went green after global definition collection. The CFG fixture stage recorded
12 behavioral failures and two passes against rejecting Branch/Goto stubs, and
the scratch-dimension case separately failed before implementation. An actual
source-to-OIR test also failed at the mandatory verifier boundary until branch
verification was implemented. Supplemental provenance, corrupt-view and builder
assertions were added afterward and are not represented as independently observed
red cycles. Early compile-only failures and canceled duplicate builds under host
contention are not counted as behavioral red or verification failures.

E0302/E0303 wording, flat-source ranges, ordinary exit 1, internal E0500 exit 2,
JSON schema 1 and safe invalid-origin filtering remain intact. A non-bool branch
condition uses E0300 at its expression. The deliberate keyword expansion means
invalid if/else placements now receive syntax E0100 instead of the predecessor's
unsupported-keyword E0101. Other unsupported constructs retain E0101. Raw-IR
first-error ordering remains deterministic; global duplicate definitions are
checked before reads, after structural and graph validation.

## Limits and deferred work

Only this Linux x86_64 host was validated here. Windows, macOS Intel/ARM64,
Linux ARM64, source installation and Docker were not rerun locally. Publication
CI must be tied to its actual later commit; configured jobs and predecessor CI
are not new platform evidence. Generated `oxid.lock` from repository verification
is outside the reviewed source/documentation payload.

The accepted property is structural intraprocedural return completeness assuming
calls return normally. It is not program termination, executable semantic
correctness, ownership/borrow checking or memory safety. Private typed-HIR
producer assertions remain release-active and can panic on internal corruption;
the raw-OIR verifier uses fallible lookups independently. Host OOM, blocking
filesystem reads and broken output pipes remain outside the engineering bounds.
Numeric semantics, a reference execution engine, loops, merge values, target/ABI,
LLVM/native compilation, self-hosting and AI remain separate roadmap work.
