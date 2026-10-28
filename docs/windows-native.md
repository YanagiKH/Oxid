# Windows native programs

Use Windows 10 or later on x64, Visual Studio 2022 C++ tools with the Windows SDK, and the complete LLVM 19.1.7 Windows MSVC toolchain containing `clang.exe`, `opt.exe`, and `lld-link.exe`.

Open **x64 Native Tools Command Prompt for VS 2022**, then create and build a project:

```bat
set OXID_LLVM_BIN=C:\llvm-19.1.7\bin
oxid new hello --edition typed-preview
cd hello
oxid doctor --native
oxid check
oxid run
oxid test
oxid build --profile release
.oxid\bin\hello.exe
```

New Windows projects select result entry. For an existing project, use:

```toml
[build]
entry-mode = "result"
```

Use a single project without package dependencies on a local NTFS drive. The executable runs without Oxid, LLVM or project sources installed. A failed build preserves the previous executable.

This preview supports bounded result-entry programs using `bool`, `i32`, `()`, functions, conditions, and `while` loops. Declared tests use the same pure scalar scope and require `fn main() -> i32` with no parameters: zero passes and any other value fails. Runtime errors fail the test and retain their source diagnostics.

Other scalar types, records, references, arrays, vectors, files, generics, process entry, process I/O, hosted execution, native debug information, native object caching, multi-package dependencies and workspace projects are unavailable on this native target. Source checking and reference execution retain their existing scope.
