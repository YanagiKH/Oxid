# utf8-normalize

Validate UTF-8 from standard input, convert CRLF and CR line endings to LF, and
write the result to standard output. The default mode returns status 65 and
produces no output for invalid UTF-8. All other characters, including NUL and a
byte order mark, are preserved. This tool does not perform Unicode normalization
such as NFC.

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

By default, the application validates all input before writing. Memory use
grows with the input and is subject to the runtime memory limit.

Pass `--stream` to process a large file or pipe.

```sh
app/.oxid/bin/utf8-normalize --stream < input.txt > output.txt
```

Streaming uses a fixed 4 KiB buffer. UTF-8 characters and CRLF pairs can span
reads. Earlier valid text may already be written when a later read contains
invalid UTF-8. An output error can also leave a partial result. Check the exit
status before using the file.

| Exit status | Meaning |
| --- | --- |
| 0 | All normalized text was written. |
| 65 | The input is not valid UTF-8. |
| 70 | Buffer capacity would overflow. |
| 71 | Allocation failed or the memory limit was reached. |
| 74 | Reading or writing failed, or a write made no progress. |

To send the workspace and its library for an offline rebuild, run this from
`apps/utf8-normalize` after creating the lock:

```sh
oxid package --workspace --frozen --output utf8-normalize.oxpkg.tar
```

Send the archive and the SHA-256 printed by Oxid. Use that checksum to unpack
into a new directory and build with the same LLVM version:

```sh
oxid package --extract utf8-normalize.oxpkg.tar --sha256 "$checksum" --output unpacked
cd unpacked/source/apps/utf8-normalize
oxid build -p app --profile=release --frozen
```

The reusable [`utf8-text` library](../../packages/utf8_text/README.md) provides
validation, owned text, Unicode scalar appends, and truncation at character
boundaries. To run its API checks after creating the workspace lock:

```sh
cd contracts
oxid test --frozen
```
