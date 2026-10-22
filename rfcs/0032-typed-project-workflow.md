# RFC 0032: manifest-driven typed project commands

Status: experimental implementation, single package.

## User outcome

A project can select `typed-preview` once in `oxid.toml`, then use ordinary
`check`, `run`, `build`, and `test` commands. Native builds retain the checked
frontend, OIR verifier, ownership rules, resource limits, LLVM 19.1.7 and Linux
x86_64 target. This is the first integration step of ROADMAP_V2 R1, not a stable
edition or completion of its multi-package acceptance project.

## Configuration and compatibility

`[project].edition` accepts `legacy-0.9` and `typed-preview`. An absent edition
retains legacy behavior. Explicit command-line edition selection takes precedence;
an explicitly selected file command keeps its existing command-line contract.
Without explicit selection, file commands use the nearest manifest above their
source path, and project commands use the nearest manifest above the working
directory. Source syntax never determines edition.

New default scaffolds record `legacy-0.9`. `new --edition typed-preview` creates
an experimental typed scaffold. `[build]` separates `backend = "llvm"`,
`target = "x86_64-unknown-linux-gnu"`, `profile = "bounded"`, and
`entry-mode = "process"` (the typed project default) or `"result"`.
Unsupported settings fail explicitly. No setting silently selects legacy code.

This increment rejects typed package dependencies. Existing `mod` and
`use crate::...` declarations still load source modules; downloading a package is
not a substitute for linking it into the typed declaration graph. Workspace,
registry, incremental cache and additional target/profile support remain work
for subsequent increments.

## Execution and test contract

Process entry requires `fn main() -> i32`, preserves stdout, and uses the
existing checked 0..255 exit-status contract. Result entry retains the existing
scalar result renderer and successful-run status. Type and ownership errors
are diagnosed before execution. JSON diagnostics are supported where the
underlying operation supports them; process execution never mixes diagnostic
JSON with program stdout.

`[tests]` explicitly maps test names to entry paths. Each test is an isolated
typed process invocation with null stdin. Exit zero passes; every nonzero exit
fails. The runner does not recursively execute module helpers. It reports all
declared test outcomes, with sorted names and a failing suite status if any test
fails. Process execution currently supports Linux x86_64 only.

## Build publication

`build` emits `.oxid/bin/<project-name>`. Names contain only ASCII letters,
digits, `_`, and `-`. The driver checks and verifies the program before touching
build directories. Compilation/linking occurs in a fresh private directory on
the output filesystem, followed by atomic replacement of a regular managed
output. A failed build preserves the last successful executable. Build-owned
directories and outputs must not be symlinks. Source and tool installations
remain trusted, stable inputs; this is not a hostile-filesystem sandbox.

Low-level `compile --output` keeps its existing no-overwrite contract. A clean
build recompiles the full declared source graph; there is no incremental cache
claim. Native outputs use the existing ABI and runtime support.

## Formatting

Inferred typed file formatting uses the existing typed formatter: complete
formatted source on stdout, or `--check` with status 1 for drift. It does not
rewrite a typed project with the legacy formatter. Directory formatting remains
unsupported by this increment.

## Verification

Public CLI tests cover manifest inheritance and overrides, explicit test roots,
nonzero test failure, rejected unsupported settings, legacy behavior, and
source-error handling. Native tests compile and execute a multi-module project,
change a module and rebuild, then verify previous-output preservation on source
and toolchain failure and low-level no-overwrite behavior.
