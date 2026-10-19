# owned_text_scalars: client contracts for `text::next_scalar`

Pure Result-mode client of the ordinary path package `packages/utf8_text`
(alias `text`). It consumes `ScalarStep` (`scalar: i32`,
`next_byte_offset: i32`), `ScalarRead` (`Value(ScalarStep)`, `End`,
`InvalidOffset`) and `next_scalar(text: &Text, offset: i32) -> ScalarRead`,
together with the existing `from_bytes`, `new_text`, `byte_len`,
`scalar_count`, `equals_utf8`, `slice` and `into_bytes`. It does no I/O, and
`main` returns 42 only when every contract passes.

The contract reads one scalar from the current Text: an offset at a scalar
start returns `Value` with the code point and the byte offset just past it;
`offset == byte_len` returns `End`; a negative offset, an offset past the end
and an offset inside a multi-byte scalar return `InvalidOffset`. The Text is
never changed. The offset is an ordinary `i32` passed by value; the caller
keeps its own cursor and advances it from `next_byte_offset`. `ScalarStep` is
plain data with no authority or borrow. No payload allocation or export is
involved, and no claim is made that the runtime as a whole allocates nothing.

Fixed corpora, written as raw byte arrays:

| Corpus | Contents | Bytes | Scalar starts |
| --- | --- | --- | --- |
| ascii | NUL, `A`, U+007F | 3 | 0 1 2 |
| two | U+0080, U+07FF | 4 | 0 2 |
| three | U+0800, U+D7FF, U+E000, U+FFFF | 12 | 0 3 6 9 |
| four | U+10000, U+10FFFF | 8 | 0 4 |
| special | U+FEFF (BOM), `e`, U+0301 (combining), U+FDD0, U+FFFE, U+1FFFE, U+10FFFE | 20 | 0 3 4 6 9 12 16 |
| mixed | `A`, U+00A2, U+20AC, U+1F600, `z` | 11 | 0 1 3 6 10 |
| tokens | `x1 cafe` + U+0301 ` ` U+20AC `5 ` U+1F600 `!` | 20 | 14 scalars |

Every expected scalar value, next byte offset and token spelling is a
separate hand-written value; nothing is derived from the package result,
from `append_scalar`, or by decoding the corpus. The `test_` cases cover NUL
and ASCII, the first and last 2-, 3- and 4-byte scalars, U+D7FF and U+E000
around the surrogate range, U+10FFFF, a BOM, a combining mark and
noncharacters; all interior offsets in the representative two-, three-, and
four-byte boundary corpora; negative
offsets including i32 minimum; offsets past the end including i32 maximum;
`End` on an empty Text and at the end of a non-empty Text, read repeatedly;
repeated reads that leave both the caller's cursor and the Text unchanged; a
caller-driven walk over a whole Text; and a scan of space-separated mixed
tokens whose recorded byte bounds are passed to `slice` and compared byte for
byte with the exact original spelling. Each case checks afterwards that the
original Text still has its exact bytes, length and scalar count.

No capacity or fuel pressure and no allocation fault injection. No command
line tool is part of this client.

```sh
oxid check --manifest-path examples/projects/owned_text_scalars/oxid.toml
oxid test --manifest-path examples/projects/owned_text_scalars/oxid.toml
oxid run --manifest-path examples/projects/owned_text_scalars/oxid.toml --engine=interpreter --entry-mode=result
```

The Rust acceptance is `tests/owned_text_scalars.rs`. Its two native tests are
ignored by default. Set `OXID_TEST_LLVM_BIN` to an LLVM 19.1.7 bin directory
and explicitly enable ignored tests:

```sh
OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --locked --test owned_text_scalars -- --include-ignored
```

## Focused validation

The repaired client passed CHECK (58 functions), typed-preview formatting,
ordinary acceptance (2 passed, 2 ignored), and explicitly enabled native
acceptance (4 passed, 0 ignored). The four tests cover interpreter Result 42,
twelve named interpreter contracts, twelve native contracts and a standalone
executable returning exact `42\n` with exit 0 and empty stderr after removing
the copied sources and clearing its environment.

The token scan was split into three small helpers after the original client
hit the existing 256-slot native per-function ceiling. All nine hand-written
arrays, return-code checks and twelve named contracts were preserved; no
compiler ceiling or package behavior was changed. The passing commands ran
with this repaired source before its one-file commit. Subsequent source and
commit deltas were checked, but the logs do not provide per-command content
hashes or a separate successful child-output archive.

The native harness uses the existing LLVM 19.1.7 version-marker gate; the
delivered repair logs do not separately include tool-version stdout. Earlier
existing-Text regression and scoped lint passes are executor-reported because
the prior raw archive was unavailable. No full-suite, exact-head CI,
allocation-failure, capacity/fuel or broader process qualification is claimed.
