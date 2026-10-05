# Checked i32 unary negation: local validation

Scope: [RFC 0021](../../rfcs/0021-checked-i32-unary-negation.md), explicit
typed-preview only. This is a compact language increment, not completion of
M2/M3, v1.0, a memory-safety proof or stable ABI. Baseline is PR34 head
`c4c4ec0958a31fc7878bdc4031e93b204ff14acb`, tree
`b53f5e4969ed6036db79c1c0456f6d541301b0b0`. Baseline CI was running when work
began; local results do not imply any hosted merge or exact-head CI result.

## Implemented seam

Shared parsing distinguishes the existing minus+decimal literal primary from
right-associative general prefix negation. One AST/HIR child lowers to one
CheckedNegateI32 assignment with its genuine operand and operator spans.
Scalar and owned verification check types, source ownership, initialization
and dominance. Reference execution uses checked_neg after the ordinary charge.
Native emission uses LLVM checked subtraction from immediate zero and branches
before publishing the value; diagnostics, guarded fuel and phi exit labels are
preserved. Formatter roles and project source-association traversals include the
new node. No synthetic source origin, extra zero local or interpreter fallback
is introduced.

## Local evidence (Linux x86_64, Rust 1.99.0, LLVM 19.1.7 O0)

- Full Cargo all-target/all-feature suites: 1,129 passed and 48 explicitly ignored
  native/private gates in each debug/release profile. The focused ignored gates
  below were also explicitly executed; no claim is made that all 48 ignored tests
  ran as part of the ordinary suite.
- Eight internal source tests cover AST/HIR/OIR origins, unchanged signed-decimal
  lowering, exact fuel, one operand call, first errors, formatter behavior and
  parser node budgets across all four source modes.
- Eight portable public tests cover text/JSON, prefix precedence, strict types,
  dead-code checking, literal validity, name resolution, nesting 64/65 and long
  prefixes, module loading, formatter idempotence and child-file diagnostics.
- Twelve raw OIR tests per profile, including four real-LLVM gates, cover
  scalar/owned MIN, exact fuel, malformed types/IDs/origins, self/forward reads,
  branch dominance and phi continuation splits. Each profile retains 64 sets of
  actual LLVM IR, ELF bytes, stdout, stderr and exit status.
- One public native gate per profile compiles 23 real scalar/owned sources,
  deletes source files, clears the runtime environment and executes the ELF.
  Results and diagnostics match reference bytes for loops/mutation, call-once
  borrowing, short circuit/joins, signed MIN and executed failures. Source,
  ELF, compile/reference/native outputs and statuses are retained separately.
- Unmodified exact-literal bigint oracle: 502 literal cases, 320 source cases,
  13 rejected spellings and 1,670 debug CLI invocations.
- Unmodified arithmetic bigint oracle: 1,025 source cases, 4,100 invocations across
  both profiles, 602 successful and 423 overflow outcomes per source corpus.
- Separate deterministic Python-bigint unary/composition model: 488 scalar/owned
  sources and 976 both-profile observations with independently calculated values,
  first-overflow operator offsets and exact JSON profile agreement.
- Repository verification: 134 language sources, 126 checks, 72 runnable programs;
  formatter 36 positive / 21 negative cases. Feature inventory, rustfmt, Clippy with
  denied warnings, diff checks and actionlint 1.7.12 are checked separately.
- Independent review reproduced focused source/public/raw LLVM controls,
  byte-for-byte signed-literal/boolean-not predecessor CLI parity in 32 comparisons,
  source manifests and the layout measurements below. It found and prompted
  explicit both-profile native CI gate/artifact registration; that omission was
  fixed before finalization.

Evidence is retained outside the source checkout in the task's immutable
checkpoint manifests/patches and the unary-negation evidence/review directories.
The final evidence index identifies hashes, actual execution inputs and logs;
earlier failed exploratory runs are retained, not relabeled as successful.
The repository verifier generated a 12-byte `oxid.lock` control file; it is
preserved in evidence and excluded from the published source closure.

## Exact cost and layout boundaries

A valid signed literal, including trivia and MIN, remains one AST expression,
one scalar temporary and one constant assignment; bare-return total fuel is 4.
`--1` is a signed literal plus one negation: total 6. `(1)` has existing grouping
cost 6; `-(1)` costs 8. `-value()` evaluates its call once and the focused one-helper
program costs 9. MIN negation charges its assignment first: `--2147483648` reaches
E0604 at fuel 5, and `-(-2147483648)` at fuel 7; one less fuel gives E0601 at the
whole unperformed unary expression. General negation adds one slot entry charge
and one assignment charge; it does not charge operand evaluation twice.

Independent compiler layout output measured identical baseline/current sizes
on this host: ast::ExprKind 64, scalar hir::ExprKind 48, owned-source ExprKind 64,
shared Rvalue 96, Token 32, Span 24 and Vec<Span> header 24 bytes; all align 8.
The prefix stack remains Vec<Span>, without a kind/token payload increase or
encoded bits. At most 64 live span entries are accumulated before rejection
(1,536 payload bytes excluding capacity); valid flat unary depth includes the
primary and stays within the existing total height 64. Existing node/byte/work,
runtime slot/frame/fuel and stricter native admission ceilings are unchanged.
These are host-specific representations and bounded work claims, not a fixed
process-memory, allocator-success or sandbox guarantee.

## Intentional compatibility amendments and remaining qualification

Current Rust tests were narrowly updated for accepted general-negation syntax
and eager unresolved-name errors: typed_i32, typed_arithmetic, typed_division,
typed_logical, and the owned-source heldout AST/diagnostic parity test.
A dangling unary minus now reports E0100 at missing operand/EOF rather than the
old literal-only E0101 at minus. Valid signed literals and unsupported unary plus
remain unchanged.

Frozen expectations were not edited. A separate narrow source-binding successor
must account for owned-source corpus rows `scalar-general-minus` (result -3) and
`scalar-minus-group` (result -1), historically E0101; archived auxiliary copies
remain historical. The boolean-logic predecessor script's `-(1)` parse-E0101 row
also changes (its bool return context now fails E0300). Source/parser inventories,
current authority bindings and their published fingerprints must be qualified
by that successor before claiming those broader gates. Full historical binding,
other platforms, hosted exact-head CI and merge status are not certified here.
