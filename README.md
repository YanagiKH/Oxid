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

Experimental [call-only borrowed scalar slices](spec/typed-preview.md#call-only-borrowed-scalar-slices) let shared `&[T]` and exclusive `&mut [T]` helpers process whole fixed bool/i32/unit arrays of different lengths through explicit borrows and reborrows. The [three-module slice sample](fixtures/typed-slice-samples/README.md) uses lengths 2, 3 and 0 and returns 515. This is limited to explicit typed-preview `check`, `run` and native `compile`, with the existing Linux module-loading and Linux x86_64 LLVM/Clang/LLD 19.1.7 at O0 native gates. Ranges, subslices and owned unsized values remain unavailable; this is not a stability or milestone-completion claim.

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

Experimental [projected array slices](rfcs/0022-projected-array-slices.md) let slice helpers borrow fixed scalar array fields, for example `bump(&mut batch.samples)`. Nested field paths and explicit whole-record-reference reborrows keep whole-root conflicts, privacy and call-only lifetimes. Resource ceilings and native targets remain unchanged.

Experimental [bounded enums and consuming matches](spec/typed-preview.md#bounded-nominal-enums-and-consuming-match) add nominal move-only values with nullary or single bool/i32/unit payload variants, plus exhaustive statement matches on named owners. Explicit typed-preview check/run/native compile and formatting support this contract. The [two-file scanner](tests/fixtures/bounded_enum_scanner/main.ox) returns 115; enum borrowing, aggregate payloads and match expressions remain outside scope. Current-source qualification and exact-head hosted CI are separate pending gates; this is not a self-hosting claim.

Experimental [bounded stdin input](spec/typed-preview.md#bounded-stdin-input) adds only the individual `std::io::read_stdin` and `std::io::ReadStatus` imports, with aliases. It fills an existing exclusive i32 slice with raw bytes, returning `Eof(n)`, `Full` without overreading, or `IoError` without changing the destination; consumed input cannot be rolled back. On Linux x86_64, the [128-byte expression application](fixtures/typed-expression-samples/README.md#bounded-stdin-entry) has returned 39 and 63 from one unchanged ELF in the local 28-case reference/native runner. Full current-source qualification and exact-head hosted CI remain separate gates; general `std`, strings, broader input targets and self-hosting remain outside this increment.

Experimental [bounded stdout and process entry](spec/typed-preview.md#bounded-stdout-and-process-entry) adds `write_stdout` over an existing shared i32 byte view and explicit `--entry-mode=process`, with exact 0..255 statuses and no scalar/JSON output trailer. The [persistent stack component](fixtures/typed-expression-samples/README.md) writes an 80-byte OXS1 artifact that a separate Oxid loader and independent decoder validate; one producer ELF handles both 39 and 63. Runtime support is Linux x86_64. Current-source qualification and hosted CI remain separate gates; Windows Process Run currently fails silently with status 74, and default mode is unchanged.

A separate [bounded typed-preview lexer](fixtures/typed-lexer-samples/README.md), written in Oxid, preserves tokens, trivia and byte spans for up to 128 ASCII source bytes. It compares against the canonical Rust lexer and runs as one native ELF; production provider dispatch and Unicode support are outside this component.

A separate [bounded scalar typed-preview parser](fixtures/typed-parser-samples/README.md), also written in Oxid, consumes that token interface and covers scalar functions, expressions, calls, bindings and control flow for up to 128 ASCII bytes. Its independent projection compares complete ASTs, byte spans and the first diagnostic against the canonical Rust parser; production provider selection and semantic name/type checking remain outside this component.

A separate [bounded scalar static frontend](fixtures/typed-static-samples/README.md), written in Oxid, validates the exact source and parser AST before resolving names and checking types and flow for up to 128 ASCII bytes. It uses the existing parser and a separate AST1 consumer, with the host only framing and relaying bytes. Local reference/native qualification covers 77 semantic pairs (29 complete typed-fact results and 48 first diagnostics across all 14 kinds); four producer failures/refusals are counted separately. All limits remain unchanged. Integrated current-source qualification and exact-head hosted CI remain pending; production provider selection, compiler-owned TypedProgram/OIR results and self-hosting remain outside this component.

An explicit [source-scale lexical provider](fixtures/typed-streaming-lexer/README.md) now feeds validated Oxid-produced ASCII tokens into the real typed-project parser for every loaded module. It preserves full-width byte spans, exact diagnostics, source binding and per-module receipts, with canonical Rust comparison and no fallback. The target is the complete existing v2 producer projects plus the modular lexer itself. Default routing, semantic/backend authority and resource ceilings remain unchanged; [qualification](docs/architecture/streaming-lexical-provider.md) and self-hosting remain distinct.
