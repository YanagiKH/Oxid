//! Connected private source controls. The production parser/CLI stay closed;
//! these real source programs still traverse paid typing and both raw proofs.
use super::*;
use crate::frontend::{
    declaration_index::SourceOwner,
    lexer, parser,
    oir::Scalar,
    project::budget::Allocator,
    source::{SourceMap, SourceView},
};

const ZERO: &str = r#"
use std::io::ReadStatus as Status;
use std::io::read_stdin as read;
enum User { Value(i32) }
fn helper(value: User) -> i32 {
    match value { User::Value(n) => { return n; }, }
}
fn main() -> i32 {
    let mut buffer: [i32; 0] = [];
    let status = read(&mut buffer);
    match status {
        Status::Eof(n) => { return n; },
        Status::Full => { return helper(User::Value(39)); },
        Status::IoError => { return -2; },
    }
}
"#;

fn parsed(text: &str) -> (SourceMap, crate::frontend::ast::Program) {
    let mut sources = SourceMap::new();
    let file = sources.add("private-stdin-source.ox".into(), text.into());
    let source = sources.get(file);
    let (ast, _) = parser::parse_builtin_candidate_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    ).unwrap();
    (sources, ast)
}

#[test]
fn builtin_source_default_parser_remains_closed() {
    let mut sources = SourceMap::new();
    let file = sources.add("closed-stdin-source.ox".into(), ZERO.into());
    let source = sources.get(file);
    assert!(parser::parse_with_mode(source, lexer::lex(source).unwrap(), parser::SourceMode::ProjectCandidate).is_err());
}

#[test]
fn builtin_source_private_no_import_control_uses_paid_path() {
    let (sources, ast) = parsed("fn main()->i32{return 7;}");
    let owner = SourceOwner::original(sources.get(crate::frontend::source::SourceFileId(0)), &ast, SourceView::Map(&sources)).unwrap();
    let output = program::run_builtin_source(owner, resolve::EnumPipelineRequest::REFERENCE).unwrap();
    assert_eq!(output.facts.result, Ok(Scalar::I32(7)));
    assert_eq!(output.facts.enum_count, 0);
    assert_eq!(output.facts.function_count, 1);
    assert!(output.facts.source_seed_before > 0);
    assert_eq!(output.facts.source_seed_before, output.facts.source_seed_after);
    assert_eq!(output.facts.source_usage.analysis, output.facts.raw_usage);
    assert_eq!(output.facts.raw_usage, output.facts.verified_usage);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn builtin_source_aliases_trailing_ids_zero_capacity_vertical() {
    let (sources, ast) = parsed(ZERO);
    let owner = SourceOwner::original(sources.get(crate::frontend::source::SourceFileId(0)), &ast, SourceView::Map(&sources)).unwrap();
    let output = program::run_builtin_source(owner, resolve::EnumPipelineRequest { emit_llvm: true, ..resolve::EnumPipelineRequest::REFERENCE }).unwrap();
    assert_eq!(output.facts.result, Ok(Scalar::I32(39)));
    assert_eq!((output.facts.enum_count, output.facts.variant_count, output.facts.function_count), (2, 4, 3));
    assert_eq!(output.facts.source_seed_before, output.facts.source_seed_after);
    assert_eq!(output.facts.source_usage.analysis, output.facts.raw_usage);
    assert_eq!(output.facts.raw_usage, output.facts.verified_usage);
    let module = output.llvm.unwrap().unwrap();
    assert!(module.contains("__oxid_read_stdin_byte"));
}

#[test]
fn builtin_source_new_enclosing_carriers_are_measured() {
    println!("BUILTIN_SOURCE_CARRIERS builder={} caller={} shared_observation={} lower_controls={}", builtin_lower::carrier_bytes(), program::builtin_program_carrier_bytes(), program::enum_pipeline_program_carrier_bytes(), lower::invocation_control_bytes());
    assert!(builtin_lower::carrier_bytes() > std::mem::size_of::<super::super::RawOwnedFunction>());
}
