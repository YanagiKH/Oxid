"""Exact current index-scratch successor; all predecessor fixtures are immutable.

Only the two expected scratch formulas receive independently modeled additions.
The retained 21 test names, work values, payloads, caps and fault ordering stay.
No Rust test is added under the retained module-wide selector.
"""
import hashlib

RESOURCE = "archive/resource/resource-review-tests.rs"
MATERIALIZED = "src/frontend/declaration_index/reviewer_resource.rs"
PREDECESSOR_BYTES = 25291
PREDECESSOR_SHA = '1b717bef7b1d6ffebcab1fe245f60ef95ba2b4819bccddad90de078fcee05831'
DERIVED_BYTES = 30917
DERIVED_SHA = 'c6b1020b89464b85f61429b8da46dfe74d6b9321dd11706e8646595023b352be'
SEAMS = ((b'assert_eq!(plan.scratch,scratch+FIXED_SCRATCH as u64);', b'assert_eq!(plan.scratch,scratch+FIXED_SCRATCH as u64+current_vector_review_fixed_growth() as u64);'), (b'let scratch=8+FIXED_SCRATCH as u64;', b'let scratch=8+FIXED_SCRATCH as u64+current_vector_review_fixed_growth() as u64;'))
MODEL = b"\n// Current fixed additions are independent of the retained 4096-byte bank.\n// These are layout/accounting witnesses only; none constructs an index or\n// grants declaration/source authority. All preceding controls remain intact.\n#[allow(dead_code)]\n#[repr(C, align(8))]\nstruct VectorReviewPriorAdmission<'s> {\n    anchors: [u32; 9],\n    inventory: [u8; 4],\n    source: std::marker::PhantomData<&'s Span>,\n}\n#[allow(dead_code)]\n#[repr(C, align(8))]\nstruct VectorReviewCurrentAdmission<'s> {\n    anchors: [u32; 10],\n    inventory: [u8; 8],\n    source: std::marker::PhantomData<&'s Span>,\n}\n#[allow(dead_code)]\nstruct VectorReviewPriorTable<'s> {\n    sources: SourceOwner<'s>,\n    originals: Vec<OriginalRow>,\n    original_order: Vec<u32>,\n    functions: Vec<FunctionRow>,\n    records: Vec<RecordRow>,\n    fields: Vec<FieldRow>,\n    enums: Vec<EnumRow>,\n    variants: Vec<VariantRow>,\n    modules: Vec<ModuleRow>,\n    children: Vec<u32>,\n    imports: Vec<ImportRow>,\n    aliases: Vec<AliasCell>,\n    alias_order: Vec<u32>,\n    root_main: u32,\n    candidate_source_origin: CandidateOrigin,\n    builtins: VectorReviewPriorAdmission<'s>,\n}\n#[allow(dead_code)]\nstruct VectorReviewCurrentTable<'s> {\n    sources: SourceOwner<'s>,\n    originals: Vec<OriginalRow>,\n    original_order: Vec<u32>,\n    functions: Vec<FunctionRow>,\n    records: Vec<RecordRow>,\n    fields: Vec<FieldRow>,\n    enums: Vec<EnumRow>,\n    variants: Vec<VariantRow>,\n    modules: Vec<ModuleRow>,\n    children: Vec<u32>,\n    imports: Vec<ImportRow>,\n    aliases: Vec<AliasCell>,\n    alias_order: Vec<u32>,\n    root_main: u32,\n    candidate_source_origin: CandidateOrigin,\n    has_element_ranges: bool,\n    builtins: VectorReviewCurrentAdmission<'s>,\n}\n#[allow(dead_code)]\nstruct VectorReviewQuery<'i, 's> {\n    tables: &'i VectorReviewCurrentTable<'s>,\n    work: &'i WorkMeter,\n}\n#[allow(dead_code)]\nstruct VectorReviewSyntaxFeatures {\n    element_ranges: bool,\n    generic_slices: bool,\n    source_slices: bool,\n}\n#[allow(dead_code)]\n#[repr(C)]\nstruct VectorReviewCatalogTransports {\n    // One discriminant byte plus the complete eight-byte packed payload.\n    admission_legacy_choice: [u8; 1 + 8],\n    admission_set_return: [u8; 1 + 8],\n    with_experimental_receiver: [u8; 1 + 8],\n    with_experimental_legacy: [u8; 1 + 8],\n    legacy_receiver: [u8; 1 + 8],\n    legacy_return: [u8; 1 + 8],\n    experimental_receiver: [u8; 1 + 8],\n    set_caller_result: [u8; 1 + 8],\n    admission_inventory_argument: [u8; 8],\n    with_experimental_inventory: [u8; 8],\n    empty_return_inventory: [u8; 8],\n    inventory_caller_result: [u8; 8],\n}\n\nfn current_vector_review_fixed_growth() -> usize {\n    let old_admission = size_of::<VectorReviewPriorAdmission<'static>>();\n    let new_admission = size_of::<VectorReviewCurrentAdmission<'static>>();\n    let query = size_of::<VectorReviewQuery<'static, 'static>>();\n    let old_table = size_of::<VectorReviewPriorTable<'static>>();\n    let new_table = size_of::<VectorReviewCurrentTable<'static>>();\n    assert_eq!(new_admission, size_of::<BuiltinAdmission<'static>>());\n    assert_eq!(query, size_of::<QuerySession<'static, 'static>>());\n    assert_eq!(new_table, size_of::<Tables<'static>>());\n    // The independent admission temporary and later query cannot coexist;\n    // the complete admission held in the table is still charged separately.\n    let header_growth = (new_table - old_table)\n        + std::cmp::max(new_admission, query) - std::cmp::max(old_admission, query);\n\n    // The syntax model is a sum of complete simultaneously priced roles, not\n    // one padded struct and not spare words from the historical fixed bank.\n    let syntax_features = size_of::<VectorReviewSyntaxFeatures>()\n        + size_of::<&mut VectorReviewSyntaxFeatures>()\n        + size_of::<Option<&SourceFile>>()\n        + size_of::<&Option<&SourceFile>>()\n        + size_of::<[bool; 2]>()\n        + size_of::<[&mut bool; 2]>();\n    let syntax_subtotals = size_of::<usize>() + size_of::<usize>()\n        + size_of::<[Option<usize>; 2]>();\n    let syntax_controls = syntax_features + syntax_subtotals;\n    let catalog_controls = size_of::<VectorReviewCatalogTransports>();\n    assert_eq!(size_of::<VectorReviewSyntaxFeatures>(), size_of::<ast::SyntaxFeatures>());\n    assert_eq!(size_of::<BuiltinSet>(), 1 + 8);\n    assert_eq!(std::mem::align_of::<BuiltinSet>(), 1);\n    assert_eq!(size_of::<experimental::Inventory>(), 8);\n    assert_eq!(std::mem::align_of::<experimental::Inventory>(), 1);\n    assert_eq!(catalog_controls, 8 * (1 + 8) + 4 * 8);\n\n    // Expected quantities come from the independent models above. These\n    // comparisons catch an omitted role in either implementation subtotal.\n    assert_eq!(header_growth, VECTOR_INDEX_HEADER_GROWTH);\n    assert_eq!(syntax_features, ast::SYNTAX_FEATURE_CONTROL_BYTES);\n    assert_eq!(syntax_controls, SYNTAX_FEATURE_CONTROL_BYTES);\n    assert_eq!(catalog_controls, crate::frontend::builtin_catalog::builtin_set_control_bytes());\n    let growth = header_growth + syntax_controls + catalog_controls;\n    assert_eq!(CURRENT_BASE_SCRATCH, FIXED_SCRATCH + growth);\n    if size_of::<usize>() == 8 {\n        assert_eq!((old_admission, new_admission, query), (40, 48, 16));\n        assert_eq!((old_table, new_table), (384, 392));\n        assert_eq!((syntax_features, syntax_subtotals), (45, 48));\n        assert_eq!((header_growth, syntax_controls, catalog_controls), (16, 93, 104));\n        assert_eq!(FIXED_SCRATCH, 4096);\n        assert_eq!(CURRENT_BASE_SCRATCH, 4096 + 16 + 93 + 104);\n    }\n    growth\n}\n"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(body):
    return hashlib.sha256(body).hexdigest()


def reverse(body):
    require(type(body) is bytes and (len(body), digest(body)) == (DERIVED_BYTES, DERIVED_SHA),
            "wrong typed-vector index resource successor")
    require(body.endswith(MODEL) and len(SEAMS) == 2 and MODEL,
            "typed-vector index resource model or seam inventory changed")
    result = body[:-len(MODEL)]
    for before, after in reversed(SEAMS):
        require(result.count(after) == 1, "typed-vector index resource inverse seam drift")
        result = result.replace(after, before, 1)
    require((len(result), digest(result)) == (PREDECESSOR_BYTES, PREDECESSOR_SHA),
            "typed-vector index resource predecessor not restored")
    return result


def adapt(body):
    require(type(body) is bytes and (len(body), digest(body)) == (PREDECESSOR_BYTES, PREDECESSOR_SHA),
            "wrong typed-vector index resource predecessor")
    require(len(SEAMS) == 2 and MODEL, "typed-vector index resource model or seam inventory changed")
    result = body
    for before, after in SEAMS:
        require(result.count(before) == 1 and after not in result,
                "typed-vector index resource seam drift")
        result = result.replace(before, after, 1)
    result += MODEL
    require(reverse(result) == body, "typed-vector index resource inverse changed bytes")
    return result


def descriptor():
    return {"version": "typed-vector-index-resource-successor-1",
            "resource": RESOURCE,
            "predecessor": {"bytes": PREDECESSOR_BYTES, "sha256": PREDECESSOR_SHA},
            "derived": {"bytes": DERIVED_BYTES, "sha256": DERIVED_SHA},
            "changed_expected_scratch_expressions": 2,
            "retained_resource_test_count": 21,
            "new_rust_test_names": [],
            "native_64_bit_components": {"table_header_growth": 8,
                "admission_query_phase_growth": 8, "syntax_features": 45,
                "checked_syntax_subtotals": 48, "catalog_transports": 104},
            "legacy_fixed_bank": 4096,
            "scope": "Independent typed carriers added to fixed scratch only; all work/payload/cap/zero/one-less/failure-order assertions retained."}
