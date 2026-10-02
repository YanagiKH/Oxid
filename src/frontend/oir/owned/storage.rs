//! Fixed-width checked reference-machine identities; never host pointers.
//! Indices are zero-based. Only identity epochs reserve zero as invalid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(super) struct OwnerKey {
    pub frame: u64,
    pub activation: u64,
    pub owner: u64,
    pub generation: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(super) struct LoanKey {
    pub frame: u64,
    pub activation: u64,
    pub loan: u64,
    pub instance: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(super) struct ReferenceHandle {
    pub root: OwnerKey,
    pub permission: LoanKey,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(super) struct OwnerRuntime {
    pub generation: u64,
    pub state: u64,
    pub shared_children: u64,
    pub exclusive_children: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(super) struct LoanRuntime {
    pub instance: u64,
    pub state: u64,
    pub root: OwnerKey,
    pub parent: LoanKey,
    pub shared_children: u64,
    pub exclusive_children: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(super) struct CallRuntime {
    pub phase: u64,
    pub next_argument: u64,
}

const _: () = {
    assert!(std::mem::size_of::<OwnerKey>() == 32);
    assert!(std::mem::size_of::<LoanKey>() == 32);
    assert!(std::mem::size_of::<ReferenceHandle>() == 64);
    assert!(std::mem::size_of::<OwnerRuntime>() == 32);
    assert!(std::mem::size_of::<LoanRuntime>() == 96);
    assert!(std::mem::size_of::<CallRuntime>() == 16);
};
