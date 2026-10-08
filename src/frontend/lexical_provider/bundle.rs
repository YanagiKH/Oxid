//! Closed local lexical-provider bundles and owned executable snapshots.
//!
//! Digests assert byte identity, not provenance. The Linux x86_64 execution
//! route accepts only a sealed memfd copy, with no pathname-execution fallback.
use std::{
    fs::File,
    path::{Path, PathBuf},
};

pub(super) const EXECUTABLE_MAX: usize = 16 * 1024 * 1024;

/// Fixed named carriers and bounded bundle-loading scratch. Dynamic retained
/// workspace-path capacity is reported separately by Bundle::named_bytes.
pub(super) fn named_bytes() -> usize {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::named_bytes()
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        std::mem::size_of::<Bundle>()
    }
}

#[derive(Debug)]
pub(super) struct Bundle {
    workspace: Workspace,
    lexer: Executable,
}

#[derive(Debug)]
struct Executable {
    file: File,
    hash: [u8; 32],
    bytes: usize,
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
            Err("lexical provider bundles require Linux x86_64")
        }
    }
    pub(super) fn directory(&self) -> &File {
        &self.workspace.file
    }
    pub(super) fn lexer(&self) -> &File {
        &self.lexer.file
    }
    pub(super) fn lexer_hash(&self) -> [u8; 32] {
        self.lexer.hash
    }
    pub(super) fn executable_bytes(&self) -> usize {
        self.lexer.bytes
    }
    /// Retained dynamic path storage; the caller accounts for Bundle itself.
    pub(super) fn named_bytes(&self) -> usize {
        self.workspace.path.capacity()
    }
}

/// The path was created exclusively; the descriptor fixes the working-directory
/// identity even if the directory's name is subsequently replaced.
#[derive(Debug)]
struct Workspace {
    path: PathBuf,
    file: File,
}
impl Drop for Workspace {
    fn drop(&mut self) {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            use std::os::unix::fs::MetadataExt;
            // Check ownership before attempting nonrecursive cleanup. An
            // executable can populate its private cwd, so leave any nonempty
            // directory untouched rather than traverse executable-created
            // contents after supervision ends. This is not a sandbox or a
            // recursively bounded cleanup guarantee.
            let owned = self.file.metadata();
            let named = std::fs::symlink_metadata(&self.path);
            if let (Ok(owned), Ok(named)) = (owned, named) {
                if named.is_dir() && (owned.dev(), owned.ino()) == (named.dev(), named.ino()) {
                    let _ = std::fs::remove_dir(&self.path);
                }
            }
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::{Bundle, Executable, Workspace};
    use sha2::{Digest, Sha256};
    use std::ffi::{CStr, OsStr};
    use std::fmt::Write as FmtWrite;
    use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
    use std::io::{self, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    };
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub(super) const EXECUTABLE_LIMIT: usize = super::EXECUTABLE_MAX;
    const MAGIC: &[u8] = b"oxid-lexical-provider-v1\n";
    const HASH_PREFIX: &[u8] = b"lexer_sha256=";
    const MANIFEST_LEN: usize = MAGIC.len() + HASH_PREFIX.len() + 64 + 1;
    const ELF_HEADER_LEN: usize = 64;
    const COPY_BUFFER_BYTES: usize = 8192;
    const PATH_BYTES: usize = 4096;
    const WORKSPACE_NAME_BYTES: usize = 96;

    pub(super) fn named_bytes() -> usize {
        // Sum named carriers conservatively even when their lifetimes do not
        // overlap. This is not an allocator/RSS or OS filesystem-memory claim.
        std::mem::size_of::<Bundle>() + COPY_BUFFER_BYTES + ELF_HEADER_LEN
            + MANIFEST_LEN + 1 + 3 * 32
            + 2 * std::mem::size_of::<Sha256>()
            + 4 * std::mem::size_of::<File>()
            + 4 * std::mem::size_of::<std::fs::Metadata>()
            + 4 * std::mem::size_of::<PathBuf>()
            + std::mem::size_of::<String>()
            + std::mem::size_of::<libc::stat>()
            + std::mem::size_of::<DescriptorPath>() + 10
            + 32 * std::mem::size_of::<usize>()
            // Fixed normalized path plus checked canonical-parent capacity
            // and exact fallible temporary name allocation.
            + 2 * PATH_BYTES + WORKSPACE_NAME_BYTES
    }
    const REQUIRED_SEALS: libc::c_int = libc::F_SEAL_WRITE
        | libc::F_SEAL_GROW
        | libc::F_SEAL_SHRINK
        | libc::F_SEAL_SEAL
        | libc::F_SEAL_EXEC;
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    pub(super) fn load(
        root: &Path,
        temporary_parent: &Path,
        executable_limit: usize,
    ) -> Result<Bundle, &'static str> {
        let executable_limit = executable_limit.min(EXECUTABLE_LIMIT);
        let root = open_directory(root)?;
        let declared = read_manifest(&root)?;
        let workspace = create_workspace(temporary_parent)?;
        let lexer = snapshot(
            open_regular_at(&root, c"lexer")?,
            declared,
            executable_limit,
        )?;
        Ok(Bundle { workspace, lexer })
    }

    fn open_directory(path: &Path) -> Result<File, &'static str> {
        if path.as_os_str().len() > PATH_BYTES {
            return Err("lexical provider bundle path exceeds fixed capacity");
        }
        // Lexically remove trailing slashes and dot components without resolving
        // links. Otherwise link/ and link/. bypass O_NOFOLLOW on the final link.
        let mut normalized = [0u8; PATH_BYTES];
        let mut used = 0usize;
        for component in path.components() {
            let bytes = component.as_os_str().as_bytes();
            let separator = usize::from(used != 0 && normalized[used - 1] != b'/');
            if bytes.len() + separator > normalized.len() - used {
                return Err("lexical provider bundle path exceeds fixed capacity");
            }
            if separator != 0 {
                normalized[used] = b'/';
                used += 1;
            }
            normalized[used..used + bytes.len()].copy_from_slice(bytes);
            used += bytes.len();
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(Path::new(OsStr::from_bytes(&normalized[..used])))
            .map_err(|_| {
                "lexical provider bundle root must be an accessible nonsymlink directory"
            })?;
        if !file
            .metadata()
            .map_err(|_| "cannot inspect opened lexical provider bundle directory")?
            .is_dir()
        {
            return Err("opened lexical provider bundle root must be a directory");
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
            return Err("cannot inspect lexical provider bundle file");
        }
        if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
            return Err("lexical provider bundle files must be regular nonsymlink files");
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
            return Err("cannot safely open lexical provider bundle file");
        }
        // SAFETY: successful openat returned a new descriptor owned only here.
        let file = unsafe { File::from_raw_fd(descriptor) };
        let opened = file
            .metadata()
            .map_err(|_| "cannot inspect opened lexical provider file")?;
        if !opened.file_type().is_file() {
            return Err("opened lexical provider file must be regular");
        }
        if opened.dev() != metadata.st_dev || opened.ino() != metadata.st_ino {
            return Err("lexical provider file changed during directory-relative open");
        }
        Ok(file)
    }

    fn read_manifest(directory: &File) -> Result<[u8; 32], &'static str> {
        let mut file = open_regular_at(directory, c"manifest.txt")?;
        // One extra byte detects trailing data without an unbounded allocation
        // or read, even when the manifest grows after open.
        let mut bytes = [0u8; MANIFEST_LEN + 1];
        let mut used = 0;
        while used < bytes.len() {
            let count = read_retry(&mut file, &mut bytes[used..])
                .map_err(|_| "cannot read lexical provider bundle manifest")?;
            if count == 0 {
                break;
            }
            used += count;
        }
        parse_manifest(&bytes[..used])
    }

    fn parse_manifest(bytes: &[u8]) -> Result<[u8; 32], &'static str> {
        const INVALID: &str = "lexical provider manifest does not match the closed format";
        if bytes.len() != MANIFEST_LEN || !bytes.starts_with(MAGIC) {
            return Err(INVALID);
        }
        let line = &bytes[MAGIC.len()..];
        if !line.starts_with(HASH_PREFIX) || line[HASH_PREFIX.len() + 64] != b'\n' {
            return Err(INVALID);
        }
        decode_hash(&line[HASH_PREFIX.len()..HASH_PREFIX.len() + 64]).ok_or(INVALID)
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
        if parent.as_os_str().len() > PATH_BYTES {
            return Err("private lexical provider parent path exceeds fixed capacity");
        }
        // TMPDIR may be relative. Snapshot paths must remain absolute when the
        // supervisor selects a different working directory for the processes.
        let parent = fs::canonicalize(parent)
            .map_err(|_| "cannot resolve private lexical provider workspace parent")?;
        if parent.capacity() > PATH_BYTES {
            return Err("private lexical provider parent path exceeds fixed capacity");
        }
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // Name unpredictability is not the isolation boundary: atomic mkdir
        // must succeed, and mode 0700 is applied at creation, never afterward.
        for _ in 0..128 {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let mut name = String::new();
            name.try_reserve_exact(WORKSPACE_NAME_BYTES)
                .map_err(|_| "cannot allocate private lexical provider workspace name")?;
            if name.capacity() != WORKSPACE_NAME_BYTES {
                return Err("private lexical provider workspace name allocation is not exact");
            }
            // The fixed prefix, 32-bit PID, 128-bit timestamp and 64-bit
            // sequence occupy at most 95 bytes, including separators.
            write!(
                &mut name,
                "oxid-lexical-provider-{}-{timestamp}-{sequence}",
                std::process::id()
            )
            .map_err(|_| "cannot format private lexical provider workspace name")?;
            let capacity = parent.as_os_str().len() + 1 + name.len();
            if capacity > PATH_BYTES {
                return Err("private lexical provider workspace path exceeds fixed capacity");
            }
            let mut path = PathBuf::new();
            path.try_reserve_exact(capacity)
                .map_err(|_| "cannot allocate private lexical provider workspace path")?;
            if path.capacity() != capacity {
                return Err("private lexical provider workspace path allocation is not exact");
            }
            path.push(&parent);
            path.push(name);
            match DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => {
                    let file = match open_directory(&path).and_then(avoid_stdio) {
                        Ok(file) => file,
                        Err(error) => {
                            let _ = fs::remove_dir(&path);
                            return Err(error);
                        }
                    };
                    return Ok(Workspace { path, file });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err("cannot create private lexical provider workspace"),
            }
        }
        Err("cannot allocate a unique private lexical provider workspace")
    }

    fn snapshot(
        mut input: File,
        declared: [u8; 32],
        limit: usize,
    ) -> Result<Executable, &'static str> {
        if input
            .metadata()
            .map_err(|_| "cannot inspect lexical provider executable size")?
            .len()
            > limit as u64
        {
            return Err("lexical provider executable exceeds size limit");
        }
        let mut output = create_memfd()?;
        let mut buffer = [0u8; COPY_BUFFER_BYTES];
        let mut header = [0u8; ELF_HEADER_LEN];
        let mut copied = 0usize;
        let mut digest = Sha256::new();
        loop {
            // Read at most the remaining allowance plus one overflow byte.
            // The fixed stack buffer also bounds memory for large executables.
            let read_limit = buffer.len().min(limit.saturating_sub(copied) + 1);
            let count = read_retry(&mut input, &mut buffer[..read_limit])
                .map_err(|_| "cannot read lexical provider executable")?;
            if count == 0 {
                break;
            }
            if count > limit.saturating_sub(copied) {
                return Err("lexical provider executable exceeds size limit");
            }
            if copied < header.len() {
                let header_count = count.min(header.len() - copied);
                header[copied..copied + header_count].copy_from_slice(&buffer[..header_count]);
            }
            output
                .write_all(&buffer[..count])
                .map_err(|_| "cannot write private lexical provider snapshot")?;
            digest.update(&buffer[..count]);
            copied += count;
        }
        check_elf(&header, copied)?;
        let actual: [u8; 32] = digest.finalize().into();
        if actual != declared {
            return Err("lexical provider executable digest does not match the bundle manifest");
        }
        output
            .flush()
            .map_err(|_| "cannot flush private lexical provider snapshot")?;
        output
            .set_permissions(Permissions::from_mode(0o500))
            .map_err(|_| "cannot make private lexical provider snapshot executable")?;
        // Seals protect the bytes even from another process with the same UID
        // that chmods/reopens /proc/<pid>/fd/<fd>. F_SEAL_EXEC also freezes the
        // executable bits. No permission/unsupported-kernel fallback is used.
        // SAFETY: output owns a live memfd and fcntl receives integer arguments.
        if unsafe { libc::fcntl(output.as_raw_fd(), libc::F_ADD_SEALS, REQUIRED_SEALS) } < 0 {
            return Err("cannot seal lexical provider executable snapshot");
        }
        let seals = unsafe { libc::fcntl(output.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 || seals & REQUIRED_SEALS != REQUIRED_SEALS {
            return Err("lexical provider executable snapshot seals are incomplete");
        }
        // Reopen this kernel-owned descriptor link read-only, then close the
        // writable handle before execution. This is not a bundle/workspace
        // pathname and cannot be redirected by renaming any user-owned file.
        let readonly = File::open(descriptor_path(&output).as_path())
            .map_err(|_| "cannot retain sealed lexical provider executable snapshot")?;
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
                .map_err(|_| "cannot verify sealed lexical provider executable snapshot")?;
            if count == 0 {
                break;
            }
            if count > copied.saturating_sub(verified) {
                return Err("sealed lexical provider snapshot size differs from copied bytes");
            }
            sealed_digest.update(&buffer[..count]);
            verified += count;
        }
        let sealed_hash: [u8; 32] = sealed_digest.finalize().into();
        if verified != copied || sealed_hash != actual {
            return Err("sealed lexical provider snapshot differs from copied bytes");
        }
        drop(output);
        Ok(Executable {
            file: readonly,
            hash: sealed_hash,
            bytes: copied,
        })
    }

    fn create_memfd() -> Result<File, &'static str> {
        // MFD_EXEC is explicit. A kernel/policy that does not support or allow
        // executable memfds makes this experimental route unavailable.
        // SAFETY: the fixed name is NUL terminated; flags are valid constants.
        let descriptor = unsafe {
            libc::memfd_create(
                c"oxid-lexical-provider".as_ptr(),
                libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | libc::MFD_EXEC,
            )
        };
        if descriptor < 0 {
            return Err("cannot create executable memfd lexical provider snapshot");
        }
        // SAFETY: successful memfd_create returned an exclusively owned fd.
        let file = unsafe { File::from_raw_fd(descriptor) };
        file.set_permissions(Permissions::from_mode(0o600))
            .map_err(|_| "cannot set lexical provider snapshot build permissions")?;
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
            return Err("cannot retain lexical provider snapshot outside standard descriptors");
        }
        // SAFETY: fcntl returned a fresh, valid descriptor owned only here.
        Ok(unsafe { File::from_raw_fd(descriptor) })
    }

    struct DescriptorPath {
        bytes: [u8; 32],
        len: usize,
    }
    impl DescriptorPath {
        fn as_path(&self) -> &Path {
            Path::new(OsStr::from_bytes(&self.bytes[..self.len]))
        }
    }
    impl AsRef<Path> for DescriptorPath {
        fn as_ref(&self) -> &Path {
            self.as_path()
        }
    }
    fn descriptor_path(file: &File) -> DescriptorPath {
        let mut path = DescriptorPath {
            bytes: [0; 32],
            len: 0,
        };
        let prefix = b"/proc/self/fd/";
        path.bytes[..prefix.len()].copy_from_slice(prefix);
        let mut value = file.as_raw_fd() as u32;
        let mut reversed = [0; 10];
        let mut count = 0;
        loop {
            reversed[count] = b'0' + (value % 10) as u8;
            count += 1;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        for index in 0..count {
            path.bytes[prefix.len() + index] = reversed[count - index - 1];
        }
        path.len = prefix.len() + count;
        path
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
            return Err("lexical provider executable must have a Linux x86_64 ELF64 header");
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::{symlink, FileExt};

        struct Fixture {
            _owner: Workspace,
            root: PathBuf,
            temporary: PathBuf,
            lexer: [u8; 91],
        }
        impl Fixture {
            fn new() -> Self {
                let owner = create_workspace(&std::env::temp_dir()).unwrap();
                let root = owner.path.join("input");
                let temporary = owner.path.join("snapshots");
                fs::create_dir(&root).unwrap();
                fs::create_dir(&temporary).unwrap();
                let lexer = fake_elf::<91>();
                fs::write(root.join("lexer"), lexer).unwrap();
                fs::write(root.join("manifest.txt"), manifest_bytes(&lexer)).unwrap();
                Self {
                    _owner: owner,
                    root,
                    temporary,
                    lexer,
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
        impl Drop for Fixture {
            fn drop(&mut self) {
                // Only these fixed test-created entries belong to the fixture.
                // Workspace itself deliberately leaves nonempty directories.
                for directory in [&self.root, &self.temporary.join("original-input")] {
                    for name in ["manifest.txt", "lexer", "original"] {
                        let _ = fs::remove_file(directory.join(name));
                    }
                    let _ = fs::remove_dir(directory);
                }
                let _ = fs::remove_file(self.temporary.join("link"));
                let _ = fs::remove_dir(&self.temporary);
            }
        }
        // These ELF-shaped bytes validate the closed bundle and seals only.
        // Actual ELF execution is exercised by supervisor's native helpers.
        fn fake_elf<const N: usize>() -> [u8; N] {
            assert!(N >= ELF_HEADER_LEN);
            let mut bytes = [0; N];
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
        fn manifest_bytes(bytes: &[u8]) -> [u8; MANIFEST_LEN] {
            let mut manifest = [0; MANIFEST_LEN];
            manifest[..MAGIC.len()].copy_from_slice(MAGIC);
            manifest[MAGIC.len()..MAGIC.len() + HASH_PREFIX.len()].copy_from_slice(HASH_PREFIX);
            const HEX: &[u8] = b"0123456789abcdef";
            for (index, byte) in digest(bytes).iter().enumerate() {
                let at = MAGIC.len() + HASH_PREFIX.len() + index * 2;
                manifest[at] = HEX[usize::from(byte >> 4)];
                manifest[at + 1] = HEX[usize::from(byte & 15)];
            }
            manifest[MANIFEST_LEN - 1] = b'\n';
            manifest
        }
        #[test]
        fn lexical_bundle_cleanup_removes_empty_owned_workspace() {
            let workspace = create_workspace(&std::env::temp_dir()).unwrap();
            let path = workspace.path.clone();
            assert!(path.is_dir());
            drop(workspace);
            assert!(!path.exists());
        }
        #[test]
        fn lexical_bundle_cleanup_retains_populated_owned_workspace_and_link_targets() {
            let owner = create_workspace(&std::env::temp_dir()).unwrap();
            let workspace = create_workspace(&owner.path).unwrap();
            let path = workspace.path.clone();
            let local = path.join("keep");
            let target = owner.path.join("target");
            let target_file = target.join("keep");
            let link = path.join("link");
            fs::write(&local, b"local contents").unwrap();
            fs::create_dir(&target).unwrap();
            fs::write(&target_file, b"target contents").unwrap();
            symlink(&target, &link).unwrap();

            drop(workspace);
            assert!(path.is_dir());
            assert_eq!(fs::read(&local).unwrap(), b"local contents");
            assert!(fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink());
            assert_eq!(fs::read_link(&link).unwrap(), target);
            assert_eq!(fs::read(&target_file).unwrap(), b"target contents");

            // Test-owned fixture cleanup names every entry; it never traverses
            // the link or an executable-created directory tree.
            fs::remove_file(&link).unwrap();
            fs::remove_file(&local).unwrap();
            fs::remove_dir(&path).unwrap();
            fs::remove_file(&target_file).unwrap();
            fs::remove_dir(&target).unwrap();
        }
        #[test]
        fn lexical_bundle_exact_manifest_and_known_sha256() {
            assert_eq!(
                decode_hash(b"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
                    .unwrap(),
                digest(b"abc")
            );
            let valid = manifest_bytes(b"abc");
            assert_eq!(parse_manifest(&valid).unwrap(), digest(b"abc"));
            for (at, value) in [
                (0, b'O'),
                (MAGIC.len() - 2, b'2'),
                (MAGIC.len(), b'p'),
                (MAGIC.len() + HASH_PREFIX.len(), b'B'),
                (MAGIC.len() + HASH_PREFIX.len(), b'g'),
                (MAGIC.len() + HASH_PREFIX.len(), 0),
                (MANIFEST_LEN - 1, b'\r'),
            ] {
                let mut changed = valid;
                changed[at] = value;
                assert!(parse_manifest(&changed).is_err());
            }
            for len in 0..MANIFEST_LEN {
                assert!(parse_manifest(&valid[..len]).is_err());
            }
            let mut trailing = [0; MANIFEST_LEN + 1];
            trailing[..MANIFEST_LEN].copy_from_slice(&valid);
            assert!(parse_manifest(&trailing).is_err());
        }
        #[test]
        fn lexical_bundle_manifest_overflow_is_bounded() {
            let f = Fixture::new();
            let mut file = OpenOptions::new()
                .append(true)
                .open(f.root.join("manifest.txt"))
                .unwrap();
            file.write_all(&[b'x'; 8192]).unwrap();
            assert!(f.load().is_err());
            f.assert_no_snapshots();
        }
        #[test]
        fn lexical_bundle_rejects_symlinks_directories_and_fifo() {
            for name in ["manifest.txt", "lexer"] {
                let f = Fixture::new();
                let original = f.root.join("original");
                fs::rename(f.root.join(name), &original).unwrap();
                symlink(&original, f.root.join(name)).unwrap();
                assert!(f.load().is_err());
                f.assert_no_snapshots();
                fs::remove_file(f.root.join(name)).unwrap();
                fs::create_dir(f.root.join(name)).unwrap();
                assert!(f.load().is_err());
                f.assert_no_snapshots();
                fs::remove_dir(f.root.join(name)).unwrap();
                let root = open_directory(&f.root).unwrap();
                let name = if name == "lexer" {
                    c"lexer"
                } else {
                    c"manifest.txt"
                };
                // SAFETY: live directory and fixed NUL-terminated basename.
                assert_eq!(
                    unsafe { libc::mkfifoat(root.as_raw_fd(), name.as_ptr(), 0o600) },
                    0
                );
                assert!(f.load().is_err());
            }
        }
        #[test]
        fn lexical_bundle_rejects_root_link_spellings() {
            let f = Fixture::new();
            let link = f.temporary.join("link");
            symlink(&f.root, &link).unwrap();
            for spelling in [
                link.clone(),
                link.join("."),
                PathBuf::from(format!("{}//./", link.display())),
            ] {
                assert!(load(&spelling, &f.temporary, EXECUTABLE_LIMIT).is_err());
            }
            assert!(load(&f.root.join("lexer"), &f.temporary, EXECUTABLE_LIMIT).is_err());
        }
        #[test]
        fn lexical_bundle_checks_digest_elf_and_inclusive_size() {
            let f = Fixture::new();
            let mut changed = f.lexer;
            changed[64] = 1;
            fs::write(f.root.join("lexer"), changed).unwrap();
            assert!(f.load().unwrap_err().contains("digest"));
            f.assert_no_snapshots();
            for (at, value) in [
                (0, 0),
                (4, 1),
                (5, 2),
                (6, 0),
                (16, 1),
                (18, 3),
                (20, 0),
                (52, 0),
            ] {
                let mut changed = f.lexer;
                changed[at] = value;
                fs::write(f.root.join("lexer"), changed).unwrap();
                fs::write(f.root.join("manifest.txt"), manifest_bytes(&changed)).unwrap();
                assert!(f.load().unwrap_err().contains("ELF64"));
                f.assert_no_snapshots();
            }
            fs::write(f.root.join("lexer"), f.lexer).unwrap();
            fs::write(f.root.join("manifest.txt"), manifest_bytes(&f.lexer)).unwrap();
            assert!(load(&f.root, &f.temporary, 90)
                .unwrap_err()
                .contains("size limit"));
            f.assert_no_snapshots();
            let bundle = load(&f.root, &f.temporary, 91).unwrap();
            assert_eq!(bundle.executable_bytes(), 91);
            drop(bundle);
            f.assert_no_snapshots();
            fs::write(f.root.join("lexer"), &f.lexer[..63]).unwrap();
            fs::write(f.root.join("manifest.txt"), manifest_bytes(&f.lexer[..63])).unwrap();
            assert!(f.load().unwrap_err().contains("ELF64"));
        }
        #[test]
        fn lexical_bundle_snapshots_stream_and_survive_path_replacement() {
            let f = Fixture::new();
            let mut bytes = fake_elf::<16511>();
            for (i, byte) in bytes[64..].iter_mut().enumerate() {
                *byte = (i % 251) as u8;
            }
            fs::write(f.root.join("lexer"), bytes).unwrap();
            fs::write(f.root.join("manifest.txt"), manifest_bytes(&bytes)).unwrap();
            let bundle = f.load().unwrap();
            fs::remove_file(f.root.join("lexer")).unwrap();
            fs::write(f.root.join("lexer"), b"replacement").unwrap();
            assert_eq!(fs::read(descriptor_path(bundle.lexer())).unwrap(), bytes);
            assert_eq!(bundle.lexer_hash(), digest(&bytes));
            assert!(bundle.lexer().as_raw_fd() >= 3 && bundle.directory().as_raw_fd() >= 3);
            assert_eq!(
                bundle.directory().metadata().unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                bundle.lexer().metadata().unwrap().permissions().mode() & 0o777,
                0o500
            );
            assert_eq!(bundle.named_bytes(), bundle.workspace.path.capacity());
            let private = bundle.workspace.path.clone();
            drop(bundle);
            assert!(!private.exists());
            f.assert_no_snapshots();
        }
        #[test]
        fn lexical_bundle_seals_forbid_mutation_and_permission_relaxation() {
            let f = Fixture::new();
            let bundle = f.load().unwrap();
            let path = descriptor_path(bundle.lexer());
            fs::set_permissions(&path, Permissions::from_mode(0o700)).unwrap();
            let reopened = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            for result in [
                reopened.write_at(b"changed", 0).map(|_| ()),
                reopened.set_len(1),
                reopened.set_len(999),
            ] {
                assert_eq!(result.unwrap_err().raw_os_error(), Some(libc::EPERM));
            }
            assert_eq!(
                reopened
                    .set_permissions(Permissions::from_mode(0o400))
                    .unwrap_err()
                    .raw_os_error(),
                Some(libc::EPERM)
            );
            // SAFETY: seal/descriptor queries use a currently owned fd.
            assert_eq!(
                unsafe { libc::fcntl(reopened.as_raw_fd(), libc::F_GET_SEALS) } & REQUIRED_SEALS,
                REQUIRED_SEALS
            );
            assert_ne!(
                unsafe { libc::fcntl(bundle.lexer().as_raw_fd(), libc::F_GETFD) }
                    & libc::FD_CLOEXEC,
                0
            );
            assert_eq!(fs::read(path).unwrap(), f.lexer);
        }
        #[test]
        fn lexical_bundle_directory_relative_identity_and_cleanup_preserve_replacements() {
            let f = Fixture::new();
            let root = open_directory(&f.root).unwrap();
            let moved = f.temporary.join("original-input");
            fs::rename(&f.root, &moved).unwrap();
            fs::create_dir(&f.root).unwrap();
            fs::write(f.root.join("manifest.txt"), b"replacement").unwrap();
            assert_eq!(read_manifest(&root).unwrap(), digest(&f.lexer));
            let executable = snapshot(
                open_regular_at(&root, c"lexer").unwrap(),
                digest(&f.lexer),
                EXECUTABLE_LIMIT,
            )
            .unwrap();
            assert_eq!(
                fs::read(descriptor_path(&executable.file)).unwrap(),
                f.lexer
            );
            let bundle = load(&moved, &f.temporary, EXECUTABLE_LIMIT).unwrap();
            let old = bundle.workspace.path.clone();
            let renamed = f.temporary.join("renamed-private");
            fs::rename(&old, &renamed).unwrap();
            fs::create_dir(&old).unwrap();
            fs::write(old.join("keep"), b"keep").unwrap();
            assert_eq!(
                bundle.directory().metadata().unwrap().ino(),
                fs::metadata(&renamed).unwrap().ino()
            );
            drop(bundle);
            assert_eq!(fs::read(old.join("keep")).unwrap(), b"keep");
            fs::remove_file(old.join("keep")).unwrap();
            fs::remove_dir(&old).unwrap();
            fs::remove_dir(&renamed).unwrap();
        }
    }
}
