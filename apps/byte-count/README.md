# Byte count

An experimental typed application built from three local packages. It reads
stdin in 32-byte chunks and prints `<newlines> <bytes>\n`. A final line without a
newline contributes bytes but no newline, like `wc -l -c`. NUL and non-UTF-8
bytes are counted without decoding.

From this directory, with the development Oxid compiler on PATH:

```sh
oxid check
oxid test
printf 'hello\nworld\n' | oxid run
oxid build
printf 'hello\nworld\n' | ./.oxid/bin/byte-count
```

Both executions print `2 12`. Native building requires Linux x86_64 and LLVM,
Clang and LLD 19.1.7. The executable can be copied elsewhere and run without
Oxid or the source packages; it retains its normal system library dependencies.

The application and `report` package both depend on `counts`. Their references
to the owned `Totals` record resolve to the same package identity. The record's
fields stay private; consumers use the public functions. Each library resolves
`crate::` from its own package root.

This application uses the existing bounded execution policy and checked i32
arithmetic. It is suitable for small inputs; exhausting fuel or overflowing a
counter fails rather than silently producing a partial result. Input errors
return 2, output errors return 3, and ordinary success returns 0. It does not
claim the roadmap's full production CLI acceptance or general file I/O.

`scripts/verify_typed_package_app.py` from the repository root checks the
interpreter against independent byte-count expectations; `--native` also
checks rebuilds, failed-build artifact preservation and source-free deployment.
