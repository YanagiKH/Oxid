use serde_json::Value;
use std::{collections::BTreeMap, path::PathBuf};

pub(super) const MAX_DOCUMENT_BYTES: usize = 1_048_576;
const MAX_TOTAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENTS: usize = 256;
const MAX_EDITS: usize = 256;

pub(super) struct Document {
    pub uri: String,
    pub path: PathBuf,
    pub version: i32,
    pub text: String,
    pub synchronized: bool,
}

#[derive(Default)]
pub(super) struct Documents {
    pub files: BTreeMap<PathBuf, Document>,
    bytes: usize,
}

pub(super) fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be a string"))
}
fn version(value: &Value) -> Result<i32, String> {
    value
        .get("version")
        .and_then(Value::as_i64)
        .and_then(|number| i32::try_from(number).ok())
        .ok_or_else(|| "document version must be a 32-bit integer".into())
}

impl Documents {
    pub fn find(&self, uri: &str) -> Option<&Document> {
        self.files.values().find(|document| document.uri == uri)
    }
    pub fn open(&mut self, value: &Value) -> Result<(), String> {
        let uri = string(value, "uri")?;
        if string(value, "languageId")? != "oxid" {
            return Err("the language server accepts Oxid documents".into());
        }
        let path = super::uri::path(uri)?
            .canonicalize()
            .map_err(|error| format!("cannot open source: {error}"))?;
        if path.to_str().is_none_or(|path| path.len() > 4096) {
            return Err("source requires a bounded UTF-8 path".into());
        }
        if !path.is_file() || path.extension().is_none_or(|extension| extension != "ox") {
            return Err("open an ordinary .ox source file".into());
        }
        if self.files.contains_key(&path) {
            return Err("source identity is already open".into());
        }
        let text = string(value, "text")?;
        let version = version(value)?;
        if self.files.len() >= MAX_DOCUMENTS
            || text.len() > MAX_DOCUMENT_BYTES
            || text.len() > MAX_TOTAL_BYTES.saturating_sub(self.bytes)
        {
            return Err("open source buffers exceed editor limits".into());
        }
        self.bytes += text.len();
        self.files.insert(
            path.clone(),
            Document {
                uri: uri.into(),
                path,
                version,
                text: text.into(),
                synchronized: true,
            },
        );
        Ok(())
    }
    pub fn close(&mut self, uri: &str) {
        let path = self.find(uri).map(|document| document.path.clone());
        if let Some(path) = path {
            if let Some(document) = self.files.remove(&path) {
                self.bytes -= document.text.len();
            }
        }
    }
    pub fn change(&mut self, identifier: &Value, changes: &Value) -> Result<bool, String> {
        let uri = string(identifier, "uri")?;
        let document = self
            .files
            .values_mut()
            .find(|document| document.uri == uri)
            .ok_or("source is not open")?;
        let version = version(identifier).inspect_err(|_| document.synchronized = false)?;
        if version <= document.version {
            return Ok(false);
        }
        let parsed = changes
            .as_array()
            .filter(|changes| changes.len() <= MAX_EDITS && !changes.is_empty())
            .ok_or("invalid document changes");
        let updated = parsed.and_then(|changes| {
            if !document.synchronized
                && changes
                    .first()
                    .is_some_and(|change| change.get("range").is_some_and(|range| !range.is_null()))
            {
                return Err("a full document update is required to resynchronize");
            }
            let mut text = document.text.clone();
            for change in changes {
                let replacement = change
                    .get("text")
                    .and_then(Value::as_str)
                    .ok_or("change text must be a string")?;
                if let Some(range) = change.get("range").filter(|range| !range.is_null()) {
                    let begin = position(&text, range.get("start").ok_or("missing range start")?)?;
                    let end = position(&text, range.get("end").ok_or("missing range end")?)?;
                    if end < begin {
                        return Err("document range is reversed");
                    }
                    if let Some(length) =
                        change.get("rangeLength").filter(|length| !length.is_null())
                    {
                        if length.as_u64() != Some(text[begin..end].encode_utf16().count() as u64) {
                            return Err("document range length differs from its UTF-16 text");
                        }
                    }
                    let next = text.len() - (end - begin);
                    if replacement.len() > MAX_DOCUMENT_BYTES.saturating_sub(next) {
                        return Err("source exceeds editor byte limit");
                    }
                    text.try_reserve(replacement.len().saturating_sub(end - begin))
                        .map_err(|_| "cannot reserve source update")?;
                    text.replace_range(begin..end, replacement);
                } else {
                    if replacement.len() > MAX_DOCUMENT_BYTES {
                        return Err("source exceeds editor byte limit");
                    }
                    text.clear();
                    text.try_reserve(replacement.len())
                        .map_err(|_| "cannot reserve source update")?;
                    text.push_str(replacement);
                }
            }
            let without = self.bytes - document.text.len();
            if text.len() > MAX_TOTAL_BYTES.saturating_sub(without) {
                return Err("source buffers exceed editor byte limit");
            }
            Ok(text)
        });
        document.version = version;
        match updated {
            Ok(text) => {
                self.bytes = self.bytes - document.text.len() + text.len();
                document.text = text;
                document.synchronized = true;
                Ok(true)
            }
            Err(error) => {
                document.synchronized = false;
                Err(error.into())
            }
        }
    }
    pub fn save(&mut self, uri: &str, text: Option<&str>) -> Result<(), String> {
        let Some(document) = self.files.values_mut().find(|document| document.uri == uri) else {
            return Ok(());
        };
        if let Some(text) = text {
            let without = self.bytes - document.text.len();
            if text.len() > MAX_DOCUMENT_BYTES
                || text.len() > MAX_TOTAL_BYTES.saturating_sub(without)
            {
                document.synchronized = false;
                return Err("saved source exceeds editor byte limit".into());
            }
            self.bytes = without + text.len();
            document.text = text.into();
            document.synchronized = true;
        }
        Ok(())
    }
}

/// Position values beyond a line clamp as required by LSP. An index inside a
/// UTF-16 surrogate pair has no UTF-8 source boundary and is rejected.
pub(super) fn position(text: &str, value: &Value) -> Result<usize, &'static str> {
    let line = value
        .get("line")
        .and_then(Value::as_u64)
        .filter(|value| *value <= i32::MAX as u64)
        .ok_or("invalid document line")?;
    let character = value
        .get("character")
        .and_then(Value::as_u64)
        .filter(|value| *value <= i32::MAX as u64)
        .ok_or("invalid document character")?;
    let mut start = 0;
    for _ in 0..line {
        let Some(next) = text[start..].find(['\r', '\n']) else {
            return Ok(text.len());
        };
        start += next;
        start += if text.as_bytes()[start..].starts_with(b"\r\n") {
            2
        } else {
            1
        };
    }
    let end = text[start..]
        .find(['\r', '\n'])
        .map_or(text.len(), |end| start + end);
    let mut units = 0u64;
    for (index, ch) in text[start..end].char_indices() {
        if units == character {
            return Ok(start + index);
        }
        units += ch.len_utf16() as u64;
        if units > character {
            return Err("document position splits a surrogate pair");
        }
    }
    Ok(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn utf16_ranges_support_crlf_carriage_returns_and_boundary_clamping() {
        let text = "a🦀b\r\n雪\rc";
        assert_eq!(position(text, &json!({"line":0,"character":3})), Ok(5));
        assert!(position(text, &json!({"line":0,"character":2})).is_err());
        assert_eq!(position(text, &json!({"line":0,"character":99})), Ok(6));
        assert_eq!(position(text, &json!({"line":1,"character":1})), Ok(11));
        assert_eq!(position(text, &json!({"line":2,"character":0})), Ok(12));
        assert_eq!(
            position(text, &json!({"line":99,"character":0})),
            Ok(text.len())
        );
    }
    #[test]
    fn batched_edits_are_atomic_versions_are_monotone_and_invalid_ranges_require_resync() {
        let path = std::env::temp_dir().join("document-model-only.ox");
        let uri = super::super::uri::encode(&path).unwrap();
        let mut documents = Documents::default();
        documents.files.insert(
            path.clone(),
            Document {
                uri: uri.clone(),
                path,
                version: 1,
                text: "a🦀b".into(),
                synchronized: true,
            },
        );
        documents.bytes = 6;
        let id = json!({"uri":uri,"version":2});
        let changes = json!([{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":1}},"text":"z"},{"range":{"start":{"line":0,"character":2},"end":{"line":0,"character":3}},"text":"!"}]);
        assert!(documents.change(&id, &changes).is_err());
        assert_eq!(documents.find(&uri).unwrap().text, "a🦀b");
        assert!(!documents.find(&uri).unwrap().synchronized);
        assert!(!documents
            .change(&json!({"uri":uri,"version":1}), &json!([{"text":"stale"}]))
            .unwrap());
        assert!(documents
            .change(
                &json!({"uri":uri,"version":3}),
                &json!([{"text":"recovered"}])
            )
            .unwrap());
        assert!(documents.find(&uri).unwrap().synchronized);
        assert_eq!(documents.find(&uri).unwrap().text, "recovered");
    }
}
