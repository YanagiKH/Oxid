# RFC 0074: Returning scalar native libraries

Status: experimental.

This slice adds an explicit Linux shared-library workflow to the typed compiler.
It establishes one real C ABI callable by C, Rust and Python without launching
the Oxid executable at call time. It does not complete release gate G7: owned
resources, callbacks, native Python-extension packaging and migration tooling
remain unsupported.

```sh
oxid --edition typed-preview library --export crate::line_total \
  --output dist/pricing-v1 src/lib.ox
```

The output parent must exist. The command publishes a new, immutable directory
with exactly `liboxid_library.so`, `oxid_library.h`, and `metadata.json`. An
existing file, directory or symlink at the destination is an error. Failed
admission, compilation, linking, metadata writing or publication preserves every
previously published version. Publication uses Linux `renameat2` with
`RENAME_NOREPLACE` on the same filesystem; files are complete before the directory
becomes visible. This is atomic publication, not a power-loss durability claim.

LLVM is the default backend; optional `--backend llvm` is accepted. Profile
`dev` is default and selects O0; `release` selects O2. `bounded` remains an alias
for `dev`. Execution is bounded in every profile. `--fuel` accepts a decimal
integer from zero through the existing scalar `MAX_FUEL`, currently 1,000,000;
that constant supplies the default and ceiling. No manifest setting or ordinary
executable default is changed. The command requires explicit typed edition and
rejects entry modes, experimental input providers, other backends and unknown
options. Initial host support is Linux, little-endian, 64-bit x86_64 or ARM64;
other hosts receive an unsupported diagnostic before native tools are invoked.

Exactly one `crate::<ASCII identifier>` export is selected. The identifier is at
most 64 bytes and names an explicitly `pub` root function with only `i32`
parameters and an `i32` result. Modules and ordinary imports can supply scalar
helpers; qualified/aliased exports and generic declarations are unsupported.
Every loaded declaration participates in admission, including unused functions.
Helper bodies admit only bool, i32 and unit scalars, bounded while loops, direct
calls and existing scalar control flow. Resources, references, arrays, wider
scalars, floats, characters, I/O, recursion and hosted execution are unsupported.

The generated header declares fixed-width types and exactly two public symbols:

```c
uint32_t oxid_library_abi_version(void); /* 1 */
int32_t oxid_library_call_v1(/* source i32 parameters in order, */ int32_t *out);
```

The concrete header contains the exact selected parameter list. Header and
metadata must accompany the matching library; the ABI version alone is not a
substitute for checking its signature. Status values are:

| Value | Meaning |
| --- | --- |
| 0 | success; one result is written |
| 1 | checked integer overflow, including MIN/-1 division or remainder |
| 2 | division or remainder by zero |
| 3 | shared invocation fuel exhausted |
| 4 | null or detectably misaligned output pointer |

`out` remains caller-owned and must otherwise identify live, writable, naturally
aligned `int32_t` storage. Its memory validity cannot be established from an
arbitrary C address. Every nonzero status leaves that storage unchanged. Valid
negative i32 results remain successful values. Output validation precedes fuel
debit and source invocation. Errors return to the host without a process exit,
diagnostic write, signal-policy change or stored pointer.

The source seal resolves the selected original declaration while the frozen
index exists, binds its checked DefId and exact signature, authenticates the
complete scalar OIR and creates a separate `CheckedScalarLibrary`. CLI ordinals,
raw OIR, source-name guesses and LLVM text substitution cannot construct it.
The old `CheckedSourceProgram`, index rows, OIR carriers and native settings keep
their previous layouts.

A separate emitter gives internal functions a private LLVM `{status, value}`
return convention. Failed calls return their status before result extraction or
continuation. Arithmetic is guarded before LLVM operations that can be undefined.
One invocation owns one fuel word; nested calls receive the same pointer. The
existing scalar tariff is retained: entry activation `1 + slots`, one unit per
merge, statement and ordinary terminator, and `1 + arguments + callee slots` per
call terminator. No callee resets the budget. Block exit labels preserve boolean
phi predecessors across new fuel, arithmetic and call-status branches.

Library linking does not compile or link the executable C runtime. There is no
global resource heap, process setup, print helper, `noreturn` failure helper,
longjmp substitute or thread-local state. Program objects use PIC; the linker
produces a shared ELF64 artifact without CRT/default-library injection
(`-nostdlib`), and an export map hides every symbol except the
two public functions. Internal function identities remain numeric/private, so
independently loaded artifacts do not share body symbols. Existing executable
compilation, caching, runtime selection and publication paths are unchanged; this
library route does not read or populate the native object cache.

The source/index caps and all existing scalar-native limits remain unchanged:
256 functions, 64 parameters, 256 slots per function, 8,192 aggregate/live slots,
4,096 aggregate blocks, depth 32 and the existing 100,000 static fuel upper bound
for acyclic functions. A library-only source bank accounts the sealed export and
constructor transports. Driver, option payloads, header-builder roles, maximum
64-parameter header capacity, publication/hash/serializer/path roles, and the
emitter's real fixed graph/status/formatter bank are additive payments within the
original 32 MiB retained/16 MiB scratch index budgets. The controller subtracts
its complete outside payment before source construction; the sealed descriptor
carries the lowered limits into native emission. Count/render admits the actual
IR length before one fallible reservation and rejects excess capacity. Header
capacity is also checked. Disjoint phases are conservatively summed, so no
unstated overlay or spare legacy envelope pays later header/publication storage.
The work meter charges graph/shape nodes, the existing scalar admission-helper
tariff, both emission passes and formatted text bytes. These are repository-owned
logical resource models, not allocator-backend, LLVM-process, machine-stack, RSS
or elapsed-time guarantees. Formatting carriers follow the existing pinned
scalar model.

Metadata records the selected source path/signature, stable statuses, actual
target, canonical profile, bounded fuel, library/header/IR/compiler hashes and
each retained module's relative path/source hash. It is streamed from the already
loaded source owner; source text is never reopened for compilation or metadata.

Native ABI validation must exercise the actual same shared-library bytes from
C, Rust and Python. Required cases include checked arithmetic boundaries, nested
errors with unchanged output, null/misaligned output, exact fuel exhaustion,
loops, short-circuit control flow, concurrent calls, independent load/unload,
symbol visibility and unchanged host signal/descriptor state. Source checking
alone does not establish these native behaviors. Resource-bearing FFI ownership,
reentrant callbacks and packaged Python native extensions remain outside this
experimental contract.
