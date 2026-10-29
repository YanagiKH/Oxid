//! Inert construction-local extension of the ordinary generic worklist.
//! Final data owns no construction mutex, guard, executable witness or raw program.
use super::*;
use crate::frontend::builtin_catalog::owned_vector as owned;
use std::sync::{LockResult, Mutex, MutexGuard, TryLockResult};

#[derive(Debug, PartialEq, Eq)]
struct OwnedCall {
    caller: DefId,
    expression: ExprId,
    template: DefId,
    key: owned::Function,
}

#[derive(Debug)]
pub(in crate::frontend::declaration_index) struct BuildState {
    origin: Span,
    limit: usize,
    retained: usize,
    live: usize,
    keys: Vec<owned::Function>,
    calls: Vec<OwnedCall>,
    files: Vec<u64>,
    planning_request: usize,
    collection_request: [u64; 2],
    source_needed: bool,
    scopes: IndexLimits,
    reserved: usize,
}

#[derive(Debug)]
pub(in crate::frontend::declaration_index) struct BuildOwner {
    state: Mutex<BuildState>,
    ranges: bool,
}
impl BuildOwner {
    pub(in crate::frontend::declaration_index) fn new(state: BuildState, ranges: bool) -> Self {
        Self {
            state: Mutex::new(state),
            ranges,
        }
    }
    pub(in crate::frontend::declaration_index) fn state(&self) -> &Mutex<BuildState> {
        &self.state
    }
    pub(in crate::frontend::declaration_index) fn ranges(&self) -> bool {
        self.ranges
    }
    pub(in crate::frontend::declaration_index) fn into_state(
        self,
        at: Span,
    ) -> Result<BuildState, Box<Diagnostic>> {
        self.state.into_inner().map_err(|_| bad(at))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::frontend::declaration_index) struct OwnedGenericPlan {
    ordinary: Plan,
    origin: Span,
    ranges: bool,
    keys: Vec<owned::Function>,
    calls: Vec<OwnedCall>,
    files: Vec<u64>,
    planning_request: usize,
    collection_request: [u64; 2],
    source_needed: bool,
    construction_scopes: [u64; 3],
    construction_metadata: usize,
    rebuild_work: u64,
}

#[allow(dead_code)]
struct FactoryBank {
    state: BuildState,
    mutex: Mutex<BuildState>,
    consumed_state: LockResult<BuildState>,
    mapped_state: Result<BuildState, Box<Diagnostic>>,
    caller_state: Result<BuildState, Box<Diagnostic>>,
    construction: BuildOwner,
    pending: Box<[BuildOwner; 1]>,
    returned_pending: Result<Box<[BuildOwner; 1]>, ReserveFailure>,
    index: DeclarationIndex<'static>,
    facts: DeclarationFacts<'static>,
    origin: CandidateOrigin,
    owner: OwnedGenericPlan,
    owner_result: Result<OwnedGenericPlan, Box<Diagnostic>>,
    boxed: Box<[OwnedGenericPlan; 1]>,
    boxed_result: Result<Box<[OwnedGenericPlan; 1]>, ReserveFailure>,
    returned: Result<DeclarationIndex<'static>, Vec<Diagnostic>>,
    source: SourceOwner<'static>,
    limits: IndexLimits,
    work: &'static WorkMeter,
    allocator: &'static mut Allocator,
    source_fact: SourceFactRoles,
}

/// Marking occurs only after element checking, without recursion while the
/// lock is held. Its real temporary/result roles are prepaid in the new factory
/// bank even when an unused generic definition emits no instance or key.
#[allow(dead_code)]
struct SourceFactRoles {
    index: &'static DeclarationIndex<'static>,
    mutex: &'static Mutex<BuildState>,
    locked: TryLockResult<MutexGuard<'static, BuildState>>,
    selected: MutexGuard<'static, BuildState>,
    element: Element,
    element_return: Result<Element, Box<Diagnostic>>,
    marked: Result<(), Box<Diagnostic>>,
    at: Span,
    owner: Option<&'static OwnedGenericPlan>,
    returned: bool,
}

#[allow(dead_code)]
struct FrameBank {
    operation: owned::Operation,
    key: owned::Function,
    actual: [Option<Type>; 3],
    signature: ([Type; 3], Type),
    result: Result<Type, Box<Diagnostic>>,
    caller_result: Result<Type, Box<Diagnostic>>,
    call: OwnedCall,
    state: &'static Mutex<BuildState>,
    locked: TryLockResult<MutexGuard<'static, BuildState>>,
    guard: MutexGuard<'static, BuildState>,
    mut_locked: TryLockResult<MutexGuard<'static, BuildState>>,
    mut_guard: MutexGuard<'static, BuildState>,
    keys: std::iter::Enumerate<std::slice::Iter<'static, owned::Function>>,
    selected: Option<(usize, &'static owned::Function)>,
    calls: std::slice::Iter<'static, OwnedCall>,
    selected_call: Option<&'static OwnedCall>,
    caller: DefId,
    expression: ExprId,
    record: RecordId,
    rank: usize,
    capacity: usize,
    checked: [Option<usize>; 6],
    value: Value,
    element: Element,
    origin: Span,
    returned_unit: Result<(), Box<Diagnostic>>,
}

pub(in crate::frontend::declaration_index) const fn factory_bytes() -> usize {
    size_of::<FactoryBank>()
}
const FRAME_BYTES: usize = size_of::<FrameBank>();

#[allow(dead_code)]
struct ReplayBank {
    live_index: &'static DeclarationIndex<'static>,
    live_plan: &'static OwnedGenericPlan,
    rebuilt_index: DeclarationIndex<'static>,
    rebuilt_return: Result<DeclarationIndex<'static>, Vec<Diagnostic>>,
    allocator: &'static mut Allocator,
    prepaid_work: WorkMeter,
    work: &'static WorkMeter,
    limits: IndexLimits,
    default_limits: IndexLimits,
    constructor_limits: IndexLimits,
    expected_scopes: [u64; 3],
    actual_scopes: [u64; 3],
    invalid_ceiling: bool,
    metadata: usize,
    request: usize,
    scopes: [Option<usize>; 4],
    collection_request: [u64; 2],
    fresh_generic: usize,
    fresh_request: Option<u64>,
    complete_request: Option<u64>,
    source: SourceOwner<'static>,
    source_files: std::ops::Range<usize>,
    ordinary: &'static Plan,
    instances: std::slice::Iter<'static, Instance>,
    instance: Option<&'static Instance>,
    names: &'static Nominals,
    keys: std::slice::Iter<'static, owned::Function>,
    calls: std::slice::Iter<'static, OwnedCall>,
    result: Result<(), Box<Diagnostic>>,
}

impl BuildState {
    pub(in crate::frontend::declaration_index) fn new(
        origin: Span,
        metadata: usize,
        scopes: IndexLimits,
    ) -> Result<Self, Box<Diagnostic>> {
        let limit = metadata.min(MAX_STORAGE);
        if factory_bytes() > limit {
            return Err(exhausted(origin));
        }
        Ok(Self {
            origin,
            limit,
            retained: factory_bytes(),
            live: 0,
            keys: Vec::new(),
            calls: Vec::new(),
            files: Vec::new(),
            planning_request: 0,
            collection_request: [0; 2],
            source_needed: false,
            scopes,
            reserved: 0,
        })
    }
    pub(in crate::frontend::declaration_index) fn set_collection_request(
        &mut self,
        retained: u64,
        scratch: u64,
    ) {
        self.collection_request = [retained, scratch];
    }
    fn admit(&self, additional: usize) -> Result<(), Box<Diagnostic>> {
        if self
            .retained
            .checked_add(self.live)
            .and_then(|n| n.checked_add(additional))
            .is_none_or(|n| n > self.limit)
        {
            return Err(exhausted(self.origin));
        }
        Ok(())
    }
    fn push<T>(
        &mut self,
        vector: &mut Vec<T>,
        value: T,
        storage: &mut Storage,
        allocator: &mut Allocator,
        work: &WorkMeter,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        work.debit(1, at, "owned vector candidate metadata")?;
        if vector.len() == vector.capacity() {
            let width = size_of::<T>();
            self.admit(width)?;
            if storage.live_after(width).is_none_or(|n| n > MAX_STORAGE) {
                return Err(exhausted(at));
            }
            let before = vector.capacity();
            allocator
                .vector_exact(vector, 1, "owned vector candidate metadata")
                .map_err(|_| exhausted(at))?;
            let extra = vector
                .capacity()
                .checked_sub(before)
                .and_then(|n| n.checked_mul(width))
                .ok_or_else(|| exhausted(at))?;
            // Inspect the allocator's actual capacity before inserting the row.
            // A refused reservation cannot publish a new key or source call.
            self.admit(extra)?;
            let bytes = storage
                .bytes
                .checked_add(extra)
                .ok_or_else(|| exhausted(at))?;
            if bytes
                .checked_add(storage.constructor_scratch)
                .is_none_or(|n| n > MAX_STORAGE)
            {
                return Err(exhausted(at));
            }
            storage.bytes = bytes;
            self.retained = self
                .retained
                .checked_add(extra)
                .ok_or_else(|| exhausted(at))?;
        }
        vector.push(value);
        Ok(())
    }
}

impl OwnedGenericPlan {
    pub(in crate::frontend::declaration_index) fn finish(
        ordinary: Plan,
        state: BuildState,
        ranges: bool,
    ) -> Result<Self, Box<Diagnostic>> {
        if state.live != 0 {
            return Err(bad(state.origin));
        }
        state.admit(0)?;
        Ok(Self {
            ordinary,
            origin: state.origin,
            ranges,
            keys: state.keys,
            calls: state.calls,
            files: state.files,
            planning_request: state.planning_request,
            collection_request: state.collection_request,
            source_needed: state.source_needed,
            construction_scopes: [
                state.scopes.retained,
                state.scopes.scratch,
                state.scopes.work,
            ],
            construction_metadata: state.limit,
            rebuild_work: 0,
        })
    }
    pub(in crate::frontend::declaration_index) fn ordinary(&self) -> &Plan {
        &self.ordinary
    }
    pub(in crate::frontend::declaration_index) fn origin(&self) -> Span {
        self.origin
    }
    pub(in crate::frontend::declaration_index) fn ranges(&self) -> bool {
        self.ranges
    }
    pub(in crate::frontend::declaration_index) fn keys(&self) -> &[owned::Function] {
        &self.keys
    }
    pub(in crate::frontend::declaration_index) fn set_rebuild_work(&mut self, work: u64) {
        self.rebuild_work = work;
    }
    fn same_source_plan(&self, other: &Self) -> bool {
        self.ordinary == other.ordinary
            && self.origin == other.origin
            && self.ranges == other.ranges
            && self.keys == other.keys
            && self.calls == other.calls
            && self.files == other.files
            && self.collection_request == other.collection_request
            && self.source_needed == other.source_needed
    }
    fn retained_request(&self, work: &WorkMeter) -> Result<usize, Box<Diagnostic>> {
        let at = self.origin;
        let plan = &self.ordinary;
        let mut bytes = size_of::<Self>();
        for (capacity, width) in [
            (self.keys.capacity(), size_of::<owned::Function>()),
            (self.calls.capacity(), size_of::<OwnedCall>()),
            (self.files.capacity(), size_of::<u64>()),
            (plan.instances.capacity(), size_of::<Instance>()),
            (plan.ordinary.capacity(), size_of::<Option<DefId>>()),
            (
                plan.vector_functions.capacity(),
                size_of::<vector::Function>(),
            ),
        ] {
            work.debit(1, at, "owned vector retained plan capacity")?;
            bytes = bytes
                .checked_add(capacity.checked_mul(width).ok_or_else(|| exhausted(at))?)
                .ok_or_else(|| exhausted(at))?;
        }
        for instance in &plan.instances {
            work.debit(1, at, "owned vector retained instance capacity")?;
            for (capacity, width) in [
                (instance.arguments.capacity(), size_of::<Value>()),
                (instance.calls.capacity(), size_of::<Call>()),
                (instance.types.capacity(), size_of::<(Span, Type)>()),
                (
                    instance.constructors.capacity(),
                    size_of::<(ExprId, Value)>(),
                ),
                (instance.matches.capacity(), size_of::<(Span, Value)>()),
            ] {
                bytes = bytes
                    .checked_add(capacity.checked_mul(width).ok_or_else(|| exhausted(at))?)
                    .ok_or_else(|| exhausted(at))?;
            }
        }
        bytes = bytes
            .checked_add(plan.nominals.candidate_payload_bytes(work, at)?)
            .ok_or_else(|| exhausted(at))?;
        Ok(bytes.max(self.planning_request))
    }
}

fn prepaid_replay_limits(
    collection: [u64; 2],
    live: usize,
    scopes: IndexLimits,
    work: u64,
    at: Span,
) -> Result<IndexLimits, Box<Diagnostic>> {
    let defaults = IndexLimits::default();
    // Collection uses its immutable per-input logical request. A fresh generic
    // walk receives the unchanged 8 MiB ceiling, including all of its retained
    // and live constructor storage. Summing the nonoverlapping phases is a
    // deliberate conservative prepayment, not a raised default or spare credit.
    let fresh = collection[0]
        .checked_add(collection[1])
        .and_then(|n| n.checked_add(factory_bytes() as u64))
        .and_then(|n| n.checked_add(MAX_STORAGE as u64))
        .ok_or_else(|| exhausted(at))?;
    let complete = u64::try_from(live)
        .ok()
        .and_then(|n| n.checked_add(fresh))
        .ok_or_else(|| exhausted(at))?;
    let retained = scopes.retained.min(defaults.retained);
    let scratch = scopes.scratch.min(defaults.scratch);
    if complete > retained || complete > scratch {
        return Err(exhausted(at));
    }
    Ok(IndexLimits {
        retained: retained
            .checked_sub(live as u64)
            .ok_or_else(|| exhausted(at))?,
        scratch: scratch
            .checked_sub(live as u64)
            .ok_or_else(|| exhausted(at))?,
        work: work.min(scopes.work).min(defaults.work),
    })
}

impl DeclarationIndex<'_> {
    /// Fresh reconstruction uses this index's immutable source owner, the same
    /// collector/walker and an explicitly prepaid work allowance. It grants no
    /// executable source authority and retains no second index after return.
    pub(in crate::frontend) fn validate_owned_vector_candidate(
        &self,
        work: &WorkMeter,
        allocator: &mut Allocator,
    ) -> Result<(), Vec<Diagnostic>> {
        let owner = self
            .owned_vector_plan()
            .ok_or_else(|| vec![*bad(self.sources().eof())])?;
        let at = owner.origin;
        let defaults = IndexLimits::default();
        if owner.construction_scopes[0] > defaults.retained
            || owner.construction_scopes[1] > defaults.scratch
            || owner.construction_scopes[2] > defaults.work
            || owner.rebuild_work > owner.construction_scopes[2]
            || owner.construction_metadata > MAX_STORAGE
            || owner.construction_metadata < factory_bytes()
        {
            return Err(vec![*bad(at)]);
        }
        let scopes = IndexLimits {
            retained: owner.construction_scopes[0],
            scratch: owner.construction_scopes[1],
            work: owner.construction_scopes[2],
        };
        let retained = owner.retained_request(work).map_err(|e| vec![*e])?;
        let live = self
            .owned_candidate_index_bytes(work)
            .map_err(|e| vec![*e])?
            .checked_add(retained)
            .and_then(|n| n.checked_add(size_of::<ReplayBank>()))
            .ok_or_else(|| vec![*exhausted(at)])?;
        let limits = prepaid_replay_limits(
            owner.collection_request,
            live,
            scopes,
            owner.rebuild_work,
            at,
        )
        .map_err(|e| vec![*e])?;
        // Every semantic field compared below occupies at least one retained
        // byte. Prepay this complete linear comparison as well as reconstruction
        // before the first fresh index allocation. Capacity slack is conservative
        // per-input work, never a new default allowance.
        let prepaid_work = owner
            .rebuild_work
            .checked_add(u64::try_from(retained).map_err(|_| vec![*exhausted(at)])?)
            .ok_or_else(|| vec![*exhausted(at)])?;
        work.debit(
            prepaid_work,
            at,
            "owned vector candidate replay prepaid work",
        )
        .map_err(|e| vec![*e])?;
        let prepaid = WorkMeter::new(owner.rebuild_work);
        let rebuilt = crate::frontend::declaration_index::sealed::collect_owned_vector_candidate(
            self.sources(),
            limits,
            owner.construction_metadata,
            &prepaid,
            allocator,
        )?;
        let actual = rebuilt.owned_vector_plan().ok_or_else(|| vec![*bad(at)])?;
        // Rebuild pays the still-live original and a fresh factory. Its actual
        // remaining ceilings therefore differ from the original construction
        // ceilings; validate this exact derived relation without restoring them.
        let expected_scopes = [
            limits
                .retained
                .checked_sub(factory_bytes() as u64)
                .ok_or_else(|| vec![*bad(at)])?,
            limits
                .scratch
                .checked_sub(factory_bytes() as u64)
                .ok_or_else(|| vec![*bad(at)])?,
            limits.work,
        ];
        if actual.construction_scopes != expected_scopes {
            return Err(vec![*bad(at)]);
        }
        if !owner.same_source_plan(actual) {
            return Err(vec![*bad(at)]);
        }
        Ok(())
    }
    pub(super) fn candidate_element(
        &self,
        value: Value,
        nominals: &Nominals,
        at: Span,
    ) -> Result<Element, Box<Diagnostic>> {
        if self.owned_vector_builder().is_some() {
            match value {
                Value::Record(id) if !nominals.copy(value) => {
                    let record = nominals.records.get(id).ok_or_else(|| bad(at))?;
                    if !record.complete || !nominals.concrete(value) {
                        return Err(error(
                            "owned vector element requires a concrete finite record",
                            at,
                        ));
                    }
                    self.note_owned_vector_source(at)?;
                    return Ok(Element::OwnedRecordApplication(id));
                }
                Value::Concrete(ValueTy::Owned(AggregateTy::Record(id)))
                    if !nominals.copy(value) =>
                {
                    if nominals.record_is_copy(id, at)? {
                        return Err(bad(at));
                    }
                    self.note_owned_vector_source(at)?;
                    return Ok(Element::OwnedConcreteRecord(id));
                }
                _ => (),
            }
        }
        Element::checked(value, nominals, at)
    }
    fn note_owned_vector_source(&self, at: Span) -> Result<(), Box<Diagnostic>> {
        let state = self.owned_vector_builder().ok_or_else(|| bad(at))?;
        state.try_lock().map_err(|_| bad(at))?.source_needed = true;
        Ok(())
    }
    /// Exact immutable source use, including checked abstract definitions that
    /// emit no concrete function. This is neither raw type inventory nor source
    /// authority; an ordinary index or pending builder always reports false.
    pub(in crate::frontend) fn owned_vector_source_needed(&self) -> bool {
        self.owned_vector_plan()
            .is_some_and(|owner| owner.source_needed)
    }
    pub(super) fn activate_owned_storage(
        &self,
        storage: &mut Storage,
        work: &WorkMeter,
    ) -> Result<(), Box<Diagnostic>> {
        if let Some(cell) = self.owned_vector_builder() {
            let retained_index = self.owned_candidate_index_bytes(work)?;
            let mut state = cell.try_lock().map_err(|_| bad(self.sources().eof()))?;
            let retained = state
                .scopes
                .retained
                .checked_sub(retained_index as u64)
                .ok_or_else(|| exhausted(state.origin))?;
            let scratch = state
                .scopes
                .scratch
                .checked_sub(retained_index as u64)
                .ok_or_else(|| exhausted(state.origin))?;
            state.limit = state
                .limit
                .min(usize::try_from(retained.min(scratch)).map_err(|_| exhausted(state.origin))?);
            state.admit(0)?;
            state.reserved = MAX_STORAGE
                .checked_sub(state.limit)
                .ok_or_else(|| exhausted(state.origin))?;
            storage.constructor_scratch = storage
                .constructor_scratch
                .checked_add(state.reserved)
                .ok_or_else(|| exhausted(state.origin))?;
            storage.bytes = storage
                .bytes
                .checked_add(state.retained)
                .ok_or_else(|| exhausted(state.origin))?;
            if storage.live_after(0).is_none_or(|n| n > MAX_STORAGE) {
                return Err(exhausted(state.origin));
            }
        }
        Ok(())
    }
    pub(super) fn record_owned_sources(
        &self,
        storage: &mut Storage,
        allocator: &mut Allocator,
        work: &WorkMeter,
    ) -> Result<(), Box<Diagnostic>> {
        if let Some(cell) = self.owned_vector_builder() {
            let mut state = cell.try_lock().map_err(|_| bad(self.sources().eof()))?;
            for module in 0..self.sources().count() {
                let file = self.sources().file(ModuleId(module))?;
                let at = self.sources().eof();
                work.debit(1, at, "owned vector source file identity")?;
                let mut files = std::mem::take(&mut state.files);
                let result = state.push(&mut files, file.identity(), storage, allocator, work, at);
                state.files = files;
                result?;
            }
        }
        Ok(())
    }
    pub(super) fn finish_owned_storage(&self, storage: &Storage) -> Result<(), Box<Diagnostic>> {
        if let Some(cell) = self.owned_vector_builder() {
            let mut state = cell.try_lock().map_err(|_| bad(self.sources().eof()))?;
            if state.live != 0 || storage.constructor_scratch != state.reserved {
                return Err(bad(state.origin));
            }
            state.planning_request = storage.bytes;
        }
        Ok(())
    }
    pub(super) fn enter_owned_frame(
        &self,
        storage: &mut Storage,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let cell = self.owned_vector_builder().ok_or_else(|| bad(at))?;
        let mut state = cell.try_lock().map_err(|_| bad(at))?;
        state.admit(FRAME_BYTES)?;
        if storage
            .live_after(FRAME_BYTES)
            .is_none_or(|n| n > MAX_STORAGE)
        {
            return Err(exhausted(at));
        }
        storage.constructor_scratch = storage
            .constructor_scratch
            .checked_add(FRAME_BYTES)
            .ok_or_else(|| exhausted(at))?;
        state.live = state
            .live
            .checked_add(FRAME_BYTES)
            .ok_or_else(|| exhausted(at))?;
        Ok(())
    }
    pub(super) fn leave_owned_frame(
        &self,
        storage: &mut Storage,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let cell = self.owned_vector_builder().ok_or_else(|| bad(at))?;
        let mut state = cell.try_lock().map_err(|_| bad(at))?;
        state.live = state.live.checked_sub(FRAME_BYTES).ok_or_else(|| bad(at))?;
        storage.constructor_scratch = storage
            .constructor_scratch
            .checked_sub(FRAME_BYTES)
            .ok_or_else(|| bad(at))?;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn register_owned_call(
        &self,
        caller: DefId,
        expression: ExprId,
        template: DefId,
        key: owned::Function,
        storage: &mut Storage,
        allocator: &mut Allocator,
        work: &WorkMeter,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        let cell = self.owned_vector_builder().ok_or_else(|| bad(at))?;
        let mut state = cell.try_lock().map_err(|_| bad(at))?;
        let mut found = false;
        for candidate in &state.keys {
            work.debit(1, at, "owned vector key lookup")?;
            found |= *candidate == key;
        }
        // Temporarily move only the vector header, ending the field borrow
        // before the same complete state pays actual capacity growth.
        if !found {
            let mut keys = std::mem::take(&mut state.keys);
            let result = state.push(&mut keys, key, storage, allocator, work, at);
            state.keys = keys;
            result?;
        }
        let mut calls = std::mem::take(&mut state.calls);
        let result = state.push(
            &mut calls,
            OwnedCall {
                caller,
                expression,
                template,
                key,
            },
            storage,
            allocator,
            work,
            at,
        );
        state.calls = calls;
        result
    }
    pub(super) fn freeze_owned_calls(
        &self,
        plan: &mut Plan,
        storage: &mut Storage,
        work: &WorkMeter,
    ) -> Result<(), Box<Diagnostic>> {
        let at = self.sources().eof();
        self.enter_owned_frame(storage, at)?;
        let result = (|| {
            let state = self
                .owned_vector_builder()
                .ok_or_else(|| bad(at))?
                .try_lock()
                .map_err(|_| bad(at))?;
            let users = plan.instances.len();
            let fixed = self.builtin_set().extra_functions();
            for (caller, instance) in plan.instances.iter_mut().enumerate() {
                for call in &mut instance.calls {
                    work.debit(1, at, "owned vector call identity freeze")?;
                    let mut selected = None;
                    for row in &state.calls {
                        work.debit(1, at, "owned vector call source identity")?;
                        if row.caller == DefId(caller)
                            && row.expression == call.expression
                            && selected.replace(row).is_some()
                        {
                            return Err(bad(at));
                        }
                    }
                    if let Some(row) = selected {
                        if row.template != call.template || call.vector.is_some() {
                            return Err(bad(at));
                        }
                        let explicit = self.owned_template(call.template, work, at)?;
                        if let Some(operation) = explicit {
                            if operation != row.key.operation() {
                                return Err(bad(at));
                            }
                        } else {
                            let shared = row
                                .key
                                .operation()
                                .shared_template()
                                .ok_or_else(|| bad(at))?;
                            if self.vector_template(call.template, work, at)? != Some(shared) {
                                return Err(bad(at));
                            }
                        }
                        let mut rank = None;
                        for (position, key) in state.keys.iter().enumerate() {
                            work.debit(1, at, "owned vector frozen key rank")?;
                            if *key == row.key && rank.replace(position).is_some() {
                                return Err(bad(at));
                            }
                        }
                        call.target = vector_id(
                            users,
                            fixed,
                            plan.vector_functions
                                .len()
                                .checked_add(rank.ok_or_else(|| bad(at))?)
                                .ok_or_else(|| exhausted(at))?,
                            at,
                        )?;
                    } else if let Some(key) = call.vector {
                        let rank = vector_rank(&plan.vector_functions, key, work, at)?
                            .ok_or_else(|| bad(at))?;
                        if self.vector_template(call.template, work, at)? != Some(key.operation()) {
                            return Err(bad(at));
                        }
                        call.target = vector_id(users, fixed, rank, at)?;
                    } else if call.template.0 >= plan.template_count {
                        let rank = call.template.0 - plan.template_count;
                        if rank >= fixed {
                            return Err(bad(at));
                        }
                        call.target = DefId(users.checked_add(rank).ok_or_else(|| exhausted(at))?);
                    }
                }
            }
            Ok(())
        })();
        self.leave_owned_frame(storage, at)?;
        result
    }
    pub(in crate::frontend) fn owned_vector_functions(&self) -> &[owned::Function] {
        self.owned_vector_plan().map_or(&[], OwnedGenericPlan::keys)
    }
    pub(super) fn owned_template(
        &self,
        template: DefId,
        work: &WorkMeter,
        at: Span,
    ) -> Result<Option<owned::Operation>, Box<Diagnostic>> {
        let start = self
            .template_function_count()
            .checked_add(self.builtin_set().extra_functions())
            .and_then(|n| n.checked_add(self.builtin_set().vector_templates()))
            .ok_or_else(|| exhausted(at))?;
        let Some(rank) = template.0.checked_sub(start) else {
            return Ok(None);
        };
        work.debit(1, at, "owned vector template rank")?;
        Ok(self
            .builtin_set()
            .experimental()
            .owned_vector_item_at(rank)
            .map(owned::Item::operation))
    }
}

impl<'a, 's> Walker<'a, 's> {
    pub(in crate::frontend::declaration_index) fn owned_vector_call(
        &mut self,
        expression: ExprId,
        template: DefId,
        operation: owned::Operation,
        arguments: &[Argument],
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        self.index.enter_owned_frame(self.storage, at)?;
        let result = (|| {
            if arguments.len() != operation.signature().parameters().len() {
                return Err(error("function argument count mismatch", at));
            }
            let explicit = self.program.generic_arguments(at);
            if explicit.is_some_and(|types| types.len() != 1) {
                return Err(error(
                    "vector function requires one element type argument",
                    at,
                ));
            }
            let mut selected = if let Some(types) = explicit {
                let value = self.ty(types[0])?.value(types[0].span)?;
                Some(
                    self.index
                        .candidate_element(value, self.nominals, types[0].span)?,
                )
            } else {
                None
            };
            let mut actuals = [None; 3];
            let first = self.argument(&arguments[0])?;
            let Type::Reference {
                value: Value::Vector(element),
                ..
            } = first
            else {
                return Err(error(
                    "owned vector function requires a vector reference",
                    at,
                ));
            };
            if selected.is_some_and(|expected| expected != element) {
                return Err(error(
                    "vector element type argument does not match its vector",
                    at,
                ));
            }
            selected = Some(element);
            actuals[0] = Some(first);
            self.owned_vector_selected_inner(
                expression,
                template,
                operation,
                arguments,
                selected.ok_or_else(|| bad(at))?,
                actuals,
                at,
            )
        })();
        self.index.leave_owned_frame(self.storage, at)?;
        result
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::frontend::declaration_index) fn owned_vector_selected(
        &mut self,
        expression: ExprId,
        template: DefId,
        operation: owned::Operation,
        arguments: &[Argument],
        element: Element,
        actuals: [Option<Type>; 3],
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        self.index.enter_owned_frame(self.storage, at)?;
        let result = self.owned_vector_selected_inner(
            expression, template, operation, arguments, element, actuals, at,
        );
        self.index.leave_owned_frame(self.storage, at)?;
        result
    }
    #[allow(clippy::too_many_arguments)]
    fn owned_vector_selected_inner(
        &mut self,
        expression: ExprId,
        template: DefId,
        operation: owned::Operation,
        arguments: &[Argument],
        element: Element,
        actuals: [Option<Type>; 3],
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        if !element.owned() || arguments.len() != operation.signature().parameters().len() {
            return Err(error(
                "operation requires a finite non-Copy record vector",
                at,
            ));
        }
        let count = Type::scalar(Ty::I32);
        let unit = Type::scalar(Ty::Unit);
        let vector = Value::Vector(element);
        let shared = Type::Reference {
            value: vector,
            kind: BorrowKind::Shared,
        };
        let exclusive = Type::Reference {
            value: vector,
            kind: BorrowKind::Exclusive,
        };
        let record_value = Type::Value(element.value());
        let (parameters, result) = match operation {
            owned::Operation::New => ([unit; 3], Type::Value(vector)),
            owned::Operation::Len | owned::Operation::Capacity => ([shared, unit, unit], count),
            owned::Operation::TryReserve => {
                let grow = self
                    .index
                    .experimental_enum_id(experimental::Enumeration::Grow)?;
                let grow = self.index.template_enum_for_instance(grow)?;
                (
                    [exclusive, count, unit],
                    Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Enum(grow)))),
                )
            }
            owned::Operation::PushReserved => ([exclusive, record_value, unit], unit),
            owned::Operation::PopLast => ([exclusive, unit, unit], record_value),
            owned::Operation::Truncate => ([exclusive, count, unit], unit),
            owned::Operation::Take => ([exclusive, unit, unit], Type::Value(vector)),
        };
        for (position, argument) in arguments.iter().enumerate() {
            let actual = match actuals[position] {
                Some(actual) => actual,
                None => self.argument(argument)?,
            };
            self.expect(actual, parameters[position], at)?;
        }
        if self.plan.is_some() {
            let record = match element {
                Element::OwnedRecordApplication(id) => {
                    let selected = self.nominals.records.get(id).ok_or_else(|| bad(at))?;
                    if !selected.complete
                        || !self.nominals.concrete(Value::Record(id))
                        || selected.is_copy
                    {
                        return Err(bad(at));
                    }
                    let mut rank = 0usize;
                    for previous in 0..id {
                        self.work.debit(1, at, "owned vector nominal rank")?;
                        rank += usize::from(self.nominals.concrete(Value::Record(previous)));
                    }
                    if rank >= 4096 {
                        return Err(exhausted(at));
                    }
                    RecordId(rank)
                }
                Element::OwnedConcreteRecord(record) => record,
                _ => return Err(bad(at)),
            };
            let key = owned::Function::record(operation, record).ok_or_else(|| exhausted(at))?;
            self.index.register_owned_call(
                self.instance.ok_or_else(|| bad(at))?,
                expression,
                template,
                key,
                self.storage,
                self.allocator,
                self.work,
                at,
            )?;
            self.storage.push(
                &mut self.calls,
                Call {
                    expression,
                    template,
                    target: DefId(usize::MAX),
                    vector: None,
                },
                self.allocator,
                self.work,
                at,
            )?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::project::{ProjectLimits, ProjectSources};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(text: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "oxid-owned-vector-candidate-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::write(path.join("main.ox"), text).unwrap();
            Self(path)
        }
        fn project(&self) -> ProjectSources {
            ProjectSources::load_typed(
                self.0.join("main.ox").to_str().unwrap(),
                ProjectLimits::default(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if std::thread::panicking() || std::env::var_os("OXID_KEEP_TEST_PROJECTS").is_some() {
                return;
            }
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    const IMPORTS: &str = "use alloc::bytes::ByteBuf;use alloc::bytes::new as bytes_new;use alloc::vec::Vec;use alloc::vec::new;use alloc::vec::len;use alloc::vec::capacity;use alloc::vec::try_reserve;use alloc::vec::push_reserved;use alloc::vec::pop_last;use alloc::vec::truncate;use alloc::vec::take;struct Entry{bytes:ByteBuf}";
    fn workflow() -> String {
        format!("{IMPORTS}fn identity<T>(value:T)->T{{return value;}}fn make()->Entry{{return Entry{{bytes:bytes_new()}};}}fn inspect(items:&Vec<Entry>)->i32{{return len(&*items);}}fn main()->i32{{let mut values=new::<Entry>();try_reserve(&mut values,2);push_reserved(&mut values,identity(make()));let entry=pop_last(&mut values);push_reserved(&mut values,entry);let before=capacity(&values);let mut moved=take(&mut values);truncate(&mut moved,0);let scalars=new::<i32>();identity(true);return len(&moved)+len(&scalars)+before;}}")
    }
    fn candidate<'s>(
        project: &'s ProjectSources,
        metadata: usize,
        work: &WorkMeter,
        allocator: &mut Allocator,
    ) -> Result<DeclarationIndex<'s>, Vec<Diagnostic>> {
        crate::frontend::declaration_index::sealed::collect_owned_vector_candidate(
            SourceOwner::project(project),
            IndexLimits::default(),
            metadata,
            work,
            allocator,
        )
    }
    fn assert_refused(errors: Vec<Diagnostic>) {
        assert!(!errors.is_empty());
        assert!(
            errors
                .iter()
                .all(|error| matches!(error.code, "E0205" | "E0300" | "E0400" | "E0500")),
            "{errors:?}"
        );
    }

    #[test]
    fn owned_vector_candidate_real_parser_freezes_disjoint_keys_without_source_authority() {
        let fixture = Fixture::new(&workflow());
        let project = fixture.project();
        let index = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(!index.is_current_source_pipeline());
        assert!(index.require_current_source_pipeline().is_err());
        assert!(index
            .validate_generic_source(&WorkMeter::default())
            .is_err());
        let plan = index.generic_plan().unwrap();
        assert_eq!(
            index.owned_vector_functions().len(),
            owned::Operation::ALL.len()
        );
        assert_eq!(index.vector_functions().len(), 2);
        assert!(index
            .owned_vector_functions()
            .iter()
            .all(|key| key.record_id() == RecordId(0)));
        for operation in owned::Operation::ALL {
            let key = owned::Function::record(operation, RecordId(0)).unwrap();
            let id = index
                .owned_vector_function_id(key, &WorkMeter::default())
                .unwrap();
            assert!(
                id.0 >= index.source_function_count()
                    + index.builtin_set().extra_functions()
                    + index.vector_functions().len()
            );
            assert!(id.0 < index.function_count());
            assert!(index
                .owned_vector_anchor(key, &WorkMeter::default())
                .is_ok());
            let signature = index
                .owned_vector_signature(key, &WorkMeter::default())
                .unwrap();
            assert_eq!(
                signature,
                key.concrete_signature(
                    index
                        .experimental_enum_id(experimental::Enumeration::Grow)
                        .unwrap()
                )
            );
        }
        let owner = index.owned_vector_plan().unwrap();
        for row in &owner.calls {
            let id = index
                .owned_vector_function_id(row.key, &WorkMeter::default())
                .unwrap();
            assert_eq!(
                index
                    .instantiated_call(row.caller, row.expression, row.template)
                    .unwrap(),
                id
            );
            assert!(index
                .instantiated_call(row.caller, row.expression, DefId(usize::MAX))
                .is_err());
        }
        assert!(owner.calls.len() > owner.keys.len());
        assert!(plan
            .instances
            .iter()
            .flat_map(|instance| &instance.calls)
            .filter_map(|call| call.vector)
            .all(|key| key.record_id().is_none()));
        let mut inspected = false;
        for (caller, instance) in plan.instances.iter().enumerate() {
            let (key, module) = index.template_function(instance.definition).unwrap();
            let function = &index.sources().ast(module).unwrap().functions[key.index];
            if index.sources().text(function.name).unwrap() != "inspect" {
                continue;
            }
            assert_eq!(
                index
                    .instantiated_parameter(
                        DefId(caller),
                        function.params[0].ty,
                        &WorkMeter::default()
                    )
                    .unwrap(),
                ParameterTy::Reference {
                    referent: BorrowedTy::Exact(AggregateTy::OwnedRecordVector(RecordId(0))),
                    kind: BorrowKind::Shared
                }
            );
            inspected = true;
        }
        assert!(inspected);
        index
            .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
            .unwrap();

        // The same genuine AST remains denied by the ordinary collector. Only
        // the two new import spellings moved from syntax denial to this gate.
        let errors = collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap_err();
        assert_eq!((errors.code, errors.stage), ("E0205", "resolve"));
    }

    #[test]
    fn owned_vector_candidate_reentrant_calls_release_construction_guards() {
        let source = format!("{IMPORTS}fn identity<T>(value:T)->T{{return value;}}fn wrap<T>(value:T)->T{{return identity(identity(value));}}fn make()->Entry{{return Entry{{bytes:bytes_new()}};}}fn main()->i32{{let mut values=new::<Entry>();try_reserve(&mut values,1);push_reserved(&mut values,wrap(identity(make())));let mut second=new::<Entry>();try_reserve(&mut second,1);push_reserved(&mut second,pop_last(&mut values));return len(&second);}} ");
        let fixture = Fixture::new(&source);
        let project = fixture.project();
        let index = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert!(index.owned_vector_plan().unwrap().calls.len() >= 7);
        index
            .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
            .unwrap();
    }

    #[test]
    fn owned_vector_candidate_source_fact_includes_unused_generic_and_zero_key_types() {
        const TYPES: &str =
            "use alloc::bytes::ByteBuf;use alloc::vec::Vec;struct Entry{bytes:ByteBuf}";
        for (declaration, functions) in [
            ("fn inspect<Marker>(values:&Vec<Entry>)->(){return;}", 1),
            (
                "fn inspect<Marker>()->(){let values:Vec<Entry> = alloc_new::<Entry>();return;}",
                1,
            ),
            ("fn inspect(values:&Vec<Entry>)->(){return;}", 2),
        ] {
            let source = format!(
                "{TYPES}use alloc::vec::new as alloc_new;{declaration}fn main()->i32{{return 0;}}"
            );
            let fixture = Fixture::new(&source);
            let project = fixture.project();
            let index = candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap();
            assert!(index.owned_vector_source_needed());
            assert!(index.owned_vector_functions().is_empty());
            assert_eq!(index.source_function_count(), functions);
            assert!(index.require_current_source_pipeline().is_err());
            index
                .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
                .unwrap();
        }
    }

    #[test]
    fn owned_vector_candidate_source_fact_distinguishes_unused_new_imports_and_old_types() {
        for (source, import) in [
            (
                "use alloc::vec::push_reserved;fn main()->i32{return 0;}",
                owned::Item::PushReserved,
            ),
            (
                "use alloc::vec::pop_last;fn main()->i32{return 0;}",
                owned::Item::PopLast,
            ),
        ] {
            let fixture = Fixture::new(source);
            let project = fixture.project();
            let index = candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap();
            assert!(!index.owned_vector_source_needed());
            assert!(index.owned_vector_functions().is_empty());
            assert!(index
                .builtin_set()
                .experimental()
                .contains_owned_vector(import));
            index
                .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
                .unwrap();
        }
        let fixture =
            Fixture::new("use alloc::vec::new;fn main()->i32{let values=new::<i32>();return 0;}");
        let project = fixture.project();
        let work = WorkMeter::default();
        let index = collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut Allocator::default(),
        )
        .unwrap()
        .finish(&work, &mut Allocator::default())
        .unwrap();
        assert!(index.is_current_source_pipeline());
        assert!(!index.owned_vector_source_needed());
    }

    #[test]
    fn owned_vector_candidate_source_fact_is_reauthenticated_not_a_mutable_marker() {
        for source in [
            "use alloc::bytes::ByteBuf;use alloc::vec::Vec;struct Entry{bytes:ByteBuf}fn inspect<Marker>(values:&Vec<Entry>)->(){return;}fn main()->i32{return 0;}",
            "use alloc::vec::push_reserved;fn main()->i32{return 0;}",
        ] {
            let fixture = Fixture::new(source);
            let project = fixture.project();
            let mut index = candidate(&project, MAX_STORAGE, &WorkMeter::default(), &mut Allocator::default()).unwrap();
            index.alter_owned_candidate_for_test(|owner| owner.source_needed = !owner.source_needed);
            let errors = index.validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default()).unwrap_err();
            assert_eq!(errors[0].code, "E0500");
        }
    }

    #[test]
    fn owned_vector_candidate_replay_retains_constructor_scopes_and_metadata() {
        let fixture = Fixture::new(&workflow());
        let project = fixture.project();
        let defaults = IndexLimits::default();
        let scopes = IndexLimits {
            retained: defaults.retained - 1024,
            scratch: defaults.scratch - 2048,
            work: defaults.work - 4096,
        };
        let index = crate::frontend::declaration_index::sealed::collect_owned_vector_candidate(
            SourceOwner::project(&project),
            scopes,
            MAX_STORAGE - 4096,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        let owner = index.owned_vector_plan().unwrap();
        assert_eq!(
            owner.construction_scopes,
            [
                scopes.retained - factory_bytes() as u64,
                scopes.scratch - factory_bytes() as u64,
                scopes.work
            ]
        );
        assert!(owner.construction_metadata <= MAX_STORAGE - 4096);
        index
            .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
            .unwrap();
        for (position, value) in [
            (0, 0),
            (1, 0),
            (2, 0),
            (0, u64::MAX),
            (1, u64::MAX),
            (2, u64::MAX),
        ] {
            let mut index = candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap();
            index.alter_owned_candidate_for_test(|owner| {
                owner.construction_scopes[position] = value
            });
            let mut allocator = Allocator::default();
            assert!(index
                .validate_owned_vector_candidate(&WorkMeter::default(), &mut allocator)
                .is_err());
            assert_eq!(allocator.attempts, 0);
        }
        for metadata in [0, MAX_STORAGE + 1] {
            let mut index = candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap();
            index.alter_owned_candidate_for_test(|owner| owner.construction_metadata = metadata);
            let mut allocator = Allocator::default();
            assert!(index
                .validate_owned_vector_candidate(&WorkMeter::default(), &mut allocator)
                .is_err());
            assert_eq!(allocator.attempts, 0);
        }
    }

    #[test]
    fn owned_vector_candidate_construction_lock_refuses_conflicts_and_poison() {
        let fixture = Fixture::new(&workflow());
        let project = fixture.project();
        let at = SourceOwner::project(&project).eof();
        let owner = BuildOwner::new(
            BuildState::new(at, MAX_STORAGE, IndexLimits::default()).unwrap(),
            false,
        );
        {
            let _held = owner.state().try_lock().unwrap();
            assert!(matches!(
                owner.state().try_lock(),
                Err(std::sync::TryLockError::WouldBlock)
            ));
        }
        assert!(owner.state().try_lock().is_ok());
        owner.into_state(at).unwrap();

        let owner = BuildOwner::new(
            BuildState::new(at, MAX_STORAGE, IndexLimits::default()).unwrap(),
            false,
        );
        let caught = std::panic::catch_unwind(|| {
            let _held = owner.state().try_lock().unwrap();
            panic!("candidate-only poison control");
        });
        assert!(caught.is_err());
        assert!(matches!(
            owner.state().try_lock(),
            Err(std::sync::TryLockError::Poisoned(_))
        ));
        assert!(owner.into_state(at).is_err());
    }

    #[test]
    fn owned_vector_candidate_admits_finite_affine_records_only() {
        for body in [
            "let mut values=new::<i32>();push_reserved(&mut values,1);",
            "let mut values=new::<ByteBuf>();",
            "let mut values=new::<Vec<Entry>>();",
            "let mut values=new::<Entry>();try_push(&mut values,Entry{bytes:bytes_new()});",
            "let mut values=new::<Entry>();try_resize(&mut values,1,Entry{bytes:bytes_new()});",
            "let mut values=new::<Entry>();let entry=values[0];",
            "let mut values=new::<Entry>();push_reserved(&values,Entry{bytes:bytes_new()});",
            "let mut values=new::<Entry>();push_reserved(&mut values,Entry{bytes:bytes_new()});let bad=take(&values);",
        ] {
            let source = format!("{IMPORTS}use alloc::vec::try_push;use alloc::vec::try_resize;fn main()->i32{{{body}return 0;}}");
            let fixture = Fixture::new(&source);
            let project = fixture.project();
            assert_refused(candidate(&project, MAX_STORAGE, &WorkMeter::default(), &mut Allocator::default()).unwrap_err());
        }
        for declaration in [
            "fn unbounded<T>(values:Vec<T>)->(){return;}",
            "struct Holder{values:Vec<Entry>}",
            "enum Holder{Values(Vec<Entry>)}",
            "fn borrowed(values:&[Entry])->(){return;}",
            "fn invalid_moves<Marker>()->(){let mut values=new::<Entry>();let first=values;let second=values;return;}",
            "fn invalid_loans<Marker>()->(){let mut values=new::<Entry>();push_reserved(&mut values,pop_last(&mut values));return;}",
        ] {
            let source = format!("{IMPORTS}{declaration}fn main()->i32{{return 0;}}");
            let fixture = Fixture::new(&source);
            let project = fixture.project();
            assert_refused(
                candidate(
                    &project,
                    MAX_STORAGE,
                    &WorkMeter::default(),
                    &mut Allocator::default(),
                )
                .unwrap_err(),
            );
        }
    }

    #[test]
    fn owned_vector_candidate_does_not_mint_ownership_authority_for_unproved_bodies() {
        // A nongeneric body is indexed and specialized here, not independently
        // ownership-verified. Preserve this program for the phase2 full proof;
        // adding a second ownership walker to generic planning would be wrong.
        for body in [
            "let mut values=new::<Entry>();let first=values;let second=values;",
            "let mut values=new::<Entry>();push_reserved(&mut values,pop_last(&mut values));",
        ] {
            let source = format!("{IMPORTS}fn main()->i32{{{body}return 0;}}");
            let fixture = Fixture::new(&source);
            let project = fixture.project();
            let index = candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap();
            assert!(!index.is_current_source_pipeline());
            assert!(index.require_current_source_pipeline().is_err());
        }
    }

    #[test]
    fn owned_vector_candidate_replay_detects_source_key_rank_call_and_ordinary_mutations() {
        let fixture = Fixture::new(&workflow());
        let project = fixture.project();
        for change in 0..8 {
            let mut index = candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap();
            index.alter_owned_candidate_for_test(|owner| match change {
                0 => {
                    owner.keys[0] =
                        owned::Function::record(owner.keys[0].operation(), RecordId(1)).unwrap()
                }
                1 => owner.keys.swap(0, 1),
                2 => owner.calls[0].template = DefId(usize::MAX),
                3 => owner.calls[0].expression = ExprId(usize::MAX),
                4 => owner.calls[0].caller = DefId(usize::MAX),
                5 => {
                    owner.calls[0].key =
                        owned::Function::record(owned::Operation::Take, RecordId(17)).unwrap()
                }
                6 => owner.ordinary.instances[0].calls[0].target = DefId(usize::MAX),
                7 => owner.files[0] ^= 1,
                _ => unreachable!(),
            });
            assert_refused(
                index
                    .validate_owned_vector_candidate(
                        &WorkMeter::default(),
                        &mut Allocator::default(),
                    )
                    .unwrap_err(),
            );
        }
        // Identical text and path-shaped spans from another real parse are not
        // the same immutable source allocation, even if all nominal ranks agree.
        let other_fixture = Fixture::new(&workflow());
        let other_project = other_fixture.project();
        let mut index = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        index.substitute_owned_candidate_source_for_test(SourceOwner::project(&other_project));
        assert_refused(
            index
                .validate_owned_vector_candidate(&WorkMeter::default(), &mut Allocator::default())
                .unwrap_err(),
        );
    }

    #[test]
    fn owned_vector_candidate_metadata_work_and_allocator_boundaries_are_observable() {
        let fixture = Fixture::new(&workflow());
        let project = fixture.project();
        let at = SourceOwner::project(&project).eof();
        assert!(BuildState::new(at, factory_bytes(), IndexLimits::default()).is_ok());
        assert_eq!(
            BuildState::new(at, factory_bytes() - 1, IndexLimits::default())
                .unwrap_err()
                .code,
            "E0400"
        );
        let exact =
            BuildState::new(at, factory_bytes() + FRAME_BYTES, IndexLimits::default()).unwrap();
        assert!(exact.admit(FRAME_BYTES).is_ok());
        assert!(exact.admit(FRAME_BYTES + 1).is_err());
        let mut allocator = Allocator::default();
        assert_refused(
            candidate(
                &project,
                factory_bytes() - 1,
                &WorkMeter::default(),
                &mut allocator,
            )
            .unwrap_err(),
        );
        assert_eq!(allocator.attempts, 0);

        // Find this real program's admitted combined-metadata boundary. The
        // input and all semantic operations stay unchanged at exact/one-less.
        let mut lower = factory_bytes();
        let mut upper = MAX_STORAGE;
        while lower < upper {
            let middle = lower + (upper - lower) / 2;
            if candidate(
                &project,
                middle,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .is_ok()
            {
                upper = middle;
            } else {
                lower = middle + 1;
            }
        }
        candidate(
            &project,
            lower,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert_refused(
            candidate(
                &project,
                lower - 1,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap_err(),
        );

        let work = WorkMeter::default();
        let mut baseline = Allocator::default();
        let index = candidate(&project, MAX_STORAGE, &work, &mut baseline).unwrap();
        let used = work.used();
        let exact_work = WorkMeter::new(used);
        candidate(
            &project,
            MAX_STORAGE,
            &exact_work,
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(exact_work.used(), used);
        assert_refused(
            candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::new(used - 1),
                &mut Allocator::default(),
            )
            .unwrap_err(),
        );
        assert!(baseline
            .trace
            .iter()
            .any(|event| event.kind == "owned vector candidate planning owner"));
        assert!(baseline
            .trace
            .iter()
            .any(|event| event.kind == "owned vector candidate frozen owner"));
        assert!(baseline
            .trace
            .iter()
            .any(|event| event.kind == "owned vector candidate metadata"));
        for (position, event) in baseline.trace.iter().enumerate() {
            if !event.kind.starts_with("owned vector candidate") {
                continue;
            }
            let mut failed = Allocator {
                fail_at: Some(position + 1),
                ..Default::default()
            };
            assert_refused(
                candidate(&project, MAX_STORAGE, &WorkMeter::default(), &mut failed).unwrap_err(),
            );
            assert!(failed.trace.iter().any(|event| !event.success));
        }
        let replay_work = WorkMeter::default();
        index
            .validate_owned_vector_candidate(&replay_work, &mut Allocator::default())
            .unwrap();
        let exact_replay = WorkMeter::new(replay_work.used());
        index
            .validate_owned_vector_candidate(&exact_replay, &mut Allocator::default())
            .unwrap();
        assert_refused(
            index
                .validate_owned_vector_candidate(
                    &WorkMeter::new(replay_work.used() - 1),
                    &mut Allocator::default(),
                )
                .unwrap_err(),
        );
        let mut allocator = Allocator::default();
        assert_refused(
            index
                .validate_owned_vector_candidate(&WorkMeter::new(0), &mut allocator)
                .unwrap_err(),
        );
        assert_eq!(allocator.attempts, 0);
    }

    #[test]
    fn owned_vector_candidate_replay_prepays_complete_allowance_before_fresh_allocation() {
        let fixture = Fixture::new(&workflow());
        let project = fixture.project();
        let mut index = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        let owner = index.owned_vector_plan().unwrap();
        let work = WorkMeter::default();
        let live = index
            .owned_candidate_index_bytes(&work)
            .unwrap()
            .checked_add(owner.retained_request(&work).unwrap())
            .and_then(|n| n.checked_add(size_of::<ReplayBank>()))
            .unwrap();
        let complete = owner.collection_request[0]
            .checked_add(owner.collection_request[1])
            .and_then(|n| n.checked_add(factory_bytes() as u64))
            .and_then(|n| n.checked_add(MAX_STORAGE as u64))
            .and_then(|n| n.checked_add(live as u64))
            .unwrap();
        assert!(complete < IndexLimits::default().scratch);
        let exact = IndexLimits {
            retained: complete,
            scratch: complete,
            work: IndexLimits::default().work,
        };
        prepaid_replay_limits(
            owner.collection_request,
            live,
            exact,
            owner.rebuild_work,
            owner.origin,
        )
        .unwrap();
        for scopes in [
            IndexLimits {
                retained: complete - 1,
                ..exact
            },
            IndexLimits {
                scratch: complete - 1,
                ..exact
            },
        ] {
            assert!(prepaid_replay_limits(
                owner.collection_request,
                live,
                scopes,
                owner.rebuild_work,
                owner.origin
            )
            .is_err());
        }
        index.alter_owned_candidate_for_test(|owner| {
            owner.collection_request[1] = IndexLimits::default().scratch
        });
        let mut allocator = Allocator::default();
        let errors = index
            .validate_owned_vector_candidate(&WorkMeter::default(), &mut allocator)
            .unwrap_err();
        assert_eq!(errors[0].code, "E0400");
        assert_eq!(allocator.attempts, 0);
    }

    #[test]
    fn owned_vector_candidate_new_metadata_insertion_is_transactional() {
        let fixture = Fixture::new(&workflow());
        let project = fixture.project();
        let at = SourceOwner::project(&project).eof();
        let width = size_of::<owned::Function>();
        let mut state =
            BuildState::new(at, factory_bytes() + width, IndexLimits::default()).unwrap();
        let mut storage = Storage {
            bytes: factory_bytes(),
            ..Default::default()
        };
        let mut values = Vec::new();
        let key = owned::Function::record(owned::Operation::New, RecordId(0)).unwrap();
        state
            .push(
                &mut values,
                key,
                &mut storage,
                &mut Allocator::default(),
                &WorkMeter::default(),
                at,
            )
            .unwrap();
        assert_eq!(values, [key]);
        let before = (
            values.len(),
            values.capacity(),
            state.retained,
            storage.bytes,
        );
        assert!(state
            .push(
                &mut values,
                key,
                &mut storage,
                &mut Allocator::default(),
                &WorkMeter::default(),
                at
            )
            .is_err());
        assert_eq!(
            (
                values.len(),
                values.capacity(),
                state.retained,
                storage.bytes
            ),
            before
        );
        let mut state = BuildState::new(at, MAX_STORAGE, IndexLimits::default()).unwrap();
        let mut storage = Storage::default();
        let mut values = Vec::new();
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Default::default()
        };
        assert!(state
            .push(
                &mut values,
                key,
                &mut storage,
                &mut allocator,
                &WorkMeter::default(),
                at
            )
            .is_err());
        assert!(values.is_empty());
        assert_eq!(
            (values.capacity(), state.retained, storage.bytes),
            (0, factory_bytes(), 0)
        );
    }

    #[test]
    fn owned_vector_candidate_import_aliases_use_the_real_parser_and_foreign_paths_stay_closed() {
        let imports = IMPORTS
            .replace(
                "use alloc::vec::push_reserved;",
                "use alloc::vec::push_reserved as store;",
            )
            .replace(
                "use alloc::vec::pop_last;",
                "use alloc::vec::pop_last as remove;",
            );
        let source = format!("{imports}fn main()->i32{{let mut values=new::<Entry>();try_reserve(&mut values,1);store(&mut values,Entry{{bytes:bytes_new()}});let item=remove(&mut values);return len(&values);}}");
        let fixture = Fixture::new(&source);
        let project = fixture.project();
        let index = candidate(
            &project,
            MAX_STORAGE,
            &WorkMeter::default(),
            &mut Allocator::default(),
        )
        .unwrap();
        assert_eq!(
            index
                .owned_vector_functions()
                .iter()
                .filter(|key| key.operation() == owned::Operation::PushReserved)
                .count(),
            1
        );
        assert_eq!(
            index
                .owned_vector_functions()
                .iter()
                .filter(|key| key.operation() == owned::Operation::PopLast)
                .count(),
            1
        );
        // Existing import target groups forbid importing one endpoint twice,
        // even when distinct alias spellings would not otherwise collide.
        for endpoint in ["push_reserved", "pop_last"] {
            let repeated = format!(
                "{IMPORTS}use alloc::vec::{endpoint} as duplicate;fn main()->i32{{return 0;}}"
            );
            let fixture = Fixture::new(&repeated);
            let project = fixture.project();
            let errors = candidate(
                &project,
                MAX_STORAGE,
                &WorkMeter::default(),
                &mut Allocator::default(),
            )
            .unwrap_err();
            assert_eq!((errors[0].code, errors[0].stage), ("E0201", "resolve"));
        }
        for endpoint in [
            "alloc::bytes::push_reserved",
            "std::vec::pop_last",
            "alloc::vec::push_reserved_extra",
            "alloc::vec::Pop_last",
            "alloc::vec::push_reserved::child",
        ] {
            let text = format!("use {endpoint};fn main()->i32{{return 0;}}");
            let malformed = Fixture::new(&text);
            assert!(
                ProjectSources::load_typed(
                    malformed.0.join("main.ox").to_str().unwrap(),
                    ProjectLimits::default()
                )
                .is_err(),
                "foreign/malformed import was parsed: {text}"
            );
        }
    }

    // Historical carriers are independent field-for-field mirrors. The new
    // metadata is separately owned and must not enlarge these old banks.
    #[allow(dead_code)]
    enum HistoricalElement {
        Concrete(Ty),
        CopyParameter(usize),
        RecordApplication(usize),
        ConcreteRecord(RecordId),
    }
    #[allow(dead_code)]
    enum HistoricalValue {
        Concrete(ValueTy),
        Parameter { position: usize, copy: bool },
        Record(usize),
        Enumeration(usize),
        Vector(HistoricalElement),
    }
    #[allow(dead_code)]
    enum HistoricalType {
        Value(HistoricalValue),
        Reference {
            value: HistoricalValue,
            kind: BorrowKind,
        },
        Slice {
            element: HistoricalElement,
            kind: BorrowKind,
        },
    }
    #[allow(dead_code)]
    struct HistoricalCall {
        expression: ExprId,
        template: DefId,
        target: DefId,
        vector: Option<vector::Function>,
    }
    #[allow(dead_code)]
    struct HistoricalInstance {
        definition: DefId,
        arguments: Vec<HistoricalValue>,
        calls: Vec<HistoricalCall>,
        types: Vec<(Span, HistoricalType)>,
        constructors: Vec<(ExprId, HistoricalValue)>,
        matches: Vec<(Span, HistoricalValue)>,
        depth: usize,
    }
    #[allow(dead_code)]
    struct HistoricalPlan {
        instances: Vec<HistoricalInstance>,
        ordinary: Vec<Option<DefId>>,
        template_count: usize,
        nominals: Nominals,
        vector_functions: Vec<vector::Function>,
    }
    #[allow(dead_code)]
    struct HistoricalStorage {
        bytes: usize,
        constructor_scratch: usize,
    }
    #[allow(dead_code)]
    enum HistoricalOrigin {
        Current,
        CurrentGeneric(Box<[HistoricalPlan; 1]>),
        CurrentRanges,
        CurrentGenericRanges(Box<[HistoricalPlan; 1]>),
        Enum(CompactSpan),
        Builtin(CompactSpan),
        Output(CompactSpan),
    }
    #[allow(dead_code)]
    struct HistoricalTables<'s> {
        sources: SourceOwner<'s>,
        originals: Vec<OriginalRow>,
        original_order: Vec<u32>,
        functions: Vec<FunctionRow>,
        records: Vec<RecordRow>,
        fields: Vec<FieldRow>,
        enums: Vec<EnumRow>,
        variants: Vec<VariantRow>,
        modules: Vec<ModuleRow>,
        children: Vec<u32>,
        imports: Vec<ImportRow>,
        aliases: Vec<AliasCell>,
        alias_order: Vec<u32>,
        root_main: u32,
        candidate_source_origin: HistoricalOrigin,
        has_element_ranges: bool,
        builtins: BuiltinAdmission<'s>,
    }
    #[allow(dead_code)]
    struct HistoricalWalker<'a, 's> {
        index: &'a DeclarationIndex<'s>,
        work: &'a WorkMeter,
        allocator: &'a mut Allocator,
        storage: &'a mut HistoricalStorage,
        nominals: &'a mut Nominals,
        plan: Option<&'a mut HistoricalPlan>,
        instance: Option<DefId>,
        definition: DefId,
        program: &'s ast::Program,
        function: &'s ast::Function,
        module: ModuleId,
        arguments: Vec<HistoricalValue>,
        bindings: Vec<Binding>,
        calls: Vec<HistoricalCall>,
        types: Vec<(Span, HistoricalType)>,
        constructors: Vec<(ExprId, HistoricalValue)>,
        matches: Vec<(Span, HistoricalValue)>,
        loans: Vec<Loan>,
        abstract_body: bool,
        reachable: bool,
        loop_frames: Vec<flow::LoopFrame>,
    }
    fn same_carrier<Current, Historical>() {
        assert_eq!(size_of::<Current>(), size_of::<Historical>());
        assert_eq!(
            std::mem::align_of::<Current>(),
            std::mem::align_of::<Historical>()
        );
        assert_eq!(
            size_of::<Result<Current, Box<Diagnostic>>>(),
            size_of::<Result<Historical, Box<Diagnostic>>>()
        );
    }
    #[test]
    fn owned_vector_candidate_preserves_historical_carriers_and_catalog_domains() {
        fn ref_unwind_safe<T: std::panic::RefUnwindSafe>() {}
        ref_unwind_safe::<DeclarationIndex<'static>>();
        ref_unwind_safe::<Tables<'static>>();
        same_carrier::<Element, HistoricalElement>();
        same_carrier::<Value, HistoricalValue>();
        same_carrier::<Type, HistoricalType>();
        same_carrier::<Call, HistoricalCall>();
        same_carrier::<Instance, HistoricalInstance>();
        same_carrier::<Plan, HistoricalPlan>();
        same_carrier::<Storage, HistoricalStorage>();
        same_carrier::<Walker<'static, 'static>, HistoricalWalker<'static, 'static>>();
        same_carrier::<CandidateOrigin, HistoricalOrigin>();
        same_carrier::<Tables<'static>, HistoricalTables<'static>>();
        assert_eq!(
            size_of::<DeclarationIndex<'static>>(),
            size_of::<HistoricalTables<'static>>()
        );
        assert_eq!(size_of::<BuiltinSet>(), 9);
        assert_eq!(size_of::<experimental::Inventory>(), 8);
        assert_eq!(size_of::<vector::Function>(), 2);
        assert_eq!(std::mem::align_of::<vector::Function>(), 1);
        assert_eq!(vector::Operation::ALL.len(), 9);
        assert_eq!(vector::Item::ALL.len(), 10);
        assert!(!vector::Item::ALL.contains(&vector::Item::PushReserved));
        assert!(!vector::Item::ALL.contains(&vector::Item::PopLast));
        let plain = experimental::Inventory::default();
        let both = plain
            .admit_owned_vector(owned::Item::PushReserved)
            .admit_owned_vector(owned::Item::PopLast);
        assert!(both.contains_vector(vector::Item::Vec));
        assert_eq!(both.vector_templates(), 0);
        assert_eq!(both.owned_vector_templates(), 2);
        assert!(vector::Operation::ALL
            .into_iter()
            .all(|operation| !both.contains_vector(operation.item())));
        for item in vector::Item::ALL {
            let old = plain.admit_vector(item);
            assert_eq!(old.owned_vector_templates(), 0);
            assert_eq!(
                old.vector_item_rank(item),
                both.admit_vector(item).vector_item_rank(item)
            );
        }
        assert_eq!(
            (
                IndexLimits::default().retained,
                IndexLimits::default().scratch,
                IndexLimits::default().work
            ),
            (32 * 1024 * 1024, 16 * 1024 * 1024, 256_000_000)
        );
        assert_eq!(
            (MAX_INSTANCES, MAX_DEPTH, MAX_STORAGE),
            (1024, 64, 8 * 1024 * 1024)
        );
    }
}
