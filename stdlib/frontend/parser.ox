use "../strings.ox";
use "../errors.ox";

fn parser_stage_name() {
    return "parser";
}

fn parser_preview(name) {
    return "parser:" + name;
}

fn parse_function_header(name) {
    return "fn " + name + "(...)";
}

fn parse_module_header(name) {
    return "mod " + name;
}

fn parse_alias_import(path, alias) {
    return "use " + path + " as " + alias;
}

fn parse_failure(file_path, line, column, message) {
    return parse_error(file_path, line, column, message, "simplify the form or split it into smaller steps");
}
