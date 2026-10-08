# RFC 0028: private checked scalar HIR import

Status: **private Verify, Run, LLVM-text Emit and bounded external native execution locally qualified**.
Public provider integration remains outside the enabled scope.
Updated 2026-10-07.

## Outcome and boundary

Connect the bounded Oxid scalar frontend to the compiler's real scalar pipeline
without treating an observation as executable authority. Keep the existing
OPA1/STF1 bytes, single-file 128-byte ASCII grammar and all compiler limits.
The initial implementation is a private, permanently denied decoder/source-owner
and storage precursor. It cannot construct candidate HIR, a typed program, checked source
program, verified OIR or native artifact. Positive private import requires a
separate measured-boundary review; public provider selection is not included.

The complete proposed bridge would reconstruct candidate-owned resolved HIR,
compare every field with genuine source resolution, then pass that candidate
value through the existing `typeck::check`. Compare all supplied type and flow
facts before ordinary lowering, source association and independent OIR
verification. The existing checker and sealing leaf retain exclusive witness
construction. Never repair missing or disagreeing facts, substitute canonical
HIR on mismatch, or expose an unchecked owner/callback API.

## Source identity and supported requests

Use the actual immutable `SourceOwner`: the original adapter must retain parser
allocation/map identity, and public syntax must use genuine root-only
`ProjectSources`. No fake project, guessed source-file ID, reopened path or digest
can substitute for the source owner. A request carries the exact bytes captured
from that owner; byte equality alone does not manufacture parser provenance.

OPA1/STF1 does not contain a full source copy. The private entry must bind its
borrowed observation to the same retained source/AST and validate complete
source/AST correspondence before any future import. Only a complete successful
transport and the exact 2,607-byte tag-0 form can become a success candidate.
First-diagnostic, pending/probe, refusal and transport-failure forms grant no
import authority. The initial decoder's framing checks are explicitly not a
complete AST or semantic validation result.

The source domain remains the entire existing bounded scalar grammar, including
public functions, library input and recursion. Root-only project syntax is
supported without enabling multi-file imports, aggregates or new syntax.
Existing production diagnostics, including multiple diagnostics across
functions, remain canonical. The observation's first-diagnostic projection is
not a replacement public diagnostic list.

## Passive storage plan

Before future candidate allocation, count all retained HIR vectors and nested
parameter, statement and call-argument vectors. Use checked arithmetic, exact
planned capacities and actual element/carrier layouts. Price any coexistence of
source AST, canonical HIR, candidate HIR, row maps, continuation state, typed
results and enclosing return/call carriers. Distinguish retained canonical
capacity from the candidate's planned reserve. No unchecked wire count grants
allocation permission.

The first plan is passive and cannot authorize construction. Record actual
layouts and observed allocation behavior before private success is enabled.
All new reserves must later be fallible and accounted before allocation, with
cleanup controls. This is an affected-importer model, not a whole scalar-HIR,
stack, allocator or RSS cap. Do not broaden inherited accounting internals.

Keep the default path and existing owner layouts unchanged. If a new carrier
changes default admission, stop and document that exact effect for explicit
review. Do not hide new bytes or raise any ceiling. The existing Oxid consumer's
I = 7,741 / W = 3,907 remains its own measured admission; it does not price new
compiler code or a separate Oxid lowerer.

## Denied precursor invariants

- Private entry returns a result with an uninhabited success type
- Genuine canonical resolution may run only to describe the passive plan; no
  candidate HIR construction or checker/lowerer/verifier/native call is allowed
- Genuine source-owner and captured-source checks precede frame interpretation
- Exact frame lengths, tags, row counts and bounded checked reads are enforced
- Basic framing or a passive plan never implies complete source/AST validation
- No public option, process launcher, retained checked-owner field or alternate
  witness is introduced
- Existing public/default behavior, fuel, ABI, error order and limits remain
  unchanged; no default consumer calls the precursor

## Enablement evidence

First save the denied implementation, measured complete carriers and passive
allocation plan. Check actual original and genuine public-source paths, empty
and nonempty programs, wrong captured source, stale/same-content replacement
owners, every framing boundary, checked arithmetic and unchanged default
carriers. Independently review the absence of construction authority and the
allocation/coexistence model before deciding to enable a private success path.

That later gate must reject structurally plausible wrong bindings, callees,
literals, types, flow masks, loop targets and source relations; retain authentic
checker construction from the candidate-owned value; and use the existing
sealed lower/associate/verify sequence. Actual reference/native output must
match the canonical route where admitted. Library entry, recursion, host,
arithmetic, fuel and later native-limit rejections stay distinct failures.

## Compatibility and delivery

No public language, wire version, provider switch or ABI changes in this
precursor. Production Rust changes require a current-source qualification
successor before publication as a completed capability; historical authorities
and failures remain immutable. First use focused denial/layout tests and a
bounded independent review, followed by one coherent qualification milestone.
The supported native target remains the existing Linux x86_64/LLVM 19.1.7 scope;
portable framing tests do not claim native support on other hosts.

## Initial measured precursor

The denied probe now uses genuine canonical resolution only to describe retained
HIR storage. Basic framing recognizes actual original/public producer outputs
with 29 rows and item head 1; it does not compare their full AST or semantic facts.
Every path still terminates in rejection, including valid successful artifacts.
An owner-derived immutable source slice remains tied to the real source owner.

On the measured 64-bit host, SourceOwner/Wire/bound-request are 32/24/72 bytes;
Counts/StoragePlan/ProbeFacts are 64/152/96 bytes. Rejection and its uninhabited
Result are 104 bytes, while the plan Result is 160 bytes. The existing checked
source carrier remains 320 bytes. These measurements do not establish a complete
new caller/construction/coexistence budget.

The 113-byte nonempty fixture requests eight-family slot counts
`[2, 2, 1, 2, 11, 5, 7, 2]`. Actual canonical capacities are
`[4, 4, 8, 8, 20, 5, 7, 8]`: 3,472 retained payload bytes versus 2,241 hypothetical
candidate requested bytes. Adding two 48-byte Program headers gives a hypothetical
HIR pair of 5,809 bytes. Actual candidate allocation remains zero. No reserve,
allocator overhead, source/AST baseline, future typed storage or full compiler
memory admission is inferred from that subtotal.

Eleven focused tests cover real producer framing, wrong source/owner routes,
excluded source domains, reserved cells, overflow and passive capacity counts.
Complete import validation, named coexistence accounting and actual candidate
reserve/failure evidence remain separate requirements before private success.

## Accepted still-denied comparison stage

The next private stage may completely compare source/OPA rows and reconstruct
candidate-owned resolved HIR only inside a closed comparison leaf. Before any
candidate reserve, admit its exact nested vector requests, actual canonical
capacities, comparator maps/scratch and complete affected coexistence carriers.
Use existing fallible exact reserves, reject observed capacity mismatch, guard
all fills and prove failure cleanup. Source/AST/input ownership is the explicit
already-owned baseline; this is an affected-storage model, not process RSS.

Candidate fields must come from validated observation data, never repaired from
canonical values. Compare the entire candidate with genuine source resolution,
then drop it before returning fixed denial facts. The enclosing result stays
uninhabited on success. Type checking, lowering, verification, execution, native
emission, public provider selection and candidate escape remain prohibited.

A private fixed-storage source/OPA comparison module is the first implementation
checkpoint of this stage. It is compiled but is not yet connected to candidate
allocation. Its actual event/map/result/caller layouts and bounded work must be
measured and charged before consumption; a source file alone is not admission.

### Bounded comparison preflight

At the source-comparison checkpoint, the disconnected comparator has genuine producer controls for all
36 scalar row kinds. A stale cached project-syntax summary cannot authorize an
original-owner route when a function's actual public field is present. This is
checked after bounded domain admission; public input continues to require the
genuine project owner.

Before byte comparison, reject different capture/source lengths, then reject an
equal length over 128. Thus an oversized, equal-length but different capture now
returns the private Domain boundary instead of scanning its bytes and returning
Source. No public diagnostic or default entry changes. Token/item counts and
parameter/argument lengths are also bounded before their scans. The complete
comparison must precede canonical resolution in the future consumed leaf, so
mutable AST fields are checked against source before resolver assumptions apply.

The comparator's 2,690 maximum visit count is only its original visit model.
Source binding, domain checks, literal and child-height scans, framing and final
claim scans require additional bounded work charges. Combine those with genuine
canonical resolution and candidate work under the existing work ceiling before
connecting candidate allocation.

At the allocation-helper checkpoint, the disconnected helper passed 12 focused tests; 22 source,
framing and comparison tests pass at the preceding immutable fixture checkpoint.
The measured helper named-value envelope is 18,784 bytes on the 64-bit test host,
including its complete Session and typed reserve/fill/finish carriers. It is not
a stack measurement or complete importer price. Caller/source/comparison roles,
actual candidate construction and simultaneous partial-candidate cleanup remain
separate integration obligations. Exact requested capacities and observed
retained capacities are distinguished from allocator metadata or transient
excess capacity, which this model does not bound.

### Complete comparison leaf before execution

The next compiled leaf remains uninvoked and its construction controls remain
ignored pending carrier measurement and review. It has an uninhabited success
result and returns only fixed comparison or mismatch facts after dropping both
candidate and canonical HIR. It does not replace the earlier passive probe or
connect a default compiler caller.

Its sequence is fixed-cost preflight, bounded source/OPA comparison, genuine
canonical resolution, observed canonical-capacity plus exact candidate reserve
admission, candidate construction, exhaustive HIR equality, and owner teardown.
Source/AST/capture storage is an already-owned baseline. Canonical resolution's
inherited allocator behavior and diagnostic storage are not presented as a new
general HIR memory cap. Actual canonical vector capacity is included with new
candidate payload in the affected comparison bound before candidate reserves.

The source/OPA prepayment is 124,501 logical units. It includes oversized and
malformed rejection prefixes, full bounded lexical/literal scans, row decoders,
stack operations, child-height checks and final claim scans. It is separate from
machine instructions and replaces no public fuel rule. The existing resolver's
meter-taking helper becomes visible within the frontend solely so the private
leaf can share one actual work meter; its implementation and default callers do
not change. Builder and allocation work must fit the same remaining allowance.

The allocation helper owns its typed vector transports; the builder owns its
candidate Program header; the outer leaf owns the canonical Program header,
source/request/result roles and full allocator/meter owners. The complete source
comparison bank is added separately. Named envelopes conservatively add roles
across phases and are not claims about optimized stack size. No construction
execution or full failure-cleanup qualification follows from compiling these
carriers; those remain the next measured gate.

### First paid construction controls

The combined leaf's 64-bit test-build measurement is 72,700 named fixed bytes:
42,073 for source/OPA comparison, 18,784 for allocation support, 7,259 for the
candidate builder and 4,584 for the outer leaf. The outer facts/rejection/result
carriers are 192/200/200 bytes. Forty focused controls passed before construction
was exercised. These figures include conservative transport roles across phases,
not allocator overhead or optimized stack usage.

The first construction controls now call that complete leaf, including source
prepayment and the same canonical resolver meter. They no longer call the builder
with a zero outside charge. A structural match still returns a fixed rejection
variant; a plausible but wrong supplied literal returns the mismatch rejection.
Logical reserve failures and exact shared-work boundaries are tested separately
from the required independent allocator-null and live-byte cleanup evidence.
Type checking, lowering and executable authority remain unavailable.

### Candidate-only cleanup observation

The first actual paid group passes 45 focused controls, including all seven
original-source producer cases and 16 logical reserve-failure positions. Full
mismatch and failure cleanup now receives an independent candidate-only heap
window using the existing test allocator observers. Canonical resolution and its
allocator trace finish before that window; candidate vectors are dropped inside
it. Trace backing is prepared separately. The forwarding action returns no
owner and reports only primitive attempt/success/live/peak values.

This instrumentation exists only in test builds. Its fixed thread-local last
observation is reset at every leaf entry, including early admission failure.
Nested observer scopes are rejected. Existing guards reset during unwinding;
panic-hook allocations may precede that reset, so unwind controls assert recovery
rather than numeric panic-path counts. No production allocator field or hook is
introduced. Instrumentation is excluded from the affected production ledger.

The rich fixture's independent 16-request oracle totals 2,241 bytes. A successful
comparison or full mismatch should show 16 attempts, 16 successful allocations,
zero live bytes and a 2,241-byte peak. A real-null failure at request k should show
k attempts, k-1 successful calls, zero live bytes and only the preceding request
bytes at peak. Logical failure differs: it makes no kth GlobalAlloc attempt.
These are explicit expectations for the new controls, not claimed results before
execution. Wrong-binding/callee/loop/annotation controls mutate only resolved
facts, while source and OPA remain intact. At that predecessor checkpoint,
type/flow import remained closed.

## Accepted private Verify phase

Status: private Verify success entry is enabled after the closed layout and
independent boundary review. Focused execution qualification has passed; the
still-denied comparison controls remain the predecessor evidence.

Add a closed Verify-only request that owns the newly constructed candidate HIR.
After complete resolved-HIR equality, release its allocation Session and drop
canonical HIR before calling the genuine `typeck::check(candidate)`. That pass
remains the sole TypedProgram constructor. Compare every supplied STF1 type and
flow cell through immutable typed views before any lowering. Expose the existing
four flow booleans read-only and test all 16 combinations; do not parse Debug text
or change flow representation. A mismatch rejects without repair or fallback.

Then use ordinary scalar lowering, source association and independent raw OIR
verification, in that order, entirely inside the terminal leaf. Return only
fixed verification facts after every HIR, typed and OIR owner has dropped. No
candidate/typed/verified owner, callback, source owner or reusable witness may
escape. Do not add a provider option, change the sealed checked-source owner,
connect a default caller, or enable Run/Emit in this phase.

For one genuine root module without imports, `Declarations::Original(root_ast)`
is permitted solely as the bound AST count/order/name projection for association.
It does not construct an original-flavor SourceOwner or fabricated index. Review
and test that projection against the genuine project index, including public
functions and a helper before main. Source maps come only from the bound owner;
no independently supplied replacement map is accepted.

Retain ordinary downstream passes' inherited allocation behavior and limits,
and measure their coexistence separately. All new importer storage remains
prepaid/fallible under the accepted affected-storage model. The old HIR-pair
subtotal does not price typechecking, lowering or verification. Drop predecessor
owners at the documented phase boundaries; add complete actual request/result,
typed-comparison and consumer carriers before execution. Prepay new compile work
on the shared meter before downstream operations. Runtime fuel is unchanged.

First compile and measure the complete new layouts, then obtain independent
boundary review before executing Verify success. Run is a later gate after
Verify proof review. Emit additionally requires a private pre-allocation final
text bound. Current-source qualification and public integration remain separate.

## Private Verify execution evidence

The contained terminal has passed genuine original/public fixture verification,
complete supplied type/flow comparison, and ordinary source-associated OIR
verification. All 36 row kinds are covered; 1,178 decoder-valid supplied-fact
mutations reject before lowering. Explicitly synthetic empty/self-recursive/
mutually recursive controls establish the same boundary for library inputs.
Authentic typechecker diagnostic vectors preserve all fields and function order.

For the rich fixture, exact private admission is 95,890 retained affected bytes,
90,177 fixed scratch bytes and 1,276,867 shared compile-work units. Each exact
threshold passes and each minus-one rejects before candidate reserves and
checking. This is importer accounting; inherited pass capacities are measured
separately and are not a whole-stack, heap or RSS limit.

Whole-pipeline observation establishes zero retained live bytes after rich
success and all 16 logical and real-null candidate reserve failures. Diagnostic
and typed-mismatch exits are executed and reviewed for ownership cleanup; they
do not carry a separate whole-pipeline zero-live-byte measurement claim.

Local bounded-profile qualification passed 1,570 unit tests with 55 intentionally
ignored tests, strict all-target lint and formatting. Public-project positives
are Linux-only under the existing loader policy. Hosted current-source binding
qualification and private Run/Emit activation remain separate gates.

## Accepted private Run precursor

Status: private Run is enabled after complete carrier measurement and independent
boundary review; focused execution qualification has passed. Emit/provider
integration remains outside this stage.

A finite private request may select Verify or Run inside the same checked
construction terminal. Run must first pass complete source/OPA/resolved-HIR and
supplied type/flow comparison, genuine checking, ordinary lowering, source
association and independent OIR verification. Only then may it call the existing
immutable `VerifiedProgram::run`. Drop that owner before returning the fixed
scalar/runtime result. No arbitrary entry index, owner, callback or witness may
cross the request boundary.

Derive original-root main solely from the bound root source/AST and the complete
checked candidate function correspondence. Retain only its optional DefId before
moving candidate HIR into the checker. Compare this projection with the genuine
project declaration index for public, helper-before-main and library cases.
Missing main and wrong arity retain the existing runtime entry diagnostics.

The existing reference engine keeps its default 1,000,000 logical fuel, 1,024
frames and 200,000 live slots, arithmetic behavior and failure order. No custom
fuel interface or interpreter rewrite is introduced. Runtime fuel is separate
from the shared compile meter. Prepay the bounded root-entry scan on that meter;
new request/result/call carriers must use measured complete types and the same
unchanged private storage ceilings. Any resulting admission delta is explicit.

Inherited runtime allocation remains the ordinary engine's behavior: its frame
vector, each frame's slot/place vectors and temporary call argument values.
Measure actual layouts and retained capacities in their owning implementation
with test-only fixed observations. This is separate from new importer storage
and does not establish a general heap/RSS/stack limit or new allocator policy.

Keep the Run entrance hard denied until complete carrier measurement and an
independent boundary review pass. Then separately qualify fixed results, entry
identity, arithmetic/runtime failures, fuel/frame limits and owner cleanup
against the ordinary source route before considering further activation.

### Denied Run carrier observation

The compiled denied precursor measures Request at 1 byte, Context at 40,
WorkPlan at 56, terminal facts/result at 288 and outer facts/result at 320 on the
qualified local 64-bit target. Complete enclosing-carrier pricing changes the private Verify fixed envelope
from 90,177 to 94,955 bytes and rich retained admission from 95,890 to 100,668
bytes. This includes the complete 40-byte source lookup and 32-byte source text
call argument carriers, plus their ledger-array growth. The shared construction carrier also
changes the private Observe fixed envelope from 74,924 to 75,356 bytes. These are
explicit private admission successors under the same ceilings; public/default
source admission does not use these disconnected leaves. Verify compilation
work remains 1,276,867 for the rich fixture, with entry_work zero and no runtime
outcome.

The separately measured inherited interpreter Frame is 144 bytes. Its ordinary
1,024-frame vector capacity retains 147,456 bytes; the rich call's maximum sampled
simultaneous vector payload is 147,592 bytes and the recursive frame-limit case
is 155,648 bytes. Missing/wrong-arity main creates no observed runtime storage.
These are successful-allocation capacity samples, excluding allocation metadata,
failed partial construction and realloc transients. They do not replace the new
importer ledger or establish a total memory bound. The 86 focused entry/denial, admission, comparison and Verify controls passed,
and independent review cleared the corrected accounting boundary. Private Run is now enabled for its separately authorized focused execution
qualification.

## Private Run execution evidence

Imported and ordinary original/project execution agree on scalar results and
complete runtime diagnostics, including spans and rendered JSON. Controls cover
genuine rich/public/library captures and explicitly synthetic bool/unit,
entry-error, arithmetic-error, fuel-exhaustion and frame-exhaustion inputs. Both
modes reject all 1,178 supplied-fact mutations before runtime. Ordinary limits,
entry policy, arithmetic and runtime fuel are unchanged.

Rich Run's shared compile work is 1,310,659, including a 33,792-unit root-entry
scan. Exact work/storage thresholds pass; each minus-one rejects before candidate
reserves, checking or runtime. Its imported-leaf allocation interval has 148
successful allocations, zero live bytes at return and a 155,224-byte peak.
Existing arithmetic/fuel/frame failure controls also return fixed errors after
zero-live owner cleanup. Source/AST/capture and prepared validation trace remain
outside those intervals; no general OOM or total-memory guarantee is claimed.

The final clean local successor passed strict all-target lint, formatting and
1,594 unit tests, with 55 intentionally ignored tests. Independent review verified
the receipts and closed the requested Run boundary/execution scope. Emit,
public/provider integration and current-source hosted qualification remain
separate gates.

## Accepted private Emit feasibility precursor

Status: feasibility and denied implementation only. Verify/Run remain qualified;
private Emit success, public/provider integration and native-tool invocation are
not enabled by this stage.

Reuse the scalar emitter's authoritative count/render passes. Add only a fixed
private admission mode at the existing count/allocation seam. Preserve default
emitter bytes, errors, limits, Result ABI and runtime fuel; do not add another
serializer or use a post-allocation length check as admission.

Before the final output String reserve, admit its exact counted byte length plus
complete new importer/carrier coexistence under the unchanged private retained
ceiling. Use one fallible reserve and reject unexpected capacity or render growth.
The eventual artifact may be the one admitted owned LLVM String, returned only
after all compiler/verified owners drop; no source or witness reference escapes.
The String payload remains one allocation across moves, while all complete
request/result/enum/call transports must be measured and priced.

Counting does allocate inherited native metadata and temporary diagnostic/label
Strings. Observe those separately; the final-buffer preflight is not a claim of
allocation-free counting or a whole-memory cap. Display paths are not bounded by
the 128-byte source domain. A checked path-dependent compile-work allowance must
be prepaid on the same meter before native admission/count/render, with practical
headroom shown under the existing ceiling.

Keep Emit hard denied until complete carrier pricing, the work bound, default
behavior preservation and private no-growth checks have independent review.
LLVM invocation and executable publication require a later explicit stage.

### Hard-denied Emit transport checkpoint

The first transport checkpoint added a compiled Emit request and an unboxed
owned-text artifact alternative. Literal false gates remained at the leaf,
common dispatch, candidate entry, direct construction and terminal entry, with
an additional Disabled terminal branch. No emission work was connected there.

Verify and Run keep their existing outward fixed-facts types and work semantics.
Complete shared enum, input and Result carriers nevertheless grow. Their actual
layouts are included in the enabled comparison/Verify/Run named ledgers, so this
is a new private storage-admission successor under the same ceilings. Previous
private byte thresholds are not promised to admit. Exact measurements and fresh
boundary controls are required before qualifying this successor.

The future artifact has no borrowed source lifetime. The terminal must drop its
verified/compiler owners before returning it; qualification must inspect the
String after the caller's genuine SourceMap/AST backing scope ends. Dropping the
Copy SourceOwner adapter alone does not destroy that backing. Default compiler
routes, source ownership and public/provider selection remain unchanged.

### Hard-denied paid Emit connection

The next compiled connection retains every false entry gate and adds a private
terminal child which receives the existing genuine VerifiedProgram by move.
Neither the importer Emit constant nor the scalar native Emit constant is
enabled. The entire source/candidate/checker/STF/lower/associate/verify sequence
is preserved; there is no alternate constructor, witness or public route.

Emit creates one original WorkMeter and pays a separate 32,768-unit fixed
connection allowance before calculating its new source-plan/carrier banks.
That same meter continues through source comparison, resolution, candidate and
terminal; WorkPlan does not charge the connection allowance again. The new
inventory work has at most 322 rows at 32 units, eight fixed prologues at 128,
and 144 other fixed connection handlers at 128: 29,760 units within 32,768.
This proposed actual-code tariff remains subject to independent review.

An explicit unmetered constant-time bootstrap obtains the origin from the
genuine root file, checks owner count/view, caps the work limit and constructs
the meter. It performs no bank calculation, source/path walk or compiler
consumer work. No fabricated owner or guessed origin supplies authority.
Verify/Run preserve their old preflight order and compile-work amounts.

The immutable OIR scan pays on the
original meter at its existing boundaries. The actual owned root display-path
UTF-8 length is then read without scanning or allocating. A distinct 4,096 debit
precedes the checked formula, and its complete body cost is debited before the
native call. No stage reuses scan setup, resets the meter, or changes runtime
fuel. Connection, scan, formula and body costs are separate artifact facts.

The two new fees total 36,864; old passive work examples omit them. For the rich
fixture's historical 1,310,659 source/check/entry prefix, the analytical p=7 total
is now 10,069,187. A p=3,365 path totals 255,928,515, while p=3,366 totals
256,001,731 and fails the unchanged 256M work ceiling. These are isolated paid
prefix/formula controls, not successful private Emit executions or native
admission/output-cap guarantees.

Before any future Emit consumer, its source plan admits the complete common and
Emit-only carrier banks, including scan/formula/native and owned artifact
transports. The completed Session receipt carries F. After Session drops its
allocator borrow, native admission receives F minus the separately added native
bank, so final preflight checks exactly F+N once. HIR-pair admission remains
Hc+Hn+F. Verified OIR, native metadata/temporaries and genuine source backing are
observed inherited heaps, not secretly included in the affected-importer cap.

Both gates stay false while complete successor layouts, fixed bounds, default
regressions, independent review and later private-output qualification remain
pending. No native tool invocation or publication is part of this connection.

### Private LLVM-text activation

The reviewed denied precursor has passed its local fixed-carrier, payment-order,
provenance, default-regression and standalone String-null checks. Its two private
Emit gates are now enabled for focused qualification. No successful private
Emit, final-text allocation-failure recovery or source-scope independence is
claimed by this activation checkpoint. Those checks must run against actual
imported emission before the step is qualified.

The scope is owned LLVM text only, using Result policy and unchanged default
emitter limits and errors when private budgets suffice. Public/default routing,
provider selection, external LLVM tools and native executable execution remain
outside this step. The historical denied controls and their receipts remain
evidence of the preceding boundary; activation needs corresponding successor
controls through the fully paid source leaf.

### Local LLVM-text qualification

The private text route has passed its focused qualification. The genuine rich
producer observation yields 14,325 LLVM bytes equal to ordinary source emission,
with 16 candidate reservations and one final String reservation. The artifact
remains valid after the actual source/AST backing scope ends. Its original-meter
work is 10,069,187. Genuine project/public observations and explicitly synthetic
scalar/entry controls preserve ordinary text or complete native diagnostics.

Actual work and retained-byte boundaries, 326 supplied-fact mutations before
lowering, final-buffer logical and real-null failures, same-allocator recovery,
no-growth and complete output/error teardown controls passed. The 3,365-byte
path succeeds at work 255,928,515; the 3,366-byte path is privately work-rejected
while ordinary emission remains admitted. These outcomes retain the declared
private admission differences and inherited-allocation exclusions.

Local validation at the qualified source includes 18 focused Emit controls,
1,649 passing unit tests with 55 explicitly ignored, strict all-target Clippy
and formatting checks. This is bounded-debug Linux evidence on Rust 1.99.0,
not hosted or standard-profile qualification. No external LLVM tool or native
executable ran in this step; no public provider/default source route changed.


### Local external native qualification

The opt-in [private native gate](../docs/architecture/private-hir-import-native-gate.md)
now consumes the unchanged owned leaf artifact after actual source/AST teardown.
At local test checkpoint `4a0c1e2`, LLVM/Clang/LLD 19.1.7 assembled, independently
verified and compiled both private and ordinary rich/overflow/division artifacts
into six Linux x86_64 O0 PIE executables. Every status/stdout/stderr result matched
the ordinary source reference and fixed value/error expectations. Three disposable
malformed LLVM controls were rejected by the real external verifier.

The genuine rich producer capture remains distinct from explicitly hand-authored
arithmetic controls. This is a private test-only artifact gate, not production
CLI/publication, public provider, hosted or current-source binding qualification.
Standard tests never invoke the gate. Clean local regression at this checkpoint
passed 1,649 unit tests (56 intentionally ignored), the 18 focused Emit controls,
strict all-target Clippy and formatting. The bounded-debug Rust 1.99.0 profile
and inherited resource exclusions remain unchanged.
