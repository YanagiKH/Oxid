# oxi32sort: little-endian i32 record sort Process CLI

Experimental ordinary-source workspace for R2. The single member takes a
direct path dependency on the pure fixed-element package `packages/i32_vec`
(alias `vec`). The package does no I/O; only the `oxi32sort` app reads and
writes process streams. This is a dedicated `I32Vec` client and a small CLI,
not a generic `Vec` and not a promise of unbounded capacity.

```text
oxi32sort/
  oxid.toml                 [workspace] members = ["app"]
  app/oxid.toml             project "oxi32sort", entry src/main.ox,
                            vec = "../../../../packages/i32_vec"
  app/src/main.ox           Process-mode CLI; the only member that does I/O
  cases/<name>.input.bin    fixed binary oracle inputs
  cases/<name>.expected.bin fixed expected stdout (empty for exit 65)
```

## I32Vec interface consumed

The package root fixes `I32Vec`, `Element` (`Value(i32)` or `Missing`),
`FromBytes` (`Ok(I32Vec)` or `Invalid(ByteBuf)`), `new_vec() -> I32Vec`,
`len(&I32Vec) -> i32`, `get(&I32Vec, i32) -> Element`,
`set(&mut I32Vec, i32, i32) -> bool`, `try_push(&mut I32Vec, i32) -> GrowStatus`,
`pop(&mut I32Vec) -> Element`, `from_le_bytes(ByteBuf) -> FromBytes` and
`into_le_bytes(I32Vec) -> ByteBuf`. `GrowStatus` is the canonical
`alloc::bytes::GrowStatus`. The app imports only `I32Vec`, `Element`,
`FromBytes`, `len`, `get`, `set`, `from_le_bytes` and `into_le_bytes`; it never
grows the vector, so `new_vec`, `try_push` and `pop` are not used here. This
workspace defines no vector of its own and extends no package API.

## Application behavior

```sh
oxid run --manifest-path examples/projects/oxi32sort/oxid.toml --package oxi32sort --engine interpreter --entry-mode process < input.bin > output.bin
```

The app reads stdin to EOF with `read_some` into an initialized 1,024-byte
chunk, appending each chunk to one growable `ByteBuf` (the unchanged oxutf8 /
oxrev read pattern). `from_le_bytes` then converts the complete input; any
length that is not a multiple of four is `Invalid`. The records are sorted in
place, ascending by signed value, with a plain insertion sort over
`len`/`get`/`set` (stable, no extra buffer). The sorted `I32Vec` is consumed
with `into_le_bytes`, recovering one `ByteBuf` without a second payload copy,
which the unchanged `write_some` loop writes to stdout, handling partial
progress. Linux EINTR (4) is retried on both read and write; a write reporting
zero bytes is rejected.

| Outcome | Exit | Stdout |
| --- | --- | --- |
| Whole records (including empty input) | 0 | the same records, sorted ascending as signed i32, little-endian |
| Trailing 1, 2 or 3 bytes after the last whole record | 65 | empty |
| Capacity overflow (read buffer initialization or growth) | 70 | empty |
| Package contract violation during sort (`get` returned `Missing` or `set` returned false for an in-range index) | 70 | empty |
| Allocation failure or memory-limit policy refusal during read buffer initialization or growth | 71 | empty |
| Read/write error or zero-progress write | 74 | possibly a prefix |

Conversion and sorting complete before any output, so a malformed tail never
produces a prefix. A write failure can occur after part of the output was
written; no rollback is promised. This is a complete-buffer program, not
streaming: the whole input is held, and insertion sort is quadratic in the
worst case. The 1,024-byte read chunk, per-allocation headers, capacity slack
and growth's old-plus-new peaks still matter while reading. Runtime limits and
ordinary owner-return checks still apply; no input size is promised to fit
existing fuel or memory policy. Exit codes 70, 71 and 74 are documented but not
forced by any test.

## Fixed binary oracle

| Name | Class | Input bytes | Exit | Stdout |
| --- | --- | --- | --- | --- |
| empty | empty input | 0 | 0 | empty |
| min-singleton | one record, i32 MIN | 4 | 0 | identical |
| max-singleton | one record, i32 MAX | 4 | 0 | identical |
| mixed-duplicates | 14 records, positive/negative, MIN/MAX, duplicates | 56 | 0 | sorted |
| short-reverse | 6 records strictly descending (3..-2) | 24 | 0 | sorted |
| tail-1 | 3 records + 1 byte | 13 | 65 | empty |
| tail-2 | 3 records + 2 bytes | 14 | 65 | empty |
| tail-3 | 3 records + 3 bytes | 15 | 65 | empty |
| sorted-259 | 259 ascending records crossing the 1,024-byte chunk | 1036 | 0 | identical |

Every file is pinned by length and SHA-256 in `tests/owned_i32_vec_package.rs`,
which also re-derives each expected output independently (decode, sort,
encode) and requires every malformed tail to expect empty stdout.

## Acceptance

`tests/owned_i32_vec_package.rs` copies the three app files and
`packages/i32_vec` into a fresh repo-shaped temp tree (relative package path
preserved) and never builds the in-tree workspace. The interpreter test runs
CHECK, the typed format check of `app/src/main.ox`, and every fixed case
through `oxid run --engine interpreter --entry-mode process`, requiring the
exact exit code, exact stdout bytes and empty stderr. The native test (ignored
unless `OXID_TEST_LLVM_BIN` names an LLVM 19.1.7 bin directory) runs CHECK,
the format check, LLVM tool versions, BUILD and PACKAGE, extracts the archive
outside the source tree, removes the source copy, and runs only the deployed
`bin/oxi32sort` through the existing `scripts/cli_driver.py` with segmented
stdin and close-EOF, cwd=`/`, empty environment. Each driver record must have
the exact exit code, `record_ok`, an empty rejection list, complete and closed
stdin, no timeout or signal, no live process group, exact stdout bytes and
empty stderr.

```sh
cargo test --test owned_i32_vec_package
OXID_TEST_LLVM_BIN=/usr/lib/llvm-19/bin cargo test --test owned_i32_vec_package -- --include-ignored
```

Raw records are removed on success unless `OXID_OXI32SORT_EVIDENCE` names a
directory to keep them in.

## Status

Focused validation passed CHECK (26 functions), the app format check, and
the fixed nine-case interpreter and native packaged acceptance. The native
path used LLVM 19.1.7, BUILD and PACKAGE, then ran the extracted executable
outside the source tree. Following a test-only Clippy correction to decode
oracle records with `as_chunks`, the final combined acceptance run passed
both tests (2 passed, 0 failed, 0 ignored). Scoped rustfmt and Clippy with
warnings denied passed. The package's separate pure contracts passed CHECK,
interpreter Result 42 and five tests, native BUILD/standalone Result 42, and
five native tests.

Interpreter assertions establish exact status, stdout and empty stderr for
the fixed corpus; they do not independently prove full stdin delivery. The
native acceptance wrapper requires complete and closed stdin and clean
process cleanup in addition to exact outputs. The nine retained native
records were independently checked against the reviewed fixed corpus: exact stdin/stdout/stderr bytes, complete and closed
stdin, stream EOF, clean process cleanup, cwd `/`, empty environment,
`record_ok`, and no rejection reasons. The nine retained interpreter output
captures and 29 command-receipt lengths and hashes were also checked. The
deployed executable and package archive were not retained in this evidence
archive, so their hashes remain receipt-reported rather than independently
recomputed. The pure-contract native Result 42 is supported by its raw log;
its structured driver record was not retained in this archive.

These are focused results, not a full repository CI pass or completed R2.
No allocation-failure injection, capacity/fuel qualification, forced
short-write or EINTR qualification, or other outstanding qualifications are
claimed. Exit codes 70, 71 and 74 were not forced by these tests. All source
code and binary fixtures retain the exact validated bytes.
