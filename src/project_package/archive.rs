//! The canonical, bounded typed source-package format. This module performs no
//! filesystem operations. Publication, extraction and project admission belong
//! to the package command; an embedded digest is not publisher authentication.

use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};

pub(super) const MAX_FILES: usize = 2047; // Plus the inventory: 2048 tar members.
pub(super) const MAX_INVENTORY_BYTES: usize = 1024 * 1024;
pub(super) const MAX_PAYLOAD_BYTES: u64 = 32 * 1024 * 1024;
pub(super) const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;

const BLOCK: usize = 512;
const MAX_PATH_BYTES: usize = 256;
const INVENTORY_PATH: &str = "OXID-PACKAGE.toml";
const KIND: &str = "typed-source-package-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Member {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Archive {
    /// The project/workspace directory, including the `source` prefix.
    pub root: String,
    pub files: Vec<Member>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Info {
    /// Lowercase SHA-256 of every archive byte, including headers and terminator.
    pub sha256: String,
    pub bytes: u64,
    /// Payload file count; the inventory member is additional.
    pub file_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    path: String,
    size: u64,
    sha256: String,
}

/// Validate the complete plan before writing, then emit deterministic POSIX
/// ustar. Input order is immaterial. The caller owns flushing/syncing/closing the
/// output and must discard any partial output after an I/O failure.
pub(super) fn write(archive: &Archive, output: &mut impl Write) -> io::Result<Info> {
    let prepared = prepare(archive).map_err(|error| invalid_input(error.to_string()))?;
    let (inventory, files) = prepared;
    let mut writer = HashWriter::new(output);
    writer.member(INVENTORY_PATH, inventory.as_bytes())?;
    for file in &files {
        writer.member(&file.path, &file.bytes)?;
    }
    writer.put(&[0; BLOCK * 2])?;
    Ok(writer.finish(files.len()))
}

/// Read only the canonical source-package format, not arbitrary tar. Validate
/// all inventory bounds before reading payloads. No result is returned until all
/// hashes, zero padding, both terminator blocks and true EOF have been checked.
pub(super) fn read(input: &mut impl Read) -> io::Result<(Archive, Info)> {
    let mut reader = HashReader::new(input);
    let mut header = [0; BLOCK];
    reader.fill(&mut header)?;
    let inventory_size = header_size(&header)?;
    if inventory_size > MAX_INVENTORY_BYTES as u64 {
        return Err(invalid_data("package inventory exceeds 1 MiB"));
    }
    check_header(&header, INVENTORY_PATH, inventory_size)?;
    let mut inventory = vec![0; inventory_size as usize];
    reader.fill(&mut inventory)?;
    reader.padding(inventory_size)?;
    let (root, entries) = parse_inventory(&inventory)?;
    let expected_size = wire_size(inventory_size, entries.iter().map(|entry| entry.size))?;
    let mut files = Vec::with_capacity(entries.len());
    for entry in entries {
        reader.fill(&mut header)?;
        check_header(&header, &entry.path, entry.size)?;
        // parse_inventory has checked individual and aggregate payload lengths.
        let mut bytes = vec![0; entry.size as usize];
        reader.fill(&mut bytes)?;
        if digest(&bytes) != entry.sha256 {
            return Err(invalid_data(format!(
                "package file digest mismatch: {}",
                entry.path
            )));
        }
        reader.padding(entry.size)?;
        files.push(Member {
            path: entry.path,
            bytes,
        });
    }
    let mut terminator = [0; BLOCK * 2];
    reader.fill(&mut terminator)?;
    if terminator != [0; BLOCK * 2] {
        return Err(invalid_data(
            "package must end with exactly two zero blocks",
        ));
    }
    reader.eof()?;
    let info = reader.finish(files.len());
    if info.bytes != expected_size {
        return Err(invalid_data("package archive length mismatch"));
    }
    Ok((Archive { root, files }, info))
}

fn prepare(archive: &Archive) -> io::Result<(String, Vec<&Member>)> {
    if archive.files.len() > MAX_FILES {
        return Err(invalid_data("package has more than 2047 payload files"));
    }
    let roster: Vec<_> = archive
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.bytes.len() as u64))
        .collect();
    validate_roster(&archive.root, &roster)?;
    let mut files: Vec<_> = archive.files.iter().collect();
    files.sort_unstable_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let entries: Vec<_> = files
        .iter()
        .map(|file| Entry {
            path: file.path.clone(),
            size: file.bytes.len() as u64,
            sha256: digest(&file.bytes),
        })
        .collect();
    let inventory = inventory_text(&archive.root, &entries)?;
    wire_size(
        inventory.len() as u64,
        entries.iter().map(|entry| entry.size),
    )?;
    Ok((inventory, files))
}

fn validate_roster(root: &str, roster: &[(&str, u64)]) -> io::Result<()> {
    if roster.len() > MAX_FILES {
        return Err(invalid_data("package has more than 2047 payload files"));
    }
    validate_path(root, true)?;
    let mut total = 0_u64;
    for &(path, size) in roster {
        validate_path(path, false)?;
        total = total
            .checked_add(size)
            .filter(|total| *total <= MAX_PAYLOAD_BYTES)
            .ok_or_else(|| invalid_data("package payload exceeds 32 MiB"))?;
    }
    // Folded membership detects file/parent conflicts; adjacent folded paths
    // detect duplicates and inconsistent ASCII directory spelling without a
    // per-component tree. A file and its children need not be adjacent: `a-`
    // sorts between `a` and `a/b`.
    // Qualified extraction is Linux scoped; this is not Unicode normalization
    // or a claim of compatibility with every filesystem's case-folding rules.
    let mut folded: Vec<_> = roster
        .iter()
        .map(|&(path, _)| (path.to_ascii_lowercase(), path))
        .collect();
    folded.sort_unstable();
    for (path, _) in &folded {
        for (index, _) in path.match_indices('/') {
            let parent = &path[..index];
            if folded
                .binary_search_by(|(candidate, _)| candidate.as_str().cmp(parent))
                .is_ok()
            {
                return Err(invalid_data(
                    "package file conflicts with a parent directory",
                ));
            }
        }
    }
    for pair in folded.windows(2) {
        let (left_folded, left) = &pair[0];
        let (right_folded, right) = &pair[1];
        if left_folded == right_folded {
            return Err(invalid_data("duplicate or case-aliased package file"));
        }
        for (left_part, right_part) in left.split('/').zip(right.split('/')) {
            if !left_part.eq_ignore_ascii_case(right_part) {
                break;
            }
            if left_part != right_part {
                return Err(invalid_data("case-aliased package directory"));
            }
        }
    }
    for required in [format!("{root}/oxid.toml"), format!("{root}/oxid.lock")] {
        if !roster.iter().any(|&(path, _)| path == required) {
            return Err(invalid_data(format!(
                "package root is missing required file: {required}"
            )));
        }
    }
    Ok(())
}

fn validate_path(path: &str, root: bool) -> io::Result<()> {
    if path.len() > MAX_PATH_BYTES || !(path.starts_with("source/") || root && path == "source") {
        return Err(invalid_data(
            "package paths must be bounded relative source paths",
        ));
    }
    for part in path.split('/') {
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.ends_with(['.', ' '])
            || part
                .chars()
                .any(|ch| ch.is_control() || "<>:\"\\|?*".contains(ch))
        {
            return Err(invalid_data("package path contains an unsafe component"));
        }
        let stem = part.split('.').next().unwrap_or_default();
        let upper = stem.to_ascii_uppercase();
        if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || upper
                .strip_prefix("COM")
                .or_else(|| upper.strip_prefix("LPT"))
                .is_some_and(|number| {
                    matches!(
                        number,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
        {
            return Err(invalid_data("package path contains a reserved device name"));
        }
    }
    if !root {
        split_ustar_path(path)?;
    }
    Ok(())
}

fn inventory_text(root: &str, entries: &[Entry]) -> io::Result<String> {
    let mut text = format!("version = 1\nkind = \"{KIND}\"\nroot = \"{root}\"\n");
    for entry in entries {
        let record = format!(
            "\n[[files]]\npath = \"{}\"\nsize = {}\nsha256 = \"{}\"\n",
            entry.path, entry.size, entry.sha256
        );
        if text
            .len()
            .checked_add(record.len())
            .is_none_or(|length| length > MAX_INVENTORY_BYTES)
        {
            return Err(invalid_data("package inventory exceeds 1 MiB"));
        }
        text.push_str(&record);
    }
    Ok(text)
}

fn parse_inventory(bytes: &[u8]) -> io::Result<(String, Vec<Entry>)> {
    if bytes.len() > MAX_INVENTORY_BYTES {
        return Err(invalid_data("package inventory exceeds 1 MiB"));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| invalid_data("package inventory is not UTF-8"))?;
    if !text.ends_with('\n') {
        return Err(invalid_data("package inventory lacks its final newline"));
    }
    let mut lines = text.split_terminator('\n');
    expect_line(&mut lines, "version = 1")?;
    expect_line(&mut lines, &format!("kind = \"{KIND}\""))?;
    let root = quoted_field(lines.next(), "root")?;
    validate_path(root, true)?;
    let mut entries = Vec::new();
    let mut total = 0_u64;
    while let Some(line) = lines.next() {
        if !line.is_empty() || entries.len() >= MAX_FILES {
            return Err(invalid_data(
                "invalid or oversized package inventory roster",
            ));
        }
        expect_line(&mut lines, "[[files]]")?;
        let path = quoted_field(lines.next(), "path")?;
        validate_path(path, false)?;
        let size = lines
            .next()
            .and_then(|line| line.strip_prefix("size = "))
            .ok_or_else(|| invalid_data("invalid package inventory size field"))?;
        if size.is_empty()
            || size.len() > 20
            || size.len() > 1 && size.starts_with('0')
            || !size.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(invalid_data("noncanonical package inventory size"));
        }
        let size: u64 = size
            .parse()
            .map_err(|_| invalid_data("package inventory size overflow"))?;
        total = total
            .checked_add(size)
            .filter(|total| *total <= MAX_PAYLOAD_BYTES)
            .ok_or_else(|| invalid_data("package payload exceeds 32 MiB"))?;
        let sha256 = quoted_field(lines.next(), "sha256")?;
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid_data("invalid package inventory SHA-256"));
        }
        if entries
            .last()
            .is_some_and(|previous: &Entry| previous.path.as_bytes() >= path.as_bytes())
        {
            return Err(invalid_data(
                "package inventory files are not strictly sorted",
            ));
        }
        entries.push(Entry {
            path: path.to_owned(),
            size,
            sha256: sha256.to_owned(),
        });
    }
    let roster: Vec<_> = entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.size))
        .collect();
    validate_roster(root, &roster)?;
    Ok((root.to_owned(), entries))
}

fn expect_line<'a>(lines: &mut impl Iterator<Item = &'a str>, expected: &str) -> io::Result<()> {
    if lines.next() != Some(expected) {
        return Err(invalid_data(
            "noncanonical package inventory field or ordering",
        ));
    }
    Ok(())
}

fn quoted_field<'a>(line: Option<&'a str>, key: &str) -> io::Result<&'a str> {
    let prefix = format!("{key} = \"");
    let value = line
        .and_then(|line| line.strip_prefix(&prefix))
        .and_then(|line| line.strip_suffix('"'))
        .ok_or_else(|| invalid_data(format!("invalid package inventory {key} field")))?;
    // Paths deliberately exclude quotes, backslashes and controls. No escapes
    // are needed in this generated TOML subset, including for UTF-8 paths.
    if value.contains(['"', '\\']) || value.chars().any(char::is_control) {
        return Err(invalid_data("noncanonical package inventory string"));
    }
    Ok(value)
}

fn split_ustar_path(path: &str) -> io::Result<(&str, &str)> {
    if path.len() <= 100 {
        return Ok(("", path));
    }
    for (index, _) in path.rmatch_indices('/') {
        let prefix = &path[..index];
        let name = &path[index + 1..];
        if prefix.len() <= 155 && !name.is_empty() && name.len() <= 100 {
            return Ok((prefix, name));
        }
    }
    Err(invalid_data("package path does not fit POSIX ustar fields"))
}

fn header(path: &str, size: u64) -> io::Result<[u8; BLOCK]> {
    let (prefix, name) = split_ustar_path(path)?;
    let mut bytes = [0_u8; BLOCK];
    bytes[..name.len()].copy_from_slice(name.as_bytes());
    bytes[100..108].copy_from_slice(b"0000644\0");
    bytes[108..116].copy_from_slice(b"0000000\0");
    bytes[116..124].copy_from_slice(b"0000000\0");
    octal(&mut bytes[124..136], size)?;
    bytes[136..148].copy_from_slice(b"00000000000\0");
    bytes[148..156].fill(b' ');
    bytes[156] = b'0';
    bytes[257..263].copy_from_slice(b"ustar\0");
    bytes[263..265].copy_from_slice(b"00");
    bytes[329..337].copy_from_slice(b"0000000\0");
    bytes[337..345].copy_from_slice(b"0000000\0");
    bytes[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
    let checksum: u64 = bytes.iter().map(|&byte| u64::from(byte)).sum();
    octal(&mut bytes[148..155], checksum)?;
    Ok(bytes)
}

fn octal(output: &mut [u8], value: u64) -> io::Result<()> {
    let number = format!("{:0width$o}", value, width = output.len() - 1);
    if number.len() != output.len() - 1 {
        return Err(invalid_data("package tar numeric field overflow"));
    }
    output[..number.len()].copy_from_slice(number.as_bytes());
    output[number.len()] = 0;
    Ok(())
}

fn header_size(header: &[u8; BLOCK]) -> io::Result<u64> {
    if header[135] != 0
        || !header[124..135]
            .iter()
            .all(|byte| (b'0'..=b'7').contains(byte))
    {
        return Err(invalid_data("noncanonical package tar size field"));
    }
    let mut size = 0_u64;
    for byte in &header[124..135] {
        size = size
            .checked_mul(8)
            .and_then(|size| size.checked_add(u64::from(byte - b'0')))
            .ok_or_else(|| invalid_data("package tar size overflow"))?;
    }
    Ok(size)
}

fn check_header(actual: &[u8; BLOCK], path: &str, size: u64) -> io::Result<()> {
    if actual != &header(path, size)? {
        return Err(invalid_data(format!(
            "noncanonical or mismatched package tar header: {path}"
        )));
    }
    Ok(())
}

fn padding(size: u64) -> usize {
    ((BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64) as usize
}

fn wire_size(inventory_size: u64, sizes: impl Iterator<Item = u64>) -> io::Result<u64> {
    let mut total = (BLOCK * 2) as u64;
    for size in std::iter::once(inventory_size).chain(sizes) {
        total = total
            .checked_add(BLOCK as u64)
            .and_then(|total| total.checked_add(size))
            .and_then(|total| total.checked_add(padding(size) as u64))
            .filter(|total| *total <= MAX_ARCHIVE_BYTES)
            .ok_or_else(|| invalid_data("package archive exceeds 64 MiB"))?;
    }
    Ok(total)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct HashWriter<'a, W> {
    output: &'a mut W,
    hash: Sha256,
    bytes: u64,
}

impl<'a, W: Write> HashWriter<'a, W> {
    fn new(output: &'a mut W) -> Self {
        Self {
            output,
            hash: Sha256::new(),
            bytes: 0,
        }
    }

    fn put(&mut self, bytes: &[u8]) -> io::Result<()> {
        let next = self
            .bytes
            .checked_add(bytes.len() as u64)
            .filter(|total| *total <= MAX_ARCHIVE_BYTES)
            .ok_or_else(|| invalid_data("package archive exceeds 64 MiB"))?;
        self.output.write_all(bytes)?;
        self.hash.update(bytes);
        self.bytes = next;
        Ok(())
    }

    fn member(&mut self, path: &str, bytes: &[u8]) -> io::Result<()> {
        self.put(&header(path, bytes.len() as u64)?)?;
        self.put(bytes)?;
        self.put(&[0; BLOCK][..padding(bytes.len() as u64)])
    }

    fn finish(self, file_count: usize) -> Info {
        Info {
            sha256: format!("{:x}", self.hash.finalize()),
            bytes: self.bytes,
            file_count,
        }
    }
}

struct HashReader<'a, R> {
    input: &'a mut R,
    hash: Sha256,
    bytes: u64,
}

impl<'a, R: Read> HashReader<'a, R> {
    fn new(input: &'a mut R) -> Self {
        Self {
            input,
            hash: Sha256::new(),
            bytes: 0,
        }
    }

    fn fill(&mut self, bytes: &mut [u8]) -> io::Result<()> {
        let next = self
            .bytes
            .checked_add(bytes.len() as u64)
            .filter(|total| *total <= MAX_ARCHIVE_BYTES)
            .ok_or_else(|| invalid_data("package archive exceeds 64 MiB"))?;
        self.input.read_exact(bytes)?;
        self.hash.update(&*bytes);
        self.bytes = next;
        Ok(())
    }

    fn padding(&mut self, size: u64) -> io::Result<()> {
        let mut bytes = [0; BLOCK];
        self.fill(&mut bytes[..padding(size)])?;
        if bytes != [0; BLOCK] {
            return Err(invalid_data("nonzero package tar padding"));
        }
        Ok(())
    }

    fn eof(&mut self) -> io::Result<()> {
        loop {
            match self.input.read(&mut [0; 1]) {
                Ok(0) => return Ok(()),
                Ok(_) => return Err(invalid_data("trailing bytes after package terminator")),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
    }

    fn finish(self, file_count: usize) -> Info {
        Info {
            sha256: format!("{:x}", self.hash.finalize()),
            bytes: self.bytes,
            file_count,
        }
    }
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture() -> Archive {
        Archive {
            root: "source".into(),
            files: vec![
                Member {
                    path: "source/src/main.ox".into(),
                    bytes: b"fn main() -> i32 { 0 }\n".to_vec(),
                },
                Member {
                    path: "source/oxid.toml".into(),
                    bytes: b"[project]\nedition = \"2026\"\n".to_vec(),
                },
                Member {
                    path: "source/oxid.lock".into(),
                    bytes: vec![0, 1, 2, 255],
                },
            ],
        }
    }

    fn encoded(archive: &Archive) -> Vec<u8> {
        let mut bytes = Vec::new();
        let info = write(archive, &mut bytes).unwrap();
        assert_eq!(info.bytes, bytes.len() as u64);
        assert_eq!(info.sha256, digest(&bytes));
        assert_eq!(info.file_count, archive.files.len());
        bytes
    }

    fn entries(archive: &Archive) -> Vec<Entry> {
        let mut entries: Vec<_> = archive
            .files
            .iter()
            .map(|file| Entry {
                path: file.path.clone(),
                size: file.bytes.len() as u64,
                sha256: digest(&file.bytes),
            })
            .collect();
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        entries
    }

    fn with_inventory(inventory: &[u8], files: &[Member]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut writer = HashWriter::new(&mut bytes);
        writer.member(INVENTORY_PATH, inventory).unwrap();
        for file in files {
            writer.member(&file.path, &file.bytes).unwrap();
        }
        writer.put(&[0; BLOCK * 2]).unwrap();
        bytes
    }

    #[test]
    fn round_trip_is_sorted_deterministic_and_preserves_exact_bytes() {
        let mut archive = fixture();
        archive.files.push(Member {
            path: "source/資料/é.ox".into(),
            bytes: (0..=255).collect(),
        });
        archive.files.push(Member {
            path: "source/empty.ox".into(),
            bytes: vec![],
        });
        let first = encoded(&archive);
        archive.files.reverse();
        assert_eq!(first, encoded(&archive));
        let (decoded, info) = read(&mut first.as_slice()).unwrap();
        archive
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(decoded, archive);
        assert_eq!(info.sha256, digest(&first));
        assert_eq!(info.bytes, first.len() as u64);
        assert_eq!(first, encoded(&decoded));
    }

    #[test]
    fn ustar_prefix_at_exact_byte_boundary_round_trips() {
        let mut archive = fixture();
        let path = format!("source/{}/{}", "a".repeat(148), "b".repeat(100));
        assert_eq!(path.len(), 256);
        archive.files.push(Member {
            path: path.clone(),
            bytes: b"long path".to_vec(),
        });
        let bytes = encoded(&archive);
        let (decoded, _) = read(&mut bytes.as_slice()).unwrap();
        assert!(decoded.files.iter().any(|file| file.path == path));
        assert_eq!(split_ustar_path(&path).unwrap().0.len(), 155);
        assert!(split_ustar_path(&format!("source/{}", "a".repeat(101))).is_err());
    }

    #[test]
    fn nested_root_requires_its_own_manifest_and_lock() {
        let mut archive = fixture();
        archive.root = "source/app".into();
        assert!(write(&archive, &mut Vec::new()).is_err());
        for file in &mut archive.files {
            file.path = file.path.replacen("source/", "source/app/", 1);
        }
        let bytes = encoded(&archive);
        assert_eq!(read(&mut bytes.as_slice()).unwrap().0.root, "source/app");
    }

    #[test]
    fn invalid_paths_are_rejected_before_any_write() {
        for path in [
            "",
            "source",
            "/source/a",
            "elsewhere/a",
            "Source/a",
            "source//a",
            "source/../a",
            "source/./a",
            "source/a/",
            "source/a\\b",
            "source/C:a",
            "source/a\0b",
            "source/a\nb",
            "source/a\u{85}b",
            "source/a.",
            "source/a ",
            "source/CON",
            "source/nul.ox",
            "source/lPt1.bin",
            "source/COM²",
            "source/a?",
            "source/a*",
            "source/a|b",
            "source/a<b",
            "source/a>b",
            "source/a\"b",
        ] {
            let mut archive = fixture();
            archive.files[0].path = path.into();
            let mut output = b"unchanged".to_vec();
            let error = write(&archive, &mut output).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path:?}");
            assert_eq!(output, b"unchanged", "{path:?}");
        }
    }

    #[test]
    fn duplicates_parent_conflicts_and_case_aliases_are_rejected() {
        for paths in [
            ["source/a", "source/a"],
            ["source/a", "source/a/b"],
            ["source/A", "source/a/b"],
            ["source/A/x", "source/a/y"],
            ["source/a/B/x", "source/a/b/y"],
        ] {
            let mut archive = fixture();
            archive.files.extend(paths.into_iter().map(|path| Member {
                path: path.into(),
                bytes: vec![],
            }));
            let mut output = Vec::new();
            assert!(write(&archive, &mut output).is_err(), "{paths:?}");
            assert!(output.is_empty());
        }
        for paths in [
            ["source/a", "source/a-", "source/a/b"],
            ["source/A", "source/a.x", "source/a/b"],
        ] {
            let mut archive = fixture();
            archive.files.extend(paths.into_iter().map(|path| Member {
                path: path.into(),
                bytes: vec![],
            }));
            assert!(write(&archive, &mut Vec::new()).is_err(), "{paths:?}");
        }
        let mut archive = fixture();
        archive
            .files
            .extend(["source/a", "source/ab/b"].map(|path| Member {
                path: path.into(),
                bytes: vec![],
            }));
        encoded(&archive);
    }

    #[test]
    fn count_payload_and_arithmetic_bounds_fail_before_output() {
        let mut archive = fixture();
        archive
            .files
            .resize(MAX_FILES + 1, archive.files[0].clone());
        let mut output = Vec::new();
        assert!(write(&archive, &mut output).is_err());
        assert!(output.is_empty());
        for sizes in [
            [MAX_PAYLOAD_BYTES + 1, 0],
            [MAX_PAYLOAD_BYTES, 1],
            [u64::MAX, 1],
        ] {
            let roster = [
                ("source/oxid.toml", sizes[0]),
                ("source/oxid.lock", sizes[1]),
            ];
            assert!(validate_roster("source", &roster).is_err());
        }
        assert!(wire_size(0, [u64::MAX].into_iter()).is_err());
        assert!(wire_size(0, [MAX_ARCHIVE_BYTES].into_iter()).is_err());
        assert!(header(INVENTORY_PATH, u64::MAX).is_err());
    }

    #[test]
    fn inventory_fields_order_and_numeric_grammar_are_exact() {
        let archive = fixture();
        let text = inventory_text(&archive.root, &entries(&archive)).unwrap();
        for changed in [
            text.replace("version = 1", "version = 2"),
            text.replace("version = 1", "version = 1\nversion = 1"),
            text.replace(KIND, "other-format"),
            text.replace("root = \"source\"", "root = \"../source\""),
            text.replace("[[files]]", "[[unknown]]"),
            text.replace("size = 4", "size = 04"),
            text.replace("size = 4", "size = +4"),
            text.replace("size = 4", "size = -4"),
            text.replace("size = 4", "size = 4_0"),
            text.replace("size = 4", "size = 18446744073709551616"),
            text.replace("sha256 = \"", "sha256 = \"G"),
            text.replace('\n', "\r\n"),
            text.trim_end().to_owned(),
            format!("{text}\n"),
            format!("{text}unknown = true\n"),
        ] {
            assert!(parse_inventory(changed.as_bytes()).is_err(), "{changed}");
        }
        let mut unsorted = entries(&archive);
        unsorted.reverse();
        assert!(parse_inventory(inventory_text("source", &unsorted).unwrap().as_bytes()).is_err());
        let mut duplicate = entries(&archive);
        duplicate.insert(0, duplicate[0].clone());
        assert!(parse_inventory(inventory_text("source", &duplicate).unwrap().as_bytes()).is_err());
        assert!(parse_inventory(&[255, b'\n']).is_err());
    }

    struct ReadBudget<'a> {
        bytes: &'a [u8],
        consumed: usize,
        limit: usize,
    }

    impl Read for ReadBudget<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            assert!(
                self.consumed + output.len() <= self.limit,
                "read beyond validated bound"
            );
            let size = self.bytes.read(output)?;
            self.consumed += size;
            Ok(size)
        }
    }

    #[test]
    fn oversized_inventory_is_refused_without_reading_its_body() {
        for size in [MAX_INVENTORY_BYTES as u64 + 1, 0o77777777777] {
            let bytes = header(INVENTORY_PATH, size).unwrap();
            let mut reader = ReadBudget {
                bytes: &bytes,
                consumed: 0,
                limit: BLOCK,
            };
            assert!(read(&mut reader).is_err());
            assert_eq!(reader.consumed, BLOCK);
        }
    }

    #[test]
    fn oversized_payload_is_refused_before_reading_any_payload_header() {
        let archive = fixture();
        let mut records = entries(&archive);
        records[0].size = MAX_PAYLOAD_BYTES + 1;
        let inventory = inventory_text("source", &records).unwrap();
        let bytes = with_inventory(inventory.as_bytes(), &[]);
        let limit = BLOCK + inventory.len() + padding(inventory.len() as u64);
        let mut reader = ReadBudget {
            bytes: &bytes,
            consumed: 0,
            limit,
        };
        assert!(read(&mut reader).is_err());
        assert_eq!(reader.consumed, limit);
    }

    #[test]
    fn inventory_count_limit_is_checked_during_parsing() {
        let records: Vec<_> = (0..=MAX_FILES)
            .map(|index| Entry {
                path: format!("source/{index:04}.ox"),
                size: 0,
                sha256: digest(&[]),
            })
            .collect();
        let text = inventory_text("source", &records).unwrap();
        let error = parse_inventory(text.as_bytes()).unwrap_err();
        assert!(error.to_string().contains("oversized"));
    }

    #[test]
    fn noncanonical_headers_and_special_member_types_are_rejected() {
        let original = encoded(&fixture());
        for index in [
            0, 100, 108, 116, 124, 135, 136, 148, 155, 157, 257, 263, 265, 297, 329, 337, 345, 500,
        ] {
            let mut bytes = original.clone();
            bytes[index] ^= 1;
            assert!(read(&mut bytes.as_slice()).is_err(), "header byte {index}");
        }
        for kind in [
            0, b'1', b'2', b'3', b'4', b'5', b'6', b'x', b'g', b'L', b'S',
        ] {
            let mut bytes = original.clone();
            bytes[156] = kind;
            // Even a correctly checksummed unsupported type must fail.
            bytes[148..156].fill(b' ');
            let sum = bytes[..BLOCK].iter().map(|&byte| u64::from(byte)).sum();
            octal(&mut bytes[148..155], sum).unwrap();
            assert!(read(&mut bytes.as_slice()).is_err(), "type {kind}");
        }
    }

    #[test]
    fn inventory_payload_mismatch_is_rejected() {
        let mut archive = fixture();
        archive
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        let inventory = inventory_text("source", &entries(&archive)).unwrap();
        for action in 0..5 {
            let mut files = archive.files.clone();
            match action {
                0 => files[0].bytes[0] ^= 1,
                1 => files[0].bytes.push(0),
                2 => {
                    files.remove(0);
                }
                3 => files.swap(0, 1),
                4 => files.push(files[0].clone()),
                _ => unreachable!(),
            }
            let bytes = with_inventory(inventory.as_bytes(), &files);
            assert!(read(&mut bytes.as_slice()).is_err(), "mismatch {action}");
        }
    }

    #[test]
    fn missing_root_files_and_conflicting_inventory_paths_are_rejected() {
        let archive = fixture();
        let mut records = entries(&archive);
        records.retain(|entry| entry.path != "source/oxid.lock");
        let bytes = with_inventory(inventory_text("source", &records).unwrap().as_bytes(), &[]);
        assert!(read(&mut bytes.as_slice()).is_err());
        for path in [
            "source/..",
            "source/oxid.lock/child",
            "source/OXID.lock",
            "source/CON",
        ] {
            let mut records = entries(&archive);
            records.push(Entry {
                path: path.into(),
                size: 0,
                sha256: digest(&[]),
            });
            records.sort_by(|left, right| left.path.cmp(&right.path));
            let bytes = with_inventory(inventory_text("source", &records).unwrap().as_bytes(), &[]);
            assert!(read(&mut bytes.as_slice()).is_err(), "{path}");
        }
    }

    #[test]
    fn nonzero_padding_truncation_and_any_trailing_bytes_are_rejected() {
        let archive = fixture();
        let original = encoded(&archive);
        let inventory_len = header_size(original[..BLOCK].try_into().unwrap()).unwrap() as usize;
        let mut bytes = original.clone();
        bytes[BLOCK + inventory_len] = 1;
        assert!(read(&mut bytes.as_slice()).is_err());
        let first_payload = BLOCK + inventory_len + padding(inventory_len as u64) + BLOCK;
        let mut bytes = original.clone();
        bytes[first_payload + 4] = 1;
        assert!(read(&mut bytes.as_slice()).is_err());
        for length in [
            0,
            1,
            BLOCK - 1,
            BLOCK,
            BLOCK + 1,
            original.len() - 1024,
            original.len() - 1,
        ] {
            assert!(read(&mut &original[..length]).is_err(), "length {length}");
        }
        for suffix in [vec![0], vec![1], vec![0; BLOCK], original.clone()] {
            let mut bytes = original.clone();
            bytes.extend(suffix);
            assert!(read(&mut bytes.as_slice()).is_err());
        }
        for index in original.len() - BLOCK * 2..original.len() {
            let mut bytes = original.clone();
            bytes[index] = 1;
            assert!(
                read(&mut bytes.as_slice()).is_err(),
                "terminator byte {index}"
            );
        }
    }

    struct ShortIo<T> {
        inner: T,
        calls: usize,
    }

    impl<T: Read> Read for ShortIo<T> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.calls % 3 == 1 {
                return Err(io::ErrorKind::Interrupted.into());
            }
            let length = bytes.len().min(7);
            self.inner.read(&mut bytes[..length])
        }
    }

    impl<T: Write> Write for ShortIo<T> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.calls % 3 == 1 {
                return Err(io::ErrorKind::Interrupted.into());
            }
            self.inner.write(&bytes[..bytes.len().min(7)])
        }

        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }

    #[test]
    fn short_io_and_interruptions_preserve_the_archive_and_whole_digest() {
        let archive = fixture();
        let expected = encoded(&archive);
        let mut writer = ShortIo {
            inner: Vec::new(),
            calls: 0,
        };
        let written = write(&archive, &mut writer).unwrap();
        assert_eq!(writer.inner, expected);
        let mut reader = ShortIo {
            inner: Cursor::new(&expected),
            calls: 0,
        };
        let (decoded, info) = read(&mut reader).unwrap();
        assert_eq!(info, written);
        assert_eq!(encoded(&decoded), expected);
    }

    struct FailingIo {
        remaining: usize,
    }

    impl Write for FailingIo {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected write failure",
                ));
            }
            let size = bytes.len().min(self.remaining);
            self.remaining -= size;
            Ok(size)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Read for FailingIo {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected read failure",
            ))
        }
    }

    #[test]
    fn underlying_io_failures_are_not_reported_as_success() {
        for remaining in [0, BLOCK - 1, BLOCK + 2, 1025] {
            let error = write(&fixture(), &mut FailingIo { remaining }).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        }
        let error = read(&mut FailingIo { remaining: 0 }).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        let mut empty_output = &mut [][..];
        assert_eq!(
            write(&fixture(), &mut empty_output).unwrap_err().kind(),
            io::ErrorKind::WriteZero
        );
    }

    #[test]
    fn sampled_single_byte_corruption_is_never_accepted() {
        let original = encoded(&fixture());
        for index in (0..original.len()).step_by(17) {
            let mut bytes = original.clone();
            bytes[index] ^= 1;
            assert!(
                read(&mut bytes.as_slice()).is_err(),
                "corruption byte {index}"
            );
        }
    }
}
