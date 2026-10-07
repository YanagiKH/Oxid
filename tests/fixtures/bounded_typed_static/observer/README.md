# Canonical static observation precursor

This is comparison tooling, not a candidate implementation, provider, native
admission result, or public ABI. Canonical files are copied byte-for-byte. The
standalone build adds unchanged `typeck.rs` to the existing parser observer's
canonical closure. The parser adapter prefix is reused unchanged and checked by
the builder. Existing canonical source visibility and accessors are preserved.

## Corrected observation routes and scope

The observer preserves the full existing successful OPA1 grammar. Original
single-file scalar syntax follows actual `hir::resolve` and `typeck::check`.
Unknown types use the actual `ast::Program::uses_owned_syntax` selector and an
explicit `public_route_required` marker. `observe.py` completes that
diagnostic-only route with the independently qualified public CLI.

A top-level `pub fn` sets project syntax. The initial bounded build incorrectly
sent it through the original-source resolver, which rejects the AST with E0500
while the public CLI accepts it. That build, source snapshot and failing
comparison are preserved as historical evidence; they are not a usable full-
domain oracle. The corrected build uses the exact `ast.uses_project_syntax()`
selector after the owned selector, producing a `scalar_project_facts` marker.
The controller then invokes the same observer with `--project-source` in its
fresh evidence directory containing the exact `stdin.ox` bytes.

The project observer compares file bytes with stdin before loading, then calls
unchanged `ProjectSources::load_typed("stdin.ox", ProjectLimits::default())`.
It checks the retained source name and exact bytes, single root module and
agreed AST subset again. The real retained source/AST pair is passed through
`SourceOwner::project`, `hir::resolve_sources` and `typeck::check`. The controller
also compares the complete reloaded AST projection to its initial projection.
No source identity, owner, declaration index or syntax flag is fabricated.
Library semantics remain: no entry gate, lowering or execution is invoked.

The copied loader explicitly returns for a root without declared child modules
before applying discovery host policy. Consequently root-only public functions
have no Linux-only loader gate. Declared-child loading retains its canonical
Linux policy and remains outside the agreed scalar subset. This is a source-
level policy observation; these focused runs execute on Linux and do not claim
a cross-platform runtime qualification.

Typed HIR projection is complete for every scalar HIR field. This remains
focused canonical tooling: no STF1 serialization, candidate row mapping,
semantic implementation or full grammar-wide candidate qualification is added.

## Build and observation

Build into a fresh directory:

```
python3 scripts/build_typed_static_observer.py --output FRESH_BUILD
```

The observer accepts raw stdin of at most 128 ASCII bytes. Its source display
name is exactly `stdin.ox`. Transport-domain failure exits 2 with empty stdout.
Language success, a diagnostic, outside-subset syntax, or a route marker exits
0 with one JSON object; a route marker is never semantic acceptance. The builder
retains unchanged source bytes and hashes, wrapper-only diff, compiler version,
exact rustc arguments, build streams/status, binary hash, and artifact hashes.

The controller accepts the same stdin and writes it unchanged into a fresh
evidence directory. A project-scalar marker uses `--project-source` as described
above; a missing or changed file is an internal observation failure (exit 70),
not a language diagnostic. For unknown-type route markers only, it invokes:

```
QUALIFIED_OXID check stdin.ox --edition typed-preview --message-format json
```

It retains both raw process streams, invocation/status/identity receipts, and
the complete public diagnostic list plus check summary. Its selected diagnostic
is the first exact public JSON record, without path rewriting or field removal.
The controller rejects unexpected acceptance, stage/code escape, process
failure, summary disagreement, and missing CLI authority. It does not implement
owned resolution or attempt to synthesize expected diagnostics. CLI
qualification is supplied externally, not asserted from an arbitrary path.

```
python3 tests/fixtures/bounded_typed_static/observer/observe.py \
  --observer FRESH_BUILD/canonical-static-observer \
  --canonical QUALIFIED_OXID --evidence FRESH_CASE < source.ox
```

## Observation schema

All objects have `schema: "canonical-static-observation-1"`. Status variants:

- `ok`: `route: "scalar" | "project_scalar"`, complete inherited `ast`, and `typed_hir`
- `diagnostic`: `phase: "parse" | "resolve" | "type"`, exact `diagnostic`
- `lexical_diagnostic`: `phase: "lex"`, exact `diagnostic`
- `outside_subset`: excluded successful-AST `family` and `span`
- `public_route_required`: `route: "owned"`, `purpose: "diagnostic_only"`,
  `source_name: "stdin.ox"`, and complete inherited `ast`; alternatively
  `route: "project_scalar"`, `purpose: "scalar_project_facts"`, the same source
  name and complete AST

The controller completes owned markers with a diagnostic observation carrying
`route: "public_owned"`, and project-scalar markers with the authentic project
static observation. It never emits partial typed facts after a failing
phase. The lexical/parser boundary and all accepted/excluded AST families remain
the existing parser observer's boundary. Diagnostic fields are exactly
`Diagnostic::render_json`, including complete primary/secondary spans and notes.

`typed_hir.functions` is actual definition order. Each object has:

- `id`, exact source `name`, `signature` with name `span`, ordered `params` and
  `result`; type strings are exactly `bool`, `i32`, `()`
- canonical root `body` block ID and function closing-brace `end`
- complete `locals`: `id`, exact source `name`, `mutable`, declaration `span`,
  optional primitive `annotation`, actual checked `ty`
- complete `expressions`: `id`, canonical `span`, checked `ty`, and all variant
  fields described below
- complete `blocks`: `id`, `span`, closing-brace `end`, actual checked `flow`,
  and source-ordered statement `body`

Every HIR expression variant is observed exhaustively:

| Kind | Fields besides id/span/ty |
| --- | --- |
| Bool / I32 | `value` |
| Unit | none |
| Local | `local` LocalId |
| Call | `target` DefId, ordered expression `args` |
| Group | `operand` ExprId |
| Negate / Not | `operand`, `operator_span` |
| Logical / Comparison / Arithmetic | canonical `op`, `left`, `right`, `operator_span` |

Every HIR statement retains `span` and all variant fields:

| Kind | Fields |
| --- | --- |
| Let | `local`, `init` |
| Assign | `local`, `target_span`, `operator_span`, `value` |
| Expr / Return | `value` (null only for valueless Return) |
| Break / Continue | actual LoopId `target` |
| While | `loop_id`, `condition`, `body` |
| If | `condition`, `then_block`, optional `else_block` |

DefId follows function order. LocalId, HIR ExprId, BodyBlockId and LoopId are
function-local. They are zero-based canonical identities, not physical AST row
references. Null is absence; ID zero is never absence. Spans use the inherited
`file_id`, half-open `start` and `end` structure. Integer values include MIN and
zero without sentinels or lossy conversion.

Each `flow` retains `debug`, `mask`, and all four actual booleans `fallthrough`,
`returns`, `breaks`, `continues`. The wrapper accepts only the sixteen exact
`FlowSummary { fallthrough: BOOL, returns: BOOL, breaks: BOOL, continues: BOOL }`
representations, with fixed field order, spelling and whitespace. It rejects
Debug format drift. Bits are F=1, R=2, B=4, C=8. This decodes the existing checker
value; it does not inspect statements to recalculate flow or construct expected
semantics. Root blocks are checked by the existing TypedProgram view invariants.

## Mapping to the proposed row-column contract

The retained AST supplies original spelling, syntax, spans, statement positions
and graph edges. HIR signatures supply function IDs/result types; local
declaration spans identify parameter/let rows and supply LocalIds/types;
local/call references supply exact declaration identities; integer expressions
supply checked signed values; expression types supply every semantic expression
role; and block views supply actual F/R/B/C masks. HIR transfer targets refer to
the nearest while-body block, whose AST row can be identified structurally.

The future projection must map identities and validate them, never resolve
names, convert literals, infer types, or infer flow to fill candidate facts.
There is no STF1 encoder/decoder, AST-row column mapper, candidate comparison,
grammar-wide qualification or production source-association claim here yet.
The existing OPA1 observer, protocol and all candidate source files are untouched.

## Focused checks

`test_observer.py --observer ... --canonical ... --evidence FRESH_DIR` retains
every input and stream. It checks complete public diagnostic JSON equality,
all 14 contracted diagnostic templates, phase/child/target precedence, unknown
type conflict/annotation ordering and 64/65-byte names, representative complete
HIR facts, per-function ID restart, MIN/MAX/zero, mutability, sibling scopes,
actual multi-exit flow including mask 15, nested loop targets, empty input,
recursion, no main, parameterized main, and syntax/transport distinctions.
This is focused schema validation, not exhaustive semantic qualification. The
public-function test detected the initial route defect and remains a regression
check for the corrected project route. Source mismatch/missing-file controls
ensure the project route cannot silently observe different bytes.
