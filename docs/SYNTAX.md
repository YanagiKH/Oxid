# Syntax

Oxid should optimize for short, direct forms.

## Existing preview forms

- `fn name()`
- `pub fn name()`
- `mod name`
- `use path as alias`
- `match value { ... }`
- `try expr`
- `defer expr`
- `value |> step()`

## Additional preview forms

- `enum Status { A, B, C }`
- `trait Readable { ... }`
- `impl Readable for File { ... }`
- `async fn fetch()`
- `await request()`
- `unsafe { raw_call() }`
- `Result<Value>`
- `ffi symbol -> target`

## Recommended style

- prefer one idea per line
- prefer short helper names
- prefer simple module boundaries
- prefer readable defaults over hidden magic
- prefer short bridges over wrapper-heavy integration code
