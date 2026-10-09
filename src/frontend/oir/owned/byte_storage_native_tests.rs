//! RFC0031 standalone bytes. Expected values are ordinary byte sequences and
//! source semantics fixed before execution. Resource observations measure the
//! successor; they are not replacements for archived no-byte endpoints.
use super::*;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use crate::frontend::source::SourceFileId;

const BYTE: &str = "fn byte(x:i32)->u8{return x.to_u8_checked();}";
const PILOT: &str = "fn relay(a:[u8;4])->[u8;4]{return a;}fn edit(a:&mut[u8])->(){a[1]=byte(255);return;}fn exact(a:&[u8;4])->i32{return sum(&*a);}fn sum(a:&[u8])->i32{let mut i=0;let mut total=0;while i<a.len(){let b=a[i];total=total+b.to_i32();i=i+1;}return total;}fn main()->i32{let mut a=relay([byte(0),byte(127),byte(128),byte(255)]);let left=[byte(17)];let empty:[u8;0]=[];let right=[byte(29)];edit(&mut a);if a[0]!=byte(0)||a[1]!=byte(255)||a[2]!=byte(128)||a[3]!=byte(255)||left[0]!=byte(17)||right[0]!=byte(29){return -1;}return exact(&a)+sum(&empty);}";

struct Case {
    sources: SourceMap,
    witness: VerifiedOwnedProgram,
    entry: hir::DefId,
}
impl Case {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fn span(&self, needle: &str) -> Span {
        let source = self.sources.get(SourceFileId(0));
        let start = source.text().find(needle).unwrap();
        source.span(start, start + needle.len())
    }
    fn observe(&self, control: NativeControl) -> NativeObservation {
        run_array_observed(&self.witness, Some(self.entry), &self.sources, control)
    }
    fn module(&self) -> String {
        self.observe(NativeControl::default()).result.unwrap()
    }
}

/// Re-run the source association and authoritative verifier. Bare raw byte
/// operations never provide the conversion authority used by these programs.
fn checked(text: &str) -> Case {
    let (sources, witness, entry) = source::byte_storage_native_fixture::checked(text);
    Case {
        sources,
        witness,
        entry,
    }
}

fn partition(start: usize) -> String {
    let elements = (start..start + 16)
        .map(|n| format!("byte({n})"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{BYTE}fn relay(a:[u8;16])->[u8;16]{{return a;}}fn mutate(a:&mut[u8],start:i32)->(){{let mut i=0;while i<a.len(){{let b=a[i];if b.to_i32()!=start+i{{a[i]=byte(999);}}a[i]=byte(255-start-i);i=i+1;}}return;}}fn exact(a:&mut[u8;16],start:i32)->(){{mutate(&mut *a,start);return;}}fn main()->i32{{let left=[byte(127),byte(128)];let empty:[u8;0]=[];let mut a=relay([{elements}]);let right=[byte(255),byte(0)];exact(&mut a,{start});let mut i=0;while i<a.len(){{let b=a[i];if b.to_i32()!=255-{start}-i{{return -1;}}i=i+1;}}if left[0]!=byte(127)||left[1]!=byte(128)||right[0]!=byte(255)||right[1]!=byte(0)||empty.len()!=0{{return -2;}}return a.len();}}")
}

fn assert_inventory(observation: &NativeObservation) {
    let ir = observation.result.as_ref().unwrap();
    let m = &observation.metrics;
    assert_eq!(m.count_bytes, ir.len());
    assert_eq!(m.render_bytes, ir.len());
    assert_eq!(m.count_expansions, m.transfer_cells + m.message_bytes);
    assert_eq!(m.count_expansions, m.render_expansions);
    assert_eq!(m.count_expansion_kinds, m.render_expansion_kinds);
    assert_eq!(m.count_ordinary_visits, m.render_ordinary_visits);
    assert_eq!(m.count_predecessor_visits, m.render_predecessor_visits);
    assert_eq!(
        m.count_borrow_projection_visits,
        m.render_borrow_projection_visits
    );
    assert_eq!(m.message_count_bytes, m.message_render_bytes);
    assert_eq!(m.count_call_scratch_peak, m.render_call_scratch_peak);
    assert!(m.metadata_peak <= Limits::DEFAULT.metadata_bytes);
    assert!(m.metadata_admitted_bytes <= Limits::DEFAULT.metadata_bytes);
    assert!(
        m.render_call_scratch_peak + plan::native_storage::FIXED_CARRIER_ALLOWANCE
            <= EMITTER_TRANSIENT_BYTES
    );
    for forbidden in [
        "inbounds",
        "noalias",
        "nonnull",
        "dereferenceable",
        "undef",
        "poison",
    ] {
        assert!(!ir.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn byte_storage_native_stride_sentinel_and_scalar_transport() {
    for prefix in ["", "while false{}"] {
        let case = checked(&format!("{BYTE}fn main()->i32{{{prefix}let mut a=[byte(0),byte(128),byte(255)];a[0]=a[2];let b=a[1];return b.to_i32();}}"));
        let observed = case.observe(NativeControl::default());
        assert_inventory(&observed);
        let module = observed.result.unwrap();
        assert!(module.contains("_value = load i8, ptr "));
        assert!(module.contains("_wide = zext i8 "));
        assert!(module.contains("_value = zext i8 "));
        assert!(module.contains(" to i32\n"));
        assert!(module.contains(" = trunc i64 "));
        assert!(!module.contains("sext i8"));
        for line in module
            .lines()
            .filter(|line| line.contains("_offset = mul i64 "))
        {
            assert!(line.ends_with(", 1"), "{line}");
        }
        for (position, _) in module.match_indices("_index64 = zext i32") {
            let previous = &module[..position];
            let success = previous.rfind("_bounds_ok:\n").unwrap();
            let branch = previous[..success].rfind("_in_range, label").unwrap();
            let signed = previous[..branch]
                .rfind("_nonnegative = icmp sge i32")
                .unwrap();
            assert!(signed < branch && branch < success);
        }
    }
    let empty = checked("fn relay(a:[u8;0])->[u8;0]{return a;}fn len(a:&[u8])->i32{return a.len();}fn main()->i32{let a:[u8;0]=[];let b=relay(a);return len(&b);}");
    let module = empty.module();
    assert!(module.contains("store i8 0, ptr %o"));
    assert!(module.contains("_empty = load i8, ptr "));
    assert!(!module.contains("_index64"));
    assert!(module.contains("ptr %arg0, i32 %arg0_length"));
}

#[test]
fn byte_storage_native_pilot_and_every_byte_inventory() {
    for text in std::iter::once(format!("{BYTE}{PILOT}")).chain((0..256).step_by(16).map(partition))
    {
        let case = checked(&text);
        let observed = case.observe(NativeControl::default());
        assert_inventory(&observed);
    }
}

#[test]
fn byte_storage_native_storage_plan_preserves_exact_ranges_and_scalar_abi() {
    let a = checked(&format!("{BYTE}{PILOT}"));
    let b = checked(&format!("{BYTE}{PILOT}"));
    let plan = ExecutionPlan::build(&a.witness).unwrap();
    let other = ExecutionPlan::build(&b.witness).unwrap();
    let storage = NativeStoragePlan::checked(&plan, true).unwrap();
    assert!(std::ptr::eq(storage.execution(), &plan));
    assert!(!std::ptr::eq(
        storage.execution().witness(),
        other.witness()
    ));
    for f in plan.witness().functions() {
        let native = storage.function(f.id);
        let usage = plan.function(f.id).usage();
        assert!(std::ptr::eq(native.raw(), f));
        assert_eq!(native.scalar_slots(), usage.scalar_slots + usage.arguments);
        for slot in 0..native.scalar_slots() {
            assert_eq!(native.scalar_offset(slot), slot * 8);
        }
        let mut end = 0;
        let mut width = 0;
        for (i, owner) in f.owners.iter().enumerate() {
            let AggregateTy::FixedArray(array) = owner.aggregate() else {
                panic!("standalone array fixture");
            };
            assert_eq!(array.element(), hir::Ty::U8);
            assert_eq!(array.stride(), 1);
            assert_eq!(native.owner_offset(OwnerPlaceId(i)), end);
            end += array.length().max(1);
            width += array.length().max(1);
        }
        assert_eq!(usage.owner_cells, width);
        assert_eq!(usage.payload_bytes, end);
        assert_eq!(native.owner_bytes(), (end + 3) & !3);
        assert_eq!(native.native_bytes().unwrap(), usage.native_bytes);
        assert_eq!(
            native.native_bytes().unwrap(),
            8 * native.scalar_slots()
                + native.owner_bytes()
                + 8 * native.reference_slots()
                + 4 * native.slice_slots()
        );
        println!("RFC0031 native frame={} scalar={} arguments={} owners={} width={} payload={} native={} refs={} slices={}", f.id.0, usage.scalar_slots, usage.arguments, usage.owners, width, end, usage.native_bytes, native.reference_slots(), native.slice_slots());
    }
}

#[test]
fn byte_storage_native_exact_text_metadata_and_allocation_endpoints() {
    for prefix in ["", "while false{}"] {
        let case = checked(&format!("{BYTE}fn relay(a:[u8;2])->[u8;2]{{return a;}}fn main()->i32{{{prefix}let a=relay([byte(128),byte(255)]);let b=a[1];return b.to_i32();}}"));
        let accepted = case.observe(NativeControl::default());
        assert_inventory(&accepted);
        let m = &accepted.metrics;
        let exact = Limits {
            ir_bytes: m.render_bytes,
            diagnostic_bytes: m.message_bytes,
            metadata_bytes: m.metadata_peak.max(m.metadata_admitted_bytes),
            ..Limits::DEFAULT
        };
        let same = case.observe(NativeControl {
            limits: exact,
            ..NativeControl::default()
        });
        assert_eq!(same.result.unwrap(), *accepted.result.as_ref().unwrap());
        for (limits, marker) in [
            (
                Limits {
                    ir_bytes: exact.ir_bytes - 1,
                    ..exact
                },
                "LLVM bytes",
            ),
            (
                Limits {
                    diagnostic_bytes: exact.diagnostic_bytes - 1,
                    ..exact
                },
                "diagnostic bytes",
            ),
            (
                Limits {
                    metadata_bytes: exact.metadata_bytes - 1,
                    ..exact
                },
                "metadata bytes",
            ),
        ] {
            let denied = case.observe(NativeControl {
                limits,
                ..NativeControl::default()
            });
            let error = denied.result.unwrap_err();
            assert_eq!(error.code, "E0700");
            assert!(error.message.contains(marker), "{error:?}");
            assert_eq!(denied.metrics.render_bytes, 0);
            assert_eq!(denied.metrics.failed_allocation, None);
            assert!(denied.metrics.allocation_attempts < m.allocation_attempts);
        }
        for fail_after in 0..m.allocation_attempts {
            let failed = case.observe(NativeControl {
                fail_after: Some(fail_after),
                ..NativeControl::default()
            });
            assert!(failed.result.unwrap_err().message.contains("allocation"));
            assert_eq!(failed.metrics.allocation_attempts, fail_after + 1);
            assert!(failed.metrics.failed_allocation.is_some());
            assert_eq!(failed.metrics.render_bytes, 0);
        }
        println!("RFC0031 native guarded={} {m:?}", !prefix.is_empty());
    }
}

#[test]
fn byte_storage_native_large_source_and_array_entry_refuse_before_emission() {
    let elements = vec!["b"; 1024].join(",");
    let large = checked(&format!(
        "{BYTE}fn main()->i32{{let b=byte(255);let a=[{elements}];return a.len();}}"
    ));
    let denied = large.observe(NativeControl::default());
    let error = denied.result.unwrap_err();
    assert_eq!(error.code, "E0700");
    assert!(error.message.contains("slot"), "{error:?}");
    assert_eq!(denied.metrics.count_bytes, 0);
    assert_eq!(denied.metrics.render_bytes, 0);
    assert_eq!(denied.metrics.allocation_attempts, 0);
    let entry = checked("fn main()->[u8;0]{let a:[u8;0]=[];return a;}");
    let denied = plan::fail_allocation_after(0, || entry.observe(NativeControl::default()));
    assert_eq!(denied.result.unwrap_err().code, "E0700");
    assert_eq!(denied.metrics.plan_bytes, 0);
    assert_eq!(denied.metrics.allocation_attempts, 0);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn assert_run(
    scratch: &super::tests::Scratch,
    case: &Case,
    module: &str,
    name: &str,
    expected: (&[u8], &[u8], i32),
) {
    use sha2::{Digest, Sha256};
    let binary = scratch.compile(module, name);
    let result = scratch.run(&binary, &[]);
    // Retain failures as well as successes. The runtime working directory has
    // no source or IR; source/evidence lives only in the separate evidence root.
    if let Some(evidence) = std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE") {
        let evidence = std::path::PathBuf::from(evidence);
        std::fs::create_dir_all(&evidence).unwrap();
        let source = case.sources.get(SourceFileId(0)).text();
        for (suffix, bytes) in [
            ("ox", source.as_bytes()),
            ("stdout", result.stdout.as_slice()),
            ("stderr", result.stderr.as_slice()),
        ] {
            std::fs::write(evidence.join(format!("{name}.{suffix}")), bytes).unwrap();
        }
        std::fs::write(
            evidence.join(format!("{name}.status")),
            result
                .status
                .code()
                .map_or_else(|| "signal\n".into(), |code| format!("{code}\n")),
        )
        .unwrap();
        std::fs::write(
            evidence.join(format!("{name}.source-sha256")),
            format!("{:x}\n", Sha256::digest(source.as_bytes())),
        )
        .unwrap();
        std::fs::write(evidence.join(format!("{name}.run.txt")), "source-free working directory; env_clear; PATH=no-tools; argv=[]; production O0 native compiler; stdout/stderr/status retained separately\n").unwrap();
    }
    assert_eq!(result.status.code(), Some(expected.2), "{name}");
    assert_eq!(result.stdout, expected.0, "{name}");
    assert_eq!(result.stderr, expected.1, "{name}");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn byte_storage_native_source_free_every_byte_pilot_and_empty_transfers() {
    let scratch = super::tests::Scratch::new();
    for start in (0..256).step_by(16) {
        let case = checked(&partition(start));
        assert_run(
            &scratch,
            &case,
            &case.module(),
            &format!("byte-storage-all-{start}"),
            (b"16\n", b"", 0),
        );
    }
    for (name, text, expected) in [
        ("pilot", format!("{BYTE}{PILOT}"), "638\n"),
        ("empty", "fn relay(a:[u8;0])->[u8;0]{return a;}fn len(a:&[u8])->i32{return a.len();}fn main()->i32{let a:[u8;0]=[];let mut b=relay(a);let c:[u8;0]=[];b=c;b=b;return len(&b);}".into(), "0\n"),
        ("rhs-snapshot", format!("{BYTE}fn index(a:&mut[u8])->i32{{a[0]=byte(255);return 0;}}fn edit(a:&mut[u8])->i32{{a[index(&mut *a)]=a[0];let b=a[0];return b.to_i32();}}fn main()->i32{{let mut a=[byte(128)];return edit(&mut a);}}"), "128\n"),
    ] {
        let case = checked(&text);
        assert_run(&scratch, &case, &case.module(), &format!("byte-storage-{name}"), (expected.as_bytes(), b"", 0));
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn byte_storage_native_source_free_bounds_conversion_and_phi_exits() {
    let scratch = super::tests::Scratch::new();
    for (mode, prefix) in [("acyclic", ""), ("guarded", "while false{}")] {
        let phi = checked(&format!("{BYTE}fn main()->bool{{{prefix}let a=[byte(128),byte(255)];let n=128;return (a[0]==n.to_u8_checked())&&(a[1]>a[0])&&(a[0]<a[1]);}}"));
        let module = phi.module();
        assert!(module.contains("icmp ugt i8"));
        assert!(module.contains("icmp ult i8"));
        assert_run(
            &scratch,
            &phi,
            &module,
            &format!("byte-storage-phi-{mode}"),
            (b"true\n", b"", 0),
        );
        for length in [0, 1] {
            for index in [i32::MIN, -1, length, i32::MAX] {
                let elements = if length == 0 { "" } else { "byte(255)" };
                let text = format!("{BYTE}fn read(a:&[u8])->i32{{let b=a[{index}];return b.to_i32();}}fn main()->i32{{{prefix}let a:[u8;{length}]=[{elements}];return read(&a);}}");
                let case = checked(&text);
                let span = case.span(&format!("a[{index}]"));
                let error = execute::OwnedRunFailure::Bounds(span)
                    .diagnostic(&case.sources)
                    .render_human(&case.sources);
                assert_run(
                    &scratch,
                    &case,
                    &case.module(),
                    &format!("byte-storage-bounds-{mode}-{length}-{index}"),
                    (b"", error.as_bytes(), 1),
                );
            }
        }
        // A failed RHS conversion wins over the earlier-written bad index.
        let case = checked(&format!("{BYTE}fn main()->(){{{prefix}let mut a=[byte(128)];let n=256;a[7]=n.to_u8_checked();return;}}"));
        let span = case.span("to_u8_checked");
        // BYTE's helper has the first spelling: select the failing local method.
        let source = case.sources.get(SourceFileId(0));
        let start = source.text().rfind("to_u8_checked").unwrap();
        let span = source.span(start, start + (span.end - span.start));
        let error = RunFailure::ByteRange(span)
            .diagnostic(&case.sources)
            .render_human(&case.sources);
        assert_run(
            &scratch,
            &case,
            &case.module(),
            &format!("byte-storage-rhs-range-{mode}"),
            (b"", error.as_bytes(), 1),
        );
    }
}

#[test]
fn byte_storage_native_carriers_retained_capacities_and_storage_endpoints() {
    use std::mem::align_of;
    macro_rules! report {($($t:ty),+ $(,)?) => {$(println!("RFC0031 native carrier {} size={} align={}", stringify!($t), size_of::<$t>(), align_of::<$t>());)+};}
    report!(FixedArrayTy, BorrowedTy, AggregateSlot, BorrowedSlot, ValueTy,
        OwnedInstruction, OwnedStatement, Scalar, ScalarLeaves<'static>,
        ExecutionPlan<'static>, plan::FrameUsage, plan::FunctionPlan, plan::CallPlan,
        NativeStoragePlan<'static, 'static>, NativeFunctionStorage<'static, 'static>,
        Result<NativeStoragePlan<'static, 'static>, plan::AdmissionFailure>,
        Result<NativeFunctionStorage<'static, 'static>, plan::AdmissionFailure>,
        Bound, Limits, NativeMetrics, Accounting, Diagnostics, DiagnosticOccurrence,
        DiagnosticLookup, Diagnostic, Emission, Continuation, FailureKind,
        std::fmt::Arguments<'static>, FormatCallsiteBacking<1>, FormatCallsiteBacking<5>,
        Result<String, Box<Diagnostic>>, (hir::Ty, String));
    let case = checked(&format!("{BYTE}{PILOT}"));
    let plan = ExecutionPlan::build(&case.witness).unwrap();
    let mut accounting = Accounting::default();
    let bounds = admit_accounted(&plan, Limits::DEFAULT, &mut accounting).unwrap();
    assert_eq!(
        accounting.metrics.retained_bound_bytes,
        bounds.capacity() * size_of::<Bound>()
    );
    let diagnostics = Diagnostics::new_accounted(
        &plan,
        case.entry,
        &case.sources,
        true,
        MAX_DIAGNOSTIC_BYTES,
        plan::MAX_PLAN_BYTES,
        &mut accounting,
    )
    .unwrap();
    assert_eq!(
        diagnostics.ids.capacity() * size_of::<DiagnosticLookup>(),
        accounting.metrics.lookup_bytes
    );
    assert_eq!(
        diagnostics.messages.capacity() * size_of::<String>(),
        accounting.metrics.message_header_bytes
    );
    let retained_message_bytes = diagnostics
        .messages
        .iter()
        .map(String::capacity)
        .sum::<usize>();
    assert_eq!(retained_message_bytes, accounting.metrics.message_bytes);
    let observed = case.observe(NativeControl::default());
    assert_inventory(&observed);
    let ir = observed.result.as_ref().unwrap();
    assert_eq!(ir.capacity(), ir.len());
    let inventory =
        admit_inventory_policy(&plan, (MAX_NATIVE_INVENTORY_ITEMS, MAX_NATIVE_OWNER_WIDTH))
            .unwrap();
    let total = plan
        .functions()
        .iter()
        .map(|f| f.usage().native_bytes)
        .sum::<usize>()
        + 8;
    let live = bounds.iter().map(|b| b.bytes).max().unwrap() + 8;
    let exact = Limits {
        inventory_items: inventory.items(),
        owner_width: inventory.owner_width(),
        bytes: total,
        live_bytes: live,
        ..Limits::DEFAULT
    };
    assert!(admit(&plan, exact).is_ok());
    for (limits, marker) in [
        (
            Limits {
                inventory_items: exact.inventory_items - 1,
                ..exact
            },
            "compiler inventory",
        ),
        (
            Limits {
                owner_width: exact.owner_width - 1,
                ..exact
            },
            "owner width",
        ),
        (
            Limits {
                bytes: total - 1,
                ..exact
            },
            "aggregate storage bytes",
        ),
        (
            Limits {
                live_bytes: live - 1,
                ..exact
            },
            "live storage bytes",
        ),
    ] {
        let denied = case.observe(NativeControl {
            limits,
            ..NativeControl::default()
        });
        assert!(denied.result.unwrap_err().message.contains(marker));
        assert_eq!(denied.metrics.allocation_attempts, 0);
        assert_eq!(denied.metrics.count_bytes, 0);
    }
    println!("RFC0031 native retained bounds_len={} bounds_capacity={} lookup_len={} lookup_capacity={} headers_len={} headers_capacity={} message_payload={} llvm_len={} llvm_capacity={} invoke_scratch={} admission_peak={} I={} W={} native_bytes={} live_bytes={}", bounds.len(), bounds.capacity(), diagnostics.ids.len(), diagnostics.ids.capacity(), diagnostics.messages.len(), diagnostics.messages.capacity(), retained_message_bytes, ir.len(), ir.capacity(), observed.metrics.render_call_scratch_peak, accounting.metrics.admission_scratch_peak, inventory.items(), inventory.owner_width(), total, live);
    // The only production edit adds U8 to two existing closed matches. Their
    // complete return/caller roles remain usize and &str; no allocation, format
    // backing or simultaneous temporary is introduced by either match. The
    // existing i8 scalar/load/store/transfer/ABI format sites are reused exactly.
    assert_eq!(element_stride(hir::Ty::U8), element_stride(hir::Ty::Unit));
    for length in [0, 1, 1024] {
        let byte = FixedArrayTy::check(hir::Ty::U8, length).unwrap();
        let unit = FixedArrayTy::check(hir::Ty::Unit, length).unwrap();
        assert_eq!(sentinel_ty(byte), sentinel_ty(unit));
        assert_eq!(ty(byte.element()), ty(unit.element()));
    }
}

#[test]
fn byte_storage_native_phi_uses_actual_bounds_and_conversion_success_exit() {
    for prefix in ["", "while false{}"] {
        let case = checked(&format!("{BYTE}fn main()->bool{{{prefix}let a=[byte(128),byte(255)];let n=128;return (a[0]==n.to_u8_checked())&&(a[1]>a[0])&&(a[0]<a[1]);}}"));
        let module = case.module();
        let mut found = [false; 2];
        for f in case.witness.functions() {
            for block in &f.blocks {
                let Some(merge) = &block.merge else {
                    continue;
                };
                for input in merge.incoming {
                    let predecessor = &f.blocks[input.predecessor.0];
                    let label = if prefix.is_empty() {
                        predecessor
                            .statements
                            .iter()
                            .enumerate()
                            .rev()
                            .find_map(|(i, statement)| {
                                let (suffix, kind) = match statement.kind {
                                    OwnedInstruction::ReadIndex { .. } => ("bounds_ok", 0),
                                    OwnedInstruction::Scalar(Statement::Assign(Assign {
                                        value: Rvalue::CheckedI32ToU8 { .. },
                                        ..
                                    })) => ("conversion_ok", 1),
                                    _ => return None,
                                };
                                found[kind] = true;
                                Some(format!(
                                    "f{}_b{}_i{i}_{suffix}",
                                    f.id.0, input.predecessor.0
                                ))
                            })
                            .unwrap_or_else(|| format!("b{}", input.predecessor.0))
                    } else {
                        format!(
                            "f{}_b{}_g{}_ok",
                            f.id.0,
                            input.predecessor.0,
                            predecessor.statements.len() + 1
                        )
                    };
                    assert!(
                        module.lines().any(|line| line.contains(" = phi ptr ")
                            && line.contains(&format!(", %{label} ]"))),
                        "{label}"
                    );
                }
            }
        }
        if prefix.is_empty() {
            assert_eq!(found, [true, true]);
        }
    }
}

#[test]
fn byte_storage_native_independent_small_frame_endpoints() {
    // Independently enumerated from the source lowering templates, before any
    // successor execution. Empty: len local; two owner slots. Nonempty: three
    // immutable binding locals plus eight expression locals; an extra literal
    // element adds its own copied scalar local. Every owner has full N.max(1)
    // logical width despite one-byte payload leaves. Native rounds each complete
    // owner arena to four bytes; scalar and argument positions remain eight.
    for (text, scalar, width, payload, inventory, bytes) in [
        ("fn main()->i32{let a:[u8;0]=[];return a.len();}", 1, 2, 2, 3, 12),
        ("fn main()->i32{let n=128;let b=n.to_u8_checked();let a=[b];let c=a[0];return c.to_i32();}", 11, 2, 2, 13, 92),
        ("fn main()->i32{let n=128;let b=n.to_u8_checked();let a=[b,b];let c=a[0];return c.to_i32();}", 12, 4, 4, 14, 100),
    ] {
        let case = checked(text);
        let plan = ExecutionPlan::build(&case.witness).unwrap();
        let usage = plan.function(case.entry).usage();
        assert_eq!(usage.scalar_slots, scalar);
        assert_eq!(usage.owners, 2);
        assert_eq!(usage.owner_cells, width);
        assert_eq!(usage.payload_bytes, payload);
        assert_eq!(usage.native_bytes, bytes);
        assert_eq!(usage.expanded_cells, scalar + width + 8);
        let exact = Limits { function_slots: scalar + 2, inventory_items: inventory, owner_width: width, bytes, live_bytes: bytes, ..Limits::DEFAULT };
        let accepted = case.observe(NativeControl { limits: exact, ..NativeControl::default() });
        assert_inventory(&accepted);
        for (limits, marker) in [
            (Limits { function_slots: scalar + 1, ..exact }, "scalar and owner slots"),
            (Limits { inventory_items: inventory - 1, ..exact }, "compiler inventory"),
            (Limits { owner_width: width - 1, ..exact }, "owner width"),
            (Limits { bytes: bytes - 1, ..exact }, "aggregate storage bytes"),
            (Limits { live_bytes: bytes - 1, ..exact }, "live storage bytes"),
        ] {
            let refused = case.observe(NativeControl { limits, ..NativeControl::default() });
            let error = refused.result.unwrap_err();
            assert_eq!(error.code, "E0700");
            assert!(error.message.contains(marker), "{error:?}");
            assert_eq!(refused.metrics.allocation_attempts, 0);
            assert_eq!(refused.metrics.count_bytes, 0);
            assert_eq!(refused.metrics.render_bytes, 0);
        }
        println!("RFC0031 independent native endpoint S={scalar} O=2 W={width} payload={payload} I={inventory} native={bytes}");
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn byte_storage_native_source_free_independent_every_fuel_schedule() {
    // Imported before native execution from the independent reference lane's
    // frozen 36ee2b6 source schedule, with the write target origin independently
    // corrected from baseline instruction_span/IndexAssign lowering. The unused
    // loop selects guarded emission
    // without changing main's source positions, activation, or execution trace.
    let text = "fn main()->i32{let x=128;let b=x.to_u8_checked();let mut a=[b];a[0]=b;let c=a[0];return c.to_i32();}fn unused_guard()->(){while false{}return;}";
    let case = checked(text);
    let at = |needle: &str| case.span(needle);
    let within = |container: &str, needle: &str| {
        let start = text.find(container).unwrap() + container.find(needle).unwrap();
        Span {
            file: SourceFileId(0),
            start,
            end: start + needle.len(),
        }
    };
    let schedule = [
        (at("main"), 24),
        (at("128"), 1),
        (at("let x=128;"), 1),
        (within("x.to_u8_checked()", "x"), 1),
        (at("x.to_u8_checked()"), 1),
        (at("let b=x.to_u8_checked();"), 1),
        (within("[b]", "b"), 1),
        (at("[b]"), 1),
        (at("[b]"), 2),
        (at("let mut a=[b];"), 1),
        (at("let mut a=[b];"), 2),
        (at("let mut a=[b];"), 2),
        (within("a[0]=b;", "b"), 1),
        (within("a[0]=b;", "0"), 1),
        (within("a[0]=b;", "a[0]"), 1),
        (within("let c=a[0];", "0"), 1),
        (within("let c=a[0];", "a[0]"), 1),
        (at("let c=a[0];"), 1),
        (within("c.to_i32()", "c"), 1),
        (at("c.to_i32()"), 1),
        (at("return c.to_i32();"), 2),
        (at("return c.to_i32();"), 3),
    ];
    assert_eq!(schedule.iter().map(|(_, cost)| cost).sum::<usize>(), 51);
    let scratch = super::tests::Scratch::new();
    for fuel in 0..=51 {
        let observation = case.observe(NativeControl {
            fuel,
            ..NativeControl::default()
        });
        assert_inventory(&observation);
        let module = observation.result.unwrap();
        let mut remaining = fuel;
        let failure = schedule.iter().find_map(|&(span, cost)| {
            if remaining < cost {
                Some(span)
            } else {
                remaining -= cost;
                None
            }
        });
        let error = failure.map(|span| {
            RunFailure::Fuel(span)
                .diagnostic(&case.sources)
                .render_human(&case.sources)
        });
        let expected: (&[u8], &[u8], i32) = match &error {
            Some(error) => (b"", error.as_bytes(), 1),
            None => (b"128\n", b"", 0),
        };
        assert_run(
            &scratch,
            &case,
            &module,
            &format!("byte-storage-fuel-{fuel}"),
            expected,
        );
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires pinned LLVM 19.1.7 and one serialized native execution slot"]
fn byte_storage_native_source_free_unpaid_bounds_priority() {
    // Independent reference-lane budget: main activation36, initialization14,
    // RHS2, call preparation2, helper invocation18, body9, return2 =83 before
    // the final access price1. No native/reference output sets these budgets.
    let case = checked("fn index(p:&mut[u8])->i32{let x=255;let b=x.to_u8_checked();p[0]=b;return 1;}fn main()->i32{let x=128;let b=x.to_u8_checked();let mut a=[b];a[index(&mut a)]=a[0];return 0;}fn unused_guard()->(){while false{}return;}");
    let span = case.span("a[index(&mut a)]");
    let scratch = super::tests::Scratch::new();
    for fuel in [83, 84] {
        let module = case
            .observe(NativeControl {
                fuel,
                ..NativeControl::default()
            })
            .result
            .unwrap();
        let error = if fuel == 83 {
            RunFailure::Fuel(span).diagnostic(&case.sources)
        } else {
            execute::OwnedRunFailure::Bounds(span).diagnostic(&case.sources)
        }
        .render_human(&case.sources);
        assert_run(
            &scratch,
            &case,
            &module,
            &format!("byte-storage-unpaid-bounds-{fuel}"),
            (b"", error.as_bytes(), 1),
        );
    }
}
