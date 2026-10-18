# Typed manifest native RUN example (R1c, experimental)

A two-module typed-preview project built and launched through an explicit
manifest with `--engine native`. This is a bounded experimental preview, not a
stable language edition, package workflow or release artifact.

```text
typed_manifest_native_run/
  oxid.toml          [project] name = "typed-manifest-native-run-example",
                     edition = "typed-preview", entry = "src/main.ox"
  src/main.ox        root module; declares `mod answer;` and defines `main`
  src/answer.ox      child module; `pub fn value() -> i32`
```

## Requirements

Linux x86_64 with LLVM 19.1.7 (`clang`, `opt`, `ld.lld`). Point
`OXID_LLVM_BIN` at the LLVM bin directory, for example
`OXID_LLVM_BIN=/usr/lib/llvm-19/bin`. Native RUN requires `[project].name`
matching `[A-Za-z0-9][A-Za-z0-9_-]{0,63}` (same rule as managed BUILD).

Native RUN is text-only. `--message-format=json` is refused before
manifest/source I/O. Omitted `--engine` and `--engine interpreter` keep the
existing interpreter Result RUN (including JSON).

## Usage

From `examples/projects`:

```sh
oxid run --manifest-path typed_manifest_native_run/oxid.toml --engine native
# 7            (Result default: prints the scalar, exit status 0; no build summary)

oxid run --manifest-path typed_manifest_native_run/oxid.toml --engine native --entry-mode process
# (no stdout; Process: exit status 7)

oxid run --manifest-path typed_manifest_native_run/oxid.toml
# 7            (interpreter Result; no native build)

oxid run --manifest-path typed_manifest_native_run/oxid.toml --engine interpreter --message-format=json
# {"schema_version":1,...,"kind":"run-summary","success":true,...,"result":{"type":"i32","value":7}}
```

Successful native RUN emits only the application's stdout/stderr (inherited
stdin and cwd). Managed output still lands under
`<manifest lexical parent>.oxid/typed-preview/<result|process>/<name>`, but
RUN does not print a build-success line. A failed build never launches a
previous executable.

## Managed output and ownership

Native RUN shares the managed BUILD publish path. Ownership, lock, stale
stage, and unowned-path refusals match R1b (`E0710` / `project-build`). The
launched executable is pinned by an open fd before publication; a later
rebuild may replace the managed pathname without changing the selected
inode for an already-running launch.

## Scope and limitations

No automatic manifest discovery, incremental cache, dependency resolution,
program arguments, or crash-durability guarantee. Native RUN always rebuilds
before launch. `--output`, `--backend`, `--target`, source operands and
experimental selectors are refused. Host support is Linux x86_64 only.
