# Native physical storage observer v2

This reviewer-owned Python tooling is separable from its evidence directory. It
does not invoke a compiler, LLVM tool, runtime or subprocess. Its only inputs are
frozen emitted LLVM text, reviewer fixture identities and explicit source hashes.
It changes observation copies only and reconstructs the original complete module
after removing each insertion and reversing the recorded alloca rebase.

The unchanged frozen chain fixture constructs three seed values in order. For
i32 they are -91, -54 and -17; for bool they are true, false and true. The v2
manifest binds every destination to its independently expected seed:

| Destination operation | i32 | bool |
| --- | ---: | --- |
| Construct Local owner0; MoveInitialize owner4 | -91 | true |
| Construct Temporary owner2; Replace moved owner0 | -54 | false |
| Construct Temporary owner6; Replace available owner0 | -17 | true |
| PrepareOwned owner8; incoming parameter; ReturnOwned owner10 | -17 | true |
| Self-replacement via Temporary owner12 and owner0 | -17 | true |

Every byte of every nonempty i32 payload is checked against the fixed repeated
little-endian bytes for its seed, after whole-destination 0xA5 poisoning. Native
qualification remains pinned to Linux x86_64. Empty i32 checks all four zero
sentinel bytes. Empty bool/unit checks one zero sentinel byte. Nonempty unit
checks every canonical zero byte.

Nonempty bool checks each cell with `load volatile i1` and an i1 comparison. It
never loads or compares the payload's upper seven bits and makes no claim that
those bits are canonical. Its whole-byte poison is 0xA4 for expected true and
0xA5 for expected false, so the low bit always opposes the expected result. Both
guards remain 0x96/0x69 and are initialized before checking them.

For extents larger than 16 bytes, each operation emits only three checking calls:
two guard checks and one complete-payload loop. The loop compares each byte to a
fixed four-byte pattern, or each i1 to the expected bool. These helpers inspect
storage only; they do not evaluate raw array operations, update ownership, copy
arrays or replace any production operation. The N1024 observation modules are
about 2.5 MB each, dominated by the original scalar-wise production expansion;
the new complete-payload checking adds approximately 5 KB. V2's unit N1024
module shrinks from 6.15 MB to 2.51 MB by replacing unrolled observer calls.

## Replaying preparation

No Python dependencies beyond the standard library are required.

```
python3 -B -m unittest -v test_observe_storage.py
python3 -B prepare_checkpoint2.py \
  --input /path/to/frozen/reviewer-chain-modules \
  --output /path/to/new/observation-bundle \
  --source-commit EXACT_SOURCE_COMMIT
python3 -B verify_checkpoint2.py /path/to/new/observation-bundle
```

The preparation input is exactly the 18 reviewer chain modules named
`chain-{i32,bool,unit}-n{0,1,4,1024}.ll` and
`selfchain-{i32,bool,unit}-n{0,4}.ll`. Their raw fixture has interleaved ordinary
record owners and a separate one-owner callee. The binder rejects any altered
owner layout, call binding or expected operation ordinal. It freezes and records
input SHA-256 hashes before deriving output. Both directories and the commit are
configurable; no machine-specific absolute directory is built into a tool.

`bind_chain.py` can also prepare one nominated exact module:

```
python3 -B bind_chain.py frozen.ll new-bundle \
  --sha256 EXACT_MODULE_SHA --element i32 --length 0 --case chain_i32_n0 \
  --mutant construct_local --mutant move_initialize
```

Use `--self-chain` for the reviewer self-replacement extension. A reviewed JSON
manifest can instead use `observe_storage.py prepare`; independently verify any
new fixture binding before qualification.

## Outputs and actual execution

The bundle contains an exact five-column TSV:

```
case<TAB>module<TAB>status<TAB>stdout_hex<TAB>stderr_hex
```

The unchanged existing Scratch harness compiles the relative module paths using
pinned LLVM 19.1.7/O0, retains commands/tool/executable hashes, then executes the
ELF source-free with tools absent from PATH. The TSV has 18 positive cases and
the same 11 empty-i32 one-byte mutants as v1. Mutants only narrow one nominated
zero store, or a type-consistent load/store pair, from i32 to i8. All 11 mutant
LLVM modules remain byte-identical to v1; their exact operation-specific fatal
messages still identify surviving poison at payload byte 1. All 29 expected
statuses/stdout/stderr stay unchanged.

Each case has untouched production text, observed text, hashes, exact operation
substring comparisons and reversible insertion receipts. Mutants additionally
retain the uninstrumented mutant and precise mutation/reversal receipts. The
JSON separates runtime payload-byte, i1-cell and guard counts from static call
counts. It records all evidence as prepared/unexecuted until actual harness
results establish the claimed outcome.

V1 artifacts and tooling remain frozen separately. `v1-v2-provenance.json`
records their verified hashes, unchanged expected outcomes, module sizes and
which modules are byte-identical. `verify-checkpoint2-v2.json` records the fresh
disk reconstruction audit. At preparation, 31 text-only tooling tests pass;
these tests are not LLVM/native qualification.
