// Observation-only store readback. Every inserted load follows the untouched
// production i32 store, so its four bytes are initialized. No behavior is replaced.
fn independent_effect_observer(module: &str) -> String {
    let sites = ["f0_b0_i2", "f1_b0_i2", "f2_b0_i2", "f0_b2_i0"];
    let mut result = module.to_string();
    let mut insertions = vec![];
    for (ordinal, prefix) in sites.into_iter().enumerate() {
        let found: Vec<_> = module.lines().filter(|line| {
            line.trim_start().starts_with("store i32 ") && line.contains(&format!("%{prefix}_"))
        }).collect();
        assert_eq!(found.len(), 1, "one production store for {prefix}");
        let line = found[0];
        let pointer = line.split_once(", ptr ").unwrap().1.split(',').next().unwrap();
        let inserted = format!(
            "  %__independent_effect_{ordinal} = load i32, ptr {pointer}, align 1\n  call i32 @__oxid_print_i32(i32 %__independent_effect_{ordinal})\n"
        );
        let marker = format!("{line}\n");
        assert_eq!(result.matches(&marker).count(), 1);
        result = result.replacen(&marker, &format!("{marker}{inserted}"), 1);
        insertions.push(inserted);
    }
    let mut reconstructed = result.clone();
    for inserted in insertions {
        assert_eq!(reconstructed.matches(&inserted).count(), 1);
        reconstructed = reconstructed.replacen(&inserted, "", 1);
    }
    assert_eq!(reconstructed, module, "every original operation body remains byte-exact");
    result
}
#[test]
#[ignore = "independent Unit2D pinned LLVM helper-effect and all-budget gate"]
fn independent_unit2d_helper_effects_all_budgets_llvm() {
    let scratch = Scratch::new();
    let evidence = std::path::PathBuf::from(std::env::var_os("OXID_OWNED_NATIVE_EVIDENCE").unwrap());
    let mut profiles = 0;
    for failure in 0..4 {
        let (sources, mut raw, mut schedule) = effects_fixture(failure);
        let base = raw.functions[0].span;
        let bounds = match failure { 1 => Some(s(base, 105)), 2 => Some(s(base, 205)), 3 => Some(s(base, 30)), _ => None };
        if let Some(at) = bounds {
            schedule.truncate(schedule.iter().position(|event| event.0 == at).unwrap() + 1);
        }
        let total: usize = schedule.iter().map(|event| event.1).sum();
        if failure == 0 { assert_eq!(total, 96); }
        if failure == 3 { assert_eq!(total, 89); }
        append_cycle(&mut raw);
        let name = format!("effects-{failure}");
        independent_structure_receipt(&raw, 1, &name, true);
        let module = independent_native(raw, &sources, total);
        std::fs::write(evidence.join(format!("{name}-production.ll")), &module).unwrap();
        let production = scratch.compile(&module, &format!("{name}-production"));
        independent_assert_output(
            independent_run(&scratch, &production, &format!("{name}-production"), &[]),
            bounds.is_none().then_some(Scalar::I32(77)),
            bounds.map(|at| independent_diagnostic("bounds", at, &sources)),
        );
        let plain = scratch.compile(&argv_fuel_harness(&module, total), &format!("{name}-argv"));
        let observation = independent_effect_observer(&module);
        std::fs::write(evidence.join(format!("{name}-observation.ll")), &observation).unwrap();
        let observed = scratch.compile(&argv_fuel_harness(&observation, total), &format!("{name}-observed-argv"));
        for fuel in 0..=total {
            let mut remaining = fuel;
            let mut unpaid = None;
            let mut paid = vec![];
            for &(at, cost) in &schedule {
                if remaining < cost { unpaid = Some(at); break; }
                remaining -= cost;
                paid.push(at);
            }
            let error = unpaid.map(|at| independent_diagnostic("fuel", at, &sources))
                .or_else(|| bounds.map(|at| independent_diagnostic("bounds", at, &sources)));
            let plain_output = independent_run(&scratch, &plain, &format!("{name}-plain-budget{fuel}"), &[fuel.to_string()]);
            independent_assert_output(plain_output, error.is_none().then_some(Scalar::I32(77)), error.clone());
            let mut stdout = String::new();
            for (at, value, valid) in [(3,17,true),(103,31,true),(203,47,true),(30,77,failure != 3)] {
                if valid && paid.contains(&s(base, at)) { stdout.push_str(&format!("{value}\n")); }
            }
            if error.is_none() { stdout.push_str("77\n"); }
            let observed_output = independent_run(&scratch, &observed, &format!("{name}-observed-budget{fuel}"), &[fuel.to_string()]);
            assert_result(observed_output, stdout.as_bytes(), error.as_deref().unwrap_or("").as_bytes(), i32::from(error.is_some()));
            profiles += 1;
        }
    }
    assert_eq!(profiles, 341);
    eprintln!("independent helper effects: 341 budgets in each of ordinary and readback-observation ELF, plus 4 untouched production wrappers");
}
