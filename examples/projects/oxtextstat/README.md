# oxtextstat

A bounded Process-mode consumer of the frozen experimental
`packages/utf8_stream` decoder. It reads stdin with the existing
`std::io::read_some` route into one 64-byte buffer initialized once, feeds
every returned byte exactly once into one retained decoder, and calls `finish`
only at real EOF. On success it writes exactly:

```text
<bytes> <scalars> <lines>\n
```

Fields are canonical nonnegative i32 decimal from `packages/i32_decimal`.
`lines` counts LF scalars, plus one when the input is nonempty and its last
scalar is not LF. CR (including CRLF), BOM and NUL have no special meaning.

```sh
oxid run --manifest-path=oxid.toml --package=oxtextstat --engine=interpreter --entry-mode=process < input
```

## Layout

- `app/`: the Process application (`main`, read loop, `write_all`).
- `core/`: ordinary-source statistics helper (`new_counts`, `feed`,
  `finalize`, `render`) with no I/O imports. The same helper is called by the
  pure contracts in `../utf8_stream_contracts` with explicit logical chunks.

## Statuses

| Status | Meaning | Output |
| --- | --- | --- |
| 0 | valid report | report; stderr empty |
| 65 | malformed or incomplete UTF-8 | none |
| 70 | checked counter/position overflow, buffer capacity overflow, unexpected decoder lifecycle result, impossible read count or impossible report length, all detected before the report write starts | none |
| 70 | impossible write count returned while the report is being written | none if no earlier write call succeeded; otherwise the already written report prefix |
| 71 | buffer allocation or memory refusal | none |
| 74 | read/write failure or zero-progress write | a report prefix only if writing the report failed |

No report is written before successful `finish` and counter finalization.
Writes retry Linux EINTR (4) and treat zero progress as failure; reads retry
EINTR. The report (at most 33 bytes) reuses the read buffer after EOF; no
rollback of a partially written report is promised.

## Execution budget

The interpreter's existing 1,000,000-unit fuel budget is unchanged. In
earlier acceptance runs a valid 2,161-byte input exhausted it (E0601, status
1, no report). That failure is unresolved: it is kept as a recorded limitation with
its original failure package, exact input and raw evidence, and it is neither
re-run nor treated as a passing acceptance test. Interpreter Process use is
therefore limited to small inputs. The 64-byte buffer only lets the smaller
147..195-byte cases cross several read_some calls within budget; those cases
verify bounded inputs and do not fix the 2,161-byte failure.

## Evidence boundary

`tests/typed_utf8_stream_package.rs` runs each Process case once through the
interpreter Process route with regular-file stdin, plus one piped case whose
separate parent writes do not determine the child's reads. Actual read_some
boundaries are not controlled or recorded, so no read-boundary placement is
claimed; logical chunk boundaries are covered by the pure contracts.
Executed statuses are 0, 65 and 74 (EPIPE after the stdout reader is closed).
Statuses 70 and 71, read errors, EINTR, short writes and zero-progress writes
are not forced and are not covered. Native execution is not part of this
acceptance.
