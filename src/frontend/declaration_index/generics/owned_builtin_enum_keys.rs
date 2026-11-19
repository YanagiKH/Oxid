//! Construction-only builtin enum keys. No provisional ID is exposed through
//! the declaration index, source types, layouts, raw OIR or an execution seal.
use super::*;

pub(super) const BUILTIN_ENUMS: usize =
    BuiltinEnum::ALL.len() + experimental::Enumeration::ALL.len();
pub(super) const FIXUP_SLOTS: usize = BUILTIN_ENUMS * owned::Operation::ALL.len();
pub(super) type BuiltinFixups = [BuiltinFixup; FIXUP_SLOTS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub(super) struct BuiltinFixup {
    rank: u32,
    key: owned::Function,
    builtin: u8,
}
impl BuiltinFixup {
    pub(super) const EMPTY: Self = Self {
        rank: 0,
        key: match owned::Function::record(owned::Operation::New, RecordId(0)) {
            Some(key) => key,
            None => panic!("closed empty construction key"),
        },
        builtin: 0,
    };
    pub(super) fn valid_pending(key: owned::Function, builtin: u8) -> bool {
        builtin != 0
            && usize::from(builtin) <= BUILTIN_ENUMS
            && key.element() == OwnedVectorRoot::Enum(EnumId(usize::from(builtin) - 1))
    }
    pub(super) fn check(rank: usize, key: owned::Function, builtin: u8) -> Option<Self> {
        if !Self::valid_pending(key, builtin) {
            return None;
        }
        Some(Self {
            rank: u32::try_from(rank).ok()?,
            key,
            builtin,
        })
    }
}

const _: () = {
    assert!(BUILTIN_ENUMS == 8);
    assert!(FIXUP_SLOTS == 64);
    assert!(FIXUP_SLOTS <= u8::MAX as usize);
    assert!(size_of::<BuiltinFixup>() == 8);
    assert!(std::mem::align_of::<BuiltinFixup>() == 4);
    assert!(size_of::<BuiltinFixups>() == 512);
};
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(size_of::<OwnedCall>() == 32);
    assert!(std::mem::align_of::<OwnedCall>() == 8);
    assert!(std::mem::offset_of!(OwnedCall, key) == 24);
    assert!(std::mem::offset_of!(OwnedCall, builtin) == 26);
};

#[allow(dead_code)]
struct FreezeCarriers {
    index: &'static DeclarationIndex<'static>,
    nominal: &'static Nominals,
    storage: &'static mut Storage,
    work: &'static WorkMeter,
    mutex: &'static Mutex<BuildState>,
    guard_result: TryLockResult<MutexGuard<'static, BuildState>>,
    guard: MutexGuard<'static, BuildState>,
    state: &'static mut BuildState,
    scopes: [BuiltinFixup; 3],
    borrowed_fixups: &'static [BuiltinFixup],
    fixup_iteration: std::slice::Iter<'static, BuiltinFixup>,
    fixup: &'static BuiltinFixup,
    fixup_next: Option<&'static BuiltinFixup>,
    enum_iteration: std::slice::Iter<'static, nominal::EnumApplication>,
    enumeration: &'static nominal::EnumApplication,
    keys: &'static [owned::Function],
    calls: std::slice::Iter<'static, OwnedCall>,
    call: &'static OwnedCall,
    mutable_calls: std::slice::IterMut<'static, OwnedCall>,
    mutable_call: &'static mut OwnedCall,
    projected: [owned::Function; 4],
    projected_results: [Result<owned::Function, Box<Diagnostic>>; 4],
    selected_fixup: Option<&'static BuiltinFixup>,
    root: OwnedVectorRoot,
    enum_id: EnumId,
    builtin_tag: u8,
    counts: [usize; 12],
    checked_counts: [Option<usize>; 8],
    work_result: Result<u64, Box<Diagnostic>>,
    charged: Result<(), Box<Diagnostic>>,
    returned: Result<(), Box<Diagnostic>>,
    caller_return: Result<(), Box<Diagnostic>>,
    at: Span,
}
pub(super) const fn control_bytes() -> usize {
    size_of::<FreezeCarriers>()
}

impl BuildState {
    pub(super) fn builtin_at(
        &self,
        rank: usize,
        work: &WorkMeter,
        at: Span,
    ) -> Result<u8, Box<Diagnostic>> {
        let count = usize::from(self.builtin_fixups_len);
        if count > FIXUP_SLOTS {
            return Err(bad(at));
        }
        if count == 0 {
            return Ok(0);
        }
        work.debit(
            (count + 1) as u64,
            at,
            "owned builtin key construction identity",
        )?;
        let mut selected = 0;
        for fixup in &self.builtin_fixups[..count] {
            if fixup.rank as usize == rank {
                if selected != 0 {
                    return Err(bad(at));
                }
                selected = fixup.builtin;
            }
        }
        Ok(selected)
    }
}

fn requested_work(
    enumerations: usize,
    fixups: usize,
    keys: usize,
    calls: usize,
    at: Span,
) -> Result<u64, Box<Diagnostic>> {
    // Every projection scans the complete fixed active table. Include key/call
    // preflight AND publication, both projections per independent key pair,
    // all nominal flags, table validation and fixed caller/Result controls.
    let pairs = keys
        .checked_mul(keys.saturating_sub(1))
        .map(|n| n / 2)
        .ok_or_else(|| exhausted(at))?;
    let lookups = keys
        .checked_add(calls)
        .and_then(|n| n.checked_add(pairs))
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(|| exhausted(at))?;
    let work = lookups
        .checked_mul(fixups.checked_add(12).ok_or_else(|| exhausted(at))?)
        .and_then(|n| n.checked_add(enumerations.checked_mul(4)?))
        .and_then(|n| n.checked_add(fixups.checked_mul(8)?))
        .and_then(|n| n.checked_add(16))
        .ok_or_else(|| exhausted(at))?;
    u64::try_from(work).map_err(|_| exhausted(at))
}

fn projected_key(
    keys: &[owned::Function],
    fixups: &[BuiltinFixup],
    rank: usize,
    users: usize,
    builtins: usize,
    at: Span,
) -> Result<owned::Function, Box<Diagnostic>> {
    let key = *keys.get(rank).ok_or_else(|| bad(at))?;
    let mut selected = None;
    for fixup in fixups {
        if fixup.rank as usize == rank && selected.replace(fixup).is_some() {
            return Err(bad(at));
        }
    }
    if let Some(fixup) = selected {
        if fixup.key != key
            || !BuiltinFixup::valid_pending(key, fixup.builtin)
            || usize::from(fixup.builtin) > builtins
        {
            return Err(bad(at));
        }
        let ordinal = users
            .checked_add(usize::from(fixup.builtin) - 1)
            .ok_or_else(|| exhausted(at))?;
        owned::Function::enumeration(key.operation(), EnumId(ordinal)).ok_or_else(|| exhausted(at))
    } else {
        if matches!(key.element(), OwnedVectorRoot::Enum(id) if id.0 >= users) {
            return Err(bad(at));
        }
        Ok(key)
    }
}

fn projected_call(
    fixups: &[BuiltinFixup],
    call: &OwnedCall,
    users: usize,
    builtins: usize,
    at: Span,
) -> Result<owned::Function, Box<Diagnostic>> {
    if call.builtin == 0 {
        if matches!(call.key.element(), OwnedVectorRoot::Enum(id) if id.0 >= users) {
            return Err(bad(at));
        }
        return Ok(call.key);
    }
    if !BuiltinFixup::valid_pending(call.key, call.builtin) || usize::from(call.builtin) > builtins
    {
        return Err(bad(at));
    }
    let mut selected = None;
    for fixup in fixups {
        if fixup.builtin == call.builtin
            && fixup.key == call.key
            && selected.replace(fixup).is_some()
        {
            return Err(bad(at));
        }
    }
    selected.ok_or_else(|| bad(at))?;
    let ordinal = users
        .checked_add(usize::from(call.builtin) - 1)
        .ok_or_else(|| exhausted(at))?;
    owned::Function::enumeration(call.key.operation(), EnumId(ordinal)).ok_or_else(|| exhausted(at))
}

fn freeze_state(
    state: &mut BuildState,
    nominals: &Nominals,
    builtins: usize,
    work: &WorkMeter,
    at: Span,
) -> Result<(), Box<Diagnostic>> {
    let count = usize::from(state.builtin_fixups_len);
    if count == 0 {
        return Ok(());
    }
    if count > FIXUP_SLOTS || state.builtin_keys_frozen {
        return Err(bad(at));
    }
    let request = requested_work(
        nominals.enumerations.len(),
        count,
        state.keys.len(),
        state.calls.len(),
        at,
    )?;
    work.debit(request, at, "owned builtin enum complete freeze")?;
    let mut users = 0usize;
    for (ordinal, enumeration) in nominals.enumerations.iter().enumerate() {
        if nominals.concrete(Value::Enumeration(ordinal)) {
            if !enumeration.complete {
                return Err(bad(at));
            }
            users = users.checked_add(1).ok_or_else(|| exhausted(at))?;
        }
    }
    if builtins > BUILTIN_ENUMS || users.checked_add(builtins).is_none_or(|count| count > 4096) {
        return Err(exhausted(at));
    }
    let fixups = &state.builtin_fixups[..count];
    for fixup in fixups {
        if fixup.rank as usize >= state.keys.len()
            || !BuiltinFixup::valid_pending(fixup.key, fixup.builtin)
            || usize::from(fixup.builtin) > builtins
        {
            return Err(bad(at));
        }
    }
    for rank in 0..state.keys.len() {
        let left = projected_key(&state.keys, fixups, rank, users, builtins, at)?;
        for previous in 0..rank {
            if left == projected_key(&state.keys, fixups, previous, users, builtins, at)? {
                return Err(bad(at));
            }
        }
    }
    for call in &state.calls {
        projected_call(fixups, call, users, builtins, at)?;
    }
    // This scope is exclusively borrowed and no callback/allocator follows
    // preflight. The same already checked projections cannot fail now.
    for rank in 0..state.keys.len() {
        let key = projected_key(
            &state.keys,
            &state.builtin_fixups[..count],
            rank,
            users,
            builtins,
            at,
        )
        .expect("preflighted builtin key");
        state.keys[rank] = key;
    }
    for call in &mut state.calls {
        call.key = projected_call(&state.builtin_fixups[..count], call, users, builtins, at)
            .expect("preflighted builtin call");
    }
    state.builtin_keys_frozen = true;
    Ok(())
}

impl DeclarationIndex<'_> {
    /// Worklist closure is before this call; no emitted source type exists yet.
    /// Fixed-table identity preflight and its entire work bill precede the first
    /// key write. No new vector, snapshot, payload clone or fresh budget exists.
    pub(in crate::frontend::declaration_index::generics) fn freeze_owned_builtin_enum_keys(
        &self,
        nominals: &Nominals,
        storage: &mut Storage,
        work: &WorkMeter,
    ) -> Result<(), Box<Diagnostic>> {
        if self.owned_vector_builder().is_none() {
            return Ok(());
        }
        let at = self.sources().eof();
        self.enter_owned_frame(storage, at)?;
        let result = (|| {
            let mut guard = self
                .owned_vector_builder()
                .ok_or_else(|| bad(at))?
                .try_lock()
                .map_err(|_| bad(at))?;
            let state = &mut *guard;
            freeze_state(state, nominals, self.builtin_set().extra_enums(), work, at)
        })();
        self.leave_owned_frame(storage, at)?;
        result
    }
}

#[cfg(test)]
#[path = "owned_builtin_enum_key_tests.rs"]
mod tests;
