//! Static emitted-text oracles. These do not invoke LLVM or execute a program.
use super::*;

fn emitted(aggregate: AggregateTy, mode: Mode) -> String {
    let mut output = Emission {
        text: Some(String::new()),
        ..Emission::count(MAX_IR_BYTES)
    };
    resource(
        &mut output,
        "cell",
        aggregate,
        "%source",
        "%destination",
        mode,
        false,
    );
    assert!(!output.exceeded);
    let text = output.text.unwrap();
    assert_eq!(output.len, text.len());
    text
}

#[test]
fn composed_vector_resources_emit_literal_element_tags_and_three_word_moves() {
    for (ty, tag) in [
        (hir::Ty::I128, 16),
        (hir::Ty::U128, 17),
        (hir::Ty::F64, 14),
        (hir::Ty::Unit, 2),
    ] {
        let aggregate = AggregateTy::Vector(ty);
        assert_eq!(
            emitted(aggregate, Mode::Validate),
            format!("  %cell_status = call i32 @__oxid_vector_valid(ptr %source, i32 {tag})\n  %cell_valid = icmp eq i32 %cell_status, 0\n  br i1 %cell_valid, label %cell_ok, label %cell_error\ncell_error:\n  ret i32 3\ncell_ok:\n")
        );
        assert_eq!(
            emitted(aggregate, Mode::Move),
            "  %cell_descriptor = load { i64, i32, i32 }, ptr %source, align 1\n  store { i64, i32, i32 } %cell_descriptor, ptr %destination, align 1\n  store { i64, i32, i32 } zeroinitializer, ptr %source, align 1\n"
        );
        assert_eq!(
            emitted(aggregate, Mode::Drop),
            format!("  call void @__oxid_vector_drop(ptr %source, i32 {tag})\n")
        );
    }
}

#[test]
fn composed_vector_value_dispatch_moves_only_the_descriptor() {
    let mut output = Emission {
        text: Some(String::new()),
        ..Emission::count(MAX_IR_BYTES)
    };
    value(
        &mut output,
        "cell",
        ValueTy::Owned(AggregateTy::Vector(hir::Ty::I128)),
        "%source",
        "%destination",
        Mode::Move,
        false,
    );
    let text = output.text.unwrap();
    assert_eq!(
        text,
        emitted(AggregateTy::Vector(hir::Ty::I128), Mode::Move)
    );
    assert_eq!(text.lines().count(), 3);
    assert!(!text.contains("i128"));
    assert!(!text.contains("memcpy"));
}

#[test]
fn composed_predecessor_bytebuf_and_file_resource_text_is_unchanged() {
    for (aggregate, family, carrier) in [
        (AggregateTy::ByteBuf, "bytebuf", "{ i64, i32, i32 }"),
        (AggregateTy::File, "file", "{ i64, i64 }"),
    ] {
        assert_eq!(
            emitted(aggregate, Mode::Validate),
            format!("  %cell_status = call i32 @__oxid_{family}_valid(ptr %source)\n  %cell_valid = icmp eq i32 %cell_status, 0\n  br i1 %cell_valid, label %cell_ok, label %cell_error\ncell_error:\n  ret i32 3\ncell_ok:\n")
        );
        assert_eq!(
            emitted(aggregate, Mode::Move),
            format!("  %cell_descriptor = load {carrier}, ptr %source, align 1\n  store {carrier} %cell_descriptor, ptr %destination, align 1\n  store {carrier} zeroinitializer, ptr %source, align 1\n")
        );
        assert_eq!(
            emitted(aggregate, Mode::Drop),
            format!("  call void @__oxid_{family}_drop(ptr %source)\n")
        );
    }
}
