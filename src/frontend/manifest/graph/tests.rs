use super::*;

#[test]
fn path_dependency_manifest_and_edge_budgets_reject_before_growth() {
    let text =
        "[project]\nedition=\"typed-preview\"\nentry=\"main.ox\"\n[dependencies]\ndep=\"../dep\"\n";
    let mut allocator = Allocator::default();
    let mut budget = Budget {
        manifest_bytes: M_RAW - text.len() + 1,
        ..Budget::default()
    };
    let error = package(
        "oxid.toml",
        "oxid.toml".into(),
        text.as_bytes(),
        Command::Check,
        false,
        &mut budget,
        &mut allocator,
    )
    .unwrap_err();
    assert!(error.message().contains("aggregate manifest"));
    assert_eq!(allocator.attempts, 0);
    let mut budget = Budget {
        edge_bytes: EDGE_BYTES,
        ..Budget::default()
    };
    let error = package(
        "oxid.toml",
        "oxid.toml".into(),
        text.as_bytes(),
        Command::Check,
        false,
        &mut budget,
        &mut allocator,
    )
    .unwrap_err();
    assert!(error.message().contains("edge storage"));
    assert_eq!(allocator.attempts, 0);
    let error = read_limit(&mut std::io::Cursor::new(b"ab"), &mut allocator, 1)
        .err()
        .unwrap();
    assert!(matches!(
        error,
        ReadFailure::Limit(Failure::Resource(
            "aggregate manifest input exceeds 16384 bytes"
        ))
    ));
}

#[test]
fn path_dependency_aliases_follow_source_lexer_keywords() {
    for alias in ["a", "_lib", "lib2", "u8"] {
        assert!(alias_valid(alias));
    }
    for alias in [
        "", "1x", "a-b", "é", "fn", "use", "as", "crate", "std", "enum", "match", "false", "loop",
        "self", "Self",
    ] {
        assert!(!alias_valid(alias), "{alias}");
    }
}
