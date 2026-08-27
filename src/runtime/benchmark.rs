use super::{
    artifact, compile_program, load_manifest, Interpreter, Parser, ProjectManifest, OXID_VERSION,
};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::hint::black_box;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const SCHEMA_VERSION: u32 = 1;
const MIN_ITERATIONS: usize = 1;
const MAX_ITERATIONS: usize = 10_000;
const PARSER_FUNCTION_COUNT: usize = 256;
const RUNTIME_LOOP_COUNT: usize = 1_000;
const ATOMIC_WRITE_ATTEMPTS: u32 = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
struct SampleStats {
    samples: usize,
    min_ns: u128,
    median_ns: u128,
    max_ns: u128,
    total_ns: u128,
}

impl SampleStats {
    fn from_nanos(samples: &[u128]) -> Result<Self, String> {
        if samples.is_empty() {
            return Err("benchmark statistics require at least one sample".to_string());
        }
        if samples.len() > MAX_ITERATIONS {
            return Err(format!(
                "benchmark sample count {} exceeds limit {}",
                samples.len(),
                MAX_ITERATIONS
            ));
        }

        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let min_ns = *sorted
            .first()
            .ok_or_else(|| "benchmark statistics lost the minimum sample".to_string())?;
        let max_ns = *sorted
            .last()
            .ok_or_else(|| "benchmark statistics lost the maximum sample".to_string())?;
        let lower_index = (sorted.len() - 1) / 2;
        let upper_index = sorted.len() / 2;
        let lower = *sorted
            .get(lower_index)
            .ok_or_else(|| "benchmark statistics lost the lower median sample".to_string())?;
        let upper = *sorted
            .get(upper_index)
            .ok_or_else(|| "benchmark statistics lost the upper median sample".to_string())?;
        let median_ns = lower + (upper - lower) / 2;
        let total_ns = samples.iter().try_fold(0_u128, |total, sample| {
            total
                .checked_add(*sample)
                .ok_or_else(|| "benchmark sample duration total overflow".to_string())
        })?;

        Ok(Self {
            samples: samples.len(),
            min_ns,
            median_ns,
            max_ns,
            total_ns,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BenchmarkReport {
    oxid_version: String,
    iterations: usize,
    cold_start: SampleStats,
    parser: SampleStats,
    parser_source_bytes: usize,
    parser_bytes_per_second: u128,
    packaging: SampleStats,
    runtime_operations: SampleStats,
}

impl BenchmarkReport {
    fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"schema_version\": {},\n",
                "  \"oxid_version\": \"{}\",\n",
                "  \"iterations\": {},\n",
                "  \"benchmarks\": {{\n",
                "    \"cold_start\": {},\n",
                "    \"parser\": {{\"unit\":\"nanoseconds\",\"samples\":{},\"min\":{},\"median\":{},\"max\":{},\"source_bytes\":{},\"bytes_per_second\":{}}},\n",
                "    \"packaging\": {},\n",
                "    \"runtime_operations\": {}\n",
                "  }}\n",
                "}}\n"
            ),
            SCHEMA_VERSION,
            escape_json_string(&self.oxid_version),
            self.iterations,
            stats_json(&self.cold_start),
            self.parser.samples,
            self.parser.min_ns,
            self.parser.median_ns,
            self.parser.max_ns,
            self.parser_source_bytes,
            self.parser_bytes_per_second,
            stats_json(&self.packaging),
            stats_json(&self.runtime_operations),
        )
    }

    fn write_human(&self, writer: &mut impl Write) -> Result<(), String> {
        writeln!(
            writer,
            "Oxid {} benchmark ({} iterations)",
            self.oxid_version, self.iterations
        )
        .map_err(|error| format!("cannot write benchmark summary: {error}"))?;
        write_stats_line(writer, "cold start", &self.cold_start)?;
        writeln!(
            writer,
            "  parser: min={} ns median={} ns max={} ns; {} bytes; {} bytes/s",
            self.parser.min_ns,
            self.parser.median_ns,
            self.parser.max_ns,
            self.parser_source_bytes,
            self.parser_bytes_per_second
        )
        .map_err(|error| format!("cannot write parser benchmark summary: {error}"))?;
        write_stats_line(writer, "packaging", &self.packaging)?;
        write_stats_line(writer, "runtime operations", &self.runtime_operations)
    }
}

pub(super) fn run(
    root: &Path,
    iterations: usize,
    json_output: Option<&Path>,
) -> Result<(), String> {
    validate_iterations(iterations)?;
    let entry = project_entry(root)?;
    let parser_source = generated_parser_source();
    let parser_source_bytes = parser_source.len();
    let runtime_program = parse_runtime_program()?;
    let executable = std::env::current_exe().map_err(|error| {
        format!("cannot locate the Oxid executable for cold-start benchmark: {error}")
    })?;

    let cold_start = measure("cold start", iterations, || {
        let output = Command::new(&executable)
            .arg("--version")
            .current_dir(root)
            .output()
            .map_err(|error| format!("cannot launch Oxid child process: {error}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "Oxid child process exited with {}{}",
                output.status,
                if stderr.trim().is_empty() {
                    String::new()
                } else {
                    format!(": {}", stderr.trim())
                }
            ));
        }
        black_box(output.stdout.len());
        Ok(())
    })?;

    let parser = measure("parser", iterations, || {
        let mut parser = Parser::new_with_source(&parser_source, "<benchmark-parser>");
        let program = parser.parse_program()?;
        black_box(program.stmts.len());
        Ok(())
    })?;
    let parser_bytes_per_second = parser_throughput(parser_source_bytes, &parser)?;

    let packaging = measure("packaging", iterations, || {
        let (program, module_count) = compile_program(&entry)?;
        let encoded = artifact::encode(&program, module_count)?;
        black_box(encoded.len());
        Ok(())
    })?;

    let runtime_operations = measure("runtime operations", iterations, || {
        let mut interpreter = Interpreter::new();
        interpreter.execute_program(&runtime_program, root)?;
        black_box(interpreter);
        Ok(())
    })?;

    let report = BenchmarkReport {
        oxid_version: OXID_VERSION.to_string(),
        iterations,
        cold_start,
        parser,
        parser_source_bytes,
        parser_bytes_per_second,
        packaging,
        runtime_operations,
    };

    let stdout = io::stdout();
    let mut output = stdout.lock();
    report.write_human(&mut output)?;
    drop(output);

    if let Some(path) = json_output {
        atomic_write(path, report.to_json().as_bytes())?;
    }
    Ok(())
}

fn validate_iterations(iterations: usize) -> Result<(), String> {
    if !(MIN_ITERATIONS..=MAX_ITERATIONS).contains(&iterations) {
        return Err(format!(
            "benchmark iterations must be between {} and {}, received {}",
            MIN_ITERATIONS, MAX_ITERATIONS, iterations
        ));
    }
    Ok(())
}

fn project_entry(root: &Path) -> Result<PathBuf, String> {
    let manifest_path = root.join("oxid.toml");
    let manifest: ProjectManifest = load_manifest(&manifest_path)?;
    let entry = manifest
        .entry
        .or_else(|| {
            root.join("src/main.ox")
                .is_file()
                .then(|| "src/main.ox".to_string())
        })
        .or_else(|| {
            root.join("main.ox")
                .is_file()
                .then(|| "main.ox".to_string())
        })
        .ok_or_else(|| {
            "benchmark packaging requires an entry in oxid.toml, src/main.ox, or main.ox"
                .to_string()
        })?;
    let entry = PathBuf::from(entry);
    let entry = if entry.is_absolute() {
        entry
    } else {
        root.join(entry)
    };
    if !entry.is_file() {
        return Err(format!(
            "benchmark packaging entry does not exist: {}",
            entry.display()
        ));
    }
    Ok(entry)
}

fn generated_parser_source() -> String {
    let mut source =
        String::from("const benchmark_config = {name: \"parser\", enabled: true, rounds: 256};\n");
    for index in 0..PARSER_FUNCTION_COUNT {
        source.push_str(&format!(
            "fun parse_item_{index}(value) => (value + {index}) * 2 % 997;\n"
        ));
    }
    source.push_str(
        "fun parser_entry() { var total = 0; for value in range(0, 256) { total = total + value; } give total; }\n",
    );
    source
}

fn parse_runtime_program() -> Result<super::Program, String> {
    let source = format!(
        concat!(
            "var total = 0;\n",
            "var index = 0;\n",
            "while index < {} {{\n",
            "    total = total + (index % 7);\n",
            "    index = index + 1;\n",
            "}}\n"
        ),
        RUNTIME_LOOP_COUNT
    );
    let mut parser = Parser::new_with_source(&source, "<benchmark-runtime>");
    parser.parse_program()
}

fn measure(
    label: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<(), String>,
) -> Result<SampleStats, String> {
    validate_iterations(iterations)?;
    let mut samples = Vec::with_capacity(iterations);
    for sample_index in 0..iterations {
        let started = Instant::now();
        operation().map_err(|error| {
            format!(
                "{} benchmark sample {} failed: {}",
                label,
                sample_index + 1,
                error
            )
        })?;
        samples.push(started.elapsed().as_nanos());
    }
    SampleStats::from_nanos(&samples)
}

fn parser_throughput(source_bytes: usize, stats: &SampleStats) -> Result<u128, String> {
    let bytes_per_sample = u128::try_from(source_bytes)
        .map_err(|_| "parser source byte count cannot be represented as u128".to_string())?;
    let sample_count = u128::try_from(stats.samples)
        .map_err(|_| "parser sample count cannot be represented as u128".to_string())?;
    let total_bytes = bytes_per_sample
        .checked_mul(sample_count)
        .ok_or_else(|| "parser processed byte count overflow".to_string())?;
    let scaled_bytes = total_bytes
        .checked_mul(1_000_000_000)
        .ok_or_else(|| "parser throughput calculation overflow".to_string())?;
    Ok(scaled_bytes / stats.total_ns.max(1))
}

fn stats_json(stats: &SampleStats) -> String {
    format!(
        "{{\"unit\":\"nanoseconds\",\"samples\":{},\"min\":{},\"median\":{},\"max\":{}}}",
        stats.samples, stats.min_ns, stats.median_ns, stats.max_ns
    )
}

fn escape_json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0c}' => escaped.push_str("\\f"),
            control if control.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => escaped.push(other),
        }
    }
    escaped
}

fn write_stats_line(
    writer: &mut impl Write,
    label: &str,
    stats: &SampleStats,
) -> Result<(), String> {
    writeln!(
        writer,
        "  {}: min={} ns median={} ns max={} ns",
        label, stats.min_ns, stats.median_ns, stats.max_ns
    )
    .map_err(|error| format!("cannot write {label} benchmark summary: {error}"))
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|candidate| !candidate.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create benchmark output directory {}: {}",
            parent.display(),
            error
        )
    })?;
    let file_name = path.file_name().ok_or_else(|| {
        format!(
            "benchmark JSON output path has no file name: {}",
            path.display()
        )
    })?;

    let mut temporary = None;
    for attempt in 0..ATOMIC_WRITE_ATTEMPTS {
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".{}.{}.tmp", std::process::id(), attempt));
        let temporary_path = parent.join(temporary_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => {
                temporary = Some((temporary_path, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create temporary benchmark JSON beside {}: {}",
                    path.display(),
                    error
                ))
            }
        }
    }

    let (temporary_path, mut file) = temporary.ok_or_else(|| {
        format!(
            "cannot reserve a temporary benchmark JSON file beside {} after {} attempts",
            path.display(),
            ATOMIC_WRITE_ATTEMPTS
        )
    })?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary_path);
        return Err(format!(
            "cannot write temporary benchmark JSON {}: {}",
            temporary_path.display(),
            error
        ));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!(
            "cannot atomically replace benchmark JSON {}: {}",
            path.display(),
            error
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_stats() -> SampleStats {
        SampleStats {
            samples: 3,
            min_ns: 10,
            median_ns: 20,
            max_ns: 30,
            total_ns: 60,
        }
    }

    #[test]
    fn statistics_are_sorted_and_use_midpoint_median() {
        let stats = SampleStats::from_nanos(&[9, 1, 5, 3]).expect("statistics");
        assert_eq!(
            stats,
            SampleStats {
                samples: 4,
                min_ns: 1,
                median_ns: 4,
                max_ns: 9,
                total_ns: 18,
            }
        );
    }

    #[test]
    fn json_schema_is_deterministic_and_path_free() {
        let stats = fixture_stats();
        let report = BenchmarkReport {
            oxid_version: "0.9.0\"test".to_string(),
            iterations: 3,
            cold_start: stats.clone(),
            parser: stats.clone(),
            parser_source_bytes: 120,
            parser_bytes_per_second: 6_000_000_000,
            packaging: stats.clone(),
            runtime_operations: stats,
        };
        let expected = concat!(
            "{\n",
            "  \"schema_version\": 1,\n",
            "  \"oxid_version\": \"0.9.0\\\"test\",\n",
            "  \"iterations\": 3,\n",
            "  \"benchmarks\": {\n",
            "    \"cold_start\": {\"unit\":\"nanoseconds\",\"samples\":3,\"min\":10,\"median\":20,\"max\":30},\n",
            "    \"parser\": {\"unit\":\"nanoseconds\",\"samples\":3,\"min\":10,\"median\":20,\"max\":30,\"source_bytes\":120,\"bytes_per_second\":6000000000},\n",
            "    \"packaging\": {\"unit\":\"nanoseconds\",\"samples\":3,\"min\":10,\"median\":20,\"max\":30},\n",
            "    \"runtime_operations\": {\"unit\":\"nanoseconds\",\"samples\":3,\"min\":10,\"median\":20,\"max\":30}\n",
            "  }\n",
            "}\n"
        );
        assert_eq!(report.to_json(), expected);
        assert_eq!(report.to_json(), report.to_json());
        assert!(!report.to_json().contains("Documents"));
    }

    #[test]
    fn iteration_bounds_and_parser_rate_are_checked_without_spawning() {
        assert!(validate_iterations(0).is_err());
        assert!(validate_iterations(MAX_ITERATIONS + 1).is_err());
        assert!(validate_iterations(MIN_ITERATIONS).is_ok());
        assert!(validate_iterations(MAX_ITERATIONS).is_ok());

        let stats = SampleStats {
            samples: 2,
            min_ns: 100,
            median_ns: 150,
            max_ns: 200,
            total_ns: 300,
        };
        assert_eq!(
            parser_throughput(300, &stats).expect("throughput"),
            2_000_000_000
        );
    }

    #[test]
    fn generated_sources_parse_without_child_processes() {
        let first = generated_parser_source();
        let second = generated_parser_source();
        assert_eq!(first, second);
        assert!(!first.is_empty());
        let mut parser = Parser::new_with_source(&first, "<benchmark-parser-test>");
        assert!(parser.parse_program().is_ok());
        assert!(parse_runtime_program().is_ok());
    }

    #[test]
    fn json_output_is_atomically_replaceable() {
        let path = std::env::temp_dir().join(format!(
            "oxid-benchmark-json-test-{}.json",
            std::process::id()
        ));
        atomic_write(&path, b"first\n").expect("first atomic write");
        atomic_write(&path, b"second\n").expect("replacement atomic write");
        let contents = fs::read(&path).expect("read atomic output");
        assert_eq!(contents, b"second\n");
        fs::remove_file(path).expect("remove atomic output");
    }
}
