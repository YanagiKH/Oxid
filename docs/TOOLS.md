# Project tools

The `oxid` executable includes the tools needed to try and package an Oxid project:

- `run`, `repl`, and `watch` for execution
- `check` and `lint` for syntax validation
- `compile`, `ast`, `inspect`, and `build` for serialized-AST artifacts
- `add`, `lock`, `fetch`, and related commands for path/Git dependencies
- `new`, `fmt`, `test`, and `doctor` for project maintenance
- `bench` for internal performance measurements
- `bridge` for generated host process adapters

`frontend` reports provider declarations. Legacy bootstrap aliases check artifact round-trips. Neither command proves compiler self-hosting.

Run `oxid help` or read [Commands](COMMANDS.md) for flags and side effects. In particular, `fmt` rewrites source, `doc` replaces `docs/API.md`, and `clean` removes the project `.oxid` directory.
