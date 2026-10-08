use super::*;
use crate::frontend::{
    oir::source::hir_import::{public_facade, tests::project},
    options::Operation,
};

const CASES: [(&str, &[u8], usize); 4] = [
    (
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/duplicate-source.txt"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/duplicate.bin"
        )),
        1,
    ),
    (
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/unknown-type-source.txt"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/unknown-type.bin"
        )),
        1,
    ),
    (
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/multiple-source.txt"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/multiple.bin"
        )),
        2,
    ),
    (
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/end255-source.txt"
        )),
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/producer_diagnostic/end255.bin"
        )),
        1,
    ),
];

fn confirmed(result: Result<Infallible, Rejected>) -> Vec<Diagnostic> {
    match result {
        Err(Rejected::Confirmed(errors)) => errors,
        other => panic!("expected confirmed source diagnostics: {other:?}"),
    }
}

#[test]
fn producer_diagnostic_actual_captures_preserve_full_vector_and_source_order() {
    for &(text, bytes, count) in &CASES {
        let project = project(text);
        let expected = check_project_candidate(
            &project,
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap_err();
        let actual = confirmed(validate(&project, bytes, IndexLimits::default()));
        assert_eq!(actual.len(), count);
        assert_eq!(
            actual
                .iter()
                .map(|d| d.render_json(project.sources()))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|d| d.render_json(project.sources()))
                .collect::<Vec<_>>()
        );
        for operation in [Operation::Check, Operation::Run, Operation::Compile] {
            let actual = public_facade::import_produced(&project, bytes, operation).unwrap_err();
            assert_eq!(
                actual
                    .iter()
                    .map(|d| d.render_json(project.sources()))
                    .collect::<Vec<_>>(),
                expected
                    .iter()
                    .map(|d| d.render_json(project.sources()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                public_facade::import_checked(&project, bytes, operation).unwrap_err()[0].code,
                "E0702"
            );
        }
    }
}

#[test]
fn producer_diagnostic_exact_and_minus_one_prepaid_and_canonical_work() {
    for &(text, bytes, _) in &CASES {
        let project = project(text);
        let d = IndexLimits::default();
        let protocol = Protocol::from_opa(bytes).unwrap();
        let plan = Plan::calculate(d, protocol).unwrap();
        let fixed = plan.fixed_bytes as u64;
        let exact = IndexLimits {
            retained: fixed,
            scratch: fixed,
            work: plan.prepaid_work,
        };
        assert!(Plan::calculate(exact, protocol).is_ok());
        for short in [
            IndexLimits {
                retained: fixed - 1,
                ..exact
            },
            IndexLimits {
                scratch: fixed - 1,
                ..exact
            },
            IndexLimits {
                work: plan.prepaid_work - 1,
                ..exact
            },
        ] {
            let mut allocator = Allocator::default();
            let work = WorkMeter::new(short.work);
            assert!(matches!(
                execute(&project, bytes, short, &work, &mut allocator),
                Err(Rejected::Budget)
            ));
            assert_eq!(allocator.attempts, 0);
            assert_eq!(work.used(), 0);
        }
        let work = WorkMeter::new(d.work);
        let mut allocator = Allocator::default();
        confirmed(execute(&project, bytes, d, &work, &mut allocator));
        assert!(allocator.attempts > 0);
        let used = work.used();
        assert!(used > plan.prepaid_work);
        // Declaration admission reserves a conservative upper work envelope;
        // it may exceed the actual debits before a genuine early diagnostic.
        // Measure its exact accepted limit, separately from work.used().
        let (mut low, mut high) = (plan.prepaid_work, d.work);
        while low < high {
            let middle = low + (high - low) / 2;
            if matches!(
                validate(&project, bytes, IndexLimits { work: middle, ..d }),
                Err(Rejected::Confirmed(_))
            ) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        confirmed(validate(&project, bytes, IndexLimits { work: low, ..d }));
        assert!(!matches!(
            validate(&project, bytes, IndexLimits { work: low - 1, ..d }),
            Err(Rejected::Confirmed(_))
        ));
        let admitted_work = low;
        let mut admitted_storage = [0; 2];
        for (index, ceiling) in [d.retained, d.scratch].into_iter().enumerate() {
            let (mut low, mut high) = (fixed, ceiling);
            while low < high {
                let middle = low + (high - low) / 2;
                let limits = if index == 0 {
                    IndexLimits {
                        retained: middle,
                        ..d
                    }
                } else {
                    IndexLimits {
                        scratch: middle,
                        ..d
                    }
                };
                if matches!(
                    validate(&project, bytes, limits),
                    Err(Rejected::Confirmed(_))
                ) {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            let exact = if index == 0 {
                IndexLimits { retained: low, ..d }
            } else {
                IndexLimits { scratch: low, ..d }
            };
            let short = if index == 0 {
                IndexLimits {
                    retained: low - 1,
                    ..d
                }
            } else {
                IndexLimits {
                    scratch: low - 1,
                    ..d
                }
            };
            confirmed(validate(&project, bytes, exact));
            assert!(!matches!(
                validate(&project, bytes, short),
                Err(Rejected::Confirmed(_))
            ));
            admitted_storage[index] = low;
        }
        println!("PRODUCER_DIAGNOSTIC protocol={} source={} fixed={} prepaid={} canonical={} total={} admitted_work={} admitted_retained={} admitted_scratch={} allocator_attempts={}", protocol.digit() as char, text.len(), fixed, plan.prepaid_work, used-plan.prepaid_work, used, admitted_work, admitted_storage[0], admitted_storage[1], allocator.attempts);
    }
}

#[test]
fn producer_diagnostic_payload_framing_and_source_mutations_fail_closed() {
    for &(text, bytes, _) in &CASES {
        let project = project(text);
        for length in [0, 3, 1559, 1574] {
            assert!(!matches!(
                validate(&project, &bytes[..length], IndexLimits::default()),
                Err(Rejected::Confirmed(_))
            ));
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(!matches!(
            validate(&project, &trailing, IndexLimits::default()),
            Err(Rejected::Confirmed(_))
        ));
        // Each header/payload field and OPA structural columns are independent
        // negative controls; none is reserialized from canonical diagnostics.
        for offset in [
            0, 3, 4, 8, 9, 10, 11, 527, 1043, 1559, 1562, 1563, 1564, 1565, 1566, 1567, 1568, 1569,
            1570, 1571, 1572, 1573, 1574,
        ] {
            let mut bad = bytes.to_vec();
            bad[offset] ^= 1;
            assert!(
                !matches!(
                    validate(&project, &bad, IndexLimits::default()),
                    Err(Rejected::Confirmed(_))
                ),
                "offset {offset}"
            );
        }
        let mut bad = bytes.to_vec();
        bad[11 + 128] = 1; // inactive OPA cell
        assert!(!matches!(
            validate(&project, &bad, IndexLimits::default()),
            Err(Rejected::Confirmed(_))
        ));
        let changed = text.replacen("fn", "xx", 1);
        let owner = SourceOwner::project(&project);
        assert!(BoundObservation::bind_diagnostic(owner, changed.as_bytes(), bytes).is_err());
    }
}

#[test]
fn producer_diagnostic_first_error_identity_and_metadata_are_exact() {
    let (text, bytes, _) = CASES[2];
    let project = project(text);
    let errors = confirmed(validate(&project, bytes, IndexLimits::default()));
    let source = project.sources().files().first().unwrap();
    let header = &bytes[1559..];
    assert!(matches_first(header, source, &errors[0]));
    assert!(!matches_first(header, source, &errors[1]));
    let mut different = errors[0].clone();
    different.code = "E0200";
    assert!(!matches_first(header, source, &different));
    different = errors[0].clone();
    different.stage = "resolve";
    assert!(!matches_first(header, source, &different));
    different = errors[0].clone();
    different.message.push('!');
    assert!(!matches_first(header, source, &different));
    different = errors[0].clone();
    different.notes.push("extra".into());
    assert!(!matches_first(header, source, &different));
    different = errors[0].clone();
    different.primary.as_mut().unwrap().start += 1;
    assert!(!matches_first(header, source, &different));
    different = errors[0].clone();
    different.primary.as_mut().unwrap().file.0 += 1;
    assert!(!matches_first(header, source, &different));
    different = errors[0].clone();
    different
        .secondary
        .push((source.span(0, 1), "extra".into()));
    assert!(!matches_first(header, source, &different));
}

#[test]
fn producer_diagnostic_claim_on_valid_source_cannot_fall_back_to_success() {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-source.txt"
    ));
    let mut bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/checked_hir_import/rich-success.bin"
    ))[..1575]
        .to_vec();
    bytes[1563] = 1;
    bytes[1565] = 11;
    bytes[1566] = 3;
    bytes[1567] = 4;
    let project = project(text);
    assert!(matches!(
        validate(&project, &bytes, IndexLimits::default()),
        Err(Rejected::Mismatch)
    ));
    for operation in [Operation::Check, Operation::Run, Operation::Compile] {
        assert_eq!(
            public_facade::import_produced(&project, &bytes, operation).unwrap_err()[0].code,
            "E0703"
        );
    }
}
