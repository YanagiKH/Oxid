use super::*;
use crate::frontend::builtin_catalog::owned_vector::cases;
use crate::frontend::project::{ProjectLimits, ProjectSources};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "oxid-builtin-enum-keys-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("main.ox"),
            format!("{}{}", cases::API, cases::INTERLEAVED_BUILTINS),
        )
        .unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
fn candidate(project: &ProjectSources) -> DeclarationIndex<'_> {
    crate::frontend::declaration_index::sealed::collect_owned_vector_candidate(
        SourceOwner::project(project),
        IndexLimits::default(),
        MAX_STORAGE,
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap()
}
fn pending(at: Span) -> BuildState {
    let mut state = BuildState::new(at, MAX_STORAGE, IndexLimits::default()).unwrap();
    state.keys = vec![
        owned::Function::enumeration(owned::Operation::New, EnumId(0)).unwrap(),
        owned::Function::enumeration(owned::Operation::New, EnumId(0)).unwrap(),
        owned::Function::enumeration(owned::Operation::New, EnumId(1)).unwrap(),
        owned::Function::enumeration(owned::Operation::New, EnumId(1)).unwrap(),
    ];
    state.builtin_fixups[0] = BuiltinFixup::check(0, state.keys[0], 1).unwrap();
    state.builtin_fixups[1] = BuiltinFixup::check(2, state.keys[2], 2).unwrap();
    state.builtin_fixups_len = 2;
    state.calls = state
        .keys
        .iter()
        .enumerate()
        .map(|(position, key)| OwnedCall {
            caller: DefId(0),
            expression: ExprId(position),
            template: DefId(0),
            key: *key,
            builtin: match position {
                0 => 1,
                2 => 2,
                _ => 0,
            },
        })
        .collect();
    state
}
fn call_facts(state: &BuildState) -> Vec<(DefId, ExprId, DefId, owned::Function, u8)> {
    state
        .calls
        .iter()
        .map(|row| {
            (
                row.caller,
                row.expression,
                row.template,
                row.key,
                row.builtin,
            )
        })
        .collect()
}

#[test]
fn builtin_enum_fixup_storage_and_call_ordinal_have_numeric_physical_oracles() {
    assert_eq!(
        (
            size_of::<BuiltinFixup>(),
            std::mem::align_of::<BuiltinFixup>()
        ),
        (8, 4)
    );
    assert_eq!((BUILTIN_ENUMS, FIXUP_SLOTS), (8, 64));
    assert_eq!(size_of::<BuiltinFixups>(), 64 * 8);
    assert_eq!(std::mem::offset_of!(BuiltinFixup, rank), 0);
    assert_eq!(std::mem::offset_of!(BuiltinFixup, key), 4);
    assert_eq!(std::mem::offset_of!(BuiltinFixup, builtin), 6);
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(
            (size_of::<OwnedCall>(), std::mem::align_of::<OwnedCall>()),
            (32, 8)
        );
        assert_eq!(std::mem::offset_of!(OwnedCall, key), 24);
        assert_eq!(std::mem::offset_of!(OwnedCall, builtin), 26);
    }
    for (builtin, ordinal) in [(1, 0), (8, 7)] {
        let key = owned::Function::enumeration(owned::Operation::Take, EnumId(ordinal)).unwrap();
        assert!(BuiltinFixup::check(0, key, builtin).is_some());
        assert!(BuiltinFixup::check(0, key, 0).is_none());
        assert!(BuiltinFixup::check(0, key, 9).is_none());
        assert!(BuiltinFixup::check(
            0,
            owned::Function::record(owned::Operation::Take, RecordId(ordinal)).unwrap(),
            builtin
        )
        .is_none());
    }
}

#[test]
fn builtin_enum_key_freeze_has_literal_exact_work_and_one_less_changes_no_identity() {
    let fixture = Fixture::new();
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = candidate(&project);
    let nominals = &index.owned_vector_plan().unwrap().ordinary.nominals;
    assert_eq!(nominals.enum_count(), 2);
    let at = index.sources().eof();
    let mut state = pending(at);
    assert_eq!(state.keys[0], state.keys[1]);
    assert_eq!(state.keys[2], state.keys[3]);
    let original_keys = state.keys.clone();
    let original_calls = call_facts(&state);
    let original_fixups = state.builtin_fixups;
    // Four keys/four calls/six independent pairs:28 projected lookup roles,
    // each2 table comparisons+12 fixed controls;16 table+16 caller controls.
    let oracle = 424 + 4 * nominals.enumerations.len() as u64;
    assert_eq!(
        requested_work(nominals.enumerations.len(), 2, 4, 4, at).unwrap(),
        oracle
    );
    let short = WorkMeter::new(oracle - 1);
    assert!(freeze_state(&mut state, nominals, 2, &short, at).is_err());
    assert_eq!(short.used(), 0);
    assert_eq!(state.keys, original_keys);
    assert_eq!(call_facts(&state), original_calls);
    assert_eq!(state.builtin_fixups, original_fixups);
    assert!(!state.builtin_keys_frozen);
    let exact = WorkMeter::new(oracle);
    freeze_state(&mut state, nominals, 2, &exact, at).unwrap();
    assert_eq!(exact.used(), oracle);
    for (rank, ordinal) in [(0, 2), (1, 0), (2, 3), (3, 1)] {
        assert_eq!(
            state.keys[rank].element(),
            OwnedVectorRoot::Enum(EnumId(ordinal))
        );
        assert_eq!(state.calls[rank].key, state.keys[rank]);
        assert!(!state.keys[..rank].contains(&state.keys[rank]));
    }
    assert!(state.builtin_keys_frozen);
    assert!(freeze_state(&mut state, nominals, 2, &WorkMeter::default(), at).is_err());
}

#[test]
fn builtin_enum_key_preflight_refuses_wrong_tags_ranks_collisions_and_calls_before_writes() {
    let fixture = Fixture::new();
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = candidate(&project);
    let nominals = &index.owned_vector_plan().unwrap().ordinary.nominals;
    let at = index.sources().eof();
    for mutation in 0..7 {
        let mut state = pending(at);
        match mutation {
            0 => state.builtin_fixups[0].builtin = 9,
            1 => state.builtin_fixups[0].rank = u32::MAX,
            2 => state.builtin_fixups[1].rank = 0,
            3 => state.keys[3] = state.keys[1],
            4 => state.calls[0].builtin = 9,
            5 => {
                state.builtin_fixups[0].key =
                    owned::Function::record(owned::Operation::New, RecordId(0)).unwrap()
            }
            6 => state.builtin_fixups_len = 65,
            _ => unreachable!(),
        }
        let original_keys = state.keys.clone();
        let original_calls = call_facts(&state);
        let original_fixups = state.builtin_fixups;
        assert!(
            freeze_state(&mut state, nominals, 2, &WorkMeter::default(), at).is_err(),
            "mutation {mutation}"
        );
        assert_eq!(state.keys, original_keys);
        assert_eq!(call_facts(&state), original_calls);
        assert_eq!(state.builtin_fixups, original_fixups);
        assert!(!state.builtin_keys_frozen);
    }
}

#[test]
fn builtin_enum_key_fixed_table_admits_all_sixty_four_pairs_and_refuses_the_sixty_fifth() {
    let fixture = Fixture::new();
    let all = format!(
        "use std::io::WriteStatus;use std::io::ReadSomeStatus;use std::io::WriteSomeStatus;use std::fs::OpenResult;use std::fs::FileStatus;use std::env::ArgStatus;{}{}",
        cases::API, cases::INTERLEAVED_BUILTINS,
    );
    fs::write(fixture.0.join("main.ox"), all).unwrap();
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = candidate(&project);
    assert_eq!(index.builtin_set().extra_enums(), 8);
    let nominals = &index.owned_vector_plan().unwrap().ordinary.nominals;
    assert_eq!(nominals.enum_count(), 2);
    let at = index.sources().eof();
    let mut state = BuildState::new(at, MAX_STORAGE, IndexLimits::default()).unwrap();
    for builtin in 1u8..=8 {
        for operation in owned::Operation::ALL {
            let key =
                owned::Function::enumeration(operation, EnumId(usize::from(builtin) - 1)).unwrap();
            let rank = state.keys.len();
            state.builtin_fixups[rank] = BuiltinFixup::check(rank, key, builtin).unwrap();
            state.keys.push(key);
            state.calls.push(OwnedCall {
                caller: DefId(0),
                expression: ExprId(rank),
                template: DefId(0),
                key,
                builtin,
            });
        }
    }
    state.builtin_fixups_len = 64;
    let original_keys = state.keys.clone();
    let original_calls = call_facts(&state);
    let original_fixups = state.builtin_fixups;
    state.builtin_fixups_len = 65;
    assert!(freeze_state(&mut state, nominals, 8, &WorkMeter::default(), at).is_err());
    assert_eq!(state.keys, original_keys);
    assert_eq!(call_facts(&state), original_calls);
    assert_eq!(state.builtin_fixups, original_fixups);
    assert!(!state.builtin_keys_frozen);
    state.builtin_fixups_len = 64;
    //2,016 distinct key pairs plus64 keys and64 calls:4,288 projections.
    // Each projection pays64 table visits and12 controls;64*8 table visits
    // and16 complete caller controls bring the fixed work to326,416.
    let oracle = 326_416 + 4 * nominals.enumerations.len() as u64;
    assert_eq!(
        requested_work(nominals.enumerations.len(), 64, 64, 64, at).unwrap(),
        oracle
    );
    let short = WorkMeter::new(oracle - 1);
    assert!(freeze_state(&mut state, nominals, 8, &short, at).is_err());
    assert_eq!(short.used(), 0);
    assert_eq!(state.keys, original_keys);
    assert_eq!(call_facts(&state), original_calls);
    let exact = WorkMeter::new(oracle);
    freeze_state(&mut state, nominals, 8, &exact, at).unwrap();
    assert_eq!(exact.used(), oracle);
    assert!(state.builtin_keys_frozen);
    for rank in 0..64 {
        let ordinal = 2 + rank / 8;
        assert_eq!(
            state.keys[rank].element(),
            OwnedVectorRoot::Enum(EnumId(ordinal))
        );
        assert_eq!(
            state.keys[rank].operation(),
            owned::Operation::ALL[rank % 8]
        );
        assert_eq!(state.calls[rank].key, state.keys[rank]);
        assert!(!state.keys[..rank].contains(&state.keys[rank]));
    }
}

#[test]
fn builtin_enum_key_projection_admits_the_final_ordinal_and_refuses_overflow_without_writes() {
    let fixture = Fixture::new();
    let project = ProjectSources::load_typed(
        fixture.0.join("main.ox").to_str().unwrap(),
        ProjectLimits::default(),
    )
    .unwrap();
    let index = candidate(&project);
    let at = index.sources().eof();
    let mut state = pending(at);
    state.keys.truncate(1);
    state.calls.truncate(1);
    state.builtin_fixups_len = 1;
    let fixups = &state.builtin_fixups[..1];
    let key = projected_key(&state.keys, fixups, 0, 4095, 1, at).unwrap();
    assert_eq!(key.element(), OwnedVectorRoot::Enum(EnumId(4095)));
    assert_eq!(
        projected_call(fixups, &state.calls[0], 4095, 1, at).unwrap(),
        key
    );
    let original_keys = state.keys.clone();
    let original_calls = call_facts(&state);
    for users in [4096, usize::MAX] {
        assert!(projected_key(&state.keys, fixups, 0, users, 1, at).is_err());
        assert!(projected_call(fixups, &state.calls[0], users, 1, at).is_err());
        assert_eq!(state.keys, original_keys);
        assert_eq!(call_facts(&state), original_calls);
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn builtin_enum_fixup_all_factory_and_frame_transports_have_independent_role_oracles() {
    use std::mem::{align_of, size_of};

    // Independent pre-fixup field inventories from4c91a36. These are test-only
    // Rust-layout mirrors, not new production layouts. In particular, keep the
    // historical call without repr(C) and without the builtin identity byte.
    #[allow(dead_code)]
    struct BeforeCall {
        caller: DefId,
        expression: ExprId,
        template: DefId,
        key: owned::Function,
    }
    #[allow(dead_code)]
    struct BeforeState {
        origin: Span,
        limit: usize,
        retained: usize,
        live: usize,
        keys: Vec<owned::Function>,
        calls: Vec<BeforeCall>,
        files: Vec<u64>,
        planning_request: usize,
        collection_request: [u64; 2],
        source_needed: bool,
        scopes: IndexLimits,
        reserved: usize,
    }
    #[allow(dead_code)]
    struct BeforeOwner {
        state: Mutex<BeforeState>,
        ranges: bool,
    }
    #[allow(dead_code)]
    struct BeforeSourceFact {
        index: &'static DeclarationIndex<'static>,
        mutex: &'static Mutex<BeforeState>,
        locked: TryLockResult<MutexGuard<'static, BeforeState>>,
        selected: MutexGuard<'static, BeforeState>,
        element: Element,
        element_return: Result<Element, Box<Diagnostic>>,
        marked: Result<(), Box<Diagnostic>>,
        at: Span,
        owner: Option<&'static OwnedGenericPlan>,
        returned: bool,
    }
    #[allow(dead_code)]
    struct BeforeFactory {
        state: BeforeState,
        mutex: Mutex<BeforeState>,
        consumed_state: LockResult<BeforeState>,
        mapped_state: Result<BeforeState, Box<Diagnostic>>,
        caller_state: Result<BeforeState, Box<Diagnostic>>,
        construction: BeforeOwner,
        pending: Box<[BeforeOwner; 1]>,
        returned_pending: Result<Box<[BeforeOwner; 1]>, ReserveFailure>,
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
        source_fact: BeforeSourceFact,
    }
    // Exact Enum-parent factory field inventory from69804318. Keep its
    // historical fixup growth separate from the new generic-signature banks.
    #[allow(dead_code)]
    struct EnumFactory {
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
    #[allow(dead_code)]
    struct BeforeFrame {
        operation: owned::Operation,
        key: owned::Function,
        actual: [Option<Type>; 3],
        signature: ([Type; 3], Type),
        result: Result<Type, Box<Diagnostic>>,
        caller_result: Result<Type, Box<Diagnostic>>,
        call: BeforeCall,
        state: &'static Mutex<BeforeState>,
        locked: TryLockResult<MutexGuard<'static, BeforeState>>,
        guard: MutexGuard<'static, BeforeState>,
        mut_locked: TryLockResult<MutexGuard<'static, BeforeState>>,
        mut_guard: MutexGuard<'static, BeforeState>,
        keys: std::iter::Enumerate<std::slice::Iter<'static, owned::Function>>,
        selected: Option<(usize, &'static owned::Function)>,
        calls: std::slice::Iter<'static, BeforeCall>,
        selected_call: Option<&'static BeforeCall>,
        caller: DefId,
        expression: ExprId,
        root: OwnedVectorRoot,
        rank: usize,
        capacity: usize,
        checked: [Option<usize>; 6],
        value: Value,
        element: Element,
        origin: Span,
        returned_unit: Result<(), Box<Diagnostic>>,
    }

    fn check_bank<T>(name: &str, roles: &[(&str, usize, usize)]) -> (usize, usize) {
        let width: usize = roles.iter().map(|(_, bytes, _)| bytes).sum();
        let maximum_alignment = roles
            .iter()
            .map(|(_, _, alignment)| *alignment)
            .max()
            .unwrap();
        let possible_gaps: usize = roles.iter().map(|(_, _, alignment)| alignment - 1).sum();
        let rounded = |bytes: usize| (bytes + maximum_alignment - 1) & !(maximum_alignment - 1);
        // This bound does not assume Rust preserves declaration order or that
        // every field has no padding. Count nested transports once at their
        // actual complete width. A permutation can add at most align-1 before
        // each field, followed by at most maximum_alignment-1 final padding.
        let lower = rounded(width);
        let upper = rounded(width + possible_gaps);
        assert_eq!(align_of::<T>(), maximum_alignment, "{name}");
        assert!(
            (lower..=upper).contains(&size_of::<T>()),
            "{name}: {lower}..={upper}"
        );
        println!("{name}: actual={} align={} role_bytes={width} independent_packing_bounds={lower}..={upper} roles={roles:?}", size_of::<T>(), align_of::<T>());
        (lower, upper)
    }
    macro_rules! inventory {
        ($bank:ident { $($field:ident: $ty:ty),+ $(,)? }) => {{
            // Deliberately omit `..`: a future unmodelled field makes this
            // inventory fail to compile. Each selector also binds the actual
            // field type, including caller/Result and borrowed/mutable roles.
            let _: fn(&$bank) = |row| {
                let $bank { $($field,)+ } = row;
                let _borrowed_roles = ($($field,)+);
            };
            $(let _: for<'a> fn(&'a $bank) -> &'a $ty = |row| &row.$field;)+
            check_bank::<$bank>(stringify!($bank), &[
                $((stringify!($field), size_of::<$ty>(), align_of::<$ty>()),)+
            ])
        }};
    }

    // All known primitive/state widths are hand-calculated on this64-bit ABI:
    // span24 + five usize40 + three Vec headers72 + collection16 + scopes24
    // + one bool1 =177, rounded to184. The current table adds512 and two
    // single-byte controls:691 rounded to696. No six-fold table copies are
    // assumed away merely because an optimizer might elide a Rust move.
    assert_eq!((size_of::<Span>(), size_of::<IndexLimits>()), (24, 24));
    assert_eq!(
        (size_of::<Vec<BeforeCall>>(), size_of::<Vec<OwnedCall>>()),
        (24, 24)
    );
    assert_eq!((size_of::<BeforeCall>(), size_of::<OwnedCall>()), (32, 32));
    assert_eq!(
        (size_of::<BeforeState>(), align_of::<BeforeState>()),
        (184, 8)
    );
    assert_eq!(
        (size_of::<BuildState>(), align_of::<BuildState>()),
        (696, 8)
    );
    let state_bounds = inventory!(BuildState {
        origin: Span, limit: usize, retained: usize, live: usize,
        keys: Vec<owned::Function>, calls: Vec<OwnedCall>, files: Vec<u64>,
        planning_request: usize, collection_request: [u64; 2], source_needed: bool,
        scopes: IndexLimits, reserved: usize, builtin_fixups: BuiltinFixups,
        builtin_fixups_len: u8, builtin_keys_frozen: bool,
    });
    assert_eq!(state_bounds.0, 696);
    inventory!(BuildOwner { state: Mutex<BuildState>, ranges: bool });

    // Distinguish six complete by-value state carriers from two boxed pointer
    // transports. Each by-value role grows512; the boxes remain one word.
    let state_transports = [
        ("state", size_of::<BuildState>(), size_of::<BeforeState>()),
        (
            "mutex",
            size_of::<Mutex<BuildState>>(),
            size_of::<Mutex<BeforeState>>(),
        ),
        (
            "consumed_state",
            size_of::<LockResult<BuildState>>(),
            size_of::<LockResult<BeforeState>>(),
        ),
        (
            "mapped_state",
            size_of::<Result<BuildState, Box<Diagnostic>>>(),
            size_of::<Result<BeforeState, Box<Diagnostic>>>(),
        ),
        (
            "caller_state",
            size_of::<Result<BuildState, Box<Diagnostic>>>(),
            size_of::<Result<BeforeState, Box<Diagnostic>>>(),
        ),
        (
            "construction",
            size_of::<BuildOwner>(),
            size_of::<BeforeOwner>(),
        ),
    ];
    println!("complete factory state transports (role,current,before)={state_transports:?}");
    for (name, current, before) in state_transports {
        assert_eq!(
            current.checked_sub(before),
            Some(512),
            "complete factory state role {name}"
        );
    }
    assert_eq!(
        (
            size_of::<Box<[BeforeOwner; 1]>>(),
            size_of::<Box<[BuildOwner; 1]>>()
        ),
        (8, 8)
    );
    assert_eq!(
        size_of::<Result<Box<[BuildOwner; 1]>, ReserveFailure>>(),
        size_of::<Result<Box<[BeforeOwner; 1]>, ReserveFailure>>(),
    );
    assert_eq!(size_of::<Result<BuiltinItem, Box<Diagnostic>>>(), 16);
    assert_eq!(
        size_of::<SourceFactRoles>() - size_of::<BeforeSourceFact>(),
        16 + 8 + 8
    );
    inventory!(SourceFactRoles {
        index: &'static DeclarationIndex<'static>, mutex: &'static Mutex<BuildState>,
        locked: TryLockResult<MutexGuard<'static, BuildState>>,
        selected: MutexGuard<'static, BuildState>, element: Element,
        element_return: Result<Element, Box<Diagnostic>>, marked: Result<(), Box<Diagnostic>>,
        at: Span, owner: Option<&'static OwnedGenericPlan>, returned: bool,
        builtin: Result<BuiltinItem, Box<Diagnostic>>, enum_id: EnumId,
        template_enum_count: usize,
    });
    inventory!(BorrowedElementRoles {
        index: &'static DeclarationIndex<'static>, value: Value, nominals: &'static Nominals,
        work: &'static WorkMeter, at: Span, position: usize, eligible: bool,
        builder: Option<&'static Mutex<BuildState>>, caller_builder: Option<&'static Mutex<BuildState>>,
        debited: Result<(), Box<Diagnostic>>, marked: Result<(), Box<Diagnostic>>,
        result: Result<Element, Box<Diagnostic>>, caller_result: Result<Element, Box<Diagnostic>>,
        caller_element: Element,
    });
    inventory!(FunctionValueRoles {
        builder: Option<&'static Mutex<BuildState>>, nominals: &'static mut Nominals,
        index: &'static DeclarationIndex<'static>, work: &'static WorkMeter,
        storage: &'static mut Storage, allocator: &'static mut Allocator, module: ModuleId,
        declaration: Span, arguments: &'static [Value], syntax: ast::TypeSyntax,
        application_id: ast::GenericApplicationId, program: &'static ast::Program,
        application: &'static ast::GenericArguments,
        program_result: Result<&'static ast::Program, Box<Diagnostic>>,
        application_option: Option<&'static ast::GenericArguments>,
        application_result: Result<&'static ast::GenericArguments, Box<Diagnostic>>,
        path: ItemPathRef, nominal: NominalId, nominal_result: Result<NominalId, Box<Diagnostic>>,
        argument: ast::TypeSyntax, values: Vec<Value>,
        argument_iterator: std::slice::Iter<'static, ast::TypeSyntax>,
        selected_argument: &'static ast::TypeSyntax, record: RecordId, enumeration: EnumId,
        constructed_value: Value, constructed_result: Result<Value, Box<Diagnostic>>,
        value: Value, value_result: Result<Value, Box<Diagnostic>>,
        element_result: Result<Element, Box<Diagnostic>>, result: Result<Type, Box<Diagnostic>>,
        caller_result: Result<Type, Box<Diagnostic>>, caller_syntax: ast::TypeSyntax,
        caller_type: Type, caller_types: &'static [(Span, Type)],
        caller_iterator: std::slice::Iter<'static, (Span, Type)>,
        caller_selected: &'static (Span, Type), caller_cached: Option<&'static (Span, Type)>,
        entered: Result<(), Box<Diagnostic>>, left: Result<(), Box<Diagnostic>>,
        debited: Result<(), Box<Diagnostic>>, rejected: Result<(), Box<Diagnostic>>,
    });
    let factory_bounds = inventory!(FactoryBank {
        state: BuildState, mutex: Mutex<BuildState>, consumed_state: LockResult<BuildState>,
        mapped_state: Result<BuildState, Box<Diagnostic>>,
        caller_state: Result<BuildState, Box<Diagnostic>>, construction: BuildOwner,
        pending: Box<[BuildOwner; 1]>, returned_pending: Result<Box<[BuildOwner; 1]>, ReserveFailure>,
        index: DeclarationIndex<'static>, facts: DeclarationFacts<'static>, origin: CandidateOrigin,
        owner: OwnedGenericPlan, owner_result: Result<OwnedGenericPlan, Box<Diagnostic>>,
        boxed: Box<[OwnedGenericPlan; 1]>, boxed_result: Result<Box<[OwnedGenericPlan; 1]>, ReserveFailure>,
        returned: Result<DeclarationIndex<'static>, Vec<Diagnostic>>, source: SourceOwner<'static>,
        limits: IndexLimits, work: &'static WorkMeter, allocator: &'static mut Allocator,
        source_fact: SourceFactRoles,
        borrowed_element: BorrowedElementRoles, function_value: FunctionValueRoles,
    });
    let factory_growth = 6 * 512 + (16 + 8 + 8);
    assert_eq!(factory_growth, 3104);
    assert_eq!(
        size_of::<EnumFactory>() - size_of::<BeforeFactory>(),
        factory_growth
    );
    let generic_growth = size_of::<BorrowedElementRoles>() + size_of::<FunctionValueRoles>();
    assert_eq!(
        size_of::<FactoryBank>() - size_of::<EnumFactory>(),
        generic_growth
    );
    println!("historical factory={} source_fact={} frame={}; enum factory={}; current factory={} source_fact={} frame={}; independent enum factory growth=3104; separate generic factory growth={generic_growth}",
        size_of::<BeforeFactory>(), size_of::<BeforeSourceFact>(), size_of::<BeforeFrame>(),
        size_of::<EnumFactory>(),
        size_of::<FactoryBank>(), size_of::<SourceFactRoles>(), size_of::<FrameBank>());
    assert_eq!(
        factory_bytes() - size_of::<BeforeFactory>(),
        3104 + generic_growth
    );
    assert!((factory_bounds.0..=factory_bounds.1).contains(&factory_bytes()));

    let frame_bounds = inventory!(FrameBank {
        operation: owned::Operation, key: owned::Function, actual: [Option<Type>; 3],
        signature: ([Type; 3], Type), result: Result<Type, Box<Diagnostic>>,
        caller_result: Result<Type, Box<Diagnostic>>, call: OwnedCall,
        state: &'static Mutex<BuildState>, locked: TryLockResult<MutexGuard<'static, BuildState>>,
        guard: MutexGuard<'static, BuildState>, mut_locked: TryLockResult<MutexGuard<'static, BuildState>>,
        mut_guard: MutexGuard<'static, BuildState>,
        keys: std::iter::Enumerate<std::slice::Iter<'static, owned::Function>>,
        selected: Option<(usize, &'static owned::Function)>, calls: std::slice::Iter<'static, OwnedCall>,
        selected_call: Option<&'static OwnedCall>, caller: DefId, expression: ExprId,
        root: OwnedVectorRoot, rank: usize, capacity: usize, checked: [Option<usize>; 6],
        value: Value, element: Element, origin: Span, returned_unit: Result<(), Box<Diagnostic>>,
        builtin: u8, fixups: [BuiltinFixup; 2], fixup_return: Result<u8, Box<Diagnostic>>,
        fixup_ordinal: Result<u32, std::num::TryFromIntError>, fixup_rank: usize,
        builtin_item: Result<BuiltinItem, Box<Diagnostic>>, builtin_ordinal: usize,
        builtin_tag: Result<u8, std::num::TryFromIntError>,
    });
    // New aligned roles are16+16+8+8+16+8=72 bytes. The one builtin byte
    // and two-byte checked tag occupy three of the old five tail-pad bytes;
    // Rust may reorder these fields, so assert widths and net growth, not offsets.
    assert_eq!(size_of::<Result<u8, Box<Diagnostic>>>(), 16);
    assert_eq!(size_of::<Result<u32, std::num::TryFromIntError>>(), 8);
    assert_eq!(size_of::<Result<u8, std::num::TryFromIntError>>(), 2);
    assert_eq!(size_of::<FrameBank>() - size_of::<BeforeFrame>(), 72);
    let freeze_bounds = inventory!(FreezeCarriers {
        index: &'static DeclarationIndex<'static>, nominal: &'static Nominals,
        storage: &'static mut Storage, work: &'static WorkMeter, mutex: &'static Mutex<BuildState>,
        guard_result: TryLockResult<MutexGuard<'static, BuildState>>, guard: MutexGuard<'static, BuildState>,
        state: &'static mut BuildState, scopes: [BuiltinFixup; 3], borrowed_fixups: &'static [BuiltinFixup],
        fixup_iteration: std::slice::Iter<'static, BuiltinFixup>, fixup: &'static BuiltinFixup,
        fixup_next: Option<&'static BuiltinFixup>, enum_iteration: std::slice::Iter<'static, nominal::EnumApplication>,
        enumeration: &'static nominal::EnumApplication, keys: &'static [owned::Function],
        calls: std::slice::Iter<'static, OwnedCall>, call: &'static OwnedCall,
        mutable_calls: std::slice::IterMut<'static, OwnedCall>, mutable_call: &'static mut OwnedCall,
        projected: [owned::Function; 4], projected_results: [Result<owned::Function, Box<Diagnostic>>; 4],
        selected_fixup: Option<&'static BuiltinFixup>, root: OwnedVectorRoot, enum_id: EnumId,
        builtin_tag: u8, counts: [usize; 12], checked_counts: [Option<usize>; 8],
        work_result: Result<u64, Box<Diagnostic>>, charged: Result<(), Box<Diagnostic>>,
        returned: Result<(), Box<Diagnostic>>, caller_return: Result<(), Box<Diagnostic>>, at: Span,
    });
    assert!((freeze_bounds.0..=freeze_bounds.1).contains(&control_bytes()));
    assert!(
        (frame_bounds.0 + freeze_bounds.0..=frame_bounds.1 + freeze_bounds.1)
            .contains(&FRAME_BYTES)
    );
    assert_eq!(
        FRAME_BYTES - size_of::<BeforeFrame>(),
        72 + size_of::<FreezeCarriers>()
    );
    // Exact/one-less admission-before-reservation is separately exercised by
    // owned_vector_candidate_metadata_work_and_allocator_boundaries_are_observable
    // and by the fixed-table freeze exact/one-less controls above.
}
