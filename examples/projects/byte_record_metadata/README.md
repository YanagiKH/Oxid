# Byte-record PNG header metadata example (candidate)

This two-package example reads the first 33 bytes of a PNG-like stream, checks its signature and IHDR length/type, and returns its positive signed-i32 width. The library retains the binary signature and metadata in an opaque owned record: [u8; 8], two u8 fields, and two i32 dimensions. It returns that record through an owned enum. Getters and a projected byte-slice helper operate through ordinary record borrows.

This is an IHDR metadata reader, not a PNG decoder or file validator. It does not verify CRC, permitted depth/color combinations, compression/filter/interlace values, subsequent chunks or pixel data. It reads only the fixed header, so extra input is not rejected. Dimensions above i32::MAX and zero dimensions are rejected by this candidate. No general file-I/O or growing-buffer support is claimed.

From the repository root, after this candidate is compiled:

    oxid check --manifest-path examples/projects/byte_record_metadata/app/oxid.toml
    oxid run --manifest-path examples/projects/byte_record_metadata/app/oxid.toml < image.png
    oxid run --manifest-path examples/projects/byte_record_metadata/app/oxid.toml --engine native < image.png

The native selector matches the existing explicit-manifest RUN contract. Build/package use the existing explicit-manifest routes and Linux x86_64 / LLVM 19.1.7 qualification limits.

Result-mode output is the decimal width plus newline. Error return values are -1 (short header), -2 (invalid byte-valued library input), -3 (signature), -4 (IHDR length/type), -5 (zero or out-of-range dimensions), and -6 (stdin I/O error). A returned negative integer is still a successful Result-mode program execution; do not confuse it with a compiler failure or Process-mode exit code.

Fixtures.json records hexadecimal inputs and fixed expected Result output for independent CLI checks. The fixtures include a correct CRC, but the implementation deliberately does not validate it. No Rust/Oxid execution has been performed in the authoring environment at this stage.
