# Native projects

Use Oxid's typed mode to check code, test local packages and build native
executables. This workflow is available in development builds on Linux x86_64
and ARM64. Native builds require LLVM, Clang and LLD 19.1.7 for the build host;
Oxid selects that host's native target automatically.

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
bounds checks and the selected execution policy. `bounded` is an alias for `dev`.
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
module root and visibility.

On Linux, dependencies can also use a Git URL pinned to a complete commit hash:

```toml
[dependencies]
codec = "git+https://github.com/example/codec.git#0123456789012345678901234567890123456789"
```

Replace the example URL and hash with a real library, then run `oxid lock` to
fetch it and record its inputs. Later `--locked`, `--offline` and `--frozen`
commands use cached checkouts without fetching. Different commits can coexist
under different aliases. Git checkouts with local changes are preserved and
refused. Workspaces share one cache under the workspace root. Source bundles
include cached Git libraries as local dependencies; registry dependencies are
not yet available. See [Git dependencies](../rfcs/0059-typed-git-dependencies.md).

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
Cached Git libraries are included as local sources, so the restored project
does not require Git or the original cache. Creation never fetches dependencies.
Use `--sha256 HEX` with verify or extract to compare a separately supplied hash.
[Package details](../rfcs/0038-typed-source-packages.md).

For complete applications, try [Byte count](../apps/byte-count/README.md) or
[Frame filter](../apps/frame-filter/README.md), which passes owned records,
arrays and nested enum values between local packages.
Try [UTF-8 normalize](../apps/utf8-normalize/README.md) to validate and transform
text using a shared [owned UTF-8 library](../packages/utf8_text/README.md).
See the [language reference](../spec/typed-preview.md) for supported syntax.

## Range loops

Use `for` to visit each integer before the end of a range:

```oxid
fn main() -> i32 {
    let values = [10, 20, 12];
    let mut total = 0;
    for index in 0..values.len() {
        total = total + values[index];
    }
    return total;
}
```

Both bounds must have the same integer type. Oxid evaluates them once, from
left to right. The index is immutable and available inside the loop body.
Empty and reversed ranges run no iterations. `break`, `continue` and `return`
perform the usual resource cleanup. Inclusive ranges and general iterators
are not available yet.

## Generic functions

Write a function once and let its arguments choose the type:

```oxid
fn identity<T>(value: T) -> T {
    return value;
}

struct Number { value: i32 }

fn main() -> i32 {
    let number = identity(Number { value: 42 });
    return number.value;
}
```

Type parameters can stand for scalar or owned values, including records, enums,
fixed arrays and byte buffers. Passing an owned value transfers ownership.
Use `T: Copy` when the function needs to read the same scalar value more than
once. `Copy` does not provide arithmetic or comparison operators.

Type arguments are inferred from arguments. You can also specify every type
parameter in declaration order, for example `identity::<Number>(Number { value: 42 })`.
Use explicit type arguments when a parameter cannot be inferred from the inputs.
Type names at a call resolve in the caller's module. Generic functions work with
`check`, `run`, tests and native builds. Recursive calls require the `hosted`
execution policy for native builds. User-defined traits are not available yet.

## Generic structs and enums

Give reusable data types their own type parameters:

```oxid
struct Cell<T> { value: T }
enum Option<T> { None, Some(T) }

fn wrap<T>(value: T) -> Option<Cell<T>> {
    return Option::Some(Cell { value: value });
}

fn main() -> i32 {
    let value = wrap(42);
    match value {
        Option::None => { return 0; },
        Option::Some(cell) => { return cell.value; },
    }
}
```

Write `Cell<i32>` in a type annotation. Constructors infer type arguments from
field values or the selected enum payload: `Cell { value: 42 }` and
`Option::Some(42)`. This also works through nested structs and enums, including
inside generic functions. Match arms use the type of the matched value.

Specify every argument in declaration order when values cannot determine the
type, for example `Option::<i32>::None` or `Cell::<i32> { value: 42 }`.
Annotations and return types do not supply missing constructor arguments;
repeated occurrences of a type parameter must agree.

Each declaration and ordered argument list defines a distinct type, even when
its fields have the same layout. An owned enum payload transfers ownership to
its match binding. Reading a generic scalar field requires `T: Copy`; record
fields otherwise keep the existing whole-owner move and borrow rules.

## Long-running programs

Add this setting to a typed binary's `oxid.toml` to run without a cumulative work budget:

```toml
[execution]
policy = "hosted"
```

It applies to `oxid run`, native builds and declared project tests. Existing
projects without the setting keep the `bounded` policy. Changing `dev` or
`release` affects optimization; it does not change the execution policy.

Each workspace binary selects its own policy. Libraries cannot change their
caller's policy. Both choices retain arithmetic, bounds, ownership, allocation
and live-memory checks. Current compiler, storage and call-depth limits remain.
See the [execution policy contract](../rfcs/0044-project-execution-policy.md).

## Program arguments

```sh
oxid run -- input.bin output.bin
.oxid/bin/application input.bin output.bin
```

Import `arg_count`, `arg_bytes` and `ArgStatus` from `std::env` to read them.
Index 0 is the executable name; user arguments start at 1. `arg_bytes` copies
raw bytes into a mutable byte slice and reports the required length when that
slice is too short. Arguments after `--` are literal program input.
See the [argument API](../rfcs/0049-program-arguments.md) for status handling.
