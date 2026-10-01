# Interop

Oxid connects to other languages through external processes and four linked C/C++ helpers. Process integration requires the external program or runtime to be installed.

## Oxid to foreign programs

- `process(program, args)` returns the process exit code.
- `process_output(program, args)` returns standard output and reports failed exit status.
- `python(target, args)`, `java(target, args)`, and `go(target, args)` provide concise adapters.
- `c_len`, `c_hash`, `cpp_len`, and `cpp_hash` call code linked from `native/`.

## Foreign programs to Oxid

Generate host SDKs with:

```bash
oxid bridge all bridges
```

Individual targets are `python`, `java`, `go`, `c`, and `cpp`. Generated adapters expose a small `run` function around the `oxid run` process boundary. These launch the Oxid executable rather than embedding an Oxid compiler.

## Safety

The normal process functions do not invoke a shell. Generated C/C++ adapters use a shell-compatible pipe for portability and therefore require trusted source paths. Keep protocol data separate from executable names and arguments.
