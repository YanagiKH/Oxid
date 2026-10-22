# RFC 0039: relocatable native project builds

Status: experimental implementation extending the bounded Linux x86_64 project
workflow. LLVM 19.1.7, the compiler, SDK and effective build environment remain
inputs to the result.

## Selection and resolved settings

```sh
oxid build --frozen --reproducible
oxid build --workspace --frozen --reproducible --incremental
oxid build --no-reproducible
```

`--reproducible` selects project-relative paths in embedded native runtime
diagnostics. `--no-reproducible` selects the original absolute paths. Either flag
may appear once after `build`; combining them, assigning a value or passing them
to another command is an error. Omission uses the selected package's optional
`[build] reproducible = true|false`, then defaults to false. The manifest value
must be an unquoted boolean. Build settings require an ordinary `[build]` table.

The CLI choice overrides the manifest without rewriting it or its lock. A
manifest edit remains a lock-input change. A workspace resolves each selected
member's settings once with the same CLI override; a dependency's settings do
not replace its importing binary's settings. Virtual workspace roots cannot
declare package build settings.

An immutable native build configuration separates the supported LLVM backend,
Linux x86_64 GNU target, O0 optimization, bounded execution and source-path
presentation. The existing `profile = "bounded"` retains its O0 and bounded
meaning. Language edition, result/process entry policy and object reuse remain
separate choices. This change adds no other target, optimization profile or
hosted execution policy. Check, interpreter run and test commands validate
manifest settings without applying native path presentation to their output.

## Source identity and diagnostic presentation

The logical root is the canonical standalone project root or enclosing workspace
root, regardless of the caller's working directory. Every already retained source
receives a name relative to that root. Examples include `src/main.ox`,
`members/client/src/main.ox` and `../shared/src/lib.ox`. Names preserve the entire
relative directory layout, including sibling dependencies, Unicode components,
shared graph edges and packages with equal display names.

The presentation map borrows the exact retained source map. It changes no source
bytes, source IDs, spans, package identity, source ownership or lock fingerprints,
and does not reopen source files. Missing source IDs and a map from another
session are errors. Its allocation is bounded separately: at most 256 source
IDs, 16 KiB per input/root/relative pathname and 4 MiB total retained map capacity.
Bounds and allocation failures reject the opt-in route before invoking tools.
Ordinary native wrappers retain their existing allocation and acceptance path.

Scalar and owned native emitters use the same presentation in diagnostic sizing,
admission and final IR emission. Result and process entrypoints share this rule.
Error codes, source positions, execution order, ownership behavior and fuel stay
unchanged. Compile-time and interpreter diagnostics retain their original source
paths. This option does not supply source-level debugging or DWARF metadata.

## Reproducibility and publication

Preserving the complete admitted relative source layout removes absolute source
root bytes from embedded runtime diagnostics. Identical source inputs, compiler,
resolved settings, toolchain, SDK and effective environment can then produce
identical executables at different project locations. Moving only the application
while leaving external dependencies at different relative locations does not
preserve this layout. Different SDKs, compiler versions and build environments
are outside this equality contract.

One captured environment is used for all native tools in a typed project build,
with or without object reuse. The resolved target applies to compilation and
linking; O0 applies to both program and runtime C compilation. LLVM verification,
runtime compilation and SDK linking still run on every successful build. There
is no post-link string replacement, stripping or executable normalization.

The object cache retains its conservative compiler/tool/environment identities.
Equal executable bytes do not require a cache hit; moving a project may still
invalidate object reuse. Cache status is reported only after successful build
publication. Stale locks or invalid sources reach neither the cache nor native
tools. Managed executables are replaced only after successful linking; a failed
build preserves previous bytes. Workspace preflight checks every selected binary
with the same settings and presentation before any native tool runs. Per-member
publication retains the existing workspace contract.

## Acceptance scope

Public CLI regressions cover strict flags and manifests, precedence, lock drift,
workspace preflight, diagnostic behavior and failed-build preservation. Compiler
tests cover retained-source identity, bounded map construction and consistent
diagnostic accounting. Existing default-path assertions remain unchanged.

`scripts/verify_reproducible_native.py` independently compares raw ELF bytes across
repeated clean builds, cold/warm object caches and relocated layouts, using both
Rust compiler profiles. Standalone and workspace layouts exercise sibling Unicode
paths, equal package names and shared dependencies. Byte/newline oracles check
actual application output; deployed binaries run after sources are removed.
Receipts retain commands, tools, environment, input/output and artifact hashes.

The current native regression suite separately checks existing source-independent
LLVM oracles. The retained single-package Unit2 assertions remain compatibility
evidence; their expanded input roster does not qualify the new presentation or
reproducibility contract.
