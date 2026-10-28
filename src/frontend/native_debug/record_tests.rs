//! Source-owned metadata checks; actual debugger behavior has separate tests.
use super::*;
use crate::frontend::{
    build_config::ExecutionPolicy,
    declaration_index::{IndexLimits, WorkMeter},
    oir,
    project::budget::Allocator,
};

fn checked(project: &ProjectSources) -> oir::CheckedSourceProgram<'_> {
    oir::project::check_project_executable(
        project,
        IndexLimits::default(),
        &WorkMeter::default(),
        &mut Allocator::default(),
    )
    .unwrap()
}

fn project(text: &str) -> ProjectSources {
    ProjectSources::from_u8_index_test_files(&[("main.ox", text)])
}

#[test]
fn flat_copy_record_types_use_checked_padding_and_distinct_nominal_ids() {
    let project = project("#[derive(Copy)]struct Padded{tag:u8,wide:i64,huge:u128,last:i16}#[derive(Copy)]struct Point{x:i32,y:i32}#[derive(Copy)]struct Other{x:i32,y:i32}fn main()->i32{let p=Point{x:20,y:1};let q=p;return p.x+q.x+p.y+q.y;}");
    let checked = checked(&project);
    let records = checked.debug_records().unwrap();
    assert_eq!(records.len(), 3);
    assert_eq!(records.metadata_nodes(), Some(14));
    let padded = records.record(0).unwrap();
    assert_eq!((padded.size(), padded.align(), padded.len()), (48, 16, 4));
    for (index, (offset, size, align)) in [(0, 1, 1), (8, 8, 8), (16, 16, 16), (32, 2, 2)]
        .into_iter()
        .enumerate()
    {
        let field = padded.field(index).unwrap();
        assert_eq!(
            (field.offset, field.size, field.align),
            (offset, size, align)
        );
    }
    assert_eq!(records.record(1).unwrap().metadata_offset(), Some(6));
    assert_eq!(records.record(2).unwrap().metadata_offset(), Some(10));
    assert!(records.record(3).is_none());
    let marked = checked
        .native_debug_module_with_execution(false, ExecutionPolicy::Bounded, None)
        .unwrap();
    let module = add(&marked, &project, None, false, Some(records)).unwrap();
    let composites = module
        .lines()
        .filter(|line| line.contains("!DICompositeType("))
        .collect::<Vec<_>>();
    assert_eq!(composites.len(), 3);
    assert!(
        composites[0].contains("name: \"Padded\"")
            && composites[0].contains("size: 384, align: 128"),
        "{module}"
    );
    assert!(
        composites[1].starts_with("!28 = distinct !DICompositeType("),
        "{module}"
    );
    assert!(
        composites[2].starts_with("!32 = distinct !DICompositeType("),
        "{module}"
    );
    for (name, offset, size) in [
        ("tag", 0, 8),
        ("wide", 64, 64),
        ("huge", 128, 128),
        ("last", 256, 16),
    ] {
        let field = module
            .lines()
            .find(|line| {
                line.contains("!DIDerivedType(") && line.contains(&format!("name: \"{name}\""))
            })
            .unwrap();
        assert!(
            field.contains(&format!("size: {size},"))
                && field.ends_with(&format!("offset: {offset})")),
            "{field}"
        );
    }
}

#[test]
fn unsupported_records_are_omitted_whole_without_rejecting_the_program() {
    let project = project("#[derive(Copy)]struct Point{x:i32}#[derive(Copy)]struct Boolean{x:i32,flag:bool}#[derive(Copy)]struct Unit{x:i32,nothing:()}#[derive(Copy)]struct Character{x:i32,letter:char}#[derive(Copy)]struct Nested{point:Point}#[derive(Copy)]struct Empty{}#[derive(Copy)]struct Generic<T:Copy>{value:T}struct Moved{x:i32}struct Array{items:[i32;1]}fn main()->i32{let boolean=Boolean{x:1,flag:true};let unit=Unit{x:2,nothing:()};let character=Character{x:3,letter:'雪'};let nested=Nested{point:Point{x:4}};let empty=Empty{};let generic=Generic::<i32>{value:5};let moved=Moved{x:6};let array=Array{items:[7]};return 42;}");
    let checked = checked(&project);
    let records = checked.debug_records().unwrap();
    let marked = checked
        .native_debug_module_with_execution(false, ExecutionPolicy::Bounded, None)
        .unwrap();
    let module = add(&marked, &project, None, false, Some(records)).unwrap();
    assert_eq!(
        module
            .lines()
            .filter(|line| line.contains("!DICompositeType("))
            .count(),
        1,
        "{module}"
    );
    for name in [
        "boolean",
        "unit",
        "character",
        "nested",
        "empty",
        "generic",
        "moved",
        "array",
    ] {
        assert!(
            !module.lines().any(|line| line.contains("!DILocalVariable(")
                && line.contains(&format!("name: \"{name}\""))),
            "{name}: {module}"
        );
    }
    let plain = checked.native_module().unwrap();
    let stripped = marked
        .lines()
        .filter(|line| !line.starts_with("; oxid.debug."))
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    assert_eq!(
        stripped, plain,
        "debug markers changed executable instructions"
    );
}

#[test]
fn copy_record_markers_require_exact_lines_and_known_checked_types() {
    for marker in [
        "0 1 2 3 0 16 address %owners",
        "0 1 2 3 2 0 address %owners",
    ] {
        assert!(records::valid_marker(marker), "{marker}");
    }
    for marker in [
        "00 1 2 3 0 16 address %owners",
        "0  1 2 3 0 16 address %owners",
        "0\t1 2 3 0 16 address %owners",
        "0 1 2 3 0 16 address %owners ",
        "0 1 2 3 0 16 address %owners ; forged",
        "0 1 2 3 0 16 value %owners",
        "0 1 2 3 0 16 address %o1",
        "0 1 2 3 0 16 address %owners\n",
        "184467440737095516160 1 2 3 0 16 address %owners",
    ] {
        assert!(!records::valid_marker(marker), "{marker}");
    }
    let project =
        project("#[derive(Copy)]struct Point{x:i32}fn main()->i32{let p=Point{x:42};return p.x;}");
    let checked = checked(&project);
    let marked = checked
        .native_debug_module_with_execution(false, ExecutionPolicy::Bounded, None)
        .unwrap();
    let record = marked
        .lines()
        .find(|line| line.starts_with(RECORD_VARIABLE))
        .unwrap();
    for invalid in [
        format!("{record} ; forged"),
        record.replacen("address %owners", "address %o0", 1),
    ] {
        let error = add(
            &marked.replacen(record, &invalid, 1),
            &project,
            None,
            false,
            checked.debug_records(),
        )
        .unwrap_err();
        assert_eq!((error.code, error.stage), ("E0700", "native-debug-info"));
    }
    let mut fields = record.split(' ').collect::<Vec<_>>();
    fields[5] = "999999";
    let invalid = fields.join(" ");
    assert!(add(
        &marked.replacen(record, &invalid, 1),
        &project,
        None,
        false,
        checked.debug_records()
    )
    .is_err());
    assert!(add(&marked, &project, None, false, None).is_err());
    let string = "@text = private constant [52 x i8] c\"; oxid.debug.record 0 1 2 999999 0 0 address %owners\"\n";
    let safe = format!("{string}{marked}");
    let module = add(&safe, &project, None, false, checked.debug_records()).unwrap();
    assert!(module.starts_with(string));
}

#[test]
fn record_metadata_control_shares_the_existing_scratch_and_output_caps() {
    let project = project("#[derive(Copy)]struct Point{x:i32,y:i32}fn main()->i32{let p=Point{x:20,y:22};return p.x+p.y;}");
    let checked = checked(&project);
    let records = checked.debug_records();
    assert!(records::control_bytes() > 0);
    assert!(records::control_bytes() < MAX_DEBUG_IR_BYTES);
    let function = &project.try_file_ast(SourceFileId(0)).unwrap().functions[0];
    let mut next = 100;
    assert!(Scopes::new(function, 99, &mut next, MAX_DEBUG_IR_BYTES).is_err());
    assert_eq!(
        next, 100,
        "scratch denial must precede metadata-ID mutation"
    );
    assert!(Scopes::new(function, 99, &mut next, usize::MAX).is_err());
    assert_eq!(next, 100);
    let marked = checked
        .native_debug_module_with_execution(false, ExecutionPolicy::Bounded, None)
        .unwrap();
    let complete = add(&marked, &project, None, false, records).unwrap();
    for maximum in [complete.len(), complete.len() - 1] {
        let mut count = Output {
            length: 0,
            maximum,
            text: None,
            exceeded: false,
        };
        assert_eq!(
            render(&marked, &project, None, false, records, &mut count).is_ok(),
            maximum == complete.len()
        );
        assert!(count.length <= maximum);
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn scope_scratch_exact_boundary_reserves_only_after_admission_and_preserves_ids_on_denial() {
    use std::mem::{align_of, size_of};
    let project = project("#[derive(Copy)]struct Point{x:i32,y:i32}fn main()->i32{let p=Point{x:20,y:22};return p.x+p.y;}");
    let function = &project.try_file_ast(SourceFileId(0)).unwrap().functions[0];
    // Independently derived source inventory: one lexical block and one named
    // let binding. Span=24B, ID=8B, Option<usize>=16B on the qualified host.
    assert_eq!(function.blocks.len(), 1);
    assert_eq!(
        (size_of::<LexicalScope>(), align_of::<LexicalScope>()),
        (48, 8)
    );
    assert_eq!(
        (size_of::<BindingScope>(), align_of::<BindingScope>()),
        (32, 8)
    );
    let scope_payload = 48 + 32;
    for remaining in [scope_payload, scope_payload - 1] {
        let reservations = SCOPE_RESERVATIONS.with(std::cell::Cell::get);
        let mut next = 100;
        let result = Scopes::new(function, 99, &mut next, MAX_DEBUG_IR_BYTES - remaining);
        assert_eq!(result.is_ok(), remaining == scope_payload);
        let attempted = SCOPE_RESERVATIONS.with(std::cell::Cell::get) - reservations;
        if remaining == scope_payload {
            let scopes = result.unwrap();
            assert_eq!(scopes.blocks.len(), 1);
            assert_eq!(scopes.bindings.len(), 1);
            assert_eq!(attempted, 2);
            assert_eq!(next, 101);
        } else {
            assert_eq!(
                attempted, 0,
                "both actual reservation sites follow scratch admission"
            );
            assert_eq!(next, 100, "scratch denial precedes metadata-ID mutation");
        }
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn source_record_ordinals_ignore_native_fuel_owned_result_and_preceding_slice_length() {
    let project = project("#[derive(Copy)]struct Point{x:i32,y:i32}fn pass(prefix:&[i32],lead:i32,incoming:Point,suffix:i32)->Point{let original=incoming;let mut changed=original;changed.x=changed.x+prefix[0]+lead;changed.y=changed.y+prefix[1]+suffix;return changed;}fn main()->i32{let prefix=[2,3];let point=Point{x:17,y:1};let returned=pass(&prefix,5,point,7);if returned.x==24 && returned.y==11{return 0;}return 1;}");
    let checked = checked(&project);
    for execution in [ExecutionPolicy::Bounded, ExecutionPolicy::Hosted] {
        let marked = checked
            .native_debug_module_with_execution(true, execution, None)
            .unwrap();
        assert!(marked.lines().any(|line| line == "define internal void @__oxid_owned_fn_0(ptr %fuel, ptr %result, ptr %arg0, i32 %arg0_length, i32 %arg1, ptr %arg2, i32 %arg3) noinline {"), "{marked}");
        let module = add(&marked, &project, None, false, checked.debug_records()).unwrap();
        for (name, argument) in [("lead", 2), ("incoming", 3), ("suffix", 4)] {
            assert!(
                module.contains(&format!(
                    "!DILocalVariable(name: \"{name}\", arg: {argument},"
                )),
                "{name}: {module}"
            );
        }
        for name in ["prefix", "fuel", "result", "arg0_length"] {
            assert!(
                !module.lines().any(|line| line.contains("!DILocalVariable(")
                    && line.contains(&format!("name: \"{name}\", arg:"))),
                "{name}: {module}"
            );
        }
    }
}
