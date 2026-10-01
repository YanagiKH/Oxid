<div align="center">
  <img width="108" height="100" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />
</div>

# Oxid

**A compact language for scripts, automation, and tools that work across languages.**

[![Repository CI](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg)](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases)](https://github.com/YanagiKH/Oxid/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

[English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)

Write a short `.ox` file, run it with `oxid`, or package it into one `.oxb` file. The release executable includes the interpreter and project tools; running Oxid programs does not require Rust.

**Current release: 0.9, experimental.** Oxid runs on a Rust-built interpreter. Native compilation, static ownership checking, and compiler self-hosting are future work.

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

- Write scripts with functions, loops, pipelines, arrays, records, and JSON
- Read files, launch processes, and use Python, Java, or Go through process adapters
- Package modules into one `.oxb` file and lock local or commit-pinned Git dependencies
- Experiment with local HTTP services and Discord interaction handlers
- Generate host adapters for Python, Java, Go, C, and C++

Start with the [examples](examples/), [runtime API](docs/API.md), or [interop guide](docs/INTEROP.md). External adapters need their own runtimes or toolchains.

## Know the limits

Oxid 0.9 is useful for experiments and small tools. Its `.oxb` files contain a serialized AST executed by the interpreter. Tasks are lazy and run sequentially when joined; networking has no concurrent scheduler or built-in TLS. The legacy `bootstrap` and `self-host` commands check artifact round-trips, not compiler self-rebuilds.

The long-term direction is a statically checked, native-compiled language. Those capabilities, Rust compatibility, and native AI training are not available in 0.9. Follow the [roadmap](docs/ROADMAP.md) and [implementation status](docs/architecture/current-baseline.md) for the current boundaries.

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
