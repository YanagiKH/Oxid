# Typed manifest TEST example (R1, experimental)

A two-module typed-preview project whose root `test_` functions are run
through an explicit manifest with `oxid test`. This is a bounded experimental
preview, not a stable language edition, package workflow or release artifact.

```text
typed_manifest_test/
  oxid.toml          [project] name = "typed-manifest-test-example",
                     edition = "typed-preview", entry = "src/main.ox"
  src/main.ox        root module; declares `mod answer;`, defines `main`,
                     and declares test_zulu, test_alpha, test_zero in that order
  src/answer.ox      child module; `pub fn value() -> i32`, plus a child
                     `test_child` that is never discovered as a suite case
```

## Requirements

The interpreter needs no native toolchain. The native engine requires
Linux x86_64 with LLVM 19.1.7 (`clang`, `opt`, `ld.lld`). Point
`OXID_LLVM_BIN` at the LLVM bin directory, for example
`OXID_LLVM_BIN=/usr/lib/llvm-19/bin`.

`oxid test` is text-only. `--message-format=json` is refused before
manifest I/O. Omitted `--engine` and `--engine interpreter` are the same.

## Usage

From `examples/projects`:

```sh
oxid test --manifest-path typed_manifest_test/oxid.toml
# PASS test_zulu
# PASS test_alpha
# PASS test_zero
# test result: 3 passed; 0 failed; 3 total

oxid test --manifest-path typed_manifest_test/oxid.toml --engine native
# (same three PASS lines and total; native artifacts are prepared first)

oxid run --manifest-path typed_manifest_test/oxid.toml
# 7            (ordinary main is not a test case and is not run by `test`)

oxid build --manifest-path typed_manifest_test/oxid.toml
# typed-preview native build ok: "typed_manifest_test/.oxid/typed-preview/result/typed-manifest-test-example"
```

Suite cases are the root module's `test_` declarations in source order.
Child-module `test_` functions and import aliases are not cases. A case
passes when it returns `true`, unit, or i32 zero; `false` and every nonzero
i32 (including negative values and multiples of 256) fail. Runtime failures
fail only their own case; later cases still run with fresh state.

## Managed output

The interpreter writes nothing. Native test artifacts and receipts live
under `<manifest-parent>/.oxid/typed-preview/test/case-<ordinal>` and
`.case-<ordinal>.build/`, separate from the application `result`/`process`
outputs and receipts, which a test run never replaces. Every native artifact
is prepared before the first case runs; a preparation failure reports no
case results. There is no interpreter fallback for the native engine.

## Scope and limitations

No automatic manifest discovery, filtering, parallel execution, separate
test manifest, conditional test compilation, dependency download, or
Process-mode test programs in this slice. `oxid test` without
`--manifest-path` keeps its legacy behaviour.
