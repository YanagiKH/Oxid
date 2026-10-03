# Current-source Unit4 parser qualification

This reviewed successor admits the private fixed-array groundwork while keeping
`tests/fixtures/typed_project_unit4_parser_portable/frozen/v3` byte-identical.
It uses the unchanged ten helper files, observer Rust, instrumentation, semantic
comparator, effective contract amendment, toolchain and dependency recipes.
The copied portable controller preserves every function outside the explicitly
reviewed source-admission, provenance and path-routing changes. Its comparator
derivation and all 22 semantic predicate handlers remain unchanged.

The historical parser package binds 283 base inputs, 113 compiler bodies and two
286-member derived views. The current source manifest binds 121 inputs; only
three existing compiler files differ from the historical package and one array
test file is added. All four paths are disjoint from the six observer and two
control instrumentation paths. The current parser therefore binds 284 base
inputs, 114 compiler bodies and two 287-member derived views. These counts refer
to different inventories and must not be interchanged.

`authority.json` independently pins the historical package identities, the
current source manifest, the exact four before/after file identities and the
current base/instrumented/control maps. No compiler output supplies an expected
source identity or semantic expectation. Every authority load re-derives the
four-path delta and current maps from the separately pinned historical/current
inputs. Unknown paths, deletions and instrumentation overlap are rejected.

Preparation checks all historical inputs against exact `d9e6b9bf` Git objects
and all current inputs against the actual checkout HEAD before making an output
directory. It invokes the unchanged historical preparers and verifies their exact
286-member outputs. Each original candidate/overlay manifest is retained under
`prepare` or `prepare-control` with a `historical-` filename. Only then are the
four approved current files copied into each build tree. Newly generated current
candidate/overlay manifests bind the current reviewed source checkpoint, current
source manifest and transition authority. Builds consume these current views.
Historical metadata is provenance only; it is never a current execution receipt.

The current session uses a distinct schema, `current_source_bound: true` and
`historical_source_equivalent: false`. Every build, passivity run, collection and
comparison rechecks the actual current checkout, all derived bodies, the retained
historical metadata and the transition binding. The compact evidence reader
consumes the same strict transition predicate and resolves both current candidate
manifest bodies against their pinned identities and re-derived 284-member map.
All 14 generated provenance artifacts remain in the compact capsule: the session,
the historical/current authorities and current source manifest, both historical
candidate/overlay pairs, and both current candidate/overlay/helper-manifest triples.
Each 287-member derived view includes its generated candidate manifest; its other
286 derived-tree members and the four executables may be full-archive-only (576 identities). The final join also binds the current 114-body map,
source checkpoint and actual build candidate/overlay identities.

Run bounded controls from the repository root:

    python3 -B tests/qualification/unit4_parser_current/test_current.py
    python3 -B tests/qualification/unit4_ci/test_integration.py

For the real complete preparation boundary, use an explicit exact historical
checkout and a fresh external output. Linux `gate.py prepare-only` now invokes
the same current parser preparation command as production stage 08:

    python3 -B tests/qualification/unit4_ci/gate.py prepare-only \
      --repo "$CURRENT_REPO" --output "$FRESH_OUTPUT" \
      --expected-head "$CURRENT_HEAD" --event-sha "$CURRENT_HEAD" \
      --host 'Linux x86_64' --historical-repo "$HISTORICAL_REPO"

Preparation reports zero compiler executions and no semantic qualification. A
passing current parser result requires four fresh observer/control builds,
12 ordinary passivity pairs and all 638 frozen observations, followed by the
unchanged effective comparator. Hosted qualification must run on the exact
published integration head. Earlier failed and historical passed receipts remain
unchanged and cannot be promoted into current passes.
