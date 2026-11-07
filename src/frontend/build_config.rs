//! Resolved native settings and presentation names for retained project inputs.
//! Neither type changes source identity, ownership, or the original file route.

use super::{
    diagnostic::Diagnostic,
    source::{SourceFileId, SourceMap},
};
use std::path::{Component, Components, Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Llvm,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Host,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeOptimization {
    O0,
    O2,
}

impl NativeOptimization {
    pub(crate) fn from_profile(profile: &str) -> Option<Self> {
        match profile {
            "bounded" | "dev" => Some(Self::O0),
            "release" => Some(Self::O2),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExecutionPolicy {
    Bounded,
    Hosted,
    HostedWithHeapLimit(u64),
}

impl ExecutionPolicy {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "bounded" => Some(Self::Bounded),
            "hosted" => Some(Self::Hosted),
            _ => None,
        }
    }

    pub(crate) const fn hosted(self) -> bool {
        matches!(self, Self::Hosted | Self::HostedWithHeapLimit(_))
    }

    pub(crate) const DEFAULT_HEAP_LIMIT: u64 = 16 * 1024 * 1024;

    pub(crate) const fn heap_limit(self) -> u64 {
        match self {
            Self::HostedWithHeapLimit(limit) => limit,
            Self::Bounded | Self::Hosted => Self::DEFAULT_HEAP_LIMIT,
        }
    }

    pub(in crate::frontend) const fn initial_fuel(self, bounded: usize) -> usize {
        if self.hosted() {
            HOSTED_FUEL
        } else {
            bounded
        }
    }
}

// Only explicit hosted entry points install this marker, after ordinary
// Limits::bounded admission. It is a mode, never a large finite program budget.
pub(in crate::frontend) const HOSTED_FUEL: usize = usize::MAX;
/// Single selection for the emitted ABI, object format and linker family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeTarget {
    LinuxX64,
    LinuxArm64,
    DarwinX64,
    DarwinArm64,
    WindowsMsvcX64,
}
impl NativeTarget {
    const fn host() -> Self {
        if NativeBuildSettings::windows_msvc_host() {
            Self::WindowsMsvcX64
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Self::DarwinArm64
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            Self::DarwinX64
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Self::LinuxArm64
        } else {
            // Unsupported hosts retain their original presentation. Admission
            // rejects them before tools, directories or native artifacts.
            Self::LinuxX64
        }
    }
    const fn triple(self) -> &'static str {
        match self {
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
            Self::LinuxArm64 => "aarch64-unknown-linux-gnu",
            Self::DarwinX64 => "x86_64-apple-darwin",
            Self::DarwinArm64 => "aarch64-apple-darwin",
            Self::WindowsMsvcX64 => "x86_64-pc-windows-msvc",
        }
    }
}

macro_rules! native_header {
    ($kind:literal, $source:literal, $triple:literal) => {
        concat!("; Oxid ", $kind, " native ABI 1\nsource_filename = \"", $source,
            "\"\ntarget triple = \"", $triple,
            "\"\n\ndeclare i32 @__oxid_print_bool(i32)\ndeclare i32 @__oxid_print_i32(i32)\ndeclare i32 @__oxid_print_unit()\ndeclare void @__oxid_overflow(ptr, i64) noreturn\ndeclare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32)\ndeclare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)\n")
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SourcePaths {
    Original,
    ProjectRelative,
}

/// Only implemented native states can be represented. Edition, entry policy,
/// and cache orchestration are deliberately separate from this immutable plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NativeBuildSettings {
    backend: Backend,
    target: Target,
    optimization: NativeOptimization,
    execution: ExecutionPolicy,
    source_paths: SourcePaths,
    debug_info: bool,
}

impl NativeBuildSettings {
    pub(crate) const fn new(reproducible: bool) -> Self {
        Self::with_optimization(reproducible, NativeOptimization::O0)
    }

    pub(crate) const fn with_optimization(
        reproducible: bool,
        optimization: NativeOptimization,
    ) -> Self {
        Self {
            backend: Backend::Llvm,
            target: Target::Host,
            optimization,
            execution: ExecutionPolicy::Bounded,
            debug_info: false,
            source_paths: if reproducible {
                SourcePaths::ProjectRelative
            } else {
                SourcePaths::Original
            },
        }
    }

    pub(crate) const fn target_triple(self) -> &'static str {
        match self.target {
            Target::Host => NativeTarget::host().triple(),
        }
    }

    pub(crate) const fn windows_msvc_host() -> bool {
        cfg!(all(
            target_os = "windows",
            target_env = "msvc",
            target_arch = "x86_64",
            target_pointer_width = "64",
            target_endian = "little"
        ))
    }

    pub(crate) const fn host_supported() -> bool {
        Self::windows_msvc_host()
            || cfg!(all(
                any(target_os = "linux", target_os = "macos"),
                target_pointer_width = "64",
                target_endian = "little",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))
    }

    pub(crate) const fn linker() -> &'static str {
        match NativeTarget::host() {
            NativeTarget::WindowsMsvcX64 => "lld-link.exe",
            NativeTarget::DarwinX64 | NativeTarget::DarwinArm64 => "ld64.lld",
            NativeTarget::LinuxX64 | NativeTarget::LinuxArm64 => "ld.lld",
        }
    }

    pub(crate) const fn program_object() -> &'static str {
        if Self::windows_msvc_host() {
            "program.obj"
        } else {
            "program.o"
        }
    }
    pub(crate) const fn runtime_object() -> &'static str {
        if Self::windows_msvc_host() {
            "runtime.obj"
        } else {
            "runtime.o"
        }
    }
    pub(crate) const fn staged_executable() -> &'static str {
        if Self::windows_msvc_host() {
            "executable.exe"
        } else {
            "executable"
        }
    }
    pub(crate) fn executable_name(name: &str) -> String {
        if Self::windows_msvc_host() {
            format!("{name}.exe")
        } else {
            name.to_owned()
        }
    }
    pub(crate) fn executable_stem(name: &str) -> Option<&str> {
        let stem = if Self::windows_msvc_host() {
            name.strip_suffix(".exe")?
        } else {
            name
        };
        (!stem.is_empty()
            && stem
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            && !(cfg!(windows) && Self::windows_device_name(stem)))
        .then_some(stem)
    }
    pub(crate) fn windows_device_name(name: &str) -> bool {
        let upper = name.to_ascii_uppercase();
        matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (upper.len() == 4
                && (upper.starts_with("COM") || upper.starts_with("LPT"))
                && matches!(upper.as_bytes()[3], b'1'..=b'9'))
    }

    pub(in crate::frontend) const fn ir_header(owned: bool) -> &'static str {
        match (NativeTarget::host(), owned) {
            (NativeTarget::LinuxX64, false) => native_header!(
                "experimental scalar",
                "oxid-native",
                "x86_64-unknown-linux-gnu"
            ),
            (NativeTarget::LinuxX64, true) => native_header!(
                "private owned",
                "oxid-owned-native",
                "x86_64-unknown-linux-gnu"
            ),
            (NativeTarget::LinuxArm64, false) => native_header!(
                "experimental scalar",
                "oxid-native",
                "aarch64-unknown-linux-gnu"
            ),
            (NativeTarget::LinuxArm64, true) => native_header!(
                "private owned",
                "oxid-owned-native",
                "aarch64-unknown-linux-gnu"
            ),
            (NativeTarget::DarwinX64, false) => {
                native_header!("experimental scalar", "oxid-native", "x86_64-apple-darwin")
            }
            (NativeTarget::DarwinX64, true) => {
                native_header!("private owned", "oxid-owned-native", "x86_64-apple-darwin")
            }
            (NativeTarget::DarwinArm64, false) => {
                native_header!("experimental scalar", "oxid-native", "aarch64-apple-darwin")
            }
            (NativeTarget::DarwinArm64, true) => {
                native_header!("private owned", "oxid-owned-native", "aarch64-apple-darwin")
            }
            (NativeTarget::WindowsMsvcX64, false) => native_header!(
                "experimental scalar",
                "oxid-native",
                "x86_64-pc-windows-msvc"
            ),
            (NativeTarget::WindowsMsvcX64, true) => native_header!(
                "private owned",
                "oxid-owned-native",
                "x86_64-pc-windows-msvc"
            ),
        }
    }

    pub(crate) const fn debug_info_supported() -> bool {
        cfg!(target_os = "linux")
    }

    pub(crate) const fn elf_machine() -> u16 {
        if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            183 // EM_AARCH64
        } else {
            62 // EM_X86_64
        }
    }

    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    pub(crate) const fn elf_interpreter() -> &'static str {
        if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            "ld-linux-aarch64.so.1"
        } else {
            "ld-linux-x86-64.so.2"
        }
    }

    pub(crate) const fn optimization_flag(self) -> &'static str {
        match (self.backend, self.optimization) {
            (Backend::Llvm, NativeOptimization::O0) => "-O0",
            (Backend::Llvm, NativeOptimization::O2) => "-O2",
        }
    }

    pub(crate) const fn with_execution(mut self, execution: ExecutionPolicy) -> Self {
        self.execution = execution;
        self
    }

    pub(crate) const fn execution(self) -> ExecutionPolicy {
        self.execution
    }

    pub(crate) const fn with_debug_info(mut self, enabled: bool) -> Self {
        self.debug_info = enabled;
        self
    }

    pub(crate) const fn debug_info(self) -> bool {
        self.debug_info
    }

    pub(crate) const fn optimized(self) -> bool {
        matches!(self.optimization, NativeOptimization::O2)
    }

    pub(crate) const fn reproducible(self) -> bool {
        matches!(self.source_paths, SourcePaths::ProjectRelative)
    }
}

/// The borrowed owner is an exact association witness, not a structural map
/// comparison. Original presentation keeps using the old allocation-free path;
/// only project-relative builds construct this bounded transient sidecar.
#[derive(Debug)]
pub(crate) struct NativeSourceNames<'s> {
    sources: &'s SourceMap,
    names: Vec<String>,
}

#[derive(Clone, Copy)]
struct NameLimits {
    files: usize,
    path_bytes: usize,
    retained_bytes: usize,
}

const NAME_LIMITS: NameLimits = NameLimits {
    files: 256,
    path_bytes: 16 * 1024,
    retained_bytes: 4 * 1024 * 1024,
};

fn failure(message: &'static str) -> Box<Diagnostic> {
    Diagnostic::new("E0700", "native-admission", message, None)
}

fn budget() -> Box<Diagnostic> {
    failure("native source presentation budget exceeded")
}

impl<'s> NativeSourceNames<'s> {
    /// Paths are borrowed from the already-admitted source owner. This method
    /// performs lexical conversion only: no metadata, canonicalize, or reopen.
    pub(in crate::frontend) fn project_relative<'p>(
        sources: &'s SourceMap,
        logical_root: &Path,
        paths: impl IntoIterator<Item = (SourceFileId, &'p Path)>,
    ) -> Result<Self, Box<Diagnostic>> {
        Self::with_limits(sources, logical_root, paths, NAME_LIMITS)
    }

    fn with_limits<'p>(
        sources: &'s SourceMap,
        logical_root: &Path,
        paths: impl IntoIterator<Item = (SourceFileId, &'p Path)>,
        limits: NameLimits,
    ) -> Result<Self, Box<Diagnostic>> {
        normalized_absolute(logical_root, limits.path_bytes)?;
        let files = sources.files().len();
        if files > limits.files {
            return Err(budget());
        }
        let mut retained = files.checked_mul(size_of::<String>()).ok_or_else(budget)?;
        if retained > limits.retained_bytes {
            return Err(budget());
        }
        let mut names = Vec::new();
        names.try_reserve_exact(files).map_err(|_| budget())?;
        // Account actual capacity, including otherwise-unused vector slots.
        if names.capacity() != files {
            return Err(budget());
        }
        names.resize_with(files, String::new);
        let mut observed = 0usize;
        for (file, path) in paths {
            let name = names.get_mut(file.0).ok_or_else(|| {
                failure("native source presentation contains an unknown source ID")
            })?;
            if !name.is_empty() {
                return Err(failure(
                    "native source presentation contains a duplicate source ID",
                ));
            }
            let relative = RelativePath::new(logical_root, path, limits.path_bytes)?;
            retained = retained.checked_add(relative.bytes).ok_or_else(budget)?;
            if retained > limits.retained_bytes {
                return Err(budget());
            }
            name.try_reserve_exact(relative.bytes)
                .map_err(|_| budget())?;
            if name.capacity() != relative.bytes {
                return Err(budget());
            }
            relative.write(name)?;
            observed += 1;
        }
        if observed != files {
            return Err(failure(
                "native source presentation is missing a retained source ID",
            ));
        }
        Ok(Self { sources, names })
    }

    pub(in crate::frontend) fn validate(&self, sources: &SourceMap) -> Result<(), Box<Diagnostic>> {
        if !std::ptr::eq(self.sources, sources) {
            return Err(Diagnostic::new(
                "E0500",
                "native-admission",
                "native source presentation belongs to a different source map",
                None,
            ));
        }
        if self.names.len() != sources.files().len() || self.names.iter().any(String::is_empty) {
            return Err(Diagnostic::new(
                "E0500",
                "native-admission",
                "native source presentation is missing a retained source ID",
                None,
            ));
        }
        Ok(())
    }

    /// Lookups never allocate or fall back to an absolute path. Entry-point
    /// validation binds this table to the source map before counting/emission.
    pub(in crate::frontend) fn path(&self, file: SourceFileId) -> Option<&str> {
        self.names.get(file.0).map(String::as_str)
    }
}

/// Compare components against the original spelling because Path::components
/// otherwise removes repeated separators and interior `.` components silently.
fn normalized_absolute(path: &Path, limit: usize) -> Result<(), Box<Diagnostic>> {
    let text = path
        .to_str()
        .ok_or_else(|| failure("native source presentation paths must be UTF-8"))?;
    if text.len() > limit {
        return Err(budget());
    }
    if !path.is_absolute() {
        return Err(failure(
            "native source presentation requires absolute canonical paths",
        ));
    }
    let mut remaining = text;
    let mut separator = false;
    for part in path.components() {
        match part {
            Component::Prefix(_) | Component::RootDir => {}
            Component::Normal(_) => {
                if separator {
                    remaining = remaining
                        .strip_prefix(std::path::MAIN_SEPARATOR)
                        .ok_or_else(|| {
                            failure("native source presentation path is not normalized")
                        })?;
                }
            }
            _ => return Err(failure("native source presentation path is not normalized")),
        }
        remaining = remaining
            .strip_prefix(
                part.as_os_str()
                    .to_str()
                    .ok_or_else(|| failure("native source presentation paths must be UTF-8"))?,
            )
            .ok_or_else(|| failure("native source presentation path is not normalized"))?;
        separator = matches!(part, Component::Normal(_));
    }
    if !remaining.is_empty() {
        return Err(failure("native source presentation path is not normalized"));
    }
    Ok(())
}

struct RelativePath<'p> {
    parents: usize,
    suffix: Components<'p>,
    bytes: usize,
}

impl<'p> RelativePath<'p> {
    fn new(root: &Path, path: &'p Path, limit: usize) -> Result<Self, Box<Diagnostic>> {
        normalized_absolute(path, limit)?;
        let common = root
            .components()
            .zip(path.components())
            .take_while(|(left, right)| left == right)
            .count();
        let mut suffix = path.components();
        for _ in 0..common {
            suffix.next();
        }
        let parents = root.components().skip(common).count();
        if common == 0
            || root
                .components()
                .skip(common)
                .any(|part| !matches!(part, Component::Normal(_)))
            || suffix.clone().next().is_none()
        {
            return Err(failure(
                "native source presentation cannot relate source and logical root",
            ));
        }
        let mut bytes = parents.checked_mul(3).ok_or_else(budget)?;
        for (index, part) in suffix.clone().enumerate() {
            let Component::Normal(name) = part else {
                return Err(failure(
                    "native source presentation cannot relate source and logical root",
                ));
            };
            bytes = bytes
                .checked_add(usize::from(index != 0))
                .and_then(|total| total.checked_add(name.len()))
                .ok_or_else(budget)?;
        }
        if bytes > limit {
            return Err(budget());
        }
        Ok(Self {
            parents,
            suffix,
            bytes,
        })
    }

    fn write(self, output: &mut String) -> Result<(), Box<Diagnostic>> {
        for _ in 0..self.parents {
            output.push_str("../");
        }
        for (index, part) in self.suffix.enumerate() {
            if index != 0 {
                output.push('/');
            }
            output.push_str(
                part.as_os_str()
                    .to_str()
                    .ok_or_else(|| failure("native source presentation paths must be UTF-8"))?,
            );
        }
        if output.len() != self.bytes {
            return Err(budget());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn root() -> PathBuf {
        std::env::temp_dir().join("oxid-native-names")
    }

    fn child(root: &Path, relative: &str) -> PathBuf {
        relative
            .split('/')
            .fold(root.to_path_buf(), |path, component| path.join(component))
    }

    fn map(paths: &[PathBuf]) -> SourceMap {
        let mut sources = SourceMap::new();
        for path in paths {
            sources.add(path.to_str().unwrap().into(), "fn main()->i32{0}".into());
        }
        sources
    }

    fn relative_names<'a>(
        sources: &'a SourceMap,
        root: &Path,
        paths: &[PathBuf],
    ) -> Result<NativeSourceNames<'a>, Box<Diagnostic>> {
        NativeSourceNames::project_relative(
            sources,
            root,
            paths
                .iter()
                .enumerate()
                .map(|(i, path)| (SourceFileId(i), path.as_path())),
        )
    }

    #[test]
    fn settings_resolve_only_the_implemented_native_states() {
        for reproducible in [false, true] {
            let settings = NativeBuildSettings::new(reproducible);
            let expected = if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
                ("aarch64-unknown-linux-gnu", 183, "ld-linux-aarch64.so.1")
            } else {
                ("x86_64-unknown-linux-gnu", 62, "ld-linux-x86-64.so.2")
            };
            let expected_target = match (std::env::consts::OS, std::env::consts::ARCH) {
                ("windows", "x86_64") if cfg!(target_env = "msvc") => "x86_64-pc-windows-msvc",
                ("macos", "aarch64") => "aarch64-apple-darwin",
                ("macos", "x86_64") => "x86_64-apple-darwin",
                _ => expected.0,
            };
            assert_eq!(settings.target_triple(), expected_target);
            assert_eq!(NativeBuildSettings::elf_machine(), expected.1);
            #[cfg(all(
                target_os = "linux",
                target_pointer_width = "64",
                target_endian = "little",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ))]
            assert_eq!(NativeBuildSettings::elf_interpreter(), expected.2);
            assert_eq!(settings.optimization_flag(), "-O0");
            assert_eq!(settings.reproducible(), reproducible);
        }
    }

    #[test]
    fn native_profiles_change_only_optimization_and_aliases_share_settings() {
        for reproducible in [false, true] {
            let original = NativeBuildSettings::new(reproducible);
            for profile in ["bounded", "dev", "release"] {
                let settings = NativeBuildSettings::with_optimization(
                    reproducible,
                    NativeOptimization::from_profile(profile).unwrap(),
                );
                assert_eq!(settings.backend, original.backend);
                assert_eq!(settings.target, original.target);
                assert_eq!(settings.execution, original.execution);
                assert_eq!(settings.source_paths, original.source_paths);
                if profile == "release" {
                    assert_ne!(settings, original);
                    assert_eq!(settings.optimization_flag(), "-O2");
                } else {
                    assert_eq!(settings, original);
                }
            }
        }
    }

    #[test]
    fn native_profiles_reject_unsupported_optimization_and_execution_names() {
        for profile in ["", "O0", "O2", "debug", "fast", "hosted", "Release", " dev"] {
            assert_eq!(NativeOptimization::from_profile(profile), None, "{profile}");
        }
    }

    #[test]
    fn relative_names_preserve_layout_unicode_and_exact_source_map_association() {
        let base = root();
        let logical = base.join("workspace");
        let paths = [
            child(&logical, "members/app/src/main.ox"),
            child(&base, "共享/src/lib.ox"),
        ];
        let sources = map(&paths);
        let names = relative_names(&sources, &logical, &paths).unwrap();
        assert_eq!(names.path(SourceFileId(0)), Some("members/app/src/main.ox"));
        assert_eq!(names.path(SourceFileId(1)), Some("../共享/src/lib.ox"));
        assert_eq!(names.path(SourceFileId(2)), None);
        names.validate(&sources).unwrap();
        let identical = map(&paths);
        let error = names.validate(&identical).unwrap_err();
        assert_eq!(error.code, "E0500");
        assert!(error.message.contains("different source map"));
        assert_eq!(
            sources.get(SourceFileId(1)).path(),
            paths[1].to_str().unwrap()
        );
    }

    #[test]
    fn missing_duplicate_and_unknown_ids_have_no_partial_name_fallback() {
        let logical = root();
        let paths = [logical.join("main.ox"), logical.join("other.ox")];
        let sources = map(&paths);
        for (ids, expected) in [
            (vec![0], "missing"),
            (vec![0, 0], "duplicate"),
            (vec![0, 2], "unknown"),
        ] {
            let error = NativeSourceNames::project_relative(
                &sources,
                &logical,
                ids.iter().map(|&id| (SourceFileId(id), paths[0].as_path())),
            )
            .unwrap_err();
            assert!(error.message.contains(expected), "{error:?}");
        }
        let mut names = relative_names(&sources, &logical, &paths).unwrap();
        names.names.pop();
        assert!(names
            .validate(&sources)
            .unwrap_err()
            .message
            .contains("missing"));
    }

    #[test]
    fn path_conversion_rejects_noncanonical_and_non_utf8_spellings() {
        let logical = root();
        let paths = [logical.join("main.ox")];
        let sources = map(&paths);
        for path in [
            PathBuf::from("relative.ox"),
            logical.join("../main.ox"),
            logical.join("./main.ox"),
            logical.join("nested//main.ox"),
            logical.clone(),
        ] {
            assert!(relative_names(&sources, &logical, &[path]).is_err());
        }
        assert!(relative_names(&sources, Path::new("relative-root"), &paths).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let path = logical.join(std::ffi::OsString::from_vec(vec![0xff]));
            assert!(relative_names(&sources, &logical, &[path])
                .unwrap_err()
                .message
                .contains("UTF-8"));
        }
    }

    #[test]
    fn sidecar_accounts_carriers_and_text_at_exact_retained_and_path_limits() {
        let logical = root();
        let paths = [logical.join("main.ox"), logical.join("other.ox")];
        let sources = map(&paths);
        let needed = 2 * size_of::<String>() + "main.ox".len() + "other.ox".len();
        let exact = NameLimits {
            files: 2,
            path_bytes: 16 * 1024,
            retained_bytes: needed,
        };
        let make = |limits| {
            NativeSourceNames::with_limits(
                &sources,
                &logical,
                paths
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (SourceFileId(i), p.as_path())),
                limits,
            )
        };
        let names = make(exact).unwrap();
        assert_eq!(
            names.names.capacity() * size_of::<String>()
                + names.names.iter().map(String::capacity).sum::<usize>(),
            needed
        );
        for limits in [
            NameLimits {
                retained_bytes: needed - 1,
                ..exact
            },
            NameLimits { files: 1, ..exact },
            NameLimits {
                path_bytes: paths[1].as_os_str().len() - 1,
                ..exact
            },
        ] {
            assert!(make(limits).unwrap_err().message.contains("budget"));
        }
        // Relative expansion can exceed the limit even when both inputs fit.
        let logical = PathBuf::from(format!("/{}", "a/".repeat(12).trim_end_matches('/')));
        let paths = [PathBuf::from("/other/main.ox")];
        let sources = map(&paths);
        if logical.is_absolute() {
            let error = NativeSourceNames::with_limits(
                &sources,
                &logical,
                [(SourceFileId(0), paths[0].as_path())],
                NameLimits {
                    path_bytes: logical.as_os_str().len(),
                    ..NAME_LIMITS
                },
            )
            .unwrap_err();
            assert!(error.message.contains("budget"));
        }
    }

    #[test]
    fn relocation_and_unix_backslash_preserve_distinct_logical_paths() {
        let first = root().join("first");
        let second = root().join("longer-relocated-root");
        let paths = [child(&first, "app/main.ox"), child(&first, "shared/lib.ox")];
        let relocated = [
            child(&second, "app/main.ox"),
            child(&second, "shared/lib.ox"),
        ];
        let sources = map(&paths);
        let other = map(&relocated);
        let names = relative_names(&sources, &first.join("app"), &paths).unwrap();
        let moved = relative_names(&other, &second.join("app"), &relocated).unwrap();
        assert_eq!(names.names, moved.names);
        #[cfg(unix)]
        {
            let paths = [first.join("dir\\file.ox"), first.join("dir/file.ox")];
            let sources = map(&paths);
            let names = relative_names(&sources, &first, &paths).unwrap();
            assert_eq!(names.path(SourceFileId(0)), Some("dir\\file.ox"));
            assert_eq!(names.path(SourceFileId(1)), Some("dir/file.ox"));
        }
    }
}
