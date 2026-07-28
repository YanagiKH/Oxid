use "../stdlib/prelude.ox";

fn main() {
    print diag_message("error", "src/main.ox", 12, 8, "unexpected token", "remove the extra operator");
    print parse_failure("src/main.ox", 18, 4, "incomplete expression");
}
