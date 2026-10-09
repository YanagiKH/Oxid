# Bounded standalone byte storage: frozen implementation plan

Status: parent-approved interface/semantic contract; admission remains open, 2026-10-09. Normative contract:
[RFC0031](../../rfcs/0031-bounded-standalone-byte-storage.md).
Base b5455ad07081590aeffdec54a927c9095e870fb0; tree
cb53acc04ac88538a74410bc0a3c037da369c02e. Proposal SHA-256
7efc3b0c93444893af94f58237088a8c16ec3ce9f114bae95fed2effc3dbf38a.

## Gate and sequence

1. Parent approves this interface contract and RFC. Independent oracle owns
   expected outcomes before implementation. No semantic edit precedes this gate.
2. Measure exact base carriers/callers/results/errors/format backing and retained
   capacities. Inventory resource branches and authority consumers before changes.
3. Parallel isolated syntax/types, reference/trust and native workers implement
   only their assigned paths. Each receives the same fixed interface below.
4. Integrate on one coordinator worktree; measure successor actual layouts and
   admission endpoints. Independently derive explicit current successors wherever
   endpoints change. Keep predecessor fixtures byte-for-byte unchanged.
5. Independent semantic/resource reviews and full local applicable verification.
   Resource review can stop admission even if semantics pass. Exact hosted CI,
   publication and merge belong to parent; no worker pushes.

## Interface agreement (fixed before parallel work)

- Add ast::ArrayElementTypeSyntax { Bool, I32, U8, Unit }; FixedArraySyntax.element
  and TypeSyntaxKind::SliceReference.element use it. Preserve the separate
  ast::ScalarTypeSyntax { Bool, I32, Unit } for EnumPayloadSyntax. No U8 variant
  there; enum_views and enum parser remain closed.
- Existing FixedArrayTy, AggregateTy, BorrowedTy, BorrowedSlot, AggregateSlot,
  ValueTy, Scalar and raw instruction variants are reused unchanged in shape.
  No new opcode, runtime variant, descriptor table or hidden byte owner identity.
- FixedArrayTy::check admits U8 at N=0..1024. U8 Layout::scalar is size/align 1/1;
  stride 1, size max(1,N), logical width max(1,N). Scalar arenas remain i64.
- BorrowedTy::accepts, BorrowedSlot::check and Declarations::check_borrowed_type
  independently admit U8 scalar slices in the existing directional exact/slice
  matrix. No slices become values, no slice-to-fixed conversion, no root offset.
- Source record_field_type rejects owned U8 arrays (including zero) with
  E0202/resolve, `u8 array record fields are not supported`, full field-type span,
  on the existing resolution schedule. Direct scalar U8 exclusion is unchanged.
  Raw value_field rejects these arrays with NonScalarField(field.id);
  record-context value_summary rejects with TypeMismatch. Every declaration is
  checked, including unused/inner/imported declarations; general check_value_type
  must still permit standalone U8 arrays. Enum payload checks remain unchanged.
- ArrayLiteral typing extends only its own scalar-element allowlist. No other
  record/enum scalar allowlist broadens. Nonempty exact first-element inference;
  annotated local empty array is the only zero-literal contextual exception.
- execute::scalar_size/decode/encode transport U8 exactly as one byte. All 256
  values are canonical. Bounds/offset checks remain checked; no bool codec reuse.
- native::element_stride/sentinel_ty and aggregate leaf load/store use i8 and
  zero-extension into canonical i64 arenas. Existing scalar conversion lowering
  remains unchanged. Empty sentinels are initialized; no inbounds/noalias promise.
- No new public API, protocol, builtin, I/O signature, cap or raw execution path.
  `byte(x)` in fixtures is an explicitly declared ordinary helper.

## Worker boundaries

A. Syntax and type/declaration worker owns ast.rs, parser/arrays.rs and related
array parser tests, declaration_index.rs array/slice queries, format array syntax
observations, owned_types.rs plus its new byte-array tests, owned/source/resolve.rs
record fence, owned/source/typeck.rs literal allowlist. It must preserve direct
U8 field/enum rejection and source whole-project owned routing. Coordinate tests
that still assert historical U8 array exclusion rather than silently deleting.

B. Reference and trust worker owns owned/execute.rs and new reference/raw/source
byte-array test modules. It audits shape/verify/plan/source association and only
changes production files there if a byte-specific integrity gap is demonstrated.
It preserves evaluation order, costs, exact file-aware spans, generation/view/root
identity and byte-record projection refusal. No declarations/native edits.

C. Native worker owns owned/native.rs and new native byte-storage tests. It audits
native storage-plan bindings, i8 transport, guarded/acyclic paths, sentinels,
copy/transfer size, exact count/render inventory and fail-before-output admission.
No declarations/reference edits; report required shared changes to coordinator.

D. Coordinator owns documentation, resource measurement integration, CI/source
boundary closure audit and archived/current evidence routing. Parent assigns
independent oracles/review. Existing authority fixtures cannot be modified by
any worker. Adding current successor wrappers requires separately reviewed exact
inverse/membership proof, not copying current output as expected authority.

Workers use isolated worktrees from the approved contract checkpoint and return
commits without push. Use one coordinated target directory, serial build leases,
bytecode-disabled Python, Rust1.99/LLVM19.1.7 toolchain environment. No duplicate
large targets; do not remove source/evidence/executables. Integrate all changes
before final full checks. No worker reads paused private parser research/review.

## Resource measurement and admission inventory

Measure size AND alignment of ArrayElementTypeSyntax, ScalarTypeSyntax,
FixedArraySyntax, TypeSyntaxKind, TypeSyntax, EnumPayloadSyntax, StructField,
StructDecl, Function/parameter rows, Expr/ExprKind, relevant HIR/source/query
Result/error carriers, FixedArrayTy, AggregateSlot, BorrowedSlot, ValueTy,
RawFieldDecl/checked field/record, raw instructions, source authentication visitors,
Scalar/frame/storage leaves, plan/diagnostic/emitter and caller return roles.
Report actual simultaneous carriers, fixed-format arguments/backing and error
allocation roles, not merely a sum of members. Measure retained capacities and
allocation-attempt denial points. Exact unchanged sizes are evidence, not an
assumption. Existing source work debits, count/fill, declaration tables, native
I/W/text and historical accounting remain unchanged unless a separately derived
successor accounts for all additions under the SAME ceilings.

Run exact and one-short source/literal/array-length/declaration/expanded-width/
storage/native-text/metadata endpoints. Any changed no-byte endpoint needs an
explicit current fixture with independently derived expectations while retaining
historical fixtures. No lower byte-cell price; width stays max(1,N). No speculative
flattening, uncharged vector, dishonest reserve-capacity assumption or RSS claim.
Stop and report if the design cannot fit honestly.

## Verification matrix and closure audit

RFC B01–B14 is the required corpus. Freeze independent expectations with all 256
bytes, N=0/1/1024 and 1025 type/literal failures, exact/shared/exclusive/slice
reborrows, moves, neighboring owners/sentinels, RHS/index/constructor effects,
independently enumerated source/raw fuel, source and forged raw unused/nested
record fences, equal-size descriptor substitutions and cross-file origins.
Preserve existing bool/i32/unit records/projected arrays and scalar U8 controls.

Audit every `.github/workflows` direct invocation, not only scripts discovery:
Unit1 source identity and compatibility, Unit2 current/source-binding/protocol/seal
and archived observer staging closures, Unit4 current parser/staging/integration
controls, all public provider qualification consumers, privacy checks, native
qualification and source-boundary consumers. Preserve exact membership/inverse
proofs and authentic parsed-module portable fixtures with unsupported-host refusal.
Do not rewrite OPA/AST/STF wire providers or their canonical historical bytes.

Required local checks: fmt, clippy all targets/features, cargo test all
targets/features locked, scripts Python discovery, direct CI suites, release
build, verify_repo, privacy and qualification commands. Rust/LLVM pinned evidence
must identify source commit/tree and actual tools. Source-free native runs record
stdout/status/stderr and effects in cleared environment; compilation alone is
insufficient. Local dpkg stager is unavailable: genuine hosted staging and all
applicable exact-head CI jobs remain mandatory, never simulated by a local shim.

## Decision record

Parent approved the contract at 191eb94 on 2026-10-09 against the exact dependency above. Resource admission and final qualification remain independent veto gates. Semantic implementation may now proceed within these interfaces.

### Reviewed authority amendment

Parent subsequently approved the narrowly scoped production
`association::lower_and_associate(typed)` seam described in RFC0031. Lane B owns
the relocation from `program::check_typed`, keeps raw-taking helper private and
all mutation entry points cfg(test), preserves diagnostics/conversion checks,
and measures existing relocated caller/result/error roles. There is no duplicate
lowering or new provenance table. Compile-time privacy tests must prove callers
cannot provide arbitrary raw + typed as a production source witness. This does
not expand the accepted language/provider domain or waive raw verifier controls.
