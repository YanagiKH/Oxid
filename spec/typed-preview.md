# Experimental bool/unit typed checking

Status: experimental, check-only. `typed-preview` is a provisional selector,
not a final language edition or a completed M1/M2 milestone. The scoped design
and review boundaries are recorded in [RFC 0001](../rfcs/0001-typed-preview-check.md),
[RFC 0002](../rfcs/0002-verified-straight-line-oir.md) and
[RFC 0003](../rfcs/0003-boolean-branch-cfg.md).

## Command and compatibility boundary

```sh
oxid check input.ox --edition typed-preview
oxid --edition=typed-preview check input.ox --message-format=json
oxid check --edition typed-preview -- --dash-prefixed.ox
```

With no edition option, existing commands retain their legacy behavior.
`--edition legacy-0.9` explicitly selects that same route. Both `--option value`
and `--option=value` work for edition and message format. These two options may
appear before the command, between the command and source, or after the source.
Duplicate, missing, empty, or unknown values fail; they are never ignored.

`--` ends option recognition. For typed checking it must follow `check`, and all
later words are literal operands; exactly one source path is required. A typed
option written after `--` is an operand, not an edition selection. With no new
options, legacy arguments, including any separator, are forwarded unchanged.

There is one deliberate process-argument boundary: all arguments after
`script <name>` belong to that manifest script, so edition-like words there are
passed through. To select an Oxid edition for a script command, place the option
before its name. An explicit typed selection there is rejected before launching
anything. This prevents accidentally consuming an external process's options.

Only `check` supports the preview. Explicit typed `run`, direct-file invocation,
`compile`, `ast`, project commands, and every other operation fail before legacy
dispatch. The gate runs before interpreter construction, preprocessing,
dependency resolution, script execution, cache writes, or artifact generation.
Preview checking reads only the requested source, creates no output/cache files,
and never executes it. It rejects OXBC input and does not fall back to the legacy
parser, dynamic values, macro expander, interpreter, or artifact writer.

`--message-format text|json` is available only with typed preview. Unknown
preview options, including backend/target/profile requests, are rejected.
Manifest edition propagation and typed project builds are not implemented.
Explicitly choosing a legacy command on a source remains possible; the selector
is not a file-carried or project-wide edition marker.

## Grammar

```text
file       := function*
function   := "fn" name "(" parameters? ")" "->" type block
block      := "{" statement* "}"
parameters := name ":" type ("," name ":" type)*
type       := "bool" | "(" ")"
statement  := "let" name (":" type)? "=" expression ";"
            | expression ";"
            | "return" expression? ";"
            | "if" expression block ("else" block)?
expression := "true" | "false" | name | "(" ")" | "(" expression ")"
            | name "(" arguments? ")"
arguments  := expression ("," expression)*
```

Identifiers are case-sensitive ASCII letters/underscore followed by ASCII
letters/digits/underscore. Keywords are reserved. Whitespace, `//` line comments,
and non-nested `/* ... */` comments are retained as trivia and ignored by the
parser. Unicode is permitted in comments; Unicode identifiers are not supported.
Trailing commas and legacy concise-keyword aliases are not supported.

The only types and values are `bool` and unit `()`. A file may contain no
functions and does not require `main`; checking is not execution.

- Function parameters and return types must be explicit
- Locals are immutable and initialized immediately, with an optional annotation
  or inferred bool/unit type; the initializer sees only earlier locals
- Functions are collected before resolving bodies, so forward direct calls and
  recursive calls resolve; no termination or executable-recursion claim follows
- Function names are unique. Parameters, function bodies and branch arms have
  lexical scopes. Bindings cannot duplicate a name in the same scope, shadow an
  active ancestor, or shadow a top-level function. Sibling arms and declarations
  following a closed child scope may reuse names; every declaration has its own ID
- An arm-local is visible only after its initializer and within that arm or its
  descendants. It cannot escape to a sibling or the surrounding block
- A bare name denotes a local; functions are not first-class values
- Calls must resolve to declared functions and match their arity and types
- There are no builtins or implicit conversions
- Every function, including unit functions, requires explicit returns on every
  reachable intraprocedural path. `return;` and `return ();` return unit. Statements
  after a return or an if with two returning arms are rejected
- An if condition must be bool. Both arms are checked even for literal conditions.
  A missing else leaves a reachable empty false path. Empty arms are legal. An if
  is a statement with no value or trailing semicolon; else requires braces, so
  else-if, if-expressions and naked block statements are unavailable
- Expression statements may discard either supported type

Example:

```text
fn identity(value: bool) -> bool {
    let answer = value;
    return answer;
}
fn main() -> () {
    identity(true);
    return;
}
```

Numbers retain their exact source spelling but have no accepted numeric
semantics. Strings, null, imports/modules, macros, mutation, borrowing, ownership,
containers, loops and other control flow, operators, async, closures, generics, FFI, host I/O,
and undeclared builtins are unavailable. Recognized unsupported syntax produces
E0101; other invalid syntax produces E0100 or a resolution error. There is no
silent approximation or legacy execution of these features. Now-recognized if/else
keywords in invalid positions produce ordinary syntax errors (E0100), replacing
the predecessor's unsupported-keyword E0101 for those newly enabled keywords.

## Compiler representation

The production path is UTF-8 source → lossless token tape → spanned AST →
resolved HIR → typed HIR → verified OIR. It lives in `src/frontend/` independently of the legacy
syntax module and runtime. The token tape retains trivia and invalid tokens; it
is not a complete formatter/LSP CST. Parsing synchronizes at the next top-level
`fn`, with a diagnostic limit; erroneous ASTs never enter name resolution.

AST names are source spans. HIR replaces value uses with function/local IDs,
allocated deterministically in source order. Local IDs belong to one function.
Signatures resolve before bodies. Successful typed construction has one supported
type for every expression and local; incomplete tables cannot be constructed
outside the checking pass. AST/HIR statement blocks use arenas and ID edges.
Each typed body also records every block's return flow. Lexical resolution and
statement checking use explicit traversal frames. Type inference walks child-before-parent arena entries,
not an unbounded recursive chain. There is no runtime `Value` in this pipeline.

The old AST, parser, lexer and lexical helpers now live in
`src/legacy/syntax.rs`. The extraction preserves their behavior and OXBC 1.0
encoding. The existing single-binary Cargo package and Rust edition are unchanged.

## Verified acyclic OIR

Every successful production check lowers actual typed bodies and passes an
independent OIR verifier before returning success. Immutable typed views expose
complete types and block return flow; there is no reparsing or legacy adapter.
Raw IR is private to `src/frontend/oir/`; only successful verification constructs
the immutable witness used by the driver. No public IR loader, dump, stable
serialization, execution, optimization or ownership contract is added.

Function-local slots contain bool or unit and are classified as parameters,
bindings or expression temporaries. Assign evaluates a bool/unit constant or
copies a typed operand. Call has a direct DefId, ordered arguments, result slot
and one normal continuation. Return uses an explicitly initialized operand.
Branch has a bool operand and two successors; Goto has one successor. Calls in
conditions and arms remain explicit terminators, never hidden in Branch.

Lowering traverses structured source bodies in lexical depth-first order with a
single expression cursor. Conditions lower before Branch; then/else expressions
lower only in their respective paths. A join exists only if some arm falls
through or else is absent. Falling arms end in Goto(join); returning arms do not.
An absent else branches directly to the join. Two returning arms produce no
synthetic join. Table order is not topological: joins may be reserved before
arm-call continuations. No fake return, phi or implicit merge value is introduced.

Verification validates aggregate bounds, every signature/reference/type/span and
terminator before following graph edges, including unreachable raw blocks.
Reachability and deterministic topological traversal reject unreachable blocks
and cycles. Equal Branch targets are legal and their edge multiplicity is handled
consistently. Direct and mutual recursive call graphs remain legal because calls
to other function entries are not intraprocedural edges.

Parameters are entry definitions and cannot be overwritten. Every other local
has at most one definition globally, including definitions in mutually exclusive
arms. An unused undefined slot is legal; every read needs a dominating definition.
Within one block an assignment must precede a read, including its own RHS. A call
result requires strict dominance by its call block, so it is unavailable in its
own arguments/statements or at a join reachable by bypassing that call. Canonical
single-definition rules are limited to this immutable no-phi preview.

The verifier computes a dominator tree in topological order using predecessor
lowest-common-ancestor queries and bounded binary lifting, then iterative tree
intervals for constant-time dominance queries. There is no blocks-by-locals
matrix, per-block initialization-set cloning or recursive graph traversal.

This proves structural intraprocedural return completeness assuming calls return
normally. It does not establish termination, user-stack safety, executable call
safety, ownership, memory safety or source-to-IR equivalence merely from spans.

All declarations, blocks, assignments, operands and terminators retain original
source spans. Existing function-entry, call-continuation, copy/use and bare-return
origins are preserved. Branch uses its full if statement; its operand uses the
condition expression. Arm entries use full source blocks; arm-end Gotos use their
closing braces. Synthetic joins use the full if statement. Exact Unicode/CRLF
provenance is tested separately from valid file/range/UTF-8 boundaries.

See [RFC 0003](../rfcs/0003-boolean-branch-cfg.md) for the bounded decision and
[RFC 0002](../rfcs/0002-verified-straight-line-oir.md) for its predecessor.

## Source and diagnostic contract

Each immutable source has a file ID, display path, original UTF-8 bytes and
line-start index. Spans are half-open byte ranges `[start, end)` with that file
ID, including empty EOF spans. Internal span constructors check bounds and UTF-8
boundaries. Offsets are stored as `usize`; source size is bounded before parsing.

Line/column positions are one-based Unicode scalar counts, not display-cell or
LSP UTF-16 counts. LF starts a line, including in CRLF; the CR counts as a scalar
on the preceding line. Tabs and combining marks each count as one scalar. No
source normalization or macro rewriting changes reported offsets.

Text errors go to stderr, with code/stage, primary location, optional secondary
labels and notes. Successful text checking writes a check-only summary to stdout.
JSON mode writes newline-delimited records only to stdout and leaves stderr
empty for ordinary frontend errors:

- Diagnostic: `schema_version: 1`, `edition: "typed-preview"`,
  `kind: "diagnostic"`, `severity: "error"`, `code`, `stage`, `message`,
  `primary`, `secondary`, `notes`
- A location contains `file_id`, `path`, `start`, `end`, `line`, `column`,
  `end_line`, `end_column`. Primary is null for CLI/read/UTF-8/artifact failures
- Secondary labels have `{ "span": location, "message": string }`
- Terminal record: `kind: "check-summary"`, the same schema/edition fields,
  `success`, `errors` (emitted count), and `functions` (count on success, otherwise null)

The JSON envelope is provisional version 1. It is also used for malformed CLI
options when JSON was selected before the separator/script-payload boundary.
It does not imply the requested edition was valid. JSON strings escape all C0
control characters, quotes and backslashes. Text rendering escapes control
characters instead of emitting source-controlled terminal commands.

| Code | Meaning |
| --- | --- |
| E0001 | Invalid CLI options or unavailable operation |
| E0002 | Source file read failure |
| E0003 | Invalid UTF-8 source |
| E0004 | Legacy OXBC input unavailable |
| E0100 | Invalid or incomplete lexical/syntax input |
| E0101 | Recognized unsupported preview syntax |
| E0200 | Unresolved local or direct function name |
| E0201 | Duplicate binding or unsupported shadowing |
| E0202 | Unknown type |
| E0300 | Binding, argument, condition or return type mismatch |
| E0301 | Call arity mismatch |
| E0302 | Missing explicit terminal return |
| E0303 | Statement after terminal return |
| E0400 | Frontend/lowering resource limit |
| E0500 | Internal OIR lowering/verification invariant failure |

Exit 0 means successful type checking, lowering and OIR verification of this
subset; ordinary source/CLI/resource failures still exit 1. Lowering budget errors
use E0400 with stage `oir-lower`. Detected OIR invariant failures use E0500,
explicitly say `internal compiler error`, use stage `oir-lower` or `oir-verify`,
and exit 2. They retain the same JSON envelope with unsuccessful summary and
`functions: null`. An invalid OIR origin is omitted (`primary: null`) rather than
passed to the asserting renderer. Only the first deterministic IR failure is
reported. The compiler does not catch arbitrary panics: earlier producer-invariant
assertions, host allocation failures and broken output pipes remain host-process
failures, not ordinary source type errors.

## Resource and trust bounds

| Resource | Current maximum |
| --- | --- |
| Input bytes | 1,048,576; reader stops after maximum + 1 bytes |
| Non-EOF tokens, including trivia | 100,000 |
| Bytes in one token, including whitespace/comment tokens | 65,536 |
| Syntax nodes counted by parser | 100,000 |
| Nested expression parser frames | 64 (63 grouping wrappers around a literal) |
| Parameters or call arguments | 256 each |
| Emitted diagnostics | 100 |
| Active statement block frames | 64, counting the function body as frame 1 |
| OIR locals and assignments | 100,000 of each, aggregate per program |
| OIR blocks | 300,000, aggregate per program |
| OIR successor edges | At most 600,000, two per block |
| Dominator ancestor cells | At most 5,700,000 usize entries (19 levels) |

All limits are engineering defaults for this experimental subset. Token limits
may be reached before source-size or node limits. Expression recursion is bounded
at parsing; statement-block nesting has a separate pre-entry bound. AST/HIR arena
ownership avoids recursive block-drop chains; later block/CFG passes are iterative.

Lowering preflights exact aggregate expansion before IR/maps are allocated. For
F functions, C calls, I if statements, P parameters, L lets, X expressions and
Rb bare returns: locals = P + L + X + Rb; assignments = X - C + L + Rb;
blocks <= F + C + 3I. These are bounded by the existing parser nodes, with only
the block budget raised to three times that limit. Old accepted source programs
are not excluded by a tighter unrelated cap. Call vectors stay capped at 256.

Verification independently checks these limits for arbitrary private raw IR and
checks graph/scratch arithmetic before allocation. A flat predecessor array and
binary-lifting ancestor table need O(locals + blocks log blocks + edges) scratch
per function; only one function's scratch is live at a time. The ancestor table
alone is at most 45.6 MB on a 64-bit host. At peak dominator construction,
ancestor cells plus offsets, predecessor edges, topological order, depth, child/
sibling links and entry/exit intervals occupy B*levels + 7B + E + 1 usize cells.
At the maxima that is 67,200,008 bytes, plus about 2,400,000 bytes for 100,000
three-word optional definition records on this host: about 69.6 MB, excluding
small vector headers, allocator overhead and raw IR/source storage. Earlier
reachability/indegree and predecessor-construction cursor arrays are already freed. Verification time is
O(functions + locals + assignments + operands + (blocks + edges) log blocks).
No host allocation-success, OS-sandbox or blocking-I/O guarantee follows.

Errors stop later compiler phases, and diagnostics beyond the cap are omitted.
No total-error-count claim is made when the cap is reached.

This constrains source parsing and forbids user-program effects during checking;
it is not a general OS sandbox or a memory-safety proof. Filesystem read blocking
and host allocation failures remain outside this resource contract.

## Evidence and deferred work

[Validation evidence](../docs/architecture/typed-preview-validation.md) identifies
the exact snapshot and actual host. Public CLI fixtures are embedded in
`tests/typed_frontend.rs` and `tests/edition_boundary.rs`, so recursive legacy
`.ox` discovery remains unchanged. Unit tests inspect source/diagnostic boundaries,
lossless numeric tokens, resolved IDs and complete type tables.

[OIR validation evidence](../docs/architecture/oir-validation.md) records the
straight-line increment, malformed-IR cases, call-order/provenance checks and
resource/long-chain tests. No ownership/borrow checking, execution engine,
numeric type system, native backend, typed artifact schema, self-hosting or AI
capability is added. [Boolean CFG evidence](../docs/architecture/boolean-cfg-validation.md)
records the later restricted branch/scope/all-path-return increment and its
dominance, graph and resource checks. Numeric semantics and reference execution
remain separate next decisions; no later roadmap capability is implied.
