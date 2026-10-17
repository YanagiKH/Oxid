# Separate-span scalar parser experiment

This additive overlay is a separate-span representation experiment. It is not
a public provider, source-capacity increase, full-language parser, or self-hosting claim. The actual
source reader and OPA2 output contract still have a 255-byte cap. Compiler budgets,
128 row IDs, 129 physical row/frame slots and expression limits are unchanged.

## Representation

- AST rows use five separate arrays: `headers`, `row_starts`, `row_ends`, `ab`,
  `cd`. The header holds only `kind + 64*next`. Payload pairs remain unchanged.
- Lexer start/end columns stay live through parsing. The token column holds
  `kind + 64*physical_slot`; `lo` and `hi` resolve that slot in the separate span
  columns. Existing token IDs, trivia handling and lookahead remain unchanged.
- Grammar helpers receive explicit borrowed span columns. Row-span updates do
  not modify links; links do not modify spans. Continuations retain their old
  encoding and capacity.
- OPA2 output validates the complete row banks before writing any header, then
  reconstructs the original packed header into one reusable byte-plane buffer.
  Invalid used lengths or diagnostic spans return 64; invalid/wide AST rows
  return 70. Both paths emit no bytes. No truncation or fallback is used.

The added storage is a deliberate preparatory cost, not an optimization claim.
Actual wider endpoints are exercised by synthetic carrier controls. No source
longer than 255 bytes is admitted by this entry.

## Materialization and validation

```
python3 scripts/build_wide_span_parser.py --output /new/candidate-source
python3 scripts/verify_wide_span_parser.py \
  --compiler target/release/oxid --llvm-bin /path/to/llvm-19/bin \
  --output /new/evidence
```

Omit `--llvm-bin` for reference-only execution; that does not qualify native
admission or execution. Rust must be on PATH to build the canonical observers.
The materializer checks the entire original v2 source manifest, copies the
closed parser dependency graph, and substitutes exactly ten explicit overlay
modules. It writes observed source identities in the fresh output directory;
no historical source or pinned manifest is rewritten. `control_main.ox` is a
separate synthetic entry admitted and copied by the materializer alongside the
parser dependency graph. Repository verification checks both materialized roots.

The verifier retains each command, input, stdout, stderr, status and hash. It
compares baseline/candidate reference execution and, when requested, both native
ELFs on 148 inputs: the existing 139-case roster and nine additional domain/edge
inputs. It separately builds unchanged canonical Rust lexer/parser observers.
For the 139 original cases, it compares the full AST/first diagnostic or the
roster's declared grammar-site refusal. The test adapter changes only OPA2 magic
to OPA1 for those <=128-byte observations; it does not widen the independent
projection's input contract. Exact OPA2 byte equality with the baseline also
covers the 254/255-byte cases. Wider independent canonical projection is pending
separate review.

Twenty-two synthetic controls run in reference and native modes:

| Modes | Check | Expected exit / output |
| --- | --- | --- |
| 0–5 | Row/token span round trips at 254, 255, 256, 65,535, 65,536 and i32::MAX; all 128 row IDs, link 128, EOF token slot 128, full continuation push/pop | 0 / empty |
| 6–7 | Used length 256; diagnostic endpoint 256 | 64 / empty |
| 8–10 | AST endpoint 256, negative start, reversed span | 70 / empty |
| 11–13 | Inactive header/span poison; link 129 | 70 / empty |
| 14 | Lexical diagnostic with used length 256 | 64 / empty |
| 15 | Diagnostic at endpoint 255 | 0 / exact 11-byte OPA2 |
| 16 | All 128 active rows, endpoint 255, exact links | 0 / exact 1,559-byte OPA2 |
| 17–19 | Row exhaustion, root pop, stack exhaustion | 70 / empty |
| 20–21 | Diagnostic end exceeds used length (0→1 and 254→255) | 64 / empty |

## Measured results and limits

Development baseline is `f29c69ee0144b04c19c09bbd905e41bf3706c156`; the preceding
measurement report is committed separately as `3c8be4ec7091ce6c5f852b0a5136a684ec48d520`.
The initial candidate, before the diagnostic end-versus-used guard and its two
regressions, passed local Linux x86_64 release-host validation with 913 retained
commands: three actual native builds, 148 inputs in four execution configurations,
278 canonical observer invocations, and 40 synthetic-control executions.
These historical checks are not 913 unique semantic cases and do not validate
the later guard correction; that successor requires fresh independent validation.
The initial full candidate passed ordinary native admission and actual LLVM
compilation/execution with
unchanged fuel and verification. No private compiler entry or fallback executes
the parser in these checks.

Supplemental logging-only instrumentation in a separate copy of accepted compiler
source measured the full closures after `NativeInventories::checked` verified its
counts. Production compiler sources were not edited:

| Full parser closure | Inventory items I | Owner width W | Functions | I headroom / W headroom |
| --- | ---: | ---: | ---: | ---: |
| Baseline v2 | 5,881 | 3,366 | 81 | 2,311 / 4,826 |
| Separate spans | 6,684 | 4,140 | 82 | 1,508 / 4,052 |
| Delta | +803 | +774 | +1 | — |

Both inventory ceilings remain 8,192. These are compiler admission inventories,
not RSS, runtime fuel consumption or throughput. Both instrumented builds also
succeeded, but ordinary uninstrumented builds/execution provide the authority.
The historical admission figures in other reports describe other exact sources
and are not substituted for these measurements.

LLVM 19.1.7 was obtained by local extraction of authenticated official Debian
packages at the CI-pinned version `1:19.1.7-3+b1`. clang/opt/llc/ld.lld version checks
and actual compile/link/run succeeded. No security settings were changed.

This patch does not qualify source-scale parsing, incremental token refill,
spelling retention beyond the source bank, a wide-span wire protocol, static
consumer changes, supervisor tariffs, full-language syntax, hosted CI or compiler
self-rebuilds. The independent corpus/review remains separate. All historical
sources, proofs, bindings, inventories and public provider selections are intact.
