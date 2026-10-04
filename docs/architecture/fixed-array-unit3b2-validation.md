# Private fixed-array source lowering validation

Unit3B2 connects the private array forms from [RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md)
to the existing ownership verifier through the real source pipeline. The first
five source cases now pass complete source-to-raw comparison through both Root
and Bytes inputs, plus controlled first-literal capacity-failure checks through
Root, in both debug and release. This is a
local Linux x86_64 Validate milestone. The production parser
and raw-array gates remain closed; the source programs' function bodies were
not executed.

## Implemented path

The test-only source entry accepts original bytes or a root path. Root requests
use relative path `main.ox`; Bytes requests use that exact display path and the
original source text. Both use `Complete`, `Validate` and fixed default limits;
the separate Root failure group uses ordinal-zero capacity failure. They traverse
the applicable real input/parser path, root selector,
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

## Initial Root source results

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
This initial Complete group and the failure group immediately below use the
retained observer-v3 binaries. The later fresh-binary Root/Bytes qualification
is recorded separately; it does not overwrite these historical results.

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

## Framed Bytes input and fresh Root compatibility

A subsequent two-file change adds a closed, test-only input-kind selector and
a bounded stdin frame reader. Root remains the default and keeps stdin on
DEVNULL. Bytes requires its explicit selector and has no fallback to Root. The
frame is a 24-byte header (magic/version `OXABY001` and two little-endian u64
lengths), followed by exact display-path bytes, source-text bytes and EOF.
Transport maxima are 4,194,304 path bytes and 1,048,576 source bytes, with one
combined allocation of at most 5,242,880 bytes. These fixed transport ceilings
do not replace a lower effective compiler limit.

The reader preserves valid UTF-8, including NUL and CR/LF, and rejects invalid
UTF-8, a code point split between fields, malformed or truncated frames and
trailing input before observation. It converts the owned byte buffer to String
without copying, then passes borrowed path/text slices to the existing entry.
Lengths are checked before any logical payload request or payload allocation.
That statement concerns the frame reader's logical operations: the standard
stdin adapter can read ahead, and its separately bounded storage remains live
through output printing. It is not an operating-system read trace.

Review corrected two checkpoint findings before publication: the first rejected
frame implementation could classify a split UTF-8 path followed by a later
invalid text byte as a text error, and its Rust-only manifest omitted unchanged
`src/main.ox`. The corrected classifier retains path-first error precedence
without a second payload scan, and the source inventory includes all 123 files.
Neither finding was an accepted malformed frame or an escape from the size
limit. Both original checkpoints and their evidence remain preserved.

On the fresh binaries, each profile independently passes 16 selected source-free
tests: six transport tests, eight observer tests and two layout tests. Strict
Clippy and formatting pass. The complete test-name inventory contains 821
entries; this is an inventory count, not a claim that all 821 were run. Each
source invocation selects the one ignored transport test with 820 others
filtered out.

Fresh instruction, relocation and layout checks bind the actual stdin
specialization in both binaries: an 8,192-byte adapter buffer and 56-byte shared
state. The input payload stays alive alongside the compiler's real text/path
copies through observation, then is dropped before the transcript is printed.
The global stdin buffer remains live. This transport memory and work are
separate from the observer's 8,001,104-byte allowance and compiler storage;
none is described as a total process peak. The Diagnostic width, capacities and
failure-work proof were also rebound to each fresh binary rather than inferred
from the older v3 executable.

The external collector hashes an on-disk frame and concurrently sends it using
one bounded 8 KiB pending chunk while draining capped stdout and stderr. It
handles partial writes, records only delivered bytes in the running hash,
closes stdin for EOF, and retains timeout, output-cap and setup/cleanup failures.
Source/path/frame hashes, full delivered count and hash, EOF and before/after
identities are mandatory comparison inputs. No expected compiler facts enter
the child's frame.

The independently source-derived Bytes supplement covers only the same five
files and the display path `main.ox`. They take equivalent syntax branches in
OwnedCandidate and ProjectCandidate parsing and retain the same downstream
semantic identities. Bytes reports source/module rows before load completion,
uses single-source-adapter module metadata, and has 21 frontend requests for
the grouped case and 20 for each other case, including eight zero-length
requests. The comparator requires these differences explicitly; changing a
Root input label is insufficient.

The new source/binary identities received separate admissions and actual runs:

| Fresh-binary route | Source/profile pairs | Result |
| --- | ---: | --- |
| Root Complete | 10 | Exact Validate comparisons pass |
| Root ordinal-zero capacity failure | 10 | Exact expected diagnostics match; validation credit remains false |
| Bytes Complete | 10 | Exact Validate comparisons and complete frame-delivery checks pass |

These are 30 calls over the same five sources, following the 20 historical v3
calls. Profile repetition, route repetition and expected failures do not add
new distinct source cases. No expectation changed after these observations.
All ten Root profile pairs and all five Bytes frame/transcript profile pairs
are byte-identical. A separate read-only check also finds all 20 fresh Root
transcripts exactly equal to their corresponding v3 transcripts, without
normalization or additional compiler calls.

Independent artifact-only replay repeats all 20 fresh Root comparisons and
all ten Bytes comparisons. Fresh Root evidence seals 324 members: 200 case
artifacts, 120 dispatch artifacts and four roster/result records. Bytes evidence
seals 184 members: 120 case artifacts (including frames and frame receipts),
60 dispatch artifacts and four roster/result records. Its ten frames total
1,568 bytes; transcripts total 25,232 structured units and 112,358 bytes.
The Bytes observations contain 1,390 static semantic rows and 202 frontend
request rows. These artifact replays make no compiler calls, and all historical
Root evidence remains unchanged.

Source-free qualification is recorded by component and is not summed as unique
global coverage: 39 frame/collector controls, 54 Bytes-predicate controls,
77 fresh Root-driver controls plus 11 independent artifact controls, and
52 final Bytes request/artifact controls each pass in normal and optimized
Python. These controls are separate from the 30 actual compiler calls.

### Fresh identities

The 123-file source manifest is
`6eed6163fa0c4c78ee37448f0648bd7baaf62433ce92f8d4d0ea244029fe5f8c`.
Base `e87789bf60d625b2df1dfe417dbe6b5450e8db0f` plus patch
`ef4b83a22dbffaf3300d021d41fbeb69ccff779f3b5b8f68f67169bc11346047`
produces tree `d506696bc2f18b8b05e33cbe644b9f7d119dd63b`, published at
`f7451ca2bf1a58e824bedbaa3278880d18810884`.
The fresh debug/release executable hashes are
`e0b11fdd6385cbf4ce47ab78e109322d850ec84b7cd87efad129ec98f4428bcc` and
`726626993f25229a70ca74724e38107e90272881fcc15fa5c8198fe52b238b79`.
Component review is
`43dae10792b4a25139be62fc96ddf0e642c70e657ef5bd480368f79d2ca8df2c`.

Fresh Root contract
`f151cc62381cfcfc123b5f6b86d6eae8bee5ab6d2e64d049e6168f62e243c92d`
has admission
`5db959d4f0b88c731b40e9727bea642e3f1f9e2c4db263ad90919dafca8fdec4`.
Its aggregate is
`cdc40c51c90f3a7e6373250c9095b5e0c8fcf264b84d3e54c39f3c3c20247c12`,
result freeze
`5c6f6ad911751bfa8c355a99dab73f58659e60dd755fd438e39e44dbef3bcbbb`,
and independent aggregate review
`cd88ad9cbfac77e3d11093b5523efc14d47c7cfafa164b0760327ebc1447bace`.
The separately recorded v3/v4 transcript parity receipt is
`7d93c577431db895215868b8ecd655245afe9c8e74d401304f4f3ae06b968889`.

Bytes source authority
`427e1b530bd50748e3f5662719e66fe33d72f7042032cf75e3d9a0e761f7415c`
has independent review
`0c7f7add7e9e6e6cc892dc2f11a966812e3c4773aadc4da83a9c1d04b6fad019`.
Its separate execution contract is
`cc3ad61b1e90cb9570bc23a2655a8322290dfab2be9a218e8234e2862549a992`,
with admission
`7220dd6efa24c047a9a1a18b98ae02142bea872ff30ebb01606d56363b04c0cb`.
The aggregate is
`cc25db4678165d9181d788a9c16426615a4792062136fef7582ef696139182b6`,
and result freeze
`d80d49fbd45fe0ced3e82e098c2501c50e4cda8fd59d2f1db1f5270220a78dc1`.
Post-run custody rechecks 304 current bindings and all 123 source members.
Independent Bytes aggregate review is
`35706b2827455bed8fac349bbeb9535167af1516244a64d98b356f5a3c7b1005`.

This fresh qualification still covers only these five sources, Root Complete
and ordinal-zero failure, and Bytes Complete with `main.ox`. It grants no
Bytes failure-mode or arbitrary-display-path semantic claim, successful source
Reference/Native execution, public syntax, hosted qualification or portable
replay claim. UTF-8/NUL transport support alone does not qualify arbitrary
source programs or display paths.
