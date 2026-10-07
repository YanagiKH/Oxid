# RFC 0028: private checked scalar HIR import

Status: **accepted private architecture; denied precursor only**.
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

The current disconnected source comparator has genuine producer controls for all
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

The disconnected allocation helper has passed 12 focused tests; 22 source,
framing and comparison tests pass at the preceding immutable fixture checkpoint.
The measured helper named-value envelope is 18,784 bytes on the 64-bit test host,
including its complete Session and typed reserve/fill/finish carriers. It is not
a stack measurement or complete importer price. Caller/source/comparison roles,
actual candidate construction and simultaneous partial-candidate cleanup remain
separate integration obligations. Exact requested capacities and observed
retained capacities are distinguished from allocator metadata or transient
excess capacity, which this model does not bound.
