# RFC 0026: explicit owned native storage plan

Status: **accepted Phase 1 contract; implementation under qualification**.
Updated 2026-10-07.

## Outcome

The owned LLVM emitter consumes a private, immutable `NativeStoragePlan` bound
to its existing `ExecutionPlan` and verified program. The plan describes and
checks the allocations already emitted. This phase consolidates representation;
it does not reduce runtime storage or change which programs are admitted.

Keep all existing admission limits and first-error ordering, source language,
reference execution, logical fuel, ABI, and process/result entry behavior.
Native-specific admission is a separately reviewed successor. A successfully
checked physical layout alone never authorizes emission of a rejected program.

## Representation

For each function, retain the existing four arenas:

- i64 scalar locals/places followed by all call argument positions, including holes
- owner bytes with existing aggregate alignment and canonical builtin scratch suffixes,
  rounded to four bytes for allocation
- reference pointers followed by loan pointers
- i32 slice lengths, with slice references followed by slice loans in declaration order

Owned results remain caller-owned. Guarded entry retains its separate eight-byte
fuel cell. Canonical input/output scratch stays inside its corresponding owner
arena and outside all language owner extents. No reference sidecar, slot reuse,
new alias assumptions, general allocator, or public ABI is introduced.

The native plan borrows existing metadata and uses fixed temporary descriptors;
it adds no retained collection or allocation attempt. Measure the complete new
fixed carriers and their coexistence within the existing emitter transient
budget. Do not raise a budget or omit a newly retained allocation.

## Validation and ordering

Run every existing admission and metadata gate in its existing order. Before
emission, independently reconcile raw declaration counts, aggregate layouts,
call argument ranges, slice ordering, scratch identities, and owner offsets
against the native description and execution plan. Use checked arithmetic for
sizes, alignment and offsets. Require exact arena coverage and disjoint owner
and scratch extents; zero-sized owners may share an empty range.

The emitter receives the bound native plan rather than a separately replaceable
witness. It obtains allocation lengths and physical address mappings from this
checked description. Preserve byte-identical LLVM for unchanged inputs.
Independent tests compare raw declarations, plan ranges and actual emitted IR;
corruption controls must reject truncated, overlapping, mismatched or overflowing
layouts without conferring an alternate execution capability.

## Delivery

1. Introduce the borrowed description and independent consistency check.
2. Route existing allocation/address emission through it without changing layout.
3. Measure fixed carriers and verify fixtures covering records, arrays, enums,
   scalar calls, borrowing, slices, builtin scratch and both entry policies.
4. Preserve exact admission/fuel/failure evidence and run one coherent regression
   milestone after focused checks and independent review.

The bounded typed parser remains a rejected witness under the existing expanded
cell gate. No parser LLVM or executable is produced in this phase.

## Fixed-carrier accounting

The description borrows existing tables; construction makes no allocation
attempt and changes no retained plan layout. On the qualified 64-bit host,
module/function descriptors measure 16/48 bytes, their complete result wrappers
48/56 bytes, and the slice iterator/item 24/32 bytes. A 1024-byte subdivision of
the existing 32768-byte emitter transient allowance covers the new fixed roles.
Tests use phase-specific maxima for the sequential owner, scratch, call, usage
and slice checks, explicitly including copied FrameUsage, loop arrays/iterators,
return carriers and bounded counters. This is a named carrier model, not a
whole compiler stack or RSS limit. Existing maximum-arity emission controls also
check coexistence with this subdivision. No extra metadata charge or ceiling
increase is introduced.

The original expanded-cell gate still rejects the staged parser inventory of
12559 cells against 8192. This phase does not reinterpret that metric.
