# Experimental binary reversal application

Experimental integration candidate for RFC0032. Focused Linux x86_64 / LLVM
19.1.7 native package/deployment acceptance passed for the seven fixed inputs
below; broader qualification and stable v1 acceptance remain pending. The `binary`
library reads initialized 1,024-byte chunks into a growable `ByteBuf` and returns
`Loaded::Ok(Document)`. The `reverse` package reverses a mutable slice. The
process application writes all bytes through a loop that handles partial
progress, retries Linux EINTR (4), and rejects a zero-progress write.

From the repository root:

```sh
oxid run --manifest-path examples/projects/oxrev/oxid.toml --package oxrev --engine interpreter --entry-mode process < input.bin > reversed.bin
oxid package --manifest-path examples/projects/oxrev/oxid.toml --package oxrev --entry-mode process --output /tmp/oxrev.tar
```

The package command requires the supported Linux x86_64 native toolchain.
Extract the archive into a clean deployment directory and run `bin/oxrev` there;
the executable needs neither sources nor the compiler checkout. Success exits
0 with only reversed binary bytes on stdout. Capacity overflow exits 70,
allocation or policy refusal exits 71, and input/output failure exits 74.

Focused acceptance, with artifacts outside the checkout:

```sh
python3 scripts/accept_oxrev.py target/debug/oxid --engine interpreter --evidence /tmp/oxrev-reference
OXID_LLVM_BIN=/path/to/llvm-19.1.7/bin python3 scripts/accept_oxrev.py target/debug/oxid --engine native --evidence /tmp/oxrev-native
```

Each evidence directory must be new. Inputs and independent reversal oracles
cover 0, 1, 3 (NUL/0x80/0xFF), 1,023, 1,024, 1,025 and 8,199 bytes. Runtime limits
still apply; the separate dynamic policy charges live requested capacities
plus 32 bytes per allocation, including growth's old-plus-new peak. It is not
an RSS guarantee or a promise that arbitrary input sizes fit existing fuel.
