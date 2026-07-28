use "../strings.ox";
use "../errors.ox";

fn lexer_stage_name() { return "lexer"; }
fn token_summary(kind, text) { return kind + ":" + text; }
fn lex_preview(source) { return token_summary(lexer_stage_name(), source); }
fn lex_failure(file_path, line, column, token_text) {
    return parse_error(file_path, line, column, "unexpected token `" + token_text + "`", "simplify the expression or split it into smaller steps");
}
fn lex_keywords() {
    return ["fn", "pub", "mod", "use", "match", "try", "defer", "pipe", "let", "const", "return", "if", "else", "while", "async", "await"];
}
