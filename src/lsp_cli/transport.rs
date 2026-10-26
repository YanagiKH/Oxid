//! Bounded LSP Content-Length framing over byte streams.
use std::io::{self, BufRead, Write};

pub(super) const MAX_FRAME: usize = 8 * 1024 * 1024;
const MAX_HEADERS: usize = 8192;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn line(reader: &mut impl BufRead, remaining: usize) -> io::Result<Option<Vec<u8>>> {
    let mut result = Vec::new();
    loop {
        let bytes = reader.fill_buf()?;
        if bytes.is_empty() {
            return if result.is_empty() {
                Ok(None)
            } else {
                Err(invalid("incomplete LSP header"))
            };
        }
        let count = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |index| index + 1);
        if count > remaining.saturating_sub(result.len()) {
            return Err(invalid("LSP headers exceed byte limit"));
        }
        result
            .try_reserve(count)
            .map_err(|_| invalid("cannot reserve LSP headers"))?;
        result.extend_from_slice(&bytes[..count]);
        let done = result.last() == Some(&b'\n');
        reader.consume(count);
        if done {
            return Ok(Some(result));
        }
    }
}

pub(super) fn read(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut length = None;
    let mut used = 0usize;
    loop {
        let Some(header) = line(reader, MAX_HEADERS - used)? else {
            return if used == 0 {
                Ok(None)
            } else {
                Err(invalid("incomplete LSP headers"))
            };
        };
        used += header.len();
        let header = header
            .strip_suffix(b"\r\n")
            .ok_or_else(|| invalid("LSP headers require CRLF"))?;
        if header.is_empty() {
            break;
        }
        let text = std::str::from_utf8(header).map_err(|_| invalid("LSP headers must be UTF-8"))?;
        let (name, value) = text
            .split_once(':')
            .ok_or_else(|| invalid("invalid LSP header"))?;
        if name.eq_ignore_ascii_case("Content-Length") {
            let value = value.trim();
            if length.is_some()
                || value.is_empty()
                || !value.bytes().all(|byte| byte.is_ascii_digit())
            {
                return Err(invalid("LSP requires one decimal Content-Length"));
            }
            let number = value
                .parse::<usize>()
                .map_err(|_| invalid("invalid LSP Content-Length"))?;
            if number > MAX_FRAME {
                return Err(invalid("LSP frame exceeds byte limit"));
            }
            length = Some(number);
        } else if name.eq_ignore_ascii_case("Content-Type") {
            for part in value.split(';').skip(1) {
                if let Some((key, encoding)) = part.trim().split_once('=') {
                    if key.eq_ignore_ascii_case("charset")
                        && !encoding.eq_ignore_ascii_case("utf-8")
                        && !encoding.eq_ignore_ascii_case("utf8")
                    {
                        return Err(invalid("LSP transport requires UTF-8"));
                    }
                }
            }
        }
    }
    let length = length.ok_or_else(|| invalid("missing LSP Content-Length"))?;
    let mut body = Vec::new();
    body.try_reserve_exact(length)
        .map_err(|_| invalid("cannot reserve LSP frame"))?;
    body.resize(length, 0);
    reader.read_exact(&mut body)?;
    Ok(Some(body))
}

const MAX_REPLY: usize = 32 * 1024 * 1024;
struct Reply(Vec<u8>);
impl Write for Reply {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_REPLY.saturating_sub(self.0.len()) {
            return Err(invalid("LSP response exceeds byte limit"));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| invalid("cannot reserve LSP response"))?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn write(writer: &mut impl Write, value: &serde_json::Value) -> io::Result<()> {
    let mut body = Reply(Vec::new());
    serde_json::to_writer(&mut body, value)
        .map_err(|_| invalid("cannot encode bounded LSP response"))?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.0.len())?;
    writer.write_all(&body.0)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};
    #[test]
    fn multibyte_payloads_and_concatenated_frames_use_byte_lengths() {
        let first = serde_json::json!({"jsonrpc":"2.0","id":"雪🦀","result":null});
        let second = serde_json::json!({"jsonrpc":"2.0","id":2,"result":42});
        let mut bytes = Vec::new();
        write(&mut bytes, &first).unwrap();
        write(&mut bytes, &second).unwrap();
        let mut reader = BufReader::with_capacity(1, Cursor::new(bytes));
        for value in [first, second] {
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&read(&mut reader).unwrap().unwrap())
                    .unwrap(),
                value
            );
        }
        assert!(read(&mut reader).unwrap().is_none());
    }
    #[test]
    fn invalid_lengths_headers_and_truncated_bodies_fail_closed() {
        let cases = [
            b"\r\n".as_slice(), b"Content-Length: -1\r\n\r\n", b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
            b"Content-Length: 8388609\r\n\r\n", b"Content-Length: 2\r\n\r\n{", b"Content-Length: 2\n\n{}",
            b"Content-Length: 2\r\nContent-Type: application/vscode-jsonrpc; charset=utf-16\r\n\r\n{}",
        ];
        for bytes in cases {
            assert!(read(&mut Cursor::new(bytes)).is_err(), "{bytes:?}");
        }
        let header = vec![b'x'; MAX_HEADERS + 1];
        assert!(read(&mut Cursor::new(header)).is_err());
    }
}
