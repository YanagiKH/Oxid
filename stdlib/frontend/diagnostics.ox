use "../strings.ox";

fn diag_line(kind, file_path, line, column) {
    return kind + " --> " + file_path + ":" + str(line) + ":" + str(column);
}

fn diag_hint(text) {
    return "help: " + text;
}

fn diag_message(kind, file_path, line, column, message, hint) {
    let out = kind + ": " + message + "\n" + diag_line(kind, file_path, line, column);
    if (hint != "") {
        out = out + "\n  = " + diag_hint(hint);
    }
    return out;
}
