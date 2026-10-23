# RFC 0036: opt-in reuse of native LLVM objects

Status: experimental implementation for Linux x86_64 and LLVM 19.1.7.

## Command and scope

`oxid build --incremental` reuses the LLVM object for an unchanged admitted
program. Ordinary `build` and explicit-file `compile` keep their existing
uncached behavior. The option combines with the existing manifest, entry mode,
target/profile and locked/offline/frozen options.

Every build still reads and validates its manifests, verifies any dependency
lock, prepares and type-checks its retained source buffers, emits LLVM IR and
runs LLVM verification. Runtime C compilation and the final SDK link also run
on every invocation. This is native object reuse, not per-module incremental
frontend compilation or final executable reuse.

After a successful incremental build, human output reports `hit`, `miss` or
`unavailable`. JSON output adds a `build-cache` event with
`stage: "native-object"` and that status before the ordinary compile summary.
`unavailable` means an uncached build succeeded; the selected toolchain could
not be completely identified, or its object could not be saved. Cache status
does not substitute for successful compilation or linking.

## Inputs and invalidation

The cache key binds the exact emitted IR buffer, a versioned cache domain, all
IR compilation arguments, the Oxid executable, selected clang/opt invocation
paths, their physical binaries, ELF interpreters and resolved shared libraries.
The loader's dependency list is discovered again under the captured effective
child environment. Each regular file is stream-hashed with before/after
identity checks. Inventory limits are 128 paths, 512 MiB per file and 1 GiB in
aggregate; exceeding them makes caching unavailable.

The effective environment is captured once, excluding the already-ignored
`CCC_OVERRIDE_OPTIONS`, and supplied to the native children. Its complete sorted
name/value pairs contribute to the key. Raw environment values are never saved
in the cache or status event. Environment changes conservatively cause misses.

Only direct ELF tools with an identifiable Linux x86_64 dynamic loader are
eligible. Script wrappers, `LD_PRELOAD`, `LD_AUDIT`, loader tracing, unresolved
libraries and unsupported discovery results use the uncached path. The LLVM
installation remains a trusted toolchain, as it is for ordinary compilation;
this is not a sandbox for arbitrary compiler executables. Tool identity is
rechecked before retaining or reusing the object, so a changed installation
does not publish an object under its former identity.

The application source is deliberately editable under dependency locks. Its
generated IR therefore belongs in the cache key independently of the lock.
No second source read is used to identify an object. Source paths in generated
diagnostics remain intact; relocating a project may cause a miss.

Runtime headers, CRT objects and system libraries are used freshly by runtime
compilation and linking. They are not bypassed by an object hit. This design
does not claim reproducibility across SDKs or execution environments.

## Storage and publication

Entries live under `.oxid/cache/native-object-v1/<key>.oxobj`. A single immutable
blob contains a 16-byte magic/schema, 32-byte key, little-endian 64-bit object
length, 32-byte object SHA-256 and the object bytes. Objects are limited to
64 MiB and must identify ELF64 little-endian x86_64 relocatable objects.

A reader opens a regular file without following symlinks, validates its bounded
header, and copies/hashes that same open file into the private build workspace.
Wrong keys, corrupt or truncated bytes, unsupported object formats and special
files are misses. Partial restored objects are removed. The linker never
receives a writable hard link to a cached object.

Writers create and sync a private complete blob, then publish it with an atomic
no-replace hard link. Concurrent builders can share a completed winner; neither
replaces a winner or exposes partially written data. A corrupt existing entry
is not overwritten automatically and continues to miss. Removing the managed
cache directory while no build is active is sufficient to reset it.

The final executable retains the ordinary managed build contract: runtime or
link failure preserves the previous executable. An uncreatable or unsafe cache
directory is a setup error; ordinary `build` remains available without cache
setup. Once setup succeeds, failure to save an object does not invalidate an
otherwise successful build and reports `unavailable`. A valid program object
may remain cached even when a later runtime compile or link fails.

## Validation and limits

Unit controls exercise immutable publication, corruption, wrong identities and
targets, special files, bounded readers, concurrent writers and independent
restored object bytes. Public native tests use real LLVM plus `strace` to check
which subprocesses actually run. They verify source/lock changes, unchanged
reuse, native outputs and failure preservation.

Full toolchain hashing has a cost: the tested LLVM installation resolves about
238 MB of shared libraries. No speedup is promised without measuring the user's
workload. Digest memoization, cache eviction, distributed caches, per-module
frontend incrementality, parallel dependency compilation and general native
reproducibility remain separate work. This increment does not complete R1 or a
v1 release gate.
