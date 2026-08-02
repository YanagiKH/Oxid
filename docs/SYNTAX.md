# Syntax

Oxid accepts familiar classical spelling and concise Oxid-native aliases in the same parser.

```oxid
fun double(value) => value * 2;

work fun fetch(name) => "ready: " + name;

fun main() {
    var values = range(1, 8);
    for value in values {
        when value % 2 == 0 { continue; }
        say value |> double |> str;
    }
    say await fetch("Oxid");
}
```

Implemented aliases:

- `fun/fn`, `var/let`, `say/print`, `give/return`
- `when/if`, `otherwise/else`, `loop/while`
- `import/use`, `yes/true`, `no/false`, `none/null`
- `all/and`, `any/or`, `work/async`

Implemented concise constructs:

- `fun name(args) => expression;`
- `for item in array_or_string { ... }`
- `break;` and `continue;`
- pipeline insertion with `value |> function` and `value |> function(extra)`
- modulo with `%`
- optional parentheses around `when` and `loop` conditions

See `examples/oxid_shortcuts.ox` and the parser unit tests for executable coverage.
