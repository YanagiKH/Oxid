use super::*;
use crate::frontend::{
    lexer,
    source::{SourceFileId, SourceMap},
};
fn parsed(text: &str, policy: StdImportPolicy) -> Result<Program, Vec<Diagnostic>> {
    let mut map = SourceMap::new();
    map.add("builtin.ox".into(), text.into());
    let source = map.get(SourceFileId(0));
    let mut allocator = Allocator::default();
    let mut storage = enums::SyntaxStorage::default();
    let parse = match policy {
        StdImportPolicy::OutputCandidate => parse_output_candidate_counted,
        StdImportPolicy::Candidate => parse_builtin_candidate_counted,
        StdImportPolicy::Enabled => parse_typed_counted,
        StdImportPolicy::Closed => parse_typed_closed_std_counted,
    };
    parse(
        source,
        lexer::lex(source).unwrap(),
        SourceMode::ProjectCandidate,
        MAX_NODES,
        &mut allocator,
        &mut storage,
    )
    .map(|(ast, _)| ast)
}
#[test]
fn builtin_import_parser_admits_only_current_catalog_and_direct_paths_stay_closed() {
    for text in [
        "use std::io::read_stdin; fn main()->(){return;}",
        "use std::io::ReadStatus as Status; fn main()->(){return;}",
        "use std::io::read_stdin as input; use std::io::ReadStatus; fn main()->(){return;}",
    ] {
        assert!(parsed(text, StdImportPolicy::Candidate).is_ok());
        assert!(parsed(text, StdImportPolicy::Enabled).is_ok());
        assert!(parsed(text, StdImportPolicy::Closed).is_err());
    }
    for text in [
        "use std::unknown::endpoint; fn main()->(){return;}",
        "use std::io::missing; fn main()->(){return;}",
        "use std::io; fn main()->(){return;}",
        "use std::io::ReadStatus::Full; fn main()->(){return;}",
    ] {
        assert!(parsed(text, StdImportPolicy::Candidate).is_ok());
        assert!(parsed(text, StdImportPolicy::Enabled).is_err());
        assert!(parsed(text, StdImportPolicy::Closed).is_err());
    }
    for text in [
        "fn main()->(){std::io::read_stdin();}",
        "fn main()->std::io::ReadStatus{return;}",
        "fn main()->(){std::io::ReadStatus::Full;}",
        "fn f(s:())->(){match s{std::io::ReadStatus::Full=>{}}}",
        "use std::io::*; fn main()->(){return;}",
        "use std::io::{ReadStatus,read_stdin}; fn main()->(){return;}",
    ] {
        assert!(parsed(text, StdImportPolicy::Candidate).is_err(), "{text}");
        assert!(parsed(text, StdImportPolicy::Enabled).is_err(), "{text}");
        assert!(parsed(text, StdImportPolicy::Closed).is_err(), "{text}");
    }
    for text in [
        "fn read_stdin()->i32{return 1;} fn std()->(){return;}",
        "use crate::std::read_stdin; fn main()->(){return;}",
    ] {
        assert!(parsed(text, StdImportPolicy::Candidate).is_ok());
        assert!(parsed(text, StdImportPolicy::Enabled).is_ok());
        assert!(parsed(text, StdImportPolicy::Closed).is_ok());
    }
}
#[test]
fn builtin_parser_policy_and_retained_carriers_are_measured() {
    use std::mem::{align_of, size_of};
    println!("builtin-parser-layout root={}/{} path={}/{} import={}/{} program={}/{} parser={}/{} std-policy={} storage={} parsed-result={}",size_of::<PathRoot>(),align_of::<PathRoot>(),size_of::<QualifiedPath>(),align_of::<QualifiedPath>(),size_of::<ImportDecl>(),align_of::<ImportDecl>(),size_of::<Program>(),align_of::<Program>(),size_of::<Parser<'_>>(),align_of::<Parser<'_>>(),size_of::<StdImportPolicy>(),size_of::<enums::SyntaxStorage>(),size_of::<Result<(Program,usize),Vec<Diagnostic>>>());
    assert_eq!(size_of::<PathRoot>(), 1);
    assert_eq!(size_of::<StdImportPolicy>(), 1);
}
