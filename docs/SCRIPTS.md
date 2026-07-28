# Scripts

`oxid.toml` can define scripts:

```toml
[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
```

Run them with:

```bash
oxid script run
oxid script test
```
