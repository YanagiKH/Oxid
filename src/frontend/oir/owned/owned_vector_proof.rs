//! Inert entry to the one ordinary signature/shape/CFG/flow authority.
//! A successful result cannot be passed to either executable consumer.
use super::signature_queries::ScopedSignatureQueries;
use super::*;

struct ProbeControls {
    context: &'static OwnedVectorContext<'static>,
    budget: &'static mut RemainingContextBudget<'static>,
    sources: &'static SourceMap,
    raw: &'static RawOwnedProgram,
    usage: OwnershipUsage,
    usage_return: Result<OwnershipUsage, OwnedFailure>,
    caller_usage_return: Result<OwnershipUsage, OwnedFailure>,
    preflight_return: Result<OwnershipUsage, OwnedFailure>,
    limits: budget::Limits,
    usage_argument: &'static mut OwnershipUsage,
    meter_argument: &'static mut budget::Meter,
    meter: budget::Meter,
    queries: ScopedSignatureQueries<'static, 'static, 'static>,
    query_return: Result<ScopedSignatureQueries<'static, 'static, 'static>, OwnedFailure>,
    validation: Result<(), OwnedFailure>,
    requested_metadata: usize,
    remaining_metadata: Option<usize>,
    remaining_work: Option<usize>,
    charged_totals: [Option<usize>; 2],
}

impl<'program> OwnedVectorContext<'program> {
    /// Core raw storage/scratch keeps its inherited logical request model.
    /// New controls are separately prepaid, as is the ordinary pass's complete
    /// old work allowance; added typed lookups spend the shared query ledger.
    pub(in crate::frontend::oir::owned) fn probe_ordinary<const SOURCE_ASSOCIATED: bool>(
        &self,
        sources: &SourceMap,
        remaining: &mut RemainingContextBudget<'program>,
    ) -> Result<OwnershipUsage, OwnedFailure> {
        if !std::ptr::eq(self.wrapper, remaining.wrapper) {
            return Err(bad());
        }
        let mut usage = budget::preflight(&self.wrapper.program, remaining.limits)?;
        let metadata = remaining
            .metadata
            .checked_sub(size_of::<ProbeControls>())
            .ok_or_else(|| resource("owned vector ordinary proof metadata"))?;
        let work = remaining
            .work
            .checked_sub(usage.work)
            .ok_or_else(|| resource("owned vector ordinary proof work"))?;
        remaining.metadata = metadata;
        remaining.work = work;
        let mut meter = budget::Meter {
            visits: 0,
            ceiling: usage.work,
        };
        {
            let mut queries = ScopedSignatureQueries::new(self, remaining)?;
            if SOURCE_ASSOCIATED {
                verified::probe_source_with_queries(
                    &self.wrapper.program,
                    &mut queries,
                    sources,
                    &mut usage,
                    &mut meter,
                )?;
            } else {
                verified::probe_with_queries(
                    &self.wrapper.program,
                    &mut queries,
                    sources,
                    &mut usage,
                    &mut meter,
                )?;
            }
        }
        // Report this caller's full charged logical scopes, including the
        // immutable wrapper/context and typed lookups, rather than relabeling
        // the old raw-core payload alone as the complete extended request.
        usage.metadata_bytes = remaining
            .limits
            .metadata
            .min(budget::Limits::DEFAULT.metadata)
            .checked_sub(remaining.metadata)
            .ok_or_else(bad)?;
        usage.work = remaining
            .limits
            .work
            .min(budget::Limits::DEFAULT.work)
            .checked_sub(remaining.work)
            .ok_or_else(bad)?;
        Ok(usage)
    }
}
