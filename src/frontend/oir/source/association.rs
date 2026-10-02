//! Source origins only. This does not certify lowering semantics or raw OIR safety.
//! Count and validation walks are separate, allocation-free and checked in release.
use super::super::*;
use crate::frontend::{ast, declaration_index::DeclarationIndex, source::SourceFileId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend::oir) struct Counts {
    pub declarations: usize,
    pub spans: usize,
}
impl Counts {
    fn total(self) -> Result<usize, Box<Diagnostic>> {
        self.declarations.checked_add(self.spans).ok_or_else(bad)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::frontend::oir) struct BindUsage {
    pub count: Counts,
    pub validation: Counts,
    pub dimensions: usize,
}
#[cfg(test)]
thread_local! {
    static LAST_USAGE: std::cell::Cell<Option<BindUsage>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(in crate::frontend::oir) fn last_usage() -> Option<BindUsage> {
    LAST_USAGE.get()
}

pub(in crate::frontend::oir) fn bad() -> Box<Diagnostic> {
    Diagnostic::new(
        "E0500",
        "oir-project-bind",
        "invalid source OIR association",
        None,
    )
}
fn increment(value: &mut usize) -> Result<(), Box<Diagnostic>> {
    *value = value.checked_add(1).ok_or_else(bad)?;
    Ok(())
}

pub(in crate::frontend::oir) struct Visitor<'s> {
    sources: Option<&'s SourceMap>,
    file: SourceFileId,
    counts: Counts,
    dimensions: usize,
}
impl<'s> Visitor<'s> {
    pub fn count() -> Self {
        Self {
            sources: None,
            file: SourceFileId(0),
            counts: Counts::default(),
            dimensions: 0,
        }
    }
    pub fn validate(sources: &'s SourceMap) -> Self {
        Self {
            sources: Some(sources),
            ..Self::count()
        }
    }
    pub fn file(&mut self, file: SourceFileId) {
        self.file = file;
    }
    pub fn dimension(&mut self, actual: usize, expected: usize) -> Result<(), Box<Diagnostic>> {
        increment(&mut self.dimensions)?;
        if actual != expected {
            return Err(bad());
        }
        Ok(())
    }
    pub fn declaration(&mut self) -> Result<(), Box<Diagnostic>> {
        increment(&mut self.counts.declarations)
    }
    pub fn span(&mut self, span: Span) -> Result<(), Box<Diagnostic>> {
        increment(&mut self.counts.spans)?;
        if self
            .sources
            .is_some_and(|map| span.file != self.file || !map.is_valid_span(span))
        {
            return Err(bad());
        }
        Ok(())
    }
    pub fn merge(&mut self, merge: &BoolMerge) -> Result<(), Box<Diagnostic>> {
        self.span(merge.span)?;
        self.span(merge.operator_span)?;
        for input in &merge.incoming {
            self.span(input.value.span)?;
        }
        Ok(())
    }
    pub fn statement(&mut self, statement: &Statement) -> Result<(), Box<Diagnostic>> {
        match statement {
            Statement::Assign(Assign { span, value, .. }) => {
                self.span(*span)?;
                match value {
                    Rvalue::Load(place) => self.span(place.span)?,
                    Rvalue::NotBool {
                        operand,
                        operator_span,
                    } => {
                        self.span(operand.span)?;
                        self.span(*operator_span)?;
                    }
                    Rvalue::Copy(operand) => self.span(operand.span)?,
                    Rvalue::CompareScalar {
                        left,
                        right,
                        operator_span,
                        ..
                    }
                    | Rvalue::CheckedI32 {
                        left,
                        right,
                        operator_span,
                        ..
                    } => {
                        self.span(left.span)?;
                        self.span(right.span)?;
                        self.span(*operator_span)?;
                    }
                    Rvalue::Bool(_) | Rvalue::I32(_) | Rvalue::Unit => (),
                }
            }
            Statement::Initialize { place, value, span } => {
                self.span(*span)?;
                self.span(place.span)?;
                self.span(value.span)?;
            }
            Statement::Store {
                place,
                value,
                operator_span,
                span,
            } => {
                self.span(*span)?;
                self.span(*operator_span)?;
                self.span(place.span)?;
                self.span(value.span)?;
            }
        }
        Ok(())
    }
    pub fn finish(self, count: Self) -> Result<BindUsage, Box<Diagnostic>> {
        // This equality is deliberately not a debug assertion.
        if self.counts != count.counts {
            return Err(bad());
        }
        self.counts.total()?;
        count.counts.total()?;
        let usage = BindUsage {
            count: count.counts,
            validation: self.counts,
            dimensions: self.dimensions,
        };
        #[cfg(test)]
        LAST_USAGE.set(Some(usage));
        Ok(usage)
    }
}

pub(super) enum Declarations<'a, 's> {
    Original(&'a ast::Program),
    Project(&'a DeclarationIndex<'s>),
}
impl Declarations<'_, '_> {
    fn count(&self) -> usize {
        match self {
            Self::Original(ast) => ast.functions.len(),
            Self::Project(index) => index.function_count(),
        }
    }
    fn name(&self, id: hir::DefId) -> Result<Span, Box<Diagnostic>> {
        match self {
            Self::Original(ast) => ast.functions.get(id.0).map(|f| f.name).ok_or_else(bad),
            Self::Project(index) => {
                let (key, module) = index.function(id).map_err(|_| bad())?;
                index
                    .sources()
                    .ast(module)
                    .map_err(|_| bad())?
                    .functions
                    .get(key.index)
                    .map(|f| f.name)
                    .ok_or_else(bad)
            }
        }
    }
}
fn scalar_function(function: &Function, visitor: &mut Visitor<'_>) -> Result<(), Box<Diagnostic>> {
    visitor.declaration()?;
    visitor.span(function.span)?;
    for local in &function.locals {
        visitor.span(local.span)?;
    }
    for place in &function.places {
        visitor.span(place.span)?;
    }
    for block in &function.blocks {
        visitor.span(block.span)?;
        if let Some(merge) = &block.merge {
            visitor.merge(merge)?;
        }
        for statement in &block.statements {
            visitor.statement(statement)?;
        }
        if let Some(terminator) = &block.terminator {
            visitor.span(terminator.span)?;
            match &terminator.kind {
                TerminatorKind::Branch { condition, .. } => visitor.span(condition.span)?,
                TerminatorKind::Call { args, .. } => {
                    for arg in args {
                        visitor.span(arg.span)?;
                    }
                }
                TerminatorKind::Return(operand) => visitor.span(operand.span)?,
                TerminatorKind::Goto { .. } => (),
            }
        }
    }
    Ok(())
}
pub(super) fn scalar(
    raw: &Program,
    sources: &SourceMap,
    declarations: Declarations<'_, '_>,
) -> Result<BindUsage, Box<Diagnostic>> {
    let mut count = Visitor::count();
    for function in &raw.functions {
        scalar_function(function, &mut count)?;
    }
    let mut visitor = Visitor::validate(sources);
    visitor.dimension(raw.functions.len(), declarations.count())?;
    for (ordinal, function) in raw.functions.iter().enumerate() {
        let expected = declarations.name(hir::DefId(ordinal))?;
        if function.id != hir::DefId(ordinal) || function.span != expected {
            return Err(bad());
        }
        visitor.file(expected.file);
        scalar_function(function, &mut visitor)?;
    }
    visitor.finish(count)
}
