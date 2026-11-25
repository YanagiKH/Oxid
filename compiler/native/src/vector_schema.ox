use alloc::vec::len;
use crate::model::Compiler;
use crate::model::aligned_width;
use crate::model::copy_record_shape;
use crate::model::is_record;
use crate::model::record_index;
use crate::model::record_type;
use crate::model::vector_runtime_tag;
use crate::emit::append;
use crate::emit::number;

fn leaf(c: &mut Compiler, ty: i32, dense: i32, owner: i32) -> () {
    let comma = b",\n";
    let prefix = b"{ i32, i32, i32 } { i32 ";
    let member = b", i32 ";
    let end = b" }";
    if dense > 0 { append(&mut *c, 0, &comma); }
    append(&mut *c, 0, &prefix); number(&mut *c, 0, dense);
    append(&mut *c, 0, &member); number(&mut *c, 0, owner);
    append(&mut *c, 0, &member); number(&mut *c, 0, vector_runtime_tag(ty));
    append(&mut *c, 0, &end);
    return;
}

fn leaves(c: &mut Compiler, ty: i32, dense: i32, owner: i32) -> () {
    if !is_record(ty) { leaf(&mut *c, ty, dense, owner); return; }
    let row = c.records[record_index(ty)];
    if row.field_count == 0 { leaf(&mut *c, 3, dense, owner); return; }
    let mut offset = owner;
    let mut packed = dense;
    for index in 0..row.field_count {
        let field = c.fields[row.first_field + index];
        let shape = copy_record_shape(&*c, field.ty);
        offset = aligned_width(offset, shape.alignment);
        leaves(&mut *c, field.ty, packed, offset);
        offset = offset + shape.width;
        packed = packed + shape.dense;
    }
    return;
}

fn leaf_table(c: &mut Compiler, ordinal: i32) -> () {
    let prefix = b"@__oxid_record_vector_leaves_";
    let header = b" = private constant [";
    let element = b" x { i32, i32, i32 }] [\n";
    let finish = b"\n], align 4\n";
    let ty = record_type(ordinal);
    let shape = copy_record_shape(&*c, ty);
    append(&mut *c, 0, &prefix); number(&mut *c, 0, ordinal);
    append(&mut *c, 0, &header); number(&mut *c, 0, shape.leaves);
    append(&mut *c, 0, &element); leaves(&mut *c, ty, 0, 0);
    append(&mut *c, 0, &finish);
    return;
}

fn schema(c: &mut Compiler, ordinal: i32) -> () {
    let start = b" { i32 ";
    let i32_member = b", i32 ";
    let leaf_table_name = b", ptr @__oxid_record_vector_leaves_";
    let end = b" }";
    let ty = record_type(ordinal);
    let shape = copy_record_shape(&*c, ty);
    append(&mut *c, 0, &start); number(&mut *c, 0, vector_runtime_tag(ty));
    append(&mut *c, 0, &i32_member); number(&mut *c, 0, shape.dense);
    append(&mut *c, 0, &i32_member); number(&mut *c, 0, shape.width);
    append(&mut *c, 0, &i32_member); number(&mut *c, 0, shape.leaves);
    append(&mut *c, 0, &leaf_table_name); number(&mut *c, 0, ordinal);
    append(&mut *c, 0, &end);
    return;
}

fn catalog(c: &mut Compiler, count: i32) -> () {
    let prefix = b"@__oxid_record_vector_schemas = constant [";
    let times = b" x ";
    let layout = b"{ i32, i32, i32, i32, ptr }";
    let open = b"] [\n";
    let comma = b",\n";
    let zero = b" zeroinitializer";
    let finish = b"\n], align 8\n";
    append(&mut *c, 0, &prefix); number(&mut *c, 0, count);
    append(&mut *c, 0, &times); append(&mut *c, 0, &layout); append(&mut *c, 0, &open);
    for index in 0..count {
        if index > 0 { append(&mut *c, 0, &comma); }
        append(&mut *c, 0, &layout);
        let row = c.records[index];
        if row.copy { schema(&mut *c, index); }
        else { append(&mut *c, 0, &zero); }
    }
    append(&mut *c, 0, &finish);
    return;
}

pub fn write(c: &mut Compiler) -> () {
    let mut needed = c.uses_record_slices;
    for index in 8..len(&*c.vector_types) {
        if c.vector_types[index] >= 0 { needed = true; }
    }
    if !needed { return; }
    let count = len(&*c.records);
    let prefix = b"@__oxid_record_vector_schema_count = constant i32 ";
    let end = b", align 4\n";
    append(&mut *c, 0, &prefix); number(&mut *c, 0, count); append(&mut *c, 0, &end);
    for index in 0..count {
        let row = c.records[index];
        if row.copy { leaf_table(&mut *c, index); }
    }
    catalog(&mut *c, count);
    return;
}
