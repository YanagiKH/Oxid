//! RFC0031 raw zero-owner oracle; no source schedule or plan cost is reused.
use super::consumer_fixtures as fixture;
use super::*;

fn empty_moves(read: bool) -> (SourceMap, RawOwnedProgram, Vec<(Span, usize)>, Span) {
    let (sources, at) = fixture::context();
    let array = AggregateTy::FixedArray(FixedArrayTy::check(hir::Ty::U8, 0).unwrap());
    let mut f = fixture::function(0, ValueTy::Scalar(hir::Ty::I32), at(0));
    f.locals = [hir::Ty::I32, hir::Ty::U8, hir::Ty::I32]
        .map(|ty| fixture::scalar(ty, at(0)))
        .to_vec();
    f.owners = (0..3)
        .map(|_| OwnerDecl {
            aggregate: AggregateSlot::try_from_aggregate(array).unwrap(),
            kind: OwnerKind::Local { mutable: true },
            span: at(0),
        })
        .collect();
    let mut statements = vec![
        fixture::assign(0, Rvalue::I32(0), at(1)),
        fixture::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(0)), at(2)),
        fixture::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(0),
                elements: vec![],
            },
            at(3),
        ),
        fixture::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(1)), at(4)),
        fixture::instruction(
            OwnedInstruction::MoveInitialize {
                destination: OwnerPlaceId(1),
                source: OwnerPlaceId(0),
            },
            at(5),
        ),
        fixture::instruction(OwnedInstruction::StorageLive(OwnerPlaceId(2)), at(6)),
        fixture::instruction(
            OwnedInstruction::ConstructArray {
                destination: OwnerPlaceId(2),
                elements: vec![],
            },
            at(7),
        ),
        fixture::instruction(
            OwnedInstruction::Replace {
                destination: OwnerPlaceId(1),
                source: OwnerPlaceId(2),
            },
            at(8),
        ),
        fixture::instruction(
            OwnedInstruction::ArrayLength {
                destination: LocalId(2),
                base: AccessBase::Owner(OwnerPlaceId(1)),
            },
            at(9),
        ),
    ];
    // S3 + P3 + 4O3, plus root-entry1. All three empty owners pay width1.
    let mut schedule = vec![
        (at(0), 19),
        (at(1), 1),
        (at(2), 1),
        (at(3), 2),
        (at(4), 1),
        (at(5), 2),
        (at(6), 1),
        (at(7), 2),
        (at(8), 3),
        (at(9), 1),
    ];
    if read {
        statements.push(fixture::instruction(
            OwnedInstruction::ReadIndex {
                destination: LocalId(1),
                base: AccessBase::Owner(OwnerPlaceId(1)),
                index: fixture::operand(0, at(10)),
            },
            at(10),
        ));
        schedule.push((at(10), 1));
    } else {
        // Teardown1 + all three retained owner-width cells.
        schedule.push((at(11), 4));
    }
    f.blocks = vec![OwnedBlock {
        merge: None,
        span: at(0),
        statements,
        terminator: fixture::end(
            OwnedTerminatorKind::ReturnScalar(fixture::operand(2, at(11))),
            at(11),
        ),
    }];
    (
        sources,
        RawOwnedProgram {
            builtins: BuiltinOrigins::None,
            enums: vec![],
            records: vec![],
            functions: vec![f],
        },
        schedule,
        at(10),
    )
}

#[test]
fn byte_storage_raw_empty_moves_replacement_and_bounds_every_fuel() {
    for read in [false, true] {
        let (sources, raw, schedule, bounds) = empty_moves(read);
        let witness = verified::verify_owned(raw, &sources).unwrap();
        let total = schedule.iter().map(|(_, cost)| cost).sum::<usize>();
        assert_eq!(total, if read { 34 } else { 37 });
        for fuel in 0..=total {
            let observed = execute::run_array_observed(
                &witness,
                Some(hir::DefId(0)),
                execute::Limits {
                    fuel,
                    ..Default::default()
                },
                execute::ObservationControl {
                    poison_destinations: true,
                    ..Default::default()
                },
            );
            let mut remaining = fuel;
            let mut paid = vec![];
            let mut expected = if read {
                Err(execute::OwnedRunFailure::Bounds(bounds))
            } else {
                Ok(Scalar::I32(0))
            };
            for &(span, cost) in &schedule {
                if remaining < cost {
                    expected = Err(execute::OwnedRunFailure::Scalar(RunFailure::Fuel(span)));
                    break;
                }
                remaining -= cost;
                paid.push((span, cost));
            }
            assert_eq!(observed.result, expected, "read={read}, fuel={fuel}");
            assert_eq!(observed.remaining_fuel, remaining);
            let charges: Vec<_> = observed
                .events
                .iter()
                .filter_map(|event| match event {
                    execute::Event::Charge(span, cost) => Some((*span, *cost)),
                    _ => None,
                })
                .collect();
            assert_eq!(charges, paid);
            assert!(!observed.truncated);
            assert!(!observed.events.iter().any(|event| matches!(
                event,
                execute::Event::ReadIndex(..) | execute::Event::WriteIndex(..)
            )));
            for snapshot in observed.storage {
                assert_eq!(snapshot.bytes, [0]);
                assert_eq!(snapshot.guards_before, snapshot.guards_after);
            }
        }
    }
}
