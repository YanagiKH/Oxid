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
pub(super) fn fixture(encoder: fn(&SourceFile, usize) -> Vec<u8>) -> Provider {
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

// Frozen from explicit protocol numbers and hand-derived source extents, never
// from encode() or the canonical lexer. One final E echo, one B partial group,
// then S(13,22) or D(2,1,3); all unused echo/group bytes remain zero.
const FROZEN_RESERVATION_TOKENS: [u8; 249] = [
    0x4c, 0x58, 0x53, 0x31, 0x45, 0x16, 0x00, 0x00, 0x00, 0x00, 0x66, 0x6e, 0x20, 0x6d, 0x61, 0x69,
    0x6e, 0x28, 0x29, 0x2d, 0x3e, 0x28, 0x29, 0x7b, 0x72, 0x65, 0x74, 0x75, 0x72, 0x6e, 0x3b, 0x7d,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x1a, 0x05, 0x02, 0x01, 0x03,
    0x02, 0x07, 0x16, 0x08, 0x17, 0x09, 0x27, 0x0b, 0x16, 0x0c, 0x17, 0x0d, 0x18, 0x0e, 0x0e, 0x14,
    0x1c, 0x15, 0x19, 0x16, 0x2f, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x53, 0x0d, 0x00, 0x00, 0x00, 0x16, 0x00, 0x00, 0x00,
];
pub(super) fn frozen_reservation_tokens(source: &SourceFile, limit: usize) -> Vec<u8> {
    assert_eq!(source.text(), ReservationFixture::Tokens.source());
    assert_eq!(limit, 100000);
    let captured = FROZEN_RESERVATION_TOKENS.to_vec();
    assert_eq!(captured.capacity(), FROZEN_RESERVATION_TOKENS.len());
    captured
}
const FROZEN_RESERVATION_DIAGNOSTIC: [u8; 250] = [
    0x4c, 0x58, 0x53, 0x31, 0x45, 0x03, 0x00, 0x00, 0x00, 0x00, 0x3b, 0x2f, 0x2a, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x42, 0x02, 0x1c, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x44, 0x02, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
];
pub(super) fn frozen_reservation_diagnostic(source: &SourceFile, limit: usize) -> Vec<u8> {
    assert_eq!(
        source.text(),
        ReservationFixture::UnterminatedComment.source()
    );
    assert_eq!(limit, 100000);
    let captured = FROZEN_RESERVATION_DIAGNOSTIC.to_vec();
    assert_eq!(captured.capacity(), FROZEN_RESERVATION_DIAGNOSTIC.len());
    captured
}

#[test]
fn lexical_reservation_wire_independent_freeze() {
    for (fixture, bytes) in [
        (
            ReservationFixture::Tokens,
            FROZEN_RESERVATION_TOKENS.as_slice(),
        ),
        (
            ReservationFixture::UnterminatedComment,
            FROZEN_RESERVATION_DIAGNOSTIC.as_slice(),
        ),
    ] {
        // Hash verification does not execute canonical lexing, decoding, capture,
        // parsing or a selector. The input fixture is assembled from LXI1 fields.
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(fixture.source().as_bytes())),
            fixture.source_sha256()
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(bytes)),
            fixture.stdout_sha256()
        );
        assert_eq!(bytes.len(), fixture.stdout_bytes());
        let mut input = Sha256::new();
        input.update(b"LXI1");
        input.update((fixture.source().len() as u32).to_le_bytes());
        input.update(100000u32.to_le_bytes());
        input.update(fixture.source().as_bytes());
        assert_eq!(<[u8; 32]>::from(input.finalize()), fixture.input_sha256());
        assert_eq!(fixture.input_bytes(), fixture.source().len() + 12);
    }
}

#[test]
fn lexical_reservation_bridge_layout_measurement_only() {
    use std::mem::{align_of, offset_of};
    let provider = Provider::reservation_fixture(ReservationFixture::Tokens);
    let layout = provider.reservation_layout();
    println!("concrete-provider-layout {layout:?} snapshot={} snapshot-align={} snapshot-option={} layout={} layout-align={} fixture={} fixture-align={}",
        size_of::<ReservationReceipt>(), align_of::<ReservationReceipt>(), size_of::<Option<ReservationReceipt>>(),
        size_of::<ReservationLayout>(), align_of::<ReservationLayout>(), size_of::<ReservationFixture>(), align_of::<ReservationFixture>());
    println!("concrete-provider-components provider={}/{} receipt={}/{} budget={} plan={} wire-bank={} provider-scratch={} receipts-requested={} receipts-retained={}",
        layout.provider, layout.provider_align, layout.receipt, layout.receipt_align, layout.budget,
        layout.plan, layout.wire_bank, layout.provider_scratch, layout.receipts_requested, layout.receipts_retained);
    macro_rules! fields {
        ($ty:ty; $($field:ident),+ $(,)?) => {
            $(println!("concrete-provider-field {}.{}={}", stringify!($ty), stringify!($field), offset_of!($ty, $field));)+
        };
    }
    fields!(Provider; receipts, budget, fixture);
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fields!(Provider; bundle);
    fields!(Receipt; file, identity, source_len, source_sha256, executable_sha256, input_sha256, stdout_sha256, stderr_sha256,
        input_bytes, input_written, stdout_bytes, stderr_bytes, status, signal, stop, spawned, leader_reaped, stdin_closed,
        stdout_eof, stderr_eof, comparison_attempted, comparison_matched, selected_for_parser, producer_allocation);
    fields!(ReservationReceipt; file, identity, source_len, source_sha256, executable_sha256, input_sha256, stdout_sha256, stderr_sha256,
        input_bytes, input_written, stdout_bytes, stderr_bytes, status, signal, stop, spawned, leader_reaped, stdin_closed,
        stdout_eof, stderr_eof, comparison_attempted, comparison_matched, selected_for_parser);
    fields!(ReservationLayout; provider, provider_align, receipt, receipt_align, budget, plan, wire_bank, provider_scratch, receipts_requested, receipts_retained);
    fields!(Budget; limits, work, selected_capacity, receipt_work_left, fixed_retained, fixed_scratch);
    fields!(Plan; output, retained, scratch, work);
    assert_eq!(provider.reservation_receipt_count(), 0);
    assert_eq!(provider.reservation_receipt(), None);
    assert_eq!(layout.receipts_requested, layout.receipts_retained);
}

// Independently frozen before these tests were written; see the endpoint
// evidence inventory and the predecessor semantic/resource review. Expectations
// use the source formula and existing literal wire, never a successful Plan.
const ENDPOINT_RECEIPT_WORK: u64 = 42_991_616;
const ENDPOINT_INVENTORY_WORK: u64 = 168;
const ENDPOINT_DYNAMIC_WORK: u64 = 7_968;

fn endpoint_fixed_banks() -> (usize, usize) {
    let retained = size_of::<Provider>()
        + size_of::<crate::frontend::options::LexicalOptions>()
        + 256 * size_of::<Receipt>();
    let lexer_bank = 3 * size_of::<lexer::Failure>()
        + 3 * size_of::<Result<Vec<Token>, lexer::Failure>>()
        + 3 * size_of::<Token>()
        + 3 * size_of::<Vec<Token>>()
        + 32 * size_of::<usize>()
        + 8 * size_of::<&str>()
        + size_of::<Allocator>();
    // These two subbanks measure source-private types. Their source-defined
    // roles are inventoried separately; no guessed layout mirrors are used.
    let scratch = wire::named_bytes()
        + lexer_bank
        + crate::frontend::project::lexical_comparison_scratch_bytes()
        + 3 * size_of::<LexicalObservation>()
        + 3 * size_of::<Receipt>()
        + 3 * size_of::<Budget>()
        + 3 * size_of::<Plan>()
        + 3 * size_of::<Diagnostic>()
        + 3 * size_of::<[u8; 1024]>()
        + size_of::<[usize; 64]>()
        + 4 * size_of::<Sha256>()
        + 3 * size_of::<Option<&mut dyn LexicalProvider>>();
    (retained, scratch)
}

fn endpoint_inventory() -> Inventory {
    Inventory {
        owner_bytes: 1,
        source_bytes: 2,
        source_headers: 4,
        line_starts: 8,
        // Logical old token length must not replace/add to selected capacity.
        tokens: 1009,
        ast_headers: 16,
        ast_payload: 32,
        module_headers: 64,
        path_bytes: 128,
        fixed_loader_scratch: 257,
    }
}

fn endpoint_usage() -> SourceUsage {
    SourceUsage {
        syntax_nodes: 11,
        non_eof_tokens: 7,
        modules: 2,
        ..SourceUsage::default()
    }
}

fn endpoint_budget() -> Budget {
    let mut budget = Budget::new(31, 37).unwrap();
    budget.selected_capacity = 3 * size_of::<Token>();
    budget
}

fn endpoint_demands() -> (usize, usize, u64) {
    let (retained, scratch) = endpoint_fixed_banks();
    // Source=22, bound=398, possible slots=23, requested W=32. This
    // intentionally prices 23 possible slots, not the frozen 13 decoded tokens.
    (
        retained + 31 + 255 + 3 * size_of::<Token>() + 23 * size_of::<Token>(),
        scratch + 37 + 257 + 34 + 399 + 23 * size_of::<Token>() + 48 * size_of::<Token>(),
        ENDPOINT_RECEIPT_WORK + ENDPOINT_INVENTORY_WORK + ENDPOINT_DYNAMIC_WORK,
    )
}

#[derive(Clone, Copy, Debug)]
enum BudgetEndpoint {
    Retained,
    Scratch,
    Work,
}
impl BudgetEndpoint {
    fn lower_limit(self, budget: &mut Budget, short: u64) {
        let (retained, scratch, work) = endpoint_demands();
        match self {
            Self::Retained => budget.limits.retained = retained as u64 - short,
            Self::Scratch => budget.limits.scratch = scratch as u64 - short,
            Self::Work => budget.limits.work = work - short,
        }
        assert!(budget.limits.retained <= 32 * 1024 * 1024);
        assert!(budget.limits.scratch <= 16 * 1024 * 1024);
        assert!(budget.limits.work <= 256_000_000);
    }

    fn refusal(self) -> &'static str {
        match self {
            Self::Retained => "lexical provider retained byte limit exceeded",
            Self::Scratch => "lexical provider scratch byte limit exceeded",
            Self::Work => "lexical provider work limit exceeded",
        }
    }
}

fn assert_endpoint_budget_state(budget: &Budget, before: Budget, admitted: bool) {
    assert_eq!(
        budget.work,
        before.work + if admitted { ENDPOINT_DYNAMIC_WORK } else { 0 }
    );
    assert_eq!(budget.receipt_work_left, before.receipt_work_left);
    assert_eq!(budget.selected_capacity, before.selected_capacity);
    assert_eq!(budget.fixed_retained, before.fixed_retained);
    assert_eq!(budget.fixed_scratch, before.fixed_scratch);
    assert_eq!(budget.limits.retained, before.limits.retained);
    assert_eq!(budget.limits.scratch, before.limits.scratch);
    assert_eq!(budget.limits.work, before.limits.work);
}

#[test]
fn lexical_budget_fixed_storage_exact_and_one_over_from_typed_roles() {
    let limits = IndexLimits::default();
    assert_eq!(limits.retained, 32 * 1024 * 1024);
    assert_eq!(limits.scratch, 16 * 1024 * 1024);
    assert_eq!(limits.work, 256_000_000);
    assert_eq!(RECEIPTS, 256);
    assert_eq!(RECEIPT_WORK, ENDPOINT_RECEIPT_WORK);
    let (retained, scratch) = endpoint_fixed_banks();
    let retained_extra = (32 * 1024 * 1024usize).checked_sub(retained).unwrap();
    let scratch_extra = (16 * 1024 * 1024usize).checked_sub(scratch).unwrap();
    for (extra_retained, extra_scratch) in [
        (retained_extra, 0),
        (0, scratch_extra),
        (retained_extra, scratch_extra),
    ] {
        let budget = Budget::new(extra_retained, extra_scratch).unwrap();
        assert_eq!(budget.fixed_retained, retained + extra_retained);
        assert_eq!(budget.fixed_scratch, scratch + extra_scratch);
        assert_eq!(budget.work, ENDPOINT_RECEIPT_WORK);
        assert_eq!(budget.receipt_work_left, ENDPOINT_RECEIPT_WORK);
        assert_eq!(budget.selected_capacity, 0);
    }
    for (extra_retained, extra_scratch) in [
        (retained_extra + 1, 0),
        (0, scratch_extra + 1),
        (retained_extra + 1, scratch_extra),
        (retained_extra, scratch_extra + 1),
    ] {
        assert_eq!(
            Budget::new(extra_retained, extra_scratch).err(),
            Some("lexical provider fixed storage limit exceeded")
        );
    }
}

fn check_budget_endpoint(endpoint: BudgetEndpoint) {
    let sources = source("fn main()->(){return;}");
    let file = sources.get(SourceFileId(0));
    assert_eq!(file.text(), ReservationFixture::Tokens.source());
    assert_eq!(file.text().len(), 22);
    let inventory = endpoint_inventory();
    let (retained, scratch, work) = endpoint_demands();
    for short in [0, 1] {
        let mut budget = endpoint_budget();
        endpoint.lower_limit(&mut budget, short);
        budget.admit_inventory_work(endpoint_usage()).unwrap();
        assert_eq!(budget.work, ENDPOINT_RECEIPT_WORK + ENDPOINT_INVENTORY_WORK);
        let before = budget;
        let result = budget.admit(file, 100000, &inventory);
        if short == 0 {
            let plan = result.unwrap();
            assert_eq!(plan.output, 398);
            assert_eq!(plan.retained, retained);
            assert_eq!(plan.scratch, scratch);
            assert_eq!(plan.work, ENDPOINT_DYNAMIC_WORK);
            assert_eq!(budget.work, work);
        } else {
            assert_eq!(result.unwrap_err(), endpoint.refusal());
        }
        assert_endpoint_budget_state(&budget, before, short == 0);
    }
}

#[test]
fn lexical_budget_retained_exact_and_one_short_from_independent_fixture() {
    check_budget_endpoint(BudgetEndpoint::Retained);
}

#[test]
fn lexical_budget_scratch_exact_and_one_short_from_independent_fixture() {
    check_budget_endpoint(BudgetEndpoint::Scratch);
}

#[test]
fn lexical_budget_total_work_exact_and_one_short_from_independent_fixture() {
    check_budget_endpoint(BudgetEndpoint::Work);
}

#[test]
fn lexical_provider_budget_exact_and_one_short_preflight_preserves_receipt() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static CAPTURES: AtomicUsize = AtomicUsize::new(0);
    fn capture(source: &SourceFile, limit: usize) -> Vec<u8> {
        CAPTURES.fetch_add(1, Ordering::SeqCst);
        frozen_reservation_tokens(source, limit)
    }

    let sources = source("fn main()->(){return;}");
    let file = sources.get(SourceFileId(0));
    assert_eq!(file.text(), ReservationFixture::Tokens.source());
    let inventory = endpoint_inventory();
    let empty_hash: [u8; 32] = Sha256::digest([]).into();
    for endpoint in [
        BudgetEndpoint::Retained,
        BudgetEndpoint::Scratch,
        BudgetEndpoint::Work,
    ] {
        for short in [0, 1] {
            let mut provider = fixture(capture);
            provider.budget = endpoint_budget();
            endpoint.lower_limit(&mut provider.budget, short);
            provider.begin_module(file, endpoint_usage()).unwrap();
            assert_eq!(provider.receipts.len(), 1);
            let receipts_capacity = provider.receipts.capacity();
            let mut expected = provider.reservation_receipt().unwrap();
            assert_eq!(expected.file, SourceFileId(0));
            assert_eq!(expected.identity, file.identity());
            assert_eq!(expected.source_len, 22);
            assert_eq!(
                expected.source_sha256,
                ReservationFixture::Tokens.source_sha256()
            );
            assert_eq!(expected.executable_sha256, [0; 32]);
            assert_eq!(expected.input_sha256, empty_hash);
            assert_eq!(expected.stdout_sha256, empty_hash);
            assert_eq!(expected.stderr_sha256, empty_hash);
            assert_eq!(expected.input_bytes, 0);
            assert_eq!(expected.input_written, 0);
            assert_eq!(expected.stdout_bytes, 0);
            assert_eq!(expected.stderr_bytes, 0);
            assert_eq!(expected.status, None);
            assert_eq!(expected.signal, None);
            assert_eq!(expected.stop, "not-started");
            assert!(!expected.spawned && !expected.leader_reaped);
            assert!(!expected.stdin_closed && !expected.stdout_eof && !expected.stderr_eof);
            assert!(!expected.comparison_attempted && !expected.comparison_matched);
            assert!(!expected.selected_for_parser);
            assert_eq!(provider.receipts[0].producer_allocation, 0);
            assert_eq!(
                provider.budget.work,
                ENDPOINT_RECEIPT_WORK + ENDPOINT_INVENTORY_WORK
            );
            assert_eq!(
                provider.budget.receipt_work_left,
                ENDPOINT_RECEIPT_WORK - 4328
            );
            let before = provider.budget;
            let captures_before = CAPTURES.load(Ordering::SeqCst);
            let mut allocator = Allocator {
                // A rejected plan must not reach even the first input request.
                fail_at: (short == 1).then_some(1),
                ..Allocator::default()
            };
            let result = provider.observe(file, 100000, &inventory, &mut allocator);
            if short == 0 {
                let _observation = result.unwrap_or_else(|error| panic!("{endpoint:?}: {error:?}"));
                assert_eq!(CAPTURES.load(Ordering::SeqCst), captures_before + 1);
                assert_eq!(allocator.attempts, 2);
                assert_eq!(allocator.trace.len(), 2);
                for (event, (kind, length, element_bytes)) in allocator.trace.iter().zip([
                    ("lexical provider input", 34, 1),
                    ("lexical provider tokens", 13, size_of::<Token>()),
                ]) {
                    assert_eq!(
                        (event.kind, event.length, event.element_bytes),
                        (kind, length, element_bytes)
                    );
                    assert!(event.success);
                }
                expected.input_sha256 = ReservationFixture::Tokens.input_sha256();
                expected.stdout_sha256 = ReservationFixture::Tokens.stdout_sha256();
                expected.input_bytes = 34;
                expected.input_written = 34;
                expected.stdout_bytes = 249;
                expected.stop = "test-observation";
                assert_ne!(provider.receipts[0].producer_allocation, 0);
            } else {
                let error = result.err().expect("one-short provider preflight refusal");
                assert_eq!(error.code, "E0703");
                assert_eq!(error.stage, "lexical-provider");
                assert_eq!(error.message, endpoint.refusal());
                assert_eq!(error.primary, Some(file.span(0, 0)));
                assert!(error.secondary.is_empty() && error.notes.is_empty());
                assert_eq!(allocator.attempts, 0);
                assert!(allocator.trace.is_empty());
                assert_eq!(CAPTURES.load(Ordering::SeqCst), captures_before);
                expected.stop = "host-refused";
                assert_eq!(provider.receipts[0].producer_allocation, 0);
            }
            assert!(!allocator.observer_trace_overflow);
            assert_endpoint_budget_state(&provider.budget, before, short == 0);
            assert_eq!(provider.receipts.len(), 1);
            assert_eq!(provider.receipts.capacity(), receipts_capacity);
            assert_eq!(provider.reservation_receipt(), Some(expected));
        }
    }
}
