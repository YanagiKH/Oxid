# Bounded typed-preview ASCII lexer component

This component targets the existing typed-preview lexer on at most 128 ASCII
bytes. It does not select a compiler provider or claim Unicode equivalence.
`main.ox` implements the lexer with the existing language and process I/O.
`admission.ox` remains a separate synthetic storage/output admission probe.

The token tape holds three 129-cell columns and a count, including EOF. IDs
1 through 47 follow the current canonical `Kind` declaration order; EOF is 47.
Lexing preserves trivia, raw spellings through source spans, and Invalid and
Unsupported kinds. Those kinds are successful lexical results. Input is read
into 129 cells; Full is refused before scanning, and the lexer preflights
all accepted bytes for ASCII before scanning or output. No limits are raised.

## OXL1 observation contract

A complete observation is exactly 395 bytes: `OXL1`, result tag, count, error
start, error end, then 129 kind bytes, 129 start bytes, 129 end bytes. Spans are
half-open source byte offsets. Tag 0 means a token tape, with count 1..129,
contiguous coverage and one final zero-width EOF. Its error fields are zero.
Tag 1 maps exactly to E0100/lex `unterminated string literal`; tag 2 maps exactly
to E0100/lex `unterminated block comment`. Error tapes have count 0 and all three
columns zero. Every unused row of a success tape is zero. The exact source bytes
are bound externally to the observation; numeric spellings are never evaluated.

A complete tape is validated before the first output call. Header and columns
use four existing whole-view writes; this avoids an inadmissible 395-cell literal
constructor under the existing 256 native scalar-slot cap. Exit 0 means all four
writes completed, including when the observation describes a lexical error.
Exit 64 with no tape refuses capacity/non-ASCII input; input or output I/O failure
is 74. An impossible internal tape/byte invariant is 70. Partial output after an
I/O failure is not a valid observation. This is not physically atomic output.

The source bound implies at most 128 non-EOF tokens and makes the canonical
65536-byte token and 100000-token limits unreachable. No parity at those larger
limits or compiler provider replacement is claimed.

## Reproduce the comparison

With a current debug compiler and the pinned native toolchain available:

```sh
python3 scripts/build_typed_lexer_observer.py --output /tmp/typed-lexer-observer
python3 scripts/verify_bounded_typed_lexer.py --oxid target/debug/oxid \
  --observer /tmp/typed-lexer-observer/canonical-lexer-observer \
  --output /tmp/typed-lexer-proof --native
```

Both output directories must be new. The observer compiles unchanged copies of
four canonical Rust source files and a small visibility/output wrapper. It never
supplies compiler authority to the Oxid component. Thirty authored cases cover
all 47 kinds and seven exact errors; additional cases exercise every ASCII byte,
keyword prefixes and input refusals. Receipts distinguish canonical comparison,
Oxid reference execution and the same native ELF across all inputs.

Native runs use an empty working directory and cleared environment after moving
the copied source project away from its build-time location. These runs succeed with an empty working directory, no PATH variable, and the
original copied source path absent. Files and executables elsewhere remain
accessible; filesystem and process activity were not traced.
Compiler provider dispatch and Unicode support remain separate work.
