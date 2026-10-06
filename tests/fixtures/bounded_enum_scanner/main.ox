mod scanner;
use crate::scanner::Token as Lexeme;
use crate::scanner::new_cursor as new_cursor;
use crate::scanner::next_token as next_token;

fn main() -> i32 {
    let codes = [49, 50, 32, 43, 32, 51];
    let mut cursor = new_cursor();
    let mut result = 0;
    let mut finished = false;
    while !finished {
        let token = next_token(&codes, &mut cursor);
        match token {
            Lexeme::Integer(value) => { result = result + value; },
            Lexeme::Plus => { result = result + 100; },
            Lexeme::End => { finished = true; },
            Lexeme::Invalid(code) => { result = result - code; },
        }
    }
    return result;
}
