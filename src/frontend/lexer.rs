use super::diagnostic::Diagnostic;
use super::source::{SourceFile, Span};

pub const MAX_TOKENS: usize = 100_000;
pub const MAX_TOKEN_BYTES: usize = 65_536;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Trivia,
    Ident,
    Number,
    String,
    Fn,
    Let,
    Return,
    If,
    Else,
    True,
    False,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Semi,
    Equal,
    Arrow,
    Minus,
    Plus,
    Star,
    Unsupported,
    Invalid,
    Eof,
}
#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: Kind,
    pub span: Span,
}

/// Retains every byte, including trivia and unsupported literal spelling.
pub fn lex(source: &SourceFile) -> Result<Vec<Token>, Box<Diagnostic>> {
    let text = source.text();
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let start = cursor;
        let ch = text[cursor..].chars().next().unwrap();
        cursor += ch.len_utf8();
        let kind = match ch {
            c if c.is_whitespace() => {
                while cursor < bytes.len() {
                    let next = text[cursor..].chars().next().unwrap();
                    if !next.is_whitespace() {
                        break;
                    }
                    cursor += next.len_utf8();
                }
                Kind::Trivia
            }
            '/' if bytes.get(cursor) == Some(&b'/') => {
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
                Kind::Trivia
            }
            '/' if bytes.get(cursor) == Some(&b'*') => {
                cursor += 1;
                let mut closed = false;
                while cursor + 1 < bytes.len() {
                    if &bytes[cursor..cursor + 2] == b"*/" {
                        cursor += 2;
                        closed = true;
                        break;
                    }
                    cursor += 1;
                }
                if !closed {
                    return Err(Diagnostic::new(
                        "E0100",
                        "lex",
                        "unterminated block comment",
                        Some(source.span(start, text.len())),
                    ));
                }
                Kind::Trivia
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                while cursor < bytes.len()
                    && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
                {
                    cursor += 1;
                }
                match &text[start..cursor] {
                    "fn" => Kind::Fn,
                    "let" => Kind::Let,
                    "return" => Kind::Return,
                    "if" => Kind::If,
                    "else" => Kind::Else,
                    "true" => Kind::True,
                    "false" => Kind::False,
                    "use" | "import" | "macro" | "macro_rules" | "mut" | "const" | "while"
                    | "for" | "loop" | "match" | "break" | "continue" | "async" | "await"
                    | "move" | "ref" | "unsafe" | "extern" | "struct" | "enum" | "trait"
                    | "impl" | "type" | "mod" | "pub" | "null" | "and" | "or" => Kind::Unsupported,
                    _ => Kind::Ident,
                }
            }
            c if c.is_numeric() => {
                // Preserve the whole candidate, including unsupported Unicode,
                // suffix/radix/float spelling. Parsing accepts ASCII digits only.
                while cursor < bytes.len() {
                    let next = text[cursor..].chars().next().unwrap();
                    if !next.is_alphanumeric() && !matches!(next, '_' | '.') {
                        break;
                    }
                    cursor += next.len_utf8();
                }
                Kind::Number
            }
            '"' => {
                let mut closed = false;
                while cursor < bytes.len() {
                    let next = text[cursor..].chars().next().unwrap();
                    cursor += next.len_utf8();
                    if next == '"' {
                        closed = true;
                        break;
                    }
                    if next == '\\' && cursor < bytes.len() {
                        cursor += text[cursor..].chars().next().unwrap().len_utf8();
                    }
                }
                if !closed {
                    return Err(Diagnostic::new(
                        "E0100",
                        "lex",
                        "unterminated string literal",
                        Some(source.span(start, cursor)),
                    ));
                }
                Kind::String
            }
            '(' => Kind::LParen,
            ')' => Kind::RParen,
            '{' => Kind::LBrace,
            '}' => Kind::RBrace,
            ':' => Kind::Colon,
            ',' => Kind::Comma,
            ';' => Kind::Semi,
            '=' => Kind::Equal,
            '-' if bytes.get(cursor) == Some(&b'>') => {
                cursor += 1;
                Kind::Arrow
            }
            '-' => Kind::Minus,
            '+' => Kind::Plus,
            '*' => Kind::Star,
            '/' | '%' | '&' | '|' | '!' | '[' | ']' | '.' | '<' | '>' | '#' | '\'' => {
                Kind::Unsupported
            }
            _ => Kind::Invalid,
        };
        let span = source.span(start, cursor);
        if cursor - start > MAX_TOKEN_BYTES || tokens.len() >= MAX_TOKENS {
            return Err(Diagnostic::new(
                "E0400",
                "lex",
                "token resource limit exceeded",
                Some(span),
            ));
        }
        tokens.push(Token { kind, span });
    }
    tokens.push(Token {
        kind: Kind::Eof,
        span: source.span(cursor, cursor),
    });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::source::{SourceFileId, SourceMap};
    #[test]
    fn token_tape_retains_every_byte_and_exact_numeric_spelling() {
        let text =
            "// 雪\r\n fn f() -> () { /* é */ 900719925474099312345678901234567890; return; } @";
        let mut sources = SourceMap::new();
        let id = sources.add("input.ox".into(), text.into());
        let tokens = lex(sources.get(id)).unwrap();
        let mut end = 0;
        let mut reconstructed = String::new();
        for token in &tokens {
            assert_eq!(token.span.file, SourceFileId(0));
            assert_eq!(token.span.start, end);
            reconstructed.push_str(&text[token.span.start..token.span.end]);
            end = token.span.end;
        }
        assert_eq!(reconstructed, text);
        let number = tokens
            .iter()
            .find(|token| token.kind == Kind::Number)
            .unwrap();
        assert_eq!(
            &text[number.span.start..number.span.end],
            "900719925474099312345678901234567890"
        );
        assert_eq!(tokens.last().unwrap().kind, Kind::Eof);
        assert_eq!(tokens.last().unwrap().span.start, text.len());
        assert!(tokens.iter().any(|token| token.kind == Kind::Invalid));
    }
}
