//! Strict contextual LXI1/LXS1 framing. The source owner, never wire bytes,
//! supplies span provenance. This decoder does not decide lexical correctness.
use crate::frontend::{
    lexer::{Kind, Token, MAX_TOKENS, MAX_TOKEN_BYTES},
    project::budget::Allocator,
    source::{SourceFile, MAX_SOURCE_BYTES},
};

pub(super) fn output_bound(n: usize, limit: usize) -> Result<usize, &'static str> {
    if n > MAX_SOURCE_BYTES || limit > MAX_TOKENS {
        return Err("lexical provider input domain exceeded");
    }
    let chunks = n / 128 + 1;
    Ok(4 + 134 * chunks + 2 * (n.min(limit) + 1) + 102 * (chunks + 1) + 10)
}

pub(super) fn request(
    source: &SourceFile,
    limit: usize,
    allocator: &mut Allocator,
) -> Result<Vec<u8>, &'static str> {
    output_bound(source.text().len(), limit)?;
    if !source.text().is_ascii() {
        return Err("lexical provider requires ASCII source");
    }
    let length = source.text().len() + 12;
    let mut bytes = Vec::new();
    reserve(&mut bytes, length, allocator, "lexical provider input")?;
    bytes.extend_from_slice(b"LXI1");
    bytes.extend_from_slice(&(source.text().len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(limit as u32).to_le_bytes());
    bytes.extend_from_slice(source.text().as_bytes());
    Ok(bytes)
}

pub(super) fn reserve<T>(
    vector: &mut Vec<T>,
    count: usize,
    allocator: &mut Allocator,
    name: &'static str,
) -> Result<(), &'static str> {
    allocator
        .vector_exact(vector, count, name)
        .map_err(|_| "lexical provider allocation failed")?;
    if vector.capacity() != count {
        return Err("lexical provider allocation capacity mismatch");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(super) struct LexicalDiagnostic {
    pub tag: u8,
    pub start: usize,
    pub end: usize,
}
impl LexicalDiagnostic {
    pub(super) fn fields(self) -> (&'static str, &'static str) {
        match self.tag {
            1 => ("E0100", "unterminated string literal"),
            2 => ("E0100", "unterminated block comment"),
            3 => ("E0400", "token resource limit exceeded"),
            _ => unreachable!("decoder validates diagnostic tag"),
        }
    }
}
#[derive(Debug)]
pub(super) enum Observation {
    Tokens(Vec<Token>),
    Diagnostic(LexicalDiagnostic),
}

// Protocol numbers are explicitly one-based. Never transmute Rust discriminants.
fn kind(id: u8) -> Option<Kind> {
    use Kind::*;
    Some(match id {
        1 => Trivia,
        2 => Ident,
        3 => Number,
        4 => String,
        5 => Fn,
        6 => Struct,
        7 => Mod,
        8 => Use,
        9 => Pub,
        10 => Ampersand,
        11 => Dot,
        12 => Let,
        13 => Mut,
        14 => Return,
        15 => Break,
        16 => Continue,
        17 => If,
        18 => While,
        19 => Else,
        20 => True,
        21 => False,
        22 => LParen,
        23 => RParen,
        24 => LBrace,
        25 => RBrace,
        26 => Colon,
        27 => Comma,
        28 => Semi,
        29 => Equal,
        30 => EqualEqual,
        31 => NotEqual,
        32 => Not,
        33 => AndAnd,
        34 => OrOr,
        35 => Less,
        36 => LessEqual,
        37 => Greater,
        38 => GreaterEqual,
        39 => Arrow,
        40 => Minus,
        41 => Plus,
        42 => Star,
        43 => Slash,
        44 => Percent,
        45 => Unsupported,
        46 => Invalid,
        47 => Eof,
        _ => return None,
    })
}

struct Decoder<'a> {
    source: &'a SourceFile,
    bytes: &'a [u8],
    offset: usize,
    limit: usize,
    base: Option<usize>,
    echoed: usize,
    final_echo: bool,
    echo_count: usize,
    echo_buffer: [u8; 128],
    token_buffer: [u8; 100],
    raw_group: [u8; 100],
    raw_used: usize,
    partial_seen: bool,
    previous: usize,
    eof: bool,
    tokens: Vec<Token>,
    token_count: usize,
    collect: bool,
}
impl Decoder<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], &'static str> {
        let end = self.offset.checked_add(n).ok_or("lexical frame overflow")?;
        let part = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated lexical frame")?;
        self.offset = end;
        Ok(part)
    }
    fn pair(&mut self, id: u8, relative: u8) -> Result<(), &'static str> {
        let base = self.base.ok_or("lexical token precedes source echo")?;
        let kind = kind(id).ok_or("invalid lexical token kind")?;
        let end = base
            .checked_add(usize::from(relative))
            .ok_or("lexical token overflow")?;
        if self.eof || relative > 128 || end < self.previous || end > self.echoed {
            return Err("invalid lexical token extent");
        }
        if kind == Kind::Eof {
            if !self.final_echo || self.previous != end || end != self.source.text().len() {
                return Err("invalid lexical EOF");
            }
            self.eof = true;
        } else if end == self.previous
            || end - self.previous > MAX_TOKEN_BYTES
            || self.token_count >= self.limit
        {
            return Err("invalid lexical token width or count");
        }
        if self.collect {
            if self.tokens.len() == self.tokens.capacity() {
                return Err("lexical token capacity exceeded");
            }
            self.tokens.push(Token {
                kind,
                span: self.source.span(self.previous, end),
            });
        }
        self.token_count += 1;
        self.previous = end;
        Ok(())
    }
    fn finish(mut self) -> Result<(Observation, usize), &'static str> {
        while let Some(&tag) = self.bytes.get(self.offset) {
            match tag {
                1..=47 => {
                    if self.partial_seen {
                        return Err("lexical tokens after partial group");
                    }
                    let pair: [u8; 2] = self.take(2)?.try_into().unwrap();
                    self.raw_group[self.raw_used..self.raw_used + 2].copy_from_slice(&pair);
                    self.raw_used += 2;
                    self.pair(pair[0], pair[1])?;
                    if self.raw_used == 100 {
                        self.token_buffer = self.raw_group;
                        self.raw_used = 0;
                    }
                }
                b'B' => {
                    if self.base.is_none() || self.raw_used != 0 || self.eof || self.partial_seen {
                        return Err("invalid lexical partial group state");
                    }
                    let record: [u8; 102] = self.take(102)?.try_into().unwrap();
                    let used = usize::from(record[1]);
                    if !(2..=98).contains(&used)
                        || used % 2 != 0
                        || record[2 + used..] != self.token_buffer[used..]
                    {
                        return Err("invalid lexical partial group size or padding");
                    }
                    for pair in record[2..2 + used].as_chunks::<2>().0 {
                        self.pair(pair[0], pair[1])?;
                    }
                    self.token_buffer.copy_from_slice(&record[2..]);
                    self.partial_seen = true;
                }
                b'E' => {
                    if self.raw_used != 0 || self.eof || self.final_echo {
                        return Err("invalid lexical source echo state");
                    }
                    let record: [u8; 134] = self.take(134)?.try_into().unwrap();
                    let used = usize::from(record[1]);
                    let base = u32::from_le_bytes(record[2..6].try_into().unwrap()) as usize;
                    let expected = 128.min(self.source.text().len() - self.echoed);
                    if base != self.echoed || used != expected {
                        return Err("invalid lexical source echo extent");
                    }
                    if record[6..6 + used]
                        != self.source.text().as_bytes()[self.echoed..self.echoed + used]
                        || record[6 + used..] != self.echo_buffer[used..]
                    {
                        return Err("lexical source echo or deterministic padding mismatch");
                    }
                    self.echo_buffer.copy_from_slice(&record[6..]);
                    self.base = Some(base);
                    self.echoed += used;
                    self.echo_count += 1;
                    self.partial_seen = false;
                    self.final_echo = used < 128;
                }
                b'S' | b'D' => {
                    let n = self.source.text().len();
                    if self.raw_used != 0
                        || !self.final_echo
                        || self.echoed != n
                        || self.echo_count != n / 128 + 1
                    {
                        return Err("incomplete lexical source or token group");
                    }
                    let observation = if tag == b'S' {
                        let record: [u8; 9] = self.take(9)?.try_into().unwrap();
                        let count = u32::from_le_bytes(record[1..5].try_into().unwrap()) as usize;
                        let extent = u32::from_le_bytes(record[5..9].try_into().unwrap()) as usize;
                        if !self.eof || count != self.token_count || extent != n {
                            return Err("invalid lexical success counters");
                        }
                        Observation::Tokens(self.tokens)
                    } else {
                        let record: [u8; 10] = self.take(10)?.try_into().unwrap();
                        let start = u32::from_le_bytes(record[2..6].try_into().unwrap()) as usize;
                        let end = u32::from_le_bytes(record[6..10].try_into().unwrap()) as usize;
                        let tag = record[1];
                        if self.eof
                            || !(1..=3).contains(&tag)
                            || start != self.previous
                            || start >= end
                            || end > n
                            || (tag != 3 && end != n)
                        {
                            return Err("invalid lexical diagnostic fields");
                        }
                        Observation::Diagnostic(LexicalDiagnostic { tag, start, end })
                    };
                    if self.offset != self.bytes.len() {
                        return Err("trailing lexical output");
                    }
                    return Ok((observation, self.token_count));
                }
                _ => return Err("unknown lexical frame tag"),
            }
        }
        Err("missing lexical terminal")
    }
}

pub(super) fn decode(
    source: &SourceFile,
    limit: usize,
    bytes: &[u8],
    allocator: &mut Allocator,
) -> Result<Observation, &'static str> {
    let bound = output_bound(source.text().len(), limit)?;
    if !source.text().is_ascii() || bytes.len() > bound || bytes.get(..4) != Some(b"LXS1") {
        return Err("invalid lexical frame domain, magic or extent");
    }
    let decoder = |tokens, collect| Decoder {
        source,
        bytes,
        offset: 4,
        limit,
        base: None,
        echoed: 0,
        final_echo: false,
        echo_count: 0,
        echo_buffer: [0; 128],
        token_buffer: [0; 100],
        raw_group: [0; 100],
        raw_used: 0,
        partial_seen: false,
        previous: 0,
        eof: false,
        tokens,
        token_count: 0,
        collect,
    };
    // A strict allocation-free pass establishes the actual complete count.
    // Allocate exactly those slots, so the producer Vec moved into the AST has
    // no new unaccounted spare capacity after transient provider state drops.
    // Both passes use the same immutable captured bytes and retained source.
    let (preflight, count) = decoder(Vec::new(), false).finish()?;
    if matches!(preflight, Observation::Diagnostic(_)) {
        return Ok(preflight);
    }
    let mut tokens = Vec::new();
    reserve(&mut tokens, count, allocator, "lexical provider tokens")?;
    let (observation, decoded_count) = decoder(tokens, true).finish()?;
    if decoded_count != count {
        return Err("lexical token preflight changed");
    }
    Ok(observation)
}

pub(super) fn named_bytes() -> usize {
    3 * std::mem::size_of::<Decoder<'static>>()
        + 3 * std::mem::size_of::<Observation>()
        + 3 * std::mem::size_of::<[u8; 134]>()
        + 3 * std::mem::size_of::<[u8; 102]>()
}
