# byte-reverse

Read binary data from standard input, reverse its bytes, and write the result to
standard output. Every byte is preserved, including NUL and values above 127.
There is no text decoding or newline conversion.

From the repository root, with `oxid` on `PATH`:

```sh
cd apps/byte-reverse
oxid lock --offline
oxid check --workspace --frozen
oxid build --workspace --profile=release --frozen
app/.oxid/bin/byte-reverse < input.bin > reversed.bin
```

Native builds currently require Linux x86_64 and LLVM 19.1.7. Set
`OXID_LLVM_BIN` to the directory containing `clang`, `opt`, and `ld.lld` if they
are not on `PATH`. The executable can be copied elsewhere and run without the
Oxid compiler or application sources.

To run through the compiler without building a native executable:

```sh
cd app
oxid run --frozen < input.bin > reversed.bin
```

The application reads all input before writing output, so its memory use grows
with the input. Runtime memory and execution limits still apply. Empty input
produces empty output. An output error can leave a partial result; check the
exit status before using the output file.

| Exit status | Meaning |
| --- | --- |
| 0 | All reversed bytes were written. |
| 70 | The buffer's capacity would overflow. |
| 71 | Allocation failed or the runtime memory limit was reached. |
| 74 | Reading or writing failed, or a write made no progress. |

The `binary` library owns the growable buffer and handles reads and partial
writes. The `reverse` library reverses a mutable byte slice. The application
connects them through its two local dependencies.
