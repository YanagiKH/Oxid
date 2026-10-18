# Typed manifest BUILD example (R1b, experimental)

A two-module typed-preview project built into a managed native executable
through an explicit manifest. This is a bounded experimental preview, not a
stable language edition, package workflow or release artifact.

```text
typed_manifest_build/
  oxid.toml          [project] name = "typed-manifest-build-example",
                     edition = "typed-preview", entry = "src/main.ox"
  src/main.ox        root module; declares `mod answer;` and defines `main`
  src/answer.ox      child module; `pub fn value() -> i32`
```

## Requirements

Linux x86_64 with LLVM 19.1.7 (`clang`, `opt`, `ld.lld`). Point
`OXID_LLVM_BIN` at the LLVM bin directory, for example
`OXID_LLVM_BIN=/usr/lib/llvm-19/bin`. Build requires `[project].name`
matching `[A-Za-z0-9][A-Za-z0-9_-]{0,63}`; CHECK and RUN still accept a
manifest without a name.

## Usage

From `examples/projects`:

```sh
oxid build --manifest-path typed_manifest_build/oxid.toml
# typed-preview native build ok: "typed_manifest_build/.oxid/typed-preview/result/typed-manifest-build-example"

typed_manifest_build/.oxid/typed-preview/result/typed-manifest-build-example
# 7            (Result entry mode: prints the scalar result, exit status 0)

oxid build --manifest-path typed_manifest_build/oxid.toml --entry-mode process
typed_manifest_build/.oxid/typed-preview/process/typed-manifest-build-example
# (no output; Process entry mode: exit status 7)

oxid build --manifest-path typed_manifest_build/oxid.toml --message-format=json
# {"schema_version":1,"edition":"typed-preview","kind":"build-summary","success":true,"errors":0,"output":"typed_manifest_build/.oxid/typed-preview/result/typed-manifest-build-example","entry_mode":"result"}
```

The output is `<manifest's lexical parent>.oxid/typed-preview/<result|process>/<name>`,
so the reported path is relative to the directory the command ran in. Build
only compiles: it never runs the program. The executable produced is the same
as `oxid compile <entry> --edition=typed-preview --backend=llvm --output=...`
for the same entry and entry mode.

## Managed output

- Rebuilding after editing any module replaces the managed executable with a
  single rename. A failed build (type error, native admission, toolchain,
  ownership, lock or staging failure) leaves the previous executable in place.
- Build only replaces an executable it published itself (tracked in the
  private `.<name>.build` directory beside it). A pre-existing file, a file
  written there by `oxid compile --output`, a symlink, a directory, or a
  symlinked `.oxid`, `typed-preview` or `result`/`process` component is
  refused with `E0710` (`project-build`) and left untouched.
- A second concurrent build of the same output is refused (`E0710`).
- A leftover `.<name>.build/stage` directory from an interrupted build is
  refused (`E0710`). After confirming no build is running, remove only that
  `stage` directory and build again. Do not delete `.<name>.build` while an
  executable it published is kept.
- `oxid compile --output` keeps its rule of never overwriting an existing file
  or symlink, including the managed executable.

## Reporting

Text mode prints the success line above on stdout and diagnostics on stderr.
JSON mode prints diagnostic records followed by exactly one `build-summary`
(`"success":false,"output":null` on failure). Exit status is 0 on success, 1
for reported errors, and 2 for an `E0500` invariant failure. If writing the
report fails after the executable was published, build exits 1 and stderr
says the executable was already published and the previous one was not
restored.

## Scope and limitations

No automatic manifest discovery, incremental cache, dependency resolution,
program arguments, crash-durability guarantee, automatic recovery of an
interrupted build, or protection against other processes tampering with the
managed directories. Build always recompiles. The `.<name>.build` directory
may keep one previous executable. `--output`, `--backend`, `--target`, source
operands and experimental selectors are refused with build.
