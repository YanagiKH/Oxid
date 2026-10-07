//! Test-only retained-capacity samples from successful per-function CFG checks.
//! The maxima cover the five named phase exits, not every allocation instant.
//! They exclude whole-program raw OIR, owner headers, allocator metadata and
//! allocator transients; they are not an exact heap peak, RSS or admission cap.
//! Failed/incomplete CFG checks publish no samples. A caller qualifying a whole
//! program must separately require the enclosing verification to succeed.
use super::*;
use std::{cell::Cell, marker::PhantomData, mem::size_of, rc::Rc};

/// Capacity sums at one actual function's phase exit. Each maximum preserves
/// the complete sample from that function; maxima of individual columns are
/// never added together. Slots use the locally measured element layouts below.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend::oir) struct PhaseSample {
    pub blocks: usize,
    pub edges: usize,
    pub usize_cells: usize,
    pub u8_cells: usize,
    pub option_definition_cells: usize,
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::frontend::oir) struct Snapshot {
    pub successful_functions: usize,
    pub excluded_failed_functions: usize,
    /// counts + offsets + predecessor blocks
    pub predecessors: PhaseSample,
    /// retained predecessors + order + parent + number + u8 next_edge
    pub depth_first: PhaseSample,
    /// retained predecessors/DFS + both Option<Definition> tables
    pub definitions: PhaseSample,
    /// retained predecessors/DFS/definitions + semi + ancestor + label + path
    /// + bucket + next_member + dominator
    pub immediate_dominators: PhaseSample,
    /// retained predecessors/DFS/definitions + dominator + first_child
    /// + next_sibling + enter + exit
    pub dominance_tree: PhaseSample,
    /// Maximum of complete observed phase sums, never a sum across functions.
    /// Whole-program raw OIR remains a separate simultaneous addition.
    pub per_function_capacity_max_bytes: usize,
}

#[derive(Clone, Copy, Default)]
struct FunctionSamples {
    predecessors: PhaseSample,
    depth_first: PhaseSample,
    definitions: PhaseSample,
    immediate_dominators: PhaseSample,
    dominance_tree: PhaseSample,
    retained_predecessor_cells: usize,
    option_definition_cells: usize,
}

#[derive(Clone, Copy, Default)]
struct State {
    snapshot: Snapshot,
    function: Option<FunctionSamples>,
}

thread_local! {
    static ACTIVE: Cell<Option<State>> = const { Cell::new(None) };
}

/// Thread-bound activation. Drop resets all state, including during unwinding.
#[must_use]
pub(in crate::frontend::oir) struct Guard(PhantomData<Rc<()>>);

pub(in crate::frontend::oir) fn begin() -> Guard {
    ACTIVE.with(|active| {
        assert!(
            active.get().is_none(),
            "verifier measurement already active"
        );
        active.set(Some(State::default()));
    });
    Guard(PhantomData)
}

impl Guard {
    pub(in crate::frontend::oir) fn finish(self) -> Snapshot {
        ACTIVE.with(|active| {
            let state = active.take().expect("active verifier measurement");
            assert!(state.function.is_none(), "CFG measurement still active");
            state.snapshot
        })
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(None));
    }
}

pub(super) struct FunctionGuard {
    active: bool,
    thread_bound: PhantomData<Rc<()>>,
}

pub(super) fn begin_function() -> FunctionGuard {
    let enabled = ACTIVE.with(|active| {
        let Some(mut state) = active.get() else {
            return false;
        };
        assert!(state.function.is_none(), "nested CFG measurement");
        state.function = Some(FunctionSamples::default());
        active.set(Some(state));
        true
    });
    FunctionGuard {
        active: enabled,
        thread_bound: PhantomData,
    }
}

fn max_sample(maximum: &mut PhaseSample, sample: PhaseSample) {
    if sample.bytes >= maximum.bytes {
        *maximum = sample;
    }
}

impl FunctionGuard {
    pub(super) fn succeed(mut self) {
        if self.active {
            ACTIVE.with(|active| {
                let mut state = active.get().expect("active verifier measurement");
                let f = state.function.take().expect("active CFG measurement");
                let s = &mut state.snapshot;
                s.successful_functions += 1;
                max_sample(&mut s.predecessors, f.predecessors);
                max_sample(&mut s.depth_first, f.depth_first);
                max_sample(&mut s.definitions, f.definitions);
                max_sample(&mut s.immediate_dominators, f.immediate_dominators);
                max_sample(&mut s.dominance_tree, f.dominance_tree);
                for sample in [
                    f.predecessors,
                    f.depth_first,
                    f.definitions,
                    f.immediate_dominators,
                    f.dominance_tree,
                ] {
                    s.per_function_capacity_max_bytes =
                        s.per_function_capacity_max_bytes.max(sample.bytes);
                }
                active.set(Some(state));
            });
            self.active = false;
        }
    }
}

impl Drop for FunctionGuard {
    fn drop(&mut self) {
        if self.active {
            ACTIVE.with(|active| {
                if let Some(mut state) = active.get() {
                    state.function = None;
                    state.snapshot.excluded_failed_functions += 1;
                    active.set(Some(state));
                }
            });
        }
    }
}

fn update(f: impl FnOnce(&mut FunctionSamples)) {
    ACTIVE.with(|active| {
        if let Some(mut state) = active.get() {
            if let Some(function) = &mut state.function {
                f(function);
                active.set(Some(state));
            }
        }
    });
}

fn sum(cells: impl IntoIterator<Item = usize>) -> usize {
    cells
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .expect("verifier observed capacity sum")
}

fn sample(
    blocks: usize,
    edges: usize,
    usize_cells: usize,
    u8_cells: usize,
    option_definition_cells: usize,
) -> PhaseSample {
    let bytes = [
        usize_cells.checked_mul(size_of::<usize>()),
        u8_cells.checked_mul(size_of::<u8>()),
        option_definition_cells.checked_mul(size_of::<Option<Definition>>()),
    ]
    .into_iter()
    .try_fold(0usize, |total, bytes| total.checked_add(bytes?))
    .expect("verifier observed byte sum");
    PhaseSample {
        blocks,
        edges,
        usize_cells,
        u8_cells,
        option_definition_cells,
        bytes,
    }
}

fn retained_cfg_cells(predecessors: &Predecessors, dfs: &DepthFirst) -> usize {
    sum([
        predecessors.offsets.capacity(),
        predecessors.blocks.capacity(),
        dfs.order.capacity(),
        dfs.parent.capacity(),
        dfs.number.capacity(),
    ])
}

pub(super) fn predecessors(
    block_count: usize,
    edges: usize,
    counts: &Vec<usize>,
    offsets: &Vec<usize>,
    blocks: &Vec<usize>,
) {
    update(|f| {
        f.retained_predecessor_cells = sum([offsets.capacity(), blocks.capacity()]);
        f.predecessors = sample(
            block_count,
            edges,
            sum([counts.capacity(), f.retained_predecessor_cells]),
            0,
            0,
        );
    });
}

pub(super) fn depth_first(dfs: &DepthFirst, next_edge: &Vec<u8>) {
    update(|f| {
        f.depth_first = sample(
            f.predecessors.blocks,
            f.predecessors.edges,
            sum([
                f.retained_predecessor_cells,
                dfs.order.capacity(),
                dfs.parent.capacity(),
                dfs.number.capacity(),
            ]),
            next_edge.capacity(),
            0,
        );
    });
}

pub(super) fn definitions(
    predecessors: &Predecessors,
    dfs: &DepthFirst,
    definitions: &Vec<Option<Definition>>,
    initializations: &Vec<Option<Definition>>,
) {
    update(|f| {
        f.option_definition_cells = sum([definitions.capacity(), initializations.capacity()]);
        f.definitions = sample(
            f.predecessors.blocks,
            f.predecessors.edges,
            retained_cfg_cells(predecessors, dfs),
            0,
            f.option_definition_cells,
        );
    });
}

pub(super) fn immediate_dominators(
    predecessors: &Predecessors,
    dfs: &DepthFirst,
    semi: &Vec<usize>,
    forest: &EvalForest,
    bucket: &Vec<usize>,
    next_member: &Vec<usize>,
    dominator: &Vec<usize>,
) {
    update(|f| {
        f.immediate_dominators = sample(
            f.predecessors.blocks,
            f.predecessors.edges,
            sum([
                retained_cfg_cells(predecessors, dfs),
                semi.capacity(),
                forest.ancestor.capacity(),
                forest.label.capacity(),
                forest.path.capacity(),
                bucket.capacity(),
                next_member.capacity(),
                dominator.capacity(),
            ]),
            0,
            f.option_definition_cells,
        );
    });
}

pub(super) fn dominance_tree(
    predecessors: &Predecessors,
    dfs: &DepthFirst,
    dominator: &Vec<usize>,
    first_child: &Vec<usize>,
    next_sibling: &Vec<usize>,
    dominance: &Dominance,
) {
    update(|f| {
        f.dominance_tree = sample(
            f.predecessors.blocks,
            f.predecessors.edges,
            sum([
                retained_cfg_cells(predecessors, dfs),
                dominator.capacity(),
                first_child.capacity(),
                next_sibling.capacity(),
                dominance.enter.capacity(),
                dominance.exit.capacity(),
            ]),
            0,
            f.option_definition_cells,
        );
    });
}

#[derive(Clone, Copy, Debug)]
pub(in crate::frontend::oir) struct Layout {
    pub definition: usize,
    pub option_definition: usize,
    pub predecessors: usize,
    pub depth_first: usize,
    pub eval_forest: usize,
    pub dominance: usize,
    pub scratch_dimensions: usize,
    pub usize_element: usize,
    pub u8_element: usize,
    pub definitions_vec: usize,
    pub usize_vec: usize,
    pub u8_vec: usize,
    pub predecessors_result: usize,
    pub depth_first_result: usize,
    pub definitions_result: usize,
    pub dominators_result: usize,
    pub dominance_result: usize,
    pub scratch_dimensions_result: usize,
    pub cfg_result: usize,
    pub verify_result: usize,
}

pub(in crate::frontend::oir) fn layout() -> Layout {
    Layout {
        definition: size_of::<Definition>(),
        option_definition: size_of::<Option<Definition>>(),
        predecessors: size_of::<Predecessors>(),
        depth_first: size_of::<DepthFirst>(),
        eval_forest: size_of::<EvalForest>(),
        dominance: size_of::<Dominance>(),
        scratch_dimensions: size_of::<ScratchDimensions>(),
        usize_element: size_of::<usize>(),
        u8_element: size_of::<u8>(),
        definitions_vec: size_of::<Vec<Option<Definition>>>(),
        usize_vec: size_of::<Vec<usize>>(),
        u8_vec: size_of::<Vec<u8>>(),
        predecessors_result: size_of::<Result<Predecessors, OirFailure>>(),
        depth_first_result: size_of::<Result<DepthFirst, OirFailure>>(),
        definitions_result: size_of::<Result<Vec<Option<Definition>>, OirFailure>>(),
        dominators_result: size_of::<Result<Vec<usize>, OirFailure>>(),
        dominance_result: size_of::<Result<Dominance, OirFailure>>(),
        scratch_dimensions_result: size_of::<Result<ScratchDimensions, OirFailure>>(),
        cfg_result: size_of::<Result<(), OirFailure>>(),
        verify_result: size_of::<Result<VerifiedProgram, OirFailure>>(),
    }
}
