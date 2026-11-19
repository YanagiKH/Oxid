# Native builds on macOS

Oxid can build native programs on Intel and Apple Silicon Macs with LLVM
19.1.7 and the macOS Command Line Tools SDK. Generated programs use the system
runtime and run without the Oxid compiler or project sources.

Install the Command Line Tools with `xcode-select --install`, then install
`llvm@19` and `lld@19` with Homebrew. Point `OXID_LLVM_BIN` at a directory
containing `clang`, `opt` and `ld64.lld` from those installations. These tools
must all report version 19.1.7.

```sh
export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
oxid doctor --native
oxid build --profile release
```

Run the commands from your project directory. `oxid doctor --native` compiles,
links and executes a small program before checking the nearest project.

Native outputs are 64-bit Mach-O executables for the current host. The native
pipeline targets macOS 11 or later; the compiler and installed LLVM tools may
require a newer macOS version. Both bounded and hosted execution policies are
available, including files and floating-point operations.

`--reproducible` uses project-relative source paths. Incremental builds currently
report the native object cache as unavailable on macOS and perform a complete
build. `--debug-info` currently requires Linux.
