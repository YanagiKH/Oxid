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
fn syntax_profile_name() { return "Oxid concise syntax profile"; }
fn syntax_note(name) { return name + " is available in the Oxid parser"; }

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
        "fun id(value) => value;",
        "var name = value;",
        "when ready { say value; } otherwise { say none; }",
        "for item in values { say item; }",
        "value |> transform |> str",
        "work fun fetch() => await request();",
        "import \"module.ox\";",
        "yes / no / none",
        "give value;",
        "break; / continue;"
    ], ", ");
}

fn syntax_shortcuts() {
    return join_lines([
        "fun / var / say / give aliases",
        "when / otherwise conditions without mandatory parentheses",
        "for-in loops with break and continue",
        "pipeline chaining with |> and argument insertion",
        "single-expression functions with =>",
        "work functions with async / await compatibility",
        "process bridges for Python, Java, Go, C, and C++"
    ], ", ");
}

fn syntax_goal() {
    return "short syntax, short commands, detailed diagnostics";
}
