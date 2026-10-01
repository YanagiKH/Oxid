# Experimental LLVM scalar native preview

This is a narrow `typed-preview` backend, not completion of M2/M3, a stable ABI,
Rust compatibility, ownership checking, a sandbox, or self-hosting. Default and
explicit `legacy-0.9` compilation remain OXBC serialized-AST generation.

## Command

```sh
oxid compile input.ox --edition typed-preview --backend llvm --output ./program
oxid compile input.ox --edition=typed-preview --backend=llvm \
  --target=x86_64-unknown-linux-gnu --output=./program --message-format=json
./program
```

The compile command never runs its output. It requires one source, an explicit
`--backend llvm`, and `--output` naming a new file in an existing directory.
Directory syntax (a trailing `/`, `/.`, or `/..`, including `.` and `..`) is
rejected before tools run; it is never normalized into a different output file.
Backend, target and output options follow the command, may surround the source,
and accept `--name=value` or `--name value`. Each occurs at most once. A value
starting with `-` must be written with a directory prefix, e.g. `./-program`.
Global edition/format options retain their existing placement and separator
rules. `--` ends option recognition; later words are source operands. No backend,
profile, optimization, CPU feature, linker-argument, library, or cross-compilation
fallback exists. Unknown options/values fail with E0001 before source reading.

The only build host is Linux x86_64 and the only target is
`x86_64-unknown-linux-gnu` (also the default). Generated files are ELF PIE
executables, not OXBC. They depend on the host-compatible libc and ELF loader;
they do not depend on Oxid, Rust, LLVM shared libraries, Python, or source files.
This is not a portable-binary or minimum-glibc-version promise.

## Admission before tools or output

Compilation first runs the complete source → typed HIR → verified OIR path,
including unused functions and unchosen branches. Only the immutable verified
witness can enter native admission. A declared zero-argument scalar `main` is
required, carried by its resolved DefId rather than reconstructed from spans.

Supported operations are exact bool/unit/i32 constants, immutable copies,
explicit direct nonrecursive calls, Branch, Goto and Return. Native compilation
rejects other OIR operations, including checked arithmetic, even when reference
execution supports them. There are no source I/O operations, pointers, containers,
loops, indirect calls, modules or implicit legacy adapters in this subset.

The entire call graph must be acyclic, including dead declarations and calls in
constant-false branches. Iterative leaf-first traversal rejects recursive graphs.
The following inclusive bounds are additional native restrictions:

| Resource | Maximum |
| --- | ---: |
| Functions in the file | 256 |
| Parameters per function | 64 |
| Locals per function, including parameters and temporaries | 256 |
| Aggregate locals / maximum live slots | 8,192 |
| Aggregate basic blocks | 4,096 |
| Call depth, including main | 32 |
| Conservative reference fuel upper bound | 100,000 |

All declarations must meet the bounds. For a function F, compute
`C(F) = locals(F) + sum(assignments(block) + 1 for all blocks) +
sum(argument_count(call) + C(callee) for all calls)` in callee-first order.
Require `1 + C(F) <= 100,000`. Calls at distinct sites are counted separately;
there is no memoization discount. Since verified intraprocedural CFGs are
acyclic, summing both arms overestimates every executed path. It also counts
callee allocation and every reference execution operation. This conservative
admission can reject programs that run successfully in the reference interpreter.

Depth is `1 + max(callee_depth)` and live slots are
`locals(F) + max(callee_live_slots)`, with zero for an empty maximum. These bounds
are below the runner's 1,000,000 fuel / 1,024 frame / 200,000 slot limits. Thus
admitted scalar computations cannot fail a reference execution budget. They do
not equate OS stack bytes to slots or bound compiler time, tool execution,
allocation failure, output blocking, or host resource limits. A 32-frame,
64-parameter representative stress case is tested with a 1 MiB process stack;
this is evidence, not an all-environments stack-safety theorem.

## Lowering and private ABI

Native ABI version 1 is private and provisional. LLVM values are `i1` for bool,
`i8` containing zero for unit, and exact `i32`. Definitions use the verifier's
single-assignment local IDs as LLVM SSA names. Copies/constants use bitwise OR
with zero, with no numeric conversion, `undef`, `poison`, `nsw`, or `nuw`.
Branches, direct calls and returns retain their OIR structure. Functions are
`internal` and `noinline`; this preview uses `-O0` and no LTO or fast-math.

Symbols are deterministic numeric `__oxid_fn_<DefId>` names. Source identifiers,
comments and paths are not inserted into LLVM text, so names such as `write` or
`printf` cannot collide with runtime/library symbols or inject IR syntax.
The C-compatible entry shim returns an output status, never the scalar value.
It widens bool to i32 when calling the C output adapter. Unit has a no-argument
printer. There is no general C/FFI ABI promise for user functions.

## Toolchain and artifact boundary

The trusted tools are Clang, opt and LLD, each exactly version 19.1.7. Set
`OXID_LLVM_BIN` to their directory containing `clang`, `opt`, and `ld.lld`, or
provide `clang-19`, `opt-19`, and `ld.lld-19` on PATH. The compiler checks all
three version reports on each invocation. The qualified Debian package build is
`1:19.1.7-3+b1`; see the validation report for provenance. A working host C
header/startup/libc development environment is also required. Building the Oxid
compiler itself still uses Rust/Cargo and C/C++, as before.

Version checks are compatibility gates, not authentication of arbitrary tools.
The user-selected tools, installation, PATH and C development environment are
trusted. Oxid does not download packages, run installation commands, accept raw
compiler flags, or use a shell. Clang default configuration is disabled and
`CCC_OVERRIDE_OPTIONS` is removed. Tools are invoked with separate argv items.

After source/admission/output preflight and version checks, a new owner-only
scratch directory is atomically reserved beside the destination. opt explicitly
verifies the emitted LLVM module; Clang generates its object and separately
compiles the embedded small C printer; Clang invokes the pinned LLD to link PIE
with non-executable stack, RELRO and immediate relocation. Generated executable
publication uses a same-filesystem hard link: it fails if any destination exists,
including a symlink or a file created after preflight. No existing file is ever
replaced. Source/output equality therefore fails safely. Normal success and
failure remove scratch files. Abrupt process death, untrusted concurrent changes
to the output directory, or cleanup I/O errors are outside the cleanup guarantee.
Filesystems without hard-link support fail cleanly; there is no unsafe fallback.

## Output and diagnostics

Successful executables write exactly `true\n`, `false\n`, `()\n`, or signed
decimal i32 plus newline, and exit 0. The printer handles i32 MIN by widening
before negation. It handles partial writes and retries EINTR. A failed write,
zero-progress write, or signal-setup failure returns 74 (EX_IOERR); SIGPIPE is
ignored so a closed pipe also returns 74. An output failure can leave a partial
line. There is no native JSON output mode or program input/arguments contract.

Compiler success is exit 0. Text mode prints a compile summary only, never the
program result. JSON mode emits one `compile-summary` with schema_version 1,
edition `typed-preview`, success, errors and output (path on success, null on
failure). Ordinary errors are exit 1. E0700/native-admission describes entry,
unsupported-operation, call-graph and bound failures; E0701/native-toolchain
covers host, tools and artifact I/O. Existing frontend/OIR diagnostic codes and
exit 2 internal-invariant behavior are unchanged. Valid global options selecting
compile use compile-summary for later errors; invalid global options retain the
pre-existing check-summary. Diagnostics escape source/tool-controlled text.

See [RFC 0007](../rfcs/0007-llvm-scalar-native.md) and
[validation evidence](../docs/architecture/native-preview-validation.md).
