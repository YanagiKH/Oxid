//! Disconnected event model: no compiler capability, OS calls, or allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    Written,
    Interrupted,
    Error,
    Zero,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Complete,
    InvalidInput,
    IoError(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Capacity,
    Staging,
    Fuel,
    MissingTestEvent,
}
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    result: Result<Status, Failure>,
    events: usize,
    written: usize,
    fuel: u64,
}
fn debit(fuel: &mut u64, amount: u64) -> Result<(), Failure> {
    let remaining = fuel.checked_sub(amount).ok_or(Failure::Fuel)?;
    *fuel = remaining;
    Ok(())
}
fn write_model(
    input: &[i32],
    staging: &mut [u8],
    output: &mut [u8],
    events: &[Event],
    initial_fuel: u64,
) -> Observation {
    let mut fuel = initial_fuel;
    let mut used = 0;
    let mut written = 0;
    let result = (|| {
        if input.len() > 1024 {
            return Err(Failure::Capacity);
        }
        if staging.len() < input.len() || output.len() < input.len() {
            return Err(Failure::Staging);
        }
        debit(&mut fuel, 4 + input.len() as u64)?;
        for (index, value) in input.iter().copied().enumerate() {
            if !(0..=255).contains(&value) {
                return Ok(Status::InvalidInput);
            }
            staging[index] = value as u8;
        }
        while written < input.len() {
            debit(&mut fuel, 1)?;
            let event = *events.get(used).ok_or(Failure::MissingTestEvent)?;
            used += 1;
            match event {
                Event::Written => {
                    output[written] = staging[written];
                    written += 1;
                }
                Event::Interrupted => {}
                Event::Error | Event::Zero => return Ok(Status::IoError(written)),
            }
        }
        Ok(Status::Complete)
    })();
    Observation {
        result,
        events: used,
        written,
        fuel,
    }
}
fn process_status(value: i32) -> Result<u8, ()> {
    u8::try_from(value).map_err(|_| ())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_and_byte_endpoints() {
        let mut stage = [9; 3];
        let mut out = [77; 3];
        assert_eq!(
            write_model(&[], &mut stage, &mut out, &[], 4),
            Observation {
                result: Ok(Status::Complete),
                events: 0,
                written: 0,
                fuel: 0
            }
        );
        assert_eq!(out, [77; 3]);
        assert_eq!(
            write_model(&[0, 255], &mut stage, &mut out, &[Event::Written; 2], 8),
            Observation {
                result: Ok(Status::Complete),
                events: 2,
                written: 2,
                fuel: 0
            }
        );
        assert_eq!(out, [0, 255, 77]);
    }
    #[test]
    fn invalid_last_cell_never_writes() {
        for invalid in [-1, 256] {
            let mut stage = [0; 2];
            let mut out = [77; 2];
            assert_eq!(
                write_model(&[1, invalid], &mut stage, &mut out, &[Event::Written], 6),
                Observation {
                    result: Ok(Status::InvalidInput),
                    events: 0,
                    written: 0,
                    fuel: 0
                }
            );
            assert_eq!(out, [77; 2]);
        }
    }
    #[test]
    fn retries_are_paid_attempts() {
        let mut stage = [0];
        let mut out = [77];
        assert_eq!(
            write_model(
                &[12],
                &mut stage,
                &mut out,
                &[Event::Interrupted, Event::Interrupted, Event::Written],
                8
            ),
            Observation {
                result: Ok(Status::Complete),
                events: 3,
                written: 1,
                fuel: 0
            }
        );
        assert_eq!(out, [12]);
    }
    #[test]
    fn error_and_zero_preserve_prefix() {
        for terminal in [Event::Error, Event::Zero] {
            let mut stage = [0; 3];
            let mut out = [77; 3];
            assert_eq!(
                write_model(
                    &[1, 2, 3],
                    &mut stage,
                    &mut out,
                    &[Event::Written, terminal],
                    9
                ),
                Observation {
                    result: Ok(Status::IoError(1)),
                    events: 2,
                    written: 1,
                    fuel: 0
                }
            );
            assert_eq!(out, [1, 77, 77]);
        }
    }
    #[test]
    fn fuel_before_and_between_attempts() {
        for (fuel, written) in [(5, 0), (6, 0), (7, 1)] {
            let mut stage = [0; 2];
            let mut out = [77; 2];
            let r = write_model(&[1, 2], &mut stage, &mut out, &[Event::Written; 2], fuel);
            assert_eq!(r.result, Err(Failure::Fuel));
            assert_eq!(r.written, written);
            assert_eq!(r.events, written);
            assert_eq!(r.fuel, if fuel == 5 { 5 } else { 0 });
            assert_eq!(out, if written == 0 { [77, 77] } else { [1, 77] });
        }
    }
    #[test]
    fn admission_precedes_debit() {
        let input = [0; 1025];
        let mut stage = [0; 1024];
        let mut out = [77; 1024];
        let r = write_model(&input, &mut stage, &mut out, &[], 99);
        assert_eq!(r.result, Err(Failure::Capacity));
        assert_eq!(r.fuel, 99);
        let r = write_model(&[1], &mut [], &mut out, &[], 99);
        assert_eq!(r.result, Err(Failure::Staging));
        assert_eq!(r.fuel, 99);
    }
    #[test]
    fn maximum_capacity_exact_cost() {
        let input = [255; 1024];
        let mut stage = [0; 1024];
        let mut out = [0; 1024];
        assert_eq!(
            write_model(&input, &mut stage, &mut out, &[Event::Written; 1024], 2052),
            Observation {
                result: Ok(Status::Complete),
                events: 1024,
                written: 1024,
                fuel: 0
            }
        );
        assert_eq!(out, [255; 1024]);
    }
    #[test]
    fn missing_test_event_is_not_host_error() {
        let mut stage = [0];
        let mut out = [77];
        let r = write_model(&[1], &mut stage, &mut out, &[], 6);
        assert_eq!(r.result, Err(Failure::MissingTestEvent));
        assert_eq!(r.written, 0);
    }
    #[test]
    fn status_never_truncates() {
        assert_eq!(process_status(0), Ok(0));
        assert_eq!(process_status(255), Ok(255));
        assert_eq!(process_status(74), Ok(74));
        assert_eq!(process_status(-256), Err(()));
        assert_eq!(process_status(256), Err(()));
    }
}
