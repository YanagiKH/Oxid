use super::{Expr, Literal, Program, SourceSpan, Stmt, TokenKind};
use std::fs;
use std::path::Path;

const MAGIC: &[u8; 4] = b"OXBC";
const FORMAT_MAJOR: u16 = 1;
const FORMAT_MINOR: u16 = 0;
const AST_VERSION: u16 = 1;

// Header layout, all integer fields little-endian:
// magic[4], format_major[u16], format_minor[u16], ast_version[u16],
// module_count[u32], payload_length[u64], payload_checksum[u64].
const HEADER_LEN: usize = 30;
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;
const MAX_STRING_BYTES: usize = 16 * 1024 * 1024;
const MAX_COLLECTION_ITEMS: usize = 1_000_000;
const MAX_AST_NODES: usize = 1_000_000;
const MAX_AST_DEPTH: usize = 256;
const MAX_MODULE_COUNT: u32 = 1_000_000;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(super) struct DecodedArtifact {
    pub(super) program: Program,
    pub(super) module_count: u32,
    pub(super) checksum: u64,
}

pub(super) fn encode(program: &Program, module_count: u32) -> Result<Vec<u8>, String> {
    if module_count > MAX_MODULE_COUNT {
        return Err(format!(
            "Oxid artifact module count {} exceeds limit {}",
            module_count, MAX_MODULE_COUNT
        ));
    }

    let mut encoder = Encoder::new();
    encoder.write_program(program)?;
    let payload = encoder.finish();
    let payload_len = u64::try_from(payload.len())
        .map_err(|_| "Oxid artifact payload length cannot be represented as u64".to_string())?;
    let checksum = fnv1a64(&payload);

    let total_len = HEADER_LEN
        .checked_add(payload.len())
        .ok_or_else(|| "Oxid artifact size overflow".to_string())?;
    let mut artifact = Vec::new();
    artifact
        .try_reserve_exact(total_len)
        .map_err(|_| format!("cannot allocate {} bytes for Oxid artifact", total_len))?;
    artifact.extend_from_slice(MAGIC);
    artifact.extend_from_slice(&FORMAT_MAJOR.to_le_bytes());
    artifact.extend_from_slice(&FORMAT_MINOR.to_le_bytes());
    artifact.extend_from_slice(&AST_VERSION.to_le_bytes());
    artifact.extend_from_slice(&module_count.to_le_bytes());
    artifact.extend_from_slice(&payload_len.to_le_bytes());
    artifact.extend_from_slice(&checksum.to_le_bytes());
    artifact.extend_from_slice(&payload);
    Ok(artifact)
}

pub(super) fn decode(bytes: &[u8]) -> Result<DecodedArtifact, String> {
    if bytes.len() < MAGIC.len() {
        return Err(format!(
            "truncated Oxid artifact header: expected at least {} bytes, found {}",
            HEADER_LEN,
            bytes.len()
        ));
    }
    if !is_artifact(bytes) {
        return Err("not an Oxid artifact: missing OXBC magic".to_string());
    }
    if bytes.len() < HEADER_LEN {
        return Err(format!(
            "truncated Oxid artifact header: expected {} bytes, found {}",
            HEADER_LEN,
            bytes.len()
        ));
    }

    let format_major = u16::from_le_bytes([bytes[4], bytes[5]]);
    let format_minor = u16::from_le_bytes([bytes[6], bytes[7]]);
    let ast_version = u16::from_le_bytes([bytes[8], bytes[9]]);
    let module_count = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]);
    let payload_len_u64 = u64::from_le_bytes([
        bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21],
    ]);
    let expected_checksum = u64::from_le_bytes([
        bytes[22], bytes[23], bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29],
    ]);

    if format_major != FORMAT_MAJOR {
        return Err(format!(
            "incompatible Oxid artifact format major version {} (supported {})",
            format_major, FORMAT_MAJOR
        ));
    }
    if format_minor != FORMAT_MINOR {
        return Err(format!(
            "incompatible Oxid artifact format minor version {} (supported {})",
            format_minor, FORMAT_MINOR
        ));
    }
    if ast_version != AST_VERSION {
        return Err(format!(
            "incompatible Oxid AST version {} (supported {})",
            ast_version, AST_VERSION
        ));
    }
    if module_count > MAX_MODULE_COUNT {
        return Err(format!(
            "Oxid artifact module count {} exceeds limit {}",
            module_count, MAX_MODULE_COUNT
        ));
    }

    let payload_len = usize::try_from(payload_len_u64).map_err(|_| {
        format!(
            "Oxid artifact payload length {} is not supported on this platform",
            payload_len_u64
        )
    })?;
    if payload_len > MAX_PAYLOAD_BYTES {
        return Err(format!(
            "Oxid artifact payload length {} exceeds limit {}",
            payload_len, MAX_PAYLOAD_BYTES
        ));
    }
    let expected_len = HEADER_LEN
        .checked_add(payload_len)
        .ok_or_else(|| "Oxid artifact size overflow".to_string())?;
    if bytes.len() < expected_len {
        return Err(format!(
            "truncated Oxid artifact payload: header declares {} bytes, only {} available",
            payload_len,
            bytes.len().saturating_sub(HEADER_LEN)
        ));
    }
    if bytes.len() > expected_len {
        return Err(format!(
            "trailing bytes after Oxid artifact payload: expected total {}, found {}",
            expected_len,
            bytes.len()
        ));
    }

    let payload = &bytes[HEADER_LEN..expected_len];
    let actual_checksum = fnv1a64(payload);
    if actual_checksum != expected_checksum {
        return Err(format!(
            "Oxid artifact checksum mismatch: expected {:016x}, computed {:016x}",
            expected_checksum, actual_checksum
        ));
    }

    let mut decoder = Decoder::new(payload);
    let program = decoder.read_program()?;
    decoder.finish()?;
    Ok(DecodedArtifact {
        program,
        module_count,
        checksum: actual_checksum,
    })
}

pub(super) fn write(path: &Path, program: &Program, module_count: u32) -> Result<(), String> {
    let bytes = encode(program, module_count)?;
    fs::write(path, bytes)
        .map_err(|error| format!("cannot write Oxid artifact {}: {}", path.display(), error))
}

#[allow(dead_code)]
pub(super) fn read(path: &Path) -> Result<DecodedArtifact, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot inspect Oxid artifact {}: {}", path.display(), error))?;
    let max_artifact_len = u64::try_from(HEADER_LEN + MAX_PAYLOAD_BYTES)
        .map_err(|_| "Oxid artifact size limit cannot be represented as u64".to_string())?;
    if metadata.len() > max_artifact_len {
        return Err(format!(
            "Oxid artifact {} is too large: {} bytes exceeds limit {}",
            path.display(),
            metadata.len(),
            max_artifact_len
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| format!("cannot read Oxid artifact {}: {}", path.display(), error))?;
    if bytes.len() > HEADER_LEN + MAX_PAYLOAD_BYTES {
        return Err(format!(
            "Oxid artifact {} grew beyond the supported size while reading",
            path.display()
        ));
    }
    decode(&bytes).map_err(|error| format!("invalid Oxid artifact {}: {}", path.display(), error))
}

pub(super) fn is_artifact(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

#[allow(dead_code)]
pub(super) fn checksum_hex(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a64(bytes))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

struct Encoder {
    bytes: Vec<u8>,
    nodes: usize,
}

impl Encoder {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            nodes: 0,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn append(&mut self, bytes: &[u8]) -> Result<(), String> {
        let next_len = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| "Oxid artifact payload size overflow".to_string())?;
        if next_len > MAX_PAYLOAD_BYTES {
            return Err(format!(
                "Oxid artifact payload exceeds limit {}",
                MAX_PAYLOAD_BYTES
            ));
        }
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| "cannot allocate Oxid artifact payload".to_string())?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn write_u8(&mut self, value: u8) -> Result<(), String> {
        self.append(&[value])
    }

    fn write_u32(&mut self, value: u32) -> Result<(), String> {
        self.append(&value.to_le_bytes())
    }

    fn write_u64(&mut self, value: u64) -> Result<(), String> {
        self.append(&value.to_le_bytes())
    }

    fn write_bool(&mut self, value: bool) -> Result<(), String> {
        self.write_u8(u8::from(value))
    }

    fn write_count(&mut self, value: usize, label: &str) -> Result<(), String> {
        if value > MAX_COLLECTION_ITEMS {
            return Err(format!(
                "{} count {} exceeds limit {}",
                label, value, MAX_COLLECTION_ITEMS
            ));
        }
        let value = u32::try_from(value)
            .map_err(|_| format!("{} count cannot be represented as u32", label))?;
        self.write_u32(value)
    }

    fn write_usize(&mut self, value: usize, label: &str) -> Result<(), String> {
        let value =
            u64::try_from(value).map_err(|_| format!("{} cannot be represented as u64", label))?;
        self.write_u64(value)
    }

    fn write_string(&mut self, value: &str, label: &str) -> Result<(), String> {
        let length = value.len();
        if length > MAX_STRING_BYTES {
            return Err(format!(
                "{} length {} exceeds limit {}",
                label, length, MAX_STRING_BYTES
            ));
        }
        let length = u32::try_from(length)
            .map_err(|_| format!("{} length cannot be represented as u32", label))?;
        self.write_u32(length)?;
        self.append(value.as_bytes())
    }

    fn enter_node(&mut self, depth: usize, label: &str) -> Result<(), String> {
        if depth >= MAX_AST_DEPTH {
            return Err(format!(
                "Oxid AST {} nesting depth exceeds limit {}",
                label, MAX_AST_DEPTH
            ));
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| "Oxid AST node count overflow".to_string())?;
        if self.nodes > MAX_AST_NODES {
            return Err(format!(
                "Oxid AST node count exceeds limit {}",
                MAX_AST_NODES
            ));
        }
        Ok(())
    }

    fn write_program(&mut self, program: &Program) -> Result<(), String> {
        self.write_count(program.stmts.len(), "program statement")?;
        for statement in &program.stmts {
            self.write_stmt(statement, 0)?;
        }
        Ok(())
    }

    // Statement tags are a persistent part of AST version 1.
    fn write_stmt(&mut self, statement: &Stmt, depth: usize) -> Result<(), String> {
        self.enter_node(depth, "statement")?;
        match statement {
            Stmt::Let(name, value) => {
                self.write_u8(0)?;
                self.write_string(name, "variable name")?;
                self.write_expr(value, depth + 1)
            }
            Stmt::Const(name, value) => {
                self.write_u8(1)?;
                self.write_string(name, "constant name")?;
                self.write_expr(value, depth + 1)
            }
            Stmt::Print(value) => {
                self.write_u8(2)?;
                self.write_expr(value, depth + 1)
            }
            Stmt::Expr(value) => {
                self.write_u8(3)?;
                self.write_expr(value, depth + 1)
            }
            Stmt::Block(statements) => {
                self.write_u8(4)?;
                self.write_stmt_list(statements, depth + 1, "block statement")
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.write_u8(5)?;
                self.write_expr(cond, depth + 1)?;
                self.write_stmt(then_branch, depth + 1)?;
                self.write_bool(else_branch.is_some())?;
                if let Some(otherwise) = else_branch {
                    self.write_stmt(otherwise, depth + 1)?;
                }
                Ok(())
            }
            Stmt::While { cond, body } => {
                self.write_u8(6)?;
                self.write_expr(cond, depth + 1)?;
                self.write_stmt(body, depth + 1)
            }
            Stmt::For {
                name,
                iterable,
                body,
            } => {
                self.write_u8(7)?;
                self.write_string(name, "loop variable name")?;
                self.write_expr(iterable, depth + 1)?;
                self.write_stmt(body, depth + 1)
            }
            Stmt::Function {
                name,
                params,
                body,
                is_async,
            } => {
                self.write_u8(8)?;
                self.write_string(name, "function name")?;
                self.write_count(params.len(), "function parameter")?;
                for parameter in params {
                    self.write_string(parameter, "function parameter")?;
                }
                self.write_stmt_list(body, depth + 1, "function body statement")?;
                self.write_bool(*is_async)
            }
            Stmt::Return(value) => {
                self.write_u8(9)?;
                self.write_bool(value.is_some())?;
                if let Some(value) = value {
                    self.write_expr(value, depth + 1)?;
                }
                Ok(())
            }
            Stmt::Break => self.write_u8(10),
            Stmt::Continue => self.write_u8(11),
            Stmt::Use(path) => {
                self.write_u8(12)?;
                self.write_string(path, "module path")
            }
            Stmt::Located(span, statement) => {
                self.write_u8(13)?;
                self.write_source_span(span)?;
                self.write_stmt(statement, depth + 1)
            }
        }
    }

    fn write_stmt_list(
        &mut self,
        statements: &[Stmt],
        depth: usize,
        label: &str,
    ) -> Result<(), String> {
        self.write_count(statements.len(), label)?;
        for statement in statements {
            self.write_stmt(statement, depth)?;
        }
        Ok(())
    }

    // Expression tags are a persistent part of AST version 1.
    fn write_expr(&mut self, expression: &Expr, depth: usize) -> Result<(), String> {
        self.enter_node(depth, "expression")?;
        match expression {
            Expr::Literal(literal) => {
                self.write_u8(0)?;
                self.write_literal(literal)
            }
            Expr::Variable(name) => {
                self.write_u8(1)?;
                self.write_string(name, "variable name")
            }
            Expr::Assign(name, value) => {
                self.write_u8(2)?;
                self.write_string(name, "assignment name")?;
                self.write_expr(value, depth + 1)
            }
            Expr::AssignIndex(target, index, value) => {
                self.write_u8(3)?;
                self.write_expr(target, depth + 1)?;
                self.write_expr(index, depth + 1)?;
                self.write_expr(value, depth + 1)
            }
            Expr::Unary(operator, value) => {
                self.write_u8(4)?;
                self.write_unary_operator(operator)?;
                self.write_expr(value, depth + 1)
            }
            Expr::Binary(left, operator, right) => {
                self.write_u8(5)?;
                self.write_binary_operator(operator)?;
                self.write_expr(left, depth + 1)?;
                self.write_expr(right, depth + 1)
            }
            Expr::Logical(left, operator, right) => {
                self.write_u8(6)?;
                self.write_logical_operator(operator)?;
                self.write_expr(left, depth + 1)?;
                self.write_expr(right, depth + 1)
            }
            Expr::Call(callee, arguments) => {
                self.write_u8(7)?;
                self.write_expr(callee, depth + 1)?;
                self.write_count(arguments.len(), "call argument")?;
                for argument in arguments {
                    self.write_expr(argument, depth + 1)?;
                }
                Ok(())
            }
            Expr::Index(target, index) => {
                self.write_u8(8)?;
                self.write_expr(target, depth + 1)?;
                self.write_expr(index, depth + 1)
            }
            Expr::Array(items) => {
                self.write_u8(9)?;
                self.write_count(items.len(), "array item")?;
                for item in items {
                    self.write_expr(item, depth + 1)?;
                }
                Ok(())
            }
            Expr::Await(value) => {
                self.write_u8(10)?;
                self.write_expr(value, depth + 1)
            }
            Expr::Grouping(value) => {
                self.write_u8(11)?;
                self.write_expr(value, depth + 1)
            }
            Expr::Record(fields) => {
                self.write_u8(12)?;
                self.write_count(fields.len(), "record field")?;
                for (name, value) in fields {
                    self.write_string(name, "record field name")?;
                    self.write_expr(value, depth + 1)?;
                }
                Ok(())
            }
            Expr::Property(target, name) => {
                self.write_u8(13)?;
                self.write_expr(target, depth + 1)?;
                self.write_string(name, "property name")
            }
            Expr::AssignProperty(target, name, value) => {
                self.write_u8(14)?;
                self.write_expr(target, depth + 1)?;
                self.write_string(name, "property name")?;
                self.write_expr(value, depth + 1)
            }
        }
    }

    fn write_literal(&mut self, literal: &Literal) -> Result<(), String> {
        match literal {
            Literal::Number(value) => {
                self.write_u8(0)?;
                self.write_u64(value.to_bits())
            }
            Literal::String(value) => {
                self.write_u8(1)?;
                self.write_string(value, "string literal")
            }
            Literal::Bool(value) => {
                self.write_u8(2)?;
                self.write_bool(*value)
            }
            Literal::Null => self.write_u8(3),
        }
    }

    fn write_unary_operator(&mut self, operator: &TokenKind) -> Result<(), String> {
        let tag = match operator {
            TokenKind::Bang => 0,
            TokenKind::Minus => 1,
            other => {
                return Err(format!(
                    "unsupported unary operator in Oxid AST: {:?}",
                    other
                ))
            }
        };
        self.write_u8(tag)
    }

    fn write_binary_operator(&mut self, operator: &TokenKind) -> Result<(), String> {
        let tag = match operator {
            TokenKind::BangEqual => 0,
            TokenKind::EqualEqual => 1,
            TokenKind::Greater => 2,
            TokenKind::GreaterEqual => 3,
            TokenKind::Less => 4,
            TokenKind::LessEqual => 5,
            TokenKind::Minus => 6,
            TokenKind::Plus => 7,
            TokenKind::Slash => 8,
            TokenKind::Star => 9,
            TokenKind::Percent => 10,
            other => {
                return Err(format!(
                    "unsupported binary operator in Oxid AST: {:?}",
                    other
                ))
            }
        };
        self.write_u8(tag)
    }

    fn write_logical_operator(&mut self, operator: &TokenKind) -> Result<(), String> {
        let tag = match operator {
            TokenKind::And => 0,
            TokenKind::Or => 1,
            other => {
                return Err(format!(
                    "unsupported logical operator in Oxid AST: {:?}",
                    other
                ))
            }
        };
        self.write_u8(tag)
    }

    fn write_source_span(&mut self, span: &SourceSpan) -> Result<(), String> {
        self.write_string(&span.source, "source span path")?;
        self.write_usize(span.start_line, "source span start line")?;
        self.write_usize(span.start_col, "source span start column")?;
        self.write_usize(span.end_line, "source span end line")?;
        self.write_usize(span.end_col, "source span end column")
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    position: usize,
    nodes: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            nodes: 0,
        }
    }

    fn finish(&self) -> Result<(), String> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(format!(
                "{} trailing bytes remain after serialized Oxid AST",
                self.bytes.len() - self.position
            ))
        }
    }

    fn read_exact(&mut self, length: usize, label: &str) -> Result<&'a [u8], String> {
        let end = self.position.checked_add(length).ok_or_else(|| {
            format!(
                "serialized Oxid AST offset overflow while reading {}",
                label
            )
        })?;
        if end > self.bytes.len() {
            return Err(format!(
                "truncated serialized Oxid AST while reading {} at payload offset {}: need {} bytes, {} remain",
                label,
                self.position,
                length,
                self.bytes.len().saturating_sub(self.position)
            ));
        }
        let start = self.position;
        self.position = end;
        Ok(&self.bytes[start..end])
    }

    fn read_u8(&mut self, label: &str) -> Result<u8, String> {
        Ok(self.read_exact(1, label)?[0])
    }

    fn read_u32(&mut self, label: &str) -> Result<u32, String> {
        let bytes = self.read_exact(4, label)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_u64(&mut self, label: &str) -> Result<u64, String> {
        let bytes = self.read_exact(8, label)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn read_bool(&mut self, label: &str) -> Result<bool, String> {
        match self.read_u8(label)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!("invalid boolean value {} for {}", value, label)),
        }
    }

    fn read_count(&mut self, label: &str) -> Result<usize, String> {
        let count = usize::try_from(self.read_u32(label)?)
            .map_err(|_| format!("{} count is not supported on this platform", label))?;
        if count > MAX_COLLECTION_ITEMS {
            return Err(format!(
                "{} count {} exceeds limit {}",
                label, count, MAX_COLLECTION_ITEMS
            ));
        }
        Ok(count)
    }

    fn read_usize(&mut self, label: &str) -> Result<usize, String> {
        let value = self.read_u64(label)?;
        usize::try_from(value).map_err(|_| {
            format!(
                "{} value {} is not supported on this platform",
                label, value
            )
        })
    }

    fn read_string(&mut self, label: &str) -> Result<String, String> {
        let length = usize::try_from(self.read_u32(&format!("{} length", label))?)
            .map_err(|_| format!("{} length is not supported on this platform", label))?;
        if length > MAX_STRING_BYTES {
            return Err(format!(
                "{} length {} exceeds limit {}",
                label, length, MAX_STRING_BYTES
            ));
        }
        let bytes = self.read_exact(length, label)?;
        let value = std::str::from_utf8(bytes)
            .map_err(|error| format!("{} is not valid UTF-8: {}", label, error))?;
        Ok(value.to_string())
    }

    fn enter_node(&mut self, depth: usize, label: &str) -> Result<(), String> {
        if depth >= MAX_AST_DEPTH {
            return Err(format!(
                "serialized Oxid AST {} nesting depth exceeds limit {}",
                label, MAX_AST_DEPTH
            ));
        }
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| "serialized Oxid AST node count overflow".to_string())?;
        if self.nodes > MAX_AST_NODES {
            return Err(format!(
                "serialized Oxid AST node count exceeds limit {}",
                MAX_AST_NODES
            ));
        }
        Ok(())
    }

    fn read_program(&mut self) -> Result<Program, String> {
        let count = self.read_count("program statement")?;
        let mut statements = Vec::new();
        for _ in 0..count {
            statements.push(self.read_stmt(0)?);
        }
        Ok(Program { stmts: statements })
    }

    fn read_stmt(&mut self, depth: usize) -> Result<Stmt, String> {
        self.enter_node(depth, "statement")?;
        let tag = self.read_u8("statement tag")?;
        match tag {
            0 => {
                let name = self.read_string("variable name")?;
                let value = self.read_expr(depth + 1)?;
                Ok(Stmt::Let(name, value))
            }
            1 => {
                let name = self.read_string("constant name")?;
                let value = self.read_expr(depth + 1)?;
                Ok(Stmt::Const(name, value))
            }
            2 => Ok(Stmt::Print(self.read_expr(depth + 1)?)),
            3 => Ok(Stmt::Expr(self.read_expr(depth + 1)?)),
            4 => Ok(Stmt::Block(
                self.read_stmt_list(depth + 1, "block statement")?,
            )),
            5 => {
                let cond = self.read_expr(depth + 1)?;
                let then_branch = Box::new(self.read_stmt(depth + 1)?);
                let else_branch = if self.read_bool("if else presence")? {
                    Some(Box::new(self.read_stmt(depth + 1)?))
                } else {
                    None
                };
                Ok(Stmt::If {
                    cond,
                    then_branch,
                    else_branch,
                })
            }
            6 => {
                let cond = self.read_expr(depth + 1)?;
                let body = Box::new(self.read_stmt(depth + 1)?);
                Ok(Stmt::While { cond, body })
            }
            7 => {
                let name = self.read_string("loop variable name")?;
                let iterable = self.read_expr(depth + 1)?;
                let body = Box::new(self.read_stmt(depth + 1)?);
                Ok(Stmt::For {
                    name,
                    iterable,
                    body,
                })
            }
            8 => {
                let name = self.read_string("function name")?;
                let parameter_count = self.read_count("function parameter")?;
                let mut params = Vec::new();
                for _ in 0..parameter_count {
                    params.push(self.read_string("function parameter")?);
                }
                let body = self.read_stmt_list(depth + 1, "function body statement")?;
                let is_async = self.read_bool("async function flag")?;
                Ok(Stmt::Function {
                    name,
                    params,
                    body,
                    is_async,
                })
            }
            9 => {
                let value = if self.read_bool("return value presence")? {
                    Some(self.read_expr(depth + 1)?)
                } else {
                    None
                };
                Ok(Stmt::Return(value))
            }
            10 => Ok(Stmt::Break),
            11 => Ok(Stmt::Continue),
            12 => Ok(Stmt::Use(self.read_string("module path")?)),
            13 => {
                let span = self.read_source_span()?;
                let statement = Box::new(self.read_stmt(depth + 1)?);
                Ok(Stmt::Located(span, statement))
            }
            other => Err(format!(
                "unknown statement tag {} in Oxid AST version 1",
                other
            )),
        }
    }

    fn read_stmt_list(&mut self, depth: usize, label: &str) -> Result<Vec<Stmt>, String> {
        let count = self.read_count(label)?;
        let mut statements = Vec::new();
        for _ in 0..count {
            statements.push(self.read_stmt(depth)?);
        }
        Ok(statements)
    }

    fn read_expr(&mut self, depth: usize) -> Result<Expr, String> {
        self.enter_node(depth, "expression")?;
        let tag = self.read_u8("expression tag")?;
        match tag {
            0 => Ok(Expr::Literal(self.read_literal()?)),
            1 => Ok(Expr::Variable(self.read_string("variable name")?)),
            2 => {
                let name = self.read_string("assignment name")?;
                let value = Box::new(self.read_expr(depth + 1)?);
                Ok(Expr::Assign(name, value))
            }
            3 => {
                let target = Box::new(self.read_expr(depth + 1)?);
                let index = Box::new(self.read_expr(depth + 1)?);
                let value = Box::new(self.read_expr(depth + 1)?);
                Ok(Expr::AssignIndex(target, index, value))
            }
            4 => {
                let operator = self.read_unary_operator()?;
                let value = Box::new(self.read_expr(depth + 1)?);
                Ok(Expr::Unary(operator, value))
            }
            5 => {
                let operator = self.read_binary_operator()?;
                let left = Box::new(self.read_expr(depth + 1)?);
                let right = Box::new(self.read_expr(depth + 1)?);
                Ok(Expr::Binary(left, operator, right))
            }
            6 => {
                let operator = self.read_logical_operator()?;
                let left = Box::new(self.read_expr(depth + 1)?);
                let right = Box::new(self.read_expr(depth + 1)?);
                Ok(Expr::Logical(left, operator, right))
            }
            7 => {
                let callee = Box::new(self.read_expr(depth + 1)?);
                let argument_count = self.read_count("call argument")?;
                let mut arguments = Vec::new();
                for _ in 0..argument_count {
                    arguments.push(self.read_expr(depth + 1)?);
                }
                Ok(Expr::Call(callee, arguments))
            }
            8 => {
                let target = Box::new(self.read_expr(depth + 1)?);
                let index = Box::new(self.read_expr(depth + 1)?);
                Ok(Expr::Index(target, index))
            }
            9 => {
                let item_count = self.read_count("array item")?;
                let mut items = Vec::new();
                for _ in 0..item_count {
                    items.push(self.read_expr(depth + 1)?);
                }
                Ok(Expr::Array(items))
            }
            10 => Ok(Expr::Await(Box::new(self.read_expr(depth + 1)?))),
            11 => Ok(Expr::Grouping(Box::new(self.read_expr(depth + 1)?))),
            12 => {
                let field_count = self.read_count("record field")?;
                let mut fields = Vec::new();
                for _ in 0..field_count {
                    let name = self.read_string("record field name")?;
                    let value = self.read_expr(depth + 1)?;
                    fields.push((name, value));
                }
                Ok(Expr::Record(fields))
            }
            13 => {
                let target = Box::new(self.read_expr(depth + 1)?);
                let name = self.read_string("property name")?;
                Ok(Expr::Property(target, name))
            }
            14 => {
                let target = Box::new(self.read_expr(depth + 1)?);
                let name = self.read_string("property name")?;
                let value = Box::new(self.read_expr(depth + 1)?);
                Ok(Expr::AssignProperty(target, name, value))
            }
            other => Err(format!(
                "unknown expression tag {} in Oxid AST version 1",
                other
            )),
        }
    }

    fn read_literal(&mut self) -> Result<Literal, String> {
        match self.read_u8("literal tag")? {
            0 => Ok(Literal::Number(f64::from_bits(
                self.read_u64("number literal")?,
            ))),
            1 => Ok(Literal::String(self.read_string("string literal")?)),
            2 => Ok(Literal::Bool(self.read_bool("boolean literal")?)),
            3 => Ok(Literal::Null),
            other => Err(format!(
                "unknown literal tag {} in Oxid AST version 1",
                other
            )),
        }
    }

    fn read_unary_operator(&mut self) -> Result<TokenKind, String> {
        match self.read_u8("unary operator")? {
            0 => Ok(TokenKind::Bang),
            1 => Ok(TokenKind::Minus),
            other => Err(format!("unknown unary operator tag {}", other)),
        }
    }

    fn read_binary_operator(&mut self) -> Result<TokenKind, String> {
        match self.read_u8("binary operator")? {
            0 => Ok(TokenKind::BangEqual),
            1 => Ok(TokenKind::EqualEqual),
            2 => Ok(TokenKind::Greater),
            3 => Ok(TokenKind::GreaterEqual),
            4 => Ok(TokenKind::Less),
            5 => Ok(TokenKind::LessEqual),
            6 => Ok(TokenKind::Minus),
            7 => Ok(TokenKind::Plus),
            8 => Ok(TokenKind::Slash),
            9 => Ok(TokenKind::Star),
            10 => Ok(TokenKind::Percent),
            other => Err(format!("unknown binary operator tag {}", other)),
        }
    }

    fn read_logical_operator(&mut self) -> Result<TokenKind, String> {
        match self.read_u8("logical operator")? {
            0 => Ok(TokenKind::And),
            1 => Ok(TokenKind::Or),
            other => Err(format!("unknown logical operator tag {}", other)),
        }
    }

    fn read_source_span(&mut self) -> Result<SourceSpan, String> {
        Ok(SourceSpan {
            source: self.read_string("source span path")?,
            start_line: self.read_usize("source span start line")?,
            start_col: self.read_usize("source span start column")?,
            end_line: self.read_usize("source span end line")?,
            end_col: self.read_usize("source span end column")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comprehensive_program() -> Program {
        let number = || Expr::Literal(Literal::Number(42.5));
        let boolean = || Expr::Literal(Literal::Bool(true));
        let variable = || Expr::Variable("value".to_string());

        Program {
            stmts: vec![
                Stmt::Located(
                    SourceSpan {
                        source: "src/main.ox".to_string(),
                        start_line: 1,
                        start_col: 1,
                        end_line: 1,
                        end_col: 20,
                    },
                    Box::new(Stmt::Let(
                        "record".to_string(),
                        Expr::Record(vec![
                            (
                                "name".to_string(),
                                Expr::Literal(Literal::String("Oxid".to_string())),
                            ),
                            ("enabled".to_string(), boolean()),
                            ("missing".to_string(), Expr::Literal(Literal::Null)),
                        ]),
                    )),
                ),
                Stmt::Const(
                    "items".to_string(),
                    Expr::Array(vec![number(), Expr::Grouping(Box::new(variable()))]),
                ),
                Stmt::Print(Expr::Property(
                    Box::new(Expr::Variable("record".to_string())),
                    "name".to_string(),
                )),
                Stmt::Expr(Expr::Assign(
                    "value".to_string(),
                    Box::new(Expr::Binary(
                        Box::new(number()),
                        TokenKind::Plus,
                        Box::new(Expr::Unary(TokenKind::Minus, Box::new(number()))),
                    )),
                )),
                Stmt::Expr(Expr::AssignIndex(
                    Box::new(Expr::Variable("items".to_string())),
                    Box::new(Expr::Literal(Literal::Number(0.0))),
                    Box::new(number()),
                )),
                Stmt::Expr(Expr::AssignProperty(
                    Box::new(Expr::Variable("record".to_string())),
                    "enabled".to_string(),
                    Box::new(Expr::Literal(Literal::Bool(false))),
                )),
                Stmt::If {
                    cond: Expr::Logical(
                        Box::new(boolean()),
                        TokenKind::And,
                        Box::new(Expr::Unary(TokenKind::Bang, Box::new(boolean()))),
                    ),
                    then_branch: Box::new(Stmt::Block(vec![Stmt::Continue])),
                    else_branch: Some(Box::new(Stmt::Break)),
                },
                Stmt::While {
                    cond: Expr::Binary(
                        Box::new(number()),
                        TokenKind::GreaterEqual,
                        Box::new(number()),
                    ),
                    body: Box::new(Stmt::Return(None)),
                },
                Stmt::For {
                    name: "item".to_string(),
                    iterable: Expr::Index(
                        Box::new(Expr::Variable("items".to_string())),
                        Box::new(Expr::Literal(Literal::Number(0.0))),
                    ),
                    body: Box::new(Stmt::Print(variable())),
                },
                Stmt::Function {
                    name: "fetch".to_string(),
                    params: vec!["url".to_string()],
                    body: vec![Stmt::Return(Some(Expr::Await(Box::new(Expr::Call(
                        Box::new(Expr::Variable("request".to_string())),
                        vec![Expr::Variable("url".to_string())],
                    )))))],
                    is_async: true,
                },
                Stmt::Use("stdlib/web.ox".to_string()),
            ],
        }
    }

    #[test]
    fn deterministic_roundtrip_covers_all_ast_shapes() {
        let program = comprehensive_program();
        let first = encode(&program, 3).expect("first encoding");
        let second = encode(&program, 3).expect("second encoding");
        assert_eq!(first, second);
        assert!(is_artifact(&first));

        let decoded = decode(&first).expect("decode artifact");
        assert_eq!(decoded.module_count, 3);
        assert_eq!(decoded.checksum, fnv1a64(&first[HEADER_LEN..]));
        assert_eq!(format!("{:?}", decoded.program), format!("{:?}", program));
        assert_eq!(
            encode(&decoded.program, decoded.module_count).expect("re-encode artifact"),
            first
        );
        assert_eq!(checksum_hex(b"hello"), "a430d84680aabd0b");
    }

    #[test]
    fn unknown_major_version_is_rejected() {
        let mut bytes = encode(&comprehensive_program(), 1).expect("encode artifact");
        bytes[4..6].copy_from_slice(&(FORMAT_MAJOR + 1).to_le_bytes());
        let error = decode(&bytes).expect_err("unknown major must fail");
        assert!(error.contains("incompatible Oxid artifact format major version"));
    }

    #[test]
    fn truncated_payload_is_rejected() {
        let mut bytes = encode(&comprehensive_program(), 1).expect("encode artifact");
        bytes.pop();
        let error = decode(&bytes).expect_err("truncated payload must fail");
        assert!(error.contains("truncated Oxid artifact payload"));
    }

    #[test]
    fn checksum_corruption_is_rejected() {
        let mut bytes = encode(&comprehensive_program(), 1).expect("encode artifact");
        let last = bytes.last_mut().expect("artifact payload");
        *last ^= 0x80;
        let error = decode(&bytes).expect_err("checksum corruption must fail");
        assert!(error.contains("Oxid artifact checksum mismatch"));
    }
}
