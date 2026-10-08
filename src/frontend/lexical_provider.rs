//! Explicit, source-bound experimental lexical provider. External observations
//! are compared with the unchanged canonical lexer; only the provider's owned
//! token allocation is moved into the unchanged parser. There is no fallback.
use super::{
    declaration_index::IndexLimits,
    diagnostic::Diagnostic,
    lexer::Token,
    project::{budget::Allocator, Inventory, LexicalObservation, LexicalProvider, SourceUsage},
    source::{SourceFile, SourceFileId, SourceMap},
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of, path::Path};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod bundle;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod supervisor;
#[cfg(test)]
mod tests;
mod wire;

const RECEIPTS: usize = 256;
// Prepay bounded source hashing and receipt formatting for a whole default
// project, including its final failed module. No failed allocation/transport
// requires spending work that was available only on the success path.
const RECEIPT_WORK: u64 =
    8 * super::source::MAX_SOURCE_BYTES as u64 + 8 * 4 * 1024 * 1024 + RECEIPTS as u64 * 4096;

#[derive(Debug)]
struct Receipt {
    file: SourceFileId,
    identity: u64,
    source_len: usize,
    source_sha256: [u8; 32],
    executable_sha256: [u8; 32],
    input_sha256: [u8; 32],
    stdout_sha256: [u8; 32],
    stderr_sha256: [u8; 32],
    input_bytes: usize,
    input_written: usize,
    stdout_bytes: usize,
    stderr_bytes: usize,
    status: Option<i32>,
    signal: Option<i32>,
    stop: &'static str,
    spawned: bool,
    leader_reaped: bool,
    stdin_closed: bool,
    stdout_eof: bool,
    stderr_eof: bool,
    comparison_attempted: bool,
    comparison_matched: bool,
    selected_for_parser: bool,
    #[cfg(test)]
    producer_allocation: usize,
}
impl Receipt {
    fn new(source: &SourceFile, executable_sha256: [u8; 32]) -> Self {
        Self {
            file: source.span(0, 0).file,
            identity: source.identity(),
            source_len: source.text().len(),
            source_sha256: Sha256::digest(source.text().as_bytes()).into(),
            executable_sha256,
            input_sha256: Sha256::digest([]).into(),
            stdout_sha256: Sha256::digest([]).into(),
            stderr_sha256: Sha256::digest([]).into(),
            input_bytes: 0,
            input_written: 0,
            stdout_bytes: 0,
            stderr_bytes: 0,
            status: None,
            signal: None,
            stop: "not-started",
            spawned: false,
            leader_reaped: false,
            stdin_closed: false,
            stdout_eof: false,
            stderr_eof: false,
            comparison_attempted: false,
            comparison_matched: false,
            selected_for_parser: false,
            #[cfg(test)]
            producer_allocation: 0,
        }
    }
}

/// Transient external owner; never embedded in ProjectSources, LoadFailure,
/// SourceSetBuilder or a checked HIR program. Drop before ordinary HIR checking.
pub(super) struct Provider {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    bundle: Option<bundle::Bundle>,
    receipts: Vec<Receipt>,
    budget: Budget,
    #[cfg(test)]
    fixture: Option<fn(&SourceFile, usize) -> Vec<u8>>,
}

#[derive(Clone, Copy)]
struct Budget {
    limits: IndexLimits,
    work: u64,
    selected_capacity: usize,
    receipt_work_left: u64,
    fixed_retained: usize,
    fixed_scratch: usize,
}
#[derive(Debug)]
struct Plan {
    output: usize,
    retained: usize,
    scratch: usize,
    work: u64,
}
impl Budget {
    fn new(extra_retained: usize, extra_scratch: usize) -> Result<Self, &'static str> {
        let fixed_retained = (size_of::<Provider>() + size_of::<super::options::LexicalOptions>())
            .checked_add(RECEIPTS * size_of::<Receipt>())
            .and_then(|n| n.checked_add(extra_retained))
            .ok_or("lexical provider storage overflow")?;
        let fixed_scratch = fixed_bytes()
            .checked_add(extra_scratch)
            .ok_or("lexical provider storage overflow")?;
        let limits = IndexLimits::default();
        if fixed_retained as u64 > limits.retained || fixed_scratch as u64 > limits.scratch {
            return Err("lexical provider fixed storage limit exceeded");
        }
        Ok(Self {
            limits,
            work: RECEIPT_WORK,
            receipt_work_left: RECEIPT_WORK,
            selected_capacity: 0,
            fixed_retained,
            fixed_scratch,
        })
    }
    fn admit_receipt(&mut self, source: &SourceFile) -> Result<(), &'static str> {
        let work = (source.text().len() as u64)
            .checked_mul(8)
            .and_then(|n| n.checked_add((source.path().len() as u64).checked_mul(8)?))
            .and_then(|n| n.checked_add(4096))
            .ok_or("lexical receipt work overflow")?;
        self.receipt_work_left = self
            .receipt_work_left
            .checked_sub(work)
            .ok_or("lexical receipt work limit exceeded")?;
        Ok(())
    }
    fn admit_inventory_work(&mut self, usage: SourceUsage) -> Result<(), &'static str> {
        // inventory() visits prior programs, declarations, blocks, statements
        // and expressions. Parser nodes plus tokens conservatively cover all
        // those visits; pay before the loader walks them for this module.
        let visits = (usage.syntax_nodes as u64)
            .checked_add(usage.non_eof_tokens as u64)
            .and_then(|n| n.checked_add(usage.modules as u64 + 1))
            .and_then(|n| n.checked_mul(8))
            .ok_or("lexical inventory work overflow")?;
        let total = self
            .work
            .checked_add(visits)
            .ok_or("lexical inventory work overflow")?;
        if total > self.limits.work {
            return Err("lexical provider inventory work limit exceeded");
        }
        self.work = total;
        Ok(())
    }
    fn admit(
        &mut self,
        source: &SourceFile,
        limit: usize,
        inventory: &Inventory,
    ) -> Result<Plan, &'static str> {
        let n = source.text().len();
        let output = wire::output_bound(n, limit)?;
        let token_slots = n.min(limit) + 1;
        // The existing infallible canonical Vec starts at four slots and doubles.
        // Charge its full next-power-of-two capacity AND the old allocation that
        // may coexist during growth. This does not make old lexing fallible.
        let canonical_capacity = token_slots
            .max(4)
            .checked_next_power_of_two()
            .ok_or("lexical canonical capacity overflow")?;
        let canonical_peak = canonical_capacity
            .checked_add(canonical_capacity / 2)
            .and_then(|n| n.checked_mul(size_of::<Token>()))
            .ok_or("lexical canonical capacity overflow")?;
        let token_bytes = token_slots
            .checked_mul(size_of::<Token>())
            .ok_or("lexical token storage overflow")?;
        let mut retained = self.fixed_retained;
        // Existing source/AST inventory keeps its established requested-storage
        // model. Replace its token lengths with actual exact provider capacity.
        for value in [
            inventory.owner_bytes,
            inventory.source_bytes,
            inventory.source_headers,
            inventory.line_starts,
            inventory.ast_headers,
            inventory.ast_payload,
            inventory.module_headers,
            inventory.path_bytes,
            self.selected_capacity,
            token_bytes,
        ] {
            retained = retained
                .checked_add(value)
                .ok_or("lexical retained storage overflow")?;
        }
        // Conservatively charge all these allocations together even though input
        // and output are released before canonical lexing and parser transfer.
        let mut scratch = self.fixed_scratch;
        for value in [
            inventory.fixed_loader_scratch,
            n + 12,
            output + 1,
            token_bytes,
            canonical_peak,
        ] {
            scratch = scratch
                .checked_add(value)
                .ok_or("lexical scratch storage overflow")?;
        }
        // Named logical work: 56 per source byte for input/hash/canonical
        // passes, 16 per bounded output byte (8 for each strict decode pass),
        // and 16 per possible token for materialization/comparison. Receipt
        // hashing/formatting and prior-AST inventory walks were prepaid above.
        // External CPU/RSS are not this meter; lifetime is separately supervised.
        let work = (n as u64)
            .checked_mul(56)
            .and_then(|v| v.checked_add((output as u64) * 16))
            .and_then(|v| v.checked_add((token_slots as u64) * 16))
            .ok_or("lexical work overflow")?;
        let total_work = self.work.checked_add(work).ok_or("lexical work overflow")?;
        if retained as u64 > self.limits.retained {
            return Err("lexical provider retained byte limit exceeded");
        }
        if scratch as u64 > self.limits.scratch {
            return Err("lexical provider scratch byte limit exceeded");
        }
        if total_work > self.limits.work {
            return Err("lexical provider work limit exceeded");
        }
        self.work = total_work;
        Ok(Plan {
            output,
            retained,
            scratch,
            work,
        })
    }
}
fn fixed_bytes() -> usize {
    wire::named_bytes()
        + 3 * size_of::<LexicalObservation>()
        + 3 * size_of::<Receipt>()
        + 3 * size_of::<Budget>()
        + 3 * size_of::<Plan>()
        + 3 * size_of::<Diagnostic>()
        + 3 * size_of::<[u8; 1024]>()
        + size_of::<[usize; 64]>()
        + 4 * size_of::<Sha256>()
        + 3 * size_of::<Option<&mut dyn LexicalProvider>>()
}
fn failure(source: &SourceFile, message: &'static str) -> Box<Diagnostic> {
    super::owned_diagnostic::diagnostic(
        "E0703",
        "lexical-provider",
        format_args!("{message}"),
        Some(source.span(0, 0)),
    )
}

impl Provider {
    pub(super) fn load(directory: &Path) -> Result<Self, &'static str> {
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = (directory, Budget::new(0, 0)?);
            Err("experimental lexical provider requires Linux x86_64")
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            // Admit bounded bundle-loading carriers and logical copy/hash work
            // before opening/copying an executable. Kernel-backed sealed bytes
            // have their separate executable bound, not a Rust/RSS claim.
            // The named loader model excludes standard-library temporary-parent
            // lookup/canonicalization setup allocations; retained admitted
            // workspace-path capacity is still accounted separately.
            let mut budget = Budget::new(4096, bundle::named_bytes() + supervisor::named_bytes())?;
            let copy_bound = (bundle::EXECUTABLE_MAX as u64) * 8 + 4096;
            if budget
                .work
                .checked_add(copy_bound)
                .is_none_or(|work| work > budget.limits.work)
            {
                return Err("lexical executable copy work limit exceeded");
            }
            let bundle = bundle::Bundle::load(directory)?;
            budget = Budget::new(
                bundle.named_bytes(),
                bundle::named_bytes() + supervisor::named_bytes(),
            )?;
            budget.work += (bundle.executable_bytes() as u64) * 8 + 4096;
            let mut receipts = Vec::new();
            wire::reserve(
                &mut receipts,
                RECEIPTS,
                &mut Allocator::default(),
                "lexical provider receipts",
            )?;
            Ok(Self {
                bundle: Some(bundle),
                receipts,
                budget,
                #[cfg(test)]
                fixture: None,
            })
        }
    }
    fn lex(
        &mut self,
        source: &SourceFile,
        limit: usize,
        inventory: &Inventory,
        allocator: &mut Allocator,
    ) -> Result<LexicalObservation, Box<Diagnostic>> {
        let index = self
            .receipts
            .len()
            .checked_sub(1)
            .ok_or_else(|| failure(source, "lexical module was not admitted"))?;
        let receipt = &self.receipts[index];
        if receipt.identity != source.identity()
            || receipt.file != source.span(0, 0).file
            || receipt.source_len != source.text().len()
            || receipt.stop != "not-started"
        {
            return Err(failure(
                source,
                "lexical module admission identity mismatch",
            ));
        }
        let result = self.lex_inner(source, limit, inventory, allocator, index);
        if result.is_err() && self.receipts[index].stop == "not-started" {
            self.receipts[index].stop = "host-refused";
        }
        result.map_err(|e| failure(source, e))
    }
    fn lex_inner(
        &mut self,
        source: &SourceFile,
        limit: usize,
        inventory: &Inventory,
        allocator: &mut Allocator,
        index: usize,
    ) -> Result<LexicalObservation, &'static str> {
        let plan = self.budget.admit(source, limit, inventory)?;
        debug_assert!(plan.retained as u64 <= self.budget.limits.retained);
        debug_assert!(plan.scratch as u64 <= self.budget.limits.scratch);
        debug_assert!(plan.work <= self.budget.work);
        let input = wire::request(source, limit, allocator)?;
        self.receipts[index].input_sha256 = Sha256::digest(&input).into();
        self.receipts[index].input_bytes = input.len();
        let observation = self.capture(source, limit, input, plan.output, allocator, index)?;
        let value = match observation {
            wire::Observation::Tokens(tokens) => {
                #[cfg(test)]
                {
                    self.receipts[index].producer_allocation = tokens.as_ptr() as usize;
                }
                Ok(tokens)
            }
            wire::Observation::Diagnostic(observed) => {
                let (code, message) = observed.fields();
                Err(Diagnostic::new(
                    code,
                    "lex",
                    message,
                    Some(source.span(observed.start, observed.end)),
                ))
            }
        };
        Ok(LexicalObservation::new(source, value))
    }
    fn capture(
        &mut self,
        source: &SourceFile,
        limit: usize,
        input: Vec<u8>,
        output_bound: usize,
        allocator: &mut Allocator,
        index: usize,
    ) -> Result<wire::Observation, &'static str> {
        #[cfg(test)]
        if let Some(fixture) = self.fixture {
            let bytes = fixture(source, limit);
            let receipt = &mut self.receipts[index];
            receipt.input_written = input.len();
            receipt.stdout_sha256 = Sha256::digest(&bytes).into();
            receipt.stdout_bytes = bytes.len();
            receipt.stop = "test-observation";
            drop(input);
            return wire::decode(source, limit, &bytes, allocator);
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let bundle = self
                .bundle
                .as_ref()
                .ok_or("lexical executable unavailable")?;
            let captured = supervisor::run(
                bundle.lexer(),
                bundle.directory(),
                &input,
                output_bound,
                supervisor::DEADLINE,
            );
            let receipt = &mut self.receipts[index];
            receipt.input_written = captured.input_written;
            receipt.stdout_bytes = captured.stdout.len();
            receipt.stderr_bytes = captured.stderr_len;
            receipt.stdout_sha256 = Sha256::digest(&captured.stdout).into();
            receipt.stderr_sha256 = Sha256::digest(&captured.stderr[..captured.stderr_len]).into();
            receipt.status = captured.code();
            receipt.signal = captured.signal();
            receipt.stop = captured.stop.name();
            receipt.spawned = captured.spawned;
            receipt.leader_reaped = captured.status.is_some();
            receipt.stdin_closed = captured.stdin_closed;
            receipt.stdout_eof = captured.stdout_eof;
            receipt.stderr_eof = captured.stderr_eof;
            let completed = captured.stop == supervisor::Stop::Exited
                && captured.status.is_some()
                && captured.code() == Some(0)
                && captured.input_written == input.len()
                && captured.stdin_closed
                && captured.stderr_len == 0
                && captured.stdout_eof
                && captured.stderr_eof;
            drop(input);
            if !completed {
                return Err("lexical provider failed bounded process/transport contract");
            }
            wire::decode(source, limit, &captured.stdout, allocator)
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = (source, limit, input, output_bound, allocator, index);
            Err("experimental lexical provider requires Linux x86_64")
        }
    }
    pub(super) fn print_receipts(&self, sources: &SourceMap, json: bool, process: bool) -> bool {
        use fmt::Write;
        let mut sink = ReceiptSink { json, process };
        for receipt in &self.receipts {
            let Some(source) = sources.files().get(receipt.file.0) else {
                return false;
            };
            if source.identity() != receipt.identity || source.text().len() != receipt.source_len {
                return false;
            }
            if writeln!(sink, "{}", ReceiptDisplay { receipt, source }).is_err() {
                return false;
            }
        }
        true
    }
}

impl LexicalProvider for Provider {
    fn begin_module(
        &mut self,
        source: &SourceFile,
        usage: SourceUsage,
    ) -> Result<(), Box<Diagnostic>> {
        if self.receipts.len() == self.receipts.capacity() {
            return Err(failure(source, "lexical provider receipt limit exceeded"));
        }
        self.budget
            .admit_receipt(source)
            .map_err(|message| failure(source, message))?;
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let executable_hash = self
            .bundle
            .as_ref()
            .map_or([0; 32], bundle::Bundle::lexer_hash);
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        let executable_hash = [0; 32];
        self.receipts.push(Receipt::new(source, executable_hash));
        if let Err(message) = self.budget.admit_inventory_work(usage) {
            self.receipts
                .last_mut()
                .expect("just admitted receipt")
                .stop = "host-refused";
            return Err(failure(source, message));
        }
        Ok(())
    }

    fn observe(
        &mut self,
        source: &SourceFile,
        limit: usize,
        inventory: &Inventory,
        allocator: &mut Allocator,
    ) -> Result<LexicalObservation, Box<Diagnostic>> {
        self.lex(source, limit, inventory, allocator)
    }
    fn comparison_started(&mut self) {
        if let Some(receipt) = self.receipts.last_mut() {
            receipt.comparison_attempted = true;
        }
    }
    fn comparison_finished(&mut self, matched: bool) {
        if let Some(receipt) = self.receipts.last_mut() {
            receipt.comparison_matched = matched;
        }
    }
    fn selected_for_parser(
        &mut self,
        source: &SourceFile,
        _tokens: &[Token],
        capacity: usize,
    ) -> Result<(), Box<Diagnostic>> {
        let receipt = self
            .receipts
            .last_mut()
            .ok_or_else(|| failure(source, "missing lexical receipt"))?;
        if receipt.identity != source.identity()
            || receipt.file != source.span(0, 0).file
            || receipt.source_len != source.text().len()
            || !receipt.comparison_matched
            || receipt.selected_for_parser
        {
            return Err(failure(source, "invalid lexical parser transfer"));
        }
        self.budget.selected_capacity = capacity
            .checked_mul(size_of::<Token>())
            .and_then(|bytes| self.budget.selected_capacity.checked_add(bytes))
            .ok_or_else(|| failure(source, "lexical selected capacity overflow"))?;
        receipt.selected_for_parser = true;
        Ok(())
    }
}

struct ReceiptSink {
    json: bool,
    process: bool,
}
impl fmt::Write for ReceiptSink {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        use std::io::Write;
        if self.process {
            return (super::oir::process::diagnostic(text.as_bytes()) == 1)
                .then_some(())
                .ok_or(fmt::Error);
        }
        if self.json {
            std::io::stdout().lock().write_all(text.as_bytes())
        } else {
            std::io::stderr().lock().write_all(text.as_bytes())
        }
        .map_err(|_| fmt::Error)
    }
}
struct Hex<'a>(&'a [u8; 32]);
impl fmt::Display for Hex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}
struct Number(Option<i32>);
impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(n) => write!(f, "{n}"),
            None => f.write_str("null"),
        }
    }
}
struct JsonText<'a>(&'a str);
impl fmt::Display for JsonText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("\"")?;
        for ch in self.0.chars() {
            match ch {
                '"' => f.write_str("\\\"")?,
                '\\' => f.write_str("\\\\")?,
                ch if ch.is_control() => write!(f, "\\u{:04x}", ch as u32)?,
                _ => write!(f, "{ch}")?,
            }
        }
        f.write_str("\"")
    }
}
struct ReceiptDisplay<'a> {
    receipt: &'a Receipt,
    source: &'a SourceFile,
}
impl fmt::Display for ReceiptDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.receipt;
        write!(f, "{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"lexical-provider\",\"file_id\":{},\"source_identity\":{},\"path\":{},\"source_bytes\":{},\"source_sha256\":\"{}\",\"executable_sha256\":\"{}\",\"input_sha256\":\"{}\",\"input_bytes\":{},\"input_written\":{},\"captured_stdout_bytes\":{},\"captured_stdout_sha256\":\"{}\",\"captured_stderr_bytes\":{},\"captured_stderr_sha256\":\"{}\",\"spawned\":{},\"leader_reaped\":{},\"stdin_closed\":{},\"stdout_eof\":{},\"stderr_eof\":{},\"stop\":\"{}\",\"exit_status\":{},\"signal\":{},\"comparison_attempted\":{},\"comparison_matched\":{},\"producer_tokens_reached_parser\":{},\"fallback\":false}}",
            r.file.0, r.identity, JsonText(self.source.path()), r.source_len, Hex(&r.source_sha256), Hex(&r.executable_sha256), Hex(&r.input_sha256), r.input_bytes, r.input_written,
            r.stdout_bytes, Hex(&r.stdout_sha256), r.stderr_bytes, Hex(&r.stderr_sha256), r.spawned, r.leader_reaped, r.stdin_closed,
            r.stdout_eof, r.stderr_eof, r.stop, Number(r.status), Number(r.signal), r.comparison_attempted, r.comparison_matched, r.selected_for_parser)
    }
}
