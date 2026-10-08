# Experimental bounded frontend v2

This opt-in source overlay extends the scalar producer source capacity from
128 to 255 ASCII bytes. It does not extend the grammar, replace a default
provider, or establish self-hosting. Historical v1 source files are unchanged.

Build using `python3 scripts/build_hir_producers_v2.py --compiler PATH --llvm-bin
PATH --output NEW_DIRECTORY`. The builder verifies every explicit source digest,
materializes a closed set of shared v1 sources plus v2 overlays without rewriting
them, and compiles the parser and static consumer through the ordinary LLVM
process-entry route. Output includes source identities, commands, diagnostics,
executable digests, and the local identity manifest.

The manifest header is `OXID-HIR-PRODUCERS-2`. Parser output is OPA2; static input
is AST2 (four-byte magic, one-byte source length, exact original source, OPA2),
and successful static output is the unchanged OPA2 followed by STF2. Layouts and
frame sizes remain 1,559 and 2,607 bytes. Mixed versions are invalid. The static
consumer independently validates source/AST correspondence before resolution
and typing; the compiler must still bind the exact original source and compare
against its genuine checked program. Wire versions convey no trust.

Only source storage changes: a 255-cell source owner and a separate one-byte
EOF probe preserve the existing 256 native scalar/owner-slot cap. A full first
read is accepted only after the probe returns zero-byte EOF. Byte 256, non-ASCII,
I/O errors, and more than 128 non-EOF tokens are refused without truncation.
The token bank remains 129 cells including EOF; syntax rows stay at 128 and
expression depth at 64. No global compiler or runtime budget is raised. The v2 supervisor input
ceiling is exactly 1,819 bytes for AST2, versus v1’s unchanged 1,692 bytes;
stdout, stderr and deadline ceilings are unchanged. Producer execution remains bounded transport, not an OS sandbox or a
whole-process RSS guarantee.

Source offsets are byte-wide, including the legal exclusive endpoint 255.
Version-specific compiler admission must use explicit endpoint optionality,
never treat byte 255 as a missing span, and charge source-dependent work at the
selected version's cap. Native and compiler integration are qualified separately
from merely building or running these producer executables.
