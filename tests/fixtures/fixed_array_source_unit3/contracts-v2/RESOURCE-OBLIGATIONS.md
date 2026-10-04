# Resource obligations before Unit3A acceptance

These are obligations, not measurements or executed resource passes. No Rust layout size, allocation failure, work count, parser boundary or consumer result was measured in this package.

Preserve the existing source1 MiB, tokens/nodes100,000, token spelling65,536 bytes, expression height/depth64, block depth64, call arguments/parameters256, modules256 and module depth32 caps. New array literal elements/type length have inclusive1024 limits. Preserve project path/probe/directory limits, records4096, fields/record1024, total fields65,536, declaration payload/layout8/1 MiB and index retained/scratch/work32 MiB/16 MiB/256M. Keep cumulative B/T/N across modules, existing I/J/W admission ordering, legacy edition handling and old diagnostics.

## Unit3A evidence required from implementation

- Record affected debug/release/host `size_of` values for AST Expr, ExprKind, Stmt, StmtKind, TypeSyntax, Param, Function and enclosing Option/Vec headers; source/index caches and continuation scratch where changed. Retaining the prior288N disjoint AST bound requires an actual new derivation. If a coefficient changes, disclose its exact admission consequence before acceptance
- Independently inventory `E_ast`, actual ArrayLiteral child ExprId entries. Charge `E_ast * sizeof(ast::ExprId)` in ProjectSources inventory and account in-flight parser vectors/heights. Each retained store target wrapper is a charged AST node with a visited index edge, although it later produces no HIR read
- Check the next logical literal length before reservation and before parsing the1025th element. Use the existing fallible parser allocation seam. Never allocate N placeholder nodes for array type syntax. An all-zero65,536-byte token is accepted lexically and scans without numeric overflow;65,537 bytes fails lex before parse
- Preserve both64 limits. Child heights for literals and index expressions contribute to total AST height, and parser descent increments the same depth budget. Wide shallow1024-element literals must not become a stack of1024 pending frames. Fresh implementation tests must exercise exact/one-over depth and height independently
- Account actual selector, span-validation and query visits under the existing nonresetting index meter. The length spelling pass is parser O(B) work, not an already-metered W claim. Do not rescan length strings in structural queries; descriptors were checked once and still reject forged lengths over1024 during AST validation
- Verify all new reservations with checked arithmetic, fail-closed errors and no partial AST/observation escape. Distinguish physical fallible allocation controls from synthetic overflow seams and from observer failures. No universal OOM/RSS claim
- Record original source binding and unchanged public/legacy/token compatibility controls. Public arrays stay unsupported even in unused functions, malformed/infinite bodies, or children; no public source/raw gate opens

## Later obligations retained without broadening Unit3A

3B independently inventories HIR literal entries `E_hir`, with one checked fallible exact-N reserve in the resolver. No general expected-type propagation or nominal array interning. Include HIR Expr/Stmt/Binding, type slots/projections, source/index/HIR overlapping lifetimes and observer retained storage.

3C independently counts raw ConstructArray operands Q and verifies E_ast=E_hir=Q on accepted source as a cross-check, not a trusted producer equality. Charge existing Operand payload/work formulas in count/emit/source association; exact reservations occur only during emission. Prove the literal cursor fits EXPR_FRAMES and keep release completion/count equality. Preserve source raw64 MiB and ownership work/metadata/scratch100M/32 MiB/32 MiB, all raw slots/blocks, independent source/map association and two-pass origin equality.

Reference plan32 MiB, fuel1M, live frames1024, scalar/expanded slots200,000 and runtime storage16 MiB remain unchanged. Native keeps64 parameters for every function, including unused helpers;256 scalar and256 scalar+owner slots per function;256 functions;8192 aggregate/live scalar slots and expanded cells;4096 blocks;depth32;acyclic fuel upper bound100,000;aggregate/live storage1 MiB;diagnostic/IR16/64 MiB. Source/raw calls remain256;1024 literal elements do not imply native admissibility.

The original proposed wide1024 literal and unused65-parameter helper are retained unchanged under inputs/proposed-fixtures. They are future predictions, not observed results. Straightforward wide literal lowering necessarily retains at least1024 operand locals and exceeds native's256 scalar-slot cap. Do not add interning/slot recycling or claim a universal exact smaller threshold. Array-returning main still has entry rejection before unrelated helper native admission.

Runtime/ownership/fuel schedules, the three-module pilot, live/held-out semantic cases, zero/unit sentinels, final bounds/fuel precedence, signed indices, generation/loan state and full result sequence remain separate3C work. No source frontend test replaces raw ownership authority or source-free native execution evidence.
