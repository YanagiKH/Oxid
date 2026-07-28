use "../strings.ox";
use "../errors.ox";
use "lexer.ox";
use "diagnostics.ox";
use "modules.ox";

fn parser_stage_name() {
    return "parser";
}

fn declaration_kind(text) {
    if (text == "fn") {
        return "function";
    }
    if (text == "let") {
        return "variable";
    }
    if (text == "const") {
        return "constant";
    }
    if (text == "use") {
        return "module";
    }
    return "expression";
}

fn parse_unit_name(file_path) {
    return "unit:" + file_path;
}

fn parse_declaration_hint(file_path, name, kind) {
    return render_parse_error(file_path, 1, 1, "parse declaration " + name, "kind: " + declaration_kind(kind));
}

fn parse_expression_hint(file_path, expression_name) {
    return render_parse_error(file_path, 1, 1, "parse expression " + expression_name, "expression is part of the frontend pipeline");
}

fn parse_module_hint(base_dir, import_path) {
    return render_module_error(base_dir, 1, 1, "resolve module " + import_path, module_load_hint(base_dir, import_path));
}

fn parse_error_preview(file_path, message, hint) {
    return render_frontend_error(file_path, 1, 1, message, hint);
}
