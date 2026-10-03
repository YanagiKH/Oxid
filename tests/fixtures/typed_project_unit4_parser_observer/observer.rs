//! Additive test-only collector. No expectations or source-grammar inference.
use super::*;
use crate::frontend::{
    diagnostic::json_string as q,
    source::{SourceMap, Span},
};
use std::cell::RefCell;

const EVENT_LIMIT: usize = 1_000_000;
const BYTE_LIMIT: usize = 134_217_728;
#[derive(Default)]
struct State {
    enabled: bool,
    events: Vec<String>,
    transitions: Vec<String>,
    scans: Vec<String>,
    current_reads: Vec<String>,
    tokens: Vec<Token>,
    bytes: usize,
    recovery: bool,
    position: Option<(usize, Token)>,
    parse_attempts: usize,
    initial: Option<bool>,
    final_bit: Option<bool>,
    nodes: Option<usize>,
    scan_indices: Vec<usize>,
    scan_start: Option<(usize, usize, u64, usize, usize)>,
    source_bytes_read: usize,
    namespace_units: u64,
    reserve_calls: usize,
    append_calls: usize,
    reject_kind: String,
    reject_occurrence: usize,
    reject_seen: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
pub(in crate::frontend) fn provenance_handle(p: &SourceProvenance) -> (u64, usize, usize) {
    (p.identity, p.text_len, p.file.0)
}
fn span(s: Span) -> String {
    format!("[{},{},{}]", s.file.0, s.start, s.end)
}
fn charge(s: &mut State, n: usize) {
    s.bytes = s
        .bytes
        .checked_add(n)
        .expect("OBSERVATION_LIMIT counter overflow");
    assert!(
        s.bytes <= BYTE_LIMIT,
        "OBSERVATION_LIMIT evidence byte budget"
    );
    assert!(
        s.events.len() < EVENT_LIMIT,
        "OBSERVATION_LIMIT event budget"
    );
}
fn emit(
    s: &mut State,
    kind: &str,
    production: &str,
    cursor: usize,
    token: Token,
    detail: String,
) -> String {
    if kind == "reserve" {
        s.reserve_calls += 1;
    }
    if kind == "append" {
        s.append_calls += 1;
    }
    let current = *s
        .tokens
        .get(cursor)
        .expect("event cursor outside observed token tape");
    let row=format!("{{\"cursor_span\":{},\"cursor_token_kind\":{},\"seq\":{},\"kind\":{},\"production\":{},\"cursor\":{},\"token_kind\":{},\"span\":{},\"context\":{},\"detail\":{}}}",span(current.span),q(&format!("{:?}",current.kind)),s.events.len(),q(kind),q(production),cursor,q(&format!("{:?}",token.kind)),span(token.span),q(if s.recovery {"recovery"}else{"grammar"}),detail);
    charge(s, row.len());
    s.events.push(row.clone());
    s.position = Some((cursor, current));
    row
}
pub(in crate::frontend) fn event(
    kind: &str,
    production: &str,
    cursor: usize,
    token: Token,
    detail: String,
) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            emit(&mut s, kind, production, cursor, token, detail);
        }
    });
}
pub(in crate::frontend) fn position(cursor: usize, token: Token) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            s.position = Some((cursor, token));
        }
    });
}
pub(in crate::frontend) fn inspect(cursor: usize, token: Token) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            if s.recovery {
                emit(
                    &mut s,
                    "recover_inspect",
                    "recovery",
                    cursor,
                    token,
                    "null".into(),
                );
            } else {
                s.position = Some((cursor, token));
            }
        }
    });
}
pub(in crate::frontend) fn parser_enter(initial: bool) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            s.parse_attempts += 1;
            s.initial = Some(initial);
        }
    });
}
pub(in crate::frontend) fn parser_return(nodes: usize, recognized: bool) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            s.nodes = Some(nodes);
            s.final_bit = Some(recognized);
        }
    });
}
pub(in crate::frontend) fn recognize(
    before: bool,
    after: bool,
    trigger: &str,
    cursor: usize,
    token: Token,
) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            let row = emit(
                &mut s,
                if !before && after {
                    "recognize"
                } else if before && after {
                    "recognize_already_true"
                } else {
                    "recognize_reset"
                },
                trigger,
                cursor,
                token,
                format!("{{\"before\":{before},\"after\":{after}}}"),
            );
            if !before && after {
                s.transitions.push(row);
            }
        }
    });
}
pub(in crate::frontend) fn recovery(enter: bool, cursor: usize, token: Token) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            if enter {
                s.recovery = true;
            }
            emit(
                &mut s,
                if enter {
                    "recover_enter"
                } else {
                    "recover_exit"
                },
                "recovery",
                cursor,
                token,
                "null".into(),
            );
            if !enter {
                s.recovery = false;
            }
        }
    });
}
pub(in crate::frontend) fn reject_node(kind: &str) -> bool {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled && s.reject_kind == kind {
            s.reject_seen += 1;
            s.reject_seen == s.reject_occurrence
        } else {
            false
        }
    })
}
pub(in crate::frontend) fn field_current_read(cursor: usize, token: Token) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            let row = format!(
                "{{\"cursor\":{cursor},\"token_kind\":{},\"span\":{}}}",
                q(&format!("{:?}", token.kind)),
                span(token.span)
            );
            charge(&mut s, row.len());
            s.current_reads.push(row);
        }
    });
}
pub(in crate::frontend) fn source_read(bytes: usize) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            s.source_bytes_read = s
                .source_bytes_read
                .checked_add(bytes)
                .expect("source-read count overflow");
        }
    });
}
pub(in crate::frontend) fn namespace_debit(units: u64) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            s.namespace_units = s
                .namespace_units
                .checked_add(units)
                .expect("namespace count overflow");
        }
    });
}
pub(in crate::frontend) fn field_scan_begin(cursor: usize, token: Token) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            assert!(s.scan_start.is_none());
            let seq = s.events.len();
            s.scan_start = Some((
                seq,
                s.source_bytes_read,
                s.namespace_units,
                s.reserve_calls,
                s.append_calls,
            ));
            emit(
                &mut s,
                "field_scan_enter",
                "field",
                cursor,
                token,
                "null".into(),
            );
        }
    });
}
pub(in crate::frontend) fn field_token_inspect(base: *const Token, token: &Token) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled && s.scan_start.is_some() {
            let delta = (token as *const Token as usize)
                .checked_sub(base as usize)
                .expect("visited token before real tape");
            assert_eq!(delta % std::mem::size_of::<Token>(), 0);
            let index = delta / std::mem::size_of::<Token>();
            let observed = s
                .tokens
                .get(index)
                .expect("visited token outside admitted tape");
            assert_eq!(observed.kind, token.kind);
            assert_eq!(observed.span, token.span);
            charge(&mut s, std::mem::size_of::<usize>());
            s.scan_indices.push(index);
        }
    });
}
pub(in crate::frontend) fn field_scan_end(initiator: usize, terminal: Kind) {
    let inspected = STATE.with(|st| std::mem::take(&mut st.borrow_mut().scan_indices));
    field_scan(initiator, inspected, terminal);
}
pub(in crate::frontend) fn field_scan(initiator: usize, inspected: Vec<usize>, terminal: Kind) {
    STATE.with(|st|{let mut s=st.borrow_mut();if s.enabled{let (start_seq,start_bytes,start_work,start_reserves,start_appends)=s.scan_start.take().expect("missing scan enter");let token=s.tokens[initiator];let end_seq=s.events.len();let source_bytes_read=s.source_bytes_read-start_bytes;let namespace_units=s.namespace_units-start_work;let reserve_calls=s.reserve_calls-start_reserves;let append_calls=s.append_calls-start_appends;emit(&mut s,"field_scan_exit","field",initiator,token,"null".into());let indices=inspected.iter().map(usize::to_string).collect::<Vec<_>>().join(",");let eof=if terminal==Kind::Eof{initiator.to_string()}else{"null".into()};let row=format!("{{\"start_seq\":{start_seq},\"end_seq\":{end_seq},\"source_bytes_read\":{source_bytes_read},\"namespace_work_units\":{namespace_units},\"reserve_calls\":{reserve_calls},\"append_calls\":{append_calls},\"initiator_token\":{initiator},\"inspected_tokens\":[{indices}],\"terminal_kind\":{},\"result_is_ident\":{},\"eof_charge_initiator\":{eof}}}",q(&format!("{terminal:?}")),terminal==Kind::Ident);charge(&mut s,row.len());s.scans.push(row);}});
}
pub(in crate::frontend) fn reserve(kind: &str, length: usize, element_bytes: usize, success: bool) {
    STATE.with(|st|{let mut s=st.borrow_mut();if s.enabled{let (cursor,token)=s.position.expect("allocator event lacks actual parser position");emit(&mut s,"reserve",kind,cursor,token,format!("{{\"kind\":{},\"length\":{length},\"element_bytes\":{element_bytes},\"success\":{success}}}",q(kind)));}});
}
pub(in crate::frontend) fn lex_token(token: Token) {
    STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.enabled {
            assert!(
                s.tokens.len() <= super::super::lexer::MAX_TOKENS,
                "OBSERVATION_LIMIT token tape"
            );
            charge(&mut s, std::mem::size_of::<Token>());
            s.tokens.push(token);
        }
    });
}
fn bool_or_null(x: Option<bool>) -> String {
    x.map_or("null".into(), |v| v.to_string())
}
fn uint_or_null(x: Option<usize>) -> String {
    x.map_or("null".into(), |v| v.to_string())
}
fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("missing {key}"))
}
fn number(key: &str) -> usize {
    env(key).parse().unwrap_or_else(|_| panic!("invalid {key}"))
}
fn reset(tokens: Vec<Token>, enabled: bool) {
    STATE.with(|st| {
        *st.borrow_mut() = State {
            enabled,
            tokens,
            reject_kind: env("UNIT4_REJECT_KIND"),
            reject_occurrence: number("UNIT4_REJECT_OCCURRENCE"),
            ..State::default()
        }
    });
}
fn snapshot() -> (String, String, String, String, String, String, String) {
    STATE.with(|st| {
        let s = st.borrow();
        (
            format!("[{}]", s.events.join(",")),
            format!("[{}]", s.transitions.join(",")),
            format!("[{}]", s.scans.join(",")),
            bool_or_null(s.initial),
            bool_or_null(s.final_bit),
            uint_or_null(s.nodes),
            s.bytes.to_string(),
        )
    })
}
fn token_json(tokens: &[Token], denied: Option<Span>) -> String {
    let all = tokens
        .iter()
        .map(|t| {
            format!(
                "{{\"kind\":{},\"span\":{}}}",
                q(&format!("{:?}", t.kind)),
                span(t.span)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let colons = tokens
        .iter()
        .filter(|t| t.kind == Kind::Colon)
        .map(|t| span(t.span))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"tokens\":[{all}],\"non_eof_tokens\":{},\"eof_tokens\":{},\"colon_spans\":[{colons}],\"maximum_token_bytes\":{},\"denied_token_span\":{}}}",tokens.iter().filter(|t|t.kind!=Kind::Eof).count(),tokens.iter().filter(|t|t.kind==Kind::Eof).count(),tokens.iter().map(|t|t.span.end-t.span.start).max().unwrap_or(0),denied.map_or("null".into(),span))
}
fn render(sources: &SourceMap, diagnostics: &[Diagnostic]) -> (String, String, String) {
    let jsons = diagnostics
        .iter()
        .map(|d| d.render_json(sources))
        .collect::<Vec<_>>();
    let literal = jsons.iter().map(|s| q(s)).collect::<Vec<_>>().join(",");
    let human = diagnostics
        .iter()
        .map(|d| d.render_human(sources))
        .collect::<String>();
    (
        format!("[{}]", jsons.join(",")),
        format!("[{literal}]"),
        q(&human),
    )
}
fn run_mode(
    sources: &SourceMap,
    id: crate::frontend::source::SourceFileId,
    tokens: &[Token],
    lex_error: Option<&Diagnostic>,
    mode: SourceMode,
    mode_index: usize,
    control: bool,
) -> String {
    let source = sources.get(id);
    reset(tokens.to_vec(), !control);
    let mut allocator = Allocator {
        fail_at: match number("UNIT4_RESERVE_FAIL_AT") {
            0 => None,
            n => Some(n),
        },
        ..Allocator::default()
    };
    let direct = number("UNIT4_DIRECT") == 1;
    let mut count_after = "null".to_string();
    let mut ast = "null".to_string();
    let mut flavor = "null".to_string();
    let mut control_nodes = None;
    let (result, diagnostics, attempts) = if let Some(d) = lex_error {
        ("lex_error", vec![d.clone()], 0)
    } else if direct {
        let mut parser = Parser {
            source,
            allocator: &mut allocator,
            mode,
            project_recovery: false,
            tokens: tokens.to_vec(),
            cursor: 0,
            expressions: Vec::new(),
            paths: Vec::new(),
            path_segments: Vec::new(),
            heights: Vec::new(),
            nodes: 0,
            node_limit: number("UNIT4_NODE_LIMIT").min(MAX_NODES),
        };
        STATE.with(|st| st.borrow_mut().initial = Some(parser.project_recovery));
        let mut count = usize::MAX;
        let answer = parser.path_segment(
            &mut count,
            source.span(number("UNIT4_SEGMENT_START"), number("UNIT4_SEGMENT_END")),
        );
        parser_return(parser.nodes, parser.project_recovery);
        STATE.with(|st| st.borrow_mut().final_bit = None);
        count_after = if count == usize::MAX {
            q("usize::MAX")
        } else {
            count.to_string()
        };
        match answer {
            Err(d) => ("direct_seam_error", vec![*d], 0),
            Ok(()) => panic!("unexpected successful direct overflow seam"),
        }
    } else {
        match parse_counted(
            source,
            tokens.to_vec(),
            mode,
            number("UNIT4_NODE_LIMIT"),
            &mut allocator,
        ) {
            Ok((program, nodes)) => {
                control_nodes = Some(nodes);
                flavor = q(if program.uses_project_syntax() {
                    "ProjectSyntax"
                } else {
                    "OriginalSingleFile"
                });
                let actual_handle = program.unit4_source_handle();
                assert_eq!(actual_handle.1, source.text().len());
                ast=format!("{{\"canonical_debug\":{},\"syntax_flavor\":{},\"belongs_to_source\":{},\"source_generation\":{},\"file_id\":{},\"node_count\":{},\"spans_and_ids_valid\":{}}}",q(&format!("{program:?}")),flavor,program.belongs_to(source),actual_handle.0,actual_handle.2,nodes,program.validate_spans_and_ids(|span|sources.is_valid_span(span)));
                ("ok", Vec::new(), 1)
            }
            Err(ds) => ("parse_error", ds, 1),
        }
    };
    let attempts = if control {
        attempts
    } else {
        STATE.with(|st| st.borrow().parse_attempts)
    };
    let (events, transitions, scans, initial, final_bit, nodes, charged) = snapshot();
    let current_reads = STATE.with(|st| st.borrow().current_reads.join(","));
    let (ds, jsons, human) = render(sources, &diagnostics);
    let denied = if lex_error.is_some() {
        lex_error.unwrap().primary
    } else {
        None
    };
    let reserves = allocator
        .trace
        .iter()
        .map(|r| {
            format!(
                "{{\"kind\":{},\"length\":{},\"element_bytes\":{},\"success\":{}}}",
                q(r.kind),
                r.length,
                r.element_bytes,
                r.success
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let mode_name = if mode == SourceMode::ProjectCandidate {
        "ProjectCandidate"
    } else {
        "OwnedCandidate"
    };
    format!("{{\"mode\":{},\"mode_execution_index\":{mode_index},\"source_generation\":{},\"runtime_os\":{},\"runtime_architecture\":{},\"pointer_width\":{},\"result\":{},\"executed\":true,\"parse_attempts\":{attempts},\"diagnostics\":{ds},\"json_diagnostic_renderings\":{jsons},\"human_diagnostic_rendering\":{human},\"token_inventory\":{},\"nodes_admitted\":{},\"recognized_initial\":{initial},\"recognized_final\":{final_bit},\"recognition_transitions\":{transitions},\"events\":{events},\"field_pub_scans\":{scans},\"field_current_token_reads\":[{current_reads}],\"reserve_attempts\":{},\"reserve_trace\":[{reserves}],\"ast\":{ast},\"syntax_flavor\":{flavor},\"path_segment_count_after\":{count_after},\"observation_complete\":{},\"observer_limits\":{{\"events\":{EVENT_LIMIT},\"evidence_bytes\":{BYTE_LIMIT},\"charged_bytes\":{charged}}}}}",q(mode_name),source.identity(),q(std::env::consts::OS),q(std::env::consts::ARCH),usize::BITS,q(result),token_json(tokens,denied),if control{uint_or_null(control_nodes)}else{nodes},allocator.attempts,!control)
}
#[test]
#[ignore]
fn observe_request() {
    let source_bytes = std::fs::read(env("UNIT4_SOURCE")).expect("source request readable");
    assert!(
        source_bytes.len() <= 1_048_576,
        "source request exceeds contract reader limit"
    );
    let text = String::from_utf8(source_bytes).expect("valid UTF8 source request");
    let mut sources = SourceMap::new();
    let id = sources.add(env("UNIT4_DISPLAY_PATH"), text);
    let control = number("UNIT4_CONTROL") == 1;
    reset(Vec::new(), !control);
    let lex = super::super::lexer::lex_with_limit(sources.get(id), number("UNIT4_TOKEN_LIMIT"));
    let (tokens, error) = match lex {
        Ok(tokens) => (tokens, None),
        Err(error) => {
            let tokens = STATE.with(|st| st.borrow().tokens.clone());
            (tokens, Some(*error))
        }
    };
    let mut rows = vec![run_mode(
        &sources,
        id,
        &tokens,
        error.as_ref(),
        SourceMode::ProjectCandidate,
        0,
        control,
    )];
    if number("UNIT4_ORIGINAL") == 1 {
        rows.push(run_mode(
            &sources,
            id,
            &tokens,
            error.as_ref(),
            SourceMode::OwnedCandidate,
            1,
            control,
        ));
    }
    let out=format!("{{\"schema\":\"oxid-unit4-parser-raw-v1\",\"nonce\":{},\"case_id\":{},\"source_utf8\":{},\"display_path\":{},\"observations\":[{}]}}",q(&env("UNIT4_NONCE")),q(&env("UNIT4_CASE_ID")),q(sources.get(id).text()),q(sources.get(id).path()),rows.join(","));
    assert!(
        out.len() <= BYTE_LIMIT,
        "OBSERVATION_LIMIT emitted evidence budget"
    );
    std::fs::write(env("UNIT4_RAW_OUTPUT"), out).expect("raw observation write");
    println!("UNIT4_EXECUTED {}", env("UNIT4_NONCE"));
}
