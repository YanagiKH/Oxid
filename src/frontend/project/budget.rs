//! Requested frontend storage, not allocator capacity or total process memory.
use std::mem::size_of;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::frontend) enum ReserveFailure {
    Overflow,
    Allocation,
}

#[derive(Debug, Default)]
pub(in crate::frontend) struct Allocator {
    pub attempts: usize,
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
        let result = vector.try_reserve_exact(if injected { usize::MAX } else { additional });
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
