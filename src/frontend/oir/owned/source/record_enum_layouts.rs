//! A separate bounded temporary inventory for mixed record/enum layouts.
use super::hir::*;
use crate::frontend::{
    declaration_index::{DeclarationIndex, WorkMeter},
    diagnostic::Diagnostic,
    oir::owned_types::{
        admit_declaration_counts, admit_enum_counts, admit_joint_value_layouts, DeclarationError,
        DeclarationUsage, EnumId, RawEnumDecl, RawFieldDecl, RawRecordDecl, RawVariantDecl,
    },
    owned_diagnostic,
    project::budget::Allocator,
    source::Span,
};
use std::mem::size_of;

const MAX_TEMPORARY_BYTES: usize = 8 * 1024 * 1024;
pub(super) fn needed(records: &[Record]) -> bool {
    records.iter().any(|record| {
        record
            .fields
            .iter()
            .any(|field| matches!(field.ty, ValueTy::Owned(AggregateTy::Enum(_))))
    })
}
fn resource(at: Span, message: &'static str) -> Box<Diagnostic> {
    owned_diagnostic::diagnostic("E0400", "resolve", format_args!("{message}"), Some(at))
}
fn declaration_error(error: DeclarationError, records: &[Record], at: Span) -> Box<Diagnostic> {
    let (code, message, at) = match error {
        DeclarationError::ContainmentCycle(id) => (
            "E0202",
            "record and enum containment must be acyclic",
            records.get(id.0).map_or(at, |record| record.name_span),
        ),
        DeclarationError::ResourceLimit(message) => ("E0400", message, at),
        DeclarationError::Allocation => ("E0400", "record and enum layout allocation failed", at),
        DeclarationError::LayoutOverflow => ("E0400", "record and enum layout size overflow", at),
        _ => ("E0500", "invalid resolved record or enum layout", at),
    };
    owned_diagnostic::diagnostic(code, "resolve", format_args!("{message}"), Some(at))
}
fn reserve<T>(
    allocator: &mut Allocator,
    count: usize,
    at: Span,
) -> Result<Vec<T>, Box<Diagnostic>> {
    let mut values = Vec::new();
    allocator
        .vector_exact(&mut values, count, "temporary source layout inventory")
        .map_err(|_| resource(at, "record and enum layout allocation failed"))?;
    Ok(values)
}

/// The caller owns this pass's allocator. It is separate from paid HIR row
/// reservations; its entire temporary payload is admitted before any request.
pub(super) fn inventory(
    index: &DeclarationIndex<'_>,
    records: &[Record],
    work: &WorkMeter,
    allocator: &mut Allocator,
) -> Result<(DeclarationUsage, usize), Box<Diagnostic>> {
    inventory_with_limit(index, records, work, allocator, MAX_TEMPORARY_BYTES)
}

fn inventory_with_limit(
    index: &DeclarationIndex<'_>,
    records: &[Record],
    work: &WorkMeter,
    allocator: &mut Allocator,
    limit: usize,
) -> Result<(DeclarationUsage, usize), Box<Diagnostic>> {
    let at = index.sources().eof();
    let usage = admit_declaration_counts(records.iter().map(|record| record.fields.len()))
        .map_err(|error| declaration_error(error, records, at))?;
    let enums = admit_enum_counts(index.enum_variant_counts(), usage)
        .map_err(|error| declaration_error(error.into(), records, at))?;
    // Joint analysis already includes 16 KiB of traversal/return controls.
    // Add another complete 16 KiB for this inventory's source-query, vector,
    // allocator and diagnostic transports while those joint controls are live.
    let mut bytes = 32 * 1024usize;
    let limit = limit.min(MAX_TEMPORARY_BYTES);
    for (count, width) in [
        (usage.records, size_of::<RawRecordDecl>()),
        (usage.fields, size_of::<RawFieldDecl>()),
        (enums.enums, size_of::<RawEnumDecl>()),
        (enums.variants, size_of::<RawVariantDecl>()),
        (usage.records + enums.enums, 64),
    ] {
        bytes = count
            .checked_mul(width)
            .and_then(|value| bytes.checked_add(value))
            .filter(|value| *value <= limit)
            .ok_or_else(|| {
                resource(
                    at,
                    "record and enum layout temporary storage limit exceeded",
                )
            })?;
    }
    let mut raw_records = reserve(allocator, usage.records, at)?;
    for record in records {
        work.debit(
            1 + record.fields.len() as u64,
            record.name_span,
            "record layout inventory",
        )?;
        let mut fields = reserve(allocator, record.fields.len(), record.name_span)?;
        for field in &record.fields {
            // Opaque generic declarations check visibility recursively before
            // specialization. A private local application does not broaden the
            // public template's visibility domain.
            if !index.has_generic_instances() {
                let nominal = match field.ty {
                    ValueTy::Owned(AggregateTy::Record(id)) => {
                        Some(crate::frontend::declaration_index::NominalId::Record(id))
                    }
                    ValueTy::Owned(AggregateTy::Enum(id)) => {
                        Some(crate::frontend::declaration_index::NominalId::Enum(id))
                    }
                    ValueTy::Owned(AggregateTy::ByteBuf) => {
                        Some(crate::frontend::declaration_index::NominalId::ByteBuf)
                    }
                    ValueTy::Owned(AggregateTy::File) => {
                        Some(crate::frontend::declaration_index::NominalId::File)
                    }
                    ValueTy::Owned(AggregateTy::Vector(_)) => {
                        Some(crate::frontend::declaration_index::NominalId::Vector)
                    }
                    _ => None,
                };
                if let Some(nominal) = nominal {
                    if let crate::frontend::declaration_index::NominalExposure::Denied {
                        declaration,
                        restrictor,
                    } = index.query(work).record_field_exposure(
                        field.id,
                        nominal,
                        field.name_span,
                    )? {
                        let mut error = owned_diagnostic::secondary(
                            owned_diagnostic::diagnostic(
                                "E0207",
                                "resolve",
                                format_args!("record field exposes a less visible type"),
                                Some(field.name_span),
                            ),
                            declaration,
                            format_args!("field type declared here"),
                        );
                        if let Some(restrictor) = restrictor {
                            error = owned_diagnostic::secondary(
                                error,
                                restrictor,
                                format_args!("restrictive module declared here"),
                            );
                        }
                        return Err(error);
                    }
                }
            }
            fields.push(RawFieldDecl {
                id: field.id,
                ty: ParameterTy::Value(field.ty),
                span: field.name_span,
            });
        }
        raw_records.push(RawRecordDecl {
            is_copy: record.is_copy,
            id: record.id,
            span: record.name_span,
            fields,
        });
    }
    let mut raw_enums = reserve(allocator, enums.enums, at)?;
    for ordinal in 0..enums.enums {
        let enumeration = index.enum_view(EnumId(ordinal))?;
        work.debit(
            1 + enumeration.variant_count() as u64,
            enumeration.diagnostic_span(),
            "enum layout inventory",
        )?;
        let mut variants = reserve(
            allocator,
            enumeration.variant_count(),
            enumeration.diagnostic_span(),
        )?;
        for position in 0..enumeration.variant_count() {
            let id = crate::frontend::oir::owned_types::VariantId {
                enumeration: enumeration.id(),
                index: position,
            };
            let variant = enumeration.variant(id)?;
            variants.push(RawVariantDecl {
                id,
                payload: variant.value_payload(work)?.map(ParameterTy::Value),
                span: variant.diagnostic_span(),
            });
        }
        raw_enums.push(RawEnumDecl {
            id: enumeration.id(),
            span: enumeration.diagnostic_span(),
            variants,
        });
    }
    admit_joint_value_layouts(&raw_records, &raw_enums, allocator)
        .map_err(|error| declaration_error(error, records, at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::{
        declaration_index::{collect_originals, IndexLimits, SourceOwner},
        project::{ProjectLimits, ProjectSources},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    fn with_inputs(test: impl FnOnce(&DeclarationIndex<'_>, &[Record])) {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-joint-source-layout-{}-{}.ox",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(
            &path,
            "enum Choice{Empty,Value(i32)}struct State{choice:Choice}fn main()->i32{return 42;}",
        )
        .unwrap();
        let project = ProjectSources::load_typed(path.to_str().unwrap(), ProjectLimits::default());
        std::fs::remove_file(path).unwrap();
        let project = project.unwrap();
        let work = WorkMeter::default();
        let mut allocator = Allocator::default();
        let index = collect_originals(
            SourceOwner::project(&project),
            IndexLimits::default(),
            &work,
            &mut allocator,
        )
        .unwrap()
        .finish(&work, &mut allocator)
        .unwrap();
        let (key, module) = index.record(RecordId(0)).unwrap();
        let original = &index.sources().ast(module).unwrap().records[key.index];
        let field = &original.fields[0];
        let records = [Record {
            is_copy: false,
            id: RecordId(0),
            name_span: original.name,
            span: original.span,
            end: original.end,
            fields: vec![Field {
                id: FieldId {
                    record: RecordId(0),
                    index: 0,
                },
                ty: ValueTy::Owned(AggregateTy::Enum(EnumId(0))),
                name_span: field.name,
                span: field.span,
            }],
        }];
        test(&index, &records);
    }
    #[test]
    fn joint_source_layout_inventory_keeps_record_and_enum_totals_separate() {
        with_inputs(|index, records| {
            assert!(needed(records));
            let work = WorkMeter::default();
            let mut allocator = Allocator::default();
            let (records, enums) = inventory(index, records, &work, &mut allocator).unwrap();
            assert_eq!(
                (records.records, records.fields, records.layout_bytes, enums),
                (1, 1, 8, 8)
            );
            assert_eq!(allocator.attempts, 6);
        });
    }
    #[test]
    fn joint_source_layout_each_reservation_and_work_failure_reclaims_temporary_storage() {
        with_inputs(|index, records| {
            let work = WorkMeter::default();
            let mut baseline = Allocator::default();
            inventory(index, records, &work, &mut baseline).unwrap();
            for failure in 1..=baseline.attempts {
                let work = WorkMeter::default();
                let mut allocator = Allocator {
                    fail_at: Some(failure),
                    ..Default::default()
                };
                allocator.observer_trace_bound(32).unwrap();
                let (code, (_, live, _)) =
                    super::super::reviewer_source::integration_measured(|| {
                        inventory(index, records, &work, &mut allocator)
                            .unwrap_err()
                            .code
                    });
                assert_eq!(code, "E0400", "allocation {failure}");
                assert_eq!(live, 0);
                assert!(!allocator.observer_trace_overflow);
            }
            let (code, (_, live, _)) = super::super::reviewer_source::integration_measured(|| {
                let work = WorkMeter::new(0);
                let mut allocator = Allocator::default();
                inventory(index, records, &work, &mut allocator)
                    .unwrap_err()
                    .code
            });
            assert_eq!(code, "E0400");
            assert_eq!(live, 0);
        });
    }

    #[test]
    fn joint_source_layout_storage_limit_precedes_every_reservation() {
        with_inputs(|index, records| {
            let mut allocator = Allocator::default();
            let error =
                inventory_with_limit(index, records, &WorkMeter::default(), &mut allocator, 0)
                    .unwrap_err();
            assert_eq!(error.code, "E0400");
            assert_eq!(allocator.attempts, 0);
            let (_, (_, live, peak)) = super::super::reviewer_source::integration_measured(|| {
                let mut allocator = Allocator::default();
                inventory(index, records, &WorkMeter::default(), &mut allocator).unwrap()
            });
            assert_eq!(live, 0);
            assert!(peak < MAX_TEMPORARY_BYTES as isize);
        });
    }
}
