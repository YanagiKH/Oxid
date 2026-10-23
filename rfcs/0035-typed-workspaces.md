# RFC 0035: typed workspaces with shared local input locks

Status: experimental implementation extending [RFC 0034](0034-typed-local-locking.md).

## Working with several packages

A workspace selects related typed packages without combining their source text
or changing their package identities. A virtual root contains:

~~~toml
[workspace]
edition = "typed-preview"
members = ["apps/client", "apps/server", "libs/protocol"]
default-members = ["apps/client"]
~~~

Every member has its own oxid.toml with an explicit typed-preview edition and
project name. Libraries declare their kind; their entry defaults to src/lib.ox:

~~~toml
[project]
name = "protocol"
edition = "typed-preview"
kind = "lib"
~~~

Binary members retain the default kind "bin" and main-entry selection. Local
dependencies remain explicit, for example protocol = "../../libs/protocol" in
[dependencies]. Membership does not add import aliases. A member imported as
a dependency must be a library. Each package retains its own crate root,
nominal types, visibility rules and alias namespace.

~~~sh
oxid check --workspace
oxid run --package client
oxid test --workspace
oxid lock
oxid build --workspace --frozen
~~~

Commands at the workspace root select default-members, or all members when
defaults are absent. Commands from a member directory or its subdirectories
select that member. --workspace selects all; --package NAME / -p NAME selects
an exact member path first, then an unambiguous project name. Distinct packages
may share a name; their paths remain selectable. These selectors conflict.

A workspace root may also declare a typed project. It joins the roster as "."
automatically; -p . selects it. Commands at the root still use workspace defaults.
Commands from its source subdirectory select the root package. An undeclared
nested package is rejected. Nested workspaces and mixed legacy members are
unsupported.

Member paths are exact relative directories. Globs, parent traversal, empty
components, backslashes, control characters, platform-special components and
symlink directory components are rejected. Quoted ordinary workspace tables and
multiline arrays of quoted member paths are supported. Inline, dotted, array and
nested workspace tables are rejected. Virtual roots cannot supply package
dependencies, build settings, scripts or tests.

Explicit file editions and explicit legacy commands retain their existing
routes. Implicit file operations inside a workspace request an explicit edition;
source-less commands select the member's actual package graph.

## Execution and output

Check performs ordinary type, ownership, visibility and body validation on all
selected packages, including libraries without main. It neither invents an
entry function nor executes a library to claim success.

Run requires one binary member. Process stdout and status belong to the program,
with no workspace trailer. Result mode retains the existing summary contract.
A library cannot be selected as an executable.

Build --workspace builds binaries and checks libraries. A library has status
"checked-library" and produces no archive or executable. Individual library
build requests check or the workspace build explicitly. Each binary remains at
its own member's .oxid/bin/<project-name>, so equal names do not collide.

All selected binary sources are checked and lowered before the first native
tool or output. Source and lock refusals preserve all previous member outputs.
Later tool, filesystem and environment failures use per-artifact atomic
publication; a workspace build is not a multi-artifact filesystem transaction.

Build --workspace --incremental forwards the existing native object reuse
option to every binary. Library checking has no native artifact and no cache.
The full workspace lock is verified before cache access. Native doctor admits
every member and its retained dependency inputs without executing or typechecking
project code; toolchain probing retains the separate RFC 0037 contract.

Test runs only declared [tests] entries. Each is a separate zero-argument
fn main() -> i32 process; zero passes and nonzero fails. A test replaces its
package's program root while keeping its dependency graph. Library entry
declarations are not injected into test sources. Every selected test is source-
and entry-checked before the first test starts. Each child re-admits the
workspace and applicable lock before running its own retained source session.
Root-package tests retain their member selection regardless of workspace
defaults. Helpers are never discovered and executed implicitly.

Human aggregate reports go to stderr. JSON mode emits ordinary compiler records,
member/test records and a final workspace-summary as JSON lines. A member or
test failure fails the aggregate command. Process-mode run retains text-only,
fallible error reporting.

Dependency graphs retain the existing Linux source-policy qualification.
Process execution and native executable builds require Linux x86_64; builds
also require the pinned LLVM toolchain. Workspaces do not broaden host, syntax
or bounded-runtime qualification.

## Complete-workspace lock

The root owns oxid.lock. An existing member-local lock is a conflict, never
silently ignored or deleted. Standalone typed v2 and legacy v1 locks retain
their separate contracts.

A generated workspace lock also marks its root during discovery. Removing the
workspace declaration while leaving that shared lock does not turn member
commands into unlocked standalone commands; the declaration must be restored.

Lock creates or verifies the complete workspace lock, even with a package
selector. A changed valid lock requires lock --update. Unknown, malformed,
legacy and standalone locks are rejected and preserved, including with update.

Ordinary commands without a lock remain no-write. An existing workspace lock,
--locked and --frozen admit every member graph regardless of selection.
Unselected member manifests and dependency sources therefore cannot hide
drift. --offline neither fetches nor creates a lock. Update conflicts and
test-child mode propagation follow RFC 0034.

The generated strict TOML subset is capped at 2 MiB:

~~~toml
version = 3
kind = "typed-workspace-v1"
manifest = "<SHA-256 of raw workspace oxid.toml>"

[[member]]
package = "<member package ID>"

[[package]]
id = "<package ID>"
path = "libs/protocol"
name = "protocol"
version = ""
edition = "typed-preview"
entry = "src/lib.ox"
manifest = "<SHA-256 of raw member oxid.toml>"
sources = "sha256:<loaded source checksum>"

[[dependency]]
owner = "<importing package ID>"
alias = "protocol"
package = "<library package ID>"
~~~

Member rows are sorted by ID. Package and dependency records preserve RFC 0034
field order, sorting and string grammar. Every member is a graph root; every
other package must be reachable from a member. Canonical directories deduplicate
the union; equal names never merge distinct roots.

Package IDs use the existing framed SHA-256 protocol with locations relative
to the canonical workspace root. External local dependencies may have ../
locations. Moving the complete relative layout preserves identity and lock
bytes. The raw workspace manifest pins the member roster and defaults. Every
package's manifest, normal entry and alias edges are pinned.

Every imported package has its actual source closure pinned, including library
members. A member that is not imported uses the source marker "root"; its
application and test bodies remain editable. Editing an imported library
requires update even when selecting that library alone. Test entries never
replace the normal entry recorded in the lock.

Source framing is unchanged. With lp(x) = u64le(len(x)) || x, SHA-256 covers
lp("oxid-typed-local-source-v1") || lp(relative entry) || u64le(file count),
then path-byte-sorted "F" || lp(relative path) || lp(raw retained bytes) records.
Loaded sources beneath target and .oxid remain included. Root-member
fingerprints also come from retained bytes, allowing a library's standalone
session to be compared with all sessions importing it.

Shared canonical manifests, entries and source fingerprints must agree across
prepared sessions. Changes between reads fail admission. Operations use those
prepared sessions; they never verify one read and execute a later source read.
Stable-tree filesystem policy, bounded regular-file manifest/lock readers,
exclusive lock creation, validated atomic update and failed-write preservation
remain in force. Creating a lock parses sources but does not typecheck them.

## Bounds and verification

Discovery examines at most 64 ancestor directories. Workspaces have 1–32
canonical members. Member paths allow 32 components, 255 bytes per component
and 4096 bytes total. Existing package graph, source and work limits still apply.
Every ancestor manifest used in discovery must be an ordinary regular file of
at most 1 MiB; its size is checked before allocating a content buffer. Explicit
legacy and explicit file editions retain their earlier routing boundaries.

Across retained member sessions, the aggregate limit is 256 graph nodes and
1024 edges, counting shared dependencies once per retained session. Normal
and selected test sessions together allow 1024 loaded files and 8 MiB source
bytes. Commands prepare at most 256 declared tests. These bounds count actual
retained sessions, not only the smaller deduplicated lock union.

The public CLI suite tests root/member selection, real library errors, lock
union and relocation, stale input refusal, source/entry preflight, special files
and resource bounds. The explicit LLVM test changes a shared dependency when
the first tool starts. Both same-name binaries must use the original retained
bytes, reject a stale rebuild and execute after their sources are removed.

Existing single-package tests are compatibility evidence; they do not qualify
these new workspace/library semantics. Registry resolution, semantic-version
selection, archive packaging, cross-compilation and release completion remain
separate work. This increment does not complete R1 or claim a v1 release.
