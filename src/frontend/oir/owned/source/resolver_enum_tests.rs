//! Private resolver semantics and independent storage controls, never executable admission.
use super::*;
use crate::frontend::{lexer, parser};
use std::mem::{align_of, size_of};

fn with_index(text: &str, action: impl FnOnce(&DeclarationIndex<'_>)) {
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
    let owner = SourceOwner::original(source, &ast, SourceView::Map(&sources)).unwrap();
    let work = WorkMeter::default();
    let mut allocator = Allocator::default();
    let index = index::collect_enum_candidate(owner, IndexLimits::default(), &work, &mut allocator)
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
    action(&index);
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
