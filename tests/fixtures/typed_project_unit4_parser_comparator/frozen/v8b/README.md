# Unit4 parser comparator: source-only checkpoint v5

This is an independent Python comparator for the newly approved parser source
contract. It does not modify or execute the compiler or observer, and it has not
read or compared candidate corpus observations. It does not claim recovery of
the historical parser contract or compiler qualification.

## Binding and execution boundary

- Parser package freeze: `7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027`
- Decoded contract: `b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc`
- Durable authority checkpoint: `f2d63a287f4910676306e1eb3165958cc3978151961e6af54e11355def31fa5f`
- Durable authority commit: `96e881116aa5d5cdd6d2aaaf0957f6c736c91571`
- Candidate base commit: `d9e6b9bf172abd5e15da7212c9e6224e29ccc768`
- Reviewed observer v4 source identity: `738dc9b47747889414859b9286e376c762dc75292ed3dedc5306e9eed881f625`
- Original production source manifest: `2e96ea16fe01532366a4af1d69f4fc8c18dfb2cca305da131aced3c9565b167a`
- Pinned independent static observer review: `cca95fe71b0c5271ac721967375537eb5a2984a38f9b7b7569422a6629cf9dfa`

The command-line comparison path requires an externally hash-pinned coordinator
authorization. No such authorization is included here. It verifies the durable
checkpoint contents, exact source identities, review identity, and every file of
this comparator's own checkpoint before opening candidate execution manifests.
Both complete debug/release manifests are mandatory. The exact 248 project cases
and 71 separately executed original-mode relations per profile produce 638 rows.
There is no subset qualification flag. Low-level functions can support an
explicitly non-qualifying, coordinator-authorized smoke diagnosis after review;
such a diagnosis must never be described as a full qualification pass.

Authorization JSON uses schema
`oxid-unit4-parser-comparison-authorization-v1`, status
`approved-for-comparison`, contract_decoded_sha256, package_freeze_sha256,
observer_source_sha256, candidate_source_manifest_sha256, observer_source_root,
and five artifact identity objects: authority_checkpoint, observer_review,
comparator_checkpoint, collector_helper_rebind, collector_helper_review. Each identity is `{path,bytes,sha256}` with an absolute
local path. The observer_review must be the pinned v4 static review; runtime
readiness and independent comparator approval are the coordinator's prerequisite
for issuing the authorization. The authorization itself must be named by its
external SHA256 on the command line.

```
python -B comparator.py --contract-dir ../new-contracts/parser --inventory
python -B -m unittest -v test_mutations.py
python -B comparator.py --contract-dir ../new-contracts/parser \
  --authorization APPROVED.json --authorization-sha256 EXTERNAL_SHA256 \
  --execution-manifest DEBUG/execution-manifest.json \
  --execution-manifest RELEASE/execution-manifest.json --output comparison.json
```

## Implemented checks

`inventory.json` enumerates all 22 expected-field handlers and their case counts.
Every present expectation is conjunctive; unknown expected fields, relation
names, missing observation signals, unexpected observation fields, noncanonical
base64, duplicate JSON keys and nonfinite JSON numbers reject evidence.

The comparator verifies every frozen package file; exact ordered source roster;
actual host, architecture, pointer width and explicit Rust target/profile;
build/overlay/source/helper/binary identities; source and request bytes/seams;
distinct case nonces; zero-execution stdout witnesses; and full raw evidence
hashes. Cargo's emitted executable path and successful build record must agree
with the build receipt. An independent Rust Debug decoder reconstructs complete
normalized ASTs from raw records instead of importing the producer normalizer.

The language-source scanner uses the approved specification's ASCII identifier,
non-nested comment and punctuation rules. All 248 sources fit its explicitly
supported vocabulary. It recomputes the full lossless admitted token tape,
inclusive resource rejections, stored EOF, maximum token length and colon ledger.
It does not invent a complete tape after lex rejection. Original UTF-8 bytes
independently establish all one-based scalar/LF origins, including CRLF and all
diagnostics outside a bounded first-error projection.

Exact diagnostic vectors retain all fields/count/order. Literal predecessor
human-rendering bytes remain literal. Actual JSON bytes must decode to their
complete observed diagnostic records. Bounded projections constrain only their
listed fields. Original/project relations compare complete canonical ASTs,
ordered IDs/node counts and real provenance, or full diagnostic/JSON/human
streams, with two raw mode executions in one actual source generation.

Actual event sequences establish contiguous ordering, cursor/operation source
origins, node attempts/decisions/ledger, checked path overflow and cap-before-node
ordering, sticky recognition with authorized source grammar triggers and no
recovery transition, allocator trace/attempt correspondence, and no append or
reserve after rejected admission until a fresh grammar admission. Required
semantic projections remain exact. AST structure, source spans, local ID arenas,
declaration order and retained token tape receive independent validation.

Field scan rows compare the contract's five semantic fields exactly. Their
additional required scope metadata is validated independently: real entry/exit
sequence IDs, node gate before scan, exact contiguous forward token indices,
terminal-kind/Ident/EOF charge, no repeated initiator, disjoint ranges, aggregate
charge bound, and actual zero source-byte/namespace/reserve/append deltas. Current
token reads are inventoried separately.

## Evidence, limits and review status

`synthetic-controls.json`, `normal-controls.log` and `optimized-controls.log` record 36 fresh test
methods with multiple mutation subcases. Positive controls include complete
synthetic paired mode/profile rows, a complete synthetic on-disk execution
receipt chain, actual-schema field scan/event traces, direct overflow, bounded
diagnostic projections and raw Debug normalization. Mutation controls include
missing/extra/duplicate/zero rows; stale source/helper/binary/profile/host;
zero/unexecuted processes; forged scan indices/EOF/deltas; stale provenance;
event reordering even after renumbering; forbidden recovery activation; allocator
ledger alterations; missing signals; unknown expectations; malformed JSON; and
literal rendered-byte changes.

No contract expected field is intentionally unsupported. Unknown lexemes or
schema variants fail closed. This is implementation coverage and synthetic
evidence, not proof that real observations will pass. Independent review and
real debug/release corpus comparison are outstanding. Public predecessor gates,
source reader limits, loader/index ledgers and native behavior remain separate
obligations. Relational AST checks alone cannot detect a shared regression in
both modes, as the approved contract explicitly states.

Source independence record: expectations came only from the approved frozen
contract, its source-only review, and admitted normative lexical/origin rules.
Observer API, raw schemas and public AST/token type declarations were read for
transport compatibility. One overly broad read of the lexer type declaration
also displayed adjacent lexer implementation lines; those lines were excluded
from expectation derivation. No candidate observations were accessed, no Rust
builds or compiler invocations occurred in this comparator task, and no remote
writes occurred.

Any change to a checkpoint-bound file requires a new checkpoint identity and
independent review before candidate qualification.

## Independent review changes from v1

v1 remains frozen as rejected historical implementation evidence. v2 binds the
derived overlay identity to the independently reviewed overlay manifest; verifies
the actual Cargo working directory, manifest and source entrypoint; and checks
emitted opt_level/debug_assertions/overflow_checks against the required profile.
Synthetic controls rehash altered overlay/source/profile receipts to demonstrate
that redundant original-source/helper identities and argv labels cannot mask
those substitutions.

v2 also binds every module/import recognition to its own declaration admission
and requires top-level public recognition before that declaration's node gate
on ordinary budgets as well as the special denial seams. Both exact independent
reviewer witnesses from comparator-v1-order-findings.json are rejected in
independent-order-replay.json. No candidate records supplied these fixes.

## Independent review changes in v3

v3 strictly enumerates raw envelope, mode-record and AST keys before any
normalization, rejecting binding/schema/canonical alias injection. The complete
AST type algebra validates each arena member and nested collection/scalar field,
including options, IDs, enum variants and boolean types. All parser cursors are
monotonic. A public field's successful scan must finish before grammar Pub
consumption and recognition. Recovery-consumed Pub does not create a public
function/record recognition obligation in either mode. Exact independent raw
and malformed-AST witnesses are rejected in independent-raw-ast-replay.json.

Historically, v3 allowed optional authorization fields collector_helper_rebind
and collector_helper_review together. They admitted only the fixed reviewed v5 optimization-guard helper delta (rebind f0e7899e834c352e53a07a7ea4c093e2aa7b4f84457cdca9df0906dcc61cc099;
review d5e50bbf0c99bef8ad9e421741fb4e61b5b970e26ea8de30713a5a52a6834c22).
The original compiled observer, overlay, binaries and historical build receipts
retain their truthful identities. All286 old/new derived source file identities
must match, the exact build must be listed, and the source checkpoint plus every
new helper byte is verified. Only driver/normalizer admission uses the new
reviewed helper roster. source-only-helper-rebind-check.json records a fresh
identity-only check; it read no raw execution or candidate observation files.

Fresh normal and optimized Python suites are both recorded. The comparator uses
explicit exceptions for admission and does not rely on removable assert checks.

## Retained v4 measured-host admission

v4 requires execution-manifest host_runtime with exact keys os, architecture,
python_pointer_width, uname(system,release,version,machine), python_executable,
and python_version. The comparator independently normalizes the measured uname
values and requires actual Linux/x86_64/64. Raw Rust OS/architecture/usize width
must agree, while the compiled target remains separately pinned. Normalized row
OS/architecture are rebuilt from the measured collector host, not Rust target
constants. Missing measurements, mismatching target labels and unsupported or
incorrectly typed host tuples fail closed.

The two collector artifact identities are now mandatory in command-line
authorization: only the independently reviewed v6 measured-host collector may
provide full qualification evidence. Its rebind is
008b9bfa7f745a2aee67a399a3452fc9fc8f8e15b1961f985241f11b0acfa7b4,
and its review is
03d144546ad638abc4d0c668d6718fc710fd8ce0fa5855342d711fe9e3e7f13e.
The compiled observer source remains738dc9b47747889414859b9286e376c762dc75292ed3dedc5306e9eed881f625,
and historical compilation/passivity records are not relabeled as new host
measurements. This revision changes only host-evidence handling and exact helper
review pins relative to independently accepted v3.

## Retained v5 local decoder admission

v5 preserves all existing comparison, origin, typed-AST, event, resource,
measured-host and source/binary binding predicate bodies. The sole changed
existing function is collector_helpers; normalizer_rebind is a new narrow
identity gate. unchanged-predicate-bodies.json verifies the source AST delta
and preservation of all35 original test methods. The independent comparator
Debug decoder is byte-identical.

Current authorization requires the reviewed v7 helper rebind
9708016e83207a33d27db3ee6a67a4186f05347d5464ecb59aeaad93e1240b8a and
review e655e3740ad8be86f49a2d2f7ebadc84e4b3597dee4a7d218a8ad446b57dca85.
This admits only the explicitly reviewed mechanical producer decoder change:
ca4bb6e661a5197cf340e7f03ad97e50df3222fec6986328f80792ef0c3b8d41 to
89c77305d8b51bebbe71b06c8e3b2d82a09ae76a05f53eadae5e2820d48b2822.
Both endpoints and the exact decoder-equivalence artifact must match the
independent review and their source manifests. Compiled observer bytes and all
286 derived Rust inputs remain bound to their original identities. Stale v6
rebinds, wrong optimized decoder hashes and substituted equivalence evidence
fail closed.

No historical execution manifest or normalized row is rewritten or reinterpreted
by this change. Fresh collection uses the separately reviewed v7 helpers. The
source-only admission verification reads helper source and review/equivalence
metadata, not candidate raw records or AST output. The separate later portable
source/recipe authority plan is outside this immutable local checkpoint.

## Retained v6 first-admission interpretation correction

The approved observer-interface says required_order applies around the relevant
first admission. v5 projected the whole trace and therefore rejected a valid
later recovery retry. v6 changes only that branch of expected_predicates and
adds required_first_admission. All other 21 predicate branches, complete-trace
global event validation, diagnostics, origin, AST, transport and identity gates
are preserved. The whole existing synthetic suite is retained unchanged.

The new predicate derives the production and target cursor from approved source
tokens and the exact seam. It selects the first attempt of that production and
requires its immediate rejection. The complete causal prefix must equal the
source-defined grammar steps: either the first module/import gate; Pub consume
and top-public recognition before the first function/record gate; or the
record gate and Struct/name/open-brace consumes before the first field gate.
Missing, duplicated, reordered or unrelated meaningful events within that prefix
are rejected. No later matching admission can replace the first one. All later
events remain subject to the unchanged sticky recognition, recovery, node,
reserve/append and source-origin checks. A duplicate decision just after the
first boundary is rejected by the full node ledger, while an actual later
attempt/decision pair following recovery is allowed.

42 normal and 42 optimized test methods pass, including six new methods with
all six source grammars, legal later retries, bad-first/good-later substitutions,
unrelated earlier events, wrong first field cursors, missing/duplicate/reordered
semantic steps and extra meaningful events. The unchanged-definition receipt
also mechanically compares every other predicate branch and all original test
methods.

The independent failure-classification artifact was read as the authorized
problem statement. It contains bounded candidate event excerpts; no raw or
normalized candidate file was read and no candidate comparison was run during
this correction. The new causal expectations come from the previously frozen
interface and literal source grammars, not the observed retry indices or output.
The original v5 checkpoint and its failed full result remain unchanged.

A separately authored location amendment is not enabled by this checkpoint.
See amendment-admission-design.md for the planned strict, separately reviewed
admission route. v6 still admits and compares only the original base contract.

## Retained v7b global event repairs

The independent v6 review accepted its first-admission scope and exposed a
separate existing global barrier defect: a failed reserve replaced last_rejection
before the prior barrier was checked. The event could therefore approve itself.
v7 checks the prior rejection barrier before recording any current allocator
failure. No other predicate, source/identity admission or authorization changes.
The original 42 tests are unchanged; four new methods cover successful and
failed reserves after node denial, repeated allocator failures, the meaningful
first failure after an admitted segment, and valid work after a source-valid second module admission under reserve_fail_at=1.
The exact held-out synthetic witness now fails with event-order; its replay is
retained in independent-reserve-replay.json. Both normal and optimized Python
runs pass all 46 tests. No actual candidate comparison was performed.

Amendment adoption remains disabled in v7. A separately reviewed successor may
add the source-authored amendment loader to this fixed semantic baseline.

The preliminary v7 checkpoint is retained unchanged. v7b refines one positive
control to model allocator failure followed by a second module admission,
avoiding the false implication that a targeted node-limit seam replenishes its
budget. Comparator source bytes additionally include the permanent targeted-budget
repair described below; both global-event changes are isolated in events().

The same isolated events() repair now models the targeted node seam exactly:
count matching production attempts, lower the effective budget to current nodes
at the requested occurrence, and retain that exhausted budget for every later
production. Premature rejection, admission at the target, and later admission
after exhaustion fail. independent-seam-replay.json records rejection of the
reviewer’s exact unchanged-source field witness. No production or expectation
value was changed.

## Current v8 reviewed coordinate-amendment admission

v8 adds only authority/loader/result integration on the exact v7b semantic
baseline. All 22 predicate branches, required_first_admission, events, source
origins, AST, raw normalization, execution manifests and their host/profile/
source/seam/observer/build/binary bindings are unchanged. The independent Debug
decoder is byte-identical. No candidate comparison is included in this package.

New comparison authorization uses schema
oxid-unit4-parser-comparison-authorization-v2 and requires
 effective_contract_authority = {checkpoint, descriptor, amendment, review}
where every member is an absolute artifact identity with path, bytes and sha256.
The four artifacts are pinned independently:

- Source amendment checkpoint: 5737321c95c0c293f80f89455024611d250908a4c039364059ca393f33fd2679
- Effective descriptor: 9bf8a80cbb30c384a70fd5696e19428d2b0aad535740e7cbe2ce8992ef96f020
- Single amendment: 70b7228c3b3c3cec074dc10a84e8e338ed9b443f6b95b1be883b9a43b225e451
- Independent source review: a87d877c7e31d76ca0431188e3b862b94432baeb6bf3469017bab65e307ff89f

Before execution evidence is opened, the loader revalidates the complete original
freeze, gzip and decoded corpus, verifies the amendment checkpoint roster,
mutual artifact bindings and independent verdict, and requires the complete
unchanged base document. It binds case index/ID/source, complete old case and
diagnostic hashes and subtrees, and the entire old primary object. It applies
one replacement to a copy, proves the full document differs at exactly the four
reviewed integer leaves, checks the new complete diagnostic and case hashes,
recomputes the unique source Fn/Pub token and scalar positions independently,
and verifies the full effective canonical length/hash. The original document
and every source/limit/seam/mode/profile/observer requirement stay byte-equal.

The effective canonical identity is
c2d4f8db28f7815b3e17ca13fec9a7da70f0923dd052656edb339bc0cd7740cd,
named oxid-unit4-parser-v1-location-amendment-v1. Transport continues to require
original execution contract b19819e2... and freeze 7a2ec4fd... in every retained
raw/normalized receipt. compare_effective_rows verifies the complete effective
document before invoking the unchanged comparator, preventing summary-only
relabeling. Results separately identify execution_contract and all six effective
receipt fields required by the reviewed descriptor. Only a fresh independently
reviewed and newly authorized comparison can qualify this effective authority.

All 46 semantic-baseline tests remain unchanged. Six added test methods cover
real source-only admission, unchanged base/execution identity, exact complete
artifact pins, stale/duplicate/wrong-base/wrong-source/already-applied authority,
noncoordinate and incorrectly typed deltas, wrong final identity, summary-only
relabeling and legacy authorization rejection. All 52 methods pass in normal and
optimized Python. The exact two held-out global-event witnesses still reject.
No candidate observation file or production source was used to choose amended
coordinates; those values came only from the separately source-authored and
independently approved amendment.

## Documentation-only v8b checkpoint

v8b corrects the preceding historical sentence about preliminary v7. Every
Python source and test byte is identical to v8, whose retained 52 normal and
52 optimized test runs remain the validation evidence. No tests or candidate
comparisons were repeated for this documentation-only correction.
