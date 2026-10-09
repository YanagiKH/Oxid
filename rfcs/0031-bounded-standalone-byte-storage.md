# RFC 0031: bounded standalone byte storage

Status: **parent-approved bounded implementation contract; resource admission and final qualification remain open**.
Date: 2026-10-09. Owner: bounded-byte implementation task. Reviewer: parent and independently assigned semantic/resource reviewers.

Exact dependency: merged main `b5455ad07081590aeffdec54a927c9095e870fb0`, tree `cb53acc04ac88538a74410bc0a3c037da369c02e` (PR56). Its PR-head CI and separately running postmerge CI are predecessor evidence, not evidence for this increment.

Authority: independently reviewed planning proposal SHA-256 `7efc3b0c93444893af94f58237088a8c16ec3ce9f114bae95fed2effc3dbf38a`; original Oxid roadmap M2/M3, M4 byte buffers, AI-A1 and sections 6–7; predecessor RFCs 0016, 0019, 0020, 0022 and 0030. This RFC carries the full bounded proposal forward. Historical observations below remain explicitly historical. The current worktree verified all listed grammar/type/storage fences on the exact dependency. The semantic contract was frozen before implementation.

This is a bounded M2/M3 increment, not completion of M2, I/O migration, tensors, self-hosting, memory-safety certification or v1. There is no artifact/ABI/protocol version change and no implicit legacy migration. Only the explicit stable-Rust typed-preview accepted source domain expands. RFC0030 byte aggregate exclusions are superseded only in the standalone positions defined here.

Parent review must approve this contract and `docs/architecture/bounded-byte-storage-plan.md` before implementation. Resource review remains a separate veto gate after measurement; approval of semantics cannot waive admission. Publication and merge are owned by the parent.

## 1. Reviewed scope decision and stop line

Extend the existing fixed scalar array and call-only scalar-slice contracts with exactly one element type, u8:

- `[u8; N]`, including existing exact fixed-array call borrows, with N in 0..1024.
- `&[u8]` and `&mut [u8]` parameters backed only by a complete admitted fixed array.
- Existing by-value move, constructor, indexed read/write, i32 length, whole-owner borrowing and explicit reborrow behavior.
- Standalone byte-array owners only. Record-contained byte arrays and byte-array record projections are explicitly deferred to a separate future extension, not conditionally included.

No arithmetic on u8, new literals, contextual integer coercions, direct scalar u8 record fields, record-contained byte arrays at any nesting depth, enum payloads (including byte-array payloads), heap buffers, nested arrays, subslices, ranges, element references, borrowed locals/results, aggregate field extraction/replacement, general conversion receivers, ABI promise, I/O signature changes or provider expansion.

The parent decision after independent review is this smaller standalone-array slice. Existing record composition and projected slices for bool/i32/unit remain unchanged. Reusing FixedArrayTy is not permission to admit byte-array record fields: dedicated source and raw declaration fences are required, including unused declarations and transitive nesting. No byte-record projection is a pass fixture in this increment. If the standalone slice cannot be honestly admitted under existing ceilings, stop for a revised design rather than raise limits.

Implementation only follows parent approval of this frozen contract. PR56 dependency resolution is complete at the exact merged identity above. Completion means reviewed public source/reference/native vertical slice, required repository checks and all applicable CI green on the exact eventual PR head, followed by the separately authorized publication/merge process.

## 2. Language contract

Array element grammar becomes bool | i32 | u8 | (); decimal lengths and punctuation are exactly RFC0016. No keyword change: u8's existing primitive/type-name reservation and ordinary function/value namespaces remain RFC0030. `[u8; 0008]` equals `[u8; 8]`; signs, separators, expressions and named lengths remain excluded. Parse bounded numeric length before allocation; source/token checks retain precedence. Length 1025 is E0400/parse at the complete length token; malformed punctuation E0100, recognized excluded forms E0101.

In all examples, `byte` is an ordinary explicitly declared helper: `fn byte(x: i32) -> u8 { return x.to_u8_checked(); }`. It is not a builtin, reserved name or new conversion spelling.

Nonempty literals infer the first scalar element, then require exact equality. `[byte(0), byte(255)]` has `[u8;2]`; `[0,255]` remains `[i32;2]`; `[byte(0),1]` is a mismatch at 1. A `[u8;2]` annotation does not reinterpret i32 literals. Element evaluation is once, left-to-right; destination initialization waits for all successful elements and paid construction. Empty literal admission is only the existing explicitly annotated zero-length local initializer, including grouping: `let a: [u8;0] = ([]);`. No context inference for an empty constructor field, return, call argument or assignment. Construct a typed local first, then move it.

Arrays are structural `(element,length)` types and move-only even at length zero. `[u8;0]` differs from `[bool;0]`; same byte size is never type compatibility. Exact borrows require exact array identity. Length erasure to slice is only at direct explicit borrow arguments. Element and borrow modes match exactly; slice-to-fixed conversion and implicit reference forwarding remain forbidden.

Index type and len result are i32. A byte index is rejected, with no promotion. Read returns a copied u8 snapshot; write requires exact u8 RHS and a mutable available owner/exclusive authority. No array equality, printing or executable array main return is added. `check` retains existing value-result admission, while run/native entry adapters reject unsupported result types before consumer/tool/output creation, using existing E0600/E0700 rules.

Conversion grammar stays named locals/by-value parameters only. Write `let b = a[i]; return b.to_i32();`, not `a[i].to_i32()`. A byte-producing helper may be a literal element or assignment RHS. No unchecked truncation or implicit literal conversion creates a byte.

Named-root record array access and projected slice borrowing remain available only for their predecessor bool/i32/unit arrays. A byte array cannot be stored in a record, projected from one, or supplied by a record-field borrow. This holds for whole-record reference projection syntax too. A future record-contained byte extension must separately review nominal/privacy paths, layout, view provenance and resources; none of those new-byte behaviors is activated here.

## 3. Grammar isolation: a concrete shared-enum hazard

At this baseline `ast.rs::ScalarTypeSyntax` has Bool/I32/Unit and is shared by FixedArraySyntax, slice reference syntax and EnumPayloadSyntax. `parser/arrays.rs::array_element_type` and `parser/enums.rs::enum_payload_type` are separate closed parsers, but declaration enum views and formatter observation matches share the carrier.

Required scoped design: introduce a separately named closed array-element syntax carrier (Bool/I32/U8/Unit) for FixedArraySyntax and slice references; retain predecessor ScalarTypeSyntax for enum payloads. This prevents a constructible U8 enum payload from reaching currently infallible enum view code just because arrays gained it. It needs fresh enclosing-layout measurements; a four-variant enum being small is not sufficient evidence. Do not expand the enum ScalarTypeSyntax carrier as a shortcut; a different carrier design requires renewed contract review.

Literal inference has its own admission seam in `oir/owned/source/typeck.rs`, historical lines 2671–2703: `ExprKind::ArrayLiteral` currently accepts only `ValueTy::Scalar(Ty::Bool | Ty::I32 | Ty::Unit)` before first-element inference and calls `FixedArrayTy::check`. Extend that literal-specific allowlist with U8 independently of annotated type grammar. Preserve first-invalid-element and exact mismatch ordering, existing per-literal/per-edge work debits, and predecessor diagnostics for unchanged invalid inputs. This is not authorization to broaden general aggregate-field or enum scalar allowlists. Reconfirm line locations at the frozen dependency.

Explicit review sites: ast.rs; parser/arrays.rs; parser/enums.rs (unchanged exclusions); declaration_index.rs::array_type/parameter_type; declaration_index/enum_views.rs; format/ast_tests.rs and production formatter/source-association validation. Audit every exhaustive scalar-syntax match. Do not expand enum syntax, unknown-type diagnostic wording, observation tags or source authority merely to make Rust matches exhaustive.

### Diagnostic contract to freeze with fixtures

Absent an earlier predecessor error: heterogeneous/annotation/call/return/element mismatch and non-i32 index are E0300/type at the mismatched expression; non-array indexing/len is E0305/type at full access; immutable-owner write/exclusive borrow E0304/type at target/borrow; unavailable owner E0310/ownership; conflicting loan E0311/ownership; bare reference E0312/type; shared-reference write/exclusive reborrow E0313/ownership. Retain existing wording/origin selection for unchanged cases rather than adding u8 to every old error message. Direct scalar u8 record fields retain E0202/resolve at the type and exact `u8 record fields are not supported`; enum `V(u8)` retains E0100/parse at u8 and `only bool, i32 and () enum payloads are supported`. Raw invalid type/ID/origin/operation failures retain E0500 at their existing verification stage. Existing non-byte record privacy/depth/cycle failures retain their predecessor diagnostics, pinned by current baseline fixtures before implementation. New byte-array record fields reject at source resolution: E0202/resolve at the complete field type with `u8 array record fields are not supported`, after predecessor declaration/import-graph checks and under the existing field-resolution schedule. This RFC freezes this narrowly new exclusion diagnostic; do not rewrite unrelated messages. The newly accepted array/slice forms intentionally supersede RFC0030's E0101 exclusions only for these placements; its conversion and other exclusion errors remain unchanged.

## 4. Identity, layout, storage and scalar transport

FixedArrayTy::check currently rejects U8. BorrowedTy::accepts, BorrowedSlot::check and Declarations::check_borrowed_type separately fence byte slices. Update all independently, retaining exact directional compatibility and enum/record non-slice exclusion. Keep `value_field`, `value_summary`, source resolve's `record_field_type`, and raw enum payload checks rejecting direct scalar U8. Additionally, add an explicit source `record_field_type` rejection of `ValueTy::Owned(AggregateTy::FixedArray(a))` when `a.element() == U8`, including N=0. Add independent raw `value_field` and record-context `value_summary` fences for that same owned-byte-array type before layout/storage admission. These are essential because changing `FixedArrayTy::check` alone otherwise makes unused and nested record fields acceptable. Do not ban U8 arrays in general `check_value_type` or array owner layout: standalone parameters/results/locals need them. Checked record containment traversal must validate every declaration, including unused inner records, so an outer record cannot hide a transitive byte-array field. Raw shape/source association and the checked declaration facade must not bypass these exclusions, even with forged valid IDs or source syntax. A generic `Layout::scalar(U8)` implementation is a storage primitive, never permission to declare such a field/payload.

Proposed u8 physical layout: size/alignment/stride 1/1/1. Array size `max(1,N)` bytes. Zero-length storage remains one initialized sentinel byte, never an element. Logical width remains `max(1,N)` cells, not byte count. Records retain checked padding, declaration-order fields, acyclic depth-64 containment, bounded iterative graph calculation and existing logical width. No new recursively flattened type tree or leaf table. Whole transfers initialize/preserve sentinel and padding according to the existing aggregate policy; no uninitialized padding reads.

Reference consumer's `execute.rs::scalar_size`, `decode`, and `encode` currently reject U8 aggregate payloads. Admit exactly one byte per U8 leaf: decode produces Scalar::U8 from every 0..255 byte; encode stores that exact byte with checked offset/range. Never reuse canonical bool decoding (only 0/1) or unit decoding (only 0). Identity verification precedes generic codecs, so broader codecs do not grant forbidden nominal payload authority. Test every value through construction, indexed storage, move, replacement, parameter/result transport and standalone array leaf traversal. Record byte-leaf execution remains unreachable through valid declaration admission.

Native `native.rs::element_stride`/`sentinel_ty` currently reject U8. Aggregate byte loads/stores must use i8, never i1/bool conversion or i32 stride. Reuse checked signed-i32 bounds before address formation. Owned scalar arenas remain i64 with eight-byte argument positions: indexed i8 reads cross into scalar locals via zero extension to canonical 0..255, and stores take verified byte values back to i8. Existing `load_scalar` truncation from canonical i64 is internal transport, not a language cast. Prove canonical producers and transfers rather than accepting arbitrary malformed i64. Byte-to-i32 remains zext; byte comparisons retain unsigned predicates. Do not introduce LLVM inbounds/noalias promises. Test guarded and acyclic paths separately, including bounds/conversion success continuations feeding short-circuit phis with the actual final predecessor. Whole-array byte copy paths must use the correct size, including empty sentinels and adjacent independent owner storage. Existing bool/unit/i32 record leaf-copy paths remain regression controls; they must not acquire byte-field authority. No packing of scalar arenas or lowered admission price.

Native qualification stays existing Linux x86_64 LLVM/Clang/LLD 19.1.7 O0/nonrecursive scope; project qualification keeps its existing host gate. Source-free executable evidence must check values, status, stderr and ordering, not merely compilation or IR text.

## 5. Order, failure, fuel and provenance

Retain RFC0016 static schedules: target binding first; complete RHS resolution/numbering and type checking before index for a store; then base kind, i32 index, element/RHS identity, mutability; ownership follows valid lowering. Preserve two-root HIR completion and count/emit traversal. Existing no-byte route diagnostics must not be globally normalized.

Runtime store: complete RHS snapshot; index once; access fuel; bounds; final store. Failed RHS skips index/access; failed index skips final charge/store; E0601 wins over bounds at an unpaid access; paid bad bounds yields E0606 `array index out of bounds` at full access origin. Earlier effects persist, including mutations made by index helpers. A successful store writes the old RHS snapshot after such mutation. An untaken out-of-bounds branch is type checked but does not execute/fail. Named conversions retain their separate receiver snapshot and conversion charges and E0610 range failure origin.

Array width is max(1,N). Preserve construction/transfer `1+w`, replacement `1+2w`, width-dependent owner/frame/temporary/staged-argument initialization and teardown. Access and length cost one after operands. Views do not copy elements. Existing PrepareBorrow charge precedes transition; activation logical prices remain 8/reference and 12/loan, with physical metadata accounted separately (RFC0022's 10/14 cells at its qualified representation, to be remeasured). Derive source trace costs from actual lowering independently; never reuse a raw fixture total as source fuel.

Raw verifier must recheck array identity, scalar operand tags, lengths, slot roles, CFG initialization/dominance, complete staged consumption, nominal paths, call mode/direction, origins and immutable declaration linkage before sealing. A byte tag is not an owner witness. Preserve root/generation/permission identity independently of view metadata. A byte slice view covers its complete standalone backing array, with root-relative offset zero and length derived from the exact backing owner; check extent before use. Reject a forged nominal field path or nonzero projected offset claiming byte-array authority. Existing non-byte projected views continue to rederive their paths under predecessor rules. Reborrow ancestry stays allocation-free and bounded by the frame limit. Reject forged slice-to-fixed authority, mismatched active-loan view, stale generations, descriptor/element substitutions (including equal-size bool/unit/u8 and zero-length arrays), nominal IDs disguised as arrays, swapped valid origins and spoofed native plans.

Production source must authenticate `[u8;N]`, each literal/store/read/len/conversion and its exact file-aware spans against the actual AST/HIR, including cross-file source identity. Record byte-array declarations and projected paths must fail before they can authenticate an executable byte view. Bare raw operations and historical observation witnesses cannot become SourceProgram. Retain old private array qualification witnesses as immutable, non-promotable evidence. No new public raw execution route.

## 6. Resources and what is actually known

Read-only inspection establishes reuse of existing FixedArrayTy `{ element: hir::Ty, length: u16 }`, AggregateSlot/BorrowedSlot compact discriminated carriers, ValueTy descriptors; bounded projection paths remain predecessor non-byte machinery. HIR Ty already has U8; runtime Scalar already has U8. Proposed storage does not require a new runtime value/owner/reference variant, but this is not a measured zero-growth assertion.

Existing historical evidence read for planning only: `u8-owned-evidence/carriers.log` reports named components [120,88,384,680,648], depth 64, 15,024 additional fixed bytes; `carriers-tracker.log` reports [120,88,384,1040,664], depth 64, 15,400. These are different checkpoints and demonstrate why earlier measurements cannot qualify a new head. Neither is asserted to measure this proposal or the exact current baseline. RFC0020 records qualified record/field sizes 72/64, 8 MiB declaration tables, 1 MiB summed declared layout, reference 200,000 expanded cells/16 MiB storage and native 8,192 expanded cells/1 MiB arenas. Source currently computes table charges using actual size_of, so changed layouts must flow through admission. No expensive build or new carrier test was run here.

Before activation measure exact base and successor size/alignment plus retained capacities for array-element syntax and enclosing AST type/enum/field/function rows, HIR/source expressions, query return/error carriers, fixed-array/borrow/aggregate slots, checked field/record tables, raw instructions, source-auth visitors, interpreter Scalar/frames, storage leaf iterators, plan/diagnostic/emitter roles. Inventory simultaneous temporary roles, caller/return carriers, formatting argument backing, error paths and allocation attempts, not only member sizes. Fresh measurements must be on the actual source head/toolchain.

Charge changed parse spelling comparisons, syntax conversion/query matches, declaration layout, source association, count/fill passes, verifier work, leaf traversal/copies, metadata and native I/W/text. Generic paths may alter no-u8 admission endpoints. Keep every source/token/node/module/function/argument/length/depth/work/byte/native ceiling unchanged, including reference 1,000,000 fuel, 1024 frames, 200,000 live scalar slots and 256 argument scratch scalars. A byte owner still consumes full expanded-cell credit.

Perform checked count/width/size arithmetic and resource preflight before allocating enlarged tables, literal vectors, transitive storage, frames, native arenas or text. Bounded graph summaries precede allocation; no speculative flattening. Test exact cap success and one-unit-short failure with allocation-attempt instrumentation, and verify denial occurs before the unaffordable allocation/tool/output. A changed endpoint needs a separately named, independently reviewed admission successor with preserved predecessor fixtures; no cap increase, missing debit, endpoint relabeling or RSS claim. If growth cannot be honestly admitted, stop for a separately reviewed smaller design.

## 7. Wire, provider and compatibility authority

Only the explicit stable-Rust typed-preview source path expands. Original-file and whole-project owned routing must recognize byte array annotations/literals/slices even in unused helpers/modules. No per-file fallback. Legacy/OXBC/OXA, builtin I/O descriptors/signatures and process policies remain unchanged. Existing i32-backed binary I/O stays i32-backed; byte arrays must fail those signatures until a separate migration decision.

No OPA1/AST1/STF1 or OPA2/AST2/STF2 marker/tag, parser frame cap, observation cap, source cap, manifest selector or producer code change is authorized. Public parser qualification may replay the frozen supported scalar domain and record closed refusals for new arrays; it must not expand that domain. Existing diagnostic observations require exact canonical first-diagnostic comparison; new accepted source cannot be normalized into an old error, and diagnostic artifacts remain unimportable. Report unchanged valid, unchanged invalid and changed-domain cases separately. Preserve historical artifacts/identities, not edited expected bytes. Private parser research/review remains paused.

## 8. Concrete acceptance corpus

Each row requires independent expected result or exact diagnostic code/stage/message/full primary and secondary origin. Pass means public check/reference/native parity where supported; reject rows additionally prove no premature execution/tool/output. Raw tests are separate evidence, never substitutes for public source cases.

| ID | Pass controls | Fail/adversarial controls |
|---|---|---|
| B01 syntax/type | `[byte(0),byte(127),byte(128),byte(255)]`; annotations, helper result/parameter; comments/formatter fixed point | i32 literals under u8 annotation, mixed literal, wrong return/argument; byte suffixes, repetition, nested arrays |
| B02 length | N=0 typed local, 1, 1024; leading-zero length; all-zero length spelling exactly at token budget; exact/slice len returns i32 | Independently test 1025-element literal (first excess element) and N=1025 type (whole length token); all-zero spelling one over token budget; enormous nonzero digit token; signs/expressions; unsupported empty-literal contexts |
| B03 round trip | All 256 bytes store/read then named widening through array, fixed borrow, slice and explicit relay; partition under caps | detect bool-only decode, sign extension at 128/255, truncation, wrong stride and adjacent-owner corruption |
| B04 ownership | Move through local/helper/result; mutable whole replacement and self-move with predecessor semantics; old owner teardown; owner reusable after borrow call | use/len/store after move, duplicate consumption, immutable mutation, implicit copy, incomplete construction |
| B05 bounds | first/last valid index, i32 loop, untaken bad-index branch | -1,N,i32 MIN/MAX, zero array access; byte index; access fuel one-under must beat bounds |
| B06 views | lengths 0/1/2/1024 through same slice helper; shared aliases; exclusive-to-shared explicit reborrow; same-root shared aliases across exact/slice formals; parent restored | Staged child loan followed by parent len/read/write in later arguments; same-root shared/exclusive and exclusive/exclusive aliases; reverse slice-to-fixed, wrong element even empty, wrong modes, bare forwarding, local/returned/stored slice |
| B07 declaration fences | Existing bool/i32/unit record arrays, composition, projected slices and their privacy/depth behavior; standalone byte array alongside unrelated records | Source and independently forged raw byte-array record fields at N=0/1/1024; unused declarations, inner record hidden by outer nesting, imported declaration; byte record projections; direct scalar u8 fields and enum u8/array payloads |
| B08 order | RHS snapshots byte128; index helper mutates it to255; final store restores128; written-order array constructors with observable earlier effects | Later constructor element failure preserves earlier effects, skips later elements and never exposes partial owner; RHS range failure skips index; index failure preserves RHS effects; paid bounds failure no final store; moved base during index rejected |
| B09 fuel | independently enumerated raw/source events for construct/move/store/len/view/reborrow and conversion; exact success | every lower budget in bounded pilot fails at independently predicted span and no unpaid store; no cheap zero-array/byte-cell path |
| B10 raw trust | valid source-bound descriptors and full-root loan traces | wrong scalar tag, malformed N, equal-size bool/unit/u8 descriptor swaps including N=0, array-ID substitution, stale generation, offset/extent/length/view forgery, forbidden byte-record projection, reborrow authority upgrade, swapped valid spans between files with identical source text and cross-file source identity/native-plan claims |
| B11 resource | exact source/literal/declaration/width/padded-byte/native/text endpoints; old no-byte controls | one-over/one-under limit denials before allocation; count/fill disagreement; fail allocator per site; no new uncharged vector |
| B12 compatibility | bool/i32/unit array/slice/composition/enums; u8 scalar PR56 corpus; scalar vs owned unchanged error precedence; unused aggregate/module | unchanged enum parser rejects u8; I/O rejects &[u8]/&mut[u8]; no conversion chaining/index receiver; legacy unchanged |
| B13 native | actual source-free ELF, cleared env, O0 pinned tools; inspect i8 loads/stores, zext and unsigned predicates | unsupported target/entry rejects before plan/tool/output; no-clobber and no fallback; neighboring sentinels/padding never exposed |
| B14 provider | public replay old valid and unchanged-invalid scalar cases with exact observations | new source/forged u8-as-i32 observations closed-fail; no protocol/tag/cap expansion or old ledger rewrite |

Pilot (new fixture, no I/O migration): explicitly declare the ordinary `byte` helper above; construct `[byte(0),byte(127),byte(128),byte(255)]`; relay the complete standalone `[u8;4]` owner by value; use `&mut array` through a byte-slice helper to replace index1 with byte255; sum through `&array`, binding each indexed byte to a local before widening. Expected sequence [0,255,128,255], sum638. Return 638. Also test exact fixed-array borrow relay, empty slice sum0, and a two-module variant preserving file-aware failure spans. Treat 638 only as a summary: retain per-element/effect/loan traces and independent neighboring-owner integrity checks. A Packet containing that array is a rejection control, never a positive pilot in this slice.

## 9. Work and independent-review splits

1. Contract/grammar/types: draft accepted RFC; array-only syntax carrier; declaration queries, literal inference, type/layout fences; formatter/source routing. Reviewer separately owns enum/type-name/legacy diagnostic exclusions and representation delta.
2. Semantics/reference: raw shape/CFG/source binding, encode/decode, move/index/whole-array slice integrity and forbidden byte-record projection rejection. Independent reviewer supplies malformed descriptors and effect/fuel oracle before seeing implementation expectations.
3. Native: byte memory/scalar-arena transport, zero extension, stride/sentinel, transfers, slice call ABI and emission inventory. Independent native reviewer checks emitted IR plus source-free executed artifacts and exact plan/resource boundaries.
4. Resource admission: separate reviewer measures all enclosing carriers/capacities/work, allocation-before-denial, no-byte endpoints and any named successor. This review may veto activation regardless of ordinary test results.
5. Public qualification/integration: frozen independent corpus, public provider compatibility only, full affected repository checks, artifact/source/toolchain identity, independent final review and exact-head CI. Do not modify PR56 workers' source/evidence or reuse their pending CI as successor evidence.

Freeze corpus expectations before implementation and preserve failed controls. Parallelize isolated work only after the dependency/contract gate. Any change to grammar scope, the explicit record exclusion, error precedence, limits, representation authority or provider domain returns to review. This draft authorizes no implementation or publication by itself. Parent contract approval must be recorded below.

## 10. Acceptance decision

Parent approved the scoped semantic/interface contract at 191eb94 on 2026-10-09, against exact base b5455ad07081590aeffdec54a927c9095e870fb0 / tree cb53acc04ac88538a74410bc0a3c037da369c02e. Resource admission, independent oracle review, repository verification and exact-head hosted CI remain independent veto gates. Changes to record/enum exclusions, provider domain, diagnostic precedence or caps require renewed review. Predecessor diagnostics/origins must come from the immutable baseline; unresolved expectations stay explicitly unresolved, never learned from successor output.

### Accepted source-authority interface amendment (2026-10-09)

Parent reviewed the production caller graph and approved one bounded tightening:
`association::lower_and_associate(typed)` accepts only the checked typed owner,
performs the existing trusted lowering and association sequence, and returns the
same consuming `AssociatedOwned`. The raw-taking helper is module-private;
mutation entry points are `cfg(test)` only. No arbitrary raw program plus typed
owner can acquire production source authority. Public raw/import boundaries stay
closed. Exact AST/HIR operation selection remains the responsibility of trusted
lowering; this amendment does not claim an independent whole-program semantic
replay comparator. Existing conversion authentication remains unchanged.

No new vector, detachable token, caller boolean, re-lowering, provenance search,
work cap or raw-to-SourceProgram path is allowed. Diagnostic mapping/order must
remain byte-identical. Relocated caller/result/error carriers require measured
accounting; compile-time privacy and forged-raw inability require independent
controls. Full semantic comparator or further API growth is not approved.

### Approved B11 lexer-allocation scope clarification (2026-10-09)

Parent explicitly reviewed and approved this bounded clarification after the
independent allocation audit. The exact predecessor already lexed the same byte
spellings through the identical identifier/token-tape implementation before
later grammar rejection. RFC0031 changes no lexer path, token carrier, capacity
policy, allocation site or ceiling.

New or changed byte-storage allocation sites still require independently
derived demand, exact/one-short admission and each-site failure controls;
reused fallible sites require exact topology evidence and linked controls.
The unchanged ordinary lexer uses infallible `Vec::push` and has no allocation
failure hook. Recoverable allocator failure at those inherited sites remains
explicitly **UNPROVED**. Provider canonical-backing prepayment is a separate
route and does not establish ordinary-lexer recovery.

The independent corpus's stronger compound source/token obligations remain
unchanged. Only their topology/limit subclaims may be established; this
clarification does not turn the whole rows into empirical passes. It adds no
recovery guarantee and changes no diagnostic, error precedence, grammar, cap,
fallback or provider behavior. Any new or changed allocation topology reopens
admission review. A separate lexer fallible-allocation/error-semantics hardening
follow-up is recorded in the implementation plan; it is not completed here.

### Approved inherited-native allocation limitation (2026-10-09)

Parent separately reviewed and approved carrying these precisely enumerated,
unchanged predecessor mechanisms; the lexer clarification did not cover them:

1. Caller-table outer vector and its empty adjacency headers.
2. Remaining-call counts.
3. Per-invoke caller-adjacency growth.
4. Global function-ready collection and pushes.
5. Function-bound rows.
6. Per-function incoming-edge counts.
7. Per-function block-ready collection and pushes.

Also included are the unchanged call emitter's argument `Vec<String>`, guarded
fuel/owned-result strings, scalar argument/name/load temporaries, owned and
borrowed pointer arguments, slice-length strings, scalar result prefix/store
temporaries, and joined argument string. Standard `vec!`, `collect`, growth-capable
`push`, `with_capacity`, `format!` and `join` at these sites remain infallible
and outside the named failure-injection hooks. Their recoverable allocation
failure and per-site injection remain **UNPROVED**; the original stronger corpus
obligations are unchanged and are not whole-row empirical passes.

Preserve the actual ordering: function count is bounded at 256 before the early
caller structures, but adjacency growth precedes the native 4,096-block gate.
Its earlier bound is the already verified raw 300,000-block ceiling. Subsequent
capacity metrics and metadata checks do not preflight those earlier allocations.
Logical entry-byte bounds are not allocator-capacity or RSS guarantees.

Exact unchanged sections, carriers, graph shapes and pointer/length templates
justify this limited inheritance. No new byte-dependent allocation, width or
capacity is exempt; any such change reopens admission. Independently derived
acyclic N0/N1 demand is separate from guarded template/topology inheritance and
actual count/render controls. No independently derived full guarded demand is
claimed. Separate native fallible-preflight hardening is tracked in the plan;
this decision changes no production behavior, error precedence or caps.
