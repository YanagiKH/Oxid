# Bootstrap

Bootstrap validates the versioned compiler artifact and the frontend provider boundary.

```bash
oxid bootstrap          # verify and write .oxid/bootstrap artifacts
oxid bootstrap --check  # verify without writing artifacts
```

The command compiles the repository compiler sources when available and falls back to the same sources embedded in the release binary. It verifies the provider manifest, produces OXBC stage artifacts, requires byte equality at the stage-0/stage-1 and stage-1/stage-2 boundaries, and executes the decoded compiler program.

Deterministic output depends on normalized module ordering, stable record ordering, versioned AST serialization, and platform-independent integer encoding. Cross-platform CI comparison is the release gate required before the remaining stage-0 providers can be retired.

