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
        one_line_helper("id", "value")
    ], ", ");
}
