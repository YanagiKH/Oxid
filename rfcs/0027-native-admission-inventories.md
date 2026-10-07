# RFC 0027: native byte admission and compiler inventories

Status: **accepted contract; production activation under qualification**.
Updated 2026-10-07. This is an explicit admission successor to RFC 0026.

## Contract

The owned native consumer retains the existing 1 MiB aggregate/live explicit
allocation-byte gates and replaces its aggregate/live reference-runtime expanded
cell gates with two inclusive whole-program compiler inventories:

- I = S + A + O + R + L + C, at most 8192 declared storage/argument items
- W = sum aggregate_width(owner), at most 8192 logical owner-width cells

S counts scalar locals and mutable places; A counts every call argument position,
including owned/borrow holes; O counts owners; R reference parameters; L loans;
C call descriptors. Count unused functions and untaken branches. W uses existing
checked nominal widths, including empty-value sentinels, max(array length, 1),
and enum width 2. These are compiler-work inventories, not native byte units.

The previous native rule coupled X=S+A+W+4O+10R+14L+2C at 8192. I<=X and W<=X,
so the new guards alone preserve every old default-admitted program. Independent
I/W intentionally admit larger combinations, with combined inventory up to 16384.
This changes admission; it is not an allocation optimization or unchanged rules.

## Physical storage and safety

Keep the checked NativeStoragePlan representation and all LLVM allocations:
D=8(S+A)+B+8(R+L)+4Q per function, where B is the existing owner byte arena rounded
to four bytes, including padding/sentinels/canonical builtin scratch; Q is the
number of slice length slots. Aggregate bytes sum D across all functions. Live
bytes use the existing maximum acyclic call-path sum. Include the conditional
wrapper's eight-byte fuel cell once in each applicable budget.

This measures explicit LLVM object bytes, not backend spills, machine call
frames, host runtime memory or RSS. Keep the same ABI, alignments, owner/result
storage and pointer/length mappings. An 8192-eight-byte-word substitute is not
chosen: accepted programs can exceed 65536 explicit bytes due to builtin scratch
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
Activation fuel remains X-2(R+L), and call/transfer/return/branch/I/O schedules are
unchanged. Never substitute I, W or D into language fuel. The reference consumer
keeps its original limits.

## Order and diagnostics

Keep aggregate scalar/block checks first, then check I followed by W where the
old aggregate X check stood. Remove the native live-X denial while preserving
all other graph/cost/depth/live-scalar/byte and later metadata/diagnostic checks.
Keep Phase 1's final independent storage reconciliation before LLVM count/render;
its equality proof validates the immutable byte totals used by prior prechecks.
Measure any changed fixed carriers without introducing a retained table.

New limit names are `aggregate compiler inventory items` and
`aggregate owner width cells`, under existing E0700/native-admission formatting.
Existing earlier failures retain precedence. Former-X denials can reach later
existing failures or succeed; retain exact old/new observations as named
outcomes. Do not normalize arbitrary E0700 differences or silently route current
controls through historical policy. Frozen authorities/oracles remain intact.

## Delivery gates

First implement private counting/policy controls while production admission
continues using X. Independently verify exact/one-over I/W cases, checked
arithmetic, unchanged scratch/wrapper accepted cases, earlier failure precedence,
retained later gates and unchanged old LLVM/fuel/ABI. Independent review must
clear before enabling the successor production rule.

Then qualify the actual current consumer and its explicit source/admission
amendments. Only after review may the unchanged staged parser pass through normal
production gates. Its current I=4646, W=2571 and predicted explicit bytes 48864 fit
these proposed budgets, but later gates are not yet proven. Parser expansion
waits until the capability is qualified; no cap increase or grammar narrowing
is authorized by this RFC.
