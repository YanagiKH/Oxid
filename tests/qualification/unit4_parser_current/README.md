# Current-source Unit4 parser qualification

This adapter admits native storage plan source checkpoint
`983dad1f21bdc9d37cf0d1364b58aad219e553d0`, full tree
`4380d906e868ad992d97ffc652e657773f1c2947`, while retaining the byte-identical
`tests/fixtures/typed_project_unit4_parser_portable/frozen/v3` package.

The replay uses the historical `parse_counted` entry, whose current implementation
selects `ArraySyntaxPolicy::Closed`, `EnumSyntaxPolicy::Closed` and
`StdImportPolicy::Closed`. It qualifies
that policy on current compiler bodies. Positive public enabled-enum grammar is
qualified by the separate feature/public gates; this historical-policy replay
must not be represented as positive public enum parser coverage.

## Exact source and preparation boundaries

The historical authority binds 283 base inputs, 113 compiler bodies and two
286-member derived views. Current source admission binds 264 inputs, including
206 `src/` and `native/` members, 56 retained non-source inputs and both scanner
fixtures. The historical-to-current override has 222 members: 78 changed
historical inputs and 144 additions. The current parser binds 427 base inputs,
209 compiler bodies and two 430-member derived views. These inventories have
different purposes and must not be substituted for one another.

`authority.json` pins the complete current maps, source checkpoint, immutable
historical identities, source-binding runner, transition packages and all
compatibility adapters. Every load derives both complete current maps again.
Unknown members, missing inputs, compiler deletions, reordered paths, changed
bytes and coherently rehashed input tails remain rejected. Scanner include
closure belongs to the separately pinned source-binding authority.

The native storage transition has no observer or control instrumentation overlap.
Its authority binds current execution source to the retained 262-input
`stdout-source.json`; no new instrumentation adapter is introduced.
The stdout transition overlaps observer/control instrumentation only at parser.
Its exact patch section first restores the retained 252-input stdin parser.
The stdin transition then overlaps only at AST and parser; its exact sections
restore their preserved 237-input enum-source identities. The stdout authority
binds the retained stdout predecessor. The separately pinned stdin and enum authorities
keep their own original checkpoints. No initializer or closed-policy semantic
projection changes are introduced by stdout.

Five enum transition paths overlap observer instrumentation: AST, parser,
source-map, namespace-resource and allocator-budget bodies. Only AST and parser
overlap control instrumentation. The exact enum sections first restore their
preserved projected-source identities. AST/parser then follow the unchanged
projected -> unary -> composition -> slices -> division -> combined inverse
chain; allocator-budget follows the existing combined inverse. Namespace-resource
is unchanged before the enum transition. Source-map first recovers projected
source, then removes the exact historical seven-line formatter accessor.
Every recovered historical body and historical composed body must equal its
immutable original identity before the original hook meaning is composed onto
actual current source. Lexer has no enum transition and retains its two original
hooks. Neither old bodies nor uninstrumented current bodies can replace a current
composed identity.

The frozen parser instrumentation helper needs three separately pinned location
adjustments: recovery-end before the new storage epilogue, path-node admission
before the conditional reservation, and the existing `unary` production label
at its moved prefix-loop location. Exact reversal restores every frozen helper
byte. The derived observer parser has an explicit panic bridge for new enum-only
`node()` routes, which are outside its closed-policy observation domain; the
control parser keeps the production method. The direct overflow probe initializer
retains the exact enum initializer predecessor, then adds closed std imports.
Reversing the separately pinned std insertion restores the enum initializer;
reversing its closed array/enum policies and empty syntax storage insertion
restores the complete frozen observer. No frozen
helper or instrumentation patch is edited.

Preparation validates historical input bytes against exact `d9e6b9bf` Git objects
and current inputs against the actual checkout HEAD before creating its output.
The original preparers produce the two verified 286-member historical views;
the adapter retains their original candidate/overlay manifests and then applies
the explicit current overrides. Generated current candidate/overlay manifests
bind the full current maps and authority. Builds consume those current views.
Historical metadata remains provenance rather than an execution receipt.

## Closed-policy AST carrier projection

Current raw Debug output adds `Program.enums` and represents the old absolute
path arena as `QualifiedPath` with `root: Crate`. Collection, raw files and
normalized observations preserve that output unchanged. Immediately before
predicate comparison, `project_enum_observations` applies the separately named
`unit4-closed-enum-ast-projection-v1` structural adapter:

- Require the exact current Program fields and an exactly empty enum vector
- Require exact QualifiedPath fields, root exactly Crate and the original
  integer path-length range 1..34
- Remove only that empty vector/root and rename QualifiedPath to AbsolutePath
- Run the unchanged frozen strict AST type checker on every projected AST
- Reverse those operations and require exact recovery of every observation field

Nonempty enum declarations, matches, enum item IDs, QualifiedValue expressions,
unknown fields/tags/roots and malformed paths reject. Even QualifiedValue with
arguments is rejected here: the closed entry still produces the original Call
carrier. The projection cannot supply expectations, hide new enum semantics or
change diagnostics, events, resource charges, bindings or source coordinates.

The `enum_structural_projection` receipt records the selected parser policies,
original/projected/restored canonical observation hashes, exact changed-row
indices and path counts. Comparison retains both the unadapted frozen comparison
and the projected frozen comparison, with canonical hashes. The current predicate
comparison consumes the projected rows and existing named diagnostic amendment.
The CI join independently re-derives the projection receipt from admitted current
normalized observations. Projection controls are source/admission evidence;
they are not a current semantic execution pass.

## Preserved contracts and required execution

The ten frozen helpers, comparator derivation and all 22 semantic predicate
handlers remain unchanged. The original effective amendment and the separate
six-case record-composition diagnostic amendment remain byte-identical. The
latter changes only its six explicitly named ProjectCandidate diagnostics and
retains the original effective comparison beside the successor. No enum-specific
semantic expected values are added.

A passing current parser result still requires four fresh observer/control
builds, 12 ordinary passivity pairs and all 638 frozen observations across
248 sources. Every build, collection, passivity and comparison rechecks the
checkout, derived bodies, historical provenance and current transition binding.
The compact capsule retains all 14 generated provenance artifacts. Each current
derived view includes a generated candidate manifest; its other 429 members and
the four executables may be full-archive-only, for 862 identities total.

Run bounded controls from the repository root:

    python3 -B tests/qualification/unit4_parser_current/test_current.py
    python3 -B tests/qualification/unit4_parser_current/test_record_composition_amendment.py
    python3 -B tests/qualification/unit4_ci/test_integration.py

Use the exact historical checkout and a fresh external output for preparation:

    python3 -B tests/qualification/unit4_ci/gate.py prepare-only \
      --repo "$CURRENT_REPO" --output "$FRESH_OUTPUT" \
      --expected-head "$CURRENT_HEAD" --event-sha "$CURRENT_HEAD" \
      --host 'Linux x86_64' --historical-repo "$HISTORICAL_REPO"

Preparation performs zero compiler executions and supplies no semantic pass.
Actual execution must use the current published qualification head and existing
host gate. Earlier historical or failed receipts cannot become current passes.
