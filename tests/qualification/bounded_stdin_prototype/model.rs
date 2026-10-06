//! Disconnected executable specification. No source binding, OIR witness or OS I/O.
//! Supplied Rust borrows stand in for already-validated exclusive runtime access.

const MAX_CAPACITY: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadStatus {
    Eof(i32),
    Full,
    IoError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadEvent {
    Byte(u8),
    Eof,
    Interrupted,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Capacity,
    Staging,
    Fuel,
    // A missing controlled event is a broken fixture, never simulated EOF.
    ScriptExhausted,
}

trait ByteInput {
    fn read_one(&mut self) -> Option<ReadEvent>;
}

fn charge(fuel: &mut usize, amount: usize) -> Result<(), Failure> {
    *fuel = fuel.checked_sub(amount).ok_or(Failure::Fuel)?;
    Ok(())
}

fn fill(
    destination: &mut [i32],
    staging: &mut [u8],
    fuel: &mut usize,
    input: &mut impl ByteInput,
) -> Result<ReadStatus, Failure> {
    let capacity = destination.len();
    if capacity > MAX_CAPACITY {
        return Err(Failure::Capacity);
    }
    charge(fuel, 4 + capacity)?;
    if staging.len() < capacity {
        return Err(Failure::Staging);
    }
    let mut count = 0;
    let status = loop {
        if count == capacity {
            break ReadStatus::Full;
        }
        charge(fuel, 1)?;
        match input.read_one().ok_or(Failure::ScriptExhausted)? {
            ReadEvent::Byte(byte) => {
                staging[count] = byte;
                count += 1;
            }
            ReadEvent::Eof => break ReadStatus::Eof(count as i32),
            ReadEvent::Interrupted => {}
            ReadEvent::Error => return Ok(ReadStatus::IoError),
        }
    };
    // Everything after the read phase is infallible and already paid. No tail read.
    for (cell, byte) in destination[..count].iter_mut().zip(&staging[..count]) {
        *cell = i32::from(*byte);
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Script<'a> {
        events: &'a [ReadEvent],
        attempts: usize,
        consumed: usize,
    }
    impl<'a> Script<'a> {
        fn new(events: &'a [ReadEvent]) -> Self {
            Self {
                events,
                attempts: 0,
                consumed: 0,
            }
        }
    }
    impl ByteInput for Script<'_> {
        fn read_one(&mut self) -> Option<ReadEvent> {
            self.attempts += 1;
            let event = self.events.get(self.consumed).copied()?;
            self.consumed += 1;
            Some(event)
        }
    }

    #[test]
    fn table_defines_success_error_and_exact_debits() {
        use ReadEvent::*;
        // capacity, input events, result, destination, remaining fuel, attempts.
        let cases: &[(usize, &[ReadEvent], ReadStatus, &[i32], usize, usize)] = &[
            (0, &[Byte(9)], ReadStatus::Full, &[], 96, 0),
            (3, &[Eof], ReadStatus::Eof(0), &[-7, -7, -7], 92, 1),
            (3, &[Byte(0), Eof], ReadStatus::Eof(1), &[0, -7, -7], 91, 2),
            (
                3,
                &[Byte(0), Byte(128), Byte(255), Byte(9)],
                ReadStatus::Full,
                &[0, 128, 255],
                90,
                3,
            ),
            (2, &[Byte(9), Eof], ReadStatus::Eof(1), &[9, -7], 92, 2),
            (2, &[Byte(9), Error], ReadStatus::IoError, &[-7, -7], 92, 2),
            (
                2,
                &[Interrupted, Byte(9), Interrupted, Eof],
                ReadStatus::Eof(1),
                &[9, -7],
                90,
                4,
            ),
            (2, &[Error], ReadStatus::IoError, &[-7, -7], 93, 1),
        ];
        for &(capacity, events, status, expected, remaining, attempts) in cases {
            let mut destination = [-7; 4];
            let mut staging = [0xa5; 4];
            let mut fuel = 100;
            let mut input = Script::new(events);
            assert_eq!(
                fill(
                    &mut destination[..capacity],
                    &mut staging,
                    &mut fuel,
                    &mut input
                ),
                Ok(status)
            );
            assert_eq!(&destination[..capacity], expected);
            assert!(destination[capacity..].iter().all(|v| *v == -7));
            assert!(staging[capacity..].iter().all(|v| *v == 0xa5));
            assert_eq!(
                (fuel, input.attempts, input.consumed),
                (remaining, attempts, attempts)
            );
        }
    }

    #[test]
    fn fuel_failures_preserve_destination_and_stop_before_next_read() {
        use ReadEvent::*;
        // Initial fuel, expected fuel, consumed events. Base is 7 for capacity 3.
        for (initial, remaining, consumed) in [(6, 6, 0), (7, 0, 0), (8, 0, 1), (9, 0, 2)] {
            let mut destination = [71, 72, 73];
            let mut staging = [0xa5; 3];
            let mut input = Script::new(&[Byte(1), Interrupted, Byte(2), Eof]);
            let mut fuel = initial;
            assert_eq!(
                fill(&mut destination, &mut staging, &mut fuel, &mut input),
                Err(Failure::Fuel)
            );
            assert_eq!(destination, [71, 72, 73]);
            assert_eq!(
                (fuel, input.attempts, input.consumed),
                (remaining, consumed, consumed)
            );
        }
    }

    #[test]
    fn endless_interruptions_are_fuel_bounded() {
        let mut destination = [71];
        let mut staging = [0];
        let mut fuel = 8;
        let mut input = Script::new(&[ReadEvent::Interrupted; 4]);
        assert_eq!(
            fill(&mut destination, &mut staging, &mut fuel, &mut input),
            Err(Failure::Fuel)
        );
        assert_eq!((fuel, input.attempts, destination), (0, 3, [71]));
    }

    #[test]
    fn admission_precedes_input_and_preserves_memory() {
        let mut input = Script::new(&[ReadEvent::Byte(1)]);
        let mut fuel = 100;
        let mut destination = [71, 72];
        assert_eq!(
            fill(&mut destination, &mut [0], &mut fuel, &mut input),
            Err(Failure::Staging)
        );
        assert_eq!((destination, fuel, input.attempts), ([71, 72], 94, 0));
        let mut too_large = [71; 1025];
        assert_eq!(
            fill(&mut too_large, &mut [], &mut fuel, &mut input),
            Err(Failure::Capacity)
        );
        assert_eq!((fuel, input.attempts), (94, 0));
        assert!(too_large.iter().all(|v| *v == 71));
    }

    #[test]
    fn exact_capacity_does_not_probe_eof_and_next_call_can_observe_it() {
        let mut input = Script::new(&[ReadEvent::Byte(8), ReadEvent::Eof]);
        let mut destination = [71];
        let mut staging = [0];
        let mut fuel = 12;
        assert_eq!(
            fill(&mut destination, &mut staging, &mut fuel, &mut input),
            Ok(ReadStatus::Full)
        );
        assert_eq!((destination, fuel, input.consumed), ([8], 6, 1));
        destination[0] = 72;
        assert_eq!(
            fill(&mut destination, &mut staging, &mut fuel, &mut input),
            Ok(ReadStatus::Eof(0))
        );
        assert_eq!((destination, fuel, input.consumed), ([72], 0, 2));
    }

    #[test]
    fn script_exhaustion_is_never_eof_and_does_not_commit_prefix() {
        let mut input = Script::new(&[ReadEvent::Byte(8)]);
        let mut destination = [71, 72];
        let mut fuel = 10;
        assert_eq!(
            fill(&mut destination, &mut [0; 2], &mut fuel, &mut input),
            Err(Failure::ScriptExhausted)
        );
        assert_eq!(
            (destination, fuel, input.attempts, input.consumed),
            ([71, 72], 2, 2, 1)
        );
    }

    #[test]
    fn maximum_capacity_has_exact_charge_and_no_lookahead() {
        let events = [ReadEvent::Byte(255); 1025];
        let mut input = Script::new(&events);
        let mut destination = [71; 1024];
        let mut fuel = 2052;
        assert_eq!(
            fill(&mut destination, &mut [0; 1024], &mut fuel, &mut input),
            Ok(ReadStatus::Full)
        );
        assert_eq!((fuel, input.consumed), (0, 1024));
        assert!(destination.iter().all(|v| *v == 255));
    }
}
