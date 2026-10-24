# Script packages: .oxb and .oxa

The Oxid 0.9 script workflow can bundle a program into an `.oxb` file. The file
contains a serialized syntax tree and runs with a compatible Oxid runtime.
For native executables in the development version, use the
[native project guide](TYPED_PROJECTS.md).

## Package a script

```sh
oxid run app.ox
oxid compile app.ox -o app.oxb
oxid inspect app.oxb
oxid run app.oxb
```

`compile` includes top-level imports and retains source locations for error
messages. `inspect` displays the artifact's versions, module count, and checksum.
Copy the `.oxb` file to another machine with a compatible Oxid runtime to run it.

`oxid ast app.ox -o app.oxa` writes the same representation using an extension
that identifies a serialized syntax tree.

## Build a script project

Run `oxid build` from a script project to package the entry point declared in
`oxid.toml`. The output is `.oxid/bin/<project>.oxb`; a build report is stored
under `.oxid/`.

See the [quickstart](QUICKSTART.md) for creating and building a project.

## Compatibility and imports

An `.oxb` file requires the Oxid runtime, including on a machine where the
source is absent. The runtime rejects unsupported versions, damaged files, and
files that exceed its format limits. The checksum detects accidental corruption;
it does not authenticate the author or make a program trustworthy.

Import initialization order can differ between source and packaged execution;
see the [module ordering limitation](MODULES.md#initialization-order). Imports
inside functions may still need source files at runtime. See the
[script module reference](../spec/legacy-modules.md) before distributing a program
that uses them.
