# Self-hosting plan

Oxid is still bootstrapped by Rust, but the repository now treats that boundary as a thin compatibility layer instead of the permanent design.

## Stage 1: bootstrap preview

- keep the bootstrap story in Oxid source files
- expose the bootstrap view through `tools/bootstrap.ox`
- keep the Rust runtime focused on loading and running Oxid scripts
- keep bootstrap output short and readable

## Stage 2: compiler preview

- keep compiler-facing snapshots in Oxid source files
- expose the compiler view through `tools/compile.ox`
- centralize phase, command, and hint text in `stdlib/compiler.ox`
- make the compiler preview consumable from `oxid script compile`

## Stage 3: self-host preview

- keep the self-host story in Oxid source files
- expose the migration summary through `tools/self_host.ox`
- keep the Rust layer as the fallback execution boundary
- make the visible toolchain surface readable from Oxid code first

## Stage 4: regression coverage

- `tests/bootstrap_smoke.ox`
- `tests/compiler_smoke.ox`
- `tests/self_host_smoke.ox`
- GitHub Actions should run the same script entry points used locally

## Success criteria

- bootstrap, compiler, and self-host previews are all runnable from Oxid scripts
- the compiler snapshot is produced from `stdlib/compiler.ox`
- the manifest exposes the same entry points used in local development
- CI fails if the preview scripts stop running
