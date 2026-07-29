use "../strings.ox";
use "../errors.ox";

fn diag_line(kind, file_path, line, column) {
    return kind + " --> " + file_path + ":" + str(line) + ":" + str(column);
}

fn diag_hint(text) { return "help: " + text; }

fn diag_frame(file_path, line, column, snippet) {
    return file_path + ":" + str(line) + ":" + str(column) + "
" + snippet;
}

fn diag_code(code) { return "OXD" + code; }

fn diag_message(kind, file_path, line, column, message, hint) {
    let out = kind + ": " + message + "
" + diag_line(kind, file_path, line, column);
    if (hint != "") { out = out + "
  = " + diag_hint(hint); }
    return out;
}

fn diag_with_code(code, kind, file_path, line, column, message, hint) {
    return diag_code(code) + "
" + diag_message(kind, file_path, line, column, message, hint);
}

fn diag_suggestion(action, target) {
    return "try " + action + " " + target;
}

fn recoverable_diag(file_path, line, column, message, hint) {
    return diag_message("warning", file_path, line, column, message, hint);
}

fn fatal_diag(file_path, line, column, message, hint) {
    return diag_message("error", file_path, line, column, message, hint);
}

fn diag_context(file_path, line, column, snippet, hint) {
    return diag_frame(file_path, line, column, snippet) + "
" + diag_hint(hint);
}

fn diag_parse_failure(file_path, line, column, token, expected) {
    return diag_with_code("1001", "error", file_path, line, column, "unexpected token " + token, "expected " + expected);
}

fn diag_module_failure(module_name, file_path, line, column) {
    return diag_with_code("2001", "error", file_path, line, column, "failed to resolve module " + module_name, "check the module path and imports");
}

fn diag_ffi_failure(target, file_path, line, column) {
    return diag_with_code("3001", "warning", file_path, line, column, "interop bridge not fully configured for " + target, "generate a bridge stub and rebuild");
}
