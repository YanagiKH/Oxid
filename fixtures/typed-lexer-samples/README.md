# Bounded typed-preview ASCII lexer component

This component targets the existing typed-preview lexer on at most 128 ASCII
bytes. It does not select a compiler provider or claim Unicode equivalence.
The first checkpoint proves storage and native output admission only:
`admission.ox` constructs a synthetic one-token-per-byte tape, not a lexer.

The token tape holds three 129-cell columns and a count, including EOF. IDs
1 through 47 follow the current canonical `Kind` declaration order; EOF is47.
Lexing preserves trivia, raw spellings through source spans, and Invalid and
Unsupported kinds. Those kinds are successful lexical results. Input is read
into129 cells; Full is refused before scanning, and the final lexer will preflight
all accepted bytes for ASCII before scanning or output. No limits are raised.

## OXL1 observation contract

A complete observation is exactly395 bytes: `OXL1`, result tag, count, error
start, error end, then129 kind bytes,129 start bytes,129 end bytes. Spans are
half-open source byte offsets. Tag0 means a token tape, with count1..129,
contiguous coverage and one final zero-width EOF. Its error fields are zero.
Tag1 maps exactly to E0100/lex `unterminated string literal`; tag2 maps exactly
to E0100/lex `unterminated block comment`. Error tapes have count0 and all three
columns zero. Every unused row of a success tape is zero. The exact source bytes
are bound externally to the observation; numeric spellings are never evaluated.

A complete tape is validated before the first output call. Header and columns
use four existing whole-view writes; this avoids an inadmissible395-cell literal
constructor under the existing256 native scalar-slot cap. Exit0 means all four
writes completed, including when the observation describes a lexical error.
Exit64 with no tape refuses capacity/non-ASCII input; input or output I/O failure
is74. An impossible internal tape/byte invariant is70. Partial output after an
I/O failure is not a valid observation. This is not physically atomic output.

The source bound implies at most128 non-EOF tokens and makes the canonical
65536-byte token and100000-token limits unreachable. No parity at those larger
limits or compiler provider replacement is claimed.
