# Oxid

Oxid is an experimental language prototype based on Rust + C/C++.

Its goal is not to be a thin wrapper. It is to turn the language core, development workflow, macros, modules, native interoperability, and learning path into a runnable project.

## Implemented so far

- `oxid run <file.ox>`: run a script
- `oxid script <name> [args...]`: execute a manifest script
- `oxid repl`: interactive REPL
- `oxid check <file.ox>`: syntax check
- `oxid new <project-name>` / `oxid init <project-name>`: scaffold a project
- `oxid add <name> <path-or-target>`: add a dependency entry
- `oxid watch <file.ox>`: watch files and rerun
- `oxid build`: validate the project
- `oxid clean`: clear build cache
- `oxid fmt [path]`: format Oxid source files
- `oxid test`: run smoke tests and examples
- `oxid doctor`: verify project health
- `oxid doc`: generate API docs
- `let` / `const` / `print` / `if` / `while` / `fn` / `async fn` / `await` / `return` / `use`
- arrays, indexing, and indexed assignment
- `len` / `push` / `pop` / `range` / `str` / `sleep`
- `c_len` / `c_hash` / `cpp_len` / `cpp_hash` native interop
- module cache, preprocess cache, and recursive loading
- single-line `macro` preprocessing expansion
- package manifest support (`oxid.toml` scripts / dependencies / features)
- developer tooling commands (`fmt`, `test`, `doctor`, `doc`, `script`, `add`, `clean`)
- GitHub Actions scaffold

## Implemented expansion directions

### 1. Fast development experience

- `oxid run` executes scripts directly.
- `oxid watch` monitors changes with polling.
- Module caching avoids repeated loads.
- `check`, `build`, `fmt`, `test`, `doctor`, `doc`, `script`, `add`, and `clean` provide a fuller toolchain.
- local cache under `.oxid/cache` avoids repeated preprocessing work.

### 2. Syntax and development experience

- `const` supports early evaluation.
- `async fn` + `await` use a task-style async model.
- `sleep()` makes demos and scripts easier to write.
- task helpers and manifest scripts reduce ceremony for everyday work.

### 3. Macro and compile-time system

- `macro name(a, b) => ...;` single-line macros.
- Preprocessing expands before lexing and parsing.
- `const` provides a small compile-time evaluation entry point.

### 4. Module and build system

- `use "../stdlib/math.ox";` module loading.
- `oxid.toml` project configuration with scripts, dependencies, and features.
- `build.rs` compiles the C and C++ bridges.
- `resolve_path` searches local project roots plus `OXID_PATH`.

### 5. Lower learning curve

- `README`, `docs/`, `examples/`, `stdlib/`, and `tests/` are included.
- `hello.ox`, `arrays.ox`, `modules.ox`, `smoke.ox`, and `future/` guide beginners.

## Usage

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/modules.ox
cargo run -- run examples/arrays.ox
cargo run -- watch examples/hello.ox
cargo run -- fmt examples/hello.ox
cargo run -- test
cargo run -- doctor
cargo run -- doc
cargo run -- script run
cargo run -- clean
cargo run -- repl
```

## Syntax sample

```oxid
const app_name = "Oxid";

macro greet(name) => print "Hello, " + name;

async fn load_message(name) {
    return "Welcome " + name;
}

fn main() {
    greet(app_name);
    let task = load_message("Master");
    print await task;
}
```

## Notes

This is still a prototype. The current focus is:
- clearer syntax
- better developer feedback
- stronger build and package conventions
- future compiler backend work
