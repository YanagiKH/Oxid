use "../strings.ox";
use "../core.ox";

fn keyword_list() {
    return ["fn", "async", "await", "let", "const", "if", "else", "while", "return", "use", "print", "true", "false", "null", "macro", "and", "or"];
}

fn is_keyword(word) {
    return contains(keyword_list(), word);
}

fn token_group(kind) {
    if (kind == "keyword") {
        return "token:keyword";
    }
    if (kind == "identifier") {
        return "token:identifier";
    }
    if (kind == "literal") {
        return "token:literal";
    }
    return "token:operator";
}

fn token_summary(kind, lexeme) {
    return token_group(kind) + ":" + lexeme;
}

fn lexer_stage_name() {
    return "lexer";
}

fn lexer_hint(source_name) {
    return "scan source: " + source_name;
}
