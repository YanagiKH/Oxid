# Private fixed-array source lowering validation

Unit3B2 connects the private array forms from [RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md)
to the existing ownership verifier through the real source pipeline. The first
five source cases now pass complete source-to-raw comparison and controlled
first-literal capacity-failure checks in both debug and release. This is a
local Linux x86_64 Validate milestone. The production parser
and raw-array gates remain closed; the source programs' function bodies were
not executed.

## Implemented path

The test-only source entry accepts original bytes or a root path. The qualified
requests here use `Root`, relative path `main.ox`, `Complete`, `Validate` and
fixed default limits. They traverse the actual loader, parser, root selector,
declaration index, resolver, typechecker, four-pass lowerer, source association
and verifier-confined validation entry. The observer returns bounded inert
facts and diagnostics. Its input does not accept a caller-built AST, typed
owner, raw program, allocation plan or executable witness.

Array literals retain written child order. Their operand request occurs after
their children complete and before the literal completes. Indexed stores lower
the complete RHS, then the complete index, then one indexed write. The target
wrapper does not introduce a read or loan. Indexed reads and array lengths
retain their whole-owner access and original source origins. The unchanged
ordinary executable guard and immutable types-only rejection remain separate
from this private lowering path.

## Actual source results

The [first-five source authorities](../../tests/fixtures/fixed_array_source_unit3/lowering-contracts-v1/README.md)
were frozen before candidate observations, including the two explicit original
model corrections. The comparison also uses separately frozen source-derived
HIR metadata, dimensions, literal-request prefixes and resource accounting.
No expected fact was changed after these source observations.

| Source case | Static rows per profile | Transcript bytes per profile |
| --- | ---: | ---: |
| grouped-complete-access-and-index | 115 | 9,710 |
| index-read-bounds-after-effect | 151 | 12,105 |
| rhs-bounds-before-index | 143 | 11,593 |
| rhs-snapshot-final-bounds-failure | 143 | 11,593 |
| rhs-snapshot-success | 143 | 11,593 |

The Complete group has five distinct sources and ten distinct successful
processes, one case/profile pair per request. All ten complete comparisons pass. The five
debug/release transcript pairs are byte-identical, with 1,390 static rows in
total. Cases named for runtime effects or bounds failures still receive only
Validate credit here; those names do not imply that their bodies ran.

Comparison checks original text, UTF-8 bytes, paths, file roles and source
identity; complete HIR, typed and raw tables; operands, regions, origins and
cleanup; all four pass completions and Counts; block totals; literal request
placement; source association; and authoritative validator usage. Each of the
54 row families has an explicit predicate. Only the positive unique runtime
source-file nonce is normalized. A short-read sequence is checked as a bounded
positive cumulative partition ending at the exact source length, rather than
claimed as a uniquely predicted operating-system call history.

An independent audit rechecked all 100 original request, process, transport,
source-binding and comparison artifacts, then repeated all ten comparisons.
The results were byte-identical to the original comparison receipts. This
audit made no additional compiler calls and did not rebuild the compiler.

### Controlled first-literal capacity failure

A separate frozen request group applies `literal-capacity-failure`, ordinal
zero, to the same five sources in both profiles. All ten observations match
the independently derived failure expectations, with no post-observation
expectation change. These are ten additional compiler calls and the same five
sources, giving twenty calls across the two control groups. No source-program
body is executed by either group.

Each request forces the existing real capacity-overflow failure at the first
literal Operand reservation. It preserves completed frontend facts and the
exact earlier helper, count-walk and child-completion prefixes, then reports
lowering failure and the exact sourceless R01/E0400 diagnostic. No current
literal completion, completed emission Counts, raw-program projection,
association or verification output follows that failed request. The terminal
keeps Q null and all acceptance, complete-validation and semantic-credit flags
false. A matching expected failure is not counted as successful source
validation.

The failed schedules were derived from the original source models and admitted
reservation prefixes. Their unchanged frontend rows come from the
pre-observation source-derived files, not from filtering the successful
Complete transcripts. All five failure transcript pairs are byte-identical
between debug and release. The ten observations contain 884 static-prefix
rows, 472 lower-schedule rows, 15,486 structured units and 78,544 transcript
bytes; the two row categories remain separate.

Independent replay repeats all ten failure comparisons byte-for-byte, verifies
the 100 original failure artifacts unchanged and also verifies the earlier
103-member Complete result seal. This audit makes no additional compiler calls.

The failure extension freshly passes 114 source-free controls in each Python
mode: 78 semantic controls, 29 request/admission controls and seven process
controls. The seven process controls repeat the already qualified collector
checks; these counts are not summed with the Complete group as unique global
coverage. The new request admission is specific to ordinal zero, and rejects
the earlier Complete admission.

## Observer and collector checks

The exact v3 test binaries independently pass eight observer support tests and
one existing private-index layout test per profile: nine source-free tests in
debug and nine in release. Formatting also passes. The preceding v2 binaries
passed 113 owned-source tests per profile, with six ignored entries, and strict
Clippy. V3 adds seven layout-print rows to a support test; the v2 compatibility
and lint results are retained as v2 evidence rather than relabelled fresh v3
runs. The ignored source transport was subsequently invoked for the ten
Complete requests and the ten controlled-failure requests counted above.

Comparator and collector qualification comprises 164 distinct synthetic or
source-free controls per Python mode. The first 162 controls passed under both
normal Python and `python -O`. Independent review then found that an output
setup failure could occur after child creation but before its cleanup scope.
The corrected collector freshly passes the original five process controls and
two new setup-failure controls in both modes. The other 157 controls retain
their unchanged prior identities and results; this is not another complete
164-control replay on the successor.

The setup controls prove zero child creation on a partial spool failure and
prompt reaping with closed handles after a post-spawn registration failure.
Collection writes the request and input identities before starting the named
test, bounds stdout and stderr while draining them, and rejects timeout,
overflow, signal, nonzero exit or malformed framing. Comparison separately
rechecks the complete artifact set and original transport extraction. A
successful collection alone does not grant semantic qualification.

Earlier compile, lint and protocol findings are preserved in the validation
record: the initial observer enum/import/borrow errors, a transport integer
width mismatch, two newline-formatting lints, selector-row ordering in the
first comparator draft, and the collector setup-cleanup defect. None was
resolved by broadening the source expectations.

## Resource scope

The observer reserves its admitted transcript ceiling once and checks every
actual formatted write before growth. It separately checks byte and structured
unit accounting, uses sticky failure and discards incomplete tentative output.
Completed emission is reported after the lowerer's release count/table checks.

For these successful default requests, the exact requested auxiliary allowance
is 8,001,104 bytes: Output 512 + Allocator 72 + CaptureWrapper 520 +
200,000 ReserveEvent rows of 40 bytes. The transcript reservation is separately
16,777,216 bytes. Successful diagnostic work is zero. These are scoped requested
storage and source-free layout facts; they are not allocator capacity, total
compiler peak memory or process RSS. External Python decoding has its own
byte, nesting and structured-unit bounds.

For the controlled failure, diagnostic work is exactly 3,704. A source-free
inspection of the retained debug executable's qualified type metadata gives
Diagnostic width 136 bytes. Independent inspection of the release executable's
header multiplication and capacity stores confirms the release figures.
The frozen bounded diagnostic constructor charges 1,440 bytes: the 136-byte
header, message capacity 1,024, one 24-byte String slot and note capacity 256.
The trace has already been released before rendering the diagnostic, so this
charge is dominated by the earlier 8,000,000-byte trace payload and the exact
peak remains 8,001,104. No candidate failure output was used to infer these
sizes, no compiler rebuild was required, and the debug measurement was not
silently assumed to apply to release.

Actual reservations check all eighteen fields in each request-site Counts
record, exact operand count, width, payload, outcome and temporal placement. Frontend allocation
ordinals are one-based; literal-request ordinals are zero-based. The successful
AST literal-entry, HIR literal-entry and emitted operand totals must each match
the independently derived source total. Equality between candidate totals alone
does not satisfy the comparator.

## Identities and continuing qualification

The tested 122-file source manifest is
`10695137f4878d7739a433b6215fae0ba94e05dd6c133bbb8a94a1ee4c9be0c4`.
It reconstructs from base `901e9491ffad0562ae8c6125029e81bde85d7313`
and patch `4bcb6ea8afc7f058728df7b5287146b3a4095fdb1e4c3dc97248a6624c4d2b31`.
The exact resulting tree is `8644a00a6315dd89e9bb08711c0510bab0abc962`,
published at `ae133901340b2eb820ff062456cd653516979135`.

Local tools were Rust 1.99.0, compiler commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, on Linux x86_64. Retained
debug/release test executable SHA-256 values are respectively
`a90c000440780dcf5f24945bd856ac82d113861c4fb8fda0ba1f8f11941bc092` and
`c5bbd93f372df0737a084cc7aa1004b81f28c3249b11fb2d407d9e0135fe8893`.

The admitted collector contract is
`4512dc5da27db1cc22cd04353c4a6318f79052115e3f285a00aea7dec2ce69ce`;
its independent admission is
`32b628730a23707f82702e34dfd231592d1553be915573eaa9229717a1aeabc5`.
The actual ten-result aggregate is
`47a2a45fb4f371d3b242acb2f4517e737a82a0f99ef40ab92d25305d42ff9cc5`,
with 103-member result freeze
`864b7cfeafdf38c45431d87e31ff0c5e8dc390983d6cf6a5748b9fe4691255e3`.
All 120 runtime/input/executable bindings and 122 source members remained
unchanged after collection. The independent actual-result audit is
`65140657e1e8d1d1d694b80f3bd7a4809a28269b179c8d81f1c99741a98f9161`.

The separate failure contract is
`9f4c7c0abe3129cbe2622b92bd68d1c89995962f18134d88d936737e14cebfd1`,
with independent admission
`8cbea36b352ac44f33db88ed77a0f384da344731c0ad446a8926220d15734ffa`.
Its original source-schedule authority is
`178bd06dc78ec912ed14f703890e00c47bfd2e33a9b4045f25a7a464d3a0b6fc`;
the source-free numeric review freeze is
`c9732950dcc1162faa31ae85ab92b3f912db5743aeb75e64dc60e468de855f97`.
The ten-failure result aggregate is
`d2fdf9abe151ab6efedfc39ced43ad14c91645b661913453394d9aede2383cdc`,
with 164-member freeze
`cff776ad1011d370d530fe7726491f7be70a18ffac77a6f98171e7dcc9e01c79`:
100 original case artifacts, 60 dispatch artifacts and four result/set records.
Post-run verification preserves 174 runtime/input/executable bindings, all 122
source members and all 103 members of the earlier Complete result set.
The independent failure-observation audit is
`ca4a9923e78b7e6a7b275c60cf16e2d1796cea2379cefa4a52d59fbbfe11cb05`.

The remaining source cases, other diagnostic and failure controls, ordinary-record
route comparison, source Reference/Native execution and integrated pilot still
need their own qualification. This local record does not establish a hosted or
portable replay gate, and it does not qualify public array syntax.
The controlled failure group does not cover zero-length literals, later
reservation ordinals or general allocator exhaustion.
