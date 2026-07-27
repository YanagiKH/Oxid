# Oxid Expansion Directions

## 1. Fast development experience

- `oxid run`, `oxid check`, `oxid watch`, and `oxid build` support a script-first workflow.
- Module caching and single-file execution reduce restart overhead.

## 2. Syntax and development experience

- `const` represents values that can be evaluated early.
- `async fn` and `await` use a task model with modern syntax.
- `sleep()` helps with demos and automation scripts.

## 3. Macro and compile-time system

- Single-line `macro` expands before lexing and parsing.
- `const` is the first practical compile-time evaluation entry point.

## 4. Module and build system

- `use` can load local `.ox` modules.
- `oxid.toml` can become the basis of future package and build metadata.
- C / C++ interoperability is provided through `native/` and `build.rs`.

## 5. Learning curve

- Documentation is split into architecture, async, macros, lifetime, FFI, and roadmap.
- `examples/` contains both runnable examples and future syntax previews.
