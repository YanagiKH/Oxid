//! Closed, local producer bundles and their owned executable snapshots.
//!
//! A manifest digest is a local byte-identity assertion, not a signature or a
//! provenance claim. Only sealed memfd copies are returned for execution. The
//! caller must keep this owner alive until both producer processes have ended.

use super::super::hir_protocol::Protocol;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct Bundle {
    protocol: Protocol,
    workspace: Workspace,
    parser: Executable,
    static_consumer: Executable,
}

#[derive(Debug)]
struct Executable {
    // This owned descriptor, rather than a mutable filesystem name, is the
    // executable's lifetime and identity. CLOEXEC closes it after ELF loading.
    _file: std::fs::File,
    path: PathBuf,
    hash: [u8; 32],
}

impl Bundle {
    pub(super) fn load(root: &Path) -> Result<Self, &'static str> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            linux::load(root, &std::env::temp_dir(), linux::EXECUTABLE_LIMIT)
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = root;
            Err("producer bundles require Linux x86_64")
        }
    }

    pub(super) fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub(super) fn parser(&self) -> &Path {
        &self.parser.path
    }

    pub(super) fn static_consumer(&self) -> &Path {
        &self.static_consumer.path
    }

    pub(super) fn directory(&self) -> &Path {
        &self.workspace.path
    }

    pub(super) fn parser_hash(&self) -> [u8; 32] {
        self.parser.hash
    }

    pub(super) fn static_hash(&self) -> [u8; 32] {
        self.static_consumer.hash
    }
}

/// Constructed only after an exclusive, successful creation of this directory.
/// Input bundle paths are never stored here or passed to recursive removal.
#[derive(Debug)]
struct Workspace {
    path: PathBuf,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        // Best effort on filesystem errors; never expand cleanup to the parent.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::{Bundle, Executable, Protocol, Workspace};
    use sha2::{Digest, Sha256};
    use std::ffi::CStr;
    use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub(super) const EXECUTABLE_LIMIT: usize = 16 * 1024 * 1024;
    const MAGIC: &[u8] = b"OXID-HIR-PRODUCERS-1\n";
    const MANIFEST_LEN: usize = MAGIC.len() + 7 + 64 + 1 + 7 + 64 + 1;
    const ELF_HEADER_LEN: usize = 64;
    const REQUIRED_SEALS: libc::c_int = libc::F_SEAL_WRITE
        | libc::F_SEAL_GROW
        | libc::F_SEAL_SHRINK
        | libc::F_SEAL_SEAL
        | libc::F_SEAL_EXEC;
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[derive(Debug, PartialEq, Eq)]
    struct Manifest {
        protocol: Protocol,
        parser: [u8; 32],
        static_consumer: [u8; 32],
    }

    pub(super) fn load(
        root: &Path,
        temporary_parent: &Path,
        executable_limit: usize,
    ) -> Result<Bundle, &'static str> {
        let executable_limit = executable_limit.min(EXECUTABLE_LIMIT);
        let root = open_directory(root)?;
        let manifest = read_manifest(&root)?;
        let workspace = create_workspace(temporary_parent)?;
        let parser = snapshot(
            open_regular_at(&root, c"parser")?,
            manifest.parser,
            executable_limit,
        )?;
        let static_consumer = snapshot(
            open_regular_at(&root, c"static")?,
            manifest.static_consumer,
            executable_limit,
        )?;
        Ok(Bundle {
            protocol: manifest.protocol,
            workspace,
            parser,
            static_consumer,
        })
    }

    fn open_directory(path: &Path) -> Result<File, &'static str> {
        // Lexically remove trailing slashes and dot components without resolving
        // links. Otherwise link/ and link/. bypass O_NOFOLLOW on the final link.
        let normalized: PathBuf = path.components().collect();
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(normalized)
            .map_err(|_| "producer bundle root must be an accessible nonsymlink directory")?;
        if !file
            .metadata()
            .map_err(|_| "cannot inspect opened producer bundle directory")?
            .is_dir()
        {
            return Err("opened producer bundle root must be a directory");
        }
        Ok(file)
    }

    fn open_regular_at(directory: &File, name: &CStr) -> Result<File, &'static str> {
        // Names are fixed call-site constants, never data read from a manifest.
        // SAFETY: stat is caller-owned writable storage; the live directory
        // descriptor and NUL-terminated name remain valid for both syscalls.
        let mut metadata: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                &mut metadata,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err("cannot inspect producer bundle file");
        }
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err("producer bundle files must be regular nonsymlink files");
        }
        // O_NONBLOCK prevents a raced-in FIFO from blocking. O_NOFOLLOW rejects
        // a raced-in symlink. fstat then checks the opened object itself.
        let descriptor = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if descriptor < 0 {
            return Err("cannot safely open producer bundle file");
        }
        // SAFETY: successful openat returned a new descriptor owned only here.
        let file = unsafe { File::from_raw_fd(descriptor) };
        if !file
            .metadata()
            .map_err(|_| "cannot inspect opened producer bundle file")?
            .file_type()
            .is_file()
        {
            return Err("opened producer bundle file must be regular");
        }
        Ok(file)
    }

    fn read_manifest(directory: &File) -> Result<Manifest, &'static str> {
        let mut file = open_regular_at(directory, c"manifest.txt")?;
        // One extra byte detects trailing data without an unbounded allocation
        // or read, even when the manifest grows after open.
        let mut bytes = [0u8; MANIFEST_LEN + 1];
        let mut used = 0;
        while used < bytes.len() {
            let count = read_retry(&mut file, &mut bytes[used..])
                .map_err(|_| "cannot read producer bundle manifest")?;
            if count == 0 {
                break;
            }
            used += count;
        }
        parse_manifest(&bytes[..used])
    }

    fn parse_manifest(bytes: &[u8]) -> Result<Manifest, &'static str> {
        const INVALID: &str = "producer bundle manifest does not match the closed format";
        if bytes.len() != MANIFEST_LEN
            || (!bytes.starts_with(MAGIC) && !bytes.starts_with(b"OXID-HIR-PRODUCERS-2\n"))
        {
            return Err(INVALID);
        }
        let parser_line = &bytes[MAGIC.len()..MAGIC.len() + 72];
        let static_line = &bytes[MAGIC.len() + 72..];
        if !parser_line.starts_with(b"parser ")
            || parser_line[71] != b'\n'
            || !static_line.starts_with(b"static ")
            || static_line[71] != b'\n'
        {
            return Err(INVALID);
        }
        Ok(Manifest {
            protocol: if bytes[MAGIC.len() - 2] == b'1' {
                Protocol::V1
            } else {
                Protocol::V2
            },
            parser: decode_hash(&parser_line[7..71]).ok_or(INVALID)?,
            static_consumer: decode_hash(&static_line[7..71]).ok_or(INVALID)?,
        })
    }

    fn decode_hash(bytes: &[u8]) -> Option<[u8; 32]> {
        fn digit(byte: u8) -> Option<u8> {
            match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            }
        }
        if bytes.len() != 64 {
            return None;
        }
        let mut hash = [0u8; 32];
        for (index, result) in hash.iter_mut().enumerate() {
            *result = (digit(bytes[index * 2])? << 4) | digit(bytes[index * 2 + 1])?;
        }
        Some(hash)
    }

    fn create_workspace(parent: &Path) -> Result<Workspace, &'static str> {
        // TMPDIR may be relative. Snapshot paths must remain absolute when the
        // supervisor selects a different working directory for the processes.
        let parent = fs::canonicalize(parent)
            .map_err(|_| "cannot resolve private producer workspace parent")?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // Name unpredictability is not the isolation boundary: atomic mkdir
        // must succeed, and mode 0700 is applied at creation, never afterward.
        for _ in 0..128 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                "oxid-hir-producers-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            match DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Workspace { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err("cannot create private producer workspace"),
            }
        }
        Err("cannot allocate a unique private producer workspace")
    }

    fn snapshot(
        mut input: File,
        declared: [u8; 32],
        limit: usize,
    ) -> Result<Executable, &'static str> {
        if input
            .metadata()
            .map_err(|_| "cannot inspect producer executable size")?
            .len()
            > limit as u64
        {
            return Err("producer executable exceeds size limit");
        }
        let mut output = create_memfd()?;
        let mut buffer = [0u8; 8192];
        let mut header = [0u8; ELF_HEADER_LEN];
        let mut copied = 0usize;
        let mut digest = Sha256::new();
        loop {
            // Read at most the remaining allowance plus one overflow byte.
            // The fixed stack buffer also bounds memory for large executables.
            let read_limit = buffer.len().min(limit.saturating_sub(copied) + 1);
            let count = read_retry(&mut input, &mut buffer[..read_limit])
                .map_err(|_| "cannot read producer executable")?;
            if count == 0 {
                break;
            }
            if count > limit.saturating_sub(copied) {
                return Err("producer executable exceeds size limit");
            }
            if copied < header.len() {
                let header_count = count.min(header.len() - copied);
                header[copied..copied + header_count].copy_from_slice(&buffer[..header_count]);
            }
            output
                .write_all(&buffer[..count])
                .map_err(|_| "cannot write private producer snapshot")?;
            digest.update(&buffer[..count]);
            copied += count;
        }
        check_elf(&header, copied)?;
        let actual: [u8; 32] = digest.finalize().into();
        if actual != declared {
            return Err("producer executable digest does not match the bundle manifest");
        }
        output
            .flush()
            .map_err(|_| "cannot flush private producer snapshot")?;
        output
            .set_permissions(Permissions::from_mode(0o500))
            .map_err(|_| "cannot make private producer snapshot executable")?;
        // Seals protect the bytes even from another process with the same UID
        // that chmods/reopens /proc/<pid>/fd/<fd>. F_SEAL_EXEC also freezes the
        // executable bits. No permission/unsupported-kernel fallback is used.
        // SAFETY: output owns a live memfd and fcntl receives integer arguments.
        if unsafe { libc::fcntl(output.as_raw_fd(), libc::F_ADD_SEALS, REQUIRED_SEALS) } < 0 {
            return Err("cannot seal producer executable snapshot");
        }
        let seals = unsafe { libc::fcntl(output.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 || seals & REQUIRED_SEALS != REQUIRED_SEALS {
            return Err("producer executable snapshot seals are incomplete");
        }
        // Reopen this kernel-owned descriptor link read-only, then close the
        // writable handle before execution. This is not a bundle/workspace
        // pathname and cannot be redirected by renaming any user-owned file.
        let readonly = File::open(descriptor_path(&output))
            .map_err(|_| "cannot retain sealed producer executable snapshot")?;
        let mut readonly = avoid_stdio(readonly)?;
        // Record the bytes after immutability is established as well as checking
        // the streaming copy. This also detects a same-UID modification made
        // before sealing; subsequent pathname/permission changes cannot alter
        // these sealed bytes. The readback has the same fixed-buffer bound.
        let mut sealed_digest = Sha256::new();
        let mut verified = 0usize;
        loop {
            let read_limit = buffer.len().min(copied.saturating_sub(verified) + 1);
            let count = read_retry(&mut readonly, &mut buffer[..read_limit])
                .map_err(|_| "cannot verify sealed producer executable snapshot")?;
            if count == 0 {
                break;
            }
            if count > copied.saturating_sub(verified) {
                return Err("sealed producer snapshot size differs from copied bytes");
            }
            sealed_digest.update(&buffer[..count]);
            verified += count;
        }
        let sealed_hash: [u8; 32] = sealed_digest.finalize().into();
        if verified != copied || sealed_hash != actual {
            return Err("sealed producer snapshot differs from copied bytes");
        }
        drop(output);
        Ok(Executable {
            path: descriptor_path(&readonly),
            _file: readonly,
            hash: sealed_hash,
        })
    }

    fn create_memfd() -> Result<File, &'static str> {
        // MFD_EXEC is explicit. A kernel/policy that does not support or allow
        // executable memfds makes this experimental route unavailable.
        // SAFETY: the fixed name is NUL terminated; flags are valid constants.
        let descriptor = unsafe {
            libc::memfd_create(
                c"oxid-hir-producer".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | libc::MFD_EXEC,
            )
        };
        if descriptor < 0 {
            return Err("cannot create executable memfd producer snapshot");
        }
        // SAFETY: successful memfd_create returned an exclusively owned fd.
        let file = unsafe { File::from_raw_fd(descriptor) };
        file.set_permissions(Permissions::from_mode(0o600))
            .map_err(|_| "cannot set producer snapshot build permissions")?;
        avoid_stdio(file)
    }

    fn avoid_stdio(file: File) -> Result<File, &'static str> {
        if file.as_raw_fd() >= 3 {
            return Ok(file);
        }
        // SAFETY: this live owned descriptor is duplicated at fd >= 3 with
        // CLOEXEC. The original remains owned and is closed by ordinary Drop.
        let descriptor = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
        if descriptor < 0 {
            return Err("cannot retain producer snapshot outside standard descriptors");
        }
        // SAFETY: fcntl returned a fresh, valid descriptor owned only here.
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }

    fn descriptor_path(file: &File) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
    }

    fn read_retry(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match reader.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => return result,
            }
        }
    }

    fn check_elf(header: &[u8; ELF_HEADER_LEN], copied: usize) -> Result<(), &'static str> {
        // This is a target/header check, not a complete ELF verifier. The OS
        // loader can still reject malformed program headers or missing loaders.
        if copied < ELF_HEADER_LEN
            || &header[..4] != b"\x7fELF"
            || header[4] != 2 // ELFCLASS64
            || header[5] != 1 // ELFDATA2LSB
            || header[6] != 1 // EV_CURRENT
            || !matches!(u16::from_le_bytes([header[16], header[17]]), 2 | 3)
            || u16::from_le_bytes([header[18], header[19]]) != 62 // EM_X86_64
            || header[20..24] != [1, 0, 0, 0]
            || u16::from_le_bytes([header[52], header[53]]) != ELF_HEADER_LEN as u16
        {
            return Err("producer executable must have a Linux x86_64 ELF64 header");
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::{symlink, FileExt, FileTypeExt, MetadataExt};

        struct Fixture {
            _owner: Workspace,
            root: PathBuf,
            temporary: PathBuf,
            parser: Vec<u8>,
            static_consumer: Vec<u8>,
        }

        impl Fixture {
            fn new() -> Self {
                let owner = create_workspace(&std::env::temp_dir()).unwrap();
                let root = owner.path.join("input");
                let temporary = owner.path.join("snapshots");
                fs::create_dir(&root).unwrap();
                fs::create_dir(&temporary).unwrap();
                let parser = fake_elf(91);
                let static_consumer = fake_elf(105);
                fs::write(root.join("parser"), &parser).unwrap();
                fs::write(root.join("static"), &static_consumer).unwrap();
                fs::write(
                    root.join("manifest.txt"),
                    manifest_bytes(&parser, &static_consumer),
                )
                .unwrap();
                Self {
                    _owner: owner,
                    root,
                    temporary,
                    parser,
                    static_consumer,
                }
            }

            fn load(&self) -> Result<Bundle, &'static str> {
                load(&self.root, &self.temporary, EXECUTABLE_LIMIT)
            }

            fn assert_no_snapshots(&self) {
                assert_eq!(fs::read_dir(&self.temporary).unwrap().count(), 0);
                assert!(self.root.join("manifest.txt").exists());
            }
        }

        // These deliberately incomplete ELF-shaped byte fixtures exercise
        // validation/ownership only. They are not native execution evidence.
        fn fake_elf(length: usize) -> Vec<u8> {
            assert!(length >= ELF_HEADER_LEN);
            let mut bytes = vec![0u8; length];
            bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
            bytes[16..18].copy_from_slice(&2u16.to_le_bytes());
            bytes[18..20].copy_from_slice(&62u16.to_le_bytes());
            bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
            bytes[52..54].copy_from_slice(&(ELF_HEADER_LEN as u16).to_le_bytes());
            bytes
        }

        fn digest(bytes: &[u8]) -> [u8; 32] {
            Sha256::digest(bytes).into()
        }

        fn hex(bytes: &[u8]) -> String {
            bytes.iter().map(|byte| format!("{byte:02x}")).collect()
        }

        fn manifest_bytes(parser: &[u8], static_consumer: &[u8]) -> Vec<u8> {
            format!(
                "OXID-HIR-PRODUCERS-1\nparser {}\nstatic {}\n",
                hex(&digest(parser)),
                hex(&digest(static_consumer))
            )
            .into_bytes()
        }

        #[test]
        fn exact_manifest_grammar_and_known_sha256() {
            let known = b"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
            assert_eq!(decode_hash(known).unwrap(), digest(b"abc"));
            for (version, protocol) in [(b'1', Protocol::V1), (b'2', Protocol::V2)] {
                let mut bytes = manifest_bytes(b"abc", b"other");
                bytes[MAGIC.len() - 2] = version;
                assert_eq!(bytes.len(), MANIFEST_LEN);
                assert_eq!(
                    parse_manifest(&bytes).unwrap(),
                    Manifest {
                        protocol,
                        parser: digest(b"abc"),
                        static_consumer: digest(b"other"),
                    }
                );
            }
        }

        #[test]
        fn rejects_unsupported_manifest_versions() {
            let mut bytes = manifest_bytes(b"abc", b"other");
            for version in u8::MIN..=u8::MAX {
                if matches!(version, b'1' | b'2') {
                    continue;
                }
                bytes[MAGIC.len() - 2] = version;
                assert!(parse_manifest(&bytes).is_err(), "version byte {version}");
            }
        }

        #[test]
        fn rejects_noncanonical_manifest_variants() {
            for version in *b"12" {
                let mut valid = manifest_bytes(b"abc", b"other");
                valid[MAGIC.len() - 2] = version;
                let text = String::from_utf8(valid.clone()).unwrap();
                let magic = format!("PRODUCERS-{}", char::from(version));
                let variants = [
                    text.replace(&magic, "PRODUCERS-"),
                    text.replace(&magic, "PRODUCERS-01"),
                    text.replace(&magic, "PRODUCERS-02"),
                    text.replace(&magic, "PRODUCERS-12"),
                    text.replace(&magic, "PRODUCERS-21"),
                    text.replace("parser ", "parser\t"),
                    text.replace("parser ", "static "),
                    text.replace("static ", "parser "),
                    text.replace('\n', "\r\n"),
                    text.trim_end().to_owned(),
                    format!("{text}\n"),
                    format!("{text}ignored"),
                    text.replace("ba7816", "BA7816"),
                    text.replace("ba7816", "ga7816"),
                    text.replace("d9298a", "D9298A"),
                    text.replace("d9298a", "g9298a"),
                    text.replace("parser ", "parser ../"),
                    text.replace("parser ", "parser  "),
                ];
                for variant in variants {
                    assert!(parse_manifest(variant.as_bytes()).is_err(), "{variant:?}");
                }
                for offset in [MAGIC.len() + 7, MAGIC.len() + 72 + 7] {
                    let mut nul = valid.clone();
                    nul[offset] = 0;
                    assert!(parse_manifest(&nul).is_err());
                }
            }
        }

        #[test]
        fn manifest_read_rejects_trailing_bytes_and_large_tail() {
            let fixture = Fixture::new();
            let mut bytes = manifest_bytes(&fixture.parser, &fixture.static_consumer);
            bytes.extend_from_slice(&[b'x'; 4096]);
            fs::write(fixture.root.join("manifest.txt"), bytes).unwrap();
            assert!(fixture.load().is_err());
            fixture.assert_no_snapshots();
        }

        #[test]
        fn verified_snapshots_are_private_readonly_and_owned() {
            let fixture = Fixture::new();
            let bundle = fixture.load().unwrap();
            assert_eq!(bundle.parser_hash(), digest(&fixture.parser));
            assert_eq!(bundle.static_hash(), digest(&fixture.static_consumer));
            assert_eq!(bundle.parser(), descriptor_path(&bundle.parser._file));
            assert_eq!(
                bundle.static_consumer(),
                descriptor_path(&bundle.static_consumer._file)
            );
            assert_eq!(fs::read_dir(bundle.directory()).unwrap().count(), 0);
            assert!(bundle.directory().starts_with(&fixture.temporary));
            assert_eq!(
                fs::metadata(bundle.directory())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            for path in [bundle.parser(), bundle.static_consumer()] {
                assert_eq!(
                    fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o500
                );
            }
            for snapshot in [&bundle.parser, &bundle.static_consumer] {
                assert!(snapshot._file.as_raw_fd() >= 3);
                // SAFETY: both queries use the currently owned descriptor.
                assert_ne!(
                    unsafe { libc::fcntl(snapshot._file.as_raw_fd(), libc::F_GETFD) }
                        & libc::FD_CLOEXEC,
                    0
                );
                assert_eq!(
                    unsafe { libc::fcntl(snapshot._file.as_raw_fd(), libc::F_GET_SEALS) }
                        & REQUIRED_SEALS,
                    REQUIRED_SEALS
                );
            }
            let directory = bundle.directory().to_path_buf();
            drop(bundle);
            assert!(!directory.exists());
            fixture.assert_no_snapshots();
            assert_eq!(
                fs::read(fixture.root.join("parser")).unwrap(),
                fixture.parser
            );
            assert_eq!(
                fs::read(fixture.root.join("static")).unwrap(),
                fixture.static_consumer
            );
        }

        #[test]
        fn snapshots_survive_input_replacement_and_removal() {
            let fixture = Fixture::new();
            let bundle = fixture.load().unwrap();
            fs::write(fixture.root.join("replacement"), b"replacement").unwrap();
            fs::rename(
                fixture.root.join("replacement"),
                fixture.root.join("parser"),
            )
            .unwrap();
            fs::remove_file(fixture.root.join("static")).unwrap();
            // An earlier same-UID producer may freely replace/chmod filenames
            // in its cwd. Neither descriptor-backed executable resolves there.
            fs::write(bundle.directory().join("parser"), b"cwd replacement").unwrap();
            fs::write(bundle.directory().join("static"), b"cwd replacement").unwrap();
            fs::set_permissions(
                bundle.directory().join("static"),
                Permissions::from_mode(0o700),
            )
            .unwrap();
            assert_eq!(fs::read(bundle.parser()).unwrap(), fixture.parser);
            assert_eq!(
                fs::read(bundle.static_consumer()).unwrap(),
                fixture.static_consumer
            );
            assert_eq!(
                digest(&fs::read(bundle.parser()).unwrap()),
                bundle.parser_hash()
            );
        }

        #[test]
        fn snapshot_streams_and_hashes_multiple_buffer_lengths() {
            let fixture = Fixture::new();
            let mut parser = fake_elf(8192 * 2 + 127);
            for (index, byte) in parser[ELF_HEADER_LEN..].iter_mut().enumerate() {
                *byte = (index % 251) as u8;
            }
            fs::write(fixture.root.join("parser"), &parser).unwrap();
            fs::write(
                fixture.root.join("manifest.txt"),
                manifest_bytes(&parser, &fixture.static_consumer),
            )
            .unwrap();
            let bundle = load(&fixture.root, &fixture.temporary, parser.len()).unwrap();
            assert_eq!(bundle.parser_hash(), digest(&parser));
            assert_eq!(fs::read(bundle.parser()).unwrap(), parser);
        }

        #[test]
        fn rejects_hash_mismatch_and_cleans_partial_success() {
            for name in ["parser", "static"] {
                let fixture = Fixture::new();
                let mut bytes = fs::read(fixture.root.join(name)).unwrap();
                bytes[64] = 1;
                fs::write(fixture.root.join(name), bytes).unwrap();
                assert!(fixture.load().unwrap_err().contains("digest"));
                fixture.assert_no_snapshots();
                assert!(fixture.root.join("parser").exists());
                assert!(fixture.root.join("static").exists());
            }
        }

        #[test]
        fn rejects_symlinks_for_each_fixed_input() {
            for name in ["manifest.txt", "parser", "static"] {
                let fixture = Fixture::new();
                let destination = fixture.root.join(format!("original-{name}"));
                fs::rename(fixture.root.join(name), &destination).unwrap();
                symlink(&destination, fixture.root.join(name)).unwrap();
                assert!(fixture.load().is_err());
                fixture.assert_no_snapshots();
                assert!(destination.exists());
            }
        }

        #[test]
        fn rejects_symlink_or_nondirectory_bundle_root() {
            let fixture = Fixture::new();
            let link = fixture.temporary.join("link");
            symlink(&fixture.root, &link).unwrap();
            for spelling in [
                link.clone(),
                PathBuf::from(format!("{}/", link.display())),
                link.join("."),
                PathBuf::from(format!("{}//./", link.display())),
            ] {
                assert!(load(&spelling, &fixture.temporary, EXECUTABLE_LIMIT).is_err());
            }
            assert!(load(
                &fixture.root.join("parser"),
                &fixture.temporary,
                EXECUTABLE_LIMIT
            )
            .is_err());
        }

        #[test]
        fn rejects_directories_and_device_files() {
            for name in ["manifest.txt", "parser", "static"] {
                let fixture = Fixture::new();
                fs::remove_file(fixture.root.join(name)).unwrap();
                fs::create_dir(fixture.root.join(name)).unwrap();
                assert!(fixture.load().is_err());
                fixture.assert_no_snapshots();
            }
            let device = Path::new("/dev/null");
            assert!(fs::symlink_metadata(device)
                .unwrap()
                .file_type()
                .is_char_device());
            assert!(open_regular_at(&open_directory(Path::new("/dev")).unwrap(), c"null").is_err());
        }

        #[test]
        fn rejects_wrong_elf_target_and_truncated_header() {
            let fixture = Fixture::new();
            for (offset, value) in [
                (0, 0),
                (4, 1),
                (5, 2),
                (6, 0),
                (16, 1),
                (18, 3),
                (20, 0),
                (52, 0),
            ] {
                let mut parser = fixture.parser.clone();
                parser[offset] = value;
                fs::write(fixture.root.join("parser"), &parser).unwrap();
                fs::write(
                    fixture.root.join("manifest.txt"),
                    manifest_bytes(&parser, &fixture.static_consumer),
                )
                .unwrap();
                assert!(fixture.load().unwrap_err().contains("ELF64"));
                fixture.assert_no_snapshots();
            }
            for parser in [&fixture.parser[..63], &fixture.parser[..0]] {
                fs::write(fixture.root.join("parser"), parser).unwrap();
                fs::write(
                    fixture.root.join("manifest.txt"),
                    manifest_bytes(parser, &fixture.static_consumer),
                )
                .unwrap();
                assert!(fixture.load().unwrap_err().contains("ELF64"));
                fixture.assert_no_snapshots();
            }
        }

        #[test]
        fn executable_limit_is_inclusive_and_cleans_oversized_second_file() {
            let fixture = Fixture::new();
            let parser = fake_elf(64);
            let static_consumer = fake_elf(65);
            fs::write(fixture.root.join("parser"), &parser).unwrap();
            fs::write(fixture.root.join("static"), &static_consumer).unwrap();
            fs::write(
                fixture.root.join("manifest.txt"),
                manifest_bytes(&parser, &static_consumer),
            )
            .unwrap();
            assert!(load(&fixture.root, &fixture.temporary, 64)
                .unwrap_err()
                .contains("size limit"));
            fixture.assert_no_snapshots();
            let bundle = load(&fixture.root, &fixture.temporary, 65).unwrap();
            assert_eq!(fs::read(bundle.static_consumer()).unwrap().len(), 65);
        }

        #[test]
        fn descriptor_root_cannot_be_redirected_by_directory_replacement() {
            let fixture = Fixture::new();
            let root = open_directory(&fixture.root).unwrap();
            let moved = fixture.temporary.join("original-input");
            fs::rename(&fixture.root, &moved).unwrap();
            fs::create_dir(&fixture.root).unwrap();
            fs::write(fixture.root.join("parser"), b"replacement").unwrap();
            fs::write(fixture.root.join("manifest.txt"), b"replacement").unwrap();
            assert_eq!(
                read_manifest(&root).unwrap().parser,
                digest(&fixture.parser)
            );
            let executable = snapshot(
                open_regular_at(&root, c"parser").unwrap(),
                digest(&fixture.parser),
                EXECUTABLE_LIMIT,
            )
            .unwrap();
            assert_eq!(fs::read(executable.path).unwrap(), fixture.parser);
        }

        #[test]
        fn sealed_bytes_resist_reopened_descriptor_writes_and_resize() {
            let fixture = Fixture::new();
            let bundle = fixture.load().unwrap();
            for (path, expected) in [
                (bundle.parser(), &fixture.parser),
                (bundle.static_consumer(), &fixture.static_consumer),
            ] {
                // A same-UID process can restore write permission. The seals,
                // rather than mode bits, must still block every mutation.
                fs::set_permissions(path, Permissions::from_mode(0o700)).unwrap();
                let reopened = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)
                    .unwrap();
                for result in [
                    reopened.write_at(b"replacement", 0).map(|_| ()),
                    reopened.set_len((expected.len() - 1) as u64),
                    reopened.set_len((expected.len() + 1) as u64),
                ] {
                    assert_eq!(result.unwrap_err().raw_os_error(), Some(libc::EPERM));
                }
                assert_eq!(fs::read(path).unwrap(), *expected);
                fs::set_permissions(path, Permissions::from_mode(0o500)).unwrap();
            }
        }

        #[test]
        fn executable_seals_and_execute_permission_cannot_be_relaxed() {
            let fixture = Fixture::new();
            let bundle = fixture.load().unwrap();
            fs::set_permissions(bundle.parser(), Permissions::from_mode(0o700)).unwrap();
            let reopened = OpenOptions::new()
                .read(true)
                .write(true)
                .open(bundle.parser())
                .unwrap();
            // SAFETY: reopened is a live descriptor; these operations only
            // attempt changes to its already-finalized seal mask.
            let before = unsafe { libc::fcntl(reopened.as_raw_fd(), libc::F_GET_SEALS) };
            for attempted in [0, libc::F_SEAL_WRITE, libc::F_SEAL_FUTURE_WRITE] {
                assert_eq!(
                    unsafe { libc::fcntl(reopened.as_raw_fd(), libc::F_ADD_SEALS, attempted) },
                    -1
                );
                assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::EPERM));
            }
            assert_eq!(
                unsafe { libc::fcntl(reopened.as_raw_fd(), libc::F_GET_SEALS) },
                before
            );
            assert_eq!(before & REQUIRED_SEALS, REQUIRED_SEALS);
            assert_eq!(
                reopened
                    .set_permissions(Permissions::from_mode(0o400))
                    .unwrap_err()
                    .raw_os_error(),
                Some(libc::EPERM)
            );
        }

        #[test]
        fn owned_descriptors_and_workspace_are_released_on_drop() {
            let fixture = Fixture::new();
            let bundle = fixture.load().unwrap();
            let identities: Vec<_> = [bundle.parser(), bundle.static_consumer()]
                .into_iter()
                .map(|path| {
                    let metadata = fs::metadata(path).unwrap();
                    (path.to_path_buf(), metadata.dev(), metadata.ino())
                })
                .collect();
            let workspace = bundle.directory().to_path_buf();
            drop(bundle);
            assert!(!workspace.exists());
            for (path, device, inode) in identities {
                // Parallel tests may immediately reuse the fd number. It must
                // no longer designate the owned memfd after Bundle is dropped.
                if let Ok(metadata) = fs::metadata(path) {
                    assert_ne!((metadata.dev(), metadata.ino()), (device, inode));
                }
            }
        }

        #[test]
        fn cleanup_preserves_sibling_and_input_files() {
            let fixture = Fixture::new();
            let sibling = fixture.temporary.join("keep");
            fs::write(&sibling, b"unrelated").unwrap();
            let bundle = fixture.load().unwrap();
            let private = bundle.directory().to_path_buf();
            drop(bundle);
            assert!(!private.exists());
            assert_eq!(fs::read(sibling).unwrap(), b"unrelated");
            assert!(fixture.root.join("manifest.txt").exists());
        }

        #[test]
        fn workspace_paths_are_absolute() {
            let workspace = create_workspace(Path::new(".")).unwrap();
            assert!(workspace.path.is_absolute());
            let private = workspace.path.clone();
            drop(workspace);
            assert!(!private.exists());
        }

        #[test]
        fn public_loader_uses_only_private_snapshot_paths() {
            let fixture = Fixture::new();
            let bundle = Bundle::load(&fixture.root).unwrap();
            assert_ne!(bundle.directory(), fixture.root);
            assert!(!bundle.parser().starts_with(&fixture.root));
            assert!(!bundle.static_consumer().starts_with(&fixture.root));
        }
    }
}
