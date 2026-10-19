# Local path dependency sessions (R1)

Implementation design, based on typed-test commit
`15b0903f3a3cbdb03cb68d125a131b69bfb3405b`.

The existing loader owns one source map and AST vector with aligned file/module
IDs. It walks modules in declaration DFS order and confines child paths to the
entry directory. The declaration index currently assumes one root, allocates
`modules - 1` child rows, and computes privacy using DFS ancestry intervals.
Those assumptions must change together: package roots have no parent, close all
previous ancestry intervals, and contribute no synthetic module declaration.
Global function/record/enum IDs and existing public-signature/field checks stay
in force. Only the selected application's root supplies main and test discovery.

One immutable session owns command configuration and one forest, including the
resolved manifest graph. Packages are identified by canonical manifest pathname,
visited in manifest declaration DFS order, and loaded once. Each package keeps
its first resolved manifest spelling, explicit entry, direct dependency edges
and its contiguous module range. Existing lexical manifest-relative entry and
output behavior is retained, including a selected manifest symlink. Graph paths
resolve relative to the declaring manifest spelling; stable filesystem contents
are assumed. This is neither a sandbox nor a concurrent filesystem snapshot.

`[dependencies]` accepts `alias = "../library"`, identifying a directory with
`oxid.toml`. Every node declares typed-preview edition and entry. Aliases must
be source identifiers excluding language keywords, crate and std. Repeated
aliases, repeated canonical targets within one package, and DFS cycles are
errors. Only direct aliases are visible. Dependency roots are available solely
in use declarations; imported functions/types then use existing alias rules.
Each package's crate:: starts at its own root. Dependencies need no main.

CHECK, interpreter RUN, native BUILD/RUN and TEST load this same session, with
existing reporting, runtime policies and managed publication. All bodies,
including unused dependency bodies, are checked before execution/publication.
Edition, engine, entry policy, host target and fixed profile remain separate;
no new CLI flags are introduced.

The whole graph shares the existing source/token/node/module/index limits.
Every package consumes a module slot. Aggregate manifest input is at most
16,384 bytes (the existing single-manifest allowance). Dependency-edge storage
has a separately checked 16,384-byte envelope including edge rows and strings;
retained graph paths also consume the existing retained-path allowance. All
graph growth is checked before reservation; graph DFS uses bounded storage.
No budgets, native pins, legacy routes or compiler-owned std imports change.

No workspaces, lockfile writes, downloads, library ABI, publication or
incremental cache are introduced. Native acceptance still requires the separately
authorized LLVM 19.1.7/Linux x86_64 toolchain. These changes do not qualify the
base or resume held qualification/capacity work.

## CLI and semantic handoff

```toml
[project]
edition = "typed-preview"
entry = "src/main.ox"
name = "application"

[dependencies]
math = "../library"
```

The library directory must contain its own oxid.toml and explicit edition/entry.
Project names and dependency aliases do not determine package identity. Native
BUILD/RUN still require the selected application's name; dependency names remain
optional metadata. CHECK, interpreter RUN and TEST retain optional app names.

```text
use math::answer;
use math::model::Record as R;
```

Import endpoints must be original public functions or nominal types, through
public modules. Imports do not re-export imported aliases. Use the imported
names for calls and types; `math::answer()` does not become an external call.
Returning an opaque public record through a public function works without
making its fields public. Same-spelled records in separate packages remain
different types. A package's own modules may use its direct dependency aliases.

All existing explicit-manifest commands use the session without new flags:
`check`, `run` (interpreter or native), `build`, and `test` (interpreter or native).
Existing JSON and I/O restrictions apply to the complete graph. Source errors
retain original file IDs, paths and spans. A later invocation reloads dependencies;
an already loaded session continues to consume its immutable snapshot.

The example is in `examples/projects/path_dependency_session/`, separate from
compiler fixtures and independent typed-test acceptance. Focused implementation
controls are `cargo test --bin oxid path_dependency` and
`cargo test --test path_dependency_session_core`. The latter's ignored native
control checks managed build, standalone execution, native RUN/TEST parity,
dependency edits, and preservation of application outputs/ownership receipts.
Run it with `-- --ignored` only with the separately provisioned LLVM toolchain.
