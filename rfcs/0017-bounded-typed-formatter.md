# RFC 0017: bounded, line-preserving typed-preview formatting

- Status: experimental implementation; independent review and exact-head CI pending
- Baseline: published main `17ed3de243785006ab888d87af4428d06117b21d`
- Owner: Oxid language/tooling maintainers
- Review: independent implementation review required before merge
- Acceptance decision: experimental command implemented; no stability or v1.0 claim
- Scope: one explicit UTF-8 source file; stdout formatting and read-only `--check`

## 1. Decision and motivation

Add an explicitly selected `typed-preview fmt` route that formats the
already-published scalar, nominal-struct and module/import source grammar.
This first slice normalizes horizontal whitespace and indentation while
preserving existing line breaks. It does not choose line widths, reflow code,
insert structural newlines, or write files.

This is a useful editor/CI building block with a small, reviewable contract:
compact code remains compact, and multiline code gains reliable indentation
without counting braces inside comments. It is deliberately not a complete
pretty printer, public CST, language-server formatter, or v1.0 milestone.

The existing typed token tape is sufficient for this slice when combined with
successful syntax parsing and a small classification of token roles. It is
not sufficient justification for reconstructing source from AST names/values:
exact token and comment bytes remain authoritative.

## 2. Public command

Accepted forms include:

```text
oxid fmt --edition typed-preview input.ox
oxid --edition=typed-preview fmt --check input.ox
oxid fmt input.ox --edition=typed-preview --check
oxid fmt --edition=typed-preview -- --flag-named.ox
```

Exactly one explicit source operand must follow the command. `--check` may
occur once after `fmt`, before the `--` separator, before or after that operand.
It is not a global option and cannot precede `fmt`. It is a
flag, so `--check=true` is invalid. Existing space/equal edition selection and
the separator/script-payload boundaries remain in force.

The formatter has no default path, directory traversal, stdin mode, multiple
file mode, `--write`, `--output`, configuration file, width option, or
module search. A lone `-` is rejected as unsupported stdin syntax. After
`--`, other dash-prefixed words are literal filenames. A file named `-`
can be selected as `./-`.

Use the existing text diagnostic rendering/escaping. Default and explicit
`--message-format=text` are supported. `--message-format=json` is rejected
for a selected typed formatter route: JSON formatter diagnostics are deferred
rather than mixed with source stdout. Formatter-owned errors use stderr.
Preexisting global routing errors before a valid edition/operation is selected
retain their existing global diagnostic contract.

| Result after selecting typed fmt | Exit | stdout | stderr |
| --- | ---: | --- | --- |
| Successful format | 0 | Exact complete formatted source | Empty |
| Successful check, already formatted | 0 | Empty | Empty |
| Successful check, would change | 1 | Empty | `typed-preview fmt: formatting required\n` |
| CLI, read, encoding, syntax, admission or invariant error | 2 | Empty before output begins | Escaped text diagnostic(s) |
| stdout write/flush failure | 2 | A prefix may already have reached the pipe | Escaped I/O error if writable |

There is no success banner and no diff in this slice. `--check` performs the
same validation and candidate construction as ordinary formatting, then
compares exact bytes. Code 1 means only “formatting required”; code 2 must
never be misreported as formatting drift.

An explicit valid typed route never falls back to the legacy formatter,
including after failure. Default `oxid fmt` and explicit legacy formatting
retain their existing behavior. Check/run/compile contracts are unchanged.

## 3. Input and malformed-input boundary

1. Validate formatter arguments before opening anything.
2. Inspect only the named source's metadata, reject known nonregular targets,
   then open it and check the handle metadata before reading. Require a regular
   file (a symlink resolving to a regular file is allowed). Reject directories,
   FIFOs and devices; do not
   walk parents or siblings. Metadata/open races and blocking host I/O are not
   an OS-sandbox guarantee.
3. Read at most 1,048,577 bytes, rejecting more than 1,048,576; validate UTF-8
   without normalization and construct an immutable source owner.
4. Lex and parse the complete file using the published project-capable syntax
   mode. Do not invoke project loading, module discovery, name resolution,
   typechecking, ownership analysis, OIR, reference execution, native tools,
   manifests, caches, package code, or source execution.
5. Any lexer or parser error rejects the whole file. Recovery diagnostics may
   be emitted up to the existing limit, but no recovered fragment is formatted.
   Unsupported spelling/syntax stays unsupported; no repair, token insertion,
   partial output, or legacy fallback is allowed.

Syntactically valid unresolved modules, imports, calls and nominal types format
successfully. So do syntactically valid type errors or oversized decimal
values that a later semantic phase would reject. “Formatted” does not mean
“typechecked.” Existing syntax restrictions, including adjacent bytes in each
`::` delimiter, remain binding.

Empty and whitespace-only files are valid and format to zero bytes. A
comment-only file is valid and preserves each comment, followed by one final
LF outside the comment if needed.

## 4. Protected atoms and whitespace ownership

A **protected atom** is either a nontrivia, non-EOF token, or a comment trivia
token. It carries its exact original UTF-8 byte slice. A **gap** is the
possibly empty sequence of whitespace trivia between adjacent atoms, or at a
file boundary.

The current lexer has one `Kind::Trivia` for whitespace, line comments and
block comments. Classify by the original slice: `//` starts a line comment,
`/*` starts a non-nested block comment, and all-whitespace trivia is a gap.
Do not search for delimiters inside comments. EOF is structural and emits no
bytes. Only gap bytes can change.

Comments are opaque, including trailing spaces, tabs, Unicode, CR characters,
embedded newlines and indentation inside multiline block comments. The
formatter never wraps, reindents inside, rewrites or deletes a comment and
does not interpret formatter-control comments in this slice.

Comments retain their order and the same preceding/following nontrivia-token
ordinals (BOF/EOF sentinels allowed). A trailing comment stays trailing and a
standalone comment stays standalone because intervening LF counts are
preserved. Horizontal spaces next to a comment normalize to one space when
both adjacent atoms share a line; line-boundary rules take precedence.
This also separates adjacent comments without merging their bodies.

## 5. Deterministic layout policy

Apply these rules in this precedence order.

### 5.1 File and line boundaries

- Remove whitespace-only gaps before the first atom and after the last atom.
- For a nonempty result, append exactly one LF outside the final atom.
  Preserve LF already *inside* an opaque final comment; never strip it.
- For each interior gap containing LF, preserve exactly its LF count and
  replace each separator with LF. Remove spaces/tabs on blank lines and
  before LF. Recompute indentation before the next atom.
- A CR outside comments is ordinary whitespace; CRLF gaps become LF gaps.
  A lone CR does not create a line break, matching the current source model.
  All other Unicode whitespace accepted by the lexer is horizontal unless it
  contains LF and is normalized by the same rules.
- Newlines inside comments are copied, never counted as editable gaps.
  Therefore a CRLF line comment keeps its terminal CR and gets LF from the
  following gap. Mixed line endings can remain inside protected comment bytes.

No interior LF is added, removed, or moved across an atom. Blank-line counts
are preserved; this slice intentionally does not impose a blank-line style.
The final LF is the only new line break permitted.

### 5.2 Indentation

Use four ASCII spaces per currently open `{` or `(` delimiter.
Matched closing delimiters pop their corresponding opener. Count delimiters
only from parsed nontrivia tokens, never comment/string text.

At BOF or after an editable newline gap, use the current delimiter depth,
minus the length of the uninterrupted prefix of closing-delimiter atoms
beginning that line. A comment interrupts this prefix. Thus a line beginning
`});` is dedented by two; a line beginning `/* note */ }` retains the
interior indentation. Do not dedent the persistent stack early: consume/pop
the actual closers normally while traversing tokens.

Every opener counts, including one opened on the current line; no alignment
to columns or continuation-width heuristic applies. Text within a multiline
comment remains untouched even if its interior lines look under-indented.

### 5.3 Horizontal gaps without comments or LF

Choose spacing from parsed token roles, not a string substitution:

- No space inside parentheses: after `(` or before `)`; no space between a
  declaration/call name and its `(`.
- No space before `,` or `;`; one space after either if another atom follows
  on the same line, except before a closing parenthesis.
- No spaces around field-access `.` or qualified-path `::`. Never split
  the adjacent two colon bytes. A type/field colon instead has no space
  before it and one after.
- One space around `->`, assignment `=`, arithmetic binary `+ - *`,
  comparisons `== != < <= > >=`, and logical `&& ||`.
- No space between logical `!` and its operand, nor between the literal-only
  minus and its decimal digits. Keep all decimal bytes, including zeros.
- Immutable references/borrows are `&T`, `&name`, or `&*name`.
  Mutable forms are `&mut T`, `&mut name`, or `&mut *name`: no gap
  after `&`, one after `mut`, no gap after the borrow-place `*`.
  Multiplication `*` remains a binary operator.
- Empty brace tokens separated by a comment-free, zero-LF gap become `{}`.
  Otherwise place one
  space after `{` and before `}` when they share a line with content.
  A brace touching a following comma/semicolon/closing parenthesis obeys that
  punctuation's no-space rule. Between `}` and `else`, or another
  same-line item/statement, use one space.
- All remaining legal pairs use one ASCII space (for example `pub fn`,
  `return x`, `if (`, and `Name {`).

Comment or newline boundaries override these horizontal rules; for example
`- /* sign */ 0007` preserves the comment and spaces, and a newline between
a call name and `(` is not joined. The second parse protects special
adjacency-sensitive grammar from accidental changes.

## 6. Invariants and fail-closed validation

Let F be a successful format operation.

1. **Exact protected projection:** input and output have the identical single
   interleaved sequence of protected atoms: (token-kind, raw-bytes) for code
   and (comment, raw-bytes) for comments. EOF and whitespace tokens are
   excluded. Separate code/comment lists are insufficient because they would
   miss comment relocation across code tokens. No punctuation,
   grouping, keyword alias, literal spelling, import order or trailing comma
   is added/removed.
2. **Comment anchors:** each comment has identical preceding/following
   nontrivia-token ordinals and preserved LF counts in interior gaps.
   BOF/EOF gaps instead follow the explicit boundary normalization in 5.1.
3. **Syntax equivalence:** both parses succeed in the same mode, and their
   ASTs are structurally equal after rebasing source locations. Retain item
   order, all variants, group nodes, operators, boolean values, numeric
   digits/sign, names, path segments, visibility, argument/field/parameter
   order, statement/arena edges and project-syntax classification. Ignore only
   source-owner identity, display path, byte offsets, line/column metadata and
   lengths that describe changed whitespace. A span's semantic payload is
   compared as its covered nontrivia-token interval plus any intra-token byte
   boundaries, not by slicing a whitespace-containing span and comparing it
   verbatim. Do not use Debug-output string normalization as an oracle.
4. **Idempotence:** F(F(source)) equals F(source), byte for byte, and the
   second operation fits the same admission limits. `--check` on that output
   returns 0.
5. **No effects:** input bytes are unchanged. Formatting never opens a module,
   executes source, starts a compiler, writes a cache, or creates output files.

Before the first stdout byte, build the full candidate, re-lex and re-parse it,
and compare the interleaved protected projection and interior-gap LF counts.
A mismatch or unexpectedly malformed
candidate is an internal formatter error (`E0500`, stage `format`), not an
attempt to repair it. Independent acceptance checks must compare normalized
ASTs and idempotence; do not derive expected goldens from the implementation.
Production AST comparison may be added if its separately counted bounded
implementation is warranted; it is not a substitute for the independent check.

A candidate exceeding an ordinary output source/token limit is an admission
error (`E0400`, stage `format`), not an internal error. Re-parsing candidates
is required because adding whitespace can increase token count. Successful
outputs must remain legal inputs under the identical limits.

## 7. Explicit resource contract

Inclusive input and candidate-output limits:

| Resource | Bound |
| --- | ---: |
| Source bytes | 1,048,576 each |
| Non-EOF tokens, including each trivia token | 100,000 each |
| Bytes in any token, including comments/whitespace | 65,536 |
| Parser syntax nodes | 100,000 each |
| Expression recursion and expression-tree height | Existing 64 limit |
| Active statement blocks | 64, including function body |
| Parameters / call arguments | 256 each |
| Qualified-path segments including crate | 34 |
| Diagnostics | 100 |
| Formatter delimiter-stack entries | 128 |
| Formatter-owned annotation/work-table logical payload | 8 MiB |
| Candidate output logical payload | 1 MiB |

Input token, node and nesting failures retain existing E0400 diagnostics/stages.
Candidate source/token/node/nesting admission failures and formatter-specific
admission failures use E0400/format and no candidate stdout. This explicitly
remaps candidate re-lex/re-parse resource errors while retaining input errors.
Run the ordinary parser with its existing bound independently for input and
candidate. The formatter must not relax parser limits to accept its own output.

For each newly introduced formatter/reader variable-size allocation, use
checked length/byte arithmetic and fallible reservation. Count candidate output
before reserving/emitting it. Existing parser/lexer allocation sites are not
converted or given new allocation-success guarantees by this slice.
Table payload accounting includes token-role annotations, token-index maps,
comment anchors and auxiliary worklists; no hidden unbounded copy of source,
comments, or token spelling. A second complete AST/source owner is permitted
only for candidate validation, under the same source/parser bounds. Those
existing parser allocations are separate from the 8 MiB formatter scratch
budget; the budget is not a whole-process RSS promise.

Require a linear implementation in input bytes + tokens + syntax nodes +
output bytes. Any lookup from AST spans to token positions must use monotone
walks or bounded indexed tables, never per-node binary searches or scanning
the full tape for each node. No recursive formatter walk beyond the fixed
delimiter stack, no repeated whole-buffer insertion, and no iteration “until
stable.” Check each count before the work/allocation it bounds. Existing
lexer/parser allocation and host allocator overhead are not reclassified as fallible by
this RFC. A host allocation failure/abort, blocking I/O or stream partial-write
guarantee is outside the counted-budget claim.

Two parses and one candidate buffer are a conscious cost of correctness for
this bounded preview. No throughput or memory-performance claim is made
before measurement. Resource fixtures must cover maximum, one-over and
candidate-expansion cases independently of formatting output.

## 8. Integration seam verified at the baseline

- `src/cli.rs`: frontend dispatch runs before the legacy runtime.
- `src/frontend/options.rs`: pure Route/Operation selection. Add a formatter
  route (path + check flag), preserve legacy argv and script payload behavior,
  and keep its diagnostics separate from source stdout.
- `src/frontend/driver.rs`: add a separate formatter path. Do not reuse
  `process_file`, which calls `ProjectSources::load_typed` and semantic
  checking. Existing report success summaries also must not be used.
- New `src/frontend/format.rs`: bounded source-to-string formatting and
  protected projection validation, with no host writes in its pure seam.
- New or narrowly shared single-source reader: bounded regular-file read,
  UTF-8 admission and source construction only. Reuse small primitives without
  importing project discovery. `SourceMap::try_add` and the existing
  allocation helper can be used directly.
- `src/frontend/lexer.rs::lex_with_limit` emits a lossless tape, and
  `src/frontend/parser.rs::parse_counted` accepts
  `SourceMode::ProjectCandidate`. Non-test wrapper functions `lex` and
  `parse` do not currently exist: the convenience wrappers are cfg(test).
  Use the real bounded seams or add narrow production wrappers.
- `src/frontend/ast.rs::Program.tokens` owns the tape; source association is
  private and checked with `belongs_to`. Preserve that association and use
  the exact same source owner to interpret spans.

Use AST context to identify unary-minus versus subtraction, borrow-place star
versus multiplication, and path colons versus type/field colons. A full CST,
public AST mutation interface, semantic dispatch or new dependency is not
required. Do not modify the legacy line-based `format_source`.

## 9. Acceptance and implementation sequence

The accompanying JSON files are hand-authored contract data. They were not
generated by a formatter, parser, test runner or an existing implementation.
Each expected output cites the rule(s) that determine its bytes.

1. Review this policy and independent exact-byte goldens first.
2. Implement the pure bounded single-file parse/format seam and protected
   projection checks with focused red/green tests.
3. Add explicit CLI routing and public subprocess tests, with read/write/tool
   traps proving no module discovery, file writes or native invocation.
4. Add independent AST normalization/equivalence, fixed-point checks, mutated
   token/comment rejection controls, limits, allocation-failure and I/O cases.
5. Run narrow checks, then applicable existing lint/build/compatibility CI on
   the exact final head; obtain fresh ordinary-host evidence.

Suggested new checks: formatter unit tests, option-route tests and one
`tests/typed_formatter.rs` public CLI test target, plus a Python reader for
the JSON fixture data if desired. They must not reconstruct an expected string
using the formatter's spacing helper.

Repository documented checks include cargo fmt, Clippy with warnings denied,
locked all-target/all-feature tests, Python script tests, release build and
`scripts/verify_repo.py`. Contributors should run the documented checks for
changed code and report their exact commands and results.
The initial design-only checkpoint ran no compilers or tests. The implementation
adds ordinary unit/integration tests and `scripts/verify_typed_formatter.py` for
the unchanged hand-authored goldens; exact-head results must be reported separately.

Target ordinary-host evidence: Linux x86_64, Windows x86_64, macOS x86_64 and
macOS arm64, following the published ordinary CI host set. Output gap LF
behavior must be byte-identical across hosts. There is no new native-target or
hardware dependency.

## 10. Compatibility, alternatives and deferred work

There is no grammar, edition identifier, artifact encoding, ABI or semantics
change. The CLI gains one opt-in operation; existing default/legacy fmt
continues to write as before. Callers wishing to capture typed output must
redirect stdout themselves. No migration is required.

Rejected for this slice: extending the legacy character/line formatter (it
cannot safely distinguish comment braces), printing an AST (loses original
spelling/comments), requiring typechecking (blocks incomplete modules), and
whole-project formatting (adds discovery and write-policy risks).

Deferred: structural line splitting/joining, width-based wrapping, import
sorting, comment reflow/control directives, JSON diagnostics, stdin, ranges,
LSP edits, files/directories/globs, in-place/atomic writes, configuration,
additional grammar, source execution and performance claims.

Only published scalar/struct/module grammar is in scope. This RFC and its
fixtures have no dependency on unpublished language or test-package work.
