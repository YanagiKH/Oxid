# Explicit workspace selection

From this directory:

```sh
oxid check --manifest-path oxid.toml --package workspace-app
oxid run --manifest-path oxid.toml --package workspace-app
oxid test --manifest-path oxid.toml --package workspace-app
oxid run --manifest-path oxid.toml --package workspace-library
```

The app prints `7`; its two tests pass. Selecting the library prints `91` and
discovers only `test_library`. Omitting `--package` lists the two names and fails.
An absolute workspace manifest path works from another working directory.

With the separately provisioned LLVM 19.1.7/Linux x86_64 toolchain:

```sh
oxid build --manifest-path oxid.toml --package workspace-app
oxid run --manifest-path oxid.toml --package workspace-app --engine native
oxid test --manifest-path oxid.toml --package workspace-app --engine native
```

Application artifacts belong under `app/.oxid/typed-preview/`; test artifacts
use its separate test namespace. The virtual root receives no application output.
Membership grants no import visibility: the app explicitly declares `math` as a
path dependency. This example demonstrates limited selection, not complete R1.
