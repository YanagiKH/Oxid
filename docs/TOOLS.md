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

Run `oxid help` or read [Commands](COMMANDS.md) for flags and side effects.
In particular, legacy `fmt` rewrites source, bare legacy `doc` replaces
`docs/API.md`, and `clean` removes the project `.oxid` directory.

The explicit typed-project command `oxid doc --manifest-path oxid.toml` checks
the ordinary compiler session and writes only selected public API Markdown to
stdout. It does not execute source or create/update documentation files. Use
`--package <name>` for workspace selection; see
[typed project documentation](../spec/typed-project-doc.md).

Explicit typed `oxid export --manifest-path ... --output ...` creates a fresh,
normalized source tree containing the selected package and its local dependency
closure. It is not a lossless backup or a registry/lockfile implementation. See
[typed project export](../spec/typed-project-export.md).
