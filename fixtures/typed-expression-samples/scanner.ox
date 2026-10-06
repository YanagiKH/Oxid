pub struct Cursor { position: i32, start: i32 }
pub enum Token { Integer(i32), Plus, Star, LeftParen, RightParen, End, Invalid(i32) }

pub fn new_cursor() -> Cursor {
    return Cursor { position: 0, start: 0 };
}

pub fn next_token(codes: &[i32], cursor: &mut Cursor) -> Token {
    return next_token_prefix(&*codes, &mut *cursor, codes.len());
}

// The backing slice can include unused cells after the input's actual end.
pub fn next_token_prefix(codes: &[i32], cursor: &mut Cursor, used: i32) -> Token {
    if used < 0 || used > codes.len() || cursor.position < 0 || cursor.position > used {
        return Token::Invalid(-1);
    }
    while cursor.position < used {
        let whitespace = codes[cursor.position];
        if whitespace == 9 || whitespace == 10 || whitespace == 13 || whitespace == 32 {
            cursor.position = cursor.position + 1;
        } else {
            break;
        }
    }
    cursor.start = cursor.position;
    if cursor.position == used {
        return Token::End;
    }
    let code = codes[cursor.position];
    if code >= 48 && code <= 57 {
        let mut value = 0;
        while cursor.position < used {
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
    if code == 42 { return Token::Star; }
    if code == 40 { return Token::LeftParen; }
    if code == 41 { return Token::RightParen; }
    return Token::Invalid(code);
}

pub fn token_start(cursor: &Cursor) -> i32 {
    return cursor.start;
}
