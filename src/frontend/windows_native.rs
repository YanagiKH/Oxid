//! Denial-only Windows native scope over already retained parser-owned sources.
//! No source, declaration, type or executable witness is manufactured here.
use super::{
    ast::{ExprKind, Program},
    build_config::{ExecutionPolicy, NativeBuildSettings},
    diagnostic::Diagnostic,
    project::ProjectSources,
    source::SourceFile,
};

pub(super) fn preflight(
    project: &ProjectSources,
    process: bool,
    settings: Option<&NativeBuildSettings>,
) -> Result<(), Box<Diagnostic>> {
    if !NativeBuildSettings::windows_msvc_host() {
        return Ok(());
    }
    let execution = settings.map_or(ExecutionPolicy::Bounded, |settings| settings.execution());
    if process || execution.hosted() || settings.is_some_and(|settings| settings.debug_info()) {
        return Err(reject());
    }
    // All modules and unused declarations participate. These bounded parser
    // feature facts only deny. Ordinary type checking and OIR verification still
    // establish the sole authority for the native consumer.
    if outside_project_scope(project) {
        return Err(reject());
    }
    Ok(())
}

pub(super) fn test_preflight(
    project: &ProjectSources,
    execution: ExecutionPolicy,
) -> Result<(), Box<Diagnostic>> {
    if execution.hosted() {
        return Err(Diagnostic::new(
            "E0608",
            "oir-run",
            "Windows project tests require bounded execution",
            None,
        ));
    }
    if outside_project_scope(project) {
        return Err(Diagnostic::new(
            "E0608", "oir-run",
            "Windows project tests support only pure bool, i32 and unit functions without owned values, references, arrays, resources, generics or process I/O",
            None,
        ));
    }
    Ok(())
}

fn outside_project_scope(project: &ProjectSources) -> bool {
    project.modules().iter().any(|header| {
        project
            .try_file_ast(header.file)
            .is_none_or(|program| outside_scalar_scope(program, project.sources().get(header.file)))
    })
}

// This is a denial predicate, not a witness or a type check. Inferred
// conversions also participate: an unused function can introduce a float,
// character or wider scalar without any literal, signature or annotation.
fn outside_scalar_scope(program: &Program, source: &SourceFile) -> bool {
    !program.generic_headers().is_empty()
        || program.uses_owned_syntax(source)
        || program.uses_byte_scalars(source)
        || program.uses_bitwise_syntax()
        || program.uses_float_syntax(source)
        || program.uses_character_syntax(source)
        || program.has_range_for()
        || program
            .expressions
            .iter()
            .any(|expression| matches!(expression.kind, ExprKind::Conversion { .. }))
}

fn reject() -> Box<Diagnostic> {
    Diagnostic::new(
        "E0700", "native-admission",
        "Windows native builds currently support bounded result-entry bool, i32 and unit scalar functions without generics, owned values, references, arrays, resources, process I/O or debug information",
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, parser, project::budget::Allocator, source::SourceMap};

    fn outside(text: &str) -> bool {
        let mut sources = SourceMap::new();
        let id = sources.add("windows-scope.ox".into(), text.into());
        let source = sources.get(id);
        let program = parser::parse_typed_counted(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            &mut Allocator::default(),
            &mut parser::SyntaxStorage::default(),
        )
        .unwrap()
        .0;
        outside_scalar_scope(&program, source)
    }

    #[test]
    fn scalar_scope_retains_functions_loops_branches_unit_and_short_circuiting() {
        for text in [
            "fn main()->i32{return -2147483648;}",
            "fn helper()->(){return;}fn main()->(){helper();return;}",
            "fn main()->bool{return false&&(1/0==0)||true;}",
            "fn step(n:i32)->i32{return n+1;}fn main()->i32{let mut n=0;while n<42{n=step(n);if n==40{continue;}}return n;}",
        ] {
            assert!(!outside(text), "{text}");
        }
    }

    #[test]
    fn inferred_intrinsics_in_unused_functions_cannot_expand_the_native_domain() {
        for body in [
            "let n=1;let f=n.to_f64();",
            "let n=1;let f=n.to_f32();",
            "let n=65;let c=n.to_char_checked();",
            "let n=65;let c=n.to_char_checked();let scalar=c.to_u32();",
            "let n=1;let f=n.to_f64();let root=f.sqrt();",
            "let n=1;let f=n.to_f64();let bits=f.to_bits();",
            "let n=1;let f=n.to_f64();let bits=f.to_bits();let back=f64::from_bits(bits);",
            "let n=1;let wide=n.to_i64();",
            "let n=1;let byte=n.to_u8_checked();let back=byte.to_i32();",
        ] {
            let text = format!("fn unused()->i32{{{body}return 0;}}fn main()->i32{{return 42;}}");
            assert!(outside(&text), "{text}");
        }
    }

    fn typed_test_status(text: &str) -> Result<i32, Box<Diagnostic>> {
        let mut sources = SourceMap::new();
        let id = sources.add("declared-test.ox".into(), text.into());
        let source = sources.get(id);
        let program = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let checked = crate::frontend::oir::check_source(source, &program, &sources).unwrap();
        checked.run_test_i32()
    }

    #[test]
    fn pure_tests_return_actual_i32_status_without_process_setup() {
        for value in [0, 7, -1, 256, i32::MIN, i32::MAX] {
            let text = format!("fn main()->i32{{return {value};}}");
            assert_eq!(typed_test_status(&text).unwrap(), value, "{text}");
        }
    }

    #[test]
    fn pure_test_signature_is_checked_before_any_user_evaluation() {
        for text in [
            "fn main()->bool{while true{}return false;}",
            "fn main()->(){let x=1/0;return;}",
            "fn main(value:i32)->i32{while true{}return value;}",
            "fn helper()->i32{return 0;}",
            "struct P{}fn main()->P{return P{};}",
        ] {
            let error = typed_test_status(text).unwrap_err();
            assert_eq!(
                (error.code, error.stage),
                ("E0600", "oir-run"),
                "{text}: {error:?}"
            );
            assert_eq!(
                error.message,
                "test entry requires fn main() -> i32 with no parameters in the declared test file"
            );
        }
    }

    #[test]
    fn pure_test_runtime_errors_keep_the_original_diagnostic_and_origin() {
        for (expression, code, marker) in [("2147483647+1", "E0604", "+"), ("1/0", "E0607", "/")] {
            let text = format!("fn main()->i32{{return {expression};}}");
            let error = typed_test_status(&text).unwrap_err();
            assert_eq!((error.code, error.stage), (code, "oir-run"));
            let span = error.primary.unwrap();
            assert_eq!(&text[span.start..span.end], marker);
        }
    }

    #[test]
    fn pure_test_scope_and_hosted_refusals_are_exercised_on_every_host() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        for (text, policy, allowed) in [
            ("fn main()->i32{return 0;}", ExecutionPolicy::Bounded, true),
            ("fn main()->i32{return 0;}", ExecutionPolicy::Hosted, false),
            (
                "struct P{}fn main()->i32{return 0;}",
                ExecutionPolicy::Bounded,
                false,
            ),
            (
                "fn main()->i32{let n=1;let value=n.to_f64();return 0;}",
                ExecutionPolicy::Bounded,
                false,
            ),
            (
                "fn unused<T:Copy>(value:T)->T{return value;}fn main()->i32{return 0;}",
                ExecutionPolicy::Bounded,
                false,
            ),
        ] {
            let root = std::env::temp_dir().join(format!(
                "oxid-pure-test-scope-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            let entry = root.join("main.ox");
            std::fs::write(&entry, text).unwrap();
            let project = ProjectSources::load_typed(
                entry.to_str().unwrap(),
                crate::frontend::project::ProjectLimits::default(),
            )
            .unwrap();
            std::fs::remove_dir_all(root).unwrap();
            let result = test_preflight(&project, policy);
            if allowed {
                result.unwrap();
            } else {
                let error = result.unwrap_err();
                assert_eq!((error.code, error.stage), ("E0608", "oir-run"), "{text}");
            }
        }
    }
}
