# Experimental typed projects

The development toolchain can select typed semantics from `oxid.toml`.
This workflow currently supports a single package on Linux x86_64 with LLVM
19.1.7. It is not part of the published v0.9.0 binaries or a stable v1 edition.

```sh
oxid new hello --edition typed-preview
cd hello
oxid check
oxid run
oxid test
oxid build
./.oxid/bin/hello
```

The scaffold's process entry returns zero on success. Build produces a real
native executable that runs without the source tree or the Oxid compiler.
Install LLVM/Clang/LLD 19.1.7 for building, as described in the
[native preview](../spec/native-preview.md).

```toml
[project]
name = "hello"
version = "0.1.0"
edition = "typed-preview"
entry = "src/main.ox"

[build]
backend = "llvm"
target = "x86_64-unknown-linux-gnu"
profile = "bounded"
entry-mode = "process"

[tests]
smoke = "tests/smoke.ox"
```

`check`, `run`, `build`, and `test` find the nearest project above the working
directory. File commands such as `oxid check src/main.ox` inherit the nearest
manifest above the source. An explicit `--edition` overrides inheritance;
explicit typed file operations retain their existing defaults and flags.
Projects without an edition keep legacy behavior.

Tests are the entry files listed in `[tests]`, sorted by test name. Each runs
as a separate typed process with closed stdin. A test must have a zero-argument
`main() -> i32`: zero passes, nonzero fails. Helpers are loaded through module
declarations, not discovered as additional tests. `test --message-format=json`
reports structured test results without interpreting the program's output as
test results.

`build` always compiles the declared module graph. Rebuilding replaces its
managed executable only after successful compilation and linking. Source or
toolchain errors preserve the previous executable. `compile --output` still
refuses to overwrite an existing output. Avoid simultaneous source edits during
a build; filesystem access uses the compiler's existing stable-tree contract.

`oxid fmt src/main.ox` prints typed formatting; `oxid fmt --check src/main.ox`
checks it. Directory formatting is not supported. Unsupported project commands
and build settings report an error instead of selecting legacy semantics.

Current limits include bounded execution, call-only borrows, the existing
typed language subset, one native target, and no typed dependency packages,
workspaces, registry, or incremental build cache. See
[RFC 0032](../rfcs/0032-typed-project-workflow.md) for the contract and
[ROADMAP_V2](ROADMAP_V2.md) for the remaining R1 and v1.0 requirements.
