# Unit4 parser observer API v1 (implementation checkpoint)

This additive test-only observer derives a source tree from exact commit d9e6b9bf172abd5e15da7212c9e6224e29ccc768. The original checkout and frozen contracts are never edited. `prepare.py` hashes all original production inputs, produces a separate instrumentation diff and manifest, and copies test instrumentation into the derived tree. No compiler result supplies an expectation.

Each observation JSON object has these fields:
- schema = oxid-unit4-parser-observation-v1
- binding = {contract_decoded_sha256, package_freeze_sha256, case_id, source_sha256, source_bytes, display_path, candidate_source_manifest_sha256, observer_source_sha256, binary_sha256, profile, actual_runtime_os, actual_runtime_architecture, actual_pointer_width, rust_compiler_target, mode, seam, execution_id, source_generation, mode_execution_index}
- result: ok | parse_error | lex_error | direct_seam_error
- executed = true, parse_attempts, diagnostics (full JSON renderer objects), json_diagnostic_bytes_base64 (ordered actual renderings without newline), human_diagnostic_bytes_base64 (actual concatenated human rendering)
- token_inventory = {tokens:[{kind,span:[file_id,start,end]}], non_eof_tokens, eof_tokens, colon_spans, maximum_token_bytes, denied_token_span}. Admitted lex tokens only, no invented completion on failure.
- nodes_admitted, recognized_initial, recognized_final
- recognition_transitions: exact event objects for false-to-true assignments
- events: ordered {seq,kind,production,cursor,token_kind,span:[file_id,start,end],context,detail}. Details carry admission ordinal/ledger, count, or allocator request payload where applicable. Kinds node_attempt/node_admit/node_reject, consume, recognize, recognize_already_true, recover_enter/recover_inspect/recover_exit, path_count_checked/path_count_overflow/path_cap_reject, reserve, append. `consume` has actual token kind; required consume_pub projections match consume + Pub. `node_reject` production=field projects to field_node_reject. Reserve records carry the real allocator result and current parser cursor.
- field_pub_scans: {initiator_token,inspected_tokens,terminal_kind,result_is_ident,eof_charge_initiator}; `field_current_token_reads` separately records current reads during the logical helper.
- reserve_attempts, reserve_trace: {kind,length,element_bytes,success} from the real Allocator
- ast: null on failure; on success {canonical: complete parsed Rust Debug structure with tags and ordered vectors, syntax_flavor, belongs_to_source, source_generation, file_id, node_count, spans_and_ids_valid}. Canonical SourceProvenance debug omits only generation, supplied explicitly in source_generation; no source owner is fabricated. Full token/spans/arena IDs/declarations/expressions remain in canonical.
- syntax_flavor: OriginalSingleFile | ProjectSyntax | null
- path_segment_count_after: null ordinarily; actual count or string usize::MAX in direct seam
- observation_complete = true, observer_limits = {events:1000000, evidence_bytes:134217728}; exceeding either aborts evidence, never truncates to a passing row

ProjectCandidate is executed for each input. A relation row additionally executes OwnedCandidate using the same immutable SourceMap and token admission/limits, with distinct mode_execution_index and actual parser execution. The input request contains only source, configured limits and declared seam; it contains no expected outcome. Rust emits raw records, the Python driver only binds identities and mechanically decodes complete Rust Debug structures; comparison belongs to the independent comparator.

Passive hooks never supply diagnostics or AST flavor. The field reject seam lowers remaining node budget immediately before the selected real node gate. The overflow seam invokes real path_segment with usize::MAX. Allocator failure uses existing fail_at. An uninstrumented control build runs the same entrypoint with hooks omitted and compares externally visible observations for ordinary source controls. All normalizations preserve raw stdout and the raw JSON record.

## Clarifications added before qualification

Events additionally retain cursor_span and cursor_token_kind from the actually admitted token tape at the current cursor. Path-segment operation span/kind may differ from these cursor fields because the segment was just consumed. No current-position fallback is allowed.

Every field scan has start_seq/end_seq, indexing its actual field_scan_enter/field_scan_exit events, and actual counter deltas source_bytes_read, namespace_work_units, reserve_calls, append_calls. Source reads hook SourceFile::try_text and SourceFile::text; work hooks WorkMeter::preflight and WorkMeter::debit. These counters describe calls during the helper only. Observer-owned Vec/event allocations are bounded separately and excluded from candidate allocator accounting. field_current_token_reads records {cursor,token_kind,span} at the actual record-field guard before testing Pub; it is not part of logical forward-scan charges.

SourceProvenance generation in AST comes from a read-only test accessor of the actual Program source association, alongside its actual file ID/text length. It is not copied from the input handle; belongs_to_source independently compares the two.

The build receipt records explicit --target x86_64-unknown-linux-gnu in argv and complete rustc --version --verbose evidence, hashed toolchain binary, input overlay manifest, resulting binary, stdout/stderr and bounded environment. Runtime OS/arch/pointer width come from the actual observer executable.

The execution manifest records status=collected, requested_case_ids, case_count, observation_count, profile, exact build receipt/checkpoint/driver/normalizer identities and per-case command, nonce, request, raw-output, stdout/stderr identities. Request rows contain inputs and seams only. Test roster must contain exactly one observer entrypoint; every child process must confirm one executed exact ignored test and its nonce. Full qualification requires both complete profiles, independently checked by the comparator. Driver subset collection is explicitly represented by the roster and cannot stand in for the full corpus.
