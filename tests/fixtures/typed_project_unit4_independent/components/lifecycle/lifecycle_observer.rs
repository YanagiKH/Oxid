//! Test-only observer in an isolated qualification snapshot, absent from production.
//! Events are logical operations, not OS syscall counts. No source data is read here.
use std::io::Write;
pub(super) fn event(kind: &str, subject: &str) {
    if let Some(path) = std::env::var_os("UNIT4_LIFECYCLE_LOG") {
        let mut file = std::fs::OpenOptions::new().create(true).append(true)
            .open(path).expect("lifecycle evidence open");
        writeln!(file, "{{\"event\":{},\"subject\":{}}}",
            super::diagnostic::json_string(kind), super::diagnostic::json_string(subject))
            .expect("lifecycle evidence write");
    }
}
