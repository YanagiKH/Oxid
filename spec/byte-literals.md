# Byte literals

Status: implemented in typed-preview, with focused interpreter and native validation.

## Value and type

An adjacent lowercase `b"..."` expression constructs an owned fixed-size byte array of type `[u8; N]`. `N` is the number of decoded bytes, not the number of source characters or source bytes in the spelling. Each accepted raw character or escape contributes exactly one element. No terminator is appended.

The existing fixed-array limit applies: `0 <= N <= 1024`. The literal does not introduce a new string type, a dynamically sized owned buffer, or a borrowed value.

```oxid
fn main() -> i32 {
    let bytes = b"A\0\xFF";
    let byte = bytes[2];
    return byte.to_i32();
}
```

Here `bytes` has type `[u8; 3]`, contains the values 65, 0, and 255, and the function returns 255. Indexing and `to_i32()` use the existing byte-array and byte-scalar operations.

An explicit annotation must agree with both the element type and the decoded length. For example, `let bytes: [u8; 1] = b"A";` agrees; `[i32; 1]` and `[u8; 2]` do not.

## Accepted spelling

The prefix is exactly lowercase `b`, immediately followed by the opening double quote. Whitespace or comments between the prefix and quote are not allowed.

Inside the quotes, raw printable ASCII bytes from `0x20` through `0x7E` are accepted, except that double quote and backslash must be escaped. A raw apostrophe is allowed.

The complete escape set is:

| Source spelling | Decoded byte |
| --- | --- |
| `\\` | `0x5C` (backslash) |
| `\"` | `0x22` (double quote) |
| `\n` | `0x0A` (line feed) |
| `\r` | `0x0D` (carriage return) |
| `\t` | `0x09` (tab) |
| `\0` | `0x00` |
| `\xHH` | The byte specified by exactly two ASCII hexadecimal digits |

Hexadecimal digits may be `0`–`9`, `a`–`f`, or `A`–`F`. Thus `b"\xFF"` and `b"\xff"` have the same one-byte value. The two-digit escape consumes exactly two digits: `b"\x414"` contains `0x41` followed by the raw ASCII byte `4`.

The following are outside the accepted grammar:

- Raw control characters, including tab, newline, and DEL.
- Raw non-ASCII characters, including `b"é"`.
- Unicode escapes, such as `b"\u{41}"`.
- Unknown escapes, including `\q` and `\'`.
- Backslash-newline continuation.
- Raw-string prefixes, including `br"..."` and `br#"..."#`.
- Uppercase or separated prefixes, such as `B"a"`, `b "a"`, and `b/**/"a"`.
- Literal suffixes, including `b"a"u8`.
- Implicit concatenation of adjacent literals.

Byte values above `0x7F` are available through hexadecimal escapes. The result carries no UTF-8 validity guarantee and performs no Unicode encoding or normalization. For example, `b"\xFF"` is a byte array even though its contents are not valid UTF-8 text. This feature adds no `String`, text conversion, or text-processing API.

## Empty literals and ownership

`b""` intrinsically has type `[u8; 0]`; no annotation is needed to infer its element type.

```oxid
fn main() -> i32 {
    let bytes = b"";
    let moved = bytes;
    return moved.len();
}
```

The result is zero. Ownership and moves follow the existing fixed-array rules.

Ordinary empty-array inference is unchanged. `let bytes = [];` still requires an explicit array annotation on its local initializer. The existing form `let bytes: [u8; 0] = [];` remains separate from the intrinsic type of `b""`.

Borrow a named array through the existing borrowing rules:

```oxid
fn length(bytes: &[u8]) -> i32 {
    return bytes.len();
}

fn main() -> i32 {
    let bytes = b"abc";
    return length(&bytes);
}
```

Direct borrowing of a literal, such as `&b"abc"`, is not added. Existing fixed-array moves, indexing, length, mutation, and named shared or mutable borrows retain their existing restrictions. No new methods or extended lifetime rules are introduced.

## Diagnostics

Malformed byte-literal contents are parse errors with code `E0101`. The primary span identifies the offending original source atom rather than a position in a decoded buffer:

- Unknown escapes report `unsupported or incomplete byte literal escape`, covering the backslash and the unsupported escape selector. For `\u{41}`, the primary span covers `\u`.
- Invalid or short hexadecimal escapes report `byte literal hex escapes require exactly two ASCII hex digits`. A complete malformed candidate such as `\xG0` covers all four source bytes. A short candidate such as `\x4` covers the available interior escape bytes, excluding the closing quote. If a malformed candidate's endpoint would split a non-ASCII character, the span extends to that character's end without including the closing quote.
- Raw non-ASCII input reports `byte literals accept raw printable ASCII only`, covering the first offending character's complete UTF-8 encoding.
- Raw controls report `byte literals require escapes for control bytes and newlines`, covering the first offending control byte.
- An attached suffix reports `byte literal suffixes are unsupported`, covering its first attached alphanumeric or underscore character.

An unterminated literal is a lexical error, `E0100`, with message `unterminated string literal`; its primary span starts at the `b` and extends to source end. Separated, uppercase, and raw-string prefixes are not recognized as byte literals and can instead receive the existing unsupported typed-preview construct diagnostic on the following ordinary string token.

Source offsets are zero-based, half-open UTF-8 byte ranges. Displayed line and column locations are one-based Unicode-scalar locations. These rules describe the specified malformed-spelling cases, not diagnostic behavior for resource-exhaustion or error-adaptation paths.

## Lowering and verification boundary

The original AST and HIR retain a compact literal description, including decoded count, expression identity, and original source span. A borrowed scanner is replayed during parsing, lowering, and independent authentication; it does not allocate a decoded byte vector.

For a nonempty literal, lowering emits one `ByteConst` assignment per byte, followed by the existing `StorageLive` and `ConstructArray` sequence. Each assignment records its original raw-character or escape span. The array's operands preserve the decoded order.

Before these byte constants can be consumed, independent source association checks:

- Structural membership of the original AST occurrence in its function, rather than relying only on function-span containment.
- Original literal spelling, decoded count, byte values, and per-byte source spans.
- `u8` temporary declarations and the exact contiguous assignment, lifecycle, and array-construction correspondence.
- Coverage of emitted byte constants and rejection of repeated use of the same source occurrence as a new authentication claim.

The existing conversion-authentication gate remains unchanged. A raw `ByteConst` value or matching public-looking fields alone do not establish source authority. Unsupported scalar, imported, synthetic, and predecessor observation routes do not gain byte-constant authority from this feature. Interpreter and native consumers use existing `u8`/`i8` storage only after the required association and verification boundary.

The empty literal emits no `ByteConst` assignment. It uses ordinary zero-length fixed-array construction and verification. This does not add an array-construction origin field or establish completeness of empty-literal source-origin authentication.

## Resource scope and limitations

Existing source, token, node, fixed-array, and other admission ceilings remain in force. New payload sizes flow through the existing symbolic `size_of` inventories. Lowering and authentication charge scanner work on their actual passes; the scanner itself uses borrowed constant-space state.

Parser `SyntaxStorage` accounting covers syntax `Vec` capacities alongside existing source, token, and node bounds. It does not account fixed stack carriers or meter parser operations. The implementation therefore makes no stronger parser CPU or stack guarantee. Named carriers, membership and replay scratch, and fallible reservation capacity in the lower/association paths are admitted under the existing ceilings.

These implementation properties are not a broad resource-qualification result. This feature does not claim full-suite, allocation-failure, capacity/fuel, terminal-failure, or process qualification.

## Validation status

Focused validation passed on the tested implementation revision:

- 31 unique unit tests, including four byte-atom guard tests.
- Four ordinary integration tests: three feature tests and one JSON-quoting helper test.
- One normally ignored native-artifact test, explicitly selected and run.

Subsequent mechanical formatting was independently checked to preserve the tested implementation. Subsequent lint-only changes passed scoped Clippy with warnings denied and formatting checks. The test results above describe the tested revision; they are not a claim that runtime tests were rerun after those later changes. No full-suite or broader resource-qualification result is implied.
