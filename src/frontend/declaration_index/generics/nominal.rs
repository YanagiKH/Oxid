//! Source-owned nominal applications. Opaque applications participate in
//! definition checking; only fully concrete layouts receive executable IDs.
use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct RecordApplication {
    pub(super) template: RecordId,
    pub(super) arguments: Vec<Value>,
    concrete: bool,
    depth: usize,
    pub(super) fields: Vec<Value>,
    pub(super) complete: bool,
    pub(super) is_copy: bool,
    pub(super) emitted: Option<RecordId>,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct EnumApplication {
    pub(super) template: EnumId,
    pub(super) arguments: Vec<Value>,
    concrete: bool,
    depth: usize,
    pub(super) variants: Vec<Option<Value>>,
    complete: bool,
    pub(super) emitted: Option<EnumId>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Nominals {
    pub(super) records: Vec<RecordApplication>,
    pub(super) enumerations: Vec<EnumApplication>,
    emitted_enums: Vec<usize>,
    pub(super) template_enum_count: usize,
    pub(super) emitted_records: Vec<usize>,
}

impl Nominals {
    pub(super) fn concrete(&self, value: Value) -> bool {
        match value {
            Value::Concrete(_) => true,
            Value::Vector(element) => self.concrete(element.value()),
            Value::Parameter { .. } => false,
            Value::Enumeration(id) => self.enumerations[id].concrete,
            Value::Record(id) => self.records[id].concrete,
        }
    }
    fn depth(&self, value: Value) -> usize {
        match value {
            Value::Record(id) => self.records[id].depth,
            Value::Enumeration(id) => self.enumerations[id].depth,
            _ => 0,
        }
    }
    fn application_shape(
        &self,
        arguments: &[Value],
        work: &WorkMeter,
        at: Span,
    ) -> Result<(bool, usize), Box<Diagnostic>> {
        work.debit(arguments.len() as u64 + 1, at, "nominal application shape")?;
        let concrete = arguments.iter().all(|argument| self.concrete(*argument));
        let depth = arguments
            .iter()
            .map(|argument| self.depth(*argument))
            .max()
            .unwrap_or(0)
            + 1;
        if depth > MAX_DEPTH {
            return Err(exhausted(at));
        }
        Ok((concrete, depth))
    }

    pub(super) fn copy(&self, value: Value) -> bool {
        match value {
            Value::Concrete(ValueTy::Scalar(_)) | Value::Parameter { copy: true, .. } => true,
            Value::Concrete(ValueTy::Owned(AggregateTy::Record(id))) => self
                .emitted_records
                .get(id.0)
                .and_then(|slot| self.records.get(*slot))
                .is_some_and(|record| record.complete && record.is_copy),
            Value::Record(id) => self
                .records
                .get(id)
                .is_some_and(|record| record.complete && record.is_copy),
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn syntax(
        &mut self,
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        storage: &mut Storage,
        allocator: &mut Allocator,
        module: ModuleId,
        declaration: Span,
        arguments: &[Value],
        syntax: ast::TypeSyntax,
        depth: usize,
    ) -> Result<Type, Box<Diagnostic>> {
        if depth > MAX_DEPTH {
            return Err(exhausted(syntax.span));
        }
        work.debit(1, syntax.span, "nominal type substitution")?;
        let application = match syntax.kind {
            TypeSyntaxKind::Applied(id)
            | TypeSyntaxKind::AppliedReference {
                application: id, ..
            } => Some(id),
            _ => None,
        };
        if let Some(id) = application {
            let program = index.sources().ast(module)?;
            let application = program
                .generic_application(id)
                .ok_or_else(|| bad(syntax.span))?;
            reject_parameter_base(
                index,
                work,
                module,
                declaration,
                application.path,
                syntax.span,
            )?;
            let nominal = index.query(work).nominal_type(
                module,
                ItemPathRef {
                    file: syntax.span.file,
                    path: application.path,
                },
                TypeContext::Value,
            )?;
            if nominal == NominalId::Vector {
                if application.arguments.len() != 1 {
                    return Err(error(
                        "Vec requires exactly one element type argument",
                        syntax.span,
                    ));
                }
                storage.enter_vector(syntax.span)?;
                let result = (|| {
                    let argument = application.arguments[0];
                    let value = self
                        .syntax(
                            index,
                            work,
                            storage,
                            allocator,
                            module,
                            declaration,
                            arguments,
                            argument,
                            depth + 1,
                        )?
                        .value(argument.span)?;
                    let value = Value::Vector(Element::checked(value, self, argument.span)?);
                    Ok(match syntax.kind {
                        TypeSyntaxKind::AppliedReference { mutable, .. } => Type::Reference {
                            value,
                            kind: if mutable {
                                BorrowKind::Exclusive
                            } else {
                                BorrowKind::Shared
                            },
                        },
                        _ => Type::Value(value),
                    })
                })();
                storage.leave_vector(syntax.span)?;
                return result;
            }
            let mut values = Vec::new();
            for argument in &application.arguments {
                let value = self
                    .syntax(
                        index,
                        work,
                        storage,
                        allocator,
                        module,
                        declaration,
                        arguments,
                        *argument,
                        depth + 1,
                    )?
                    .value(argument.span)?;
                storage.push(&mut values, value, allocator, work, argument.span)?;
            }
            let value = match nominal {
                NominalId::Record(record) => self.record(
                    index,
                    work,
                    storage,
                    allocator,
                    record,
                    values,
                    syntax.span,
                    depth + 1,
                )?,
                NominalId::Enum(enumeration) if enumeration.0 < self.template_enum_count => self
                    .enumeration(
                        index,
                        work,
                        storage,
                        allocator,
                        enumeration,
                        values,
                        syntax.span,
                        depth + 1,
                    )?,
                _ => {
                    return Err(error(
                        "this type does not accept type arguments",
                        syntax.span,
                    ))
                }
            };
            return Ok(match syntax.kind {
                TypeSyntaxKind::AppliedReference { mutable, .. } => Type::Reference {
                    value,
                    kind: if mutable {
                        BorrowKind::Exclusive
                    } else {
                        BorrowKind::Shared
                    },
                },
                _ => Type::Value(value),
            });
        }
        if let TypeSyntaxKind::GenericSliceReference { mutable, element } = syntax.kind {
            storage.enter_vector(syntax.span)?;
            let result = (|| {
                let span = index.sources().path_span(ItemPathRef {
                    file: syntax.span.file,
                    path: element,
                })?;
                let value = self
                    .syntax(
                        index,
                        work,
                        storage,
                        allocator,
                        module,
                        declaration,
                        arguments,
                        ast::TypeSyntax {
                            kind: TypeSyntaxKind::Name(element),
                            span,
                        },
                        depth + 1,
                    )?
                    .value(span)?;
                Ok(Type::Slice {
                    element: Element::checked(value, self, span)?,
                    kind: if mutable {
                        BorrowKind::Exclusive
                    } else {
                        BorrowKind::Shared
                    },
                })
            })();
            storage.leave_vector(syntax.span)?;
            return result;
        }
        let vector_frame = matches!(syntax.kind, TypeSyntaxKind::GenericSliceReference { .. });
        if vector_frame {
            storage.enter_vector(syntax.span)?;
        }
        let result = syntax_type(index, work, module, declaration, arguments, syntax);
        if vector_frame {
            storage.leave_vector(syntax.span)?;
        }
        let ty = result?;
        Ok(match ty {
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Record(record)))) => {
                Type::Value(self.record(
                    index,
                    work,
                    storage,
                    allocator,
                    record,
                    Vec::new(),
                    syntax.span,
                    depth + 1,
                )?)
            }
            Type::Value(Value::Concrete(ValueTy::Owned(AggregateTy::Enum(enumeration))))
                if enumeration.0 < self.template_enum_count =>
            {
                Type::Value(self.enumeration(
                    index,
                    work,
                    storage,
                    allocator,
                    enumeration,
                    Vec::new(),
                    syntax.span,
                    depth + 1,
                )?)
            }
            Type::Reference {
                value: Value::Concrete(ValueTy::Owned(AggregateTy::Record(record))),
                kind,
            } => Type::Reference {
                value: self.record(
                    index,
                    work,
                    storage,
                    allocator,
                    record,
                    Vec::new(),
                    syntax.span,
                    depth + 1,
                )?,
                kind,
            },
            _ => ty,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &mut self,
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        storage: &mut Storage,
        allocator: &mut Allocator,
        template: RecordId,
        arguments: Vec<Value>,
        at: Span,
        depth: usize,
    ) -> Result<Value, Box<Diagnostic>> {
        if depth > MAX_DEPTH {
            return Err(exhausted(at));
        }
        let (key, module) = index.template_record(template)?;
        let program = index.sources().ast(module)?;
        let declaration = &program.records[key.index];
        let parameters = program.generic_parameters(declaration.name);
        if parameters.len() != arguments.len() {
            return Err(error("record type argument count mismatch", at));
        }
        for (parameter, argument) in parameters.iter().zip(&arguments) {
            if !parameter.bounds.is_empty() && !self.copy(*argument) {
                return Err(error("type argument does not satisfy Copy", at));
            }
        }
        for (id, record) in self.records.iter().enumerate() {
            work.debit(
                (1 + arguments.len()) as u64,
                at,
                "nominal application lookup",
            )?;
            if record.template == template {
                if !record.complete {
                    return Err(error("recursive record layout has no finite size", at));
                }
                if record.arguments == arguments {
                    return Ok(Value::Record(id));
                }
            }
        }
        if !arguments.is_empty()
            && self
                .records
                .iter()
                .filter(|record| !record.arguments.is_empty())
                .count()
                >= MAX_INSTANCES
        {
            return Err(exhausted(at));
        }
        let (concrete, argument_depth) = self.application_shape(&arguments, work, at)?;
        let id = self.records.len();
        storage.push(
            &mut self.records,
            RecordApplication {
                template,
                arguments,
                concrete,
                depth: argument_depth,
                fields: Vec::new(),
                complete: false,
                is_copy: false,
                emitted: None,
            },
            allocator,
            work,
            at,
        )?;
        let mut fields = Vec::new();
        // A bounded stack copy avoids holding an arena borrow across recursion.
        let mut environment =
            [Value::Concrete(ValueTy::Scalar(Ty::Unit)); crate::frontend::parser::MAX_PARAMS];
        let len = self.records[id].arguments.len();
        environment[..len].copy_from_slice(&self.records[id].arguments);
        for field in &declaration.fields {
            let value = self
                .syntax(
                    index,
                    work,
                    storage,
                    allocator,
                    module,
                    declaration.name,
                    &environment[..len],
                    field.ty,
                    depth + 1,
                )?
                .value(field.ty.span)?;
            if declaration.derive_copy.is_some() && !self.copy(value) {
                return Err(error(
                    "Copy record fields require scalars or checked Copy records",
                    field.ty.span,
                ));
            }
            storage.push(&mut fields, value, allocator, work, field.span)?;
        }
        self.records[id].fields = fields;
        self.records[id].complete = true;
        self.records[id].is_copy = declaration.derive_copy.is_some();
        Ok(Value::Record(id))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn enumeration(
        &mut self,
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        storage: &mut Storage,
        allocator: &mut Allocator,
        template: EnumId,
        arguments: Vec<Value>,
        at: Span,
        depth: usize,
    ) -> Result<Value, Box<Diagnostic>> {
        if depth > MAX_DEPTH {
            return Err(exhausted(at));
        }
        let (key, module) = index.template_enumeration(template)?;
        let program = index.sources().ast(module)?;
        let declaration = &program.enums[key.index];
        let parameters = program.generic_parameters(declaration.name);
        if parameters.len() != arguments.len() {
            return Err(error("enum type argument count mismatch", at));
        }
        for (parameter, argument) in parameters.iter().zip(&arguments) {
            if !parameter.bounds.is_empty() && !self.copy(*argument) {
                return Err(error("type argument does not satisfy Copy", at));
            }
        }
        for (id, enumeration) in self.enumerations.iter().enumerate() {
            work.debit((1 + arguments.len()) as u64, at, "enum application lookup")?;
            if enumeration.template == template {
                if !enumeration.complete {
                    return Err(error("recursive enum layout has no finite size", at));
                }
                if enumeration.arguments == arguments {
                    return Ok(Value::Enumeration(id));
                }
            }
        }
        if !arguments.is_empty()
            && self
                .enumerations
                .iter()
                .filter(|enumeration| !enumeration.arguments.is_empty())
                .count()
                >= MAX_INSTANCES
        {
            return Err(exhausted(at));
        }
        let (concrete, argument_depth) = self.application_shape(&arguments, work, at)?;
        let id = self.enumerations.len();
        storage.push(
            &mut self.enumerations,
            EnumApplication {
                template,
                arguments,
                concrete,
                depth: argument_depth,
                variants: Vec::new(),
                complete: false,
                emitted: None,
            },
            allocator,
            work,
            at,
        )?;
        let mut variants = Vec::new();
        let mut environment =
            [Value::Concrete(ValueTy::Scalar(Ty::Unit)); crate::frontend::parser::MAX_PARAMS];
        let len = self.enumerations[id].arguments.len();
        environment[..len].copy_from_slice(&self.enumerations[id].arguments);
        for variant in &declaration.variants {
            let value = match variant.payload {
                Some(payload) => Some(
                    self.syntax(
                        index,
                        work,
                        storage,
                        allocator,
                        module,
                        declaration.name,
                        &environment[..len],
                        payload.value_type(),
                        depth + 1,
                    )?
                    .value(payload.span)?,
                ),
                None => None,
            };
            storage.push(&mut variants, value, allocator, work, variant.span)?;
        }
        self.enumerations[id].variants = variants;
        self.enumerations[id].complete = true;
        Ok(Value::Enumeration(id))
    }

    pub(super) fn exposure(
        &self,
        index: &DeclarationIndex<'_>,
        work: &WorkMeter,
        owner: ExposureOwner,
        value: Value,
        at: Span,
        depth: usize,
    ) -> Result<(), Box<Diagnostic>> {
        if depth > MAX_DEPTH {
            return Err(exhausted(at));
        }
        work.debit(1, at, "nominal argument visibility")?;
        let (nominal, arguments): (NominalId, &[Value]) = match value {
            Value::Record(id) => (
                NominalId::Record(self.records[id].template),
                &self.records[id].arguments,
            ),
            Value::Enumeration(id) => (
                NominalId::Enum(self.enumerations[id].template),
                &self.enumerations[id].arguments,
            ),
            Value::Concrete(ValueTy::Owned(AggregateTy::Record(id))) => {
                (NominalId::Record(id), &[])
            }
            Value::Concrete(ValueTy::Owned(AggregateTy::Enum(id))) => (NominalId::Enum(id), &[]),
            Value::Concrete(ValueTy::Owned(AggregateTy::ByteBuf)) => (NominalId::ByteBuf, &[]),
            Value::Concrete(ValueTy::Owned(AggregateTy::File)) => (NominalId::File, &[]),
            Value::Vector(_) | Value::Concrete(ValueTy::Owned(AggregateTy::Vector(_))) => {
                (NominalId::Vector, &[])
            }
            _ => return Ok(()),
        };
        let exposure =
            match owner {
                ExposureOwner::Function(function) => index
                    .query(work)
                    .nominal_signature_exposure(function, nominal, at)?,
                ExposureOwner::Enumeration(enumeration) => index
                    .query(work)
                    .enum_payload_exposure(enumeration, nominal, at)?,
                ExposureOwner::Field(field) => index
                    .query(work)
                    .record_field_exposure(field, nominal, at)?,
            };
        if matches!(exposure, NominalExposure::Denied { .. }) {
            if matches!(owner, ExposureOwner::Field(_)) {
                return Err(crate::frontend::owned_diagnostic::diagnostic(
                    "E0207",
                    "resolve",
                    format_args!("record field exposes a private type"),
                    Some(at),
                ));
            }
            return Err(error(
                "public declaration exposes a private type argument",
                at,
            ));
        }
        for argument in arguments {
            self.exposure(index, work, owner, *argument, at, depth + 1)?;
        }
        Ok(())
    }

    pub(super) fn enum_template(&self, value: Value, at: Span) -> Result<EnumId, Box<Diagnostic>> {
        match value {
            Value::Enumeration(id) => Ok(self.enumerations[id].template),
            Value::Concrete(ValueTy::Owned(AggregateTy::Enum(id)))
                if id.0 >= self.template_enum_count =>
            {
                Ok(id)
            }
            _ => Err(error("match requires a known enum type", at)),
        }
    }
    pub(super) fn variant_count(
        &self,
        index: &DeclarationIndex<'_>,
        value: Value,
        at: Span,
    ) -> Result<usize, Box<Diagnostic>> {
        match value {
            Value::Enumeration(id) => Ok(self.enumerations[id].variants.len()),
            _ => Ok(index
                .template_builtin_enum_item(self.enum_template(value, at)?)?
                .variant_count()),
        }
    }
    pub(super) fn variant(
        &self,
        index: &DeclarationIndex<'_>,
        value: Value,
        ordinal: usize,
        at: Span,
    ) -> Result<Option<Value>, Box<Diagnostic>> {
        if let Value::Enumeration(id) = value {
            return self.enumerations[id]
                .variants
                .get(ordinal)
                .copied()
                .ok_or_else(|| bad(at));
        }
        let item = index.template_builtin_enum_item(self.enum_template(value, at)?)?;
        Ok(match item {
            BuiltinItem::Experimental(item) => item
                .enumeration()
                .and_then(|kind| kind.value_payload(ordinal))
                .map(Value::Concrete),
            _ => item
                .member_payload(ordinal)
                .map(|ty| Value::Concrete(ValueTy::Scalar(ty))),
        })
    }
    pub(super) fn enum_count(&self) -> usize {
        self.emitted_enums.len()
    }
    pub(super) fn enum_definition(&self, id: EnumId, at: Span) -> Result<EnumId, Box<Diagnostic>> {
        if id.0 >= self.emitted_enums.len() {
            return Ok(EnumId(
                self.template_enum_count + id.0 - self.emitted_enums.len(),
            ));
        }
        self.emitted_enums
            .get(id.0)
            .and_then(|id| self.enumerations.get(*id))
            .map(|enumeration| enumeration.template)
            .ok_or_else(|| bad(at))
    }
    pub(super) fn payload(
        &self,
        id: VariantId,
        at: Span,
    ) -> Result<Option<ValueTy>, Box<Diagnostic>> {
        let application = self
            .emitted_enums
            .get(id.enumeration.0)
            .and_then(|id| self.enumerations.get(*id))
            .ok_or_else(|| bad(at))?;
        application
            .variants
            .get(id.index)
            .ok_or_else(|| bad(at))?
            .map(|value| self.value(value, at))
            .transpose()
    }

    pub(super) fn finish(
        &mut self,
        storage: &mut Storage,
        allocator: &mut Allocator,
        work: &WorkMeter,
        at: Span,
    ) -> Result<(), Box<Diagnostic>> {
        for id in 0..self.records.len() {
            if self.concrete(Value::Record(id)) {
                let emitted = RecordId(self.emitted_records.len());
                storage.push(&mut self.emitted_records, id, allocator, work, at)?;
                self.records[id].emitted = Some(emitted);
            }
        }
        for id in 0..self.enumerations.len() {
            if self.concrete(Value::Enumeration(id)) {
                let emitted = EnumId(self.emitted_enums.len());
                storage.push(&mut self.emitted_enums, id, allocator, work, at)?;
                self.enumerations[id].emitted = Some(emitted);
            }
        }
        Ok(())
    }

    pub(super) fn value(&self, value: Value, at: Span) -> Result<ValueTy, Box<Diagnostic>> {
        match value {
            Value::Concrete(ValueTy::Owned(AggregateTy::Enum(id)))
                if id.0 >= self.template_enum_count =>
            {
                Ok(ValueTy::Owned(AggregateTy::Enum(EnumId(
                    self.emitted_enums.len() + id.0 - self.template_enum_count,
                ))))
            }
            Value::Concrete(value) => Ok(value),
            Value::Enumeration(id) => self
                .enumerations
                .get(id)
                .and_then(|enumeration| enumeration.emitted)
                .map(|id| ValueTy::Owned(AggregateTy::Enum(id)))
                .ok_or_else(|| bad(at)),
            Value::Record(id) => self
                .records
                .get(id)
                .and_then(|record| record.emitted)
                .map(|id| ValueTy::Owned(AggregateTy::Record(id)))
                .ok_or_else(|| bad(at)),
            Value::Parameter { .. } => Err(bad(at)),
            Value::Vector(element) => match self.value(element.value(), at)? {
                ValueTy::Scalar(ty) => Ok(ValueTy::Owned(AggregateTy::Vector(ty))),
                ValueTy::Owned(AggregateTy::Record(id)) if self.record_is_copy(id, at)? => {
                    Ok(ValueTy::Owned(AggregateTy::RecordVector(id)))
                }
                _ => Err(bad(at)),
            },
        }
    }

    pub(super) fn parameter(&self, ty: Type, at: Span) -> Result<ParameterTy, Box<Diagnostic>> {
        let ty = match ty {
            Type::Value(value) => Type::Value(Value::Concrete(self.value(value, at)?)),
            Type::Reference { value, kind } => Type::Reference {
                value: Value::Concrete(self.value(value, at)?),
                kind,
            },
            Type::Slice { element, kind } => Type::Slice {
                element: match self.value(element.value(), at)? {
                    ValueTy::Scalar(ty) => Element::Concrete(ty),
                    ValueTy::Owned(AggregateTy::Record(id)) if self.record_is_copy(id, at)? => {
                        Element::ConcreteRecord(id)
                    }
                    _ => return Err(bad(at)),
                },
                kind,
            },
        };
        ty.parameter(at)
    }

    pub(super) fn record_is_copy(
        &self,
        record: RecordId,
        at: Span,
    ) -> Result<bool, Box<Diagnostic>> {
        self.emitted_records
            .get(record.0)
            .and_then(|id| self.records.get(*id))
            .map(|record| record.complete && record.is_copy)
            .ok_or_else(|| bad(at))
    }
    pub(super) fn record_count(&self) -> usize {
        self.emitted_records.len()
    }
    pub(super) fn template(&self, record: RecordId, at: Span) -> Result<RecordId, Box<Diagnostic>> {
        self.emitted_records
            .get(record.0)
            .and_then(|id| self.records.get(*id))
            .map(|record| record.template)
            .ok_or_else(|| bad(at))
    }
    pub(super) fn field(
        &self,
        record: RecordId,
        field: usize,
        at: Span,
    ) -> Result<ValueTy, Box<Diagnostic>> {
        let id = self.emitted_records.get(record.0).ok_or_else(|| bad(at))?;
        self.value(
            *self.records[*id].fields.get(field).ok_or_else(|| bad(at))?,
            at,
        )
    }
}
