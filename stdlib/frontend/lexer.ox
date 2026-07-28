use "../strings.ox";
use "../errors.ox";

fn token_summary(kind, text) {
    return kind + ":" + text;
}

fn lexer_stage_name() {
    return "lexer";
}

fn lex_preview(source) {
    return token_summary(lexer_stage_name(), source);
}

fn lex_failure(file_path, line, column, token_text) {
    return parse_error(file_path, line, column, "unexpected token `" + token_text + "`", "check the syntax preview or simplify the expression");
}
