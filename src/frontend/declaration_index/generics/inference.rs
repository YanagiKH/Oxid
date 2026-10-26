//! Constructor-local unification, using the ordinary generic type relation.
use super::*;
const PARAMETERS: usize = crate::frontend::parser::MAX_PARAMS;

// Complete new factory/return/caller transports, in addition to existing index
// and Walker controls. This is an admitted role envelope, not an RSS estimate.
#[allow(dead_code)]
struct ConstructorFrame {
    storage: Storage,
    inferred: [Option<Value>; PARAMETERS],
    parameters: &'static [ast::GenericParam],
    fields: &'static [ast::FieldInit],
    programs: [&'static ast::Program; 2],
    names: [&'static str; 6],
    values: [Value; 6],
    types: [Type; 3],
    vectors: [Vec<Value>; 4],
    vector_returns: [Result<Vec<Value>, Box<Diagnostic>>; 3],
    expressions: [ExprId; 3],
    positions: [usize; 8],
    origins: [Span; 3],
    queries: [QuerySession<'static, 'static>; 2],
    returns: [Result<Option<Type>, Box<Diagnostic>>; 3],
}

const FRAME_BYTES: usize = size_of::<ConstructorFrame>();

impl Storage {
    fn enter_constructor(&mut self, at: Span) -> Result<(), Box<Diagnostic>> {
        if self
            .live_after(FRAME_BYTES)
            .is_none_or(|bytes| bytes > MAX_STORAGE)
        {
            return Err(exhausted(at));
        }
        self.constructor_scratch = self
            .constructor_scratch
            .checked_add(FRAME_BYTES)
            .ok_or_else(|| exhausted(at))?;
        Ok(())
    }
    fn leave_constructor(&mut self, at: Span) -> Result<(), Box<Diagnostic>> {
        self.constructor_scratch = self
            .constructor_scratch
            .checked_sub(FRAME_BYTES)
            .ok_or_else(|| bad(at))?;
        Ok(())
    }
}

impl Walker<'_, '_> {
    fn with_constructor_frame<T>(
        &mut self,
        at: Span,
        action: impl FnOnce(&mut Self) -> Result<T, Box<Diagnostic>>,
    ) -> Result<T, Box<Diagnostic>> {
        self.storage.enter_constructor(at)?;
        let result = action(self);
        self.storage.leave_constructor(at)?;
        result
    }
    fn constructor_formals(
        &mut self,
        parameters: &[ast::GenericParam],
        at: Span,
    ) -> Result<Vec<Value>, Box<Diagnostic>> {
        if parameters.len() > PARAMETERS {
            return Err(exhausted(at));
        }
        let mut arguments = Vec::new();
        for (position, parameter) in parameters.iter().enumerate() {
            self.storage.push(
                &mut arguments,
                Value::Parameter {
                    position,
                    copy: !parameter.bounds.is_empty(),
                },
                self.allocator,
                self.work,
                parameter.name,
            )?;
        }
        Ok(arguments)
    }

    fn constructor_arguments(
        &mut self,
        parameters: &[ast::GenericParam],
        inferred: &[Option<Value>; PARAMETERS],
        at: Span,
    ) -> Result<Vec<Value>, Box<Diagnostic>> {
        let mut arguments = Vec::new();
        for (position, parameter) in parameters.iter().enumerate() {
            let value = inferred[position].ok_or_else(|| {
                error(
                    "cannot infer type parameter from constructor; specify type arguments",
                    at,
                )
            })?;
            if !parameter.bounds.is_empty() && !self.copy(value) {
                return Err(error("type argument does not satisfy Copy", at));
            }
            self.storage.push(
                &mut arguments,
                value,
                self.allocator,
                self.work,
                parameter.name,
            )?;
        }
        Ok(arguments)
    }

    fn remember_inferred_constructor(
        &mut self,
        expression: ExprId,
        value: Value,
        at: Span,
    ) -> Result<Type, Box<Diagnostic>> {
        self.work.debit(
            self.constructors.len() as u64 + 1,
            at,
            "inferred constructor identity lookup",
        )?;
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

    pub(super) fn infer_record_constructor(
        &mut self,
        expression: ExprId,
        record: RecordId,
        fields: &[ast::FieldInit],
        at: Span,
    ) -> Result<Option<Type>, Box<Diagnostic>> {
        let (key, module) = self.index.template_record(record)?;
        let program = self.index.sources().ast(module)?;
        let declaration = &program.records[key.index];
        let parameters = program.generic_parameters(declaration.name);
        if parameters.is_empty() {
            return Ok(None);
        }
        self.work.debit(1, at, "record constructor inference")?;
        if !matches!(
            self.index
                .query(self.work)
                .construction_access(self.module, record, at)?,
            Access::Allowed
        ) {
            return Err(error("record construction uses a private field", at));
        }
        if declaration.fields.len() != fields.len() {
            return Err(error("record initializer field count mismatch", at));
        }
        self.with_constructor_frame(at, |this| {
            let formals = this.constructor_formals(parameters, at)?;
            let formal = this.nominals.record(
                this.index,
                this.work,
                this.storage,
                this.allocator,
                record,
                formals,
                at,
                0,
            )?;
            let Value::Record(application) = formal else {
                return Err(bad(at));
            };
            let mut inferred = [None; PARAMETERS];
            for (ordinal, field) in fields.iter().enumerate() {
                let name = this.index.sources().text(field.name)?;
                for earlier in &fields[..ordinal] {
                    let earlier_name = this.index.sources().text(earlier.name)?;
                    this.work.debit(
                        (1 + earlier_name.len().min(name.len())) as u64,
                        field.name,
                        "constructor field name",
                    )?;
                    if earlier_name == name {
                        return Err(owned_diagnostic::secondary(
                            error("duplicate record initializer field", field.name),
                            earlier.name,
                            format_args!("first initialized here"),
                        ));
                    }
                }
                let mut selected = None;
                for (position, candidate) in declaration.fields.iter().enumerate() {
                    let candidate_name = this.index.sources().text(candidate.name)?;
                    this.work.debit(
                        (1 + candidate_name.len().min(name.len())) as u64,
                        field.name,
                        "constructor field name",
                    )?;
                    if candidate_name == name {
                        selected = Some(position);
                        break;
                    }
                }
                let position = selected.ok_or_else(|| error("unknown record field", field.name))?;
                // Copy one small formal out of the arena before typing the child,
                // which may grow the same nominal-application tables.
                let expected = Type::Value(this.nominals.records[application].fields[position]);
                let actual = this.expression(field.value)?;
                this.infer(expected, actual, &mut inferred, field.span)?;
            }
            let arguments = this.constructor_arguments(parameters, &inferred, at)?;
            let value = this.nominals.record(
                this.index,
                this.work,
                this.storage,
                this.allocator,
                record,
                arguments,
                at,
                0,
            )?;
            this.remember_inferred_constructor(expression, value, at)
                .map(Some)
        })
    }

    pub(super) fn infer_enum_constructor(
        &mut self,
        expression: ExprId,
        variant: VariantId,
        arguments: Option<&[Argument]>,
        at: Span,
    ) -> Result<Option<Type>, Box<Diagnostic>> {
        if variant.enumeration.0 >= self.index.template_enum_count() {
            return Ok(None);
        }
        let (key, module) = self.index.template_enumeration(variant.enumeration)?;
        let program = self.index.sources().ast(module)?;
        let declaration = &program.enums[key.index];
        let parameters = program.generic_parameters(declaration.name);
        if parameters.is_empty() {
            return Ok(None);
        }
        self.work.debit(1, at, "enum constructor inference")?;
        let declaration_variant = declaration
            .variants
            .get(variant.index)
            .ok_or_else(|| bad(at))?;
        if declaration_variant.payload.is_none() {
            if arguments.is_some() {
                return Err(error("enum variant payload mismatch", at));
            }
            return Err(error(
                "cannot infer type parameter from constructor; specify type arguments",
                at,
            ));
        }
        let Some([Argument::Value(child)]) = arguments else {
            return Err(error("enum variant payload mismatch", at));
        };
        self.with_constructor_frame(at, |this| {
            let formals = this.constructor_formals(parameters, at)?;
            let formal = this.nominals.enumeration(
                this.index,
                this.work,
                this.storage,
                this.allocator,
                variant.enumeration,
                formals,
                at,
                0,
            )?;
            let expected = this
                .nominals
                .variant(this.index, formal, variant.index, at)?
                .ok_or_else(|| error("enum variant payload mismatch", at))?;
            let actual = this.expression(*child)?;
            let mut inferred = [None; PARAMETERS];
            this.infer(Type::Value(expected), actual, &mut inferred, at)?;
            let arguments = this.constructor_arguments(parameters, &inferred, at)?;
            let value = this.nominals.enumeration(
                this.index,
                this.work,
                this.storage,
                this.allocator,
                variant.enumeration,
                arguments,
                at,
                0,
            )?;
            this.remember_inferred_constructor(expression, value, at)
                .map(Some)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn origin() -> Span {
        Span {
            file: crate::frontend::source::SourceFileId(0),
            start: 0,
            end: 1,
        }
    }

    #[test]
    fn constructor_live_frames_and_metadata_share_the_exact_storage_ceiling() {
        let at = origin();
        let mut storage = Storage {
            bytes: MAX_STORAGE - FRAME_BYTES,
            ..Default::default()
        };
        storage.enter_constructor(at).unwrap();
        let mut allocator = Allocator::default();
        let mut values = Vec::new();
        let error = storage
            .push(&mut values, 7u8, &mut allocator, &WorkMeter::default(), at)
            .unwrap_err();
        assert_eq!(error.code, "E0400");
        assert_eq!(allocator.attempts, 0);
        assert!(values.is_empty());
        storage.leave_constructor(at).unwrap();
        storage
            .push(&mut values, 7u8, &mut allocator, &WorkMeter::default(), at)
            .unwrap();
        assert_eq!(values, [7]);
        assert_eq!(storage.constructor_scratch, 0);
        assert!(storage.live_after(0).unwrap() <= MAX_STORAGE);
        storage.bytes = MAX_STORAGE;
        assert_eq!(storage.enter_constructor(at).unwrap_err().code, "E0400");
        assert_eq!(storage.constructor_scratch, 0);
    }

    #[test]
    fn constructor_nested_frame_refusal_preserves_the_active_frame_balance() {
        let at = origin();
        let mut storage = Storage::default();
        let maximum = MAX_STORAGE / FRAME_BYTES;
        for _ in 0..maximum {
            storage.enter_constructor(at).unwrap();
        }
        let before = storage.constructor_scratch;
        assert_eq!(storage.enter_constructor(at).unwrap_err().code, "E0400");
        assert_eq!(storage.constructor_scratch, before);
        for _ in 0..maximum {
            storage.leave_constructor(at).unwrap();
        }
        assert_eq!(storage.constructor_scratch, 0);
        assert_eq!(storage.bytes, 0);
    }

    #[test]
    fn constructor_frames_unwind_on_type_work_and_allocation_errors() {
        use crate::frontend::{
            lexer, parser,
            source::{SourceMap, SourceView},
        };
        for source in [
            "struct Pair<T>{first:T,second:T}fn main()->i32{let pair=Pair{first:20,second:22};return pair.first+pair.second;}",
            "enum Option<T>{None,Some(T)}struct Cell<T>{value:T}fn main()->i32{let cell=Cell{value:Option::Some(42)};return 42;}",
        ] {
            let mut map=SourceMap::new();
            let file=map.add("inference-frame.ox".into(), source.into());
            let file=map.get(file);
            let tokens=lexer::lex_current_with_allocator(file,lexer::MAX_TOKENS,&mut Allocator::default()).unwrap();
            let (program, _)=parser::parse_typed_counted(file,tokens,parser::SourceMode::ProjectCandidate,parser::MAX_NODES,&mut Allocator::default(),&mut Default::default()).unwrap();
            let owner=SourceOwner::original(file,&program,SourceView::Map(&map)).unwrap();
            let work=WorkMeter::default();
            let mut allocator=Allocator::default();
            let index=collect_originals(owner,IndexLimits::default(),&work,&mut allocator).unwrap().finish(&work,&mut allocator).unwrap();
            let definition=DefId(0);
            let run=|fail_at:Option<usize>,limit:u64,force_type_error:bool| {
                let mut storage=Storage::default();
                let mut allocator=Allocator{fail_at,..Default::default()};
                let work=WorkMeter::new(limit);
                let mut nominals=Nominals::default();
                let result={
                    let mut walker=Walker::new(&index,&work,&mut allocator,&mut storage,&mut nominals,None,definition,Vec::new(),None).unwrap();
                    if force_type_error {
                        walker.with_constructor_frame(origin(), |this| {
                            // A repeated T must reject conflicting constraints,
                            // while releasing the active inference frame.
                            let mut inferred=[None;PARAMETERS];
                            this.infer(Type::Value(Value::Parameter{position:0,copy:false}),Type::scalar(Ty::I32),&mut inferred,origin())?;
                            this.infer(Type::Value(Value::Parameter{position:0,copy:false}),Type::scalar(Ty::Bool),&mut inferred,origin())
                        })
                    } else {walker.check().map(|_|())}
                };
                assert_eq!(storage.constructor_scratch,0);
                (result,allocator.attempts,work.used())
            };
            assert_eq!(run(None,u64::MAX,true).0.unwrap_err().code,"E0300");
            let (result,attempts,used)=run(None,u64::MAX,false);result.unwrap();
            for failure in 1..=attempts {
                let (result,observed,_)=run(Some(failure),u64::MAX,false);
                assert_eq!(result.unwrap_err().code,"E0400","allocation {failure}");
                assert_eq!(observed,failure);
            }
            assert_eq!(run(None,used-1,false).0.unwrap_err().code,"E0400");
        }
    }
}
