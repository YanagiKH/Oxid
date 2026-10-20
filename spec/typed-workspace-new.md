# Typed workspace scaffolding

This describes an experimental developer-tool extension, not a v1.0 release
qualification. Bare `oxid new NAME` keeps its legacy template, and
`oxid new --edition typed-preview PATH` keeps its plain typed template and
behavior byte for byte.

## Command

```
oxid new PATH --edition typed-preview --workspace
oxid --edition=typed-preview new --workspace PATH
oxid new --edition typed-preview --workspace -- PATH
```

`--workspace` is a valueless flag of the `new` command. It must follow the
`new` command word, precede any `--`, and may appear once. `--workspace=VALUE`,
a repeated flag, the flag before the command word, a missing or empty path and
an extra operand are rejected. An explicit `--edition typed-preview` is
required. Without it, an unseparated `--workspace` is rejected explicitly; it
is never forwarded to the legacy template as a directory literally named
`--workspace`. After `--`, `--workspace` is a literal operand like any other
word, and every argument after `script NAME` still belongs to that script.

Rejections use the existing typed `new` contract: a JSON request reports
`typed-preview new does not support --message-format=json` on stdout as
before, and Process-mode and other unsupported options report on stderr.
Neither contract is extended. Every rejection happens before filesystem access.

## Generated workspace

The command creates a virtual root and two ordinary typed-preview projects with
fixed names `workspace-app` and `workspace-library`, independent of PATH.
Exactly seven files are written:

```
PATH/oxid.toml          [workspace] members = ["app", "library"]
PATH/README.md
PATH/.gitignore
PATH/app/oxid.toml      workspace-app, entry main.ox, math = "../library"
PATH/app/main.ox
PATH/library/oxid.toml  workspace-library, entry lib.ox
PATH/library/lib.ox
```

The sources keep the semantics of `examples/projects/typed_workspace_selection/`
in formatter-normalized layout: the app calls the library's `pub fn number()`
through its explicit `math` path dependency and prints `7`; it has two tests.
The library has its own `main` printing `91` and one test. After creation,
`oxid fmt --check` passes for both members. Membership still grants no import
visibility, and the generated root has no `[project]` table.

On success the command prints exactly one stdout line,
`created typed workspace: "PATH"`, with PATH JSON-style quoted and terminal
control characters escaped, and nothing on stderr. The seven files are the
fixed template bytes of `src/frontend/workspace_scaffold.rs`; the companion
asserts them byte for byte.

No library project kind, edition, lockfile, cache, artifact directory or
version-control repository is created. Commands on the generated workspace
follow [workspace selection](workspace-selection.md) unchanged, so `check`,
`run` and `test` require `--package` because there are two members.

## Filesystem policy

The policy matches plain typed `new`. The parent of PATH must already exist and
is never created. PATH itself is created with an exclusive directory creation;
an existing file, empty directory, symbolic link or dangling symbolic link is
refused without modification and without following the link. Each file is
opened with create-new semantics.

If a later directory or file creation fails, the files already written are
retained and the error says so. There is no automatic rollback or deletion.
Creation is not atomic as a whole and is not a defense against a concurrent or
adversarial replacement of the directory tree.

## Evidence boundary

Ordinary CLI controls are in `tests/typed_workspace_new.rs`: the exact file
inventory, unchanged plain typed and legacy `new`, flag and error handling
without side effects, no-overwrite cases, interpreter `check`/`run`/`test` for
both members, `fmt --check`, the missing `--package` refusal, and an edit of
`library/lib.ox` that changes the app's result through the explicit dependency. They do not
exercise LLVM, native builds or held acceptance gates, and they do not change
the engine, loader or source model.
