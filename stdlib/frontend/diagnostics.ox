use "../strings.ox";
use "../errors.ox";

fn diag_line(kind, file_path, line, column) {
    return kind + " --> " + file_path + ":" + str(line) + ":" + str(column);
}

fn diag_hint(text) { return "help: " + text; }

fn diag_frame(file_path, line, column, snippet) {
    return file_path + ":" + str(line) + ":" + str(column) + "\n" + snippet;
}

fn diag_message(kind, file_path, line, column, message, hint) {
    let out = kind + ": " + message + "\n" + diag_line(kind, file_path, line, column);
    if (hint != "") { out = out + "\n  = " + diag_hint(hint); }
    return out;
}

fn diag_code(code) { return "OXD" + code; }

fn diag_with_code(code, kind, file_path, line, column, message, hint) {
    return diag_code(code) + "\n" + diag_message(kind, file_path, line, column, message, hint);
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
    return diag_frame(file_path, line, column, snippet) + "\n" + diag_hint(hint);
}
