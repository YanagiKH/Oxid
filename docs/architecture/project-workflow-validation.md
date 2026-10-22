# Typed project workflow validation

The manifest workflow in [RFC 0032](../../rfcs/0032-typed-project-workflow.md)
adds single-package orchestration around the existing typed compiler. It does
not complete R1 or certify any v1 release gate. The development workflow and
its limits are described in [Typed projects](../TYPED_PROJECTS.md).

## Current source checks

Repository CI checks out the pull request head or triggering commit explicitly.
It runs all Cargo tests, formatting, Clippy, repository program checks, the
current Python script suite, and public native regressions in both compiler
profiles with LLVM 19.1.7. The five-host Rust matrix checks the compiler host;
the native project workflow remains Linux x86_64 only.

`scripts/run_current_script_tests.py` automatically discovers current script
tests. It explicitly lists historical-only modules and reports the reason for
each; those unchanged modules run in the mandatory historical job described
below. Their bytes must match the predecessor, so later additions cannot be
silently delegated without execution. New test modules are included by default.
Import failures and test failures fail the current job.

`scripts/verify_project_workflow_current.py` separately runs the retained Unit2
semantic and resource assertions against the complete current compiler input
set. The authenticated predecessor is
`9ff8ef90a26f003f4616838dfc186fa3bfb6845c`. Admission permits changes only in
the five CLI, manifest and build orchestration files plus the new
`src/project_cli.rs`; the other 371 inputs remain byte-identical. Compile-time
include and path directives cannot add inputs through the changed files.

The gate verifies the expected checkout SHA, materialized source bytes, resource
instrumentation, all 3,603 semantic cases and 21 resource tests in each of debug
and release, and six observer controls per profile. It records the current
source manifest, controller digest, commands, receipts and final source check.
Run it from a clean commit with an output directory outside the repository:

```sh
python3 -B scripts/verify_project_workflow_current.py \
  --repo . --expected-head "$(git rev-parse HEAD)" \
  --output /tmp/oxid-project-unit2
```

The output directory must not exist. `--allow-dirty` labels local results as
worktree evidence; CI does not enable it. `--prepare-only` performs admission
and assembly with zero compiler executions and cannot report a semantic pass.
This narrowly scoped gate must be revised when a later language change modifies
additional compiler inputs; widening its source allowance alone cannot qualify
new semantics.

`scripts/verify_current_native.py` runs the existing independent native oracles,
private LLVM regression groups, checked-HIR import, and expression/stack/stdin
applications against debug and release binaries built from a clean current
checkout. It records the actual compiler and test binary hashes, selected Rust
tests, commands, stdout/stderr and retained native artifacts, and rejects a
changed checkout or executable. These results supplement the public project
tests; they do not reissue the private historical certificates.

## Historical certificates

`.github/workflows/historical-qualification.yml` retains the previous jobs,
assertions, host matrices and artifacts. Qualification checkouts and expected-head
inputs use the fixed predecessor SHA above. The caller's event SHA remains
unchanged as event metadata. Source-pinned tests still read the original workflow
and source tree, and their expected hashes are unchanged.

One frozen suite, `test_published_formatter_source`, requires its checkout to be
the actual GitHub event commit. A later pull request cannot replay that event.
`scripts/run_historical_script_tests.py` therefore verifies its original genuine
schedule run [39226443899](https://github.com/YanagiKH/Oxid/actions/runs/39226443899),
attempt 1, and successful quality job/step through GitHub's API. It authenticates
the exact original artifact retained under
[historical-event-9ff8ef9](../evidence/historical-event-9ff8ef9/README.md), including
every indexed source-control receipt, original Unit2 input, completion count and
checkout identity. The archive is retained to survive hosted artifact expiry.
Only this exact module uses that existing evidence; every other frozen script
test is discovered and executed normally. Unavailable or mismatched provenance,
changed sources and replay failures fail CI. No GitHub event variables are
rewritten, and the output labels original evidence separately from replayed tests.

The current replay controller and archive are copied to runner temporary storage
before checking out the historical revision. Their hashes and actual caller
metadata are recorded in a separate `historical-script-replay` receipt.

Repository CI requires both this replay and the current checks. Historical
receipts describe the predecessor only. In particular they do not establish
current Unit1, Unit3 or Unit4 private certification, performance budgets,
self-hosting, or v1 readiness. Current Unit2 receipts and public CLI/native
results have distinct artifact names and state their actual scope.

## Release checks

`scripts/check_release_gates.py` reports the technical gaps in
`docs/release-gates-v1.json`. A release tag must match `Cargo.toml`; a stable
v1 release is rejected until real technical acceptance collectors and passing
results exist. Inventory flags cannot substitute for those results. Explicit
prereleases are published as prereleases. Release packaging waits for both CI
paths, and publication downloads only the `oxid-*` platform package artifacts.
