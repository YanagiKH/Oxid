use "../strings.ox";

fn lexer_stage_name() { return "lexer"; }
fn lex_preview(source_text) { return "lex:" + source_text; }
fn lex_token(kind, value) { return kind + ":" + value; }
fn lex_preview_tokens() {
    return join_lines([
        lex_token("ident", "main"),
        lex_token("symbol", "("),
        lex_token("symbol", ")")
    ], ", ");
}
