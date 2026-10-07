# Canonical scalar-function parser observer

This wrapper observes the real, unchanged Rust typed-preview parser. It is not
an Oxid parser implementation and is not a production compiler entrypoint.
Build with `python3 scripts/build_typed_parser_observer.py --output FRESH_DIR`.
The executable reads raw stdin, allows at most 128 ASCII bytes, and prints one
JSON object. An input-domain violation exits 2 with no JSON; it is not a language
diagnostic. Input bytes and the `stdin.ox` display path are not normalized.

The wrapper uses the actual `SourceMap::try_add`, default project `Allocator`,
`lexer::lex_with_limit` with `MAX_TOKENS`, and `parser::parse_typed_counted` with
`ProjectCandidate` and `MAX_NODES`. That entrypoint enables the current array,
enum and std-import syntax policies. It does not resolve, type-check, execute,
load a module or run a provider. Those policies are deliberately not reduced to
make the observer agree with the bounded consumer.

## Results and schema

- Success: `{"status":"ok","ast":{...}}`
- Parse failure: `{"status":"diagnostic","projection":"first_parser_diagnostic","diagnostic":...}`
- Lexer failure: `{"status":"lexical_diagnostic","diagnostic":...}`
- Canonical parse success outside this observation domain:
  `{"status":"outside_subset","projection":"successful_ast_outside_subset","family":"...","span":...}`

A diagnostic is exactly the first actual `Diagnostic::render_json` result; it
retains all renderer fields, including secondary spans and notes. There is no
partial AST, diagnostic message replacement, synthetic error or recovery-list
comparison. A lexical failure remains distinct and precedes parsing.

Every syntax span is `{"file_id":0,"start":N,"end":N}`, with half-open byte
bounds. IDs are the actual zero-based Rust arena indices. `null` means absence;
ID 0 never means absence. The AST object has these fields:

- `tokens`: the complete canonical token tape, including trivia and EOF. Each
  token is `{"id":K,"kind":"KindName","file_id":0,"start":N,"end":N}`;
  `id` is the lexer kind discriminator plus one, not a token index.
- `items`: source-ordered `{"kind":"Function","id":function_index}` entries.
- `functions`: source-ordered objects with `id`, `public` (span or null), `name`,
  `params`, `result`, `body`, `blocks`, `end`. Parameters have `name` and `ty`.
  `body` is the actual per-function block ID. Blocks have `id`, complete `span`,
  closing-brace `end`, and source-ordered statement `body`. The function `end`
  is its canonical closing-brace span, not an invented full-function span.
- `expressions`: the complete global expression arena in actual ID order.
  Every expression has `id`, complete `span`, `kind` and all fields below.

Types are `{"kind":"TypeName","span":...,"name":...}` for unqualified
names, including unknown names, or `{"kind":"TypeUnit","span":...}`.

Each statement has `span` and `kind`, plus exactly these variant fields:

| Kind | Fields |
|---|---|
| Let | `mutable`, `name`, `annotation` (type or null), `init` expression ID |
| Assign | `name`, `operator_span`, `value` expression ID |
| Expr | `value` expression ID |
| Return | `value` expression ID or null |
| Break, Continue | none |
| While | `condition` expression ID, `body` block ID |
| If | `condition`, `then_block`, `else_block` (block ID or null) |

Each expression has these variant fields:

| Kind | Fields |
|---|---|
| Number | `digits` span, `negative` boolean; no decimal conversion |
| Bool | `value` boolean |
| Unit | none |
| Name | `name` span |
| Call | `callee` unqualified-name span, `args` ordered value expression IDs |
| Group | `operand` expression ID |
| Negate, Not | `operand`, `operator_span` |
| Arithmetic, Comparison, Logical | `op` (canonical Rust variant name), `left`, `right`, `operator_span` |

Arithmetic `op` is Add/Subtract/Multiply/Divide/Remainder; comparison `op` is
Equal/NotEqual/Less/LessEqual/Greater/GreaterEqual; logical `op` is And/Or.
The parser's transient expression heights and counted resource totals are not
retained AST fields and are not synthesized. Source identity and derived private
feature summaries are not syntax serialization. The admitted grammar has empty
record/enum/module/import/path arenas; successful nonempty excluded arenas are
reported outside the subset, never silently dropped.

## Outside the scalar observation domain

The agreed bounded grammar is defined by
`fixtures/typed-parser-samples/CONTRACT.md`. This observer does not independently
parse its grammar-site refusal rules. Only after the real parser succeeds does
it inspect the retained AST for excluded families. The returned span is a full
canonical excluded-node span, not a claimed grammar starter token. Malformed
excluded constructs still return the real first canonical diagnostic.

Excluded successful syntax includes module/import/record/enum declarations;
qualified/reference/array/slice-reference/array-reference types; field/index
assignment and match statements; qualified calls/values; borrow arguments;
record and array literals; field reads, indexing and array length; and any
otherwise-unexpected qualified path arena. Family strings are respectively
`module`, `import`, `record`, `enum`, `qualified_type`, `reference_type`,
`array_type`, `slice_reference_type`, `array_reference_type`, `field_assignment`,
`index_assignment`, `match`, `qualified_call`, `qualified_value`,
`borrow_argument`, `record_literal`, `array_literal`, `field_access`, `indexing`,
`array_length`, `qualified_path`, `qualified_path_segment`.
The boundary chooses the first excluded item in item order, then function
parameter/result/statement types or statements in arena order, then expressions
in arena order, and finally residual path storage. This is an observation-domain
label, not a parser error-priority claim.

## Reproducibility and scope

The builder snapshots and hashes 20 unchanged canonical files, records the exact
checkout head using isolated, read-only Git with only this resolved checkout in
`safe.directory`, then invokes standalone rustc. The std-import catalog requires
HIR/declaration-index/owned-type definitions at compile time, so the unmodified
closure includes them and their source owner/project types. They are not called
by the observation entrypoint. The only custom Rust files are this observation
adapter, `main.rs`, and an OIR module-wiring adapter. No source is patched and no
catalog/type stand-ins are supplied. All added wrapper bytes are recorded in
`wrapper-only.diff`, independently hashed, and compiled from the same snapshot.
Receipts include toolchain, command, elapsed build time, status, binary hash and
all artifact hashes. The builder requires a fresh output directory.

Run the focused wrapper checks with
`python3 tests/fixtures/bounded_typed_parser/observer/test_observer.py --observer FRESH_DIR/canonical-parser-observer --report REPORT.json`.
These test projection fidelity and boundary reporting. They are not a full Cargo
matrix, whole-language qualification, or a parser implementation parity claim.
