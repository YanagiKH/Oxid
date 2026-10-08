use super::*;
use crate::frontend::{
    lexer,
    project::{ProjectLimits, ProjectSources},
    source::SourceMap,
};

fn source(text: &str) -> SourceMap {
    let mut sources = SourceMap::new();
    sources.add("test.ox".into(), text.into());
    sources
}

/// Independent test encoder implementing the written groups/echo rules. The
/// production decoder never calls this and the production lexer is unchanged.
fn encode(source: &SourceFile, limit: usize) -> Vec<u8> {
    let (tokens, diagnostic) = match lexer::lex_with_limit(source, limit) {
        Ok(tokens) => (tokens, None),
        Err(diagnostic) => {
            let at = diagnostic.primary.unwrap();
            let prefix_source = self::source(&source.text()[..at.start]);
            let mut prefix = lexer::lex(prefix_source.get(SourceFileId(0))).unwrap();
            prefix.pop();
            let tag = match diagnostic.message.as_str() {
                "unterminated string literal" => 1,
                "unterminated block comment" => 2,
                "token resource limit exceeded" => 3,
                _ => panic!("unexpected diagnostic"),
            };
            (prefix, Some((tag, at.start, at.end)))
        }
    };
    let mut out = b"LXS1".to_vec();
    let mut echo = [0u8; 128];
    let mut buffer = [0u8; 100];
    let mut token = 0;
    let n = source.text().len();
    for base in (0..=n / 128).map(|chunk| chunk * 128) {
        let used = (n - base).min(128);
        echo[..used].copy_from_slice(&source.text().as_bytes()[base..base + used]);
        out.push(b'E');
        out.push(used as u8);
        out.extend_from_slice(&(base as u32).to_le_bytes());
        out.extend_from_slice(&echo);
        let mut used_tokens = 0;
        while let Some(current) = tokens.get(token) {
            if current.span.end > base + used || (current.kind == lexer::Kind::Eof && used == 128) {
                break;
            }
            buffer[used_tokens] = current.kind as u8 + 1;
            buffer[used_tokens + 1] = (current.span.end - base) as u8;
            used_tokens += 2;
            token += 1;
            if used_tokens == 100 {
                out.extend_from_slice(&buffer);
                used_tokens = 0;
            }
        }
        if used_tokens != 0 {
            out.push(b'B');
            out.push(used_tokens as u8);
            out.extend_from_slice(&buffer);
        }
    }
    assert_eq!(token, tokens.len());
    if let Some((tag, start, end)) = diagnostic {
        out.extend_from_slice(&[b'D', tag]);
        out.extend_from_slice(&(start as u32).to_le_bytes());
        out.extend_from_slice(&(end as u32).to_le_bytes());
    } else {
        out.push(b'S');
        out.extend_from_slice(&(tokens.len() as u32).to_le_bytes());
        out.extend_from_slice(&(n as u32).to_le_bytes());
    }
    out
}

fn decode(
    source: &SourceFile,
    bytes: &[u8],
    limit: usize,
) -> Result<wire::Observation, &'static str> {
    wire::decode(source, limit, bytes, &mut Allocator::default())
}
fn fixture(encoder: fn(&SourceFile, usize) -> Vec<u8>) -> Provider {
    let mut receipts = Vec::new();
    wire::reserve(
        &mut receipts,
        RECEIPTS,
        &mut Allocator::default(),
        "lexical provider receipts",
    )
    .unwrap();
    Provider {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        bundle: None,
        receipts,
        budget: Budget::new(0, 0).unwrap(),
        fixture: Some(encoder),
    }
}

#[test]
fn lexical_wire_roundtrips_full_width_chunks_groups_and_all_kinds() {
    let language = "fn struct mod use pub & . let mut return break continue if while else true false (){}:,;= == != ! && || < <= > >= -> - + * / % enum @ 123 \"ok\" ";
    for text in [
        "".into(),
        language.into(),
        ";".repeat(49),
        ";".repeat(50),
        ";".repeat(51),
        ";".repeat(127),
        ";".repeat(128),
        ";".repeat(129),
        " ".repeat(65536),
        format!("{};", " ".repeat(65536)),
    ] {
        let sources = source(&text);
        let file = sources.get(SourceFileId(0));
        let bytes = encode(file, lexer::MAX_TOKENS);
        let wire::Observation::Tokens(tokens) = decode(file, &bytes, lexer::MAX_TOKENS).unwrap()
        else {
            panic!("tokens");
        };
        let canonical = lexer::lex(file).unwrap();
        assert_eq!(tokens.len(), canonical.len());
        assert_eq!(tokens.capacity(), tokens.len());
        assert!(tokens
            .iter()
            .zip(canonical)
            .all(|(a, b)| a.kind == b.kind && a.span == b.span));
        assert!(bytes.len() <= wire::output_bound(text.len(), lexer::MAX_TOKENS).unwrap());
    }
}
#[test]
fn lexical_wire_rejects_every_truncation_and_all_trailing_data() {
    for text in ["", "fn main() -> i32 { return 42; }", &";".repeat(129)] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        let bytes = encode(file, lexer::MAX_TOKENS);
        for length in 0..bytes.len() {
            assert!(
                decode(file, &bytes[..length], lexer::MAX_TOKENS).is_err(),
                "length {length}"
            );
        }
        for suffix in [&[0][..], b"S", b"LXS1", b"\n"] {
            let mut corrupted = bytes.clone();
            corrupted.extend_from_slice(suffix);
            assert!(decode(file, &corrupted, lexer::MAX_TOKENS).is_err());
        }
    }
}
#[test]
fn lexical_wire_checks_same_kind_same_length_source_and_padding() {
    let sources = source("first");
    let file = sources.get(SourceFileId(0));
    let bytes = encode(file, lexer::MAX_TOKENS);
    let stale_sources = source("other");
    let stale = stale_sources.get(SourceFileId(0));
    assert!(decode(stale, &bytes, lexer::MAX_TOKENS).is_err());
    for index in [
        4, 5, 6, 10, 15, 137, 138, 139, 140, 141, 144, 239, 240, 241, 245,
    ] {
        if index < bytes.len() {
            let mut corrupted = bytes.clone();
            corrupted[index] ^= 128;
            assert!(
                decode(file, &corrupted, lexer::MAX_TOKENS).is_err(),
                "offset {index}"
            );
        }
    }
}
#[test]
fn lexical_wire_rejects_raw_partial_groups_and_duplicate_partial() {
    let sources = source("x");
    let file = sources.get(SourceFileId(0));
    let bytes = encode(file, 100);
    let mut raw_partial = bytes[..138].to_vec();
    raw_partial.extend_from_slice(&[2, 1, 47, 1]);
    raw_partial.extend_from_slice(&bytes[240..]);
    assert!(decode(file, &raw_partial, 100).is_err());
    let mut duplicate = bytes[..240].to_vec();
    duplicate.extend_from_slice(&bytes[138..240]);
    duplicate.extend_from_slice(&bytes[240..]);
    assert!(decode(file, &duplicate, 100).is_err());
    let mut after_partial = bytes[..240].to_vec();
    after_partial.extend_from_slice(&[2, 1]);
    after_partial.extend_from_slice(&bytes[240..]);
    assert!(decode(file, &after_partial, 100).is_err());
}
#[test]
fn lexical_wire_exact_diagnostics_token_budget_and_precedence() {
    for (text, limit, tag) in [
        ("x", 0, 3),
        ("x x", 1, 3),
        ("\"unfinished", 0, 1),
        ("/*unfinished", 0, 2),
        (&" ".repeat(65537), 100000, 3),
    ] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        let bytes = encode(file, limit);
        let wire::Observation::Diagnostic(observed) = decode(file, &bytes, limit).unwrap() else {
            panic!("diagnostic");
        };
        let expected = lexer::lex_with_limit(file, limit).unwrap_err();
        assert_eq!(observed.tag, tag);
        assert_eq!(
            observed.fields(),
            (expected.code, expected.message.as_str())
        );
        let span = expected.primary.unwrap();
        assert_eq!((observed.start, observed.end), (span.start, span.end));
        let mut corrupt = bytes.clone();
        let end = corrupt.len();
        corrupt[end - 1] = 255;
        assert!(decode(file, &corrupt, limit).is_err());
    }
    let sources = source("");
    let file = sources.get(SourceFileId(0));
    assert!(
        matches!(decode(file, &encode(file, 0),0),Ok(wire::Observation::Tokens(tokens)) if tokens.len()==1)
    );
}
#[test]
fn lexical_wire_domain_exact_allocation_failures_and_bound() {
    assert_eq!(wire::output_bound(1048576, 100000).unwrap(), 2133666);
    assert!(wire::output_bound(1048577, 100000).is_err());
    assert!(wire::output_bound(1, 100001).is_err());
    for text in ["", "fn", "é"] {
        let sources = source(text);
        let file = sources.get(SourceFileId(0));
        let mut allocator = Allocator {
            fail_at: Some(1),
            ..Allocator::default()
        };
        assert!(wire::request(file, 100, &mut allocator).is_err());
        if text.is_ascii() {
            let mut allocator = Allocator {
                fail_at: Some(1),
                ..Allocator::default()
            };
            assert!(wire::decode(file, 100, &encode(file, 100), &mut allocator).is_err());
        }
    }
}
#[test]
fn lexical_budget_charges_capacity_canonical_peak_and_existing_ceilings() {
    let sources = source(&" ".repeat(1048576));
    let file = sources.get(SourceFileId(0));
    let mut budget = Budget::new(0, 0).unwrap();
    let inventory = Inventory::default();
    let plan = budget.admit(file, 100000, &inventory).unwrap();
    assert!(plan.scratch > 12_000_000);
    assert!(plan.scratch < 16 * 1024 * 1024);
    assert!(plan.retained >= 100001 * size_of::<Token>());
    assert!(plan.work > 90_000_000);
    for which in 0..3 {
        let mut budget = Budget::new(0, 0).unwrap();
        match which {
            0 => budget.limits.retained = 0,
            1 => budget.limits.scratch = 0,
            _ => budget.limits.work = 0,
        };
        assert!(budget.admit(file, 100000, &inventory).is_err());
    }
    let mut budget = Budget::new(0, 0).unwrap();
    budget.selected_capacity = 32 * 1024 * 1024;
    assert!(budget.admit(file, 100000, &inventory).is_err());
}

struct Files(std::path::PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oxid-lexical-loader-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path.to_str().unwrap().to_string()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn lexical_loader_moves_actual_provider_allocation_into_root_parser_on_every_host() {
    let files = Files::new();
    let root = files.write("main.ox", "fn main() -> i32 { return 42; }");
    let mut provider = fixture(encode);
    let project =
        ProjectSources::load_typed_with_provider(&root, ProjectLimits::default(), &mut provider)
            .unwrap();
    assert_eq!(project.sources().files().len(), 1);
    assert_eq!(provider.receipts.len(), 1);
    let receipt = &provider.receipts[0];
    assert_eq!(receipt.file, SourceFileId(0));
    assert_eq!(receipt.stop, "test-observation");
    assert!(!receipt.spawned);
    assert!(
        receipt.comparison_attempted && receipt.comparison_matched && receipt.selected_for_parser
    );
    let program = project.try_file_ast(receipt.file).unwrap();
    assert_eq!(program.tokens.capacity(), program.tokens.len());
    assert_eq!(
        program.tokens.as_ptr() as usize,
        receipt.producer_allocation
    );
    assert_eq!(
        project.sources().get(receipt.file).identity(),
        receipt.identity
    );
}

#[cfg(target_os = "linux")]
#[test]
fn lexical_loader_moves_actual_provider_allocation_into_each_module_parser() {
    let files = Files::new();
    let root = files.write(
        "main.ox",
        "mod child; fn main() -> i32 { return child::value(); }",
    );
    files.write("child.ox", "pub fn value() -> i32 { return 42; }");
    let mut provider = fixture(encode);
    let project =
        ProjectSources::load_typed_with_provider(&root, ProjectLimits::default(), &mut provider)
            .unwrap();
    assert_eq!(provider.receipts.len(), 2);
    for receipt in &provider.receipts {
        assert!(
            receipt.comparison_attempted
                && receipt.comparison_matched
                && receipt.selected_for_parser
        );
        let program = project.try_file_ast(receipt.file).unwrap();
        assert_eq!(program.tokens.capacity(), program.tokens.len());
        assert_eq!(
            program.tokens.as_ptr() as usize,
            receipt.producer_allocation
        );
        assert_eq!(
            project.sources().get(receipt.file).identity(),
            receipt.identity
        );
    }
}

#[cfg(not(target_os = "linux"))]
#[test]
fn lexical_loader_declared_child_refuses_unqualified_host_without_child_fallback() {
    let files = Files::new();
    let root = files.write("main.ox", "mod child;");
    files.write("child.ox", "pub fn value() -> i32 { return 42; }");
    let mut provider = fixture(encode);
    let failure =
        ProjectSources::load_typed_with_provider(&root, ProjectLimits::default(), &mut provider)
            .unwrap_err();
    assert_eq!(failure.diagnostics.len(), 1);
    let diagnostic = &failure.diagnostics[0];
    assert_eq!((diagnostic.code, diagnostic.stage), ("E0005", "source"));
    assert_eq!(
        diagnostic.message,
        "module source policy is not qualified on this host"
    );
    assert_eq!(failure.sources.text(diagnostic.primary.unwrap()), "child");
    assert_eq!(failure.sources.files().len(), 1);
    assert_eq!(failure.usage.modules, 1);
    assert_eq!(failure.usage.probes, 0);
    assert_eq!(provider.receipts.len(), 1);
    let receipt = &provider.receipts[0];
    assert_eq!(receipt.file, SourceFileId(0));
    assert_eq!(receipt.stop, "test-observation");
    assert!(!receipt.spawned);
    assert!(
        receipt.comparison_attempted && receipt.comparison_matched && receipt.selected_for_parser
    );
    assert_eq!(
        failure.sources.get(receipt.file).identity(),
        receipt.identity
    );
}

fn corrupted_tokens(source: &SourceFile, limit: usize) -> Vec<u8> {
    let mut bytes = encode(source, limit);
    // First token starts in the final partial group for the short source below.
    let first = 138;
    assert_eq!(bytes[first], b'B');
    bytes[first + 2] = 2; // fn -> ident, same byte span, valid framing.
    bytes
}
#[test]
fn lexical_loader_mismatch_refuses_before_parser_without_canonical_fallback() {
    let files = Files::new();
    let root = files.write("main.ox", "fn main() -> i32 { return 42; }");
    let mut provider = fixture(corrupted_tokens);
    let failure =
        ProjectSources::load_typed_with_provider(&root, ProjectLimits::default(), &mut provider)
            .unwrap_err();
    assert_eq!(failure.diagnostics[0].code, "E0703");
    assert!(failure.diagnostics[0].message.contains("canonical tokens"));
    let receipt = &provider.receipts[0];
    assert!(
        receipt.comparison_attempted && !receipt.comparison_matched && !receipt.selected_for_parser
    );
    assert_eq!(failure.sources.files().len(), 1);
}
#[test]
fn lexical_loader_diagnostic_and_parser_failure_keep_receipts() {
    for (text, code, selected) in [("\"unterminated", "E0100", false), ("fn", "E0101", true)] {
        let files = Files::new();
        let root = files.write("main.ox", text);
        let mut provider = fixture(encode);
        let failed = ProjectSources::load_typed_with_provider(
            &root,
            ProjectLimits::default(),
            &mut provider,
        )
        .unwrap_err();
        if selected {
            assert_eq!(failed.diagnostics[0].stage, "parse");
        } else {
            assert_eq!(failed.diagnostics[0].code, code);
        }
        let receipt = &provider.receipts[0];
        assert!(receipt.comparison_attempted && receipt.comparison_matched);
        assert_eq!(receipt.selected_for_parser, selected);
    }
}
#[cfg(target_os = "linux")]
#[test]
fn lexical_loader_passes_remaining_aggregate_limit_and_accepts_exact_diagnostic() {
    let files = Files::new();
    let root_text = "mod child;";
    let root = files.write("main.ox", root_text);
    files.write("child.ox", "fn value() -> i32 { return 1; }");
    let root_sources = source(root_text);
    let root_tokens = lexer::lex(root_sources.get(SourceFileId(0))).unwrap().len() - 1;
    let mut provider = fixture(encode);
    let limits = ProjectLimits {
        tokens: root_tokens,
        ..ProjectLimits::default()
    };
    let failed =
        ProjectSources::load_typed_with_provider(&root, limits, &mut provider).unwrap_err();
    assert_eq!(failed.diagnostics[0].code, "E0400");
    assert_eq!(failed.diagnostics[0].stage, "lex");
    assert_eq!(provider.receipts.len(), 2);
    assert!(provider.receipts[1].comparison_matched);
    assert!(!provider.receipts[1].selected_for_parser);
}

struct ForeignProvider {
    other: SourceMap,
    selected: bool,
    matched: bool,
}
impl LexicalProvider for ForeignProvider {
    fn begin_module(&mut self, _: &SourceFile, _: SourceUsage) -> Result<(), Box<Diagnostic>> {
        Ok(())
    }
    fn observe(
        &mut self,
        source: &SourceFile,
        limit: usize,
        _: &Inventory,
        _: &mut Allocator,
    ) -> Result<LexicalObservation, Box<Diagnostic>> {
        let tokens = lexer::lex_with_limit(source, limit);
        Ok(LexicalObservation::new(
            self.other.get(SourceFileId(0)),
            tokens,
        ))
    }
    fn comparison_started(&mut self) {}
    fn comparison_finished(&mut self, matched: bool) {
        self.matched = matched;
    }
    fn selected_for_parser(
        &mut self,
        _: &SourceFile,
        _: &[Token],
        _: usize,
    ) -> Result<(), Box<Diagnostic>> {
        self.selected = true;
        Ok(())
    }
}
#[test]
fn lexical_loader_consuming_carrier_rejects_wrong_identity_same_path_id_length() {
    let files = Files::new();
    let text = "fn main() -> i32 { return 42; }";
    let root = files.write("main.ox", text);
    let mut other = SourceMap::new();
    other.add(root.clone(), text.into());
    let mut provider = ForeignProvider {
        other,
        selected: false,
        matched: false,
    };
    let failed =
        ProjectSources::load_typed_with_provider(&root, ProjectLimits::default(), &mut provider)
            .unwrap_err();
    assert_eq!(failed.diagnostics[0].code, "E0703");
    assert!(failed.diagnostics[0]
        .message
        .contains("another retained source"));
    assert!(!provider.selected && !provider.matched);
}

#[test]
fn lexical_provider_new_input_token_and_receipt_reserves_fail_closed() {
    let sources = source("fn main() -> i32 { return 42; }");
    let file = sources.get(SourceFileId(0));
    for ordinal in [1, 2] {
        let mut provider = fixture(encode);
        let mut allocator = Allocator {
            fail_at: Some(ordinal),
            ..Allocator::default()
        };
        provider.begin_module(file, SourceUsage::default()).unwrap();
        let error = provider
            .observe(file, 100000, &Inventory::default(), &mut allocator)
            .err()
            .expect("fallible allocation refusal");
        assert_eq!(error.code, "E0703");
        assert!(error.message.contains("allocation"));
        assert_eq!(provider.receipts.len(), 1);
        assert!(!provider.receipts[0].comparison_attempted);
        assert!(!provider.receipts[0].selected_for_parser);
    }
    let mut allocator = Allocator {
        fail_at: Some(1),
        ..Allocator::default()
    };
    assert!(wire::reserve(
        &mut Vec::<Receipt>::new(),
        RECEIPTS,
        &mut allocator,
        "lexical provider receipts"
    )
    .is_err());
}
fn wrong_diagnostic(source: &SourceFile, limit: usize) -> Vec<u8> {
    let mut bytes = encode(source, limit);
    let n = bytes.len();
    assert_eq!(bytes[n - 10], b'D');
    bytes[n - 9] = 2;
    bytes
}
#[test]
fn lexical_loader_structurally_valid_wrong_diagnostic_fails_comparison() {
    let files = Files::new();
    let root = files.write("main.ox", "\"unfinished");
    let mut provider = fixture(wrong_diagnostic);
    let failed =
        ProjectSources::load_typed_with_provider(&root, ProjectLimits::default(), &mut provider)
            .unwrap_err();
    assert_eq!(failed.diagnostics[0].code, "E0703");
    assert!(failed.diagnostics[0]
        .message
        .contains("canonical diagnostic"));
    assert!(
        provider.receipts[0].comparison_attempted
            && !provider.receipts[0].comparison_matched
            && !provider.receipts[0].selected_for_parser
    );
}
#[test]
fn lexical_wire_rejects_noncanonical_counters_eof_base_padding_and_sizes() {
    let sources = source("x");
    let file = sources.get(SourceFileId(0));
    let bytes = encode(file, 100);
    // One E, one B(two pairs), S. Exact record offsets are part of the contract.
    assert_eq!(bytes.len(), 249);
    for (offset, value) in [
        (4, 0),
        (5, 0),
        (6, 1),
        (138, b'E'),
        (139, 0),
        (139, 1),
        (139, 3),
        (139, 100),
        (140, 0),
        (140, 48),
        (141, 0),
        (141, 129),
        (142, 2),
        (143, 0),
        (144, 1),
        (240, b'D'),
        (241, 1),
        (245, 2),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(
            decode(file, &bad, 100).is_err(),
            "mutation {offset}={value}"
        );
    }
    // Every emitted byte in later echo padding must preserve the previous read.
    let sources = source(&format!("{}x", " ".repeat(128)));
    let file = sources.get(SourceFileId(0));
    let bytes = encode(file, 100);
    let second_echo = 138 + 102;
    assert_eq!(bytes[second_echo], b'E');
    let mut bad = bytes.clone();
    bad[second_echo + 7] ^= 1;
    assert!(decode(file, &bad, 100).is_err());
}

#[test]
fn lexical_receipts_escape_all_terminal_control_characters() {
    let input = "path\u{1b}[31m\n\u{7f}\u{85}\u{9b}é";
    let encoded = format!("{}", JsonText(input));
    assert!(!encoded.chars().any(char::is_control));
    assert!(encoded.contains("\\u001b"));
    assert!(encoded.contains("\\u009b"));
    assert!(encoded.ends_with("é\""));
}
#[test]
fn lexical_budget_prepays_failure_receipts_and_reports_full_carriers() {
    let sources = source(&" ".repeat(1048576));
    let file = sources.get(SourceFileId(0));
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let extra = bundle::named_bytes() + supervisor::named_bytes();
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    let extra = 0;
    let mut budget = Budget::new(4096, extra).unwrap();
    let initial = budget.work;
    budget.admit_receipt(file).unwrap();
    assert_eq!(budget.work, initial);
    assert!(budget.receipt_work_left < RECEIPT_WORK);
    let plan = budget.admit(file, 100000, &Inventory::default()).unwrap();
    assert!(budget.work < 256_000_000);
    println!("LEXICAL_BUDGET token_bytes={} receipt_bytes={} fixed_retained={} fixed_scratch={} max_source_retained={} max_source_scratch={} receipt_work={} source_work={} total_work={} canonical_capacity={} canonical_growth_peak={}",
        size_of::<Token>(), size_of::<Receipt>(), budget.fixed_retained, budget.fixed_scratch,
        plan.retained, plan.scratch, RECEIPT_WORK, plan.work, budget.work,
        131072 * size_of::<Token>(), 196608 * size_of::<Token>());
}

#[test]
fn lexical_budget_refuses_inventory_work_before_traversal_and_keeps_receipt() {
    let sources = source("x");
    let file = sources.get(SourceFileId(0));
    let mut provider = fixture(encode);
    provider.budget.limits.work = provider.budget.work;
    assert!(provider.begin_module(file, SourceUsage::default()).is_err());
    assert_eq!(provider.receipts.len(), 1);
    assert_eq!(provider.receipts[0].stop, "host-refused");
    assert!(
        !provider.receipts[0].comparison_attempted && !provider.receipts[0].selected_for_parser
    );
}
