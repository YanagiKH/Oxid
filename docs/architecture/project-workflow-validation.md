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
the five CLI, manifest and build orchestration files plus seven explicitly
named package loader/declaration/OIR source interfaces. The three additions are
`src/project_cli.rs`, `src/frontend/project/packages.rs`, and its compiler tests.
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
that they do not qualify new package semantics. Parser, ownership verification,
OIR verification, native lowering and runtime inputs remain unchanged. Future
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

`scripts/verify_current_native.py` runs the existing independent native oracles,
private LLVM regression groups, checked-HIR import, and expression/stack/stdin
applications against debug and release binaries built from a clean current
checkout. It records the actual compiler and test binary hashes, selected Rust
tests, commands, stdout/stderr and retained native artifacts, and rejects a
changed checkout or executable. These results supplement the public project
tests; they do not reissue the private historical certificates.

## Historical certificates

`.github/workflows/historical-qualification.yml` retains the complete previous
workflow, its jobs, assertions, host matrices and artifacts. Every checkout and
expected-head input uses the fixed predecessor SHA above. The caller's event SHA
remains available as event metadata. Its source-pinned tests still read the
original workflow and source tree, and their expected hashes are unchanged.

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
