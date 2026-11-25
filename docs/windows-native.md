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

This preview supports bounded programs using `bool`, `i32`, `()`, functions,
conditions and `while` loops. Set `entry-mode = "process"` to return a status
from `fn main() -> i32` without printing the value. The status is in `0..255`.
Declaring a test requires the same entry signature. A test passes when it
returns zero.

For binary file I/O, use `std::fs::File` with byte arrays or slices and the standard file status enums. Paths use UTF-8, and `seek_absolute` takes a signed 64-bit byte offset. [File operations](../spec/typed-preview.md#native-files)

Use `std::io::read_some`, `write_some` and `write_stderr` with byte arrays and
slices in process mode. These functions use the inherited streams and return
Windows error codes when a transfer fails.

Pass arguments with `oxid run -- ARGUMENTS` or after the native executable's
name. `std::env::arg_bytes` copies UTF-8 into a byte array or slice. User
arguments start at index 1.

Other scalar types, records, vectors, generics, hosted execution, native debug
information, native object caching, multi-package dependencies and workspace
projects are unavailable on this native target.
