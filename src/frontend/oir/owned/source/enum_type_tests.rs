//! Parsed-source enum typing through the closed fixed-statistics entry only.
use super::*;
use crate::frontend::{
    declaration_index::{
        collect_enum_candidate, DeclarationIndex, IndexLimits, Observation, SourceOwner, WorkMeter,
    },
    lexer, parser,
    source::{SourceFileId, SourceMap, SourceView},
};

fn with_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
    let mut sources = SourceMap::new();
    sources.add("enum-type.ox".into(), text.into());
    let file = sources.get(SourceFileId(0));
    let ast = parser::parse_enum_candidate_counted(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0;
    let owner = SourceOwner::original(file, &ast, SourceView::Map(&sources)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    action(&index);
}

#[test]
fn bounded_enum_type_parsed_values_payloads_nested_arms_and_loop_exits_succeed() {
    let cases = [
        "enum E{N,V(i32),B(bool),U(())} fn relay(x:E)->E{return x;} fn take(x:E)->i32{match x{E::N=>{return 0;},E::V(v)=>{return v;},E::B(b)=>{if b{return 1;}else{return 0;}},E::U(u)=>{u;return 0;}}} fn main()->i32{return take(relay(E::V(7)));}",
        "enum E{A,B} fn take(x:E)->i32{match x{E::A=>{},E::B=>{return 1;}}return 0;} fn main()->i32{return take(E::A);}",
        "enum E{A,B} fn take(x:E,y:E)->i32{match x{E::A=>{match y{E::A=>{return 1;},E::B=>{return 2;}}},E::B=>{return 3;}}} fn main()->i32{return take(E::A,E::B);}",
        "enum E{N,V(i32)} fn take(x:E)->i32{let mut n=0;while n<2{match x{E::N=>{break;},E::V(v)=>{if v>0{return v;}else{n=n+1;continue;}}}}return n;} fn main()->i32{return take(E::N);}",
        "enum E{A,U(())} fn take(x:E)->(){match x{E::A=>{return;},E::U(u)=>{return u;}}} fn main()->(){take(E::U(()));return;}",
        "enum E{A,B} fn take(x:E)->i32{let mut y:E=x;y=E::B;match y{E::A=>{return 0;},E::B=>{return 1;}}} fn main()->i32{return take(E::A);}",
    ];
    for text in cases {
        with_index(text, |index| {
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(256).unwrap();
            let (result, (_, live, peak)) =
                super::super::reviewer_source::integration_measured(|| {
                    super::super::resolve::probe_enum_type_storage(index, &work, &mut allocator)
                });
            let facts = result.unwrap().unwrap();
            assert_eq!(live, 0, "{text}");
            assert!(peak > 0);
            assert_eq!(facts.typed.path_vectors, 0);
            assert_eq!(facts.typed.final_cell, facts.resolver.plan.total);
            assert_eq!(
                allocator.attempts,
                facts.resolver.reservation_attempts + facts.typed.typed_attempts
            );
            assert!(!allocator.observer_trace_overflow);
        });
    }
}

#[test]
fn bounded_enum_type_parsed_negative_semantics_fail_without_owner_escape() {
    let cases = [
        ("enum E{V(i32)} fn main()->i32{let x=E::V(true);return 0;}", "E0300"),
        ("enum E{N} fn main()->i32{let x=E::N(1);return 0;}", "E0300"),
        ("enum E{V(i32)} fn main()->i32{let x=E::V;return 0;}", "E0300"),
        ("enum E{A,B} enum F{A,B} fn take(x:E)->i32{match x{F::A=>{return 0;},F::B=>{return 1;}}} fn main()->i32{return 0;}", "E0300"),
        ("enum E{A,B} fn take(x:E)->i32{match x{E::A=>{return 0;},E::A=>{return 1;}}} fn main()->i32{return 0;}", "E0300"),
        ("enum E{A,B} fn take(x:E)->i32{match x{E::A=>{return 0;}}} fn main()->i32{return 0;}", "E0300"),
        ("enum E{V(bool)} fn take(x:E)->i32{match x{E::V(v)=>{return v;}}} fn main()->i32{return 0;}", "E0300"),
        ("enum E{A,B} fn take(x:E)->i32{match x{E::A=>{},E::B=>{return 1;}}} fn main()->i32{return 0;}", "E0302"),
        ("enum E{A,B} fn take(x:E)->i32{match x{E::A=>{return 0;},E::B=>{return 1;}}return 2;} fn main()->i32{return 0;}", "E0303"),
        ("enum E{V(i32)} fn take(x:E)->i32{match x{E::V(v)=>{v;}}return v;} fn main()->i32{return 0;}", "E0200"),
        ("enum E{A} enum F{A} fn take()->E{return F::A;} fn main()->i32{return 0;}", "E0300"),
    ];
    for (text, code) in cases {
        with_index(text, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(256).unwrap();
            let errors = super::super::resolve::probe_enum_type_storage(
                index,
                &WorkMeter::default(),
                &mut allocator,
            )
            .unwrap_err();
            assert!(
                errors.iter().any(|error| error.code == code),
                "{text}: {errors:?}"
            );
            assert!(!allocator.observer_trace_overflow);
        });
    }
}

#[test]
fn bounded_enum_type_constructor_payload_and_arm_binding_are_observed_once() {
    let text = "enum E{V(i32)} fn number()->i32{return 7;} fn main()->i32{let e=E::V(number());match e{E::V(v)=>{return v;}}}";
    with_index(text, |index| {
        let work = WorkMeter::default();
        work.enable_observation();
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(128).unwrap();
        super::super::resolve::probe_enum_type_storage(index, &work, &mut allocator)
            .unwrap()
            .unwrap();
        let observations = work.observations.borrow();
        let payloads = observations
            .iter()
            .filter(|event| {
                matches!(event,
            Observation::Expression { origin, ty: ValueTy::Scalar(Ty::I32), .. }
                if index.sources().text(*origin).unwrap() == "number()" )
            })
            .count();
        let binders = observations
            .iter()
            .filter(|event| {
                matches!(event,
            Observation::Binding { origin, ty: ParameterTy::Value(ValueTy::Scalar(Ty::I32)), .. }
                if index.sources().text(*origin).unwrap() == "v" )
            })
            .count();
        assert_eq!(payloads, 1);
        assert_eq!(binders, 1);
    });
}

#[test]
fn bounded_enum_pipeline_enum_free_and_preflight_guards_remain_inert() {
    with_index("fn main()->i32{return 0;}", |index| {
        let work = WorkMeter::new(0);
        let mut allocator = Allocator {
            attempts: 7,
            ..Allocator::default()
        };
        let (result, heap) = super::super::reviewer_source::integration_measured(|| {
            super::super::resolve::probe_enum_pipeline(index, &work, &mut allocator)
        });
        assert!(result.unwrap().is_none());
        assert_eq!(heap, (0, 0, 0));
        assert_eq!(allocator.attempts, 7);
        assert!(allocator.trace.is_empty());
    });
    with_index("enum E{V} fn main()->i32{return 0;}", |index| {
        let work = WorkMeter::new(0);
        let mut allocator = Allocator {
            attempts: 7,
            ..Allocator::default()
        };
        let errors =
            super::super::resolve::probe_enum_pipeline(index, &work, &mut allocator).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "E0400");
        assert_eq!(allocator.attempts, 7);
        assert!(allocator.trace.is_empty());
    });
    assert!(!SourceAdmission::EnumPipeline.executable());
    assert!(SourceAdmission::EnumPipeline.allows_lowering());
    assert!(!SourceAdmission::ObserveEnumTypes.allows_lowering());
}

#[test]
fn bounded_enum_pipeline_precursor_named_carrier_layouts() {
    println!("ENUM_PIPELINE_LAYOUT source_extra={} type_controls={} program_controls={} observation={} observation_return={} typed_facts={} typed_return={} program_facts={} program_return={}",
        super::super::resolve::enum_pipeline_source_extra_bytes(),
        enum_pipeline_type_carrier_bytes(),
        super::super::program::enum_pipeline_program_carrier_bytes(),
        std::mem::size_of::<super::super::resolve::EnumPipelineObservation>(),
        std::mem::size_of::<Result<Option<super::super::resolve::EnumPipelineObservation>, Vec<Diagnostic>>>(),
        std::mem::size_of::<EnumPipelineTypedFacts>(),
        std::mem::size_of::<Result<EnumPipelineTypedFacts, Vec<Diagnostic>>>(),
        std::mem::size_of::<super::super::program::EnumPipelineFacts>(),
        std::mem::size_of::<Result<super::super::program::EnumPipelineFacts, Vec<Diagnostic>>>());
}

#[test]
fn bounded_enum_pipeline_tiny_relay_executes_after_full_proof_and_releases_backing() {
    // Independent source-enum-independent-fixtures-d6249e7.json tiny_relay_take.
    // SHA256 95bbfb95b733bc272e2b11e6772b66a74bf14507f4a86a7f3e9258d4c88d2220.
    const TEXT: &str = "enum E{N,V(i32)} fn relay(x:E)->E{return x;} fn take(x:E)->i32{match x{E::N=>{return 0;},E::V(v)=>{return v;},}} fn main()->i32{return take(relay(E::V(7)));}";
    with_index(TEXT, |index| {
        for preused in [false, true] {
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(256).unwrap();
            let mut sentinel = Vec::<u8>::new();
            if preused {
                allocator
                    .vector_exact(&mut sentinel, 3, "pipeline sentinel")
                    .unwrap();
                sentinel.extend_from_slice(&[17, 29, 43]);
            }
            let before = allocator.attempts;
            let trace_capacity = allocator.trace.capacity();
            let trace_pointer = allocator.trace.as_ptr();
            let (result, (calls, live, peak)) =
                super::super::reviewer_source::integration_measured(|| {
                    super::super::resolve::probe_enum_pipeline(index, &work, &mut allocator)
                });
            let facts = result.unwrap().unwrap();
            assert_eq!(facts.pipeline.result, crate::frontend::oir::Scalar::I32(7));
            assert_eq!(
                (
                    facts.pipeline.enum_count,
                    facts.pipeline.variant_count,
                    facts.pipeline.function_count,
                    facts.pipeline.match_count,
                    facts.pipeline.arm_count
                ),
                (1, 2, 3, 1, 2)
            );
            assert_eq!(facts.pipeline.raw_usage.owners, 7);
            assert_eq!(facts.pipeline.raw_usage.expanded_events, 28);
            assert_eq!(facts.pipeline.verified_usage.owners, 7);
            assert_eq!(facts.pipeline.verified_usage.expanded_events, 28);
            assert_eq!(
                facts.pipeline.source_usage.analysis,
                facts.pipeline.raw_usage
            );
            assert_eq!(facts.pipeline.source_seed_before, facts.typed.final_cell);
            assert_eq!(facts.pipeline.source_seed_after, facts.typed.final_cell);
            assert_eq!(facts.typed.final_cell, facts.resolver.plan.total);
            assert_eq!(
                allocator.attempts.checked_sub(before).unwrap(),
                facts
                    .resolver
                    .reservation_attempts
                    .checked_add(facts.typed.typed_attempts)
                    .unwrap()
            );
            assert_eq!(live, 0);
            assert!(calls > 0 && peak > 0);
            assert_eq!(allocator.trace.capacity(), trace_capacity);
            assert_eq!(allocator.trace.as_ptr(), trace_pointer);
            assert!(!allocator.observer_trace_overflow);
            assert!(allocator.trace.iter().all(|event| event.success));
            if preused {
                assert_eq!(sentinel, [17, 29, 43]);
                assert_eq!(sentinel.capacity(), 3);
                assert_eq!(allocator.trace[0].kind, "pipeline sentinel");
                assert_eq!(allocator.trace[0].length, 3);
            }
            println!("ENUM_PIPELINE_TINY preused={preused} result=7 resolver={} typed={} seed={} calls={calls} live={live} peak={peak}",
                facts.resolver.reservation_attempts, facts.typed.typed_attempts, facts.typed.final_cell);
        }
    });
}

#[test]
fn bounded_enum_pipeline_scanner_canonical_project_returns_115() {
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    let project = ProjectSources::load_enum_index_candidate(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/bounded_enum_scanner/main.ox"
        ),
        ProjectLimits::default(),
        &mut Allocator::default(),
    )
    .unwrap();
    let index_work = WorkMeter::default();
    let mut index_allocator = Allocator::default();
    let index = collect_enum_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &index_work,
        &mut index_allocator,
    )
    .unwrap()
    .finish(&index_work, &mut index_allocator)
    .unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    allocator.observer_trace_bound(1024).unwrap();
    let (result, (calls, live, peak)) = super::super::reviewer_source::integration_measured(|| {
        super::super::resolve::probe_enum_pipeline(&index, &work, &mut allocator)
    });
    let facts = result.unwrap().unwrap();
    // Independent pilot oracle: Integer(12), Plus, Integer(3), End => 12+100+3.
    assert_eq!(
        facts.pipeline.result,
        crate::frontend::oir::Scalar::I32(115)
    );
    assert_eq!(
        (
            facts.pipeline.enum_count,
            facts.pipeline.variant_count,
            facts.pipeline.function_count,
            facts.pipeline.match_count,
            facts.pipeline.arm_count
        ),
        (1, 4, 3, 1, 4)
    );
    assert_eq!(
        facts.pipeline.source_usage.analysis,
        facts.pipeline.raw_usage
    );
    assert_eq!(
        facts.pipeline.verified_usage.owners,
        facts.pipeline.raw_usage.owners
    );
    assert_eq!(facts.pipeline.source_seed_before, facts.typed.final_cell);
    assert_eq!(facts.pipeline.source_seed_after, facts.typed.final_cell);
    assert!(facts.typed.path_vectors > 0);
    assert!(facts.typed.final_cell > facts.resolver.plan.total);
    assert_eq!(
        allocator.attempts,
        facts.resolver.reservation_attempts + facts.typed.typed_attempts
    );
    assert_eq!(live, 0);
    assert!(calls > 0 && peak > 0);
    assert!(!allocator.observer_trace_overflow);
    assert!(allocator.trace.iter().all(|event| event.success));
    println!("ENUM_PIPELINE_SCANNER result=115 resolver={} typed={} paths={} seed={} calls={calls} live={live} peak={peak}",
        facts.resolver.reservation_attempts, facts.typed.typed_attempts,
        facts.typed.path_vectors, facts.typed.final_cell);
}
