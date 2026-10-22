<img align="right" width="96" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />

# Oxid

**A Rust-inspired language for scripts and command-line tools.**

Write a `.ox` file and run it with `oxid`. The experimental 0.9 release includes
an interpreter, a formatter, a test runner, and project tools in one executable.

[Install](docs/INSTALLATION.md) · [Quickstart](docs/QUICKSTART.md) · [Documentation](docs/README.md)

[English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)

## Try it

Download an archive for your platform from [Releases](https://github.com/YanagiKH/Oxid/releases)
and put `oxid` or `oxid.exe` on your `PATH`. Running a release binary does not
require Rust. The [installation guide](docs/INSTALLATION.md) also covers installers
and building from source.

Save this as `double.ox`:

```oxid
fn main() {
    for n in [1, 2, 3] {
        print n * 2;
    }
}
```

Run it:

```sh
oxid run double.ox
```

It prints `2`, `4`, and `6`, one per line.

## Start a project

```sh
oxid new hello
cd hello
oxid run src/main.ox
oxid test
oxid build
oxid run .oxid/bin/hello.oxb
```

The starter prints `Hello from Oxid`. Edit `src/main.ox` to change the program.
`oxid build` creates `.oxid/bin/hello.oxb`, which runs with a compatible Oxid
runtime. Continue with the [quickstart](docs/QUICKSTART.md) to work with data and project tools.

## Native projects in the development version

Development source also supports projects with explicit types, ownership checks,
and native executables. This experimental workflow is **not included in the
published 0.9 binaries**.

Native builds currently require **Linux x86_64**, **LLVM/Clang/LLD 19.1.7**, and
the system C development tools. Execution steps and resource use are currently
limited; see the project guide below.

Start with the [native project guide](docs/TYPED_PROJECTS.md) for creating,
checking, testing, and building a project. It also covers local dependencies,
workspaces, and source packages. The [byte-count application](apps/byte-count/README.md)
shows a complete program split across several packages.

## Guides and examples

- [Language syntax](docs/SYNTAX.md) and [built-in functions](docs/API.md)
- [Modules](docs/MODULES.md) and [script dependencies](docs/PACKAGES.md)
- [Command reference](docs/COMMANDS.md) for formatting, testing, and project tools
- [Calling other languages](docs/INTEROP.md)
- [Example programs](examples/)

## Contribute

To build Oxid itself, follow the [source installation instructions](docs/INSTALLATION.md#build-a-checkout).
For changes and bug reports, see [Contributing](CONTRIBUTING.md) and
[Issues](https://github.com/YanagiKH/Oxid/issues).

Licensed under [MIT](LICENSE) or [Apache-2.0](LICENSE-APACHE).
