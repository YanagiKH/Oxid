use super::*;

const SOURCE: &str = "#[derive(Copy)]struct Point{x:i32,y:i32}fn inspect(prefix:i32,input:Point,suffix:i32)->i32{let original=input;let mut changed=original;changed.x=changed.x+3;let snapshot=changed;changed.y=changed.y+3;return original.x+changed.x+snapshot.y+changed.y+prefix+suffix;}fn main()->i32{let preceding=Point{x:17,y:1};return inspect(0,preceding,0);}";

#[allow(clippy::too_many_arguments)]
fn emit(
    witness: &VerifiedOwnedProgram,
    entry: hir::DefId,
    sources: &SourceMap,
    policy: NativeEntryPolicy,
    debug: bool,
    limits: Limits,
    accounting: &mut Accounting,
) -> Result<String, Box<Diagnostic>> {
    native_module_policy_accounted_with_source_names_debug(
        witness,
        Some(entry),
        sources,
        plan::MAX_FUEL,
        limits,
        policy,
        accounting,
        None,
        debug,
    )
}

#[test]
fn record_debug_markers_use_checked_nonzero_owner_offsets_and_argument_ordinals() {
    let (sources, witness, entry) = source::copy_record_tests::checked(SOURCE);
    for policy in [
        NativeEntryPolicy::TypedResult,
        NativeEntryPolicy::HostedTypedResult,
    ] {
        let mut plain_counts = Accounting::default();
        let plain = emit(
            &witness,
            entry,
            &sources,
            policy,
            false,
            Limits::for_policy(policy),
            &mut plain_counts,
        )
        .unwrap();
        let mut debug_counts = Accounting::default();
        let marked = emit(
            &witness,
            entry,
            &sources,
            policy,
            true,
            Limits::for_policy(policy),
            &mut debug_counts,
        )
        .unwrap();
        assert_eq!(
            debug_counts.metrics.emitter_transient_bound,
            plain_counts.metrics.emitter_transient_bound + RECORD_DEBUG_CONTROL_BYTES
        );
        assert_eq!(
            debug_counts.metrics.plan_bytes,
            plain_counts.metrics.plan_bytes
        );
        assert_eq!(
            debug_counts.metrics.allocation_attempts,
            plain_counts.metrics.allocation_attempts
        );
        assert_eq!(
            debug_counts.metrics.count_expansions,
            plain_counts.metrics.count_expansions
        );
        assert_eq!(
            debug_counts.metrics.count_ordinary_visits,
            plain_counts.metrics.count_ordinary_visits
        );
        let stripped = marked
            .lines()
            .filter(|line| !line.starts_with("; oxid.debug."))
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        assert_eq!(stripped, plain);
        assert_eq!(
            marked.matches("alloca ").count(),
            plain.matches("alloca ").count()
        );
        let plan = ExecutionPlan::build(&witness).unwrap();
        let storage = NativeStoragePlan::checked(&plan, true).unwrap();
        let mut nonzero_local = false;
        for (function_id, function) in witness.functions().iter().enumerate() {
            let function_storage = storage.function(hir::DefId(function_id));
            for (index, owner) in function.owners.iter().enumerate() {
                let argument = match owner.kind {
                    OwnerKind::Local { .. } => 0,
                    OwnerKind::Parameter { position } => position + 1,
                    _ => continue,
                };
                let AggregateTy::Record(record) = owner.aggregate() else {
                    continue;
                };
                let layout = witness.declarations().record(record).unwrap().layout();
                let offset = function_storage.owner_offset(OwnerPlaceId(index));
                assert_eq!(offset % layout.align(), 0);
                assert!(
                    offset.checked_add(layout.size()).unwrap() <= function_storage.owner_bytes()
                );
                nonzero_local |= argument == 0 && offset != 0;
                let marker = format!(
                    "; oxid.debug.record {} {} {} {} {argument} {offset} address %owners",
                    owner.span.file.0, owner.span.start, owner.span.end, record.0
                );
                assert!(marked.lines().any(|line| line == marker), "{marker}");
                if argument != 0 {
                    assert_eq!(
                        argument, 2,
                        "hidden native arguments must not shift source ordinals"
                    );
                    let marker_at = marked.find(&marker).unwrap();
                    let initialized = marked[..marker_at].rfind("store i32").unwrap();
                    assert!(
                        initialized < marker_at,
                        "record parameter transfer precedes its marker"
                    );
                }
            }
        }
        assert!(
            nonzero_local,
            "fixture must exercise offsets inside the owner arena"
        );
    }
}

#[test]
fn record_debug_emission_keeps_exact_byte_metadata_and_allocation_denials() {
    let (sources, witness, entry) = source::copy_record_tests::checked(SOURCE);
    let policy = NativeEntryPolicy::TypedResult;
    let mut counts = Accounting::default();
    let module = emit(
        &witness,
        entry,
        &sources,
        policy,
        true,
        Limits::DEFAULT,
        &mut counts,
    )
    .unwrap();
    assert_eq!(counts.metrics.count_bytes, module.len());
    assert_eq!(counts.metrics.render_bytes, module.len());
    for less in [0, 1] {
        for limits in [
            Limits {
                ir_bytes: module.len() - less,
                ..Limits::DEFAULT
            },
            Limits {
                metadata_bytes: counts.metrics.metadata_peak - less,
                ..Limits::DEFAULT
            },
        ] {
            let mut denied = Accounting::default();
            let result = emit(&witness, entry, &sources, policy, true, limits, &mut denied);
            assert_eq!(result.is_ok(), less == 0);
            if let Err(error) = result {
                assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
            }
        }
    }
    for failure in 0..counts.metrics.allocation_attempts {
        let mut denied = Accounting {
            fail_after: Some(failure),
            ..Accounting::default()
        };
        let error = emit(
            &witness,
            entry,
            &sources,
            policy,
            true,
            Limits::DEFAULT,
            &mut denied,
        )
        .unwrap_err();
        assert_eq!((error.code, error.stage), ("E0700", "native-admission"));
        assert_eq!(denied.metrics.allocation_attempts, failure + 1);
        assert!(denied.metrics.failed_allocation.is_some());
        assert_eq!(denied.metrics.render_bytes, 0);
    }
}
