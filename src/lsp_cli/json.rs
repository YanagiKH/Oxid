//! JSON-RPC values with bounded node count and unambiguous object keys.
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::{cell::Cell, fmt};

const MAX_NODES: usize = 65_536;

struct Seed<'a>(&'a Cell<usize>);
struct Node<'a>(&'a Cell<usize>);

impl<'de> DeserializeSeed<'de> for Seed<'_> {
    type Value = Value;
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        let remaining = self
            .0
            .get()
            .checked_sub(1)
            .ok_or_else(|| de::Error::custom("JSON-RPC value count exceeded"))?;
        self.0.set(remaining);
        deserializer.deserialize_any(Node(self.0))
    }
}

impl<'de> Visitor<'de> for Node<'_> {
    type Value = Value;
    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("a JSON value")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("JSON-RPC number must be finite"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.into()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(Seed(self.0))? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON-RPC object key"));
            }
            let value = map.next_value_seed(Seed(self.0))?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    let budget = Cell::new(MAX_NODES);
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    let value = Seed(&budget).deserialize(&mut parser)?;
    parser.end()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_integer_ids_and_unicode_surrogate_pairs_survive_parsing() {
        let value =
            parse(br#"{"id":9223372036854775807,"method":"x","params":{"text":"\ud83e\udd80"}}"#)
                .unwrap();
        assert_eq!(value["id"].as_i64(), Some(i64::MAX));
        assert_eq!(value["params"]["text"], "🦀");
    }
    #[test]
    fn duplicate_keys_trailing_documents_and_bad_unicode_are_rejected() {
        for bytes in [
            br#"{"id":1,"id":2}"#.as_slice(),
            br#"{"params":{"a":1,"\u0061":2}}"#,
            br#"{} {}"#,
            br#"{"text":"\ud83e"}"#,
        ] {
            assert!(parse(bytes).is_err());
        }
    }
    #[test]
    fn many_small_values_are_bounded_independently_of_frame_bytes() {
        let text = format!("[{}]", vec!["0"; MAX_NODES].join(","));
        assert!(parse(text.as_bytes()).is_err());
    }
}
