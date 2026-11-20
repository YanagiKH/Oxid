use super::Options;
use std::{
    num::NonZeroUsize,
    path::Path,
    process::{Command, Output, Stdio},
    thread,
};

pub(super) struct TestRun<'a> {
    pub root: &'a Path,
    pub member: Option<&'a str>,
    pub name: &'a str,
}

pub(super) fn run(
    tests: &[TestRun<'_>],
    options: &Options,
    has_lock: bool,
    mut report: impl FnMut(&TestRun<'_>, Output) -> Result<(), String>,
) -> Result<usize, String> {
    let jobs = options.test_parallelism()?;
    let executable =
        std::env::current_exe().map_err(|error| format!("cannot locate test runner: {error}"))?;
    let mut passed = 0;
    run_batches(
        tests,
        jobs,
        |test| {
            let mut child = Command::new(&executable);
            child
                .current_dir(test.root)
                .args(["run", "--edition=typed-preview", "--entry-mode=process"])
                .arg(format!("--project-test={}", test.name))
                .stdin(Stdio::null());
            if options.frozen {
                child.arg("--frozen");
            } else {
                if has_lock || options.locked {
                    child.arg("--locked");
                }
                if options.offline {
                    child.arg("--offline");
                }
            }
            child.output().map_err(|error| match test.member {
                Some(member) => format!("cannot run test {member}::{}: {error}", test.name),
                None => format!("cannot run test {:?}: {error}", test.name),
            })
        },
        |test, result| {
            passed += usize::from(result.status.success());
            report(test, result)
        },
    )?;
    Ok(passed)
}

fn run_batches<T: Sync, R: Send>(
    tests: &[T],
    jobs: NonZeroUsize,
    execute: impl Fn(&T) -> Result<R, String> + Sync,
    mut report: impl FnMut(&T, R) -> Result<(), String>,
) -> Result<(), String> {
    for batch in tests.chunks(jobs.get()) {
        if let [test] = batch {
            report(test, execute(test)?)?;
            continue;
        }
        // Join the whole batch before reporting, including when a test could
        // not start. At most one batch of captured output is retained.
        let execute = &execute;
        let results = thread::scope(|scope| {
            let workers: Vec<_> = batch
                .iter()
                .map(|test| thread::Builder::new().spawn_scoped(scope, move || execute(test)))
                .collect();
            workers
                .into_iter()
                .map(|worker| {
                    let worker =
                        worker.map_err(|error| format!("cannot start test worker: {error}"))?;
                    worker
                        .join()
                        .map_err(|_| "test worker panicked".to_owned())?
                })
                .collect::<Vec<_>>()
        });
        for (test, result) in batch.iter().zip(results) {
            report(test, result?)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Condvar, Mutex},
        time::Duration,
    };

    #[test]
    fn two_jobs_overlap_and_report_in_declaration_order() {
        let state = (Mutex::new(0), Condvar::new());
        let mut reported = Vec::new();
        run_batches(
            &[1usize, 2, 3, 4],
            NonZeroUsize::new(2).unwrap(),
            |test| {
                let mut count = state.0.lock().unwrap();
                *count += 1;
                state.1.notify_all();
                let end_of_batch = 2 * test.div_ceil(2);
                let (_guard, time) = state
                    .1
                    .wait_timeout_while(count, Duration::from_secs(10), |started| {
                        *started < end_of_batch
                    })
                    .unwrap();
                if time.timed_out() {
                    return Err("two tests did not overlap".to_owned());
                }
                Ok(*test)
            },
            |test, output| {
                assert_eq!(*test, output);
                reported.push(output);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(reported, [1, 2, 3, 4]);
    }

    #[test]
    fn worker_error_joins_other_started_tests_and_stops_the_next_batch() {
        let executed = Mutex::new(Vec::new());
        let mut reported = Vec::new();
        let result = run_batches(
            &[1, 2, 3],
            NonZeroUsize::new(2).unwrap(),
            |test| {
                executed.lock().unwrap().push(*test);
                if *test == 2 {
                    return Err("could not start test 2".to_owned());
                }
                Ok(*test)
            },
            |_, output| {
                reported.push(output);
                Ok(())
            },
        );
        assert_eq!(result.unwrap_err(), "could not start test 2");
        assert_eq!(reported, [1]);
        let mut attempted = executed.into_inner().unwrap();
        attempted.sort_unstable();
        assert_eq!(attempted, [1, 2]);
    }
}
