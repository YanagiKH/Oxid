# Actual scalar grammar observations

The six `scalar-*-source.txt` files are exact ASCII inputs of at most 128 bytes
to the same retained native Oxid parser used for `rich-success.bin`. Each
`scalar-*-success.bin` is the unchanged 2,607-byte output of that parser's native
static consumer. Both processes exited successfully with empty stderr. The host
only supplied the existing `AST1` transport: marker, one source-length byte,
exact source bytes, then unchanged parser stdout. It authored no OPA or STF rows.

`scalar-origin.json` records the executable, input, stdout and stderr hashes,
source sizes, active row counts, and observed row kinds for each process. The
producer tree and executable hashes match `origin.json`.

- Arithmetic covers grouping, unary negation, and all five arithmetic operators
- Boolean covers logical negation, conjunction, disjunction, and false
- Comparison covers all six comparison operators
- Unit covers unit types and values, an annotation, expression statements, and
  a return without a value; its two-space trivia run is deliberate
- Loop covers while, if/else, break, and continue
- Assignment covers a mutable annotated local, assignment, and the i32 minimum
  with trivia between the sign and decimal digits

Together with the original rich observation these exercise all 36 scalar OPA
row kinds. Tests use genuine original source ownership and the disconnected OPA
comparator; they do not assume project or canonical admission on another host.
Changed row-kind controls and split whitespace/comment trivia tapes must reject.
These observations confer no candidate, typed-program, or executable authority.

## Public bridge arithmetic controls

`synthetic-overflow-{source.txt,success.bin}` and
`synthetic-division-{source.txt,success.bin}` are the exact existing hand-authored
arithmetic row tables from `emit_tests::arithmetic_frames`, exported by the
private native qualification at 6022477. They are explicitly synthetic untrusted
observations, not producer-execution or compiler-generated fact evidence. They
exist so the separate public CLI integration test can replay those same fixed
inputs and compare complete ordinary and imported runtime diagnostics.
