# Editor name completion

Status: experimental

`textDocument/completion` suggests names from the current unsaved source.
[Editor setup](../docs/TYPED_PROJECTS.md#edit-with-live-diagnostics) uses the
same language server command as diagnostics, formatting and navigation.

Suggestions include keywords, scalar type names, current-module functions,
records, enums, modules and import aliases. Function and nominal type parameters
appear within their declarations. Parameters, local bindings, destructured
bindings, range indices and match-arm bindings follow their lexical scopes.
Locals appear after their initializers. A nearer binding replaces a same-named
outer suggestion; names from a completed block do not escape it.

The response uses `CompletionList` with alphabetically ordered items. Each item
has a name, kind, short description and a `textEdit` replacing the entire
identifier around the cursor. The prefix before the cursor selects candidates.
Ranges use UTF-16 positions and current buffer text, including CRLF and non-BMP
characters. Unsynchronized documents and invalid positions fail the request.

Typing `crate::` or `crate::helper::` lists accessible modules, functions, records
and enums. The first segment also lists the project's direct dependencies.
Suggestions follow unsaved module and dependency buffers, including renamed
declarations. Private names follow the same visibility rules as ordinary source.
An unknown or inaccessible module has no suggestions.

An empty endpoint inserts the selected name at the cursor. A partly typed name
replaces its entire identifier, without changing the rest of the path. Completion
also works before an existing `as alias` in a `use` declaration, and after `::`
at the end of an unfinished function body. Source diagnostics still report
unfinished or invalid code; receiving a suggestion does not make that code valid.

Names do not require successful body type checking. If structural parsing fails,
ordinary name completion can still provide keywords and scalar type names.
Qualified module completion requires valid declaration headers.
Source-loading, namespace-policy and resource failures remain request errors.
Comments, string/character literals and numeric literals produce no suggestions.
Receiver members such as `value.field`, enum variants, and `std::`/`alloc::`
catalog paths are outside qualified module completion. Member completion,
automatic imports, snippets, hover, rename and receiver type inference are not
provided.

Completion uses the ordinary project graph and source overlay, with the same
1024-source-visit and 8 MiB aggregate source limits as navigation. Cross-file
project loading follows the ordinary project's host and path rules. One query
admits at most 4096 matching candidates and 1 MiB of candidate names before
sorting and deduplication. Larger
queries fail before publication and can be retried with a longer prefix.
Responses contain at most 512 items; a larger valid result sets `isIncomplete`.
The server performs no source execution, native compilation, source writes,
lock updates or network resolution for completion.

Public stdio tests cover unsaved aliases, lexical exits, reference and pattern
bindings, generic parameters, complete-word UTF-16 edits, partial syntax,
comments/literals, malformed positions, resynchronization and both result limits.
Qualified-path tests also cover empty endpoints, aliased imports, unsaved module
and dependency names, privacy, direct dependencies and unchanged saved locks.
