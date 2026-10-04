# Private fixed-array resolution and typing validation

Unit3B1 adds private resolved HIR and type checking for the scalar arrays in
[RFC 0016](../../rfcs/0016-fixed-scalar-arrays.md). The production parser and
raw-array admission gates remain closed. This checkpoint produces type facts
through a test-only entry; it does not lower arrays, certify ownership, run
source arrays, emit their native code, or activate public syntax.

## Implemented boundary

The same resolver and typechecker handle the private forms. Array descriptors
remain structural, with no nominal table or type interner. Literals retain
ordered element IDs; indexed reads and lengths retain their named binding and
file-aware origins. An indexed assignment resolves its base, complete RHS,
then complete index. Its AST target wrapper does not become a HIR read.

Only an explicitly array-annotated local initializer can seed an empty literal.
This entry peels complete-initializer Groups, fills the empty leaf through the
ordinary type-slot path, then completes every Group. Calls, returns,
reassignments and nested literals receive no expected-type propagation.
Indexed writes type the RHS and index before checking base kind, index kind,
element identity and owner mutability. Whole-program resolution still precedes
typing. Reference modes are retained for later ownership analysis; a successful
type result is not evidence of borrowing permission or owner availability.

An immutable types-only tag travels with the resolved and typed owner and its
borrowed function views. Existing executable resolver entries retain their
earlier array fence. Program-level lowering guards reject the types-only tag
at the checked root EOF before map/entry checks, declaration accounting,
function iteration, counting or allocation. Function-level guards reject before
walk construction and output work. Empty, record-only and array-free types-only
programs receive the same boundary. No executable witness or raw-parts factory
is exposed by this entry.

## Source and validation identities

The local builds used base `38922724d32d849f4f46114bde137e208602fdac` plus
the ten Rust changes and their fixture inputs. The tested source snapshot
manifest is `866bb8db23ce9e393550b3fc3f107d7b3567f627622b90dc8b3c1af30d9426d7`.
The corrected public-input metadata snapshot is
`f44586df897fb27b0b47684f32108f1c6d24eede7b852730308a48939992c101`:
only the input README, verifier and manifest changed; every Rust, fixture and
semantic expectation byte remained identical. The ten-file code checkpoint
was then rebound to parent `2e84c9de9d9b02b62283fff1902cf1a840254ac4`, producing
tree `a5398061eecc0e35726b648bf7edcd56434e1337`. This relation does not relabel
the local builds as hosted results on a later commit.

The [source-only contract](../../tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/README.md)
preserves the 29 existing language diagnostics and selected source facts, with
separate internal guard and resource controls. Its original authority freeze is
`7cec518fdc38c02f8cfc6c3cf467239423621bf242e44c3e838be490a3060039`.
The selected public replay closure has its own manifest,
`2cba1dbd200d75fbeb33b504b08279d89d2baa1784f88718413305446d458c35`;
it explicitly distinguishes shipped inputs from historical derivation inputs.
All new `.ox` files remain individually registered test data, separate from
successful examples and executable language coverage.

Local qualification used Linux x86_64 and Rust 1.99.0, full compiler commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`.

| Gate | Result |
| --- | --- |
| Owned-source regression, debug and release | 103 passing test functions per profile, with five pre-existing ignored entries |
| Focused private-array tests | 14 passing functions per profile, included in the 103 |
| Control records | 89 per profile: 72 guard denials, eight real reservation failures, four work controls, four synthetic overflows, one positive calibration |
| Formatting and strict Clippy | Pass |
| Independent core replay | Both retained test binaries reproduce all 14 focused tests and 89 control records after fixture identity checks |
| Complete source/type comparison | 54 cases per profile: 22 typed successes and 32 complete diagnostics |
| Separate candidate-copy sensitivity | 64 rejected mutations per profile: 46 source-observation and 18 control mutations |
| Observer protocol | 10 controls per profile, 20 executions in total: limit, label, request and output-preservation checks |
| Combined public replay | 54 cases per profile passed in fresh staging directories with exact reviewed binary reuse |

The independent core replay and combined public replay reused source-bound
binaries; neither was a fresh independent compiler build. The complete ordinary
repository suite was not run for this slice. No source-array LLVM or runtime
corpus was invoked.

The inert observer assembly has 1,297 source files and adapter manifest
`e73acb933f15c938f7256f8902de86ff1d0d5ac55644f3222b3420fa404d2f72`.
The final v4 comparator freeze is
`91134cdbf71e27cc9439446197b9edb8d6f2ab0773935e2e65fd0b1d24c4bb12`.
All 54 original debug/release output pairs are byte-identical. For phases
actually completed, comparison covers append-order HIR and root IDs, Groups and complete type
slots, file-aware identities, complete diagnostics, actual reservation records,
and checked resource accounting. The 89 control records are compared separately
from the 54 source cases. They are not added to the language-case count.

The [public replay package](../../tests/fixtures/fixed_array_source_unit3/typing-replay-v1/README.md)
binds package manifest
`977860da7912737196a4dcf027000f58afcbe03e22f9d9ae26bb2fd60f1fa85d`.
Its combined sequence reconstructs the frozen assembly, verifies and materializes
65 source bodies, binds nine semantic projections, issues 54 source-only
requests, observes, then compares. Both first fresh-stage attempts passed.
Independent review checked the complete source closure and every input/output
identity, then reran the unchanged v4 predicates over all 108 artifacts.
These fresh staging paths differ, so raw byte equality is claimed only for the
original profile pair, not across these public replay directories. The replay
does not rerun the separate historical control or sensitivity families.

The [scoped replay review](fixed-array-unit3b1-replay-review.md) is the review
attachment accepted by the package command. Exact debug/release reused binary
SHA-256 values are
`61856cf6f02919e7b42826e1d84e52a02ae4ba16b9fb8cd550a20e3cb3eeb24a` and
`682d129230c3a8b8605cb1b7f81198449ce4973be9b014cfcfd03042634b6645`.

## Requested storage and work

Measured HIR rows agree between debug and release: Expr 88, ExprKind 64, Stmt
144, StmtKind 120, Binding 72, Signature 64, Function 112 and BodyBlock 72 bytes.
New variants fit the existing maximum enum widths. Test-mode resolved/typed
owners and borrowed function views are 504/528/32 bytes, each eight bytes larger
than the corresponding retained baseline test-binary DWARF measurement.
Production has only the zero-sized Executable tag variant; the quoted owner
measurements are test-mode measurements, not a production whole-memory result.

For actual retained counts R (records), F (functions), D (fields), P
(parameters), B (bindings), E (HIR expressions), S (statements), K (blocks), A
(call arguments), I (record field initializers), and L (array element entries),
the measured test-target HIR requested-row formula is:

`504 + 104R + 176F + 72D + 24P + 72B + 88E + 144S + 72K + 104A + 48I + 8L`.

Headers occur in their enclosing rows once. Independently bounding each count
by the existing source-node ceiling N gives `504 + 912N`. Retained typing and
conservative Option-to-final-vector overlap add at most `24 + 296N`, so the
combined loose envelope is `528 + 1208N`, about 120.8 MB at N=100,000. This is
not a new admission cap. It excludes separately accounted source/AST/index
storage, existing hash-table overhead, traversal and call scratch, diagnostic
storage and observer copies. It is neither allocator capacity nor RSS.

For the grouped-store source, the exact retained HIR formula gives 2,152 bytes;
the retained typed increment is 772 bytes, and conservative conversion overlap
adds 188 bytes before the other storage listed above. The source/index owner
stays live throughout typing. Copied resolution facts in an observer also
coexist with the typechecker, so its memory requires separate accounting.

Each literal makes one fallible exact-N HIR vector request, including a logical
zero request for an empty literal. Checked cumulative entry and byte counts
span functions and modules. Parent literals reserve before child resolution:
attached plus pending literal slots are bounded by E_ast (AST literal-element
entries), rather than partially filled E_hir (HIR literal-element entries).
Successful complete resolution checks E_hir=E_ast;
the HIR expression count excludes exactly the AST store-target wrappers.
Neither equality is trusted as a caller-supplied allocation plan.

New work is bounded by `R_nodes + E_ast + T_nodes + E_ast + accesses +
peeled_nodes`, at most 6N. Debits precede their inspections and share the
nonresetting existing WorkMeter. This does not claim all legacy resolver work
is metered. The independently derived grouped control adds 16 units, has actual
total 247, and requires a mandatory index-admission ceiling of 379. The wide
unannotated literal adds 2,053 units, requires mandatory admission 8,468, and
finishes at exactly 10,369. A limit of 10,368 fails at the final array access
with the existing E0400/resolve-project diagnostic. The direct two-module
control finishes at 321 with 16 added units; a separate selector adds its own
29-unit debit when the broader observer invokes it.

Broad observation leaves generic WorkMeter event logging disabled. Only three
fixed small controls enable it: the 86-byte grouped source, 2,097-byte wide
source and 116-byte two-module source set. Guard sentinels retain seven usize
counters and receive a positive executable-path calibration, so zero guard-work
counts are not accepted without evidence that instrumentation works. Injected
reservation failures use real fallible capacity-overflow errors. Checked
arithmetic overflow is separate synthetic evidence, not general OOM recovery.

## Preserved corrections and remaining gates

Three resource-test assumptions failed before the final pass: using the grouped
actual total below mandatory preflight, charging an annotation peel to the
unannotated wide fixture, and labeling existing index-work errors as resolve
instead of resolve-project. The original attempts and source-derived successor
ledgers remain separate. These corrections changed no compiler semantics,
language expectations or admission bounds. An earlier documentation sum used
1,012N instead of 912N; that bound was conservative, but its derivation was
incorrect and is corrected above.

Observer and comparator pre-admission checks found incomplete vector-length
validation, auxiliary/trace accounting gaps and Python-optimization-sensitive
assertions; each was corrected before admitted observations. A later held-out
mutation removed a literal reservation outside the explicit case roster and
survived v3. V4 requires one actual reservation for every resolved literal in
preorder, including empty and type-error literals. It rechecks the original
immutable observations and rejects both held-out omissions. Earlier predicates,
results and the surviving mutations remain retained; no source expectation or
compiler semantic byte changed to obtain the comparison pass.

The first public wrapper deferred package-contained output rejection until its
final closure check. The admitted successor rejects output inside the package or
input Git repository before creating it, including symlink-parent containment.
Public fixture preparation preserves the original 117 registered source-data
files and 125 language members / 123 checks / 69 runnable programs. The new
replay root receives its own byte-preserving checkout attribute.

Lowering, Q inventory, source association, ownership, reference/native
execution, the two frozen effect cases and the integrated pilot remain later
work. Public activation and exact-head hosted qualification remain separate
from these local typing and replay results. Both production array gates remain
closed; this replay/documentation checkpoint changes no compiler source.
