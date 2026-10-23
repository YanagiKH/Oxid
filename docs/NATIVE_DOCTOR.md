# Native environment diagnosis

Run `oxid doctor --native` from any directory to diagnose the currently supported
bounded native target: Linux x86_64, `x86_64-unknown-linux-gnu`, LLVM **19.1.7**.
`oxid diagnose --native` is an alias. `--message-format=json` produces one JSON
document; `--message-format=text` selects human output. Explicit
`--edition=typed-preview` is also accepted. Other options are rejected before
probes or project reads. The existing `oxid doctor` project health command keeps
its previous behavior.

The command performs these checks in order:

1. Confirm that this compiler runs on a supported native build host.
2. Reserve private temporary storage.
3. Launch `clang`, `opt` and `ld.lld` and verify their exact LLVM versions.
4. Compile Oxid's production C runtime against the target SDK headers and verify
   that the result is an ELF64 x86_64 object.
5. Use the running Oxid compiler to compile a fixed `fn main()->i32{return 42;}`
   program through the normal typed frontend, LLVM verifier, code generator,
   C runtime compiler and linker. Require an ELF64 x86_64 PIE executable.
6. Execute that fixed program and require exit status 0, stdout `42\n` and empty
   stderr. This checks the target loader and runtime libraries in use.
7. If the nearest manifest selects `typed-preview`, admit its local package graph
   and loaded sources and verify any existing typed lock against those retained
   inputs. A missing lock is reported as optional; it does not establish locked
   reproducibility. An invalid nearest manifest is an error.

Tool selection and version checks use the same rules as native builds:
`OXID_LLVM_BIN` selects a directory containing `clang`, `opt` and `ld.lld`;
otherwise `clang-19`, `opt-19` and `ld.lld-19` are selected through `PATH`.
Relative selections are resolved from the caller's directory. Clang's
`CCC_OVERRIDE_OPTIONS` environment override is removed, as during normal builds.
No target download, package fetch, source program, project test or script runs.
Project sources are parsed to discover their closure; doctor does not type check
the user's program or replace `oxid check` and `oxid test`.

Probe files exist only in a private directory under the system temporary
directory and are removed on success and failure. No project build output,
cache entry or lockfile is published. Tool and SDK probes have a 5 second
deadline each, the compiler probe has a 20 second deadline, and execution has a
2 second deadline. Each child stream is limited to 65,536 bytes. The command
kills its probe process groups before reaping their leaders and bounds leader
cleanup. Inherited non-default `SIGCHLD` handling or automatic child reaping is
rejected before spawning probes. Each probe process
and its descendants inherit an address-space hard limit of at most 1 GiB and
disabled core dumps. Existing lower limits are preserved. This also bounds
the compiler child's internal buffering of tool output; ordinary builds keep
their existing resource policy. Installed tools and
the local filesystem remain trusted inputs; this is a diagnostic, not a sandbox
for hostile executables.

## Result contract

Exit status is 0 when all applicable checks pass, 1 for an option or diagnostic
failure, and 74 if the result cannot be written. JSON uses `schema_version: 1`,
`kind: "native-doctor"`, `edition: "typed-preview"`, `target`, `llvm_version`,
`toolchain_ready`, `success`, and an ordered `checks` array. Each check has `id`,
`status` and a bounded `message`. Status is `passed`, `failed`, `skipped` or
`not-applicable`; skipped checks have not been tested. IDs are `host`,
`temporary-directory`, `clang`, `opt`, `linker`, `sdk`, `compile-link`, `execute`
and `project`. Invalid command options instead produce one failed `options`
check. Both output formats escape control characters in external diagnostics.

`toolchain_ready` reports the native probes independently of project health.
For example, an installed toolchain may pass while a stale dependency lock
makes `success` false. Outside a typed project, `project` is `not-applicable`.
Unsupported compiler hosts report a failed `host` check and skip native probes;
the command does not claim cross-platform native support.
