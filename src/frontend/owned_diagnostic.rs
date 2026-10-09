//! Bounded presentation for owned-source diagnostics.
//!
//! Callers pass `format_args!`, borrowed source text and `name()` wrappers, never
//! a preformatted source-sized `String`. These bounds cover retained message,
//! label and note bytes, not allocator overhead, rendering, paths or process RSS.

use super::{diagnostic::Diagnostic, source::Span};
use std::fmt::{self, Write};

pub(super) const MAX_MESSAGE_BYTES: usize = 1024;
pub(super) const MAX_LABEL_BYTES: usize = 256;
pub(super) const MAX_LABELS: usize = 2;
pub(super) const MAX_NOTE_BYTES: usize = 256;
pub(super) const MAX_NOTES: usize = 2;
pub(super) const MAX_NAME_BYTES: usize = 64;
pub(super) const MAX_MISSING_NAMES: usize = 8;
const SUFFIX: &str = "...";
const RESOURCE_MESSAGE: &str = "owned diagnostic storage exhausted";

#[derive(Clone, Copy)]
struct Limits {
    message: usize,
    label: usize,
    labels: usize,
    note: usize,
    notes: usize,
    name: usize,
    missing_names: usize,
}

impl Limits {
    const DEFAULT: Self = Self {
        message: MAX_MESSAGE_BYTES,
        label: MAX_LABEL_BYTES,
        labels: MAX_LABELS,
        note: MAX_NOTE_BYTES,
        notes: MAX_NOTES,
        name: MAX_NAME_BYTES,
        missing_names: MAX_MISSING_NAMES,
    };

    fn valid(self) -> bool {
        (SUFFIX.len()..=MAX_MESSAGE_BYTES).contains(&self.message)
            && (SUFFIX.len()..=MAX_LABEL_BYTES).contains(&self.label)
            && self.labels <= MAX_LABELS
            && (SUFFIX.len()..=MAX_NOTE_BYTES).contains(&self.note)
            && self.notes <= MAX_NOTES
            && (SUFFIX.len()..=MAX_NAME_BYTES).contains(&self.name)
            && self.missing_names <= MAX_MISSING_NAMES
    }
}

/// Construct an owned-route error without allocating a full formatted message.
/// Dynamic identifiers must be wrapped with `name()`; borrowed text, integers
/// and other allocation-free bounded formatters may be passed directly.
pub(super) fn diagnostic(
    code: &'static str,
    stage: &'static str,
    args: fmt::Arguments<'_>,
    span: Option<Span>,
) -> Box<Diagnostic> {
    diagnostic_with_limits(code, stage, args, span, Limits::DEFAULT)
}

fn diagnostic_with_limits(
    code: &'static str,
    stage: &'static str,
    args: fmt::Arguments<'_>,
    span: Option<Span>,
    limits: Limits,
) -> Box<Diagnostic> {
    if !limits.valid() {
        return resource(stage, span);
    }
    match component(args, limits.message) {
        Ok(message) => Box::new(Diagnostic {
            code,
            stage,
            message,
            primary: span,
            secondary: Vec::new(),
            notes: Vec::new(),
        }),
        Err(()) => resource(stage, span),
    }
}

/// Add a bounded label to an error made by this module. Excess labels are
/// ignored before reserving storage or invoking any argument formatter.
pub(super) fn secondary(
    error: Box<Diagnostic>,
    span: Span,
    args: fmt::Arguments<'_>,
) -> Box<Diagnostic> {
    secondary_with_limits(error, span, args, Limits::DEFAULT)
}

fn secondary_with_limits(
    mut error: Box<Diagnostic>,
    span: Span,
    args: fmt::Arguments<'_>,
    limits: Limits,
) -> Box<Diagnostic> {
    if !limits.valid() {
        return resource(error.stage, error.primary);
    }
    if is_resource(&error) || error.secondary.len() >= limits.labels {
        return error;
    }
    // The count check precedes both the vector reservation and text formatting.
    if reserve_one(&mut error.secondary).is_err() {
        return resource(error.stage, error.primary);
    }
    match component(args, limits.label) {
        Ok(message) => {
            error.secondary.push((span, message));
            error
        }
        Err(()) => resource(error.stage, error.primary),
    }
}

/// Add a bounded note to an error made by this module.
pub(super) fn note(error: Box<Diagnostic>, args: fmt::Arguments<'_>) -> Box<Diagnostic> {
    note_with_limits(error, args, Limits::DEFAULT)
}

fn note_with_limits(
    mut error: Box<Diagnostic>,
    args: fmt::Arguments<'_>,
    limits: Limits,
) -> Box<Diagnostic> {
    if !limits.valid() {
        return resource(error.stage, error.primary);
    }
    if is_resource(&error) || error.notes.len() >= limits.notes {
        return error;
    }
    if reserve_one(&mut error.notes).is_err() {
        return resource(error.stage, error.primary);
    }
    match component(args, limits.note) {
        Ok(message) => {
            error.notes.push(message);
            error
        }
        Err(()) => resource(error.stage, error.primary),
    }
}

/// A borrowed identifier with a byte ceiling that includes its explicit suffix.
pub(super) struct Name<'a> {
    text: &'a str,
    limit: usize,
}

pub(super) fn name(text: &str) -> Name<'_> {
    Name {
        text,
        limit: MAX_NAME_BYTES,
    }
}

impl fmt::Display for Name<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shortened = self.text.len() > self.limit;
        let end = if shortened {
            prefix_end(self.text, self.limit - SUFFIX.len())
        } else {
            self.text.len()
        };
        #[cfg(test)]
        record_name(end + if shortened { SUFFIX.len() } else { 0 });
        out.write_str(&self.text[..end])?;
        if shortened {
            out.write_str(SUFFIX)?;
        }
        Ok(())
    }
}

/// Select only borrowed missing names. The caller filters the declaration table
/// in declaration order; this traversal does not copy or format omitted names.
pub(super) fn missing_fields<'a>(
    code: &'static str,
    stage: &'static str,
    names: impl IntoIterator<Item = &'a str>,
    span: Option<Span>,
) -> Box<Diagnostic> {
    missing_fields_with_limits(code, stage, names, span, Limits::DEFAULT)
}

fn missing_fields_with_limits<'a>(
    code: &'static str,
    stage: &'static str,
    names: impl IntoIterator<Item = &'a str>,
    span: Option<Span>,
    limits: Limits,
) -> Box<Diagnostic> {
    if !limits.valid() {
        return resource(stage, span);
    }
    let mut first = [None; MAX_MISSING_NAMES];
    let mut count = 0_usize;
    for text in names {
        if count < limits.missing_names {
            first[count] = Some(text);
        }
        let Some(next) = count.checked_add(1) else {
            return resource(stage, span);
        };
        count = next;
    }
    let displayed = count.min(limits.missing_names);
    diagnostic_with_limits(
        code,
        stage,
        format_args!(
            "{}",
            MissingFields {
                first,
                displayed,
                omitted: count - displayed,
                name_limit: limits.name,
            }
        ),
        span,
        limits,
    )
}

struct MissingFields<'a> {
    first: [Option<&'a str>; MAX_MISSING_NAMES],
    displayed: usize,
    omitted: usize,
    name_limit: usize,
}

impl fmt::Display for MissingFields<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("missing fields: ")?;
        for (index, text) in self.first[..self.displayed].iter().enumerate() {
            if index != 0 {
                out.write_str(", ")?;
            }
            write!(
                out,
                "{}",
                Name {
                    text: text.expect("selected missing name"),
                    limit: self.name_limit,
                }
            )?;
        }
        if self.displayed == 0 && self.omitted == 0 {
            out.write_str("none")?;
        }
        if self.omitted != 0 {
            if self.displayed != 0 {
                out.write_str("; ")?;
            }
            write!(out, "{} more omitted", self.omitted)?;
        }
        Ok(())
    }
}

/// A reservation is bounded before formatting starts. The writer checks the
/// remaining bytes before every copy, and returns `fmt::Error` on truncation so
/// formatting does not continue through a long list or later arguments.
struct BoundedText {
    text: String,
    limit: usize,
    truncated: bool,
}

impl BoundedText {
    fn new(limit: usize) -> Result<Self, ()> {
        if !(SUFFIX.len()..=MAX_MESSAGE_BYTES).contains(&limit) {
            return Err(());
        }
        let mut text = String::new();
        allocation_test_point()?;
        text.try_reserve_exact(limit).map_err(|_| ())?;
        Ok(Self {
            text,
            limit,
            truncated: false,
        })
    }

    fn copy(&mut self, text: &str) {
        debug_assert!(text.len() <= self.limit - self.text.len());
        // No allocation: the entire inclusive component limit was reserved.
        self.text.push_str(text);
        #[cfg(test)]
        record_copy(text.len());
    }
}

impl Write for BoundedText {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
        }
        if text.len() <= self.limit - self.text.len() {
            self.copy(text);
            return Ok(());
        }
        let prefix_limit = self.limit - SUFFIX.len();
        if self.text.len() > prefix_limit {
            self.text.truncate(prefix_end(&self.text, prefix_limit));
        } else {
            let end = prefix_end(text, prefix_limit - self.text.len());
            self.copy(&text[..end]);
        }
        self.copy(SUFFIX);
        self.truncated = true;
        Err(fmt::Error)
    }
}

fn component(args: fmt::Arguments<'_>, limit: usize) -> Result<String, ()> {
    let mut out = BoundedText::new(limit)?;
    match fmt::write(&mut out, args) {
        Ok(()) => Ok(out.text),
        Err(_) if out.truncated => Ok(out.text),
        Err(_) => Err(()),
    }
}

fn prefix_end(text: &str, limit: usize) -> usize {
    let mut end = text.len().min(limit);
    // At most three decrements for valid UTF-8; never scan the omitted tail.
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    end
}

fn reserve_one<T>(values: &mut Vec<T>) -> Result<(), ()> {
    if values.len() == values.capacity() {
        allocation_test_point()?;
        values.try_reserve_exact(1).map_err(|_| ())?;
    }
    Ok(())
}

fn is_resource(error: &Diagnostic) -> bool {
    error.code == "E0400" && error.message == RESOURCE_MESSAGE
}

fn resource(stage: &'static str, span: Option<Span>) -> Box<Diagnostic> {
    // Diagnostic's existing Box/String representation needs this fixed-size
    // emergency allocation. New source-sized formatting/vector allocations are
    // fallible; recovery from total process allocator exhaustion is not claimed.
    Diagnostic::new("E0400", stage, RESOURCE_MESSAGE, span)
}

#[cfg(not(test))]
fn allocation_test_point() -> Result<(), ()> {
    Ok(())
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Metrics {
    pub(super) allocations: usize,
    pub(super) displayed_names: usize,
    pub(super) name_bytes: usize,
    pub(super) copied_bytes: usize,
}

#[cfg(test)]
thread_local! {
    static ALLOCATION_FAILURE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    static METRICS: std::cell::Cell<Metrics> = const { std::cell::Cell::new(Metrics {
        allocations: 0, displayed_names: 0, name_bytes: 0, copied_bytes: 0,
    }) };
}

/// Source integration can prove that its 101st error was never formatted.
/// This only measures new formatter reservations and copied text, not Box
/// metadata, fallback allocation, source/maps, renderers or process memory.
#[cfg(test)]
pub(super) fn measure_formatting<T>(operation: impl FnOnce() -> T) -> (T, Metrics) {
    struct Reset(Metrics);
    impl Drop for Reset {
        fn drop(&mut self) {
            METRICS.with(|stats| stats.set(self.0));
        }
    }
    let _reset = Reset(METRICS.with(|stats| stats.replace(Metrics::default())));
    let result = operation();
    (result, METRICS.with(std::cell::Cell::get))
}

#[cfg(test)]
fn allocation_test_point() -> Result<(), ()> {
    METRICS.with(|stats| {
        let mut next = stats.get();
        next.allocations += 1;
        stats.set(next);
    });
    ALLOCATION_FAILURE.with(|point| match point.get() {
        Some(0) => Err(()),
        Some(left) => {
            point.set(Some(left - 1));
            Ok(())
        }
        None => Ok(()),
    })
}

#[cfg(test)]
fn record_name(bytes: usize) {
    METRICS.with(|stats| {
        let mut next = stats.get();
        next.displayed_names += 1;
        next.name_bytes += bytes;
        stats.set(next);
    });
}

#[cfg(test)]
fn record_copy(bytes: usize) {
    METRICS.with(|stats| {
        let mut next = stats.get();
        next.copied_bytes += bytes;
        stats.set(next);
    });
}

#[cfg(test)]
pub(super) fn fail_allocation_after<T>(count: usize, operation: impl FnOnce() -> T) -> T {
    struct Reset(Option<usize>);
    impl Drop for Reset {
        fn drop(&mut self) {
            ALLOCATION_FAILURE.with(|point| point.set(self.0));
        }
    }
    let _reset = Reset(ALLOCATION_FAILURE.with(|point| point.replace(Some(count))));
    operation()
}

/// Repository-owned diagnostic roles used by the u8 reservation phase ledger.
/// No constructor behavior or allocation policy is changed.
pub(super) const fn u8_error_phase_layouts() -> [(&'static str, usize, usize); 7] {
    macro_rules! row {
        ($label:literal,$ty:ty) => {
            (
                $label,
                std::mem::size_of::<$ty>(),
                std::mem::align_of::<$ty>(),
            )
        };
    }
    [
        row!(
            "owned diagnostic arguments",
            (
                &'static str,
                &'static str,
                fmt::Arguments<'static>,
                Option<Span>,
                Box<Diagnostic>
            )
        ),
        row!(
            "diagnostic_with_limits arguments",
            (
                &'static str,
                &'static str,
                fmt::Arguments<'static>,
                Option<Span>,
                Limits,
                Box<Diagnostic>
            )
        ),
        row!(
            "component arguments, writer and return",
            (
                fmt::Arguments<'static>,
                usize,
                BoundedText,
                Result<String, ()>,
                fmt::Result
            )
        ),
        row!(
            "BoundedText::new reserve branch",
            (
                usize,
                String,
                Result<BoundedText, ()>,
                Result<(), std::collections::TryReserveError>,
                std::ops::RangeInclusive<usize>
            )
        ),
        row!(
            "bounded writer and copy requests",
            (
                &'static mut BoundedText,
                &'static str,
                &'static mut BoundedText,
                &'static str,
                usize,
                usize,
                fmt::Result
            )
        ),
        row!(
            "resource fallback arguments and result",
            (&'static str, Option<Span>, Box<Diagnostic>)
        ),
        row!(
            "Limits::valid copied value/range/borrow/result",
            (
                Limits,
                std::ops::RangeInclusive<usize>,
                &usize,
                &std::ops::RangeInclusive<usize>,
                bool
            )
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::source::{SourceFileId, SourceMap};
    use std::cell::Cell;

    fn span() -> Span {
        Span {
            file: SourceFileId(0),
            start: 0,
            end: 1,
        }
    }

    fn fresh() -> Box<Diagnostic> {
        diagnostic("E0300", "type", format_args!("a message"), Some(span()))
    }

    fn retained(error: &Diagnostic) -> usize {
        error.message.len()
            + error
                .secondary
                .iter()
                .map(|(_, text)| text.len())
                .sum::<usize>()
            + error.notes.iter().map(String::len).sum::<usize>()
    }

    fn reset_metrics() {
        METRICS.with(|stats| stats.set(Metrics::default()));
    }

    fn metrics() -> Metrics {
        METRICS.with(Cell::get)
    }

    struct Probe<'a>(&'a Cell<usize>);

    impl fmt::Display for Probe<'_> {
        fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.set(self.0.get() + 1);
            out.write_str("probe")
        }
    }

    #[test]
    fn owned_diagnostic_message_preserves_exact_limit_and_shortens_one_over() {
        let exact = "x".repeat(1024);
        let over = "x".repeat(1025);
        assert_eq!(
            diagnostic("E0300", "type", format_args!("{exact}"), None).message,
            exact
        );
        assert_eq!(
            diagnostic("E0300", "type", format_args!("{over}"), None).message,
            format!("{}...", "x".repeat(1021))
        );
    }

    #[test]
    fn owned_diagnostic_message_stops_before_formatting_later_arguments() {
        let large = "x".repeat(65_536);
        let calls = Cell::new(0);
        reset_metrics();
        let result = diagnostic(
            "E0300",
            "type",
            format_args!("{}{}", large, Probe(&calls)),
            None,
        );
        assert_eq!(result.message.len(), 1024);
        assert_eq!(calls.get(), 0);
        assert_eq!(metrics().copied_bytes, 1024);
        assert_eq!(metrics().allocations, 1);
    }

    #[test]
    fn owned_diagnostic_exact_and_one_over_components_at_lowered_limits() {
        let limits = Limits {
            message: 8,
            label: 5,
            note: 6,
            ..Limits::DEFAULT
        };
        let mut exact = diagnostic_with_limits(
            "E0300",
            "type",
            format_args!("12345678"),
            Some(span()),
            limits,
        );
        exact = secondary_with_limits(exact, span(), format_args!("12345"), limits);
        exact = note_with_limits(exact, format_args!("123456"), limits);
        assert_eq!(exact.message, "12345678");
        assert_eq!(exact.secondary[0].1, "12345");
        assert_eq!(exact.notes[0], "123456");

        let mut over = diagnostic_with_limits(
            "E0300",
            "type",
            format_args!("123456789"),
            Some(span()),
            limits,
        );
        over = secondary_with_limits(over, span(), format_args!("123456"), limits);
        over = note_with_limits(over, format_args!("1234567"), limits);
        assert_eq!(over.message, "12345...");
        assert_eq!(over.secondary[0].1, "12...");
        assert_eq!(over.notes[0], "123...");
        assert_eq!(over.primary, Some(span()));
        assert_eq!(over.secondary[0].0, span());
    }

    #[test]
    fn owned_diagnostic_default_label_and_note_lengths_are_inclusive() {
        let exact = "x".repeat(256);
        let over = "x".repeat(257);
        let mut error = secondary(fresh(), span(), format_args!("{exact}"));
        error = secondary(error, span(), format_args!("{over}"));
        error = note(error, format_args!("{exact}"));
        error = note(error, format_args!("{over}"));
        assert_eq!(error.secondary[0].1, exact);
        assert_eq!(error.secondary[1].1, format!("{}...", "x".repeat(253)));
        assert_eq!(error.notes[0], error.secondary[0].1);
        assert_eq!(error.notes[1], error.secondary[1].1);
    }

    #[test]
    fn owned_diagnostic_component_counts_stop_before_formatting_or_reserving() {
        let calls = Cell::new(0);
        let mut error = fresh();
        for _ in 0..2 {
            error = secondary(error, span(), format_args!("label"));
            error = note(error, format_args!("note"));
        }
        reset_metrics();
        error = secondary(error, span(), format_args!("{}", Probe(&calls)));
        error = note(error, format_args!("{}", Probe(&calls)));
        assert_eq!(error.secondary.len(), 2);
        assert_eq!(error.notes.len(), 2);
        assert_eq!(calls.get(), 0);
        assert_eq!(metrics().allocations, 0);
        assert_eq!(metrics().copied_bytes, 0);
    }

    #[test]
    fn owned_diagnostic_lowered_zero_and_one_component_counts() {
        for limit in [0, 1] {
            let limits = Limits {
                labels: limit,
                notes: limit,
                ..Limits::DEFAULT
            };
            let mut error = fresh();
            for _ in 0..limit {
                error = secondary_with_limits(error, span(), format_args!("label"), limits);
                error = note_with_limits(error, format_args!("note"), limits);
            }
            let calls = Cell::new(0);
            reset_metrics();
            error = secondary_with_limits(error, span(), format_args!("{}", Probe(&calls)), limits);
            error = note_with_limits(error, format_args!("{}", Probe(&calls)), limits);
            assert_eq!(error.secondary.len(), limit);
            assert_eq!(error.notes.len(), limit);
            assert_eq!(calls.get(), 0);
            assert_eq!(metrics().allocations, 0);
        }
    }

    #[test]
    fn owned_diagnostic_retained_text_reaches_exactly_2048_bytes() {
        let message = "m".repeat(1024);
        let label = "l".repeat(256);
        let note_text = "n".repeat(256);
        let mut error = diagnostic("E0300", "type", format_args!("{message}"), Some(span()));
        for _ in 0..2 {
            error = secondary(error, span(), format_args!("{label}"));
            error = note(error, format_args!("{note_text}"));
        }
        assert_eq!(retained(&error), 2048);
        let calls = Cell::new(0);
        error = secondary(error, span(), format_args!("{}", Probe(&calls)));
        error = note(error, format_args!("{}", Probe(&calls)));
        assert_eq!(retained(&error), 2048);
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn owned_diagnostic_name_64_bytes_unchanged_and_65_shortened() {
        let exact = "x".repeat(64);
        let over = "x".repeat(65);
        assert_eq!(
            component(format_args!("{}", name(&exact)), 1024).unwrap(),
            exact
        );
        assert_eq!(
            component(format_args!("{}", name(&over)), 1024).unwrap(),
            format!("{}...", "x".repeat(61))
        );
        let exact_unicode = format!("{}é", "x".repeat(62));
        assert_eq!(
            component(format_args!("{}", name(&exact_unicode)), 1024).unwrap(),
            exact_unicode
        );
    }

    #[test]
    fn owned_diagnostic_name_truncation_preserves_multibyte_boundaries() {
        for letter in ["é", "雪", "🦀"] {
            let input = format!("{}{}", "x".repeat(60), letter.repeat(4));
            let actual = component(format_args!("{}", name(&input)), 1024).unwrap();
            assert_eq!(actual, format!("{}...", "x".repeat(60)));
            assert_eq!(actual.len(), 63);
        }
    }

    #[test]
    fn owned_diagnostic_component_truncation_preserves_multibyte_boundaries() {
        for letter in ["é", "雪", "🦀"] {
            let tail = letter.repeat(5);
            let whole = format!("xxx{tail}");
            assert_eq!(component(format_args!("{whole}"), 7).unwrap(), "xxx...");
            assert_eq!(component(format_args!("xxx{tail}"), 7).unwrap(), "xxx...");
            // A previously written complete code point may need removing when
            // a later fragment establishes that the whole component is long.
            let full = format!("xxx{letter}");
            assert_eq!(component(format_args!("{full}zzzzz"), 7).unwrap(), "xxx...");
        }
    }

    #[test]
    fn owned_diagnostic_lowered_name_limits_and_minimum_suffix() {
        for limit in [3, 4, 5, 7, 64] {
            let exact = "x".repeat(limit);
            let over = "x".repeat(limit + 1);
            let exact_name = Name {
                text: &exact,
                limit,
            };
            let over_name = Name { text: &over, limit };
            assert_eq!(
                component(format_args!("{exact_name}"), 1024).unwrap(),
                exact
            );
            assert_eq!(
                component(format_args!("{over_name}"), 1024).unwrap(),
                format!("{}...", "x".repeat(limit - 3))
            );
        }
        assert_eq!(component(format_args!("abc"), 3).unwrap(), "abc");
        assert_eq!(component(format_args!("abcd"), 3).unwrap(), "...");
    }

    #[test]
    fn owned_diagnostic_missing_fields_zero_eight_and_nine_in_declaration_order() {
        let names = ["z", "a", "y", "b", "x", "c", "w", "d", "omitted"];
        assert_eq!(
            missing_fields("E0300", "type", [], Some(span())).message,
            "missing fields: none"
        );
        let eight = missing_fields("E0300", "type", names[..8].iter().copied(), Some(span()));
        assert_eq!(eight.message, "missing fields: z, a, y, b, x, c, w, d");
        let nine = missing_fields("E0300", "type", names, Some(span()));
        assert_eq!(
            nine.message,
            "missing fields: z, a, y, b, x, c, w, d; 1 more omitted"
        );
        assert_eq!(nine.primary, Some(span()));
        assert_eq!(nine.code, "E0300");
        assert_eq!(nine.stage, "type");
    }

    #[test]
    fn owned_diagnostic_missing_fields_lowered_name_and_list_limits() {
        let limits = Limits {
            name: 5,
            missing_names: 2,
            ..Limits::DEFAULT
        };
        let error = missing_fields_with_limits(
            "E0300",
            "type",
            ["abcdef", "ghijkl", "mnopqr"],
            None,
            limits,
        );
        assert_eq!(
            error.message,
            "missing fields: ab..., gh...; 1 more omitted"
        );
        let no_names = Limits {
            missing_names: 0,
            ..limits
        };
        let error = missing_fields_with_limits(
            "E0300",
            "type",
            ["abcdef", "ghijkl", "mnopqr"],
            None,
            no_names,
        );
        assert_eq!(error.message, "missing fields: 3 more omitted");
    }

    #[test]
    fn owned_diagnostic_missing_count_fits_even_with_largest_usize() {
        let long = "x".repeat(900);
        let message = MissingFields {
            first: [Some(long.as_str()); 8],
            displayed: 8,
            omitted: usize::MAX,
            name_limit: 64,
        };
        let actual = diagnostic("E0300", "type", format_args!("{message}"), None);
        assert!(actual.message.len() < 1024);
        assert!(actual
            .message
            .ends_with(&format!("; {} more omitted", usize::MAX)));
        assert_eq!(actual.message.matches("...").count(), 8);
    }

    #[test]
    fn owned_diagnostic_large_schema_fixture_copies_only_eight_bounded_prefixes() {
        // Reproduce the independent design review's 932,069-byte counterexample
        // as valid source text, then borrow its declared names for this formatter
        // test. Parsing/checking this fixture belongs to source integration.
        let mut source = String::from("struct T {\n");
        for index in 0..1024 {
            if index != 0 {
                source.push_str(",\n");
            }
            write!(source, "f{index:04}_{}: i32", "x".repeat(894)).unwrap();
        }
        source.push_str("\n}\n");
        for index in 0..100 {
            if index != 0 {
                source.push('\n');
            }
            write!(source, "fn f{index}() -> () {{ T {{}}; return; }}").unwrap();
        }
        assert_eq!(source.len(), 932_069);
        let schema_end = source.find("\n}\n").unwrap();
        let fields = &source["struct T {\n".len()..schema_end];
        let visited = Cell::new(0);
        let (errors, stats) = measure_formatting(|| {
            let mut errors = Vec::new();
            for _ in source[schema_end + 3..].lines() {
                let names = fields.lines().map(|line| {
                    visited.set(visited.get() + 1);
                    line.split_once(':').unwrap().0
                });
                errors.push(missing_fields("E0300", "type", names, None));
            }
            errors
        });
        assert_eq!(errors.len(), 100);
        assert_eq!(visited.get(), 1024 * 100);
        for error in &errors {
            assert_eq!(error.message.len(), 561);
            assert!(error.message.starts_with("missing fields: f0000_"));
            assert!(error.message.contains("f0007_"));
            assert!(!error.message.contains("f0008_"));
            assert!(error.message.ends_with("; 1016 more omitted"));
        }
        assert_eq!(
            errors.iter().map(|error| retained(error)).sum::<usize>(),
            56_100
        );
        assert_eq!(stats.displayed_names, 8 * 100);
        assert_eq!(stats.name_bytes, 64 * 8 * 100);
        assert_eq!(stats.copied_bytes, 56_100);
        assert_eq!(stats.allocations, 100);
    }

    #[test]
    fn owned_diagnostic_all_new_allocation_seams_return_resource_error() {
        for count in 0..=9 {
            let error = fail_allocation_after(count, || {
                let mut error = fresh();
                for _ in 0..2 {
                    error = secondary(error, span(), format_args!("label"));
                    error = note(error, format_args!("note"));
                }
                error
            });
            if count < 9 {
                assert_eq!(error.code, "E0400", "failed reservation {count}");
                assert_eq!(error.stage, "type");
                assert_eq!(error.message, RESOURCE_MESSAGE);
                assert_eq!(error.primary, Some(span()));
                assert!(error.secondary.is_empty());
                assert!(error.notes.is_empty());
            } else {
                assert_eq!(error.code, "E0300");
                assert_eq!(error.secondary.len(), 2);
                assert_eq!(error.notes.len(), 2);
            }
        }
    }

    #[test]
    fn owned_diagnostic_failed_reservation_does_not_invoke_formatter() {
        let calls = Cell::new(0);
        let error = fail_allocation_after(0, || {
            diagnostic("E0300", "type", format_args!("{}", Probe(&calls)), None)
        });
        assert_eq!(error.code, "E0400");
        assert_eq!(calls.get(), 0);
        for before in [0, 1] {
            let error = fresh();
            let error = fail_allocation_after(before, || {
                secondary(error, span(), format_args!("{}", Probe(&calls)))
            });
            assert_eq!(error.code, "E0400");
            assert_eq!(calls.get(), 0);
            let error = fresh();
            let error =
                fail_allocation_after(before, || note(error, format_args!("{}", Probe(&calls))));
            assert_eq!(error.code, "E0400");
            assert_eq!(calls.get(), 0);
        }
    }

    #[test]
    fn owned_diagnostic_genuine_formatter_failure_is_not_truncation() {
        struct Broken;
        impl fmt::Display for Broken {
            fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                out.write_str("partial")?;
                Err(fmt::Error)
            }
        }
        let error = diagnostic("E0300", "type", format_args!("{Broken}"), None);
        assert_eq!(error.code, "E0400");
        assert_eq!(error.message, RESOURCE_MESSAGE);
    }

    #[test]
    fn owned_diagnostic_private_limits_cannot_raise_defaults_or_lose_suffix_space() {
        let invalid = [
            Limits {
                message: 1025,
                ..Limits::DEFAULT
            },
            Limits {
                label: 257,
                ..Limits::DEFAULT
            },
            Limits {
                labels: 3,
                ..Limits::DEFAULT
            },
            Limits {
                note: 257,
                ..Limits::DEFAULT
            },
            Limits {
                notes: 3,
                ..Limits::DEFAULT
            },
            Limits {
                name: 65,
                ..Limits::DEFAULT
            },
            Limits {
                missing_names: 9,
                ..Limits::DEFAULT
            },
            Limits {
                message: 2,
                ..Limits::DEFAULT
            },
            Limits {
                label: 2,
                ..Limits::DEFAULT
            },
            Limits {
                note: 2,
                ..Limits::DEFAULT
            },
            Limits {
                name: 2,
                ..Limits::DEFAULT
            },
        ];
        let calls = Cell::new(0);
        reset_metrics();
        for limits in invalid {
            let error = diagnostic_with_limits(
                "E0300",
                "type",
                format_args!("{}", Probe(&calls)),
                None,
                limits,
            );
            assert_eq!(error.code, "E0400");
        }
        assert_eq!(calls.get(), 0);
        assert_eq!(metrics().allocations, 0);
    }

    #[test]
    fn owned_diagnostic_rendering_escapes_bounded_text_with_separate_metadata() {
        let mut sources = SourceMap::new();
        sources.add("fixed.ox".into(), "x".into());
        let message = "\u{1b}".repeat(1024);
        let extra = "\u{1b}".repeat(256);
        let mut full = diagnostic("E0300", "type", format_args!("{message}"), Some(span()));
        let mut empty = diagnostic("E0300", "type", format_args!(""), Some(span()));
        for _ in 0..2 {
            full = secondary(full, span(), format_args!("{extra}"));
            full = note(full, format_args!("{extra}"));
            empty = secondary(empty, span(), format_args!(""));
            empty = note(empty, format_args!(""));
        }
        assert_eq!(retained(&full), 2048);
        let json = full.render_json(&sources);
        let human = full.render_human(&sources);
        assert_eq!(json.len() - empty.render_json(&sources).len(), 6 * 2048);
        assert_eq!(human.len() - empty.render_human(&sources).len(), 6 * 2048);
        assert!(!json.contains('\u{1b}'));
        assert!(!human.contains('\u{1b}'));
        assert!(json.contains("\\u001b"));
        assert!(human.contains("\\u{1b}"));
    }

    #[test]
    fn owned_diagnostic_long_escaped_paths_are_outside_retained_text_bound() {
        let mut sources = SourceMap::new();
        sources.add("\u{1b}".repeat(5000), "x".into());
        let error = fresh();
        assert_eq!(retained(&error), 9);
        assert!(error.render_json(&sources).len() > 30_000);
        assert!(error.render_human(&sources).len() > 30_000);
        assert_eq!(retained(&error), 9);
    }

    #[test]
    fn owned_diagnostic_scalar_constructor_and_rendering_stay_unchanged() {
        let long = "x".repeat(4096);
        let old = Diagnostic::new("E0300", "type", long.clone(), None);
        assert_eq!(old.message, long);
        let old = Diagnostic::new("E0200", "resolve", "unknown name `old`", None);
        assert_eq!(
            old.render_human(&SourceMap::new()),
            "error[E0200] (resolve): unknown name `old`\n"
        );
        assert_eq!(
            old.render_json(&SourceMap::new()),
            concat!(
            "{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"diagnostic\",",
            "\"severity\":\"error\",\"code\":\"E0200\",\"stage\":\"resolve\",",
            "\"message\":\"unknown name `old`\",\"primary\":null,\"secondary\":[],\"notes\":[]}"
        )
        );
    }
}
