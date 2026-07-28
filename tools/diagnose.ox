use "../stdlib/prelude.ox";

fn main() {
    print render_parse_error("src/main.ox", 12, 8, "unexpected token", "remove the extra operator or finish the expression");
    print render_module_error("src/main.ox", 4, 1, "module not found", "check the path and the current file directory");
    print render_frontend_error("src/main.ox", 1, 1, "frontend preview only", "move parser helpers into Oxid modules");
}
