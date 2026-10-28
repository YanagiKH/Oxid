# Windows native programs

Use Windows 10 or later on x64, Visual Studio 2022 C++ tools with the Windows SDK, and the complete LLVM 19.1.7 Windows MSVC toolchain containing `clang.exe`, `opt.exe`, and `lld-link.exe`.

Open **x64 Native Tools Command Prompt for VS 2022**, then set the LLVM directory and build a typed-preview project:

```bat
set OXID_LLVM_BIN=C:\llvm-19.1.7\bin
oxid doctor --native
oxid build --profile release
.oxid\bin\app.exe
```

Replace `app` with the project name. The executable runs without Oxid or LLVM installed.

This preview supports bounded result-entry programs using `bool`, `i32`, `()`, functions, conditions, and `while` loops. Other scalar types, records, references, arrays, vectors, files, generics, process entry, hosted execution, debug information, and native object caching are unavailable on this target. Source checking and reference execution retain their existing scope.
