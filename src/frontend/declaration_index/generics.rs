//! Source-only generic checking and bounded concrete function instantiation.
//! Template identities always refer to the original frozen declaration index.
mod flow;
use super::*;
use crate::frontend::ast::{self, Argument, ExprId, ExprKind, StmtKind, TypeSyntaxKind};
use crate::frontend::lexer::Kind as TokenKind;
use crate::frontend::oir::owned_types::FixedArrayTy;

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

#[derive(Default)]
struct Storage {
    bytes: usize,
}
impl Storage {
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
                .bytes
                .checked_add(size)
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
            if self.bytes > MAX_STORAGE {
                return Err(exhausted(at));
            }
        }
        values.push(value);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Concrete(ValueTy),
    Parameter(usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Type {
    Value(Value),
    Reference { value: Value, kind: BorrowKind },
    Slice { element: Ty, kind: BorrowKind },
}
impl Type {
    fn scalar(ty: Ty) -> Self {
        Self::Value(Value::Concrete(ValueTy::Scalar(ty)))
    }
    fn concrete(ty: ParameterTy) -> Self {
        match ty {
            ParameterTy::Value(ty) => Self::Value(Value::Concrete(ty)),
            ParameterTy::Reference {
                referent: BorrowedTy::Exact(ty),
                kind,
            } => Self::Reference {
                value: Value::Concrete(ValueTy::Owned(ty)),
                kind,
            },
            ParameterTy::Reference {
                referent: BorrowedTy::ScalarSlice(element),
                kind,
            } => Self::Slice { element, kind },
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
                referent: BorrowedTy::ScalarSlice(element),
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

#[derive(Debug, PartialEq, Eq)]
struct Call {
    expression: ExprId,
    template: DefId,
    target: DefId,
}
#[derive(Debug, PartialEq, Eq)]
struct Instance {
    definition: DefId,
    arguments: Vec<ValueTy>,
    calls: Vec<Call>,
    depth: usize,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Plan {
    instances: Vec<Instance>,
    ordinary: Vec<Option<DefId>>,
    template_count: usize,
}
impl Plan {
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
        };
        let mut plan = Self {
            instances: Vec::new(),
            ordinary: Vec::new(),
            template_count: index.template_function_count(),
        };
        for module in 0..index.sources().count() {
            let program = index.sources().ast(ModuleId(module))?;
            for declaration in program
                .records
                .iter()
                .map(|item| item.name)
                .chain(program.enums.iter().map(|item| item.name))
            {
                if !program.generic_parameters(declaration).is_empty() {
                    return Err(error(
                        "generic struct and enum declarations are not available yet",
                        declaration,
                    ));
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
                        Value::Parameter(position),
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
                    Value::Concrete(*argument),
                    allocator,
                    work,
                    index.sources().eof(),
                )?;
            }
            let calls = Walker::new(
                index,
                work,
                allocator,
                &mut storage,
                Some(&mut plan),
                definition,
                arguments,
                Some(DefId(next)),
            )?
            .check()?;
            plan.instances[next].calls = calls;
            next += 1;
        }
        let count = plan.instances.len();
        for instance in &mut plan.instances {
            for call in &mut instance.calls {
                if call.template.0 >= plan.template_count {
                    call.target = DefId(count + call.template.0 - plan.template_count);
                }
            }
        }
        Ok(plan)
    }
    // Keep the caller's shared storage budget, failure seam and work meter explicit.
    #[allow(clippy::too_many_arguments)]
    fn instantiate(
        &mut self,
        definition: DefId,
        arguments: Vec<ValueTy>,
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
    function: &ast::Function,
    arguments: &[Value],
    syntax: ast::TypeSyntax,
) -> Result<Type, Box<Diagnostic>> {
    let parameters = index
        .sources()
        .ast(module)?
        .generic_parameters(function.name);
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
            TypeSyntaxKind::Name(path) | TypeSyntaxKind::Reference { referent: path, .. } => path,
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
        let (key, module) = self.template_function(instance.definition)?;
        let declaration = &self.sources().ast(module)?.functions[key.index];
        let mut arguments =
            [Value::Concrete(ValueTy::Scalar(Ty::Unit)); crate::frontend::parser::MAX_PARAMS];
        for (target, source) in arguments.iter_mut().zip(&instance.arguments) {
            *target = Value::Concrete(*source);
        }
        syntax_type(
            self,
            work,
            module,
            declaration,
            &arguments[..instance.arguments.len()],
            syntax,
        )?
        .parameter(syntax.span)
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
}
struct Walker<'a, 's> {
    index: &'a DeclarationIndex<'s>,
    work: &'a WorkMeter,
    allocator: &'a mut Allocator,
    storage: &'a mut Storage,
    plan: Option<&'a mut Plan>,
    instance: Option<DefId>,
    definition: DefId,
    program: &'s ast::Program,
    function: &'s ast::Function,
    module: ModuleId,
    arguments: Vec<Value>,
    bindings: Vec<Binding>,
    calls: Vec<Call>,
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
            plan,
            instance,
            definition,
            program,
            function: &program.functions[key.index],
            module,
            arguments,
            bindings: Vec::new(),
            calls: Vec::new(),
            loans: Vec::new(),
            reachable: true,
            loop_frames: Vec::new(),
        })
    }
    fn check(mut self) -> Result<Vec<Call>, Box<Diagnostic>> {
        for parameter in &self.function.params {
            let ty = self.ty(parameter.ty)?;
            self.exposure(ty, parameter.ty.span)?;
            self.bind(parameter.name, ty, false)?;
        }
        self.exposure(self.ty(self.function.result)?, self.function.result.span)?;
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
        Ok(self.calls)
    }
    fn ty(&self, syntax: ast::TypeSyntax) -> Result<Type, Box<Diagnostic>> {
        syntax_type(
            self.index,
            self.work,
            self.module,
            self.function,
            &self.arguments,
            syntax,
        )
    }
    fn exposure(&self, ty: Type, at: Span) -> Result<(), Box<Diagnostic>> {
        if !self.abstract_body {
            return Ok(());
        }
        let nominal = match ty {
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Record(record))))
            | Type::Reference {
                value: Value::Concrete(ValueTy::Owned(AggregateTy::Record(record))),
                ..
            } => NominalId::Record(record),
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Enum(enumeration)))) => {
                NominalId::Enum(enumeration)
            }
            _ => return Ok(()),
        };
        match self.index.query(self.work).nominal_signature_exposure(
            self.definition,
            nominal,
            at,
        )? {
            NominalExposure::Allowed => Ok(()),
            NominalExposure::Denied { .. } => Err(error(
                "generic function signature exposes a private type",
                at,
            )),
        }
    }
    fn mutable_place(&self, position: usize, at: Span) -> Result<(), Box<Diagnostic>> {
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
        match value {
            Value::Concrete(ValueTy::Scalar(_)) => true,
            Value::Concrete(ValueTy::Owned(_)) => false,
            Value::Parameter(position) => self
                .program
                .generic_parameters(self.function.name)
                .get(position)
                .is_some_and(|parameter| !parameter.bounds.is_empty()),
        }
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
        Err(error("unknown local in generic function", name))
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
                array.element() == element
                    && (kind == BorrowKind::Exclusive || expected == BorrowKind::Shared)
            }
            (
                Type::Reference {
                    value: Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)),
                    kind,
                },
                Type::Slice {
                    element: Ty::U8,
                    kind: expected,
                },
            ) => kind == BorrowKind::Exclusive || expected == BorrowKind::Shared,
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
        let inherited_reachable = self.reachable;
        let mut flow = Flow::FALL;
        let mut terminal_state = None;
        for (position, statement) in block.body.iter().enumerate() {
            self.reachable = inherited_reachable && flow.falls;
            self.work.debit(1, statement.span, "generic statement")?;
            let next = match &statement.kind {
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
                    ty.value(statement.span)?;
                    if let Some(annotation) = annotation {
                        self.expect(ty, self.ty(*annotation)?, annotation.span)?;
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
                    self.expect(ty, self.ty(self.function.result)?, statement.span)?;
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
                    self.check_loop(*condition, *body, loops, statement.span)?
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
                    self.mutable_place(self.local(*base)?, *base)?;
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
                    let Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Enum(
                        enumeration,
                    )))) = self.bindings[position].ty
                    else {
                        return Err(error("match requires a known enum type", *scrutinee));
                    };
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
                        let payload = self
                            .index
                            .enum_view(enumeration)?
                            .variant(variant)?
                            .value_payload(self.work)?;
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
                            (Some(name), Some(ty)) => {
                                self.bind(name, Type::concrete(ParameterTy::Value(ty)), false)?
                            }
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
                    if seen[..self.index.enum_view(enumeration)?.variant_count()]
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
        self.reachable = inherited_reachable;
        Ok(flow)
    }
    fn expression(&mut self, id: ExprId) -> Result<Type, Box<Diagnostic>> {
        let expression = &self.program.expressions[id.0];
        self.work.debit(1, expression.span, "generic expression")?;
        Ok(match &expression.kind {
            ExprKind::Name(name) => {
                let position = self.local(*name)?;
                let ty = self.bindings[position].ty;
                if let Type::Value(value) = ty {
                    if self.abstract_body && self.reachable {
                        if self.loans.iter().any(|loan| {
                            loan.binding == position
                                && (loan.kind == BorrowKind::Exclusive || !self.copy(value))
                        }) {
                            return Err(error("value is borrowed during this call", *name));
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
                        let payload = self
                            .index
                            .enum_view(variant.enumeration)?
                            .variant(variant)?
                            .value_payload(self.work)?;
                        match (payload, args.as_deref()) {
                            (None, None) => (),
                            (Some(expected), Some([Argument::Value(value)])) => {
                                let actual = self.expression(*value)?;
                                self.expect(
                                    actual,
                                    Type::concrete(ParameterTy::Value(expected)),
                                    expression.span,
                                )?;
                            }
                            _ => {
                                return Err(error("enum variant payload mismatch", expression.span))
                            }
                        }
                        Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Enum(
                            variant.enumeration,
                        ))))
                    }
                }
            }
            ExprKind::StructLiteral { record, fields } => {
                let ty = self.index.query(self.work).value_type(
                    self.module,
                    ast::TypeSyntax {
                        span: self
                            .program
                            .item_path_span(*record)
                            .ok_or_else(|| bad(expression.span))?,
                        kind: TypeSyntaxKind::Name(*record),
                    },
                    TypeContext::Value,
                )?;
                let ValueTy::Owned(AggregateTy::Record(record)) = ty else {
                    return Err(error(
                        "record literal requires a record type",
                        expression.span,
                    ));
                };
                let (key, module) = self.index.record(record)?;
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
                    let expected = declaration
                        .fields
                        .iter()
                        .find(|candidate| {
                            self.index.sources().text(candidate.name).ok() == Some(name)
                        })
                        .ok_or_else(|| error("unknown record field", field.name))?;
                    let expected = self
                        .index
                        .query(self.work)
                        .parameter_type(module, expected.ty)?;
                    let actual = self.expression(field.value)?;
                    self.expect(actual, Type::concrete(expected), field.span)?;
                }
                Type::Value(Value::Concrete(ty))
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
                Type::scalar(element)
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
        if self.abstract_body && matches!(ty, Type::Value(value) if !self.copy(value)) {
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
                _ => return Err(error("field access requires a known record type", fields)),
            };
            let (key, module) = self.index.record(record)?;
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
            ty = Type::concrete(
                self.index
                    .query(self.work)
                    .parameter_type(module, member.ty)?,
            );
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
    fn element(&self, base: Span) -> Result<Ty, Box<Diagnostic>> {
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
            } => Ok(array.element()),
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)))
            | Type::Reference {
                value: Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)),
                ..
            } => Ok(Ty::U8),
            _ => Err(error("indexing requires an array or byte buffer", base)),
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
    fn exclusive_loan(&self, binding: usize, _path: Span) -> Result<bool, Box<Diagnostic>> {
        Ok(self.abstract_body
            && self.reachable
            && self
                .loans
                .iter()
                .any(|loan| loan.binding == binding && loan.kind == BorrowKind::Exclusive))
    }
    fn loan(&mut self, name: Span, kind: BorrowKind, at: Span) -> Result<(), Box<Diagnostic>> {
        if !self.abstract_body || !self.reachable {
            return Ok(());
        }
        let binding = self.place(name)?.0;
        for loan in &self.loans {
            if loan.binding == binding
                && (loan.kind == BorrowKind::Exclusive || kind == BorrowKind::Exclusive)
            {
                return Err(error("conflicting borrows in generic function call", at));
            }
        }
        self.storage.push(
            &mut self.loans,
            Loan { binding, kind },
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
        if definition.0 >= self.index.template_function_count() {
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
        if arguments.len() != function.params.len() {
            return Err(error("function argument count mismatch", at));
        }
        let mut inferred = [None; crate::frontend::parser::MAX_PARAMS];
        let mut formal_arguments =
            [Value::Concrete(ValueTy::Scalar(Ty::Unit)); crate::frontend::parser::MAX_PARAMS];
        for (position, argument) in formal_arguments[..type_parameters.len()]
            .iter_mut()
            .enumerate()
        {
            *argument = Value::Parameter(position);
        }
        for (argument, parameter) in arguments.iter().zip(&function.params) {
            let actual = self.argument(argument)?;
            let formal = syntax_type(
                self.index,
                self.work,
                module,
                function,
                &formal_arguments[..type_parameters.len()],
                parameter.ty,
            )?;
            match (formal, actual) {
                (Type::Value(Value::Parameter(position)), Type::Value(value))
                | (
                    Type::Reference {
                        value: Value::Parameter(position),
                        ..
                    },
                    Type::Reference { value, .. },
                ) => {
                    if inferred[position].is_some_and(|previous| previous != value) {
                        return Err(error("type parameter inferred as incompatible types", at));
                    }
                    inferred[position] = Some(value);
                    let expected = match formal {
                        Type::Value(_) => Type::Value(value),
                        Type::Reference { kind, .. } => Type::Reference { value, kind },
                        _ => unreachable!(),
                    };
                    self.expect(actual, expected, at)?;
                }
                _ => self.expect(actual, formal, at)?,
            }
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
                let Value::Concrete(value) = value else {
                    return Err(bad(at));
                };
                self.storage
                    .push(&mut concrete, value, self.allocator, self.work, at)?;
            }
        }
        let result = syntax_type(
            self.index,
            self.work,
            module,
            function,
            &formal_arguments[..type_parameters.len()],
            function.result,
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
                },
                self.allocator,
                self.work,
                at,
            )?;
        }
        Ok(result)
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
                result = Some((parameters, signature.result));
            }
        }
        result.ok_or_else(|| bad(at))
    }
}
