use "../strings.ox";

fn short_fn(name) { return "fn " + name + "()"; }
fn exported_fn(name) { return "pub fn " + name + "()"; }
fn local_group(name) { return "mod " + name; }
fn import_alias(path_text, alias) { return "use " + path_text + " as " + alias; }
fn match_preview(value_name) { return "match " + value_name + " { ... }"; }
fn try_preview(expr) { return "try " + expr; }
fn defer_preview(expr) { return "defer " + expr; }
fn pipe_preview(left, right) { return left + " |> " + right; }
fn record_preview(name) { return name + " { field: Type }"; }
fn typed_record_preview(name) { return name + " { field: Type, other: Type }"; }
fn one_line_helper(name, expr) { return "fn " + name + "() = " + expr; }

fn enum_preview(name) { return "enum " + name + " { A, B, C }"; }
fn trait_preview(name) { return "trait " + name + " { ... }"; }
fn impl_preview(name, trait_name) { return "impl " + trait_name + " for " + name + " { ... }"; }
fn async_fn_preview(name) { return "async fn " + name + "()"; }
fn await_preview(expr) { return "await " + expr; }
fn result_preview(expr) { return "Result<" + expr + ">"; }
fn ffi_preview(symbol, target) { return "ffi " + symbol + " -> " + target; }
fn unsafe_preview(expr) { return "unsafe { " + expr + " }"; }

fn syntax_summary() {
    return join_lines([
        exported_fn("run"),
        local_group("parser"),
        import_alias("stdlib/frontend/parser.ox", "parse"),
        match_preview("value"),
        try_preview("expr"),
        defer_preview("cleanup()"),
        pipe_preview("value", "step()"),
        record_preview("User"),
        typed_record_preview("Config"),
        one_line_helper("id", "value"),
        enum_preview("Status"),
        trait_preview("Readable"),
        impl_preview("File", "Readable"),
        async_fn_preview("fetch"),
        await_preview("request()"),
        result_preview("Value"),
        ffi_preview("c_strlen", "C"),
        unsafe_preview("raw_call()")
    ], ", ");
}

fn syntax_shortcuts() {
    return join_lines([
        "compact imports",
        "compact pattern matching",
        "compact error paths",
        "compact pipeline chaining",
        "compact helper definitions",
        "compact async / await previews",
        "compact ffi bridge previews"
    ], ", ");
}

fn syntax_goal() {
    return "short syntax, short commands, detailed diagnostics";
}
