# Frame filter

A small binary stream tool that validates four-byte records, sorts numeric
pairs, preserves raw byte pairs, and removes padding records.

Build and run on Linux x86_64 with Oxid and LLVM 19.1.7 installed:

```sh
cd apps/frame-filter
oxid lock
oxid check --frozen
oxid test --frozen
oxid build --profile=release --frozen
printf '\001\011\003\015\002\000\377\001' | .oxid/bin/frame-filter | od -An -tu1
# 1 3 9 13 2 0 255 1
```

The executable reads stdin and writes stdout. Each input record is exactly
four bytes: `tag`, `first`, `second`, and `(tag + first + second) modulo 256`.
There is no header or delimiter. All values are unsigned bytes.

| Tag | Meaning | Output |
| --- | --- | --- |
| 0 | Padding | Removed, after validating its checksum |
| 1 | Numeric pair | The two values in ascending order, with tag and checksum |
| 2 | Raw pair | The original four bytes |

Exit status is 0 at a record boundary at EOF, 64 for an unknown tag, 65 for a
checksum mismatch, 66 for a truncated record, 74 for a read failure, or 75 for a
write failure. The checksum is checked before the tag. Processing stops at the
first error; output from earlier records remains. A failed write can leave a
partial record in the destination. Diagnostics are reserved for runtime errors.

`oxid run --frozen < input.bin > output.bin` runs the same program. Native builds
retain Oxid's bounded execution limits, so this example is intended for small
streams. Use `--profile=dev` for an unoptimized build.

The `codec` package owns the decoded pair and nested enum types. The `output`
package consumes those values, including fixed arrays, and shares `codec` with
the application through a local path dependency.
