//! Closed test transport. No parser, source map, selector, or compiler call.
//! Logical Read consumption excludes the separately qualified stdin read-ahead.
use super::SourceInput;
use std::{env::VarError, io::Read};

const MAGIC: &[u8; 8] = b"OXABY001";
const HEADER_BYTES: usize = 24;
const MAX_TEXT_BYTES: usize = 1_048_576;
const MAX_PATH_BYTES: usize = 4_194_304;
const MAX_PAYLOAD_BYTES: usize = MAX_TEXT_BYTES + MAX_PATH_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InputKind {
    Root,
    Bytes,
}
impl InputKind {
    pub(super) fn from_environment(value: Result<String, VarError>) -> Result<Self, Error> {
        match value {
            Err(VarError::NotPresent) => Ok(Self::Root),
            Ok(value) if value == "root" => Ok(Self::Root),
            Ok(value) if value == "bytes" => Ok(Self::Bytes),
            Err(VarError::NotUnicode(_)) => Err(Error::NonUnicodeKind),
            Ok(_) => Err(Error::UnknownKind),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    UnknownKind,
    NonUnicodeKind,
    HeaderRead,
    Magic,
    LengthConversion,
    LengthOverflow,
    TextLimit,
    PathLimit,
    PayloadLimit,
    PayloadReservation,
    PayloadRead,
    PathUtf8,
    TextUtf8,
    TrailingRead,
    TrailingByte,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FramePlan {
    path: usize,
    payload: usize,
}
impl FramePlan {
    fn from_header(header: &[u8; HEADER_BYTES]) -> Result<Self, Error> {
        if &header[..8] != MAGIC {
            return Err(Error::Magic);
        }
        let path = u64::from_le_bytes(header[8..16].try_into().expect("fixed path length"));
        let text = u64::from_le_bytes(header[16..24].try_into().expect("fixed text length"));
        let path = usize::try_from(path).map_err(|_| Error::LengthConversion)?;
        let text = usize::try_from(text).map_err(|_| Error::LengthConversion)?;
        let payload = path.checked_add(text).ok_or(Error::LengthOverflow)?;
        // Transport maxima are independent of any lower compiler request.
        if text > MAX_TEXT_BYTES {
            return Err(Error::TextLimit);
        }
        if path > MAX_PATH_BYTES {
            return Err(Error::PathLimit);
        }
        if payload > MAX_PAYLOAD_BYTES {
            return Err(Error::PayloadLimit);
        }
        Ok(Self { path, payload })
    }
}

pub(super) struct BytesFrame {
    // String::from_utf8 takes the single Vec allocation without copying it.
    payload: String,
    path: usize,
}
impl BytesFrame {
    pub(super) fn input(&self) -> SourceInput<'_> {
        let (path, text) = self.payload.split_at(self.path);
        SourceInput::Bytes { path, text }
    }
}

pub(super) fn read_bytes_frame(reader: &mut impl Read) -> Result<BytesFrame, Error> {
    let mut header = [0; HEADER_BYTES];
    reader
        .read_exact(&mut header)
        .map_err(|_| Error::HeaderRead)?;
    let plan = FramePlan::from_header(&header)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(plan.payload)
        .map_err(|_| Error::PayloadReservation)?;
    payload.resize(plan.payload, 0);
    reader
        .read_exact(&mut payload)
        .map_err(|_| Error::PayloadRead)?;
    let payload = String::from_utf8(payload).map_err(|error| {
        let valid = error.utf8_error().valid_up_to();
        // A later text error can follow a character spanning the field split.
        // Inside the valid prefix, only a continuation byte is not a boundary.
        let split_character = plan.path < valid && error.as_bytes()[plan.path] & 0xc0 == 0x80;
        if valid < plan.path || split_character {
            Error::PathUtf8
        } else {
            Error::TextUtf8
        }
    })?;
    // A valid combined string must not join partial code points across fields.
    if !payload.is_char_boundary(plan.path) {
        return Err(Error::PathUtf8);
    }
    let mut trailing = [0];
    if reader
        .read(&mut trailing)
        .map_err(|_| Error::TrailingRead)?
        != 0
    {
        return Err(Error::TrailingByte);
    }
    Ok(BytesFrame {
        payload,
        path: plan.path,
    })
}

#[cfg(test)]
mod controls {
    use super::*;
    use std::io::{self, Cursor};

    fn header(path: u64, text: u64) -> [u8; HEADER_BYTES] {
        let mut bytes = [0; HEADER_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..16].copy_from_slice(&path.to_le_bytes());
        bytes[16..24].copy_from_slice(&text.to_le_bytes());
        bytes
    }
    fn frame(path: &[u8], text: &[u8]) -> Vec<u8> {
        assert!(path.len() + text.len() <= 128, "finite control input");
        let mut bytes = header(path.len() as u64, text.len() as u64).to_vec();
        bytes.extend_from_slice(path);
        bytes.extend_from_slice(text);
        bytes
    }
    fn error(bytes: &[u8]) -> Error {
        match read_bytes_frame(&mut Cursor::new(bytes)) {
            Ok(_) => panic!("expected transport rejection"),
            Err(error) => error,
        }
    }

    #[test]
    fn unit3b2_transport_kind_is_closed_and_root_is_default() {
        assert_eq!(
            InputKind::from_environment(Err(VarError::NotPresent)),
            Ok(InputKind::Root)
        );
        assert_eq!(
            InputKind::from_environment(Ok("root".into())),
            Ok(InputKind::Root)
        );
        assert_eq!(
            InputKind::from_environment(Ok("bytes".into())),
            Ok(InputKind::Bytes)
        );
        for text in ["", "Root", "Bytes", "bytes ", "file"] {
            assert_eq!(
                InputKind::from_environment(Ok(text.into())),
                Err(Error::UnknownKind)
            );
        }
        assert_eq!(
            InputKind::from_environment(Err(VarError::NotUnicode("fixed control".into()))),
            Err(Error::NonUnicodeKind)
        );
    }

    #[test]
    fn unit3b2_transport_preserves_utf8_nul_and_empty_fields() {
        for (path, text) in [
            ("", ""),
            ("main.ox", "plain text"),
            ("a\0é\r\n", "\0🦀\r\n"),
        ] {
            let input = read_bytes_frame(&mut Cursor::new(frame(path.as_bytes(), text.as_bytes())))
                .unwrap();
            let SourceInput::Bytes {
                path: actual_path,
                text: actual_text,
            } = input.input()
            else {
                panic!("Bytes cannot fall back to Root")
            };
            assert_eq!((actual_path, actual_text), (path, text));
            assert_eq!(actual_path.as_ptr(), input.payload.as_ptr());
            assert_eq!(
                actual_text.as_ptr(),
                input.payload.as_bytes()[path.len()..].as_ptr()
            );
        }
    }

    #[test]
    fn unit3b2_transport_checks_header_limits_before_payload() {
        assert_eq!(
            FramePlan::from_header(&header(MAX_PATH_BYTES as u64, MAX_TEXT_BYTES as u64)),
            Ok(FramePlan {
                path: MAX_PATH_BYTES,
                payload: MAX_PAYLOAD_BYTES
            })
        );
        assert_eq!(
            FramePlan::from_header(&header(0, MAX_TEXT_BYTES as u64 + 1)),
            Err(Error::TextLimit)
        );
        assert_eq!(
            FramePlan::from_header(&header(MAX_PATH_BYTES as u64 + 1, 0)),
            Err(Error::PathLimit)
        );
        let overflow = FramePlan::from_header(&header(u64::MAX, u64::MAX));
        assert!(matches!(
            overflow,
            Err(Error::LengthOverflow | Error::LengthConversion)
        ));
        let mut bytes = header(0, MAX_TEXT_BYTES as u64 + 1);
        let mut reader = Probe::new(&bytes);
        assert!(matches!(
            read_bytes_frame(&mut reader),
            Err(Error::TextLimit)
        ));
        assert_eq!(reader.position, HEADER_BYTES);
        assert_eq!(&reader.requests[..reader.calls], &[HEADER_BYTES]);
        bytes[0] = b'?';
        assert_eq!(error(&bytes), Error::Magic);
        let lower = super::super::ProjectLimits {
            source_bytes: 0,
            path_bytes: 0,
            modules: 0,
            ..Default::default()
        };
        assert!(super::super::admit_bytes_input(1, 1, lower).is_err());
        assert!(read_bytes_frame(&mut Cursor::new(frame(b"a", b"b"))).is_ok());
        assert_eq!(
            MAX_TEXT_BYTES,
            super::super::ProjectLimits::default().source_bytes
        );
        assert_eq!(
            MAX_PATH_BYTES,
            super::super::ProjectLimits::default().path_bytes
        );
    }

    #[test]
    fn unit3b2_transport_rejects_truncation_utf8_and_trailing_bytes() {
        let bytes = frame(b"a", b"b");
        for end in 0..HEADER_BYTES {
            assert_eq!(error(&bytes[..end]), Error::HeaderRead);
        }
        for end in HEADER_BYTES..bytes.len() {
            assert_eq!(error(&bytes[..end]), Error::PayloadRead);
        }
        assert_eq!(error(&frame(&[0xff], b"x")), Error::PathUtf8);
        assert_eq!(error(&frame(b"x", &[0xff])), Error::TextUtf8);
        assert_eq!(error(&frame(&[0xc3], &[0xa9])), Error::PathUtf8);
        assert_eq!(error(&frame(&[0xc3], &[0xa9, 0xff])), Error::PathUtf8);
        assert_eq!(error(&frame(b"x", &[0x80])), Error::TextUtf8);
        let mut trailing = bytes;
        trailing.extend_from_slice(b"tail that must not be logically drained");
        let mut reader = Probe::new(&trailing);
        assert!(matches!(
            read_bytes_frame(&mut reader),
            Err(Error::TrailingByte)
        ));
        assert_eq!(reader.position, HEADER_BYTES + 2 + 1);
        assert_eq!(&reader.requests[..reader.calls], &[HEADER_BYTES, 2, 1]);
    }

    struct Probe<'a> {
        bytes: &'a [u8],
        position: usize,
        requests: [usize; 32],
        calls: usize,
        chunk: usize,
        fail_at: Option<usize>,
    }
    impl<'a> Probe<'a> {
        fn new(bytes: &'a [u8]) -> Self {
            Self {
                bytes,
                position: 0,
                requests: [0; 32],
                calls: 0,
                chunk: usize::MAX,
                fail_at: None,
            }
        }
    }
    impl Read for Probe<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            assert!(
                self.calls < self.requests.len(),
                "finite logical-reader trace"
            );
            self.requests[self.calls] = output.len();
            self.calls += 1;
            if self.fail_at == Some(self.position) {
                return Err(io::Error::other("fixed read failure"));
            }
            let count = output
                .len()
                .min(self.chunk)
                .min(self.bytes.len() - self.position);
            output[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
            self.position += count;
            Ok(count)
        }
    }

    #[test]
    fn unit3b2_transport_handles_short_reads_and_read_errors() {
        let bytes = frame(b"main.ox", b"input");
        let mut reader = Probe::new(&bytes);
        reader.chunk = 3;
        assert!(read_bytes_frame(&mut reader).is_ok());
        assert_eq!(reader.position, bytes.len());
        for (position, expected) in [
            (0, Error::HeaderRead),
            (HEADER_BYTES, Error::PayloadRead),
            (bytes.len(), Error::TrailingRead),
        ] {
            let mut reader = Probe::new(&bytes);
            reader.fail_at = Some(position);
            assert!(matches!(read_bytes_frame(&mut reader), Err(error) if error == expected));
        }
    }

    #[test]
    fn unit3b2_transport_layout_without_source_invocation() {
        println!("TRANSPORT_LAYOUT {{\"BytesFrame\":{},\"FramePlan\":{},\"Error\":{},\"InputKind\":{},\"Stdin\":{},\"StdinLock\":{},\"header_bytes\":{},\"sentinel_bytes\":1,\"max_text_bytes\":{},\"max_path_bytes\":{},\"max_payload_bytes\":{}}}",
            size_of::<BytesFrame>(), size_of::<FramePlan>(), size_of::<Error>(), size_of::<InputKind>(), size_of::<std::io::Stdin>(), size_of::<std::io::StdinLock<'static>>(), HEADER_BYTES, MAX_TEXT_BYTES, MAX_PATH_BYTES, MAX_PAYLOAD_BYTES);
    }
}
