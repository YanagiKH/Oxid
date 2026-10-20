//! Sequential explicit-workspace CHECK and interpreter TEST coordinator.
//!
//! The bounded manifest catalog only enumerates validated member names. Each
//! member then gets a fresh ordinary selected-package request and the existing
//! CHECK or interpreter TEST operation, one at a time, in declared order. This
//! is not whole-workspace preparation, an atomic snapshot, a pooled fuel or
//! resource budget, parallel execution or a native/receipt path.
use super::{
    diagnostic::Diagnostic,
    manifest::{self, graph::WorkspaceMembers, Command, Failure, Request},
    options::BuildProfile,
    source::SourceMap,
};
use std::{
    fmt,
    io::{self, Write},
};

/// Render a manifest/CLI diagnostic to stderr with its original classification.
pub(super) fn cli_error(failure: Failure) -> i32 {
    let (code, stage) = failure.classification();
    let diagnostic = Diagnostic::new(code, stage, failure.message(), None);
    let mut stderr = io::stderr().lock();
    let _ = write!(stderr, "{}", diagnostic.render_human(&SourceMap::new()));
    let _ = stderr.flush();
    1
}

pub(super) fn run(request: Box<[Request; 1]>, test: bool, fail_fast: bool) -> i32 {
    let operation = if test { Command::Test } else { Command::Check };
    // Catalog failures happen before any heading, summary or member operation.
    match manifest::graph::with_workspace_members(request, operation, |path, members| {
        coordinate(path, &members, fail_fast, &mut Stdout, |path, name| {
            member(path, name, test, fail_fast)
        })
    }) {
        Ok(status) => status,
        Err(failure) => cli_error(failure),
    }
}

/// One ordinary selected-package operation. The request and the existing
/// runner's whole session are owned by this call and dropped before it returns.
fn member(path: &str, name: &str, test: bool, fail_fast: bool) -> i32 {
    match manifest::format_request(path, Some(name)) {
        Ok(request) if test => {
            super::project_test::run(request, false, None, fail_fast, BuildProfile::Dev)
        }
        Ok(request) => super::driver::check_manifest(request),
        Err(failure) => cli_error(failure),
    }
}

/// Name/length view of the validated catalog; a seam for coordinator units.
trait Members {
    fn len(&self) -> usize;
    fn name(&self, index: usize) -> Option<&str>;
}

impl Members for WorkspaceMembers<'_> {
    fn len(&self) -> usize {
        WorkspaceMembers::len(self)
    }

    fn name(&self, index: usize) -> Option<&str> {
        WorkspaceMembers::name(self, index)
    }
}

/// Batch heading/summary output. Each line is written and flushed on its own;
/// no stdout lock is retained while an existing runner produces its output.
trait Lines {
    fn line(&mut self, text: fmt::Arguments<'_>) -> io::Result<()>;
}

struct Stdout;

impl Lines for Stdout {
    fn line(&mut self, text: fmt::Arguments<'_>) -> io::Result<()> {
        emit(&mut io::stdout().lock(), text)
    }
}

fn emit(out: &mut impl Write, text: fmt::Arguments<'_>) -> io::Result<()> {
    out.write_fmt(text)?;
    out.flush()
}

fn coordinate(
    path: &str,
    members: &impl Members,
    fail_fast: bool,
    out: &mut impl Lines,
    mut operation: impl FnMut(&str, &str) -> i32,
) -> i32 {
    let total = members.len();
    let mut passed = 0usize;
    let mut failed = 0usize;
    for index in 0..total {
        // The catalog validated every member; a missing name is never skipped.
        let Some(name) = members.name(index) else {
            return cli_error(Failure::Invariant);
        };
        // A broken heading prevents this member and every later member.
        if out
            .line(format_args!("workspace package: {name}\n"))
            .is_err()
        {
            return 1;
        }
        if operation(path, name) == 0 {
            passed += 1;
        } else {
            failed += 1;
            if fail_fast {
                break;
            }
        }
    }
    let unrun = total - passed - failed;
    if out
        .line(format_args!(
            "workspace result: {passed} passed; {failed} failed; {unrun} unrun; {total} total packages\n"
        ))
        .is_err()
    {
        return 1;
    }
    i32::from(passed != total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Line(String),
        Start(String),
        End(String),
    }

    struct Catalog<'a>(&'a [Option<&'a str>]);

    impl Members for Catalog<'_> {
        fn len(&self) -> usize {
            self.0.len()
        }

        fn name(&self, index: usize) -> Option<&str> {
            self.0.get(index).copied().flatten()
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Fault {
        Write,
        Flush,
    }

    /// A bounded writer that can fail the write or the flush of one line.
    struct Sink<'a> {
        events: &'a RefCell<Vec<Event>>,
        fault: Option<(usize, Fault)>,
        lines: usize,
        pending: Vec<u8>,
    }

    impl Write for Sink<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fault == Some((self.lines, Fault::Write)) {
                return Err(io::Error::other("injected heading/summary write failure"));
            }
            self.pending.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fault == Some((self.lines, Fault::Flush)) {
                return Err(io::Error::other("injected heading/summary flush failure"));
            }
            let text = String::from_utf8(std::mem::take(&mut self.pending)).unwrap();
            self.events.borrow_mut().push(Event::Line(text));
            Ok(())
        }
    }

    impl Lines for Sink<'_> {
        fn line(&mut self, text: fmt::Arguments<'_>) -> io::Result<()> {
            let result = emit(self, text);
            self.lines += 1;
            result
        }
    }

    struct Outcome {
        status: i32,
        events: Vec<Event>,
        attempts: usize,
    }

    /// Drive the coordinator with hand-written member statuses. The operation
    /// records start/end and proves that only one member call is ever live.
    fn drive(
        names: &[Option<&str>],
        failing: &[&str],
        fail_fast: bool,
        fault: Option<(usize, Fault)>,
    ) -> Outcome {
        let events = RefCell::new(Vec::new());
        let live = Cell::new(0usize);
        let mut sink = Sink {
            events: &events,
            fault,
            lines: 0,
            pending: Vec::new(),
        };
        let status = coordinate(
            "workspace/oxid.toml",
            &Catalog(names),
            fail_fast,
            &mut sink,
            |path, name| {
                assert_eq!(path, "workspace/oxid.toml");
                assert_eq!(live.replace(live.get() + 1), 0, "overlapping members");
                // The heading for this member was written and flushed first.
                assert_eq!(
                    events.borrow().last(),
                    Some(&Event::Line(format!("workspace package: {name}\n")))
                );
                events.borrow_mut().push(Event::Start(name.into()));
                events.borrow_mut().push(Event::End(name.into()));
                live.set(live.get() - 1);
                i32::from(failing.contains(&name))
            },
        );
        assert_eq!(live.get(), 0);
        // Every attempted heading/summary line, including a failed one.
        let attempts = sink.lines;
        Outcome {
            status,
            events: events.take(),
            attempts,
        }
    }

    fn heading(name: &str) -> Event {
        Event::Line(format!("workspace package: {name}\n"))
    }

    fn member_events(name: &str) -> [Event; 3] {
        [
            heading(name),
            Event::Start(name.into()),
            Event::End(name.into()),
        ]
    }

    fn summary(passed: usize, failed: usize, unrun: usize, total: usize) -> Event {
        assert_eq!(passed + failed + unrun, total);
        Event::Line(format!(
            "workspace result: {passed} passed; {failed} failed; {unrun} unrun; {total} total packages\n"
        ))
    }

    fn started(events: &[Event]) -> Vec<&str> {
        events
            .iter()
            .filter_map(|event| match event {
                Event::Start(name) => Some(name.as_str()),
                _ => None,
            })
            .collect()
    }

    const THREE: [Option<&str>; 3] = [Some("first"), Some("second"), Some("third")];

    #[test]
    fn all_members_run_once_in_declared_order_and_pass() {
        let outcome = drive(&THREE, &[], false, None);
        assert_eq!(outcome.status, 0);
        let mut expected = Vec::new();
        for name in ["first", "second", "third"] {
            expected.extend(member_events(name));
        }
        expected.push(summary(3, 0, 0, 3));
        assert_eq!(outcome.events, expected);
        assert_eq!(outcome.attempts, 4);
        let single = drive(&[Some("only")], &[], true, None);
        assert_eq!(single.status, 0);
        assert_eq!(single.events.last(), Some(&summary(1, 0, 0, 1)));
    }

    #[test]
    fn default_mode_continues_after_failed_members() {
        let outcome = drive(&THREE, &["first", "second"], false, None);
        assert_eq!(outcome.status, 1);
        assert_eq!(started(&outcome.events), ["first", "second", "third"]);
        assert_eq!(outcome.events.last(), Some(&summary(1, 2, 0, 3)));
        let last = drive(&THREE, &["third"], false, None);
        assert_eq!(last.status, 1);
        assert_eq!(last.events.last(), Some(&summary(2, 1, 0, 3)));
    }

    #[test]
    fn fail_fast_stops_after_the_first_failed_member() {
        for (failing, ran, expected) in [
            (&["first"][..], &["first"][..], summary(0, 1, 2, 3)),
            (
                &["second", "third"][..],
                &["first", "second"][..],
                summary(1, 1, 1, 3),
            ),
            (
                &["third"][..],
                &["first", "second", "third"][..],
                summary(2, 1, 0, 3),
            ),
        ] {
            let outcome = drive(&THREE, failing, true, None);
            assert_eq!(outcome.status, 1);
            assert_eq!(started(&outcome.events), ran);
            assert_eq!(outcome.events.last(), Some(&expected));
            if let Some(next) = THREE.get(ran.len()) {
                assert!(!outcome.events.contains(&heading(next.unwrap())));
            }
        }
        // Passing members are not affected by fail-fast.
        let outcome = drive(&THREE, &[], true, None);
        assert_eq!(outcome.status, 0);
        assert_eq!(outcome.events.last(), Some(&summary(3, 0, 0, 3)));
    }

    #[test]
    fn heading_write_or_flush_failure_prevents_that_and_every_later_member() {
        for fault in [Fault::Write, Fault::Flush] {
            for line in 0..THREE.len() {
                let outcome = drive(&THREE, &[], false, Some((line, fault)));
                assert_eq!(outcome.status, 1);
                let names: Vec<_> = THREE[..line].iter().map(|name| name.unwrap()).collect();
                assert_eq!(started(&outcome.events), names);
                let mut expected = Vec::new();
                for name in &names {
                    expected.extend(member_events(name));
                }
                // No summary is attempted after the broken heading.
                assert_eq!(outcome.events, expected);
                assert_eq!(outcome.attempts, line + 1);
            }
        }
    }

    #[test]
    fn summary_write_or_flush_failure_never_returns_success() {
        for fault in [Fault::Write, Fault::Flush] {
            for (failing, fail_fast) in
                [(&[][..], false), (&[][..], true), (&["second"][..], false)]
            {
                let outcome = drive(&THREE, failing, fail_fast, Some((3, fault)));
                assert_eq!(outcome.status, 1);
                assert_eq!(started(&outcome.events), ["first", "second", "third"]);
                assert!(!outcome
                    .events
                    .iter()
                    .any(|event| matches!(event, Event::Line(text) if text.starts_with("workspace result:"))));
                assert_eq!(outcome.attempts, 4);
            }
            let stopped = drive(&THREE, &["first"], true, Some((1, fault)));
            assert_eq!(stopped.status, 1);
            assert_eq!(started(&stopped.events), ["first"]);
            assert_eq!(stopped.attempts, 2);
        }
    }

    #[test]
    fn an_impossible_missing_name_is_an_invariant_failure_not_a_skip() {
        let outcome = drive(&[Some("first"), None, Some("third")], &[], false, None);
        assert_eq!(outcome.status, 1);
        assert_eq!(outcome.events, member_events("first"));
        assert_eq!(outcome.attempts, 1);
        let leading = drive(&[None, Some("second")], &[], true, None);
        assert_eq!(leading.status, 1);
        assert!(leading.events.is_empty());
        assert_eq!(leading.attempts, 0);
    }

    #[test]
    fn package_counts_always_partition_the_declared_members() {
        let names: Vec<String> = (0..6).map(|index| format!("member-{index}")).collect();
        let catalog: Vec<Option<&str>> = names.iter().map(|name| Some(name.as_str())).collect();
        for mask in 0u32..(1 << names.len()) {
            let failing: Vec<&str> = names
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, name)| name.as_str())
                .collect();
            for fail_fast in [false, true] {
                let outcome = drive(&catalog, &failing, fail_fast, None);
                let ran = started(&outcome.events).len();
                let failed = started(&outcome.events)
                    .iter()
                    .filter(|name| failing.contains(*name))
                    .count();
                let expected_ran = match (fail_fast, mask.trailing_zeros() as usize) {
                    (true, first) if first < names.len() => first + 1,
                    _ => names.len(),
                };
                assert_eq!(ran, expected_ran);
                assert_eq!(
                    outcome.events.last(),
                    Some(&summary(
                        ran - failed,
                        failed,
                        names.len() - ran,
                        names.len()
                    ))
                );
                assert_eq!(outcome.status, i32::from(mask != 0));
            }
        }
    }
}
