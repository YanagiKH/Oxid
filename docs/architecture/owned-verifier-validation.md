# Private owned OIR verifier validation

## Implemented scope

This unit adds an authoritative private ownership IR and verifier. It does not
add struct/borrow source syntax, source lowering, reference ownership execution,
native aggregate lowering, a CLI switch, or runtime/fuel admission for aggregates.
Existing scalar source behavior and both scalar consumers remain unchanged.

The raw program contains nominal scalar-field records, whole owned places,
reference parameters, explicit storage boundaries, field operations, whole
transfers, ordered argument preparations, and direct invocation/normal-return
edges. `PrepareOwned` consumes its source immediately; `PrepareBorrow` acquires
the loan immediately. `Invoke` consumes the staged owned arguments and, only on
normal return, releases its loans and initializes its result. There is no raw
arbitrary release instruction or producer-supplied safety sidecar.

Call declarations retain the direct target, ordered scalar/staging/loan argument
descriptors, result, and optional parent pending-argument descriptor. Types are
looked up in the target signature. Canonical open, preparation, acquisition and
invocation sites are derived from actual instructions and checked for uniqueness.
Construction retains the written order of scalar field operands.

### Sealed boundary

`owned/verified.rs` owns the private fields and sole constructing entry of
`VerifiedOwnedProgram`. The witness owns the exact raw instruction stream and
checked declaration facade. Sibling consumers receive only immutable function,
declaration and usage accessors. The type has no execution/native method,
unchecked builder, mutable accessor or deserializer.

An actual-source compile gate, `scripts/verify_owned_witness_privacy.py`, checks
that a sibling can inspect immutable accessors, while struct-update body
substitution fails E0451, direct body replacement fails E0616, and mutation
through the accessor fails E0596. This gate is included in repository CI. An
initial parent-module witness was found forgeable by a child consumer during
review; moving the boundary into its own child module fixes that actual exploit.
The negative gate checks privacy/mutability error codes, not missing imports.

### Closed storage and permission rules

- Named locals and owned parameters can be borrowed; exclusivity requires a
  mutable local. Owned parameter bindings cannot be rebound or revived
- Available temporary/result owners permit scalar field reads and whole
  consumption, but no borrowing or field mutation
- Staging owners permit only their matching implicit Open/PrepareOwned/Invoke
  operations. Result owners initialize only on their owning normal-return edge
- Local/temporary storage has one canonical live site and at most one canonical
  initializer. A live uninitialized raw slot may end without reading its payload;
  this is not source-level uninitialized declaration support
- Shared incoming reference parameters may alias. An exclusive incoming parameter
  relies only on the caller-proven disjointness contract. Shared permissions cannot
  be upgraded; compatible shared children continue to allow reads

Malformed IR, ordinary ownership violations and resource failures are distinct.
None constructs a witness. Every raw instruction, including unreachable malformed
input, is structurally checked before CFG reachability or ownership acceptance.

## Shared scalar and CFG proof

The existing scalar verifier exposes private `CfgView`, scalar shape and scalar
use helpers. Both scalar Function and RawOwnedFunction adapt their real instruction
positions directly. Field reads contribute scalar definitions; constructor fields,
field writes and scalar preparation contribute their real uses; scalar invocation
results contribute the actual normal-return definition. No ownership opcode is
erased into a substitute scalar program.

The one canonical implementation retains arbitrary-entry/cyclic reachability,
exact bool-merge predecessor multiplicity, canonical immutable SSA definitions,
mutable-place initialization and Lengauer–Tarjan dominance. Call-result edge
semantics and old scalar limits are unchanged. Scratch reservations are fallible.

## Finite analyses and diagnostic origins

Availability checks each owner independently with four states: dead storage,
live/uninitialized, live/available, and live/moved. A four-bit block mask enqueues
each `(block,incoming state)` pair once. Analysis saturates all reachable valid
transitions; it does not truncate loop depth. Multi-owner transfers have separable
source/destination conditions and require distinct raw owner identities.

Each loan independently checks exact Off/On region state. Each call independently
checks Off/Open(next argument). Every incoming state at a join must agree. The
parent pass checks child opening even when the parent is Off; an ancestor cannot
prepare another argument or invoke while a descendant remains active. A nonreturn
cycle may hold a region indefinitely, but every normal function exit must have
closed regions. Reference overlap is derived from checked authorities, not a raw
producer assertion of disjointness.

Availability saturates before selecting a moved-use secondary origin. A bounded
reverse reconstruction follows realizable M-state predecessors, replays at most
four incoming states, and stops each path at its latest move. Replacement or a
new lifetime cannot expose an older move. The reverse queue has B entries because
its target incoming state is always M; four-state replay does not make four reverse
queue identities. Out-degree at most two bounds replay multiplicity. Tests cover
a later-discovered earlier source origin and an unrelated terminated branch.
Diagnostics use allocation-free compact optional origins.

## Checked private resource envelope

These are engineering limits for private raw verification, not source/runtime ABI
or aggregate execution limits:

- 100,000 owners; combined scalar locals, scalar places and owners also remain
  within the existing 100,000-slot limit
- 100,000 expanded statement/merge, argument-descriptor/preparation and constructor
  field entries
- 100,000,000 planned ownership scan units
- 32 MiB additional ownership-analysis scratch and 32 MiB checked ownership
  metadata payload
- Raw/reference parameter count remains 256; native's separate limit remains 64

The private test seam may lower but never raise these limits. Scalar-only
functions bypass every new ownership cap. All counts, sums and products are
preflighted with checked arithmetic before analysis allocation.

Let B count blocks, E ordered successor slots including duplicates, S statements
and merges, A call argument descriptors plus preparation entries, F constructor
field entries, and O/L/C owners/loans/calls. Per function:

```
N = 1 + 2B + E + S + A + F + O + L + C
      + parameter bindings + reference declarations + scalar locals + scalar places
W = (4O + L + C + 32) * N
```

The program gate sums W per ownership-bearing function. Counting both block and
terminator visits and the typed tables makes explicit the implementation work
omitted from the initial proposal's shorthand N. The runtime flow meter counts
block, opcode, terminator and edge visits; auxiliary signature/site/forest scans
have an analytic linear bound covered by the constant allowance. Neither measure
is CPU-instruction accounting or a wall-clock promise.

No blocks-by-owner/loan/call matrix is allocated. On the qualified x86_64 host,
availability requests B mask bytes and 4B usize queue cells. It explicitly drops
that queue before diagnostic reconstruction. Reconstruction retains the B-byte
mask, predecessor CSR, and either insertion cursors or visited bytes plus a
B-entry queue. Core requested scratch is:

```
max(33B, 2B + (2B + E + 1) * sizeof(usize), constructor-field scratch)
```

Constructor-field scratch is the largest raw constructor list capped at 1,024;
a function without constructors requests zero cells. Field-count validation
precedes every field-index access. Exact-state loan/call state and queues fit
inside the availability envelope. Existing structural CFG scratch is released
before ownership passes. Canonical metadata is separately charged, including
owner/call/preparation/loan site inventories and forest arrays, plus typed
parameter/reference/loan descriptors and constructor operand payload. Raw input,
source storage, the declaration facade, vector headers and allocator overhead
are not represented as ownership scratch RSS.

The 300,000-block one-owner fixture requests 9,900,000 ownership scratch bytes and
32,400,180 planned scan units. A 300,000-block/99,999-owner fixture (plus one scalar
local) rejects at work preflight before analysis reservation, rather than trying
to allocate a Cartesian state matrix. Lowered-limit tests exercise every inclusive
limit and one unit below the required amount. Sequential allocation-failure
injection covers valid and diagnostic routes; errors cannot produce a witness.

## Independent bounded evidence

The test models use their own graph/state/capability representation and transition
rules. They do not call production transfer, overlap or CFG helpers. Translation
to raw OIR happens only at the test boundary. Six families make 865,209 actual
verifier comparisons:

- 185,612 reachable one-owner graph/action models, expanded to 741,528 raw adapters
  by placing the initialization prologue at every physical position
- 291 canonical alias/mode cases: 75 accepted, 216 conflicting
- 14,946 executed unreachable graph/wrapper representatives; the much larger
  excluded action-candidate count is reported separately, not as executed tests
- 11,204 lifecycle words: 661 accepted, 3,186 ownership failures, 7,357 canonical
  site denials
- 16,600 instrumented alias/access cases: 1,677 accepted
- 80,640 exact-region adapters: 6,000 accepted, 13,440 permission failures and
  61,200 region failures

The initial lifecycle oracle incorrectly required an initializer for every raw
StorageLive. RFC0014 permits uninitialized storage to end, so the oracle was
corrected to zero-or-one canonical initialization. The failed log is retained
separately from corrected qualification. A focused whole-owner test was observed
red against an unimplemented verifier seam and then green; broader property tests
were added after that first implementation and are not all claimed test-first.

### Nested activation and reborrow qualification

The complete 768-tuple capability family and 1,536 shared-alias/distinct
instantiations make 2,304 additional real verifier calls. The independent model
uses a separate function/call AST, capability parent handles and root identities,
and explicit activation/preparation stacks. The raw adapter shares no production
transfer, overlap or traversal helper.

The frozen activation chain is Root(owner O) → Parent(parameter P) → FirstChild
(parameter C). Before, HeldLaterArgument and After run in Parent; Inside runs in
FirstChild. A second request returns before the designated probe. During Held,
the first loan stays active while a lexically nested call evaluates a later scalar
argument. Only the current activation's authority is addressable. Root-owner
probes at these base timings deliberately produce invalid raw IDs; they are
provenance negatives, not executable scenarios.

The shared variants receive shared P/Q handles from one root (aliasing) or two
roots (distinct). The former base probe selects Q. Parent-mode E becomes an
explicit attempted exclusive reborrow from shared P, never an exclusive parameter
silently assumed to alias Q. Whole-program provenance classification precedes
capability execution, matching structural validation before ownership checking.
Complete independently cross-counted outcome arrays are asserted:

- Base 768: 52 accepted, 640 Malformed(Id), 62 Permission, 14 LoanConflict
- Shared-alias 768: 14 accepted, 544 Malformed(Id), 210 Permission, no LoanConflict
- Shared-distinct 768: the same category counts, with different raw root identities

Thus 576 tuples have materializable provenance: 80 accept and 496 fail ownership.
The remaining 1,728 each have an actual deliberately malformed raw comparison.
The complete raw corpus SHA256 is
`5af5b8fe7269e2fff2d0c80381d927d7cdfa7d4711bfcdd2851298c6a409776d`.
The test emits full raw encodings when `OXID_OWNED_NESTED_CORPUS` names an output
file. Diagnostic FNV-1a checksums are reproducibility aids, not cryptographic
claims; source/model and full raw corpus hashes are kept separate.

### Held-out supplement and disclosed reduction

A separate reviewer-authored module adds 59,343 raw comparisons, including all
57,792 one-/two-block lifecycle/prologue cases (19,292 graph/action/initial-state
models), all 240 unique call-event orderings, a branching sibling-call diamond,
and 987 reachable two-owner four-/five-block fixtures selected from 12,000 seeded
graph candidates. The two-owner corpus accepts 79 cases, with zero mismatches;
its seed is `57f8ba1d33449017` and complete raw corpus SHA256 is
`71a6b8b4e77154feb6118c38799e782bb3490c7bffb6f487400ac982fd47fa35`.
The exhaustive lifecycle supplement reports 3,408 accepts, 29,172 canonical-site
denials, 12,222 unreachable denials and 12,990 ownership denials. Eight additional
four-block lifecycle templates at all five prologue positions add 40 raw cases
for joins, repair, generation changes, skipped cleanup backedges and early return.

The combined independent and held-out total is 926,896 actual raw verifier
comparisons. This does not mean 926,896 distinct executable programs: physical
prologue placements, structural/provenance negatives and distinct model families
are explicitly separated above. The complete 9,062,144-case three-block lifecycle
proposal space and 1,073,088-case region-label proposal space were not mechanically
exhausted. They are replaced by the disclosed exhaustive smaller lifecycle family,
full three-block four-action availability family, 80,640 nonlinear exact-region
adapters, call-forest checks and targeted generation/join/termination cases. The
region adapters require distinct canonical acquire/invoke sites and a real
single normal-return successor; malformed-site variants are separate negatives.
This compositional reduction is bounded evidence, not proof over larger CFGs.

Authored tests additionally cover malformed unreachable operations, nominal and
field identities, exact constructor order, scalar dominance through owned
instructions, owner-class misuse, staged/results, exact parent activation,
one-arm acquisition joins, normal-return loan lifetime, shared reborrow permissions,
source-order diagnostic origins, allocation denial, raw 65/256 parameters and
zero-cap scalar bypass. Independent review also exercises held-out alias vectors,
call-event orderings, reborrow permissions and malformed mutations.

## Reproduction and qualification status

With Rust and the pinned LLVM 19.1.7 tools configured as for
[loop-control validation](loop-control-validation.md):

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
python3 scripts/verify_owned_witness_privacy.py
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
cargo test --locked --bin oxid frontend::oir::owned:: -- --nocapture
cargo test --release --locked --bin oxid frontend::oir::owned:: -- --nocapture
cargo test --locked --bin oxid maximum_block_shape_uses_linear_scratch_and_oversize_product_is_preflighted -- --ignored --nocapture
cargo test --release --locked --bin oxid maximum_block_shape_uses_linear_scratch_and_oversize_product_is_preflighted -- --ignored --nocapture
```

The unchanged scalar qualification commands are:

```sh
cargo build --locked
cargo build --release --locked
for test in \
  frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm \
  frontend::oir::mutable_tests::raw_mutable_places_use_real_llvm \
  frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm \
  frontend::oir::loop_control_tests::loop_control_uses_real_llvm
do
  cargo test --locked --bin oxid "$test" -- --ignored --exact
  cargo test --release --locked --bin oxid "$test" -- --ignored --exact
done
python3 scripts/verify_loop_control.py target/debug/oxid target/release/oxid
python3 scripts/verify_while_loops.py target/debug/oxid target/release/oxid
python3 scripts/verify_mutable_locals.py target/debug/oxid target/release/oxid
python3 scripts/verify_boolean_logic.py target/debug/oxid target/release/oxid
python3 scripts/verify_scalar_comparisons.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_native_preview.py target/release/oxid
python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
python3 scripts/verify_i32_literals.py target/release/oxid
python3 -m unittest discover -s scripts -p 'test_*.py' -v
python3 scripts/verify_feature_status.py
python3 scripts/verify_repo.py target/release/oxid
```

To reproduce the nested raw corpus separately:

```sh
OXID_OWNED_NESTED_CORPUS=/tmp/oxid-nested-corpus.txt \
  cargo test --release --locked --bin oxid \
  frontend::oir::owned::oracle_tests::independent_nested_reborrow_capability_tree_768_plus_1536_alias_tuples \
  -- --exact --nocapture
sha256sum /tmp/oxid-nested-corpus.txt
```

### Final local result (2026-10-01)

All listed gates passed on Rust/Cargo 1.98.1 and LLVM 19.1.7 Linux x86_64. Final
candidate and independent-review debug/release runs each passed 467 ordinary
tests (338 unit plus 129 integration). Five intentional ignores were each run
separately in both profiles: four actual LLVM gates and the maximum-block
resource gate. Formatting, strict Clippy and all four sibling privacy probes pass.
The combined 926,896 raw comparisons have zero mismatches.

All seven unchanged source/native gates passed: loop control 186 sources,
while 144, mutable locals 163, Boolean logic 525, comparisons 1,610,
checked arithmetic 1,051 and native scalar/tool-boundary 57 cases. Reference
arithmetic made 4,100 checked invocations and literals 1,670. Metadata's seven
tests, the 24-entry feature inventory and repository verification of 121 sources/
67 runnable programs pass. Compiler binaries were rehashed after the chain:

- Debug: `a4cd6eb63aa3296bbb68d02355e985b39afbdd73d57de3c317a1d0845fe59d6e`
- Release: `58104cf0ee1dde75c958c0bdb3502059b4c7403c984189e63adbdf3cfe86b687`

The maximum-resource fixture's observed peak process RSS was 162,224 KiB on
this host. It includes caller-owned raw fixtures/clones, test harness, declaration facade, structural CFG scratch and
allocator overhead. It is reported separately from its 9,900,000 requested
ownership-scratch bytes, not as an isolated analysis-memory claim.

The independent review found and closed the sibling-seal and unconditional
constructor-buffer issues described above. Initial failed oracle/test/tool runs
are retained separately from corrected final qualification; they are not
relabeled as initial passes. No committed or remote CI result is claimed by
these local results.

This bounded evidence does not establish every program's safety, runtime
provenance, heap/resource ownership, stored-reference lifetimes, native aggregate
correctness or Rust equivalence. Those remain outside this verification-only unit.
