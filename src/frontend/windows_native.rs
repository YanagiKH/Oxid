//! Denial-only Windows native scope over already retained parser-owned sources.
//! No source, declaration, type or executable witness is manufactured here.
use super::{
    build_config::{ExecutionPolicy, NativeBuildSettings},
    diagnostic::Diagnostic,
    project::ProjectSources,
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
        if !program.generic_headers().is_empty()
            || program.uses_owned_syntax(source)
            || program.uses_byte_scalars(source)
            || program.uses_bitwise_syntax()
            || program.uses_float_syntax(source)
            || program.uses_character_syntax(source)
            || program.has_range_for()
        {
            return Err(reject());
        }
    }
    Ok(())
}

fn reject() -> Box<Diagnostic> {
    Diagnostic::new(
        "E0700", "native-admission",
        "Windows native builds currently support bounded result-entry bool, i32 and unit scalar functions without generics, owned values, references, arrays, resources, process I/O or debug information",
        None,
    )
}
