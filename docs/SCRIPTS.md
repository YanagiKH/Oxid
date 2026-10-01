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

Scripts make project commands repeatable. Oxid parses quotes and supported backslash escapes, then starts the declared executable with a direct argument vector. It does not invoke a command shell, so operators such as `|`, `&&`, redirection, and command substitution are passed literally rather than expanded. Arguments supplied after the script name are appended unchanged.
