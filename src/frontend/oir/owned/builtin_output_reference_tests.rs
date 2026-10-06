//! Closed reference-output precursor controls. These do not fabricate output
//! witnesses or claim that the admitted-input path exercises an output effect.
use super::super::builtin_input_fixtures as input_fixture;
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
fn process_reference_entry_stays_closed_before_allocation_and_activation() {
    let (sources, raw, schedule) = fixture::empty_record();
    let witness = verified::verify_owned(raw, &sources).unwrap();
    let denied = plan::fail_allocation_after(0, || {
        run_process_limits(
            &witness,
            Some(schedule.entry),
            Limits {
                fuel: 0,
                ..Limits::default()
            },
        )
    });
    assert_eq!(
        denied,
        Err(OwnedRunFailure::Invariant(
            "process execution admission closed",
            None,
        )),
    );
    assert_eq!(run(&witness, Some(schedule.entry)), Ok(schedule.result));
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
