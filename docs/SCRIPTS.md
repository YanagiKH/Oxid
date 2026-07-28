# Scripts

`oxid.toml` can define scripts:

```toml
[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
fmt = "oxid fmt"
doctor = "oxid doctor"
```

Run them with:

```bash
oxid script run
oxid script test
oxid script fmt
oxid script doctor
```

Scripts are intended for repeatable project operations and package workflow previews.
