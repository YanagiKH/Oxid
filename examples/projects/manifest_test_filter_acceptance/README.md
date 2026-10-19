# Manifest TEST filter acceptance (R6, experimental)

Independent acceptance fixture for `tests/manifest_test_filter_acceptance.rs`
(task r1-test-filter-24781574). It freezes the selected cases, their
results and their source order for the explicit-manifest `oxid test
--filter TEXT` / `--filter=TEXT` contract. The filter is a case-sensitive
substring match on root `test_` function names; without `--filter` the
existing output is unchanged.

```text
manifest_test_filter_acceptance/
  oxid.toml          [workspace] members = ["app", "helper"]
  app/               name = "filteraccept-app", entry = "src/main.ox",
                     [dependencies] helper = "../helper"
    src/main.ox      main + root suite, in this order:
                     test_keep_z (bool true), test_drop_false (bool false),
                     test_keep_a (i32 0), test_drop_error (division by zero),
                     test_keep_unit (unit)
    src/child.ox     child `test_keep_child`: never a suite case
  helper/            name = "filteraccept-helper", entry = "src/lib.ox";
                     seven() = 7, nothing() = 0, its own main/test_keep_helper
```

## Usage

From this directory (the interpreter needs no toolchain):

```sh
oxid test --manifest-path app/oxid.toml
# PASS test_keep_z
# FAIL test_drop_false
# PASS test_keep_a
# FAIL test_drop_error
# PASS test_keep_unit
# test result: 3 passed; 2 failed; 5 total

oxid test --manifest-path app/oxid.toml --filter keep
# PASS test_keep_z
# PASS test_keep_a
# PASS test_keep_unit
# test result: 3 passed; 0 failed; 3 total

oxid test --manifest-path app/oxid.toml --filter=test_drop_false
# FAIL test_drop_false
# test result: 0 passed; 1 failed; 1 total

oxid test --manifest-path oxid.toml --package filteraccept-app --filter keep
oxid test --manifest-path oxid.toml --package=filteraccept-helper --filter=keep
# PASS test_keep_helper
# test result: 1 passed; 0 failed; 1 total
```

The summary counts only the executed (selected) cases. A filter that
matches no root `test_` name fails after the full check with
`error[E0800] (project-test): no tests match --filter`, without running any
case or printing a summary. Empty and missing values (`--filter requires a
value`), duplicates (`--filter may only be specified once`) and `--filter`
on any other operation or on non-manifest `oxid test` (`--filter is only
supported by explicit manifest test`) are refused on the existing CLI path
before the project is read; a following option is never taken as the
value. The whole project, including the path dependency, is still
type-checked and every test signature is still validated before the
selected cases run. `oxid test` remains text-only.

With LLVM 19.1.7 (Linux x86_64, `OXID_LLVM_BIN=/usr/lib/llvm-19/bin`),
`--engine native` reports the same cases. Every selected case is prepared
before the first one runs; its artifact keeps the original suite ordinal
(`app/.oxid/typed-preview/test/case-<ordinal>`), and application
`result`/`process` outputs and receipts are never replaced.

## Focused filter validation status

The independent companion recorded twelve ordinary and five native reference
tests passing on the formatted implementation before its final test-only
oracle corrections. Its checks cover root discovery and source order, workspace
selection, path dependencies, whole-project/signature preflight, selected native
artifact ordinals, preparation-before-execution and application-output isolation.
The implementation's formatting delta and final source identities were reviewed
independently. Those reference logs do not contain a pre-run final-source hash
and compiler-binary binding.

The executor subsequently reported successful compile gates, three CLI filter
unit tests, the help test, the exact native ordinal/barrier control, scoped
Clippy, and the two tightened CLI-precedence controls. The broader gate and
precedence raw archives were unavailable to the final review; these remain
executor-reported results. The original ordinary core run had nine passes and
one test-oracle failure: PACKAGE's existing failure-summary stdout had been
incorrectly expected to be empty. Its corrected exact test, core-only rustfmt
and scoped Clippy are independently supported by the final raw logs. The
correction preserves the PACKAGE summary, status, diagnostic and no-output-file
checks; production behavior was unchanged.

The final source also fixes a consuming `Failure::message` call in a CLI unit
test. No production changes followed the companion reference runs. Existing
formatting differences elsewhere in main.rs were preserved. No full-repository
format, full-suite, exact-head hosted CI, source qualification or resource-policy
qualification is claimed by these focused results.
