//! Private resolver semantics and independent storage controls, never executable admission.
use super::*;
use crate::frontend::{
    lexer, parser,
    project::{ProjectLimits, ProjectSources},
};
use std::mem::{align_of, size_of};

pub(super) fn with_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
    let mut sources = SourceMap::new();
    let file = sources.add("resolver-enum.ox".into(), text.into());
    let source = sources.get(file);
    let ast = parser::parse_enum_candidate_counted(
        source,
        lexer::lex(source).unwrap(),
        parser::SourceMode::ProjectCandidate,
        parser::MAX_NODES,
        &mut Allocator::default(),
        &mut Default::default(),
    )
    .unwrap()
    .0;
    if ast.uses_project_syntax() {
        // Absolute paths need an honestly loaded project owner. The original
        // single-file adapter correctly rejects project syntax; keep that fence.
        let fixture = ProjectFixture::new(&[("main.ox", text)]);
        let project = fixture.candidate();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = index::collect_enum_candidate(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        action(&index);
        return;
    }
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = index::collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    action(&index);
}

struct ProjectFixture(std::path::PathBuf);
impl ProjectFixture {
    fn new(files: &[(&str, &str)]) -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-resolver-enum-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        for (name, text) in files {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        Self(root)
    }
    fn candidate(&self) -> ProjectSources {
        ProjectSources::load_enum_index_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
            &mut Allocator::default(),
        )
        .unwrap()
    }
    fn ordinary(&self) -> ProjectSources {
        ProjectSources::load_project_candidate(
            self.0.join("main.ox").to_str().unwrap(),
            ProjectLimits::default(),
        )
        .unwrap()
    }
}
impl Drop for ProjectFixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

const POSITIVE: &str = "
    enum E{Num(i32),Other(i32),Unit(()),End}
    fn relay(value:E)->E{return value;}
    fn take(value:E)->i32{match value{
        E::Other(payload)=>{return payload;},
        E::Num(payload)=>{return payload;},
        E::Unit(unit)=>{unit;return 0;},
        E::End=>{return 0;}
    }}
    fn number()->i32{return 7;}
    fn main()->i32{let token:E=relay(E::Num(crate::number()));return take(token);}
";

#[test]
fn bounded_enum_resolver_values_constructors_and_arm_bindings_are_nominal_and_scoped() {
    with_index(POSITIVE, |index| {
        let work = WorkMeter::default();
        work.enable_observation();
        let plan = super::super::hir_budget::preflight_enum_hir(index, &work)
            .unwrap()
            .unwrap();
        let mut paid = PaidStorage::new(plan.counts);
        let mut allocator = Allocator::default();
        allocator.observer_trace_bound(256).unwrap();
        let parts = resolve_index_impl(index, &work, &mut allocator, Some(&mut paid)).unwrap();
        assert!(matches!(
            parts.1[0].result,
            ValueTy::Owned(AggregateTy::Enum(_))
        ));
        assert_eq!(parts.1[0].params[0], ParameterTy::Value(parts.1[0].result));
        let consume = &parts.2[1];
        let StmtKind::Match { scrutinee, arms } = &consume.blocks[0].body[0].kind else {
            panic!("match")
        };
        assert_eq!(*scrutinee, BindingId(0));
        assert_eq!(
            arms.iter().map(|arm| arm.variant.index).collect::<Vec<_>>(),
            [1, 0, 2, 3]
        );
        assert_eq!(arms.len(), 4);
        for (position, expected) in [Some(Ty::I32), Some(Ty::I32), Some(Ty::Unit), None]
            .into_iter()
            .enumerate()
        {
            assert_eq!(arms[position].binding.is_some(), expected.is_some());
            if let Some(binding) = arms[position].binding {
                let binding = &consume.bindings[binding.0];
                assert!(!binding.mutable);
                assert_eq!(binding.scope, arms[position].body);
                assert_eq!(binding.annotation, expected.map(ValueTy::Scalar));
                assert!(binding.parameter_position.is_none());
            }
        }
        assert_ne!(arms[0].binding, arms[1].binding);
        let calls = parts
            .2
            .iter()
            .flat_map(|function| &function.expressions)
            .filter(|expr| matches!(expr.kind, ExprKind::Call { .. }))
            .count();
        let constructors = parts
            .2
            .iter()
            .flat_map(|function| &function.expressions)
            .filter(|expr| matches!(expr.kind, ExprKind::ConstructEnum { .. }))
            .count();
        assert_eq!((calls, constructors), (3, 1));
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|event| event.kind == "paid HIR arguments")
                .count(),
            calls
        );
        assert_eq!(
            allocator
                .trace
                .iter()
                .filter(|event| event.kind == "paid HIR match arms")
                .count(),
            1
        );
        let inventory = storage::inventory_parts(&parts, &work, index.sources().eof()).unwrap();
        let facts = paid
            .reconcile(
                &plan,
                inventory,
                allocator.attempts,
                &work,
                index.sources().eof(),
            )
            .unwrap();
        assert_eq!(facts.retained_counts[Kind::MatchArms as usize], 4);
        assert_eq!(facts.capacities[Kind::MatchArms as usize], 4);
        assert_eq!(facts.retained_counts[Kind::Bindings as usize], 6);
        assert_eq!(
            work.observations
                .borrow()
                .iter()
                .filter(|event| matches!(event,
                    index::Observation::Target { operation: "callee", origin, .. }
                        if index.sources().text(*origin).unwrap() == "crate::number"
                ))
                .count(),
            1
        );
        assert_eq!(
            resolve_index(index, &work, &mut Allocator::default()).unwrap_err()[0].code,
            "E0101"
        );
    });
}

#[test]
fn bounded_enum_resolver_rejects_constructor_shape_before_payload_resolution() {
    for expression in [
        "E::End()",
        "E::Num",
        "E::Num()",
        "E::Num(missing, missing)",
        "E::Num(&missing)",
    ] {
        with_index(
            &format!("enum E{{End,Num(i32)}} fn main()->(){{{expression};return;}}"),
            |index| {
                let errors = probe_enum_resolver_storage(
                    index,
                    &WorkMeter::default(),
                    &mut Allocator::default(),
                )
                .unwrap_err();
                assert_eq!(errors[0].code, "E0300", "{expression}: {errors:?}");
                assert!(!errors[0].message.contains("unknown local"));
            },
        );
    }
    for expression in ["E::End", "E::Num(7)", "E::Flag(true)", "E::Unit(())"] {
        with_index(
            &format!(
                "enum E{{End,Num(i32),Flag(bool),Unit(())}} fn main()->(){{{expression};return;}}"
            ),
            |index| {
                assert!(probe_enum_resolver_storage(
                    index,
                    &WorkMeter::default(),
                    &mut Allocator::default()
                )
                .unwrap()
                .is_some());
            },
        );
    }
}

#[test]
fn bounded_enum_resolver_checks_all_pattern_shapes_and_lexical_scopes() {
    for (body, code) in [
        ("match token{E::A(x)=>{return x;}}", "E0300"),
        (
            "match token{E::A(x)=>{return x;},E::A(y)=>{return y;}}",
            "E0300",
        ),
        (
            "match token{E::A(x)=>{return x;},F::B=>{return 0;}}",
            "E0300",
        ),
        ("match token{E::A=>{return 0;},E::B=>{return 0;}}", "E0300"),
        (
            "match token{E::A(x)=>{return x;},E::B(x)=>{return x;}}",
            "E0300",
        ),
        (
            "match token{E::A(token)=>{return 0;},E::B=>{return 0;}}",
            "E0201",
        ),
        (
            "match token{E::A(helper)=>{return 0;},E::B=>{return 0;}}",
            "E0201",
        ),
        ("match token{E::A(x)=>{x;},E::B=>{}} return x;", "E0200"),
        (
            "match token{E::A(x)=>{return x;},E::B=>{return missing;}}",
            "E0200",
        ),
    ] {
        let source = format!("enum E{{A(i32),B}} enum F{{B}} fn helper()->(){{return;}} fn test(token:E)->i32{{{body}}}");
        with_index(&source, |index| {
            let errors = probe_enum_resolver_storage(
                index,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap_err();
            assert_eq!(errors[0].code, code, "{body}: {errors:?}");
        });
    }
}

#[test]
fn bounded_enum_resolver_zero_enum_match_is_denied_in_the_eager_walk() {
    for text in [
        "fn main()->(){match token{E::V=>{return;}}}",
        "fn main()->i32{return missing;} fn unused()->(){match token{E::V=>{return;}}}",
        "fn main()->(){if true{match token{E::V=>{return;}}}return;}",
    ] {
        with_index(text, |index| {
            let work = WorkMeter::default();
            work.enable_observation();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(64).unwrap();
            let errors = resolve_index_impl(index, &work, &mut allocator, None).unwrap_err();
            assert_eq!(errors[0].code, "E0101");
            assert_eq!(allocator.attempts, 0);
            assert!(!work
                .observations
                .borrow()
                .iter()
                .any(|event| matches!(event, index::Observation::Phase("body-resolution"))));
        });
    }
    with_index(
        "fn bad(value:Missing)->(){match token{E::V=>{}}}",
        |index| {
            let errors = resolve_index_impl(
                index,
                &WorkMeter::default(),
                &mut Allocator::default(),
                None,
            )
            .unwrap_err();
            assert_eq!(
                index.sources().text(errors[0].primary.unwrap()).unwrap(),
                "Missing"
            );
        },
    );
}

#[test]
fn bounded_enum_resolver_cursor_scales_with_depth_not_arm_count() {
    for count in [1, 64, 65, 128, 129, 192, 193, 256] {
        let variants = (0..count)
            .map(|i| format!("V{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let arms = (0..count)
            .rev()
            .map(|i| format!("E::V{i}=>{{return;}}"))
            .collect::<Vec<_>>()
            .join(",");
        with_index(
            &format!("enum E{{{variants}}} fn consume(token:E)->(){{match token{{{arms}}}}}"),
            |index| {
                let facts = probe_enum_resolver_storage(
                    index,
                    &WorkMeter::default(),
                    &mut Allocator::default(),
                )
                .unwrap()
                .unwrap();
                assert_eq!(facts.plan.counts.max_body_depth, 2);
                assert_eq!(facts.plan.counts.resolve_frames, 16);
                assert_eq!(facts.capacities[Kind::Frames as usize], 16);
                assert_eq!(facts.retained_counts[Kind::MatchArms as usize], count);
            },
        );
    }
    for depth in [1, 31, 63] {
        let mut text = "enum E{V} fn consume(token:E)->(){".to_owned();
        for _ in 0..depth {
            text.push_str("match token{E::V=>{");
        }
        text.push_str("return;");
        for _ in 0..depth {
            text.push_str("}}");
        }
        text.push('}');
        with_index(&text, |index| {
            let facts = probe_enum_resolver_storage(
                index,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap()
            .unwrap();
            assert_eq!(facts.plan.counts.max_body_depth, depth + 1);
            assert_eq!(facts.plan.counts.resolve_frames, 4 * (depth + 1) + 8);
            assert_eq!(facts.retained_counts[Kind::MatchArms as usize], depth);
        });
    }
}

#[test]
fn bounded_enum_resolver_allocation_failures_drop_arm_vectors_and_preused_state() {
    with_index(POSITIVE, |index| {
        let facts =
            probe_enum_resolver_storage(index, &WorkMeter::default(), &mut Allocator::default())
                .unwrap()
                .unwrap();
        for preused in [0, 1] {
            for ordinal in 1..=facts.reservation_attempts {
                let mut allocator = Allocator::default();
                // Observation backing is test-owned. Admit it before the heap
                // window, including the optional preused sentinel reservation.
                allocator
                    .observer_trace_bound(facts.reservation_attempts + preused)
                    .unwrap();
                if preused != 0 {
                    allocator
                        .vector_exact(&mut Vec::<u8>::new(), 0, "preused resolver allocator")
                        .unwrap();
                }
                allocator.fail_at = Some(preused + ordinal);
                let (_, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                    let result =
                        probe_enum_resolver_storage(index, &WorkMeter::default(), &mut allocator);
                    assert!(result.is_err(), "preused={preused}, ordinal={ordinal}");
                    assert!(result
                        .unwrap_err()
                        .iter()
                        .any(|error| error.code == "E0400"));
                });
                assert_eq!(live, 0, "preused={preused}, ordinal={ordinal}");
                assert!(!allocator.observer_trace_overflow);
            }
        }
    });
}

#[test]
fn bounded_enum_resolver_actual_changed_carrier_layouts() {
    macro_rules! layout { ($($ty:ty),* $(,)?) => { $(
        println!("ENUM_RESOLVER_LAYOUT {} bytes={} align={}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
    )* }; }
    layout!(ResolveFrame, Option<ResolveFrame>, Vec<ResolveFrame>, MatchBuilder, MatchArm,
        Vec<MatchArm>, Result<Vec<MatchArm>, Box<Diagnostic>>, PaidStorage,
        storage::ResolverInventory, storage::ResolverStorageObservation,
        Option<storage::ResolverStorageObservation>, Result<Option<storage::ResolverStorageObservation>, Vec<Diagnostic>>,
        QualifiedValueEndpoint, Result<QualifiedValueEndpoint, Box<Diagnostic>>,
        index::EnumView<'_>, Result<index::EnumView<'_>, Box<Diagnostic>>,
        index::VariantView<'_>, Result<index::VariantView<'_>, Box<Diagnostic>>,
        Expr, Stmt, Function, ResolvedOwnedProgram<'_>);
    layout!(
        (Option<Ty>, Option<&[ast::Argument]>), Result<ExprId, Box<Diagnostic>>,
        ast::ExprId, &ast::ExprId, Option<&Vec<ast::Argument>>, BindingId,
        crate::frontend::oir::owned_types::EnumId, Span,
        &Vec<ast::MatchArmSyntax>, &mut Vec<MatchArm>
    );
    println!(
        "ENUM_RESOLVER_CHARGES expression={} match={}",
        enum_expression_carrier_bytes(),
        enum_match_carrier_bytes()
    );
    assert_eq!(storage::KINDS, 18);
    assert_eq!(storage::RETAINED_KINDS, 13);
    assert_eq!(
        storage::WIDTHS[Kind::MatchArms as usize],
        size_of::<MatchArm>()
    );
    assert!(enum_match_carrier_bytes() >= size_of::<MatchBuilder>());
}

#[test]
fn bounded_enum_resolver_zero_enum_qualified_calls_preserve_the_legacy_route_exactly() {
    for text in [
        "fn get()->i32{return 7;} fn main()->i32{return crate::get();}",
        "fn first()->i32{return 1;} fn pair(a:i32,b:i32)->i32{return a+b;} fn main()->i32{return crate::pair(crate::first(),crate::first());}",
        "struct R{x:i32} fn get(p:&R)->i32{return p.x;} fn forward(p:&R)->i32{return crate::get(&*p);} fn main()->i32{let r=R{x:7};return crate::forward(&r);}",
        "fn main()->i32{return crate::missing(unknown_argument);}",
        "fn get(a:i32,b:i32)->i32{return a+b;} fn main()->i32{return crate::get(first_missing,second_missing);}",
    ] {
        let fixture = ProjectFixture::new(&[("main.ox", text)]);
        let ordinary = fixture.ordinary();
        let candidate = fixture.candidate();
        let ordinary_work = WorkMeter::default();
        let candidate_work = WorkMeter::default();
        let legacy = index::collect_originals(SourceOwner::project(&ordinary), IndexLimits::default(), &ordinary_work, &mut Allocator::default())
            .unwrap().finish(&ordinary_work, &mut Allocator::default()).unwrap();
        let candidate = index::collect_enum_candidate(SourceOwner::project(&candidate), IndexLimits::default(), &candidate_work, &mut Allocator::default())
            .unwrap().finish(&candidate_work, &mut Allocator::default()).unwrap();
        assert_eq!(candidate.enum_count(), 0);
        let snapshot = |index: &DeclarationIndex<'_>, limit| {
            let work = WorkMeter::new(limit);
            work.enable_observation();
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(32).unwrap();
            let result = resolve_index_impl(index, &work, &mut allocator, None);
            (
                format!("{result:?}"),
                format!("{:?}", work.events.borrow()),
                format!("{:?}", work.observations.borrow()),
                format!("{:?}", allocator.trace),
                work.used(), allocator.attempts,
            )
        };
        let old = snapshot(&legacy, IndexLimits::default().work);
        let new = snapshot(&candidate, IndexLimits::default().work);
        assert_eq!(old, new, "{text}");
        for limit in [old.4.saturating_sub(1), old.4, old.4 + 1] {
            assert_eq!(snapshot(&legacy, limit), snapshot(&candidate, limit), "limit={limit}: {text}");
        }
    }
}

#[cfg(target_os = "linux")]
#[test]
fn bounded_enum_resolver_project_aliases_and_signature_privacy_use_nominal_index() {
    let fixture = ProjectFixture::new(&[
        ("main.ox", "mod tokens;use crate::tokens::Token as Token;fn main()->i32{let value:Token=Token::Integer(7);match value{Token::Integer(n)=>{return n;},crate::tokens::Token::End=>{return 0;}}}"),
        ("tokens.ox", "pub enum Token{Integer(i32),End}"),
    ]);
    let project = fixture.candidate();
    let work = WorkMeter::default();
    let index = index::collect_enum_candidate(
        SourceOwner::project(&project),
        IndexLimits::default(),
        &work,
        &mut Allocator::default(),
    )
    .unwrap()
    .finish(&work, &mut Allocator::default())
    .unwrap();
    let facts =
        probe_enum_resolver_storage(&index, &WorkMeter::default(), &mut Allocator::default())
            .unwrap()
            .unwrap();
    assert_eq!(facts.retained_counts[Kind::MatchArms as usize], 2);
}

#[test]
fn bounded_enum_resolver_single_file_signature_privacy_uses_nominal_index() {
    for (source, code, expected) in [
        (
            "enum E{V} pub fn expose(value:E)->(){return;}",
            "E0207",
            "E",
        ),
        ("enum E{V} pub fn expose()->E{return E::V;}", "E0207", "E"),
        ("enum E{V} struct R{value:E}", "E0300", "value:E"),
    ] {
        with_index(source, |index| {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(64).unwrap();
            let errors = probe_enum_resolver_storage(index, &WorkMeter::default(), &mut allocator)
                .unwrap_err();
            assert_eq!(errors[0].code, code);
            assert_eq!(
                index.sources().text(errors[0].primary.unwrap()).unwrap(),
                expected
            );
            assert!(!allocator
                .trace
                .iter()
                .any(|event| event.kind == "paid HIR functions"));
        });
    }
}

#[test]
fn bounded_enum_resolver_match_work_boundaries_fail_before_arm_visits() {
    with_index("enum E{A(i32),B} fn take(token:E)->i32{match token{E::A(n)=>{return n;},E::B=>{return 0;}}}", |index| {
        let work = WorkMeter::default();
        work.enable_observation();
        let baseline = probe_enum_resolver_storage(index, &work, &mut Allocator::default()).unwrap().unwrap();
        let mut before = 0;
        let mut boundaries = Vec::new();
        for event in work.events.borrow().iter() {
            if matches!(event.operation, "enum resolve arm" | "enum resolve arm entry") {
                boundaries.push((before, event.operation, event.origin));
            }
            before += event.units;
        }
        assert_eq!(boundaries.len(), 4);
        for (limit, operation, origin) in boundaries {
            let limited = WorkMeter::new(limit);
            limited.enable_observation();
            let (_, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                let errors = probe_enum_resolver_storage(index, &limited, &mut Allocator::default()).unwrap_err();
                assert!(errors.iter().any(|error| error.code == "E0400" && error.primary == Some(origin)));
                assert!(!limited.events.borrow().iter().any(|event| event.operation == operation && event.origin == origin));
                drop(errors);
                // Observation storage belongs to the test; release it inside
                // the measurement before checking production cleanup.
                limited.events.borrow_mut().clear();
                limited.events.borrow_mut().shrink_to_fit();
                limited.observations.borrow_mut().clear();
                limited.observations.borrow_mut().shrink_to_fit();
            });
            assert_eq!(live, 0);
        }
        for limit in [work.used() - 1, work.used(), work.used() + 1] {
            let actual = probe_enum_resolver_storage(index, &WorkMeter::new(limit), &mut Allocator::default());
            if limit < work.used() { assert!(actual.is_err()); }
            else { assert_eq!(actual.unwrap().unwrap(), baseline); }
        }
    });
}

#[test]
fn bounded_enum_production_fresh_entry_rejects_zero_and_candidate_without_reserve() {
    for (text, code) in [
        ("fn main()->i32{return 0;}", "E0500"),
        ("enum E{N} fn main()->i32{return 0;}", "E0101"),
    ] {
        with_index(text, |index| {
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            let errors = type_enum_source(index, &work, &mut allocator).unwrap_err();
            assert_eq!(errors[0].code, code);
            assert_eq!(work.used(), 0);
            assert_eq!(allocator.attempts, 0);
        });
    }
}
