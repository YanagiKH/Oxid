# Experimental owned CSV record reader

An ordinary-source typed-preview package for a precise byte-oriented CSV
dialect. It owns a complete input ByteBuf, scans fields sequentially and copies
the current decoded field into caller-provided storage. It performs no I/O,
UTF-8 conversion, numeric parsing, header interpretation or schema inference.
It is not a stable CSV standard library or completed R4/R5 milestone.

## Public API

- `Reader`: owned record with private input, cursor and current-field state.
- `new_reader(ByteBuf) -> Reader`: consume the existing input owner without
  copying its payload. Construction does not validate the document.
- `FieldInfo`: public informational `decoded_len: i32` and `record_end: bool`.
- `next_field(&mut Reader) -> Next`:
  - `Field(FieldInfo)`: a syntactically valid field was scanned and is current.
  - `End`: no more fields; repeated calls return End.
  - `Malformed(byte_offset)`: syntax error; repeated calls return the same
    error and offset.
- `copy_current_field(&Reader, &mut [u8]) -> CopyResult`:
  - `Written(byte_count)`: decoded bytes occupy exactly that output prefix;
    every suffix byte is unchanged.
  - `BufferTooSmall(required)`: every destination byte is unchanged.
  - `NoCurrent`: every destination byte is unchanged.

Use a path dependency, for example from `examples/projects/my_app/oxid.toml`:

```toml
[dependencies]
csv = "../../../packages/csv_records"
```

The input cannot be mutated or exported through this API. FieldInfo contains
no access authority. The copy operation takes no metadata argument, and
selects only the reader's private current field. Forging or retaining an old
FieldInfo cannot select another range or change destination sizing.

## Dialect

- Comma separates fields.
- LF or CRLF ends a record. Bare CR outside a quoted field is malformed.
- A field is quoted only when its first byte is ASCII double quote.
- A quoted field ends at an unpaired double quote; two consecutive quotes
  inside it decode to one quote.
- Inside quotes all other bytes are payload, including comma, LF, CRLF,
  bare CR, NUL and bytes that are not valid UTF-8.
- After the closing quote, only comma, LF, CRLF or EOF is legal.
- A quote anywhere inside an unquoted field is malformed.
- Whitespace is never trimmed. Backslash is an ordinary byte, not an escape.
- No implicit header, comment, BOM stripping, newline normalization, field
  count enforcement or text-encoding interpretation is performed.

This is the complete declared dialect, not a claim of universal CSV
compatibility. Validate UTF-8 separately if the application requires text.

## Empty inputs, records and field boundaries

`record_end` is true for a field followed by LF, CRLF or EOF, and false for a
field followed by a comma. Delimiters and surrounding quotes are never copied.

| Input (escaped notation) | Decoded fields and record_end |
| --- | --- |
| empty | no fields |
| `a` | `a` true |
| `,` | empty false, empty true |
| `a,` | `a` false, empty true |
| `\n` | empty true |
| `\n\n` | empty true, empty true |
| `a\n` | `a` true |
| `a,\n` | `a` false, empty true |
| `""` | empty true |
| `"a,b",c` | `a,b` false, `c` true |
| `"a""b"` | `a"b` true |

An ending record separator does not create an additional record. A trailing
comma does create the final empty field. The empty document has zero records,
whereas one blank line has one record containing one empty field.

## Errors and lifecycle

Offsets are zero-based byte positions in the original complete input:

- Quote inside an unquoted field: that quote's offset.
- Bare CR outside quotes: the CR's offset, including CR at EOF.
- Unexpected byte after a closing quote: that unexpected byte's offset.
- Unterminated quoted field: its opening quote's offset.

For example, `ab"c` reports 2; `"a"x` reports 3; `a\rX` reports 1;
`x,"a` reports 2. Parsing is left-to-right and stops at the first field error.
The parser validates a field's separator before reporting that field, so
`a\rX` does not first return a successful `a` field.

A current field exists only after next_field returns Field. It may be copied
zero or more times; copying never advances the reader. An undersized copy can
be retried with larger storage. The next next_field call invalidates the
previous current field, whether it returns another Field, End or Malformed.
Thus copying before the first field, after End or after Malformed returns
NoCurrent. A final successful field remains copyable until next_field is
called again, even when that field ended at EOF.

Errors are sticky and there is no reset or skip-invalid operation. Previously
returned fields do not establish that the rest of the input is valid. A
consumer requiring all-or-nothing external output must defer output until End
and all of its own validation succeeds.

## Ownership, arithmetic and costs

Reader retains the original ByteBuf. There is no second payload allocation in
construction, scanning or copying, and no escaping borrowed slice. Runtime
owner/loan/descriptor checks and ordinary bookkeeping remain in effect; whole
invocations are not promised to be allocation-free.

Field scanning is linear in its raw byte length; copying is linear in the raw
field body. Quotes are decoded during copying, with no payload scratch buffer.
The caller controls any destination allocation. Input plus caller output must
fit existing memory policy. This is a complete-buffer reader, not streaming.

Every internal cursor is between zero and input length. Incrementing a cursor
by one occurs only below length; advancing past a doubled quote or CRLF occurs
only after proving two bytes remain. Decoded length cannot exceed the raw
body length, which cannot exceed the input's representable i32 length.
No total-field or total-record counter is maintained by the library.

Returned NoCurrent and BufferTooSmall preserve the entire output. Successful
copy preserves the suffix. These are completed-call contracts, not
transactional guarantees for terminal fuel/runtime interruption during a
write. Existing limits and terminal failures are unchanged and not caught.
No input size is promised to complete under the execution budget.

## Validation status

This is a local source-only candidate. No compiler, formatter, interpreter,
Process application, native toolchain or CI result is claimed.

Independent acceptance should test the dialect table, exact malformed offsets,
empty/trailing delimiters, quoted embedded line endings, NUL/non-UTF-8 payload,
copy preservation and lifecycle. It should include consuming the last field
before End, retrying an undersized copy, and advancing without copying.
Changing a returned FieldInfo must have no effect on copying the current field.

Use ordinary path dependencies and public APIs. Keep tests and I/O outside the
library. See the separately prepared consumer handoff for the proposed
two-column numeric-table application and focused interpreter acceptance.
