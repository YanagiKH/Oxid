# Typed scalar comparisons: local validation

Scope: the accepted i32/bool comparison policy in
[RFC 0009](../../rfcs/0009-scalar-comparisons.md), based on merged main
`cbd149d7d74570704d359dfa1fb7f635a4086735` (tree
`cd77ae818ced9f3cfc2422aa79ceec57c4613f84`). Local results below do not substitute
for independent review, hosted CI, non-Linux native support, production safety,
a stable language edition or completion of M2/M3.

## Environment and unchanged boundaries

2026-10-01, Linux x86_64, Debian 13/glibc 2.41, Rust/Cargo 1.98.1. Native tools
are LLVM/Clang/LLD 19.1.7 with the same qualified package provenance described
in the [native predecessor report](native-preview-validation.md). Both debug and
release compiler binaries invoke native generation at O0. No O2/LTO claim is made.

The lexer/parser, HIR/type checker, OIR producer/verifier/reference executor and
native allowlist/lowerer change. The C scalar/error adapter, external native tool
invocation, no-clobber publication, all resource ceilings, legacy syntax/runtime
and OXBC serialization are unchanged. Existing unsupported-comparison assertions
are replaced by still-unsupported logical-operator controls; legacy tests remain.

## Source-to-runtime tests

`tests/typed_comparisons.rs` has ten public CLI tests covering:

- All signed comparisons, exact MIN/MAX and negative/positive boundaries, true
  and false scalar output with success status 0, and bool equality truth tables
- Arithmetic precedence, explicit nested comparison/bool equality, every ordered
  pair of unparenthesized comparators, and the complete invalid scalar/type table
- Mixed-type equality, unit equality and ordered bool rejection at the first
  invalid operand, including unused declarations and unchosen branches
- Adjacent multibyte tokens, standalone logical-operator rejection, source-order
  name resolution, eager comparison operands, left/right call and argument order,
  discarded operands/results and first-overflow source locations
- Bindings, calls and branch conditions that consume comparison results, plus
  exact total-expression-height boundaries with grouping and arithmetic

The focused public comparison test was compiled independently against the frozen
arithmetic binary first and failed with exit 101 because the lexer/parser lacked
`==`. A separate oracle run failed at the same absent syntax. Native admission
was separately demonstrated red after reference support: the valid comparison
ran successfully but native compilation returned E0700 before any tools/files.
After the explicit native allowlist/lowering addition it reaches tool selection,
and actual LLVM smoke cases match reference output, including E0604 operands.

## Independent private-IR and budget checks

`src/frontend/oir/comparison_tests.rs` supplies six raw-IR test groups. The complete
six-operator by three-left-type by three-right-type matrix checks 54 independently
mutated candidates: only i32/i32 for all six and bool/bool for equality/inequality
can verify. Other tests mutate each operand/destination ID and type, both operand
initialization positions, self/late reads, duplicate definitions, branch-only call
results and all three origin spans. Unicode/CRLF AST/HIR/OIR origins and two-byte
operator spans are checked through actual source lowering.

Two execution tests hand-count eight fuel for direct i32 or bool comparison,
fourteen for `(1 < 2) == true` including grouping, and check the next unperformed
origin with one less fuel. An independent event trace verifies operand functions
are each entered/returned once, left before right, for both i32 ordering and bool
equality. Two native unit tests check every signed predicate/operand width, exclude
unsigned/subtraction/floating lowering, and match the unchanged cost recurrence.
Two native CLI tests cover admission and grammar/type failures before tools.

## Tagged Python / reference / native corpus

`scripts/verify_scalar_comparisons.py` uses tagged i32/bool/unit values with exact
Python host types; its own negative assertions ensure Python's bool-as-int subtype
relationship cannot accidentally approve a coercion. It uses Python arbitrary
precision and explicit per-node i32 range checks for arithmetic, source-order
first-error positions and direct comparison of supported tagged values.

The corpus contains 1,610 valid sources: 1,014 i32 boundary/operator combinations,
eight bool truth-table cases, 500 generated trees and 88 further grammar, branch,
call, discard, ordering and boundary cases. It expects 1,202 successful values and
408 operand-overflow results. For each source, both compiler profiles perform
public check, human run, JSON run and real native compilation/execution. This
produces 3,220 native artifacts; result types, stdout/stderr/exit, exact origins
and paired-profile artifact hashes must match the independent model. E0604 human
stderr is independently constructed, including control-character filename escapes
and Unicode-scalar columns, rather than copied from reference stderr.

The source is removed while each executable runs with no compiler, LLVM, Python,
source or tool path in its environment. Filenames contain Unicode, quotes,
backslash, shell metacharacters and terminal control bytes; sources include
Unicode comments and CRLF. ELF magic is checked for each artifact; representative
true/false/error executables have libc-only dependencies and are exercised with
`/dev/full` and closed output pipes. The actual inherited C error adapter is also
run with controlled partial-write, EINTR, zero-progress, EIO and signal failures.

There are another 198 negative sources, checked through check/run/compile in both
profiles. Fourteen native admission cases test inclusive and plus-one function,
parameter, local, aggregate-local, aggregate-block, call-depth and fuel bounds.
The independently counted fuel pair is exactly 100,000 versus 100,001. Missing
LLVM in these admission probes is intentional: E0701 distinguishes accepted
language/resource input from E0700 before tool execution. The main valid corpus
uses real pinned tools and cannot silently skip missing LLVM.

## Reproduction

Use the existing qualified LLVM environment, then run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
python3 -m unittest discover -s scripts -p 'test_*.py' -v
cargo build --release --locked
python3 scripts/verify_scalar_comparisons.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_preview.py target/release/oxid
python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_i32_literals.py target/release/oxid
python3 scripts/verify_repo.py target/release/oxid
```

## Local results

- Debug and release all-target/all-feature tests: 308 passed in each profile
  (214 unit + 94 integration), zero failures
- Formatting, Clippy with warnings denied and release compiler build: passed
- Focused reference corpus and all 14 native-admission pairs: passed
- Complete tagged comparison oracle: 1,610 valid sources (1,202 success and 408
  exact operand-overflow results), 3,220 real LLVM artifacts, 198 negative sources
  and 14 native-resource cases passed across both compiler profiles; 14,124 compiler
  CLI invocations. Every paired native artifact hash agreed
- Native arithmetic predecessor oracle: 1,051 sources / 2,102 artifacts passed
- Scalar native predecessor oracle: 57 real compiled cases plus printer fault
  injection passed, including the 32-frame/64-parameter 1 MiB-stack case
- Reference arithmetic oracle: 4,100 invocations over 1,025 sources passed
- Exact literal oracle: 1,670 invocations passed
- Metadata unit tests: 7 passed; feature inventory: 20 entries valid
- Full repository verifier: 121 sources and 67 runnable programs passed
- Workflow YAML parsed locally; no hosted CI result is claimed for this increment

The CI native job includes the new gate in addition to both native predecessor
gates. CI configuration is not evidence of a hosted run for this new revision.

## Residual limits

Independent review and hosted CI remain separate. No non-Linux native target,
O2/LTO optimization, new scalar type, generic equality, unit equality, coercion,
boolean short-circuiting, loop, mutation, native recursion, memory/ownership proof,
benchmark superiority or portability certification is claimed. Existing host
allocation/output/signal/stack limitations remain, and diagnostic paths embedded
by arithmetic retain their compile-time meaning.
