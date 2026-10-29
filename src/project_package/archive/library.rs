//! Separate immutable-library policy. The deployment-bundle reader/writer and
//! its lock requirement remain unchanged. Only the canonical tar primitives
//! are shared; all new dynamic reservations use the registry request ledger.
use super::*;
use crate::project_registry::Budget;
use std::fmt::Write as _;

const HEADER: &str = "version = 1\nkind = \"typed-library-source-v1\"\nroot = \"source\"\n";

struct BorrowedEntry<'a> {
    path: &'a str,
    size: usize,
    sha256: &'a str,
}

fn charge<T>(result: Result<T, String>) -> io::Result<T> {
    result.map_err(invalid_data)
}

fn path(value: &str, budget: &mut Budget) -> io::Result<()> {
    if value != "source/oxid.toml" && !value.ends_with(".ox") {
        return Err(invalid_data(
            "library releases contain only their manifest and Oxid sources",
        ));
    }
    if value.split('/').count() > 32 {
        return Err(invalid_data("library source path exceeds 32 components"));
    }
    // validate_path's only dynamic success-path temporary is each uppercase
    // device-name stem. Admit its actual ASCII-copy length before that call.
    let bytes = value
        .split('/')
        .map(|component| component.split('.').next().unwrap_or_default().len())
        .sum();
    charge(budget.scoped(
        bytes,
        || validate_path(value, false).map_err(|error| error.to_string()),
        |_| Ok(0),
    ))
}

fn conflicts(left: &str, right: &str) -> bool {
    let mut left_parts = left.split('/');
    let mut right_parts = right.split('/');
    loop {
        match (left_parts.next(), right_parts.next()) {
            (Some(left), Some(right)) if left.eq_ignore_ascii_case(right) => {
                if left != right {
                    return true;
                }
            }
            (Some(_), Some(_)) => return false,
            _ => return true, // Same path, or one file is the other's parent.
        }
    }
}

fn roster<'a>(paths: impl Iterator<Item = &'a str> + Clone) -> io::Result<()> {
    if !paths.clone().any(|path| path == "source/oxid.toml")
        || paths.clone().any(|path| path == "source/oxid.lock")
    {
        return Err(invalid_data(
            "library releases require source/oxid.toml and exclude the author lock",
        ));
    }
    let mut previous = None;
    for (index, path) in paths.clone().enumerate() {
        if previous.is_some_and(|previous: &str| previous.as_bytes() >= path.as_bytes()) {
            return Err(invalid_data("library paths must be strictly sorted"));
        }
        for earlier in paths.clone().take(index) {
            if conflicts(earlier, path) {
                return Err(invalid_data(
                    "library path collision or case-aliased directory",
                ));
            }
        }
        previous = Some(path);
    }
    Ok(())
}

fn quoted<'a>(line: Option<&'a str>, prefix: &str) -> io::Result<&'a str> {
    line.and_then(|line| line.strip_prefix(prefix))
        .and_then(|line| line.strip_suffix('"'))
        .filter(|value| !value.contains(['"', '\\']) && !value.chars().any(char::is_control))
        .ok_or_else(|| invalid_data("noncanonical library inventory field"))
}

fn decimal(value: &str) -> io::Result<usize> {
    if value.is_empty()
        || value.len() > 20
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid_data("noncanonical library byte count"));
    }
    value
        .parse()
        .map_err(|_| invalid_data("library byte count overflow"))
}

fn inventory<'a>(bytes: &'a [u8], budget: &mut Budget) -> io::Result<Vec<BorrowedEntry<'a>>> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| invalid_data("library inventory is not UTF-8"))?;
    let body = text
        .strip_prefix(HEADER)
        .filter(|_| text.ends_with('\n'))
        .ok_or_else(|| invalid_data("unsupported library source artifact"))?;
    let mut lines = body.split_terminator('\n');
    let mut entries = Vec::new();
    let mut total = 0usize;
    while let Some(line) = lines.next() {
        if !line.is_empty() || lines.next() != Some("[[files]]") || entries.len() >= MAX_FILES {
            return Err(invalid_data("invalid library inventory roster"));
        }
        let name = quoted(lines.next(), "path = \"")?;
        path(name, budget)?;
        let size = decimal(
            lines
                .next()
                .and_then(|line| line.strip_prefix("size = "))
                .ok_or_else(|| invalid_data("missing library byte count"))?,
        )?;
        total = total
            .checked_add(size)
            .filter(|total| *total <= MAX_PAYLOAD_BYTES as usize)
            .ok_or_else(|| invalid_data("library payload exceeds 32 MiB"))?;
        let sha256 = quoted(lines.next(), "sha256 = \"")?;
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid_data("invalid library SHA-256"));
        }
        charge(budget.reserve(&mut entries, 1))?;
        entries.push(BorrowedEntry {
            path: name,
            size,
            sha256,
        });
    }
    roster(entries.iter().map(|entry| entry.path))?;
    Ok(entries)
}

pub(in crate::project_package) fn read_library(
    input: &mut impl Read,
    budget: &mut Budget,
) -> io::Result<(Archive, Info)> {
    let mut reader = HashReader::new(input);
    let mut header = [0; BLOCK];
    reader.fill(&mut header)?;
    let size = header_size(&header)?;
    if size > MAX_INVENTORY_BYTES as u64 {
        return Err(invalid_data("library inventory exceeds 1 MiB"));
    }
    check_header(&header, INVENTORY_PATH, size)?;
    let mut raw = Vec::new();
    charge(budget.reserve(&mut raw, size as usize))?;
    raw.resize(size as usize, 0);
    reader.fill(&mut raw)?;
    reader.padding(size)?;
    let entries = inventory(&raw, budget)?;
    let expected = wire_size(size, entries.iter().map(|entry| entry.size as u64))?;
    charge(budget.payload_admission(entries.len(), entries.iter().map(|entry| entry.size)))?;
    let mut files = Vec::new();
    charge(budget.reserve(&mut files, entries.len()))?;
    for entry in entries {
        reader.fill(&mut header)?;
        check_header(&header, entry.path, entry.size as u64)?;
        let mut bytes = charge(budget.payload(entry.size))?;
        bytes.resize(entry.size, 0);
        reader.fill(&mut bytes)?;
        charge(budget.metadata(64))?;
        if digest(&bytes) != entry.sha256 {
            return Err(invalid_data("library file digest mismatch"));
        }
        reader.padding(entry.size as u64)?;
        files.push(Member {
            path: charge(budget.text(entry.path))?,
            bytes,
        });
    }
    let mut terminator = [0; BLOCK * 2];
    reader.fill(&mut terminator)?;
    if terminator != [0; BLOCK * 2] {
        return Err(invalid_data(
            "library archive lacks its two zero terminators",
        ));
    }
    reader.eof()?;
    charge(budget.metadata(64))?;
    let info = reader.finish(files.len());
    if info.bytes != expected {
        return Err(invalid_data("library archive length mismatch"));
    }
    Ok((
        Archive {
            root: charge(budget.text("source"))?,
            files,
        },
        info,
    ))
}

pub(in crate::project_package) fn write_library(
    archive: &Archive,
    output: &mut impl Write,
    budget: &mut Budget,
) -> io::Result<Info> {
    if archive.root != "source" || archive.files.len() > MAX_FILES {
        return Err(invalid_input(
            "library release requires one bounded source root",
        ));
    }
    roster(archive.files.iter().map(|file| file.path.as_str()))?;
    let mut length = HEADER.len();
    let mut total = 0usize;
    for file in &archive.files {
        path(&file.path, budget)?;
        total = total
            .checked_add(file.bytes.len())
            .filter(|n| *n <= MAX_PAYLOAD_BYTES as usize)
            .ok_or_else(|| invalid_input("library payload exceeds 32 MiB"))?;
        let digits = if file.bytes.is_empty() {
            1
        } else {
            file.bytes.len().ilog10() as usize + 1
        };
        // Exact grammar bytes, including all quotes and newlines; digest is 64.
        length = length
            .checked_add(
                "\n[[files]]\npath = \"\"\nsize = \nsha256 = \"\"\n".len()
                    + file.path.len()
                    + digits
                    + 64,
            )
            .filter(|n| *n <= MAX_INVENTORY_BYTES)
            .ok_or_else(|| invalid_input("library inventory exceeds 1 MiB"))?;
    }
    wire_size(
        length as u64,
        archive.files.iter().map(|file| file.bytes.len() as u64),
    )?;
    let mut text = charge(budget.string(length))?;
    text.push_str(HEADER);
    for file in &archive.files {
        charge(budget.metadata(64))?;
        write!(
            text,
            "\n[[files]]\npath = \"{}\"\nsize = {}\nsha256 = \"{}\"\n",
            file.path,
            file.bytes.len(),
            digest(&file.bytes)
        )
        .map_err(|_| invalid_input("cannot encode library inventory"))?;
    }
    if text.len() != length || text.capacity() != length {
        return Err(invalid_input(
            "library inventory exceeded its exact admission",
        ));
    }
    let mut writer = HashWriter::new(output);
    writer.member(INVENTORY_PATH, text.as_bytes())?;
    for file in &archive.files {
        writer.member(&file.path, &file.bytes)?;
    }
    writer.put(&[0; BLOCK * 2])?;
    charge(budget.metadata(64))?;
    Ok(writer.finish(archive.files.len()))
}

/// Complete codec transports are charged once per registry operation; dynamic
/// inventory, entry/member slots and strings are charged at actual capacities.
pub(in crate::project_package) fn control_bytes() -> usize {
    use std::mem::size_of;
    size_of::<HashReader<'static, std::io::Empty>>()
        + size_of::<HashWriter<'static, std::io::Sink>>()
        + size_of::<BorrowedEntry<'static>>()
        + size_of::<Result<(Archive, Info), String>>()
        + size_of::<Result<(Archive, Info), io::Error>>()
        + size_of::<Result<Info, io::Error>>()
        + size_of::<Vec<BorrowedEntry<'static>>>()
        + size_of::<Vec<Member>>()
        + size_of::<String>()
        + size_of::<Vec<u8>>()
        + size_of::<Archive>()
        + size_of::<Info>()
        + 4 * BLOCK
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Archive {
        Archive {
            root: "source".into(),
            files: vec![
                Member {
                    path: "source/oxid.toml".into(),
                    bytes: b"[project]\nkind=\"lib\"\n".to_vec(),
                },
                Member {
                    path: "source/src/lib.ox".into(),
                    bytes: b"pub fn value()->i32{return 7;}".to_vec(),
                },
            ],
        }
    }
    #[test]
    fn library_and_deployment_bundle_kinds_do_not_cross_their_boundaries() {
        let archive = fixture();
        let mut bytes = Vec::new();
        let info = write_library(&archive, &mut bytes, &mut Budget::new().unwrap()).unwrap();
        let restored = read_library(&mut bytes.as_slice(), &mut Budget::new().unwrap()).unwrap();
        assert_eq!(restored, (archive.clone(), info));
        assert!(super::super::read(&mut bytes.as_slice()).is_err());
        let mut bundle = archive;
        bundle.files.push(Member {
            path: "source/oxid.lock".into(),
            bytes: b"lock".to_vec(),
        });
        bundle.files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut original = Vec::new();
        super::super::write(&bundle, &mut original).unwrap();
        assert!(read_library(&mut original.as_slice(), &mut Budget::new().unwrap()).is_err());
        let mut refused = Vec::new();
        assert!(write_library(&bundle, &mut refused, &mut Budget::new().unwrap()).is_err());
        assert!(refused.is_empty());
    }
    #[test]
    fn library_format_refuses_case_aliases_depth_and_damaged_wire_before_publication() {
        let mut archive = fixture();
        archive.files.push(Member {
            path: "source/SRC/other.ox".into(),
            bytes: Vec::new(),
        });
        archive.files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut output = Vec::new();
        assert!(write_library(&archive, &mut output, &mut Budget::new().unwrap()).is_err());
        assert!(output.is_empty());
        archive = fixture();
        archive.files.push(Member {
            path: format!("source/{}leaf.ox", "a/".repeat(31)),
            bytes: Vec::new(),
        });
        archive.files.sort_by(|a, b| a.path.cmp(&b.path));
        assert!(write_library(&archive, &mut output, &mut Budget::new().unwrap()).is_err());
        assert!(output.is_empty());
        let mut valid = Vec::new();
        write_library(&fixture(), &mut valid, &mut Budget::new().unwrap()).unwrap();
        for mutation in [0, BLOCK + 30, valid.len() - 1] {
            let mut changed = valid.clone();
            changed[mutation] ^= 1;
            assert!(
                read_library(&mut changed.as_slice(), &mut Budget::new().unwrap()).is_err(),
                "{mutation}"
            );
        }
        valid.extend_from_slice(b"tail");
        assert!(read_library(&mut valid.as_slice(), &mut Budget::new().unwrap()).is_err());
    }
}
