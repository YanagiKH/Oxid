//! Source-only facade. The leaf alone binds source, body and entry.
#[cfg(test)]
use super::*;
#[cfg(test)]
use crate::frontend::ast;
pub(in crate::frontend::oir) mod association;
mod sealed;
pub(in crate::frontend::oir) use sealed::enum_facade_carrier_bytes;
pub(in crate::frontend::oir) use sealed::{
    check_project_candidate, check_project_executable_candidate,
};
pub(in crate::frontend) use sealed::{check_source, CheckedSourceProgram, ProcessFailure};

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
        assert!(matches!(
            checked.route(),
            super::super::project::ProjectRoute::Scalar
        ));
        assert_eq!(checked.entry(), Some(hir::DefId(1)));
        assert_eq!(checked.function_count(), 2);
        assert_eq!(checked.run().unwrap(), Scalar::I32(11));
        let old = lower_and_verify(
            &typeck::check(hir::resolve(source, &ast).unwrap()).unwrap(),
            &sources,
        )
        .unwrap();
        assert_eq!(
            checked.native_module().unwrap(),
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
            checked.run().unwrap_err().render_json(&sources),
            old.run(None)
                .unwrap_err()
                .diagnostic(&sources)
                .render_json(&sources)
        );
        assert_eq!(
            checked.native_module().unwrap_err().render_json(&sources),
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
                checked.run().unwrap_err().render_json(&sources),
                old.run(Some(hir::DefId(0)))
                    .unwrap_err()
                    .diagnostic(&sources)
                    .render_json(&sources),
            );
        }
    }

    #[test]
    fn owned_dispatch_verifies_entire_module_and_preserves_entry_identity() {
        let (sources, ast) = parsed(
            "struct C { n: i32 } fn helper() -> i32 { return 7; } fn main() -> i32 { let x = C { n: helper() }; return x.n + 4; }",
        );
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        let checked = check_source(source, &ast, &sources).unwrap();
        assert!(matches!(
            checked.route(),
            super::super::project::ProjectRoute::Owned
        ));
        assert_eq!(checked.entry(), Some(hir::DefId(1)));
        assert_eq!(checked.function_count(), 2);
        assert_eq!(checked.run().unwrap(), Scalar::I32(11));
        let module = checked.native_module().unwrap();
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
            assert!(matches!(
                checked.route(),
                super::super::project::ProjectRoute::Owned
            ));
            assert_eq!(checked.run().unwrap_err().code, run_code);
            assert_eq!(checked.native_module().unwrap_err().code, native_code);
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
    #[test]
    fn original_association_failures_have_no_unchecked_origins() {
        let (sources, ast) = parsed("fn main()->i32{return 1;}");
        let (substitute, other_ast) = parsed("fn main()->i32{return 2;}");
        let source = sources.get(crate::frontend::source::SourceFileId(0));
        for (map, program) in [(&substitute, &ast), (&sources, &other_ast)] {
            let errors = check_source(source, program, map).unwrap_err();
            assert_eq!(
                (errors[0].code, errors[0].stage),
                ("E0500", "resolve-project")
            );
            assert!(errors[0].primary.is_none());
            assert!(errors[0].secondary.is_empty());
            assert!(errors[0].render_json(map).contains("\"primary\":null"));
            assert!(!errors[0].render_human(map).contains("-->"));
        }
        let empty = SourceMap::new();
        let error = check_source(source, &ast, &empty).unwrap_err().remove(0);
        assert!(error.primary.is_none());
        error.render_json(&empty);
    }

    #[test]
    fn original_result_does_not_borrow_parser_or_work_state() {
        let mut sources = SourceMap::new();
        let id = sources.add("lifetime.ox".into(), "fn main()->i32{return 3;}".into());
        let checked = {
            let source = sources.get(id);
            let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
            check_source(source, &ast, &sources).unwrap()
        };
        assert_eq!(checked.run().unwrap(), Scalar::I32(3));
    }
    #[test]
    fn original_map_association_preserves_nonzero_file_identity() {
        let mut map = SourceMap::new();
        map.add("decoy.ox".into(), "fn main()->i32{return 99;}".into());
        let id = map.add("actual.ox".into(), "fn main()->i32{return 3;}".into());
        let source = map.get(id);
        let ast = parser::parse(source, lexer::lex(source).unwrap()).unwrap();
        let checked = check_source(source, &ast, &map).unwrap();
        assert_eq!(checked.run().unwrap(), Scalar::I32(3));
        assert_eq!(checked.entry(), Some(hir::DefId(0)));
        let mut substitute = SourceMap::new();
        substitute.add("decoy.ox".into(), "fn main()->i32{return 99;}".into());
        substitute.add("actual.ox".into(), "fn main()->i32{return 3;}".into());
        let error = check_source(source, &ast, &substitute)
            .unwrap_err()
            .remove(0);
        assert_eq!(error.code, "E0500");
        assert!(error.primary.is_none());
        assert!(error.secondary.is_empty());
        error.render_json(&substitute);
    }
}
