# Typed byte literals client

This ordinary `typed-preview` path-dependency consumer exercises adjacent
lowercase `b"..."` byte literals. Each literal is the existing owned
`[u8; N]` array; `b""` is `[u8; 0]`. It adds no compiler operation, runtime
code, borrowing form or UTF-8 string type.

Direct characters are printable ASCII only. Escapes are `\\`, `\"`, `\n`,
`\r`, `\t`, `\0` and `\x` followed by exactly two ASCII hex digits (either
case, `00`-`FF`). Every example byte is pinned against an existing
`[byte(n), ...]` array in `src/main.ox`.

Literals are bound to names before borrowing: a named array is borrowed by
`alloc::bytes::try_extend` and checked through the ordinary
[`packages/utf8_text`](../../../packages/utf8_text) validation. `b"\xFF"` is a
legal byte array that `from_bytes` must refuse, returning the original owner.

Result-mode execution prints exactly `42`. Run it with:

```sh
oxid check --manifest-path=oxid.toml
oxid run --manifest-path=oxid.toml --engine=interpreter --entry-mode=result
oxid build --manifest-path=oxid.toml --entry-mode=result
```

Native build requires LLVM 19.1.7 through `OXID_LLVM_BIN`; there is no
fallback executor. Acceptance lives in `tests/typed_byte_literals.rs`.

## Focused validation

The compiler's 31 unique byte-literal unit tests passed, including four
byte-atom guards. The ordinary integration suite passed four tests (three
feature tests and a JSON-quoting helper), covering the fixed positive
example, existing type/ownership refusals, and 17 exact diagnostic records.
The explicitly selected ignored native test built with LLVM 19.1.7 and
ran the standalone artifact with cwd `/` and an empty environment, yielding
exact `42\n`, exit 0, and empty stderr.

Subsequent mechanical formatting was independently checked. The final
three-file lint-only increment passed scoped Clippy with warnings denied
and formatting checks; functional tests were not repeated for that increment.
These results do not establish full repository CI or capacity/fuel,
allocation-failure, terminal-failure, or process qualification. See
[the byte-literal specification](../../../spec/byte-literals.md).
