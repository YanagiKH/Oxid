# Experimental checked typed-project API documentation

This is a compiler-backed documentation consumer for `typed-preview`, not a
new parser, language edition, runtime capability or completed G6 release gate.
It addresses ROADMAP_V2 R6's requirement that documentation tools share the
compiler's syntax and semantics. Existing limits and platform restrictions
remain in force. No new native or runtime qualification is implied.

## Selection and compatibility

```text
oxid doc --manifest-path PATH [--package NAME]
```

The literal `doc` command and an explicit in-scope `--manifest-path` opt in.
The selector must follow the command. Manifest and package values accept both
split and `=` spelling; each supported option occurs at most once. A split
value cannot begin with `-`; use the `=` spelling for such a path. `--` ends
recognition, and subsequent words are rejected source operands.

Explicit `--edition typed-preview` and `--message-format text` are optional
and may occur before or after the command. Legacy edition, JSON, source
operands, profile, entry mode, engine, backend, target, output path, native
mode and experimental compiler selectors are not supported. The selector
preserves existing option ownership around scripts and `--`.

Bare/default or explicitly legacy `doc` without an in-scope manifest selector
retains its old fixed-reference file-writing behavior. Typed-preview `doc`
without a manifest selector remains unsupported. This feature does not turn
the source-only `doc` command into a different implicit project lookup.

## Check before documenting

The command reuses existing manifest request admission, workspace selection,
path-dependency resolution, `CompilerSession::load` in Check mode and
`CompilerSession::check`. The complete selected source graph, including unused
dependency bodies, undergoes the ordinary compiler check. Dependencies need not
be separately documented. A root without `main` is allowed by Check; no entry
function is synthesized. Do not treat a successful syntax-only load as a
successful documentation check.

After checking, a renderer preflight walks retained AST/source views without
stdout output. Only after that succeeds is the document emitted. No source
execution, stdin read, native tool invocation or source/dependency/cache/build
file write occurs. The command does not create documentation files or launch
a browser. Users may explicitly redirect stdout.

Status 0 means the complete Markdown was written and flushed, with no stderr.
All documentation failures exit 1. CLI failures use E0001/cli; existing
manifest and compiler failures retain their codes/stages. Renderer/preparation
and output errors use E0002/project-doc. Diagnostics are best-effort. Loading,
checking and preparation failures emit no successful document. Output failure
may leave partial stdout, so this is not a transactional output stream.

## Public API view

Only the selected package appears. The root module is always shown. A child
module is shown only when all edges from the selected root are public. Private
subtrees are omitted even when an item inside says `pub`.

Each visible module shows its public functions, structs and enums in retained
source declaration order. Public record fields retain their declaration order;
private fields are omitted and produce one `// private fields omitted` marker.
A public enum shows all of its variants and payload types. Function bodies,
private types and private functions are not shown. Modules follow the compiler's
retained source-discovery order, not a new filesystem traversal or hash order.

Signatures are rendered from the checked AST with canonical punctuation and
spacing. The source's type/name spellings are retained. Module-level
`Signature imports` include only imports referenced as unqualified type names
in displayed signatures. Their original paths/aliases and declaration order
are retained; an explicit alias remains explicit. This section explains local
spelling, not public re-export or dependency API documentation.

Qualified names start at `crate`. Source locations use one-based lines and
Unicode scalar columns from `SourceFile::location`, at each item's name token.
The root source label is its entry basename; child labels are retained module
paths relative to that entry's directory. Successful output does not contain
absolute source paths, timestamps or build identifiers.

## Plain-text documentation comments

Functions, structs, enums and non-root modules can have immediately preceding
standalone `///` comments. A module's comments come from its declaration in the
parent source. This initial feature does not introduce root `//!` comments or
separate field/variant documentation.

Indentation is allowed before both the declaration and comment lines. Only a
consecutive doc-comment block attaches: a blank source line, ordinary comment,
intervening token or code before `pub` on its line breaks attachment. `////`
is ordinary commentary. `///` alone is a retained empty documentation line.
One optional ASCII space after `///` is removed. LF and CRLF are supported.

Each candidate line must correspond exactly to a retained line-comment trivia
token. Text that looks like `///` within a block comment or byte literal cannot
attach. This convention inspects the compiler's retained tokens rather than
lexing or parsing source again.

Each documentation line is a generated blockquote line `> TEXT`. Comments
and metadata are plain text: every ASCII punctuation character is backslash
escaped. Controls become visible escaped `[U+XXXX]` notation using uppercase
hexadecimal and at least four digits; ordinary Unicode text remains unchanged.
Generated structural punctuation and typed code blocks are not prose-escaped.
Comment Markdown, HTML, backticks and links are not interpreted as markup.

## Deterministic output shape

The document begins with the following fixed text; NAME is escaped manifest
name text, or escaped `(unnamed)` when no name was supplied:

```text
# Typed API: NAME

Edition: typed-preview (experimental).

The selected package was checked; dependencies are checked but not documented.
No code was executed. Source paths are relative to the selected entry directory.

```

Every module begins with `## Module crate[::child...]`, a blank line,
`Source: FILE`, and a blank line. FILE is prose-escaped. Attached module comments
follow, then optional Signature imports in an `oxid` fenced code block.

Each item starts with `### fn crate::name`, `### struct crate::Name` or
`### enum crate::Name`, followed by a blank line, `Source: FILE:LINE:COLUMN`,
a blank line and an `oxid` fenced signature block. Function declarations end
in `;`. Struct fields use `    pub name: TYPE,`; variants use `    Name,` or
`    Name(TYPE),`. Braces use a declaration line and a closing line, including
for empty records/enums. Signature blocks and comment blocks end with one
blank line. Attached item documentation follows the signature block.

No count/success trailer follows the document. Existing error diagnostics may
still contain normal compiler source paths; the relative-path rule applies to
successful documentation, not a rewrite of compiler diagnostics.

## Verification boundary

Implementation/source review alone does not establish compiler acceptance or
runtime/CI results. Independent tests should exercise ordinary CLI use with
path dependencies, workspace selection, public ancestry, library roots,
aliases and all supported signature shapes, comment boundaries and escaping,
check-error precedence, output failure and unchanged files. Native, security,
held-provider, fuel and complete release qualification are separate work.
