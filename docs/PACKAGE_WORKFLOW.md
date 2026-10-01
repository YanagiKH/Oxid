# Package workflow

1. Create a project with `oxid new <name>` or `oxid init <name>`; the scaffold includes an empty valid `oxid.lock`.
2. Interpret `src/main.ox` directly while developing.
3. Add local or pinned Git dependencies with `oxid add`.
4. Resolve and write `oxid.lock` with `oxid lock`.
5. Use `oxid build --locked` to require the existing lockfile and avoid network access; `--frozen` also explicitly enables offline mode.
6. Run `oxid test`, `oxid lint`, and `oxid doctor`.
7. Measure representative changes with `oxid bench --json <path>`.
8. Distribute `.oxid/bin/<project>.oxb` to a compatible Oxid 0.9 runtime.

Recommended layout:

```text
project/
├── oxid.toml
├── oxid.lock
├── src/
│   ├── main.ox
│   └── lib.ox
├── examples/
└── tests/
```

Manifest scripts provide reusable project commands through `oxid script <name> [args...]`. Dependency resolution reads nested manifests and detects cycles, while application source can import installed dependencies by their declared alias.
