# RFC 0027: native byte admission and compiler inventories

Status: **accepted contract; production active; hosted qualification pending**.
Updated 2026-10-07. This is an explicit admission successor to RFC 0026.

## Contract

The owned native consumer retains the existing 1 MiB aggregate/live explicit
allocation-byte gates and replaces its aggregate/live reference-runtime expanded
cell gates with two inclusive whole-program compiler inventories:

- I = S + A + O + R + L + C, at most 8,192 declared storage/argument items
- W = sum aggregate_width(owner), at most 8,192 logical owner-width cells

S counts scalar locals and mutable places; A counts every call argument position,
including owned/borrow holes; O counts owners; R reference parameters; L loans;
C call descriptors. These are whole-program sums, including unused functions
and untaken branches. W uses existing checked nominal widths, including
empty-value sentinels, max(array length, 1), and enum width 2. These are
compiler-work inventories, not native byte units.

The previous native rule coupled `Xphysical = S + A + W + 4O + 10R + 14L + 2C`
at 8,192. I <= Xphysical and W <= Xphysical, so the new guards alone preserve
every old default-admitted program. Independent I/W intentionally admit larger
combinations, with combined inventory up to 16,384.
This changes admission; it is not an allocation optimization or unchanged rules.

## Physical storage and safety

Keep the checked NativeStoragePlan representation and all LLVM allocations:
`D = 8(S+A) + align4(B+T) + 8(R+L) + 4Q` per function, where B is the existing
owner byte arena including padding and sentinels; T is 1,024 private scratch
bytes for each canonical input or output builtin and 0 otherwise; Q is the
number of slice length slots. Aggregate bytes sum D across all functions. Live
bytes use the existing maximum acyclic call-path sum. Add the wrapper's eight-byte
fuel cell once in each budget for Process entry or when any function, including
an unused one, has cyclic or I/O-dependent cost. Otherwise that cell is absent.

This measures explicit LLVM object bytes, not backend spills, machine call
frames, host runtime memory or RSS. Keep the same ABI, alignments, owner/result
storage and pointer/length mappings. A limit of 8,192 eight-byte words is not
chosen: accepted programs can exceed 65,536 explicit bytes due to builtin scratch
or the wrapper while remaining within the existing 1 MiB byte limits.

All source association, raw shape/CFG/ownership proof, immutable witness binding,
policy/entry checks and runtime effect ordering remain mandatory. There is no
new source provider, syntax, alternate witness, privileged callback, reference
representation or storage reuse.

## Preserved limits and fuel

Keep existing function/parameter/ABI/per-function-slot, scalar-slot, block,
nonrecursive call-depth, static-cost, byte, metadata, diagnostic and IR limits.
Retain raw verification's independent work and scratch preflight, declaration
and projection depth limits, bounded enum expansion, and the early-stopping
count/render pipeline. I/W do not retroactively preflight allocations preceding
the native gates or replace the raw work ledger. Diagnostic prefix scans retain
their existing input-dependent bound; no new source-prefix cap is introduced.

FrameUsage.expanded_cells, reference storage and logical fuel stay unchanged.
Activation fuel remains `Xphysical - 2(R+L)`, and the call, transfer, return,
branch and I/O schedules are unchanged. Never substitute I, W or D into language
fuel. The reference consumer keeps its original limits.

## Order and diagnostics

Keep aggregate scalar/block checks first, then check I followed by W where the
old aggregate Xphysical check stood. Remove the native live-Xphysical denial
while preserving all other graph/cost/depth/live-scalar/byte and later
metadata/diagnostic checks.
Keep Phase 1's final independent storage reconciliation before LLVM count/render;
its equality proof validates the immutable byte totals used by prior prechecks.
Measure any changed fixed carriers without introducing a retained table.

New limit names are `aggregate compiler inventory items` and
`aggregate owner width cells`, under existing E0700/native-admission formatting.
Existing earlier failures retain precedence. Former-Xphysical denials can reach
later existing failures or succeed; retain exact old/new observations as named
outcomes. Do not normalize arbitrary E0700 differences or silently route current
controls through historical policy. Frozen authorities/oracles remain intact.

## Local evidence and remaining gates

Production activation is `189cbed0d2216fb53816a6efe72bcdbc4eb5147e`; the tested
compiler source, including its test successor, is
`ffa2e00543b7a1958321b719677ed3b48f42bc74`. Local Linux x86_64 checks with
LLVM/Clang/LLD 19.1.7 at O0 record:

- Full default unit suite: 1,490 passed, 54 ignored
- Existing LLVM controls: 52 passed; all 121 retained artifacts are byte-identical,
  comprising 52 LLVM modules plus raw, source, diagnostic and inventory artifacts
- Inventory controls: 16 passed, covering independent exact/one-over I/W bounds,
  checked accounting, earlier failure precedence and retained later gates
- An ordinary source case with I = 1,359, W = 2 and historical Xphysical = 9,995
  returns 7 through reference/native execution; production ELF fuel boundaries
  match exactly at 26,127 (failure), 26,128 and 26,129 (success)
- All four existing scratch/wrapper controls pass ordinary native compilation
  with byte-identical LLVM, preserving acceptance above the 65,536-byte substitute

The unchanged partial parser source at
`8f1fe2035a0513f33fca2897ed5b2285ef83179e` now passes normal production native
CLI admission and produces an ELF. Its I = 4,646, W = 2,571 and 48,864 explicit
bytes fit the retained gates; 54 reference/native corpus cases match, with 41
separate malformed strict-decoder controls. These results establish the formerly
unproven later gates for this source. They do not complete the full typed grammar or switch a production
compiler provider.

Here, source-free ELF execution means execution from a working directory
containing only the ELF, with a controlled environment. It does not establish
filesystem isolation or an OS sandbox. Historical qualification ledgers and frozen oracles retain their
original source and admission identities; this section records their explicit
successor. Capability qualification and exact-head hosted CI remain pending.
Parser expansion waits for that qualification; this RFC authorizes no cap
increase or grammar narrowing.
