# utf8-text

Owned UTF-8 text for Oxid projects using `edition = "typed-preview"`. `Text`
keeps its bytes private so validation, appends, and truncation preserve valid
UTF-8. Values move between owners and can be borrowed by functions.

Add a local dependency to your project's `oxid.toml`, adjusting the path:

```toml
[dependencies]
text = "../../packages/utf8_text"
```

```rust
use crate::text::new_text;
use crate::text::append_utf8;
use crate::text::byte_len;
use crate::text::AppendStatus;

fn main() -> i32 {
    let mut greeting = new_text();
    let bytes = b"Hello, Oxid 👋";
    let status = append_utf8(&mut greeting, &bytes);
    match status {
        AppendStatus::Ok => { return byte_len(&greeting); },
        AppendStatus::InvalidInput(offset) => { return -1; },
        AppendStatus::CapacityOverflow => { return -2; },
        AppendStatus::AllocationFailed => { return -3; },
        AppendStatus::MemoryLimit => { return -4; },
    }
}
```

This appends the greeting and returns its UTF-8 byte length, 16. A `b"..."`
literal produces a fixed byte array; `&bytes` borrows it for the call. Other
match arms handle invalid UTF-8 and allocation errors.

| Function | Behavior |
| --- | --- |
| `validate(&[u8])` | Accept UTF-8 or return the first invalid sequence's byte offset. |
| `new_text()` | Create empty text. |
| `from_bytes(ByteBuf)` | Move valid bytes into text; return the original buffer on invalid input. |
| `into_bytes(Text)` | Consume the text and return its byte buffer without copying or allocating. |
| `byte_len(&Text)` | Count UTF-8 bytes. |
| `scalar_count(&Text)` | Count Unicode scalar values, rather than grapheme clusters. |
| `append_utf8(&mut Text, &[u8])` | Validate the complete input before appending it. |
| `append_scalar(&mut Text, i32)` | Encode and append one Unicode scalar value. |
| `append_char(&mut Text, char)` | Append one character, preserving valid UTF-8. |
| `char_at(&Text, i32)` | Read a character at a byte offset and return its next byte offset. |
| `equals_utf8(&Text, &[u8])` | Compare the initialized bytes exactly. |
| `truncate(&mut Text, i32)` | Shorten at a scalar boundary; return `false` and preserve the text otherwise. |
| `append_to_bytes(&Text, &mut ByteBuf)` | Append a copy of the text bytes to a byte buffer. |

Append failures preserve the previous text. UTF-8 validation rejects overlong
encodings, surrogate code points, values above U+10FFFF, and incomplete
sequences. It accepts NUL, byte order marks, and Unicode noncharacters.
The library does not normalize text or group grapheme clusters. Allocations
share the runtime memory limit with other byte buffers.

To walk through characters, call `char_at` starting at byte offset zero.
`CharacterRead::Found(character)` returns `character.value` as `char` and
`character.next_byte` for the next call. `End` means the offset equals the
text's byte length. `InvalidOffset` means the offset is negative, past the end,
or inside a character's UTF-8 encoding. Reading leaves the text available for
another read, append or truncation. A letter and a separate combining accent
are two characters.

`append_char` returns the same `AppendStatus` as `append_utf8`. A `char` is
already a valid Unicode scalar; allocation and capacity failures still leave
the previous text intact.

See [`utf8-normalize`](../../apps/utf8-normalize/README.md) for a complete
application that reads, validates, transforms, and writes UTF-8 text.
