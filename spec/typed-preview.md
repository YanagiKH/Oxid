# Experimental bool/unit typed checking

Status: experimental, check-only. `typed-preview` is a provisional selector,
not a final language edition or a completed M1/M2 milestone. The scoped design
and review boundary are recorded in [RFC 0001](../rfcs/0001-typed-preview-check.md).

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
function   := "fn" name "(" parameters? ")" "->" type "{" statement* "}"
parameters := name ":" type ("," name ":" type)*
type       := "bool" | "(" ")"
statement  := "let" name (":" type)? "=" expression ";"
            | expression ";"
            | "return" expression? ";"
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
- Function names are unique. Parameters and locals cannot duplicate another
  binding in that function or shadow a top-level function name. Different
  functions may use the same parameter/local names
- A bare name denotes a local; functions are not first-class values
- Calls must resolve to declared functions and match their arity and types
- There are no builtins or implicit conversions
- Every function, including a unit function, requires an explicit terminal
  return. `return;` and `return ();` both return unit. Statements after that
  return are rejected
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
containers, control flow, operators, async, closures, generics, FFI, host I/O,
and undeclared builtins are unavailable. Recognized unsupported syntax produces
E0101; other invalid syntax produces E0100 or a resolution error. There is no
silent approximation or legacy execution of these features.

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
outside the checking pass. Type inference walks child-before-parent arena entries,
not an unbounded recursive chain. There is no runtime `Value` in this pipeline.

The old AST, parser, lexer and lexical helpers now live in
`src/legacy/syntax.rs`. The extraction preserves their behavior and OXBC 1.0
encoding. The existing single-binary Cargo package and Rust edition are unchanged.

## Verified straight-line OIR

Every successful production check lowers the actual typed bodies and passes an
independent OIR verifier before returning the success summary. Immutable typed
views expose recorded types to lowering; there is no reparsing or legacy adapter.
The raw IR is private to `src/frontend/oir/`; only successful verification creates
the opaque, immutable verified result used by the driver. There is no public IR
loader, dump, stable serialization, execution command, or typed artifact.

Each function has typed body-local slots for parameters, bindings and expression
temporaries. Distinct HIR/OIR local types and explicit lowering maps separate the
phases. Numeric slot IDs are body-relative, not owner-tagged identities: a valid
index transplanted from another body is not distinguishable by bounds alone.
Function IDs follow declaration order. Parameter types are the initial local
prefix, so calls and definitions share one checked signature representation.

The only statements are assignments of bool, unit or a copied operand. Each
basic block has exactly one terminator: a direct call with a result destination
and one explicit normal continuation, or return of an initialized operand.
Calls returning unit and discarded results remain in the IR. Bare return creates
an initialized unit temporary. There is no fallthrough or omitted call result.
The block builder rejects appending after closure and closing twice; the raw
representation permits a missing terminator only so it can be detected.

Lowering is iterative over the existing child-before-parent expression arena.
It preserves source-statement order and evaluates call arguments left-to-right
in the IR, retaining group copies and every call. This records the current IR
sequence; checking does not execute it or settle evaluation rules for future
constructs. A function with C call expressions produces exactly C + 1 blocks.
No optimization, branch, join, unwind edge, ownership operation or drop exists.

The verifier first checks aggregate bounds, function IDs/signatures, all source
spans and all blocks, including unreachable blocks. It checks local/function/
continuation references, parameter-prefix kinds, call arity, assignment/call/
return types, and terminator presence before following edges. It then walks the
unique entry continuation chain with one initialized-local bitmap and one
visited-block bitmap per function. Parameters begin initialized. Every read must
follow initialization; each other slot may be initialized only once and parameters
cannot be overwritten. Call arguments are read before the destination becomes
available on its normal continuation. Cycles and unreachable blocks are rejected.
Acyclic chains may reference any block-table order; table order is not execution
order. No per-block local-state matrix or recursive graph walk is used.

This establishes structural intraprocedural return completeness, assuming every
call returns normally. Direct and mutual recursion remain valid. A callee's entry
is not the caller's continuation edge. This is not program termination, executable
call safety, user-stack safety, memory safety, or ownership/borrow checking.

Functions, local declarations, blocks, assignments, operands and terminators carry
original source spans. Copies and arguments retain use-site spans. Entry blocks
use the function name; call continuations use their originating call; synthesized
bare-return units use the return statement. The verifier validates file identity,
range and UTF-8 boundaries before rendering any failure. Exact provenance is
tested separately; in-bounds spans alone do not prove source-to-IR equivalence.

The bounded design and acceptance scope are in
[RFC 0002](../rfcs/0002-verified-straight-line-oir.md).

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
| E0300 | Binding, argument or return type mismatch |
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
| OIR locals, blocks, assignments | 100,000 of each, aggregate per program |

All limits are engineering defaults for this experimental subset. A source can
hit a token limit before its source-size or node limit. Long flat unsupported
expressions fail before creating deep trees. Resolution recurses only through
AST expressions already bounded by the parser; type checking is iterative.
Lowering preflights its exact expansion with checked arithmetic before allocating
IR storage/maps. Locals map to distinct counted parameters, expressions, let
statements or bare returns; assignments map to non-call expressions, lets or bare
returns; blocks map to functions plus calls. Each total is bounded by the existing
100,000 parser-node budget, so the new bounds do not reduce the accepted source
subset. Call vectors retain the 256-argument cap. Verification independently
checks these bounds, even for malformed raw IR. Lowering/verification take linear
time in functions + locals + blocks + assignments + operands; verification uses
O(locals + blocks) scratch space per function. These are engineering bounds,
not an allocation-failure guarantee.

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
capability is added. The next separately reviewed dependency is boolean branches,
scopes, joins and general CFG dataflow; it is not complete in this slice.
