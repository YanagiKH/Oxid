//! Effect-free reference-output controls. These neither establish SIGPIPE
//! policy nor call the positive subprocess entry inside the unit-test runner.
use super::super::builtin_input_fixtures as input_fixture;
use super::super::builtin_output_fixtures as output_fixture;
use super::super::consumer_fixtures as fixture;
use super::*;

#[test]
fn output_staging_preserves_bytes_and_rejects_invalid_last_cell() {
    for capacity in [0, 1, 1024] {
        let source: Vec<u8> = (0..capacity)
            .flat_map(|ordinal| ((ordinal % 256) as i32).to_le_bytes())
            .collect();
        let before = source.clone();
        let mut scratch = [0xA5; 1024];
        assert!(stage_stdout(&source, &mut scratch[..capacity]));
        for (ordinal, byte) in scratch[..capacity].iter().enumerate() {
            assert_eq!(*byte, (ordinal % 256) as u8);
        }
        assert!(scratch[capacity..].iter().all(|byte| *byte == 0xA5));
        assert_eq!(source, before);
    }
    for invalid in [-1i32, 256, i32::MIN, i32::MAX] {
        let mut source = vec![0u8; 1024 * 4];
        source[1023 * 4..].copy_from_slice(&invalid.to_le_bytes());
        let before = source.clone();
        let mut scratch = [0xA5; 1024];
        assert!(!stage_stdout(&source, &mut scratch));
        assert_eq!(source, before);
    }
}

#[test]
fn output_identity_denial_precedes_runtime_view_and_fuel() {
    let (sources, raw, _) = input_fixture::program(3, input_fixture::Observation::Status);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(witness.builtin_output_function(), None);
    assert_eq!(witness.builtin_output_enumeration(), None);
    let plan = ExecutionPlan::build(&witness).unwrap();
    let function = hir::DefId(1);
    let span = witness.functions()[function.0].span;
    let frame = Frame::allocate(&plan, function, 1, None).unwrap();
    // The input frame has deliberately uninitialized owners and references.
    // Its shared source cannot accidentally satisfy stdout's nominal identity.
    let mut machine = Machine {
        plan: &plan,
        frames: vec![frame],
        limits: Limits::default(),
        fuel: 0,
        next_activation: 2,
        live_slots: 0,
        live_cells: 0,
        live_bytes: 0,
        header_bytes: size_of::<Frame>(),
        events: vec![],
        observer: array_observe::Observer::default(),
    };
    let before = machine.frames[0].payload.clone();
    for (buffer, destination) in [(0, 0), (usize::MAX, 0), (0, usize::MAX)] {
        assert_eq!(
            machine.write_stdout(0, ReferenceParamId(buffer), OwnerPlaceId(destination), span,),
            Err(bad("output builtin identity", span)),
        );
        assert_eq!(machine.fuel, 0);
        assert!(machine.events.is_empty());
        assert_eq!(machine.frames[0].payload, before);
    }
}

#[test]
fn process_reference_admission_checks_signature_without_setup_or_allocation() {
    let (sources, raw, schedule) = fixture::empty_record();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let span = witness.functions()[schedule.entry.0].span;
    let denied = plan::fail_allocation_after(0, || {
        checked_entry_policy(&witness, Some(schedule.entry), EntryPolicy::Process)
    });
    let expected = if output::supported_host() {
        OwnedRunFailure::ProcessEntry(span)
    } else {
        OwnedRunFailure::ProcessHost
    };
    assert_eq!(denied, Err(expected));
    assert_eq!(run(&witness, Some(schedule.entry)), Ok(schedule.result));

    let (sources, raw) = output_fixture::inventory(BuiltinOrigins::None);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let admitted = plan::fail_allocation_after(0, || {
        checked_entry_policy(&witness, Some(hir::DefId(0)), EntryPolicy::Process)
    });
    assert_eq!(
        admitted,
        if output::supported_host() {
            Ok(hir::DefId(0))
        } else {
            Err(OwnedRunFailure::ProcessHost)
        },
    );
}

#[test]
fn default_output_inventory_denial_precedes_plan_allocation_and_fuel() {
    let (sources, raw) = output_fixture::inventory(BuiltinOrigins::WriteStdout);
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let builtin = witness.builtin_output_function().unwrap();
    let span = witness.functions()[builtin.0].span;
    // The output function is unused. Its admitted inventory still requires
    // Process policy, even when the entry or zero fuel could fail first.
    for entry in [None, Some(hir::DefId(0)), Some(hir::DefId(usize::MAX))] {
        assert_eq!(
            plan::fail_allocation_after(0, || run_limits(
                &witness,
                entry,
                Limits {
                    fuel: 0,
                    ..Limits::default()
                },
            )),
            Err(OwnedRunFailure::OutputEntry(span)),
        );
    }
    let checked = plan::fail_allocation_after(0, || {
        checked_entry_policy(&witness, Some(hir::DefId(0)), EntryPolicy::Process)
    });
    assert_eq!(
        checked,
        if output::supported_host() {
            Ok(hir::DefId(0))
        } else {
            Err(OwnedRunFailure::OutputHost(span))
        },
    );
    let (sources, raw) = output_fixture::inventory(BuiltinOrigins::WriteStatus);
    let status_only = verified::verify_owned(raw, &sources).unwrap();
    assert_eq!(run(&status_only, Some(hir::DefId(0))), Ok(Scalar::I32(0)),);
}

#[test]
fn process_silent_observer_does_not_reserve_or_emit_charge_events() {
    let (sources, raw, schedule) = fixture::empty_record();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let plan = ExecutionPlan::build(&witness).unwrap();
    let span = witness.functions()[schedule.entry.0].span;
    let mut machine = Machine {
        plan: &plan,
        frames: Vec::new(),
        limits: Limits::default(),
        fuel: 3,
        next_activation: 1,
        live_slots: 0,
        live_cells: 0,
        live_bytes: 0,
        header_bytes: 0,
        events: Vec::new(),
        observer: array_observe::Observer {
            silent: true,
            control: ObservationControl {
                allocation_failure: Some(ObservationAllocationSite::Event),
                ..ObservationControl::default()
            },
            ..array_observe::Observer::default()
        },
    };
    // Exercise the actual charge/record_event path, including preservation of
    // remaining fuel on failed subtraction. No effect-capable witness exists.
    for remaining in [2, 1, 0] {
        machine.charge(1, span).unwrap();
        assert_eq!(machine.fuel, remaining);
        assert!(machine.events.is_empty());
        assert_eq!(machine.events.capacity(), 0);
    }
    assert_eq!(
        machine.charge(1, span),
        Err(OwnedRunFailure::Scalar(RunFailure::Fuel(span))),
    );
    assert_eq!(machine.fuel, 0);
    assert!(machine.events.is_empty());
    assert_eq!(machine.events.capacity(), 0);
    assert!(!machine.observer.allocation_fault_applied);
    // Preserve ordinary test instrumentation when the private process flag is
    // absent, including the existing unbounded event-only observation mode.
    assert!(!array_observe::Observer::default().silent);
    machine.observer.silent = false;
    machine.charge(0, span).unwrap();
    assert_eq!(machine.events, vec![Event::Charge(span, 0)]);
}

#[test]
fn output_reference_carriers_keep_the_existing_frame_reservations() {
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(size_of::<Frame>(), 272);
        assert_eq!(size_of::<ReferenceHandle>(), 80);
        assert_eq!(size_of::<OwnerRuntime>(), 32);
        assert_eq!(size_of::<Event>(), 72);
        assert_eq!(size_of::<EntryPolicy>(), 1);
        assert_eq!(size_of::<output::Attempt>(), 1);
    }
    println!(
        "output reference carriers: Frame={} ReferenceHandle={} OwnerRuntime={} Event={} Machine(test)={} OwnedRunFailure={} Result<Scalar,OwnedRunFailure>={} EntryPolicy={} Attempt={}",
        size_of::<Frame>(),
        size_of::<ReferenceHandle>(),
        size_of::<OwnerRuntime>(),
        size_of::<Event>(),
        size_of::<Machine<'_, '_>>(),
        size_of::<OwnedRunFailure>(),
        size_of::<Result<Scalar>>(),
        size_of::<EntryPolicy>(),
        size_of::<output::Attempt>(),
    );
}
