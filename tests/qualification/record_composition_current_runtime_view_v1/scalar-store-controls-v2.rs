#[cfg(test)]
mod record_adapter_controls {
    use super::*;
    #[test]
    fn record_adapter_scalar_store_events_and_unpaid_store_boundary() {
        let out = PathBuf::from(std::env::var_os("OXID_RECORD_OBSERVER_CONTROLS").expect("explicit evidence directory"));
        std::fs::create_dir_all(&out).unwrap();
        let input = out.join("scalar-store.ox");
        std::fs::write(&input, "struct C { value: i32, flag: bool, empty: () } fn main()->i32 { let mut c=C { value: 1, flag: true, empty: () }; c.value=7; c.flag=false; c.empty=(); return c.value; }").unwrap();
        let loaded = project::ProjectSources::load_project_candidate(input.to_str().unwrap(), project::ProjectLimits::default()).unwrap();
        let checked = oir::project::check_project_executable_candidate(&loaded,
            declaration_index::IndexLimits::default(), &declaration_index::WorkMeter::default(), &mut project::budget::Allocator::default()).unwrap();
        let ordinary = checked.run().unwrap();
        JOURNAL.with_borrow_mut(|j| *j = Journal { enabled: true, ..Journal::default() });
        begin_run(None);
        assert_eq!(checked.run().unwrap(), ordinary);
        assert_eq!(ordinary.json(), "{\"type\":\"i32\",\"value\":7}");
        JOURNAL.with_borrow(|j| {
            let writes: Vec<_> = j.rows.iter().enumerate().filter(|(_, row)| row.kind == "payload_write").collect();
            assert_eq!(writes.len(), 9, "three initializer stores, three transfer stores, three scalar writes");
            for (position, _) in &writes[6..] {
                assert_eq!(j.rows[position + 1].kind, "existing_owned_event");
                assert!(j.rows[position + 1].payload.starts_with("WriteField("));
            }
            assert!(writes[6].1.payload.contains("Some([1, 0, 0, 0]), Some([7, 0, 0, 0])"));
            assert!(writes[7].1.payload.contains("Some([1]), Some([0])"));
            assert!(writes[8].1.payload.contains("Some([0]), Some([0])"));
        });
        let mut first_success = None;
        for fuel in 0..=200 {
            begin_run(Some(fuel));
            let result = checked.run();
            JOURNAL.with_borrow(|j| {
                if let Some(denied) = j.rows.iter().position(|row| row.kind == "charge_attempt" && row.payload.ends_with("false)")) {
                    assert!(j.rows[denied + 1..].iter().all(|row| row.kind != "payload_write"));
                    assert!(result.is_err());
                }
            });
            if result.is_ok() { first_success = Some(fuel); break; }
        }
        assert!(first_success.is_some());
        JOURNAL.with_borrow_mut(|j| *j = Journal::default());
    }
}
