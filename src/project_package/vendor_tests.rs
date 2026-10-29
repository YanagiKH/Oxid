use super::*;
use std::mem::{align_of, size_of};

#[test]
fn vendor_transfer_uses_real_member_widths_and_moves_both_owned_buffers() {
    assert_eq!(size_of::<archive::Member>(), size_of::<(String, Vec<u8>)>());
    #[cfg(all(target_pointer_width = "64", target_endian = "little"))]
    {
        assert_eq!(size_of::<archive::Member>(), 48);
        assert_eq!(align_of::<archive::Member>(), 8);
        assert_eq!(size_of::<VendorPayload>(), 24);
        assert_eq!(size_of::<VendorPayloadIterator>(), 32);
        assert_eq!(size_of::<VendorMembersResult>(), 32);
    }
    let mut input = Vec::with_capacity(4);
    input.push((
        String::from("source/main.ox"),
        Vec::from(b"fn main()->i32{return 0;}"),
    ));
    let path = input[0].0.as_ptr();
    let bytes = input[0].1.as_ptr();
    let path_capacity = input[0].0.capacity();
    let bytes_capacity = input[0].1.capacity();
    // Independent dynamic-width equation: original tuple slots plus the new
    // Member slots. Neither path capacity nor payload capacity is duplicated.
    let expected_slots = input.capacity() * size_of::<(String, Vec<u8>)>()
        + input.len() * size_of::<archive::Member>();
    let mut admission = VendorAdmission::new(VENDOR_BRIDGE_BYTES).unwrap();
    let initial = admission.used;
    let output = vendor_members(input, &mut admission).unwrap();
    assert_eq!(admission.used - initial, expected_slots);
    assert_eq!(output[0].path.as_ptr(), path);
    assert_eq!(output[0].bytes.as_ptr(), bytes);
    assert_eq!(output[0].path.capacity(), path_capacity);
    assert_eq!(output[0].bytes.capacity(), bytes_capacity);
    assert_eq!(output.capacity(), 1);
}

#[test]
fn vendor_exact_and_one_less_member_admission_precede_reservation() {
    fn input() -> VendorPayload {
        vec![("source/main.ox".into(), vec![42; 16])]
    }
    assert_eq!(VENDOR_PATH_BYTES, 8192);
    assert_eq!(VENDOR_PATH_CAPACITY, 16384);
    let fixed = size_of::<VendorBridgeRoles>() + crate::project_vendor::control_bytes() + 8 * 16384;
    let required = fixed + 2 * size_of::<archive::Member>();
    let mut exact = VendorAdmission::new(required).unwrap();
    VENDOR_RESERVATIONS.with(|count| count.set(0));
    assert_eq!(vendor_members(input(), &mut exact).unwrap().len(), 1);
    assert_eq!(exact.used, required);
    assert_eq!(VENDOR_RESERVATIONS.with(|count| count.get()), 1);
    let mut short = VendorAdmission::new(required - 1).unwrap();
    VENDOR_RESERVATIONS.with(|count| count.set(0));
    assert!(vendor_members(input(), &mut short)
        .unwrap_err()
        .contains("metadata"));
    assert_eq!(VENDOR_RESERVATIONS.with(|count| count.get()), 0);
    assert!(VendorAdmission::new(fixed - 1).is_err());
}

#[test]
fn vendor_preserves_the_existing_file_cap_and_rejects_metadata_overflow_before_reserve() {
    let input = (0..archive::MAX_FILES)
        .map(|index| (format!("source/f{index}.ox"), Vec::new()))
        .collect::<VendorPayload>();
    let mut admitted = VendorAdmission::new(VENDOR_BRIDGE_BYTES).unwrap();
    assert_eq!(vendor_members(input, &mut admitted).unwrap().len(), 2047);
    let input = (0..=archive::MAX_FILES)
        .map(|index| (format!("source/f{index}.ox"), Vec::new()))
        .collect::<VendorPayload>();
    let mut refused = VendorAdmission::new(VENDOR_BRIDGE_BYTES).unwrap();
    VENDOR_RESERVATIONS.with(|count| count.set(0));
    assert!(vendor_members(input, &mut refused)
        .unwrap_err()
        .contains("2047"));
    assert_eq!(VENDOR_RESERVATIONS.with(|count| count.get()), 0);
    assert!(vendor_member_slots(usize::MAX, 1).is_err());
    assert!(vendor_member_slots(usize::MAX / size_of::<archive::Member>(), 1).is_err());
    assert_eq!(VENDOR_RESERVATIONS.with(|count| count.get()), 0);
}

#[test]
fn final_location_depth_keeps_the_existing_discovery_limit_for_every_package_root() {
    let archive = archive::Archive {
        root: "source".into(),
        files: vec![archive::Member {
            path: "source/app/oxid.toml".into(),
            bytes: Vec::new(),
        }],
    };
    let mut output = PathBuf::from("/");
    for _ in 0..62 {
        output.push("x");
    }
    assert_eq!(output.ancestors().count() + 2, 65);
    vendor_final_depth(&output, &archive).unwrap();
    output.push("x");
    assert!(vendor_final_depth(&output, &archive)
        .unwrap_err()
        .contains("64-ancestor"));
}

#[test]
fn destination_discovery_models_actual_file_parser_and_traversal_transports() {
    let mut offset = 0usize;
    let mut alignment = 1usize;
    macro_rules! role {
        ($field:ident, $ty:ty) => {{
            let align = align_of::<$ty>();
            alignment = alignment.max(align);
            offset = offset.div_ceil(align) * align;
            assert_eq!(std::mem::offset_of!(VendorLocationRoles, $field), offset);
            offset += size_of::<$ty>();
        }};
    }
    role!(
        ancestors,
        std::iter::Enumerate<std::path::Ancestors<'static>>
    );
    role!(next, Option<(usize, &'static Path)>);
    role!(directory, &'static Path);
    role!(depth, usize);
    role!(candidate_return, VendorPathResult);
    role!(candidate, PathBuf);
    role!(metadata_return, io::Result<fs::Metadata>);
    role!(before, fs::Metadata);
    role!(opened, fs::Metadata);
    role!(after, fs::Metadata);
    role!(file_return, io::Result<File>);
    role!(file, File);
    role!(options, OpenOptions);
    role!(raw, Vec<u8>);
    role!(reservation, VendorReservation);
    role!(mapped_reservation, Result<(), String>);
    role!(read_return, io::Result<()>);
    role!(witness_return, io::Result<usize>);
    role!(declaration_construction, VendorWorkspaceResult);
    role!(declaration_return, VendorWorkspaceResult);
    role!(declaration_caller, VendorWorkspaceResult);
    role!(declaration, Option<crate::runtime::WorkspaceManifest>);
    role!(lossy, std::borrow::Cow<'static, str>);
    role!(pending, Option<(String, String)>);
    role!(key, String);
    role!(value, String);
    role!(decoded, String);
    role!(arrays, [Vec<String>; 2]);
    role!(location_construction, VendorLocationResult);
    role!(location_return, VendorLocationResult);
    role!(location_caller, VendorLocationResult);
    role!(input, &'static [u8]);
    role!(file_length, usize);
    role!(scope, usize);
    role!(saved_admission, usize);
    role!(checked, Option<usize>);
    role!(file_length_result, Result<usize, std::num::TryFromIntError>);
    role!(files, &'static [archive::Member]);
    role!(files_iter, std::slice::Iter<'static, archive::Member>);
    role!(member, &'static archive::Member);
    role!(relative, &'static Path);
    role!(components, std::path::Components<'static>);
    role!(base_depth, usize);
    role!(relative_depth, usize);
    assert_eq!(
        size_of::<VendorLocationRoles>(),
        offset.div_ceil(alignment) * alignment
    );
}

#[test]
fn vendor_bridge_carriers_keep_separate_real_return_and_selected_roles() {
    // An independent repr(C) layout oracle walks the actual member types and
    // role multiplicities; it does not use the reported whole bridge fee.
    let mut offset = 0usize;
    let mut alignment = 1usize;
    macro_rules! role {
        ($field:ident, $ty:ty) => {{
            let align = align_of::<$ty>();
            alignment = alignment.max(align);
            offset = offset.div_ceil(align) * align;
            assert_eq!(std::mem::offset_of!(VendorBridgeRoles, $field), offset);
            offset += size_of::<$ty>();
        }};
    }
    role!(source_argument, VendorPayload);
    role!(source_iterator, VendorPayloadIterator);
    role!(source_next, Option<(String, Vec<u8>)>);
    role!(source_selected, (String, Vec<u8>));
    role!(member, archive::Member);
    role!(members, Vec<archive::Member>);
    role!(members_construction, VendorMembersResult);
    role!(members_return, VendorMembersResult);
    role!(members_caller, VendorMembersResult);
    role!(reservation, VendorReservation);
    role!(mapped_reservation, Result<(), String>);
    role!(plan_return, Result<SourcePlan, String>);
    role!(retained_plan, SourcePlan);
    role!(archive, archive::Archive);
    role!(destination_return, Result<PathBuf, String>);
    role!(destination, PathBuf);
    role!(path_construction, VendorPathResult);
    role!(path_return, VendorPathResult);
    role!(path_caller, VendorPathResult);
    role!(stage_return, Result<PrivateDirectory, String>);
    role!(stage, PrivateDirectory);
    role!(exported_root, PathBuf);
    role!(info_return, io::Result<archive::Info>);
    role!(info_mapped, VendorInfoResult);
    role!(info, archive::Info);
    role!(summary, VendorSummary);
    role!(summary_construction, VendorResult);
    role!(summary_return, VendorResult);
    role!(summary_caller, VendorResult);
    role!(admission, VendorAdmission);
    role!(admission_construction, VendorAdmissionResult);
    role!(admission_return, VendorAdmissionResult);
    role!(admission_caller, VendorAdmissionResult);
    role!(parent, Option<&'static Path>);
    role!(check_return, Result<(), String>);
    role!(requested_slots, usize);
    role!(requested_bytes, Option<usize>);
    role!(bound_construction, VendorBound);
    role!(bound_return, VendorBound);
    role!(bound_caller, VendorBound);
    role!(selected_bound, usize);
    role!(path_input, &'static Path);
    role!(path_output, PathBuf);
    role!(path_reservation, VendorReservation);
    role!(location, VendorLocationRoles);
    role!(requested_name, Option<&'static std::ffi::OsStr>);
    assert_eq!(
        size_of::<VendorBridgeRoles>(),
        offset.div_ceil(alignment) * alignment
    );
    assert_eq!(align_of::<VendorBridgeRoles>(), alignment);
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod filesystem {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "oxid-vendor-unit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("app")).unwrap();
            let app = root.join("app");
            let manifest = b"[project]\nname=\"app\"\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[build]\nentry-mode=\"result\"\n";
            fs::write(app.join("oxid.toml"), manifest).unwrap();
            fs::write(app.join("main.ox"), "fn main()->i32{return 42;}").unwrap();
            let snapshot =
                crate::runtime::parse_typed_manifest_bytes(&app.join("oxid.toml"), manifest)
                    .unwrap();
            let graph = [crate::frontend::PackageSpec {
                root: app.clone(),
                entry: app.join("main.ox"),
                dependencies: Vec::new(),
            }];
            let expected = crate::project_lock::expected(&graph, &[snapshot], &[]).unwrap();
            crate::project_lock::publish(&app, &expected, None, false).unwrap();
            Self(root)
        }
        fn no_staging(&self) {
            assert!(fs::read_dir(&self.0).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".oxid-package-")));
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    struct FaultGuard(Option<VendorFault>);
    impl FaultGuard {
        fn new(fault: VendorFault) -> Self {
            Self(VENDOR_FAULT.with(|current| current.replace(Some(fault))))
        }
    }
    impl Drop for FaultGuard {
        fn drop(&mut self) {
            VENDOR_FAULT.with(|current| current.set(self.0));
        }
    }

    #[test]
    fn admission_reservation_and_prepublication_failures_preserve_inputs_and_clean_staging() {
        for fault in [VendorFault::Reservation, VendorFault::BeforePublish] {
            let fixture = Fixture::new();
            let app = fixture.0.join("app");
            let destination = fixture.0.join("offline");
            let lock = fs::read(app.join("oxid.lock")).unwrap();
            let manifest = fs::read(app.join("oxid.toml")).unwrap();
            let _guard = FaultGuard::new(fault);
            assert!(vendor_directory(&app, &destination, false).is_err());
            assert!(!destination.exists());
            assert_eq!(fs::read(app.join("oxid.lock")).unwrap(), lock);
            assert_eq!(fs::read(app.join("oxid.toml")).unwrap(), manifest);
            fixture.no_staging();
        }
        let fixture = Fixture::new();
        assert!(vendor_directory_with_limit(
            &fixture.0.join("app"),
            &fixture.0.join("offline"),
            false,
            0
        )
        .is_err());
        assert!(!fixture.0.join("offline").exists());
        fixture.no_staging();
    }

    #[test]
    fn parent_sync_failure_reports_the_complete_already_published_checkout() {
        let fixture = Fixture::new();
        let destination = fixture.0.join("offline");
        let _guard = FaultGuard::new(VendorFault::ParentSync);
        let error = vendor_directory(&fixture.0.join("app"), &destination, false)
            .err()
            .unwrap();
        assert!(
            error.contains("was published at") && error.contains("parent directory"),
            "{error}"
        );
        assert!(destination.join("source/oxid.toml").is_file());
        assert!(destination.join("source/oxid.lock").is_file());
        assert_eq!(
            fs::read(destination.join("source/main.ox")).unwrap(),
            b"fn main()->i32{return 42;}"
        );
        fixture.no_staging();
    }

    #[test]
    fn destination_workspace_scope_is_fallible_exact_and_one_less_before_read_buffer_reservation() {
        let fixture = Fixture::new();
        let raw = b"[workspace]\nedition=\"typed-preview\"\nmembers=[\"app\"]\n";
        fs::write(fixture.0.join("oxid.toml"), raw).unwrap();
        let expected = (raw.len() + 1) * 32 + 128 * size_of::<String>() + 4096;
        assert_eq!(vendor_workspace_scope(raw.len()).unwrap(), expected);
        assert!(vendor_workspace_scope(usize::MAX).is_err());
        let fixed = VendorAdmission::new(VENDOR_BRIDGE_BYTES).unwrap().used;
        let mut exact = VendorAdmission::new(fixed + expected).unwrap();
        VENDOR_RESERVATIONS.with(|count| count.set(0));
        let error = vendor_destination_context(&fixture.0.join("offline"), &mut exact).unwrap_err();
        assert!(error.contains("inside the workspace"), "{error}");
        assert_eq!(VENDOR_RESERVATIONS.with(|count| count.get()), 1);
        assert_eq!(exact.used, fixed);
        let mut short = VendorAdmission::new(fixed + expected - 1).unwrap();
        VENDOR_RESERVATIONS.with(|count| count.set(0));
        assert!(
            vendor_destination_context(&fixture.0.join("offline"), &mut short)
                .unwrap_err()
                .contains("metadata")
        );
        assert_eq!(VENDOR_RESERVATIONS.with(|count| count.get()), 0);
        assert_eq!(fs::read(fixture.0.join("oxid.toml")).unwrap(), raw);
        assert!(!fixture.0.join("offline").exists());
        fixture.no_staging();
    }
}
