use "../stdlib/prelude.ox";

fn main() {
    print diag_message("error", "src/main.ox", 12, 8, "unexpected token", "remove the extra operator or finish the expression");
    print parse_failure("src/main.ox", 18, 4, "incomplete expression");
    print recoverable_diag("src/main.ox", 20, 2, "recovered after missing delimiter", "continue parsing and report follow-up notes");
}
