//! Module-wide source selection with sealed scalar and owned consumer routes.
use super::{owned, *};
use crate::frontend::{ast, source::SourceFile};

#[derive(Debug)]
enum CheckedBody {
    Scalar(VerifiedProgram),
    Owned(owned::SourceProgram),
}

/// Its source-only constructor binds the resolved entry to its verified body.
#[derive(Debug)]
pub(in crate::frontend) struct CheckedSourceProgram {
    body: CheckedBody,
    entry: Option<hir::DefId>,
}

pub(in crate::frontend) fn check_source(
    source: &SourceFile,
    ast: &ast::Program,
    sources: &SourceMap,
) -> Result<CheckedSourceProgram, Vec<Diagnostic>> {
    let (body, entry) = if ast.uses_owned_syntax(source) {
        let (verified, entry) = owned::check_source(source, ast, sources)?;
        (CheckedBody::Owned(verified), entry)
    } else {
        // Keep this sequence identical to the existing scalar driver, including
        // source-declaration entry extraction and unchanged diagnostic adapters.
        let resolved = hir::resolve(source, ast)?;
        let entry = ast
            .functions
            .iter()
            .zip(&resolved.functions)
            .find(|(declaration, _)| {
                &source.text()[declaration.name.start..declaration.name.end] == "main"
            })
            .map(|(_, function)| function.id);
        let typed = typeck::check(resolved)?;
        let verified =
            lower_and_verify(&typed, sources).map_err(|error| vec![*error.diagnostic(sources)])?;
        (CheckedBody::Scalar(verified), entry)
    };
    Ok(CheckedSourceProgram { body, entry })
}

impl CheckedSourceProgram {
    pub(in crate::frontend) fn function_count(&self) -> usize {
        match &self.body {
            CheckedBody::Scalar(program) => program.function_count(),
            CheckedBody::Owned(program) => program.function_count(),
        }
    }

    pub(in crate::frontend) fn run(&self, sources: &SourceMap) -> Result<Scalar, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program
                .run(self.entry)
                .map_err(|error| error.diagnostic(sources)),
            CheckedBody::Owned(program) => program.run(self.entry, sources),
        }
    }

    pub(in crate::frontend) fn native_module(
        &self,
        sources: &SourceMap,
    ) -> Result<String, Box<Diagnostic>> {
        match &self.body {
            CheckedBody::Scalar(program) => program.native_module(self.entry, sources),
            CheckedBody::Owned(program) => program.native_module(self.entry, sources),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{lexer, parser};

    fn parsed(text: &str) -> (SourceMap, ast::Program) {
        let mut sources = SourceMap::new();
        let file = sources.add("source-dispatch.ox".into(), text.into());
        let source = sources.get(file);
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        (sources, ast)
    }

    #[test]
    fn scalar_dispatch_preserves_execution_native_and_entry_identity() {
        let text = "/* struct C {} &mut x.n */ fn helper() -> i32 { return 7; } fn main() -> i32 { return helper() + 4; }";
        let (sources, ast) = parsed(text);
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let checked = check_source(source, &ast, &sources).unwrap();
        assert!(matches!(checked.body, CheckedBody::Scalar(_)));
        assert_eq!(checked.entry, Some(hir::DefId(1)));
        assert_eq!(checked.function_count(), 2);
        assert_eq!(checked.run(&sources).unwrap(), Scalar::I32(11));
        let old = lower_and_verify(
            &typeck::check(hir::resolve(source, &ast).unwrap()).unwrap(),
            &sources,
        )
        .unwrap();
        assert_eq!(
            checked.native_module(&sources).unwrap(),
            old.native_module(Some(hir::DefId(1)), &sources).unwrap()
        );
    }

    #[test]
    fn scalar_dispatch_preserves_error_rendering() {
        for text in [
            "fn main() -> i32 { return true; }",
            "fn main() -> i32 { return missing; }",
        ] {
            let (sources, ast) = parsed(text);
            let source = sources.get(crate::frontend::source::SourceFileId(0));
            let old = hir::resolve(source, &ast)
                .and_then(typeck::check)
                .unwrap_err();
            let new = check_source(source, &ast, &sources).unwrap_err();
            assert_eq!(old.len(), new.len());
            for (old, new) in old.iter().zip(&new) {
                assert_eq!(old.render_json(&sources), new.render_json(&sources));
                assert_eq!(old.render_human(&sources), new.render_human(&sources));
            }
        }
        let (sources, ast) = parsed("fn helper() -> () { return; }");
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let checked = check_source(source, &ast, &sources).unwrap();
        let old = lower_and_verify(
            &typeck::check(hir::resolve(source, &ast).unwrap()).unwrap(),
            &sources,
        )
        .unwrap();
        assert_eq!(
            checked.run(&sources).unwrap_err().render_json(&sources),
            old.run(None)
                .unwrap_err()
                .diagnostic(&sources)
                .render_json(&sources)
        );
        assert_eq!(
            checked
                .native_module(&sources)
                .unwrap_err()
                .render_json(&sources),
            old.native_module(None, &sources)
                .unwrap_err()
                .render_json(&sources)
        );
        for text in [
            "fn main() -> i32 { return 2147483647 + 1; }",
            "fn main(x: i32) -> i32 { return x; }",
        ] {
            let (sources, ast) = parsed(text);
            let source = sources.get(crate::frontend::source::SourceFileId(0));
            let checked = check_source(source, &ast, &sources).unwrap();
            let old = lower_and_verify(
                &typeck::check(hir::resolve(source, &ast).unwrap()).unwrap(),
                &sources,
            )
            .unwrap();
            assert_eq!(
                checked.run(&sources).unwrap_err().render_json(&sources),
                old.run(Some(hir::DefId(0)))
                    .unwrap_err()
                    .diagnostic(&sources)
                    .render_json(&sources),
            );
        }
    }

    #[test]
    fn owned_dispatch_verifies_entire_module_and_preserves_entry_identity() {
        let (sources, ast) = parsed("struct C { n: i32 } fn helper() -> i32 { return 7; } fn main() -> i32 { let x = C { n: helper() }; return x.n + 4; }");
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let checked = check_source(source, &ast, &sources).unwrap();
        assert!(matches!(checked.body, CheckedBody::Owned(_)));
        assert_eq!(checked.entry, Some(hir::DefId(1)));
        assert_eq!(checked.function_count(), 2);
        assert_eq!(checked.run(&sources).unwrap(), Scalar::I32(11));
        let module = checked.native_module(&sources).unwrap();
        assert!(module.contains("define i32 @main"));

        let (sources, ast) = parsed(
            "struct C {} fn unused(x: C) -> () { x; x; return; } fn main() -> i32 { return 11; }",
        );
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let errors = check_source(source, &ast, &sources).unwrap_err();
        assert_eq!((errors[0].code, errors[0].stage), ("E0310", "ownership"));
    }

    #[test]
    fn owned_dispatch_has_no_fallback_and_keeps_entry_admission() {
        for (text, run_code, native_code) in [
            (
                "struct C {} fn helper() -> () { return; }",
                "E0600",
                "E0700",
            ),
            (
                "struct C {} fn main(x: i32) -> i32 { return x; }",
                "E0600",
                "E0700",
            ),
            (
                "struct C {} fn main() -> C { return C {}; }",
                "E0600",
                "E0700",
            ),
        ] {
            let (sources, ast) = parsed(text);
            let source = sources.get(crate::frontend::source::SourceFileId(0));
            let checked = check_source(source, &ast, &sources).unwrap();
            assert!(matches!(checked.body, CheckedBody::Owned(_)));
            assert_eq!(checked.run(&sources).unwrap_err().code, run_code);
            assert_eq!(
                checked.native_module(&sources).unwrap_err().code,
                native_code
            );
        }
        let (sources, ast) = parsed("struct C {} fn main() -> () { let x = C {}; x; x; return; }");
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        assert_eq!(
            check_source(source, &ast, &sources).unwrap_err()[0].code,
            "E0310"
        );
        let tokens = lexer::lex(source).unwrap();
        assert!(parser::parse(source, tokens)
            .unwrap()
            .uses_owned_syntax(source));
    }
}
