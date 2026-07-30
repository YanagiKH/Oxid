# Commands

This repository treats scripts as a first-class language surface.

## Core script entry points

- `oxid script bootstrap`
- `oxid script compile`
- `oxid script frontend`
- `oxid script diagnose`
- `oxid script lint`
- `oxid script emit`
- `oxid script module`
- `oxid script syntax`
- `oxid script interop`
- `oxid script bridge`
- `oxid script self-host`

## Project commands

- `oxid run`
- `oxid check`
- `oxid repl`
- `oxid new`
- `oxid init`
- `oxid add`
- `oxid watch`
- `oxid build`
- `oxid clean`
- `oxid fmt`
- `oxid test`
- `oxid doctor`
- `oxid doc`

## Intent

The command surface should stay short, obvious, and scriptable. Bootstrap and compiler previews should live in Oxid source files, while the Rust runtime stays as the minimal execution boundary.
