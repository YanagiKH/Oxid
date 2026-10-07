# Bounded scalar static frontend

This Oxid-written component resolves names and checks types and control flow for
one source of at most 128 ASCII bytes in the existing scalar parser grammar. It
produces complete observed HIR facts or the first canonical static diagnostic.
Local reference/native qualification covers the bounded component; integrated
current-source replay and exact-head hosted CI remain separate pending gates.

## Use the component

The frontend deliberately uses two executables. The existing
[parser](../typed-parser-samples/README.md) emits OPA1. For syntax success, a host
frames the exact original source and unchanged OPA1 bytes as
[AST1](AST_INPUT.md), then passes that record to
[`ast_static_main.ox`](../typed-lexer-samples/ast_static_main.ox). The host only
frames and relays bytes. It does not parse, repair AST rows, resolve names, infer
types or compute flow. Lexical/parser failures and grammar-domain refusals remain
unchanged producer observations and do not enter the AST1 consumer.

The consumer re-lexes the exact source and completely validates framing, source
coverage, syntax, ownership, spans, precedence, heights and canonical allocation
order. It clears and checks all scratch storage before resolution. Resolution of
the entire program finishes before type checking begins, including unreachable
syntax and both branches. The library-checking contract accepts empty input,
recursion, no `main`, and parameterized `main`; it does not execute the input.

A complete result retains the 1,559-byte OPA1 AST and adds STF1: tag 0 with both
complete fact columns totals 2,607 bytes; tag 1 with the first static diagnostic
totals 1,575 bytes. A process status of 0 means a complete observation, which may
be a diagnostic. Malformed AST1 returns 64 with empty artifact stdout; I/O failure
returns 74 and internal failure returns 70. Output failure can leave a prefix,
so status and exact length must both be checked. See the
[fact and diagnostic contract](CONTRACT.md) and the
[replay instructions](../../tests/fixtures/bounded_typed_static/README.md).

## Local qualification and limits

The separate `ast_static_main.ox` consumer admits with I = 7,741,
W = 3,907, 95 functions, 1,818 blocks and 79,048 explicit native bytes, including
the eight-byte process wrapper. The I/W ceilings remain 8,192 each; scalar-slot,
owner, input, depth, fuel and all other compiler limits are unchanged. Native
qualification uses the existing Linux x86_64, LLVM/Clang/LLD 19.1.7 at O0 scope.

The completed local semantic replay has 77 real reference/native consumer pairs:
29 compare complete typed facts and 48 compare first diagnostics, covering all
14 static diagnostic kinds. Four producer relays are counted separately: three
canonical lexical/parse failures and one existing record-domain refusal at the
keyword span `0..6`. The first replay failed because a generic observer reported
that out-of-domain record span as `0..15`; that failed evidence is retained. The
corrected comparison selected the existing parser-domain contract without a
candidate source change or a fabricated semantic pass. Independent integrated
source/projection review found no material issues in this bounded
scope and re-projected all 77 retained semantic pairs successfully. It also passed
62 observation tests, including 19 typed-projection tests; this review did not
re-execute the consumer or establish hosted CI results.

Independent validator qualification includes 59 closed controls and 34
parser-produced positive pairs. Separate I/O qualification has 96 executions
(48 reference/native pairs), including 31 malformed cases, real directory-input
`EISDIR`, pipes with no reader, injected fd-1 prefix-write failures and the exact
single trailing-byte witness. The injected cases establish behavior under those
injected outcomes; they do not prove a real partial kernel write. Earlier
400-observation lexer, 139-case parser, 41-corruption, two-transport and 16-carrier
replays retain their own source/binary identities and scopes; they are not
fresh integrated-head results. [ADMISSION.md](ADMISSION.md) preserves this history.

## Separate roots and remaining work

- `static_main.ox` remains the permanent synthetic STF1 tag-2 probe
- `resolver_main.ox` remains resolution-only, with typing explicitly pending
- `ast_consumer_probe.ox` remains closed, returning pending/internal status 70
  after accepted validation rather than emitting a typed success
- The combined source-to-type `typed_main.ox` candidate remains inadmissible at
  I = 8,778; the separate AST1 consumer does not turn that failure into a pass

The reusable boundary advances the frontend while keeping the parser's own
syntax-only scope. Its observations are not a compiler-owned `TypedProgram`, an
OIR artifact or a source-association witness. Production provider selection and
compiler self-rebuilds are unchanged. Reproducible current-source controller
qualification and hosted CI are still required; future
provider integration must establish the compiler's actual source identity and
authoritative ownership separately.
