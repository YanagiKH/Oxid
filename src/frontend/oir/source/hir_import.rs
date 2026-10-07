//! Denied, source-bound observation probe. Framing is not semantic validation.
//! No candidate constructor, checker, lowerer or executable consumer is called.
use crate::frontend::{
    ast,
    declaration_index::SourceOwner,
    diagnostic::Diagnostic,
    hir,
    project::{ModuleId, SyntaxFlavor},
};
use std::{convert::Infallible, mem::size_of};

// Complete source/OPA comparison remains disconnected from candidate construction.
mod ast_compare;
// Prepaid vector helper remains disconnected from any candidate Program builder.
mod allocation;
// Contained candidate helper is compiled for layout review; consumers remain disconnected.
mod candidate;
// Still-denied complete comparison leaf is uninvoked pending boundary review.
mod leaf;

const CELLS: usize = 129;
const MAX_ROWS: usize = 128;
const OPA_BYTES: usize = 1559;
const SUCCESS_BYTES: usize = 2607;
const COLUMN_STARTS: [usize; 5] = [11, 527, 1043, 1575, 2091];

#[derive(Debug, PartialEq, Eq)]
enum Boundary {
    Source,
    Domain,
    Frame,
    Overflow,
}
#[derive(Debug)]
enum Rejection {
    Boundary(Boundary),
    Canonical(Vec<Diagnostic>),
    Disabled(ProbeFacts),
}
impl From<Boundary> for Rejection {
    fn from(value: Boundary) -> Self {
        Self::Boundary(value)
    }
}
#[derive(Clone, Copy)]
struct Wire<'w> {
    bytes: &'w [u8],
    rows: u8,
}
impl<'w> Wire<'w> {
    fn decode(bytes: &'w [u8], source_len: usize) -> Result<Self, Boundary> {
        if bytes.len() != SUCCESS_BYTES
            || &bytes[..4] != b"OPA1"
            || bytes[4..8] != [0; 4]
            || usize::from(bytes[8]) > MAX_ROWS
            || usize::from(bytes[10]) != source_len
            || &bytes[OPA_BYTES..OPA_BYTES + 4] != b"STF1"
            || bytes[OPA_BYTES + 4] != 0
            || bytes[OPA_BYTES + 5] != bytes[8]
            || bytes[OPA_BYTES + 6..OPA_BYTES + 16] != [0; 10]
        {
            return Err(Boundary::Frame);
        }
        let rows = bytes[8];
        if (rows == 0) != (bytes[9] == 0) || bytes[9] > rows {
            return Err(Boundary::Frame);
        }
        let wire = Self { bytes, rows };
        // Only reserved/inactive storage is interpreted at this precursor.
        // Active row ownership, source correspondence and semantics are pending.
        for column in 0..COLUMN_STARTS.len() {
            for cell in usize::from(rows)..CELLS {
                if wire.word(column, cell)? != 0 {
                    return Err(Boundary::Frame);
                }
            }
        }
        Ok(wire)
    }
    fn word(self, column: usize, cell: usize) -> Result<i32, Boundary> {
        let start = *COLUMN_STARTS.get(column).ok_or(Boundary::Frame)?;
        if cell >= CELLS {
            return Err(Boundary::Frame);
        }
        let mut value = [0; 4];
        for (plane, byte) in value.iter_mut().enumerate() {
            let at = start
                .checked_add(plane.checked_mul(CELLS).ok_or(Boundary::Overflow)?)
                .and_then(|n| n.checked_add(cell))
                .ok_or(Boundary::Overflow)?;
            *byte = *self.bytes.get(at).ok_or(Boundary::Frame)?;
        }
        Ok(i32::from_le_bytes(value))
    }
}
fn scalar_type(ty: ast::TypeSyntax) -> bool {
    matches!(
        ty.kind,
        ast::TypeSyntaxKind::Unit | ast::TypeSyntaxKind::Name(ast::ItemPath::Unqualified(_))
    )
}
fn bounded_ast_domain(program: &ast::Program) -> Result<(), Boundary> {
    if program.tokens.len() > CELLS
        || program.items.len() > MAX_ROWS
        || !program.records.is_empty()
        || !program.enums.is_empty()
        || !program.modules.is_empty()
        || !program.imports.is_empty()
        || !program.paths.is_empty()
        || !program.path_segments.is_empty()
    {
        return Err(Boundary::Domain);
    }
    // Exact unified scalar-row count; this is a domain bound, not wire equality.
    let mut rows = program.expressions.len();
    let mut add = |amount: usize| -> Result<(), Boundary> {
        rows = rows
            .checked_add(amount)
            .filter(|&n| n <= MAX_ROWS)
            .ok_or(Boundary::Domain)?;
        Ok(())
    };
    add(program.functions.len())?;
    for function in &program.functions {
        add(1)?; // result type
        add(function
            .params
            .len()
            .checked_mul(2)
            .ok_or(Boundary::Overflow)?)?;
        if !scalar_type(function.result) || function.params.iter().any(|p| !scalar_type(p.ty)) {
            return Err(Boundary::Domain);
        }
        add(function.blocks.len())?;
        for block in &function.blocks {
            add(block.body.len())?;
            for statement in &block.body {
                match statement.kind {
                    ast::StmtKind::Let { annotation, .. } => {
                        if let Some(ty) = annotation {
                            if !scalar_type(ty) {
                                return Err(Boundary::Domain);
                            }
                            add(1)?;
                        }
                    }
                    ast::StmtKind::Assign { .. }
                    | ast::StmtKind::Expr(_)
                    | ast::StmtKind::Return(_)
                    | ast::StmtKind::Break
                    | ast::StmtKind::Continue
                    | ast::StmtKind::While { .. }
                    | ast::StmtKind::If { .. } => (),
                    ast::StmtKind::FieldAssign { .. }
                    | ast::StmtKind::IndexAssign { .. }
                    | ast::StmtKind::Match { .. } => return Err(Boundary::Domain),
                }
            }
        }
    }
    let mut arguments = 0usize;
    for expression in &program.expressions {
        match &expression.kind {
            ast::ExprKind::Negate { .. }
            | ast::ExprKind::Not { .. }
            | ast::ExprKind::Logical { .. }
            | ast::ExprKind::Comparison { .. }
            | ast::ExprKind::Bool(_)
            | ast::ExprKind::Number { .. }
            | ast::ExprKind::Unit
            | ast::ExprKind::Name(_)
            | ast::ExprKind::Group(_)
            | ast::ExprKind::Arithmetic { .. } => (),
            ast::ExprKind::Call {
                callee: ast::ItemPath::Unqualified(_),
                args,
            } => {
                arguments = arguments
                    .checked_add(args.len())
                    .filter(|&n| n <= MAX_ROWS)
                    .ok_or(Boundary::Domain)?;
                if !args
                    .iter()
                    .all(|arg| matches!(arg, ast::Argument::Value(_)))
                {
                    return Err(Boundary::Domain);
                }
            }
            _ => return Err(Boundary::Domain),
        }
    }
    Ok(())
}

struct BoundObservation<'s, 'w> {
    owner: SourceOwner<'s>,
    wire: Wire<'w>,
    source: &'s [u8],
}
impl<'s, 'w> BoundObservation<'s, 'w> {
    fn bind(
        owner: SourceOwner<'s>,
        captured_source: &[u8],
        bytes: &'w [u8],
    ) -> Result<Self, Boundary> {
        if owner.count() != 1 {
            return Err(Boundary::Domain);
        }
        let source = owner.file(ModuleId(0)).map_err(|_| Boundary::Source)?;
        let ast = owner.ast(ModuleId(0)).map_err(|_| Boundary::Source)?;
        if owner.flavor() == SyntaxFlavor::OriginalSingleFile && ast.uses_project_syntax() {
            return Err(Boundary::Source);
        }
        if captured_source.len() != source.text().len() {
            return Err(Boundary::Source);
        }
        // Reject oversized equal-length inputs before inspecting their bytes.
        // The private boundary therefore has a bounded equality/ASCII scan.
        if captured_source.len() > MAX_ROWS {
            return Err(Boundary::Domain);
        }
        if captured_source != source.text().as_bytes() {
            return Err(Boundary::Source);
        }
        if !captured_source.is_ascii() {
            return Err(Boundary::Domain);
        }
        bounded_ast_domain(ast)?;
        // Public fields are mutable inside the compiler; a parser-owned AST
        // can have a stale cached syntax summary. The bounded scalar scan must
        // independently retain the genuine project-route requirement.
        if owner.flavor() == SyntaxFlavor::OriginalSingleFile
            && ast
                .functions
                .iter()
                .any(|function| function.public.is_some())
        {
            return Err(Boundary::Source);
        }
        let wire = Wire::decode(bytes, captured_source.len())?;
        Ok(Self {
            owner,
            wire,
            source: source.text().as_bytes(),
        })
    }
}

// Eight complete retained HIR payload families. Nested Vec headers are already
// included in their containing element; only Program's header is separate.
const ELEMENT_BYTES: [usize; 8] = [
    size_of::<hir::Signature>(),
    size_of::<hir::Function>(),
    size_of::<hir::Ty>(),
    size_of::<hir::Local>(),
    size_of::<hir::Expr>(),
    size_of::<hir::BodyBlock>(),
    size_of::<hir::Stmt>(),
    size_of::<hir::ExprId>(),
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Counts([usize; 8]);
impl Counts {
    fn add(&mut self, family: usize, amount: usize) -> Result<(), Boundary> {
        self.0[family] = self.0[family]
            .checked_add(amount)
            .ok_or(Boundary::Overflow)?;
        Ok(())
    }
    fn payload_bytes(self) -> Result<usize, Boundary> {
        self.0
            .iter()
            .zip(ELEMENT_BYTES)
            .try_fold(0usize, |total, (&count, width)| {
                count
                    .checked_mul(width)
                    .and_then(|n| total.checked_add(n))
                    .ok_or(Boundary::Overflow)
            })
    }
}
#[derive(Debug)]
struct StoragePlan {
    requested: Counts,
    canonical_capacity: Counts,
    candidate_request_bytes: usize,
    canonical_payload_bytes: usize,
    hypothetical_hir_pair_bytes: usize,
}
impl StoragePlan {
    // Private raw counter; provenance is established by the enclosing probe.
    // This plan cannot reserve or construct anything.
    fn describe(program: &hir::Program) -> Result<Self, Boundary> {
        let mut requested = Counts::default();
        let mut canonical_capacity = Counts::default();
        macro_rules! vector {
            ($family:expr, $value:expr) => {{
                requested.add($family, $value.len())?;
                canonical_capacity.add($family, $value.capacity())?;
            }};
        }
        vector!(0, program.signatures);
        vector!(1, program.functions);
        for signature in &program.signatures {
            vector!(2, signature.params);
        }
        for function in &program.functions {
            vector!(3, function.locals);
            vector!(4, function.expressions);
            vector!(5, function.blocks);
            for block in &function.blocks {
                vector!(6, block.body);
            }
            for expression in &function.expressions {
                if let hir::ExprKind::Call { args, .. } = &expression.kind {
                    vector!(7, args);
                }
            }
        }
        if requested.0.iter().any(|&n| n > MAX_ROWS) {
            return Err(Boundary::Domain);
        }
        let candidate_request_bytes = requested.payload_bytes()?;
        let canonical_payload_bytes = canonical_capacity.payload_bytes()?;
        let hypothetical_hir_pair_bytes = candidate_request_bytes
            .checked_add(canonical_payload_bytes)
            .and_then(|n| {
                size_of::<hir::Program>()
                    .checked_mul(2)
                    .and_then(|headers| n.checked_add(headers))
            })
            .ok_or(Boundary::Overflow)?;
        Ok(Self {
            requested,
            canonical_capacity,
            candidate_request_bytes,
            canonical_payload_bytes,
            hypothetical_hir_pair_bytes,
        })
    }
}
#[derive(Debug)]
struct ProbeFacts {
    requested: Counts,
    candidate_request_bytes: usize,
    canonical_payload_bytes: usize,
    hypothetical_hir_pair_bytes: usize,
    rows: u8,
    source_len: u8,
}

// Prepaid source/domain/wire/OPA work, including malformed prefixes. This is
// the named logical-visit model reviewed separately from the old 2,690 event
// statistic. It includes full per-token lexical and per-number byte scans.
fn source_work_bound() -> Result<u64, Boundary> {
    let add = |a: u64, b: u64| a.checked_add(b).ok_or(Boundary::Overflow);
    let mul = |a: u64, b: u64| a.checked_mul(b).ok_or(Boundary::Overflow);
    let b = u64::try_from(MAX_ROWS).map_err(|_| Boundary::Overflow)?;
    let r = b;
    let t = u64::try_from(CELLS).map_err(|_| Boundary::Overflow)?;
    let c = t;
    let e = add(mul(16, r)?, 1)?;
    let q = r;
    // Equivalent checked sum of the reviewed category table: boundary/domain,
    // wire reads, event initialization/visits, token/lexical, numbers/heights.
    let terms = [
        173,
        mul(2, b)?,
        mul(30, c)?,
        mul(5, e)?,
        mul(t, add(mul(3, add(b, 1)?)?, 258)?)?,
        mul(r, add(b, 80)?)?,
        q,
    ];
    terms.into_iter().try_fold(0u64, add)
}

fn denied_probe(
    owner: SourceOwner<'_>,
    captured_source: &[u8],
    observation: &[u8],
) -> Result<Infallible, Rejection> {
    let bound = BoundObservation::bind(owner, captured_source, observation)?;
    // Genuine canonical resolution exists solely to describe retained storage.
    // There is no externally supplied HIR or candidate-construction capability.
    let canonical = hir::resolve_sources(bound.owner).map_err(Rejection::Canonical)?;
    let plan = StoragePlan::describe(&canonical)?;
    let facts = ProbeFacts {
        requested: plan.requested,
        candidate_request_bytes: plan.candidate_request_bytes,
        canonical_payload_bytes: plan.canonical_payload_bytes,
        hypothetical_hir_pair_bytes: plan.hypothetical_hir_pair_bytes,
        rows: bound.wire.rows,
        source_len: u8::try_from(bound.source.len()).map_err(|_| Boundary::Domain)?,
    };
    drop(canonical);
    Err(Rejection::Disabled(facts))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod pass_measurements;

#[cfg(test)]
mod verify_tests;

#[cfg(test)]
mod verify_fact_tests;

#[cfg(test)]
mod verify_diagnostic_tests;

#[cfg(test)]
mod run_measurements;

#[cfg(test)]
mod run_execution_tests;

#[cfg(test)]
mod emit_measurements;
