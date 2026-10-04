# Unit3A source observation boundary, version 1

This is a prospective interface contract. No adapter or candidate compiler was run to construct these expectations. The selected production tree is `a826c1782dac19b8c34bdafb0202240e5d414719` (head `d582d1dca2a26eb3230632a921f9e8166c39d5c0`). Neither public source nor production raw array admission opens in Unit3A.

## Minimal private API

Add a cfg(test)-only source observation helper in the frontend test boundary. Its inputs are an explicit single-source versus whole-project mode, a root source path in the isolated fixture directory, and downward-only observer/resource limits. The helper uses the ordinary source loader, lossless lexer tape, parser provenance, declaration-index construction and checked structural type-query paths. Candidate array syntax is fixed by that helper; no CLI/environment flag, new SourceMode variant, source-content routing switch, expected-result argument, callback or caller-constructed AST enters it.

Internally use a separate syntax policy alongside the existing OwnedCandidate/ModuleCandidate/ProjectCandidate modes: Closed in production, Candidate only in cfg(test). Interpret bracket spellings of existing Unsupported tokens only in Candidate parser helpers. Do not change the public token tape. Source files and file IDs come from ordinary load order; the fixtures require root `main.ox` as file0 and the declared single child as file1. Do not supply file IDs from expected output.

The returned owned observation is copied, inert data only:

1. Protocol/version, requested scope, observed phase completion, exact original source file identity and the production source identity used by the harness
2. Lossless token kind and `(file,start,end)` projection where requested; every span is checked against the original UTF-8 source map
3. AST forms, edges and retained origins. At minimum export every new array TypeSyntax, ArrayLiteral, IndexRead, ArrayLength and IndexAssign plus the relevant Group/child expressions. Export their actual per-file ExprIds keyed by original file identity and validate the whole AST graph before normalization. The comparison joins IDs to `(file,start,end,kind,role)`; it never substitutes expected spans into observed rows
4. Independently observed AST literal-entry inventory and store-target-wrapper count; total expression count when requested. Array element order is significant. A store's target is a retained direct IndexRead AST node; it is not coerced from a Group
5. Each module's existing AST `uses_owned_syntax` result, the independent metered SourceOwner result, selected whole-project producer and original root entry identity. An error cannot trigger scalar/legacy fallback
6. Checked structural query observations for requested source type uses: scalar element tag, exact length, owner/reference tag and shared/exclusive kind; nominal types retain their actual declaration identity. No array nominal rows or interning table
7. Exact diagnostic list in phase/function order, including code, stage, text, primary and ordered secondary origins, notes, rendered human and JSON bytes. Original34 assertions only gain wording through the separately reviewed appendix

Unit3A ends after syntax, AST/provenance validation, selectors, inventory and declaration-index type queries. It returns no HIR, TypedOwnedProgram, raw program, executable witness, plan, arbitrary callback, mutable compiler object or raw-parts factory. It does not lower, associate raw source output, run ownership analysis, execute reference code or emit native IR. Existing record-only/file-less candidate exports remain untouched.

## Exact comparison scope

Each cases.json record is an expectation, never a pass. `first_observation_slice=3A` identifies lex/parse and the other fields named above; keys prefixed `later_`, `downstream`, `semantic_sequence`, or `execution` are not Unit3A success assertions. Cases marked3B freeze later resolution/type expectations now; cases marked3C preserve later effect/resource expectations without executable schedules or fabricated fuel totals.

For `ast`, compare the complete projection of the kinds present in that case's requested AST list, including all occurrences of those kinds in that source. `elements` is the complete ordered child-origin list, not samples. `statements` is the complete projection of the listed statement kinds. All explicitly supplied fields must compare exactly. Other scalar leaf kinds need only appear to discharge referenced edges unless a total count is given. Type queries compare every explicitly listed type use, including its original use-site span. For selectors compare every module's result and the independent whole-project result separately. Sources ending in parse/lex failure have no successful AST/route assertion.

`type_queries.query_kind` defaults to `value`. A `parameter` query checks the actual parameter position and projects the checked ParameterTy form, including shared/exclusive fixed-array references. This does not permit a reference type in ordinary value/return/local positions or require body resolution. If the implementation needs a small private parameter query helper, keep it type-only and use the existing checked aggregate descriptor.

For full-source identity compare path, bytes and SHA-256 before any observation is accepted. The observer must identify which scopes completed; a successful parse is not a typed success. Source and coordinate manifest hashes are independently constructed by authority.py from literal source facts, with no candidate-output input.

Diagnostic display paths are fixture-relative `main.ox` and `child.ox`, frozen in diagnostic-wording-v1.json. An ordinary project loader may retain absolute materialization paths; the observation's presentation-only path mapping must be bijective to the already-bound original file IDs and verified source bytes. Record that mapping separately, then render with those display paths. Never normalize offsets, choose files from expected spans, or alter production source-map provenance. New E0101 candidate diagnostics use borrowed formatting through the bounded owned diagnostic helper; do not allocate a source-sized String for an invalid target/token merely to truncate it later. Existing public formatting remains untouched.

Use a versioned observation envelope, `oxid-array-source-observation-v1`, distinct from the old array-free candidate projection. The external comparison harness may request a fixed subset of these observation categories, but cannot inject expected values into the compiler. Reject duplicate/missing files or case IDs, missing fields, wrong schema/phase, incomplete arrays, unknown tags, invalid edges, wrong-file ranges and non-UTF-8-boundary spans.

## Observer bounds and failures

The test observer must have explicit checked row and retained-byte counters before reservation. Proposed default ceilings for these bounded fixtures are200,000 rows and16 MiB serialized observation bytes per case, with downward-only overrides for fault controls. These are observer ceilings, not new compiler admission caps. If reached, return an explicit incomplete-observation error with zero qualification credit; never truncate and label it success. The controller must record requested/actual limits and complete=true for every claimed case. A harness may use a larger reviewed ceiling only with a new observation-contract version. No measured memory/resource claim follows from these proposed ceilings.

## Temporary Unit3A executable fence

The ordinary executable owned-source entrypoints must reject candidate array ASTs before producing resolved/lowered executable authority. Proposed diagnostic: E0500, stage resolve, `array source execution is unavailable in this dormant syntax checkpoint`, full original array type/expression or indexed-assignment target as primary; no secondary/notes. This is an internal checkpoint admission result, not the permanent language diagnostic for any3B/3C case. QuerySession::value_type and this3A observation helper must remain usable for arrays. Public Closed parse rejection and earlier loader/lex/parse errors win. Final first-feature traversal order requires the producer/reviewer appendix below; this package does not compare arbitrary mixed-feature E0500 origins until that order is frozen.

## Later observations

3B may add a versioned source-only resolver/type observer. It must report binding and callee declaration identity, type slots and HIR postorder from the real resolver/checker. Entire program resolution precedes typing. IndexAssign resolves its base, then RHS subtree, then index subtree; no HIR read represents the AST target wrapper. Typing completes RHS then index before base/index/element/mutability checks. Annotated-local empty initialization fills the leaf and all transparent Group slots only.

3C separately adds bounded lowering/ownership/reference/native observations through the existing verifier-confined probes. It must freeze the complete source-derived operation/fuel/owner/loan schedule before running those candidates. The two preserved snapshot examples are counterexamples to lexical-first ordering, not complete3C qualification. They cannot establish executable support or native limits by themselves.
