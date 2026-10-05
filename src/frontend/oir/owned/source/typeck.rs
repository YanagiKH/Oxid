//! Complete source typing, deliberately without an ownership/loan checker.
use super::{
    hir::*,
    resolve::{ResolvedOwnedProgram, SourceAdmission},
    type_storage::{self as storage, Kind},
};
use crate::frontend::{
    declaration_index::{Access, PreparedTypeName},
    diagnostic::Diagnostic,
    lexer::Token,
    owned_diagnostic,
    parser::MAX_DIAGNOSTICS,
    project::budget::Allocator,
    source::Span,
};

#[derive(Debug)]
pub(in crate::frontend::oir) struct TypedOwnedProgram<'src> {
    program: ResolvedOwnedProgram<'src>,
    bodies: Vec<TypedBody>,
}
#[derive(Debug)]
pub(super) struct TypedBody {
    expressions: Vec<ValueTy>,
    bindings: Vec<ParameterTy>,
    block_flows: Vec<FlowSummary>,
    projections: Vec<Option<Projection>>,
    statement_projections: Vec<Vec<Option<Projection>>>,
    borrow_projections: Vec<BorrowProjection>,
}
#[derive(Debug)]
pub(super) struct BorrowProjection {
    expression: ExprId,
    argument: usize,
    projection: Projection,
}
/// Possible exits from a statement list. Loop transfers always refer to its
/// nearest enclosing while; that while consumes them before its own summary
/// reaches the containing list. These outcomes describe conservative source
/// paths, not constant-condition or termination analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FlowSummary {
    fallthrough: bool,
    returns: bool,
    breaks: bool,
    continues: bool,
}
impl FlowSummary {
    const FALLTHROUGH: Self = Self::new(true, false, false, false);
    const RETURN: Self = Self::new(false, true, false, false);
    const BREAK: Self = Self::new(false, false, true, false);
    const CONTINUE: Self = Self::new(false, false, false, true);

    const fn new(fallthrough: bool, returns: bool, breaks: bool, continues: bool) -> Self {
        Self {
            fallthrough,
            returns,
            breaks,
            continues,
        }
    }
    pub(super) fn falls_through(self) -> bool {
        self.fallthrough
    }
    pub(super) fn returns_only(self) -> bool {
        self == Self::RETURN
    }
    fn union(self, other: Self) -> Self {
        Self::new(
            self.fallthrough || other.fallthrough,
            self.returns || other.returns,
            self.breaks || other.breaks,
            self.continues || other.continues,
        )
    }
    /// Only fallthrough paths enter a following statement; previous terminal
    /// outcomes must survive even when that statement has different exits.
    fn then(self, next: Self) -> Self {
        if !self.fallthrough {
            return self;
        }
        Self::new(false, self.returns, self.breaks, self.continues).union(next)
    }
    fn after_while(self) -> Self {
        // The condition's false edge is always possible. Body fallthrough and
        // continue repeat it; break exits; only returns escape the whole loop.
        Self::new(true, self.returns, false, false)
    }
}

pub(super) struct TypedOwnedFunction<'a> {
    admission: SourceAdmission,
    function: &'a Function,
    signatures: &'a [Signature],
    body: &'a TypedBody,
}
impl TypedOwnedProgram<'_> {
    pub(super) fn admission(&self) -> SourceAdmission {
        self.program.admission()
    }
    pub(super) fn index(&self) -> &crate::frontend::declaration_index::DeclarationIndex<'_> {
        self.program.index()
    }
    pub(super) fn records(&self) -> &[Record] {
        self.program.records()
    }
    pub(super) fn signatures(&self) -> &[Signature] {
        self.program.signatures()
    }
    pub(in crate::frontend::oir) fn entry(&self) -> Option<DefId> {
        self.program.entry()
    }
    pub(super) fn text(&self, span: Span) -> &str {
        self.program.text(span)
    }
    pub(super) fn functions(&self) -> impl ExactSizeIterator<Item = TypedOwnedFunction<'_>> {
        assert_eq!(self.program.functions().len(), self.bodies.len());
        assert_eq!(self.program.signatures().len(), self.bodies.len());
        self.program
            .functions()
            .iter()
            .enumerate()
            .map(|(index, function)| {
                let body = &self.bodies[index];
                let signature = &self.program.signatures()[index];
                assert_eq!(function.id, DefId(index));
                assert_eq!(function.expressions.len(), body.expressions.len());
                assert_eq!(function.bindings.len(), body.bindings.len());
                assert_eq!(function.blocks.len(), body.block_flows.len());
                assert!(body.block_flows[function.body.0].returns_only());
                assert_eq!(&body.bindings[..signature.params.len()], &signature.params);
                TypedOwnedFunction {
                    admission: self.admission(),
                    function,
                    signatures: self.signatures(),
                    body,
                }
            })
    }
}
impl<'a> TypedOwnedFunction<'a> {
    pub(super) fn admission(&self) -> SourceAdmission {
        self.admission
    }
    pub(super) fn hir(&self) -> &'a Function {
        self.function
    }
    pub(super) fn signature(&self) -> &'a Signature {
        &self.signatures[self.function.id.0]
    }
    pub(super) fn call_parameter_ty(&self, target: DefId, position: usize) -> ParameterTy {
        self.signatures[target.0].params[position]
    }
    pub(super) fn expression_ty(&self, id: ExprId) -> ValueTy {
        self.body.expressions[id.0]
    }
    pub(super) fn binding_ty(&self, id: BindingId) -> ParameterTy {
        self.body.bindings[id.0]
    }
    pub(super) fn block_flow(&self, id: BodyBlockId) -> FlowSummary {
        self.body.block_flows[id.0]
    }
    pub(super) fn expression_projection(&self, id: ExprId) -> Option<&'a Projection> {
        self.body.projections[id.0].as_ref()
    }
    pub(super) fn borrow_projection(
        &self,
        expression: ExprId,
        argument: usize,
    ) -> Option<&'a Projection> {
        self.body
            .borrow_projections
            .binary_search_by_key(&(expression.0, argument), |entry| {
                (entry.expression.0, entry.argument)
            })
            .ok()
            .map(|index| &self.body.borrow_projections[index].projection)
    }
    pub(super) fn statement_projection(
        &self,
        block: BodyBlockId,
        index: usize,
    ) -> Option<&'a Projection> {
        self.body.statement_projections[block.0][index].as_ref()
    }
}
fn diagnostic(
    code: &'static str,
    stage: &'static str,
    message: impl std::fmt::Display,
    span: Option<Span>,
) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic(code, stage, format_args!("{message}"), span)
}
fn error(code: &'static str, message: impl std::fmt::Display, span: Span) -> Box<Diagnostic> {
    diagnostic(code, "type", message, Some(span))
}
trait Label {
    fn owned_secondary(self, span: Span, message: &'static str) -> Self;
}
impl Label for Box<Diagnostic> {
    fn owned_secondary(self, span: Span, message: &'static str) -> Self {
        owned_diagnostic::secondary(self, span, format_args!("{message}"))
    }
}
#[allow(clippy::large_enum_variant)] // Both bounded formatter frames are preaccounted; no heap allocation.
enum TypeName<'a> {
    Scalar(Ty),
    Array(FixedArrayTy),
    Record(PreparedTypeName<'a>),
}
impl std::fmt::Display for TypeName<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar(ty) => write!(f, "{ty}"),
            Self::Array(ty) => write!(f, "[{}; {}]", ty.element(), ty.length()),
            Self::Record(name) => write!(f, "{name}"),
        }
    }
}
fn type_name<'a>(
    program: &ResolvedOwnedProgram<'a>,
    ty: ValueTy,
    span: Span,
) -> Result<TypeName<'a>, Box<Diagnostic>> {
    match ty {
        ValueTy::Scalar(ty) => Ok(TypeName::Scalar(ty)),
        ValueTy::Owned(AggregateTy::Record(record)) => {
            program.prepare_name(record, span).map(TypeName::Record)
        }
        ValueTy::Owned(AggregateTy::FixedArray(array)) => Ok(TypeName::Array(array)),
        ValueTy::Owned(AggregateTy::Enum(_)) => {
            Err(error("E0300", "enum source types are unavailable", span))
        }
    }
}
fn mismatch(
    program: &ResolvedOwnedProgram<'_>,
    expected: ValueTy,
    actual: ValueTy,
    span: Span,
) -> Box<Diagnostic> {
    // Complete both bounded preparations before invoking infallible Display.
    let expected = match type_name(program, expected, span) {
        Ok(name) => name,
        Err(error) => return error,
    };
    let actual = match type_name(program, actual, span) {
        Ok(name) => name,
        Err(error) => return error,
    };
    error(
        "E0300",
        format_args!("type mismatch: expected {expected}, found {actual}"),
        span,
    )
}
fn immutable(function: &Function, binding: BindingId, span: Span) -> Box<Diagnostic> {
    error("E0304", "operation requires a mutable owned binding", span).owned_secondary(
        function.bindings[binding.0].span,
        "immutable binding declared here",
    )
}
pub(in crate::frontend::oir) fn check(
    program: ResolvedOwnedProgram<'_>,
) -> Result<TypedOwnedProgram<'_>, Vec<Diagnostic>> {
    // A paid statistics owner must never select the legacy storage branch.
    // Deny before phase observation, borrowed preparation or any body vectors.
    #[cfg(test)]
    if program.admission() == SourceAdmission::ObserveEnumTypes {
        return Err(vec![*diagnostic(
            "E0500",
            "type",
            "paid enum type observation requires its private checker",
            None,
        )]);
    }
    match check_bodies(&program, None) {
        Ok(bodies) => Ok(TypedOwnedProgram { program, bodies }),
        Err(diagnostics) => Err(diagnostics),
    }
}

/// Construction-local borrows only. No paid caller constructs this context in
/// this checkpoint; the only real checker call still supplies None.
struct ProgramPaid<'borrow, 'hir> {
    plan: &'borrow mut storage::TypePlan<'hir>,
    allocator: &'borrow mut Allocator,
    projection_bytes: &'borrow std::cell::Cell<usize>,
}
struct BodyPaid<'borrow> {
    quota: &'borrow mut storage::FunctionQuota,
    allocator: &'borrow mut Allocator,
    projection_bytes: &'borrow std::cell::Cell<usize>,
}
// Actual newly authored context/receiver surfaces, measured separately while
// unreachable. These partial models are NOT yet a complete T1 price and are NOT
// added to HirPlan: semantic iterators/projection frames, the eventual program
// context construction, fresh owner and observations still require full review.
// Existing T0 stage/frame/actuals/presence/quota/Bodies receivers stay assigned
// there; retained cache headers cannot pay these independent local receivers.
#[allow(dead_code)]
struct PaidProgramControls {
    argument: Option<&'static mut ProgramPaid<'static, 'static>>,
    reborrow: Option<&'static mut ProgramPaid<'static, 'static>>,
    selected: &'static mut ProgramPaid<'static, 'static>,
    enum_types: bool,
    bodies_room: Result<(), Box<Diagnostic>>,
}
#[allow(dead_code)]
struct PaidBodyControls {
    constructed: BodyPaid<'static>,
    constructor_borrow: &'static mut BodyPaid<'static>,
    constructor_option: Option<&'static mut BodyPaid<'static>>,
    argument: Option<&'static mut BodyPaid<'static>>,
    mutable_reborrow: Option<&'static mut BodyPaid<'static>>,
    mutable_selected: &'static mut BodyPaid<'static>,
    shared_reborrow: Option<&'static BodyPaid<'static>>,
    shared_selected: &'static BodyPaid<'static>,
    // Additional named outcome between check_body's return and final match.
    checked: Result<TypedBody, Box<Diagnostic>>,
    parameter_guard: bool,
    frame_room: Result<(), Box<Diagnostic>>,
    row_room: Result<(), Box<Diagnostic>>,
    // The initializer frame encloses (rather than replaces) expression frames.
    initializer_argument: Option<&'static mut BodyPaid<'static>>,
    // The final call moves this Option into expression_type, without reborrow.
    initializer_transfer: Option<&'static mut BodyPaid<'static>>,
}
#[allow(dead_code)]
struct PaidBodyReceivers {
    expression_projections: Vec<Option<Projection>>,
    statement_projections: Vec<Vec<Option<Projection>>>,
    paid_statement_rows: Vec<Vec<Option<Projection>>>,
    borrow_projections: Vec<BorrowProjection>,
    final_bindings: Vec<ParameterTy>,
    final_flows: Vec<FlowSummary>,
    final_expressions: Vec<ValueTy>,
}
#[allow(dead_code)]
struct PaidRowReceiver {
    row: Vec<Option<Projection>>,
}
#[allow(dead_code)]
struct PaidExpressionReborrows {
    argument: Option<&'static mut BodyPaid<'static>>,
    child_reborrow: Option<&'static mut BodyPaid<'static>>,
    selected: &'static mut BodyPaid<'static>,
    room: Result<(), Box<Diagnostic>>,
}

fn paid_state(at: Span) -> Box<Diagnostic> {
    error("E0500", "invalid paid checker storage state", at)
}

/// One borrowed semantic checker. Paid branches remain disconnected behind the
/// uninhabited observer result. Matching policy/context alone will never prove
/// fresh owner/plan/Cell provenance; the later entrypoint must establish that.
fn check_bodies(
    program: &ResolvedOwnedProgram<'_>,
    mut paid: Option<&mut ProgramPaid<'_, '_>>,
) -> Result<Vec<TypedBody>, Vec<Diagnostic>> {
    #[cfg(test)]
    let enum_types = program.admission() == SourceAdmission::ObserveEnumTypes;
    #[cfg(not(test))]
    let enum_types = false;
    if enum_types != paid.is_some() {
        return Err(vec![*diagnostic(
            "E0500",
            "type",
            "paid checker admission and storage disagree",
            None,
        )]);
    }
    program.work().phase("type");
    let mut bodies = match paid.as_deref_mut() {
        Some(paid) => paid
            .plan
            .reserve_bodies(paid.allocator, program.index().sources().eof())
            .map_err(|error| {
                program.work().record_error(&error);
                vec![*error]
            })?,
        None => Vec::new(),
    };
    let mut diagnostics = Vec::new();
    for function in program.functions() {
        if diagnostics.len() >= MAX_DIAGNOSTICS {
            break;
        }
        let checked = match paid.as_deref_mut() {
            Some(paid) => match paid.plan.partition_next(program.work(), function.end) {
                Ok(mut quota) => {
                    if quota.ordinal != function.id.0 {
                        let error = paid_state(function.end);
                        program.work().record_error(&error);
                        diagnostics.push(*error);
                        break;
                    }
                    let mut context = BodyPaid {
                        quota: &mut quota,
                        allocator: &mut *paid.allocator,
                        projection_bytes: paid.projection_bytes,
                    };
                    check_body(program, function, Some(&mut context))
                }
                Err(error) => {
                    // A failed atomic partition leaves its ordinal unchanged.
                    // Never continue, retry, or reuse those unconsumed rights.
                    program.work().record_error(&error);
                    diagnostics.push(*error);
                    break;
                }
            },
            None => check_body(program, function, None),
        };
        match checked {
            Ok(body) => {
                if paid.is_some() {
                    if let Err(error) =
                        storage::room(&bodies, program.functions().len(), function.end)
                    {
                        program.work().record_error(&error);
                        diagnostics.push(*error);
                        break;
                    }
                }
                bodies.push(body)
            }
            Err(error) => {
                // Only actual body errors after a consumed partition retain
                // the historical per-function diagnostic continuation.
                program.work().record_error(&error);
                diagnostics.push(*error)
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(bodies)
    } else {
        Err(diagnostics)
    }
}
// Complete new borrowed-core transports. The core's local Bodies header is
// the existing T0 BodiesReserveCarriers caller role and is not added again.
// This bank adds the distinct returned collection and consuming caller receiver.
// The primary owner remains the owner embedded in the existing complete
// TypedOwnedProgram envelope. Its new borrow does not copy that owner.
#[allow(dead_code)]
struct BorrowedCheckCarriers {
    program_borrows: [&'static ResolvedOwnedProgram<'static>; 2],
    functions: &'static [Function],
    iterator: std::slice::Iter<'static, Function>,
    next: Option<&'static Function>,
    function: &'static Function,
    returned: Result<Vec<TypedBody>, Vec<Diagnostic>>,
    caller_bodies: Vec<TypedBody>,
}
#[allow(dead_code)]
struct BorrowedBodyReturnCarriers {
    // check_body's complete construction, fallible return, and success pattern
    // coexist conservatively with the already prepaid retained Bodies slot.
    constructed: TypedBody,
    returned: Result<TypedBody, Box<Diagnostic>>,
    caller: TypedBody,
}
pub(super) const fn borrowed_check_carrier_bytes() -> usize {
    std::mem::size_of::<BorrowedCheckCarriers>()
}
pub(super) const fn borrowed_body_return_carrier_bytes() -> usize {
    std::mem::size_of::<BorrowedBodyReturnCarriers>()
}

// Shared semantic factories make their exact opaque output types available to
// the non-invoking layout witness below. No source owner is made for sizing.
fn projection_start(bound: usize) -> impl FnMut(&Token) -> bool {
    move |token| token.span.end <= bound
}
fn projection_end(end: usize) -> impl FnMut(&&Token) -> bool + Clone {
    move |token| token.span.start < end
}
fn projection_window(tokens: &[Token], end: usize) -> impl Iterator<Item = &Token> + Clone {
    tokens.iter().take_while(projection_end(end))
}
fn projection_identifier() -> impl FnMut(&&Token) -> bool + Clone {
    |token| token.kind == crate::frontend::lexer::Kind::Ident
}
fn projection_identifiers<'a>(
    tokens: impl Iterator<Item = &'a Token> + Clone,
) -> impl Iterator<Item = &'a Token> + Clone {
    tokens.filter(projection_identifier())
}
fn projection_field_predicate<'borrow, 'src: 'borrow>(
    program: &'borrow ResolvedOwnedProgram<'src>,
    spelling: &'borrow str,
) -> impl FnMut(&&Field) -> bool + 'borrow + use<'borrow, 'src> {
    move |field| program.text(field.name_span) == spelling
}
fn borrow_projection_key() -> impl FnMut(&BorrowProjection) -> (usize, usize) {
    |entry| (entry.expression.0, entry.argument)
}

// Measurement-only complete generic carriers. These exact W/I/P/S/Z/K/V types
// come from the semantic factories, never from equal-size stand-ins. No new
// model in this section is currently added to HirPlan or confers admission.
#[allow(dead_code)]
struct ProjectionIteratorCarriers<W: 'static, I, P, S, Z, K, E, V> {
    // Two window calls (projection and its enclosing array helper): each has
    // a complete factory construction and return, in addition to receivers.
    window_constructions_and_returns: [W; 4],
    window_receiver: W,
    cloned_window_return: W,
    clone_input: &'static W,
    // Filter's by-value input is distinct from its enclosing output payload.
    filter_inputs: [W; 2],
    filter_constructions_and_returns: [I; 4],
    count_input: I,
    identifiers_receiver: I,
    // Captured partition_point predicates for the two actual start searches.
    start_constructions_returns_and_inputs: [S; 6],
    end_constructions_returns_and_inputs: [E; 6],
    window_slice_iterator_inputs: [std::slice::Iter<'static, Token>; 2],
    end_factory_inputs: [usize; 2],
    identifier_constructions_returns_and_inputs: [Z; 6],
    field_construction_return_and_find_input: [P; 3],
    sort_construction_return_and_input: [K; 3],
    scalar_function_item: V,
    token_factory_inputs: [(&'static [Token], usize); 2],
    partition_inputs: [(&'static [Token], usize); 2],
    field_factory_inputs: (&'static ResolvedOwnedProgram<'static>, &'static str),
    token_next: Option<&'static Token>,
    token_current: &'static Token,
    identifier_next: [Option<&'static Token>; 2],
    identifier_current: &'static Token,
    fields: std::slice::Iter<'static, Field>,
    field_found: Option<&'static Field>,
    field_returned: Result<&'static Field, Box<Diagnostic>>,
    field_current: &'static Field,
}
#[allow(dead_code)]
#[allow(clippy::type_complexity)] // The complete actual factory tuple, without erased items.
struct IteratorWitnessCarriers<FW, FI, FP, FS, FZ, FK, FE, V> {
    // Function-item arguments are actual generic types, including their call
    // construction/parameter transfers. None of these factories is invoked.
    factory_items: [(FW, FI, FP, FS, FZ, FK, FE, V); 2],
    // Fixed layout tuple construction, witness return and outer return/caller.
    layout_transports: [[usize; 11]; 4],
}
fn iterator_layout_witness<W, I, P, S, Z, K, E, V, FW, FI, FP, FS, FZ, FK, FE>(
    _factories: (FW, FI, FP, FS, FZ, FK, FE, V),
) -> [usize; 11]
where
    W: Iterator<Item = &'static Token> + Clone + 'static,
    FW: FnOnce(&'static [Token], usize) -> W,
    FI: FnOnce(W) -> I,
    FP: FnOnce(&'static ResolvedOwnedProgram<'static>, &'static str) -> P,
    FS: FnOnce(usize) -> S,
    FZ: FnOnce() -> Z,
    FK: FnOnce() -> K,
    FE: FnOnce(usize) -> E,
    V: FnOnce(Ty) -> ValueTy,
{
    use std::mem::{align_of, size_of};
    [
        size_of::<ProjectionIteratorCarriers<W, I, P, S, Z, K, E, V>>(),
        align_of::<ProjectionIteratorCarriers<W, I, P, S, Z, K, E, V>>(),
        size_of::<IteratorWitnessCarriers<FW, FI, FP, FS, FZ, FK, FE, V>>(),
        size_of::<W>(),
        size_of::<I>(),
        size_of::<P>(),
        size_of::<S>(),
        size_of::<Z>(),
        size_of::<K>(),
        size_of::<V>(),
        size_of::<E>(),
    ]
}
#[allow(dead_code)] // Unpaid measurement surface, not yet used by source admission.
fn semantic_iterator_layout() -> [usize; 11] {
    iterator_layout_witness((
        projection_window,
        projection_identifiers,
        projection_field_predicate,
        projection_start,
        projection_identifier,
        borrow_projection_key,
        projection_end,
        ValueTy::Scalar,
    ))
}

// Explicit checker banks only. No source admission uses these measurement-only
// models yet. They are named construction/return/caller envelopes, not machine
// stack/RSS bounds or recursive accounting of inherited helper internals.
#[allow(dead_code)]
mod semantic_carriers {
    use super::*;
    use crate::frontend::oir::owned_types::DeclarationError;
    use crate::frontend::{ast, declaration_index::SourceOwner, project::ModuleId};
    use std::{
        array,
        iter::{Enumerate, Zip},
        mem::size_of,
        slice, vec,
    };

    type ArrayProjection = (Option<Projection>, AccessBase, Ty);
    type FrameState = (BodyBlockId, usize, FlowSummary, Option<LoopId>);
    type CallComparison =
        Enumerate<Zip<vec::IntoIter<(ParameterTy, Span)>, slice::Iter<'static, ParameterTy>>>;
    type CallComparisonItem = (usize, ((ParameterTy, Span), &'static ParameterTy));

    // One nonrecursive bank: array projection can enclose field projection, but
    // neither helper types a child, and each path is cached before a later child.
    pub(super) struct ProjectionSemanticCarriers {
        programs: [&'static ResolvedOwnedProgram<'static>; 2],
        functions: [&'static Function; 2],
        binding_inputs: [BindingId; 2],
        span_inputs: [Span; 4],
        bindings: [&'static [Option<ParameterTy>]; 2],
        contexts: [Option<&'static mut BodyPaid<'static>>; 2],
        context_reborrow: Option<&'static mut BodyPaid<'static>>,
        selected_context: &'static mut BodyPaid<'static>,
        borrow: bool,
        base_selection: (RecordId, AccessBase),
        selected_record: RecordId,
        selected_base: AccessBase,
        // The loop shadows the base record; never assume its slot is reused.
        loop_record: RecordId,
        binding_option: Option<ParameterTy>,
        binding_value: ParameterTy,
        current: ValueTy,
        requester_returns: [Result<ModuleId, Box<Diagnostic>>; 2],
        requesters: [ModuleId; 2],
        source_owners: [SourceOwner<'static>; 2],
        ast_returns: [Result<&'static ast::Program, Box<Diagnostic>>; 2],
        ast_receivers: [&'static ast::Program; 2],
        starts: [usize; 2],
        length: usize,
        spelling: &'static str,
        permission: Result<Access, Box<Diagnostic>>,
        denied_field: FieldId,
        path: Vec<FieldId>,
        path_last: Option<&'static FieldId>,
        last_return: Result<&'static FieldId, Box<Diagnostic>>,
        final_field: FieldId,
        constructed: Projection,
        returned: Result<Projection, Box<Diagnostic>>,
        nested_receiver: Projection,
        array: FixedArrayTy,
        root_token_return: Result<&'static Token, Box<Diagnostic>>,
        root: Span,
        field_span: Span,
        array_base: AccessBase,
        constructed_option: Option<Projection>,
        constructed_array: ArrayProjection,
        array_return: Result<ArrayProjection, Box<Diagnostic>>,
        // FieldRead/FieldAssign are mutually exclusive roles of this type.
        direct_field_caller: Projection,
        array_caller_tuple: ArrayProjection,
        caller_projection: Option<Projection>,
        caller_access: AccessBase,
        caller_element: Ty,
        borrow_conversion: Result<Projection, Box<Diagnostic>>,
        borrow_receiver: Projection,
        borrow_row: BorrowProjection,
        cache_transfer: Option<Projection>,
        path_room: Result<(), Box<Diagnostic>>,
        sparse_room: Result<(), Box<Diagnostic>>,
        // Path-free array_access is an explicit alternate helper surface.
        whole_array_input: (
            &'static Function,
            BindingId,
            Span,
            &'static [Option<ParameterTy>],
        ),
        whole_array_parameter: ParameterTy,
        whole_array_constructed: (AccessBase, Ty),
        whole_array_returned: Result<(AccessBase, Ty), Box<Diagnostic>>,
        whole_array_caller: (AccessBase, Ty),
    }
    // Closure invocation values are separate from their opaque object captures.
    // Sort's argument/key result belongs only to BodyFrameSemanticCarriers.
    pub(super) struct PredicateInvocationCarriers {
        start_inputs: [&'static Token; 2],
        start_returns: [bool; 2],
        end_inputs: [&'static &'static Token; 2],
        end_returns: [bool; 2],
        identifier_inputs: [&'static &'static Token; 2],
        identifier_returns: [bool; 2],
        field_input: &'static &'static Field,
        field_return: bool,
    }
    pub(super) struct CallSemanticCarriers {
        called: &'static Signature,
        target: &'static DefId,
        arguments: &'static Vec<Argument>,
        argument_cursor: Enumerate<slice::Iter<'static, Argument>>,
        argument_next: Option<(usize, &'static Argument)>,
        argument_tuple: (usize, &'static Argument),
        argument_position: usize,
        argument: &'static Argument,
        value_argument: &'static ExprId,
        constructed_actual: (ParameterTy, Span),
        current_actual: (ParameterTy, Span),
        borrowed_kind: &'static BorrowKind,
        borrowed_place: &'static BorrowPlace,
        borrowed_span: &'static Span,
        borrowed_name: &'static Span,
        binding_pattern: &'static BindingId,
        binding: BindingId,
        whole: ParameterTy,
        selected_actual: ParameterTy,
        // Complete cursor includes its owned IntoIter, Zip and parameter cursor
        // once; the source chain names no extra intermediate cursor receivers.
        comparison_cursor: CallComparison,
        comparison_next: Option<CallComparisonItem>,
        comparison_tuple: CallComparisonItem,
        position: usize,
        actual: ParameterTy,
        actual_span: Span,
        expected: &'static ParameterTy,
        mode_input: (ParameterTy, ParameterTy),
        authority: BorrowedTy,
        target_type: BorrowedTy,
        actual_kind: BorrowKind,
        expected_kind: BorrowKind,
        matches: bool,
        borrow_inputs: (
            &'static Function,
            BorrowKind,
            BorrowPlace,
            Span,
            &'static [Option<ParameterTy>],
        ),
        borrow_record: BorrowedTy,
        borrow_constructed: ParameterTy,
        borrow_returned: Result<ParameterTy, Box<Diagnostic>>,
    }
    pub(super) struct LiteralSemanticCarriers {
        record: &'static RecordId,
        fields: &'static Vec<FieldInit>,
        declared: &'static Record,
        cursor: slice::Iter<'static, FieldInit>,
        next: Option<&'static FieldInit>,
        field: &'static FieldInit,
        // Recursive callee's complete Result is in the depth bank. Here are
        // only the actual parent's named success receiver and expected value.
        actual: ValueTy,
        expected: ValueTy,
    }
    pub(super) struct StatementPatternCarriers {
        // Root-selection match and later semantic match have distinct patterns.
        root_binding: BindingId,
        root_init: ExprId,
        root_value: ExprId,
        root_index: ExprId,
        root_condition: ExprId,
        root_return: Option<ExprId>,
        root_target_span: Span,
        statement_binding: BindingId,
        statement_base: BindingId,
        statement_init: ExprId,
        statement_value: ExprId,
        statement_condition: ExprId,
        statement_base_span: Span,
        statement_field_span: Span,
        statement_target_span: Span,
        statement_return: Option<ExprId>,
        statement_target: LoopId,
        statement_loop: LoopId,
        statement_body: BodyBlockId,
        statement_then: BodyBlockId,
        statement_else: Option<BodyBlockId>,
        // Two ExprId locals, distinct from the frame's usize index as well.
        element_index: ExprId,
        local_index: ExprId,
        otherwise: BodyBlockId,
    }
    pub(super) struct BodyFrameSemanticCarriers {
        program: &'static ResolvedOwnedProgram<'static>,
        function: &'static Function,
        signature: &'static Signature,
        parameters: Enumerate<slice::Iter<'static, ParameterTy>>,
        parameter_next: Option<(usize, &'static ParameterTy)>,
        parameter_tuple: (usize, &'static ParameterTy),
        parameter_index: usize,
        parameter_type: &'static ParameterTy,
        inserted_binding: Option<ParameterTy>,
        rows: slice::Iter<'static, BodyBlock>,
        row_next: Option<&'static BodyBlock>,
        row: &'static BodyBlock,
        popped: Option<TypeFrame>,
        frame: TypeFrame,
        tuple: FrameState,
        block: BodyBlockId,
        index: usize,
        flow: FlowSummary,
        active_loop: Option<LoopId>,
        // Arm patterns and outer tuple destructuring do not share slots here.
        match_block: BodyBlockId,
        match_index: usize,
        match_active_loop: Option<LoopId>,
        match_block_flow: FlowSummary,
        before: FlowSummary,
        then_flow: FlowSummary,
        else_flow: FlowSummary,
        body_flow: FlowSummary,
        then_block: BodyBlockId,
        else_block: Option<BodyBlockId>,
        loop_body: BodyBlockId,
        pushed: TypeFrame,
        statement_option: Option<&'static Stmt>,
        statement: &'static Stmt,
        root_option: Option<ExprId>,
        root: ExprId,
        indexed_roots: [ExprId; 2],
        indexed_cursor: array::IntoIter<ExprId, 2>,
        indexed_next: Option<ExprId>,
        indexed_root: ExprId,
        declaration: &'static Binding,
        expected_parameter: ParameterTy,
        expected: ValueTy,
        actual: ValueTy,
        index_type: ValueTy,
        transfer: FlowSummary,
        borrow_slots: usize,
        work: usize,
        sort_product: Option<usize>,
        sort_product_return: Result<usize, Box<Diagnostic>>,
        sparse_slice: &'static mut [BorrowProjection],
        sort_argument: &'static BorrowProjection,
        sort_key_return: (usize, usize),
    }
    pub(super) struct InitializerSemanticCarriers {
        program: &'static ResolvedOwnedProgram<'static>,
        function: &'static Function,
        initializer: (BindingId, ExprId),
        binding: BindingId,
        root: ExprId,
        bindings: &'static [Option<ParameterTy>],
        expressions: &'static mut [Option<ValueTy>],
        projections: &'static mut [Option<Projection>],
        borrow_projections: &'static mut Vec<BorrowProjection>,
        annotation: Option<ValueTy>,
        array_annotation: FixedArrayTy,
        leaf: ExprId,
        expr: &'static Expr,
        array: FixedArrayTy,
        group_inner: &'static ExprId,
        array_elements: &'static Vec<ExprId>,
        returned: Result<ValueTy, Box<Diagnostic>>,
    }
    pub(super) struct ExpressionSemanticCarriers {
        program: &'static ResolvedOwnedProgram<'static>,
        function: &'static Function,
        id: ExprId,
        bindings: &'static [Option<ParameterTy>],
        expressions: &'static mut [Option<ValueTy>],
        projections: &'static mut [Option<Projection>],
        borrow_projections: &'static mut Vec<BorrowProjection>,
        cached: Option<ValueTy>,
        expr: &'static Expr,
        ty: ValueTy,
        field_read_inner_ty: ValueTy,
        returned: Result<ValueTy, Box<Diagnostic>>,
        binding_pattern: &'static BindingId,
        group_inner: &'static ExprId,
        projected_base: &'static BindingId,
        projected_base_span: &'static Span,
        field_span_pattern: &'static Span,
        index_pattern: &'static ExprId,
        expected: ValueTy,
        first_actual: ValueTy,
        second_comparison_actual: ValueTy,
        comparison_op: &'static ComparisonOp,
        left: &'static ExprId,
        right: &'static ExprId,
        operand: &'static ExprId,
        binary_operands: [&'static ExprId; 2],
        binary_cursor: array::IntoIter<&'static ExprId, 2>,
        binary_next: Option<&'static ExprId>,
        binary_operand: &'static ExprId,
        array_elements: &'static Vec<ExprId>,
        array_cursor: slice::Iter<'static, ExprId>,
        array_next: Option<&'static ExprId>,
        element: &'static ExprId,
        element_type: Option<Ty>,
        scalar_type: Ty,
        expected_element: Ty,
        checked_array: Result<FixedArrayTy, DeclarationError>,
        mapped_array: Result<FixedArrayTy, Box<Diagnostic>>,
        array: FixedArrayTy,
    }
    pub(super) struct MapOrSemanticCarriers<E, R> {
        // Object construction, factory return and actual map_or input.
        else_closures: [E; 3],
        return_closures: [R; 3],
        flow_factory_input: &'static [Option<FlowSummary>],
        value_factory_input: &'static [Option<ValueTy>],
        flow_option: Option<BodyBlockId>,
        flow_default: FlowSummary,
        flow_invocation_id: BodyBlockId,
        flow_invocation_return: FlowSummary,
        flow_map_return: FlowSummary,
        value_option: Option<ExprId>,
        value_default: ValueTy,
        value_invocation_id: ExprId,
        value_invocation_return: ValueTy,
        value_map_return: ValueTy,
        // Outer else_flow and actual receivers live in BodyFrameSemanticCarriers.
    }
    pub(super) struct MapOrWitnessCarriers<FE, FR> {
        factories: [(FE, FR); 2],
        layouts: [[usize; 5]; 4],
    }
    pub(super) const fn declared_bank_sizes() -> [usize; 8] {
        [
            size_of::<ProjectionSemanticCarriers>(),
            size_of::<PredicateInvocationCarriers>(),
            size_of::<CallSemanticCarriers>(),
            size_of::<LiteralSemanticCarriers>(),
            size_of::<StatementPatternCarriers>(),
            size_of::<BodyFrameSemanticCarriers>(),
            size_of::<InitializerSemanticCarriers>(),
            size_of::<ExpressionSemanticCarriers>(),
        ]
    }
}
fn else_flow_lookup(flows: &[Option<FlowSummary>]) -> impl FnOnce(BodyBlockId) -> FlowSummary + '_ {
    move |id| flows[id.0].expect("else arm was checked")
}
fn returned_value_lookup(values: &[Option<ValueTy>]) -> impl FnOnce(ExprId) -> ValueTy + '_ {
    move |id| values[id.0].expect("typed return")
}
fn map_or_layout_witness<E, R, FE, FR>(_factories: (FE, FR)) -> [usize; 5]
where
    FE: FnOnce(&'static [Option<FlowSummary>]) -> E,
    FR: FnOnce(&'static [Option<ValueTy>]) -> R,
{
    use std::mem::{align_of, size_of};
    [
        size_of::<semantic_carriers::MapOrSemanticCarriers<E, R>>(),
        align_of::<semantic_carriers::MapOrSemanticCarriers<E, R>>(),
        size_of::<semantic_carriers::MapOrWitnessCarriers<FE, FR>>(),
        size_of::<E>(),
        size_of::<R>(),
    ]
}
#[allow(dead_code)] // Measurement only; no source pricing or observer activation.
fn semantic_map_or_layout() -> [usize; 5] {
    map_or_layout_witness((else_flow_lookup, returned_value_lookup))
}

fn projection(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    binding: BindingId,
    base_span: Span,
    field_span: Span,
    bindings: &[Option<ParameterTy>],
    mut paid: Option<&mut BodyPaid<'_>>,
) -> Result<Projection, Box<Diagnostic>> {
    let (record, base) = match bindings[binding.0].expect("resolved field base initialized") {
        ParameterTy::Value(ValueTy::Owned(AggregateTy::Record(record))) => {
            (record, AccessBase::Owner(binding))
        }
        ParameterTy::Reference {
            referent: BorrowedTy::Exact(AggregateTy::Record(record)),
            kind,
        } => (record, AccessBase::Reference { binding, kind }),
        _ => {
            return Err(
                error("E0305", "field access requires a record binding", base_span)
                    .owned_secondary(function.bindings[binding.0].span, "binding declared here"),
            )
        }
    };
    let requester = program.requester(function.id)?;
    let ast = program.index().sources().ast(requester)?;
    let start = ast
        .tokens
        .partition_point(projection_start(field_span.start));
    let tokens = projection_window(&ast.tokens[start..], field_span.end);
    let length = projection_identifiers(tokens.clone()).count();
    let mut path = match paid.as_deref_mut() {
        Some(paid) => storage::projection_fields_metered(
            paid.projection_bytes,
            paid.allocator,
            length,
            program.work(),
            field_span,
        )?,
        None => {
            program.admit_projection(length, field_span)?;
            let mut path = Vec::new();
            path.try_reserve_exact(length)
                .map_err(|_| error("E0400", "record projection allocation failed", field_span))?;
            path
        }
    };
    let mut current = ValueTy::Owned(AggregateTy::Record(record));
    for token in tokens {
        if token.kind != crate::frontend::lexer::Kind::Ident {
            continue;
        }
        if path.len() >= 64 {
            return Err(error(
                "E0400",
                "record access path depth limit exceeded",
                token.span,
            ));
        }
        let ValueTy::Owned(AggregateTy::Record(record)) = current else {
            return Err(error(
                "E0305",
                "intermediate field access requires a record",
                token.span,
            ));
        };
        let spelling = program.text(token.span);
        let field = program.records()[record.0]
            .fields
            .iter()
            .find(projection_field_predicate(program, spelling))
            .ok_or_else(|| {
                error(
                    "E0305",
                    format_args!("unknown field `{}`", owned_diagnostic::name(spelling)),
                    token.span,
                )
            })?;
        if let Access::Denied(field) = program
            .query()
            .field_access(requester, field.id, token.span)?
        {
            return Err(program
                .query()
                .private_field_diagnostic(field, token.span, "type")?);
        }
        current = field.ty;
        if paid.is_some() {
            storage::room(&path, length, token.span)?;
        }
        path.push(field.id);
    }
    let field = *path
        .last()
        .ok_or_else(|| error("E0500", "empty resolved field path", field_span))?;
    Ok(Projection { base, field, path })
}
#[allow(clippy::too_many_arguments)] // Existing projection inputs plus the construction-local context.
fn projected_array_access(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    binding: BindingId,
    base_span: Span,
    access_span: Span,
    bindings: &[Option<ParameterTy>],
    borrow: bool,
    paid: Option<&mut BodyPaid<'_>>,
) -> Result<(Option<Projection>, AccessBase, Ty), Box<Diagnostic>> {
    let requester = program.requester(function.id)?;
    let ast = program.index().sources().ast(requester)?;
    let start = ast
        .tokens
        .partition_point(projection_start(base_span.start));
    let mut identifiers =
        projection_identifiers(projection_window(&ast.tokens[start..], base_span.end));
    let root = identifiers
        .next()
        .ok_or_else(|| error("E0500", "missing projection root", base_span))?
        .span;
    let Some(first) = identifiers.next() else {
        let (base, ty) = array_access(function, binding, access_span, bindings)?;
        return Ok((None, base, ty));
    };
    let mut field_span = first.span;
    field_span.end = base_span.end;
    let projection = projection(program, function, binding, root, field_span, bindings, paid)?;
    let ValueTy::Owned(AggregateTy::FixedArray(array)) =
        program.records()[projection.field.record.0].fields[projection.field.index].ty
    else {
        return Err(error(
            "E0305",
            if borrow {
                "projected borrowing requires a fixed scalar array field"
            } else {
                "indexed access requires a fixed scalar array field"
            },
            base_span,
        ));
    };
    let base = projection.base;
    Ok((Some(projection), base, array.element()))
}

fn array_access(
    function: &Function,
    binding: BindingId,
    access_span: Span,
    bindings: &[Option<ParameterTy>],
) -> Result<(AccessBase, Ty), Box<Diagnostic>> {
    match bindings[binding.0].expect("resolved array base initialized") {
        ParameterTy::Value(ValueTy::Owned(AggregateTy::FixedArray(array))) => {
            Ok((AccessBase::Owner(binding), array.element()))
        }
        ParameterTy::Reference { referent, kind } if referent.element().is_some() => Ok((
            AccessBase::Reference { binding, kind },
            referent.element().expect("array or slice"),
        )),
        _ => Err(error(
            "E0305",
            "array access requires an array binding",
            access_span,
        )
        .owned_secondary(function.bindings[binding.0].span, "binding declared here")),
    }
}

fn borrow_type(
    function: &Function,
    kind: BorrowKind,
    place: BorrowPlace,
    span: Span,
    bindings: &[Option<ParameterTy>],
) -> Result<ParameterTy, Box<Diagnostic>> {
    let record = match place {
        BorrowPlace::Owner(binding) => {
            match bindings[binding.0].expect("borrow binding initialized") {
                ParameterTy::Value(ValueTy::Owned(AggregateTy::Enum(_))) => {
                    return Err(error("E0300", "enum owners cannot be borrowed", span));
                }
                ParameterTy::Value(ValueTy::Owned(record)) => {
                    if kind == BorrowKind::Exclusive && !function.bindings[binding.0].mutable {
                        return Err(immutable(function, binding, span));
                    }
                    BorrowedTy::Exact(record)
                }
                ParameterTy::Value(ValueTy::Scalar(_)) => {
                    return Err(error(
                        "E0300",
                        "borrowing requires a whole record owner",
                        span,
                    ))
                }
                ParameterTy::Reference { .. } => {
                    return Err(error(
                        "E0312",
                        "reference parameters require explicit &*p or &mut *p forwarding",
                        span,
                    ))
                }
            }
        }
        BorrowPlace::Forwarded(binding) => {
            match bindings[binding.0].expect("borrow binding initialized") {
                ParameterTy::Reference {
                    referent: record, ..
                } => record,
                ParameterTy::Value(_) => {
                    return Err(error(
                        "E0312",
                        "explicit reborrow requires a reference parameter",
                        span,
                    ))
                }
            }
        }
    };
    if matches!(record, BorrowedTy::Exact(AggregateTy::Enum(_))) {
        return Err(error("E0300", "enum owners cannot be borrowed", span));
    }
    // Requested permission is retained even if the parent grants only shared.
    // The authoritative raw verifier diagnoses that ownership permission error.
    Ok(ParameterTy::Reference {
        referent: record,
        kind,
    })
}
#[allow(clippy::too_many_arguments)] // Narrow local paid context, unchanged semantic arguments.
fn expression_type(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    id: ExprId,
    bindings: &[Option<ParameterTy>],
    expressions: &mut [Option<ValueTy>],
    projections: &mut [Option<Projection>],
    borrow_projections: &mut Vec<BorrowProjection>,
    mut paid: Option<&mut BodyPaid<'_>>,
) -> Result<ValueTy, Box<Diagnostic>> {
    if let Some(ty) = expressions[id.0] {
        return Ok(ty);
    }
    let expr = &function.expressions[id.0];
    let scalar = ValueTy::Scalar;
    macro_rules! child {
        ($id:expr) => {
            expression_type(
                program,
                function,
                $id,
                bindings,
                expressions,
                projections,
                borrow_projections,
                paid.as_deref_mut(),
            )
        };
    }
    let ty = match &expr.kind {
        ExprKind::ConstructEnum { .. } => {
            return Err(error(
                "E0300",
                "enum source constructors are unavailable",
                expr.span,
            ));
        }
        ExprKind::Bool(_) => scalar(Ty::Bool),
        ExprKind::I32(_) => scalar(Ty::I32),
        ExprKind::Unit => scalar(Ty::Unit),
        ExprKind::Binding(binding) => match bindings[binding.0]
            .expect("resolved binding initialized")
        {
            ParameterTy::Value(ty) => ty,
            ParameterTy::Reference { .. } => return Err(error(
                "E0312",
                "reference parameter cannot be used as a value; use a field or explicit reborrow",
                expr.span,
            )),
        },
        ExprKind::Group(inner) => child!(*inner)?,
        ExprKind::Negate { operand, .. } => {
            let actual = child!(*operand)?;
            if actual != scalar(Ty::I32) {
                return Err(mismatch(
                    program,
                    scalar(Ty::I32),
                    actual,
                    function.expressions[operand.0].span,
                ));
            }
            scalar(Ty::I32)
        }
        ExprKind::Not { operand, .. } => {
            let actual = child!(*operand)?;
            if actual != scalar(Ty::Bool) {
                return Err(mismatch(
                    program,
                    scalar(Ty::Bool),
                    actual,
                    function.expressions[operand.0].span,
                ));
            }
            scalar(Ty::Bool)
        }
        ExprKind::Arithmetic { left, right, .. } | ExprKind::Logical { left, right, .. } => {
            let expected = if matches!(expr.kind, ExprKind::Logical { .. }) {
                scalar(Ty::Bool)
            } else {
                scalar(Ty::I32)
            };
            for operand in [left, right] {
                let actual = child!(*operand)?;
                if actual != expected {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[operand.0].span,
                    ));
                }
            }
            expected
        }
        ExprKind::Comparison {
            op, left, right, ..
        } => {
            let actual = child!(*left)?;
            let expected = match op {
                ComparisonOp::Equal | ComparisonOp::NotEqual => {
                    if !matches!(actual, ValueTy::Scalar(Ty::Bool | Ty::I32)) {
                        return Err(error(
                            "E0300",
                            "equality requires i32 or bool operands",
                            function.expressions[left.0].span,
                        ));
                    }
                    actual
                }
                _ => {
                    if actual != scalar(Ty::I32) {
                        return Err(mismatch(
                            program,
                            scalar(Ty::I32),
                            actual,
                            function.expressions[left.0].span,
                        ));
                    }
                    scalar(Ty::I32)
                }
            };
            let actual = child!(*right)?;
            if actual != expected {
                return Err(mismatch(
                    program,
                    expected,
                    actual,
                    function.expressions[right.0].span,
                ));
            }
            scalar(Ty::Bool)
        }
        ExprKind::Call { target, args } => {
            let called = &program.signatures()[target.0];
            // Visit all well-formed argument values in source order, including
            // statically skipped logical branches and ignored helper results.
            let mut actuals = match paid.as_deref_mut() {
                Some(paid) => paid.quota.storage.reserve(
                    paid.allocator,
                    Kind::CallActuals,
                    args.len(),
                    args.len(),
                    expr.span,
                )?,
                None => Vec::with_capacity(args.len()),
            };
            for (argument, arg) in args.iter().enumerate() {
                let actual = match arg {
                    Argument::Value(value) => (
                        ParameterTy::Value(expression_type(
                            program,
                            function,
                            *value,
                            bindings,
                            expressions,
                            projections,
                            borrow_projections,
                            paid.as_deref_mut(),
                        )?),
                        function.expressions[value.0].span,
                    ),
                    Argument::Borrow {
                        kind,
                        place,
                        span,
                        name_span,
                        ..
                    } => {
                        let whole = borrow_type(function, *kind, *place, *span, bindings)?;
                        let actual = if program.text(*name_span).contains('.') {
                            let binding = match place {
                                BorrowPlace::Owner(binding) | BorrowPlace::Forwarded(binding) => {
                                    *binding
                                }
                            };
                            let (projected, _, element) = projected_array_access(
                                program,
                                function,
                                binding,
                                *name_span,
                                *span,
                                bindings,
                                true,
                                paid.as_deref_mut(),
                            )?;
                            let projection = projected.ok_or_else(|| {
                                error("E0500", "missing projected borrow path", *span)
                            })?;
                            if let Some(paid) = paid.as_deref_mut() {
                                storage::room(
                                    borrow_projections,
                                    paid.quota.counts.borrow_arguments,
                                    *span,
                                )?;
                            } else if borrow_projections.len() == borrow_projections.capacity() {
                                // Geometric, fallible growth avoids repeated quadratic copies.
                                // Charge the entire requested capacity increase before reserving.
                                let growth = borrow_projections.capacity().max(1);
                                let bytes = growth
                                    .checked_mul(std::mem::size_of::<BorrowProjection>())
                                    .ok_or_else(|| {
                                        error("E0400", "projected borrow inventory overflow", *span)
                                    })?;
                                program.admit_projection_metadata(bytes, *span)?;
                                program.work().debit(
                                    growth as u64,
                                    *span,
                                    "projected borrow inventory growth",
                                )?;
                                borrow_projections.try_reserve_exact(growth).map_err(|_| {
                                    error(
                                        "E0400",
                                        "projected borrow inventory allocation failed",
                                        *span,
                                    )
                                })?;
                            }
                            borrow_projections.push(BorrowProjection {
                                expression: id,
                                argument,
                                projection,
                            });
                            // A projected fixed array is admitted only as a scalar slice.
                            // It cannot silently add exact-array or record field borrowing.
                            ParameterTy::Reference {
                                referent: BorrowedTy::ScalarSlice(element),
                                kind: *kind,
                            }
                        } else {
                            whole
                        };
                        (actual, *span)
                    }
                };
                if paid.is_some() {
                    storage::room(&actuals, args.len(), expr.span)?;
                }
                actuals.push(actual);
            }
            if args.len() != called.params.len() {
                return Err(error(
                    "E0301",
                    format_args!(
                        "wrong argument count: expected {}, found {}",
                        called.params.len(),
                        args.len()
                    ),
                    expr.span,
                )
                .owned_secondary(called.span, "function declared here"));
            }
            #[allow(clippy::unused_enumerate_index)]
            // Position is used only by passive test observation.
            for (_position, ((actual, span), expected)) in
                actuals.into_iter().zip(&called.params).enumerate()
            {
                let matches = match (actual, *expected) {
                    (
                        ParameterTy::Reference {
                            referent: authority,
                            kind: actual_kind,
                        },
                        ParameterTy::Reference {
                            referent: target,
                            kind: expected_kind,
                        },
                    ) => actual_kind == expected_kind && target.accepts(authority),
                    _ => actual == *expected,
                };
                if !matches {
                    return Err(error(
                        "E0300",
                        "call argument type or borrow mode does not match parameter",
                        span,
                    )
                    .owned_secondary(called.span, "function declared here"));
                }
                #[cfg(test)]
                if let Argument::Borrow { place, .. } = args[_position] {
                    let binding = match place {
                        BorrowPlace::Owner(binding) | BorrowPlace::Forwarded(binding) => binding,
                    };
                    program.work().observe(
                        crate::frontend::declaration_index::Observation::BorrowArgument {
                            function: function.id,
                            origin: span,
                            binding: binding.0,
                            ty: *expected,
                        },
                    );
                }
            }
            called.result
        }
        ExprKind::StructLiteral { record, fields } => {
            let declared = &program.records()[record.0];
            let mut present = match paid.as_deref_mut() {
                Some(paid) => {
                    paid.quota
                        .storage
                        .presence(paid.allocator, declared.fields.len(), expr.span)?
                }
                None => vec![false; declared.fields.len()],
            };
            for field in fields {
                present[field.field.index] = true;
                let actual = child!(field.value)?;
                let expected = declared.fields[field.field.index].ty;
                if actual != expected {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[field.value.0].span,
                    ));
                }
            }
            if fields.len() != declared.fields.len() {
                return Err(owned_diagnostic::missing_fields(
                    "E0300",
                    "type",
                    declared
                        .fields
                        .iter()
                        .filter(|decl| !present[decl.id.index])
                        .map(|field| program.text(field.name_span)),
                    Some(expr.span),
                ));
            }
            ValueTy::Owned(AggregateTy::Record(*record))
        }
        ExprKind::ArrayLiteral { elements } => {
            program.work().debit(1, expr.span, "array type literal")?;
            if elements.is_empty() {
                return Err(error("E0300", "empty array literal requires an explicit array annotation on its local initializer", expr.span));
            }
            let mut element_type = None;
            for element in elements {
                program.work().debit(1, expr.span, "array type edge")?;
                let actual = child!(*element)?;
                let ValueTy::Scalar(scalar_type) = actual else {
                    return Err(error(
                        "E0300",
                        "array elements must have scalar bool, i32 or () type",
                        function.expressions[element.0].span,
                    ));
                };
                if let Some(expected) = element_type {
                    if scalar_type != expected {
                        return Err(mismatch(
                            program,
                            scalar(expected),
                            actual,
                            function.expressions[element.0].span,
                        ));
                    }
                } else {
                    element_type = Some(scalar_type);
                }
            }
            let array =
                FixedArrayTy::check(element_type.expect("nonempty literal"), elements.len())
                    .map_err(|_| error("E0500", "invalid resolved array length", expr.span))?;
            ValueTy::Owned(AggregateTy::FixedArray(array))
        }
        ExprKind::IndexRead {
            base,
            base_span,
            index,
        } => {
            program.work().debit(1, expr.span, "array type read")?;
            let actual = child!(*index)?;
            program.work().debit(1, expr.span, "array type access")?;
            let (projection, _, element) = projected_array_access(
                program,
                function,
                *base,
                *base_span,
                expr.span,
                bindings,
                false,
                paid.as_deref_mut(),
            )?;
            projections[id.0] = projection;
            if actual != scalar(Ty::I32) {
                return Err(mismatch(
                    program,
                    scalar(Ty::I32),
                    actual,
                    function.expressions[index.0].span,
                ));
            }
            scalar(element)
        }
        ExprKind::ArrayLength { base, base_span } => {
            program.work().debit(1, expr.span, "array type length")?;
            program.work().debit(1, expr.span, "array type access")?;
            let (projection, _, _) = projected_array_access(
                program,
                function,
                *base,
                *base_span,
                expr.span,
                bindings,
                false,
                paid.as_deref_mut(),
            )?;
            projections[id.0] = projection;
            scalar(Ty::I32)
        }
        ExprKind::FieldRead {
            base,
            base_span,
            field_span,
        } => {
            let projection = projection(
                program,
                function,
                *base,
                *base_span,
                *field_span,
                bindings,
                paid,
            )?;
            let ty = program.records()[projection.field.record.0].fields[projection.field.index].ty;
            if !matches!(ty, ValueTy::Scalar(_)) {
                return Err(error(
                    "E0305",
                    "aggregate field extraction is unavailable; move or borrow the whole root",
                    *field_span,
                ));
            }
            #[cfg(test)]
            program.work().observe(
                crate::frontend::declaration_index::Observation::Projection {
                    operation: "read",
                    function: function.id,
                    origin: *field_span,
                    field: projection.field,
                    ty,
                },
            );
            projections[id.0] = Some(projection);
            ty
        }
    };
    finish_expression_type(program, function, id, ty, expressions, projections);
    Ok(ty)
}
fn finish_expression_type(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    id: ExprId,
    ty: ValueTy,
    expressions: &mut [Option<ValueTy>],
    projections: &[Option<Projection>],
) {
    assert!(expressions[id.0].is_none(), "type slot finalized once");
    expressions[id.0] = Some(ty);
    #[cfg(test)]
    program.work().observe(
        crate::frontend::declaration_index::Observation::Expression {
            function: function.id,
            origin: function.expressions[id.0].span,
            ty,
            field: projections[id.0].as_ref().map(|p| p.field),
        },
    );
    #[cfg(not(test))]
    let _ = (program, function, projections);
}
#[allow(clippy::too_many_arguments)] // Narrow local paid context, unchanged semantic arguments.
fn initializer_type(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    initializer: (BindingId, ExprId),
    bindings: &[Option<ParameterTy>],
    expressions: &mut [Option<ValueTy>],
    projections: &mut [Option<Projection>],
    borrow_projections: &mut Vec<BorrowProjection>,
    paid: Option<&mut BodyPaid<'_>>,
) -> Result<ValueTy, Box<Diagnostic>> {
    let (binding, root) = initializer;
    if let Some(ValueTy::Owned(AggregateTy::FixedArray(annotation))) =
        function.bindings[binding.0].annotation
    {
        let mut leaf = root;
        loop {
            let expr = &function.expressions[leaf.0];
            program
                .work()
                .debit(1, expr.span, "array initializer peel")?;
            match &expr.kind {
                ExprKind::Group(inner) => leaf = *inner,
                ExprKind::ArrayLiteral { elements } if elements.is_empty() => {
                    program.work().debit(1, expr.span, "array type literal")?;
                    let array = FixedArrayTy::check(annotation.element(), 0)
                        .expect("zero is a checked fixed-array length");
                    finish_expression_type(
                        program,
                        function,
                        leaf,
                        ValueTy::Owned(AggregateTy::FixedArray(array)),
                        expressions,
                        projections,
                    );
                    break;
                }
                _ => break,
            }
        }
    }
    expression_type(
        program,
        function,
        root,
        bindings,
        expressions,
        projections,
        borrow_projections,
        paid,
    )
}
pub(super) enum TypeFrame {
    Block {
        block: BodyBlockId,
        index: usize,
        flow: FlowSummary,
        active_loop: Option<LoopId>,
    },
    IfJoin {
        block: BodyBlockId,
        index: usize,
        before: FlowSummary,
        active_loop: Option<LoopId>,
        then_block: BodyBlockId,
        else_block: Option<BodyBlockId>,
    },
    WhileJoin {
        block: BodyBlockId,
        index: usize,
        before: FlowSummary,
        active_loop: Option<LoopId>,
        body: BodyBlockId,
    },
}
fn check_body(
    program: &ResolvedOwnedProgram<'_>,
    function: &Function,
    mut paid: Option<&mut BodyPaid<'_>>,
) -> Result<TypedBody, Box<Diagnostic>> {
    let signature = &program.signatures()[function.id.0];
    let mut bindings = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.none(
            paid.allocator,
            Kind::BindingStage,
            function.bindings.len(),
            function.end,
        )?,
        None => vec![None; function.bindings.len()],
    };
    if paid.is_some() && signature.params.len() > bindings.len() {
        return Err(paid_state(function.end));
    }
    for (index, ty) in signature.params.iter().enumerate() {
        bindings[index] = Some(*ty);
        #[cfg(test)]
        program
            .work()
            .observe(crate::frontend::declaration_index::Observation::Binding {
                function: function.id,
                origin: function.bindings[index].span,
                ty: *ty,
            });
    }
    let mut expressions = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.none(
            paid.allocator,
            Kind::ExpressionStage,
            function.expressions.len(),
            function.end,
        )?,
        None => vec![None; function.expressions.len()],
    };
    let mut projections = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.none(
            paid.allocator,
            Kind::ExpressionProjections,
            function.expressions.len(),
            function.end,
        )?,
        None => vec![None; function.expressions.len()],
    };
    let mut borrow_projections = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.reserve(
            paid.allocator,
            Kind::BorrowProjections,
            paid.quota.counts.borrow_arguments,
            paid.quota.counts.borrow_arguments,
            function.end,
        )?,
        None => {
            // Preserve the ordinary enclosing-header charge and sparse growth.
            program.admit_projection_metadata(
                std::mem::size_of::<Vec<BorrowProjection>>(),
                function.end,
            )?;
            Vec::new()
        }
    };
    let mut statement_projections: Vec<Vec<Option<Projection>>> = match paid.as_deref_mut() {
        Some(paid) => {
            let mut rows = paid.quota.storage.reserve(
                paid.allocator,
                Kind::StatementRows,
                function.blocks.len(),
                function.blocks.len(),
                function.end,
            )?;
            for block in &function.blocks {
                let row = paid.quota.storage.none(
                    paid.allocator,
                    Kind::StatementProjections,
                    block.body.len(),
                    function.end,
                )?;
                storage::room(&rows, function.blocks.len(), function.end)?;
                rows.push(row);
            }
            rows
        }
        None => function
            .blocks
            .iter()
            .map(|block| vec![None; block.body.len()])
            .collect(),
    };
    // A continuation frame records each statement-list result without Rust
    // recursion. Both children complete before their parent's flow is resumed.
    let mut block_flows: Vec<Option<FlowSummary>> = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.none(
            paid.allocator,
            Kind::FlowStage,
            function.blocks.len(),
            function.end,
        )?,
        None => vec![None; function.blocks.len()],
    };
    let mut frames = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.reserve(
            paid.allocator,
            Kind::TypeFrames,
            paid.quota.counts.type_frames,
            paid.quota.counts.type_frames,
            function.end,
        )?,
        // The historical vec![root] asks for exactly one initial slot.
        None => Vec::with_capacity(1),
    };
    if let Some(paid) = paid.as_deref() {
        storage::room(&frames, paid.quota.counts.type_frames, function.end)?;
    }
    frames.push(TypeFrame::Block {
        block: function.body,
        index: 0,
        flow: FlowSummary::FALLTHROUGH,
        active_loop: None,
    });
    while let Some(frame) = frames.pop() {
        let (block, index, mut flow, active_loop) = match frame {
            TypeFrame::Block {
                block,
                index,
                flow,
                active_loop,
            } => (block, index, flow, active_loop),
            TypeFrame::IfJoin {
                block,
                index,
                before,
                active_loop,
                then_block,
                else_block,
            } => {
                let then_flow = block_flows[then_block.0].expect("then arm was checked");
                let else_flow =
                    else_block.map_or(FlowSummary::FALLTHROUGH, else_flow_lookup(&block_flows));
                (
                    block,
                    index,
                    before.then(then_flow.union(else_flow)),
                    active_loop,
                )
            }
            TypeFrame::WhileJoin {
                block,
                index,
                before,
                active_loop,
                body,
            } => {
                let body_flow = block_flows[body.0].expect("while body was checked");
                (
                    block,
                    index,
                    before.then(body_flow.after_while()),
                    active_loop,
                )
            }
        };
        let Some(statement) = function.blocks[block.0].body.get(index) else {
            block_flows[block.0] = Some(flow);
            continue;
        };
        if !flow.falls_through() {
            return Err(diagnostic(
                "E0303",
                "type",
                if flow.returns_only() {
                    "statement after terminal return is unavailable in typed-preview"
                } else {
                    "statement after terminal control transfer is unavailable in typed-preview"
                },
                Some(statement.span),
            ));
        }
        let root = match statement.kind {
            StmtKind::Match { .. } => {
                return Err(error(
                    "E0300",
                    "enum source matches are unavailable",
                    statement.span,
                ));
            }
            StmtKind::Let { binding, init } => {
                initializer_type(
                    program,
                    function,
                    (binding, init),
                    &bindings,
                    &mut expressions,
                    &mut projections,
                    &mut borrow_projections,
                    paid.as_deref_mut(),
                )?;
                None
            }
            StmtKind::IndexAssign {
                value,
                index,
                target_span,
                ..
            } => {
                program.work().debit(1, target_span, "array type store")?;
                for root in [value, index] {
                    expression_type(
                        program,
                        function,
                        root,
                        &bindings,
                        &mut expressions,
                        &mut projections,
                        &mut borrow_projections,
                        paid.as_deref_mut(),
                    )?;
                }
                None
            }
            StmtKind::Assign { value: init, .. }
            | StmtKind::FieldAssign { value: init, .. }
            | StmtKind::Expr(init) => Some(init),
            StmtKind::Return(value) => value,
            StmtKind::Break { .. } | StmtKind::Continue { .. } => None,
            StmtKind::If { condition, .. } | StmtKind::While { condition, .. } => Some(condition),
        };
        if let Some(root) = root {
            expression_type(
                program,
                function,
                root,
                &bindings,
                &mut expressions,
                &mut projections,
                &mut borrow_projections,
                paid.as_deref_mut(),
            )?;
        }
        match statement.kind {
            StmtKind::Match { .. } => {
                return Err(error(
                    "E0300",
                    "enum source matches are unavailable",
                    statement.span,
                ));
            }
            StmtKind::Let { binding, init } => {
                let actual = expressions[init.0].expect("typed initializer");
                if let Some(expected) = function.bindings[binding.0].annotation {
                    if expected != actual {
                        return Err(mismatch(
                            program,
                            expected,
                            actual,
                            function.expressions[init.0].span,
                        )
                        .owned_secondary(
                            function.bindings[binding.0].span,
                            "binding declared here",
                        ));
                    }
                }
                bindings[binding.0] = Some(ParameterTy::Value(actual));
                #[cfg(test)]
                program
                    .work()
                    .observe(crate::frontend::declaration_index::Observation::Binding {
                        function: function.id,
                        origin: function.bindings[binding.0].span,
                        ty: ParameterTy::Value(actual),
                    });
            }
            StmtKind::Assign {
                binding,
                target_span,
                value,
                ..
            } => {
                let declaration = &function.bindings[binding.0];
                if !declaration.mutable {
                    return Err(diagnostic(
                        "E0304",
                        "type",
                        "assignment requires a mutable binding",
                        Some(target_span),
                    )
                    .owned_secondary(declaration.span, "immutable binding declared here"));
                }
                let expected =
                    bindings[binding.0].expect("resolved assignment target has an initializer");
                let ParameterTy::Value(expected) = expected else {
                    return Err(error(
                        "E0312",
                        "reference parameters cannot be assigned",
                        target_span,
                    ));
                };
                let actual = expressions[value.0].expect("typed assignment");
                if actual != expected {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[value.0].span,
                    )
                    .owned_secondary(declaration.span, "binding declared here"));
                }
            }
            StmtKind::FieldAssign {
                base,
                base_span,
                field_span,
                target_span,
                value,
                ..
            } => {
                let projection = projection(
                    program,
                    function,
                    base,
                    base_span,
                    field_span,
                    &bindings,
                    paid.as_deref_mut(),
                )?;
                if matches!(projection.base, AccessBase::Owner(_))
                    && !function.bindings[base.0].mutable
                {
                    return Err(immutable(function, base, target_span));
                }
                let expected =
                    program.records()[projection.field.record.0].fields[projection.field.index].ty;
                if !matches!(expected, ValueTy::Scalar(_)) {
                    return Err(error(
                        "E0305",
                        "aggregate field replacement is unavailable; replace the whole root",
                        target_span,
                    ));
                }
                let actual = expressions[value.0].expect("typed field assignment");
                if expected != actual {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[value.0].span,
                    ));
                }
                #[cfg(test)]
                program.work().observe(
                    crate::frontend::declaration_index::Observation::Projection {
                        operation: "write",
                        function: function.id,
                        origin: field_span,
                        field: projection.field,
                        ty: expected,
                    },
                );
                statement_projections[block.0][index] = Some(projection);
            }
            StmtKind::IndexAssign {
                base,
                base_span,
                target_span,
                value,
                index: element_index,
                ..
            } => {
                program.work().debit(1, target_span, "array type access")?;
                let (projection, access, element) = projected_array_access(
                    program,
                    function,
                    base,
                    base_span,
                    target_span,
                    &bindings,
                    false,
                    paid.as_deref_mut(),
                )?;
                statement_projections[block.0][index] = projection;
                let index = element_index;
                let index_ty = expressions[index.0].expect("typed store index");
                if index_ty != ValueTy::Scalar(Ty::I32) {
                    return Err(mismatch(
                        program,
                        ValueTy::Scalar(Ty::I32),
                        index_ty,
                        function.expressions[index.0].span,
                    ));
                }
                let actual = expressions[value.0].expect("typed store value");
                let expected = ValueTy::Scalar(element);
                if actual != expected {
                    return Err(mismatch(
                        program,
                        expected,
                        actual,
                        function.expressions[value.0].span,
                    ));
                }
                if matches!(access, AccessBase::Owner(_)) && !function.bindings[base.0].mutable {
                    return Err(immutable(function, base, target_span));
                }
            }
            StmtKind::Expr(_) => {}
            StmtKind::Return(value) => {
                let actual = value.map_or(
                    ValueTy::Scalar(Ty::Unit),
                    returned_value_lookup(&expressions),
                );
                if signature.result != actual {
                    return Err(mismatch(
                        program,
                        signature.result,
                        actual,
                        value.map_or(statement.span, |id| function.expressions[id.0].span),
                    ));
                }
                flow = flow.then(FlowSummary::RETURN);
            }
            StmtKind::Break { target } | StmtKind::Continue { target } => {
                assert_eq!(
                    active_loop,
                    Some(target),
                    "resolved transfer targets the nearest active loop"
                );
                assert!(target.0 < function.blocks.len());
                let transfer = if matches!(statement.kind, StmtKind::Break { .. }) {
                    FlowSummary::BREAK
                } else {
                    FlowSummary::CONTINUE
                };
                flow = flow.then(transfer);
            }
            StmtKind::While {
                loop_id,
                condition,
                body,
            } => {
                let actual = expressions[condition.0].expect("typed condition");
                if actual != ValueTy::Scalar(Ty::Bool) {
                    return Err(mismatch(
                        program,
                        ValueTy::Scalar(Ty::Bool),
                        actual,
                        function.expressions[condition.0].span,
                    ));
                }
                assert_eq!(
                    loop_id.0, body.0,
                    "resolved loop ID is its unique body block"
                );
                if let Some(paid) = paid.as_deref() {
                    storage::room(&frames, paid.quota.counts.type_frames, statement.span)?;
                }
                frames.push(TypeFrame::WhileJoin {
                    block,
                    index: index + 1,
                    before: flow,
                    active_loop,
                    body,
                });
                if let Some(paid) = paid.as_deref() {
                    storage::room(&frames, paid.quota.counts.type_frames, statement.span)?;
                }
                frames.push(TypeFrame::Block {
                    block: body,
                    index: 0,
                    flow: FlowSummary::FALLTHROUGH,
                    active_loop: Some(loop_id),
                });
                continue;
            }
            StmtKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let actual = expressions[condition.0].expect("typed condition");
                if actual != ValueTy::Scalar(Ty::Bool) {
                    return Err(mismatch(
                        program,
                        ValueTy::Scalar(Ty::Bool),
                        actual,
                        function.expressions[condition.0].span,
                    ));
                }
                if let Some(paid) = paid.as_deref() {
                    storage::room(&frames, paid.quota.counts.type_frames, statement.span)?;
                }
                frames.push(TypeFrame::IfJoin {
                    block,
                    index: index + 1,
                    before: flow,
                    active_loop,
                    then_block,
                    else_block,
                });
                if let Some(otherwise) = else_block {
                    if let Some(paid) = paid.as_deref() {
                        storage::room(&frames, paid.quota.counts.type_frames, statement.span)?;
                    }
                    frames.push(TypeFrame::Block {
                        block: otherwise,
                        index: 0,
                        flow: FlowSummary::FALLTHROUGH,
                        active_loop,
                    });
                }
                if let Some(paid) = paid.as_deref() {
                    storage::room(&frames, paid.quota.counts.type_frames, statement.span)?;
                }
                frames.push(TypeFrame::Block {
                    block: then_block,
                    index: 0,
                    flow: FlowSummary::FALLTHROUGH,
                    active_loop,
                });
                continue;
            }
        }
        if let Some(paid) = paid.as_deref() {
            storage::room(&frames, paid.quota.counts.type_frames, statement.span)?;
        }
        frames.push(TypeFrame::Block {
            block,
            index: index + 1,
            flow,
            active_loop,
        });
    }
    if !block_flows[function.body.0]
        .expect("function body was checked")
        .returns_only()
    {
        return Err(diagnostic(
            "E0302",
            "type",
            "function requires an explicit terminal return",
            Some(function.end),
        ));
    }
    let final_bindings = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.finalize(
            paid.allocator,
            Kind::BindingFinal,
            &bindings,
            function.end,
        )?,
        None => bindings
            .into_iter()
            .map(|ty| ty.expect("all resolved bindings have typed initializers"))
            .collect(),
    };
    let final_flows = match paid.as_deref_mut() {
        Some(paid) => paid.quota.storage.finalize(
            paid.allocator,
            Kind::FlowFinal,
            &block_flows,
            function.end,
        )?,
        None => block_flows
            .into_iter()
            .map(|flow| flow.expect("all resolved blocks have checked flow"))
            .collect(),
    };
    // Borrow typing follows source evaluation order, which can interleave nested
    // call sites. Sort the sparse table for bounded binary lookup during lowering.
    let borrow_slots = borrow_projections.len();
    if borrow_slots > 0 {
        let work = borrow_slots
            .checked_mul(borrow_slots.ilog2() as usize + 1)
            .ok_or_else(|| {
                error(
                    "E0400",
                    "projected borrow lookup work overflow",
                    function.end,
                )
            })?;
        program.work().debit(
            work as u64,
            function.end,
            "projected borrow lookup ordering",
        )?;
        borrow_projections.sort_unstable_by_key(borrow_projection_key());
    }
    let final_expressions = match paid {
        Some(paid) => paid.quota.storage.finalize(
            paid.allocator,
            Kind::ExpressionFinal,
            &expressions,
            function.end,
        )?,
        None => expressions
            .into_iter()
            .map(|ty| ty.expect("every expression was typed"))
            .collect(),
    };
    Ok(TypedBody {
        borrow_projections,
        expressions: final_expressions,
        projections,
        statement_projections,
        bindings: final_bindings,
        block_flows: final_flows,
    })
}

#[cfg(test)]
mod array_type_layout_tests {
    use super::*;
    #[test]
    fn projected_slice_retained_source_layouts() {
        use std::mem::size_of;
        // AST and resolved borrow arguments retain spans rather than new vectors.
        // The only new per-body enclosing allocation carrier is this sixth Vec.
        assert_eq!(size_of::<TypedBody>(), 6 * size_of::<Vec<()>>());
        println!("projected-slice-layout ast-borrow={} ast-argument={} hir-borrow={} hir-argument={} resolved-program={} typed-body={} typed-function={} sparse-entry={} field-id={} body-growth={}",
            size_of::<crate::frontend::ast::BorrowPlace>(),
            size_of::<crate::frontend::ast::Argument>(),
            size_of::<BorrowPlace>(), size_of::<Argument>(),
            size_of::<ResolvedOwnedProgram<'_>>(), size_of::<TypedBody>(),
            size_of::<TypedOwnedFunction<'_>>(), size_of::<BorrowProjection>(),
            size_of::<FieldId>(), size_of::<Vec<BorrowProjection>>());
    }
    #[test]
    fn unit3b1_owner_slot_layouts() {
        use std::mem::size_of;
        macro_rules! sizes { ($($ty:ty),* $(,)?) => { $(println!("layout {} {}", stringify!($ty), size_of::<$ty>());)* }; }
        sizes!(
            ResolvedOwnedProgram<'_>,
            TypedOwnedProgram<'_>,
            TypedOwnedFunction<'_>,
            TypedBody,
            SourceAdmission,
            Record,
            Field,
            BodyBlock,
            Argument,
            FieldInit,
            ValueTy,
            Option<ValueTy>,
            ParameterTy,
            Option<ParameterTy>,
            Projection,
            Option<Projection>,
            FlowSummary,
            Option<FlowSummary>,
            FixedArrayTy,
            Vec<ValueTy>,
            crate::frontend::project::budget::Allocator,
            crate::frontend::project::budget::ReserveEvent,
            crate::frontend::declaration_index::WorkMeter,
            (ParameterTy, Span),
            std::collections::HashMap<&str, (BindingId, Span)>
        );
    }
}

#[cfg(test)]
mod enum_source_gate_tests {
    use super::*;
    use crate::frontend::oir::owned_types::EnumId;
    use crate::frontend::{lexer, parser, source::SourceMap};

    #[test]
    fn bounded_enum_source_borrow_fence_rejects_owned_and_forwarded_types() {
        let mut sources = SourceMap::new();
        let file = sources.add(
            "enum-borrow-gate.ox".into(),
            "fn f(x:i32)->(){return;}".into(),
        );
        let source = sources.get(file);
        let ast = parser::parse_with_mode(
            source,
            lexer::lex(source).unwrap(),
            parser::SourceMode::OwnedCandidate,
        )
        .unwrap();
        let resolved = super::super::resolve::resolve(source, &ast).unwrap();
        let function = &resolved.functions()[0];
        for id in [0, usize::MAX] {
            let ty = AggregateTy::Enum(EnumId(id));
            for kind in [BorrowKind::Shared, BorrowKind::Exclusive] {
                for (place, binding) in [
                    (
                        BorrowPlace::Owner(BindingId(0)),
                        ParameterTy::Value(ValueTy::Owned(ty)),
                    ),
                    (
                        BorrowPlace::Forwarded(BindingId(0)),
                        ParameterTy::Reference {
                            referent: BorrowedTy::Exact(ty),
                            kind,
                        },
                    ),
                ] {
                    let error = borrow_type(function, kind, place, function.end, &[Some(binding)])
                        .unwrap_err();
                    assert_eq!(error.code, "E0300");
                    assert_eq!(error.primary, Some(function.end));
                }
            }
        }
    }
}

#[test]
fn c3_t1_borrowed_checker_actual_carrier_components() {
    use std::mem::{align_of, size_of};
    let core = 2 * size_of::<&ResolvedOwnedProgram<'_>>()
        + size_of::<&[Function]>()
        + size_of::<std::slice::Iter<'_, Function>>()
        + size_of::<Option<&Function>>()
        + size_of::<&Function>()
        + size_of::<Result<Vec<TypedBody>, Vec<Diagnostic>>>()
        + size_of::<Vec<TypedBody>>();
    let body = 2 * size_of::<TypedBody>() + size_of::<Result<TypedBody, Box<Diagnostic>>>();
    assert!(borrowed_check_carrier_bytes() >= core);
    assert!(borrowed_body_return_carrier_bytes() >= body);
    println!(
        "C3_T1_EARLY_LAYOUT BorrowedCheckCarriers {} {}",
        size_of::<BorrowedCheckCarriers>(),
        align_of::<BorrowedCheckCarriers>()
    );
    println!(
        "C3_T1_EARLY_LAYOUT BorrowedBodyReturnCarriers {} {}",
        size_of::<BorrowedBodyReturnCarriers>(),
        align_of::<BorrowedBodyReturnCarriers>()
    );
    println!(
        "C3_T1_EARLY_LAYOUT TypedOwnedProgram {} {}",
        size_of::<TypedOwnedProgram<'_>>(),
        align_of::<TypedOwnedProgram<'_>>()
    );
    println!(
        "C3_T1_EARLY_LAYOUT TypedOwnedFunction {} {}",
        size_of::<TypedOwnedFunction<'_>>(),
        align_of::<TypedOwnedFunction<'_>>()
    );
}

/// Assertion-only synthetic admission control. This helper never runs a paid
/// checker, establishes source provenance, or returns any owner/view/witness.
#[cfg(test)]
pub(super) fn assert_enum_observation_downstream_fences(program: ResolvedOwnedProgram<'_>) {
    use super::{budget, diagnostic as source_diagnostic, lower};
    use crate::frontend::source::SourceView;

    assert_eq!(program.admission(), SourceAdmission::ObserveEnumTypes);
    assert!(!program.admission().executable());
    assert!(!program.admission().allows_lowering());
    let errors = check_bodies(&program, None).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!((errors[0].code, errors[0].stage), ("E0500", "type"));
    assert_eq!(
        errors[0].message,
        "paid checker admission and storage disagree"
    );
    let SourceView::Map(sources) = program.index().sources().view() else {
        panic!("synthetic fence fixture must retain its own source map");
    };
    let eof = program.index().sources().eof();
    let bodies = if let Some(function) = program.functions().first() {
        assert_eq!(program.functions().len(), 1);
        assert_eq!(function.id, DefId(0));
        assert!(function.bindings.is_empty());
        assert!(function.expressions.is_empty());
        assert_eq!(function.body, BodyBlockId(0));
        assert_eq!(function.blocks.len(), 1);
        assert_eq!(function.blocks[0].body.len(), 1);
        assert!(matches!(
            function.blocks[0].body[0].kind,
            StmtKind::Return(None)
        ));
        let signature = &program.signatures()[0];
        assert!(signature.params.is_empty());
        assert_eq!(signature.result, ValueTy::Scalar(Ty::Unit));
        vec![TypedBody {
            expressions: Vec::new(),
            bindings: Vec::new(),
            block_flows: vec![FlowSummary::RETURN],
            projections: Vec::new(),
            statement_projections: vec![vec![None]],
            borrow_projections: Vec::new(),
        }]
    } else {
        assert!(program.signatures().is_empty());
        Vec::new()
    };
    let typed = TypedOwnedProgram { program, bodies };
    fn assert_fence(error: &Diagnostic, expected: Span) {
        assert_eq!(
            (
                error.code,
                error.stage,
                error.message.as_str(),
                error.primary
            ),
            (
                "E0500",
                "oir-owned-lower",
                "internal compiler error: owned invariant violation",
                Some(expected)
            ),
        );
        assert!(error.notes.is_empty());
        assert!(error.secondary.is_empty());
    }
    budget::reset_guard_counts();
    budget::fail_allocation_after(0, || {
        let errors = super::program::check_typed(&typed).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_fence(&errors[0], eof);
    });
    assert_eq!(budget::guard_counts(), [0; 7]);
    for preflight in [true, false] {
        budget::reset_guard_counts();
        let failure = budget::fail_allocation_after(0, || {
            if preflight {
                budget::preflight(&typed, budget::Limits::DEFAULT).unwrap_err()
            } else {
                lower::lower(&typed).unwrap_err()
            }
        });
        assert_fence(&source_diagnostic::lower(&failure, sources), eof);
        assert_eq!(budget::guard_counts(), [0; 7]);
    }
    for view in typed.functions() {
        budget::reset_guard_counts();
        let failure = budget::fail_allocation_after(0, || {
            lower::count_preflight_function(&view).unwrap_err()
        });
        assert_fence(
            &source_diagnostic::lower(&failure, sources),
            view.signature().span,
        );
        assert_eq!(budget::guard_counts(), [0; 7]);
        for per_block in [false, true] {
            let mut blocks = [usize::MAX; 1];
            budget::reset_guard_counts();
            let failure = budget::fail_allocation_after(0, || {
                lower::count_function(&view, per_block.then_some(&mut blocks[..])).unwrap_err()
            });
            assert_fence(
                &source_diagnostic::lower(&failure, sources),
                view.signature().span,
            );
            assert_eq!(blocks, [usize::MAX; 1]);
            assert_eq!(budget::guard_counts(), [0; 7]);
        }
        budget::reset_guard_counts();
        let failure = budget::fail_allocation_after(0, || {
            lower::check_array_type_emission_fence(&view).unwrap_err()
        });
        assert_fence(
            &source_diagnostic::lower(&failure, sources),
            view.signature().span,
        );
        assert_eq!(budget::guard_counts(), [0; 7]);
    }
    drop(typed);
}

#[test]
fn c3_t1_disconnected_paid_context_actual_layouts() {
    use std::mem::{align_of, size_of};
    macro_rules! layout {
        ($($ty:ty),* $(,)?) => { $(
            println!("C3_T1_PAID_CONTEXT_LAYOUT {} {} {}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        )* };
    }
    layout!(
        ProgramPaid<'static, 'static>,
        BodyPaid<'static>,
        Option<&'static mut ProgramPaid<'static, 'static>>,
        Option<&'static mut BodyPaid<'static>>,
        PaidProgramControls,
        PaidBodyControls,
        PaidBodyReceivers,
        PaidRowReceiver,
        PaidExpressionReborrows,
    );
    assert_eq!(
        size_of::<ProgramPaid<'_, '_>>(),
        size_of::<&mut storage::TypePlan<'_>>()
            + size_of::<&mut Allocator>()
            + size_of::<&std::cell::Cell<usize>>()
    );
    assert_eq!(
        size_of::<BodyPaid<'_>>(),
        size_of::<&mut storage::FunctionQuota>()
            + size_of::<&mut Allocator>()
            + size_of::<&std::cell::Cell<usize>>()
    );
    assert_eq!(
        size_of::<PaidBodyReceivers>(),
        size_of::<Vec<Option<Projection>>>()
            + 2 * size_of::<Vec<Vec<Option<Projection>>>>()
            + size_of::<Vec<BorrowProjection>>()
            + size_of::<Vec<ParameterTy>>()
            + size_of::<Vec<FlowSummary>>()
            + size_of::<Vec<ValueTy>>()
    );
    assert_eq!(
        size_of::<PaidRowReceiver>(),
        size_of::<Vec<Option<Projection>>>()
    );
}

#[test]
fn c3_t1_exact_semantic_iterator_factories_are_sized_without_invocation_or_heap() {
    let (layout, measured) = super::reviewer_source::integration_measured(semantic_iterator_layout);
    assert_eq!(measured, (0, 0, 0));
    assert!(layout[0] > layout[3] + layout[4] + layout[5] + layout[6]);
    assert_eq!((layout[7], layout[8], layout[9]), (0, 0, 0));
    assert_eq!(layout[2], 4 * std::mem::size_of::<[usize; 11]>());
    println!("C3_T1_SEMANTIC_ITERATOR_LAYOUT bank={} align={} witness={} window={} identifiers={} field_predicate={} start_predicate={} identifier_predicate={} sort_key={} scalar_item={} end_predicate={}",
        layout[0], layout[1], layout[2], layout[3], layout[4], layout[5], layout[6], layout[7], layout[8], layout[9], layout[10]);
}

#[test]
fn c3_t1_remaining_explicit_semantic_banks_are_measurement_only() {
    use semantic_carriers::*;
    use std::mem::{align_of, size_of};
    macro_rules! layout {
        ($($ty:ty),* $(,)?) => { $(
            println!("C3_T1_SEMANTIC_BANK_LAYOUT {} {} {}", stringify!($ty), size_of::<$ty>(), align_of::<$ty>());
        )* };
    }
    layout!(
        ProjectionSemanticCarriers,
        PredicateInvocationCarriers,
        CallSemanticCarriers,
        LiteralSemanticCarriers,
        StatementPatternCarriers,
        BodyFrameSemanticCarriers,
        InitializerSemanticCarriers,
        ExpressionSemanticCarriers
    );
    let (banks, measured) = super::reviewer_source::integration_measured(declared_bank_sizes);
    assert_eq!(measured, (0, 0, 0));
    assert!(banks.iter().all(|bytes| *bytes > 0));
    let (map_or, measured) = super::reviewer_source::integration_measured(semantic_map_or_layout);
    assert_eq!(measured, (0, 0, 0));
    assert_eq!(map_or[2], 4 * size_of::<[usize; 5]>());
    assert_eq!(map_or[3], size_of::<&[Option<FlowSummary>]>());
    assert_eq!(map_or[4], size_of::<&[Option<ValueTy>]>());
    println!("C3_T1_SEMANTIC_BANK_LAYOUT map_or={} align={} witness={} else_closure={} return_closure={}",
        map_or[0], map_or[1], map_or[2], map_or[3], map_or[4]);
}
