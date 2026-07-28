use "../stdlib/prelude.ox";

fn main() {
    print frontend_pipeline("examples/frontend_preview.ox");
    print lex_preview("pub fn run()");
    print parser_preview("demo");
    print parse_public_function("run");
    print parse_match_preview("value");
    print parse_try_preview("expr");
    print parse_defer_preview("cleanup()");
    print parse_pipe_preview("value", "step()");
    print syntax_summary();
}
