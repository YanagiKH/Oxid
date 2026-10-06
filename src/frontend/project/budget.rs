//! Requested frontend storage, not allocator capacity or total process memory.
use std::mem::size_of;
use std::path::PathBuf;

#[cfg(test)]
#[path = "budget_real_null_observer.rs"]
pub(in crate::frontend) mod real_null_observer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum ReserveFailure {
    Overflow,
    Allocation,
}

#[derive(Debug, Default)]
pub(in crate::frontend) struct Allocator {
    pub attempts: usize,
    #[cfg(test)]
    pub observer_trace_limit: Option<usize>,
    #[cfg(test)]
    pub observer_trace_overflow: bool,
    #[cfg(test)]
    pub fail_at: Option<usize>,
    /// Qualification instrumentation is not part of production storage.
    #[cfg(test)]
    pub trace: Vec<ReserveEvent>,
}

#[cfg(test)]
#[derive(Debug)]
pub(in crate::frontend) struct ReserveEvent {
    pub kind: &'static str,
    pub length: usize,
    pub element_bytes: usize,
    pub success: bool,
}

impl Allocator {
    /// Qualification-only retention bound; allocation attempts and results are
    /// unchanged. The caller admits this separate trace payload before reserve.
    #[cfg(test)]
    pub fn observer_trace_bound(&mut self, rows: usize) -> Result<(), ReserveFailure> {
        self.observer_trace_reserve(rows, false)
    }

    /// Force the trace's own real capacity failure without consuming or changing
    /// any frontend allocation attempt ordinal.
    #[cfg(test)]
    pub fn observer_trace_bound_capacity_failure(
        &mut self,
        rows: usize,
    ) -> Result<(), ReserveFailure> {
        self.observer_trace_reserve(rows, true)
    }

    #[cfg(test)]
    fn observer_trace_reserve(
        &mut self,
        rows: usize,
        capacity_failure: bool,
    ) -> Result<(), ReserveFailure> {
        if rows > 200_000 || !self.trace.is_empty() || self.observer_trace_limit.is_some() {
            return Err(ReserveFailure::Overflow);
        }
        rows.checked_mul(size_of::<ReserveEvent>())
            .ok_or(ReserveFailure::Overflow)?;
        self.trace
            .try_reserve_exact(if capacity_failure { usize::MAX } else { rows })
            .map_err(|_| ReserveFailure::Allocation)?;
        self.observer_trace_limit = Some(rows);
        Ok(())
    }

    fn request(&mut self, length: usize, element_bytes: usize) -> Result<bool, ReserveFailure> {
        length
            .checked_mul(element_bytes)
            .ok_or(ReserveFailure::Overflow)?;
        self.attempts = self
            .attempts
            .checked_add(1)
            .ok_or(ReserveFailure::Overflow)?;
        #[cfg(test)]
        return Ok(self.fail_at == Some(self.attempts));
        #[cfg(not(test))]
        Ok(false)
    }

    fn record(&mut self, kind: &'static str, length: usize, element_bytes: usize, success: bool) {
        #[cfg(test)]
        if self
            .observer_trace_limit
            .is_some_and(|cap| self.trace.len() >= cap)
        {
            self.observer_trace_overflow = true;
            return;
        }
        #[cfg(test)]
        self.trace.push(ReserveEvent {
            kind,
            length,
            element_bytes,
            success,
        });
        #[cfg(not(test))]
        let _ = (kind, length, element_bytes, success);
    }

    pub fn vector<T>(
        &mut self,
        vector: &mut Vec<T>,
        additional: usize,
        kind: &'static str,
    ) -> Result<(), ReserveFailure> {
        let length = vector
            .len()
            .checked_add(additional)
            .ok_or(ReserveFailure::Overflow)?;
        let injected = self.request(length, size_of::<T>())?;
        // The seam exercises a real fallible reserve error (capacity overflow),
        // not a promise to recover a process-wide OOM.
        let result = vector.try_reserve(if injected { usize::MAX } else { additional });
        self.record(kind, length, size_of::<T>(), result.is_ok());
        result.map_err(|_| ReserveFailure::Allocation)
    }

    /// Exact requested slots for a known bounded vector; the existing vector
    /// reservation path intentionally retains its original growth behavior.
    pub fn vector_exact<T>(
        &mut self,
        vector: &mut Vec<T>,
        additional: usize,
        kind: &'static str,
    ) -> Result<(), ReserveFailure> {
        let length = vector
            .len()
            .checked_add(additional)
            .ok_or(ReserveFailure::Overflow)?;
        let injected = self.request(length, size_of::<T>())?;
        let result = {
            #[cfg(test)]
            let _guard = real_null_observer::enter_exact::<T>(
                real_null_observer::identity(self),
                self.attempts,
                kind,
                vector.len(),
                vector.capacity(),
                additional,
                self.fail_at.is_some(),
            );
            vector.try_reserve_exact(if injected { usize::MAX } else { additional })
        };
        self.record(kind, length, size_of::<T>(), result.is_ok());
        result.map_err(|_| ReserveFailure::Allocation)
    }

    pub fn string(
        &mut self,
        string: &mut String,
        additional: usize,
        kind: &'static str,
    ) -> Result<(), ReserveFailure> {
        let length = string
            .len()
            .checked_add(additional)
            .ok_or(ReserveFailure::Overflow)?;
        let injected = self.request(length, 1)?;
        let result = string.try_reserve_exact(if injected { usize::MAX } else { additional });
        self.record(kind, length, 1, result.is_ok());
        result.map_err(|_| ReserveFailure::Allocation)
    }

    pub fn path(
        &mut self,
        path: &mut PathBuf,
        additional: usize,
        kind: &'static str,
    ) -> Result<(), ReserveFailure> {
        let length = path
            .as_os_str()
            .len()
            .checked_add(additional)
            .ok_or(ReserveFailure::Overflow)?;
        let injected = self.request(length, 1)?;
        let result = path.try_reserve_exact(if injected { usize::MAX } else { additional });
        self.record(kind, length, 1, result.is_ok());
        result.map_err(|_| ReserveFailure::Allocation)
    }
}

#[cfg(test)]
mod observer_trace_tests {
    use super::*;

    #[test]
    fn unit3b2_trace_zero_exact_and_overflow_preserve_requests() {
        for rows in [0, 1] {
            let mut allocator = Allocator::default();
            allocator.observer_trace_bound(rows).unwrap();
            let mut values = Vec::<u8>::new();
            allocator.vector_exact(&mut values, 0, "zero").unwrap();
            assert_eq!(allocator.attempts, 1);
            assert_eq!(allocator.trace.len(), rows);
            assert_eq!(allocator.observer_trace_overflow, rows == 0);
            allocator.vector_exact(&mut values, 1, "one").unwrap();
            assert_eq!(allocator.attempts, 2);
            assert_eq!(allocator.trace.len(), rows);
            assert!(allocator.observer_trace_overflow);
        }
    }

    #[test]
    fn unit3b2_trace_own_real_failure_does_not_consume_frontend_ordinal() {
        for rows in [0, 1, 200_000] {
            let mut allocator = Allocator::default();
            assert_eq!(
                allocator.observer_trace_bound_capacity_failure(rows),
                Err(ReserveFailure::Allocation)
            );
            assert_eq!(allocator.attempts, 0);
            assert!(allocator.trace.is_empty());
            assert_eq!(allocator.observer_trace_limit, None);
            assert!(!allocator.observer_trace_overflow);
        }
    }

    #[test]
    fn unit3b2_trace_retains_real_frontend_failure_at_original_ordinal() {
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Allocator::default()
        };
        allocator.observer_trace_bound(1).unwrap();
        assert_eq!(
            allocator.vector_exact(&mut Vec::<u8>::new(), 0, "zero"),
            Err(ReserveFailure::Allocation)
        );
        assert_eq!(allocator.attempts, 1);
        assert_eq!(allocator.trace.len(), 1);
        assert_eq!(allocator.trace[0].length, 0);
        assert!(!allocator.trace[0].success);
        assert!(!allocator.observer_trace_overflow);
    }
}
