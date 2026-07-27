# Oxid Expansion Directions

## 1. Fast development experience

- `oxid run`, `oxid script`, `oxid check`, `oxid watch`, `oxid build`, `oxid clean`, `oxid fmt`, `oxid test`, `oxid doctor`, and `oxid doc` support a script-first workflow.
- Module caching, manifest parsing, and single-file execution reduce restart overhead.
- Local `.oxid/cache` reuse reduces repeated preprocessing cost.

## 2. Syntax and development experience

- `const` represents values that can be evaluated early.
- `async fn` and `await` use a task model with modern syntax.
- `spawn`, `join`, `join_all`, `task_status`, and `yield_now` lower async ceremony.
- `sleep()` helps with demos and automation scripts.

## 3. Macro and compile-time system

- Single-line `macro` expands before lexing and parsing.
- `const` is the first practical compile-time evaluation entry point.
- Preprocessing is cache-backed so repeated edits stay responsive.

## 4. Module and build system

- `use` can load local `.ox` modules.
- `oxid.toml` now carries project metadata, scripts, dependencies, and features.
- `oxid add` and `oxid script` make manifests more ergonomic.
- C / C++ interoperability is provided through `native/` and `build.rs`.
- Module search falls back to `src/`, `stdlib/`, `modules/`, `deps/`, `vendor/`, and `OXID_PATH`.

## 5. Learning curve

- Documentation is split into architecture, async, macros, lifetime, FFI, packages, tooling, and roadmap.
- `examples/` contains both runnable examples and future syntax previews.
- `tests/` provides a smoke-test entry point for the toolchain.
