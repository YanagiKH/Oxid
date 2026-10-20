# Sequential workspace CHECK and TEST

A virtual workspace with three ordinary members, declared in this order:

- `core` (`batch-core`): a library-like project with one root test and a
  child module `scale`. The child module's `test_` function is never discovered.
- `app` (`batch-app`) and `tools` (`batch-tools`): both depend on `core`
  through the explicit `shared` path dependency. Membership alone grants no
  import visibility, and dependency tests are not discovered for dependents.

From this directory:

```sh
oxid check --manifest-path oxid.toml --workspace
oxid test --manifest-path oxid.toml --workspace
oxid test --manifest-path oxid.toml --workspace --fail-fast
```

Before each member, stdout shows `workspace package: NAME`, followed by that
member's ordinary CHECK or interpreter TEST output. TEST prints:

```text
workspace package: batch-core
PASS test_base
test result: 1 passed; 0 failed; 1 total
workspace package: batch-app
PASS test_double
PASS test_unit
test result: 2 passed; 0 failed; 2 total
workspace package: batch-tools
PASS test_offset
test result: 1 passed; 0 failed; 1 total
workspace result: 3 passed; 0 failed; 0 unrun; 3 total packages
```

The final line counts packages, not test cases. By default a failed member does
not stop later members; `--fail-fast` stops after the first failed member and
also stops that member's remaining test cases. All member manifests are
validated before the first member starts, but each member's sources and test
suite are checked only when that member runs. `core` is checked again for each
dependent; there is no shared workspace-wide budget, snapshot or parallelism.

Use an ordinary `--package NAME` command for native execution, JSON, profiles
or `--filter`. This example demonstrates explicit sequential batches only.
