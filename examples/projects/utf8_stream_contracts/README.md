# utf8_stream contracts

Independent pure contract cases for the frozen experimental
`packages/utf8_stream` decoder (dependency alias `stream`) and for the shared
oxtextstat statistics helper `../oxtextstat/core` (alias `stat`). The project
imports no I/O and has no `main`; run it through ordinary interpreter TEST:

```sh
oxid test --manifest-path=oxid.toml --engine=interpreter
```

Every expected scalar, invalid lead offset, lifecycle result and report is a
fixed literal (cross-checked by hand and with an external UTF-8 codec when
authored); none is computed by the decoder under test.

## Decoder cases

- ASCII/NUL/CR/LF/DEL scalars; 2/3/4-byte boundary values including U+0080,
  U+07FF, U+0800, U+D7FF, U+E000, U+FFFF, U+10000, U+10FFFF; BOM,
  noncharacters (U+FDD0, U+FFFE, U+1FFFF) and a combining sequence.
- Invalid leads (80, 8F, BF, C0, C1, F5, F7, F8, FE, FF) and stray
  continuations, both at offset 0 and after a valid prefix.
- Second-byte restrictions after E0, ED, F0 and F4, plus ordinary 7F/C0
  rejections after C2, E1 and F1; wrong third and fourth continuations.
  A rejected continuation reports its sequence lead.
- Incomplete EOF after each partial 2/3/4-byte prefix, at offset 0 and after
  valid prefixes; premature EOF cannot be repaired by later bytes.
- Sticky Invalid(n) for later pushes and finishes; successful finish is
  idempotent and later pushes return Finished.

## Shared consumer helper cases

`stat::feed` is the same ordinary-source function the oxtextstat Process read
loop calls with each returned `read_some` prefix. These cases call it with
explicit logical chunk partitions: every two-chunk split of representative
2/3/4-byte scalars and an 11-byte mixed input (empty first/last chunks
included), every three-chunk split of 3-, 4- and 7-byte inputs, and one-byte
chunks separated by empty chunks. `stat::finalize` runs once, only after the
last chunk; no report is rendered before it succeeds.

This establishes state retention of the shared helper across controlled logical
chunks. It is not evidence about kernel read partitions; the public decoder
itself accepts exactly one byte per call.

## Not covered at runtime

Position overflow (`PositionOverflow` after 2147483647 submitted bytes) and the
helper's checked i32 counter increments are reviewed from source only. No
state-forging hook exists and no two-gigabyte input is processed.
