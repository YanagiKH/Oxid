use "strings.ox";

fn source_header(file_path, line, column) {
    return file_path + ":" + str(line) + ":" + str(column);
}

fn error_line(kind, file_path, line, column, message) {
    return kind + ": " + message + "\n --> " + source_header(file_path, line, column);
}

fn hint_line(hint) {
    if (hint == "") { return ""; }
    return "  = help: " + hint;
}

fn make_error(kind, file_path, line, column, message, hint) {
    let out = error_line(kind, file_path, line, column, message);
    if (hint != "") {
        out = out + "\n" + hint_line(hint);
    }
    return out;
}

fn parse_error(file_path, line, column, message, hint) {
    return make_error("error", file_path, line, column, message, hint);
}

fn runtime_error(file_path, line, column, message, hint) {
    return make_error("runtime error", file_path, line, column, message, hint);
}

fn note(message) { return "  = note: " + message; }

fn recoverable_error(file_path, line, column, message, hint) {
    return make_error("warning", file_path, line, column, message, hint);
}
