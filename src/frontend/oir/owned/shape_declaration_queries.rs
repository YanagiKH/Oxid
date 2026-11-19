//! Concrete static declaration queries for the one shared signature proof.
//! The scoped form consumes facts synchronously; it exposes no reusable proof.
use super::queries::AggregateFacts;
use super::*;

mod sealed {
    pub trait SignatureQueries {}
}

pub(in crate::frontend::oir::owned) trait SignatureDeclarationQueries:
    sealed::SignatureQueries
{
    fn authenticates_program(&self, raw: &RawOwnedProgram) -> bool;
    fn has_resource_declarations(&self) -> bool;
    fn permits_owned_vectors(&self) -> bool;
    fn check_value_type(&mut self, ty: ValueTy) -> Result<(), OwnedFailure>;
    fn check_aggregate_type(&mut self, ty: AggregateTy) -> Result<(), OwnedFailure>;
    fn check_borrowed_type(&mut self, ty: BorrowedTy) -> Result<(), OwnedFailure>;
}

pub(in crate::frontend::oir::owned) trait ShapeDeclarationQueries<'facade>:
    SignatureDeclarationQueries
{
    fn core(&self) -> &'facade Declarations;
    fn authenticates_function(&self, function: &RawOwnedFunction) -> bool;
    fn fixed_builtin_base(&self, raw: &RawOwnedProgram) -> Option<usize>;
    fn old_vector_operation(
        &self,
        raw: &RawOwnedProgram,
        id: hir::DefId,
    ) -> Option<vector::Function>;
    fn owned_vector_operation(&self, id: hir::DefId) -> Option<Function>;
    fn same_aggregate_type(
        &mut self,
        actual: AggregateTy,
        expected: AggregateTy,
    ) -> Result<(), OwnedFailure>;
    fn same_value_type(&mut self, actual: ValueTy, expected: ValueTy) -> Result<(), OwnedFailure>;
    fn same_borrowed_type(
        &mut self,
        actual: BorrowedTy,
        expected: BorrowedTy,
    ) -> Result<(), OwnedFailure>;
    fn check_borrowed_view(
        &mut self,
        authority: BorrowedTy,
        target: BorrowedTy,
    ) -> Result<(), OwnedFailure>;
    fn projection(
        &mut self,
        root: AggregateTy,
        path: &[FieldId],
    ) -> Result<(ValueTy, usize), OwnedFailure>;
    fn aggregate_layout(&mut self, aggregate: AggregateTy) -> Result<Layout, OwnedFailure>;
    fn aggregate_width(&mut self, aggregate: AggregateTy) -> Result<usize, OwnedFailure>;
}

impl sealed::SignatureQueries for &Declarations {}
impl SignatureDeclarationQueries for &Declarations {
    fn authenticates_program(&self, _: &RawOwnedProgram) -> bool {
        true
    }
    fn has_resource_declarations(&self) -> bool {
        Declarations::has_resource_declarations(self)
    }
    fn permits_owned_vectors(&self) -> bool {
        false
    }
    fn check_value_type(&mut self, ty: ValueTy) -> Result<(), OwnedFailure> {
        Declarations::check_value_type(self, ty).map_err(Into::into)
    }
    fn check_aggregate_type(&mut self, ty: AggregateTy) -> Result<(), OwnedFailure> {
        Declarations::check_aggregate_type(self, ty).map_err(Into::into)
    }
    fn check_borrowed_type(&mut self, ty: BorrowedTy) -> Result<(), OwnedFailure> {
        Declarations::check_borrowed_type(self, ty).map_err(Into::into)
    }
}

impl<'facade> ShapeDeclarationQueries<'facade> for &'facade Declarations {
    fn core(&self) -> &'facade Declarations {
        self
    }
    fn authenticates_function(&self, _: &RawOwnedFunction) -> bool {
        true
    }
    fn fixed_builtin_base(&self, raw: &RawOwnedProgram) -> Option<usize> {
        raw.fixed_builtin_base()
    }
    fn old_vector_operation(
        &self,
        raw: &RawOwnedProgram,
        id: hir::DefId,
    ) -> Option<vector::Function> {
        raw.vector_operation_claim(id)
    }
    fn owned_vector_operation(&self, _: hir::DefId) -> Option<Function> {
        None
    }
    fn same_aggregate_type(
        &mut self,
        actual: AggregateTy,
        expected: AggregateTy,
    ) -> Result<(), OwnedFailure> {
        Declarations::same_aggregate_type(self, actual, expected).map_err(Into::into)
    }
    fn same_value_type(&mut self, actual: ValueTy, expected: ValueTy) -> Result<(), OwnedFailure> {
        Declarations::same_value_type(self, actual, expected).map_err(Into::into)
    }
    fn same_borrowed_type(
        &mut self,
        actual: BorrowedTy,
        expected: BorrowedTy,
    ) -> Result<(), OwnedFailure> {
        Declarations::same_borrowed_type(self, actual, expected).map_err(Into::into)
    }
    fn check_borrowed_view(
        &mut self,
        authority: BorrowedTy,
        target: BorrowedTy,
    ) -> Result<(), OwnedFailure> {
        Declarations::check_borrowed_view(self, authority, target).map_err(Into::into)
    }
    fn projection(
        &mut self,
        root: AggregateTy,
        path: &[FieldId],
    ) -> Result<(ValueTy, usize), OwnedFailure> {
        Declarations::projection(self, root, path).map_err(Into::into)
    }
    fn aggregate_layout(&mut self, aggregate: AggregateTy) -> Result<Layout, OwnedFailure> {
        Declarations::aggregate_layout(self, aggregate).map_err(Into::into)
    }
    fn aggregate_width(&mut self, aggregate: AggregateTy) -> Result<usize, OwnedFailure> {
        Declarations::aggregate_width(self, aggregate).map_err(Into::into)
    }
}

trait FactOutput: Copy {}
impl FactOutput for () {}
impl FactOutput for Layout {}
impl FactOutput for usize {}

pub(in crate::frontend::oir::owned) struct ScopedSignatureQueries<'context, 'program, 'budget> {
    context: &'context OwnedVectorContext<'program>,
    remaining: &'budget mut RemainingContextBudget<'program>,
    declarations: &'program Declarations,
    query_metadata: usize,
}

struct ScopeControls {
    context: &'static OwnedVectorContext<'static>,
    budget: &'static mut RemainingContextBudget<'static>,
    facade: &'static Declarations,
    facade_option: Option<&'static Declarations>,
    facade_result: Result<&'static Declarations, OwnedFailure>,
    query_request: Result<(usize, usize), OwnedFailure>,
    request: (usize, usize),
    request_result: Result<(usize, usize), OwnedFailure>,
    caller_request: Result<(usize, usize), OwnedFailure>,
    scoped: ScopedSignatureQueries<'static, 'static, 'static>,
    scope_result: Result<ScopedSignatureQueries<'static, 'static, 'static>, OwnedFailure>,
    caller_scope_result: Result<ScopedSignatureQueries<'static, 'static, 'static>, OwnedFailure>,
    query_argument: &'static mut ScopedSignatureQueries<'static, 'static, 'static>,
    raw: &'static RawOwnedProgram,
    sources: &'static SourceMap,
    signature_return: Result<(), OwnedFailure>,
    signature_caller_return: Result<(), OwnedFailure>,
    aggregate: AggregateTy,
    value: ValueTy,
    borrowed: BorrowedTy,
    facts_return: Result<AggregateFacts<'static, 'static>, OwnedFailure>,
    consumed_return: Result<(), OwnedFailure>,
    caller_consumed_return: Result<(), OwnedFailure>,
    old_borrowed_result: Result<(), DeclarationError>,
    mapped_borrowed_result: Result<(), OwnedFailure>,
    declaration_failure: DeclarationError,
    selected_failure: OwnedFailure,
    checked_metadata: [Option<usize>; 2],
    checked_work: Option<usize>,
    selected_counts: [usize; 4],
    remaining_return: (usize, usize),
    remaining_caller_return: (usize, usize),
    identities: [bool; 2],
    layouts: [Layout; 2],
    layout_returns: [Result<Layout, OwnedFailure>; 2],
    widths: [usize; 2],
    width_returns: [Result<usize, OwnedFailure>; 2],
    aggregates: [AggregateTy; 2],
    values: [ValueTy; 2],
    borrowed_types: [BorrowedTy; 2],
    projection_path: &'static [FieldId],
    projection_return: Result<(ValueTy, usize), DeclarationError>,
    mapped_projection_return: Result<(ValueTy, usize), OwnedFailure>,
    selected_function: &'static RawOwnedFunction,
    function_option: Option<&'static RawOwnedFunction>,
    old_key: Option<vector::Function>,
    new_key: Option<Function>,
    function: hir::DefId,
    builtin_base: Option<usize>,
    core_return: &'static Declarations,
    core_caller_return: &'static Declarations,
    canonical_destination: CallResult,
    canonical_owners: &'static mut [shape::OwnerSites],
    canonical_site: shape::Site,
    canonical_span: Span,
    canonical_return: Result<(), OwnedFailure>,
    canonical_caller_return: Result<(), OwnedFailure>,
    shape: shape::Shape,
    shape_returns: [Result<shape::Shape, OwnedFailure>; 2],
    meter_argument: &'static mut budget::Meter,
    source_associated: bool,
    cfg_return: Result<(), OirFailure>,
    mapped_cfg_return: Result<(), OwnedFailure>,
    flow_return: Result<(), OwnedFailure>,
}

impl<'context, 'program, 'budget> ScopedSignatureQueries<'context, 'program, 'budget> {
    pub(in crate::frontend::oir::owned) fn requested() -> Result<(usize, usize), OwnedFailure> {
        let (query, _) = OwnedVectorContext::aggregate_query_request()?;
        Ok((add(size_of::<ScopeControls>(), query)?, 4))
    }

    pub(in crate::frontend::oir::owned) fn new(
        context: &'context OwnedVectorContext<'program>,
        remaining: &'budget mut RemainingContextBudget<'program>,
    ) -> Result<Self, OwnedFailure> {
        if !std::ptr::eq(context.wrapper, remaining.wrapper) {
            return Err(bad());
        }
        let declarations = context.wrapper.declarations.as_ref().ok_or_else(bad)?;
        let (metadata, work) = Self::requested()?;
        let metadata = remaining
            .metadata
            .checked_sub(metadata)
            .ok_or_else(|| resource("owned vector signature query metadata"))?;
        let work = remaining
            .work
            .checked_sub(work)
            .ok_or_else(|| resource("owned vector signature query work"))?;
        let (query_metadata, _) = OwnedVectorContext::aggregate_query_request()?;
        remaining.metadata = metadata;
        remaining.work = work;
        Ok(Self {
            context,
            remaining,
            declarations,
            query_metadata,
        })
    }

    pub(in crate::frontend::oir::owned) fn remaining(&self) -> (usize, usize) {
        self.remaining.remaining()
    }

    fn aggregate(&mut self, aggregate: AggregateTy) -> Result<(), OwnedFailure> {
        self.aggregate_value(aggregate, |_| ())
    }

    fn aggregate_value<T: FactOutput>(
        &mut self,
        aggregate: AggregateTy,
        select: impl FnOnce(AggregateFacts<'_, 'program>) -> T,
    ) -> Result<T, OwnedFailure> {
        let held_metadata = self.remaining.metadata;
        // Lend only this scope's already prepaid synchronous bank. The ledger
        // is exclusively borrowed until the bank is reclaimed below.
        self.remaining.metadata = held_metadata
            .checked_add(self.query_metadata)
            .ok_or_else(bad)?;
        let consumed = {
            let facts = self.context.aggregate_facts(aggregate, self.remaining);
            facts.map(select)
        };
        // No AggregateFacts escapes: both successful facts and failure-only
        // transports have ceased to use the temporary bank before refund.
        self.remaining.metadata = held_metadata;
        consumed
    }

    fn scalar_visit(&mut self) -> Result<(), OwnedFailure> {
        self.remaining.work = self
            .remaining
            .work
            .checked_sub(1)
            .ok_or_else(|| resource("owned vector signature query work"))?;
        Ok(())
    }
}

impl<'program> ShapeDeclarationQueries<'program> for ScopedSignatureQueries<'_, 'program, '_> {
    fn core(&self) -> &'program Declarations {
        self.declarations
    }
    fn authenticates_function(&self, function: &RawOwnedFunction) -> bool {
        self.context
            .wrapper
            .program
            .functions
            .get(function.id.0)
            .is_some_and(|actual| std::ptr::eq(actual, function))
    }
    fn fixed_builtin_base(&self, _: &RawOwnedProgram) -> Option<usize> {
        self.context.fixed_builtin_base()
    }
    fn old_vector_operation(
        &self,
        _: &RawOwnedProgram,
        id: hir::DefId,
    ) -> Option<vector::Function> {
        self.context.old_vector_operation(id)
    }
    fn owned_vector_operation(&self, id: hir::DefId) -> Option<Function> {
        self.context.owned_vector_operation(id)
    }
    fn same_aggregate_type(
        &mut self,
        actual: AggregateTy,
        expected: AggregateTy,
    ) -> Result<(), OwnedFailure> {
        self.aggregate(actual)?;
        self.aggregate(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch.into())
        }
    }
    fn same_value_type(&mut self, actual: ValueTy, expected: ValueTy) -> Result<(), OwnedFailure> {
        self.check_value_type(actual)?;
        self.check_value_type(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch.into())
        }
    }
    fn same_borrowed_type(
        &mut self,
        actual: BorrowedTy,
        expected: BorrowedTy,
    ) -> Result<(), OwnedFailure> {
        self.check_borrowed_type(actual)?;
        self.check_borrowed_type(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch.into())
        }
    }
    fn check_borrowed_view(
        &mut self,
        authority: BorrowedTy,
        target: BorrowedTy,
    ) -> Result<(), OwnedFailure> {
        self.check_borrowed_type(authority)?;
        self.check_borrowed_type(target)?;
        if target.accepts(authority) {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch.into())
        }
    }
    fn projection(
        &mut self,
        root: AggregateTy,
        path: &[FieldId],
    ) -> Result<(ValueTy, usize), OwnedFailure> {
        self.scalar_visit()?;
        self.declarations.projection(root, path).map_err(Into::into)
    }
    fn aggregate_layout(&mut self, aggregate: AggregateTy) -> Result<Layout, OwnedFailure> {
        self.aggregate_value(aggregate, |facts| facts.layout())
    }
    fn aggregate_width(&mut self, aggregate: AggregateTy) -> Result<usize, OwnedFailure> {
        self.aggregate_value(aggregate, |facts| facts.width())
    }
}

impl sealed::SignatureQueries for ScopedSignatureQueries<'_, '_, '_> {}
impl SignatureDeclarationQueries for ScopedSignatureQueries<'_, '_, '_> {
    fn authenticates_program(&self, raw: &RawOwnedProgram) -> bool {
        std::ptr::eq(raw, &self.context.wrapper.program)
    }
    fn has_resource_declarations(&self) -> bool {
        self.declarations.has_resource_declarations()
    }
    fn permits_owned_vectors(&self) -> bool {
        true
    }
    fn check_value_type(&mut self, ty: ValueTy) -> Result<(), OwnedFailure> {
        match ty {
            ValueTy::Scalar(_) => self.scalar_visit(),
            ValueTy::Owned(aggregate) => self.aggregate(aggregate),
        }
    }
    fn check_aggregate_type(&mut self, ty: AggregateTy) -> Result<(), OwnedFailure> {
        self.aggregate(ty)
    }
    fn check_borrowed_type(&mut self, ty: BorrowedTy) -> Result<(), OwnedFailure> {
        match ty {
            BorrowedTy::Exact(AggregateTy::Enum(_)) => {
                self.scalar_visit()?;
                Err(DeclarationError::TypeMismatch.into())
            }
            BorrowedTy::Exact(aggregate) => self.aggregate(aggregate),
            BorrowedTy::ScalarSlice(_) | BorrowedTy::RecordSlice(_) => {
                self.scalar_visit()?;
                self.declarations
                    .check_borrowed_type(ty)
                    .map_err(Into::into)
            }
        }
    }
}
