//! Opt-in cache of the verified LLVM program object, never a final executable.
//! The C runtime and SDK link still execute on every build. Cache entries are
//! immutable, bounded blobs; a reader copies and hashes the same opened bytes.
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(super) type Environment = Vec<(OsString, OsString)>;
const MAGIC: &[u8; 16] = b"OXID-OBJECT-v1\0\0";
const MAX_OBJECT: u64 = 64 * 1024 * 1024;
const HEADER: u64 = 16 + 32 + 8 + 32;

#[derive(Clone, Copy)]
pub(super) enum Outcome {
    Hit,
    Miss,
    Unavailable,
}
impl Outcome {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::Unavailable => "unavailable",
        }
    }
}

pub(super) fn environment() -> Environment {
    let mut values: Environment = std::env::vars_os()
        .filter(|(name, _)| name != "CCC_OVERRIDE_OPTIONS")
        .collect();
    values.sort();
    values
}

fn regular(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() || fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::other("cache input must be a regular file"));
    }
    Ok(file)
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.size() == right.size()
            && left.mode() == right.mode()
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len() && left.modified().ok() == right.modified().ok()
    }
}

fn object_header(bytes: &[u8]) -> bool {
    bytes.len() >= 64 && &bytes[..7] == b"\x7fELF\x02\x01\x01" && bytes[16..20] == [1, 0, 62, 0]
    // ET_REL, EM_X86_64
}

fn digest_file(path: &Path, limit: u64) -> std::io::Result<([u8; 32], u64)> {
    let mut file = regular(path)?;
    let before = file.metadata()?;
    if before.len() > limit {
        return Err(std::io::Error::other("cache input exceeds byte limit"));
    }
    let mut hasher = Sha256::new();
    let mut count = 0;
    let mut buffer = [0u8; 65536];
    loop {
        let size = file.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        count += size as u64;
        if count > limit {
            return Err(std::io::Error::other("cache input exceeds byte limit"));
        }
        hasher.update(&buffer[..size]);
    }
    if count != before.len()
        || !same_file(&before, &file.metadata()?)
        || !same_file(&before, &fs::symlink_metadata(path)?)
    {
        return Err(std::io::Error::other("cache input changed while hashing"));
    }
    Ok((hasher.finalize().into(), count))
}

pub(super) struct Cache {
    key: [u8; 32],
    entry: PathBuf,
}

impl Cache {
    pub(super) fn new(root: &Path, key: [u8; 32]) -> std::io::Result<Self> {
        // All ancestors up to .oxid are checked by the managed output boundary.
        for path in [root.join("cache"), root.join("cache/native-object-v1")] {
            match fs::create_dir(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            let metadata = fs::symlink_metadata(&path)?;
            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                return Err(std::io::Error::other(
                    "native cache directories must be real directories",
                ));
            }
        }
        let name: String = key.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(Self {
            key,
            entry: root
                .join("cache/native-object-v1")
                .join(format!("{name}.oxobj")),
        })
    }

    /// A malformed or unavailable entry is a miss. Never follow it, replace it
    /// or hand a writable cache inode to the linker.
    pub(super) fn restore(&self, output: &Path) -> bool {
        let mut created = false;
        let result = (|| -> std::io::Result<()> {
            let mut source = regular(&self.entry)?;
            let before = source.metadata()?;
            if before.len() < HEADER + 64 || before.len() > HEADER + MAX_OBJECT {
                return Err(std::io::Error::other("invalid native cache length"));
            }
            let mut header = [0u8; HEADER as usize];
            source.read_exact(&mut header)?;
            let size = u64::from_le_bytes(header[48..56].try_into().unwrap());
            if &header[..16] != MAGIC
                || header[16..48] != self.key
                || size.checked_add(HEADER) != Some(before.len())
            {
                return Err(std::io::Error::other("invalid native cache header"));
            }
            let mut prefix = [0u8; 64];
            source.read_exact(&mut prefix)?;
            if !object_header(&prefix) {
                return Err(std::io::Error::other("invalid cached object target"));
            }
            let mut destination = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            created = true;
            let mut hasher = Sha256::new();
            hasher.update(prefix);
            destination.write_all(&prefix)?;
            let mut remaining = size - prefix.len() as u64;
            let mut buffer = [0u8; 65536];
            while remaining != 0 {
                let wanted = remaining.min(buffer.len() as u64) as usize;
                source.read_exact(&mut buffer[..wanted])?;
                hasher.update(&buffer[..wanted]);
                destination.write_all(&buffer[..wanted])?;
                remaining -= wanted as u64;
            }
            if hasher.finalize()[..] != header[56..88]
                || source.read(&mut [0])? != 0
                || !same_file(&before, &source.metadata()?)
                || !same_file(&before, &fs::symlink_metadata(&self.entry)?)
            {
                return Err(std::io::Error::other(
                    "cached object changed or failed integrity",
                ));
            }
            destination.sync_all()?;
            Ok(())
        })();
        if result.is_err() && created {
            // output is a fresh compiler-owned path in its private workspace.
            let _ = fs::remove_file(output);
        }
        result.is_ok()
    }

    /// stage and object belong to the current private native workspace. Saving
    /// is optional: races, corruption, or a full cache never invalidate a build.
    pub(super) fn save(&self, object: &Path, stage: &Path) -> std::io::Result<()> {
        let (digest, size) = digest_file(object, MAX_OBJECT)?;
        let mut input = regular(object)?;
        let before = input.metadata()?;
        let mut prefix = [0u8; 64];
        input.read_exact(&mut prefix)?;
        if !object_header(&prefix) {
            return Err(std::io::Error::other(
                "native compiler did not emit an x86_64 relocatable object",
            ));
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage)?;
        output.write_all(MAGIC)?;
        output.write_all(&self.key)?;
        output.write_all(&size.to_le_bytes())?;
        output.write_all(&digest)?;
        output.write_all(&prefix)?;
        let mut copied_hash = Sha256::new();
        copied_hash.update(prefix);
        let mut remaining = size
            .checked_sub(64)
            .ok_or_else(|| std::io::Error::other("short object"))?;
        let mut buffer = [0u8; 65536];
        while remaining != 0 {
            let wanted = remaining.min(buffer.len() as u64) as usize;
            input.read_exact(&mut buffer[..wanted])?;
            output.write_all(&buffer[..wanted])?;
            copied_hash.update(&buffer[..wanted]);
            remaining -= wanted as u64;
        }
        if copied_hash.finalize()[..] != digest
            || input.read(&mut [0])? != 0
            || !same_file(&before, &input.metadata()?)
        {
            return Err(std::io::Error::other(
                "native object changed before cache publication",
            ));
        }
        output.sync_all()?;
        match fs::hard_link(stage, &self.entry) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error),
        }
    }
}

pub(super) fn fingerprint(
    module: &str,
    tools: &[&Path],
    args: &[OsString],
    environment: &Environment,
    cwd: &Path,
) -> Option<[u8; 32]> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::fingerprint(module, tools, args, environment, cwd)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (module, tools, args, environment, cwd);
        None
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::*;
    use std::{
        collections::BTreeSet,
        ffi::OsStr,
        io::{Seek, SeekFrom},
        os::unix::ffi::OsStrExt,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    fn framed(hash: &mut Sha256, bytes: &[u8]) {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }

    fn interpreter(path: &Path) -> Option<PathBuf> {
        let mut file = regular(path).ok()?;
        let mut header = [0u8; 64];
        file.read_exact(&mut header).ok()?;
        if &header[..7] != b"\x7fELF\x02\x01\x01" || header[18..20] != [62, 0] {
            return None;
        }
        let offset = u64::from_le_bytes(header[32..40].try_into().ok()?);
        let size = u16::from_le_bytes(header[54..56].try_into().ok()?) as u64;
        let count = u16::from_le_bytes(header[56..58].try_into().ok()?) as u64;
        if size != 56
            || count > 128
            || offset.checked_add(size * count)? > file.metadata().ok()?.len()
        {
            return None;
        }
        let mut found = None;
        for index in 0..count {
            file.seek(SeekFrom::Start(offset + size * index)).ok()?;
            let mut row = [0u8; 56];
            file.read_exact(&mut row).ok()?;
            if u32::from_le_bytes(row[..4].try_into().ok()?) != 3 {
                continue;
            }
            if found.is_some() {
                return None;
            }
            let start = u64::from_le_bytes(row[8..16].try_into().ok()?);
            let length = u64::from_le_bytes(row[32..40].try_into().ok()?);
            if !(2..=4096).contains(&length) {
                return None;
            }
            file.seek(SeekFrom::Start(start)).ok()?;
            let mut value = vec![0u8; length as usize];
            file.read_exact(&mut value).ok()?;
            if value.pop()? != 0 || value.contains(&0) {
                return None;
            }
            let path = PathBuf::from(OsStr::from_bytes(&value));
            if !path.is_absolute() || path.file_name()? != "ld-linux-x86-64.so.2" {
                return None;
            }
            found = Some(path);
        }
        found
    }

    fn loader_list(
        loader: &Path,
        tool: &Path,
        environment: &Environment,
        cwd: &Path,
    ) -> Option<String> {
        let mut child = Command::new(loader)
            .arg("--list")
            .arg(tool)
            .current_dir(cwd)
            .env_clear()
            .envs(environment.iter().cloned())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let output = child.stdout.take()?;
        let reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            output.take(65537).read_to_end(&mut bytes).map(|_| bytes)
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        let success = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.success(),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break false;
                }
            }
        };
        let bytes = reader.join().ok()?.ok()?;
        if !success || bytes.len() > 65536 {
            return None;
        }
        String::from_utf8(bytes).ok()
    }

    pub(super) fn fingerprint(
        module: &str,
        tools: &[&Path],
        args: &[OsString],
        environment: &Environment,
        cwd: &Path,
    ) -> Option<[u8; 32]> {
        if environment.iter().any(|(name, _)| {
            name == "LD_PRELOAD" || name == "LD_AUDIT" || name == "LD_TRACE_LOADED_OBJECTS"
        }) {
            return None;
        }
        let mut hash = Sha256::new();
        framed(&mut hash, b"oxid-native-object-cache-v1");
        framed(&mut hash, module.as_bytes());
        hash.update((args.len() as u64).to_le_bytes());
        for arg in args {
            framed(&mut hash, arg.as_bytes());
        }
        framed(&mut hash, b"environment");
        hash.update((environment.len() as u64).to_le_bytes());
        for (name, value) in environment {
            framed(&mut hash, name.as_bytes());
            framed(&mut hash, value.as_bytes());
        }
        let mut paths = BTreeSet::new();
        paths.insert(std::env::current_exe().ok()?);
        hash.update((tools.len() as u64).to_le_bytes());
        for tool in tools {
            framed(&mut hash, tool.as_os_str().as_bytes());
            let physical = fs::canonicalize(tool).ok()?;
            let loader = interpreter(&physical)?;
            let listing = loader_list(&loader, &physical, environment, cwd)?;
            paths.insert(physical);
            paths.insert(loader);
            let mut libraries = 0;
            for line in listing.lines() {
                let line = line.trim();
                if line.starts_with("linux-vdso.so.1 (") {
                    continue;
                }
                let (body, address) = line.rsplit_once(" (0x")?;
                if !address.ends_with(')') {
                    return None;
                }
                let path = body.split_once(" => ").map_or(body, |(_, path)| path);
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    return None;
                }
                paths.insert(path);
                libraries += 1;
                if paths.len() > 128 {
                    return None;
                }
            }
            if libraries == 0 {
                return None;
            }
        }
        let mut total = 0u64;
        hash.update((paths.len() as u64).to_le_bytes());
        for path in paths {
            let physical = fs::canonicalize(&path).ok()?;
            let (digest, bytes) = digest_file(&physical, 512 * 1024 * 1024).ok()?;
            total = total.checked_add(bytes)?;
            if total > 1024 * 1024 * 1024 {
                return None;
            }
            framed(&mut hash, path.as_os_str().as_bytes());
            framed(&mut hash, physical.as_os_str().as_bytes());
            hash.update(bytes.to_le_bytes());
            hash.update(digest);
        }
        Some(hash.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "oxid-object-cache-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn object(&self) -> PathBuf {
            let mut bytes = vec![0u8; 128];
            bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
            bytes[16..20].copy_from_slice(&[1, 0, 62, 0]);
            bytes[64..].fill(42);
            let path = self.0.join("program.o");
            fs::write(&path, bytes).unwrap();
            path
        }
        fn populated(&self) -> Cache {
            let cache = Cache::new(&self.0, [7; 32]).unwrap();
            cache
                .save(&self.object(), &self.0.join("publish-stage"))
                .unwrap();
            cache
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn cache_copies_verified_bytes_without_sharing_a_writable_object_inode() {
        let fixture = Fixture::new();
        let cache = fixture.populated();
        let output = fixture.0.join("restored.o");
        assert!(cache.restore(&output));
        assert_eq!(
            fs::read(&output).unwrap(),
            fs::read(fixture.0.join("program.o")).unwrap()
        );
        fs::write(&output, b"modified private object").unwrap();
        assert!(cache.restore(&fixture.0.join("another.o")));
        assert_ne!(
            fs::read(fixture.0.join("another.o")).unwrap(),
            fs::read(output).unwrap()
        );
    }

    #[test]
    fn corrupt_header_body_length_key_and_target_cannot_hit_or_leave_partial_objects() {
        let fixture = Fixture::new();
        let cache = fixture.populated();
        let original = fs::read(&cache.entry).unwrap();
        let mut mutations = vec![
            original[..20].to_vec(),
            original[..original.len() - 1].to_vec(),
        ];
        for index in [
            0,
            16,
            48,
            56,
            88,
            88 + 4,
            88 + 16,
            88 + 18,
            original.len() - 1,
        ] {
            let mut bytes = original.clone();
            bytes[index] ^= 1;
            if (88..152).contains(&index) {
                // A coherent checksum must not bless an object for the wrong
                // class, endianness, file type or machine.
                let digest = Sha256::digest(&bytes[88..]);
                bytes[56..88].copy_from_slice(&digest);
            }
            mutations.push(bytes);
        }
        let mut overflow = original.clone();
        overflow[48..56].fill(255);
        mutations.push(overflow);
        let mut extra = original.clone();
        extra.push(0);
        mutations.push(extra);
        for (index, bytes) in mutations.iter().enumerate() {
            fs::write(&cache.entry, bytes).unwrap();
            let output = fixture.0.join(format!("rejected-{index}.o"));
            assert!(!cache.restore(&output), "accepted mutation {index}");
            assert!(!output.exists(), "left partial object {index}");
        }
        fs::write(&cache.entry, original).unwrap();
        let file = OpenOptions::new().write(true).open(&cache.entry).unwrap();
        file.set_len(HEADER + MAX_OBJECT + 1).unwrap();
        assert!(!cache.restore(&fixture.0.join("oversized.o")));
    }

    #[test]
    fn preexisting_output_and_corrupt_winner_are_preserved() {
        let fixture = Fixture::new();
        let cache = fixture.populated();
        let output = fixture.0.join("existing.o");
        fs::write(&output, b"existing output").unwrap();
        assert!(!cache.restore(&output));
        assert_eq!(fs::read(&output).unwrap(), b"existing output");
        fs::write(&cache.entry, b"bad cache winner").unwrap();
        cache
            .save(&fixture.object(), &fixture.0.join("second-stage"))
            .unwrap();
        assert_eq!(fs::read(&cache.entry).unwrap(), b"bad cache winner");
        assert!(!cache.restore(&fixture.0.join("bad-winner.o")));
    }

    #[test]
    fn concurrent_writers_publish_one_complete_immutable_entry() {
        let fixture = Fixture::new();
        let cache = Cache::new(&fixture.0, [3; 32]).unwrap();
        let object = fixture.object();
        let barrier = std::sync::Barrier::new(8);
        std::thread::scope(|scope| {
            for index in 0..8 {
                let (cache, object, root, barrier) = (&cache, &object, &fixture.0, &barrier);
                scope.spawn(move || {
                    barrier.wait();
                    cache
                        .save(object, &root.join(format!("stage-{index}")))
                        .unwrap();
                });
            }
        });
        let output = fixture.0.join("winner.o");
        assert!(cache.restore(&output));
        assert_eq!(fs::read(output).unwrap(), fs::read(object).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_directories_and_fifos_are_never_cache_inputs_or_publication_targets() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let cache = Cache::new(&fixture.0, [5; 32]).unwrap();
        let sentinel = fixture.0.join("sentinel");
        fs::write(&sentinel, b"preserve").unwrap();
        symlink(&sentinel, &cache.entry).unwrap();
        assert!(!cache.restore(&fixture.0.join("symlink.o")));
        cache
            .save(&fixture.object(), &fixture.0.join("stage"))
            .unwrap();
        assert_eq!(fs::read(&sentinel).unwrap(), b"preserve");
        fs::remove_file(&cache.entry).unwrap();
        fs::create_dir(&cache.entry).unwrap();
        assert!(!cache.restore(&fixture.0.join("directory.o")));
        fs::remove_dir(&cache.entry).unwrap();
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::CString::new(cache.entry.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        assert!(!cache.restore(&fixture.0.join("fifo.o")));
        fs::remove_file(&cache.entry).unwrap();
        fs::remove_dir(fixture.0.join("cache/native-object-v1")).unwrap();
        symlink(&fixture.0, fixture.0.join("cache/native-object-v1")).unwrap();
        assert!(Cache::new(&fixture.0, [5; 32]).is_err());
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn scripts_and_loader_injections_are_ineligible_for_reuse() {
        let fixture = Fixture::new();
        let script = fixture.0.join("clang");
        fs::write(&script, b"#!/bin/sh\nexit 1\n").unwrap();
        let values = environment();
        assert!(fingerprint("IR", &[&script], &[], &values, &fixture.0).is_none());
        for name in ["LD_PRELOAD", "LD_AUDIT", "LD_TRACE_LOADED_OBJECTS"] {
            let mut values = values.clone();
            values.push((name.into(), "".into()));
            assert!(fingerprint("IR", &[], &[], &values, &fixture.0).is_none());
        }
    }
}
