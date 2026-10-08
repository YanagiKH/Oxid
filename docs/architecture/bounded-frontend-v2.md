# Bounded scalar frontend v2: source-capacity successor

Status: experimental, opt-in Linux x86_64 qualification. This increment advances
bounded frontend input capacity; it does not complete the frontend roadmap,
expand its grammar, replace the default provider, or establish self-hosting.

## Observable behavior and reproduction

The existing explicit `--experimental-hir-producers BUNDLE` route accepts a
closed `OXID-HIR-PRODUCERS-2` manifest. V2 supports one retained root-only ASCII
scalar source of at most 255 bytes. A v1 bundle still refuses sources over 128
bytes. Ordinary/default providers are unchanged.

Build the v2 source closure and run a standalone end-to-end qualification:

```
python3 scripts/build_hir_producers_v2.py --compiler /path/to/oxid \
  --llvm-bin /path/to/llvm-19/bin --output /new/build-directory
python3 scripts/qualify_hir_producers_v2.py --compiler /path/to/oxid \
  --llvm-bin /path/to/llvm-19/bin --output /new/qualification-directory
python3 scripts/verify_hir_v2_edge_controls.py --compiler /path/to/oxid \
  --llvm-bin /path/to/llvm-19/bin --bundle /new/build-directory/bundle \
  --output /new/edge-control-directory
```

Each output directory must be new. The builder verifies every listed input
SHA-256 before copying a closed module set. Historical v1 sources are reused
byte-for-byte; changed v2 sources are explicit overlays. It does not patch a
compiler, raise backend budgets, or manufacture successful observation rows.
A digest is an identity assertion, not a signature or provenance authority.

## Version and capacity contract

V2 uses OPA2, AST2 and STF2. Byte layouts and OPA/STF sizes remain unchanged:
1,559 parser bytes and 2,607 combined bytes. AST2 contains the four-byte magic,
one source-length byte, exact source bytes and the exact parser observation.
The compiler and static consumer reject mixed versions. Wire decoding and
source binding check their selected version's cap, and every supplied fact is
still compared with genuine canonical source/AST/HIR before execution.

Source positions remain unsigned byte values. The exclusive endpoint 255 is
compared using explicit optional boundaries; it is never a missing-span
sentinel. Row and token sentinels remain separate and unchanged.

The source owner has 255 cells. A full first read requires a separate one-byte
EOF probe. A 256th byte or probe I/O error is a refusal, never truncation. Lexer
capacity is now explicit: at most 128 non-EOF tokens plus one reserved EOF
slot. Syntax rows remain at most 128 and expression depth at most 64. The
larger source cap chiefly permits longer trivia, spellings and moderately
larger programs within those independent bounds.

The version-selected supervisor input caps are 1,692 bytes for v1 and 1,819 for
v2. Output remains 2,607 bytes, stderr 4,096 bytes, and deadline five seconds.
The host AST transport backing bank grows by 127 bytes for either version and
is included in `hir_producer::named_bytes`; v1's accepted read ceiling does not
grow. Execution still uses identity-checked sealed executable snapshots and
bounded process cleanup. These guarantees are not an OS sandbox or total RSS
bound.

## Resource-accounting successor, including v1

This is a shared named-storage accounting successor. It is **not byte-admission
compatible with the earlier v1 layout**. Explicit protocol arguments, complete
call/result transports and optional endpoint carriers are charged for both
versions. Global retained, scratch and work ceilings are unchanged.

For the existing rich `main.ox` fixture, the directly measured fixed paid bank
changes from 138,865 to 139,939 bytes (+1,074). Its exact output-retained endpoint
changes from 153,190 to 154,264 bytes. V1's work endpoint remains 10,069,187 and
its emitted LLVM text remains 14,325 bytes. The former byte endpoint is now
explicitly tested as a refusal before candidate allocation. New exact/minus-one
scratch, retained and work tests remain, and ordinary default allowances pass.
The actual 255-byte v2 fixture independently measures 139,939 fixed bytes,
141,475 retained bytes, 690 emitted LLVM bytes and 1,004,093 work. Its retained
endpoint is dominated by canonical/candidate HIR coexistence, so the test uses
the larger of that bound and fixed-plus-output bytes; it tests every exact
endpoint and the corresponding minus-one refusal.

These figures describe the import leaf; producer/driver enclosing transports
are charged separately. They are not whole-process memory measurements.

Only source-dependent tariffs change by version. With B the source cap,
R=128 rows, T=C=129 cells and E=16R+1 events, source comparison pays:

```
173 + 2B + 30C + 5E + T*(3*(B+1)+258) + R*(B+80) + R
```

This is 124,501 for v1 and 190,160 for v2. Run/Emit root-entry work is
`256*(functions+B+2)`, a v2 increase of 32,512. Canonical resolution retains its
existing shared meter. The comparison's observational visit maximum rises from
2,690 to 2,817; that statistic is not the production work tariff.

The native diagnostic fixed render allowance rises from 4,096 to 8,192 for v2
only, covering the bounded source-location search/column scan. Human diagnostic
length remains `71+6*path_bytes`: one-based line and column numbers through 256
still have at most three decimal digits. Event tariffs, native inventories,
row banks, output size limits and exact count/reserve/render rules are unchanged.
These logical work tariffs are not timing or CPU guarantees.

## Verification scope

The local debug and release recipes each completed 58 retained commands. They
exercise real v2 parser/static compilation, 129/254/255-byte sources, comments,
newline-heavy spans, mutable-loop semantics, check/run/LLVM compile, native
execution and byte-identical manual-import artifacts. Refusals include 256-byte
source, non-ASCII, token exhaustion, malformed/mixed static framing, wrong
producer versions, and an actual source endpoint mutation. Authentic v1 bundles
also pass check/run/compile/native and retain their 129-byte refusal.

Rust checks cover source-owner/canonical fact binding, version mixing, endpoint
255 equality, source/entry/native tariffs, exact/minus-one admission, immutable
bundle execution and supervisor lifetimes. The source reviewer separately
exercised primary diagnostic [254,255], EOF hold-open/extra-byte behavior and
builder identity refusals. The focused edge script establishes a real parser
one-byte EOF-probe read after all 255 bytes were consumed, injects PTY EIO, and
checks a clean exit 74. A separate **synthetic diagnostic caller** checks the
secondary span [254,255]; it is not claimed as an accepted-source grammar case.

The checked-in `checked_hir_import_v2` fixture records real producer output and
its input/build digests. No external or supplied binary is accepted as compiler
authority. Qualification is local Linux x86_64 only; hosted CI, other platforms,
full-language conformance and full self-compilation are not established here.
