//! Connected private source controls. The production parser/CLI stay closed;
//! these real source programs still traverse paid typing and both raw proofs.
use super::*;
use crate::frontend::{
    declaration_index::SourceOwner,
    lexer,
    oir::Scalar,
    parser,
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

struct ProjectFixture(std::path::PathBuf);
impl ProjectFixture {
    fn new(text: &str) -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-stdin-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("main.ox"), text).unwrap();
        Self(path)
    }
    fn load(&self) -> crate::frontend::project::ProjectSources {
        crate::frontend::project::ProjectSources::load_builtin_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            crate::frontend::project::ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
}
impl Drop for ProjectFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

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
    )
    .unwrap();
    (sources, ast)
}

#[test]
fn builtin_source_default_parser_remains_closed() {
    let mut sources = SourceMap::new();
    let file = sources.add("closed-stdin-source.ox".into(), ZERO.into());
    let source = sources.get(file);
    assert!(parser::parse_with_mode(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::ProjectCandidate
    )
    .is_err());
}

#[test]
fn builtin_source_private_no_import_control_uses_paid_path() {
    let (sources, ast) = parsed("fn main()->i32{return 7;}");
    let owner = SourceOwner::original(
        sources.get(crate::frontend::source::SourceFileId(0)),
        &ast,
        SourceView::Map(&sources),
    )
    .unwrap();
    let output =
        program::run_builtin_source(owner, resolve::EnumPipelineRequest::REFERENCE).unwrap();
    assert_eq!(output.facts.result, Ok(Scalar::I32(7)));
    assert_eq!(output.facts.enum_count, 0);
    assert_eq!(output.facts.function_count, 1);
    assert!(output.facts.source_seed_before > 0);
    assert_eq!(
        output.facts.source_seed_before,
        output.facts.source_seed_after
    );
    assert_eq!(output.facts.source_usage.analysis, output.facts.raw_usage);
    assert_eq!(output.facts.raw_usage, output.facts.verified_usage);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn builtin_source_aliases_trailing_ids_zero_capacity_vertical() {
    let fixture = ProjectFixture::new(ZERO);
    let project = fixture.load();
    let owner = SourceOwner::project(&project);
    let output = program::run_builtin_source(
        owner,
        resolve::EnumPipelineRequest {
            emit_llvm: true,
            ..resolve::EnumPipelineRequest::REFERENCE
        },
    )
    .unwrap();
    assert_eq!(output.facts.result, Ok(Scalar::I32(39)));
    assert_eq!(
        (
            output.facts.enum_count,
            output.facts.variant_count,
            output.facts.function_count
        ),
        (2, 4, 3)
    );
    assert_eq!(
        output.facts.source_seed_before,
        output.facts.source_seed_after
    );
    assert_eq!(output.facts.source_usage.analysis, output.facts.raw_usage);
    assert_eq!(output.facts.raw_usage, output.facts.verified_usage);
    let module = output.llvm.unwrap().unwrap();
    assert!(module.contains("__oxid_read_stdin_byte"));
}

#[test]
fn builtin_source_new_enclosing_carriers_are_measured() {
    println!(
        "BUILTIN_SOURCE_CARRIERS builder={} caller={} shared_observation={} lower_controls={}",
        builtin_lower::carrier_bytes(),
        program::builtin_program_carrier_bytes(),
        program::enum_pipeline_program_carrier_bytes(),
        lower::invocation_control_bytes()
    );
    assert!(builtin_lower::carrier_bytes() > std::mem::size_of::<super::super::RawOwnedFunction>());
}

#[test]
fn builtin_source_association_independently_binds_import_anchors_and_suffix() {
    use super::super::*;
    use crate::frontend::declaration_index::{self as index, IndexLimits, WorkMeter};
    let fixture = ProjectFixture::new(ZERO);
    let project = fixture.load();
    let sources = project.sources();
    for mutation in 0..7 {
        let owner = SourceOwner::project(&project);
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index =
            index::collect_builtin_candidate(owner, IndexLimits::default(), &work, &mut allocator)
                .unwrap()
                .finish(&work, &mut allocator)
                .unwrap();
        let typed = resolve::type_builtin_source(&index, &work, &mut allocator).unwrap();
        let mut raw = super::lower::lower(&typed).unwrap();
        association::check_builtin_candidate(&raw, &index, sources).unwrap();
        // Exact requested lane capacities are observed after real reservations.
        let builtin = raw.functions.last().unwrap();
        for (length, capacity) in [
            (builtin.parameters.len(), builtin.parameters.capacity()),
            (builtin.owners.len(), builtin.owners.capacity()),
            (builtin.references.len(), builtin.references.capacity()),
            (builtin.blocks.len(), builtin.blocks.capacity()),
            (
                builtin.blocks[0].statements.len(),
                builtin.blocks[0].statements.capacity(),
            ),
        ] {
            assert_eq!(length, capacity);
        }
        let foreign_anchor = index.sources().eof();
        match mutation {
            0 => raw.builtins = BuiltinOrigins::None,
            1 => {
                let f = raw.functions.last_mut().unwrap();
                f.span = foreign_anchor;
                for row in &mut f.owners {
                    row.span = foreign_anchor;
                }
                for row in &mut f.references {
                    row.span = foreign_anchor;
                }
                for block in &mut f.blocks {
                    block.span = foreign_anchor;
                    for row in &mut block.statements {
                        row.span = foreign_anchor;
                    }
                    block.terminator.as_mut().unwrap().span = foreign_anchor;
                }
                // This remains a canonical raw thunk. Only source association
                // knows that its valid source range is the wrong import anchor.
                builtins::check(&raw).unwrap();
            }
            2 => {
                let e = raw.enums.last_mut().unwrap();
                e.span = foreign_anchor;
                for v in &mut e.variants {
                    v.span = foreign_anchor;
                }
                builtins::check(&raw).unwrap();
            }
            3 => {
                raw.functions.pop();
            }
            4 => {
                raw.enums.pop();
            }
            5 => {
                raw.functions[0].blocks[0].statements.push(OwnedStatement {
                    kind: OwnedInstruction::ReadStdin {
                        buffer: ReferenceParamId(0),
                        destination: OwnerPlaceId(0),
                    },
                    span: foreign_anchor,
                    diagnostic_origins: None,
                });
                builtins::check(&raw).unwrap();
            }
            6 => raw.functions.swap(0, 1),
            _ => unreachable!(),
        }
        assert_eq!(
            association::check_builtin_candidate(&raw, &index, sources)
                .unwrap_err()
                .code,
            "E0500",
            "mutation {mutation}"
        );
    }
}
