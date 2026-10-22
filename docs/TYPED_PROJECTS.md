# Native projects

Use Oxid's typed mode to check code, test local packages and build native
executables. This workflow is available in development builds on Linux x86_64.
Native builds require LLVM, Clang and LLD 19.1.7.

## Create and run

```sh
oxid new hello --edition typed-preview
cd hello
oxid check
oxid run
oxid test
oxid build
./.oxid/bin/hello
```

The executable runs without the Oxid compiler or project sources. The generated
`oxid.toml` selects typed mode for later commands, so you only specify the edition
when creating the project.

## Everyday commands

| Command | Result |
|---|---|
| `oxid check` | Check the project without running it. |
| `oxid run` | Run its entry point. |
| `oxid test` | Run tests declared in `oxid.toml`. |
| `oxid build` | Build an executable under `.oxid/bin/` using project settings. |
| `oxid build --profile release` | Build with native optimization enabled. |
| `oxid fmt src/main.ox` | Print formatted source; add `--check` to check formatting. |
| `oxid lock` | Record local dependency inputs in `oxid.lock`. |
| `oxid doctor --native` | Check the native tools, SDK and project configuration. |

The commands find `oxid.toml` above the current directory. Build failures preserve
the previous executable. `fmt` takes one file and writes its result to stdout.

## Project settings

```toml
[project]
name = "hello"
edition = "typed-preview"
entry = "src/main.ox"

[build]
backend = "llvm"
profile = "dev"
entry-mode = "process"
reproducible = true

[tests]
smoke = "tests/smoke.ox"
```

A process entry is `fn main() -> i32`; return zero for success.
`reproducible = true` uses project-relative paths in native error messages.

## Build profiles

```sh
oxid build --profile dev
oxid build --profile release
```

`dev` builds without optimization (`-O0`); `release` enables optimization
(`-O2`) for both the program and its runtime. Both preserve checked arithmetic,
bounds checks and the current execution limits. `bounded` is an alias for `dev`.
These profiles do not add source-level debugger information.

The command-line profile overrides `[build] profile` in the selected package.
Omitting both uses `dev`. A workspace uses each selected member's setting;
`--profile` overrides every selected member. Dependency settings do not change
the importing application's profile. All profiles write the same output path,
so copy an executable elsewhere if you need to keep both builds.

Profiles also work with `--incremental` and `--reproducible`. Switching between
`dev` and `release` selects a different cached program object. Switching between
`dev` and `bounded` can reuse the same object.
[Build profile details](../rfcs/0040-native-build-profiles.md).

## Local dependencies

```toml
[dependencies]
math = "../math"
```

The library needs its own typed project manifest and a `src/lib.ox` entry.
Import public items with `use crate::math::answer;`. Each package keeps its own
module root and visibility. Git and registry dependencies are not yet available
in typed projects.

After changing dependency manifests or source code intentionally, run
`oxid lock --update`. Normal commands verify an existing lock. `--locked`
requires one; `--frozen` also requires offline operation. Application and test
source bodies remain editable.

## Workspaces

Put this in a shared root's `oxid.toml`:

```toml
[workspace]
edition = "typed-preview"
members = ["apps/client", "apps/server", "libs/protocol"]
default-members = ["apps/client"]
```

```sh
oxid check --workspace
oxid run --package client
oxid test --workspace
oxid lock
oxid build --workspace --frozen
```

At the root, commands select the default members. Inside a member, commands
select that member. `--workspace` selects every member; `--package` or `-p`
selects a member by name or relative path. The shared lock covers all members.
Libraries are checked during workspace builds; each binary gets its own output.

## Repeatable builds

```sh
oxid build --frozen --reproducible --incremental
```

`--reproducible` uses paths relative to the project or workspace root in native
runtime errors. With the same compiler, LLVM/SDK, settings and environment,
moving an unchanged relative source layout preserves executable bytes.
`--no-reproducible` overrides the manifest setting.

`--incremental` reuses a matching native program object. Sources and locks are
still checked, and the runtime and executable are rebuilt. The cache reports
`hit`, `miss` or `unavailable`. [Build details](../rfcs/0039-reproducible-native-builds.md).

## Share source packages

```sh
oxid lock
oxid package --frozen --output application.oxpkg.tar
oxid package --verify application.oxpkg.tar
oxid package --extract application.oxpkg.tar --output restored
```

A source package includes the project, local dependencies, declared tests and
matching lock. Extraction requires a new directory. Enter the project root
printed by extraction, then run `oxid build --frozen` to rebuild offline.
Use `--sha256 HEX` with verify or extract to compare a separately supplied hash.
[Package details](../rfcs/0038-typed-source-packages.md).

For complete applications, try [Byte count](../apps/byte-count/README.md) or
[Frame filter](../apps/frame-filter/README.md), which passes owned records,
arrays and nested enum values between local packages.
See the [language reference](../spec/typed-preview.md) for supported syntax.
