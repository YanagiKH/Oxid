use "../stdlib/prelude.ox";

fn main() {
    print render_parse_error("examples/diagnostics_preview.ox", 3, 9, "unexpected token", "finish the expression before the semicolon");
    print render_module_error("examples/diagnostics_preview.ox", 8, 2, "module not found", "verify the relative import path");
}
