# Private fixed-array native consumer validation (Unit 2D)

Status: the complete combined producer and independent debug/release rehearsal
passes locally on Linux x86_64 with Rust 1.99.0 and LLVM 19.1.7, including full
retention and relocated compact admission. The permanent replay runner and
combined CI controller are committed. Exact-head hosted checks remain open. These results implement the private native
consumer in [RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md). Typed source
arrays and the production raw verifier remain gated; this adds no cross-host
native support or milestone completion claim.

Production `verify_with_limits` still calls `reject_array_carriers`. The only
array-native qualification entry is a `cfg(test)` function inside the verifier:
it runs the existing authoritative `prepare` and `validate`, creates the same
immutable seal, invokes the actual native consumer synchronously, and returns
bounded LLVM/error text and inert accounting. It cannot return a witness, plan,
raw program, declarations or callback. Reference and native consume the same
representation. Public source parsing and route selection are unchanged.

## Implemented behavior

The four raw operations are `ConstructArray`, `ReadIndex`, `WriteIndex` and
`ArrayLength`. Construction and every existing whole-owner transfer dispatch on
the full checked aggregate type. Incoming owned parameters, move initialization,
both replacement states, staging and owned returns copy scalar cells without a
per-element buffer, `memcpy`, synthetic record or array type table.

Empty arrays have positive initialized storage. In particular, `[i32; 0]`
initializes/copies all four sentinel bytes; empty bool/unit arrays use one byte.
Nonempty unit cells use canonical full-byte zero. Bool payloads use LLVM `i1`:
logical bool evidence does not assert that their unused padding bits are zero.
Existing record field-wise transfer and padding behavior remain unchanged.

In guarded mode the operation charge precedes the signed i32 bounds test.
Conversion, stride multiplication, reference-base lookup, element GEP and
load/store occur only in the successful bounds block. Constant invalid and
zero-length indices retain runtime failure paths. Earlier helper effects and
the already evaluated RHS snapshot survive a later fuel or bounds failure;
there is no final store on either failure edge. Bounds uses E0606 and the same
filtered source-origin policy as the reference consumer.

One exhaustive continuation classifier supplies arithmetic/bounds success
labels to both the operation emitter and phi predecessor lookup. A guarded
terminator's success label remains the final predecessor. Bool merges select
the incoming slot pointer before loading, so an untaken uninitialized slot is
not eagerly read. `ArrayLength` remains a full verifier-checked read with its
ordinary charge, even though its emitted scalar is a known length; it adds no
bounds diagnostic.

## Diagnostic and resource accounting

Native keys now distinguish Fuel, Overflow and Bounds together with full
file/start/end. Identical complete keys deduplicate; end-different or
kind-different keys remain distinct. IDs retain first encounter order. A frozen
PR28 baseline of 90 fixture/map/guard combinations (59 distinct modules) matches
all historical candidate LLVM bytes and its inventory TSV. Current projected
sidecars preserve the LLVM bytes and use the explicit physical-resource
successor described below.

The implementation reserves K occurrence rows, deduplicates and sorts in place,
filters against the supplied immutable SourceMap, computes checked
H = sum of the largest surviving valid start in each file, then performs one
monotonic UTF-8 sweep. Filtered spans keep their keys but have no primary
location. A different valid supplied map uses its own path and coordinates.
Both message passes use stored coordinates through the same human-formatting
body; there is no native source-prefix fallback. Each unique message is counted
once, reserved once and rendered once. Raw caller sources use their actual H;
the public loader's 1 MiB source cap is not imposed on raw callers.

Measured rows are occurrence 64, lookup 40, String header 24, Bound 48 and
Diagnostic 136 bytes. Capacity K stays charged after deduplication. The observed
factory transient maximum is 167 bytes; the preallocation gate allows 200.
With K <= 204,097, the successful diagnostic phase is at most 26,511,688 bytes,
below the accepted 26,620,032 envelope and unchanged 32 MiB metadata cap.
Diagnostic text remains separately capped at 16 MiB and LLVM text at 64 MiB.

The earlier 120,832-byte admission expression counted logical vector lengths.
Accounting now measures capacities, including minimum/geometric growth; the
conservative admission-scratch bound is 157,696 bytes. Admission scratch drops
before diagnostic construction except for Bound rows. The occurrence inventory
drops before LLVM emission. Phase separation preserves the accepted envelope
without increasing caps or narrowing old native admission.

`metadata_peak` is an accounted phase bound, including a conservative 32 KiB
emitter scratch allowance; it is not total measured heap/RSS. The sampled
`count_call_scratch_peak` and `render_call_scratch_peak` are retained Invoke
collection subtotals, with fixed names covered separately. New array/phi name
lifetimes fit well below that allowance. The capacity/format/join proof is bound
to the exact Rust compiler commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`.

Explicit new reservation failpoints cover occurrence, lookup, message-header,
each message payload and LLVM-output reserve calls, including zero-length no-op
requests. They do not claim to count every allocator call or recover from
inherited admission/factory/name allocation failure. Partial metrics preserve
already live plan, admission and diagnostic buffers. The independent review
found and fixed a checkpoint1 omission of successful reservations before a
later failure; its unchanged regression now observes the required 808/1,168-byte
partial peaks.

LLVM counting charges each constructor/transfer scalar or sentinel and each
escaped diagnostic byte. These expansion loops stop inside the byte limit;
their checked work ceiling is byte-limit + 1. The transfer inventory is a walk
of actual raw emit sites, not an eager element expansion. Accepted count/render
bytes, expansion categories and ordinary/predecessor counters must agree.
Reverse predecessor scans may finish after a byte denial; their separate bound
is at most 2I, or 200,000 visits per pass. The no-oversized-suffix claim applies
to element/escaped-byte expansion loops, not every ordinary traversal.

## Local evidence and exact scopes

The complete implementation first froze at local commit
`3882ccd174e361ef1a1dcfa030e12740cb139d93`, published as
`8251d2a96dd78e26ed63bd25ec9353b111cc28b5`; both have tree
`ef54c68a3688e855325f5879e55c124fdf6e91d6`. The later test-only lint/privacy delta
has local commit `34c6d42d573f4c4d93c0d6b06ed31b1ccbf5add5`, published as
`61fd84cba002c0cd91da323457fa4896822832d9`, tree
`2a80f40140ad99e79c1fe3fb6f91490b9e0c0865`. The production native, diagnostic and
verifier files are identical across that test delta. Earlier test-source hashes
and their passing receipts remain separate.

Producer results:

| Gate | Observed result |
| --- | --- |
| Debug ordinary, all targets/features | 883 passed, 27 ignored, 17 groups |
| Release ordinary, all targets/features | 883 passed, 27 ignored, 17 groups |
| Formatting and Clippy | Pass; initial five test-only style lints retained, then fixed without changing expectations |
| Witness/privacy compile probes | 42 pass: 7 positive controls and 35 expected denials |
| Debug inherited owned-native LLVM gate | 10 functions pass; 198 ELF artifacts retained |
| Release complete owned-native LLVM gate | 16 functions pass; 722 ELF artifacts retained |
| Debug array-native subset | 6 functions, 259 distinct inputs, 524 ELF artifacts, 7,058 executions |
| Release array-native subset | Same 6 functions/259 inputs/524 artifacts/7,058 executions |

Each array profile also makes 6,793 reference comparisons and preserves 259
original guarded-production modules. These modules are not counted as extra
ELF executions. The six families are small core (195 inputs), maximum width
(39), transfers (9), continuations (8), effects (4) and extreme payloads (4),
with six additional guarded production-wrapper artifacts included in the 524.
Profile repeats do not create new unique inputs.

The array ordinary tests exercise 263 emission profiles, 12 full-type verifier
denials and 18 production-gate controls. Those are not execution counts. Models
and expectations were frozen before candidate observation. The native harness
verifies LLVM 19.1.7 modules, builds real ELF, removes source inputs and executes
with a tool-free PATH. Guarded argv variants preserve the production function
and diagnostic prefix; original modules are retained separately.

The [independent replay package](../../tests/fixtures/fixed_array_unit2d_independent/REPLAY.md)
retains the original historical run of 14 independent functions,
331 ELF artifacts and 1,205 source-free executions on its fresh marked debug
observer. Its disjoint scopes include the 234 core/width cases, nine fuel traces,
341 effect-budget profiles in ordinary/readback ELF, cross-file identities,
18 physical positives and 11 expected-failing one-byte mutants. Physical checks
cover 174 destinations, 37,220 i32 payload bytes, 9,305 bool i1 cells, 9,425
zero/unit bytes and 348 initialized guard bytes. This is the packaged historical
source-bound evidence, not a claim that merely adding the directory ran CI.

The original modules and operation bodies are retained separately from closed
poison/readback instrumentation. All observed bytes are initialized before
instrumentation loads. One-byte mutants are deliberately altered programs and
must fail their unchanged physical checks. No bool-padding or total-memory
claim is inferred from successful scalar results.

## Independent replay and integration history

The original independent package and the authority/extreme supplement preserve
their own source, binary and expectation identities. The latter adds seven raw
denials, five ELF artifacts and five source-free executions. Their historical
336-ELF/1,210-execution sum describes two separate debug runs.

A fresh complete replay then ran all 17 registered functions (eight ordinary
and nine native/physical) against commit
`365fe41198f025925dc678eeb10272a62c48e2ae`, tree
`8b7f42c2e76c8fdc63b165046c5f16c2c82c8ea8`. This one debug replay retained 336
ELF artifacts, 1,210 source-free executions and 3,360 official LLVM command
receipts. Its 29 physical cases, 90 historical IR comparisons, 379 bounds sites
and 28 pointer phis passed their frozen comparisons. The retained 349 LLVM
texts also match their historical counterparts; this is a compatibility check,
not an independently derived language oracle. Profile repetitions do not add
unique source cases.

The replay's fresh marked test binary has SHA-256
`e97f2db73f1407075aef8f9485bebffa4ae35d241fb67afe55d14de1868aebab`.
Its final verified receipt has SHA-256
`7bd2178016fac37910b349c7fff2a9c2399b11f2a9f26386d98953275f4eb254`;
its exact cumulative evidence manifest has SHA-256
`263e9a5c459045a37a03802df27e059273eaeb1ec7b73488f0cc9e28cae9b48a`.
These identify that local run, not a later GitHub head. The published runner at
`f8a30b2443d9d7a89498e0c243f75072cbf26c2f` contains the exact admitted helper
bytes. Run [the full replay command](../../tests/fixtures/fixed_array_unit2d_independent/REPLAY.md)
on a clean exact commit with the pinned tools; the runner creates a fresh
isolated source view and test executable, verifies all copied input hashes,
executes each family once, and seals the original result streams and required
artifacts. A successful completed-run resume only revalidates existing evidence;
it is not another compiler or native execution.

The committed regressions include 29 bounded replay-admission controls, with
missing/changed original checkout or package members, stale binary/source
bindings, extra sealed members and incompatible resume arguments rejected.
Native storage observation and compiler fixtures are separate from these
Python orchestration controls. The physical observer verifies initialized
payloads and guards, including the 11 expected-failing one-byte mutants; it
does not establish bool-padding canonicalization or a complete memory model.

The first real combined recipe on
`13cfd8a8f4f0640286a08bb042fb1f6d6346c218` stopped before native execution:
its producer listed tests with libtest's terse output while the strict roster
validator required the final summary line that pretty output provides. Earlier
synthetic command fixtures had supplied pretty-style bytes while claiming terse
arguments. The successor changes the exact list command and its checked
invocation identity to pretty output, keeping all 16 required names and the
footer mandatory. A regression binds both actual captured output bodies;
original exit status, streams and explicitly incomplete archives are preserved.
The failure supplied no native pass and no later profile ran.

A fresh combined recipe uses the corrected committed head
`1b97ad604dab33a6f27f713ba5e3d2c99f9542ef`, tree
`f67c0ef3a82f322e2ca42895a618aecc77adc589`. One producer operation ran both
profiles, followed by one full independent debug invocation and one full
independent release invocation. Each built its own bound test executable.
All three operations, combined packaging and a relocated compact audit exited
zero. The local event-SHA argument identifies this checkout; hosted event,
run, attempt and runner fields are empty, so this is not a GitHub Actions result.

| Actual local gate | Result per profile |
| --- | --- |
| Producer debug and release | 16 attributed passing names, including all six array families; exactly 1,307 array members |
| Producer array evidence | 524 ELF artifacts, 524 compiled LLVM modules, 259 separate original guarded modules; 7,058 asserted executions and 6,793 source-derived reference comparisons |
| Independent debug and release | 17 passing named functions, 336 ELF artifacts, 1,210 original source-free execution receipts |
| Independent native/physical closure | 3,360 LLVM command receipts in each of two recording layers for the same commands; 29 physical cases, including 11 expected-failing mutants |
| Independent compatibility/structure | 90 frozen IR comparisons, 379 bounds sites and 28 pointer phis; all five phase seals admitted |

The two LLVM recording layers do not double the command count. Producer counts
remain assertion summaries, whereas independent executions retain individual
receipts and streams. Each independent terminal seal contains 29,558 typed
members. An independent read-only audit re-admitted both profiles with process
creation blocked and checked the original producer source/binary/command joins.

The combined index is COMPLETE with no errors and
`complete_unit2e_qualification: true` for this bounded native recipe. Its SHA-256
is `94c62037621fe1ca20adfe8ed72563c19cd6bdcdb7c09c580cac6beba0cb26ec`.
The full archive is 100,607,975 bytes, SHA-256
`22f5dad4b0a6f5e2ba926463c929d147e724ade597c5d15b39e80d1cf927da9c`.
The compact archive is 10,607,005 bytes, SHA-256
`ace7ff648e486c3b58b105d342d92d11917759c60e92f39bf3270503d70f9a8d`,
below its 32 MiB compressed cap. Its relocated audit admits 48,599 selected
members and 14,387 explicit full-only commitments. This verifies retained
original bodies and identity joins; it does not replay full-only binaries or
inspect omitted large IR. The full archive is retained separately without
pruning required evidence.

The [controller contract](fixed-array-unit2e-native-ci.md) defines the exact
source/tool/binary joins, original result retention, partial-failure behavior,
and full/compact archive boundaries. Original failed attempts stay separate
from the successful run. No original source, semantic expectation, native
fixture or production gate changed to obtain these results.

Current-source compatibility also passed its separate debug/release recipe:
3,603 unchanged semantic cases, 21 resource tests and four explicit observer
adapter controls per profile. Its current-only adapter checks the aggregate
carrier and rejects array projections; archived observer bodies and semantic
corpora remain unchanged. Real Linux parser stage08 preparation passed on
`13cfd8a8f4f0640286a08bb042fb1f6d6346c218`. The subsequent `1b97ad60` admission
confirmed all 300 protected binding/workflow bodies unchanged, preserving the
original preparation identity instead of relabeling it as a rerun. The quality
artifact's three additive globs retain the 12 original adapter-control bodies;
a separate relocated 17-body capsule audit checks those with existing metadata.

These completed local runs do not establish success on a later published head.
All applicable exact-head hosted scalar/record/current-source, archived,
portable and native CI jobs remain required, including actual full/compact
artifact admission. The documentation-only successor changes no compiler,
controller, workflow, input or source-binding body.

Any production raw gate opening requires a separate reviewed change. Typed
source-array grammar, public activation, additional native hosts and
self-hosting are outside this Unit2D result.

## Projected-view physical-resource successor

Projected view sidecars increase physical frame admission by two cells per
reference or loan. They do not change activation fuel or historical native IR.
The independent exporter's nine root builders have no references; only `shared`
has a loan (one). Thus its ten guard/source-map rows now report 24 physical cells
rather than 22, while every other TSV field and all 90 LLVM modules stay exact.

The archived exporter and inventory remain unchanged. A reversible, SHA-bound
current exporter adds assertions for those per-builder counts, the independently
listed historical cell totals, the two-cell physical increment and unchanged
activation fuel. It still writes the actual physical `inventory.tsv` untouched.
The reader compares that output to an exact, reversible ten-row successor of the
frozen inventory, never a broad normalization. Both inventory hashes and the
explicit amendment are retained in the comparison receipt and checked by the
full and compact readers. Mutation tests reject every field in all 90 rows,
missing/extra rows, predecessor physical output and exporter/binding drift.

This compatibility successor changes no production resource limit, fuel rule,
LLVM byte expectation or frozen fixture. Hosted debug/release Unit2D execution
and full/compact admission remain required on the published exact head.
