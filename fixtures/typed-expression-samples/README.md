# Bounded expression parser and evaluator

This component is written in typed-preview Oxid. An iterative parser
builds a fixed expression arena, then a separate iterative evaluator consumes
that tree. `main.ox` parses `12 + 3 * (4 + 5)`, checks that it built seven nodes,
and returns 39. It is not compiler self-hosting or a language extension.

```sh
oxid check fixtures/typed-expression-samples/main.ox --edition typed-preview
oxid run fixtures/typed-expression-samples/main.ox --edition typed-preview
oxid compile fixtures/typed-expression-samples/main.ox --edition typed-preview \
  --backend llvm --output ./expression-example
./expression-example
```

## Input and storage contract

The input grammar is decimal integers, `+`, `*`, and parentheses. Multiplication
binds more tightly than addition; both associate left. ASCII space, tab, CR and
LF are ignored between tokens. Unary signs, implicit multiplication, other
operators and identifiers are rejected. Integer spellings may have leading zeros.

Input is limited to 128 character codes. The arena stores at most 15 nodes;
operator/parenthesis and operand stacks each independently hold at most 15
entries. Parentheses do not allocate nodes. Eight literals and seven binary
operators fit the node bound, but parenthesis depth has its own stack bound.
Every append and push checks capacity before its array write. Limits are
component-level constants; compiler ceilings are unchanged.

`parse(codes, arena)` resets the arena's logical count and returns `Parsed(root)`
or a position-carrying error enum. Positions are zero-based input-code indexes;
EOF uses the input length. Any failed parse leaves the arena unusable until the
next parse resets it. Columns remain initialized, and entries beyond count are
not read. The original `tests/fixtures/bounded_enum_scanner` is unchanged; this
sibling scanner adds `*`, `(`, `)` and token-start positions.

`parse_prefix(codes, used, arena)` scans only indexes below `used`, including all
whitespace, digit and EOF decisions. It resets the arena before validating the
length. Negative lengths and lengths beyond the backing slice return
`InputLimit(used)`; an otherwise valid length above 128 returns `InputLimit(128)`.
No unused cells are padded or interpreted. The original `parse` entry delegates
with `codes.len()`, preserving its whole-input contract. The scanner likewise
retains `next_token`, with `next_token_prefix` providing the used-length entry;
an invalid prefix or cursor range returns `Invalid(-1)` before indexing.

Arena columns are `kind`, `value`, `left`, `right`, and `count`. Kinds 1 (integer),
2 (addition), and 3 (multiplication) are application data, not Oxid enum tags.
Literal children are -1; binary nodes have unused value 0. Children precede
parents, and a successful root is the final node. Helpers borrow whole records.

`evaluate(arena, root)` first validates the complete postorder tree, including
count/root, node kinds, field conventions and exact pending-child identities.
It rejects cycles, forward/repeated children and orphan nodes before arithmetic.
Invalid count/root returns `InvalidArena(-1)`; other malformed storage returns
the invalid node index, or the final node for residual orphan roots. Only after
validation does a separate loop compute values with checked i32 operations.

## Failure ordering

Input length is checked before scanning. Each token is completely scanned before
its grammatical role is considered. An invalid character returns its start
position. A close parenthesis without an unmatched open is `UnexpectedClose`;
otherwise a missing operand takes priority. At EOF, a missing operand precedes
an unmatched-open error; otherwise the innermost open is reported before final
operator reductions. Capacity errors point to the incoming literal/operator,
or the stored operator when its reduction would exceed the node bound.

Decimal accumulation and evaluation overflow remain ordinary E0604 errors.
There is no wrapping or lexical fallback. Thus `1 2147483648` overflows while
scanning, while `2147483647+1+` reports a syntax failure before any evaluation.
Runtime diagnostics point to the Oxid scanner/evaluator arithmetic source span.

## Bounded stdin entry

`stdin.ox` uses the same four component modules and the bounded
`std::io::read_stdin` / `std::io::ReadStatus` imports. Its zero-argument `main`
allocates exactly 129 i32 cells: 128 input bytes and one explicit over-capacity
witness. `Eof(used)` passes only that prefix to the parser. `Full` rejects the
input without asking whether EOF follows, and leaves any 130th byte unread.
`IoError` returns an input failure without parsing. Initially unused cells are
zero, which is invalid expression input; successful short-input cases therefore
also check that unused storage is not scanned.

Results are ordinary scalar values printed by the existing main ABI. Nonnegative
values are evaluated expressions. Syntax and node/operator/operand capacity
failures return -1, invalid arenas return -3, the 128-byte input limit returns
-4, and input errors return -5. These application results exit successfully;
checked i32 arithmetic overflow still exits with the ordinary E0604 diagnostic.
The language grammar, 15-node arena, 15-entry stacks and 128-byte budget are
unchanged. This is batch input that waits for EOF or capacity, not a line editor.

With an input-enabled compiler, one compilation supports distinct streams:

```sh
oxid compile fixtures/typed-expression-samples/stdin.ox --edition typed-preview \
  --backend llvm --output ./expression-stdin
printf '12 + 3 * (4 + 5)' | ./expression-stdin
printf '7*(8+1)' | ./expression-stdin
```

The independently specified results are 39 and 63. Public-route qualification is
performed by the focused runner below; source files alone are not execution
evidence.

```sh
python3 -B scripts/verify_expression_stdin.py --oxid target/release/oxid \
  --output /tmp/expression-stdin-evidence --native
```

The runner checks and compiles the application once, then keeps its ELF hash
unchanged across 28 hand-derived cases. It checks pipe and regular-file input,
empty and malformed expressions, raw NUL/non-ASCII bytes, EOF at 127/128 bytes,
capacity at 129/130 bytes, node/stack endpoints, and scanner/evaluator overflow.
A closed stdin descriptor checks the distinct input error. An independent
controller reads the remaining bytes from the shared input descriptor after
every pipe/file execution. Check and compile receive sentinel bytes and must
leave all of them unread. Reference and native outputs, including overflow
diagnostics, must agree. The runner retains sources, input bytes, commands,
raw output, unread bytes, exit statuses and hashes. Native execution uses an
empty working directory and cleared environment; this is not filesystem
isolation. Omit `--native` for reference-only verification. The output directory
must be new.

## Evidence and scope

The original fixed-input component passed public check, reference run, LLVM
compile and ELF execution for all 45 hand-derived cases in `cases.json`: 33
parser/evaluation cases and 12 malformed-arena cases. Exact tree rows distinguish
precedence and associativity; capacity endpoints and failure positions are
asserted. Five arithmetic-overflow
cases assert E0604, the scanner/evaluator origin, and identical reference/native
diagnostics. A malformed later node is rejected before an earlier overflowing
sum can execute.

```sh
python3 -B scripts/verify_expression_component.py --oxid target/release/oxid \
  --output /tmp/expression-evidence --native
```

The output directory must be new. The runner retains source copies, expectations,
commands, exit statuses, raw streams and compiler/fixture hashes. Native programs
run from an empty working directory with a cleared environment; this is not
filesystem isolation. Omit `--native` for reference-only checks. CI runs the
native group with its pinned LLVM toolchain and retains its evidence.

That historical execution used the PR37 compiler and LLVM 19.1.7 on Linux x86_64.
All component source functions passed existing native admission, including the
128-code and 15-entry endpoints. Exact S/O/X and fuel totals were not separately
measured. Independent review and exact-head hosted CI are separate gates; these
local results do not establish other targets, performance, unbounded parsing or
completion of a roadmap milestone.
