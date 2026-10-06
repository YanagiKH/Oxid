# Bounded enum diagnostic amendment for the scalar native suite

The explicit `bounded-enum-match-v1` selection changes one historical mutable
locals negative expectation. Its source remains byte-identical, including the
Unicode comment and CRLF:

```python
"// 雪\r\nfn main() -> () { let mut x = 1; match true { x = 2; } return; }"
```

Source SHA-256: `dd68ba7d2d902bce84163bd305d09ee4ef4ce63efccfe20dc4417a8892dde2fe`.
Original six-field tuple SHA-256 (JSON, ASCII escaping, compact separators):
`3a8bb56c474222dd5d503c6e04de5ef6883244dfdf258a2a63cd6ab9ffd2858f`.

The original oracle expects E0101 at parse stage and specifies no primary span.
The bounded enum grammar recognizes `match`, but its scrutinee must be a bare
binding name. The same invalid source therefore produces E0100 at parse stage,
`match requires a bare binding name`, with primary UTF-8 byte span 47..51 over
`true` (line 2, columns 40..44). The current amendment asserts that message and
origin; it does not invent an old message or span.

`verify_mutable_locals.py` keeps its original generator and historical default.
Only an explicit `--expectation-amendment bounded-enum-match-v1` selects the new
expectation, and source/old-tuple drift or nonunique membership is rejected.
The native-suite launcher names this selection. The final oracle report retains
both expectations and their identities. Positive models, input sources, category
labels, corpus hashes, resource cases, runtime/native parity and other negative
expectations remain unchanged. This is not a general E0101-to-E0100 mapping.

The existing checked-unary amendment remains separately selected for the boolean
suite. While and loop-control negatives have no match/enum fixture to amend.
Current-source and exact-head hosted qualification remain separate from this
narrow expectation decision.
