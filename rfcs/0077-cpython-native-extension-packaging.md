# RFC 0077: CPython extension packages from scalar native libraries

Status: implemented source contract; runtime qualification is recorded by the
package verifier, separately from compiler compatibility qualification.

## Public contract

A self-contained PEP 517 backend under `apps/python-pricing/build_backend`
produces a real CPython extension and an sdist. It invokes the explicit typed
`oxid library` command during wheel construction only. An installed package
calls the resulting native function directly, without a runtime compiler,
subprocess or ctypes adapter. It requires no frontend, executable AST, project
CLI, existing resource cap or compatibility-policy changes.

The initial host is normal-GIL CPython >=3.11 on little-endian Linux64 x86_64 or
ARM64, with the actual interpreter's headers and LLVM Clang 19.1.7. TOML uses
stdlib `tomllib`. Tags are derived from that running interpreter and machine:
`cpXY-cpXY-linux_ARCH`. A limited-API 3.10 C shim is an implementation choice;
it does not justify an `abi3` tag. No manylinux, musllinux, per-interpreter-GIL,
free-threaded, Windows, macOS or cross-distribution claim follows.

Only static project name/version/description/requires-python metadata and the
closed `tool.oxid` module/source/sources/export/profile/fuel settings are
accepted. Names and module/export identifiers are bounded; supported versions
have two through four canonical decimal release components. All selected
sources are regular, singly linked files under the entry's parent. The complete
`.ox` neighbor set must equal the manifest; identities cannot traverse, alias by
case, escape through links or duplicate another input.

The build copies a bounded source snapshot, then invokes the actual CLI from
the entry's parent using its basename. This gives a closed mapping from the
compiler's root and module metadata to the retained manifest. The backend
requires exactly `liboxid_library.so`, `oxid_library.h` and `metadata.json`.
The schema, fixed ABI/statuses, i32 parameter list of length 0..64, result,
export, target, profile, bounded policy, fuel, compiler identity, source closure
and payload hashes must match. The C header must equal the exact scalar ABI
template, not an arbitrary parsed declaration. Source signatures are never
inferred by Python; the actual Oxid compiler is the semantic authority.

## Loader and lifetime

Every library exports the same two ABI names and has the same SONAME. The
extension does not link those symbols through `DT_NEEDED`. It packages the
original unchanged `.so` under a content-digest filename. A PEP 489 multiphase
module obtains its actual installed filename, opens its adjacent private file
by absolute path with `RTLD_NOW | RTLD_LOCAL`, and resolves both ABI functions
with that specific handle. It checks the ABI version before publishing state.
No RPATH search, global symbol lookup, SONAME rewriting, host callback, owned
pointer or Python object crosses the Oxid ABI.

A positive-sized per-module state owns the handle and typed call pointer.
Partial initialization closes any acquired handle; reexecution does not reopen
an initialized handle. Module finalization nulls state and closes that handle
once. Calls validate the exact positional arity and each integer before
releasing the GIL. Booleans are excluded and i32 bounds are checked. A strong
module reference spans the complete GIL-free native call, and no Python API or
module-state access occurs without the GIL. A result is read only on status0.

Statuses map to Python `OverflowError`, `ZeroDivisionError`, `RuntimeError` or
`SystemError`; loader failures are `ImportError`. Every invocation retains the
native library's independent, per-call fuel. Module removal/GC, concurrent
calls, shared-GIL subinterpreters and interpreter shutdown require actual
lifecycle qualification; no compatibility claim can be inferred from source
inspection alone.

## Tool and publication boundaries

Oxid and Clang are selected as individual executables. No shell, dynamic build
dependencies, CFLAGS/LDFLAGS evaluation, hidden Cargo fallback or network fetch
is used. Invocation paths retain their argv0 meaning; resolved payloads are
hashed separately. Host CPython headers and the selected compiler/toolchain
are trusted. Tool output and deadlines are bounded and failures preserve old
artifacts. Temporary directories are exclusively owned; final regular-file
publication uses atomic no-replace links on the same filesystem.

Host limits are separate from Oxid's accounting: 256 input files, 1 MiB each,
8 MiB total; 1 MiB metadata; 32 MiB each native/shared object; 80 MiB wheel;
64 KiB per process stream; 180-second Oxid, 60-second C and 10-second version
query deadlines. Checks precede reading bounded payloads or publishing output.
Wheel members are closed regular paths with per-file hashes and sizes in
`RECORD`. The sdist contains only declared inputs, backend and template plus
static `PKG-INFO`. Old output names are never overwritten.

Build hashes and RECORD provide tuple consistency, not a signature, hostile
compiler sandbox or runtime installed-file verification. The package remains
native code and the installed files must be trusted.

## Qualification

`verify_python_native_package.py` must use ordinary isolated pip wheel and
install operations with the actual compiler. It must cover a direct wheel,
sdist round trip, relocated source-free installation, exact real extension
type/tag/RECORD, pricing1875, Python input errors, returned statuses and 0/64
arity. Two independently configured packages must retain distinct behavior
with identical exported Oxid symbol names, including a conflicting global
preload. Lifecycle, relocation and failures are exercised through actual
interpreters. Synthetic native fixtures are exclusively negative tests and
cannot qualify the genuine producer.

Source-only controls cover metadata preparation without tools, unsafe config,
links/traversal/duplicates/extra bundle members, mismatching ABI/header/compiler/
source/payload tuples, artifact/process limits and old-output preservation.
Real commands, selected tools, interpreter identity, source hashes and artifacts
are retained. Preparation and source controls alone make no runtime, ABI,
platform or release-completion claim.
