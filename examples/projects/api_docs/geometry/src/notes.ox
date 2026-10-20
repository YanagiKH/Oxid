// Ordinary comments never attach.
pub fn plain() -> i32 {
    return 1;
}

/// Separated by a blank line.

pub fn after_blank() -> i32 {
    return 2;
}

/// Broken by an ordinary comment.
// ordinary
pub fn after_ordinary() -> i32 {
    return 3;
}

//// Four slashes are an ordinary comment.
pub fn four_slashes() -> i32 {
    return 4;
}

/// Not attached: a four-slash line follows.
//// ordinary
pub fn four_breaks() -> i32 {
    return 5;
}

/// Attached to the private function, not to the next one.
fn private_one() -> i32 { return 6; } pub fn code_before() -> i32 { return 7; }

/// Broken by a block comment token.
/* block */ pub fn after_block_token() -> i32 {
    return 8;
}

/* A block comment
/// that only looks like documentation */
pub fn inside_block() -> i32 {
    return 9;
}

fn slashes() -> [u8; 3] { return b"///"; }
pub fn after_byte_literal() -> i32 {
    return 10;
}

    /// Indented documentation is allowed.
///No space after the slashes.
///  Two spaces: one is removed.
///
///     Indented content stays content.
    pub fn indented() -> i32 {
        return 11;
    }

/// Escaped: \ ! " # $ % & ' ( ) * + , - . / : ; < = > ? @ [ ] ^ _ ` { | } ~
/// HTML <script>alert(1)</script> and <!-- comment --> stay text.
/// Links [x](javascript:alert) <https://example.com> and ```fences``` stay text.
/// Not markup: # heading, > quote, - item, 1. item, | table |, *em* __strong__
/// Literal [U+0007] is text, not a control.
pub fn escaped() -> i32 {
    return 12;
}

/// Unicode stays: café, 日本語, Ελληνικά, 🦀, é (combining).
pub fn unicode() -> i32 {
    return 13;
}

/* é🦀 */ pub fn wide_column() -> i32 {
    return 14;
}

pub	fn	tabbed() -> i32 {
    return 15;
}
