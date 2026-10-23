# Program arguments

Status: experimental, `typed-preview`, Linux x86_64.

`std::env::arg_count()` returns the number of program arguments, including the
executable at index 0. `arg_bytes(index, &mut destination)` copies an argument's
raw OS bytes into a mutable byte slice:

| `ArgStatus` | Meaning |
| --- | --- |
| `Copied(length)` | The argument was copied; bytes after `length` are unchanged. |
| `TooSmall(required)` | The destination is too short; no bytes were changed. |
| `Missing` | The index is negative or outside the argument list. |
| `TooLong` | The argument's length cannot be represented by `i32`. |

Empty arguments return `Copied(0)`. Arguments contain neither an appended NUL
nor an implicit UTF-8 conversion. `Missing` and `TooLong` preserve every
destination byte. Arguments are immutable; reading one does not consume it.
Fixed byte arrays, borrowed byte ranges and initialized `ByteBuf` ranges can
serve as destinations. The status is an ordinary move-only enum.

```sh
oxid run -- input.bin output.bin
oxid run main.ox --edition=typed-preview -- input.bin output.bin
./application input.bin output.bin
```

The first `--` after a project `run`, or after an explicit source path, starts
program arguments. Their spelling is literal, including compiler-looking
options, empty strings and non-UTF-8 bytes. An explicit typed source beginning
with `-` can use `run -- SOURCE -- ARGUMENTS`. Compiler options and source paths
still require UTF-8. Other commands retain their existing argument grammar.

In reference execution, argument 0 is the Oxid launcher executable; in a native
program it is the program executable. User arguments start at 1 in both. An
embedded executor without an installed argument context has zero arguments.
Both result and process entry modes admit this API. Checking never reads a
program argument. Unsupported execution hosts reject before source effects.

For bounded execution, a present argument costs one unit per byte plus its NUL
terminator scan. A successful copy additionally costs one unit per copied byte.
A missing argument has no byte-work cost. The complete status delivery and loan
release are prepared, and the immediate primitive return is prepaid, before
any destination changes. Fuel refusal preserves the destination and result.
Source wrappers and subsequent caller instructions retain their normal costs.
Hosted execution uses the same extent and delivery checks without a cumulative
work budget.

The native entry receives the OS argument vector and keeps it immutable without
allocating another copy. The reference executor reads the CLI's owned input
strings. OS/CLI argument backing storage belongs to the input boundary; it is
not a language heap allocation or part of the language live-storage limit.
The runtime accounts for its argument context and fixed copy-control storage.
No language pointer, mutable argument vector or borrowed argument string escapes.

The catalog appends identities after all existing builtins. Its four-byte
extension inventory and four-bit legacy selector share a four-byte packed
payload, preserving the existing five-byte builtin-set carrier. Decoding cannot
create dependencies or change item ranks. Complete conversion scratch is
charged separately; existing compiler and execution limits remain unchanged.
