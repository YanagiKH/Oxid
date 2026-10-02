# Experimental typed checking and bounded reference execution

Source-integration status: experimental. The ownership source subset uses the
production `typed-preview` route. See the
[source qualification ledger](../docs/architecture/owned-source-validation.md)
for the exact source, compiler, target and validation scope. This describes the
current repository, not support in an older released binary.

Status: experimental. Checking is non-executing; explicit run is bounded reference execution. A narrower optional [LLVM native compilation preview](native-preview.md) is also available. `typed-preview` is a provisional selector,
not a final language edition or a completed M1/M2 milestone. The scoped design
and review boundaries are recorded in [RFC 0001](../rfcs/0001-typed-preview-check.md),
[RFC 0002](../rfcs/0002-verified-straight-line-oir.md) and
[RFC 0003](../rfcs/0003-boolean-branch-cfg.md) and
[RFC 0004](../rfcs/0004-bounded-reference-execution.md) and
[RFC 0005](../rfcs/0005-exact-i32-literals.md) and
[RFC 0006](../rfcs/0006-checked-i32-arithmetic.md) and
[RFC 0009](../rfcs/0009-scalar-comparisons.md) and
[RFC 0010](../rfcs/0010-boolean-logical-operators.md),
[RFC 0011](../rfcs/0011-mutable-scalar-locals.md) and
[RFC 0012](../rfcs/0012-while-runtime-fuel.md),
[RFC 0013](../rfcs/0013-loop-control.md) and
[RFC 0014](../rfcs/0014-owned-structs-call-borrows.md).

## Command and compatibility boundary

```sh
oxid check input.ox --edition typed-preview
oxid --edition=typed-preview check input.ox --message-format=json
oxid check --edition typed-preview -- --dash-prefixed.ox
oxid run input.ox --edition typed-preview
```

With no edition option, existing commands retain their legacy behavior.
`--edition legacy-0.9` explicitly selects that same route. Both `--option value`
and `--option=value` work for edition and message format. These two options may
appear before the command, between the command and source, or after the source.
Duplicate, missing, empty, or unknown values fail; they are never ignored.

`--` ends option recognition. For typed checking/running it must follow `check` or `run`, and all
later words are literal operands; exactly one source path is required. A typed
option written after `--` is an operand, not an edition selection. With no new
options, legacy arguments, including any separator, are forwarded unchanged.

There is one deliberate process-argument boundary: all arguments after
`script <name>` belong to that manifest script, so edition-like words there are
passed through. To select an Oxid edition for a script command, place the option
before its name. An explicit typed selection there is rejected before launching
anything. This prevents accidentally consuming an external process's options.

The preview supports `check`, explicit `run`, and the separately specified
[native `compile --backend llvm`](native-preview.md). Direct-file invocation,
`ast`, project commands, and every other operation fail before legacy dispatch. The gate runs before interpreter construction, preprocessing,
dependency resolution, script execution, cache writes, or artifact generation.
Checking and reference running read only the requested source and create no output/cache files. Native compilation has its own explicit artifact boundary.
Checking never executes; run consumes only the completely verified OIR for the selected source route. It rejects OXBC input and does not fall back to the legacy
parser, dynamic values, macro expander, interpreter, or artifact writer.

`--message-format text|json` is available only with typed preview. Unknown
preview options are rejected. Backend/target/output options are available only for the native compile route; profile requests remain unavailable.
Manifest edition propagation and typed project builds are not implemented.
Explicitly choosing a legacy command on a source remains possible; the selector
is not a file-carried or project-wide edition marker.

## Grammar

```text
file           := item*
item           := function | struct_decl
function       := "fn" name "(" parameters? ")" "->" value_type block
parameters     := name ":" parameter_type ("," name ":" parameter_type)*
scalar_type    := "bool" | "i32" | "(" ")"
value_type     := scalar_type | type_name
parameter_type := value_type | "&" type_name | "&" "mut" type_name
struct_decl    := "struct" type_name "{" field_decls? "}"
field_decls    := name ":" scalar_type ("," name ":" scalar_type)* ","?
block          := "{" statement* "}"
statement      := "let" "mut"? name (":" value_type)? "=" expression ";"
                | name "=" expression ";"
                | name "." name "=" expression ";"
                | expression ";"
                | "return" expression? ";"
                | "break" ";" | "continue" ";"
                | "if" expression block ("else" block)?
                | "while" expression block
expression     := logical_or
logical_or     := logical_and ("||" logical_and)*
logical_and    := comparison ("&&" comparison)*
comparison     := sum (comparison_op sum)?
comparison_op  := "==" | "!=" | "<" | "<=" | ">" | ">="
sum            := product (("+" | "-") product)*
product        := unary ("*" unary)*
unary          := "!" unary | primary
primary        := "true" | "false" | name | "(" ")" | "(" expression ")"
                | name "(" arguments? ")" | decimal | "-" decimal
                | name "." name | type_name "{" field_inits? "}"
decimal        := ASCII_DIGIT+
field_inits    := name ":" expression ("," name ":" expression)* ","?
arguments      := argument ("," argument)*
argument       := expression | borrow_argument
borrow_argument := "&" name | "&" "mut" name
                 | "&" "*" name | "&" "mut" "*" name
```

Identifiers are case-sensitive ASCII letters/underscore followed by ASCII
letters/digits/underscore. Keywords are reserved. Whitespace, `//` line comments,
and non-nested `/* ... */` comments are retained as trivia and ignored by the
parser. Unicode is permitted in comments; Unicode identifiers are not supported.
Optional trailing commas are confined to struct field declarations and literal
initializers. Function parameters and call arguments still reject trailing
commas. Legacy concise-keyword aliases are not supported.

In an unparenthesized `if`/`while` condition, the restriction on struct literals
applies through the whole top-level precedence expression: its body brace is not
a constructor brace. Parentheses and direct-call arguments admit literals again;
the resulting condition must still be bool. A field projection has exactly the
form `name.field`; `(s).field`, `(*p).field`, `make().field`, chained fields and
methods are unavailable. A borrow is the complete direct-call argument, never a
general expression or parenthesized place. Standalone block statements are not
part of this grammar.

Scalar values are `bool`, `i32` and unit `()`. The owned addition admits nominal
move-only structs with only those scalar fields, including empty structs.
Reference types occur only in function parameters. A file may contain no
functions and does not require `main`; checking is not execution. `main` may
return an owned type when checking, but run and native compile require a
zero-argument main returning bool/i32/unit.

- Function parameters and return types must be explicit
- Locals are initialized immediately, with an optional annotation or inferred
  scalar or owned value type; the initializer sees only earlier locals. Ordinary `let` and
  parameters are immutable; `let mut` enables later same-type statement assignment
- Functions are collected before resolving bodies, so forward direct calls and
  recursive calls resolve. Run uses isolated iterative activations; no termination
  claim follows and recursive programs can exhaust execution limits
- Function names are unique. Parameters, function bodies, branch arms and while bodies have
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
- Expression statements may discard any supported type

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

### Exact decimal i32 literals

One or more ASCII decimal digits, optionally preceded by one minus token, denote
an exact i32 in [-2147483648, 2147483647]. Existing trivia may separate minus and
digits: `- /* comment */ 2147483648` is valid. Leading zeroes are decimal, not
octal. `0`, `0000`, `-0` and `-0000` all print `0`. An explicit i32 context and
an unconstrained literal (`let x = 1`) both produce i32; bool/unit contexts give
E0300, with no coercion or truthiness. Other widths are E0202 unknown types.
This provisional single-width default establishes no promotion algorithm.

The sign is literal-only: `(-2147483648)` works, while `-(1)`, `-x`, `-f()`,
`--1` and `+1` remain E0101 unsupported. Suffixes, separators, radices, floats,
exponents and non-ASCII digits (`1i32`, `1_000`, `0xff`, `0o7`, `0b1`, `1.0`,
`1e9`, `１`) are E0101/parse. The parser validates the complete numeric token
before conversion; a long invalid suffix is not misreported as a range error.
Digits beyond the i32 range produce E0203/resolve at the complete literal span,
including sign/trivia. A 65,536-byte all-zero token is valid; many digits alone
are not a range error. Full-file checking includes unused functions and unchosen
branches. Exact integer conversion never uses f64 or a bigint.

### Checked i32 arithmetic

Binary `+`, `-` and `*` require i32 operands and return i32. `*` binds more tightly
than `+`/`-`; each level associates left. Parentheses override precedence. The
left operand is fully evaluated before the right, then the operation executes.
Calls execute exactly once in that order, and the first error stops execution.
`1--2` subtracts the signed literal -2; general unary negation is still unavailable.

Every operation checks its exact result against the i32 range. Overflow is
E0604/oir-run at the operator's one-byte source span, with exit 1 and no partial
result. Host debug and release builds behave identically. No folding, wrapping,
saturation, widening or reassociation occurs: `2147483647 + 1 - 1` and
`0 * (2147483647 + 1)` both overflow. Checking those expressions succeeds without
execution; out-of-range literals still fail E0203 before running. Unchosen arms
do not execute, but every arm is name/type checked. Wrong arithmetic operands
produce E0300 at the first wrongly typed operand. Discarded arithmetic still runs.
The bounded [native preview](native-preview.md) supports these same three checked
operations, with the reference human overflow diagnostic and exit 1. Its stricter
whole-file admission bounds and native I/O failure status 74 still apply.

Division, remainder, casts, shifts, explicit wrapping,
other numeric types and their overflow rules remain unavailable. Literal range
validity remains a separate compile-time rule.

### Scalar comparisons

`==` and `!=` accept matching i32 or matching bool operands. `<`, `<=`, `>` and
`>=` accept i32 only and use signed order. Every result is bool. Unit equality,
mixed-type operands, bool ordering and implicit conversions are unavailable.
E0300 points to the first invalid operand; all declarations and branches are
checked. Equality first validates the left type as i32/bool, then requires the
right type to match it. Ordering requires i32 at each operand.

All six comparisons share one non-associative tier below arithmetic.
`1 + 2 < 4 * 2` is valid. `(1 < 2) == true` explicitly compares bool results.
An unparenthesized second comparator, as in `1 < 2 < 3` or `1 == 2 < 3`, fails
E0100/parse at the second operator's full span. Parenthesized comparisons used
where i32 is required instead fail type checking. The multibyte operators must
be adjacent; comments/whitespace do not join separate punctuation tokens.

Operands execute exactly once, fully left before right, then the comparison
executes. No comparison short-circuits, including discarded results and bool
equality. Operand arithmetic can fail E0604 before comparison; comparison itself
cannot overflow. Unchosen branches remain unexecuted. The reference and bounded native backends share
this table and order; see [RFC 0009](../rfcs/0009-scalar-comparisons.md).

### Boolean logical operators

`!` accepts bool and returns its inverse. `&&` and `||` require bool/bool and
return bool. `a && b` evaluates a once and skips b if a is false; `a || b` skips
b if a is true. Otherwise b is evaluated once and becomes the result. Both
operands are always resolved and type checked, even a statically skipped RHS.
There is no truthiness or conversion from i32/unit. Executed work preserves
left-to-right, first-error and discarded-expression rules. Skipped RHS calls,
arguments and overflow operations have no runtime effect.

`!` binds above arithmetic/comparisons, then `&&`, then `||`. Binary logical
operators associate left at each tier, and prefix `!` nests right. `!a == b`
means `(!a) == b`; `!1 < 2` fails E0300, while `!(1 < 2)` is valid. Existing
comparison chaining restrictions remain. `1 < 2 && 2 < 3` is valid.
Adjacent `!=` remains one token; separated `& &`, `| |`, standalone bitwise
operators and textual `and`/`or` are unavailable. Newly recognized `!` in invalid
grammar positions receives ordinary E0100 rather than unsupported E0101.
See [RFC 0010](../rfcs/0010-boolean-logical-operators.md).

### Initialized mutable scalar locals

`let mut x (: type)? = expression;` introduces an initialized mutable bool/i32/unit
local with a fixed type. `x = expression;` is a statement, not an expression, and
requires an existing mutable bare-name target. Evaluate the RHS completely once
before storing; errors or exhausted store fuel perform no write. Existing if
branches can mutate outer places. Reads, immutable copies and call arguments are
scalar snapshots, and each activation has independent state. Both branches and
all RHSs remain statically checked. Existing lexical scope/shadowing rules apply.

This scalar-local increment adds no uninitialized declaration, mutable parameter,
compound/index assignment, parenthesized target or assignment expression.
Scalar-field assignment on an owned struct is specified separately below. Invalid
positions for `mut` and `=` now receive E0100; immutable targets receive E0304 at
the name with a declaration label, unknown targets E0200, and fixed-type mismatch
E0300 at the RHS with a declaration label. See
[RFC 0011](../rfcs/0011-mutable-scalar-locals.md).

### Ordinary while and operation budget

`while condition { statements }` reevaluates a bool condition before each iteration;
false skips its body. Body-local declarations initialize freshly on every dynamic
execution, using reusable activation slots. Outer mutable places persist and
immutable snapshots remain values. Conditions and bodies are fully checked even
when never executed. Return exits the function, but while itself never proves a
terminal return, even for literal true. No trailing semicolon, while-else,
loop expression or implicit truthiness is introduced.

Reference and guarded native invocations share a total 1,000,000-operation budget
across loops/calls, with E0601 before the next charged operation. This is not a
wall-clock limit or final performance mode. Existing costs and frame/slot limits
remain; loop execution does not accumulate frame storage. See
[RFC 0012](../rfcs/0012-while-runtime-fuel.md) for exact costs, origins, cyclic
verification and native guarded-only representation bounds.

### Unlabeled loop transfers

`break;` exits the nearest enclosing while in the same function. `continue;`
reevaluates its full condition, including calls and lazy logic. Both are
semicolon-only statements, with no values or labels. Statements after any
unconditional transfer, or an if whose arms all transfer, fail E0303 even when
arm outcomes differ. All source paths remain statically checked. Nested while
consumes its own transfers; conservative while false edges and explicit function
return requirements remain unchanged.

Typed block summaries distinguish fallthrough, return, break and continue.
Only falling paths create join edges. In scalar-only modules each transfer lowers to one existing,
precharged Goto at its full statement span, without a subsequent closing-brace
edge. No value/place slots, OIR blocks, runtime frame allocation, new verifier
algorithm or resource limit is added. Out-of-loop transfers give E0204/resolve;
malformed/value/labeled transfer statements give E0100/parse. See
[RFC 0013](../rfcs/0013-loop-control.md).

### Nominal owned structs and call-only borrowing

The owned contract is [RFC 0014](../rfcs/0014-owned-structs-call-borrows.md).
Type names have a separate namespace from functions/locals; fields belong to
their declaring struct. All declarations are collected before bodies. Types are
nominal: equal layouts do not make two structs interchangeable. A literal names
every field exactly once, with an exactly matching scalar expression. Fields
evaluate once in written source order, then construction occurs. There are no
defaults, shorthand fields, spread/update syntax or implicit conversions.

| Form | Contract |
| --- | --- |
| `let b = a;`, `consume(a)`, `return a;`, discarded `a;` | Move the complete owned value; the source becomes unavailable |
| `a.field` or reference parameter `p.field` | Copy a scalar snapshot without moving the owner |
| `a.field = rhs;` | Evaluate RHS first, then require an available mutable owner or exclusive reference permission |
| `let mut a = expr; a = replacement;` | Evaluate replacement fully, then replace the complete value of the same nominal type |
| Reinitialize a moved mutable owner | Allowed by whole replacement; field writes cannot restore a moved owner |
| `a = a;` or `a = relay(a);` | Move the available RHS to separate temporary storage, then replace a |
| `&a` / `&mut a` as an entire call argument | Borrow a complete available owner; exclusive borrowing also requires a mutable binding |
| `&*p` / `&mut *p` as an entire call argument | Explicitly reborrow a reference parameter; exclusive permission requires an exclusive parent |

Empty structs still move. Owned parameters are immutable bindings and may be
moved/returned but cannot be rebound or directly mutated. An exclusive reference
parameter has an immutable binding and permits mutation of its referent.
Unavailable owners fail on reads, moves and borrows. Availability must hold on
every reaching path, including loop backedges and continue. A terminating arm
does not contribute state to a successor it cannot reach. Whole reinitialization
restores availability. Loop-local lifetimes restart each iteration; break,
continue, return and normal block exits end the storage they leave. These are
logical lifetime operations, not user destructors.

Arguments are prepared immediately, left to right: scalar snapshots are saved,
owned arguments move, and borrows acquire their loans before the next argument
runs. Loans last until the owning call returns normally, including evaluation
of later arguments and nested calls. Shared loans may alias each other and allow
reads; they prohibit writes, moves, replacement and exclusive borrowing of that
owner. An exclusive loan excludes every other overlapping loan and direct owner
access. Discarding an argument in the callee does not release its loan early.

Reference argument modes match exactly. An `&T` parameter requires `&a` or
`&*p`; an `&mut T` parameter requires `&mut a` or `&mut *p`. Neither `read(&mut a)`
for `read(p: &T)` nor `write(&a)` for `write(p: &mut T)` is a coercion: both fail
E0300. To obtain shared access from an exclusive reference parameter, explicitly
write `&*p`. Bare forwarding `callee(p)` and `callee(&p)` are unavailable.
Repeated shared children of an exclusive parent are permitted; while they live,
the parent permits only reads and more shared reborrows. An exclusive child
suspends all parent access. A shared parent cannot grant exclusive access.

For example, `take(a.field, &mut a)` saves the earlier scalar before acquiring
the loan; `take(&mut a, a.field)` conflicts at the later read. Likewise,
`a.field = mutate(&mut a);` can complete its inner loan before the final write,
whereas `a.field = consume(a);` fails when the final target is unavailable.
Earlier moves and writes are not rolled back if a later operation fails; an
unpaid store performs no write.

Nested owned fields, partial moves, destructuring, aggregate equality, stored or
returned references, scalar/field/temporary borrows, general dereference,
reference coercions, mutable parameter bindings, heap resources and user
destructors remain unavailable.

The [Batch pilot](../fixtures/owned_source/batch.ox) combines nested loops,
break/continue, exclusive reborrowing, shared inspection and owned returns.
Its scalar result is 816. Its command contract is:

```sh
oxid check fixtures/owned_source/batch.ox --edition typed-preview
oxid run fixtures/owned_source/batch.ox --edition typed-preview
oxid compile fixtures/owned_source/batch.ox --edition typed-preview \
  --backend llvm --output ./batch
```

The source program has its own lowering/fuel schedule; the historical raw
adaptation's 1,086 fuel is not its cost.

Strings, null, imports/modules, macros, heap containers, for/loop and other
control flow, other operators, async, closures, generics, FFI, host I/O,
and undeclared builtins are unavailable. Recognized unsupported syntax produces
E0101; other invalid syntax produces E0100 or a resolution error. There is no
silent approximation or legacy execution of these features. Now-recognized if/else/while/break/continue
keywords in invalid positions produce ordinary syntax errors (E0100), replacing
the predecessor's unsupported-keyword E0101 for those newly enabled keywords.
The owned parser also recognizes struct/field/borrow syntax. Previously
unsupported forms may therefore receive more precise errors: an `&bool`
parameter changes from E0101/parse at `&` to E0202/resolve at `bool` because only
record referents are supported. This does not enable scalar borrowing. Accepted
scalar-only programs retain their behavior; byte-identical diagnostics are not
promised for every formerly unsupported ownership token sequence.

## Compiler representation

The compiler path is UTF-8 source → lossless token tape → spanned AST →
resolved HIR → typed HIR → verified OIR. The source integration selects one
route for the entire parsed module. Any struct declaration, non-scalar named
type annotation/signature, reference parameter, struct literal, field access or
borrow argument selects owned HIR and owned OIR for every function. This
includes unused declarations and statically skipped paths; comments containing
owned spellings do not select that route. Unknown nominal names also select it
and fail resolution. Scalar-only modules retain the existing scalar pipeline,
diagnostics, costs and native admission. There is no per-function mixture and
no fallback after an owned parse, resolution, type, verification, runtime or
native-admission failure. It lives in `src/frontend/` independently of the legacy
syntax module and runtime. The token tape retains trivia and invalid tokens; it
is not a complete formatter/LSP CST. Parsing synchronizes at the next top-level
`fn` or `struct`, with a diagnostic limit; erroneous ASTs never enter name resolution.

AST names are source spans. Numeric AST nodes store exact digits spans and a
literal-only sign; resolution uses checked signed integer accumulation to create
representable i32 HIR constants, retaining full literal origins. MIN is accumulated
negatively, never by negating an unrepresentable positive i32. HIR replaces value
uses with function/local IDs,
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

## Verified scalar OIR with cyclic control flow

Every successful check or run lowers actual typed bodies and passes the
independent verifier for its selected route before returning success. This
section describes scalar-only modules; owned modules have the additional
authoritative availability/loan verification described below. Immutable typed views expose
complete types and block return flow; there is no reparsing or legacy adapter.
Raw IR is private to `src/frontend/oir/`; only successful verification constructs
the immutable witness used by the driver. No public IR loader, dump, stable
serialization or optimization contract is added by the scalar representation. The bounded reference
consumer below accepts only that immutable verified witness.

Function-local value slots contain bool, i32 or unit and are classified as
parameters, immutable bindings or expression temporaries. Mutable places have a
separate typed declaration table and PlaceId namespace. A mutable binding uses
one place, without also allocating an immutable binding slot. Assign evaluates a bool/i32/unit constant or
copies a typed operand, computes CheckedI32 from two ordered i32 operands, or
computes CompareScalar from the explicit scalar comparison type table, or
computes NotBool from a bool operand into a bool destination, or snapshots a
mutable place through Load. Ordered block statements distinguish SSA Assign,
Initialize(place, value) and Store(place, value); stores define no SSA value.
Lowering completes the whole initializer/RHS before its initialization/store.
CheckedI32 retains its operator origin separately from the full assignment span.
Both arithmetic operands must have dominating initialized definitions; its
destination must be i32. CompareScalar independently checks each operand ID,
span and allowed type pair, both dominating initialized definitions and a bool
destination. Equality permits i32/i32 or bool/bool; ordering permits i32/i32 only.
Call has a direct DefId, ordered arguments, result slot
and one normal continuation. Return uses an explicitly initialized operand.
Branch has a bool operand and two successors; Goto has one successor. Calls in
conditions and arms remain explicit terminators, never hidden in Branch.

Lowering traverses structured source bodies in lexical depth-first order using
root-directed expression continuations and a checked postorder completion cursor. Conditions lower before Branch; then/else expressions
lower only in their respective paths. A join exists only if some arm falls
through or else is absent. Falling arms end in Goto(join); returning arms do not.
An absent else branches directly to the join. Two returning arms produce no
synthetic join. Table order is not topological: joins may be reserved before
arm-call continuations. Statement-if joins introduce no value. Logical expressions
reserve two blocks (RHS and join), evaluate RHS only on its required path and
define one explicit BoolMerge at the join from the selected left/right input.
The existing expression temporary is the result; no opposite-arm writes occur.

Verification validates aggregate bounds, every signature/reference/type/span and
terminator before following graph edges, including unreachable raw blocks.
Reachability rejects unreachable blocks; arbitrary reachable cycles, including
irreducible raw graphs, are independently verified. Equal Branch targets are legal and their edge multiplicity is handled
consistently for non-merge targets. A BoolMerge requires exactly two distinct
incoming edges matching its input predecessors and is forbidden at function entry.
Direct and mutual recursive call graphs remain legal because calls
to other function entries are not intraprocedural edges.

Parameters are entry definitions and cannot be overwritten. Every other local
has at most one definition globally, including definitions in mutually exclusive
arms. An unused undefined slot is legal; every read needs a dominating definition.
Within one block an assignment must precede a read, including its own RHS. A call
result requires strict dominance by its call block, so it is unavailable in its
own arguments/statements or at a join reachable by bypassing that call. Canonical
single-definition rules also cover BoolMerge destinations. Each fixed two-input
merge independently requires bool destination/input types, valid IDs and origins,
and availability of each input on its own predecessor edge. A call result is
available on that call's normal edge into a merge, never in its own arguments.
The merge destination is available from block entry and in dominated blocks.

Every place has exactly one canonical initialization in its separate table,
including unused places. Initializer/store inputs must be available SSA values.
Each load/store requires earlier same-block or dominating cross-block
initialization. Opposite-arm, self-initialization and pre-init access fail;
stores never establish initialization or weaken unique SSA definitions.
All place IDs/types/origins and store operator spans are checked structurally.

The verifier uses iterative Lengauer–Tarjan simple link/eval with path compression,
then iterative dominator-tree intervals for constant-time dominance queries. There is no blocks-by-locals
matrix, per-block initialization-set cloning or recursive graph traversal.

Every block has a valid terminator; the source checker separately enforces its
conservative return rule. Cyclic raw IR need not reach any Return. It does not establish termination, user-stack safety, executable call
safety, ownership, memory safety or source-to-IR equivalence merely from spans.

All declarations, blocks, assignments, operands and terminators retain original
source spans. Existing function-entry, call-continuation, copy/use and bare-return
origins are preserved. Branch uses its full if statement; its operand uses the
condition expression. Arm entries use full source blocks; arm-end Gotos use their
closing braces. Synthetic joins use the full if statement. Exact Unicode/CRLF
provenance is tested separately from valid file/range/UTF-8 boundaries. For logical
expressions, Branch, RHS-ending Goto, merge and join block use the full expression;
RHS entry uses its operand expression, and both input operands retain their spans.
NotBool and BoolMerge also retain their exact operator origins. Place declarations
and targets retain name spans; Load uses its name-expression span. Initialize
and Store use full statement spans, Store also retains `=`, and their input
operands retain complete initializer/RHS spans.

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
| E0203 | Exact decimal literal outside i32 range (resolve stage) |
| E0204 | Loop transfer outside a while in the same function (resolve stage) |
| E0300 | Binding, argument, scalar/logical operand, condition or return type mismatch |
| E0301 | Call arity mismatch |
| E0302 | Missing explicit terminal return |
| E0303 | Statement after terminal return or loop control transfer |
| E0304 | Mutation or exclusive borrowing of an immutable binding |
| E0305 | Unknown projected field or projection on a non-record binding (type stage) |
| E0310 | Owned value unavailable on a reaching path (ownership stage) |
| E0311 | Access conflicts with an active loan (ownership stage) |
| E0312 | Unsupported reference value use or forwarding form (type stage) |
| E0313 | Shared reference does not permit exclusive access (ownership stage) |
| E0400 | Frontend/lowering resource limit |
| E0500 | Internal OIR lowering/verification/execution invariant failure |
| E0600 | Missing or invalid zero-argument main for run |
| E0601 | Execution fuel exhausted |
| E0602 | Live call-frame limit exceeded |
| E0603 | Live local-slot limit exceeded |
| E0604 | Checked i32 arithmetic overflow at its operator |
| E0605 | Owned execution-plan, expanded-cell, requested-byte or allocation limit (oir-owned-run stage) |

For check, exit 0 means successful type checking, lowering and OIR verification of this
subset; for run it additionally means a bool/i32/unit result (including false, zero and negatives); ordinary source/CLI/resource failures still exit 1. Scalar lowering budget errors use E0400 with stage `oir-lower`; owned source
lowering/verification resource errors use E0400 with stage `oir-owned-lower` or
`oir-owned-verify`. Detected OIR invariant failures use E0500,
explicitly say `internal compiler error`, use the relevant lowering, verification or execution stage,
and exit 2. They retain the same JSON envelope with unsuccessful summary and
`functions: null` for check, or `result: null` for run. An invalid OIR origin is omitted (`primary: null`) rather than
passed to the asserting renderer. Only the first deterministic IR failure is
reported. The compiler does not catch arbitrary panics: earlier producer-invariant
assertions, host allocation failures and broken output pipes remain host-process
failures, not ordinary source type errors.


Owned source failures preserve resolve → type → OIR shape → ownership order.
Unknown/duplicate literal fields are E0200/E0201 at resolution; missing fields
are E0300 at the literal; projected-field lookup is E0305 during typing. E0310,
E0311 and E0313 are derived from authoritative verifier denial context, including
the operation, access role, subject and state. A bad internal temporary, cleanup
event or malformed IR stays E0500; a source span alone does not make it a user
ownership error. Primary locations identify the rejected operation and related
labels identify a valid causal move/loan and declaration. When several independent
owner roots fail, the selected complete diagnostic is deterministic; it need not
be the earliest failure across all roots in textual order.

New owned diagnostic builders retain at most 1,024 message bytes, two 256-byte
labels and two 256-byte notes: at most 2,048 text bytes per diagnostic and 204,800
at the existing 100-diagnostic cap. Names retain at most 64 bytes including any
`...` suffix, cut at a UTF-8 boundary. Missing-field messages show at most the
first eight names in declaration order plus the omitted count. These bounds
apply only to new owned diagnostic text. They exclude preexisting shared
lexer/parser formatting, vector/String capacities, headers, allocator overhead,
rendered JSON/human output, path escaping and total RSS. They are not a bound on
all diagnostic bytes emitted by the frontend.

## Bounded explicit run

The complete file must pass the same compiler/verifier path before execution,
including statically erroneous unchosen branches. Run then requires a declared
`fn main() -> bool`, `fn main() -> i32` or `fn main() -> ()`. Missing main is E0600 without a location;
main parameters produce E0600 at the main name, and an owned result also fails E0600. Checking itself has no entry
requirement. The compiler carries main's resolved DefId; the runner inspects the
verified signature instead of reconstructing names from OIR spans.

On text success stdout is exactly `true\n`, `false\n`, `()\n` or a canonical
signed decimal i32 followed by newline; each exits 0. Numeric results are never
used as process exit codes.
Failure writes no partial result. JSON replaces the check-summary with one
`run-summary`: the same schema/edition fields, `success`, `errors`, and `result`
containing `{ "type": "bool", "value": true|false }`, `{ "type": "unit" }`,
`{ "type": "i32", "value": -2147483648 }` (an exactly serialized JSON integer),
or null on failure. No successful check record precedes a failed run. Errors while
validating global edition/format options retain the existing check-summary;
once valid global options select explicit typed run, command-option/operand,
compile, entry and execution errors use run-summary. Check records are unchanged.

In scalar-only modules, each activation has isolated optional bool/i32/unit value and place arrays.
Initialize executes a place's unique static declaration and resets its reused
storage on reexecution; Store requires an initialized place and Load snapshots it
into a unique static SSA value. Nonparameter definitions can execute repeatedly;
parameters remain immutable. A merge reads its input before replacing its result. A failed RHS performs no store. Assign reads before
writing; Branch executes exactly one arm; Goto uses its explicit target.
Calls copy arguments in recorded order and suspend callers until normal return,
then replace that static call result's current value and resume at the explicit continuation. Bare
returns contain explicit unit. Discarded calls still run. There is no folding,
tail-call elimination, memoization or execution of an unchosen arm. Fresh calls
and repeated invocations share no mutable execution state. The activation stack
is iterative; the host call stack does not track source recursion.

For scalar-only modules the execution ceilings are 1,000,000 fuel units, 1,024
live frames and 200,000 live slots. The scalar costs and examples in this section
apply to that route; they must not be reused as owned-module fuel totals. An i32 counts as one slot, with the same instruction costs as bool/unit.
These are slot counts, not bytes; host scalar sizes are measured in the
[i32 validation report](../docs/architecture/i32-literal-validation.md).
Main counts as a frame and every activation counts its complete value+place tables,
including places declared in unchosen arms. Root allocation costs 1 + combined
slot count. Assign/Initialize/Store/Branch/Goto/Return cost 1 each; a Load is one
assignment. `let mut x = 1; x = 2; return x;` has four slots and eleven fuel;
a unary/arithmetic/comparison assignment or block-entry bool merge costs one
(in addition to operand evaluation), charged before reading any operand or
checking overflow. A merge reads only its selected incoming operand and needs
no extra slot or variable-sized scratch. Short-circuiting skips RHS instruction
and call costs, but whole-function allocation includes its temporary slots.
Ungrouped `return !true;` costs 6; `return false && true;` / `return true || false;`
cost 8; `return true && false;` / `return false || true;` cost 10.
For example, `return 1 + 2;`, `return 1 < 2;` and `return true == false;` each
cost exactly eight including root allocation/return;
Call costs 1 + argument count + callee combined value/place slot count. Costs are charged before work.
Checked cost/fuel, frame count and live-slot count are checked in that order,
before allocation. Returning releases the callee's slots. Argument scratch is
bounded by 256 scalar entries; frame-header capacity by the fixed frame cap.

E0601–E0603 point to the next unperformed operation, or main's name for root
allocation. Counter overflow/invariant failure is E0500, distinct from resource
exhaustion. No entry override, program arguments or budget flags are exposed.
Fuel measures deterministic reference work, not wall-clock time, source-level
complexity or a stable profiling ABI. Source reads, compiler work, allocation
success and output-pipe behavior are not bounded by fuel. This is not an OS
sandbox. Full details are in [RFC 0004](../rfcs/0004-bounded-reference-execution.md).

## Owned verification, execution and accounting

Owned source lowering is a producer, never a proof. It emits explicit owner
storage, construction/transfer/replacement/discard, field operations and ordered
call/loan events. The independent raw verifier checks declarations, scalar CFG
shape/dominance, availability and exact call/loan regions before constructing the
sealed immutable owned witness. Reference and native consumers require that
same witness through an immutable witness-bound plan. No mutable raw program,
standalone plan or source-side acceptance summary can authorize execution.

For each function, let S count scalar locals and mutable places, A all call
argument descriptors, O all owner slots, R incoming references, L loans, C call
sites, and P the sum of `max(1, field_count)` for every owner. B is the checked
aligned owner arena, including parameters, locals, expression temporaries,
argument staging, call results and inter-owner padding. All declared storage is
counted, including unused/skipped work. On the qualified x86_64 representation:

```text
X = S + A + P + 4O + 8R + 12L + 2C
Dref = 8(S+A) + B + 32O + 64R + 96L + 16C
```

Reference execution retains 1,000,000 fuel, 1,024 live frames and 200,000 live
scalar slots, and additionally caps live X at 200,000 and requested runtime
storage at 16 MiB. Requested bytes include reserved frame-header capacity,
active Dref and one 8-byte scalar scratch cell. On the qualified representation
each header is 272 bytes; these formulas are not portable layout promises.
Execution-plan metadata is separately capped at 32 MiB. Owned return storage is
already reserved in the caller's owner arena; no extra variable return scratch
is hidden. Loops reuse activation storage with checked owner generations.

The owned ledger charges before work. Let w be `max(1, field_count)` of the
affected owner and r be the call's number of borrowed arguments:

| Event | Fuel |
| --- | ---: |
| Root activation | `1 + X(root)` |
| Scalar statement/merge, Branch/Goto, StorageLive, field read/write, scalar/borrow preparation | `1` |
| Construct, move-initialize, discard, StorageEnd, owned preparation | `1 + w` |
| Whole replacement | `1 + 2w` |
| OpenCall | `1 + owned_argument_count` |
| Invoke | `1 + argc + X(callee) + sum(owned_argument_widths) + r(r-1)/2` |
| Scalar return | `1 + P + L + C + R` |
| Owned return | `scalar_return_charge + w(returned_owner)` |

Return's R term accounts for normal-edge loan release. Explicit lexical storage
ends and frame teardown are distinct charged events, even for moved owners.
Source lowering may introduce several of these events for one expression; a
source move is not one fuel unit. Owned groups forward their result owner without
another owner/event; owned name expressions move through explicit temporaries.
Scalar expressions in owned modules still preserve scalar snapshots and ordered
checked arithmetic, but whole-module calls/storage use the owned ledger.

Activation admission checks fuel, frames, scalar slots, expanded cells, bytes,
then allocation. An exhausted charge performs no part of the operation. Earlier
effects remain; abrupt failure promises no rollback, normal-return loan release,
destructor execution or unwinding. Root errors retain entry origins and call
activation errors retain Invoke origins. Existing scalar overflow retains its
operator span.

## Resource and trust bounds

| Resource | Current maximum |
| --- | --- |
| Input bytes | 1,048,576; reader stops after maximum + 1 bytes |
| Non-EOF tokens, including trivia | 100,000 |
| Bytes in one token, including whitespace/comment tokens | 65,536 |
| Syntax nodes counted by parser | 100,000 |
| Nested expression parser frames and total expression-tree height | 64 (63 grouping wrappers/operators above a literal) |
| Parameters or call arguments | 256 each |
| Emitted diagnostics | 100 |
| Active statement block frames | 64, counting the function body as frame 1 |
| Scalar OIR combined value/place slots and instructions (including merges) | 100,000 of each, aggregate per program |
| OIR blocks | 300,000, aggregate per program |
| OIR successor edges | At most 600,000, two per block |
| Dominator ancestor cells | At most 5,700,000 usize entries (19 levels) |

All limits are engineering defaults for this experimental subset. Token limits
may be reached before source-size or node limits. Expression recursion and total tree height are bounded
at parsing; this includes flat left-associative chains, so 64 literal terms pass
and 65 terms fail E0400. Height tracking uses one usize per expression, at most
800,000 bytes on a 64-bit host (excluding vector capacity). Statement-block nesting has a separate pre-entry bound. AST/HIR arena
ownership avoids recursive block-drop chains; later block/CFG passes are iterative.

Scalar-only lowering preflights exact aggregate expansion before IR/maps are allocated. For
F functions, C calls, I if statements, W while statements, S logical expressions, P parameters,
L lets (including M mutable), A reassignment statements, X expressions and Rb bare
returns: values = P + (L - M) + X + Rb; places = M; combined slots = P + L + X + Rb;
instructions (including merges/init/store/load) = X - C + L + A + Rb;
blocks <= F + C + 3I + 3W + 2S. These are bounded by the existing parser nodes, with only
the block budget raised to three times that limit. Old accepted source programs
are not excluded by a tighter unrelated cap. Call vectors stay capped at 256.

Verification independently checks these limits for arbitrary private raw IR and
checks graph/scratch arithmetic before allocation. Iterative cyclic dominance
uses O(locals + blocks + edges) scratch and O((blocks + edges) log blocks) graph
work. Peak dominator/predecessor tables use 11B + E + 1 usize cells, 31,200,008
bytes at B=300,000/E=600,000 on a 64-bit host, plus about 2.4MB for at most 100,000
combined optional value-definition/place-initialization records. This excludes
raw IR/source, vector headers and allocator costs; only one function's scratch
is live at once. Full verification also walks functions/slots/instructions/operands.
No host allocation-success, OS-sandbox or blocking-I/O guarantee follows.

Owned source additionally applies the following inclusive preflight limits:

| Resource | Maximum |
| --- | ---: |
| Nominal record declarations | 4,096 |
| Fields per record / aggregate fields | 1,024 / 65,536 |
| Checked declaration-table payload / sum of padded declaration layouts | 8 MiB / 1 MiB |
| Aggregate scalar value/place plus owner slots | 100,000 |
| Aggregate statements plus merges / expanded ownership events | 100,000 / 100,000 |
| Aggregate blocks | 300,000 |
| Counted ownership-analysis work | 100,000,000 |
| Ownership metadata / peak ownership-flow scratch | 32 MiB / 32 MiB |
| Requested source-produced raw payload | 64 MiB |

Raw verification independently recounts actual nested vectors, rather than
trusting source counts. Expanded events include statements/merges, call argument
descriptors and preparation sites, and constructed field operands. Work is a
checked inventory-based bound, not CPU instructions or a timeout. Declaration,
raw-output, verifier, plan and consumer ledgers are separate admissions.

The source producer counts before reserving raw output or maps. It makes three
count traversals and one emission traversal per function; count-only cleanup
uses a saved lexical-prefix length rather than walking every exited owner. All
new variable output/map reservations are fallible and append/count parity is
checked in release builds. The raw ledger includes each nested payload and
its containing headers once, including diagnostic-origin fields.

On the qualified x86_64 representation, one active function's requested
producer-map payload is `8B + 16N + 40E + 8O` bytes, where B is its block count,
N its binding count, E its expression count and O its owner count (these B/E
symbols are local to this map formula). Conservative independent source maxima
give 8,800,000 bytes; this does not assert all maxima are jointly attainable.
Fixed iterative traversal capacities are 203 expression frames, 268 body frames
and 65 loop targets. Inclusive source nesting needs at most 127 expression
frames, 253 body frames and 63 loop targets. Their measured fixed representation
is 33,872 bytes, not a compiled-machine-stack or total-memory bound.

Source raw/map ledgers exclude source/AST/typed frontend storage, new bounded
diagnostic text and rendering, checked declaration tables, verifier/plan/consumer
storage, allocator overhead or excess capacity, LLVM processes and total RSS.
The raw 300,000-block resource fixture exceeds the source 64 MiB payload ceiling;
it is raw-only evidence, not a promise that a source program reaches that limit.

Errors stop later compiler phases, and diagnostics beyond the cap are omitted.
No total-error-count claim is made when the cap is reached.

This constrains source parsing and forbids user-program effects during checking;
it is not a general OS sandbox or a memory-safety proof. Filesystem read blocking
and host allocation failures remain outside this resource contract.

## Evidence and deferred work

[Validation evidence](../docs/architecture/typed-preview-validation.md) identifies
the exact snapshot and actual host. Predecessor public CLI fixtures are embedded in
`tests/typed_frontend.rs` and `tests/edition_boundary.rs`. The owned Batch pilot
is registered in `scripts/verify_repo.py` for explicit typed check/run with
`--edition=typed-preview`. Other discovered source files retain their legacy
checks. `tests/typed_owned_boundary.rs` covers the public owned entry and early
failure boundaries; actual final counts and results are recorded in the source
qualification ledger. Unit tests inspect source/diagnostic boundaries,
lossless numeric tokens, resolved IDs and complete type tables.

[OIR validation evidence](../docs/architecture/oir-validation.md) records the
straight-line increment, malformed-IR cases, call-order/provenance checks and
resource/long-chain tests. The straight-line and boolean-CFG increments did not add execution. No
ownership/borrow checking, numeric type system, native backend, typed artifact
schema, self-hosting or AI capability is added by the reference runner. [Boolean CFG evidence](../docs/architecture/boolean-cfg-validation.md)
records the later restricted branch/scope/all-path-return increment and its
dominance, graph and resource checks. The subsequent [reference execution evidence](../docs/architecture/reference-execution-validation.md)
records the bounded opt-in runner and its independent source oracle. The later
[i32 literal evidence](../docs/architecture/i32-literal-validation.md) adds exact
decimal constants/copies, strict i32 types and result serialization. Broader
numeric operations remain separate decisions. [Checked i32 arithmetic evidence](../docs/architecture/i32-arithmetic-validation.md)
records the subsequent +/−/* increment and its runtime overflow policy; no later
roadmap capability is implied.

[Scalar comparison evidence](../docs/architecture/scalar-comparison-validation.md)
records the explicit i32/bool comparison table, non-associative grammar and
reference/native/Python checks. No generalized equality or later operator is implied.

[Boolean logic evidence](../docs/architecture/boolean-logic-validation.md) records
short-circuit value joins, exact path costs and reference/native differential checks.

[While evidence](../docs/architecture/while-validation.md) records cyclic verifier, shared fuel and actual LLVM gates.

[Loop-control evidence](../docs/architecture/loop-control-validation.md) records source transfers and precise fallthrough/exit qualification.


[Owned source qualification](../docs/architecture/owned-source-validation.md)
records preactivation versus actual production CLI results, exact compiler/source
identities, resource gates and source-free native execution. The historical
[raw-consumer report](../docs/architecture/owned-consumers-validation.md) remains
evidence for its own raw fixtures and frozen snapshot. Its counts, fuel and
artifact hashes are not source-integration results. Ownership foundations are
one experimental capability; stored-reference lifetimes, partial moves,
field-disjoint loans, heap/drop semantics, unsafe/FFI contracts and complete
static-core or v1.0 qualification remain open.
