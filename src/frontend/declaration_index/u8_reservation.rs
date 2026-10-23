//! Post-graph primitive-name reservation. Core errors are data; diagnostics are
//! constructed only after the complete core call frame has returned.
use super::*;
use resource::DebitFailure;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reason {
    BadIdentity,
    CountOverflow,
    WorkLimit,
    ReservedU8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Failure {
    reason: Reason,
    at: Span,
}
impl Failure {
    fn bad(at: &Span) -> Self {
        Self {
            reason: Reason::BadIdentity,
            at: *at,
        }
    }
    fn diagnostic(self) -> Box<Diagnostic> {
        match self.reason {
            Reason::BadIdentity=>diagnostic("E0500","resolve-project","invalid declaration index source or identity",self.at),
            Reason::CountOverflow=>diagnostic("E0400","resolve-project","declaration index count overflow",self.at),
            Reason::WorkLimit=>diagnostic("E0400","resolve-project","declaration index work limit exceeded",self.at),
            Reason::ReservedU8=>diagnostic("E0208","resolve","type name u8 is reserved for the unsigned-byte primitive; rename the type or import alias",self.at),
        }
    }
}
fn debit(work: &WorkMeter, at: &Span, operation: &'static str) -> Result<(), Failure> {
    work.try_debit_compact(1, at, operation)
        .map_err(|reason| Failure {
            reason: match reason {
                DebitFailure::Overflow => Reason::CountOverflow,
                DebitFailure::Limit => Reason::WorkLimit,
            },
            at: *at,
        })
}
fn origin<'s>(program: &'s ast::Program, item: &ast::ItemId) -> Option<&'s Span> {
    match item {
        ast::ItemId::Function(i) => program.functions.get(*i).map(|v| &v.name),
        ast::ItemId::Struct(i) => program.records.get(*i).map(|v| &v.name),
        ast::ItemId::Enum(i) => program.enums.get(*i).map(|v| &v.name),
        ast::ItemId::Module(i) => program.modules.get(*i).map(|v| &v.name),
        ast::ItemId::Import(i) => program.imports.get(*i).map(|v| &v.alias),
    }
}
fn equals_u8(text: &str, work: &WorkMeter, at: &Span) -> Result<bool, Failure> {
    debit(work, at, "comparison")?;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < 2 && i < bytes.len() {
        debit(work, at, "compared byte")?;
        if bytes[i] != b"u8"[i] {
            return Ok(false);
        }
        i += 1;
    }
    Ok(bytes.len() == 2)
}
struct Frame<'a, 's> {
    tables: &'a Tables<'s>,
    work: &'a WorkMeter,
    eof: &'a Span,
    module: usize,
    item: usize,
    ast: Option<&'s ast::Program>,
    row: Option<&'a ModuleRow>,
    previous: Option<&'s Span>,
}
fn core(tables: &Tables<'_>, work: &WorkMeter, eof: &Span) -> Result<(), Failure> {
    let mut f = Frame {
        tables,
        work,
        eof,
        module: 0,
        item: 0,
        ast: None,
        row: None,
        previous: None,
    };
    while f.module < f.tables.modules.len() {
        debit(f.work, f.eof, "u8 order module")?;
        f.ast = Some(
            f.tables
                .sources
                .try_ast_borrowed(ModuleId(f.module))
                .ok_or_else(|| Failure::bad(f.eof))?,
        );
        f.item = 0;
        f.previous = None;
        while f.item < f.ast.unwrap().items.len() {
            debit(f.work, f.eof, "u8 order item")?;
            let at = origin(f.ast.unwrap(), &f.ast.unwrap().items[f.item])
                .ok_or_else(|| Failure::bad(f.eof))?;
            debit(f.work, at, "u8 order comparison")?;
            if f.previous
                .is_some_and(|p| p.file != at.file || p.end > at.start)
            {
                return Err(Failure::bad(at));
            }
            f.previous = Some(at);
            f.item += 1;
        }
        f.module += 1;
    }
    f.module = 0;
    while f.module < f.tables.modules.len() {
        debit(f.work, f.eof, "u8 reservation module")?;
        f.ast = Some(
            f.tables
                .sources
                .try_ast_borrowed(ModuleId(f.module))
                .ok_or_else(|| Failure::bad(f.eof))?,
        );
        f.row = f.tables.modules.get(f.module);
        f.item = 0;
        while f.item < f.ast.unwrap().items.len() {
            debit(f.work, f.eof, "u8 reservation item")?;
            match &f.ast.unwrap().items[f.item] {
                ast::ItemId::Function(_) | ast::ItemId::Module(_) => {}
                item => {
                    let at = origin(f.ast.unwrap(), item).ok_or_else(|| Failure::bad(f.eof))?;
                    debit(f.work, at, "u8 reservation binding")?;
                    let candidate = match item {
                        ast::ItemId::Struct(_) | ast::ItemId::Enum(_) => true,
                        ast::ItemId::Import(local) => {
                            let id = (f.row.unwrap().import_start as usize)
                                .checked_add(*local)
                                .ok_or_else(|| Failure::bad(at))?;
                            let row = f.tables.imports.get(id).ok_or_else(|| Failure::bad(at))?;
                            let alias = f
                                .tables
                                .aliases
                                .get(row.alias_group as usize)
                                .ok_or_else(|| Failure::bad(at))?;
                            alias.type_target != NO_DECLARATION
                                && alias.type_first_import as usize == id
                        }
                        _ => unreachable!(),
                    };
                    if candidate
                        && equals_u8(
                            f.tables
                                .sources
                                .try_text_borrowed(at)
                                .ok_or_else(|| Failure::bad(f.eof))?,
                            f.work,
                            at,
                        )?
                    {
                        return Err(Failure {
                            reason: Reason::ReservedU8,
                            at: *at,
                        });
                    }
                }
            }
            f.item += 1;
        }
        f.module += 1;
    }
    Ok(())
}
pub(super) fn scan(
    tables: &Tables<'_>,
    work: &WorkMeter,
    eof: &Span,
) -> Result<(), Box<Diagnostic>> {
    // No callback is passed into core. Its complete frame is gone before this
    // match materializes any diagnostic, including budget/owner failures.
    match core(tables, work, eof) {
        Ok(()) => Ok(()),
        Err(failure) => Err(failure.diagnostic()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_comparison_matches_generic_at_every_debit_prefix() {
        let at = Span {
            file: SourceFileId(0),
            start: 7,
            end: 9,
        };
        let mut cases = vec![
            String::new(),
            "u".into(),
            "u8".into(),
            "u80".into(),
            "u9".into(),
            "v8".into(),
            "ü8".into(),
            "😀".into(),
        ];
        for a in 0u8..=127 {
            for b in 0u8..=127 {
                cases.push(String::from_utf8(vec![a, b]).unwrap());
            }
        }
        for text in cases {
            for limit in 0..=3 {
                let old = WorkMeter::new(limit);
                let new = WorkMeter::new(limit);
                old.enable_observation();
                new.enable_observation();
                let a = compare_bytes(&text, "u8", &old, at).map(|o| o == Ordering::Equal);
                let b = equals_u8(&text, &new, &at);
                assert_eq!(a.as_ref().ok(), b.as_ref().ok(), "{text:?} limit {limit}");
                if let (Err(a), Err(b)) = (a, b) {
                    let b = b.diagnostic();
                    assert_eq!(format!("{a:?}"), format!("{b:?}"));
                }
                assert_eq!(old.used(), new.used());
                assert_eq!(
                    format!("{:?}", old.events.borrow()),
                    format!("{:?}", new.events.borrow())
                );
            }
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn phase_layout_inventory_not_admission() {
        macro_rules! show {
            ($label:literal,$ty:ty) => {
                println!(
                    "{} size={} align={}",
                    $label,
                    size_of::<$ty>(),
                    std::mem::align_of::<$ty>()
                );
            };
        }
        show!("frame", Frame<'static, 'static>);
        show!("reason", Reason);
        show!("failure", Failure);
        show!("core result",Result<(),Failure>);
        show!("comparison result",Result<bool,Failure>);
        show!("debit reason", DebitFailure);
        show!("debit result",Result<(),DebitFailure>);
        show!("fmt Arguments", std::fmt::Arguments<'static>);
        show!("fmt Formatter", std::fmt::Formatter<'static>);
        show!("fmt write trait object", &'static mut dyn std::fmt::Write);
        for (name, size, align) in owned_diagnostic::u8_error_phase_layouts() {
            println!("{name} size={size} align={align}");
        }
        println!(
            "prepared pair={} unchanged fixed={}",
            2 * size_of::<PreparedTypeName<'static>>(),
            FIXED_SCRATCH
        );
    }
    #[test]
    fn finite_error_materialization_preserves_complete_diagnostic() {
        let at = Span {
            file: SourceFileId(2),
            start: 7,
            end: 9,
        };
        for reason in [
            Reason::BadIdentity,
            Reason::CountOverflow,
            Reason::WorkLimit,
            Reason::ReservedU8,
        ] {
            let new = Failure { reason, at }.diagnostic();
            let old=match reason {
                Reason::BadIdentity=>bad(at),Reason::CountOverflow=>overflow(at),
                Reason::WorkLimit=>resource("declaration index work limit exceeded",at),
                Reason::ReservedU8=>diagnostic("E0208","resolve","type name u8 is reserved for the unsigned-byte primitive; rename the type or import alias",at),
            };
            assert_eq!(format!("{old:?}"), format!("{new:?}"));
            assert_eq!(old.message.capacity(), new.message.capacity());
        }
    }
    #[test]
    fn diagnostic_injected_allocation_failure_parity() {
        let at = Span {
            file: SourceFileId(2),
            start: 7,
            end: 9,
        };
        for reason in [
            Reason::BadIdentity,
            Reason::CountOverflow,
            Reason::WorkLimit,
            Reason::ReservedU8,
        ] {
            for fail_after in [0, 1, 2] {
                let (new, nm) = owned_diagnostic::measure_formatting(|| {
                    owned_diagnostic::fail_allocation_after(fail_after, || {
                        Failure { reason, at }.diagnostic()
                    })
                });
                let (old, om) = owned_diagnostic::measure_formatting(|| {
                    owned_diagnostic::fail_allocation_after(fail_after, || {
                        match reason {
                Reason::BadIdentity=>bad(at),Reason::CountOverflow=>overflow(at),
                Reason::WorkLimit=>resource("declaration index work limit exceeded",at),
                Reason::ReservedU8=>diagnostic("E0208","resolve","type name u8 is reserved for the unsigned-byte primitive; rename the type or import alias",at),
            }
                    })
                });
                assert_eq!(format!("{old:?}"), format!("{new:?}"));
                assert_eq!(format!("{om:?}"), format!("{nm:?}"));
                assert_eq!(old.message.capacity(), new.message.capacity());
                assert_eq!(nm.allocations, 1);
                if fail_after == 0 {
                    assert_eq!(new.code, "E0400");
                    assert_eq!(new.message, "owned diagnostic storage exhausted");
                }
            }
        }
    }
}

/// Source-role envelope, not optimized machine stack or std/allocator internals.
/// Every tuple names repository-owned arguments, locals and result transports.
/// Independently exclusive helper calls use max; no predecessor counter credit.
mod carrier_envelope {
    use super::*;
    // Compiler-inspected callsite witness, not a substitute for std runtime state.
    // Qualified rustc print-type-sizes reports rt::Argument as two pointer fields,
    // size16/align8. The private actual type cannot be named by stable user code.
    #[repr(C)]
    struct FormatArgumentCarrier {
        value: *const (),
        formatter: *const (),
    }

    const fn maximum(a: usize, b: usize) -> usize {
        if a > b {
            a
        } else {
            b
        }
    }
    macro_rules! role {
        ($name:literal, $ty:ty) => {
            size_of::<$ty>()
        };
    }
    pub(super) const fn phase_bytes() -> usize {
        // Tables/Scratch, including their heap payloads, are unchanged and remain
        // priced by predecessor terms. No term or bank is reduced here.
        let common = role!(
            "outer EOF/grouping result/empty errors/scan arguments/outcome",
            (
                Span,
                Result<(), Box<Diagnostic>>,
                Vec<Diagnostic>,
                &Tables<'static>,
                &WorkMeter,
                &Span,
                Result<(), Failure>
            )
        );
        let frame = role!(
            "core frame and core argument/return transports",
            (
                Frame<'static, 'static>,
                &Tables<'static>,
                &WorkMeter,
                &Span,
                Result<(), Failure>
            )
        );
        let body = role!(
            "body item/name/candidate/id/import/alias plus checked temporaries",
            (
                &ast::ItemId,
                &Span,
                bool,
                usize,
                &ImportRow,
                &AliasCell,
                Option<usize>,
                Option<&ImportRow>,
                Option<&AliasCell>,
                Failure,
                Result<(), Failure>
            )
        );
        let origin = role!(
            "origin request/per-kind index/optional borrow/result",
            (
                &ast::Program,
                &ast::ItemId,
                &usize,
                Option<&ast::Function>,
                Option<&Span>
            )
        );
        let identity = role!(
            "AST/provenance identity call chain requests and scalar/span results",
            (
                &ast::Program,
                &SourceFile,
                &super::super::super::parser::SourceProvenance,
                &SourceFile,
                u64,
                u64,
                usize,
                usize,
                Span,
                &SourceFile,
                usize,
                usize,
                bool,
                &str
            )
        );
        let ast = role!(
            "borrowed AST/helper project lookup requests and returns",
            (
                &SourceOwner<'static>,
                ModuleId,
                &ProjectSources,
                &super::super::super::project::ModuleHeader,
                Option<&super::super::super::project::ModuleHeader>,
                SourceFileId,
                Option<&ast::Program>,
                Option<&SourceFile>,
                &ast::Program,
                &SourceFile,
                Option<&ast::Program>
            )
        ) + identity;
        let text = role!(
            "borrowed text/SourceFile/range/optional-string chain",
            (
                &SourceOwner<'static>,
                &Span,
                &ProjectSources,
                &SourceFile,
                Option<&SourceFile>,
                Span,
                std::ops::Range<usize>,
                &str,
                Option<&str>,
                Option<&str>
            )
        );
        let compact_debit = role!(
            "compact meter request/checked sum/Cell values/return",
            (
                &WorkMeter,
                u64,
                &Span,
                &str,
                Option<u64>,
                u64,
                u64,
                u64,
                Result<(), DebitFailure>,
                DebitFailure
            )
        );
        let debit = role!(
            "debit adapter request/closure reason/span/result",
            (
                &WorkMeter,
                &Span,
                &str,
                DebitFailure,
                &Span,
                Failure,
                Result<(), Failure>
            )
        ) + compact_debit;
        let compare = role!(
            "finite comparison request/slice/cursor/byte/bool/result",
            (
                &str,
                &WorkMeter,
                &Span,
                &[u8],
                usize,
                u8,
                u8,
                bool,
                Result<bool, Failure>
            )
        ) + debit;
        // All fallible option->failure constructors capture only an existing
        // origin borrow. Their Failure and result are also priced in body.
        let failure_closure = role!(
            "checked access closure capture and result",
            (&Span, Failure)
        );
        let core =
            frame + body + failure_closure + maximum(maximum(maximum(origin, ast), text), compare);

        let layouts = owned_diagnostic::u8_error_phase_layouts();
        let dispatch = role!(
            "failure dispatch value/tag/span/box return",
            (Failure, Reason, Span, Box<Diagnostic>)
        );
        // Finite dispatch calls the SAME existing diagnostic constructor once.
        // It does not call overflow/resource wrappers, specialize formatting or
        // change allocation/fallback policy. Compiler-emitted callsite storage:
        // tuple of message borrow; actual descriptor16/align8 (probe receipt);
        // template/array borrows; public fmt::Arguments. No std stack claim.
        let wrappers = role!(
            "direct diagnostic request/return and generated formatting carriers",
            (
                &str,
                &str,
                &str,
                Span,
                Box<Diagnostic>,
                (&&str,),
                FormatArgumentCarrier,
                &[FormatArgumentCarrier; 1],
                &[u8; 2],
                std::fmt::Arguments<'static>
            )
        );
        let format_call = role!(
            "fmt::write caller trait object/arguments/result",
            (
                &mut dyn std::fmt::Write,
                std::fmt::Arguments<'static>,
                std::fmt::Result
            )
        );
        let prefix = role!(
            "write_str truncation/prefix helper arguments and local ranges",
            (&str, usize, usize, usize, usize, std::ops::Range<usize>)
        );
        // Formatting and initial reserve are sequential inside component; the
        // owned component envelope deliberately retains both possible results.
        let component = layouts[2].1 + maximum(layouts[3].1, format_call + layouts[4].1 + prefix);
        let diagnostic_value = role!(
            "successful diagnostic assembly/value/box",
            (
                String,
                Option<Span>,
                Vec<(Span, String)>,
                Vec<String>,
                Diagnostic,
                Box<Diagnostic>
            )
        );
        let emergency = role!(
            "Diagnostic::new emergency request/message/value/result",
            (
                &str,
                &str,
                &str,
                Option<Span>,
                String,
                Diagnostic,
                Box<Diagnostic>
            )
        );
        let error = dispatch
            + wrappers
            + layouts[0].1
            + layouts[1].1
            + maximum(
                maximum(
                    maximum(component, layouts[6].1),
                    size_of::<Result<String, ()>>() + diagnostic_value,
                ),
                size_of::<Result<String, ()>>() + layouts[5].1 + emergency,
            );
        let vector = role!(
            "post-scan diagnostic move/vector/result",
            (
                Box<Diagnostic>,
                Diagnostic,
                Vec<Diagnostic>,
                Result<(), Vec<Diagnostic>>
            )
        );
        // Additional phase in calculate. Existing Counts/IndexPlan/
        // IndexLimits remain charged by their unchanged predecessor terms.
        // Borrowed helper adds no owner copies and returns before overflow is
        // materialized; all new callsite results/captures are explicit here.
        let bound_common = role!(
            "bound callsite request/outcome/additional/capture/error",
            (
                &Counts,
                Option<u64>,
                u64,
                &Span,
                Result<u64, Box<Diagnostic>>
            )
        );
        let bound_core = role!(
            "checked-bound request/five locals/intermediate/output",
            (&Counts, u64, u64, u64, u64, u64, Option<u64>, Option<u64>)
        );
        let bound_wrappers = role!(
            "bound overflow/resource/diagnostic requests and generated carriers",
            (
                Span,
                &str,
                Span,
                &str,
                &str,
                &str,
                Span,
                Box<Diagnostic>,
                Box<Diagnostic>,
                Box<Diagnostic>,
                (&&str,),
                FormatArgumentCarrier,
                &[FormatArgumentCarrier; 1],
                &[u8; 2],
                std::fmt::Arguments<'static>
            )
        );
        let bound_error = bound_wrappers
            + layouts[0].1
            + layouts[1].1
            + maximum(
                maximum(
                    maximum(component, layouts[6].1),
                    size_of::<Result<String, ()>>() + diagnostic_value,
                ),
                size_of::<Result<String, ()>>() + layouts[5].1 + emergency,
            );
        let bound_add = role!(
            "final build_work add request/checked result/closure/return",
            (
                u64,
                u64,
                Span,
                Option<u64>,
                &Span,
                Result<u64, Box<Diagnostic>>
            )
        );
        let bound_phase = bound_common + maximum(bound_core, bound_add + bound_error);
        maximum(common + maximum(maximum(core, error), vector), bound_phase)
    }
    #[test]
    fn complete_repository_owned_phase_envelope() {
        let peak = phase_bytes();
        let pair = 2 * size_of::<PreparedTypeName<'static>>();
        println!("U8-INDEX-INTEGRATION-1 peak={peak} prepared_pair={pair} fixed={FIXED_SCRATCH}");
        assert_eq!(peak, 912);
        assert!(peak <= pair);
        assert_eq!(FIXED_SCRATCH, 4096);
    }
}

/// Mandatory-bound helper has no copied Counts or diagnostic call chain.
/// Its Option is translated by calculate only after this frame returns.
pub(super) fn checked_bound(c: &Counts) -> Option<u64> {
    let n = c.originals.checked_add(c.imports)?;
    let bindings = c.records.checked_add(c.enums)?.checked_add(c.imports)?;
    let modules = c.modules.checked_mul(2)?;
    let visits = n.checked_mul(3)?;
    let candidates = bindings.checked_mul(4)?;
    modules.checked_add(visits)?.checked_add(candidates)
}
#[cfg(test)]
mod bound_tests {
    use super::*;
    fn historical_bound(c: Counts, at: Span) -> Result<u64, Box<Diagnostic>> {
        // N=O+I. Full source-order validation M+2N, then scanner M+N+4(R+E+I).
        let n = add(c.originals, c.imports, at)?;
        let bindings = add(add(c.records, c.enums, at)?, c.imports, at)?;
        add(
            add(
                c.modules.checked_mul(2).ok_or_else(|| overflow(at))?,
                n.checked_mul(3).ok_or_else(|| overflow(at))?,
                at,
            )?,
            bindings.checked_mul(4).ok_or_else(|| overflow(at))?,
            at,
        )
    }

    #[test]
    fn compact_bound_matches_historical_checked_arithmetic() {
        let at = Span {
            file: SourceFileId(0),
            start: 0,
            end: 0,
        };
        for value in [
            0,
            1,
            2,
            255,
            u64::MAX / 4,
            u64::MAX / 3,
            u64::MAX / 2,
            u64::MAX,
        ] {
            for lane in 0..5 {
                let mut c = Counts::default();
                match lane {
                    0 => c.modules = value,
                    1 => c.originals = value,
                    2 => c.imports = value,
                    3 => c.records = value,
                    _ => c.enums = value,
                }
                assert_eq!(checked_bound(&c), historical_bound(c, at).ok());
                c.imports = value;
                assert_eq!(checked_bound(&c), historical_bound(c, at).ok());
            }
        }
    }
    #[test]
    fn added_bound_overflow_precedes_all_three_limits() {
        let at = Span {
            file: SourceFileId(0),
            start: 2,
            end: 4,
        };
        let c = Counts {
            modules: 1,
            variant_duplicate_work: u64::MAX - 145,
            ..Counts::default()
        };
        let limits = IndexLimits {
            retained: 0,
            scratch: 0,
            work: 0,
        };
        let new = IndexPlan::calculate(c, 376, FIXED_SCRATCH, limits, at).unwrap_err();
        assert_eq!(new.message, "declaration index count overflow");
        assert_eq!(new.primary, Some(at));
    }
}

/// Complete repository-owned phase envelope; excludes unchanged opaque std internals.
pub(super) const PHASE_BYTES: usize = carrier_envelope::phase_bytes();
