//! Private scalar Emit output-buffer admission. The reviewed importer prepays
//! compile work and the complete outside carrier inventory; default routes keep
//! their original allocation and diagnostic behavior.
//!
//! Only the final LLVM String request is controlled here. Existing native graph
//! metadata, diagnostics, labels and formatting temporaries remain inherited
//! allocations, including allocations in the authoritative count pass.
use super::{
    execute, hir, Diagnostic, Emission, EntryPolicy, SourceMap, VerifiedProgram,
    MAX_GUARDED_DIAGNOSTIC_BYTES, MAX_GUARDED_IR_BYTES,
};
use crate::frontend::project::budget::{Allocator, ReserveFailure};
use std::mem::{size_of, size_of_val};

const PRIVATE_EMIT_ADMITTED: bool = true;
const MAX_RETAINED: usize = 32 * 1024 * 1024;
const MAX_SCRATCH: usize = 16 * 1024 * 1024;

/// Authentic native diagnostics retain their box and content. New failures are
/// fixed discriminants, with no supposedly infallible diagnostic allocation.
#[derive(Debug)]
pub(in crate::frontend::oir) enum Failure {
    Diagnostic(Box<Diagnostic>),
    Budget,
    Allocation,
    Invariant,
    Disabled,
}
impl From<Box<Diagnostic>> for Failure {
    fn from(value: Box<Diagnostic>) -> Self {
        Self::Diagnostic(value)
    }
}

/// Fixed resource inputs only: no candidate-supplied byte count, owner, source,
/// Program, witness, callback, fuel or policy. Outside bytes must include every
/// complete importer carrier/transport, as measured in its owning module.
pub(in crate::frontend::oir) struct Admission<'a> {
    remaining_retained: usize,
    remaining_scratch: usize,
    outside_carrier_bytes: usize,
    allocator: &'a mut Allocator,
}
impl<'a> Admission<'a> {
    pub(in crate::frontend::oir) fn new(
        remaining_retained: usize,
        remaining_scratch: usize,
        outside_carrier_bytes: usize,
        allocator: &'a mut Allocator,
    ) -> Result<Self, Failure> {
        if remaining_retained > MAX_RETAINED || remaining_scratch > MAX_SCRATCH {
            return Err(Failure::Budget);
        }
        Ok(Self {
            remaining_retained,
            remaining_scratch,
            outside_carrier_bytes,
            allocator,
        })
    }

    fn preflight(&self, counted: usize) -> Result<(), Failure> {
        let fixed = named_bytes()?
            .checked_add(self.outside_carrier_bytes)
            .ok_or(Failure::Budget)?;
        let retained = fixed.checked_add(counted).ok_or(Failure::Budget)?;
        if fixed > self.remaining_scratch || retained > self.remaining_retained {
            return Err(Failure::Budget);
        }
        Ok(())
    }
}

/// Closed internal choice, never a caller-provided allocator callback.
pub(super) enum OutputMode<'a> {
    Default,
    Private(Admission<'a>),
}
impl OutputMode<'_> {
    pub(super) fn preflight_fixed(&self) -> Result<(), Failure> {
        match self {
            Self::Default => Ok(()),
            Self::Private(admission) => admission.preflight(0),
        }
    }

    pub(super) fn allocate(self, counted: usize, policy: EntryPolicy) -> Result<Emission, Failure> {
        match self {
            Self::Default => Ok(Emission {
                len: 0,
                text: Some(String::with_capacity(counted)),
                policy,
                private: None,
            }),
            Self::Private(admission) => {
                // The authoritative count and the complete fixed coexistence
                // envelope are admitted before this one output-buffer request.
                admission.preflight(counted)?;
                let mut text = String::new();
                admission
                    .allocator
                    .string(&mut text, counted, "private LLVM text")
                    .map_err(|failure| match failure {
                        ReserveFailure::Overflow => Failure::Budget,
                        ReserveFailure::Allocation => Failure::Allocation,
                    })?;
                // This observes requested String capacity, not physical allocator
                // rounding or total process memory. Preflight already happened.
                if text.capacity() != counted {
                    return Err(Failure::Invariant);
                }
                Ok(Emission {
                    len: 0,
                    text: Some(text),
                    policy,
                    private: Some(RenderLimit {
                        expected: counted,
                        failed: false,
                    }),
                })
            }
        }
    }
}

pub(super) struct RenderLimit {
    pub(super) expected: usize,
    pub(super) failed: bool,
}

impl Emission {
    pub(super) fn finish(self, counted: usize) -> Result<String, Failure> {
        if let Some(limit) = self.private {
            if limit.failed || self.len != counted || limit.expected != counted {
                return Err(Failure::Invariant);
            }
            let text = self.text.ok_or(Failure::Invariant)?;
            if text.len() != counted || text.capacity() != counted {
                return Err(Failure::Invariant);
            }
            Ok(text)
        } else {
            // Preserve the original default assertion and error behavior.
            debug_assert_eq!(self.len, counted);
            Ok(self.text.expect("render pass"))
        }
    }
}

impl VerifiedProgram {
    pub(in crate::frontend::oir) fn native_module_private(
        &self,
        entry: Option<hir::DefId>,
        sources: &SourceMap,
        admission: Admission<'_>,
    ) -> Result<String, Failure> {
        // The fixed private gate precedes entry checks, native graph admission,
        // diagnostics, count, final reserve and render. Tests cannot enable it.
        if !PRIVATE_EMIT_ADMITTED {
            return Err(Failure::Disabled);
        }
        self.native_module_policy_limits_mode(
            entry,
            sources,
            execute::MAX_FUEL,
            MAX_GUARDED_DIAGNOSTIC_BYTES,
            MAX_GUARDED_IR_BYTES,
            EntryPolicy::Result,
            OutputMode::Private(admission),
        )
    }
}

/// Complete native-owned changed carriers and named call/return roles. The
/// caller adds its independently measured complete outside envelope. Summing
/// sequential moves as well as coexistence is conservative; this is neither a
/// stack-frame estimate nor pricing of inherited native heap payloads. The one
/// final String payload is added once, separately, at preflight.
pub(in crate::frontend::oir) fn named_bytes() -> Result<usize, Failure> {
    let copies = |bytes: usize, count: usize| bytes.checked_mul(count).ok_or(Failure::Budget);
    let roles = [
        // Admission constructor input, local, return and held caller result.
        size_of::<(usize, usize, usize, &mut Allocator)>(),
        size_of::<Admission<'_>>(),
        copies(size_of::<Result<Admission<'_>, Failure>>(), 2)?,
        // Private entry caller/callee complete input tuples.
        copies(
            size_of::<(
                &VerifiedProgram,
                Option<hir::DefId>,
                &SourceMap,
                Admission<'_>,
            )>(),
            2,
        )?,
        // Shared body's complete call input and callee transport, including
        // the larger enum. There is no field-subtotal approximation.
        copies(
            size_of::<(
                &VerifiedProgram,
                Option<hir::DefId>,
                &SourceMap,
                usize,
                usize,
                usize,
                EntryPolicy,
                OutputMode<'_>,
            )>(),
            2,
        )?,
        copies(size_of::<OutputMode<'_>>(), 2)?,
        // Count and render use the complete changed Emission carrier.
        copies(size_of::<Emission>(), 2)?,
        size_of::<(OutputMode<'_>, usize, EntryPolicy)>(),
        copies(size_of::<Result<Emission, Failure>>(), 2)?,
        size_of::<RenderLimit>(),
        size_of::<Option<RenderLimit>>(),
        // Complete preflight inputs/locals/results and checked arithmetic.
        size_of::<(&Admission<'_>, usize)>(),
        copies(size_of::<usize>(), 4)?,
        copies(size_of::<Option<usize>>(), 4)?,
        copies(size_of::<Result<usize, Failure>>(), 4)?,
        copies(size_of::<Result<(), Failure>>(), 2)?,
        // Existing allocator is borrowed, not constructed. Conservatively pay
        // its complete carrier too; test-only trace payload remains excluded.
        size_of::<Allocator>(),
        size_of::<(&mut Allocator, &mut String, usize, &'static str)>(),
        size_of::<Result<(), ReserveFailure>>(),
        size_of::<ReserveFailure>(),
        size_of::<String>(),
        // finish's complete moved input; returned String moves through finish,
        // shared body, private entry and the caller without cloning its payload.
        size_of::<(Emission, usize)>(),
        copies(size_of::<Result<String, Failure>>(), 4)?,
        copies(size_of::<Failure>(), 4)?,
        // Per-append guard receiver/input and fixed local/result carriers.
        size_of::<(&mut Emission, &str)>(),
        size_of::<&mut RenderLimit>(),
        size_of::<&mut String>(),
        size_of::<Option<usize>>(),
        size_of::<std::fmt::Result>(),
        // RFC0030: complete new admission/emission/helper roles, not merely
        // unchanged enum sizes. const sizeof adds no nested inventory loop.
        super::scalar_resource::named_bytes(),
    ];
    // Include the accounting array, move transport and actual iterator itself.
    let bank = size_of_val(&roles)
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(size_of_val(&roles.into_iter())))
        .ok_or(Failure::Budget)?;
    roles.into_iter().try_fold(bank, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Failure::Budget)
    })
}
