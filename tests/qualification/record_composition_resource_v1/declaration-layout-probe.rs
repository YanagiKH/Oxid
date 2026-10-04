#![allow(dead_code)]
const MAX_CONTAINMENT_DEPTH: usize = 64;
struct SourceFileId(usize);
struct Span {
    file: SourceFileId,
    start: usize,
    end: usize,
}
enum Ty {
    Bool,
    I32,
    Unit,
}
struct RecordId(usize);
struct FixedArrayTy {
    element: Ty,
    length: u16,
}
enum AggregateTy {
    Record(RecordId),
    FixedArray(FixedArrayTy),
}
enum BorrowedTy {
    Exact(AggregateTy),
    ScalarSlice(Ty),
}
struct FieldId {
    record: RecordId,
    index: usize,
}
struct OwnerPlaceId(usize);
enum ValueTy {
    Scalar(Ty),
    Owned(AggregateTy),
}
enum BorrowKind {
    Shared,
    Exclusive,
}
enum ParameterTy {
    Value(ValueTy),
    Reference {
        referent: BorrowedTy,
        kind: BorrowKind,
    },
}
struct RawRecordDecl {
    id: RecordId,
    span: Span,
    fields: Vec<RawFieldDecl>,
}
struct RawFieldDecl {
    id: FieldId,
    // Raw input can express unsupported fields so validation, not a trusted
    // producer, is responsible for excluding nested records and references.
    ty: ParameterTy,
    span: Span,
}
struct Layout {
    size: usize,
    align: usize,
}
struct FieldDecl {
    id: FieldId,
    ty: ValueTy,
    span: Span,
    offset: usize,
}
struct RecordDecl {
    id: RecordId,
    span: Span,
    field_start: usize,
    field_end: usize,
    layout: Layout,
    width: usize,
}
struct DeclarationUsage {
    records: usize,
    fields: usize,
    table_bytes: usize,
    layout_bytes: usize,
}
struct Declarations {
    records: Vec<RecordDecl>,
    fields: Vec<FieldDecl>,
    usage: DeclarationUsage,
}
struct ContainmentSummary {
    layout: Layout,
    width: usize,
    depth: usize,
}
struct ScalarLeaf {
    offset: usize,
    ty: Ty,
}
struct ScalarLeaves<'a> {
    declarations: &'a Declarations,
    stack: [Option<(ValueTy, usize, usize)>; MAX_CONTAINMENT_DEPTH + 1],
    length: usize,
}
struct LocalId(usize);
struct Operand {
    local: LocalId,
    span: Span,
}
enum FieldInitializer {
    Scalar(Operand),
    // Must name a complete ordinary Temporary; producer stages at evaluation
    // time using MoveInitialize. The verifier independently enforces consumption.
    Owned(OwnerPlaceId),
}
struct FunctionCounts {
    locals: usize,
    places: usize,
    owners: usize,
    references: usize,
    parameters: usize,
    calls: usize,
    loans: usize,
    blocks: usize,
    edges: usize,
    statements: usize,
    merges: usize,
    descriptor_arguments: usize,
    preparations: usize,
    constructed_fields: usize,
    constructed_elements: usize,
    composite_fields: usize,
    projection_fields: usize,
    diagnostic_origins: usize,
    max_constructor_fields: usize,
    ownership_active: bool,
}
struct BindingId(usize);
struct Record {
    id: RecordId,
    name_span: Span,
    span: Span,
    fields: Vec<Field>,
    end: Span,
}
struct Field {
    id: FieldId,
    ty: ValueTy,
    name_span: Span,
    span: Span,
}
enum AccessBase {
    Owner(BindingId),
    Reference {
        binding: BindingId,
        kind: BorrowKind,
    },
}
struct Projection {
    base: AccessBase,
    field: FieldId,
    path: Vec<FieldId>,
}
fn main(){
println!("{} {} {}", "SourceFileId", std::mem::size_of::<SourceFileId>(), std::mem::align_of::<SourceFileId>());
println!("{} {} {}", "Span", std::mem::size_of::<Span>(), std::mem::align_of::<Span>());
println!("{} {} {}", "Ty", std::mem::size_of::<Ty>(), std::mem::align_of::<Ty>());
println!("{} {} {}", "RecordId", std::mem::size_of::<RecordId>(), std::mem::align_of::<RecordId>());
println!("{} {} {}", "FixedArrayTy", std::mem::size_of::<FixedArrayTy>(), std::mem::align_of::<FixedArrayTy>());
println!("{} {} {}", "AggregateTy", std::mem::size_of::<AggregateTy>(), std::mem::align_of::<AggregateTy>());
println!("{} {} {}", "BorrowedTy", std::mem::size_of::<BorrowedTy>(), std::mem::align_of::<BorrowedTy>());
println!("{} {} {}", "FieldId", std::mem::size_of::<FieldId>(), std::mem::align_of::<FieldId>());
println!("{} {} {}", "OwnerPlaceId", std::mem::size_of::<OwnerPlaceId>(), std::mem::align_of::<OwnerPlaceId>());
println!("{} {} {}", "ValueTy", std::mem::size_of::<ValueTy>(), std::mem::align_of::<ValueTy>());
println!("{} {} {}", "BorrowKind", std::mem::size_of::<BorrowKind>(), std::mem::align_of::<BorrowKind>());
println!("{} {} {}", "ParameterTy", std::mem::size_of::<ParameterTy>(), std::mem::align_of::<ParameterTy>());
println!("{} {} {}", "RawRecordDecl", std::mem::size_of::<RawRecordDecl>(), std::mem::align_of::<RawRecordDecl>());
println!("{} {} {}", "RawFieldDecl", std::mem::size_of::<RawFieldDecl>(), std::mem::align_of::<RawFieldDecl>());
println!("{} {} {}", "Layout", std::mem::size_of::<Layout>(), std::mem::align_of::<Layout>());
println!("{} {} {}", "FieldDecl", std::mem::size_of::<FieldDecl>(), std::mem::align_of::<FieldDecl>());
println!("{} {} {}", "RecordDecl", std::mem::size_of::<RecordDecl>(), std::mem::align_of::<RecordDecl>());
println!("{} {} {}", "DeclarationUsage", std::mem::size_of::<DeclarationUsage>(), std::mem::align_of::<DeclarationUsage>());
println!("{} {} {}", "Declarations", std::mem::size_of::<Declarations>(), std::mem::align_of::<Declarations>());
println!("{} {} {}", "ContainmentSummary", std::mem::size_of::<ContainmentSummary>(), std::mem::align_of::<ContainmentSummary>());
println!("{} {} {}", "ScalarLeaf", std::mem::size_of::<ScalarLeaf>(), std::mem::align_of::<ScalarLeaf>());
println!("{} {} {}", "ScalarLeaves", std::mem::size_of::<ScalarLeaves<'_>>(), std::mem::align_of::<ScalarLeaves<'_>>());
println!("{} {} {}", "LocalId", std::mem::size_of::<LocalId>(), std::mem::align_of::<LocalId>());
println!("{} {} {}", "Operand", std::mem::size_of::<Operand>(), std::mem::align_of::<Operand>());
println!("{} {} {}", "FieldInitializer", std::mem::size_of::<FieldInitializer>(), std::mem::align_of::<FieldInitializer>());
println!("{} {} {}", "FunctionCounts", std::mem::size_of::<FunctionCounts>(), std::mem::align_of::<FunctionCounts>());
println!("{} {} {}", "BindingId", std::mem::size_of::<BindingId>(), std::mem::align_of::<BindingId>());
println!("{} {} {}", "Record", std::mem::size_of::<Record>(), std::mem::align_of::<Record>());
println!("{} {} {}", "Field", std::mem::size_of::<Field>(), std::mem::align_of::<Field>());
println!("{} {} {}", "AccessBase", std::mem::size_of::<AccessBase>(), std::mem::align_of::<AccessBase>());
println!("{} {} {}", "Projection", std::mem::size_of::<Projection>(), std::mem::align_of::<Projection>());
println!("{} {} {}", "Option<Projection>", std::mem::size_of::<Option<Projection>>(), std::mem::align_of::<Option<Projection>>());
println!("{} {} {}", "(FieldId, Operand)", std::mem::size_of::<(FieldId, Operand)>(), std::mem::align_of::<(FieldId, Operand)>());
println!("{} {} {}", "Option<(ValueTy, usize, usize)>", std::mem::size_of::<Option<(ValueTy, usize, usize)>>(), std::mem::align_of::<Option<(ValueTy, usize, usize)>>());
println!("{} {} {}", "[Option<(ValueTy, usize, usize)>;65]", std::mem::size_of::<[Option<(ValueTy, usize, usize)>;65]>(), std::mem::align_of::<[Option<(ValueTy, usize, usize)>;65]>());
println!("{} {} {}", "(usize,usize)", std::mem::size_of::<(usize,usize)>(), std::mem::align_of::<(usize,usize)>());
println!("{} {} {}", "(FieldId, FieldInitializer)", std::mem::size_of::<(FieldId, FieldInitializer)>(), std::mem::align_of::<(FieldId, FieldInitializer)>());
}
