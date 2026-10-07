# Carrier admission experiment

The first probe shares the existing lexer modules and source bound. Its entry is
`../typed-lexer-samples/parser_admission.ox`; it constructs synthetic full-width
banks, checks push/resume/pop and duplicate/limit behavior, poisons an unused row,
and serializes checked column values. It does not parse the scalar grammar.

The initial seven-column AST and three-column continuation representation was
rejected by the current native consumer with E0700: aggregate expanded cells
exceeded 8192. Typing passed before that gate. No ELF execution or viable parser
admission is claimed for this initial layout. Existing compiler limits remain
unchanged. Constructor/initializer/binding copies make the raw resident payload
sum insufficient to predict this admission cost.

This checkpoint preserves the failed candidate before any layout change. A
successor may compact finite integer fields without reducing grammar, row count,
source bound or continuation capacity, but needs its own actual admission and
round-trip/control evidence. The full parser remains unimplemented.

Two lossless successors also reached the same E0700 aggregate-cell gate:

- Six AST columns and two continuation columns combined kind/span and state/aux
- Three AST columns and one continuation column further paired bounded fields

The current three-column row stores header_links, ab and cd. Its encodings are
`kind + 64*(start + 256*end) + 4194304*next`, `a + 256*b`, and `c + 256*d`.
With kind 1..63, offsets 0..128 and start<=end, and IDs 0..128, the first value is
at most 538976319 (<2^30); pairs are at most 32896. Non-EOF successful syntax token
references fit 0..128; diagnostic EOF token 129 stays outside these row fields.

A continuation is `node*65536 + state + 32*aux`, at most 8452117 (<2^24).
State 1..21 and aux depth/context/precedence components are checked before packing;
aux's holes with depth>64 are rejected. Root alone has node 0; pushed rows have
node 1..128. All unused physical row/frame slots are zero. Probe serialization
uses four separate byte planes and never computes 256^4 in i32.

Independent arithmetic review confirms reversibility within these checked
domains. That does not imply native admission: all three candidates were rejected
under the unchanged compiler. Broad parser work is stopped pending attribution of
the actual aggregate accounting. A future diagnostic observer must preserve the
same admission refusal and remain separate from production compiler authority.

## Separate array owners: admitted carrier, limited headroom

The same three packed AST columns and single continuation column now live in
separate array locals, with only row/stack counts in a two-scalar record. Helpers
receive borrowed column views. This removes constructor staging copies while
preserving every logical field and the full 128-row/129-frame capacities.

An isolated diagnostic observer of the unchanged admission calculation measured
8385 expanded cells for the prior nominal-bank candidate and 7845 for the separate
arrays:347 cells remain under 8192. Owner payload decreased by 1032 cells; extra
views/arguments, helper calls and stricter frame decoding offset 492 of that
saving. The observer changed only logging before the original limit; production
compiler limits and authority were unchanged. The original compiler also admits
and builds the separate-array probe.

Six input/control scenarios pass in both reference and native execution. The
successful cases fill all 128 rows, exercise maximum stack depth and in-place
resumptions, reject duplicate/full/root-pop operations, detect an unused-row
poison, and produce exactly 1553 independently checked bytes. These are synthetic
storage/control results, not AST parsing or a complete parser-control budget.

347 cells of headroom is not sufficient evidence for broad parser implementation.
The next proposed reuse boundary is a lexer core operating on caller-owned token
columns, preserving the existing Tape API through a wrapper. It can avoid the
1197-cell tape constructor in a parser executable. This requires separate review,
lexical parity and actual admission before claiming additional headroom. No
compiler lifetime/accounting change or reduced grammar is part of this probe.
