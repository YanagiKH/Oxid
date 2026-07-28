use "../strings.ox";
use "diagnostics.ox";

fn recovery_hint(message) { return "recover: " + message; }

fn recovery_strategy(name) {
    return "strategy:" + name;
}

fn recovery_preview(file_path, line, column, message) {
    return fatal_diag(file_path, line, column, message, recovery_hint("split the expression"));
}
