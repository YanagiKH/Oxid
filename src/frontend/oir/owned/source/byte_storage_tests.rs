//! RFC0031 source/reference controls. Values and effect order are source-derived.
use super::super::*;
use super::{association, lower, program, resolve, typeck};
use crate::frontend::{ast, lexer, parser, source::SourceFileId};

pub(super) const BYTE: &str = "fn byte(x:i32)->u8{return x.to_u8_checked();}";

pub(super) fn parsed(text: &str) -> (SourceMap, ast::Program) {
    let mut sources = SourceMap::new();
    let id = sources.add("byte-storage.ox".into(), text.into());
    let file = sources.get(id);
    let (ast, _) = parser::parse_typed_counted(
        file,
        lexer::lex(file).unwrap(),
        parser::SourceMode::OwnedCandidate,
        parser::MAX_NODES,
        &mut crate::frontend::project::budget::Allocator::default(),
        &mut parser::SyntaxStorage::default(),
    )
    .unwrap();
    (sources, ast)
}

pub(in crate::frontend::oir::owned) fn with_raw<T>(
    text: &str,
    action: impl FnOnce(&SourceMap, &typeck::TypedOwnedProgram<'_>, RawOwnedProgram) -> T,
) -> T {
    let (sources, ast) = parsed(text);
    let typed = typeck::check(
        resolve::resolve_in_map(sources.get(SourceFileId(0)), &ast, &sources).unwrap(),
    )
    .unwrap();
    let raw = lower::lower(&typed).unwrap();
    action(&sources, &typed, raw)
}

pub(super) fn observe(
    text: &str,
    limits: execute::Limits,
    control: execute::ObservationControl,
) -> execute::ReferenceObservation {
    with_raw(text, |_, typed, raw| {
        let entry = typed.entry();
        let witness =
            verified::verify_associated(association::associate(raw, typed).unwrap()).unwrap();
        execute::run_array_observed(&witness, entry, limits, control)
    })
}

fn run(text: &str) -> Result<Scalar, Vec<Diagnostic>> {
    let (sources, ast) = parsed(text);
    let (program, entry) = program::check_source(sources.get(SourceFileId(0)), &ast, &sources)?;
    program.run(entry, &sources).map_err(|e| vec![*e])
}

#[test]
fn byte_storage_source_pilot_and_complete_exact_slice_reborrows() {
    let source = format!("{BYTE} fn relay(a:[u8;4])->[u8;4]{{return a;}} fn edit(p:&mut[u8])->(){{p[1]=byte(255);return;}} fn sum(p:&[u8])->i32{{let mut n=0;let mut i=0;while i<p.len(){{let b=p[i];n=n+b.to_i32();i=i+1;}}return n;}} fn exact(p:&[u8;4])->i32{{return sum(&*p);}} fn shared(p:&mut[u8;4])->i32{{edit(&mut *p);return exact(&*p);}} fn main()->i32{{let a=[byte(0),byte(127),byte(128),byte(255)];let mut b=relay(a);let n=shared(&mut b);let x=b[1];return n+x.to_i32()-255;}}");
    assert_eq!(run(&source).unwrap(), Scalar::I32(638));
    let observed = observe(
        &source,
        execute::Limits::default(),
        execute::ObservationControl {
            poison_destinations: true,
            ..Default::default()
        },
    );
    assert_eq!(observed.result, Ok(Scalar::I32(638)));
    assert!(!observed.truncated);
    let reads: Vec<_> = observed
        .events
        .iter()
        .filter_map(|event| match event {
            execute::Event::ReadIndex(_, i, Scalar::U8(b)) => Some((*i, *b)),
            _ => None,
        })
        .collect();
    assert_eq!(reads, [(0, 0), (1, 255), (2, 128), (3, 255), (1, 255)]);
    for row in observed.storage {
        assert_eq!(row.guards_before, row.guards_after);
    }
}

#[test]
fn byte_storage_source_every_octet_constructs_moves_stores_and_reads() {
    // Sixteen independent partitions keep complete observations below the fixed
    // observer cap. The expected 0..255 sequence is arithmetic, not observed output.
    for first in (0..256).step_by(16) {
        let literal = (first..first + 16)
            .map(|n| format!("byte({n})"))
            .collect::<Vec<_>>()
            .join(",");
        let source = format!("{BYTE} fn relay(a:[u8;16])->[u8;16]{{return a;}} fn check(p:&[u8;16],first:i32)->bool{{let mut i=0;while i<16{{let b=p[i];if b.to_i32()!=first+i{{return false;}}i=i+1;}}return true;}} fn edit(p:&mut[u8],first:i32)->(){{let mut i=0;while i<p.len(){{let n=255-first-i;p[i]=n.to_u8_checked();i=i+1;}}return;}} fn sum(p:&[u8])->i32{{let mut i=0;let mut n=0;while i<p.len(){{let b=p[i];n=n+b.to_i32();i=i+1;}}return n;}} fn main()->i32{{let left=[true];let empty:[u8;0]=([]);let a=[{literal}];let right=[305419896];let mut b=relay(a);if !check(&b,{first}){{return -1;}}edit(&mut b,{first});let total=sum(&b);if !left[0]||right[0]!=305419896||empty.len()!=0{{return -2;}}return total;}}");
        let expected = (first..first + 16).map(|n| 255 - n).sum::<i32>();
        assert_eq!(run(&source).unwrap(), Scalar::I32(expected));
        let observed = observe(
            &source,
            execute::Limits::default(),
            execute::ObservationControl {
                poison_destinations: true,
                ..Default::default()
            },
        );
        assert_eq!(observed.result, Ok(Scalar::I32(expected)));
        assert!(!observed.truncated);
        let writes: Vec<_> = observed
            .events
            .iter()
            .filter_map(|event| match event {
                execute::Event::WriteIndex(_, i, Scalar::U8(b)) => Some((*i, *b)),
                _ => None,
            })
            .collect();
        let expected_writes: Vec<_> = (0..16)
            .map(|i| (i, (255 - first - i as i32) as u8))
            .collect();
        assert_eq!(writes, expected_writes);
        for row in observed.storage {
            assert_eq!(row.guards_before, row.guards_after);
        }
    }
}

#[test]
fn byte_storage_source_zero_one_maximum_lengths_and_sentinels() {
    for n in [0, 1, 1024] {
        let literal = (0..n).map(|_| "b").collect::<Vec<_>>().join(",");
        let source = format!("{BYTE} fn len(p:&[u8])->i32{{return p.len();}} fn relay(a:[u8;{n}])->[u8;{n}]{{return a;}} fn main()->i32{{let b=byte(255);let mut a:[u8;{n}]=[{literal}];let c=relay(a);a=c;a=a;return len(&a);}}");
        assert_eq!(run(&source).unwrap(), Scalar::I32(n));
        let observed = observe(
            &source,
            execute::Limits::default(),
            execute::ObservationControl {
                poison_destinations: true,
                ..Default::default()
            },
        );
        assert_eq!(observed.result, Ok(Scalar::I32(n)));
        assert!(!observed.truncated);
        let expected = if n == 0 {
            vec![0]
        } else {
            vec![255; n as usize]
        };
        for row in &observed.storage {
            assert_eq!(row.guards_before, row.guards_after);
            if matches!(
                row.kind,
                execute::StorageObservationKind::Construction
                    | execute::StorageObservationKind::Transfer
                    | execute::StorageObservationKind::Incoming
            ) {
                assert_eq!(row.bytes, expected);
            }
        }
    }
}

#[test]
fn byte_storage_source_rhs_snapshot_precedes_mutating_index_helper() {
    let source = format!("{BYTE} fn index(p:&mut[u8])->i32{{p[0]=byte(255);return 0;}} fn main()->i32{{let mut a=[byte(128)];a[index(&mut a)]=a[0];let b=a[0];return b.to_i32();}}");
    assert_eq!(run(&source).unwrap(), Scalar::I32(128));
    let observed = observe(
        &source,
        execute::Limits::default(),
        execute::ObservationControl::default(),
    );
    let effects: Vec<_> = observed
        .events
        .iter()
        .filter_map(|e| match e {
            execute::Event::ReadIndex(_, _, b) => Some((false, *b)),
            execute::Event::WriteIndex(_, _, b) => Some((true, *b)),
            _ => None,
        })
        .collect();
    assert_eq!(
        effects,
        [
            (false, Scalar::U8(128)),
            (true, Scalar::U8(255)),
            (true, Scalar::U8(128)),
            (false, Scalar::U8(128))
        ]
    );
}

#[test]
fn byte_storage_source_failed_constructor_never_exposes_partial_destination() {
    let source = format!("{BYTE} fn effect(p:&mut[u8],n:i32,x:i32)->u8{{p[0]=byte(n);return byte(x);}} fn main()->i32{{let mut a=[byte(0)];let b=[effect(&mut a,1,128),effect(&mut a,2,256),effect(&mut a,3,255)];return b.len();}}");
    let observed = observe(
        &source,
        execute::Limits::default(),
        execute::ObservationControl::default(),
    );
    assert!(matches!(
        observed.result,
        Err(execute::OwnedRunFailure::Scalar(RunFailure::ByteRange(_)))
    ));
    let writes: Vec<_> = observed
        .events
        .iter()
        .filter_map(|e| match e {
            execute::Event::WriteIndex(_, _, b) => Some(*b),
            _ => None,
        })
        .collect();
    assert_eq!(writes, [Scalar::U8(1), Scalar::U8(2)]);
    assert_eq!(
        observed
            .storage
            .iter()
            .filter(|s| s.kind == execute::StorageObservationKind::Construction)
            .count(),
        1
    );
}

#[test]
fn byte_storage_source_bounds_are_signed_and_report_complete_access() {
    for (n, index) in [
        (0, "0"),
        (1, "-1"),
        (1, "1"),
        (1, "-2147483648"),
        (1, "2147483647"),
    ] {
        let literal = if n == 0 { "" } else { "byte(255)" };
        let source = format!("{BYTE} fn main()->i32{{let a:[u8;{n}]=[{literal}];let b=a[{index}];return b.to_i32();}}");
        let errors = run(&source).unwrap_err();
        let e = &errors[0];
        assert_eq!(
            (e.code, e.stage, e.message.as_str()),
            ("E0606", "oir-owned-run", "array index out of bounds")
        );
        let span = e.primary.unwrap();
        assert_eq!(&source[span.start..span.end], format!("a[{index}]"));
    }
    assert_eq!(run(&format!("{BYTE} fn main()->i32{{let a=[byte(255)];if false{{let b=a[-1];return b.to_i32();}}return a.len();}}")).unwrap(), Scalar::I32(1));
}

#[test]
fn byte_storage_source_failed_rhs_index_and_paid_bounds_keep_prior_effects() {
    for case in 0..3 {
        let rhs = if case == 0 { "byte(256)" } else { "byte(128)" };
        let index = if case == 1 {
            "let bad=byte(256);return 0;"
        } else {
            "return 1;"
        };
        let source = format!("{BYTE} fn rhs(p:&mut[u8])->u8{{p[0]=byte(127);return {rhs};}} fn index(p:&mut[u8])->i32{{p[0]=byte(255);{index}}} fn main()->i32{{let mut a=[byte(0)];a[index(&mut a)]=rhs(&mut a);return a.len();}}");
        let observed = observe(
            &source,
            execute::Limits::default(),
            execute::ObservationControl::default(),
        );
        let writes: Vec<_> = observed
            .events
            .iter()
            .filter_map(|e| match e {
                execute::Event::WriteIndex(_, _, b) => Some(*b),
                _ => None,
            })
            .collect();
        assert_eq!(
            writes,
            if case == 0 {
                vec![Scalar::U8(127)]
            } else {
                vec![Scalar::U8(127), Scalar::U8(255)]
            }
        );
        if case < 2 {
            assert!(matches!(
                observed.result,
                Err(execute::OwnedRunFailure::Scalar(RunFailure::ByteRange(_)))
            ));
        } else {
            assert!(matches!(
                observed.result,
                Err(execute::OwnedRunFailure::Bounds(_))
            ));
        }
        let final_store = source.find("a[index(&mut a)]=rhs(&mut a);").unwrap();
        let paid = observed
            .events
            .iter()
            .any(|e| matches!(e, execute::Event::Charge(s, 1) if s.start == final_store));
        assert_eq!(paid, case == 2);
    }
}

#[test]
fn byte_storage_source_independent_every_fuel_schedule() {
    let text = "fn main()->i32{let x=128;let b=x.to_u8_checked();let mut a=[b];a[0]=b;let c=a[0];return c.to_i32();}";
    let at = |needle: &str| {
        let start = text.find(needle).unwrap();
        Span {
            file: SourceFileId(0),
            start,
            end: start + needle.len(),
        }
    };
    let within = |container: &str, needle: &str| {
        let start = text.find(container).unwrap() + container.find(needle).unwrap();
        Span {
            file: SourceFileId(0),
            start,
            end: start + needle.len(),
        }
    };
    // Source inventory: 13 scalar locals, two width-one owners, no calls,
    // references, loans or argument snapshots. Activation = 1+13+2+4*2=24.
    // The explicit temporary-to-local move and both lifetime ends are charged.
    let schedule = [
        (at("main"), 24),
        (at("128"), 1),
        (at("let x=128;"), 1),
        (within("x.to_u8_checked()", "x"), 1),
        (at("x.to_u8_checked()"), 1),
        (at("let b=x.to_u8_checked();"), 1),
        (within("[b]", "b"), 1),
        (at("[b]"), 1),
        (at("[b]"), 2),
        (at("let mut a=[b];"), 1),
        (at("let mut a=[b];"), 2),
        (at("let mut a=[b];"), 2),
        (within("a[0]=b;", "b"), 1),
        (within("a[0]=b;", "0"), 1),
        (within("a[0]=b;", "a[0]"), 1),
        (within("let c=a[0];", "0"), 1),
        (within("let c=a[0];", "a[0]"), 1),
        (at("let c=a[0];"), 1),
        (within("c.to_i32()", "c"), 1),
        (at("c.to_i32()"), 1),
        (at("return c.to_i32();"), 2),
        (at("return c.to_i32();"), 3),
    ];
    assert_eq!(schedule.iter().map(|(_, cost)| cost).sum::<usize>(), 51);
    with_raw(text, |_, typed, raw| {
        assert_eq!(
            (
                raw.functions[0].locals.len(),
                raw.functions[0].owners.len(),
                raw.functions[0].blocks[0].statements.len()
            ),
            (13, 2, 20)
        );
        let witness =
            verified::verify_associated(association::associate(raw, typed).unwrap()).unwrap();
        for fuel in 0..=51 {
            let observed = execute::run_array_observed(
                &witness,
                typed.entry(),
                execute::Limits {
                    fuel,
                    ..Default::default()
                },
                execute::ObservationControl::default(),
            );
            let mut remaining = fuel;
            let mut paid = Vec::new();
            let mut expected = Ok(Scalar::I32(128));
            for &(span, cost) in &schedule {
                if remaining < cost {
                    expected = Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(span)));
                    break;
                }
                remaining -= cost;
                paid.push((span, cost));
            }
            assert_eq!(observed.result, expected, "fuel={fuel}");
            assert_eq!(observed.remaining_fuel, remaining, "fuel={fuel}");
            let charges: Vec<_> = observed
                .events
                .iter()
                .filter_map(|event| match event {
                    execute::Event::Charge(span, cost) => Some((*span, *cost)),
                    _ => None,
                })
                .collect();
            assert_eq!(charges, paid, "fuel={fuel}");
            let stored = paid
                .iter()
                .any(|(span, _)| *span == within("a[0]=b;", "a[0]"));
            assert_eq!(
                observed
                    .events
                    .iter()
                    .filter(|event| matches!(
                        event,
                        execute::Event::WriteIndex(_, 0, Scalar::U8(128))
                    ))
                    .count(),
                usize::from(stored)
            );
        }
    });
}

#[test]
fn byte_storage_unpaid_bounds_keeps_mutating_index_effects() {
    let text = "fn index(p:&mut[u8])->i32{let x=255;let b=x.to_u8_checked();p[0]=b;return 1;}fn main()->i32{let x=128;let b=x.to_u8_checked();let mut a=[b];a[index(&mut a)]=a[0];return 0;}";
    let target = "a[index(&mut a)]";
    let start = text.find(target).unwrap();
    let span = Span {
        file: SourceFileId(0),
        start,
        end: start + target.len(),
    };
    // Main S10/A1/P2/O2/L1/C1: activation=1+10+1+2+8+12+2=36.
    // Scalar/owner initialization costs14, RHS read costs2, open/borrow costs2.
    // Helper S8/R1: invoke=1+1+8+8=18; body9; return1+R1=2.
    // Hence36+14+2+2+18+9+2=83 before final access, whose price is1.
    with_raw(text, |_, typed, raw| {
        assert_eq!(raw.functions[0].locals.len(), 8);
        assert_eq!(raw.functions[1].locals.len(), 10);
        let witness =
            verified::verify_associated(association::associate(raw, typed).unwrap()).unwrap();
        for fuel in [83, 84] {
            let observed = execute::run_array_observed(
                &witness,
                typed.entry(),
                execute::Limits {
                    fuel,
                    ..Default::default()
                },
                execute::ObservationControl::default(),
            );
            let expected = if fuel == 83 {
                execute::OwnedRunFailure::Scalar(RunFailure::Fuel(span))
            } else {
                execute::OwnedRunFailure::Bounds(span)
            };
            assert_eq!(observed.result, Err(expected));
            let effects: Vec<_> = observed
                .events
                .iter()
                .filter_map(|event| match event {
                    execute::Event::ReadIndex(_, i, b) => Some((false, *i, *b)),
                    execute::Event::WriteIndex(_, i, b) => Some((true, *i, *b)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                effects,
                [(false, 0, Scalar::U8(128)), (true, 0, Scalar::U8(255))]
            );
            assert_eq!(
                observed.events.contains(&execute::Event::Charge(span, 1)),
                fuel == 84
            );
            assert!(observed
                .storage
                .iter()
                .filter(|s| s.kind == execute::StorageObservationKind::Failure && s.state == 2)
                .any(|s| s.bytes == [255]));
        }
    });
}
