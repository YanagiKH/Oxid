use super::*;

#[test]
fn checked_copy_native_counts_preflight_and_copy_and_keeps_complete_validation_before_stores() {
    let text = "#[derive(Copy)]struct P{first:i32,last:bool}fn main()->i32{let p=P{first:21,last:true};let q=p;return p.first+q.first;}";
    let (sources, witness, entry) = source::copy_record_tests::checked(text);
    let observation = run_array_observed(&witness, Some(entry), &sources, NativeControl::default());
    let module = observation.result.unwrap();
    assert_eq!(observation.metrics.count_bytes, module.len());
    assert_eq!(observation.metrics.render_bytes, module.len());
    assert_eq!(
        observation.metrics.count_expansions,
        observation.metrics.transfer_cells + observation.metrics.message_bytes
    );
    assert_eq!(
        observation.metrics.count_expansion_kinds,
        observation.metrics.render_expansion_kinds
    );
    assert!(
        observation.metrics.emitter_transient_bound
            >= COPY_TRANSFER_CONTROL_BYTES + EMITTER_TRANSIENT_BYTES
    );
    let (index, _) = witness.functions()[entry.0].blocks[0]
        .statements
        .iter()
        .enumerate()
        .find(|(_, statement)| matches!(statement.kind, OwnedInstruction::CopyInitialize { .. }))
        .unwrap();
    let prefix = format!("f{}_b0_i{index}", entry.0);
    let last_checked = module.find(&format!("{prefix}_validate1_ok:")).unwrap();
    let first_write = module
        .find(&format!("  store i32 %{prefix}_copy_field0"))
        .unwrap();
    assert!(last_checked < first_write);
    assert!(module.contains(&format!("{prefix}_aggregate_ok:")));
    for forbidden in [
        "inbounds",
        "noalias",
        "nonnull",
        "dereferenceable",
        "undef",
        "poison",
    ] {
        assert!(!module.contains(forbidden), "{forbidden}");
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires LLVM 19.1.7; source-free Copy record execution at O0 and O2"]
fn checked_copy_native_nested_generic_values_execute_at_both_optimizations() {
    use crate::frontend::build_config::{NativeBuildSettings, NativeOptimization};
    let (sources, witness, entry) =
        source::copy_record_tests::checked(source::copy_record_tests::NESTED);
    let module = native_module(&witness, Some(entry), &sources).unwrap();
    let root = std::env::temp_dir().join(format!(
        "oxid-copy-native-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    for optimization in [NativeOptimization::O0, NativeOptimization::O2] {
        let output = root.join(format!("copy-{optimization:?}"));
        crate::frontend::native::compile_with_settings(
            &module,
            output.to_str().unwrap(),
            &NativeBuildSettings::with_optimization(false, optimization),
        )
        .unwrap();
        let result = std::process::Command::new(&output)
            .current_dir(&root)
            .env_clear()
            .output()
            .unwrap();
        assert_eq!(
            (result.status.code(), result.stdout, result.stderr),
            (Some(0), b"84\n".to_vec(), vec![])
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
#[ignore = "requires LLVM 19.1.7; late-leaf Copy corruption and partial-write oracle controls at O0/O2"]
fn checked_copy_native_late_invalid_scalars_preserve_destination_and_source() {
    use crate::frontend::build_config::{NativeBuildSettings, NativeOptimization};
    for (field, initial, storage, valid, invalid) in [
        ("bool", "true", "i8", 1u32, 2u32),
        ("()", "()", "i8", 0, 1),
        ("char", "'A'", "i32", 65, 0xd800),
        ("char", "'A'", "i32", 65, 0x110000),
    ] {
        let text = format!("#[derive(Copy)]struct P{{earlier:i32,later:{field}}}fn main()->i32{{let p=P{{earlier:21,later:{initial}}};let q=p;return p.earlier+q.earlier;}}");
        let (sources, witness, entry) = source::copy_record_tests::checked(&text);
        let (index, statement, from, to) = witness.functions()[entry.0].blocks[0]
            .statements
            .iter()
            .enumerate()
            .find_map(|(i, statement)| match statement.kind {
                OwnedInstruction::CopyInitialize {
                    source,
                    destination,
                } => Some((i, statement, source.0, destination.0)),
                _ => None,
            })
            .unwrap();
        let prefix = format!("f{}_b0_i{index}", entry.0);
        let needle = format!("  %{prefix}_validate0_ptr = getelementptr i8, ptr %o{from}, i64 0\n");
        let failure = format!("{prefix}_validate1_error:\n");
        let original = native_module(&witness, Some(entry), &sources).unwrap();
        assert_eq!(original.matches(&needle).count(), 1);
        assert_eq!(original.matches(&failure).count(), 1);
        let mut observe = String::new();
        for (owner_index, owner) in [to, from].into_iter().enumerate() {
            for byte in 0..8 {
                observe.push_str(&format!("  %probe{owner_index}_{byte} = getelementptr i8, ptr %o{owner}, i64 {byte}\n  %probe_byte{owner_index}_{byte} = load volatile i8, ptr %probe{owner_index}_{byte}, align 1\n  %probe_value{owner_index}_{byte} = zext i8 %probe_byte{owner_index}_{byte} to i32\n  %probe_print{owner_index}_{byte} = call i32 @__oxid_print_i32(i32 %probe_value{owner_index}_{byte})\n"));
            }
        }
        let original = original.replacen(&failure, &format!("{failure}{observe}"), 1);
        let root = std::env::temp_dir().join(format!(
            "oxid-copy-fault-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        for optimization in [NativeOptimization::O0, NativeOptimization::O2] {
            for (control, last, partial) in [
                ("valid", valid, false),
                ("invalid", invalid, false),
                ("partial-control", invalid, true),
            ] {
                let mut insert = String::new();
                for byte in 0..8 {
                    insert.push_str(&format!("  %seed{byte} = getelementptr i8, ptr %o{to}, i64 {byte}\n  store i8 165, ptr %seed{byte}, align 1\n"));
                }
                // Initialize source padding in the harness solely to make its
                // observation deterministic; the generated copy never reads it.
                let width = if storage == "i8" { 1 } else { 4 };
                for byte in 4 + width..8 {
                    insert.push_str(&format!("  %source_seed{byte} = getelementptr i8, ptr %o{from}, i64 {byte}\n  store i8 90, ptr %source_seed{byte}, align 1\n"));
                }
                insert.push_str(&format!("  %corrupt = getelementptr i8, ptr %o{from}, i64 4\n  store {storage} {last}, ptr %corrupt, align 1\n"));
                if partial {
                    insert.push_str(&format!("  store i8 255, ptr %o{to}, align 1\n"));
                }
                let harness = original.replacen(&needle, &format!("{insert}{needle}"), 1);
                let output = root.join(format!("copy-{optimization:?}-{control}"));
                crate::frontend::native::compile_with_settings(
                    &harness,
                    output.to_str().unwrap(),
                    &NativeBuildSettings::with_optimization(false, optimization),
                )
                .unwrap();
                let result = std::process::Command::new(&output)
                    .current_dir(&root)
                    .env_clear()
                    .output()
                    .unwrap();
                if last == valid {
                    assert_eq!(
                        (result.status.code(), result.stdout, result.stderr),
                        (Some(0), b"42\n".to_vec(), vec![])
                    );
                } else {
                    let mut bytes = vec![165; 8];
                    if partial {
                        bytes[0] = 255;
                    }
                    bytes.extend([21, 0, 0, 0]);
                    bytes.extend(&last.to_le_bytes()[..width]);
                    bytes.resize(16, 90);
                    let expected = bytes
                        .iter()
                        .map(|byte| format!("{byte}\n"))
                        .collect::<String>();
                    assert_eq!(result.status.code(), Some(1));
                    assert_eq!(result.stdout, expected.as_bytes());
                    assert_eq!(
                        result.stderr,
                        execute::OwnedRunFailure::Invariant("enum payload", Some(statement.span))
                            .diagnostic(&sources)
                            .render_human(&sources)
                            .as_bytes()
                    );
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
