//! One-attempt binary transport for verified experimental process operations.
//! The caller owns process setup, loans, and result-storage admission. Dynamic
//! byte work is paid before the attempt; returning the fixed result is infallible.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReadStatus {
    Data(i32),
    Eof,
    Error(i32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WriteStatus {
    Written(i32),
    Error(i32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Failure {
    Bounds,
    Fuel,
    Host,
}

/// Private test seam. An implementation must preserve bytes outside its
/// reported read extent, return counts within the supplied extent, and make
/// exactly one attempt. It must not retain or prefetch from the slice.
pub(super) trait Transport {
    fn read(&mut self, bytes: &mut [u8]) -> std::result::Result<usize, i32>;
    fn write(&mut self, bytes: &[u8]) -> std::result::Result<usize, i32>;
}
pub(super) struct Stdio;
/// Prepay the supplied extent and the sealed primitive's immediate delivery.
/// No transport or result mutation occurs on refusal; successful attempts need
/// no further fuel check. This does not prepay any caller source operation.
pub(super) fn admit_delivery(
    length: usize,
    return_cost: usize,
    fuel: &mut usize,
) -> std::result::Result<(), Failure> {
    if !cfg!(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) {
        return Err(Failure::Host);
    }
    i32::try_from(length).map_err(|_| Failure::Bounds)?;
    #[cfg(test)]
    if testing::refuse_delivery_admission() {
        return Err(Failure::Fuel);
    }
    let remaining = fuel
        .checked_sub(length)
        .and_then(|remaining| remaining.checked_sub(return_cost))
        .ok_or(Failure::Fuel)?;
    *fuel = remaining;
    Ok(())
}
pub(super) fn read_admitted(bytes: &mut [u8]) -> ReadStatus {
    read_attempt(&mut Stdio, bytes)
}
pub(super) fn write_admitted(bytes: &[u8]) -> WriteStatus {
    write_attempt(&mut Stdio, bytes)
}
impl Transport for Stdio {
    fn read(&mut self, bytes: &mut [u8]) -> std::result::Result<usize, i32> {
        #[cfg(test)]
        if let Some(result) = testing::read(bytes) {
            return result;
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            // SAFETY: The writable slice owns its exact initialized extent.
            // libc::read makes a single unbuffered attempt. Capture errno first.
            let result = unsafe { libc::read(0, bytes.as_mut_ptr().cast(), bytes.len()) };
            if result < 0 {
                Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(5))
            } else {
                Ok(result as usize)
            }
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = bytes;
            Err(38)
        }
    }
    fn write(&mut self, bytes: &[u8]) -> std::result::Result<usize, i32> {
        #[cfg(test)]
        if let Some(result) = testing::write(bytes) {
            return result;
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            // SAFETY: Exactly the borrowed initialized extent is readable.
            let result = unsafe { libc::write(1, bytes.as_ptr().cast(), bytes.len()) };
            if result < 0 {
                Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(5))
            } else {
                Ok(result as usize)
            }
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = bytes;
            Err(38)
        }
    }
}

/// Resource-specific deterministic transport. It is absent from production and
/// never changes process descriptors or shares scripts between test threads.
#[cfg(test)]
pub(super) mod testing {
    use std::{cell::RefCell, collections::VecDeque};
    #[derive(Clone, Copy, Debug)]
    pub(in crate::frontend::oir::owned) enum DeliveryFault {
        LocalGeneration,
        DestinationState,
        DestinationGeneration,
        LoanRelease,
        Admission,
    }
    #[derive(Default, Debug)]
    pub(in crate::frontend::oir::owned) struct Script {
        pub reads: VecDeque<Result<Vec<u8>, i32>>,
        pub writes: VecDeque<Result<usize, i32>>,
        pub output: Vec<u8>,
        pub read_calls: usize,
        pub write_calls: usize,
        pub delivery_fault: Option<DeliveryFault>,
        pub fault_applied: bool,
        pub deliveries: usize,
    }
    thread_local! { static ACTIVE: RefCell<Option<Script>> = const { RefCell::new(None) }; }
    pub(in crate::frontend::oir::owned) fn delivery_fault() -> Option<DeliveryFault> {
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let script = active.as_mut()?;
            if matches!(script.delivery_fault, Some(DeliveryFault::Admission)) {
                return None;
            }
            let fault = script.delivery_fault.take()?;
            script.fault_applied = true;
            Some(fault)
        })
    }
    pub(in crate::frontend::oir::owned) fn refuse_delivery_admission() -> bool {
        ACTIVE.with(|active| {
            let mut active = active.borrow_mut();
            let Some(script) = active.as_mut() else {
                return false;
            };
            if matches!(script.delivery_fault, Some(DeliveryFault::Admission)) {
                script.delivery_fault = None;
                script.fault_applied = true;
                true
            } else {
                false
            }
        })
    }
    pub(in crate::frontend::oir::owned) fn delivered() {
        ACTIVE.with(|active| {
            if let Some(script) = active.borrow_mut().as_mut() {
                script.deliveries += 1;
            }
        });
    }
    pub(super) fn read(bytes: &mut [u8]) -> Option<Result<usize, i32>> {
        ACTIVE.with(|active| {
            active.borrow_mut().as_mut().map(|script| {
                script.read_calls += 1;
                let data = script.reads.pop_front().expect("unexpected source read")?;
                assert!(data.len() <= bytes.len());
                bytes[..data.len()].copy_from_slice(&data);
                Ok(data.len())
            })
        })
    }
    pub(super) fn write(bytes: &[u8]) -> Option<Result<usize, i32>> {
        ACTIVE.with(|active| {
            active.borrow_mut().as_mut().map(|script| {
                script.write_calls += 1;
                let count = script.writes.pop_front().unwrap_or(Ok(bytes.len()))?;
                assert!(count <= bytes.len());
                script.output.extend_from_slice(&bytes[..count]);
                Ok(count)
            })
        })
    }
    pub(in crate::frontend::oir::owned) fn capture<T>(
        script: Script,
        run: impl FnOnce() -> T,
    ) -> (T, Script) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                ACTIVE.with(|active| *active.borrow_mut() = None);
            }
        }
        ACTIVE.with(|active| {
            assert!(active.borrow().is_none());
            *active.borrow_mut() = Some(script);
        });
        let reset = Reset;
        let result = run();
        let script = ACTIVE.with(|active| active.borrow_mut().take().unwrap());
        drop(reset);
        (result, script)
    }
}

#[cfg(test)]
fn admit(length: usize, fuel: &mut usize) -> std::result::Result<(), Failure> {
    i32::try_from(length).map_err(|_| Failure::Bounds)?;
    *fuel = fuel.checked_sub(length).ok_or(Failure::Fuel)?;
    Ok(())
}
#[cfg(test)]
pub(super) fn read_some<T: Transport>(
    transport: &mut T,
    bytes: &mut [u8],
    fuel: &mut usize,
) -> std::result::Result<ReadStatus, Failure> {
    admit(bytes.len(), fuel)?;
    Ok(read_attempt(transport, bytes))
}
fn read_attempt<T: Transport>(transport: &mut T, bytes: &mut [u8]) -> ReadStatus {
    if bytes.is_empty() {
        return ReadStatus::Data(0);
    }
    match transport.read(bytes) {
        Ok(0) => ReadStatus::Eof,
        Ok(count) if count <= bytes.len() => ReadStatus::Data(count as i32),
        Ok(_) => ReadStatus::Error(5),
        Err(code) => ReadStatus::Error(code),
    }
}
#[cfg(test)]
pub(super) fn write_some<T: Transport>(
    transport: &mut T,
    bytes: &[u8],
    fuel: &mut usize,
) -> std::result::Result<WriteStatus, Failure> {
    admit(bytes.len(), fuel)?;
    Ok(write_attempt(transport, bytes))
}
fn write_attempt<T: Transport>(transport: &mut T, bytes: &[u8]) -> WriteStatus {
    if bytes.is_empty() {
        return WriteStatus::Written(0);
    }
    match transport.write(bytes) {
        Ok(count) if count <= bytes.len() => WriteStatus::Written(count as i32),
        Ok(_) => WriteStatus::Error(5),
        Err(code) => WriteStatus::Error(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Script {
        reads: std::collections::VecDeque<std::result::Result<Vec<u8>, i32>>,
        writes: std::collections::VecDeque<std::result::Result<usize, i32>>,
        output: Vec<u8>,
        calls: usize,
    }
    impl Transport for Script {
        fn read(&mut self, bytes: &mut [u8]) -> std::result::Result<usize, i32> {
            self.calls += 1;
            let result = self.reads.pop_front().expect("unexpected read")?;
            assert!(result.len() <= bytes.len());
            bytes[..result.len()].copy_from_slice(&result);
            Ok(result.len())
        }
        fn write(&mut self, bytes: &[u8]) -> std::result::Result<usize, i32> {
            self.calls += 1;
            let result = self.writes.pop_front().expect("unexpected write")?;
            assert!(result <= bytes.len());
            self.output.extend_from_slice(&bytes[..result]);
            Ok(result)
        }
    }
    fn script() -> Script {
        Script {
            reads: Default::default(),
            writes: Default::default(),
            output: Vec::new(),
            calls: 0,
        }
    }
    #[test]
    fn binary_io_empty_and_rejected_attempts_have_no_transport_effect() {
        let mut io = script();
        assert_eq!(read_some(&mut io, &mut [], &mut 0), Ok(ReadStatus::Data(0)));
        assert_eq!(
            write_some(&mut io, &[], &mut 0),
            Ok(WriteStatus::Written(0))
        );
        let mut bytes = [99; 5];
        assert_eq!(read_some(&mut io, &mut bytes, &mut 4), Err(Failure::Fuel));
        assert_eq!(write_some(&mut io, &bytes, &mut 4), Err(Failure::Fuel));
        assert_eq!(bytes, [99; 5]);
        assert_eq!(io.calls, 0);
        assert_eq!(admit(i32::MAX as usize + 1, &mut 0), Err(Failure::Bounds));
    }
    #[test]
    fn binary_io_short_reads_interrupt_eof_and_errors_preserve_tail() {
        let mut io = script();
        io.reads = [
            Ok(vec![0, 128, 255]),
            Err(4),
            Ok(vec![7]),
            Err(9),
            Ok(vec![]),
        ]
        .into();
        let mut bytes = [99; 8];
        let mut fuel = 40;
        assert_eq!(
            read_some(&mut io, &mut bytes, &mut fuel),
            Ok(ReadStatus::Data(3))
        );
        assert_eq!(bytes, [0, 128, 255, 99, 99, 99, 99, 99]);
        assert_eq!(
            read_some(&mut io, &mut bytes, &mut fuel),
            Ok(ReadStatus::Error(4))
        );
        assert_eq!(bytes, [0, 128, 255, 99, 99, 99, 99, 99]);
        assert_eq!(
            read_some(&mut io, &mut bytes, &mut fuel),
            Ok(ReadStatus::Data(1))
        );
        assert_eq!(bytes, [7, 128, 255, 99, 99, 99, 99, 99]);
        assert_eq!(
            read_some(&mut io, &mut bytes, &mut fuel),
            Ok(ReadStatus::Error(9))
        );
        assert_eq!(
            read_some(&mut io, &mut bytes, &mut fuel),
            Ok(ReadStatus::Eof)
        );
        assert_eq!(io.calls, 5);
        assert_eq!(fuel, 0);
    }
    #[test]
    fn binary_io_short_writes_do_not_retry_duplicate_or_hide_zero_progress() {
        let mut io = script();
        io.writes = [Ok(2), Err(4), Ok(1), Ok(0), Err(32), Ok(2)].into();
        let bytes = [0, 128, 255, 7, 9];
        let mut fuel = 100;
        assert_eq!(
            write_some(&mut io, &bytes, &mut fuel),
            Ok(WriteStatus::Written(2))
        );
        assert_eq!(
            write_some(&mut io, &bytes[2..], &mut fuel),
            Ok(WriteStatus::Error(4))
        );
        assert_eq!(
            write_some(&mut io, &bytes[2..], &mut fuel),
            Ok(WriteStatus::Written(1))
        );
        assert_eq!(
            write_some(&mut io, &bytes[3..], &mut fuel),
            Ok(WriteStatus::Written(0))
        );
        assert_eq!(
            write_some(&mut io, &bytes[3..], &mut fuel),
            Ok(WriteStatus::Error(32))
        );
        assert_eq!(
            write_some(&mut io, &bytes[3..], &mut fuel),
            Ok(WriteStatus::Written(2))
        );
        assert_eq!(io.output, bytes);
        assert_eq!(io.calls, 6);
        assert_eq!(fuel, 83);
    }
}
