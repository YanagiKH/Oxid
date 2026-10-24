# Oxid documentation

Choose a guide for the program you want to write. The published 0.9 binaries run
scripts with the interpreter. Native projects are available in development source.

## Get started

- [Install Oxid](INSTALLATION.md): download a release or build from source
- [Run your first project](QUICKSTART.md): create, run, test, and build a script project
- [Learn the syntax](SYNTAX.md): functions, variables, loops, and data
- [Look up a built-in function](API.md)
- [Find a command](COMMANDS.md)

## Organize and share scripts

These guides cover the 0.9 interpreter and its `.oxb` files, which run with a
compatible Oxid runtime.

- [Split code into modules](MODULES.md)
- [Add dependencies and use a lockfile](PACKAGES.md)
- [Build and distribute a script project](PACKAGE_WORKFLOW.md)
- [Save project commands as manifest scripts](SCRIPTS.md)
- [Browse example programs](../examples/)

## Build native programs

The experimental native project workflow is **not included in the published 0.9
binaries**. Native builds require **Linux x86_64**, **LLVM/Clang/LLD 19.1.7**, and
the system C development tools. The language subset and execution limits are
described in the guides below.

- [Create and build a native project](TYPED_PROJECTS.md): local dependencies, locks, workspaces, and source packages
- [Check the native toolchain](NATIVE_DOCTOR.md)
- [Try the multi-package byte-count application](../apps/byte-count/README.md)
- [Read the typed language reference](../spec/typed-preview.md)
- [Check native compilation requirements and limits](../spec/native-preview.md)

## Work with other programs

- [Call Python, Java, Go, C, or C++](INTEROP.md)
- [Use local HTTP and Discord helpers](WEB_AND_BOTS.md)
- [Understand task and I/O behavior](ASYNC.md)

## Contribute

See the [contribution guide](../CONTRIBUTING.md) for source builds and checks,
and the [language specifications](../spec/README.md) for language contracts.
Report reproducible problems through [Issues](https://github.com/YanagiKH/Oxid/issues).
