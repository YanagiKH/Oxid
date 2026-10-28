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
    for header in project.modules() {
        let Some(program) = project.try_file_ast(header.file) else {
            return Err(reject());
        };
        let source = project.sources().get(header.file);
        if outside_scalar_scope(program, source) {
            return Err(reject());
        }
    }
    Ok(())
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
}
