use super::{RuntimeError, Value};

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

const MAX_JSON_INPUT_BYTES: usize = 4 * 1024 * 1024;
const MAX_JSON_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_JSON_DEPTH: usize = 128;

fn runtime_error(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Message(message.into())
}

fn string_argument(value: &Value, label: &str) -> Result<String, RuntimeError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        _ => Err(runtime_error(format!("{label} must be a string"))),
    }
}

fn record_argument(
    value: &Value,
    label: &str,
) -> Result<Rc<RefCell<BTreeMap<String, Value>>>, RuntimeError> {
    match value {
        Value::Record(record) => Ok(record.clone()),
        _ => Err(runtime_error(format!("{label} must be a record"))),
    }
}

pub(super) fn json_parse(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(runtime_error("json_parse requires exactly 1 string"));
    }
    let source = string_argument(&args[0], "json_parse input")?;
    JsonParser::new(&source)
        .parse_document()
        .map_err(runtime_error)
}

pub(super) fn json_stringify(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(runtime_error("json_stringify requires exactly 1 value"));
    }
    JsonStringifier::new()
        .stringify(&args[0])
        .map(Value::String)
        .map_err(runtime_error)
}

pub(super) fn keys(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(runtime_error("keys requires exactly 1 record"));
    }
    let record = record_argument(&args[0], "keys value")?;
    let record = record
        .try_borrow()
        .map_err(|_| runtime_error("record is already mutably borrowed"))?;
    let values = record.keys().cloned().map(Value::String).collect();
    Ok(Value::Array(Rc::new(RefCell::new(values))))
}

pub(super) fn has_key(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(runtime_error("has_key requires a record and string key"));
    }
    let record = record_argument(&args[0], "has_key value")?;
    let key = string_argument(&args[1], "has_key key")?;
    let record = record
        .try_borrow()
        .map_err(|_| runtime_error("record is already mutably borrowed"))?;
    Ok(Value::Bool(record.contains_key(&key)))
}

pub(super) fn get(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&args.len()) {
        return Err(runtime_error(
            "get requires a record, string key, and optional default",
        ));
    }
    let record = record_argument(&args[0], "get value")?;
    let key = string_argument(&args[1], "get key")?;
    let default = args.get(2).cloned().unwrap_or(Value::Null);
    let record = record
        .try_borrow()
        .map_err(|_| runtime_error("record is already mutably borrowed"))?;
    Ok(record.get(&key).cloned().unwrap_or(default))
}

pub(super) fn set(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(runtime_error(
            "set requires a record, string key, and value",
        ));
    }
    let record = record_argument(&args[0], "set value")?;
    let key = string_argument(&args[1], "set key")?;
    record
        .try_borrow_mut()
        .map_err(|_| runtime_error("record is already borrowed"))?
        .insert(key, args[2].clone());
    Ok(Value::Record(record))
}

pub(super) fn remove(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(runtime_error("remove requires a record and string key"));
    }
    let record = record_argument(&args[0], "remove value")?;
    let key = string_argument(&args[1], "remove key")?;
    let removed = record
        .try_borrow_mut()
        .map_err(|_| runtime_error("record is already borrowed"))?
        .remove(&key)
        .unwrap_or(Value::Null);
    Ok(removed)
}

pub(super) fn record(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let entries = if args.len() == 1 {
        match &args[0] {
            Value::Array(values) => values
                .try_borrow()
                .map_err(|_| runtime_error("record entries are already mutably borrowed"))?
                .clone(),
            _ => args,
        }
    } else {
        args
    };

    if entries.len() % 2 != 0 {
        return Err(runtime_error(
            "record requires alternating string keys and values",
        ));
    }

    let mut values = BTreeMap::new();
    let mut entries = entries.into_iter();
    while let Some(key) = entries.next() {
        let value = entries
            .next()
            .ok_or_else(|| runtime_error("record key is missing its value"))?;
        let key = string_argument(&key, "record key")?;
        values.insert(key, value);
    }
    Ok(Value::Record(Rc::new(RefCell::new(values))))
}

fn utf8_char_width(first: u8) -> Option<usize> {
    match first {
        0x00..=0x7f => Some(1),
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

struct JsonParser<'a> {
    source: &'a [u8],
    index: usize,
}

impl<'a> JsonParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source: source.as_bytes(),
            index: 0,
        }
    }

    fn parse_document(mut self) -> Result<Value, String> {
        if self.source.len() > MAX_JSON_INPUT_BYTES {
            return Err(format!(
                "JSON input exceeds the {MAX_JSON_INPUT_BYTES}-byte limit"
            ));
        }
        self.skip_whitespace();
        let value = self.parse_value(0)?;
        self.skip_whitespace();
        if self.index != self.source.len() {
            return Err(self.error("unexpected trailing JSON data"));
        }
        Ok(value)
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, String> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(Value::Null)
            }
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b'[') => {
                if depth >= MAX_JSON_DEPTH {
                    return Err(self.error("JSON nesting limit exceeded"));
                }
                self.parse_array(depth + 1)
            }
            Some(b'{') => {
                if depth >= MAX_JSON_DEPTH {
                    return Err(self.error("JSON nesting limit exceeded"));
                }
                self.parse_object(depth + 1)
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(Value::Number),
            Some(_) => Err(self.error("invalid JSON value")),
            None => Err(self.error("unexpected end of JSON input")),
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, String> {
        self.expect_byte(b'[', "array must start with `[`")?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume_byte(b']') {
            return Ok(Value::Array(Rc::new(RefCell::new(values))));
        }

        loop {
            values.push(self.parse_value(depth)?);
            self.skip_whitespace();
            if self.consume_byte(b']') {
                break;
            }
            self.expect_byte(b',', "array entries must be separated by `,`")?;
            self.skip_whitespace();
            if self.peek() == Some(b']') {
                return Err(self.error("trailing commas are not valid JSON"));
            }
        }
        Ok(Value::Array(Rc::new(RefCell::new(values))))
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, String> {
        self.expect_byte(b'{', "object must start with `{`")?;
        self.skip_whitespace();
        let mut values = BTreeMap::new();
        if self.consume_byte(b'}') {
            return Ok(Value::Record(Rc::new(RefCell::new(values))));
        }

        loop {
            if self.peek() != Some(b'"') {
                return Err(self.error("JSON object keys must be strings"));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect_byte(b':', "JSON object keys must be followed by `:`")?;
            let value = self.parse_value(depth)?;
            if values.insert(key.clone(), value).is_some() {
                return Err(self.error(&format!("duplicate JSON object key `{key}`")));
            }
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                break;
            }
            self.expect_byte(b',', "object entries must be separated by `,`")?;
            self.skip_whitespace();
            if self.peek() == Some(b'}') {
                return Err(self.error("trailing commas are not valid JSON"));
            }
        }
        Ok(Value::Record(Rc::new(RefCell::new(values))))
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.expect_byte(b'"', "string must start with a quote")?;
        let mut output = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| self.error("unterminated JSON string"))?;
            match byte {
                b'"' => {
                    self.index += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.index += 1;
                    let escape = self
                        .take()
                        .ok_or_else(|| self.error("unterminated JSON escape"))?;
                    match escape {
                        b'"' => output.push('"'),
                        b'\\' => output.push('\\'),
                        b'/' => output.push('/'),
                        b'b' => output.push('\u{0008}'),
                        b'f' => output.push('\u{000c}'),
                        b'n' => output.push('\n'),
                        b'r' => output.push('\r'),
                        b't' => output.push('\t'),
                        b'u' => output.push(self.parse_unicode_escape()?),
                        _ => return Err(self.error("invalid JSON string escape")),
                    }
                }
                0x00..=0x1f => {
                    return Err(self.error("unescaped control character in JSON string"));
                }
                0x20..=0x7f => {
                    output.push(byte as char);
                    self.index += 1;
                }
                _ => {
                    // Validate and consume only the next UTF-8 scalar. The previous
                    // approach re-validated the entire remaining input on every
                    // non-ASCII character, which scanned the same bytes repeatedly.
                    output.push(self.consume_utf8_char()?);
                }
            }
        }
    }

    /// Decode one UTF-8 character starting at `self.index` without scanning the
    /// rest of the input. Acceptance rules match `str::from_utf8` on that span.
    fn consume_utf8_char(&mut self) -> Result<char, String> {
        let bytes = &self.source[self.index..];
        let first = *bytes
            .first()
            .ok_or_else(|| self.error("unterminated JSON string"))?;
        let width =
            utf8_char_width(first).ok_or_else(|| self.error("invalid UTF-8 in JSON string"))?;
        let Some(candidate) = bytes.get(..width) else {
            return Err(self.error("invalid UTF-8 in JSON string"));
        };
        let character = match std::str::from_utf8(candidate) {
            Ok(text) => {
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(character), None) if character.len_utf8() == width => character,
                    _ => return Err(self.error("invalid UTF-8 in JSON string")),
                }
            }
            Err(_) => return Err(self.error("invalid UTF-8 in JSON string")),
        };
        self.index += width;
        Ok(character)
    }

    fn parse_unicode_escape(&mut self) -> Result<char, String> {
        let first = self.parse_hex_quad()?;
        let scalar = if (0xd800..=0xdbff).contains(&first) {
            if self.source.get(self.index..self.index + 2) != Some(b"\\u") {
                return Err(self.error("high surrogate must be followed by a low surrogate"));
            }
            self.index += 2;
            let second = self.parse_hex_quad()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(self.error("high surrogate must be followed by a low surrogate"));
            }
            0x10000 + (((first as u32 - 0xd800) << 10) | (second as u32 - 0xdc00))
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err(self.error("unexpected low surrogate in JSON string"));
        } else {
            first as u32
        };
        char::from_u32(scalar).ok_or_else(|| self.error("invalid Unicode scalar in JSON string"))
    }

    fn parse_hex_quad(&mut self) -> Result<u16, String> {
        if self.index + 4 > self.source.len() {
            return Err(self.error("incomplete Unicode escape"));
        }
        let mut value = 0u16;
        for _ in 0..4 {
            let byte = self
                .take()
                .ok_or_else(|| self.error("incomplete Unicode escape"))?;
            let digit = match byte {
                b'0'..=b'9' => (byte - b'0') as u16,
                b'a'..=b'f' => (byte - b'a' + 10) as u16,
                b'A'..=b'F' => (byte - b'A' + 10) as u16,
                _ => return Err(self.error("Unicode escapes require four hexadecimal digits")),
            };
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<f64, String> {
        let start = self.index;
        self.consume_byte(b'-');

        match self.peek() {
            Some(b'0') => {
                self.index += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error("JSON numbers cannot contain leading zeroes"));
                }
            }
            Some(b'1'..=b'9') => {
                self.index += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.index += 1;
                }
            }
            _ => return Err(self.error("invalid JSON number")),
        }

        if self.consume_byte(b'.') {
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("JSON number fraction requires a digit"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }

        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.index += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.index += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("JSON number exponent requires a digit"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }

        let text = std::str::from_utf8(&self.source[start..self.index])
            .map_err(|_| self.error("invalid JSON number"))?;
        let number = text
            .parse::<f64>()
            .map_err(|_| self.error("invalid JSON number"))?;
        if !number.is_finite() {
            return Err(self.error("JSON number is outside the finite numeric range"));
        }
        Ok(number)
    }

    fn expect_literal(&mut self, literal: &[u8]) -> Result<(), String> {
        if self.source.get(self.index..self.index + literal.len()) == Some(literal) {
            self.index += literal.len();
            Ok(())
        } else {
            Err(self.error("invalid JSON literal"))
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.index += 1;
        }
    }

    fn expect_byte(&mut self, byte: u8, message: &str) -> Result<(), String> {
        if self.consume_byte(byte) {
            Ok(())
        } else {
            Err(self.error(message))
        }
    }

    fn consume_byte(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.index).copied()
    }

    fn take(&mut self) -> Option<u8> {
        let value = self.peek()?;
        self.index += 1;
        Some(value)
    }

    fn error(&self, message: &str) -> String {
        format!("{message} at byte {}", self.index)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ContainerId {
    Array(usize),
    Record(usize),
}

struct JsonStringifier {
    output: String,
    active: HashSet<ContainerId>,
}

impl JsonStringifier {
    fn new() -> Self {
        Self {
            output: String::new(),
            active: HashSet::new(),
        }
    }

    fn stringify(mut self, value: &Value) -> Result<String, String> {
        self.write_value(value, 0)?;
        Ok(self.output)
    }

    fn write_value(&mut self, value: &Value, depth: usize) -> Result<(), String> {
        match value {
            Value::Null => self.push_str("null")?,
            Value::Bool(value) => self.push_str(if *value { "true" } else { "false" })?,
            Value::Number(value) => {
                if !value.is_finite() {
                    return Err("JSON cannot encode NaN or infinity".to_string());
                }
                self.push_str(&value.to_string())?;
            }
            Value::String(value) => self.write_string(value)?,
            Value::Array(values) => {
                if depth >= MAX_JSON_DEPTH {
                    return Err("JSON nesting limit exceeded".to_string());
                }
                let id = ContainerId::Array(Rc::as_ptr(values) as *const () as usize);
                if !self.active.insert(id) {
                    return Err("cannot stringify a cyclic array".to_string());
                }
                let result = (|| {
                    let values = values
                        .try_borrow()
                        .map_err(|_| "array is already mutably borrowed".to_string())?;
                    self.push_char('[')?;
                    for (index, value) in values.iter().enumerate() {
                        if index != 0 {
                            self.push_char(',')?;
                        }
                        self.write_value(value, depth + 1)?;
                    }
                    self.push_char(']')
                })();
                self.active.remove(&id);
                result?;
            }
            Value::Record(values) => {
                if depth >= MAX_JSON_DEPTH {
                    return Err("JSON nesting limit exceeded".to_string());
                }
                let id = ContainerId::Record(Rc::as_ptr(values) as *const () as usize);
                if !self.active.insert(id) {
                    return Err("cannot stringify a cyclic record".to_string());
                }
                let result = (|| {
                    let values = values
                        .try_borrow()
                        .map_err(|_| "record is already mutably borrowed".to_string())?;
                    self.push_char('{')?;
                    for (index, (key, value)) in values.iter().enumerate() {
                        if index != 0 {
                            self.push_char(',')?;
                        }
                        self.write_string(key)?;
                        self.push_char(':')?;
                        self.write_value(value, depth + 1)?;
                    }
                    self.push_char('}')
                })();
                self.active.remove(&id);
                result?;
            }
            Value::Function(_)
            | Value::Task(_)
            | Value::NativeFunction(_)
            | Value::Listener(_)
            | Value::Connection(_) => {
                return Err("value cannot be represented as JSON".to_string());
            }
        }
        Ok(())
    }

    fn write_string(&mut self, value: &str) -> Result<(), String> {
        self.push_char('"')?;
        for character in value.chars() {
            match character {
                '"' => self.push_str("\\\"")?,
                '\\' => self.push_str("\\\\")?,
                '\u{0008}' => self.push_str("\\b")?,
                '\u{000c}' => self.push_str("\\f")?,
                '\n' => self.push_str("\\n")?,
                '\r' => self.push_str("\\r")?,
                '\t' => self.push_str("\\t")?,
                '\u{2028}' => self.push_str("\\u2028")?,
                '\u{2029}' => self.push_str("\\u2029")?,
                character if character <= '\u{001f}' => {
                    const HEX: &[u8; 16] = b"0123456789abcdef";
                    let value = character as usize;
                    let escape = [
                        b'\\',
                        b'u',
                        b'0',
                        b'0',
                        HEX[(value >> 4) & 0x0f],
                        HEX[value & 0x0f],
                    ];
                    let escape = std::str::from_utf8(&escape)
                        .map_err(|_| "failed to encode JSON escape".to_string())?;
                    self.push_str(escape)?;
                }
                character => self.push_char(character)?,
            }
        }
        self.push_char('"')
    }

    fn push_char(&mut self, value: char) -> Result<(), String> {
        self.output.push(value);
        self.check_output_limit()
    }

    fn push_str(&mut self, value: &str) -> Result<(), String> {
        self.output.push_str(value);
        self.check_output_limit()
    }

    fn check_output_limit(&self) -> Result<(), String> {
        if self.output.len() > MAX_JSON_OUTPUT_BYTES {
            Err(format!(
                "JSON output exceeds the {MAX_JSON_OUTPUT_BYTES}-byte limit"
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_surrogate_pairs_and_stringifies_records_deterministically() {
        let parsed = json_parse(vec![Value::String(
            r#"{"z":[1,true,null],"a":"\uD83D\uDE80"}"#.to_string(),
        )])
        .expect("valid JSON");
        let rendered = json_stringify(vec![parsed]).expect("stringify JSON");
        assert!(matches!(
            rendered,
            Value::String(value) if value == r#"{"a":"🚀","z":[1,true,null]}"#
        ));
    }

    #[test]
    fn rejects_invalid_surrogates_duplicate_keys_and_non_finite_numbers() {
        assert!(json_parse(vec![Value::String(r#""\uD800""#.to_string())]).is_err());
        assert!(json_parse(vec![Value::String(r#"{"a":1,"a":2}"#.to_string())]).is_err());
        assert!(json_parse(vec![Value::String(" ".repeat(MAX_JSON_INPUT_BYTES + 1))]).is_err());
        assert!(json_stringify(vec![Value::Number(f64::NAN)]).is_err());
        assert!(json_stringify(vec![Value::Number(f64::INFINITY)]).is_err());
    }

    #[test]
    fn enforces_depth_and_detects_container_cycles() {
        let too_deep = format!(
            "{}null{}",
            "[".repeat(MAX_JSON_DEPTH + 1),
            "]".repeat(MAX_JSON_DEPTH + 1)
        );
        assert!(json_parse(vec![Value::String(too_deep)]).is_err());

        let values = Rc::new(RefCell::new(Vec::new()));
        values.borrow_mut().push(Value::Array(values.clone()));
        assert!(json_stringify(vec![Value::Array(values)]).is_err());
    }

    #[test]
    fn record_helpers_are_sorted_and_mutable_without_panics() {
        let value = record(vec![
            Value::String("z".to_string()),
            Value::Number(1.0),
            Value::String("a".to_string()),
            Value::Bool(true),
        ])
        .expect("record");

        let result = keys(vec![value.clone()]).expect("keys");
        let Value::Array(result) = result else {
            panic!("keys must return an array");
        };
        assert!(matches!(
            result.borrow().as_slice(),
            [Value::String(a), Value::String(z)] if a == "a" && z == "z"
        ));

        assert!(matches!(
            has_key(vec![value.clone(), Value::String("a".to_string())]),
            Ok(Value::Bool(true))
        ));
        set(vec![
            value.clone(),
            Value::String("a".to_string()),
            Value::Number(2.0),
        ])
        .expect("set");
        assert!(matches!(
            get(vec![value.clone(), Value::String("a".to_string())]),
            Ok(Value::Number(2.0))
        ));
        assert!(matches!(
            remove(vec![value, Value::String("a".to_string())]),
            Ok(Value::Number(2.0))
        ));
    }

    fn parse_ok(source: &str) -> Value {
        json_parse(vec![Value::String(source.to_string())]).expect("expected JSON to parse")
    }

    fn parse_err(source: &str) -> String {
        match json_parse(vec![Value::String(source.to_string())]) {
            Err(RuntimeError::Message(message)) => message,
            Err(other) => panic!("unexpected error variant: {other:?}"),
            Ok(_) => panic!("expected JSON to fail"),
        }
    }

    fn expect_string(value: Value) -> String {
        match value {
            Value::String(text) => text,
            _ => panic!("expected string value"),
        }
    }

    #[test]
    fn parses_mixed_ascii_and_multibyte_unicode_with_escapes() {
        // 2-byte (é), 3-byte (你), 4-byte (𝄞 / 🚀) mixed with ASCII and escapes.
        let source = "\"Aé你\\uD834\\uDD1E🚀\\\\n\\tB\"";
        let value = expect_string(parse_ok(source));
        assert_eq!(value, "Aé你𝄞🚀\\n\tB");

        // Literal UTF-8 scalar for U+1F680 alongside the escaped form (UTF-16 surrogate pair).
        let literal = parse_ok("\"🚀\"");
        let escaped = parse_ok(r#""\uD83D\uDE80""#);
        assert_eq!(expect_string(literal), expect_string(escaped));
    }

    #[test]
    fn unicode_keys_and_escape_equivalent_keys_detect_duplicates() {
        let ok = parse_ok(r#"{"你好":1,"世界":2}"#);
        let Value::Record(record) = ok else {
            panic!("expected record");
        };
        let record = record.borrow();
        assert!(matches!(
            record.get("你好"),
            Some(Value::Number(value)) if *value == 1.0
        ));
        assert!(matches!(
            record.get("世界"),
            Some(Value::Number(value)) if *value == 2.0
        ));

        // Literal UTF-8 key and \u-escaped equivalent must collide.
        let err = parse_err(r#"{"é":1,"\u00e9":2}"#);
        assert!(
            err.contains("duplicate JSON object key"),
            "missing duplicate detection: {err}"
        );

        let err = parse_err(r#"{"🚀":1,"\uD83D\uDE80":2}"#);
        assert!(
            err.contains("duplicate JSON object key"),
            "missing surrogate-key duplicate detection: {err}"
        );
    }

    #[test]
    fn unicode_error_positions_unterminated_strings_and_illegal_escapes() {
        // "é" is bytes: 22 C3 A9 22. Trailing junk starts after the closing quote.
        let err = parse_err("\"é\"x");
        assert!(
            err.contains("unexpected trailing JSON data at byte 4"),
            "wrong trailing error position: {err}"
        );

        // Unterminated string after a 3-byte character: opening quote + 你 (E4 BD A0).
        let err = parse_err("\"你");
        assert!(
            err.contains("unterminated JSON string at byte 4"),
            "wrong unterminated position: {err}"
        );

        assert!(parse_err(r#""\x""#).contains("invalid JSON string escape"));
        assert!(
            parse_err(r#""\uD800""#).contains("high surrogate must be followed by a low surrogate")
        );
        assert!(parse_err(r#""\uDC00""#).contains("unexpected low surrogate"));
        assert!(parse_err("\"\u{0001}\"").contains("unescaped control character"));
    }

    #[test]
    fn large_unicode_strings_and_near_input_limit_still_parse() {
        // ~48 KiB of 3-byte characters inside quotes keeps reproduction under ~64 KiB.
        let chunk = "こんにちは"; // 5 chars * 3 bytes = 15 bytes
        let repeats = (48 * 1024) / chunk.len();
        let body = chunk.repeat(repeats);
        let source = format!("\"{body}\"");
        assert!(
            source.len() < 64 * 1024,
            "perf/repro input must stay under 64 KiB"
        );
        let parsed = expect_string(parse_ok(&source));
        assert_eq!(parsed, body);

        // Near the existing input-size limit: whitespace padding + a small Unicode string.
        let prefix = " ".repeat(MAX_JSON_INPUT_BYTES - 8);
        let near_limit = format!("{prefix}\"你好\"");
        assert_eq!(near_limit.len(), MAX_JSON_INPUT_BYTES);
        let parsed = expect_string(parse_ok(&near_limit));
        assert_eq!(parsed, "你好");

        // Near the input-size limit with string content that is entirely multi-byte
        // Unicode (no whitespace padding), so the string scanner walks ~4 MiB of
        // non-ASCII bytes. The two quotes count toward the limit.
        let chunk = "é你好🚀"; // 2 + 3 + 3 + 4 = 12 bytes
        let budget = MAX_JSON_INPUT_BYTES - 2;
        let mut body = chunk.repeat(budget / chunk.len());
        // Fill the leftover bytes with 2-byte "é" so the input lands on the limit.
        body.push_str(&"é".repeat((budget % chunk.len()) / 2));
        assert!(body.chars().all(|c| !c.is_ascii()));
        let source = format!("\"{body}\"");
        assert!(source.len() <= MAX_JSON_INPUT_BYTES);
        assert_eq!(source.len(), MAX_JSON_INPUT_BYTES);
        let parsed = expect_string(parse_ok(&source));
        assert_eq!(parsed, body);

        assert!(parse_err(&" ".repeat(MAX_JSON_INPUT_BYTES + 1)).contains("JSON input exceeds the"));
    }
}
