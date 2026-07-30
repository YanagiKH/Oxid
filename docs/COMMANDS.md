# Commands

This repository treats commands as a first-class language surface.

## Core commands

- `oxid bootstrap`
- `oxid compile`
- `oxid self-compile`
- `oxid frontend`
- `oxid diagnose`
- `oxid lint`
- `oxid emit`
- `oxid module`
- `oxid syntax`
- `oxid interop`
- `oxid bridge`
- `oxid self-host`

## Project commands

- `oxid run`
- `oxid script`
- `oxid repl`
- `oxid check`
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

The command surface should stay short, obvious, and scriptable. The bootstrap, compile, and self-compile workflows are now available as native commands and are mirrored by the scripts in `oxid.toml` for users who prefer manifest-driven entry points.
