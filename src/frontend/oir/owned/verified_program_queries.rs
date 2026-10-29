//! Static queries over the two real sealed owners. This is not another
//! verifier, declaration facade or execution permission.
use super::*;
use crate::frontend::builtin_catalog::{experimental, owned_vector, vector};
use crate::frontend::oir::owned::owned_record_codec::bounds::CheckedElementView;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::VerifiedOwnedProgram {}
    impl Sealed for super::VerifiedOwnedVectorProgram {}
}

pub(in crate::frontend::oir::owned) trait PlanWitness:
    sealed::Sealed
{
    fn raw_program(&self) -> &RawOwnedProgram;
    fn declarations(&self) -> &Declarations;
    fn fixed_builtin_base(&self) -> Option<usize>;
    fn usage(&self) -> OwnershipUsage;
    fn aggregate_layout(&self, aggregate: AggregateTy) -> Result<Layout, DeclarationError>;
    fn aggregate_width(&self, aggregate: AggregateTy) -> Result<usize, DeclarationError>;
    fn vector_operation(&self, id: hir::DefId) -> Option<vector::Function>;
    fn owned_vector_operation(&self, id: hir::DefId) -> Option<owned_vector::Function>;
    fn uses_owned_vectors(&self) -> bool;

    fn elements(&self) -> Option<CheckedElementView<'_>>;
    fn contains_resources(&self, aggregate: AggregateTy) -> Result<bool, DeclarationError>;
    fn has_enum_record_nesting(&self, aggregate: AggregateTy) -> Result<bool, DeclarationError> {
        if self.uses_owned_vectors() && matches!(aggregate, AggregateTy::OwnedRecordVector(_)) {
            self.aggregate_layout(aggregate)?;
            Ok(false)
        } else {
            self.declarations().has_enum_record_nesting(aggregate)
        }
    }
    fn same_aggregate_type(
        &self,
        actual: AggregateTy,
        expected: AggregateTy,
    ) -> Result<(), DeclarationError> {
        if !self.uses_owned_vectors() {
            return self.declarations().same_aggregate_type(actual, expected);
        }
        self.aggregate_layout(actual)?;
        self.aggregate_layout(expected)?;
        if actual == expected {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch)
        }
    }
    fn check_borrowed_view(
        &self,
        authority: BorrowedTy,
        target: BorrowedTy,
    ) -> Result<(), DeclarationError> {
        if !self.uses_owned_vectors() {
            return self.declarations().check_borrowed_view(authority, target);
        }
        for borrowed in [authority, target] {
            match borrowed {
                BorrowedTy::Exact(aggregate @ AggregateTy::OwnedRecordVector(_)) => {
                    self.aggregate_layout(aggregate)?;
                }
                _ => self.declarations().check_borrowed_type(borrowed)?,
            }
        }
        if target.accepts(authority) {
            Ok(())
        } else {
            Err(DeclarationError::TypeMismatch)
        }
    }
    fn resource_leaves<F>(
        &self,
        aggregate: AggregateTy,
        read_tag: F,
    ) -> Result<ResourceLeaves<'_, F>, DeclarationError>
    where
        F: FnMut(usize) -> Result<usize, DeclarationError>;

    fn projection_permission(
        &self,
        function: hir::DefId,
        block: BlockId,
        statement: usize,
        request: ProjectionRequest<'_>,
    ) -> Option<ProjectionPermission<'_, Self>>
    where
        Self: Sized,
    {
        super::projection_permission(self, function, block, statement, request)
    }

    fn functions(&self) -> &[RawOwnedFunction] {
        &self.raw_program().functions
    }
    fn experimental_function(&self, item: experimental::Function) -> Option<hir::DefId> {
        let rank = self
            .raw_program()
            .builtins
            .experimental_function_rank(item)?;
        self.fixed_builtin_base()?.checked_add(rank).map(hir::DefId)
    }
    fn has_experimental_operations(&self) -> bool {
        self.raw_program().builtins.experimental().functions() != 0
            || !self.raw_program().vector_functions.is_empty()
    }
    fn builtin_function(&self) -> Option<hir::DefId> {
        let rank = self
            .raw_program()
            .builtins
            .function_rank(BuiltinFunction::ReadStdin)?;
        self.fixed_builtin_base()?.checked_add(rank).map(hir::DefId)
    }
    fn builtin_output_function(&self) -> Option<hir::DefId> {
        let rank = self
            .raw_program()
            .builtins
            .function_rank(BuiltinFunction::WriteStdout)?;
        self.fixed_builtin_base()?.checked_add(rank).map(hir::DefId)
    }
    fn builtin_enumeration(&self) -> Option<EnumId> {
        self.builtin_enum(BuiltinEnum::ReadStatus)
    }
    fn builtin_output_enumeration(&self) -> Option<EnumId> {
        self.builtin_enum(BuiltinEnum::WriteStatus)
    }
    fn builtin_enum(&self, item: BuiltinEnum) -> Option<EnumId> {
        let program = self.raw_program();
        let rank = program.builtins.enum_rank(item)?;
        program
            .enums
            .len()
            .checked_sub(program.builtins.extra_enums())?
            .checked_add(rank)
            .map(EnumId)
    }
    fn has_builtin_origins(&self) -> bool {
        self.raw_program().builtins != BuiltinOrigins::None
    }
}

impl PlanWitness for VerifiedOwnedProgram {
    fn raw_program(&self) -> &RawOwnedProgram {
        &self.program
    }
    fn declarations(&self) -> &Declarations {
        &self.declarations
    }
    fn fixed_builtin_base(&self) -> Option<usize> {
        self.program.fixed_builtin_base()
    }
    fn usage(&self) -> OwnershipUsage {
        self.usage
    }
    fn aggregate_layout(&self, aggregate: AggregateTy) -> Result<Layout, DeclarationError> {
        self.declarations.aggregate_layout(aggregate)
    }
    fn aggregate_width(&self, aggregate: AggregateTy) -> Result<usize, DeclarationError> {
        self.declarations.aggregate_width(aggregate)
    }
    fn vector_operation(&self, id: hir::DefId) -> Option<vector::Function> {
        self.program.vector_operation_claim(id)
    }
    fn owned_vector_operation(&self, _: hir::DefId) -> Option<owned_vector::Function> {
        None
    }
    fn uses_owned_vectors(&self) -> bool {
        false
    }
    fn elements(&self) -> Option<CheckedElementView<'_>> {
        None
    }
    fn contains_resources(&self, aggregate: AggregateTy) -> Result<bool, DeclarationError> {
        self.declarations.contains_resources(aggregate)
    }
    fn resource_leaves<F>(
        &self,
        aggregate: AggregateTy,
        read_tag: F,
    ) -> Result<ResourceLeaves<'_, F>, DeclarationError>
    where
        F: FnMut(usize) -> Result<usize, DeclarationError>,
    {
        self.declarations.resource_leaves(aggregate, read_tag)
    }
}

impl PlanWitness for VerifiedOwnedVectorProgram {
    fn raw_program(&self) -> &RawOwnedProgram {
        Self::raw_program(self)
    }
    fn declarations(&self) -> &Declarations {
        Self::declarations(self)
    }
    fn fixed_builtin_base(&self) -> Option<usize> {
        Self::fixed_builtin_base(self)
    }
    fn usage(&self) -> OwnershipUsage {
        Self::usage(self)
    }
    fn aggregate_layout(&self, aggregate: AggregateTy) -> Result<Layout, DeclarationError> {
        Self::aggregate_layout(self, aggregate)
    }
    fn aggregate_width(&self, aggregate: AggregateTy) -> Result<usize, DeclarationError> {
        Self::aggregate_width(self, aggregate)
    }
    fn vector_operation(&self, id: hir::DefId) -> Option<vector::Function> {
        Self::vector_operation(self, id)
    }
    fn owned_vector_operation(&self, id: hir::DefId) -> Option<owned_vector::Function> {
        Self::owned_vector_operation(self, id)
    }
    fn uses_owned_vectors(&self) -> bool {
        true
    }
    fn elements(&self) -> Option<CheckedElementView<'_>> {
        Some(Self::elements(self))
    }
    fn contains_resources(&self, aggregate: AggregateTy) -> Result<bool, DeclarationError> {
        if matches!(aggregate, AggregateTy::OwnedRecordVector(_)) {
            self.aggregate_layout(aggregate)?;
            Ok(true)
        } else {
            self.declarations().contains_resources(aggregate)
        }
    }
    fn resource_leaves<F>(
        &self,
        aggregate: AggregateTy,
        read_tag: F,
    ) -> Result<ResourceLeaves<'_, F>, DeclarationError>
    where
        F: FnMut(usize) -> Result<usize, DeclarationError>,
    {
        self.aggregate_layout(aggregate)?;
        Ok(self
            .declarations()
            .resource_leaves_for_checked_type(aggregate, read_tag))
    }
}
