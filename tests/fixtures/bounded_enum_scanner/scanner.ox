pub struct Cursor { position: i32 }
pub enum Token { Integer(i32), Plus, End, Invalid(i32) }

pub fn new_cursor() -> Cursor {
    return Cursor { position: 0 };
}

pub fn next_token(codes: &[i32], cursor: &mut Cursor) -> Token {
    while cursor.position < codes.len() {
        let whitespace = codes[cursor.position];
        if whitespace == 9 || whitespace == 10 || whitespace == 13 || whitespace == 32 {
            cursor.position = cursor.position + 1;
        } else {
            break;
        }
    }
    if cursor.position == codes.len() {
        return Token::End;
    }
    let code = codes[cursor.position];
    if code >= 48 && code <= 57 {
        let mut value = 0;
        while cursor.position < codes.len() {
            let digit = codes[cursor.position];
            if digit < 48 || digit > 57 {
                break;
            }
            value = value * 10 + (digit - 48);
            cursor.position = cursor.position + 1;
        }
        return Token::Integer(value);
    }
    cursor.position = cursor.position + 1;
    if code == 43 {
        return Token::Plus;
    }
    return Token::Invalid(code);
}
