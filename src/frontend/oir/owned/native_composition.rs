//! Acyclic LLVM helpers for records with tagged descendants.
//!
//! One helper per nominal declaration and operation keeps generated text
//! proportional to declaration members, rather than every path through a DAG.
//! Validation finishes before move/drop, and only active initialized cells are
//! read or copied. These helpers do not change language fuel or ownership rules.
use super::*;

// All names derive from bounded numeric IDs, never source text. Price the new
// helper/caller transports in addition to the unchanged native emitter model.
// No expanded declaration path or generated text fragment is retained here.
const NAME_BYTES: usize = 8 * usize::BITS as usize + 128;
#[allow(dead_code)]
struct EmitterCarriers {
    modes: [Mode; 4],
    aggregates: [AggregateTy; 4],
    values: [ValueTy; 3],
    scalar: hir::Ty,
    names: [String; 19],
    captures: [FormatCallsiteBacking<8>; 4],
    writes: [std::fmt::Result; 4],
    caller_parameters: [&'static ExecutionPlan<'static>; 10],
    vector_type: hir::Ty,
    vector_element: byte_buffer::VectorElement,
    vector_tag: u32,
    vector_validation_format: FormatCallsiteBacking<3>,
    vector_move_format: FormatCallsiteBacking<3>,
    vector_drop_format: FormatCallsiteBacking<2>,
    vector_writes: [std::fmt::Result; 3],
}
pub(super) const fn control_bytes() -> usize {
    size_of::<EmitterCarriers>() + 19 * NAME_BYTES
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Validate,
    Move,
    Drop,
}
impl Mode {
    fn key(self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::Move => "move",
            Self::Drop => "drop",
        }
    }
    fn result(self) -> &'static str {
        if self == Self::Validate {
            "i32"
        } else {
            "void"
        }
    }
    fn success(self) -> &'static str {
        if self == Self::Validate {
            "ret i32 0"
        } else {
            "ret void"
        }
    }
}
fn helper(aggregate: AggregateTy, mode: Mode) -> String {
    let (family, id) = match aggregate {
        AggregateTy::Record(id) => ('r', id.0),
        AggregateTy::Enum(id) => ('e', id.0),
        _ => unreachable!("nominal composition helper"),
    };
    format!("__oxid_composed_{}_{family}{id}", mode.key())
}

pub(super) fn needed(plan: &ExecutionPlan<'_>, aggregate: AggregateTy) -> bool {
    plan.witness()
        .declarations()
        .has_enum_record_nesting(aggregate)
        .expect("verified aggregate nesting")
}

pub(super) fn construction_cells(plan: &ExecutionPlan<'_>, aggregate: AggregateTy) -> usize {
    if matches!(aggregate, AggregateTy::Record(_) | AggregateTy::Enum(_)) {
        1
    } else {
        plan.witness()
            .declarations()
            .aggregate_width(aggregate)
            .expect("verified aggregate width")
    }
}

pub(super) fn helper_inventory(
    plan: &ExecutionPlan<'_>,
    visits: &mut usize,
) -> Result<usize, Box<Diagnostic>> {
    if !plan.uses_enum_record_nesting() {
        return Ok(0);
    }
    let declarations = plan.witness().declarations();
    let mut cells = 0;
    for record in declarations.records() {
        *visits = add(*visits, 1)?;
        cells = add(
            cells,
            add(
                1,
                declarations
                    .fields(record.id())
                    .expect("verified record")
                    .len(),
            )?,
        )?;
    }
    for enumeration in declarations.enums().enums() {
        *visits = add(*visits, 1)?;
        cells = add(
            cells,
            add(
                1,
                mul(
                    2,
                    declarations
                        .enums()
                        .variants(enumeration.id())
                        .expect("verified enum")
                        .len(),
                )?,
            )?,
        )?;
    }
    mul(3, cells)
}

/// The caller supplies a verified nominal type and disjoint owner extents.
/// Internal helpers propagate a closed status code, keeping source diagnostics
/// at the original operation instead of creating synthetic source locations.
#[allow(clippy::too_many_arguments)]
pub(super) fn transfer(
    out: &mut Emission,
    name: &str,
    aggregate: AggregateTy,
    source: &str,
    intent: TransferIntent<'_>,
    expansion: Expansion,
    failure: (&Diagnostics, Span),
) {
    if !out.expand(expansion) {
        return;
    }
    let validate = helper(aggregate, Mode::Validate);
    writeln!(out, "  %{name}_composed_status = call i32 @{validate}(ptr {source})\n  switch i32 %{name}_composed_status, label %{name}_composed_payload_error [\n    i32 0, label %{name}_composed_ok\n    i32 1, label %{name}_composed_tag_error\n    i32 3, label %{name}_composed_resource_error\n  ]\n{name}_composed_tag_error:").unwrap();
    emit_failure(out, failure.0, FailureKind::EnumTag, failure.1);
    writeln!(out, "{name}_composed_payload_error:").unwrap();
    emit_failure(out, failure.0, FailureKind::EnumPayload, failure.1);
    writeln!(out, "{name}_composed_resource_error:").unwrap();
    emit_failure(out, failure.0, FailureKind::ResourceDescriptor, failure.1);
    writeln!(out, "{name}_composed_ok:").unwrap();
    match intent {
        TransferIntent::Validate => (),
        TransferIntent::Move(destination) => {
            let function = helper(aggregate, Mode::Move);
            writeln!(
                out,
                "  call void @{function}(ptr {source}, ptr {destination})"
            )
            .unwrap();
        }
        TransferIntent::Release => {
            let function = helper(aggregate, Mode::Drop);
            writeln!(out, "  call void @{function}(ptr {source})").unwrap();
        }
    }
    let suffix = match aggregate {
        AggregateTy::Record(_) => "aggregate_ok",
        AggregateTy::Enum(_) => "enum_ok",
        _ => unreachable!("nominal transfer continuation"),
    };
    writeln!(out, "  br label %{name}_{suffix}\n{name}_{suffix}:").unwrap();
}

fn scalar(
    out: &mut Emission,
    name: &str,
    scalar: hir::Ty,
    source: &str,
    destination: &str,
    mode: Mode,
) {
    if mode == Mode::Drop {
        return;
    }
    let storage = if matches!(scalar, hir::Ty::Bool | hir::Ty::Unit) {
        "i8"
    } else {
        ty(scalar)
    };
    writeln!(
        out,
        "  %{name}_value = load {storage}, ptr {source}, align 1"
    )
    .unwrap();
    if mode == Mode::Move {
        writeln!(
            out,
            "  store {storage} %{name}_value, ptr {destination}, align 1"
        )
        .unwrap();
        return;
    }
    match scalar {
        hir::Ty::Bool | hir::Ty::Unit => {
            let predicate = if scalar == hir::Ty::Bool { "ule" } else { "eq" };
            let bound = usize::from(scalar == hir::Ty::Bool);
            writeln!(
                out,
                "  %{name}_valid = icmp {predicate} i8 %{name}_value, {bound}"
            )
            .unwrap();
        }
        hir::Ty::Char => {
            writeln!(out, "  %{name}_below = icmp ule i32 %{name}_value, 1114111\n  %{name}_before_surrogate = icmp ult i32 %{name}_value, 55296\n  %{name}_after_surrogate = icmp ugt i32 %{name}_value, 57343\n  %{name}_not_surrogate = or i1 %{name}_before_surrogate, %{name}_after_surrogate\n  %{name}_valid = and i1 %{name}_below, %{name}_not_surrogate").unwrap();
        }
        _ => return,
    }
    writeln!(out, "  br i1 %{name}_valid, label %{name}_ok, label %{name}_error\n{name}_error:\n  ret i32 2\n{name}_ok:").unwrap();
}

fn resource(
    out: &mut Emission,
    name: &str,
    aggregate: AggregateTy,
    source: &str,
    destination: &str,
    mode: Mode,
) {
    let (family, carrier) = match aggregate {
        AggregateTy::ByteBuf => ("bytebuf", "{ i64, i32, i32 }"),
        AggregateTy::File => ("file", "{ i64, i64 }"),
        AggregateTy::Vector(ty) => {
            let tag = super::super::vector_type::element(ty).tag();
            match mode {
                Mode::Validate => {
                    writeln!(out, "  %{name}_status = call i32 @__oxid_vector_valid(ptr {source}, i32 {tag})\n  %{name}_valid = icmp eq i32 %{name}_status, 0\n  br i1 %{name}_valid, label %{name}_ok, label %{name}_error\n{name}_error:\n  ret i32 3\n{name}_ok:").unwrap();
                }
                Mode::Move => {
                    writeln!(out, "  %{name}_descriptor = load {{ i64, i32, i32 }}, ptr {source}, align 1\n  store {{ i64, i32, i32 }} %{name}_descriptor, ptr {destination}, align 1\n  store {{ i64, i32, i32 }} zeroinitializer, ptr {source}, align 1").unwrap();
                }
                Mode::Drop => {
                    writeln!(
                        out,
                        "  call void @__oxid_vector_drop(ptr {source}, i32 {tag})"
                    )
                    .unwrap();
                }
            }
            return;
        }
        _ => unreachable!("verified resource aggregate"),
    };
    match mode {
        Mode::Validate => {
            writeln!(out, "  %{name}_status = call i32 @__oxid_{family}_valid(ptr {source})\n  %{name}_valid = icmp eq i32 %{name}_status, 0\n  br i1 %{name}_valid, label %{name}_ok, label %{name}_error\n{name}_error:\n  ret i32 3\n{name}_ok:").unwrap();
        }
        Mode::Move => {
            writeln!(out, "  %{name}_descriptor = load {carrier}, ptr {source}, align 1\n  store {carrier} %{name}_descriptor, ptr {destination}, align 1\n  store {carrier} zeroinitializer, ptr {source}, align 1").unwrap();
        }
        Mode::Drop => writeln!(out, "  call void @__oxid_{family}_drop(ptr {source})").unwrap(),
    }
}

fn value(
    out: &mut Emission,
    name: &str,
    value: ValueTy,
    source: &str,
    destination: &str,
    mode: Mode,
) {
    match value {
        ValueTy::Scalar(ty) => scalar(out, name, ty, source, destination, mode),
        ValueTy::Owned(
            aggregate @ (AggregateTy::ByteBuf | AggregateTy::File | AggregateTy::Vector(_)),
        ) => resource(out, name, aggregate, source, destination, mode),
        ValueTy::Owned(aggregate @ (AggregateTy::Record(_) | AggregateTy::Enum(_))) => {
            let function = helper(aggregate, mode);
            match mode {
                Mode::Validate => writeln!(out, "  %{name}_status = call i32 @{function}(ptr {source})\n  %{name}_valid = icmp eq i32 %{name}_status, 0\n  br i1 %{name}_valid, label %{name}_ok, label %{name}_error\n{name}_error:\n  ret i32 %{name}_status\n{name}_ok:").unwrap(),
                Mode::Move => writeln!(out, "  call void @{function}(ptr {source}, ptr {destination})").unwrap(),
                Mode::Drop => writeln!(out, "  call void @{function}(ptr {source})").unwrap(),
            }
        }
        ValueTy::Owned(AggregateTy::FixedArray(array)) => {
            if mode == Mode::Drop {
                return;
            }
            // The existing representation stores one element-typed zero cell
            // for an empty array. A loop bounds text independently of length.
            let length = array.length().max(1);
            writeln!(out, "  br label %{name}_start\n{name}_start:\n  br label %{name}_loop\n{name}_loop:\n  %{name}_index = phi i64 [0, %{name}_start], [%{name}_next_index, %{name}_next]\n  %{name}_offset = mul i64 %{name}_index, {}\n  %{name}_source = getelementptr i8, ptr {source}, i64 %{name}_offset", array.stride()).unwrap();
            let destination = if mode == Mode::Move {
                writeln!(out, "  %{name}_destination = getelementptr i8, ptr {destination}, i64 %{name}_offset").unwrap();
                format!("%{name}_destination")
            } else {
                String::new()
            };
            scalar(
                out,
                &format!("{name}_element"),
                array.element(),
                &format!("%{name}_source"),
                &destination,
                mode,
            );
            writeln!(out, "  br label %{name}_next\n{name}_next:\n  %{name}_next_index = add i64 %{name}_index, 1\n  %{name}_finished = icmp eq i64 %{name}_next_index, {length}\n  br i1 %{name}_finished, label %{name}_done, label %{name}_loop\n{name}_done:").unwrap();
        }
    }
}

fn member(out: &mut Emission, name: &str, ty: ValueTy, offset: usize, mode: Mode) {
    let source = field_pointer(out, &format!("{name}_in"), "%source", offset);
    let destination = if mode == Mode::Move {
        field_pointer(out, &format!("{name}_out"), "%destination", offset)
    } else {
        String::new()
    };
    value(out, name, ty, &source, &destination, mode);
}

pub(super) fn emit_helpers(out: &mut Emission, plan: &ExecutionPlan<'_>) {
    if !plan.uses_enum_record_nesting() {
        return;
    }
    let declarations = plan.witness().declarations();
    for mode in [Mode::Validate, Mode::Move, Mode::Drop] {
        for aggregate in declarations
            .records()
            .iter()
            .map(|record| AggregateTy::Record(record.id()))
            .chain(
                declarations
                    .enums()
                    .enums()
                    .iter()
                    .map(|enumeration| AggregateTy::Enum(enumeration.id())),
            )
        {
            if !out.expand(Expansion::Transfer) {
                return;
            }
            let function = helper(aggregate, mode);
            let destination = if mode == Mode::Move {
                ", ptr %destination"
            } else {
                ""
            };
            writeln!(
                out,
                "\ndefine internal {} @{function}(ptr %source{destination}) {{\nentry:",
                mode.result()
            )
            .unwrap();
            match aggregate {
                AggregateTy::Record(record) => {
                    let fields = declarations.fields(record).expect("verified record");
                    if fields.is_empty() {
                        scalar(out, "empty", hir::Ty::Unit, "%source", "%destination", mode);
                    }
                    for (ordinal, field) in fields.iter().enumerate() {
                        if !out.expand(Expansion::Transfer) {
                            return;
                        }
                        member(
                            out,
                            &format!("field{ordinal}"),
                            field.value_ty(),
                            field.offset(),
                            mode,
                        );
                    }
                    writeln!(out, "  {}\n}}", mode.success()).unwrap();
                }
                AggregateTy::Enum(enumeration) => {
                    let variants = declarations
                        .enums()
                        .variants(enumeration)
                        .expect("verified enum");
                    writeln!(out, "  %tag = load i32, ptr %source, align 1\n  switch i32 %tag, label %invalid [").unwrap();
                    for variant in variants {
                        if !out.expand(Expansion::Transfer) {
                            return;
                        }
                        writeln!(
                            out,
                            "    i32 {}, label %variant{}",
                            variant.tag(),
                            variant.tag()
                        )
                        .unwrap();
                    }
                    writeln!(
                        out,
                        "  ]\ninvalid:\n  {}",
                        if mode == Mode::Validate {
                            "ret i32 1"
                        } else {
                            "unreachable"
                        }
                    )
                    .unwrap();
                    for variant in variants {
                        if !out.expand(Expansion::Transfer) {
                            return;
                        }
                        writeln!(out, "variant{}:", variant.tag()).unwrap();
                        if let Some(payload) = variant.value_payload() {
                            member(
                                out,
                                &format!("variant{}_payload", variant.tag()),
                                payload,
                                variant.payload_offset().expect("payload offset"),
                                mode,
                            );
                        }
                        if mode == Mode::Move {
                            writeln!(
                                out,
                                "  store i32 {}, ptr %destination, align 1",
                                variant.tag()
                            )
                            .unwrap();
                        }
                        writeln!(out, "  {}", mode.success()).unwrap();
                    }
                    writeln!(out, "}}").unwrap();
                }
                _ => unreachable!("nominal helper inventory"),
            }
        }
    }
}

#[cfg(test)]
#[path = "native_vector_composition_tests.rs"]
mod vector_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::oir::owned::source::resource_fixtures::checked_enum_project;

    #[test]
    fn record_enum_native_helpers_grow_with_declarations_instead_of_expanded_paths() {
        let mut source = String::from("enum Choice{Empty,Value(bool)}struct R0{value:Choice}");
        for id in 1..=10 {
            let previous = id - 1;
            write!(
                source,
                "struct R{id}{{first:R{previous},second:R{previous}}}"
            )
            .unwrap();
        }
        source.push_str("fn main()->i32{return 42;}");
        let checked = checked_enum_project(&source);
        let observed = run_array_observed(
            &checked.witness,
            Some(checked.entry),
            checked.project.sources(),
            NativeControl::default(),
        );
        let module = observed.result.unwrap();
        assert_eq!(
            module
                .matches("define internal i32 @__oxid_composed_validate_")
                .count(),
            12
        );
        assert_eq!(
            module
                .matches("define internal void @__oxid_composed_move_")
                .count(),
            12
        );
        assert_eq!(
            module
                .matches("define internal void @__oxid_composed_drop_")
                .count(),
            12
        );
        assert!(module.len() < 100_000, "{} bytes", module.len());
        for forbidden in [
            "memcpy",
            "inbounds",
            "noalias",
            "nonnull",
            "dereferenceable",
            "undef",
            "poison",
        ] {
            assert!(!module.contains(forbidden), "{forbidden}");
        }
        let metrics = observed.metrics;
        assert_eq!(metrics.count_bytes, module.len());
        assert_eq!(metrics.render_bytes, module.len());
        assert_eq!(
            metrics.count_expansions,
            metrics.transfer_cells + metrics.message_bytes
        );
        assert_eq!(
            metrics.count_expansion_kinds,
            metrics.render_expansion_kinds
        );
        assert!(metrics.metadata_peak <= Limits::DEFAULT.metadata_bytes);
        let exact = NativeControl {
            limits: Limits {
                ir_bytes: module.len(),
                ..Limits::DEFAULT
            },
            ..Default::default()
        };
        assert_eq!(
            run_array_observed(
                &checked.witness,
                Some(checked.entry),
                checked.project.sources(),
                exact
            )
            .result
            .unwrap(),
            module
        );
        let short = NativeControl {
            limits: Limits {
                ir_bytes: module.len() - 1,
                ..Limits::DEFAULT
            },
            ..Default::default()
        };
        let failed = run_array_observed(
            &checked.witness,
            Some(checked.entry),
            checked.project.sources(),
            short,
        );
        assert!(failed.result.unwrap_err().message.contains("LLVM bytes"));
        assert_eq!(failed.metrics.render_bytes, 0);
    }

    #[test]
    fn record_enum_native_metadata_and_output_allocations_remain_fallible() {
        let checked = checked_enum_project("enum State{Empty,Value(char)}struct Cell{state:State}fn main()->i32{let cell=Cell{state:State::Value('雪')};let Cell{state}=cell;match state{State::Empty=>{return 1;},State::Value(c)=>{if c=='雪'{return 42;}return 2;},}}");
        let baseline = run_array_observed(
            &checked.witness,
            Some(checked.entry),
            checked.project.sources(),
            NativeControl::default(),
        );
        assert!(baseline.result.is_ok());
        for failure in 0..baseline.metrics.allocation_attempts {
            let failed = run_array_observed(
                &checked.witness,
                Some(checked.entry),
                checked.project.sources(),
                NativeControl {
                    fail_after: Some(failure),
                    ..Default::default()
                },
            );
            let diagnostic = failed.result.unwrap_err();
            assert_eq!(diagnostic.code, "E0700");
            assert!(diagnostic.message.contains("allocation"));
            assert!(failed.metrics.failed_allocation.is_some());
            assert_eq!(failed.metrics.render_bytes, 0);
        }
    }

    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    #[test]
    #[ignore = "requires Linux x86_64 or ARM64 and LLVM 19.1.7"]
    fn record_enum_native_helpers_validate_active_cells_and_leave_padding_untouched() {
        use std::{
            path::PathBuf,
            process::Command,
            sync::atomic::{AtomicUsize, Ordering},
        };
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        struct Directory(PathBuf);
        impl Drop for Directory {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let directory = Directory(std::env::temp_dir().join(format!(
            "oxid-composed-cells-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        std::fs::create_dir(&directory.0).unwrap();
        let checked = checked_enum_project("enum Choice{Empty,Flag(bool),Letter(char),Text([char;2]),EmptyArray([char;0]),EmptyRecord(Empty)}struct State{first:Choice,second:Choice}struct Empty{}fn main()->i32{return 42;}");
        let plan = ExecutionPlan::build(&checked.witness).unwrap();
        // Independently laid out: two 12-byte enums, with four-byte tags and
        // eight-byte maximum payloads. Padding and inactive bytes are poison.
        assert_eq!(
            plan.witness()
                .declarations()
                .aggregate_layout(AggregateTy::Record(RecordId(0)))
                .unwrap()
                .size(),
            24
        );
        let mut output = Emission {
            text: Some(String::new()),
            ..Emission::count(MAX_IR_BYTES)
        };
        emit_helpers(&mut output, &plan);
        let mut module = format!(
            "target triple = \"{}\"\n{}",
            crate::frontend::build_config::NativeBuildSettings::new(false).target_triple(),
            output.text.unwrap()
        );
        module.push_str("\ndefine i32 @validate(ptr %source) {\n %status = call i32 @__oxid_composed_validate_r0(ptr %source)\n ret i32 %status\n}\ndefine void @move_active(ptr %source, ptr %destination) {\n call void @__oxid_composed_move_r0(ptr %source, ptr %destination)\n ret void\n}\ndefine void @drop_active(ptr %source) {\n call void @__oxid_composed_drop_r0(ptr %source)\n ret void\n}\n");
        let input = directory.0.join("cells.ll");
        let harness = directory.0.join("cells.c");
        std::fs::write(&input, module).unwrap();
        std::fs::write(
            &harness,
            r#"
#include <stdint.h>
#include <string.h>
extern int32_t validate(const unsigned char *);
extern void move_active(unsigned char *, unsigned char *);
extern void drop_active(unsigned char *);
#define CHECK(condition) do { if (!(condition)) return __LINE__; } while (0)
static void word(unsigned char *p, uint32_t value) { memcpy(p, &value, sizeof(value)); }
int main(void) {
    unsigned char source[24], before[24], destination[40];
    memset(source, 0xEE, sizeof(source));
    word(source, 0); word(source + 12, 1); source[16] = 1;
    CHECK(validate(source) == 0);
    memcpy(before, source, sizeof(source));
    memset(destination, 0xA5, sizeof(destination));
    move_active(source, destination + 8);
    CHECK(memcmp(source, before, sizeof(source)) == 0);
    for (unsigned i = 0; i < sizeof(destination); ++i) {
        unsigned char expected = 0xA5;
        if (i >= 8 && i < 12) expected = source[i - 8];
        if (i >= 20 && i < 25) expected = source[i - 8];
        CHECK(destination[i] == expected);
    }
    drop_active(source); CHECK(memcmp(source, before, sizeof(source)) == 0);
    source[16] = 2; CHECK(validate(source) == 2); source[16] = 1;
    word(source + 12, UINT32_MAX); CHECK(validate(source) == 1);
    word(source + 12, 1);
    word(source, 2); word(source + 4, 0xD800); CHECK(validate(source) == 2);
    word(source + 4, 0x110000); CHECK(validate(source) == 2);
    word(source + 4, 0x96EA); CHECK(validate(source) == 0);
    word(source, 3); word(source + 4, 0x96EA); word(source + 8, 0xDFFF);
    CHECK(validate(source) == 2); word(source + 8, 0x4EBA); CHECK(validate(source) == 0);
    word(source, 4); word(source + 4, 0); CHECK(validate(source) == 0);
    word(source + 4, UINT32_MAX); CHECK(validate(source) == 2);
    word(source, 5); source[4] = 0; CHECK(validate(source) == 0);
    source[4] = 1; CHECK(validate(source) == 2);
    return 0;
}
"#,
        )
        .unwrap();
        let clang = std::env::var_os("OXID_LLVM_BIN").map_or_else(
            || PathBuf::from("clang-19"),
            |path| PathBuf::from(path).join("clang"),
        );
        let version = Command::new(&clang).arg("--version").output().unwrap();
        assert!(
            version.status.success()
                && String::from_utf8_lossy(&version.stdout).contains("clang version 19.1.7")
        );
        for optimization in ["-O0", "-O2"] {
            let executable = directory.0.join(optimization);
            let built = Command::new(&clang)
                .args([optimization, "-Werror"])
                .arg(format!(
                    "--target={}",
                    crate::frontend::build_config::NativeBuildSettings::new(false).target_triple()
                ))
                .arg(&input)
                .arg(&harness)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(built.status.success(), "{built:?}");
            let run = Command::new(executable)
                .env_clear()
                .current_dir(&directory.0)
                .output()
                .unwrap();
            assert_eq!(
                (run.status.code(), run.stdout, run.stderr),
                (Some(0), Vec::new(), Vec::new()),
                "{optimization}"
            );
        }
    }
}
