# Typed language server

Status: experimental

`oxid lsp --stdio` provides LSP diagnostics and document formatting for typed
Oxid projects. It uses the ordinary parser, namespace resolver, type checker,
ownership checker and verified OIR. It requires no native compiler and executes
no source code. [Editor setup](../docs/TYPED_PROJECTS.md#edit-with-live-diagnostics)
shows a working client configuration.

## Project and source selection

Project mode requires `edition = "typed-preview"` in `oxid.toml`. An explicit
`--edition typed-preview` also permits standalone source files. This choice is
independent of the default legacy edition. Existing script/file invocations
continue to use their ordinary command dispatch.

Each refresh discovers projects from workspace folders and open files. A typed
workspace checks every member, including members outside `default-members`.
Declared `[tests]` entries use the same entry override as project test checking.
Checking a child module uses its project's entry and package-local namespaces.
An open file outside every successfully loaded entry receives a diagnostic.
Dependencies use the ordinary manifest resolver in locked, offline mode. Git
dependencies must already be available in the project cache; editor requests
never authorize fetching them. Manifests and locks are not editor buffers.

Canonical local file identities select open UTF-8 source buffers. Every admitted
module and dependency reads its matching buffer in the same retained source
session, then follows the existing source, graph and compiler limits. Saved
source and lock bytes remain unchanged. Source buffer changes intentionally do
not match the saved-input checksums; use `oxid check --frozen` to verify those
inputs after saving. Existing files are required. Untitled documents, remote
file authorities and non-file URI schemes are rejected. Cross-file loading has
the existing Linux-only scope; portable single-file checks remain separate.

## Protocol and synchronization

Standard input and output carry UTF-8 JSON-RPC with CRLF `Content-Length`
headers measured in bytes. Standard output contains only protocol messages;
transport failures use standard error and exit 74. JSON object keys must be
unique; duplicate escaped keys, trailing documents, malformed UTF-8 and invalid
surrogate sequences are rejected. Integer and string request IDs are echoed
without floating-point conversion. Initialization, shutdown and exit follow
the LSP lifecycle. Unknown requests return `MethodNotFound`.

The advertised position encoding is UTF-16. Incremental changes apply in order
within one atomic batch. CRLF, LF and bare CR line endings are supported; line
and column values past the text clamp as specified by LSP. Positions inside a
surrogate pair, reversed ranges and mismatching `rangeLength` are rejected.
Versions must increase; stale changes are ignored. An invalid current update
marks that document unsynchronized and clears prior diagnostics. Formatting and
checking resume after a full text update, a save carrying complete text, or a
close and reopen. No partial edit batch can replace the prior buffer.

Refreshes publish diagnostics for current source locations and clear obsolete
reports. Open documents include their current version. Primary ranges and
related locations use retained buffer text, including characters outside the
Unicode basic multilingual plane. Closing a file restores disk input for
future checks. Save, watched-file and workspace-folder notifications trigger
fresh checks. A refresh is synchronous; later messages cannot interleave with
its source session.

Formatting returns a full-document text edit or an empty edit list when the
buffer is already formatted. It uses the typed formatter without type checking,
source execution or writes to the source file. Syntax or edition failures return
`RequestFailed` and preserve the buffer.

## Limits and supported operations

The server accepts at most 16 workspace folders, 256 open documents, 1 MiB per
document, 16 MiB of open source text and 256 edits per update. One refresh admits
at most 256 package visits, 1024 dependency edges, 1024 loaded source visits and
8 MiB of source text across its deduplicated entries. Existing per-project limits
still apply. Limit exhaustion produces a visible configuration diagnostic.

Input frames are limited to 8 MiB, headers to 8 KiB, JSON values to 65,536 nodes
and the JSON parser's default nesting limit. Responses are bounded to 32 MiB
before publication. A refresh retains at most 256 compiler diagnostics, 8 KiB of
message text per diagnostic and eight related locations with 2 KiB messages.

The service advertises synchronization, diagnostics, workspace folders and
formatting. Completion, hover, go-to-definition, rename, code actions and
profiling are not implemented. This experimental service does not complete the
development-tool release gate.

## Verification

`tests/typed_language_server.rs` starts the actual public executable over pipes
and checks protocol recovery, exact request IDs, UTF-16 edits and diagnostic
ranges, stale versions, resynchronization and formatting. Linux checks cover
unsaved child modules, declared tests, dependency changes with an unchanged
saved lockfile and every workspace member. Division-by-zero fixture bodies
prove checks do not run programs. Unit tests cover bounded framing, JSON, URI
handling, atomic document changes and frontend buffer admission. Hosted results
only qualify the exact source and host on which they execute.
