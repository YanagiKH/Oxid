//! Test-only retained-capacity observations of the inherited scalar executor.
//! Samples follow successful allocation sites, including the overlap between
//! suspended frames, a new child and the still-live call-argument Vec. Popping a
//! frame retains its slot/place buffers only until that return arm ends, without
//! an intervening allocation; the prior sample already includes those buffers.
//!
//! This is not an allocator, heap/RSS/stack peak, admission bound or OOM promise.
//! It excludes source/verified owners, caller-owned entry arguments, inline
//! locals and Vec headers outside the frame allocation, allocator metadata,
//! realloc transients, failed partial argument collection and failed frame
//! construction. Observation state is fixed-size and never retains owners,
//! borrowed data, source spans or callbacks. Nothing is compiled into production.
use super::*;
use std::{cell::Cell, marker::PhantomData, mem::size_of, rc::Rc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend) struct Snapshot {
    pub frames_len_max: usize,
    pub frames_capacity_max: usize,
    /// The largest sum of capacities of all simultaneously retained frames.
    pub live_slots_capacity_max: usize,
    pub live_places_capacity_max: usize,
    /// Computed at each sample, not by adding the separate family maxima.
    pub live_scalar_capacity_max: usize,
    pub call_arguments_capacity_max: usize,
    /// Includes slot/place Vec headers embedded in the frame allocation once.
    pub frame_headers_bytes_max: usize,
    pub live_slots_bytes_max: usize,
    pub live_places_bytes_max: usize,
    pub live_scalar_bytes_max: usize,
    pub call_arguments_bytes_max: usize,
    /// Maximum coexisting frame, slot/place and temporary argument payload.
    /// Independent family maxima above need not describe the same sample.
    pub simultaneous_vector_payload_bytes_max: usize,
    /// If true, some sample arithmetic overflowed. Byte maxima are incomplete,
    /// never silently saturated, and must not be reported as complete peaks.
    pub observation_overflowed: bool,
}

thread_local! {
    static ACTIVE: Cell<Option<Snapshot>> = const { Cell::new(None) };
}

/// No heap allocation. Activation cannot move or be shared across threads.
#[must_use]
pub(in crate::frontend) struct Guard(PhantomData<Rc<()>>);

pub(in crate::frontend) fn begin() -> Guard {
    ACTIVE.with(|active| {
        assert!(active.get().is_none(), "execute measurement already active");
        active.set(Some(Snapshot::default()));
    });
    Guard(PhantomData)
}

impl Guard {
    pub(in crate::frontend) fn finish(self) -> Snapshot {
        ACTIVE.with(|active| active.take().expect("active execute measurement"))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(None));
    }
}

/// Borrowed frames are inspected only while the fixed observation is active.
/// Checked observation failures never alter execution's result or fuel.
pub(super) fn storage(frames: &[Frame], frames_capacity: usize, arguments_capacity: usize) {
    ACTIVE.with(|active| {
        let Some(mut snapshot) = active.get() else {
            return;
        };
        snapshot.frames_len_max = snapshot.frames_len_max.max(frames.len());
        snapshot.frames_capacity_max = snapshot.frames_capacity_max.max(frames_capacity);
        snapshot.call_arguments_capacity_max =
            snapshot.call_arguments_capacity_max.max(arguments_capacity);
        let capacities = frames
            .iter()
            .try_fold((0usize, 0usize), |(slots, places), frame| {
                Some((
                    slots.checked_add(frame.slots.capacity())?,
                    places.checked_add(frame.places.capacity())?,
                ))
            });
        let measured = capacities.and_then(|(slots, places)| {
            snapshot.live_slots_capacity_max = snapshot.live_slots_capacity_max.max(slots);
            snapshot.live_places_capacity_max = snapshot.live_places_capacity_max.max(places);
            let scalars = slots.checked_add(places)?;
            snapshot.live_scalar_capacity_max = snapshot.live_scalar_capacity_max.max(scalars);
            let headers_bytes = frames_capacity.checked_mul(size_of::<Frame>())?;
            let slots_bytes = slots.checked_mul(size_of::<Option<Scalar>>())?;
            let places_bytes = places.checked_mul(size_of::<Option<Scalar>>())?;
            let scalar_bytes = slots_bytes.checked_add(places_bytes)?;
            let arguments_bytes = arguments_capacity.checked_mul(size_of::<Scalar>())?;
            let simultaneous_bytes = headers_bytes
                .checked_add(scalar_bytes)?
                .checked_add(arguments_bytes)?;
            snapshot.frame_headers_bytes_max = snapshot.frame_headers_bytes_max.max(headers_bytes);
            snapshot.live_slots_bytes_max = snapshot.live_slots_bytes_max.max(slots_bytes);
            snapshot.live_places_bytes_max = snapshot.live_places_bytes_max.max(places_bytes);
            snapshot.live_scalar_bytes_max = snapshot.live_scalar_bytes_max.max(scalar_bytes);
            snapshot.call_arguments_bytes_max =
                snapshot.call_arguments_bytes_max.max(arguments_bytes);
            snapshot.simultaneous_vector_payload_bytes_max = snapshot
                .simultaneous_vector_payload_bytes_max
                .max(simultaneous_bytes);
            Some(())
        });
        snapshot.observation_overflowed |= measured.is_none();
        active.set(Some(snapshot));
    });
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // Fields also belong to the immutable Debug measurement report.
pub(in crate::frontend) struct Layout {
    pub frame: usize,
    pub resume: usize,
    pub option_resume: usize,
    pub scalar: usize,
    pub option_scalar: usize,
    pub run_failure: usize,
    pub run_result: usize,
    pub frame_result: usize,
    pub unit_result: usize,
    pub frames_vec: usize,
    pub slots_vec: usize,
    pub places_vec: usize,
    pub call_arguments_vec: usize,
}

pub(in crate::frontend) fn layout() -> Layout {
    Layout {
        frame: size_of::<Frame>(),
        resume: size_of::<Resume>(),
        option_resume: size_of::<Option<Resume>>(),
        scalar: size_of::<Scalar>(),
        option_scalar: size_of::<Option<Scalar>>(),
        run_failure: size_of::<RunFailure>(),
        run_result: size_of::<Result<Scalar, RunFailure>>(),
        frame_result: size_of::<Result<Frame, RunFailure>>(),
        unit_result: size_of::<Result<(), RunFailure>>(),
        frames_vec: size_of::<Vec<Frame>>(),
        slots_vec: size_of::<Vec<Option<Scalar>>>(),
        places_vec: size_of::<Vec<Option<Scalar>>>(),
        call_arguments_vec: size_of::<Vec<Scalar>>(),
    }
}

#[test]
fn activation_rejects_nesting_and_resets_on_finish_drop_and_unwind() {
    let guard = begin();
    storage(&[], 7, 0);
    assert!(std::panic::catch_unwind(begin).is_err());
    assert_eq!(guard.finish().frames_capacity_max, 7);
    assert_eq!(begin().finish(), Snapshot::default());
    {
        let _guard = begin();
        storage(&[], 11, 0);
    }
    assert_eq!(begin().finish(), Snapshot::default());
    assert!(std::panic::catch_unwind(|| {
        let _guard = begin();
        storage(&[], 13, 0);
        panic!("exercise measurement unwind");
    })
    .is_err());
    assert_eq!(begin().finish(), Snapshot::default());
}

#[test]
fn activations_and_observations_are_thread_isolated() {
    let guard = begin();
    storage(&[], 7, 0);
    let child = std::thread::spawn(|| {
        assert_eq!(begin().finish(), Snapshot::default());
        let guard = begin();
        storage(&[], 11, 0);
        guard.finish()
    })
    .join()
    .unwrap();
    assert_eq!(child.frames_capacity_max, 11);
    assert_eq!(guard.finish().frames_capacity_max, 7);
}

#[test]
fn observation_overflow_is_sticky_visible_and_does_not_panic() {
    let guard = begin();
    storage(&[], 1, 0);
    storage(&[], usize::MAX, 0);
    storage(&[], 2, 0);
    let snapshot = guard.finish();
    assert!(snapshot.observation_overflowed);
    assert_eq!(snapshot.frames_capacity_max, usize::MAX);
    assert_eq!(snapshot.frame_headers_bytes_max, 2 * size_of::<Frame>());
    assert_eq!(begin().finish(), Snapshot::default());
}

#[test]
fn actual_call_overlaps_frame_slots_places_and_argument_capacity() {
    let (_, program, entry) = super::tests::compiled(
        "fn bump(n: i32) -> i32 { let mut x = n; x = x + 1; return x; } \
         fn main() -> i32 { let mut x = 1; x = bump(x); return x; }",
    );
    let guard = begin();
    assert_eq!(run(&program, entry), Ok(Scalar::I32(2)));
    let snapshot = guard.finish();
    let layout = layout();
    let slots: usize = program
        .program
        .functions
        .iter()
        .map(|f| f.locals.len())
        .sum();
    let places: usize = program
        .program
        .functions
        .iter()
        .map(|f| f.places.len())
        .sum();
    assert!(!snapshot.observation_overflowed);
    assert_eq!(snapshot.frames_len_max, 2);
    assert!(snapshot.frames_capacity_max >= MAX_FRAMES);
    assert_eq!(snapshot.live_slots_capacity_max, slots);
    assert_eq!(snapshot.live_places_capacity_max, places);
    assert_eq!(snapshot.live_scalar_capacity_max, slots + places);
    assert!(snapshot.call_arguments_capacity_max >= 1);
    assert_eq!(
        snapshot.simultaneous_vector_payload_bytes_max,
        snapshot.frames_capacity_max * layout.frame
            + (slots + places) * layout.option_scalar
            + snapshot.call_arguments_capacity_max * layout.scalar,
    );
}

#[test]
fn failed_entry_preflight_has_no_storage_observation() {
    let (_, program, entry) = super::tests::compiled("fn main() -> i32 { return 1; }");
    let guard = begin();
    assert!(matches!(
        run_with_fuel(&program, entry, 0),
        Err(RunFailure::Fuel(_))
    ));
    assert_eq!(guard.finish(), Snapshot::default());
}
