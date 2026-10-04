# RFC 0018: checked i32 division and remainder

- Status: experimental implementation for independent review
- Baseline: published PR #29 head `8a3b8683d911bdabfcdc7ca7d3ba867f6235ded3`,
  tree `15a7f5e92502ec6bfdcf91eb46b1db1de9d215c9`; local replay checkpoint
  `847fc6b6def05c5f5db4c241b840e4fb0620bada` has the same tree
- Owner: Oxid language/tooling maintainers
- Review: independent review and applicable exact-head CI required before merge
- Acceptance decision: bounded typed-preview extension; no stability or milestone-completion claim
- Scope: binary i32 `/` and `%` in checking, reference execution, native compilation and formatting

## 1. Motivation and decision

Fixed-array programs need integer quotients and remainders for routine numeric
work. Extend the existing checked i32 arithmetic contract of
[RFC 0006](0006-checked-i32-arithmetic.md), its
[native implementation contract](0008-native-checked-i32.md), and the current
[typed-preview source specification](../spec/typed-preview.md) with these two
ordinary binary operators. The default and explicit legacy edition retain
existing dynamic arithmetic.

This is an intentional evolution of current source semantics. Earlier RFCs,
validation ledgers, frozen input packages, semantic-authority manifests, source
pins and historical expected observations remain immutable evidence of their
original scopes. In particular, old statements that division or remainder were
deferred describe those earlier contracts, and historical rejection cases must
not be relabeled as having admitted these operators. Current implementation
specifications and new tests describe the new behavior; they do not retrospectively
qualify old compilers or replace a frozen source or observer identity. Any
historical replay continues to use its declared compiler/input authority.

This extension does not claim completion of M2, M3, any fixed-array milestone,
or a general numeric system, and does not broaden native target qualification.

## 2. Grammar and typing

The multiplicative precedence level becomes:

```text
product := unary (("*" | "/" | "%") unary)*
```

All three operators have equal precedence and associate left. They bind more
tightly than binary `+` and `-`; existing comparison and logical precedence is
unchanged. Parentheses override grouping. Consequently `20 / 3 * 2` is 12,
`20 % 6 * 3` is 6, `20 / 3 % 4` is 2, and `80 / 4 / 2` is 10.

Both operands and the result must be i32. There is no bool/unit coercion, float
or array arithmetic. A mismatched operand is E0300/type under the existing
operand-span rules, including in unused functions and unchosen branches.
The complete source/project is checked before execution or native tool discovery.

The minus sign remains part of a signed decimal literal only. Negative divisors
such as `17 / -10` are accepted, but `-x`, `-(2)`, `-f()`, `--1` and unary `+1`
remain unsupported. Decimal range checking remains E0203 and occurs statically;
this extension does not admit positive `2147483648` or negative `-2147483649`.
Slash still begins existing `//` and `/* ... */` comment forms where applicable.

The formatter treats binary `/` and `%` as ordinary spaced arithmetic operators,
preserving protected token/comment bytes, line breaks, grouping, and idempotence
under [RFC 0017](0017-bounded-typed-formatter.md). Formatting does not evaluate
arithmetic or turn a runtime error into a syntax error.

## 3. Exact arithmetic and failures

For nonzero divisor `b`, the quotient `a / b` truncates toward zero. The remainder
`a % b` satisfies `a = (a / b) * b + (a % b)` mathematically; a nonzero remainder
has the dividend's sign and magnitude less than the divisor's magnitude.
For example:

| a | b | a / b | a % b |
| ---: | ---: | ---: | ---: |
| 17 | 10 | 1 | 7 |
| -17 | 10 | -1 | -7 |
| 17 | -10 | -1 | 7 |
| -17 | -10 | 1 | -7 |

Both `/ 0` and `% 0` stop execution with E0607/oir-run and the exact message
`checked i32 division by zero`. Both `-2147483648 / -1` and
`-2147483648 % -1` stop with E0604/oir-run and the existing message
`checked i32 arithmetic overflow`. The remainder case is deliberately checked
even though mathematical remainder zero is representable: it shares the checked
signed-division pair's overflow boundary.

Every error points to that operator's single-byte source span, with the correct
file/line/column retained through modules. The owned-array route uses the same
arithmetic diagnostic contract as the scalar route. Array-bounds diagnostics
remain separate. There is no host panic, wraparound, saturation, infinity or NaN.
Host debug/release settings do not change this behavior.

`check` accepts type-correct division by zero and arithmetic-overflow expressions
because it does not execute them. Running a failing expression exits 1, prints
no result to text stdout, and emits the existing escaped text diagnostic. JSON
keeps schema 1, one diagnostic and a failed run-summary with result null. Native
executables use the identical text diagnostic, status and empty stdout contract.

## 4. Evaluation order, effects and resources

Fully evaluate the left operand, then the right operand, then perform the
checked operation. Each operand call runs once. Stop at the first failure; do
not speculate, reassociate or erase a failing discarded expression. In
`(2147483647 + 1) / 0`, addition overflow wins. In `0 / (1 % 0)`, the inner
remainder's zero-divisor failure wins. Unchosen branch and short-circuit operands
do not execute, although their types are checked.

Existing array-store order remains binding: evaluate the RHS before its target
index. Thus `a[1 / 0] = 7 % 0` fails at `%`. Borrowed mutation in operand calls
retains the same checked ownership and left-to-right effect order. Array moves,
borrows, logical values, call continuations, loop backedges and project routing
are otherwise unchanged.

Each operator remains one binary syntax/HIR node and one checked arithmetic OIR
assignment with its ordered operands and operator origin. Existing verified
source association, type/dominance/read-before-write checks and raw admission
remain mandatory. The assignment has the same fuel cost as existing checked
arithmetic; charge fuel before reading operands or checking arithmetic and never
write its destination on failure. Existing tree-height, storage, call, project,
array and native limits are not raised. Division introduces only constant-size
operator/error handling, not a new variable-size runtime allocation.

The LLVM backend must guard zero and the MIN/-1 pair before executing `sdiv` or
`srem`. A select of an already-invalid arithmetic result is insufficient. The
success continuation must preserve logical-merge predecessors, calls, loop-fuel
guards and source origins. No LLVM undefined signed-division path may be
reachable for an admitted runtime input. Existing checked entry/admission and
source-free native output requirements remain unchanged.

## 5. Compatibility and alternatives

Only explicit `typed-preview` source that previously rejected `/` or `%` gains
new behavior. There is no new edition name, CLI flag, dependency, public OIR/ABI,
or artifact encoding. Default/explicit legacy execution and OXBC handling remain
unchanged. No migration is needed for already-valid typed-preview programs.

Rejected alternatives: floor/Euclidean division (different negative results),
implicit floating-point arithmetic, wrapping/saturating overflow, and a
profile-dependent trap policy. General unary negation, casts, widening,
wrapping variants, unsigned arithmetic and operator overloading are deferred.
The work makes no throughput, memory-performance or production-readiness claim.

## 6. Acceptance and evidence

The public CLI target `tests/typed_division.rs` covers:

- `check` plus text/JSON `run` with all four sign combinations, zero dividends,
  i32 extrema, grouping, equal multiplicative precedence and left association
- Scalar and owned-array routes using the same independently specified values
- E0607 zero-divisor and E0604 MIN/-1 failures for both operators, exact operator
  spans, source-file origins and first-error behavior
- Single, ordered operand-call effects, array-store RHS order, discarded
  expressions and nonexecuted short-circuit/branch operands
- Strict operand types, unchanged unary/literal exclusions, and type rejection
  before missing native tools can be consulted
- Formatter exact bytes and idempotence, bounded loop fuel, and a real two-module
  owned-array program: `[17, 23, 35]`, summed as `(v / 10) * 100 + v % 10`, is 615
- Explicit optional LLVM 19.1.7 native gates using the same expected results and
  failures, including logical merges and loop fuel; executables run after source
  files are removed and with a cleared environment

The normal Rust test target does not require LLVM. Native gates follow the
existing Linux x86_64 ignored-test convention and must be run explicitly with the
pinned LLVM/Clang/LLD 19.1.7 toolchain. Declared-child module tests retain the
existing Linux discovery gate; scalar root-file checks remain host-independent.
Existing CI hosts and backend qualification boundaries remain authoritative.

Run the focused public target and explicit native gates, then repository format,
Clippy, full locked Rust tests, Python script tests, release build and repository
verification against the integrated change. Independent review and applicable
exact-head CI results must be recorded separately. Test definitions, configured
CI or prior-stage evidence do not themselves establish a green final head.
