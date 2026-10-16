# Alias / Unicode source successor — review draft

This generation binds the published source checkpoint
858c70d862587b6a8b5d59a70afe4b8334b1f150, tree
c44b7dfb2938dc510016ff4939104a2236ba2e9c, parent
40e5aae618d7ce1f5efe7096ee634f19ab61063c. It is source-only and
execution_qualified=false. The source implementation and focused controls do
not establish full execution qualification or replace independent review.

`generation.py` authenticates the two genuine Git-protocol readbacks, rebuilds
all tree nodes and raw commit objects, checks the full four-file publication
scope, and verifies all 376 source bodies against both Git trees. Only
src/main.rs and src/runtime/data.rs differ. The unchanged, pinned retained
inverse recovers all 376 accepted bodies; 374 bodies remain identical.
The readbacks include raw Git commit and tree objects; verification does not
depend on REST API metadata.

`predecessor.py` materializes a finite accepted-main view: 2724 regular blobs,
78,507,317 logical bytes. The two unrelated executable entrypoints install.sh
and scripts/verify_repo.py are explicitly excluded. This view is not a Git
checkout. Changed active facades are preserved under predecessor-v1. Selected
source bodies come only from the verified inverse. The view is checked before
and after use and removed on exit, including failure paths.

`package_current.py` retains the entire formatter capture and all earlier
accounting bindings. Its Unit2 wrapper calls the unchanged original verifier
first, checks the actual instrumentation assembly, then checks both ordinary
source files against the new checkpoint and against the predecessor. Neither
source is an accounting dependency. `parser.py` transports the full historical
maps, retaining published_main_parser untouched and using a separate
package_json_parser provenance with plural source_deltas. Actual cardinalities
are source delta342, base539, derived542 and control542. Both ordinary sources
are excluded from instrumentation, control_instrumentation, inserted_roster
and ACCOUNTING_PATHS. Public lifecycle uses the original formatter admission
in the accepted view and verifies complete observer forward/inverse maps.

Current dispatcher, parser, public lifecycle, Unit1, bounded native entrypoints,
owned-array source snapshot closure, two workflow arguments, direct-consumer
plan, Unit4 common module and named CI input inventory are updated. Historical
published_formatter_current files and all existing named v1 source/lifecycle
artifacts are unchanged. No Unit3 historical authority or old qualification
result is upgraded or relabeled.

## Reproduction and focused checks

Pure producers: generation.py, parser.transport, derive_consumers.derive and
ci.derive. The review-tools scripts reproduce facade changes using the real
accepted-main Git objects; they are explicit mutating review tools, not runtime
entrypoints. Run activate_hosted.py from that directory after reviewing all source bodies;
it derives the dispatcher, parser, consumers, hosted bridge and final CI closure. Source artifacts are independently
recomputed byte-for-byte by the source controls.

- `python -B scripts/test_package_json_source.py -v`: 14 new source/parser
  controls, 3 accepted-view controls, 3 real runtime controls,
  37 unchanged v1 unit/refusal controls, 8 bounded-dispatch controls,
  and 5 hosted-bridge mechanical/refusal controls (70 total).
  Unit2 uses prepare-only and executes zero compilers.
- `python -B -m unittest discover -s tests/qualification/unit4_public_v3 -p test_lexer_reservation.py -v`
- Unit4 `LexerInputClosureControls` and `scripts/test_lexer_current_ci.py`.

Historical lexer-preservation fixtures require mode 0644 for the preservation
controls, which validate file modes as well as contents and membership.

## Hosted bridge and remaining execution scope

The active scripts/test_published_formatter_source.py is now a small pinned
facade for hosted_controls.py. Its original complete v1 body is retained under
predecessor-v1 at its original repository-relative path and checked against the
accepted Git blob. derive_hosted.py authenticates that exact original and changes
only four method seams: existing receipt admission, export namespace, export
schema, and the outer source-control run scope. All other old methods, all 92
controls, actual preparation commands, full-run predicates, failure handling,
exclusive evidence writes and preservation checks remain byte-identical.

The bridge authenticates the successor dispatcher and full current capture,
then runs the original v1 input_roster and controls against the accepted
formatter view. Its actual checkout identity and completed Unit2 evidence are
validated against the new source, including the original full Unit2 verifier
and both materialized ordinary sources. Historical source constants remain
historical; no checkpoint or receipt is relabeled. Exports use the new
source-controls-package-json-v1- namespace and preserve the six existing full
execution records. The complete accepted view is checked and closed before a
successful completion/index can be written. Its additional 2724-file,
78,507,317-byte fixture scope is explicitly recorded separately from the
unchanged nested-view effects audit.

Actual successor bootstrap and all 717 retained v1 pinned input bodies were
successfully authenticated in the genuine accepted view. Mechanical controls
check the exact generated bodies, original method preservation, full execution
gate, post-verifier placement, closure-before-success ordering and refusal of
missing, relabeled or bool/int-aliased provenance. These are source controls,
not a completed hosted run. The hosted full-run/export route still requires a
fresh complete Unit2 execution receipt in the real GitHub CI environment. That
route and the full retained 92-control hosted batch were not executed here;
no GitHub context or completed execution receipt was fabricated.

Full execution qualification requires the complete CI/qualification gates,
including the hosted bridge and its accepted-view cleanup checks.
