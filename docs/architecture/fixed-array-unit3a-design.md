# Dormant array source syntax: Unit 3A checkpoint

Implementation boundary, 2026-10-04. Base commit
`d582d1dca2a26eb3230632a921f9e8166c39d5c0`, tree
`a826c1782dac19b8c34bdafb0202240e5d414719`.

This checkpoint adds parser/AST groundwork only. Public grammar and production
raw array admission remain closed. It does not establish source execution,
type checking, lowering, origin association, native support or activation.

## Interfaces and affected files

- `src/frontend/parser.rs`, `src/frontend/parser/arrays.rs`: private `ArraySyntaxPolicy::Closed` and test-only
  `Candidate`, alongside unchanged `SourceMode`; production `parse_counted`
  supplies Closed. Candidate bracket recognition uses the existing Unsupported
  token spellings and does not alter the lexer tape. Dedicated candidate syntax
  helpers use the adopted diagnostic contract; new prose awaits independent
  source-authority freeze.
- `src/frontend/ast.rs`: compact scalar array descriptor, value/reference type
  variants, literal/index/length expression variants and direct-index assignment
  target wrapper. Validate retained spans, bounded descriptors and arena edges;
  select the owned route for every form.
- `src/frontend/project.rs`: test-only policy entry through the same bounded
  project loader, cumulative B/T/N and provenance; actual literal-vector payload
  in inventory.
- `src/frontend/declaration_index/source_owner.rs`: independent metered selector
  recognizes all new forms in all loaded modules.
- `src/frontend/declaration_index.rs`, `src/frontend/hir.rs` and
  `src/frontend/oir/owned/source/resolve.rs`: explicit array-source rejection at
  resolver seams. The index query itself returns checked structural array
  descriptors through the existing `FixedArrayTy::check`; array reference
  syntax stays distinct from value syntax. No implicit scalar fallback or admitted-AST panic. The
  chosen rejection code/stage and exact wording are recorded before observations.
- `src/frontend/parser/array_syntax_tests.rs` and
  `src/frontend/project/array_syntax_tests.rs`: narrow tests over the actual
  parser/AST/loader, public negative controls, resource and selector controls.
- `src/frontend/parser/project_tests.rs`: supply Closed to the one existing
  manually constructed parser used by its overflow test.
- `src/frontend/oir/owned/source/tests.rs`: measure unchanged enclosing HIR rows.

No HIR forms, executable witness, lowering, raw operations, public CLI flag,
environment switch, type interner, nominal array row or generic place is added.

## Resource change inventory before implementation

The array descriptor has scalar kind plus u16 length (0..1024); its full origin
is the enclosing TypeSyntax span. Index assignment retains exactly one direct
IndexRead AST wrapper and its index child. Each literal owns one Vec<ExprId>;
element count is independent of the unchanged 256 parameter/argument cap.

Every new element append preflights checked logical count and bytes and uses
the existing fallible Allocator before allocation. The first excess element is
rejected before parsing or reserving it. Length parsing uses checked bounded
arithmetic and never allocates placeholder elements. Literal/index nesting and
tree height preserve both existing depth-64 limits.

Measure actual enclosing Expr, Stmt, TypeSyntax, Param, Function, Program,
Option/Vec and HIR rows in supported profiles. Re-derive the disjoint 288N AST
payload envelope; do not infer that compact tags preserve enum sizes. Actual
retained element entries E_ast add E_ast*sizeof(ExprId) to inventory; each entry
is charged to its distinct child root. Preserve existing array-free layouts,
limits, allocations and diagnostic behavior. Account parser in-flight vectors,
new validation edge visits and selector visits; length spelling scans are O(B)
parser work, not declaration-index W debits. Index/raw/consumer caps stay fixed.

The checkpoint will record measured results, exact tested bytes, checks actually
run and unexecuted obligations separately. Downstream source-array completion
and independent semantic/resource qualification remain later Unit 3 work.

## Checkpoint 1 measurements and proof

Local Linux x86_64 Rust 1.99.0 debug test compilation succeeded. The predecessor
test binary was hash-verified and its existing layout control rerun into a new
receipt. Running that same array-free control against the candidate measured
identical rows: Expr 88, Stmt 136, TypeSyntax 64, TypeSyntaxKind 40, Param 88,
Function 200, BodyBlock 72, Argument 88, FieldInit 56, ItemId 16 and Program 248
bytes. Unchanged source HIR measured Expr 88, Stmt 144, Binding 72, Signature 64
and Function 112 bytes. These are local debug measurements, not release/host
qualification. No candidate array parser/type/resource comparison had run when
the first immutable source snapshot was made.

The disjoint-node AST payload bound remains 288N with Program headers separate:

- A function node owns Function + outer BodyBlock + ItemId = 200+72+16 = 288.
- A statement node owns its Stmt and at most two newly introduced child blocks:
  136+2*72 = 280. IndexAssign has no heap vector and its retained IndexRead target
  is a separately charged expression node.
- An expression node owns 88 bytes. At most one external wrapper belongs to its
  root: a value call argument (88), an array literal element edge (8), or no
  wrapper. These are syntactically disjoint: a call argument containing an array
  attaches its argument wrapper to the array root, and array edges to its
  separate child roots. Thus the former maximum 176 remains sufficient.
- A record field/literal field/parameter retains its previous separately charged
  row, with no array field/nested type allocation; scalar array annotations are
  inline. Existing module/import/path categories keep their earlier charges.

New literal-vector payload is exactly E_ast*sizeof(ExprId), counted from actual
retained element vectors, independent of all future HIR/raw counts. A successful
literal element reserves one additional logical entry using the existing
Allocator's checked count/product before the child parse; no element reservation
occurs after the inclusive 1024 limit. At most one not-yet-attached reservation
per active literal is live while recursively parsing its child. At depth 64,
this adds at most 64 logical ExprId entries plus 64 Vec headers to the retained
node inventory; allocator capacity/rounding and compiler call frames are not
included in this requested-storage envelope. Expression heights keep their
existing parallel Vec<usize> bound; no 1024-child scratch stack is created.

The existing metered validation theorem V<=11N+2T+4 is conservative for the new
forms: a literal has its usual expression/span callbacks plus one arena-edge
callback per distinct child, charged to that child's node; IndexRead has four
callbacks, ArrayLength three, and IndexAssign five. An array type has one full
span callback and a constant checked length, with no nominal path. Existing call
argument cases already dominate the new child-edge charge. The independent
selector visits each existing declaration/statement/expression once and marks
array forms directly; it does not rescan literal entries. Its earlier bound
6N+2B+1 therefore remains conservative. The new structural query contributes one
explicit `query array type` debit, plus one `query value type` debit when using
that entry; its scalar tag/length check is constant work with no table growth.
Array-free queries, admission order and debits stay unchanged.

Length spelling validation and bounded accumulation scan each decimal spelling
at most twice, so add at most 2B parser byte inspections; this is separate from
the index meter. No length scan, literal-entry clone, new HIR storage or raw Q
claim is attributed to index W. The predecessor full W theorem covers old source
resolution; future array HIR typing/lowering needs its own extension when it is
implemented. Both existing expression recursion and constructed-tree height
limits remain 64 through the actual parser's literal/index child cases.

Execution admission is explicitly temporary: E0500/resolve with message `array
source execution is unavailable in this dormant syntax checkpoint`, at a full
array type/expression or store target, before an array HIR node can be made.
This belongs to a Unit 3A admission roster, not the eventual language diagnostic
contract. The array type facade remains usable independently. Production
`verify_with_limits` still unconditionally invokes `reject_array_carriers`.

## Prospective boundary corrections before observations

After 1024 elements, a closing bracket still completes a trailing-comma literal.
EOF/nonstarters receive the existing missing/excluded-expression classification
before another reserve; only a possible expression starter receives the element
limit error first. Primary dispatch is shared by actual expression parsing and
this decision, with the existing unary/borrow prefixes also recognized.

The new nonstarter diagnostic reuses the public unsupported-token classifier,
but formats through the existing bounded candidate helpers using borrowed
`format_args!`. Public error formatting remains unchanged. The focused resource
control prepares a 63,002-byte quoted UTF-8 token in immutable source before
measurement, then runs lexing and the full parser-error path inside the existing
test global allocation observer. Tokens and parser buffers are allocated inside
the interval, so their deallocation cannot hide a preexisting source-sized
allocation. The prospective bound is less than 16 KiB of peak successful
requested heap payload for the candidate; the unchanged public error control
must expose at least the token length, proving sensitivity to the old temporary
String behavior. A separate existing formatter observer checks one bounded
message reservation and at most 1024 copied bytes. These controls do not measure
allocator rounding, failed allocations, total RSS or universal OOM recovery.
