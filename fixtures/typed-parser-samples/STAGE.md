# Scalar parser implementation and qualification

The agreed scalar-function grammar is implemented, with exact canonical AST,
byte spans and first-diagnostic projection. Production StagePending transitions
are gone. Providers, semantic name/type checking, Unicode and compiler
self-rebuilds remain outside this component.

The repository controller at `60f8c33` passed 139 inputs in both reference and
native modes: 118 complete canonical comparisons and 21 site refusals per mode,
with zero pending. Both modes also passed two transport-domain probes and all
41 malformed-observation controls. Native compilation is ordinary public CLI
compilation on Linux x86_64 with LLVM/Clang/LLD 19.1.7 at O0. Twenty-seven authored
decoder methods, six controller tests and 51 registry tests pass separately.
Exact-head hosted debug/release CI remains a separate gate.

## Preserved development evidence

| Checkpoint | Scope | Paired input count |
| --- | --- | ---: |
| `8f1fe203` | Initial signatures, blocks and leaves | 54 |
| `00545401` | Prefixes, grouping and all operator tiers | 87 |
| `40898984` | Calls and linked arguments | 105 |
| `6aff6597` | Let/LetMut, assignment and match-site refusal | 119 |
| `bc8e719b` | If/Else-block and While | 135 |
| `0bb3bbb` source / `60f8c33` controller | Reviewed public-prefix diagnostic correction | 139 |

The original expanded-cell native gate rejected the initial real dispatcher at
12,559 cells. The separately qualified native inventory successor allowed the
unchanged source through ordinary gates. [ADMISSION.md](ADMISSION.md) keeps the
failed layouts, exact measurements and distinction from allocation reduction.

Independent full-grammar review then found a diagnostic-priority defect not
covered by the initial 135 inputs: non-declaration `pub` was consumed too early.
The seven-line root guard now reports the canonical diagnostic at the original
`pub`, while preserving public functions and declaration-family refusals. All
four reproductions are fixed roster cases; nine focused reference/native
neighbors pass. Original failed outputs remain separate from the corrected
qualification.

The retained 41 corruption controls have 39 unchanged expectations and two exact
successors: Number-to-Call and statement-to-Let mutations now reject malformed
token kinds rather than unsupported row kinds. No successful corruption is
accepted and no broad exception normalization is used.

A 128-byte combined two-function input exercises the grammar as syntax; its
counter is not evaluated. Real input covers 19 nested If blocks, not the
unreachable 64-block endpoint under the source bound. Prefix chains exercise
expression depth and height boundaries. Native execution uses a cleared
environment and empty working directory with copied sources hidden; it makes
no filesystem-sandbox claim.
