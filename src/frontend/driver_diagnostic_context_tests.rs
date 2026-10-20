//! Rendering-selection controls only. No source execution or native toolchain.
use super::*;

fn fixture(code: &'static str, stage: &'static str) -> (SourceMap, Diagnostic) {
    let mut sources = SourceMap::new();
    let id = sources.add("main.ox".into(), "bad_token\n".into());
    let diagnostic = *Diagnostic::new(
        code,
        stage,
        "test diagnostic",
        Some(sources.get(id).span(0, 9)),
    );
    (sources, diagnostic)
}

#[test]
fn source_context_requires_explicit_opt_in_and_a_reviewed_frontend_stage() {
    for stage in ["source", "lex", "parse", "resolve", "type", "ownership"] {
        let (sources, diagnostic) = fixture("E0300", stage);
        let old_text = diagnostic.render_human(&sources);
        let old_json = diagnostic.render_json(&sources);
        assert_eq!(
            render_compile_diagnostic(&diagnostic, &sources, false),
            old_text
        );
        let text = render_compile_diagnostic(&diagnostic, &sources, true);
        assert!(text.starts_with(&old_text), "{stage}: {text}");
        assert!(text.contains("bad_token"), "{stage}: {text}");
        assert!(text.contains('^'), "{stage}: {text}");
        assert_eq!(diagnostic.render_json(&sources), old_json);
    }
}

#[test]
fn backend_runtime_internal_and_unknown_stages_keep_the_original_text() {
    for (code, stage) in [
        ("E0701", "native-toolchain"),
        ("E0700", "native-admission"),
        ("E0500", "native-emission"),
        ("E0710", "project-build"),
        ("E0400", "compiler-session"),
        ("E0607", "oir-run"),
        ("E0500", "oir-verify"),
        ("E0500", "resolve"),
        ("E0703", "lexical-provider"),
        ("E0702", "hir-import"),
        ("E0703", "hir-producer"),
        ("E0300", "future-frontend-stage"),
    ] {
        // Even a primary span cannot opt a non-frontend diagnostic in.
        let (sources, diagnostic) = fixture(code, stage);
        assert_eq!(
            render_compile_diagnostic(&diagnostic, &sources, true),
            diagnostic.render_human(&sources),
            "{code} {stage}"
        );
    }
    let diagnostic = *Diagnostic::new("E0002", "source", "no source loaded", None);
    assert_eq!(
        render_compile_diagnostic(&diagnostic, &SourceMap::new(), true),
        "error[E0002] (source): no source loaded\n"
    );
}

#[test]
fn build_text_context_is_result_only_and_backend_failures_are_unchanged() {
    for (code, stage, policy, context) in [
        ("E0300", "type", EntryPolicy::Result, true),
        ("E0300", "type", EntryPolicy::Process, false),
        ("E0701", "native-toolchain", EntryPolicy::Result, false),
        ("E0710", "project-build", EntryPolicy::Result, false),
    ] {
        let (sources, diagnostic) = fixture(code, stage);
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            write_build_report(
                &sources,
                std::slice::from_ref(&diagnostic),
                false,
                policy,
                None,
                &mut out,
                &mut err,
            ),
            1
        );
        assert!(out.is_empty());
        if context {
            assert_eq!(
                err,
                super::super::diagnostic_context::render_human_context(&diagnostic, &sources)
                    .as_bytes()
            );
        } else {
            assert_eq!(err, diagnostic.render_human(&sources).as_bytes());
        }
    }
}

#[test]
fn build_json_uses_the_original_record_and_summary_for_both_entry_policies() {
    let (sources, diagnostic) = fixture("E0300", "type");
    for (policy, mode) in [
        (EntryPolicy::Result, "result"),
        (EntryPolicy::Process, "process"),
    ] {
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            write_build_report(
                &sources,
                std::slice::from_ref(&diagnostic),
                true,
                policy,
                None,
                &mut out,
                &mut err,
            ),
            1
        );
        assert!(err.is_empty());
        assert_eq!(
            String::from_utf8(out).unwrap(),
            format!(
                "{}\n{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"build-summary\",\"success\":false,\"errors\":1,\"output\":null,\"entry_mode\":\"{mode}\"}}\n",
                diagnostic.render_json(&sources)
            )
        );
    }
}
