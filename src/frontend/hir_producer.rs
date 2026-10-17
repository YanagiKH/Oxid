//! Explicit two-process observation pipeline. The compiler still owns source,
//! canonical checking and executable authority; producer bytes stay untrusted.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use crate::frontend::project::ModuleId;
use crate::frontend::{declaration_index::SourceOwner, project::ProjectSources};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use sha2::{Digest, Sha256};
use std::{fmt, path::Path};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod bundle;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod supervisor;

// Immutable executable and bounded leader cleanup boundary reviewed at33f11cd.
// Enable only this explicit route for its separate functional qualification.
const PRODUCER_PIPELINE_ADMITTED: bool = true;
const OPA_BYTES: usize = 1559;
const OBSERVATION_BYTES: usize = 2607;
const SOURCE_MAX: usize = 255;
const AST1_MAX: usize = 5 + SOURCE_MAX + OPA_BYTES;
const AST3_MAX: usize = 6 + 511 + 1304;

#[derive(Clone, Copy)]
pub(super) struct Record {
    role: &'static str,
    executable_sha256: [u8; 32],
    input_sha256: [u8; 32],
    stdout_sha256: [u8; 32],
    stderr_sha256: [u8; 32],
    input_written: usize,
    stdout_bytes: usize,
    stderr_bytes: usize,
    status: Option<i32>,
    signal: Option<i32>,
    stop: &'static str,
    spawned: bool,
    leader_reaped: bool,
}

/// Exact observed extent is retained separately from fixed bounded storage.
/// Unused capacity is never submitted to either checked decoder.
pub(super) struct Observation {
    bytes: [u8; OBSERVATION_BYTES],
    len: usize,
}
impl Observation {
    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub(super) struct Outcome {
    pub(super) observation: Option<Observation>,
    pub(super) error: Option<&'static str>,
    pub(super) records: [Option<Record>; 2],
}
impl Outcome {
    fn empty() -> Self {
        Self {
            observation: None,
            error: None,
            records: [None; 2],
        }
    }
    fn reject(mut self, error: &'static str) -> Self {
        self.error = Some(error);
        self
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn record(
    role: &'static str,
    hash: [u8; 32],
    input: &[u8],
    captured: &supervisor::Capture,
) -> Record {
    Record {
        role,
        executable_sha256: hash,
        input_sha256: Sha256::digest(input).into(),
        stdout_sha256: Sha256::digest(&captured.stdout[..captured.stdout_len]).into(),
        stderr_sha256: Sha256::digest(&captured.stderr[..captured.stderr_len]).into(),
        input_written: captured.input_written,
        stdout_bytes: captured.stdout_len,
        stderr_bytes: captured.stderr_len,
        status: captured.code(),
        signal: captured.signal(),
        stop: captured.stop.name(),
        spawned: captured.spawned,
        leader_reaped: captured.status.is_some(),
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn completed(captured: &supervisor::Capture, input_len: usize) -> bool {
    captured.status.is_some()
        && captured.stop == supervisor::Stop::Exited
        && captured.code() == Some(0)
        && captured.input_written == input_len
        && captured.stderr_len == 0
}

pub(super) fn produce(project: &ProjectSources, directory: &Path) -> Outcome {
    let result = Outcome::empty();
    if !PRODUCER_PIPELINE_ADMITTED {
        return result.reject("experimental HIR producer pipeline is not yet admitted");
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (project, directory);
        result.reject("experimental HIR producers require Linux x86_64")
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let mut result = result;
        let owner = SourceOwner::project(project);
        if owner.count() != 1 {
            return result
                .reject("experimental HIR producers require one retained root-only source");
        }
        let Ok(source) = owner.file(ModuleId(0)) else {
            return result.reject("experimental HIR producer source owner is unavailable");
        };
        let input = source.text().as_bytes();
        // Preserve legacy source-error priority before any bundle access.
        // Only bounded ASCII256..511 inputs may ask the existing immutable
        // loader to establish explicit V3 admission; no manifest peek or
        // alternative execution path is introduced.
        if input.len() > 511 || !input.is_ascii() {
            return result
                .reject("experimental HIR producers require at most 255 ASCII source bytes");
        }
        let selected = match bundle::Bundle::load(directory) {
            Ok(bundle) => bundle,
            Err(_) if input.len() > SOURCE_MAX => {
                return result
                    .reject("experimental HIR producers require at most 255 ASCII source bytes")
            }
            Err(message) => return result.reject(message),
        };
        let protocol = selected.protocol();
        // A fully loaded V1/V2 bundle cannot grant the additional source capacity.
        if protocol != super::hir_protocol::Protocol::V3 && input.len() > SOURCE_MAX {
            return result
                .reject("experimental HIR producers require at most 255 ASCII source bytes");
        }
        if input.len() > protocol.source_max() {
            return result.reject(match protocol {
                super::hir_protocol::Protocol::V1 => {
                    "v1 HIR producers require at most 128 ASCII source bytes"
                }
                super::hir_protocol::Protocol::V2 => {
                    "experimental HIR producers require at most 255 ASCII source bytes"
                }
                super::hir_protocol::Protocol::V3 => {
                    "v3 HIR producers require at most 511 ASCII source bytes"
                }
            });
        }
        let parser = supervisor::run_versioned(
            selected.parser(),
            selected.directory(),
            input,
            protocol.opa_bytes(),
            supervisor::DEADLINE,
            protocol,
        );
        result.records[0] = Some(record("parser", selected.parser_hash(), input, &parser));
        if !completed(&parser, input.len()) {
            return result.reject("parser producer failed its bounded process/transport contract");
        }
        let opa_bytes = protocol.opa_bytes();
        let source_length_matches = match protocol {
            super::hir_protocol::Protocol::V3 => {
                usize::from(u16::from_le_bytes([parser.stdout[12], parser.stdout[13]]))
                    == input.len()
            }
            _ => usize::from(parser.stdout[10]) == input.len(),
        };
        if parser.stdout_len != opa_bytes
            || &parser.stdout[..3] != b"OPA"
            || parser.stdout[3] != protocol.digit()
            || !parser.stdout[4..if protocol == super::hir_protocol::Protocol::V3 {
                10
            } else {
                8
            }]
                .iter()
                .all(|byte| *byte == 0)
            || !source_length_matches
        {
            return result.reject(
                "parser producer did not return one complete successful version-matched OPA frame",
            );
        }
        // Each protocol family has its own actual backing array. The legacy
        // frame never pays for the V3 two-byte source length or larger capacity.
        let typed = if protocol == super::hir_protocol::Protocol::V3 {
            let mut ast3 = [0u8; AST3_MAX];
            ast3[..4].copy_from_slice(b"AST3");
            ast3[4..6].copy_from_slice(&(input.len() as u16).to_le_bytes());
            ast3[6..6 + input.len()].copy_from_slice(input);
            ast3[6 + input.len()..6 + input.len() + opa_bytes]
                .copy_from_slice(&parser.stdout[..opa_bytes]);
            let framed = &ast3[..6 + input.len() + opa_bytes];
            let typed = supervisor::run_versioned(
                selected.static_consumer(),
                selected.directory(),
                framed,
                protocol.success_bytes(),
                supervisor::DEADLINE,
                protocol,
            );
            result.records[1] = Some(record("static", selected.static_hash(), framed, &typed));
            typed
        } else {
            let mut ast1 = [0u8; AST1_MAX];
            ast1[..3].copy_from_slice(b"AST");
            ast1[3] = protocol.digit();
            ast1[4] = input.len() as u8;
            ast1[5..5 + input.len()].copy_from_slice(input);
            ast1[5 + input.len()..5 + input.len() + OPA_BYTES]
                .copy_from_slice(&parser.stdout[..OPA_BYTES]);
            let framed = &ast1[..5 + input.len() + OPA_BYTES];
            let typed = supervisor::run_versioned(
                selected.static_consumer(),
                selected.directory(),
                framed,
                OBSERVATION_BYTES,
                supervisor::DEADLINE,
                protocol,
            );
            result.records[1] = Some(record("static", selected.static_hash(), framed, &typed));
            typed
        };
        let framed_len = input.len()
            + opa_bytes
            + if protocol == super::hir_protocol::Protocol::V3 {
                6
            } else {
                5
            };
        if !completed(&typed, framed_len) {
            return result.reject("static producer failed its bounded process/transport contract");
        }
        let tag = typed.stdout[opa_bytes + 4];
        let exact = match tag {
            0 => typed.stdout_len == protocol.success_bytes(),
            1 => typed.stdout_len == protocol.diagnostic_bytes(),
            _ => false,
        };
        if !exact
            || typed.stdout[..opa_bytes] != parser.stdout[..opa_bytes]
            || &typed.stdout[opa_bytes..opa_bytes + 3] != b"STF"
            || typed.stdout[opa_bytes + 3] != protocol.digit()
        {
            return result.reject("static producer did not return one complete unchanged version-matched OPA/STF success or diagnostic observation");
        }
        let mut observation = Observation {
            bytes: [0u8; OBSERVATION_BYTES],
            len: typed.stdout_len,
        };
        observation.bytes[..observation.len].copy_from_slice(&typed.stdout[..typed.stdout_len]);
        result.observation = Some(observation);
        result
        // The verified private executable snapshots drop here, after the
        // supervisor has killed each process group and reaped its leader.
    }
}

struct Hex<'a>(&'a [u8; 32]);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(out, "{byte:02x}")?;
        }
        Ok(())
    }
}
struct Number(Option<i32>);
impl fmt::Display for Number {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(n) => write!(out, "{n}"),
            None => out.write_str("null"),
        }
    }
}

pub(super) fn print_records(outcome: &Outcome, json: bool) {
    for record in outcome.records.iter().flatten() {
        if json {
            println!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"hir-producer\",\"role\":\"{}\",\"executable_sha256\":\"{}\",\"input_sha256\":\"{}\",\"input_written\":{},\"captured_stdout_bytes\":{},\"captured_stdout_sha256\":\"{}\",\"captured_stderr_bytes\":{},\"captured_stderr_sha256\":\"{}\",\"spawned\":{},\"leader_reaped\":{},\"stop\":\"{}\",\"exit_status\":{},\"signal\":{}}}",
                record.role, Hex(&record.executable_sha256), Hex(&record.input_sha256), record.input_written,
                record.stdout_bytes, Hex(&record.stdout_sha256), record.stderr_bytes, Hex(&record.stderr_sha256),
                record.spawned, record.leader_reaped, record.stop, Number(record.status), Number(record.signal));
        } else {
            eprintln!("experimental HIR producer {} sha256={} spawned={} leader_reaped={} stop={} exit={} signal={} captured_stdout={} captured_stderr={}",
                record.role, Hex(&record.executable_sha256), record.spawned, record.leader_reaped, record.stop, Number(record.status), Number(record.signal), record.stdout_bytes, record.stderr_bytes);
        }
    }
}

/// Conservative enclosing Rust carriers, separate from the bounded filesystem
/// copies and external process costs. No total RSS or CPU tariff is claimed.
pub(super) fn named_bytes() -> usize {
    use std::mem::size_of;
    let carriers = 3 * size_of::<Outcome>()
        + 3 * size_of::<Observation>()
        + size_of::<(usize, u8, bool)>()
        + size_of::<[Record; 8]>()
        + 3 * size_of::<[u8; AST1_MAX]>()
        + 2 * size_of::<SourceOwner<'static>>()
        + size_of::<[usize; 16]>()
        + size_of::<[(usize, crate::frontend::hir_protocol::Protocol); 4]>()
        + size_of::<[&[u8]; 8]>();
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        carriers + 3 * size_of::<supervisor::Capture>() + 3 * size_of::<bundle::Bundle>()
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        carriers
    }
}

/// V3 substitutes three actual AST backing roles; all enclosing transport
/// carriers remain unchanged. Scalar temporaries fit the existing role arrays.
pub(super) fn named_bytes_for(protocol: super::hir_protocol::Protocol) -> usize {
    named_bytes()
        + match protocol {
            super::hir_protocol::Protocol::V3 => 3 * (AST3_MAX - AST1_MAX),
            _ => 0,
        }
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use crate::frontend::hir_protocol::Protocol;

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn v3_source_preflight_preserves_legacy_error_priority_without_spawn() {
        use crate::frontend::project::ProjectLimits;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "oxid-v3-source-order-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let absent = root.join("absent");
        let malformed = root.join("malformed");
        std::fs::create_dir(&malformed).unwrap();
        std::fs::write(
            malformed.join("manifest.txt"),
            b"OXID-HIR-PRODUCERS-3\ninvalid\n",
        )
        .unwrap();
        let symlink = root.join("symlink");
        std::os::unix::fs::symlink(&malformed, &symlink).unwrap();
        let mut directories = vec![absent.clone(), malformed.clone(), symlink];
        // These header-shaped files are loader/hash fixtures only. No test
        // grants them execution authority or expects the OS to execute them.
        let mut elf = [0u8; 64];
        elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        elf[16] = 2;
        elf[18] = 62;
        elf[20] = 1;
        elf[52] = 64;
        for version in [1, 2, 3] {
            let directory = root.join(format!("hash-bad-v{version}"));
            std::fs::create_dir(&directory).unwrap();
            for role in ["parser", "static"] {
                std::fs::write(directory.join(role), elf).unwrap();
            }
            std::fs::write(
                directory.join("manifest.txt"),
                format!(
                    "OXID-HIR-PRODUCERS-{version}\nparser {}\nstatic {}\n",
                    "0".repeat(64),
                    "0".repeat(64)
                ),
            )
            .unwrap();
            assert_eq!(
                bundle::Bundle::load(&directory).unwrap_err(),
                "producer executable digest does not match the bundle manifest"
            );
            directories.push(directory);
        }
        for version in [1, 2] {
            let directory = root.join(format!("valid-legacy-v{version}"));
            std::fs::create_dir(&directory).unwrap();
            for role in ["parser", "static"] {
                std::fs::write(directory.join(role), elf).unwrap();
            }
            let hash = format!("{:x}", Sha256::digest(elf));
            std::fs::write(
                directory.join("manifest.txt"),
                format!("OXID-HIR-PRODUCERS-{version}\nparser {hash}\nstatic {hash}\n"),
            )
            .unwrap();
            assert_eq!(
                bundle::Bundle::load(&directory).unwrap().protocol().digit(),
                b'0' + version
            );
            directories.push(directory);
        }
        let base = "fn main()->i32{return 0;}";
        for text in [
            format!("{}{}", " ".repeat(256 - base.len()), base),
            format!("{}{}", " ".repeat(257 - base.len()), base),
            format!("{}{}", " ".repeat(511 - base.len()), base),
            format!("{}{}", " ".repeat(512 - base.len()), base),
            format!("//é\n{base}"),
        ] {
            let file = root.join("main.ox");
            std::fs::write(&file, &text).unwrap();
            let project =
                ProjectSources::load_typed(file.to_str().unwrap(), ProjectLimits::default())
                    .unwrap();
            for directory in &directories {
                let outcome = produce(&project, directory);
                assert_eq!(
                    outcome.error,
                    Some("experimental HIR producers require at most 255 ASCII source bytes")
                );
                assert!(outcome.observation.is_none());
                assert!(outcome.records.iter().all(Option::is_none));
            }
        }
        let file = root.join("main.ox");
        std::fs::write(&file, base).unwrap();
        let project =
            ProjectSources::load_typed(file.to_str().unwrap(), ProjectLimits::default()).unwrap();
        let outcome = produce(&project, &absent);
        assert_eq!(
            outcome.error,
            Some("producer bundle root must be an accessible nonsymlink directory")
        );
        assert!(outcome.records.iter().all(Option::is_none));
    }

    #[test]
    fn v3_ast_roles_are_disjoint_and_legacy_charge_is_unchanged() {
        assert_eq!(AST1_MAX, 1819);
        assert_eq!(AST3_MAX, 1821);
        assert_eq!(named_bytes_for(Protocol::V1), named_bytes());
        assert_eq!(named_bytes_for(Protocol::V2), named_bytes());
        assert_eq!(named_bytes_for(Protocol::V3), named_bytes() + 6);
        assert_eq!(std::mem::size_of::<[u8; OBSERVATION_BYTES]>(), 2607);
    }
}
