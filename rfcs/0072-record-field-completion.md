# Record field completion

Status: experimental

The language server offers visible record fields after `point.`. Suggestions
use the current open buffers and replace the entire field identifier around the
cursor. Completion at an empty endpoint or at the end of an unfinished function
body inserts the field name without changing surrounding text. Positions and
replacement ranges use UTF-16, including CRLF and non-BMP characters.

For example, `point.` in the following function offers `x` and `y`:

```oxid
struct Point { x: i32, y: i32 }

fn horizontal(point: &Point) -> i32 {
    return point.x;
}
```

The first supported receivers are simple identifiers bound as:

- a parameter of a monomorphic record type, `&Record`, or `&mut Record`;
- a preceding local initialized with a complete direct record constructor;
- that same local with an explicit annotation naming the identical record.

Type names may refer to the current module or use an explicit `crate::...` path,
including a direct dependency. Import aliases and generic record applications
are outside this scope. A generic function is also outside receiver completion.
Other generic declarations in the project do not prevent a query for an
independent monomorphic record.

The receiver must occur in the function's straight-line prefix. Complete `let`
statements may precede the query; the query can be a bare expression, a return
expression or the start of another local's initializer. A duplicate binding,
preceding reference to the receiver, control-flow statement or unsupported
expression context produces no field suggestions. This conservative rule does
not infer liveness or borrow permissions. Calls, indexing, chained receivers,
methods, enum variants and names inside comments or literals are not completed.
Recognizing a member query never falls back to unrelated lexical names.

Record and module visibility use the loaded project's real module identities.
Private fields are visible only in their declaring module and its descendants.
A constructor must supply each declared field exactly once, and those fields
must be accessible at its use site. Field value type checking remains the job
of ordinary diagnostics; receiving a completion does not make the source valid.

Open module and dependency buffers take precedence over disk. Closing a buffer
restores the saved declaration on the next query. Unsynchronized buffers, invalid
positions, source-policy failures and resource failures remain request errors.
If a file belongs to multiple application or test entry graphs, all graphs that
load it must agree on the receiver's nominal declaration and visible fields.
Different records are never combined, even if their names or field lists match.

## Source and resource contract

This extends [name completion](0069-editor-name-completion.md) with a separate,
lazy member query. It does not change the ordinary AST, source map, editor file,
qualified-path query, namespace layout or executable witnesses. The query reads
canonical tokens retained by the existing source loader. Its type-path handles
are bound to that exact source generation. Record handles borrow the actual
declaration namespace; spelling or a numeric ID alone cannot identify a record.

Cross-graph comparison binds the canonical package location, defining source
bytes and declaration range, loaded module ancestry and visible field names.
It reads the retained source session without reopening source files. Completion
does not invoke generic planning, ownership verification, execution, compilation,
lock updates or network resolution. Existing diagnostic requests keep their
ordinary checking behavior.

The existing source, token, nesting and namespace caps remain unchanged. The
member route admits a separate bank of complete typed cursor, lookup, sorting,
identity, result and return-transport carriers. It then charges actual field
slots and output strings before reserving them through the existing fallible
allocator. Namespace storage keeps its existing charge; no old allowance is
reused to hide new state. This is a logical request-storage model, not an RSS or
machine-stack limit. Existing diagnostic rendering and LSP JSON transport remain
outside that model.

Token visits, name comparisons, initializer checks, sorting, identity hashing
and result copying consume the existing query work budget. At most 4096 matching
candidates and 1 MiB of names are admitted; responses contain at most 512 items
and set `isIncomplete` when truncated. Across project graphs, the existing 1024
source-visit and 8 MiB source limits still apply. A refusal publishes no partial
field list.
