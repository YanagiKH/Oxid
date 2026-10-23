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
use crate::text::append_scalar;
use crate::text::byte_len;
use crate::text::ScalarAppendStatus;

fn main() -> i32 {
    let mut greeting = new_text();
    let status = append_scalar(&mut greeting, 128075);
    match status {
        ScalarAppendStatus::Ok => { return byte_len(&greeting); },
        ScalarAppendStatus::InvalidScalar(value) => { return -1; },
        ScalarAppendStatus::CapacityOverflow => { return -2; },
        ScalarAppendStatus::AllocationFailed => { return -3; },
        ScalarAppendStatus::MemoryLimit => { return -4; },
    }
}
```

This appends U+1F44B (the waving hand emoji) and returns its UTF-8 byte length,
4. Other match arms handle invalid scalar values and allocation errors.

| Function | Behavior |
| --- | --- |
| `validate(&[u8])` | Accept UTF-8 or return the first invalid sequence's byte offset. |
| `new_text()` | Create empty text. |
| `from_bytes(ByteBuf)` | Move valid bytes into text; return the original buffer on invalid input. |
| `byte_len(&Text)` | Count UTF-8 bytes. |
| `scalar_count(&Text)` | Count Unicode scalar values, rather than grapheme clusters. |
| `append_utf8(&mut Text, &[u8])` | Validate the complete input before appending it. |
| `append_scalar(&mut Text, i32)` | Encode and append one Unicode scalar value. |
| `equals_utf8(&Text, &[u8])` | Compare the initialized bytes exactly. |
| `truncate(&mut Text, i32)` | Shorten at a scalar boundary; return `false` and preserve the text otherwise. |
| `append_to_bytes(&Text, &mut ByteBuf)` | Append a copy of the text bytes to a byte buffer. |

Append failures preserve the previous text. UTF-8 validation rejects overlong
encodings, surrogate code points, values above U+10FFFF, and incomplete
sequences. It accepts NUL, byte order marks, and Unicode noncharacters.
The library does not normalize text or group grapheme clusters. Allocations
share the runtime memory limit with other byte buffers.

See [`utf8-normalize`](../../apps/utf8-normalize/README.md) for a complete
application that reads, validates, transforms, and writes UTF-8 text.
