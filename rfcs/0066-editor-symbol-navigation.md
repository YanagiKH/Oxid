# Editor definition navigation and symbol lists

Status: experimental

The [typed language server](0065-typed-language-server.md) supports
`textDocument/definition`, `textDocument/documentSymbol` and `workspace/symbol`.
These requests read the same project settings and open buffers as diagnostics.
They perform no execution, native compilation or source writes.

## Jump to a definition

Function calls navigate to the source function's name. The ordinary declaration
index resolves package aliases, module paths, import aliases and visibility.
Function source locations are retained before generic instantiation, so a
navigation result names the original declaration. Built-in functions without a
source declaration, unresolved names and private functions outside the caller's
scope have no result. Namespace or resource failures remain request errors.

Parameter, ordinary `let` and range-index uses navigate within their source
function and lexical block. A local is available after its initializer; a range
index is available only inside its loop body. Comments and unrelated functions
cannot supply a name match. Definition requests currently cover functions and
these local bindings. Type names, fields, enum variants, destructuring bindings
and general reference-place navigation are outside this increment.

Definitions use UTF-16 ranges from the current buffers. Invalid positions and
unsynchronized documents fail the request. A request retains one source session
per admitted entry and returns at most 32 distinct locations; a larger result
set reports the limit rather than publishing an incomplete result.
Aggregate source visits use the same 1024-file and 8 MiB limits as diagnostics.

## Document and workspace symbols

Document symbols list source functions, modules, structs and their fields, enums
and their variants. Private declarations are included in their own source files.
Fields and variants carry their declaring type's name. The response uses the LSP
`SymbolInformation` form with precise name locations.

Symbol lists use retained parsed declarations without requiring successful body
type checking. Structural syntax must still parse. A body type error therefore
does not hide the rest of a well-formed file's outline.

Workspace search visits every project/workspace entry in the configured folders,
including dependencies and declared tests. Identical source declarations reached
through several entries appear once. Equal names in different files remain
distinct. The query is a case-sensitive substring of at most 128 UTF-8 bytes.
Workspace search requires a typed project or workspace folder; standalone files
support document symbols and definition requests.

One symbol request uses the existing aggregate package, edge and source limits.
It returns at most 4096 symbols with an 8 MiB aggregate name/path/container budget.
Exhaustion reports a request error; narrow a workspace query to reduce results.
Output retains the transport's bounded JSON publication limit.

Bulk symbol locations use a fallible UTF-16 checkpoint index every 256 source
bytes. Each lookup scans at most one checkpoint suffix rather than repeatedly
walking the file prefix. UTF-8 boundaries, CRLF, LF and bare CR follow the same
coordinate contract as diagnostics.

## Verification

The public stdio tests in `tests/typed_language_server.rs` cover parameter and
local identity, function calls, aliases, private targets, unsaved declaration
positions, UTF-16 positions and outlines during body type errors. Workspace
tests distinguish equal names in different members and query unsaved names.
Cross-file tests retain the current Linux-only source-loading scope. Other
language-server synchronization and compiler checks remain required.
