# Hosted native program sizes

Status: experimental

Use hosted execution when building larger applications:

```toml
[execution]
policy = "hosted"
```

```sh
oxid build --profile release
```

The following inclusive limits apply to the complete compiled program, including
its libraries, unused functions and untaken branches.

| Resource | Bounded | Hosted |
| --- | ---: | ---: |
| Functions | 256 | 1,024 |
| Scalar storage slots | 8,192 | 32,768 |
| Basic blocks | 4,096 | 16,384 |
| Compiler inventory items for owned values | 8,192 | 65,536 |
| Owner width cells | 8,192 | 65,536 |

Each function still permits at most 64 source parameters and 256 scalar plus
owner slots. Compiler inventory items count scalar declarations, argument
descriptors, owner declarations, incoming references, loans and calls. Owner
width counts the scalar or resource cells represented by owned values.

These are compilation limits. Hosted execution retains its limits of 1,024
active calls, 200,000 active scalar slots and 1 MiB of planned native frame and
runtime storage. Buffer heap allocation retains its separate 16 MiB live limit.
Larger programs must also fit the compiler's existing verification, metadata,
diagnostic and emitted-code budgets. A program may reach one of those limits
before reaching the size limits in this table.

Both native optimization profiles, process and result entries, and optional
debug information use the same program-size limits. Explicit file compilation
and projects without a hosted policy retain bounded execution.

An oversized program reports `E0700` before invoking native compilation tools
or publishing an executable. Ownership, borrow, arithmetic and bounds checks
apply throughout the program.

See [hosted calls and runtime errors](0058-hosted-native-recursion.md) and
[native compilation](../spec/native-preview.md).
