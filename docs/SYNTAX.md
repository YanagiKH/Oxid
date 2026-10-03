# Legacy syntax

Oxid accepts classical spelling and concise aliases in the same parser: `fun/fn`, `var/let`, `say/print`, `give/return`, `when/if`, `otherwise/else`, `loop/while`, `import/use`, `yes/true`, `no/false`, `none/null`, `all/and`, `any/or`, and `work/async`.

```oxid
fun double(value) => value * 2;

work fun fetch(name) => {name: name, ready: yes};

fun main() {
    var state = {items: [1, 2, 3], owner: "Oxid"};
    state.count = len(state.items);
    for value in state.items {
        when value % 2 == 0 { continue; }
        say value |> double |> str;
    }
    say json_stringify(await fetch(state.owner));
}
```

Implemented constructs include short expression functions, `for … in`, `break`, `continue`, pipelines, modulo, optional condition parentheses, arrays, deterministic records, property access, string-key indexing, assignment, `async`/`await`, comments, and one-line macros.

Record literal keys may be identifiers or strings. Duplicate literal keys are rejected. `value.name` is equivalent to string-key access for records, and both forms support assignment.


The explicit `--edition typed-preview` route has a separate statically checked
[grammar](../spec/typed-preview.md), including bounded modules, direct imports
and visibility. Legacy aliases above do not carry into that grammar.
