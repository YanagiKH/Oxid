//! Deterministic native-bundle serialization. No application execution.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Entry behavior is explicit metadata, never inferred from ELF contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BundleEntry {
    Result,
    Process,
}
impl BundleEntry {
    fn name(self) -> &'static str {
        match self {
            Self::Result => "result",
            Self::Process => "process",
        }
    }
}
pub(super) const MAX_EXECUTABLE: usize = 64 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);
fn name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn octal(field: &mut [u8], value: u64) -> Result<(), String> {
    let digits = format!("{value:o}");
    if digits.len() >= field.len() {
        return Err("archive numeric field overflow".into());
    }
    field.fill(b'0');
    let start = field.len() - 1 - digits.len();
    field[start..start + digits.len()].copy_from_slice(digits.as_bytes());
    *field.last_mut().unwrap() = 0;
    Ok(())
}
fn member(out: &mut impl Write, name: &str, mode: u64, bytes: &[u8]) -> Result<(), String> {
    if name.len() > 100 || !name.is_ascii() {
        return Err("invalid archive member name".into());
    }
    let mut header = [0u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    octal(&mut header[100..108], mode)?;
    octal(&mut header[108..116], 0)?;
    octal(&mut header[116..124], 0)?;
    octal(&mut header[124..136], bytes.len() as u64)?;
    octal(&mut header[136..148], 0)?;
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let sum: u64 = header.iter().map(|b| u64::from(*b)).sum();
    octal(&mut header[148..155], sum)?;
    header[155] = b' ';
    out.write_all(&header)
        .map_err(|e| format!("write header for {name}: {e}"))?;
    out.write_all(bytes)
        .map_err(|e| format!("write member {name}: {e}"))?;
    let padding = (512 - bytes.len() % 512) % 512;
    out.write_all(&[0u8; 512][..padding])
        .map_err(|e| format!("pad member {name}: {e}"))
}
pub(super) fn serialize(
    out: &mut impl Write,
    name: &str,
    entry: BundleEntry,
    executable: &[u8],
) -> Result<(), String> {
    if !name_valid(name) {
        return Err("invalid native package name".into());
    }
    if executable.len() < 64
        || executable.len() > MAX_EXECUTABLE
        || &executable[..4] != b"\x7fELF"
        || executable[4..7] != [2, 1, 1]
        || executable[16..20] != [3, 0, 62, 0]
        || executable[20..24] != [1, 0, 0, 0]
        || executable[52..54] != [64, 0]
    {
        return Err("native bundle requires a bounded x86_64 ELF64 artifact".into());
    }
    let entry_mode = entry.name();
    let digest = format!("{:x}", Sha256::digest(executable));
    let metadata = format!("{{\"schema\":\"oxid-native-bundle-v1\",\"name\":\"{name}\",\"target\":\"x86_64-unknown-linux-gnu\",\"profile\":\"native-preview-O0\",\"entry_mode\":\"{entry_mode}\",\"executable\":\"bin/{name}\",\"sha256\":\"{digest}\",\"dynamic_runtime\":\"host-provided\"}}\n");
    let readme = format!("Oxid native executable bundle: {name}\nExtract this tar archive into a new directory with a standard tar reader.\nThen run ./bin/{name} from that extracted directory on a compatible Linux x86_64 host.\nEntry mode: {entry_mode}.\nSystem dynamic runtime libraries are not included.\nThis bundle does not certify compatibility with every Linux distribution.\nThe application was not run while packaging.\n");
    member(out, &format!("bin/{name}"), 0o755, executable)?;
    member(out, "package.json", 0o644, metadata.as_bytes())?;
    member(out, "README.txt", 0o644, readme.as_bytes())?;
    out.write_all(&[0u8; 1024])
        .map_err(|e| format!("finish archive: {e}"))
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub(super) fn create(
    output: &Path,
    name: &str,
    entry: BundleEntry,
    executable: &[u8],
) -> Result<(), String> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for _ in 0..100 {
        let path = parent.join(format!(
            ".oxid-package-{}-{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("create package temporary file: {e}")),
        };
        let temporary = Temporary(path);
        serialize(&mut file, name, entry, executable)?;
        file.flush().map_err(|e| format!("flush package: {e}"))?;
        fs::hard_link(&temporary.0, output)
            .map_err(|e| format!("publish package without replacement: {e}"))?;
        return Ok(());
    }
    Err("package temporary-name collisions".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn elf() -> Vec<u8> {
        let mut b = vec![0; 64];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4..7].copy_from_slice(&[2, 1, 1]);
        b[16..20].copy_from_slice(&[3, 0, 62, 0]);
        b[20..24].copy_from_slice(&[1, 0, 0, 0]);
        b[52..54].copy_from_slice(&[64, 0]);
        b
    }
    #[test]
    fn archive_is_deterministic_and_ustar_framed() {
        let mut a = Vec::new();
        let mut b = Vec::new();
        serialize(&mut a, "demo", BundleEntry::Result, &elf()).unwrap();
        serialize(&mut b, "demo", BundleEntry::Result, &elf()).unwrap();
        assert_eq!(a, b);
        assert_eq!(&a[..8], b"bin/demo");
        assert_eq!(&a[257..265], b"ustar\x0000");
        assert_eq!(a.len() % 512, 0);
        assert!(a[a.len() - 1024..].iter().all(|b| *b == 0));
    }
    #[test]
    fn entry_behavior_is_explicit_metadata() {
        for (entry, expected) in [
            (BundleEntry::Result, "result"),
            (BundleEntry::Process, "process"),
        ] {
            let mut out = Vec::new();
            serialize(&mut out, "demo", entry, &elf()).unwrap();
            let text = String::from_utf8_lossy(&out);
            assert!(text.contains(&format!("\"entry_mode\":\"{expected}\"")));
        }
    }
    #[test]
    fn invalid_inputs_write_nothing() {
        for name in ["", "../x", "a/b", "a\\b", "_x"] {
            let mut out = Vec::new();
            assert!(serialize(&mut out, name, BundleEntry::Result, &elf()).is_err());
            assert!(out.is_empty());
        }
        let mut out = Vec::new();
        assert!(serialize(&mut out, "demo", BundleEntry::Result, b"not elf").is_err());
        assert!(out.is_empty());
    }
}
