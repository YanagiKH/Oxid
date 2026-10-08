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
const SOURCE_MAX: usize = 128;
const AST1_MAX: usize = 5 + SOURCE_MAX + OPA_BYTES;

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

pub(super) struct Outcome {
    pub(super) observation: Option<[u8; OBSERVATION_BYTES]>,
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
        if input.len() > SOURCE_MAX || !input.is_ascii() {
            return result
                .reject("experimental HIR producers require at most 128 ASCII source bytes");
        }
        let selected = match bundle::Bundle::load(directory) {
            Ok(bundle) => bundle,
            Err(message) => return result.reject(message),
        };
        let parser = supervisor::run(
            selected.parser(),
            selected.directory(),
            input,
            OPA_BYTES,
            supervisor::DEADLINE,
        );
        result.records[0] = Some(record("parser", selected.parser_hash(), input, &parser));
        if !completed(&parser, input.len()) {
            return result.reject("parser producer failed its bounded process/transport contract");
        }
        if parser.stdout_len != OPA_BYTES
            || &parser.stdout[..4] != b"OPA1"
            || parser.stdout[4..8] != [0; 4]
            || usize::from(parser.stdout[10]) != input.len()
        {
            return result
                .reject("parser producer did not return one complete successful OPA1 frame");
        }
        // Transport exact original bytes and unchanged OPA1. No host-derived
        // name/type/flow data, repaired row or substituted source is introduced.
        let mut ast1 = [0u8; AST1_MAX];
        ast1[..4].copy_from_slice(b"AST1");
        ast1[4] = input.len() as u8;
        ast1[5..5 + input.len()].copy_from_slice(input);
        ast1[5 + input.len()..5 + input.len() + OPA_BYTES]
            .copy_from_slice(&parser.stdout[..OPA_BYTES]);
        let framed = &ast1[..5 + input.len() + OPA_BYTES];
        let typed = supervisor::run(
            selected.static_consumer(),
            selected.directory(),
            framed,
            OBSERVATION_BYTES,
            supervisor::DEADLINE,
        );
        result.records[1] = Some(record("static", selected.static_hash(), framed, &typed));
        if !completed(&typed, framed.len()) {
            return result.reject("static producer failed its bounded process/transport contract");
        }
        if typed.stdout_len != OBSERVATION_BYTES
            || typed.stdout[..OPA_BYTES] != parser.stdout[..OPA_BYTES]
            || &typed.stdout[OPA_BYTES..OPA_BYTES + 5] != b"STF1\0"
        {
            return result.reject("static producer did not return one complete successful unchanged OPA1/STF1 observation");
        }
        let mut observation = [0u8; OBSERVATION_BYTES];
        observation.copy_from_slice(&typed.stdout[..OBSERVATION_BYTES]);
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
        + size_of::<[Record; 8]>()
        + 3 * size_of::<[u8; AST1_MAX]>()
        + 2 * size_of::<SourceOwner<'static>>()
        + size_of::<[usize; 16]>()
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
