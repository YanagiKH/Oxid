use "../strings.ox";

fn ast_node(kind, name) { return kind + ":" + name; }
fn ast_span(file_path, start_line, start_column, end_line, end_column) {
    return file_path + ":" + str(start_line) + ":" + str(start_column) + "-" + str(end_line) + ":" + str(end_column);
}
fn ast_decl(name) { return ast_node("decl", name); }
fn ast_stmt(name) { return ast_node("stmt", name); }
fn ast_expr(name) { return ast_node("expr", name); }
fn ast_module_summary(module_name) { return "ast module: " + module_name; }
