<p align="center">
  <img width="216" height="200" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
</p>

<h1 align="center">Oxid</h1>

<p align="center">
  <strong>A compact language for scripts, automation, and tools that work across languages.</strong>
</p>

<p align="center">
  <a href="https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml"><img alt="Repository CI" src="https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg" /></a>
  <a href="https://github.com/YanagiKH/Oxid/releases"><img alt="Release" src="https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases" /></a>
  <a href="LICENSE"><img alt="License: MIT or Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README_ZH.md">繁體中文</a> · <a href="README_JP.md">日本語</a>
</p>

<p align="center">
  Write a short <code>.ox</code> file, run it with <code>oxid</code>, or package top-level imports into one <code>.oxb</code> file.<br />
  One executable includes the interpreter and project tools. Running Oxid programs does not require Rust.
</p>

<p align="center"><strong>0.9 · Experimental · Rust-built interpreter</strong></p>

```oxid
fun double(n) => n * 2;

fun main() {
    for n in [1, 2, 3] {
        say n |> double;
    }
}
```

Save this as `double.ox` and run `oxid run double.ox`. It prints `2`, `4`, and `6`, one per line. `fn`, `let`, and `print` are also available alongside `fun`, `var`, and `say`.

## Try it

[Install Oxid](docs/INSTALLATION.md), then create a project:

```bash
oxid new hello
cd hello
oxid run src/main.ox
oxid build
oxid run .oxid/bin/hello.oxb
oxid test
```

Both run commands print `Hello from Oxid`. The build produces a serialized-AST artifact that runs on a compatible Oxid runtime.

[Quickstart](docs/QUICKSTART.md) · [Syntax](docs/SYNTAX.md) · [Command reference](docs/COMMANDS.md)

## What you can do today

Oxid 0.9 includes the following working features. Each guide describes the current behavior and limits.

### Language and data

- **Concise or familiar syntax:** use `fun` / `fn`, `var` / `let`, `say` / `print`, and other aliases in the same program
- **Functions and control flow:** expression-bodied functions, `if` / `when`, `while` / `loop`, `for … in`, `break`, `continue`, and `|>` pipelines
- **Built-in values:** numbers, strings, booleans, null, arrays, and records with property or string-key access
- **Text and JSON:** split, join, and replace text; parse and serialize JSON with deterministic record-key ordering
- **Source macros:** expand one-line parameterized macros before parsing, with cached preprocessing
- **Lazy tasks:** `async` / `work`, `await`, `spawn`, task state inspection, and memoized results; `join_all` runs tasks sequentially

### Run, package, and manage dependencies

- **Direct execution:** run `.ox` files, use the REPL, or rerun a file with `watch` as project files change
- **Portable AST artifacts:** bundle top-level imports into `.oxb`, emit the same representation as `.oxa`, and inspect format versions, counts, and checksums
- **Module loading:** resolve relative imports and dependency aliases, with source locations retained in artifacts; imports inside functions still need their source files at runtime
- **Locked dependencies:** local paths and full-commit-pinned HTTPS Git sources, recursive resolution, `oxid.lock`, and locked / offline / frozen modes

### Project tools

- **Starter projects:** `new` / `init`, plus HTTP and Discord scaffolds
- **Everyday checks:** syntax checking, syntax-based `lint`, source formatting, and execution of project tests and examples
- **Manifest scripts:** reusable commands with quoted arguments, launched without a command shell
- **Project inspection:** `doctor`, generated API documentation, and benchmark reports for startup, parsing, packaging, and runtime operations

### Files, networking, and other languages

- **Local I/O:** text files, directory listings, environment variables, clocks, and sleep helpers
- **External programs:** launch processes, collect exit codes or standard output, and call Python, Java, or Go through process adapters
- **C/C++ and host adapters:** four linked native helpers plus generated Python, Java, Go, C, and C++ adapters that launch Oxid
- **Local HTTP and Discord logic:** reusable TCP listeners, timeout-bounded HTTP helpers, routing, and Discord command/interaction dispatch; gateway transport remains external

[Syntax](docs/SYNTAX.md) · [Runtime API](docs/API.md) · [Packages](docs/PACKAGES.md) · [Project tools](docs/COMMANDS.md) · [Interop](docs/INTEROP.md) · [HTTP / Discord](docs/WEB_AND_BOTS.md)

Try the [examples](examples/) to see these features in use. External adapters need their own runtimes or toolchains. `.oxb` artifacts require a compatible Oxid runtime; they contain a serialized AST, not native machine code.

## Know the limits

Oxid 0.9 is useful for experiments and small tools. Its `.oxb` files contain a serialized AST executed by the interpreter. Tasks are lazy and run sequentially when joined; networking has no concurrent scheduler or built-in TLS. The legacy `bootstrap` and `self-host` commands check artifact round-trips, not compiler self-rebuilds.

The long-term direction is a statically checked, native-compiled language. An opt-in [experimental LLVM preview](spec/native-preview.md) now compiles a bounded nonrecursive bool/unit/i32 subset to Linux x86_64 executables. Complete static-core semantics, Rust compatibility, and native AI training remain unavailable. Follow the [roadmap](docs/ROADMAP.md) and [implementation status](docs/architecture/current-baseline.md) for the current boundaries.

The opt-in [ownership-foundations extension](spec/typed-preview.md#nominal-owned-structs-and-call-only-borrowing) for `--edition typed-preview` supports move-only scalar-field structs, whole moves/replacement and explicit call-only borrows. The [Batch pilot](fixtures/owned_source/batch.ox) combines those rules with loops and owned helper returns, producing 816; see the [source qualification](docs/architecture/owned-source-validation.md) for exact evidence and limits. Run/native `main` remains zero-argument and scalar; native scope remains Linux x86_64, LLVM 19.1.7 at O0. Legacy dynamic records are unchanged; stored references, heap/destructor safety and v1.0 completion are outside this increment.

The experimental [typed project route](spec/typed-preview.md#bounded-typed-projects) adds declaration-only modules, direct imports and visibility. The [three-file Batch example](fixtures/typed-project-batch/README.md) keeps its record fields private and has a source-derived result of 816. Declared-child loading admits Linux; root-only project syntax has no discovery host gate. Exact current qualification is recorded in the [project ledger](docs/architecture/typed-project-unit4-validation.md).

The experimental [fixed scalar array extension](spec/typed-preview.md#fixed-scalar-arrays) adds move-only bool/i32/unit arrays, checked indexing, `len()` and whole-array call borrows to explicit typed-preview `check`, `run` and `compile`. The [three-module sample](fixtures/typed-array-samples/README.md) returns 5325. See the spec for grammar, platform and stricter native limits; default/legacy arrays are unchanged.

The experimental [typed formatter](spec/typed-preview.md#single-file-formatting) supports fixed-array syntax and `oxid fmt --edition typed-preview input.ox` (complete source on stdout) and `--check` (exit 1 for drift). It preserves comments and existing line breaks while normalizing spacing and indentation, without loading modules or writing files. Default legacy formatting is unchanged.

Run only trusted programs and review dependencies. Generated C/C++ process adapters require trusted paths. See the [security policy](SECURITY.md) for private vulnerability reporting.

## Build from source

Requires stable Rust and a C/C++ compiler. Run these commands in a Unix-like shell:

```bash
git clone https://github.com/YanagiKH/Oxid.git
cd Oxid
cargo build --release --locked
./target/release/oxid run examples/hello.ox
```

See [installation](docs/INSTALLATION.md) for Windows, release archives, Cargo installation, and Docker.

## Explore and contribute

[Documentation](docs/README.md) · [Architecture](docs/ARCHITECTURE.md) · [Contributing](CONTRIBUTING.md) · [Issues](https://github.com/YanagiKH/Oxid/issues)

Visual guides: [quickstart](docs/assets/quickstart.svg), [execution path](docs/assets/architecture.svg), [language bridges](docs/assets/interop.svg), and [Web / Discord](docs/assets/web-discord.svg).

Bug reports with a small runnable example, clearer diagnostics, tests, and documentation fixes are welcome. See the [contribution guide](CONTRIBUTING.md) for the checks to run before opening a pull request.

Licensed under [MIT](LICENSE) or [Apache-2.0](LICENSE-APACHE).
