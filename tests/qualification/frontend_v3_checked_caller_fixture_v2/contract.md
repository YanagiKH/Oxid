# Fixture-only checked caller contract v2

This document describes the synthetic checked-caller fixture and its bounded test-support loader. No production admission, launcher, compiler, producer, capacity probe or qualification is included.

## Exact fixture interface and trust boundary

inspect_fixture_pair(fixture, binding, owner) accepts exact reviewed in-process record classes FixtureV2, FixtureBindingV2 and FixtureOwnerV2, never dict substitutes or subclasses. FixtureV2 fields are exactly schema, source_root, evidence_root, wide_root, source_rows, outer_rows, known_absent, reader_pin. schema must equal 'oxid-v3-local-fixture-v2'. Paths are canonical absolute ASCII component strings; wide_root is exactly evidence_root/wide-span. Rows are immutable, ordered, complete directory/file rows including each root, with original-coordinate component tuples, permission mode and, for files, expected bytes and SHA256. Constants are authored before fixture creation, not derived from a candidate scan. Root/directory modes are explicit. No links, special files, omitted directories or invented metadata are admitted in this slice. known_absent is an explicit tuple of evidence-relative absent paths with admitted directory parents. reader_pin identifies the reviewed local physical_checks.py bytes, with its exact digest recorded in fixed-inputs.json.

FixtureBindingV2 fields are exactly schema, source_probe, raw_probe. schema must equal 'oxid-v3-local-fixture-probes-v2'. Only the two trusted synchronous test probes passed by the reviewed test harness are accepted. Production-shaped bindings, other schemas, extra fields, subclasses and production consumer entry points reject before consumer entry. The API has no production mode, authentication placeholder, qualification callback, publisher or output receipt path. Every result has exact schema 'oxid-v3-local-fixture-result-v2', qualified=False, fixture_only=True, ordered probe outcomes and finalization record. Success cannot produce a production receipt. The fixture tests exercise the caller protocol, not real wide-validator closure. Probe trust is a precondition: this is not a Python sandbox against arbitrary introspection, threads or malicious callbacks. Probes may not start asynchronous work or processes.

FixtureOwnerV2 is constructed only by reviewed fixture-owner code. Its exact fields are schema='oxid-v3-fixture-owner-v2', ownership_token, owned_root, owned_root_identity, parent_identity. The owner independently creates the temporary namespace and records its path and identities at creation, before constructing the fixture or running admission. Ownership is not accepted from fixture data, consumer output or a freely supplied release callback. The caller holds the owner and cleanup implementation privately. Source/evidence roots are non-owned read-only scopes for this caller; their eventual test-harness teardown is separate. All three roots must be pairwise disjoint, with no ancestor/descendant overlap, physically non-aliased, and matched to validated ancestor chains. The owner root cannot contain or be contained by source/evidence. Unknown/replaced roots are never cleanup targets.

Production loader/helper closure, real consumer bindings, source/event/execution provenance, launcher identity, report parsing and the full production corpus are deferred obligations. They have no placeholder fields here. The fixture has no general pathlib facade or import engine.

## Consumer capability and single-view dispatch

After admission construct exactly one full-outer EvidenceAuthority, one SourceAuthority and one FreshReadView from the pinned v5 code. Zero views is valid when earlier admission fails. Construction failure counts as attempted construction, never an admitted view. No child authority/view is created. Root rows remain in caller obligations even though v5 entries omit them.

Both probes receive only the same narrow FixtureReads capability. Its complete public operation set is read_source(relative_components), read_wide(relative_components), read_outer(relative_components). Each returns bytes; each requires exact tuple-of-component input naming an admitted regular file. read_wide prefixes ('wide-span',) in the original outer coordinates. No path objects, raw view, native paths, file descriptors, stat results, enumeration, callbacks or underlying bound methods are returned. The capability exposes no other public operation. Expected fixture bytes and results are ordinary immutable values, not admission capabilities.

Every invocation first checks the same irreversible session state == CONSUMING, including a bound method captured earlier. Dispatch then uses the single private view.regular at its admitted original absolute coordinate. The native core adapter's regular is the pinned reader described below. Both probes share that exact view object; identity/count assertions are caller/test instrumentation unavailable to probes. Caller preservation uses separate private functions without the consumer capability and cannot restore CONSUMING. Session states progress ADMITTING -> CONSUMING (only on successful admission) -> REVOKED -> CLOSED; failures during admission move directly to REVOKED. No state can move backwards.

## Pinned regular reader protocol (Linux only)

Exact signature: read_regular(root_anchor, relative_components, expected_file) -> bytes. Inputs are caller-private exact records/tuples. RootAnchor retains an opened directory fd, its admitted identity and the original root/ancestor chain. ExpectedFile contains permission_mode, bytes, sha256; bytes is an exact nonnegative int. No consumer can supply anchors or expected records. The implemented function's reviewed source digest is pinned by fixed-inputs.json and verified before use.

Reject platforms without Linux POSIX dir_fd operations, O_NOFOLLOW, O_NONBLOCK, O_CLOEXEC, O_DIRECTORY, fd-based listing and nanosecond timestamps; do not silently replace missing flags with zero. No Windows fallback. Open/traverse directories relative to retained parent fds using O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC; verify directory type, identity and parent chain. Reject symlink/reparse observations, missing fields, special files and lexical aliases. Anchor checks compare both held descriptors and current original pathname/ancestor observations; retaining an fd alone cannot excuse root replacement.

For the final component perform non-following stat relative to its pinned parent fd, then os.open with O_RDONLY|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC and dir_fd=parent_fd. Before any body read require a regular file, st_nlink==1, expected permission mode and st_size==expected_file.bytes. Full identity is (st_dev, st_ino, st_mode, st_nlink, st_uid, st_gid, st_size, st_mtime_ns, st_ctime_ns); require every field. Compare pre-open stat, fstat immediately after open, fstat after reading, and final non-following stat of the original name against that exact tuple. Also recheck admitted ancestors/root names. Reparse-style attributes, if present and nonzero for aliasing, reject. Directory identities exclude size/time/link-count fluctuations but bind device, inode, full mode, uid and gid; directories need not have nlink==1.

Read from that SAME fd in chunks at most min(65536, remaining_expected_bytes), refusing early EOF; after exactly expected bytes read at most one byte to demand EOF. Total body read cap is expected bytes plus one sentinel byte, with no limit increase, size discovery loop or alternate reopening reader. Unexpected extra data, EAGAIN or any read error rejects. Hash the exact returned bytes and require the expected digest. fd-after and name/ancestor postchecks must also be attempted on read/hash failure, with errors preserved. Every opened fd, including partial directory traversals, closes in finally on all failures and cancellation; failed close is retained. No independent reopening reader can stand in for these same-fd checks.

All regular content reads dispatch here: initial source/evidence body seals, every v5 regular read for either probe, and final source/evidence body seals. No fixture report/module body parser or dynamic loader exists. Loading already reviewed local test support is outside consumer IO and must be pinned by the test manifest; any admitted fixture helper file remains in full seals. Metadata-only v5 operations use the retained core metadata primitives plus caller nlink/root checks, not an unchecked alternate body reader.

## Admission, partial scope and ordered finalization

Register cleanup responsibility immediately after independently validating owner identity/separation, before reading candidate bodies. Reject invalid owner records without deleting anything; the fixture owner retains teardown responsibility and the caller reports ownership unavailable. Record each source/evidence root scope as established only once its admitted directory identity/ancestor chain and complete expected row roster are known, BEFORE its first body read. Thus failure partway through the initial seal still requires a complete final check of that established scope against expected constants. Record initial seal success separately. Failure before establishment records that scope as unavailable with the original reason; never call it preserved. Attempt independent source/evidence admission checks where safely possible; no consumers after any failure. A failed view construction does not eliminate established root obligations.

At the end of consumer execution, or immediately upon admission/consumer BaseException, irreversibly revoke consumer capabilities FIRST. Then, in this exact order, independently attempt: (1) caller-only v5 recheck_layout if a view was admitted, otherwise record unavailable; (2) extra root/ancestor/nlink and known-absence checks for established scopes; (3) complete source final seal; (4) complete outer evidence final seal; (5) close caller reader/anchor resources and dispose of private view references; (6) attempt owned-scope cleanup exactly once, last among external actions; (7) set session CLOSED; (8) return the unqualified result or propagate failure. Each step is protected so its failure cannot skip later steps. Consumer methods remain revoked throughout; final checks use caller-private access and never reopen consumer capabilities. Full final seals compare against expected constants and, when available, successful initial seals. They include every body, empty directory and sibling, not only consumed wide files. v5 layout recheck is metadata/membership-only and cannot substitute for these body seals.

Failure records are ordered tuples (phase_label, original_exception), with primary admission/source_probe/raw_probe failures first in occurrence order, then revoke, layout, extra_metadata, source_final, evidence_final, resource_close, cleanup, session_close failures in attempt order. Unavailable scopes have labelled reason records distinct from passes; admission errors remain failures. Catch BaseException for finalization obligations. If any cancellation occurs, preserve the first original cancellation object by identity and re-raise that SAME object after cleanup and CLOSED; attach the complete ordered failure record and chain a BaseExceptionGroup of other failures when present. Never convert cancellation to success or an ordinary error. Without cancellation, raise a BaseExceptionGroup retaining original exception objects and labels. If only one error exists it is still represented with its label. The implementation must explicitly classify cancellation (KeyboardInterrupt, SystemExit and the harness-designated cancellation type); it must not infer cancellation from exception text.

Cleanup uses only the independently recorded owned root and pinned parent identity. Recheck root/parent identities without following links before deletion. If the root is replaced, or ownership/parent identity cannot be established, leave it untouched and record cleanup failure; never chase the replacement or retry another target. Cleanup is confined to the reviewed private test namespace, rejecting unexpected aliases/types. These pathname checks are NOT a claim of generally race-free recursive deletion. Trusted synchronous fixture ownership with no concurrent external mutation is a precondition; a platform that cannot enforce the specified checks rejects cleanup and reports the leftover path. Session closure must occur even when cleanup fails. Test-harness disposal of deliberate replacement fixtures is separate from caller cleanup.

## Controls and implementation scope

Smallest fixture: separate source/evidence/owned roots, one source helper body, explicit outer root and empty sibling directory, wide-span/source.ox, wide-span/raw.stdout, shared wide body and one unconsumed outer sibling. Expectations come from fixed literals. Two trusted synchronous probes read their respective and shared bodies through the same capability. Assert one admitted physical view/outer authority and actual checked-read dispatch for both; all successful output remains fixture_only=True and qualified=False.

Parameter cases: success; source failure preventing raw; raw failure; partial source/evidence initial-seal failure; failure before scope establishment; failed view construction (zero admitted views); sibling body/membership mutation; initial hardlink; link-count changes at pre/fd-before/fd-after/post observations; early EOF/extra byte/read/hash/close failures; final layout/body failure; escaped bound method called during preservation, during cleanup and after return; cleanup failure; owned-root replacement; cancellation from a probe and during preservation. Assert ordered original error objects, independent final attempts, unavailable labels, descriptor closure, revocation before any final checks, cleanup once/last, CLOSED on failure, untouched replacement root, and unchanged non-owned source/evidence cleanup ownership. Hooks are private test instrumentation, never consumer operations. These cases are synthetic development controls, not production qualification.

Implementation lives in tests/qualification/frontend_v3_checked_caller_fixture_v2, including the two exact pinned support modules and provenance metadata. The explicit scripts/test_frontend_v3_checked_caller_fixture.py wrapper runs these controls through existing script discovery. There are no production bindings, general loader, workflow changes or qualification path. Production consumer/helper routing, launcher compatibility, authenticated provenance and full valid production corpus remain separate work.

## Exact recovered interfaces

The recovered native core is scripts/run_hir_producer_v3_ci.py (8,732 bytes,
SHA-256 306d5ce9874412f45f63f052e4d1ed5a70377ea58d2e0a4ec00ba4e2370bf9aa).
Its exact body interface is regular(path), delegating to read(path), returning
bytes or raising exceptions. It has no status/result tuple. Its need checks raise
ValueError and OS errors propagate; its original finally close can supersede an
active exception. It does not implement this fixture's expected-size/nlink/full
identity contract and is never used as a fallback body reader.

The recovered sealed_read_view.py v5 is 20,144 bytes, SHA-256
ba3cad72317bea5a0ef1177c35c548040ccdbafc4e74f70e76f92c04f58cd34f. Its constructor
captures core.regular, core.loads, core.stamp, core.ancestor and core.windows_host.
FreshReadView.regular checks the descriptor and metadata, calls the captured
reader once as native_read(NativePath(name)), checks exact bytes type, length and
SHA-256, then checks metadata again. Reader exceptions propagate. The trailing
v5 metadata check is not in a finally; the fixture reader therefore performs its
own fd/name/ancestor checks even on read/hash failure. No v5/core file is edited.

Before constructing the view, the caller builds one private MappingProxyType
from canonical original absolute file names to (established RootAnchor,
relative-component tuple, immutable ExpectedFile). Source/evidence roots are
separate and each name belongs to exactly one scope. Native regular accepts an
exact Linux PosixPath, validates canonical spelling and exact map membership,
and calls read_regular(anchor, components, expected) exactly once. It never
reopens through core.read, retries another path, discovers a new expected size,
or falls back. RootAnchor also retains the immutable admitted identities of
nested directories, checked before opening/reading descendants. Partial scopes
retain their available identities and full expected roster; unestablished
ancestors refuse body reads while remaining final obligations are attempted.

FixtureCancellation is the explicit class FixtureCancellation(BaseException)
in caller.py. It, KeyboardInterrupt and SystemExit are cancellation; exception
text is never used. The first original cancellation object in occurrence order
is re-raised after cleanup and CLOSED, with fixture_failures (ordered labelled
original exceptions) and fixture_result attached. Other original failures are
chained in a BaseExceptionGroup. Non-cancellation errors are always grouped and
carry the same records. Physical reader groups preserve original error objects.

Each trusted probe has interface probe(FixtureReads). The source probe must
return the singleton SOURCE_OUTCOME, the raw probe RAW_OUTCOME. Both are frozen
FixtureOutcome records with schema='oxid-v3-local-fixture-probe-outcome-v2',
probe='source'/'raw', value='fixed-source-ok'/'fixed-raw-ok', fixture_only=True,
qualified=False. No arbitrary payload, path, handle, callback or admission
capability can be returned as a successful outcome. A clone of the same record
is rejected. The harness registers exact function objects in one exact
FixtureBindingV2; this is a trust precondition and in-process identity check,
not a Python sandbox or a detector of disguised production functions.

The owner implementation creates only an empty temporary namespace. Cleanup
requires the same privately registered owner, root and parent identities and
an empty non-aliased directory, then uses one rmdir relative to its pinned parent.
Unexpected contents/replacements are left untouched and reported. No recursive
cleanup is needed by this fixture. Invalid ownership leaves teardown with the
harness. Resource close failures are retained; no callback or pathname operation
occurs after the last cleanup attempt. CLOSED is an unconditional private state
assignment after that attempt.

fixed-inputs.json supplies the actual physical_checks.py digest, exact recovered
v5/core paths and digests, and local support source pins. This is a local reviewed
manifest, not independent production authentication. The implementation and direct tests live in this namespace; the explicit hosted
development wrapper lives under scripts/. No producer/compiler, capacity probe
or production consumer is invoked.

## Portable support packaging clarification

Support packaging consists of support/sealed_read_view.py,
support/run_hir_producer_v3_ci.py and support/provenance.json. The two dependency
byte sequences and digests remain unchanged. Canonical manifest-relative names
are validated before pathlib normalization and resolved against the caller's
directory; no ambient recovery path, download or fallback is used. Support path
checks refuse symlink ancestors/files and hardlinked files. Each load captures
bytes once, verifies their digest, and compiles that same captured value. This
trusted synchronous harness is not a concurrent hostile-filesystem sandbox.

The repository MIT notice is retained; separate recovered-component licensing
provenance is unresolved, explicitly recorded, and publication is deferred.
The explicit wrapper is included in existing scripts/test_*.py discovery; no
workflow or closed Unit4 roster is changed. Support packaging does not alter
the reader or owner protocols or produce qualification evidence.

## Ownership separation and hosted watchdog controls

Before cleanup responsibility is registered, owner validation opens root-inclusive
ancestor chains for source, evidence and owner. Each root device/inode pair must
be absent from both other chains. Missing roots or failed chain admission leave
cleanup unavailable; temporary validation descriptors are closed on failure.
Cleanup parent acquisition is protected and failures retain the owned-root path.

The hosted wrapper requires the fixed set of 42 IDs at discovery, start, stop and
success in both isolated normal and optimized Python children. Its production
watchdog is 60 seconds. A separate trusted direct-child test waits at most ten
seconds for readiness before testing a one-second timeout, and checks diagnostic
retention, termination and reaping. It does not claim process-group guarantees.
