# Linux checked-caller fixture seam

This namespace is a synthetic local fixture, with `fixture_only=True` and
`qualified=False` on every result and probe outcome. It has no production entry
point, receipt writer, workflow, producer, compiler, capacity probe or loader for
fixture bodies. A file named `helper.py` is a fixed byte literal, never imported.

The caller and controls use two byte-identical reviewed dependencies under
`support/`, pinned in `fixed-inputs.json`. `support/provenance.json` records their
complete enclosing archive/member chain and exact hashes. These are recovered
Oxid development source bytes, not recreated execution evidence. Archive
identities document the retained byte chain; they do not establish originating
task authorization or component license coverage.

The repository retains its MIT LICENSE. Separate licensing provenance for the
recovered components remains unresolved: their inspected component tarballs and
source headers contain no separate license notice. This packaging does not assert
MIT coverage for those bytes, and publication remains deferred.

## Run the focused fixture controls

From this directory, using Linux and Python 3.11 or later:

```text
python -B -m unittest -v test_caller
python -B -O -m unittest -v test_caller
```

The manifest names canonical fixture-relative dependency paths, resolved from
the caller file directory independently of the working directory. Absolute, dot,
parent-traversal, repeated-separator and alias spellings are rejected before
normalization; symlink ancestors/files and hardlinked files are rejected before
reading. Missing or altered dependencies fail; there is no download, alternate
loader or fallback. Local
test-support source hashes are checked before admission. The manifest itself is
a reviewed fixed input; it is not an authentication mechanism for an untrusted
repository or arbitrary Python process. Digest checks never use `assert`, so
optimized Python retains them.

Tests create fresh literal fixtures with independent source, evidence and empty
owned roots. Expected rows/hashes exist before creation. Both trusted probes
receive one capability backed by one actual v5 view. The private native adapter
maps exact paths to established anchors and immutable expected records, then
calls the same-fd reader once. The retained core's reopening body reader is never
used. v5/core source files are loaded only after pin verification, without edits.

Controls cover four nlink observations, exact byte bounds including empty
files, partial traversal/initial admission, fresh final seals of unconsumed
siblings, escaped bound methods, source/raw failures, cancellation identity,
ordered independent finalization, descriptor closure and replaced owner roots.
Private instrumentation is synchronous and unavailable through the consumer
API. Observation injections are distinguished from actual temporary-file and
directory replacement controls. Neither is production qualification evidence.

## Limits

The two probes are trusted reviewed harness functions. Registration is an
in-process identity check, not a sandbox or proof that a function is not a
disguised production consumer. Deliberate introspection, monkeypatching by probes,
threads, asynchronous work and child processes are outside this fixture contract.

The owner creates an empty namespace. Cleanup refuses unexpected entries and
uses `rmdir` on that one root after identity checks. It never recursively deletes
candidate data or follows replacement roots. Trusted synchronous ownership with
no concurrent external mutation is required; this is not generally race-free
recursive cleanup. Test-harness teardown of deliberate replacement trees is
separate. Cleanup failures retain the target path in exception notes.

Initial/final seals remain caller obligations. v5 `recheck_layout` does not read
all bodies, and v5/core metadata identities do not supply the fixture reader's
full uid/gid/nlink checks. The fixture therefore retains separate anchor,
metadata, known-absence and complete body checks. No claim of production loader
closure, launcher identity, provenance, valid corpus coverage, performance budget
or production qualification readiness follows from these controls.

## Explicit hosted development checks

`scripts/test_frontend_v3_checked_caller_fixture.py` is included by the existing
`scripts/test_*.py` unittest discovery in the Ubuntu full-verification CI job.
Its two hosted cases launch isolated normal and optimized Python children, each
requiring the fixed 42 unique control IDs to be discovered, started, completed
and successful. Inner skips, expected failures, unexpected successes, failures,
errors, incomplete results and invalid summaries fail the wrapper. Each child
has a named 60-second watchdog; timeout kills and reaps it. Protocol controls in
the wrapper exercise rejection and propagation without running unrelated suites.
A Linux-only real-child control separately checks watchdog termination/reaping:
a private readiness file has a ten-second startup bound before the test-only
one-second timeout begins. Its trusted helper remains alive until killed, and
its diagnostic is retained. This makes no descendant/process-group guarantee.

Linux requires Python 3.11+ and the fixture's filesystem capabilities. Non-Linux
hosts explicitly skip the two hosted cases and the Linux-only real-child
watchdog control as unsupported development fixture scope, never as qualification. The outer runner counts wrapper cases; child
summaries separately report 42 normal and 42 optimized fixture controls. Existing
workflows, Unit4 rosters, historical pins and qualification budgets are unchanged.
`make verify` alone does not perform Python script discovery.
