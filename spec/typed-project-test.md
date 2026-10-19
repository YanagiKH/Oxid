# Typed project tests (R1)

Select a project explicitly:

```sh
oxid test --manifest-path path/to/oxid.toml
oxid test --manifest-path path/to/oxid.toml --engine interpreter
oxid test --manifest-path path/to/oxid.toml --engine native
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
manifest, implicit manifest discovery, filtering, parallel execution or
dependency download. `oxid test` without `--manifest-path` retains its legacy
behavior.

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
`.case-<ordinal>.build/`. They reuse managed publication and retain exact open
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
