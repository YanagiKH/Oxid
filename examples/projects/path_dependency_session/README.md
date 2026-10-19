# Path dependency session

This application imports scalar functions and an opaque record API from the
local `library` package. The library imports `shared` transitively, including
from its `model` child module. No dependencies are downloaded.

From this directory:

```sh
oxid check --manifest-path app/oxid.toml
oxid run --manifest-path app/oxid.toml
oxid test --manifest-path app/oxid.toml
oxid build --manifest-path app/oxid.toml
app/.oxid/typed-preview/result/path-session
oxid run --manifest-path app/oxid.toml --engine native
oxid test --manifest-path app/oxid.toml --engine native
```

The application returns 14 and both tests pass. The dependency's `main` and
parameterized `test_dependency` are ordinary library functions; neither is an
application entry or a discovered test. The record's private field can only be
accessed through its public library API. Native commands require LLVM 19.1.7 on
Linux x86_64. Changing the shared number to 8 and rebuilding returns 16 and makes
both tests fail. Each invocation loads a fresh session; there is no incremental
cache or lockfile write.
