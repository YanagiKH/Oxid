# Oxid commands

Run `oxid --version` to see your installed version. Use `oxid help` for the
complete command list.

## Script projects

The published 0.9 binaries use the interpreter. Start with:

```sh
oxid new hello
cd hello
oxid run src/main.ox
```

| Command | What it does |
|---|---|
| `oxid run FILE` | Run source or an `.oxb` package. |
| `oxid check FILE` | Check syntax without running the script. |
| `oxid test` | Run project tests and examples. |
| `oxid build` | Package the project as `.oxid/bin/NAME.oxb`. |
| `oxid compile FILE -o app.oxb` | Package one entry point and its imports. |
| `oxid inspect app.oxb` | Show package versions, module counts and checksum. |
| `oxid fmt [PATH]` | Format script files in place. |
| `oxid lint [PATH]` | Run syntax-based checks. |
| `oxid repl` | Open the interactive interpreter. |
| `oxid watch FILE` | Rerun when source files change. |

An `.oxb` package runs with a compatible Oxid runtime. Script `check` and `lint`
check syntax; they do not enforce static types or ownership.

## Native projects

Development builds also support typed native projects. They require Linux
x86_64; native builds need LLVM, Clang and LLD 19.1.7 and system C development
tools. This workflow is not included in the published 0.9 binaries.

```sh
oxid new hello --edition typed-preview
cd hello
oxid check
oxid run
oxid test
oxid build
./.oxid/bin/hello
```

| Command | What it does |
|---|---|
| `oxid check` | Check types, ownership, modules and local dependencies. |
| `oxid run` | Run the project's entry point. |
| `oxid test` | Run tests declared in `oxid.toml`. |
| `oxid build` | Build `.oxid/bin/NAME` as a native executable. |
| `oxid build --incremental` | Reuse a matching native object when available. |
| `oxid build --reproducible` | Use stable relative paths in native runtime errors. |
| `oxid fmt FILE` | Print formatted source without modifying the file. |
| `oxid fmt --check FILE` | Report whether formatting changes are needed. |
| `oxid doctor --native` | Check the native tools, SDK and project inputs. |

The project's manifest selects typed mode for later commands. `build` preserves
the previous executable if compilation fails. The executable runs without the
Oxid compiler or source files. See [native projects](TYPED_PROJECTS.md) for settings
and supported syntax.

With the same compiler, tools, SDK, settings and environment, `--reproducible`
preserves executable bytes when the complete relative source layout is moved.
It overrides the manifest's boolean `reproducible` setting;
`--no-reproducible` explicitly disables it.

## Dependencies and workspaces

`oxid lock` records dependency inputs. Normal project commands verify an existing
lock. Add `--locked` to require one, `--offline` to avoid fetching, or `--frozen`
for both. Use `oxid lock --update` after intentional changes to typed dependency
inputs.

Script projects support [local and pinned Git dependencies](PACKAGES.md). Typed
projects currently support local path dependencies. Add a path under
`[dependencies]` in `oxid.toml`.

Typed workspaces share one lockfile. `--workspace` selects all members;
`--package NAME` or `-p PATH` selects one:

```sh
oxid check --workspace
oxid run --package client
oxid test --workspace
oxid build --workspace --frozen
```

## Share native-project sources

```sh
oxid lock
oxid package --frozen --output application.oxpkg.tar
oxid package --verify application.oxpkg.tar
oxid package --extract application.oxpkg.tar --output restored
```

Source packages include local dependencies, tests and a matching lock. Extraction
requires a new destination directory. Add `--sha256 HEX` to verify or extract to
compare a separately supplied archive hash.

## More tools

| Command | Guide |
|---|---|
| `oxid script NAME [ARGS...]` | [Manifest scripts](SCRIPTS.md) |
| `oxid add NAME PATH`, `oxid remove NAME` | [Script dependencies](PACKAGES.md) |
| `oxid fetch`, `oxid update`, `oxid list` | [Package workflow](PACKAGE_WORKFLOW.md) |
| `oxid bridge LANGUAGE` | [Calling other languages](INTEROP.md) |
| `oxid web new NAME`, `oxid discord new NAME` | [HTTP and Discord helpers](WEB_AND_BOTS.md) |
| `oxid doctor`, `oxid doc`, `oxid bench` | [Project tools](TOOLS.md) |

For a short introduction, follow the [quickstart](QUICKSTART.md).
