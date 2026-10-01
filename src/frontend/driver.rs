use super::{
    diagnostic::{json_string, Diagnostic},
    hir, lexer, oir,
    options::{self, Route},
    parser,
    source::{SourceMap, MAX_SOURCE_BYTES},
    typeck,
};
use std::{fs::File, io::Read};

/// None returns an untouched/default or explicitly legacy command to the old CLI.
pub fn dispatch(args: &mut Vec<String>) -> Option<i32> {
    match options::route(args) {
        Route::Legacy(legacy) => {
            *args = legacy;
            None
        }
        Route::Error { message, json } => Some(report(
            &SourceMap::new(),
            vec![*Diagnostic::new("E0001", "cli", message, None)],
            json,
            None,
        )),
        Route::TypedCheck { path, json } => Some(check_file(&path, json)),
    }
}
fn report(
    sources: &SourceMap,
    diagnostics: Vec<Diagnostic>,
    json: bool,
    functions: Option<usize>,
) -> i32 {
    let success = diagnostics.is_empty();
    for diagnostic in &diagnostics {
        if json {
            println!("{}", diagnostic.render_json(sources));
        } else {
            eprint!("{}", diagnostic.render_human(sources));
        }
    }
    if json {
        println!("{}", check_summary(diagnostics.len(), functions));
    } else if success {
        println!(
            "typed-preview check ok ({} functions; check only)",
            functions.unwrap_or(0)
        );
    }
    exit_status(&diagnostics)
}
fn check_summary(errors: usize, functions: Option<usize>) -> String {
    format!("{{\"schema_version\":1,\"edition\":{},\"kind\":\"check-summary\",\"success\":{},\"errors\":{},\"functions\":{}}}",
        json_string("typed-preview"), errors == 0, errors,
        functions.filter(|_| errors == 0).map_or("null".to_string(), |n| n.to_string()))
}

fn exit_status(diagnostics: &[Diagnostic]) -> i32 {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "E0500")
    {
        2
    } else {
        i32::from(!diagnostics.is_empty())
    }
}

fn check_file(path: &str, json: bool) -> i32 {
    let mut sources = SourceMap::new();
    let result = (|| {
        let file = File::open(path).map_err(|e| {
            vec![*Diagnostic::new(
                "E0002",
                "source",
                format!("cannot read file {path}: {e}"),
                None,
            )]
        })?;
        let mut bytes = Vec::new();
        file.take((MAX_SOURCE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| {
                vec![*Diagnostic::new(
                    "E0002",
                    "source",
                    format!("cannot read file {path}: {e}"),
                    None,
                )]
            })?;
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err(vec![*Diagnostic::new(
                "E0400",
                "source",
                format!("source exceeds {MAX_SOURCE_BYTES} bytes: {path}"),
                None,
            )]);
        }
        if bytes.starts_with(b"OXBC") {
            return Err(vec![*Diagnostic::new(
                "E0004",
                "source",
                "legacy OXBC artifacts are unavailable in typed-preview",
                None,
            )]);
        }
        let text = String::from_utf8(bytes).map_err(|_| {
            vec![*Diagnostic::new(
                "E0003",
                "source",
                format!("source is not valid UTF-8: {path}"),
                None,
            )]
        })?;
        let id = sources.add(path.to_string(), text);
        let source = sources.get(id);
        let tokens = lexer::lex(source).map_err(|e| vec![*e])?;
        let ast = parser::parse(source, tokens)?;
        debug_assert!(!ast.tokens.is_empty());
        let resolved = hir::resolve(source, &ast)?;
        let typed = typeck::check(resolved)?;
        let verified = oir::lower_and_verify(&typed, &sources)
            .map_err(|error| vec![*error.diagnostic(&sources)])?;
        Ok(verified.function_count())
    })();
    match result {
        Ok(functions) => report(&sources, Vec::new(), json, Some(functions)),
        Err(diagnostics) => report(&sources, diagnostics, json, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_exit_status_preserves_source_errors_and_distinguishes_oir_internal_errors() {
        assert_eq!(exit_status(&[]), 0);
        for (code, stage) in [("E0300", "type"), ("E0400", "oir-lower")] {
            assert_eq!(
                exit_status(&[*Diagnostic::new(code, stage, "failure", None)]),
                1
            );
        }
        let diagnostic = *Diagnostic::new("E0500", "oir-verify", "internal compiler error", None);
        assert_eq!(exit_status(std::slice::from_ref(&diagnostic)), 2);
        assert!(diagnostic
            .render_json(&SourceMap::new())
            .contains("\"primary\":null"));
        assert_eq!(report(&SourceMap::new(), vec![diagnostic], true, None), 2);
        assert_eq!(check_summary(1, Some(7)), "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"check-summary\",\"success\":false,\"errors\":1,\"functions\":null}");
    }
}
