# Typed manifest RUN example (R1a, experimental)

A two-module typed-preview project selected through an explicit manifest.
This is a bounded experimental preview, not a stable language edition or a
complete project workflow.

```text
typed_manifest_run/
  oxid.toml          [project] edition = "typed-preview", entry = "src/main.ox"
  src/main.ox        root module; declares `mod answer;` and defines `main`
  src/answer.ox      child module; `pub fn value() -> i32`
```

## Result output and status

`oxid run --manifest-path` uses the existing bounded reference runner in Result
mode. A root `fn main()` with no parameters returning `i32`, `bool` or `()` is
required. On success the scalar result is printed to stdout followed by a
newline (`7`, `false`, `()`), stderr is empty, and the exit status is 0 for every
valid result, including negative or nonzero i32 values. Manifest, CLI, source,
entry and runtime failures print the existing human diagnostics to stderr and
exit 1; a reported `E0500` invariant failure exits 2.

With `--message-format=json`, stdout holds only diagnostic records followed by
one terminal summary. Global option-validation errors still use `check-summary`;
other legal RUN subsequent errors use `run-summary`. A successful run ends with
`{"schema_version":1,"edition":"typed-preview","kind":"run-summary","success":true,"errors":0,"result":{"type":"i32","value":7}}`.

## Usage

From `examples/projects`:

```sh
oxid check --manifest-path=typed_manifest_run/oxid.toml
# typed-preview check ok (2 functions; check only)

oxid run --manifest-path=typed_manifest_run/oxid.toml
# 7

oxid run --manifest-path typed_manifest_run/oxid.toml --message-format=json
```

The entry is joined to the literal parent of the supplied manifest path, so the
same project can be checked or run from any directory by naming its manifest.

## Scope

- Only `check` and `run` accept `--manifest-path`, and the selector must follow
  the command word. Only `--edition typed-preview` and `--message-format` may
  accompany it; source operands, `--entry-mode`, `--backend`, `--target`,
  `--output` and experimental selectors are refused.
- The manifest uses the same strict decoder as manifest CHECK: one `[project]`
  table with `edition` (must be `typed-preview`), `entry`, and optional opaque
  `name`/`version`. Unknown tables or keys are refused.
- There is no automatic manifest discovery, Process entry mode, native compile,
  build, test, package, dependency resolution, program arguments or budget
  override through a manifest. Existing budgets are unchanged.
- Declared child modules load under the existing typed project rules
  (currently Linux only); no new platform support is implied.
