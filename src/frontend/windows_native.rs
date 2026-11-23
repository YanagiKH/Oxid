//! Denial-only Windows native scope over already retained parser-owned sources.
//! No source, declaration, type or executable witness is manufactured here.
use super::{
    ast::{ExprKind, Program},
    build_config::{ExecutionPolicy, NativeBuildSettings},
    diagnostic::Diagnostic,
    project::ProjectSources,
    source::SourceFile,
};

fn preflight(
    project: &ProjectSources,
    settings: Option<&NativeBuildSettings>,
) -> Result<(), Box<Diagnostic>> {
    let execution = settings.map_or(ExecutionPolicy::Bounded, |settings| settings.execution());
    if execution.hosted() || settings.is_some_and(|settings| settings.debug_info()) {
        return Err(reject());
    }
    // All modules and unused declarations participate. These bounded parser
    // feature facts only deny. Ordinary type checking and OIR verification still
    // establish the sole authority for the native consumer.
    if outside_project_scope(project, false) {
        return Err(reject());
    }
    Ok(())
}

/// The native consumer verifies the complete type and operation inventory
/// for checked programs using File or time operations.
pub(super) fn preflight_checked(
    project: &ProjectSources,
    checked: &super::oir::CheckedSourceProgram<'_>,
    process: bool,
    settings: Option<&NativeBuildSettings>,
) -> Result<(), Box<Diagnostic>> {
    if !NativeBuildSettings::windows_msvc_host() {
        return Ok(());
    }
    let binary_streams = checked.windows_binary_io_candidate();
    if binary_streams || checked.windows_file_or_time_candidate() {
        let execution = settings.map_or(ExecutionPolicy::Bounded, |settings| settings.execution());
        if process != binary_streams
            || execution.hosted()
            || settings.is_some_and(|settings| settings.debug_info())
        {
            return Err(reject());
        }
        // Reuse the existing complete module walk. This narrower parser fact
        // only denies generic syntax; the sealed consumer denies unported
        // resource/type metadata before emitting any native code.
        if outside_project_scope(project, true) {
            return Err(reject());
        }
        Ok(())
    } else {
        preflight(project, settings)
    }
}

/// Additional borrowed routing roles, paid only by the Windows File and time consumer.
/// No source owner, parser arena, witness or retained permit is copied.
#[allow(dead_code)]
#[cfg(any(
    test,
    all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )
))]
struct FileRoutingCarriers {
    wrapper_arguments: (
        &'static ProjectSources,
        &'static super::oir::CheckedSourceProgram<'static>,
        bool,
        Option<&'static NativeBuildSettings>,
    ),
    candidate_arguments: [&'static super::oir::CheckedSourceProgram<'static>; 2],
    selected_body: &'static (),
    candidate_inventory: super::builtin_catalog::termination::Inventory,
    candidate_returns: [bool; 6],
    execution: ExecutionPolicy,
    scope_arguments: (&'static ProjectSources, bool),
    scope_iterator: std::slice::Iter<'static, super::project::ModuleHeader>,
    selected_module: &'static super::project::ModuleHeader,
    ast_query: Option<&'static Program>,
    selected_ast: &'static Program,
    generic_header_borrow: &'static [super::ast::GenericHeader],
    scope_returns: [bool; 3],
    predicate_capture: (&'static ProjectSources, &'static bool),
    returns: [Result<(), Box<Diagnostic>>; 2],
}
#[cfg(any(
    test,
    all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    )
))]
pub(super) const fn file_routing_control_bytes() -> usize {
    std::mem::size_of::<FileRoutingCarriers>()
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
    if outside_project_scope(project, false) {
        return Err(Diagnostic::new(
            "E0608", "oir-run",
            "Windows project tests support only pure bool, i32 and unit functions without owned values, references, arrays, resources, generics or process I/O",
            None,
        ));
    }
    Ok(())
}

fn outside_project_scope(project: &ProjectSources, file_or_time_candidate: bool) -> bool {
    project.modules().iter().any(|header| {
        project.try_file_ast(header.file).is_none_or(|program| {
            if file_or_time_candidate {
                !program.generic_headers().is_empty()
            } else {
                outside_scalar_scope(program, project.sources().get(header.file))
            }
        })
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
        "Windows native builds support bounded bool/i32/unit functions with result or process entry. Binary streams use process entry. File byte I/O and std::time can use result entry. Generics, other owned types and debug information are unavailable",
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, parser, project::budget::Allocator, source::SourceMap};

    #[cfg(all(
        target_os = "windows",
        target_env = "msvc",
        target_arch = "x86_64",
        target_pointer_width = "64",
        target_endian = "little"
    ))]
    #[test]
    fn windows_file_routing_has_a_closed_borrowed_carrier_inventory() {
        fn inventory(bank: &FileRoutingCarriers) {
            let FileRoutingCarriers {
                wrapper_arguments,
                candidate_arguments,
                selected_body,
                candidate_inventory,
                candidate_returns,
                execution,
                scope_arguments,
                scope_iterator,
                selected_module,
                ast_query,
                selected_ast,
                generic_header_borrow,
                scope_returns,
                predicate_capture,
                returns,
            } = bank;
            let _ = (
                wrapper_arguments,
                candidate_arguments,
                selected_body,
                candidate_inventory,
                candidate_returns,
                execution,
                scope_arguments,
                scope_iterator,
                selected_module,
                ast_query,
                selected_ast,
                generic_header_borrow,
                scope_returns,
                predicate_capture,
                returns,
            );
        }
        let _: fn(&FileRoutingCarriers) = inventory;
        println!(
            "Windows File source routing={} align={}",
            file_routing_control_bytes(),
            std::mem::align_of::<FileRoutingCarriers>()
        );
    }

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
