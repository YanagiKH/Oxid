# Characters

Status: experimental, `typed-preview`.

Use `char` for one Unicode scalar value. Put the character between single quotes:

```text
fn main() -> char {
    return '🦀';
}
```

This prints `🦀`. Characters can be copied, compared, passed to functions and
stored in records, fixed arrays, slices and enum payloads. A `char` occupies
four bytes with four-byte alignment.

## Literals and escapes

A literal contains exactly one Unicode scalar. `'a'`, `'中'`, `'🦀'` and
`'\u{1f980}'` are valid. A visible symbol composed of multiple scalars, such
as a letter followed by a combining accent, requires a string representation.
Empty literals and literals containing multiple scalars are checking errors.

| Escape | Value |
| --- | --- |
| `\n` | Line feed |
| `\r` | Carriage return |
| `\t` | Tab |
| `\0` | NUL |
| `\\` | Backslash |
| `\'` | Single quote |
| `\"` | Double quote |
| `\xNN` | ASCII value from `00` through `7f`, using exactly two hexadecimal digits |
| `\u{N}` | Unicode scalar, using one through six hexadecimal digits |

Raw control characters require an escape. Unicode escapes exclude surrogate
values `d800` through `dfff` and values above `10ffff`. Hexadecimal digits are
case insensitive; underscores are not accepted in escapes.

## Comparisons and conversions

`==`, `!=`, `<`, `<=`, `>` and `>=` compare Unicode scalar values. Characters
do not support arithmetic or integer bit operations. Character ordering does
not perform language-specific alphabetical sorting or Unicode normalization.

Convert a named integer with `.to_char_checked()`. Every supported integer
type is accepted. A negative value, surrogate or value above `0x10ffff` reports
E0612 when executed. The complete integer is checked before any narrowing.

```text
fn main() -> char {
    let code = 129408u32;
    return code.to_char_checked();
}
```

This also prints `🦀`. Use `.to_u32()` on a named character to obtain its
Unicode scalar value. Conversions are explicit; a character does not become
a number through a type annotation. Store a computed receiver in a variable
before calling either conversion method.

## Output and execution

Result-mode execution writes the character as UTF-8 followed by a newline.
JSON results use a string containing one scalar, with JSON escapes for control
characters, quotes and backslashes. NUL is preserved. Process entries still
return `i32`.

Reference execution supports both execution policies. Native builds support
Linux x86_64 with LLVM/Clang/LLD 19.1.7 in both build profiles. A deployed
program needs neither its source files nor an Oxid installation. Existing
libc, resource and borrowing requirements apply.
