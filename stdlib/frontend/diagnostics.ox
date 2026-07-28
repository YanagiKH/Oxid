use "../strings.ox";
use "../errors.ox";

fn diagnostic_header(stage, file_path, line, column) {
    return "[" + stage + "] " + source_header(file_path, line, column);
}

fn diagnostic_hint(stage, hint) {
    if (hint == "") {
        return "";
    }
    return "[" + stage + "] " + hint;
}

fn render_diagnostic(stage, file_path, line, column, message, hint) {
    let out = diagnostic_header(stage, file_path, line, column) + "\n" + message;
    if (hint != "") {
        out = out + "\n" + diagnostic_hint(stage, hint);
    }
    return out;
}

fn render_parse_error(file_path, line, column, message, hint) {
    return render_diagnostic("parse", file_path, line, column, message, hint);
}

fn render_module_error(file_path, line, column, message, hint) {
    return render_diagnostic("module", file_path, line, column, message, hint);
}

fn render_frontend_error(file_path, line, column, message, hint) {
    return render_diagnostic("frontend", file_path, line, column, message, hint);
}
