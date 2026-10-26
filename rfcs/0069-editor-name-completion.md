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

Names do not require successful body type checking. If structural parsing fails,
ordinary lexical input can still provide keywords and scalar type names.
Source-loading, namespace-policy and resource failures remain request errors.
Comments, string/character literals, numeric literals, member access and
qualified paths produce no suggestions. Member completion, automatic imports,
snippets, hover, rename and type inference are not provided.

Completion uses the ordinary project graph and source overlay, with the same
1024-source-visit and 8 MiB aggregate source limits as navigation. Cross-file
project loading currently requires Linux. One query admits at most 4096 matching
candidates and 1 MiB of candidate names before sorting and deduplication. Larger
queries fail before publication and can be retried with a longer prefix.
Responses contain at most 512 items; a larger valid result sets `isIncomplete`.
The server performs no source execution, native compilation, source writes,
lock updates or network resolution for completion.

Public stdio tests cover unsaved aliases, lexical exits, reference and pattern
bindings, generic parameters, complete-word UTF-16 edits, partial syntax,
comments/literals, malformed positions, resynchronization and both result limits.
