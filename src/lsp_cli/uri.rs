use std::path::{Path, PathBuf};

pub(super) fn path(uri: &str) -> Result<PathBuf, String> {
    if uri.len() > 16_384 || uri.contains(['?', '#']) {
        return Err("invalid or oversized file URI".into());
    }
    let raw = uri
        .strip_prefix("file://")
        .ok_or("only local file URIs are supported")?;
    let raw = if raw.starts_with('/') {
        raw.to_owned()
    } else if let Some(local) = raw.strip_prefix("localhost/") {
        format!("/{local}")
    } else {
        return Err("remote file URI authorities are unsupported".into());
    };
    if raw.starts_with("//") {
        return Err("remote file paths are unsupported".into());
    }
    let mut decoded = Vec::new();
    let bytes = raw.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .ok_or("incomplete URI escape")?;
            let digit = |value: u8| match value {
                b'0'..=b'9' => Some(value - b'0'),
                b'a'..=b'f' => Some(value - b'a' + 10),
                b'A'..=b'F' => Some(value - b'A' + 10),
                _ => None,
            };
            decoded.push(
                digit(hex[0]).ok_or("invalid URI escape")? * 16
                    + digit(hex[1]).ok_or("invalid URI escape")?,
            );
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let text = String::from_utf8(decoded).map_err(|_| "file URI is not UTF-8")?;
    if text.contains('\0') || text.len() > 4096 {
        return Err("invalid or oversized source path".into());
    }
    #[cfg(windows)]
    let text = if text.as_bytes().get(2) == Some(&b':') && text.starts_with('/') {
        text[1..].to_owned()
    } else {
        text
    };
    let path = PathBuf::from(text);
    if !path.is_absolute() {
        return Err("file URI must name an absolute path".into());
    }
    Ok(path)
}

pub(super) fn encode(path: &Path) -> Result<String, String> {
    let text = path.to_str().ok_or("source path is not UTF-8")?;
    #[cfg(windows)]
    let text = {
        let text = text
            .strip_prefix("\\\\?\\")
            .unwrap_or(text)
            .replace('\\', "/");
        if text.starts_with("//") {
            return Err("remote source paths are unsupported".into());
        }
        format!("/{text}")
    };
    let mut result = String::from("file://");
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            result.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(result, "%{byte:02X}").unwrap();
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_paths_preserve_unicode_spaces_and_reserved_filename_bytes() {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let name = root.join("雪 🦀 #percent%.ox");
        let uri = encode(&name).unwrap();
        assert!(uri.contains("%23") && uri.contains("%25") && uri.contains("%20"));
        assert_eq!(encode(&path(&uri).unwrap()).unwrap(), uri);
        #[cfg(not(windows))]
        assert_eq!(path(&uri).unwrap(), name);
    }
    #[test]
    fn remote_ambiguous_and_malformed_uris_are_rejected() {
        for uri in [
            "https://example/file.ox",
            "file://server/a.ox",
            "file:////server/a.ox",
            "file:///a%",
            "file:///a%GG",
            "file:///a%00",
            "file:///a?query",
            "file:///a#fragment",
            "file:///%FF",
        ] {
            assert!(path(uri).is_err(), "{uri}");
        }
    }
}
