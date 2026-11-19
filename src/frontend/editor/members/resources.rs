//! Member-query request storage. These complete typed roles conservatively sum
//! successive helper frames and return transports. Dynamic requests are added
//! before reserve; this is not a process RSS or machine-stack bound.
//!
//! The existing source loader and namespace retain their own ledgers. Diagnostic
//! text/Box construction and LSP JSON serialization retain their existing scope;
//! test allocator traces are instrumentation, not editor payload. No old query
//! pays this bank, and no unused old allowance is subtracted from it.
use super::*;
use crate::frontend::{
    declaration_index::editor_completion::members::Record,
    lexer::{Kind, Token},
    parser::{
        editor_query::{
            members::{Fields, PathNames, Reader, TypePath},
            EditorFile,
        },
        MAX_BLOCK_NESTING,
    },
};
use std::{
    cmp::Ordering,
    slice::{Iter, Windows},
};

#[repr(C)]
#[allow(dead_code)]
struct CursorRoles<'a> {
    file: &'a EditorFile,
    source: &'a SourceFile,
    work: &'a WorkMeter,
    reader_return: Answer<Reader<'a>>,
    all: Reader<'a>,
    header: Reader<'a>,
    body: Reader<'a>,
    value_scan: Reader<'a>,
    constructor_scan: Reader<'a>,
    type_reader: Reader<'a>,
    anchor_iterator: std::iter::Enumerate<Iter<'a, Token>>,
    cursor: usize,
    before: usize,
    receiver_index: usize,
    opening: Option<usize>,
    function_anchor: Option<Span>,
    function_index: usize,
    parameter_depth: usize,
    initializer_start: usize,
    initializer_end: usize,
    constructor_depth: usize,
    replacement: Span,
    receiver: Token,
    local_name: Token,
    query_token: Token,
    first_type: Token,
    last_type: Token,
    type_colon: Token,
    type_second_colon: Token,
    type_name: Token,
    type_count: usize,
    constructor_open: Token,
    constructor_close: Option<Token>,
    bound: bool,
    same_name: bool,
    path: Option<TypePath>,
    annotation: Option<TypePath>,
    declared: Option<TypePath>,
    constructor: Option<Span>,
    type_return: Answer<Option<TypePath>>,
    query_return: Answer<Option<MemberQuery>>,
    query_selected: Option<MemberQuery>,
    query_caller: MemberQuery,
    peek_return: Answer<Option<Token>>,
    next_return: Answer<Option<Token>>,
    previous_return: Answer<Option<Token>>,
    next_selected: Option<Token>,
    take_return: Answer<bool>,
    name_return: Answer<bool>,
    selected_text: &'a str,
    name_left: &'a str,
    name_right: &'a str,
}

#[repr(C)]
#[allow(dead_code)]
struct DelimiterRoles<'a> {
    reader: &'a mut Reader<'a>,
    terminator: Kind,
    type_syntax: bool,
    closes: [Kind; MAX_BLOCK_NESTING],
    depth: usize,
    any: bool,
    token: Token,
    token_return: Answer<Option<Token>>,
    spelling: &'a str,
    closing: Option<Kind>,
    return_value: Answer<Option<usize>>,
    caller_value: Option<usize>,
}

#[repr(C)]
#[allow(dead_code)]
struct LookupRoles<'a> {
    requester: ModuleId,
    path: TypePath,
    names_return: Answer<PathNames<'a>>,
    names: PathNames<'a>,
    path_reader: Reader<'a>,
    path_start: usize,
    first: Span,
    current: Option<Span>,
    next: Option<Span>,
    names_next_return: Answer<Option<Span>>,
    selected_module: ModuleId,
    child: Option<ModuleId>,
    first_segment: bool,
    record_return: Answer<Option<Record<'a, 'a>>>,
    annotation_return: Answer<Option<Record<'a, 'a>>>,
    record: Record<'a, 'a>,
    annotated_record: Option<Record<'a, 'a>>,
    file_return: Answer<(&'a EditorFile, &'a SourceFile)>,
    file_selected: (&'a EditorFile, &'a SourceFile),
    fields_return: Answer<Option<Fields<'a>>>,
    fields_selected: Option<Fields<'a>>,
    field_counter: Fields<'a>,
    field_collector: Fields<'a>,
    constructor_fields_return: Answer<Fields<'a>>,
    constructor_counter: Fields<'a>,
    constructor_search: Fields<'a>,
    field_return: Answer<Option<FieldName>>,
    field_selected: Option<FieldName>,
    field_name: Token,
    field_public: bool,
    permission_return: Answer<bool>,
    private_visible: bool,
    constructor_count: usize,
    constructor_found: usize,
    constructor_result: Answer<bool>,
    constructor_selected: bool,
    namespace_remaining: Option<u64>,
    namespace_bytes: Option<usize>,
}

#[repr(C)]
#[allow(dead_code)]
struct SortRoles<'a> {
    fields: &'a mut [FieldName],
    map: &'a SourceMap,
    work: &'a WorkMeter,
    root: usize,
    end: usize,
    length: usize,
    child: usize,
    left: Option<usize>,
    roots: std::iter::Rev<std::ops::Range<usize>>,
    ends: std::iter::Rev<std::ops::Range<usize>>,
    duplicates: Windows<'a, FieldName>,
    a: Span,
    b: Span,
    a_name: &'a str,
    b_name: &'a str,
    comparison: Answer<Ordering>,
    sift_return: Answer<()>,
    sort_return: Answer<()>,
}

#[repr(C)]
#[allow(dead_code)]
struct IdentityRoles<'a> {
    hash: Sha256,
    digest: sha2::digest::Output<Sha256>,
    output: Identity,
    returned: Answer<Identity>,
    package: Option<&'a PackageSpec>,
    root: ModuleId,
    module: Option<ModuleId>,
    depth: usize,
    packages: std::iter::Enumerate<Iter<'a, PackageSpec>>,
    fields: Iter<'a, FieldName>,
    part: &'a [u8],
    length_bytes: [u8; 8],
    offset_bytes: [u8; 8],
    permission_byte: [u8; 1],
    part_work: Option<u64>,
    part_return: Answer<()>,
    path: &'a str,
}

#[repr(C)]
#[allow(dead_code)]
struct CompletionRoles<'a> {
    session: &'a QuerySources<'a>,
    packages: &'a [PackageSpec],
    path: &'a Path,
    offset: usize,
    work: &'a WorkMeter,
    allocator: &'a mut Allocator,
    budget: Budget,
    fixed_charge: Answer<()>,
    namespace_charge: Answer<()>,
    fields_charge: Answer<()>,
    output_charge: Answer<()>,
    source_iterator: std::iter::Enumerate<Iter<'a, SourceFile>>,
    source_selected: Option<(usize, &'a SourceFile)>,
    fields: Vec<FieldName>,
    entries: Vec<EditorCompletionItem>,
    field_count: usize,
    candidates: usize,
    names: usize,
    output_bytes: usize,
    result_count: usize,
    requested_payload: Option<usize>,
    prefix: &'a str,
    matched: Answer<bool>,
    copy_input: &'a str,
    copy_string: String,
    copy_return: Answer<String>,
    selected_label: String,
    selected_item: EditorCompletionItem,
    selected_location: EditorLocation,
    identity: Identity,
    result: MemberCompletions,
    raw_return: Answer<MemberCompletions>,
    mapped_return: Result<MemberCompletions, Vec<EditorDiagnostic>>,
    caller_result: MemberCompletions,
    graph_merge: Merge,
    graph_return: Option<EditorCompletions>,
    graph_conflict: bool,
    forwarded_return: Result<EditorCompletions, Vec<EditorDiagnostic>>,
}

#[repr(C)]
#[allow(dead_code)]
struct PositionRoles<'a> {
    source: &'a SourceFile,
    span: Span,
    work: &'a WorkMeter,
    characters: std::str::CharIndices<'a>,
    next: Option<(usize, char)>,
    offset: usize,
    scalar: char,
    line: usize,
    column: usize,
    start: Option<(usize, usize)>,
    end: (usize, usize),
    selected_start: (usize, usize),
    returned: Answer<Positions>,
    caller: Positions,
}

pub(super) fn fixed_bytes() -> usize {
    size_of::<CursorRoles<'_>>()
        + size_of::<DelimiterRoles<'_>>()
        + size_of::<LookupRoles<'_>>()
        + size_of::<SortRoles<'_>>()
        + size_of::<IdentityRoles<'_>>()
        + size_of::<PositionRoles<'_>>()
        + size_of::<CompletionRoles<'_>>()
        + crate::frontend::declaration_index::editor_completion::members::lookup_control_bytes()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::mem::{align_of, offset_of};

    // An independent C-layout member-sum oracle includes each full field's
    // alignment, then tail padding. Missing return envelopes cannot disappear
    // behind a guessed constant or a production helper's observed total.
    fn aligned(offset: usize, alignment: usize) -> usize {
        offset.div_ceil(alignment) * alignment
    }
    macro_rules! layout {
        ($ty:ty; $($field:ident : $member:ty),+ $(,)?) => {{
            let mut bytes = 0usize;
            let mut alignment = 1usize;
            $(
                bytes = aligned(bytes, align_of::<$member>());
                assert_eq!(offset_of!($ty, $field), bytes, stringify!($field));
                bytes += size_of::<$member>();
                alignment = alignment.max(align_of::<$member>());
            )+
            assert_eq!(align_of::<$ty>(), alignment);
            assert_eq!(size_of::<$ty>(), aligned(bytes, alignment));
            aligned(bytes, alignment)
        }};
    }

    pub(in crate::frontend::editor::members) fn independently_modeled_fixed_bytes() -> usize {
        #[cfg(target_pointer_width = "64")]
        {
            assert_eq!(size_of::<Reader<'_>>(), 3 * 8 + 2 * 8);
            assert_eq!(size_of::<TypePath>(), 8 + 8 + 24);
            assert_eq!(size_of::<FieldName>(), 32); // Span24, bool1, padding7.
            assert_eq!(size_of::<Record<'_, '_>>(), 2 * 8);
            assert_eq!(size_of::<Identity>(), 32);
        }
        let cursor = layout!(CursorRoles<'_>;
            file: &EditorFile, source: &SourceFile, work: &WorkMeter,
            reader_return: Answer<Reader<'_>>, all: Reader<'_>, header: Reader<'_>, body: Reader<'_>,
            value_scan: Reader<'_>, constructor_scan: Reader<'_>, type_reader: Reader<'_>,
            anchor_iterator: std::iter::Enumerate<Iter<'_, Token>>, cursor: usize, before: usize,
            receiver_index: usize, opening: Option<usize>, function_anchor: Option<Span>, function_index: usize,
            parameter_depth: usize, initializer_start: usize, initializer_end: usize, constructor_depth: usize,
            replacement: Span, receiver: Token, local_name: Token, query_token: Token,
            first_type: Token, last_type: Token, type_colon: Token, type_second_colon: Token, type_name: Token,
            type_count: usize, constructor_open: Token, constructor_close: Option<Token>,
            bound: bool, same_name: bool, path: Option<TypePath>, annotation: Option<TypePath>, declared: Option<TypePath>,
            constructor: Option<Span>, type_return: Answer<Option<TypePath>>, query_return: Answer<Option<MemberQuery>>,
            query_selected: Option<MemberQuery>, query_caller: MemberQuery,
            peek_return: Answer<Option<Token>>, next_return: Answer<Option<Token>>, previous_return: Answer<Option<Token>>,
            next_selected: Option<Token>, take_return: Answer<bool>, name_return: Answer<bool>,
            selected_text: &str, name_left: &str, name_right: &str);
        let delimiters = layout!(DelimiterRoles<'_>; reader: &mut Reader<'_>, terminator: Kind, type_syntax: bool,
            closes: [Kind; MAX_BLOCK_NESTING], depth: usize, any: bool, token: Token,
            token_return: Answer<Option<Token>>, spelling: &str, closing: Option<Kind>,
            return_value: Answer<Option<usize>>, caller_value: Option<usize>);
        let lookup = layout!(LookupRoles<'_>; requester: ModuleId, path: TypePath,
            names_return: Answer<PathNames<'_>>, names: PathNames<'_>, path_reader: Reader<'_>, path_start: usize,
            first: Span, current: Option<Span>, next: Option<Span>, names_next_return: Answer<Option<Span>>,
            selected_module: ModuleId, child: Option<ModuleId>, first_segment: bool,
            record_return: Answer<Option<Record<'_, '_>>>, annotation_return: Answer<Option<Record<'_, '_>>>,
            record: Record<'_, '_>, annotated_record: Option<Record<'_, '_>>,
            file_return: Answer<(&EditorFile, &SourceFile)>, file_selected: (&EditorFile, &SourceFile),
            fields_return: Answer<Option<Fields<'_>>>, fields_selected: Option<Fields<'_>>,
            field_counter: Fields<'_>, field_collector: Fields<'_>, constructor_fields_return: Answer<Fields<'_>>,
            constructor_counter: Fields<'_>, constructor_search: Fields<'_>, field_return: Answer<Option<FieldName>>,
            field_selected: Option<FieldName>, field_name: Token, field_public: bool, permission_return: Answer<bool>,
            private_visible: bool, constructor_count: usize, constructor_found: usize, constructor_result: Answer<bool>, constructor_selected: bool,
            namespace_remaining: Option<u64>, namespace_bytes: Option<usize>);
        let sort = layout!(SortRoles<'_>; fields: &mut [FieldName], map: &SourceMap, work: &WorkMeter,
            root: usize, end: usize, length: usize, child: usize, left: Option<usize>, roots: std::iter::Rev<std::ops::Range<usize>>,
            ends: std::iter::Rev<std::ops::Range<usize>>, duplicates: Windows<'_, FieldName>,
            a: Span, b: Span, a_name: &str, b_name: &str, comparison: Answer<Ordering>, sift_return: Answer<()>, sort_return: Answer<()>);
        let identity = layout!(IdentityRoles<'_>; hash: Sha256, digest: sha2::digest::Output<Sha256>, output: Identity,
            returned: Answer<Identity>, package: Option<&PackageSpec>, root: ModuleId, module: Option<ModuleId>, depth: usize,
            packages: std::iter::Enumerate<Iter<'_, PackageSpec>>, fields: Iter<'_, FieldName>, part: &[u8],
            length_bytes: [u8; 8], offset_bytes: [u8; 8], permission_byte: [u8; 1], part_work: Option<u64>,
            part_return: Answer<()>, path: &str);
        let completion = layout!(CompletionRoles<'_>; session: &QuerySources<'_>, packages: &[PackageSpec], path: &Path,
            offset: usize, work: &WorkMeter, allocator: &mut Allocator, budget: Budget,
            fixed_charge: Answer<()>, namespace_charge: Answer<()>, fields_charge: Answer<()>, output_charge: Answer<()>,
            source_iterator: std::iter::Enumerate<Iter<'_, SourceFile>>, source_selected: Option<(usize, &SourceFile)>,
            fields: Vec<FieldName>, entries: Vec<EditorCompletionItem>, field_count: usize, candidates: usize,
            names: usize, output_bytes: usize, result_count: usize, requested_payload: Option<usize>, prefix: &str,
            matched: Answer<bool>, copy_input: &str, copy_string: String, copy_return: Answer<String>, selected_label: String,
            selected_item: EditorCompletionItem, selected_location: EditorLocation, identity: Identity,
            result: MemberCompletions, raw_return: Answer<MemberCompletions>, mapped_return: Result<MemberCompletions, Vec<EditorDiagnostic>>,
            caller_result: MemberCompletions, graph_merge: Merge, graph_return: Option<EditorCompletions>, graph_conflict: bool,
            forwarded_return: Result<EditorCompletions, Vec<EditorDiagnostic>>);
        let positions = layout!(PositionRoles<'_>; source: &SourceFile, span: Span, work: &WorkMeter,
            characters: std::str::CharIndices<'_>, next: Option<(usize, char)>, offset: usize, scalar: char,
            line: usize, column: usize, start: Option<(usize, usize)>, end: (usize, usize),
            selected_start: (usize, usize), returned: Answer<Positions>, caller: Positions);
        cursor
            + delimiters
            + lookup
            + sort
            + identity
            + positions
            + completion
            + crate::frontend::declaration_index::editor_completion::members::lookup_control_bytes()
    }

    #[test]
    fn member_cursor_and_complete_return_carriers_have_independent_layouts() {
        assert_eq!(fixed_bytes(), independently_modeled_fixed_bytes());
    }
}
