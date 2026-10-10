# Final-freeze generation and activation recipe

This recipe is preparation only. **Do not execute generation until the
coordinator supplies the final committed compiler checkpoint and independent
source-freeze approval.** Also obtain independent review of the exact adapter
bytes before any generated source identity is accepted. The preparation branch
is neither approval nor a final compiler checkpoint. No command in this document
has been run to generate a lexer successor.

## 1. Freeze the two inputs, then derive the scope

Record full compiler commit C and tree T, full reviewed adapter commit A, and
both independent approval references. Verify clean tracked and untracked input
closures at each checkpoint. Read source only from committed Git objects or a
fresh checkout of C, never the primary worker's mutable tree. No Cargo/rustc
command runs without the serial build lease.

The source predecessor is the complete 74,192-byte manifest with SHA-256
402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa,
commit e3c1b4a1a3ef457326f11a802896c125202fe797, tree
5fdb4f06a8fcbc61724676df55a4c3bee130eaf7. Preserve it as
byte-storage-source-v1.json and preserve its original current-source.json bytes
where the retained byte-storage package expects that name. Do not modify the
byte-storage helper, authority, transition patch, or any older input.

Enumerate every committed member of src/ and native/ in C with Git mode and blob,
plus the retained Cargo.toml, Cargo.lock, build.rs, all 78 fixture bodies, and all
other retained selected members. All 376 predecessor members must remain.
Compare complete source-directory membership, including non-Rust proof files;
reject symlinks, executable input modes, extra files, and extra empty directories.
Derive additions from the actual committed tree. Have every addition explicitly
reviewed; no assumed four-file roster, deletions, or generic cache exclusions.

Known possible changes include lexer.rs, project.rs, format.rs,
lexical_provider.rs, project/budget.rs, project/budget_real_null_observer.rs,
oir/owned/source/array_pipeline.rs, its caller tests, and the separately approved
read-only tracker-layout accessor in oir/owned/source/reviewer_source.rs. This
list is informative, never an authority roster. Further observer seams are
pending, so C must not be guessed from the preparation checkpoint.

For each selected path bind complete bytes, SHA-256, Git blob and mode. Derive
ordered changed paths P by comparing old and new selected maps. Confirm this
matches committed Git delta under the selected paths. Generate exactly:

git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE_TREE CHECKPOINT_TREE -- PATHS

Use the literal predecessor tree above, T, and ordered P as argv elements; no
shell wildcard scope or untracked working-copy input. Write a **new**
lexer-reservation-transition-v1.patch. Never overwrite an earlier patch. The
retained inverse parser accepts textual exact-offset hunks and additions; if
Git emits a binary patch or other unsupported form, stop for review rather than
silently replacing the inverse implementation or normalizing bytes.

## 2. Build and independently verify the outer authority

Generate new lexer-reservation-source-v1.json and
lexer-reservation-authority-v1.json. Keep the predecessor provenance fields
unchanged; only files, purpose, reviewed_source_head, source_only_tree, and the
explicit lexer_reservation_predecessor_sha256 field differ in the new manifest.
The authority schema consumed by source_transition.admit records:

- The exact recipe, C/T/A, predecessor commit/tree and source bytes/digest
- Current manifest bytes/digest; patch bytes/digest; sorted changed paths
- Exact named compiler/fixture additions, no removals
- Complete current and predecessor input identities
- Every changed path with before/after identities, including additions
- Actual current/predecessor/compiler/body member counts
- Complete compile-time fixture body identities
- Exact ordered include source/ordinal/expression inventory and named additions
- All three unchanged Unit2 accounting dependency identities

Use the retained public lexical scanner only as a lexical expression inventory.
Independently inspect closure changes; preserve all 136 predecessor expressions
and all 78 original fixture bodies. Macro-generated fixture closure still needs
the full unchanged historical preflight after inversion. Neither the scanner
nor Rust-file extension filtering alone establishes that closure.

Independently reconstruct the 376-member predecessor with the pinned exact
inverse parser. Compare every recovered body, mode/blob identity, include and
fixture row. Check wrong stage and double inversion reject. On a separate fresh
materialization call **unchanged byte_storage.admit** with the exact predecessor
manifest under its expected current-source.json name. Never give that old helper
new compiler bodies.

Continue the entire original inverse chain using an unchanged saved public
source-binding runner and its complete original package. Full preflight also
needs its authenticated public repository_inputs, complete independent Unit2
package, lexical/diagnostic/producer closures, and existing resource/semantic
inputs; the 376 selected compiler inputs alone are insufficient. Capture these
public closures before materialization with the old identities and membership
rules. Preserve the old package-manifest.json (SHA-256 c68a80705e43c2711d492d68fdbf63213e7d7b7f02dd1288c12403382ad3208c)
and all bytes it lists. Do not copy a broad private-parser tree as a shortcut.
The final archived selected view remains the original 117 members.

The frozen seal supplied to admit must pin the source manifest, authority, patch,
C/T/A, and helper package bytes in a trusted current dispatcher. It is not a
caller-supplied arbitrary hash. Generated maps and seals are reviewed outputs;
self-consistent regeneration alone is not independent approval.

## 3. Integrate current execution separately from historical restoration

Keep the existing source-binding dispatcher as a named immutable predecessor
before adding an explicit outer current stage. In current execution, use the new
admitted input bodies, not the restored old bodies. Historical preparation uses
the restored closed predecessor and retains its historical receipt identity.
Include the new outer stage in current/head/event associations and source
preservation receipts. A source-only preflight or prepare-only result is never a
semantic/resource execution pass.

Preserve the unchanged Unit2 u8 accounting authority and transport receipt. Its
resource.rs, sealed.rs, and u8_reservation.rs dependencies must be byte-identical
between new and restored maps. Any changed dependency or resource endpoint is a
new review boundary, not permission to recompute the old oracle.

Derive a separately named current Unit1 runner from the exact original runner.
It stages unchanged frozen helper/fixture files, replacing only the staged
reviewer_additional.rs with unit1_counter_domain's exact reversible output. Bind
both bodies and the all-69-case correspondence receipt. Run all 69 original names
in both profiles, retaining their identity, plus the separately listed single
current control from unit1_lexer_controls.rs. The original run.py and fixture
files stay byte-identical. Keep the original case and extra control totals
separate in receipts, and preserve the original runner's before/after source,
resource, executable, and roster checks. No build is authorized by this recipe.

## 4. Compose complete Unit4 public and current-parser successors

Preserve the current public authority and observer-u8-v1.patch as named
predecessors. Derive the new lifecycle overlay only after C/T is frozen. For the
lexer, apply the two exact current seams in adapters.py, bind both full body
identities, and verify exact reversal. Transport every other retained lifecycle
insertion unchanged at its existing semantic position. Regenerate its exact
hunk coordinates against C; never rely on fuzz or preserve stale offsets.
Compare the complete inserted-event roster and all nonlexer inserted bytes with
the retained overlay. Materialize complete ordinary and lifecycle maps and
independently reverse every overlay section to the exact current source.

Preserve the current parser authority (539 base / 542 observer / 542 control)
under an explicit byte-storage predecessor name. Retain compose_division_lexer
and its historical checks unchanged. New current composition first authenticates
the outer inverse to the exact predecessor, then uses the retained public
interfaces for historical relationship checks. The new lexer observer body uses
instrument_lexer(kind='token'); the parser control body stays uninstrumented.
The changed budget observer body uses instrument_budget's retained reserve hook;
its control body stays uninstrumented. All other existing semantic amendments,
closed-policy adapters and fixed observation rosters remain unchanged.

Derive complete current base, observer-derived, control-derived and candidate
manifest maps from actual bodies and the reviewed complete compiler membership.
Do not copy an old map and merely replace source SHA literals. Include all new
source/test/observer members; derive new map sizes rather than assuming unchanged
539/542/542 counts. Bind exact old/current instrumentation overlaps and reject
stale maps or any unexplained observer/control difference.

**Private boundary:** only the approved public current composer interface may be
used. Do not open, inspect, regenerate or restart the paused private parser
research/helper package. If the existing interface cannot produce a required
artifact under this permission boundary, report the exact interface and needed
input to the coordinator and stop that gate. This preparation has not invoked
it and does not claim the full parser successor can already be produced.

## 5. Close every direct and transitive CI consumer

ci_inventory.inventory authenticates the complete approved workflow plus audit
and yields all 173 entries with exact commands/job/step/line identities. Create
an explicit disposition for every entry; no missing entry or blanket waiver.
Retain distinctions between current semantics, historical execution authenticated
through the outer inverse, repository/artifact checks, and terminal evidence
preservation. A private-boundary block is recorded as blocked, not passed.

Explicit active-pin integrations include:

- source-binding current dispatcher, package membership, manifest and helper pins
- verify_bounded_byte_storage.py; verify_bounded_enum_native.py and its controls
- verify_bounded_stdin_native.py (including member count), stdout controller,
  and both workflow command-line source-manifest arguments
- Unit4 public authority/build/runtime/current input/observer maps
- Unit4 current parser portable.py authority pin and complete source/maps
- Unit4 CI common.py and complete inputs.json closure, with new closed roots
- Native producer/source inventories and current build receipt admission
- Documentation's active pointers, while every historical receipt stays unchanged

Also run the audited transitive standalone typed/streaming lexer, typed/static
parser, provider/HIR, formatter/module/source, source/privacy/native and
repository consumers. Preserve dependency-light copied closures; no unconditional
growth-observer dependency may enter a non-test build. Keep all direct commands,
profiles, failure collectors/uploads, genuine LLVM staging, actual-host matrix,
and terminal artifact joins. The old 271-member/eight-root CI input inventory is
a predecessor; enumerate the actual new closure after all approved adapter bytes
are committed. Do not simply update its SHA while omitting added helpers.

## 6. Review, negative controls, then execution under the serial lease

Before any compiler invocation, run positive exact forward/inverse checks and
negative controls for every changed hunk/context; missing/extra/new files,
symlinks/modes; fixture mutation; changed/missing/reordered/duplicate includes;
patch section order/duplicates; wrong/double inverse; coherent helper/manifest/
authority tampering; stale maps; missing/double lifecycle/token/reserve callbacks;
unexpected materialization or invocation before failed admission; and old/new
accounting and head/event associations. Preparation tests cover only their named
helper-level cases, not this entire final authority matrix.

After independent final source and adapter acceptance, run the approved complete
current Unit1/Unit2/public Unit4/parser gates, full Rust/Python/formatter/source
and native controls, no-failure corpus token-event equality, each new failure
site and full span/EOF controls, wrapper/project/provider/formatter/source-byte
lifecycle matrix, no-subsequent-work, and the original fresh-storage plus new
real-null refusal/ownership suites. Missing and duplicate callbacks must fail
execution checks, not just source hashes. Preserve all failed/blocked attempts.

Finally verify every applicable CI job on the exact published PR head only after
publication is separately authorized. Actual foreign hosts, installed LLVM,
root/capability tests, and the four-host join cannot be simulated locally.
Passing this work does not establish process-wide OOM diagnostic delivery,
native graph/call recovery, or completion of RFC0031's compound obligations.
