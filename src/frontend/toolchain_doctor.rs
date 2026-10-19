//! Explicit typed toolchain diagnostics; no project source or program execution.
use super::{diagnostic::Diagnostic, native, source::SourceMap};
use std::io::Write;

pub(super) fn run() -> i32 {
    match native::probe() {
        Ok(report) => {
            let result = writeln!(
                std::io::stdout().lock(),
                "typed native toolchain ready\ntarget: {}\nLLVM: {}\nprobe: bundled runtime compiled and linked; program not executed",
                report.target,
                report.llvm_version
            );
            if result.is_ok() {
                0
            } else {
                let diagnostic = Diagnostic::new(
                    "E0701",
                    "native-toolchain",
                    "toolchain probe succeeded, but its report could not be written",
                    None,
                );
                let _ = writeln!(
                    std::io::stderr().lock(),
                    "{}",
                    diagnostic.render_human(&SourceMap::new())
                );
                1
            }
        }
        Err(diagnostic) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "{}",
                diagnostic.render_human(&SourceMap::new())
            );
            1
        }
    }
}
