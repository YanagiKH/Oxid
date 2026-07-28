use "../stdlib/prelude.ox";

fn main() {
    print frontend_pipeline("examples/frontend_preview.ox");
    print lex_preview("pub fn run()");
    print parser_preview("demo");
    print diag_message("note", "examples/frontend_preview.ox", 1, 1, "preview only", "replace with real parsing rules later");
}
