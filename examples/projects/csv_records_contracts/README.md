# csv_records contracts

Independent pure contract cases for the frozen experimental
`packages/csv_records` reader (dependency alias `csv`) and for the shared
oxcsvsum summary helper `../oxcsvsum/core` (alias `sum`). The project imports
no I/O and has no `main`; run it through ordinary interpreter TEST:

```sh
oxid test --manifest-path=oxid.toml --engine=interpreter
```

Every expected field, `record_end` flag, malformed offset, copy size and
report is a fixed hand-written literal (cross-checked during authoring against
an independent model of the package README rules); none is produced by the
code under test. Test inputs are byte literals with explicit escapes, so no
binary input depends on lossy text conversion.

## Reader cases

- Every row of the package README dialect table, plus CRLF records, blank
  CRLF lines, comma followed by CRLF/EOF and closing quote followed by comma,
  CRLF or EOF.
- Quoted payload with comma, LF, CRLF, bare CR, NUL, literal backslash and
  bytes that are not UTF-8 (the byte parser accepts them); NUL and non-UTF-8
  bytes in unquoted fields; untrimmed spaces; several doubled quotes.
- Whole documents of 1 through 5 quote bytes, and quote runs around content.
- Exact malformed offsets: quote inside an unquoted field, bare CR (also at
  EOF and directly after a closing quote), unexpected byte after a closing
  quote, unterminated quoted field (also `"a""` at EOF), a late error after
  complete records. Every error is repeated three times and leaves no current
  field.
- Copy into empty, undersized, exact and oversized destinations. Refusals
  (`BufferTooSmall`, `NoCurrent`) leave every byte unchanged, including on an
  undersized retry; success writes exactly the decoded prefix and preserves
  the suffix. Empty fields copy zero bytes into empty and non-empty storage.
- Lifecycle: no current field before the first `next_field`, repeated copy,
  advancing without copying, a final EOF field that stays copyable until the
  next call, and invalidation by `End` and by `Malformed`; `End` is sticky.
- A returned `FieldInfo` that is changed or retained across an advance does
  not change which field is copied or the size that copying requires. No
  forged-span API exists, so none is tested.

## Shared summary helper cases

`sum::summarize` is the same ordinary-source function the oxcsvsum Process
application calls after reading stdin. These cases give it complete owned
inputs directly: the handoff success rows, Unicode/NUL labels, a quoted
negative value and i32::MIN; rejections with status 65 (field count, empty
or spaced or out-of-range or quote-containing decimal, malformed CSV,
invalid UTF-8, a header row) and 70 (intermediate sum overflow in both
directions, also when a later value would return into range). A rejected
input leaves `render` refusing with the output untouched; a second
`summarize` call is a lifecycle error.

## Not covered at runtime

Row-count overflow (2147483647 records), maximal input offsets, allocation or
memory-policy refusal and terminal interruption are reviewed from source only.
No state-forging hook exists and no enormous input is processed.
