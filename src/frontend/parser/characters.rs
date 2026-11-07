/// Decode exactly one Unicode scalar without allocating temporary strings.
pub(in crate::frontend) fn character_value(text: &str) -> Option<char> {
    let content = text.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut chars = content.chars();
    let first = chars.next()?;
    if first != '\\' {
        return (chars.next().is_none() && first != '\'' && !first.is_control()).then_some(first);
    }
    let escape = chars.next()?;
    let simple = match escape {
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        '0' => Some('\0'),
        '\\' => Some('\\'),
        '\'' => Some('\''),
        '"' => Some('"'),
        _ => None,
    };
    if let Some(value) = simple {
        return chars.next().is_none().then_some(value);
    }
    let hex = |value: char| {
        value
            .is_ascii_hexdigit()
            .then(|| value.to_digit(16))
            .flatten()
    };
    match escape {
        'x' => {
            let value = hex(chars.next()?)? * 16 + hex(chars.next()?)?;
            (chars.next().is_none() && value <= 0x7f)
                .then(|| char::from_u32(value))
                .flatten()
        }
        'u' => {
            if chars.next()? != '{' {
                return None;
            }
            let mut value = 0u32;
            for (digits, next) in chars.by_ref().enumerate() {
                if next == '}' {
                    return (digits > 0 && chars.next().is_none())
                        .then(|| char::from_u32(value))
                        .flatten();
                }
                if digits == 6 {
                    return None;
                }
                value = value * 16 + hex(next)?;
            }
            None
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::character_value;

    #[test]
    fn scalar_escapes_agree_with_the_unicode_scalar_domain() {
        for code in 0..=0x10ffffu32 {
            let literal = format!("'\\u{{{code:x}}}'");
            assert_eq!(character_value(&literal), char::from_u32(code));
        }
    }

    #[test]
    fn quoted_scalar_and_escape_boundaries_are_exact() {
        for (literal, expected) in [
            ("'a'", 'a'),
            ("'中'", '中'),
            ("'🦀'", '🦀'),
            ("'\"'", '"'),
            (r"'\n'", '\n'),
            (r"'\r'", '\r'),
            (r"'\t'", '\t'),
            (r"'\0'", '\0'),
            (r"'\\'", '\\'),
            (r"'\''", '\''),
            (r"'\x00'", '\0'),
            (r"'\x7F'", '\u{7f}'),
            (r"'\u{000000}'", '\0'),
        ] {
            assert_eq!(character_value(literal), Some(expected), "{literal:?}");
        }
        for literal in [
            "",
            "'",
            "''",
            "'ab'",
            "'e\u{301}'",
            "'\n'",
            "'\0'",
            "'''",
            r"'\x80'",
            r"'\xff'",
            r"'\x0'",
            r"'\x000'",
            r"'\q'",
            r"'\u{}'",
            r"'\u{0_0}'",
            r"'\u{0000000}'",
            r"'\u{110000}'",
            r"'\u{d800}'",
            r"'\u{dfff}'",
            r"'\u{ff}x'",
            r"'\u{ff'",
            "'a",
            "a'",
            "a",
            "'\\'",
        ] {
            assert_eq!(character_value(literal), None, "{literal:?}");
        }
    }
}
