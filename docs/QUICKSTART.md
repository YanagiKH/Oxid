# Quickstart

## Install

Use `install.sh`, `install.ps1`, a portable release archive, Cargo, source/Make, or Docker as documented in the three top-level READMEs.

## Create and run

```bash
oxid new my-project
cd my-project
oxid run src/main.ox
oxid build
oxid test
oxid doctor
```

## Compile a bundle

```bash
oxid compile src/main.ox -o app.oxb
oxid run app.oxb
```

## Create an application profile

```bash
oxid web new my-api
oxid discord new my-bot
```

## Generate language bridges

```bash
oxid bridge all bridges
```

## Use manifest scripts

```toml
[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
fmt = "oxid fmt"
doctor = "oxid doctor"
```

```bash
oxid script run
oxid script test
```
