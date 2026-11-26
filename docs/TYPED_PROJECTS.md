# Native projects

Use Oxid's typed mode to check code, test local packages and build native
executables. Native project builds target Linux and macOS on x86_64 and ARM64.
Windows MSVC x64 supports bounded process entry with binary streams and binary
file I/O in result programs.
Native builds require LLVM, Clang and LLD 19.1.7 for the build host;
Oxid selects that host's native target automatically.

To call an Oxid function directly from C, Rust or Python, see
[native libraries](NATIVE_LIBRARIES.md). That experimental command builds a
Linux shared library with an explicit integer signature and returning errors.
For standard Python wheel and source-distribution builds that call the native
function directly, see [Python packages](PYTHON_PACKAGES.md).

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

## Windows projects

Windows MSVC x64 supports a single project without dependencies on a local NTFS
drive. Open the Visual Studio x64 Native Tools Command Prompt with the Windows
SDK and complete LLVM 19.1.7 toolchain available:

```bat
oxid new hello --edition typed-preview
cd hello
oxid check
oxid run
oxid test
oxid build --profile release
.oxid\bin\hello.exe
```

New Windows projects select result entry. For an existing project, set
`entry-mode = "result"` under `[build]`. Native programs currently support
bounded `bool`, `i32` and `()` functions, conditions and `while` loops. Use
`std::fs::File` with byte arrays or slices for [binary files](../spec/typed-preview.md#native-files).
Declared tests use bounded pure functions and require a no-argument `fn main() -> i32`:
zero passes and any other value fails. Checking and reference execution retain
their existing language scope.

Use the exact letter case declared by each module. Child modules cannot pass
through symbolic links or junctions. Network paths, other filesystems,
multi-package dependencies, workspace projects and source bundles are unsupported.
Process entry and process I/O, hosted native execution, native debug information
and native object caching remain unavailable. See [Windows native programs](windows-native.md)
for tool setup and the [module contract](../spec/typed-preview.md#bounded-typed-projects)
for source limits and path rules.

## Everyday commands

| Command | Result |
|---|---|
| `oxid check` | Check the project without running it. |
| `oxid run` | Run its entry point. |
| `oxid test [--jobs N]` | Run tests declared in `oxid.toml`. |
| `oxid build` | Build an executable under `.oxid/bin/` using project settings. |
| `oxid build --profile release` | Build with native optimization enabled. |
| `oxid fmt src/main.ox` | Print formatted source; add `--check` to check formatting. |
| `oxid fmt --check` | List files in the project that need formatting. |
| `oxid doc` | Generate public API documentation under `.oxid/doc/`. |
| `oxid lsp --stdio` | Check open editor buffers and provide formatting. |
| `oxid lock` | Record local dependency inputs in `oxid.lock`. |
| `oxid doctor --native` | Check the native tools, SDK and project configuration. |

Use `oxid test --jobs 4` to run up to four tests at a time. The runner reports results in the usual order. Omit `--jobs` to run tests one at a time.

The commands find `oxid.toml` above the current directory. Build failures preserve
the previous executable. `fmt` with a file path prints its result to stdout.
Run `oxid fmt --check --workspace` to check every workspace member, or use
`oxid fmt --check -p NAME` to select one. The check reads the entry, its modules
and the tests declared in `oxid.toml`. It leaves files unchanged.

## Edit with live diagnostics

Configure your editor's language server command as `oxid lsp --stdio` and use
the `oxid` language ID for `.ox` files. The server reads `oxid.toml`, checks open
buffers together with their modules and dependencies, and reports type and
ownership errors before saving. It also provides document formatting. LLVM is
not required, and checking does not run your program or write formatted source.

With Neovim 0.11 or later, add this to your configuration:

```lua
vim.filetype.add({ extension = { ox = 'oxid' } })
vim.lsp.config('oxid', {
  cmd = { 'oxid', 'lsp', '--stdio' },
  filetypes = { 'oxid' },
  root_markers = { 'oxid.toml', '.git' },
})
vim.lsp.enable('oxid')
```

Use `:lua vim.lsp.buf.format()` to format the current buffer. Save it normally
when ready. For standalone files, add `--edition`, `typed-preview` to the command
arguments. Files must already exist on disk; untitled buffers are unsupported.

Use `:lua vim.lsp.buf.definition()` to jump from a function call or a local
variable use to its declaration. `:lua vim.lsp.buf.document_symbol()` lists the
current file's declarations. In a project or workspace, use
`:lua vim.lsp.buf.workspace_symbol('name')` to search declarations by name.
These operations read unsaved buffers too; import aliases and function privacy
follow the compiler's ordinary namespace rules.

Use `<C-x><C-o>` in Neovim for name completion. Suggestions include keywords,
functions, types, import aliases and variables visible in the current block.
They follow unsaved text and replace the name around the cursor.
Type `crate::` or `crate::helper::` for visible module names, functions and types.
This also works with unsaved declarations, partial names and unfinished `use`
declarations, including an existing `as alias`.
Type `State::` for a local or imported enum to list its variants. You can also
use its full path, such as `crate::response::State::`.

Type `point.` to complete fields of a record parameter (`Point`, `&Point`, or
`&mut Point`), or a local initialized directly with `Point { ... }`. Field names
follow unsaved declarations and module visibility. Partial names replace the
whole identifier, including when the cursor is in its middle.

Field completion supports records without type parameters. Use a local name,
an imported name or alias, or a full path such as `crate::model::Point`.

Workspace checks include every member and declared test entry. Dependencies are
resolved offline; prepare Git dependencies with ordinary project commands first.
Manifests and lockfiles are read from disk. Diagnostics reflect editor source
buffers and do not replace `oxid check --frozen` for saved-input verification.
Use Go to Definition on a named type, a record or field in a constructor or
destructuring pattern, or a variant in a constructor or `match` arm. In a
type-correct project, you can also jump to each field in a read or assignment
such as `settings.display.width`. Type arguments and type parameters link to
their declarations. Navigation follows visible imports and reads unsaved source. See the [language server contract](../rfcs/0065-typed-language-server.md)
and [navigation scope](../rfcs/0066-editor-symbol-navigation.md).

## Generate API documentation

```sh
oxid doc
```

Open `.oxid/doc/index.html` in a browser to read the project's public functions,
generic parameters, structs and enums. Place `///` descriptions directly above
declarations or fields, and use `//!` at the start of a module. Descriptions are
shown as plain text; private declarations and function bodies are omitted.

Use `oxid doc --workspace` for every workspace member or
`oxid doc --package math` for one member. The selected APIs share one page at
the workspace root. Locking options such as `--frozen` also apply. All selected
packages are checked before replacing a page; checking or file publication
failures preserve the previous page. The command generates documentation without running the program.

Use `oxid doc --versioned --frozen` to keep a separate HTML page for the
package version in `oxid.toml`. For a workspace member, run
`oxid doc --versioned --package math --frozen`. Open or share
`.oxid/doc/versions/math/1.2.3/index.html`, using your package name and version.

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

Use `std::io::write_stderr(&bytes)` to write messages to
standard error. It returns `WriteSomeStatus::Written(count)` or
`WriteSomeStatus::Error(code)`. Handle short writes by passing the remaining
bytes in the next call. `std::io::write_some` writes to standard output.

Import `enable_termination_notifications` and `termination_requested` from
`std::process`. Call `enable_termination_notifications()` from a process entry
to receive SIGINT and SIGTERM on Linux and macOS, or Ctrl+C and Ctrl+Break in a
Windows x64 console. If it returns true, poll
`termination_requested()` between work items and return from `main` to close
files and release other owned values.
Import `wait_for_termination` from `std::process` to wait when there is no work
ready. Pass a timeout in milliseconds. It returns true after a notification and
false when the wait expires. A zero timeout polls immediately. Negative timeouts
and calls before enabling notifications return false immediately.

On Linux and macOS, enable notifications before using `std::process::wait_for_stdin(1000)` to
wait up to one second for input. It returns true when a read can proceed,
including when the input stream closes. A timeout, termination notification,
or unavailable input returns false. Check `termination_requested()` to decide
when to exit. Pass zero to check immediately.

## Measuring elapsed time

Import `monotonic_millis` from `std::time`. Save a reading before your work
and subtract it from a later reading to get elapsed milliseconds as an `i64`.
Check both readings for `-1`, which means the clock is unavailable. It works
on Linux and macOS with x86-64 or ARM64, and on Windows with x86-64. Changes
to the system date and time do not affect the readings.

For timestamps, import `unix_millis` from `std::time`. It returns an `i64`
counting milliseconds since midnight UTC on January 1, 1970. The smallest
`i64` value means the clock is unavailable.

To write timestamped JSON events, use the logging package. [Structured logging](../packages/logging/README.md).

Use `sleep_millis(100)` from `std::time` to pause the current thread for
about 100 milliseconds. It returns true when the wait completes. Zero completes
immediately. A negative duration or a failed wait returns false.
On Windows, use the default result entry and bounded execution policy to
build native programs that use these functions.

## Drawing repeatable random numbers

Add the [seeded random library](../packages/random/README.md) as a dependency
and use `seeded(42u32)` to create a generator. Call `below(&mut generator, 100u32)`
to draw a number from 0 to 99. Reuse the seed to repeat the same sequence.

## Reading and writing file buffers

Use `open_append(&path)` from `std::fs` to add bytes at the end of a file.
It creates the file when needed and preserves existing contents.

Add the [file I/O library](../packages/file_io/oxid.toml) as a local dependency
and import `read_exact` and `write_all` through its dependency name. `read_exact`
fills a mutable byte slice or reports how many bytes it read before EOF.
`write_all` continues until the slice is written or a write makes no progress.
Both functions report the number of bytes transferred and the system error code
if an I/O error occurs.
`copy` uses a nonempty buffer you provide, reads until EOF and returns the
number of bytes written. It leaves both files open and reports read or write
failures with the byte count so far.

## Build profiles

```sh
oxid build --profile dev
oxid build --profile release
```

`dev` builds without optimization (`-O0`); `release` enables optimization
(`-O2`) for both the program and its runtime. Both preserve checked arithmetic,
bounds checks and the selected execution policy. `bounded` is an alias for `dev`.
Source-level line information can be enabled separately with `--debug-info`.

For programs using owned values, `release` omits function bodies that cannot be
reached from the entry point. Every declared function still undergoes the full
type, ownership and native resource checks. `dev` retains the complete output.

The command-line profile overrides `[build] profile` in the selected package.
Omitting both uses `dev`. A workspace uses each selected member's setting;
`--profile` overrides every selected member. Dependency settings do not change
the importing application's profile. All profiles write the same output path,
so copy an executable elsewhere if you need to keep both builds.

Profiles also work with `--incremental` and `--reproducible`. Switching between
`dev` and `release` selects a different cached program object. Switching between
`dev` and `bounded` can reuse the same object.
[Build profile details](../rfcs/0040-native-build-profiles.md).

## Debug a program

```sh
oxid build --debug-info
gdb .oxid/bin/hello
```

In GDB, use `break main`, `break src/main.ox:2`, `run`, `next` and `backtrace`
to stop in Oxid source and inspect the call stack. Source files must be available
to show their text. The executable still runs after those files are removed.
Use `info args`, `info locals` and `print name` to inspect scalar parameters and
initialized local variables. This includes integer widths, floating-point values,
booleans and characters. Variables follow their source block's scope.

Set `debug-info = true` under `[build]` to keep line information enabled.
`--debug-info` and `--no-debug-info` override the selected package's setting;
workspace builds apply the command-line choice to every selected member.
Both `dev` and `release` support it. Optimized builds can fold or remove source
lines, functions and values. A value without a recoverable machine location
appears as `<optimized out>`; this can also happen to an SSA value in `dev`.
Nonempty, non-generic Copy records with only numeric scalar fields also support
`print point`, `print point.x` and `ptype point`, including by-value parameters
and independently modified copies. Records containing booleans, characters,
unit, nested values or resources are omitted as a whole. Other owned values and
references are not yet described for variable inspection.

`--reproducible` keeps debug paths relative to the project or workspace root.
Run the debugger from that root after moving the project, or configure its source
search path. Debug and non-debug builds use different cached program objects.

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
include cached Git libraries as local dependencies.
See [Git dependencies](../rfcs/0059-typed-git-dependencies.md).

After changing dependency manifests or source code intentionally, run
`oxid lock --update`. Normal commands verify an existing lock. `--locked`
requires one; `--frozen` also requires offline operation. Application and test
source bodies remain editable.

## Private registry libraries

On Linux, use a private file registry to share versioned libraries between
projects.

Create a new registry directory:

```sh
oxid registry init /home/me/oxid-registry
```

Give the library a name and version in its `oxid.toml`:

```toml
[project]
name = "math"
version = "1.2.0"
edition = "typed-preview"
kind = "lib"
entry = "src/lib.ox"
```

From the library root, record its inputs and publish:

```sh
oxid lock
oxid publish --registry file:///home/me/oxid-registry
```

Use `--package NAME` to select a workspace library. Publishing checks the full
author lock and the library's declared tests without running their bodies.
Use dependencies from the same registry when publishing. The published library
records the dependency versions from your lockfile.

Find packages by name or prefix. The search lists the newest available version
of each match and skips releases that have been yanked.

```sh
oxid registry search file:///home/me/oxid-registry math
```

Choose an alias, package name and version for each dependency.

```toml
[dependencies]
math = { registry = "file:///home/me/oxid-registry", package = "math", version = "^1.2.0" }
old_math = { registry = "file:///home/me/oxid-registry", package = "math", version = "1.1.0" }
```

Use public items through `crate::math` or `crate::old_math`. The range `^1.2.0`
selects the newest available release at or above 1.2.0 and below 2.0.0. The
range `^0.2.0` stays within 0.2.x. Run `oxid lock` to cache the selected releases
and fix their versions. Run `oxid lock --update` to select newer releases.
Restore the previous manifest and lock to roll back to its cached versions.

Registry maintainers can remove a release from future range selections and
restore it later.

```sh
oxid registry yank file:///home/me/oxid-registry math 1.2.0
oxid registry unyank file:///home/me/oxid-registry math 1.2.0
```

Existing locks and explicit exact-version dependencies can still use the
release. Its source archive stays available for existing builds and rollbacks.

`--locked`, `--offline` and `--frozen` never fetch a missing release, even from a
local registry. Plain `doc`, `doctor` and editor requests also use cached releases
only. Cached source changes are refused and preserved. Trust the registry
directory you select: hashes check integrity against the selected inputs, not
the publisher's identity. Keep `oxid.lock` in version control.

`oxid package --frozen` includes cached registry libraries as local sources.
The extracted source bundle can be checked and built offline without the
registry or original cache. See the [private registry contract](../rfcs/0075-typed-private-registry.md)
for artifact, identity and resource limits.

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
Cached Git and private-registry libraries are included as local sources, so the
restored project does not require Git, a registry or the original cache.
Creation never fetches dependencies.
Use `--sha256 HEX` with verify or extract to compare a separately supplied hash.
[Package details](../rfcs/0038-typed-source-packages.md).

To include test data or other files, list their paths relative to `oxid.toml`.

```toml
[package-files]
"tests/data/example.bin" = true
```

For complete applications, try [Byte count](../apps/byte-count/README.md) or
[Frame filter](../apps/frame-filter/README.md), which passes owned records,
arrays and nested enum values between local packages.
Try [UTF-8 normalize](../apps/utf8-normalize/README.md) to validate and transform
text using a shared [owned UTF-8 library](../packages/utf8_text/README.md).

Use a streaming decoder when a character can span two file or network reads.
[Streaming UTF-8](../packages/utf8_stream/README.md).
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

Owned vectors can store finite user-defined enums with scalar or owned payloads,
including UTF-8 `Text`, byte buffers and files. Use `new`, `len`, `capacity`,
`try_reserve`, `push_reserved`, `pop_last`, `truncate` and `take` from `alloc::vec`.
`push_reserved` moves its enum argument into reserved space; `pop_last` moves
the last value out. Reserve capacity before pushing, and check that the vector
is nonempty before popping. Truncation releases resources in the removed values.
Built-in status enums use the same operations. The vector itself always moves
when passed by value.

An enum variant can hold a vector, for example
`enum Batch { Empty, Records(Vec<Message>) }`. Construct
`Batch::Records(messages)` to move the vector in. Match
`Batch::Records(records)` to move it back out.

Enum vectors do not support indexing, element references, slices, elements
that contain another owned vector, recursive element layouts or custom
destructors. `#[derive(Copy)]` on enums remains unsupported.

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
fields otherwise keep the existing move rules. Different record fields may be
[borrowed together](../rfcs/0067-disjoint-field-borrows.md), including nested
fields and fields that own buffers or vectors. A record can move once its field
references have ended. Whole values can also have [local reference bindings](../rfcs/0068-local-reference-bindings.md)
such as `let current = &mut value;`. A local reference lasts until its declaring
block ends; `return`, `break` and `continue` also end the scopes they leave.
Forward it to a helper with `&*current` or `&mut *current`. References cannot
yet be returned from functions or stored in records.

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

Hosted projects can set the shared allocation budget in bytes:

```toml
[execution]
policy = "hosted"
heap-limit = 67108864 # 64 MiB
```

The default is 16 MiB. The budget counts buffer capacity and allocation metadata,
including both buffers during growth. It applies to byte buffers and the
collections built from them, across dependencies in the same program. A value
of `0` prevents nonempty allocations. `run`, project tests and native builds use
the same setting; libraries do not override the calling program's budget.
Process memory also includes code, stacks and other runtime storage. Allocation
can still fail when the operating system cannot provide memory.

## Program arguments

```sh
oxid run -- input.bin output.bin
.oxid/bin/application input.bin output.bin
```

Import `arg_count`, `arg_bytes` and `ArgStatus` from `std::env` to read them.
Index 0 is the executable name. User arguments start at 1. `arg_bytes` copies
bytes into a mutable byte slice and reports the required length when that slice
is too short. Linux and macOS preserve the original bytes. Windows provides
UTF-8 and replaces unpaired UTF-16 surrogates with `U+FFFD`. Arguments after
`--` are passed to the program as given.
See the [argument API](../rfcs/0049-program-arguments.md) for status handling.
