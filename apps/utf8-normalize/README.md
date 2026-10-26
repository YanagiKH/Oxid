# utf8-normalize

Validate UTF-8 from standard input, convert CRLF and CR line endings to LF, and
write the result to standard output. Invalid UTF-8 returns status 65 without
writing output. All other characters, including NUL and a byte order mark, are
preserved. This tool does not perform Unicode normalization such as NFC.

With `oxid` on `PATH`, run these commands from the repository root:

```sh
cd apps/utf8-normalize
oxid lock --offline
oxid check --workspace --frozen
oxid build -p app --profile=release --frozen
app/.oxid/bin/utf8-normalize < input.txt > output.txt
```

Native builds require Linux x86_64 and LLVM 19.1.7. Set `OXID_LLVM_BIN` to the
directory containing `clang`, `opt`, and `ld.lld` if needed. The executable can
be copied elsewhere and run without Oxid or the application sources.

To run directly through Oxid:

```sh
cd app
oxid run --frozen < input.txt > output.txt
```

The application reads and validates all input before writing, then changes line
endings in the same byte buffer. Memory use grows with the input and remains
subject to the runtime memory limit. An output
error can leave a partial result; check the exit status before using the file.

| Exit status | Meaning |
| --- | --- |
| 0 | All normalized text was written. |
| 65 | The input is not valid UTF-8. |
| 70 | Buffer capacity would overflow. |
| 71 | Allocation failed or the memory limit was reached. |
| 74 | Reading or writing failed, or a write made no progress. |

The reusable [`utf8-text` library](../../packages/utf8_text/README.md) provides
validation, owned text, Unicode scalar appends, and truncation at character
boundaries. To run its API checks after creating the workspace lock:

```sh
cd contracts
oxid test --frozen
```
