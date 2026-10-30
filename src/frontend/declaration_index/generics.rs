//! Source-only generic checking and bounded concrete function instantiation.
//! Template identities always refer to the original frozen declaration index.
mod flow;
mod inference;
mod nominal;
pub(super) mod owned_candidate;
#[cfg(test)]
mod vector_tests;
use super::*;
use crate::frontend::ast::{self, Argument, ExprId, ExprKind, StmtKind, TypeSyntaxKind};
use crate::frontend::builtin_catalog::vector;
use crate::frontend::lexer::{Kind as TokenKind, Token};
use crate::frontend::oir::owned_types::FixedArrayTy;
use nominal::Nominals;

const MAX_INSTANCES: usize = 1024;
const MAX_DEPTH: usize = 64;
const MAX_STORAGE: usize = 8 * 1024 * 1024;

fn error(message: &'static str, at: Span) -> Box<Diagnostic> {
    diagnostic("E0300", "generics", message, at)
}
fn exhausted(at: Span) -> Box<Diagnostic> {
    diagnostic(
        "E0400",
        "generics",
        "generic instantiation resource limit exceeded",
        at,
    )
}

fn reject_parameter_base(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    module: ModuleId,
    declaration: Span,
    path: ast::ItemPath,
    at: Span,
) -> Result<(), Box<Diagnostic>> {
    let program = index.sources().ast(module)?;
    let name = match path {
        ast::ItemPath::Unqualified(name) => name,
        ast::ItemPath::Absolute(path) if program.paths[path.0].root == ast::PathRoot::LocalType => {
            *program
                .path_segments(path)
                .and_then(|segments| segments.first())
                .ok_or_else(|| bad(at))?
        }
        _ => return Ok(()),
    };
    for parameter in program.generic_parameters(declaration) {
        work.debit(1, at, "generic parameter base resolution")?;
        if index.sources().text(name)? == index.sources().text(parameter.name)? {
            return Err(error(
                "a type parameter cannot be used as a nominal constructor or type application",
                at,
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct Storage {
    bytes: usize,
    // Constructor/vector frames coexist with retained type metadata. Account their
    // live controls in the same ceiling, releasing them on every Result path.
    constructor_scratch: usize,
}
impl Storage {
    fn live_after(&self, additional: usize) -> Option<usize> {
        self.bytes
            .checked_add(self.constructor_scratch)?
            .checked_add(additional)
    }
    fn push<T>(
        &mut self,
        values: &mut Vec<T>,
        value: T,
        allocator: &mut Allocator,
        work: &WorkMeter,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        work.debit(1, at, "generic metadata")?;
        if values.len() == values.capacity() {
            let size = size_of::<T>();
            if self
                .live_after(size)
                .is_none_or(|bytes| bytes > MAX_STORAGE)
            {
                return Err(exhausted(at));
            }
            let before = values.capacity();
            allocator
                .vector_exact(values, 1, "generic metadata")
                .map_err(|_| exhausted(at))?;
            self.bytes = self
                .bytes
                .checked_add(
                    (values.capacity() - before)
                        .checked_mul(size)
                        .ok_or_else(|| exhausted(at))?,
                )
                .ok_or_else(|| exhausted(at))?;
            if self.live_after(0).is_none_or(|bytes| bytes > MAX_STORAGE) {
                return Err(exhausted(at));
            }
        }
        values.push(value);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ExposureOwner {
    Function(DefId),
    Enumeration(EnumId),
    Field(FieldId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Concrete(ValueTy),
    Parameter { position: usize, copy: bool },
    Record(usize),
    Enumeration(usize),
    Vector(Element),
}

/// Generic vector element identities. Copy parameters retain their position
/// until substitution; private owned candidates use a separate key domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Element {
    Concrete(Ty),
    CopyParameter(usize),
    RecordApplication(usize),
    ConcreteRecord(RecordId),
    OwnedRecordApplication(usize),
    OwnedConcreteRecord(RecordId),
    OwnedEnumApplication(usize),
    OwnedBuiltinEnum(EnumId),
}
impl Element {
    fn check(value: Value, at: Span) -> Result<Self, Box<Diagnostic>> {
        match value {
            Value::Concrete(ValueTy::Scalar(ty)) if vector::scalar_admitted(ty) => {
                Ok(Self::Concrete(ty))
            }
            Value::Parameter {
                position,
                copy: true,
            } => Ok(Self::CopyParameter(position)),
            _ => Err(error(
                "vector and slice elements require a scalar or Copy-bound parameter",
                at,
            )),
        }
    }
    fn value(self) -> Value {
        match self {
            Self::Concrete(ty) => Value::Concrete(ValueTy::Scalar(ty)),
            Self::CopyParameter(position) => Value::Parameter {
                position,
                copy: true,
            },
            Self::RecordApplication(id) => Value::Record(id),
            Self::ConcreteRecord(id) => Value::Concrete(ValueTy::Owned(AggregateTy::Record(id))),
            Self::OwnedRecordApplication(id) => Value::Record(id),
            Self::OwnedConcreteRecord(id) => {
                Value::Concrete(ValueTy::Owned(AggregateTy::Record(id)))
            }
            Self::OwnedEnumApplication(id) => Value::Enumeration(id),
            Self::OwnedBuiltinEnum(id) => Value::Concrete(ValueTy::Owned(AggregateTy::Enum(id))),
        }
    }
    fn checked(value: Value, nominals: &Nominals, at: Span) -> Result<Self, Box<Diagnostic>> {
        if let Value::Concrete(ValueTy::Owned(AggregateTy::Record(id))) = value {
            if nominals.copy(value) {
                return Ok(Self::ConcreteRecord(id));
            }
            return Err(error(
                "vector and slice record elements require checked Copy",
                at,
            ));
        }
        if let Value::Record(id) = value {
            if nominals.copy(value) {
                return Ok(Self::RecordApplication(id));
            }
            return Err(error(
                "vector and slice record elements require checked Copy",
                at,
            ));
        }
        Self::check(value, at)
    }
    fn borrowed(self, at: Span) -> Result<BorrowedTy, Box<Diagnostic>> {
        match self {
            Self::ConcreteRecord(id) => Ok(BorrowedTy::RecordSlice(id)),
            _ => Ok(BorrowedTy::ScalarSlice(self.scalar(at)?)),
        }
    }
    fn scalar(self, at: Span) -> Result<Ty, Box<Diagnostic>> {
        match self {
            Self::Concrete(ty) if vector::scalar_admitted(ty) => Ok(ty),
            _ => Err(bad(at)),
        }
    }
}
impl Value {
    fn concrete(ty: ValueTy) -> Self {
        match ty {
            ValueTy::Owned(AggregateTy::Vector(element)) => {
                Self::Vector(Element::Concrete(element))
            }
            ValueTy::Owned(AggregateTy::RecordVector(id)) => {
                Self::Vector(Element::ConcreteRecord(id))
            }
            _ => Self::Concrete(ty),
        }
    }
    fn vector_element(self) -> Option<Element> {
        match self {
            Self::Vector(element) => Some(element),
            _ => None,
        }
    }
}
impl Element {
    fn owned(self) -> bool {
        matches!(
            self,
            Self::OwnedRecordApplication(_)
                | Self::OwnedConcreteRecord(_)
                | Self::OwnedEnumApplication(_)
                | Self::OwnedBuiltinEnum(_)
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    Value(Value),
    Reference { value: Value, kind: BorrowKind },
    Slice { element: Element, kind: BorrowKind },
}
impl Type {
    fn scalar(ty: Ty) -> Self {
        Self::Value(Value::Concrete(ValueTy::Scalar(ty)))
    }
    fn concrete(ty: ParameterTy) -> Self {
        match ty {
            ParameterTy::Value(ty) => Self::Value(Value::concrete(ty)),
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(ty),
                kind,
            } => Self::Reference {
                value: Value::concrete(ValueTy::Owned(ty)),
                kind,
            },
            ParameterTy::Reference {
                referent: BorrowedTy::RecordSlice(id),
                kind,
            } => Self::Slice {
                element: Element::ConcreteRecord(id),
                kind,
            },
            ParameterTy::Reference {
                referent: BorrowedTy::ScalarSlice(element),
                kind,
            } => Self::Slice {
                element: Element::Concrete(element),
                kind,
            },
        }
    }
    fn parameter(self, at: Span) -> Result<ParameterTy, Box<Diagnostic>> {
        Ok(match self {
            Self::Value(Value::Concrete(ty)) => ParameterTy::Value(ty),
            Self::Reference {
                value: Value::Concrete(ValueTy::Owned(AggregateTy::Enum(_))),
                ..
            } => {
                return Err(error(
                    "enum references are not supported by this parameter",
                    at,
                ))
            }
            Self::Reference {
                value: Value::Concrete(ValueTy::Owned(ty)),
                kind,
            } => ParameterTy::Reference {
                referent: BorrowedTy::Exact(ty),
                kind,
            },
            Self::Slice { element, kind } => ParameterTy::Reference {
                referent: element.borrowed(at)?,
                kind,
            },
            _ => {
                return Err(error(
                    "a concrete owned type is required for this reference",
                    at,
                ))
            }
        })
    }
    fn value(self, at: Span) -> Result<Value, Box<Diagnostic>> {
        match self {
            Self::Value(value) => Ok(value),
            _ => Err(error("a reference cannot be used as an owned value", at)),
        }
    }
}

/// Complete additional synchronous roles for vector syntax, call typing,
/// specialization lookup/registration, and freeze. Sequential roles are summed;
/// nested vector calls and constructor frames remain live independently.
#[allow(dead_code)]
struct VectorFrame {
    operation: vector::Operation,
    function: vector::Function,
    pending: Option<vector::Function>,
    signature: vector::Signature,
    abstract_signature: ([Type; 3], Type),
    type_inputs: [Type; 3],
    type_results: [Result<Type, Box<Diagnostic>>; 3],
    actuals: [Option<Type>; 3],
    elements: [Element; 2],
    element_result: Result<Element, Box<Diagnostic>>,
    scalar_result: Result<Ty, Box<Diagnostic>>,
    value: Value,
    value_result: Result<Value, Box<Diagnostic>>,
    syntax: ast::TypeSyntax,
    explicit: Option<&'static [ast::TypeSyntax]>,
    arguments: &'static [Argument],
    argument_iterator: std::slice::Iter<'static, Argument>,
    argument: Option<&'static Argument>,
    keys: &'static [vector::Function],
    key_iterator: std::iter::Enumerate<std::slice::Iter<'static, vector::Function>>,
    selected_key: Option<(usize, &'static vector::Function)>,
    instance_iterator: std::slice::IterMut<'static, Instance>,
    selected_instance: Option<&'static mut Instance>,
    call_iterator: std::slice::IterMut<'static, Call>,
    selected_call: Option<&'static mut Call>,
    rank: Option<usize>,
    rank_lookup_result: Result<Option<usize>, Box<Diagnostic>>,
    rank_result: Result<usize, Box<Diagnostic>>,
    id: DefId,
    id_result: Result<DefId, Box<Diagnostic>>,
    checked_ids: [Option<usize>; 2],
    enum_result: Result<EnumId, Box<Diagnostic>>,
    call: Call,
    paths: [ItemPathRef; 2],
    source_span: Result<Span, Box<Diagnostic>>,
    parameters: &'static [ast::GenericParam],
    parameter_iterator: std::iter::Enumerate<std::slice::Iter<'static, ast::GenericParam>>,
    origins: [Span; 3],
    counts: [usize; 4],
    unit_result: Result<(), Box<Diagnostic>>,
}
const VECTOR_FRAME_BYTES: usize = size_of::<VectorFrame>();

impl Storage {
    fn enter_vector(&mut self, at: Span) -> Result<(), Box<Diagnostic>> {
        if self
            .live_after(VECTOR_FRAME_BYTES)
            .is_none_or(|bytes| bytes > MAX_STORAGE)
        {
            return Err(exhausted(at));
        }
        self.constructor_scratch = self
            .constructor_scratch
            .checked_add(VECTOR_FRAME_BYTES)
            .ok_or_else(|| exhausted(at))?;
        Ok(())
    }
    fn leave_vector(&mut self, at: Span) -> Result<(), Box<Diagnostic>> {
        self.constructor_scratch = self
            .constructor_scratch
            .checked_sub(VECTOR_FRAME_BYTES)
            .ok_or_else(|| bad(at))?;
        Ok(())
    }
}

fn vector_rank(
    functions: &[vector::Function],
    key: vector::Function,
    work: &WorkMeter,
    at: Span,
) -> Result<Option<usize>, Box<Diagnostic>> {
    for (rank, candidate) in functions.iter().enumerate() {
        work.debit(1, at, "vector specialization lookup")?;
        if *candidate == key {
            return Ok(Some(rank));
        }
    }
    Ok(None)
}

fn vector_id(users: usize, fixed: usize, rank: usize, at: Span) -> Result<DefId, Box<Diagnostic>> {
    users
        .checked_add(fixed)
        .and_then(|base| base.checked_add(rank))
        .map(DefId)
        .ok_or_else(|| exhausted(at))
}

/// The source checker's abstract relation. Concrete instances are independently
/// compared with the closed catalog by the downstream descriptor proof.
fn vector_signature(
    operation: vector::Operation,
    element: Element,
    grow: EnumId,
) -> ([Type; 3], Type) {
    use vector::Operation::*;
    let unit = Type::scalar(Ty::Unit);
    let count = Type::scalar(Ty::I32);
    let shared = Type::Reference {
        value: Value::Vector(element),
        kind: BorrowKind::Shared,
    };
    let exclusive = Type::Reference {
        value: Value::Vector(element),
        kind: BorrowKind::Exclusive,
    };
    let value = Type::Value(element.value());
    let elements = Type::Slice {
        element,
        kind: BorrowKind::Shared,
    };
    let parameters = match operation {
        New => [unit; 3],
        Len | Capacity => [shared, unit, unit],
        Take => [exclusive, unit, unit],
        TryReserve | Truncate => [exclusive, count, unit],
        TryPush => [exclusive, value, unit],
        TryExtend => [exclusive, elements, unit],
        TryResize => [exclusive, count, value],
    };
    let result = match operation {
        New | Take => Type::Value(Value::Vector(element)),
        Len | Capacity => count,
        Truncate => unit,
        TryReserve | TryPush | TryExtend | TryResize => {
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Enum(grow))))
        }
    };
    (parameters, result)
}

#[derive(Debug, PartialEq, Eq)]
struct Call {
    expression: ExprId,
    template: DefId,
    target: DefId,
    // Keep the imported template independently of its eventual executable ID.
    // A pending vector target is rewritten only after user discovery freezes.
    vector: Option<vector::Function>,
}
#[derive(Debug, PartialEq, Eq)]
struct Instance {
    definition: DefId,
    arguments: Vec<Value>,
    calls: Vec<Call>,
    types: Vec<(Span, Type)>,
    constructors: Vec<(ExprId, Value)>,
    matches: Vec<(Span, Value)>,
    depth: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Plan {
    instances: Vec<Instance>,
    ordinary: Vec<Option<DefId>>,
    template_count: usize,
    nominals: Nominals,
    vector_functions: Vec<vector::Function>,
}
impl Plan {
    pub(super) fn vector_functions(&self) -> &[vector::Function] {
        &self.vector_functions
    }
    // Read-only observation; paid query callers must debit each comparison.
    pub(super) fn vector_function_rank(&self, key: vector::Function) -> Option<usize> {
        self.vector_functions
            .iter()
            .position(|candidate| *candidate == key)
    }
    pub(super) fn vector_count(&self) -> usize {
        self.vector_functions.len()
    }
    pub(super) fn vector_function(&self, rank: usize) -> Option<vector::Function> {
        self.vector_functions.get(rank).copied()
    }
    pub(super) fn enum_count(&self) -> usize {
        self.nominals.enum_count()
    }
    pub(super) fn record_count(&self) -> usize {
        self.nominals.record_count()
    }
    pub(super) fn len(&self) -> usize {
        self.instances.len()
    }
    pub(super) fn definition(&self, id: DefId) -> Result<DefId, Box<Diagnostic>> {
        self.instances
            .get(id.0)
            .map(|instance| instance.definition)
            .ok_or_else(|| {
                Diagnostic::new(
                    "E0500",
                    "generics",
                    "invalid generic function identity",
                    None,
                )
            })
    }
    pub(super) fn non_generic(&self, id: DefId) -> Option<DefId> {
        self.ordinary.get(id.0).copied().flatten()
    }
    pub(super) fn build(
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        allocator: &mut Allocator,
    ) -> Result<Self, Box<Diagnostic>> {
        authenticate_syntax(index, work, allocator)?;
        let mut storage = Storage {
            bytes: size_of::<Plan>(),
            ..Default::default()
        };
        index.activate_owned_storage(&mut storage, work)?;
        index.record_owned_sources(&mut storage, allocator, work)?;
        let mut plan = Self {
            instances: Vec::new(),
            ordinary: Vec::new(),
            template_count: index.template_function_count(),
            nominals: Nominals::default(),
            vector_functions: Vec::new(),
        };
        let mut nominals = Nominals::default();
        nominals.template_enum_count = index.template_enum_count();
        for id in 0..index.template_record_count() {
            let record = RecordId(id);
            let (key, module) = index.template_record(record)?;
            let program = index.sources().ast(module)?;
            let declaration = &program.records[key.index];
            let parameters = program.generic_parameters(declaration.name);
            validate_parameters(index, work, parameters)?;
            let mut arguments = Vec::new();
            for (position, parameter) in parameters.iter().enumerate() {
                storage.push(
                    &mut arguments,
                    Value::Parameter {
                        position,
                        copy: !parameter.bounds.is_empty(),
                    },
                    allocator,
                    work,
                    parameter.name,
                )?;
            }
            let value = nominals.record(
                index,
                work,
                &mut storage,
                allocator,
                record,
                arguments,
                declaration.name,
                0,
            )?;
            let Value::Record(application) = value else {
                return Err(bad(declaration.name));
            };
            for (position, (field, value)) in declaration
                .fields
                .iter()
                .zip(&nominals.records[application].fields)
                .enumerate()
            {
                if matches!(
                    value,
                    Value::Record(_)
                        | Value::Enumeration(_)
                        | Value::Vector(_)
                        | Value::Concrete(ValueTy::Owned(
                            AggregateTy::Record(_)
                                | AggregateTy::Enum(_)
                                | AggregateTy::ByteBuf
                                | AggregateTy::File
                        ))
                ) {
                    nominals.exposure(
                        index,
                        work,
                        ExposureOwner::Field(FieldId {
                            record,
                            index: position,
                        }),
                        *value,
                        field.ty.span,
                        0,
                    )?;
                }
            }
        }
        for id in 0..index.template_enum_count() {
            let enumeration = EnumId(id);
            let (key, module) = index.template_enumeration(enumeration)?;
            let program = index.sources().ast(module)?;
            let declaration = &program.enums[key.index];
            let parameters = program.generic_parameters(declaration.name);
            validate_parameters(index, work, parameters)?;
            let mut arguments = Vec::new();
            for (position, parameter) in parameters.iter().enumerate() {
                storage.push(
                    &mut arguments,
                    Value::Parameter {
                        position,
                        copy: !parameter.bounds.is_empty(),
                    },
                    allocator,
                    work,
                    parameter.name,
                )?;
            }
            let value = nominals.enumeration(
                index,
                work,
                &mut storage,
                allocator,
                enumeration,
                arguments,
                declaration.name,
                0,
            )?;
            let Value::Enumeration(id) = value else {
                return Err(bad(declaration.name));
            };
            for (variant, payload) in declaration
                .variants
                .iter()
                .zip(&nominals.enumerations[id].variants)
            {
                if let Some(value) = payload {
                    nominals.exposure(
                        index,
                        work,
                        ExposureOwner::Enumeration(enumeration),
                        *value,
                        variant.payload.map_or(variant.name, |payload| payload.span),
                        0,
                    )?;
                }
            }
        }
        // Check definitions with distinct opaque parameters before considering
        // concrete call sites. Scalar instantiations cannot hide invalid moves.
        for id in 0..plan.template_count {
            let definition = DefId(id);
            let (key, module) = index.template_function(definition)?;
            let program = index.sources().ast(module)?;
            let function = &program.functions[key.index];
            let parameters = program.generic_parameters(function.name);
            validate_parameters(index, work, parameters)?;
            storage.push(&mut plan.ordinary, None, allocator, work, function.name)?;
            if parameters.is_empty() {
                let target = DefId(plan.instances.len());
                plan.ordinary[id] = Some(target);
                storage.push(
                    &mut plan.instances,
                    Instance {
                        definition,
                        arguments: Vec::new(),
                        calls: Vec::new(),
                        types: Vec::new(),
                        constructors: Vec::new(),
                        matches: Vec::new(),
                        depth: 0,
                    },
                    allocator,
                    work,
                    function.name,
                )?;
            } else {
                if module == ModuleId(0) && index.sources().text(function.name)? == "main" {
                    return Err(error(
                        "the entry function cannot have type parameters",
                        function.name,
                    ));
                }
                let mut arguments = Vec::new();
                for (position, parameter) in parameters.iter().enumerate() {
                    storage.push(
                        &mut arguments,
                        Value::Parameter {
                            position,
                            copy: !parameter.bounds.is_empty(),
                        },
                        allocator,
                        work,
                        parameter.name,
                    )?;
                }
                Walker::new(
                    index,
                    work,
                    allocator,
                    &mut storage,
                    &mut nominals,
                    None,
                    definition,
                    arguments,
                    None,
                )?
                .check()?;
            }
        }
        let mut next = 0usize;
        while next < plan.instances.len() {
            let definition = plan.instances[next].definition;
            let mut arguments = Vec::new();
            for argument in &plan.instances[next].arguments {
                storage.push(
                    &mut arguments,
                    *argument,
                    allocator,
                    work,
                    index.sources().eof(),
                )?;
            }
            let checked = Walker::new(
                index,
                work,
                allocator,
                &mut storage,
                &mut nominals,
                Some(&mut plan),
                definition,
                arguments,
                Some(DefId(next)),
            )?
            .check()?;
            plan.instances[next].calls = checked.calls;
            plan.instances[next].types = checked.types;
            plan.instances[next].constructors = checked.constructors;
            plan.instances[next].matches = checked.matches;
            next += 1;
        }
        index.freeze_owned_builtin_enum_keys(&nominals, &mut storage, work)?;
        plan.freeze_calls(index, &mut storage, work)?;
        nominals.finish(&mut storage, allocator, work, index.sources().eof())?;
        plan.nominals = nominals;
        index.finish_owned_storage(&storage)?;
        Ok(plan)
    }
    fn register_vector(
        &mut self,
        key: vector::Function,
        storage: &mut Storage,
        allocator: &mut Allocator,
        work: &WorkMeter,
        at: Span,
    ) -> Result<usize, Box<Diagnostic>> {
        storage.enter_vector(at)?;
        let result = (|| {
            work.debit(1, at, "vector specialization admission")?;
            if key.record_id().is_none() && !vector::scalar_admitted(key.element()) {
                return Err(error("vector specialization requires a scalar element", at));
            }
            if let Some(rank) = vector_rank(&self.vector_functions, key, work, at)? {
                return Ok(rank);
            }
            let rank = self.vector_functions.len();
            storage.push(&mut self.vector_functions, key, allocator, work, at)?;
            Ok(rank)
        })();
        storage.leave_vector(at)?;
        result
    }
    fn freeze_calls(
        &mut self,
        index: &DeclarationIndex<'_>,
        storage: &mut Storage,
        work: &WorkMeter,
    ) -> Result<(), Box<Diagnostic>> {
        if index.owned_vector_builder().is_some() {
            return index.freeze_owned_calls(self, storage, work);
        }
        let at = index.sources().eof();
        // The existing fixed-builtin rewrite does not reserve vector controls
        // or spend vector freeze work when no concrete vector key exists.
        if self.vector_functions.is_empty() {
            let count = self.instances.len();
            for instance in &mut self.instances {
                for call in &mut instance.calls {
                    if call.vector.is_some() {
                        return Err(bad(at));
                    }
                    if call.template.0 >= self.template_count {
                        call.target = DefId(count + call.template.0 - self.template_count);
                    }
                }
            }
            return Ok(());
        }
        storage.enter_vector(at)?;
        let result = (|| {
            let users = self.instances.len();
            let fixed = index.builtin_set().extra_functions();
            let functions = &self.vector_functions;
            for instance in &mut self.instances {
                for call in &mut instance.calls {
                    work.debit(1, at, "vector call identity freeze")?;
                    if let Some(key) = call.vector {
                        if index.vector_template(call.template, work, at)? != Some(key.operation())
                        {
                            return Err(bad(at));
                        }
                        let rank = vector_rank(functions, key, work, at)?.ok_or_else(|| bad(at))?;
                        call.target = vector_id(users, fixed, rank, at)?;
                    } else if call.template.0 >= self.template_count {
                        let rank = call.template.0 - self.template_count;
                        if rank >= fixed
                            || index.vector_template(call.template, work, at)?.is_some()
                        {
                            return Err(bad(at));
                        }
                        call.target = users
                            .checked_add(rank)
                            .map(DefId)
                            .ok_or_else(|| exhausted(at))?;
                    }
                }
            }
            Ok(())
        })();
        storage.leave_vector(at)?;
        result
    }
    // Keep the caller's shared storage budget, failure seam and work meter explicit.
    #[allow(clippy::too_many_arguments)]
    fn instantiate(
        &mut self,
        definition: DefId,
        arguments: Vec<Value>,
        depth: usize,
        storage: &mut Storage,
        allocator: &mut Allocator,
        work: &WorkMeter,
        at: Span,
    ) -> Result<DefId, Box<Diagnostic>> {
        let mut specializations = 0usize;
        for (id, instance) in self.instances.iter().enumerate() {
            specializations += usize::from(!instance.arguments.is_empty());
            work.debit((1 + arguments.len()) as u64, at, "generic instance lookup")?;
            if instance.definition == definition && instance.arguments == arguments {
                return Ok(DefId(id));
            }
        }
        if specializations >= MAX_INSTANCES || depth > MAX_DEPTH {
            return Err(exhausted(at));
        }
        let id = DefId(self.instances.len());
        storage.push(
            &mut self.instances,
            Instance {
                definition,
                arguments,
                calls: Vec::new(),
                types: Vec::new(),
                constructors: Vec::new(),
                matches: Vec::new(),
                depth,
            },
            allocator,
            work,
            at,
        )?;
        Ok(id)
    }
}

fn authenticate_syntax(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<(), Box<Diagnostic>> {
    use crate::frontend::{lexer, parser};
    for module in 0..index.sources().count() {
        let module = ModuleId(module);
        let file = index.sources().file(module)?;
        if file.text().len() > crate::frontend::source::MAX_SOURCE_BYTES {
            return Err(exhausted(file.span(0, 0)));
        }
        let tokens = lexer::lex_current_with_allocator(file, lexer::MAX_TOKENS, allocator)
            .map_err(|failure| failure.diagnostic())?;
        work.debit(
            file.text().len() as u64 + 32 * tokens.len() as u64,
            file.span(0, 0),
            "generic syntax authentication",
        )?;
        let (fresh, _) = parser::parse_typed_counted(
            file,
            tokens,
            parser::SourceMode::ProjectCandidate,
            parser::MAX_NODES,
            allocator,
            &mut Default::default(),
        )
        .map_err(|errors| {
            errors
                .into_iter()
                .next()
                .map(Box::new)
                .unwrap_or_else(|| bad(file.span(0, 0)))
        })?;
        if !index.sources().ast(module)?.syntax_matches(&fresh) {
            return Err(bad(file.span(0, 0)));
        }
    }
    Ok(())
}

fn validate_parameters(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    parameters: &[ast::GenericParam],
) -> Result<(), Box<Diagnostic>> {
    for (position, parameter) in parameters.iter().enumerate() {
        let name = index.sources().text(parameter.name)?;
        work.debit(
            (1 + name.len()) as u64,
            parameter.name,
            "type parameter name",
        )?;
        if matches!(name, "bool" | "char" | "f32" | "f64" | "Copy")
            || ast::IntegerTypeSyntax::from_spelling(name).is_some()
        {
            return Err(error(
                "a type parameter cannot replace a built-in type or bound",
                parameter.name,
            ));
        }
        for earlier in &parameters[..position] {
            work.debit(1, parameter.name, "type parameter uniqueness")?;
            if index.sources().text(earlier.name)? == name {
                return Err(duplicate(parameter.name, earlier.name));
            }
        }
        for (position, bound) in parameter.bounds.iter().enumerate() {
            work.debit(1, parameter.name, "generic bound")?;
            if position != 0
                || !matches!(bound, ast::ItemPath::Unqualified(span) if index.sources().text(*span)? == "Copy")
            {
                return Err(error("unknown or repeated trait bound", parameter.span));
            }
        }
    }
    Ok(())
}

fn syntax_type(
    index: &DeclarationIndex<'_>,
    work: &WorkMeter,
    module: ModuleId,
    declaration: Span,
    arguments: &[Value],
    syntax: ast::TypeSyntax,
) -> Result<Type, Box<Diagnostic>> {
    if let TypeSyntaxKind::GenericSliceReference { mutable, element } = syntax.kind {
        let span = index.sources().path_span(ItemPathRef {
            file: syntax.span.file,
            path: element,
        })?;
        let value = syntax_type(
            index,
            work,
            module,
            declaration,
            arguments,
            ast::TypeSyntax {
                kind: TypeSyntaxKind::Name(element),
                span,
            },
        )?
        .value(span)?;
        return Ok(Type::Slice {
            element: Element::check(value, span)?,
            kind: if mutable {
                BorrowKind::Exclusive
            } else {
                BorrowKind::Shared
            },
        });
    }
    let parameters = index.sources().ast(module)?.generic_parameters(declaration);
    let path = match syntax.kind {
        TypeSyntaxKind::Name(path) => Some((path, None)),
        TypeSyntaxKind::Reference { mutable, referent } => Some((
            referent,
            Some(if mutable {
                BorrowKind::Exclusive
            } else {
                BorrowKind::Shared
            }),
        )),
        _ => None,
    };
    if let Some((ast::ItemPath::Unqualified(name), reference)) = path {
        for (position, parameter) in parameters.iter().enumerate() {
            work.debit(1, syntax.span, "generic type substitution")?;
            if index.sources().text(name)? == index.sources().text(parameter.name)? {
                let value = *arguments.get(position).ok_or_else(|| bad(syntax.span))?;
                return Ok(match reference {
                    None => Type::Value(value),
                    Some(kind) => Type::Reference { value, kind },
                });
            }
        }
    }
    index
        .query(work)
        .parameter_type(module, syntax)
        .map(Type::concrete)
}

impl DeclarationIndex<'_> {
    pub fn template_enum_for_instance(
        &self,
        enumeration: EnumId,
    ) -> Result<EnumId, Box<Diagnostic>> {
        self.generic_plan().map_or(Ok(enumeration), |plan| {
            plan.nominals
                .enum_definition(enumeration, self.sources().eof())
        })
    }
    pub fn ordinary_enum_instance(&self, template: EnumId) -> Result<EnumId, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            return Ok(template);
        };
        plan.nominals
            .enumerations
            .iter()
            .find(|enumeration| {
                enumeration.template == template && enumeration.arguments.is_empty()
            })
            .and_then(|enumeration| enumeration.emitted)
            .ok_or_else(|| bad(self.sources().eof()))
    }
    pub fn template_nominal_for_instance(
        &self,
        nominal: NominalId,
    ) -> Result<NominalId, Box<Diagnostic>> {
        Ok(match nominal {
            NominalId::Record(record) => {
                NominalId::Record(self.template_record_for_instance(record)?)
            }
            NominalId::Enum(enumeration) => {
                NominalId::Enum(self.template_enum_for_instance(enumeration)?)
            }
            _ => nominal,
        })
    }
    pub(super) fn generic_enum_payload(&self, variant: VariantId) -> Option<Option<ValueTy>> {
        let plan = self.generic_plan()?;
        if variant.enumeration.0 >= plan.nominals.enum_count() {
            return None;
        }
        Some(
            plan.nominals
                .payload(variant, self.sources().eof())
                .expect("completed source enum application"),
        )
    }
    pub fn generic_variant_template(
        &self,
        module: ModuleId,
        path: ast::ItemPath,
        name: Span,
        work: &WorkMeter,
    ) -> Result<VariantId, Box<Diagnostic>> {
        let nominal = self.query(work).nominal_type(
            module,
            ItemPathRef {
                file: name.file,
                path,
            },
            TypeContext::Constructor,
        )?;
        let NominalId::Enum(enumeration) = nominal else {
            return Err(error("variant construction requires an enum type", name));
        };
        if enumeration.0 >= self.template_enum_count() {
            return Err(error("this enum does not accept type arguments", name));
        }
        let (key, module) = self.template_enumeration(enumeration)?;
        for (index, variant) in self.sources().ast(module)?.enums[key.index]
            .variants
            .iter()
            .enumerate()
        {
            work.debit(1, name, "generic enum variant lookup")?;
            if self.sources().text(variant.name)? == self.sources().text(name)? {
                return Ok(VariantId { enumeration, index });
            }
        }
        Err(error("unknown enum variant", name))
    }
    fn lowered_variant(
        &self,
        value: Value,
        template: VariantId,
    ) -> Result<VariantId, Box<Diagnostic>> {
        let plan = self
            .generic_plan()
            .ok_or_else(|| bad(self.sources().eof()))?;
        let ValueTy::Owned(AggregateTy::Enum(enumeration)) =
            plan.nominals.value(value, self.sources().eof())?
        else {
            return Err(bad(self.sources().eof()));
        };
        if self.template_enum_for_instance(enumeration)? != template.enumeration {
            return Err(bad(self.sources().eof()));
        }
        Ok(VariantId {
            enumeration,
            index: template.index,
        })
    }
    pub fn instantiated_variant(
        &self,
        caller: DefId,
        expression: ExprId,
        template: VariantId,
    ) -> Result<VariantId, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            return Ok(template);
        };
        let instance = plan
            .instances
            .get(caller.0)
            .ok_or_else(|| bad(self.sources().eof()))?;
        let value = instance
            .constructors
            .iter()
            .find(|(id, _)| *id == expression)
            .map(|(_, value)| *value)
            .ok_or_else(|| bad(self.sources().eof()))?;
        self.lowered_variant(value, template)
    }
    pub fn instantiated_match_variant(
        &self,
        caller: DefId,
        arm: Span,
        template: VariantId,
    ) -> Result<VariantId, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            return Ok(template);
        };
        let instance = plan.instances.get(caller.0).ok_or_else(|| bad(arm))?;
        let value = instance
            .matches
            .iter()
            .find(|(span, _)| *span == arm)
            .map(|(_, value)| *value)
            .ok_or_else(|| bad(arm))?;
        self.lowered_variant(value, template)
    }
    pub fn template_record_for_instance(
        &self,
        record: RecordId,
    ) -> Result<RecordId, Box<Diagnostic>> {
        self.generic_plan().map_or(Ok(record), |plan| {
            plan.nominals.template(record, self.sources().eof())
        })
    }
    pub fn record_is_copy(&self, record: RecordId) -> Result<bool, Box<Diagnostic>> {
        self.generic_plan().map_or(Ok(false), |plan| {
            plan.nominals.record_is_copy(record, self.sources().eof())
        })
    }
    pub fn ordinary_record_instance(
        &self,
        template: RecordId,
    ) -> Result<RecordId, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            return Ok(template);
        };
        plan.nominals
            .records
            .iter()
            .find(|record| record.template == template && record.arguments.is_empty())
            .and_then(|record| record.emitted)
            .ok_or_else(|| bad(self.sources().eof()))
    }
    pub fn template_field_for_instance(&self, field: FieldId) -> Result<FieldId, Box<Diagnostic>> {
        Ok(FieldId {
            record: self.template_record_for_instance(field.record)?,
            index: field.index,
        })
    }
    pub fn instantiated_record_field(
        &self,
        field: FieldId,
        syntax: ast::TypeSyntax,
        work: &WorkMeter,
    ) -> Result<ParameterTy, Box<Diagnostic>> {
        if let Some(plan) = self.generic_plan() {
            work.debit(1, syntax.span, "instantiated record field")?;
            return plan
                .nominals
                .field(field.record, field.index, syntax.span)
                .map(ParameterTy::Value);
        }
        let (_, module) = self.record(field.record)?;
        self.query(work).parameter_type(module, syntax)
    }
    pub fn source_record_pattern(
        &self,
        caller: DefId,
        span: Span,
        work: &WorkMeter,
    ) -> Result<(RecordId, &[ast::RecordPatternField], Span), Box<Diagnostic>> {
        let (key, module) = self.function(caller)?;
        let program = self.sources().ast(module)?;
        for block in &program.functions[key.index].blocks {
            for statement in &block.body {
                work.debit(1, span, "record pattern source identity")?;
                if statement.span != span {
                    continue;
                }
                let StmtKind::DestructureRecord {
                    record,
                    fields,
                    init,
                } = &statement.kind
                else {
                    return Err(bad(span));
                };
                let template = self.query(work).record_type(
                    module,
                    ItemPathRef {
                        file: span.file,
                        path: *record,
                    },
                    TypeContext::Value,
                )?;
                let mut initializer = *init;
                let initializer = loop {
                    work.debit(1, span, "record pattern initializer origin")?;
                    let expression = program
                        .expressions
                        .get(initializer.0)
                        .ok_or_else(|| bad(span))?;
                    match expression.kind {
                        ExprKind::Group(inner) => initializer = inner,
                        _ => break expression.span,
                    }
                };
                return Ok((
                    self.instantiated_record_pattern(caller, span, template, work)?,
                    fields,
                    initializer,
                ));
            }
        }
        Err(bad(span))
    }
    pub fn instantiated_record_pattern(
        &self,
        caller: DefId,
        span: Span,
        template: RecordId,
        work: &WorkMeter,
    ) -> Result<RecordId, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            return Ok(template);
        };
        let instance = plan.instances.get(caller.0).ok_or_else(|| bad(span))?;
        work.debit(
            instance.matches.len() as u64 + 1,
            span,
            "record pattern source type",
        )?;
        let value = instance
            .matches
            .iter()
            .find(|(at, _)| *at == span)
            .map(|(_, value)| *value)
            .ok_or_else(|| bad(span))?;
        let ValueTy::Owned(AggregateTy::Record(record)) = plan.nominals.value(value, span)? else {
            return Err(bad(span));
        };
        if self.template_record_for_instance(record)? != template {
            return Err(bad(span));
        }
        Ok(record)
    }
    pub fn instantiated_constructor(
        &self,
        caller: DefId,
        expression: ExprId,
        template: RecordId,
    ) -> Result<RecordId, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            return Ok(template);
        };
        let instance = plan
            .instances
            .get(caller.0)
            .ok_or_else(|| bad(self.sources().eof()))?;
        let value = instance
            .constructors
            .iter()
            .find(|(id, _)| *id == expression)
            .map(|(_, value)| *value)
            .ok_or_else(|| bad(self.sources().eof()))?;
        let ValueTy::Owned(AggregateTy::Record(record)) =
            plan.nominals.value(value, self.sources().eof())?
        else {
            return Err(bad(self.sources().eof()));
        };
        if self.template_record_for_instance(record)? != template {
            return Err(bad(self.sources().eof()));
        }
        Ok(record)
    }
    pub fn template_for_instance(&self, function: DefId) -> Result<DefId, Box<Diagnostic>> {
        match self.generic_plan() {
            Some(plan) => plan.definition(function),
            None => Ok(function),
        }
    }
    pub fn is_type_parameter(
        &self,
        function: DefId,
        syntax: ast::TypeSyntax,
    ) -> Result<bool, Box<Diagnostic>> {
        if self.generic_plan().is_none() {
            return Ok(false);
        }
        let (key, module) = self.function(function)?;
        let program = self.sources().ast(module)?;
        let path = match syntax.kind {
            TypeSyntaxKind::Name(path)
            | TypeSyntaxKind::Reference { referent: path, .. }
            | TypeSyntaxKind::GenericSliceReference { element: path, .. } => path,
            _ => return Ok(false),
        };
        let ast::ItemPath::Unqualified(name) = path else {
            return Ok(false);
        };
        let name = self.sources().text(name)?;
        for parameter in program.generic_parameters(program.functions[key.index].name) {
            if self.sources().text(parameter.name)? == name {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn instantiated_parameter(
        &self,
        function: DefId,
        syntax: ast::TypeSyntax,
        work: &WorkMeter,
    ) -> Result<ParameterTy, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            let (_, module) = self.function(function)?;
            return self.query(work).parameter_type(module, syntax);
        };
        let instance = plan
            .instances
            .get(function.0)
            .ok_or_else(|| bad(syntax.span))?;
        work.debit(
            instance.types.len() as u64 + 1,
            syntax.span,
            "instantiated source type",
        )?;
        let ty = instance
            .types
            .iter()
            .find(|(span, _)| *span == syntax.span)
            .map(|(_, ty)| *ty)
            .ok_or_else(|| bad(syntax.span))?;
        plan.nominals.parameter(ty, syntax.span)
    }
    pub fn instantiated_value(
        &self,
        function: DefId,
        syntax: ast::TypeSyntax,
        work: &WorkMeter,
    ) -> Result<ValueTy, Box<Diagnostic>> {
        if self.generic_plan().is_none() {
            let (_, module) = self.function(function)?;
            return self
                .query(work)
                .value_type(module, syntax, TypeContext::Value);
        }
        match self.instantiated_parameter(function, syntax, work)? {
            ParameterTy::Value(ty) => Ok(ty),
            _ => Err(error("a reference type cannot be used here", syntax.span)),
        }
    }
    pub fn instantiated_call(
        &self,
        caller: DefId,
        expression: ExprId,
        template: DefId,
    ) -> Result<DefId, Box<Diagnostic>> {
        let Some(plan) = self.generic_plan() else {
            return Ok(template);
        };
        let instance = plan
            .instances
            .get(caller.0)
            .ok_or_else(|| bad(self.sources().eof()))?;
        let row = instance
            .calls
            .binary_search_by_key(&expression.0, |call| call.expression.0)
            .ok()
            .and_then(|position| instance.calls.get(position))
            .ok_or_else(|| bad(self.sources().eof()))?;
        if row.template != template {
            return Err(bad(self.sources().eof()));
        }
        Ok(row.target)
    }
    pub fn validate_generic_source(&self, work: &WorkMeter) -> Result<(), Box<Diagnostic>> {
        if let Some(plan) = self.generic_plan() {
            let expected = Plan::build(self, work, &mut Allocator::default())?;
            if *plan != expected {
                return Err(bad(self.sources().eof()));
            }
        }
        Ok(())
    }
    pub fn has_generic_instances(&self) -> bool {
        self.generic_plan().is_some()
    }
    pub fn expected_generic_construction(
        &self,
        caller: DefId,
        span: Span,
        work: &WorkMeter,
    ) -> Result<(ValueTy, Option<VariantId>), Box<Diagnostic>> {
        let plan = self.generic_plan().ok_or_else(|| bad(span))?;
        let instance = plan.instances.get(caller.0).ok_or_else(|| bad(span))?;
        let (_, module) = self.template_function(instance.definition)?;
        let ast = self.sources().ast(module)?;
        for (id, value) in &instance.constructors {
            work.debit(1, span, "generic constructor source identity")?;
            let expression = &ast.expressions[id.0];
            if expression.span != span {
                continue;
            }
            let variant = match expression.kind {
                ExprKind::GenericVariant {
                    application,
                    variant,
                    ..
                } => {
                    let application = ast
                        .generic_application(application)
                        .ok_or_else(|| bad(span))?;
                    Some(self.generic_variant_template(module, application.path, variant, work)?)
                }
                ExprKind::QualifiedValue { path, .. } => Some(self.query(work).variant(
                    module,
                    QualifiedPathRef {
                        file: span.file,
                        path,
                    },
                )?),
                ExprKind::StructLiteral { .. } => None,
                _ => return Err(bad(span)),
            };
            return Ok((
                plan.nominals.value(*value, span)?,
                variant
                    .map(|variant| self.lowered_variant(*value, variant))
                    .transpose()?,
            ));
        }
        Err(bad(span))
    }
    pub fn expected_generic_match(
        &self,
        caller: DefId,
        span: Span,
        work: &WorkMeter,
    ) -> Result<EnumId, Box<Diagnostic>> {
        let plan = self.generic_plan().ok_or_else(|| bad(span))?;
        let instance = plan.instances.get(caller.0).ok_or_else(|| bad(span))?;
        work.debit(
            instance.matches.len() as u64 + 1,
            span,
            "generic match source identity",
        )?;
        let value = instance
            .matches
            .iter()
            .find(|(at, _)| *at == span)
            .map(|(_, value)| *value)
            .ok_or_else(|| bad(span))?;
        match plan.nominals.value(value, span)? {
            ValueTy::Owned(AggregateTy::Enum(enumeration)) => Ok(enumeration),
            _ => Err(bad(span)),
        }
    }
    pub fn expected_generic_call(
        &self,
        caller: DefId,
        span: Span,
        work: &WorkMeter,
    ) -> Result<DefId, Box<Diagnostic>> {
        let plan = self.generic_plan().ok_or_else(|| bad(span))?;
        let instance = plan.instances.get(caller.0).ok_or_else(|| bad(span))?;
        let (_, module) = self.template_function(instance.definition)?;
        let program = self.sources().ast(module)?;
        for call in &instance.calls {
            work.debit(1, span, "generic call source identity")?;
            if program.expressions[call.expression.0].span == span {
                return Ok(call.target);
            }
        }
        Err(bad(span))
    }
}

#[derive(Clone, Copy)]
struct Binding {
    name: Span,
    ty: Type,
    mutable: bool,
    moved: bool,
}
struct Loan {
    binding: usize,
    kind: BorrowKind,
    path: Span,
}
struct Checked {
    calls: Vec<Call>,
    types: Vec<(Span, Type)>,
    constructors: Vec<(ExprId, Value)>,
    matches: Vec<(Span, Value)>,
}
struct Walker<'a, 's> {
    index: &'a DeclarationIndex<'s>,
    work: &'a WorkMeter,
    allocator: &'a mut Allocator,
    storage: &'a mut Storage,
    nominals: &'a mut Nominals,
    plan: Option<&'a mut Plan>,
    instance: Option<DefId>,
    definition: DefId,
    program: &'s ast::Program,
    function: &'s ast::Function,
    module: ModuleId,
    arguments: Vec<Value>,
    bindings: Vec<Binding>,
    calls: Vec<Call>,
    types: Vec<(Span, Type)>,
    constructors: Vec<(ExprId, Value)>,
    matches: Vec<(Span, Value)>,
    loans: Vec<Loan>,
    abstract_body: bool,
    reachable: bool,
    loop_frames: Vec<flow::LoopFrame>,
}

#[derive(Clone, Copy)]
struct Flow {
    falls: bool,
    returns: bool,
    breaks: bool,
    continues: bool,
}
impl Flow {
    const FALL: Self = Self {
        falls: true,
        returns: false,
        breaks: false,
        continues: false,
    };
    fn union(self, other: Self) -> Self {
        Self {
            falls: self.falls || other.falls,
            returns: self.returns || other.returns,
            breaks: self.breaks || other.breaks,
            continues: self.continues || other.continues,
        }
    }
}
impl<'a, 's> Walker<'a, 's> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        index: &'a DeclarationIndex<'s>,
        work: &'a WorkMeter,
        allocator: &'a mut Allocator,
        storage: &'a mut Storage,
        nominals: &'a mut Nominals,
        plan: Option<&'a mut Plan>,
        definition: DefId,
        arguments: Vec<Value>,
        instance: Option<DefId>,
    ) -> Result<Self, Box<Diagnostic>> {
        let (key, module) = index.template_function(definition)?;
        let program = index.sources().ast(module)?;
        Ok(Self {
            index,
            work,
            allocator,
            storage,
            abstract_body: plan.is_none(),
            nominals,
            plan,
            instance,
            definition,
            program,
            function: &program.functions[key.index],
            module,
            arguments,
            bindings: Vec::new(),
            calls: Vec::new(),
            types: Vec::new(),
            constructors: Vec::new(),
            matches: Vec::new(),
            loans: Vec::new(),
            reachable: true,
            loop_frames: Vec::new(),
        })
    }
    fn check(mut self) -> Result<Checked, Box<Diagnostic>> {
        for parameter in &self.function.params {
            let ty = self.ty(parameter.ty)?;
            self.exposure(ty, parameter.ty.span)?;
            self.bind(parameter.name, ty, false)?;
        }
        let result = self.ty(self.function.result)?;
        self.exposure(result, self.function.result.span)?;
        let flow = self.block(self.function.body, 0)?;
        if flow.falls && self.ty(self.function.result)? != Type::scalar(Ty::Unit) {
            return Err(error("not all paths return a value", self.function.end));
        }
        self.calls.sort_unstable_by_key(|call| call.expression.0);
        if self
            .calls
            .windows(2)
            .any(|pair| pair[0].expression == pair[1].expression)
        {
            return Err(bad(self.function.name));
        }
        Ok(Checked {
            calls: self.calls,
            types: self.types,
            constructors: self.constructors,
            matches: self.matches,
        })
    }
    fn ty(&mut self, syntax: ast::TypeSyntax) -> Result<Type, Box<Diagnostic>> {
        self.work.debit(
            self.types.len() as u64 + 1,
            syntax.span,
            "generic syntax type lookup",
        )?;
        if let Some((_, ty)) = self.types.iter().find(|(span, _)| *span == syntax.span) {
            return Ok(*ty);
        }
        let ty = self.nominals.syntax(
            self.index,
            self.work,
            self.storage,
            self.allocator,
            self.module,
            self.function.name,
            &self.arguments,
            syntax,
            0,
        )?;
        self.storage.push(
            &mut self.types,
            (syntax.span, ty),
            self.allocator,
            self.work,
            syntax.span,
        )?;
        Ok(ty)
    }
    fn exposure(&self, ty: Type, at: Span) -> Result<(), Box<Diagnostic>> {
        if !self.abstract_body
            && !self
                .program
                .generic_parameters(self.function.name)
                .is_empty()
        {
            return Ok(());
        }
        let value = match ty {
            Type::Value(value) | Type::Reference { value, .. } => value,
            _ => return Ok(()),
        };
        self.nominals.exposure(
            self.index,
            self.work,
            ExposureOwner::Function(self.definition),
            value,
            at,
            0,
        )
    }
    fn mutable_place(&self, position: usize, at: Span) -> Result<(), Box<Diagnostic>> {
        if self.borrow_conflict(position, true, at)? {
            return Err(error("cannot write a borrowed value", at));
        }
        let binding = self.bindings[position];
        if matches!(
            binding.ty,
            Type::Reference {
                kind: BorrowKind::Exclusive,
                ..
            } | Type::Slice {
                kind: BorrowKind::Exclusive,
                ..
            }
        ) || (matches!(binding.ty, Type::Value(_)) && binding.mutable)
        {
            Ok(())
        } else {
            Err(error("assignment requires mutable access", at))
        }
    }
    fn copy(&self, value: Value) -> bool {
        self.nominals.copy(value)
    }
    fn bind(&mut self, name: Span, ty: Type, mutable: bool) -> Result<(), Box<Diagnostic>> {
        let spelling = self.index.sources().text(name)?;
        for binding in &self.bindings {
            self.work.debit(1, name, "generic local lookup")?;
            if self.index.sources().text(binding.name)? == spelling {
                return Err(duplicate(name, binding.name));
            }
        }
        self.storage.push(
            &mut self.bindings,
            Binding {
                name,
                ty,
                mutable,
                moved: false,
            },
            self.allocator,
            self.work,
            name,
        )
    }
    fn local(&self, name: Span) -> Result<usize, Box<Diagnostic>> {
        let spelling = self.index.sources().text(name)?;
        for (id, binding) in self.bindings.iter().enumerate().rev() {
            self.work.debit(1, name, "generic local lookup")?;
            if self.index.sources().text(binding.name)? == spelling {
                if self.abstract_body && self.reachable && binding.moved {
                    return Err(error(
                        if self.in_repeated_loop() {
                            "use of a moved value on a previous loop iteration"
                        } else {
                            "use of a moved value in generic function"
                        },
                        name,
                    ));
                }
                return Ok(id);
            }
        }
        Err(error("unknown local variable", name))
    }
    fn expect(&self, actual: Type, expected: Type, at: Span) -> Result<(), Box<Diagnostic>> {
        if actual == expected {
            return Ok(());
        }
        let compatible = match (actual, expected) {
            (
                Type::Reference {
                    value: a,
                    kind: actual_kind,
                },
                Type::Reference {
                    value: b,
                    kind: expected_kind,
                },
            ) => {
                a == b
                    && (actual_kind == BorrowKind::Exclusive || expected_kind == BorrowKind::Shared)
            }
            (
                Type::Reference {
                    value: Value::Concrete(ValueTy::Owned(AggregateTy::FixedArray(array))),
                    kind,
                },
                Type::Slice {
                    element,
                    kind: expected,
                },
            ) => {
                Element::Concrete(array.element()) == element
                    && (kind == BorrowKind::Exclusive || expected == BorrowKind::Shared)
            }
            (
                Type::Reference {
                    value: Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)),
                    kind,
                },
                Type::Slice {
                    element: Element::Concrete(Ty::U8),
                    kind: expected,
                },
            ) => kind == BorrowKind::Exclusive || expected == BorrowKind::Shared,
            (
                Type::Reference {
                    value: Value::Vector(element),
                    kind,
                },
                Type::Slice {
                    element: expected_element,
                    kind: expected,
                },
            ) => {
                element == expected_element
                    && (kind == BorrowKind::Exclusive || expected == BorrowKind::Shared)
            }
            (
                Type::Slice { element, kind },
                Type::Slice {
                    element: expected,
                    kind: expected_kind,
                },
            ) => {
                element == expected
                    && (kind == BorrowKind::Exclusive || expected_kind == BorrowKind::Shared)
            }
            _ => false,
        };
        if compatible {
            Ok(())
        } else {
            Err(error("generic argument or value type mismatch", at))
        }
    }
    fn snapshot(&mut self) -> Result<Vec<Binding>, Box<Diagnostic>> {
        let mut result = Vec::new();
        for binding in &self.bindings {
            self.storage.push(
                &mut result,
                *binding,
                self.allocator,
                self.work,
                binding.name,
            )?;
        }
        Ok(result)
    }
    fn block(&mut self, id: ast::BodyBlockId, loops: usize) -> Result<Flow, Box<Diagnostic>> {
        let block = &self.function.blocks[id.0];
        let base = self.bindings.len();
        let loan_base = self.loans.len();
        let inherited_reachable = self.reachable;
        let mut flow = Flow::FALL;
        let mut terminal_state = None;
        for (position, statement) in block.body.iter().enumerate() {
            self.reachable = inherited_reachable && flow.falls;
            self.work.debit(1, statement.span, "generic statement")?;
            let next = match &statement.kind {
                StmtKind::DestructureRecord {
                    record,
                    fields,
                    init,
                } => {
                    reject_parameter_base(
                        self.index,
                        self.work,
                        self.module,
                        self.function.name,
                        *record,
                        statement.span,
                    )?;
                    let value = self.expression(*init)?.value(statement.span)?;
                    let Value::Record(application) = value else {
                        return Err(error(
                            "record pattern requires a known owned record",
                            statement.span,
                        ));
                    };
                    let template = self.index.query(self.work).record_type(
                        self.module,
                        ItemPathRef {
                            file: statement.span.file,
                            path: *record,
                        },
                        TypeContext::Value,
                    )?;
                    if self.nominals.records[application].template != template {
                        return Err(error("record pattern type mismatch", statement.span));
                    }
                    let (key, module) = self.index.template_record(template)?;
                    let declaration = &self.index.sources().ast(module)?.records[key.index];
                    if fields.len() != declaration.fields.len() {
                        return Err(error(
                            "record pattern must bind every field",
                            statement.span,
                        ));
                    }
                    let mut seen = Vec::new();
                    for _ in &declaration.fields {
                        self.storage.push(
                            &mut seen,
                            false,
                            self.allocator,
                            self.work,
                            statement.span,
                        )?;
                    }
                    for field in fields {
                        self.work.debit(
                            declaration.fields.len() as u64 + 1,
                            field.span,
                            "record pattern field identity",
                        )?;
                        let position = declaration
                            .fields
                            .iter()
                            .position(|member| {
                                self.index.sources().text(member.name).ok()
                                    == self.index.sources().text(field.field).ok()
                            })
                            .ok_or_else(|| error("unknown record pattern field", field.field))?;
                        if seen[position] {
                            return Err(error("duplicate record pattern field", field.field));
                        }
                        seen[position] = true;
                        if !matches!(
                            self.index.query(self.work).field_access(
                                self.module,
                                FieldId {
                                    record: template,
                                    index: position
                                },
                                field.field
                            )?,
                            Access::Allowed
                        ) {
                            return Err(error("record pattern uses a private field", field.field));
                        }
                        let ty = Type::Value(self.nominals.records[application].fields[position]);
                        self.bind(field.binding, ty, false)?;
                    }
                    if !self.matches.iter().any(|(span, _)| *span == statement.span) {
                        self.storage.push(
                            &mut self.matches,
                            (statement.span, value),
                            self.allocator,
                            self.work,
                            statement.span,
                        )?;
                    }
                    Flow::FALL
                }
                StmtKind::Let {
                    name,
                    annotation,
                    init,
                    mutable,
                } => {
                    let ty = if matches!(&self.program.expressions[init.0].kind, ExprKind::ArrayLiteral { elements } if elements.is_empty())
                    {
                        let annotation = annotation.ok_or_else(|| {
                            error(
                                "empty array literal requires a local type annotation",
                                statement.span,
                            )
                        })?;
                        let expected = self.ty(annotation)?;
                        if !matches!(expected, Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::FixedArray(array)))) if array.length() == 0)
                        {
                            return Err(error(
                                "empty array annotation requires a zero-length array",
                                annotation.span,
                            ));
                        }
                        expected
                    } else {
                        self.expression(*init)?
                    };
                    if !matches!(
                        self.program.expressions[init.0].kind,
                        ExprKind::Borrow { .. }
                    ) {
                        ty.value(statement.span)?;
                    } else if *mutable {
                        return Err(error("reference bindings cannot be reassigned", *name));
                    }
                    if let Some(annotation) = annotation {
                        let expected = self.ty(*annotation)?;
                        self.expect(ty, expected, annotation.span)?;
                    }
                    self.bind(*name, ty, *mutable)?;
                    Flow::FALL
                }
                StmtKind::Assign { name, value, .. } => {
                    // Assignment is allowed to restore a moved mutable binding.
                    let spelling = self.index.sources().text(*name)?;
                    let position = self
                        .bindings
                        .iter()
                        .position(|binding| {
                            self.index.sources().text(binding.name).ok() == Some(spelling)
                        })
                        .ok_or_else(|| error("unknown assignment target", *name))?;
                    let target = self.bindings[position];
                    if !target.mutable {
                        return Err(error("assignment requires a mutable binding", *name));
                    }
                    if self.borrow_conflict(position, true, *name)? {
                        return Err(error("cannot replace a borrowed value", *name));
                    }
                    let actual = self.expression(*value)?;
                    self.expect(actual, target.ty, *name)?;
                    if self.reachable {
                        self.bindings[position].moved = false;
                    }
                    Flow::FALL
                }
                StmtKind::Expr(expression) => {
                    self.expression(*expression)?.value(statement.span)?;
                    Flow::FALL
                }
                StmtKind::Return(expression) => {
                    let ty = if let Some(expression) = expression {
                        self.expression(*expression)?
                    } else {
                        Type::scalar(Ty::Unit)
                    };
                    let expected = self.ty(self.function.result)?;
                    self.expect(ty, expected, statement.span)?;
                    Flow {
                        falls: false,
                        returns: true,
                        breaks: false,
                        continues: false,
                    }
                }
                StmtKind::If {
                    condition,
                    then_block,
                    else_block,
                } => {
                    let condition = self.expression(*condition)?;
                    self.expect(condition, Type::scalar(Ty::Bool), statement.span)?;
                    let before = self.snapshot()?;
                    let then_flow = self.block(*then_block, loops)?;
                    let then_state = self.snapshot()?;
                    self.bindings = before;
                    let else_flow = if let Some(block) = else_block {
                        self.block(*block, loops)?
                    } else {
                        Flow::FALL
                    };
                    if then_flow.falls {
                        for (other, binding) in then_state.iter().zip(&mut self.bindings) {
                            if else_flow.falls {
                                binding.moved |= other.moved;
                            } else {
                                binding.moved = other.moved;
                            }
                        }
                    }
                    then_flow.union(else_flow)
                }
                StmtKind::While { condition, body } => {
                    self.check_loop(Some(*condition), *body, loops, statement.span)?
                }
                StmtKind::ForRange {
                    name,
                    start,
                    end,
                    body,
                } => {
                    let first = self.expression(*start)?;
                    if !matches!(first, Type::Value(Value::Concrete(ValueTy::Scalar(ty))) if ty.integer_bits().is_some())
                    {
                        return Err(error(
                            "range bounds require known integer types",
                            self.program.expressions[start.0].span,
                        ));
                    }
                    let last = self.expression(*end)?;
                    self.expect(last, first, self.program.expressions[end.0].span)?;
                    let outer = self.bindings.len();
                    self.bind(*name, first, false)?;
                    let flow = self.check_loop(None, *body, loops, statement.span)?;
                    self.bindings.truncate(outer);
                    flow
                }
                StmtKind::Break | StmtKind::Continue => {
                    if loops == 0 {
                        return Err(error("loop transfer outside a loop", statement.span));
                    }
                    self.record_loop_exit(
                        matches!(statement.kind, StmtKind::Break),
                        statement.span,
                    )?;
                    Flow {
                        falls: false,
                        returns: false,
                        breaks: matches!(statement.kind, StmtKind::Break),
                        continues: matches!(statement.kind, StmtKind::Continue),
                    }
                }
                StmtKind::FieldAssign {
                    base, field, value, ..
                } => {
                    self.mutable_place(
                        self.local(*base)?,
                        Span {
                            start: base.start,
                            ..*field
                        },
                    )?;
                    let expected = self.field(*base, *field)?;
                    let actual = self.expression(*value)?;
                    self.expect(actual, expected, statement.span)?;
                    Flow::FALL
                }
                StmtKind::IndexAssign { target, value, .. } => {
                    let ExprKind::IndexRead { base, .. } = self.program.expressions[target.0].kind
                    else {
                        return Err(bad(statement.span));
                    };
                    self.mutable_place(self.place(base)?.0, base)?;
                    let expected = self.expression(*target)?;
                    let actual = self.expression(*value)?;
                    self.expect(actual, expected, statement.span)?;
                    Flow::FALL
                }
                StmtKind::Match { scrutinee, arms } => {
                    let position = self.local(*scrutinee)?;
                    let value = self.bindings[position].ty.value(*scrutinee)?;
                    let enumeration = self.nominals.enum_template(value, *scrutinee)?;
                    if !self.matches.iter().any(|(span, _)| *span == statement.span) {
                        self.storage.push(
                            &mut self.matches,
                            (statement.span, value),
                            self.allocator,
                            self.work,
                            statement.span,
                        )?;
                    }
                    if self.abstract_body && self.reachable {
                        self.bindings[position].moved = true;
                    }
                    let before = self.snapshot()?;
                    let mut merged = self.snapshot()?;
                    let mut result = Flow {
                        falls: false,
                        returns: false,
                        breaks: false,
                        continues: false,
                    };
                    let mut seen = [false; crate::frontend::parser::MAX_ENUM_VARIANTS];
                    let mut first_fall = true;
                    for arm in arms {
                        let variant = self.index.query(self.work).variant(
                            self.module,
                            QualifiedPathRef {
                                file: arm.span.file,
                                path: arm.variant,
                            },
                        )?;
                        if variant.enumeration != enumeration || seen[variant.index] {
                            return Err(error("duplicate or mismatched enum match arm", arm.span));
                        }
                        seen[variant.index] = true;
                        if !self.matches.iter().any(|(span, _)| *span == arm.span) {
                            self.storage.push(
                                &mut self.matches,
                                (arm.span, value),
                                self.allocator,
                                self.work,
                                arm.span,
                            )?;
                        }
                        let payload =
                            self.nominals
                                .variant(self.index, value, variant.index, arm.span)?;
                        self.bindings.clear();
                        for binding in &before {
                            self.storage.push(
                                &mut self.bindings,
                                *binding,
                                self.allocator,
                                self.work,
                                arm.span,
                            )?;
                        }
                        match (arm.binding, payload) {
                            (Some(name), Some(ty)) => self.bind(name, Type::Value(ty), false)?,
                            (None, None) => (),
                            _ => return Err(error("match arm payload binding mismatch", arm.span)),
                        }
                        let flow = self.block(arm.body, loops)?;
                        if flow.falls {
                            for (target, binding) in merged.iter_mut().zip(&self.bindings) {
                                if first_fall {
                                    target.moved = binding.moved;
                                } else {
                                    target.moved |= binding.moved;
                                }
                            }
                            first_fall = false;
                        }
                        result = result.union(flow);
                    }
                    if seen[..self
                        .nominals
                        .variant_count(self.index, value, statement.span)?]
                        .iter()
                        .any(|seen| !seen)
                    {
                        return Err(error("non-exhaustive enum match", statement.span));
                    }
                    self.bindings = merged;
                    result
                }
            };
            if flow.falls {
                flow = Flow {
                    falls: false,
                    ..flow
                }
                .union(next);
                if !flow.falls && position + 1 < block.body.len() {
                    terminal_state = Some(self.snapshot()?);
                }
            }
        }
        if let Some(state) = terminal_state {
            for (binding, terminal) in self.bindings.iter_mut().zip(&state) {
                binding.moved = terminal.moved;
            }
        }
        self.bindings.truncate(base);
        self.loans.truncate(loan_base);
        self.reachable = inherited_reachable;
        Ok(flow)
    }
    fn expression(&mut self, id: ExprId) -> Result<Type, Box<Diagnostic>> {
        let expression = &self.program.expressions[id.0];
        self.work.debit(1, expression.span, "generic expression")?;
        Ok(match &expression.kind {
            ExprKind::GenericVariant {
                application,
                variant,
                args,
            } => {
                let syntax = self
                    .program
                    .generic_application(*application)
                    .ok_or_else(|| bad(expression.span))?;
                reject_parameter_base(
                    self.index,
                    self.work,
                    self.module,
                    self.function.name,
                    syntax.path,
                    expression.span,
                )?;
                let variant = self.index.generic_variant_template(
                    self.module,
                    syntax.path,
                    *variant,
                    self.work,
                )?;
                self.enum_literal(
                    id,
                    variant,
                    args.as_deref(),
                    Some(&syntax.arguments),
                    expression.span,
                )?
            }
            ExprKind::Borrow {
                mutable,
                name,
                forwarded,
            } => {
                let place = if *forwarded {
                    ast::BorrowPlace::ForwardedParameter {
                        name: *name,
                        star_span: expression.span,
                    }
                } else {
                    ast::BorrowPlace::OwnerName(*name)
                };
                let borrowed = self.argument(&Argument::Borrow {
                    mutable: *mutable,
                    place,
                    span: expression.span,
                })?;
                if matches!(
                    borrowed,
                    Type::Reference {
                        value: Value::Concrete(
                            ValueTy::Scalar(_) | ValueTy::Owned(AggregateTy::Enum(_))
                        ),
                        ..
                    } | Type::Slice { .. }
                ) {
                    return Err(error(
                        "local references require a complete owned value",
                        expression.span,
                    ));
                }
                borrowed
            }
            ExprKind::Name(name) => {
                let position = self.local(*name)?;
                let ty = self.bindings[position].ty;
                if let Type::Value(value) = ty {
                    if self.abstract_body && self.reachable {
                        if self.borrow_conflict(position, !self.copy(value), *name)? {
                            return Err(error("value is borrowed", *name));
                        }
                        if !self.copy(value) {
                            self.bindings[position].moved = true;
                        }
                    }
                }
                ty
            }
            ExprKind::Bool(_) => Type::scalar(Ty::Bool),
            ExprKind::Character(_) => Type::scalar(Ty::Char),
            ExprKind::Number { digits, negative } => {
                self.number(*digits, *negative, Ty::I32)?;
                Type::scalar(Ty::I32)
            }
            ExprKind::I64Number { digits, negative } => {
                self.number(*digits, *negative, Ty::I64)?;
                Type::scalar(Ty::I64)
            }
            ExprKind::IntegerNumber {
                digits,
                negative,
                kind,
            } => {
                let ty = Ty::from_extended_integer(*kind);
                self.number(*digits, *negative, ty)?;
                Type::scalar(ty)
            }
            ExprKind::FloatNumber { digits, kind, .. } => {
                let text = self.index.sources().text(*digits)?;
                let finite = match kind {
                    ast::FloatTypeSyntax::F32 => {
                        text.parse::<f32>().is_ok_and(|value| value.is_finite())
                    }
                    ast::FloatTypeSyntax::F64 => {
                        text.parse::<f64>().is_ok_and(|value| value.is_finite())
                    }
                };
                if !finite {
                    return Err(error(
                        "floating-point literal is outside its finite range",
                        expression.span,
                    ));
                }
                Type::scalar(Ty::from_float(*kind))
            }
            ExprKind::Unit => Type::scalar(Ty::Unit),
            ExprKind::Group(inner) => self.expression(*inner)?,
            ExprKind::Call { callee, args } => {
                let definition = self.index.query(self.work).callee(
                    self.module,
                    ItemPathRef {
                        file: expression.span.file,
                        path: *callee,
                    },
                    false,
                )?;
                self.call(id, definition, args, expression.span)?
            }
            ExprKind::QualifiedValue { path, args } => {
                reject_parameter_base(
                    self.index,
                    self.work,
                    self.module,
                    self.function.name,
                    ast::ItemPath::Absolute(*path),
                    expression.span,
                )?;
                match self.index.query(self.work).qualified_value_endpoint(
                    self.module,
                    QualifiedPathRef {
                        file: expression.span.file,
                        path: *path,
                    },
                )? {
                    QualifiedValueEndpoint::Function(definition) => self.call(
                        id,
                        definition,
                        args.as_deref().unwrap_or(&[]),
                        expression.span,
                    )?,
                    QualifiedValueEndpoint::Variant(variant) => {
                        if self.program.generic_arguments(expression.span).is_some() {
                            return Err(error(
                                "enum variants do not accept function type arguments",
                                expression.span,
                            ));
                        }
                        self.enum_literal(id, variant, args.as_deref(), None, expression.span)?
                    }
                }
            }
            ExprKind::StructLiteral { record, fields } => {
                reject_parameter_base(
                    self.index,
                    self.work,
                    self.module,
                    self.function.name,
                    *record,
                    expression.span,
                )?;
                let path_span = self
                    .program
                    .item_path_span(*record)
                    .ok_or_else(|| bad(expression.span))?;
                let record = self.index.query(self.work).record_type(
                    self.module,
                    ItemPathRef {
                        file: expression.span.file,
                        path: *record,
                    },
                    TypeContext::Constructor,
                )?;
                if self.program.generic_arguments(expression.span).is_none() {
                    if let Some(value) =
                        self.infer_record_constructor(id, record, fields, expression.span)?
                    {
                        return Ok(value);
                    }
                }
                let mut arguments = Vec::new();
                if let Some(explicit) = self.program.generic_arguments(expression.span) {
                    for syntax in explicit {
                        let value = self.ty(*syntax)?.value(syntax.span)?;
                        self.storage.push(
                            &mut arguments,
                            value,
                            self.allocator,
                            self.work,
                            syntax.span,
                        )?;
                    }
                }
                let value = self.nominals.record(
                    self.index,
                    self.work,
                    self.storage,
                    self.allocator,
                    record,
                    arguments,
                    path_span,
                    0,
                )?;
                let Value::Record(application) = value else {
                    return Err(bad(expression.span));
                };
                if !self
                    .constructors
                    .iter()
                    .any(|(expression, _)| *expression == id)
                {
                    self.storage.push(
                        &mut self.constructors,
                        (id, value),
                        self.allocator,
                        self.work,
                        expression.span,
                    )?;
                }
                let (key, module) = self.index.template_record(record)?;
                let declaration = &self.index.sources().ast(module)?.records[key.index];
                if !matches!(
                    self.index.query(self.work).construction_access(
                        self.module,
                        record,
                        expression.span
                    )?,
                    Access::Allowed
                ) {
                    return Err(error(
                        "record construction uses a private field",
                        expression.span,
                    ));
                }
                if declaration.fields.len() != fields.len() {
                    return Err(error(
                        "record initializer field count mismatch",
                        expression.span,
                    ));
                }
                for (position, field) in fields.iter().enumerate() {
                    let name = self.index.sources().text(field.name)?;
                    for earlier in &fields[..position] {
                        if self.index.sources().text(earlier.name)? == name {
                            return Err(duplicate(field.name, earlier.name));
                        }
                    }
                    let position = declaration
                        .fields
                        .iter()
                        .position(|candidate| {
                            self.index.sources().text(candidate.name).ok() == Some(name)
                        })
                        .ok_or_else(|| error("unknown record field", field.name))?;
                    let expected = Type::Value(self.nominals.records[application].fields[position]);
                    let actual = self.expression(field.value)?;
                    self.expect(actual, expected, field.span)?;
                }
                Type::Value(value)
            }
            ExprKind::FieldRead { base, field } => self.field(*base, *field)?,
            ExprKind::ArrayLiteral { elements } => {
                if elements.is_empty() {
                    return Err(error(
                        "empty array literal requires a local type annotation",
                        expression.span,
                    ));
                }
                let mut element = None;
                for value in elements {
                    let Type::Value(Value::Concrete(ValueTy::Scalar(ty))) =
                        self.expression(*value)?
                    else {
                        return Err(error(
                            "array elements must have a concrete scalar type",
                            expression.span,
                        ));
                    };
                    if element.is_some_and(|element| element != ty) {
                        return Err(error("array element types differ", expression.span));
                    }
                    element = Some(ty);
                }
                let array = FixedArrayTy::check(element.unwrap_or(Ty::I32), elements.len())
                    .map_err(|_| error("invalid fixed array type", expression.span))?;
                Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::FixedArray(
                    array,
                ))))
            }
            ExprKind::ByteString { length, .. } => {
                Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::FixedArray(
                    FixedArrayTy::check(Ty::U8, usize::from(*length))
                        .map_err(|_| error("invalid byte string type", expression.span))?,
                ))))
            }
            ExprKind::IndexRead { base, index } => {
                let element = self.element(*base)?;
                let index = self.expression(*index)?;
                self.expect(index, Type::scalar(Ty::I32), expression.span)?;
                Type::Value(element.value())
            }
            ExprKind::ArrayLength { base } => {
                self.element(*base)?;
                Type::scalar(Ty::I32)
            }
            ExprKind::Not { operand, .. } => {
                let ty = self.expression(*operand)?;
                self.expect(ty, Type::scalar(Ty::Bool), expression.span)?;
                ty
            }
            ExprKind::Negate { operand, .. } | ExprKind::BitNot { operand, .. } => {
                let ty = self.expression(*operand)?;
                let Type::Value(Value::Concrete(ValueTy::Scalar(scalar))) = ty else {
                    return Err(error(
                        "this operation requires a numeric type bound",
                        expression.span,
                    ));
                };
                if scalar.integer_bits().is_none()
                    && !(scalar.floating() && matches!(expression.kind, ExprKind::Negate { .. }))
                {
                    return Err(error(
                        "numeric operation requires a numeric value",
                        expression.span,
                    ));
                }
                if matches!(expression.kind, ExprKind::Negate { .. })
                    && !scalar.signed_integer()
                    && !scalar.floating()
                {
                    return Err(error(
                        "negation requires a signed numeric type",
                        expression.span,
                    ));
                }
                ty
            }
            ExprKind::Logical { left, right, .. } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                self.expect(left, Type::scalar(Ty::Bool), expression.span)?;
                self.expect(right, left, expression.span)?;
                left
            }
            ExprKind::Arithmetic { left, right, .. }
            | ExprKind::Comparison { left, right, .. }
            | ExprKind::Bitwise { left, right, .. } => {
                let left = self.expression(*left)?;
                let right = self.expression(*right)?;
                if matches!(expression.kind, ExprKind::Bitwise { op, .. } if op.shift()) {
                    if !matches!(right, Type::Value(Value::Concrete(ValueTy::Scalar(ty))) if ty.integer_bits().is_some())
                    {
                        return Err(error(
                            "shift count requires a concrete integer",
                            expression.span,
                        ));
                    }
                } else {
                    self.expect(right, left, expression.span)?;
                }
                let Type::Value(Value::Concrete(ValueTy::Scalar(scalar))) = left else {
                    return Err(error(
                        "this operation requires a numeric or comparison type bound",
                        expression.span,
                    ));
                };
                let comparison = matches!(expression.kind, ExprKind::Comparison { .. });
                if scalar.integer_bits().is_none()
                    && !scalar.floating()
                    && !(comparison && matches!(scalar, Ty::Bool | Ty::Char))
                {
                    return Err(error(
                        "operation is not supported by this scalar type",
                        expression.span,
                    ));
                }
                if matches!(expression.kind, ExprKind::Bitwise { .. })
                    && scalar.integer_bits().is_none()
                {
                    return Err(error("bit operations require an integer", expression.span));
                }
                if scalar == Ty::Bool
                    && matches!(expression.kind, ExprKind::Comparison { op, .. } if !matches!(op, ast::ComparisonOp::Equal | ast::ComparisonOp::NotEqual))
                {
                    return Err(error(
                        "ordered comparison requires a numeric or character value",
                        expression.span,
                    ));
                }
                if comparison {
                    Type::scalar(Ty::Bool)
                } else {
                    left
                }
            }
            ExprKind::Conversion { operand, op, .. } => {
                let Type::Value(Value::Concrete(ValueTy::Scalar(input))) =
                    self.expression(*operand)?
                else {
                    return Err(error(
                        "conversion requires a concrete scalar type",
                        expression.span,
                    ));
                };
                let output = match op {
                    ast::ConversionOp::ToU8Checked if input.integer_bits().is_some() => {
                        Some(Ty::U8)
                    }
                    ast::ConversionOp::ToI32 if input == Ty::U8 => Some(Ty::I32),
                    ast::ConversionOp::ToI64 if matches!(input, Ty::U8 | Ty::I32) => Some(Ty::I64),
                    ast::ConversionOp::ToI32Checked if input.integer_bits().is_some() => {
                        Some(Ty::I32)
                    }
                    ast::ConversionOp::ToIntegerChecked(kind) if input.integer_bits().is_some() => {
                        Some(Ty::from_extended_integer(*kind))
                    }
                    _ => input
                        .character_conversion(*op)
                        .or_else(|| input.float_conversion(*op)),
                }
                .ok_or_else(|| error("unsupported scalar conversion", expression.span))?;
                Type::scalar(output)
            }
        })
    }
    fn enum_literal(
        &mut self,
        expression: ExprId,
        variant: VariantId,
        arguments: Option<&[Argument]>,
        explicit: Option<&[ast::TypeSyntax]>,
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        if explicit.is_none() {
            if let Some(value) = self.infer_enum_constructor(expression, variant, arguments, at)? {
                return Ok(value);
            }
        }
        let mut types = Vec::new();
        if let Some(explicit) = explicit {
            for syntax in explicit {
                let value = self.ty(*syntax)?.value(syntax.span)?;
                self.storage
                    .push(&mut types, value, self.allocator, self.work, syntax.span)?;
            }
        }
        let value = if variant.enumeration.0 < self.index.template_enum_count() {
            self.nominals.enumeration(
                self.index,
                self.work,
                self.storage,
                self.allocator,
                variant.enumeration,
                types,
                at,
                0,
            )?
        } else {
            if explicit.is_some() {
                return Err(error("this enum does not accept type arguments", at));
            }
            Value::Concrete(ValueTy::Owned(AggregateTy::Enum(variant.enumeration)))
        };
        let payload = self
            .nominals
            .variant(self.index, value, variant.index, at)?;
        match (payload, arguments) {
            (None, None) => (),
            (Some(expected), Some([Argument::Value(expression)])) => {
                let actual = self.expression(*expression)?;
                self.expect(actual, Type::Value(expected), at)?;
            }
            _ => return Err(error("enum variant payload mismatch", at)),
        }
        if !self.constructors.iter().any(|(id, _)| *id == expression) {
            self.storage.push(
                &mut self.constructors,
                (expression, value),
                self.allocator,
                self.work,
                at,
            )?;
        }
        Ok(Type::Value(value))
    }
    fn number(&self, digits: Span, negative: bool, ty: Ty) -> Result<(), Box<Diagnostic>> {
        let magnitude = self
            .index
            .sources()
            .text(digits)?
            .parse::<u128>()
            .map_err(|_| error("integer literal is outside its type range", digits))?;
        let bits = ty.integer_bits().ok_or_else(|| bad(digits))?;
        let limit = if ty.signed_integer() {
            (1u128 << (bits - 1)) - u128::from(!negative)
        } else if negative {
            return Err(error("negative literal requires a signed type", digits));
        } else if bits == 128 {
            u128::MAX
        } else {
            (1u128 << bits) - 1
        };
        if magnitude > limit {
            return Err(error("integer literal is outside its type range", digits));
        }
        Ok(())
    }
    fn field(&self, base: Span, fields: Span) -> Result<Type, Box<Diagnostic>> {
        let position = self.local(base)?;
        let read = Span {
            start: base.start,
            ..fields
        };
        if self.exclusive_loan(position, read)? {
            return Err(error(
                "record is exclusively borrowed during this call",
                fields,
            ));
        }
        let ty = self.fields(self.bindings[position].ty, fields)?;
        // Copy records do not add an aggregate field-extraction producer.
        // Keep opaque Copy parameters admissible until concrete substitution.
        if self.abstract_body
            && matches!(ty, Type::Value(value) if !matches!(value,
                Value::Concrete(ValueTy::Scalar(_)) | Value::Parameter { copy: true, .. }))
        {
            return Err(error(
                "cannot move an owned record field in a generic function",
                fields,
            ));
        }
        Ok(ty)
    }
    fn fields(&self, mut ty: Type, fields: Span) -> Result<Type, Box<Diagnostic>> {
        let start = self
            .program
            .tokens
            .partition_point(|token| token.span.end <= fields.start);
        for token in self.program.tokens[start..]
            .iter()
            .take_while(|token| token.span.start < fields.end)
            .filter(|token| token.kind == TokenKind::Ident)
        {
            let field = self.index.sources().text(token.span)?;
            self.work
                .debit(1, token.span, "generic record projection")?;
            let record = match ty {
                Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Record(id))))
                | Type::Reference {
                    value: Value::Concrete(ValueTy::Owned(AggregateTy::Record(id))),
                    ..
                } => id,
                Type::Value(Value::Record(id))
                | Type::Reference {
                    value: Value::Record(id),
                    ..
                } => self.nominals.records[id].template,
                _ => return Err(error("field access requires a known record type", fields)),
            };
            let application = match ty {
                Type::Value(Value::Record(id))
                | Type::Reference {
                    value: Value::Record(id),
                    ..
                } => Some(id),
                _ => None,
            };
            let (key, module) = self.index.template_record(record)?;
            let declaration = &self.index.sources().ast(module)?.records[key.index];
            let (position, member) = declaration
                .fields
                .iter()
                .enumerate()
                .find(|(_, candidate)| {
                    self.index.sources().text(candidate.name).ok() == Some(field)
                })
                .ok_or_else(|| error("unknown record field", fields))?;
            if !matches!(
                self.index.query(self.work).field_access(
                    self.module,
                    FieldId {
                        record,
                        index: position
                    },
                    fields
                )?,
                Access::Allowed
            ) {
                return Err(error("field access uses a private field", fields));
            }
            ty = match application {
                Some(id) => Type::Value(self.nominals.records[id].fields[position]),
                None => Type::concrete(
                    self.index
                        .query(self.work)
                        .parameter_type(module, member.ty)?,
                ),
            };
        }
        Ok(ty)
    }
    fn place(&self, path: Span) -> Result<(usize, Type), Box<Diagnostic>> {
        let token = self
            .program
            .tokens
            .binary_search_by_key(&path.start, |token| token.span.start)
            .map_err(|_| bad(path))?;
        let root = self.program.tokens[token];
        if root.kind != TokenKind::Ident || root.span.end > path.end {
            return Err(bad(path));
        }
        let position = self.local(root.span)?;
        let ty = self.fields(
            self.bindings[position].ty,
            Span {
                start: root.span.end,
                ..path
            },
        )?;
        Ok((position, ty))
    }
    fn element(&self, base: Span) -> Result<Element, Box<Diagnostic>> {
        let (position, ty) = self.place(base)?;
        if self.exclusive_loan(position, base)? {
            return Err(error(
                "array is exclusively borrowed during this call",
                base,
            ));
        }
        match ty {
            Type::Slice { element, .. } => Ok(element),
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::FixedArray(array))))
            | Type::Reference {
                value: Value::Concrete(ValueTy::Owned(AggregateTy::FixedArray(array))),
                ..
            } => Ok(Element::Concrete(array.element())),
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)))
            | Type::Reference {
                value: Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)),
                ..
            } => Ok(Element::Concrete(Ty::U8)),
            Type::Value(Value::Vector(element))
            | Type::Reference {
                value: Value::Vector(element),
                ..
            } => {
                if element.owned() {
                    return Err(error(
                        "owned vector elements must be moved with pop_last",
                        base,
                    ));
                }
                Ok(element)
            }
            _ => Err(error(
                "indexing requires an array, slice, byte buffer or vector",
                base,
            )),
        }
    }
    fn argument(&mut self, argument: &Argument) -> Result<Type, Box<Diagnostic>> {
        let Argument::Borrow {
            mutable,
            place,
            span,
        } = argument
        else {
            let Argument::Value(value) = argument else {
                unreachable!()
            };
            let ty = self.expression(*value)?;
            ty.value(self.program.expressions[value.0].span)?;
            return Ok(ty);
        };
        let kind = if *mutable {
            BorrowKind::Exclusive
        } else {
            BorrowKind::Shared
        };
        let (name, forwarded, range) = match place {
            ast::BorrowPlace::OwnerName(name) => (*name, false, None),
            ast::BorrowPlace::ForwardedParameter { name, .. } => (*name, true, None),
            ast::BorrowPlace::Range {
                name,
                start,
                end,
                star,
            } => (*name, star.is_some(), Some((*start, *end))),
        };
        let (position, ty) = self.place(name)?;
        let binding = self.bindings[position];
        if kind == BorrowKind::Exclusive
            && ((!forwarded && !binding.mutable)
                || matches!(
                    binding.ty,
                    Type::Reference {
                        kind: BorrowKind::Shared,
                        ..
                    } | Type::Slice {
                        kind: BorrowKind::Shared,
                        ..
                    }
                ))
        {
            return Err(error("exclusive borrow requires mutable access", *span));
        }
        if let Some((start, end)) = range {
            // Forwarding belongs to the root binding, including a projected
            // vector field. Check unused definitions before this early return;
            // keep the existing non-range loan/diagnostic order below.
            if forwarded != matches!(binding.ty, Type::Reference { .. } | Type::Slice { .. }) {
                return Err(error(
                    "borrow requires an owned value or forwarded reference",
                    *span,
                ));
            }
            let element = self.element(name)?;
            let start = self.expression(start)?;
            self.expect(start, Type::scalar(Ty::I32), *span)?;
            let end = self.expression(end)?;
            self.expect(end, Type::scalar(Ty::I32), *span)?;
            self.loan(name, kind, *span)?;
            return Ok(Type::Slice { element, kind });
        }
        self.loan(name, kind, *span)?;
        if forwarded != matches!(binding.ty, Type::Reference { .. } | Type::Slice { .. }) {
            return Err(error(
                "borrow requires an owned value or forwarded reference",
                *span,
            ));
        }
        match ty {
            Type::Value(value) => Ok(Type::Reference { value, kind }),
            Type::Reference { value, .. } if forwarded => Ok(Type::Reference { value, kind }),
            Type::Slice { element, .. } if forwarded => Ok(Type::Slice { element, kind }),
            _ => Err(error(
                "borrow requires an owned value or forwarded reference",
                *span,
            )),
        }
    }
    fn call(
        &mut self,
        expression: ExprId,
        definition: DefId,
        arguments: &[Argument],
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        let before = self.loans.len();
        let result = self.call_inner(expression, definition, arguments, at);
        self.loans.truncate(before);
        result
    }
    fn borrow_conflict(
        &self,
        binding: usize,
        write: bool,
        at: Span,
    ) -> Result<bool, Box<Diagnostic>> {
        if self.abstract_body && self.reachable {
            for loan in &self.loans {
                self.work
                    .debit(1, at, "generic active reference comparison")?;
                if loan.binding == binding
                    && (write || loan.kind == BorrowKind::Exclusive)
                    && self.paths_overlap(loan.path, at)?
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn exclusive_loan(&self, binding: usize, path: Span) -> Result<bool, Box<Diagnostic>> {
        self.borrow_conflict(binding, false, path)
    }
    fn path_fields(&self, path: Span) -> Result<std::slice::Iter<'_, Token>, Box<Diagnostic>> {
        // Charge a machine-word upper bound before the binary search. The
        // remaining scan visits only this path, including its trivia.
        self.work
            .debit(u64::from(usize::BITS), path, "generic loan path lookup")?;
        let start = self
            .program
            .tokens
            .partition_point(|token| token.span.start < path.start);
        let root = self.program.tokens.get(start).ok_or_else(|| bad(path))?;
        if root.kind != TokenKind::Ident
            || root.span.file != path.file
            || root.span.start != path.start
            || root.span.end > path.end
        {
            return Err(bad(path));
        }
        Ok(self.program.tokens[start + 1..].iter())
    }
    fn next_path_field(
        &self,
        tokens: &mut std::slice::Iter<'_, Token>,
        path: Span,
    ) -> Result<Option<Span>, Box<Diagnostic>> {
        for token in tokens {
            self.work.debit(1, path, "generic loan path token")?;
            if token.span.start >= path.end {
                break;
            }
            if token.span.file != path.file || token.span.end > path.end {
                return Err(bad(path));
            }
            if token.kind == TokenKind::Ident {
                return Ok(Some(token.span));
            }
        }
        Ok(None)
    }
    fn paths_overlap(&self, left: Span, right: Span) -> Result<bool, Box<Diagnostic>> {
        if left.file != right.file {
            return Err(bad(right));
        }
        let mut left_tokens = self.path_fields(left)?;
        let mut right_tokens = self.path_fields(right)?;
        loop {
            let left_field = self.next_path_field(&mut left_tokens, left)?;
            let right_field = self.next_path_field(&mut right_tokens, right)?;
            let (Some(left_field), Some(right_field)) = (left_field, right_field) else {
                // Equal paths and either prefix share storage. Only a distinct
                // field at the same depth proves that two places are disjoint.
                return Ok(true);
            };
            if compare_bytes(
                self.index.sources().text(left_field)?,
                self.index.sources().text(right_field)?,
                self.work,
                right,
            )? != Ordering::Equal
            {
                return Ok(false);
            }
        }
    }
    fn loan(&mut self, name: Span, kind: BorrowKind, at: Span) -> Result<(), Box<Diagnostic>> {
        if !self.abstract_body || !self.reachable {
            return Ok(());
        }
        let binding = self.place(name)?.0;
        for loan in &self.loans {
            self.work
                .debit(1, at, "generic reference acquisition comparison")?;
            if loan.binding == binding
                && (loan.kind == BorrowKind::Exclusive || kind == BorrowKind::Exclusive)
                && self.paths_overlap(loan.path, name)?
            {
                return Err(error("conflicting borrows of the same value", at));
            }
        }
        self.storage.push(
            &mut self.loans,
            Loan {
                binding,
                kind,
                path: name,
            },
            self.allocator,
            self.work,
            at,
        )
    }
    fn call_inner(
        &mut self,
        expression: ExprId,
        definition: DefId,
        arguments: &[Argument],
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        if self.index.owned_vector_builder().is_some() {
            if let Some(operation) = self.index.owned_template(definition, self.work, at)? {
                return self.owned_vector_call(expression, definition, operation, arguments, at);
            }
        }
        if self.index.builtin_set().contains_vector(vector::Item::Vec) {
            if let Some(operation) = self.index.vector_template(definition, self.work, at)? {
                return self.vector_call(expression, definition, operation, arguments, at);
            }
        }
        let explicit = self.program.generic_arguments(at);
        if definition.0 >= self.index.template_function_count() {
            if explicit.is_some() {
                return Err(error("this function does not have type parameters", at));
            }
            let (parameters, result) = self.builtin_signature(definition, at)?;
            if parameters.len() != arguments.len() {
                return Err(error("function argument count mismatch", at));
            }
            for (argument, expected) in arguments.iter().zip(parameters) {
                let actual = self.argument(argument)?;
                self.expect(actual, Type::concrete(expected), at)?;
            }
            if self.plan.is_some() {
                self.storage.push(
                    &mut self.calls,
                    Call {
                        expression,
                        template: definition,
                        target: definition,
                        vector: None,
                    },
                    self.allocator,
                    self.work,
                    at,
                )?;
            }
            return Ok(Type::concrete(ParameterTy::Value(result)));
        }
        let (key, module) = self.index.template_function(definition)?;
        let program = self.index.sources().ast(module)?;
        let function = &program.functions[key.index];
        let type_parameters = program.generic_parameters(function.name);
        if explicit.is_some_and(|arguments| arguments.len() != type_parameters.len()) {
            return Err(error("function type argument count mismatch", at));
        }
        if arguments.len() != function.params.len() {
            return Err(error("function argument count mismatch", at));
        }
        let mut inferred = [None; crate::frontend::parser::MAX_PARAMS];
        if let Some(arguments) = explicit {
            for (position, syntax) in arguments.iter().enumerate() {
                inferred[position] = Some(self.ty(*syntax)?.value(syntax.span)?);
            }
        }
        let mut formal_arguments =
            [Value::Concrete(ValueTy::Scalar(Ty::Unit)); crate::frontend::parser::MAX_PARAMS];
        for (position, argument) in formal_arguments[..type_parameters.len()]
            .iter_mut()
            .enumerate()
        {
            *argument = Value::Parameter {
                position,
                copy: !type_parameters[position].bounds.is_empty(),
            };
        }
        for (argument, parameter) in arguments.iter().zip(&function.params) {
            let actual = self.argument(argument)?;
            let formal = self.nominals.syntax(
                self.index,
                self.work,
                self.storage,
                self.allocator,
                module,
                function.name,
                &formal_arguments[..type_parameters.len()],
                parameter.ty,
                0,
            )?;
            self.infer(formal, actual, &mut inferred, at)?;
        }
        let mut concrete = Vec::new();
        for (position, parameter) in type_parameters.iter().enumerate() {
            let value = inferred[position]
                .ok_or_else(|| error("cannot infer type parameter from arguments", at))?;
            if !parameter.bounds.is_empty() && !self.copy(value) {
                return Err(error("type argument does not satisfy Copy", at));
            }
            formal_arguments[position] = value;
            if self.plan.is_some() {
                if !self.nominals.concrete(value) {
                    return Err(bad(at));
                }
                self.storage
                    .push(&mut concrete, value, self.allocator, self.work, at)?;
            }
        }
        let result = self.nominals.syntax(
            self.index,
            self.work,
            self.storage,
            self.allocator,
            module,
            function.name,
            &formal_arguments[..type_parameters.len()],
            function.result,
            0,
        )?;
        if let Some(plan) = self.plan.as_deref_mut() {
            let target = if type_parameters.is_empty() {
                plan.non_generic(definition).ok_or_else(|| bad(at))?
            } else {
                let depth = self
                    .instance
                    .and_then(|id| plan.instances.get(id.0))
                    .map_or(1, |instance| instance.depth + 1);
                plan.instantiate(
                    definition,
                    concrete,
                    depth,
                    self.storage,
                    self.allocator,
                    self.work,
                    at,
                )?
            };
            self.storage.push(
                &mut self.calls,
                Call {
                    expression,
                    template: definition,
                    target,
                    vector: None,
                },
                self.allocator,
                self.work,
                at,
            )?;
        }
        Ok(result)
    }
    fn vector_call(
        &mut self,
        expression: ExprId,
        template: DefId,
        operation: vector::Operation,
        arguments: &[Argument],
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        self.storage.enter_vector(at)?;
        let result = self.vector_call_inner(expression, template, operation, arguments, at);
        self.storage.leave_vector(at)?;
        result
    }
    fn vector_call_inner(
        &mut self,
        expression: ExprId,
        template: DefId,
        operation: vector::Operation,
        arguments: &[Argument],
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        self.work.debit(1, at, "vector template call")?;
        if arguments.len() != operation.parameter_count() {
            return Err(error("function argument count mismatch", at));
        }
        let explicit = self.program.generic_arguments(at);
        if explicit.is_some_and(|types| types.len() != 1) {
            return Err(error(
                "vector function requires one element type argument",
                at,
            ));
        }
        let mut element = if let Some(types) = explicit {
            let value = self.ty(types[0])?.value(types[0].span)?;
            Some(
                self.index
                    .candidate_element(value, self.nominals, types[0].span)?,
            )
        } else {
            None
        };
        let mut actuals = [None; 3];
        if operation != vector::Operation::New {
            let actual = self.argument(&arguments[0])?;
            let Type::Reference { value, .. } = actual else {
                return Err(error("vector function requires a vector reference", at));
            };
            let inferred = value
                .vector_element()
                .ok_or_else(|| error("vector function requires a vector reference", at))?;
            if element.is_some_and(|explicit| explicit != inferred) {
                return Err(error(
                    "vector element type argument does not match its vector",
                    at,
                ));
            }
            element = Some(inferred);
            actuals[0] = Some(actual);
        }
        let element =
            element.ok_or_else(|| error("cannot infer new element; specify new::<T>", at))?;
        if element.owned() {
            let operation = match operation {
                vector::Operation::New => owned_vector::Operation::New,
                vector::Operation::Len => owned_vector::Operation::Len,
                vector::Operation::Capacity => owned_vector::Operation::Capacity,
                vector::Operation::TryReserve => owned_vector::Operation::TryReserve,
                vector::Operation::Truncate => owned_vector::Operation::Truncate,
                vector::Operation::Take => owned_vector::Operation::Take,
                _ => return Err(error("owned record vectors require push_reserved and pop_last; copying operations remain unavailable", at)),
            };
            return self.owned_vector_selected(
                expression, template, operation, arguments, element, actuals, at,
            );
        }
        let grow = if operation.requires_grow_status() {
            let id = self
                .index
                .experimental_enum_id(experimental::Enumeration::Grow)?;
            self.index.template_enum_for_instance(id)?
        } else {
            // The selected operation never reads a status identity.
            EnumId(0)
        };
        let (parameters, result) = vector_signature(operation, element, grow);
        for (position, argument) in arguments.iter().enumerate() {
            let actual = match actuals[position] {
                Some(actual) => actual,
                None => self.argument(argument)?,
            };
            self.expect(actual, parameters[position], at)?;
        }
        if let Some(plan) = self.plan.as_deref_mut() {
            let value = element.value();
            let key = match value {
                Value::Concrete(ValueTy::Scalar(ty)) => vector::Function::new(operation, ty),
                Value::Record(_) | Value::Concrete(ValueTy::Owned(AggregateTy::Record(_))) => {
                    vector::Function::record(
                        operation,
                        self.nominals
                            .vector_record_id(value, self.storage, self.work, at)?,
                    )
                }
                _ => None,
            }
            .ok_or_else(|| bad(at))?;
            plan.register_vector(key, self.storage, self.allocator, self.work, at)?;
            self.storage.push(
                &mut self.calls,
                Call {
                    expression,
                    template,
                    target: DefId(usize::MAX),
                    vector: Some(key),
                },
                self.allocator,
                self.work,
                at,
            )?;
        }
        Ok(result)
    }
    fn infer(
        &self,
        formal: Type,
        actual: Type,
        inferred: &mut [Option<Value>],
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        match (formal, actual) {
            (Type::Value(formal), Type::Value(actual)) => {
                self.infer_value(formal, actual, inferred, at, 0)
            }
            (
                Type::Reference {
                    value: formal,
                    kind: expected,
                },
                Type::Reference {
                    value: actual,
                    kind,
                },
            ) if kind == BorrowKind::Exclusive || expected == BorrowKind::Shared => {
                self.infer_value(formal, actual, inferred, at, 0)
            }
            (
                Type::Slice {
                    element: formal,
                    kind: expected,
                },
                Type::Slice {
                    element: actual,
                    kind,
                },
            ) if kind == BorrowKind::Exclusive || expected == BorrowKind::Shared => {
                self.infer_value(formal.value(), actual.value(), inferred, at, 0)
            }
            (
                Type::Slice {
                    element: formal,
                    kind: expected,
                },
                Type::Reference {
                    value: Value::Vector(actual),
                    kind,
                },
            ) if kind == BorrowKind::Exclusive || expected == BorrowKind::Shared => {
                self.infer_value(formal.value(), actual.value(), inferred, at, 0)
            }
            (
                Type::Slice {
                    element: formal,
                    kind: expected,
                },
                Type::Reference {
                    value: Value::Concrete(ValueTy::Owned(AggregateTy::FixedArray(array))),
                    kind,
                },
            ) if kind == BorrowKind::Exclusive || expected == BorrowKind::Shared => self
                .infer_value(
                    formal.value(),
                    Value::Concrete(ValueTy::Scalar(array.element())),
                    inferred,
                    at,
                    0,
                ),
            (
                Type::Slice {
                    element: formal,
                    kind: expected,
                },
                Type::Reference {
                    value: Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)),
                    kind,
                },
            ) if kind == BorrowKind::Exclusive || expected == BorrowKind::Shared => self
                .infer_value(
                    formal.value(),
                    Value::Concrete(ValueTy::Scalar(Ty::U8)),
                    inferred,
                    at,
                    0,
                ),
            _ => self.expect(actual, formal, at),
        }
    }
    fn infer_value(
        &self,
        formal: Value,
        actual: Value,
        inferred: &mut [Option<Value>],
        at: Span,
        depth: usize,
    ) -> Result<(), Box<Diagnostic>> {
        if depth > MAX_DEPTH {
            return Err(exhausted(at));
        }
        self.work.debit(1, at, "nominal type inference")?;
        match (formal, actual) {
            (Value::Parameter { position, .. }, value) => {
                if inferred[position].is_some_and(|previous| previous != value) {
                    return Err(error("type parameter inferred as incompatible types", at));
                }
                inferred[position] = Some(value);
                Ok(())
            }
            (Value::Vector(formal), Value::Vector(actual)) => {
                self.infer_value(formal.value(), actual.value(), inferred, at, depth + 1)
            }
            (Value::Record(formal), Value::Record(actual)) => {
                let formal = &self.nominals.records[formal];
                let actual = &self.nominals.records[actual];
                if formal.template != actual.template
                    || formal.arguments.len() != actual.arguments.len()
                {
                    return Err(error("generic nominal type mismatch", at));
                }
                for (formal, actual) in formal.arguments.iter().zip(&actual.arguments) {
                    self.infer_value(*formal, *actual, inferred, at, depth + 1)?;
                }
                Ok(())
            }
            (Value::Enumeration(formal), Value::Enumeration(actual)) => {
                let formal = &self.nominals.enumerations[formal];
                let actual = &self.nominals.enumerations[actual];
                if formal.template != actual.template
                    || formal.arguments.len() != actual.arguments.len()
                {
                    return Err(error("generic nominal type mismatch", at));
                }
                for (formal, actual) in formal.arguments.iter().zip(&actual.arguments) {
                    self.infer_value(*formal, *actual, inferred, at, depth + 1)?;
                }
                Ok(())
            }
            _ if formal == actual => Ok(()),
            _ => Err(error("generic argument or value type mismatch", at)),
        }
    }
    fn builtin_signature(
        &mut self,
        definition: DefId,
        at: Span,
    ) -> Result<(Vec<ParameterTy>, ValueTy), Box<Diagnostic>> {
        let rank = definition
            .0
            .checked_sub(self.index.template_function_count())
            .ok_or_else(|| bad(at))?;
        let mut result = None;
        for builtin in BuiltinFunction::ALL {
            if !self.index.builtin_set().contains_function(builtin) {
                continue;
            }
            let id = self.index.builtin_function_id(builtin)?;
            if id.0 - self.index.source_function_count() == rank {
                let enumeration = self.index.builtin_enum_id(match builtin {
                    BuiltinFunction::ReadStdin => BuiltinEnum::ReadStatus,
                    BuiltinFunction::WriteStdout => BuiltinEnum::WriteStatus,
                })?;
                let enumeration = self.index.template_enum_for_instance(enumeration)?;
                let (parameter, ty) = builtin.signature(enumeration);
                let mut parameters = Vec::new();
                self.storage
                    .push(&mut parameters, parameter, self.allocator, self.work, at)?;
                result = Some((parameters, ty));
            }
        }
        for builtin in experimental::Function::ALL {
            if !self
                .index
                .builtin_set()
                .experimental()
                .contains(builtin.item())
            {
                continue;
            }
            let id = self.index.experimental_function_id(builtin)?;
            if id.0 - self.index.source_function_count() == rank {
                let signature = self.index.experimental_signature(builtin)?;
                let mut parameters = Vec::new();
                for parameter in signature.parameters() {
                    self.storage.push(
                        &mut parameters,
                        *parameter,
                        self.allocator,
                        self.work,
                        at,
                    )?;
                }
                let result_ty = match signature.result {
                    ValueTy::Owned(AggregateTy::Enum(id)) => ValueTy::Owned(AggregateTy::Enum(
                        self.index.template_enum_for_instance(id)?,
                    )),
                    _ => signature.result,
                };
                result = Some((parameters, result_ty));
            }
        }
        result.ok_or_else(|| bad(at))
    }
}
