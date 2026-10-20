# oxcsvsum

An experimental typed-preview Process application that reads a complete
two-column CSV document from stdin and prints the checked record count and
signed i32 sum of the second column:

```text
<rows> <total>\n
```

It is a complete-buffer program: all of stdin is read to EOF into one growable
`ByteBuf` before validation starts, and nothing is written before the whole
input has been accepted. It is not an unbounded or streaming summary tool.

## Layout

- `app` (package `oxcsvsum`): Process I/O only. `read_some` into a 64-byte
  read buffer appended to a growable owner until EOF (EINTR retried), then
  `sum::summarize`, `sum::render` and a partial-write/EINTR/zero-progress
  checked write of the report.
- `core` (package `oxcsvsum-core`): pure summary helper shared with
  `../csv_records_contracts`. It validates the complete bytes with the
  existing `utf8_text::validate`, then moves the same owner into
  `csv_records::new_reader` without copying the payload.

Dependencies are ordinary path dependencies on `packages/csv_records`,
`packages/utf8_text` and `packages/i32_decimal`.

```sh
cd examples/projects/oxcsvsum
oxid run --manifest-path=oxid.toml --package=oxcsvsum --engine=interpreter --entry-mode=process < input.csv
```

## Records

Each record is exactly two fields in the `csv_records` dialect:

1. Label: any UTF-8 text, including empty, quotes, commas and embedded line
   endings when quoted. It must not end the record. It is never copied.
2. Value: must end the record. Copied with `copy_current_field` into one
   reused growable `ByteBuf` (grown with `try_resize` only when `decoded_len`
   exceeds its length; no fixed input-width cap, so long runs of leading
   zeroes are accepted) and parsed as a whole with `i32_decimal::parse_i32`.

There is no header, BOM stripping, comment handling, whitespace trimming or
numeric coercion. Quoted decimal values are accepted; a decoded quote inside
a value is an invalid digit. Leading `+`/`-` and zeroes follow `i32_decimal`.

Counting is checked in input record order: the row count is checked before
incrementing, and each value is added only after `total > 2147483647 - value`
(positive values) or `total < -2147483648 - value` (negative values) has been
ruled out. An intermediate overflow is rejected even when a later value would
return the total into range. Numbers are written with `format_i32` through an
11-byte scratch array per number; the empty document reports `0 0`.

## Exit statuses

| Status | Meaning | Output |
| --- | --- | --- |
| 0 | exact report | report on stdout, stderr empty |
| 65 | invalid UTF-8, malformed CSV, wrong field count, invalid or out-of-range decimal | none |
| 70 | checked row/sum overflow, capacity overflow, impossible copy/format/read/write result | none before the report; an impossible write count may follow a written prefix |
| 71 | existing allocation failure or memory-policy refusal | none |
| 74 | read/write error or zero-progress write | a report prefix is possible only after valid input once writing began |

Existing terminal interpreter failures (for example fuel exhaustion) are not
converted into these statuses; their diagnostics and exit behavior are
unchanged. No input size is promised to complete under the execution budget.

## Acceptance scope

`tests/typed_csv_records_package.rs` runs the contract project's interpreter
TEST once and each fixed Process case once (regular-file stdin; one closed
stdout pipe for the EPIPE write failure). Executed statuses: 0, 65, 70 and 74.
Statuses 71, read errors, EINTR, short or zero-progress writes, row-count
overflow and capacity overflow are reviewed from source only. Multi-read
inputs larger than the 64-byte read buffer are executed, but the child's
actual `read_some` partitions are neither controlled nor recorded. Native or
LLVM execution is not part of this scope.
