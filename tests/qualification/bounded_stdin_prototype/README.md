# Bounded stdin effect prototype

Status: disconnected internal model. No Oxid syntax, compiler entry point, raw
operation, host input or native input path is enabled by these files.

`model.rs` is a standalone executable specification of filling an existing i32
buffer from controlled one-byte events. Rust mutable borrows stand in for access
that a future runtime must independently validate. The caller supplies all
staging and destination storage; the model performs no allocation or OS calls.
It is not an alternative production runtime or proof of production admission.

The proposed core operation checks capacity 0..1024, debits `4 + capacity`, checks
staging capacity, then debits one unit before each attempted byte/EOF/interruption/
error event. Failed subtraction preserves remaining fuel. Full commits exactly
capacity bytes without an EOF probe. EOF commits only its initialized prefix and
preserves the destination tail. An error or fuel/staging failure preserves the
entire destination, though earlier input events remain consumed. Missing test
input is a separate fixture failure, never EOF. Zero capacity returns Full
without an input attempt. Later caller failures cannot roll back a prior commit.

The tests use independent expected values for byte, EOF, error, interruption,
fuel, staging and capacity boundaries, including repeated operations on one
stream and the 1024-byte endpoint. Run with the installed Rust toolchain:

```sh
rustc --edition=2021 --test tests/qualification/bounded_stdin_prototype/model.rs -o /tmp/oxid-stdin-model
/tmp/oxid-stdin-model
```

The prospective source binding remains under design: explicit individual imports
of compiler-owned `std::io::read_stdin` and `std::io::ReadStatus`, with no implicit
prelude or effect recognition from a user function's spelling. No binding choice
is implemented here. Production integration must prove exclusive runtime views,
nominal result identity, pre-effect staging/result admission, infallible commit,
and shared native runtime fuel even for an acyclic input caller. It must measure
new/affected carriers and preserve all existing ceilings and old-program costs.
Real OS no-overread, runtime ownership and reference/native parity remain future
validation requirements.
