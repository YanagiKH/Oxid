# Typed project tests (R1)

Select a project explicitly:

```sh
oxid test --manifest-path path/to/oxid.toml
oxid test --manifest-path path/to/oxid.toml --engine interpreter
oxid test --manifest-path path/to/oxid.toml --engine native
oxid test --manifest-path path/to/oxid.toml --filter parser
```

The interpreter is the default. The manifest requires
`[project].edition = "typed-preview"` and a relative `.ox` `project.entry`.
Optional name/version metadata is accepted; neither engine requires a name.
The same checked project and original root module serve both engines.

Tests are root function **declarations** whose source names start with `test_`,
in source order. Imported aliases and child-module functions are not discovered.
Child modules can supply ordinary production helpers used by tests. Each test
must have no parameters and return `bool`, `i32`, or `()`: true, zero, and unit
pass; false and every nonzero signed i32 fail, including negative values and
multiples of 256. A missing suite or any unsupported test signature is an error.
The complete project is checked, lowered and verified before the first case.
An optional ordinary `main` retains its identity and is not automatically run.

Tests remain ordinary functions under check/build/run, including their bodies
and ownership rules. There is no conditional test compilation, separate test
manifest, implicit manifest discovery, parallel execution or
dependency download. `oxid test` without `--manifest-path` retains its legacy
behavior.

`--filter TEXT` and `--filter=TEXT` select root test names containing TEXT as a
case-sensitive substring. They are supported only by explicit manifest `test`.
Missing, empty or duplicate filters are rejected before project I/O; a following
option is never consumed as the value. The entire selected project, including
path dependencies, is still checked and every discovered test signature is
validated before selection. A filter matching nothing fails with a diagnostic,
without running cases or printing a success summary. Matching tests retain source
order; the summary counts only executed cases. Without a filter, behavior and
output are unchanged. Workspace `--package` selection is unchanged.

The CLI retains an index into the unchanged argument vector and borrows the
filter at dispatch; it does not copy the filter or add a filter length limit.
Selection scans the already bounded root suite, without copying names or storing
an ordinal vector. Native handle storage is fallibly reserved for exactly the
selected case count, never more than the original suite. This is host-side
selection work, not a new runtime/source qualification or a raised resource limit.

Each case uses fresh existing runtime limits. Runtime diagnostics and logical
failures fail the case; subsequent cases still run. The Result-mode restriction
on `write_stdout` imports remains. Importing the actual `std::io::read_stdin`
function is also rejected during suite preflight, even if unused or aliased or
imported only in a child module. `ReadStatus` alone and user functions named
`read_stdin` are permitted. Testing Process-mode user programs is later scope.

Native tests require Linux x86_64 and LLVM 19.1.7, using the existing
`OXID_LLVM_BIN` selection. Emission or toolchain refusal is a failure with no
interpreter fallback. Every selected artifact is prepared before any case runs.
A dedicated wrapper maps each checked result directly to status 0 or 1; runtime
failures retain their stderr and also fail the case. Stdout never determines
truth. i32 is compared with zero before conversion to an exit status.

Native artifacts and their ownership receipts use
`<manifest-parent>/.oxid/typed-preview/test/case-<zero-based-ordinal>` and
`.case-<ordinal>.build/`. The ordinal is the original discovered-suite ordinal,
even when earlier cases are filtered out. Execution maps each selected case to
its separately indexed prepared handle. They reuse managed publication and retain exact open
executable handles. Application `result`/`process` outputs and receipts are
separate. Successfully prepared test artifacts can remain after a later
preparation failure, but no test ran in that invocation. Managed staging and
concurrent-build refusal follow ordinary build behavior.

Output is text only, for example:

```text
PASS test_helper
FAIL test_false
test result: 1 passed; 1 failed; 2 total
```

Exit zero requires at least one executed test and all tests passing. CLI,
manifest, project and native preparation errors abort without case results or a
success total. JSON requests fail before I/O with one existing schema-version-1
diagnostic record; no check/run/test success summary is fabricated.

Implementation controls: `cargo test --test project_test_core` and
`cargo test --bin oxid typed_project_test`. The separately provisioned native
comparison is `cargo test --bin oxid typed_project_test_native_agrees_with_interpreter -- --ignored`.
The native CLI preparation barrier is covered by
`cargo test --test project_test_core -- --ignored` with that same toolchain.
These functional checks do not establish source qualification or integration.

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
