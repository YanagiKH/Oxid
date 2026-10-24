# Typed project workflow validation

The manifest workflow in [RFC 0032](../../rfcs/0032-typed-project-workflow.md)
adds project orchestration around the existing typed compiler; [RFC 0033](../../rfcs/0033-typed-path-dependencies.md)
extends it to local dependency sessions. It does not complete R1 or certify any
v1 release gate. The development workflow and
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
the five CLI, manifest and build orchestration files, seven explicitly
named package loader/declaration/OIR source interfaces and four native
presentation interfaces (one overlaps the package interface set). The twelve additions are
`src/project_cli.rs`, `src/project_cli/workspace.rs`, `src/project_cli/package.rs`,
`src/project_package.rs`, `src/project_package/archive.rs`, `src/project_lock.rs`,
`src/doctor_cli.rs`,
`src/frontend/native_cache.rs`, `src/frontend/native/doctor.rs`,
`src/frontend/project/packages.rs`, its compiler tests and
`src/frontend/build_config.rs`.
All other predecessor inputs remain byte-identical. Compile-time
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
These unchanged assertions cover predecessor single-package behavior and
resource counts on the changed compiler. The source manifest explicitly records
that they do not qualify new package/workspace semantics or native reproducibility.
Native presentation changes are listed separately in the source manifest; scalar
and owned emission receive independently tested presentation maps. Parser,
ownership verification, OIR verification and runtime inputs remain unchanged. Future
changes require another explicit scope revision; a wider source allowance alone
cannot qualify new semantics.

`tests/typed_path_dependencies.rs` and the package loader's compiler tests cover
the new graph, namespace, visibility, identity, ownership and admission behavior.
`scripts/verify_typed_package_app.py` separately checks the real three-package
byte-count application against an independent byte/newline oracle. With
`--native`, it also edits a dependency, rebuilds, rejects broken source while
preserving the previous executable, deletes the copied sources and executes a
deployed ELF with an empty PATH. Commands, input/output bytes, compiler and
artifact hashes are retained under the selected output directory:

```sh
python3 -B scripts/verify_typed_package_app.py target/debug/oxid target/release/oxid \
  --output /tmp/oxid-package-application --native
```

The command uses Linux x86_64 and LLVM 19.1.7. CI runs both Rust compiler profiles;
this does not establish an optimized Oxid release profile or an incremental cache.

`tests/typed_package_lock.rs` covers local locking through public project
commands. The prepared-session compiler tests verify that dependency checksums
and execution use the same retained bytes after an on-disk source edit.
`scripts/verify_typed_package_lock.py` checks the real application with Python's
TOML reader and independently calculated SHA-256 identities/source checksums.
It exercises locked tests, relocation, editable application sources, rejection
of changed dependencies and malformed locks, explicit update, and loaded source
modules under `target`/`.oxid`. Its optional native mode also checks that lock
rejection preserves the old executable and an accepted change reaches the next
native build:

```sh
python3 -B scripts/verify_typed_package_lock.py target/debug/oxid target/release/oxid \
  --output /tmp/oxid-package-lock --native
```

CI retains this evidence separately from the unlocked application workflow.
These results qualify local dependency input locking, not reproducible machine
code, registry resolution or general supply-chain acceptance.

`tests/native_incremental.rs` separately exercises the opt-in native object
cache described in [RFC 0036](../../rfcs/0036-native-object-reuse.md). Its real
LLVM tests use process traces to distinguish IR code generation from LLVM
verification, runtime compilation and linking. The latter three run on every
build. Cache unit controls cover bounded immutable blobs and failure cases.
This evidence concerns object reuse; the retained Unit2 assertions do not
qualify new cache semantics or per-module incremental frontend compilation.

`tests/reproducible_native.rs` and
`scripts/verify_reproducible_native.py` separately exercise the opt-in relative
native diagnostic paths in [RFC 0039](../../rfcs/0039-reproducible-native-builds.md).
The native controller compares raw executables across clean builds, cold/warm
caches and relocated standalone/workspace layouts, and checks byte-count output
using independent expected values. It records the toolchain, explicit environment
overrides, commands and artifact hashes for each compiler profile. Public tests
also check default diagnostic paths, mapped failure paths, strict selection,
workspace preflight and output preservation. Compiler tests cover the retained
source-map witness and diagnostic sizing. These results have their own scope;
the unchanged Unit2 predicates do not establish native reproducibility.

`tests/native_doctor.rs` checks explicit native diagnosis, bounded probe failures,
process cleanup, project/lock admission and real SDK compilation, linking and
execution. Doctor probes never run a project program or modify its outputs.
The command and JSON result contract are documented in
[Native environment diagnosis](../NATIVE_DOCTOR.md). These checks cover the
installed preview toolchain; they do not extend the native target matrix.

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

## Workspace qualification

[ RFC 0035 ](../../rfcs/0035-typed-workspaces.md) adds bounded member selection,
real library checking and a shared v3 local-input lock. The source roster admits
only the ordinary nested module at src/project_cli/workspace.rs; the prohibition
on new include/path directives is unchanged. Its addition does not broaden any
retained single-package assertion into workspace acceptance.

The public typed_workspace suite covers virtual/package roots, defaults and
member selectors, full-union locks, relocation, library source pins, process
tests, preflight preservation, special files and aggregate source bounds.
A sparse 128 MiB ancestor manifest under a 64 MiB address-space limit verifies
that the 1 MiB regular-file admission occurs before content allocation.

Its two opt-in LLVM cases verify retained source snapshots during tool-driven
file changes, same-name binary isolation and source-free deployment, plus
per-member cache miss/hit, selective rebuild, whole-workspace native doctor and
stale-lock preservation of cached objects and executables. CI executes these
cases with debug and release compiler builds. Library checks produce no native
artifact or cache, and doctor reports parsing/lock admission without claiming
that it typechecked or executed member programs.

## Real workspace application

`scripts/verify_typed_workspace_app.py` copies the byte-count application outside
the checkout and makes its existing three packages a workspace with explicit
library members. It independently calculates the v3 member roster, canonical
package IDs, raw manifest hashes, loaded source checksums and shared graph edges;
seven deliberate lock mutations must be rejected by the Python oracle.

Both compiler profiles check and test the real application, verify that member
selection cannot hide invalid or stale unselected inputs, and perform explicit
lock updates. Native mode also checks library-versus-binary output records,
object-cache miss/hit/invalidation, output preservation, and exact byte/newline
results from a deployed ELF after deleting the copied sources with an empty PATH.
The unchanged repository application retains its separate v2 lock coverage.

```sh
python3 -B scripts/verify_typed_workspace_app.py target/debug/oxid target/release/oxid \
  --output /tmp/oxid-workspace-application --native
```

CI runs this verifier with and without native mode, retaining copied compiler
binaries, commands, input/output bytes, source snapshots, locks and deployed
executables. This evidence covers the workspace application workflow; it does
not establish general native reproducibility or complete a v1 release gate.

## Source-package qualification

[RFC 0038](../../rfcs/0038-typed-source-packages.md) adds explicit retained input
collection and a canonical bounded ustar format. The new source files are
admitted by exact path; existing compile-time input and historical assertion
boundaries remain unchanged. Single-package Unit2 cases provide compatibility
evidence and do not qualify source-package format or publication semantics.

`tests/typed_package.rs` covers command admission, source/test closures, shared
workspace locks, stale inputs, relocation, special files and atomic preservation.
The archive module separately tests bounded inventory/header parsing, exact
member/EOF checks, path/parent conflicts and short I/O.

`scripts/verify_typed_package_bundle.py` uses Python tarfile, TOML and SHA-256
implementations to inspect the full archive and compare deterministic bytes. It
packages the real byte-count application as standalone and workspace layouts,
deletes their originals, extracts elsewhere and checks frozen offline rebuilding
with independent byte-count cases. Both compiler profiles run in CI, and native
evidence is retained separately from interpreter evidence. Source-bundle
determinism does not establish native-code reproducibility or registry trust.
