use super::{RuntimeError, Value};

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT_MS: usize = 30_000;
const MAX_TIMEOUT_MS: usize = 300_000;
const MAX_NET_READ_BYTES: usize = 1024 * 1024;
const MAX_NET_WRITE_BYTES: usize = 8 * 1024 * 1024;
const MAX_HTTP_HEADER_BYTES: usize = 32 * 1024;
const MAX_HTTP_LINE_BYTES: usize = 8 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
const MAX_HTTP_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const IO_POLL_INTERVAL: Duration = Duration::from_millis(2);

#[derive(Debug)]
pub(super) struct ListenerHandle {
    inner: RefCell<Option<TcpListener>>,
    local_addr: SocketAddr,
}

#[derive(Debug)]
pub(super) struct ConnectionHandle {
    inner: RefCell<Option<TcpStream>>,
    pending: RefCell<Vec<u8>>,
    local_addr: SocketAddr,
    peer_addr: SocketAddr,
}

impl ListenerHandle {
    pub(super) fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl ConnectionHandle {
    pub(super) fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }
}

fn runtime_error(message: impl Into<String>) -> RuntimeError {
    RuntimeError::Message(message.into())
}

fn string_argument(value: &Value, label: &str) -> Result<String, RuntimeError> {
    match value {
        Value::String(value) => Ok(value.clone()),
        _ => Err(runtime_error(format!("{label} must be a string"))),
    }
}

fn integer_argument(value: &Value, label: &str, maximum: usize) -> Result<usize, RuntimeError> {
    match value {
        Value::Number(value)
            if value.is_finite()
                && *value >= 0.0
                && value.fract() == 0.0
                && *value <= maximum as f64 =>
        {
            Ok(*value as usize)
        }
        _ => Err(runtime_error(format!(
            "{label} must be an integer between 0 and {maximum}"
        ))),
    }
}

fn listener_argument(value: &Value, label: &str) -> Result<Rc<ListenerHandle>, RuntimeError> {
    match value {
        Value::Listener(listener) => Ok(listener.clone()),
        _ => Err(runtime_error(format!("{label} must be a network listener"))),
    }
}

fn connection_argument(value: &Value, label: &str) -> Result<Rc<ConnectionHandle>, RuntimeError> {
    match value {
        Value::Connection(connection) => Ok(connection.clone()),
        _ => Err(runtime_error(format!(
            "{label} must be a network connection"
        ))),
    }
}

fn timeout_argument(value: &Value, label: &str) -> Result<Duration, RuntimeError> {
    integer_argument(value, label, MAX_TIMEOUT_MS).map(|value| Duration::from_millis(value as u64))
}

fn default_timeout() -> Duration {
    Duration::from_millis(DEFAULT_TIMEOUT_MS as u64)
}

fn record_value(entries: impl IntoIterator<Item = (String, Value)>) -> Value {
    Value::Record(Rc::new(RefCell::new(entries.into_iter().collect())))
}

pub(super) fn net_listen(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(runtime_error("net_listen requires host and port"));
    }
    let host = string_argument(&args[0], "net_listen host")?;
    if host.is_empty() || host.len() > 255 || host.chars().any(char::is_control) {
        return Err(runtime_error("net_listen host is invalid"));
    }
    let port = integer_argument(&args[1], "net_listen port", u16::MAX as usize)? as u16;
    let listener = TcpListener::bind((host.as_str(), port))
        .map_err(|error| runtime_error(format!("cannot bind network listener: {error}")))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| runtime_error(format!("cannot configure network listener: {error}")))?;
    let local_addr = listener
        .local_addr()
        .map_err(|error| runtime_error(format!("cannot read listener address: {error}")))?;
    Ok(Value::Listener(Rc::new(ListenerHandle {
        inner: RefCell::new(Some(listener)),
        local_addr,
    })))
}

pub(super) fn net_local_addr(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(runtime_error("net_local_addr requires exactly 1 listener"));
    }
    let listener = listener_argument(&args[0], "net_local_addr value")?;
    let is_open = listener
        .inner
        .try_borrow()
        .map_err(|_| runtime_error("network listener is already mutably borrowed"))?
        .is_some();
    if !is_open {
        return Err(runtime_error("network listener is closed"));
    }
    Ok(record_value([
        (
            "host".to_string(),
            Value::String(listener.local_addr.ip().to_string()),
        ),
        (
            "port".to_string(),
            Value::Number(listener.local_addr.port() as f64),
        ),
    ]))
}

pub(super) fn net_accept(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 2 {
        return Err(runtime_error(
            "net_accept requires a listener and timeout in milliseconds",
        ));
    }
    let listener = listener_argument(&args[0], "net_accept listener")?;
    let timeout = timeout_argument(&args[1], "net_accept timeout")?;
    accept_connection(&listener, timeout)
        .map(Value::Connection)
        .map_err(runtime_error)
}

pub(super) fn net_try_accept(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(runtime_error("net_try_accept requires exactly 1 listener"));
    }
    let listener = listener_argument(&args[0], "net_try_accept listener")?;
    try_accept_connection(&listener)
        .map(|connection| connection.map(Value::Connection).unwrap_or(Value::Null))
        .map_err(runtime_error)
}

pub(super) fn net_read(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 3 {
        return Err(runtime_error(
            "net_read requires a connection, byte limit, and timeout in milliseconds",
        ));
    }
    let connection = connection_argument(&args[0], "net_read connection")?;
    let max_bytes = integer_argument(&args[1], "net_read byte limit", MAX_NET_READ_BYTES)?;
    if max_bytes == 0 {
        return Err(runtime_error(
            "net_read byte limit must be greater than zero",
        ));
    }
    let timeout = timeout_argument(&args[2], "net_read timeout")?;

    let pending = take_pending(&connection, max_bytes).map_err(runtime_error)?;
    let bytes = if pending.is_empty() {
        read_connection_chunk_until(&connection, max_bytes, deadline(timeout), "net_read")
            .map_err(runtime_error)?
            .unwrap_or_default()
    } else {
        pending
    };
    let text = String::from_utf8(bytes)
        .map_err(|_| runtime_error("net_read received bytes that are not valid UTF-8"))?;
    Ok(Value::String(text))
}

pub(super) fn net_write(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&args.len()) {
        return Err(runtime_error(
            "net_write requires a connection, string data, and optional timeout",
        ));
    }
    let connection = connection_argument(&args[0], "net_write connection")?;
    let data = string_argument(&args[1], "net_write data")?;
    if data.len() > MAX_NET_WRITE_BYTES {
        return Err(runtime_error(format!(
            "net_write data exceeds the {MAX_NET_WRITE_BYTES}-byte limit"
        )));
    }
    let timeout = match args.get(2) {
        Some(value) => timeout_argument(value, "net_write timeout")?,
        None => default_timeout(),
    };
    write_connection_until(&connection, data.as_bytes(), deadline(timeout), "net_write")
        .map_err(runtime_error)?;
    Ok(Value::Number(data.len() as f64))
}

pub(super) fn net_close(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(runtime_error(
            "net_close requires exactly 1 listener or connection",
        ));
    }
    match &args[0] {
        Value::Listener(listener) => {
            listener
                .inner
                .try_borrow_mut()
                .map_err(|_| runtime_error("network listener is already borrowed"))?
                .take();
        }
        Value::Connection(connection) => close_connection(connection).map_err(runtime_error)?,
        _ => {
            return Err(runtime_error(
                "net_close requires a network listener or connection",
            ));
        }
    }
    Ok(Value::Null)
}

pub(super) fn http_read_request(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !(1..=2).contains(&args.len()) {
        return Err(runtime_error(
            "http_read_request requires a connection and optional timeout",
        ));
    }
    let connection = connection_argument(&args[0], "http_read_request connection")?;
    let timeout = match args.get(1) {
        Some(value) => timeout_argument(value, "http_read_request timeout")?,
        None => default_timeout(),
    };
    read_http_request(&connection, timeout).map_err(runtime_error)
}

pub(super) fn http_write_response(args: Vec<Value>) -> Result<Value, RuntimeError> {
    if !(2..=3).contains(&args.len()) {
        return Err(runtime_error(
            "http_write_response requires a connection, response, and optional timeout",
        ));
    }
    let connection = connection_argument(&args[0], "http_write_response connection")?;
    let response = encode_http_response(&args[1]).map_err(runtime_error)?;
    let timeout = match args.get(2) {
        Some(value) => timeout_argument(value, "http_write_response timeout")?,
        None => default_timeout(),
    };
    write_connection_until(
        &connection,
        &response,
        deadline(timeout),
        "http_write_response",
    )
    .map_err(runtime_error)?;
    Ok(Value::Number(response.len() as f64))
}

pub(super) fn web_serve_once(args: Vec<Value>) -> Result<Value, RuntimeError> {
    let (listener, response, timeout) = match args.first() {
        Some(Value::Listener(listener)) if (2..=3).contains(&args.len()) => {
            let timeout = match args.get(2) {
                Some(value) => timeout_argument(value, "web_serve_once timeout")?,
                None => default_timeout(),
            };
            (listener.clone(), &args[1], timeout)
        }
        Some(Value::String(_)) if (3..=4).contains(&args.len()) => {
            let host = string_argument(&args[0], "web_serve_once host")?;
            let port = integer_argument(&args[1], "web_serve_once port", u16::MAX as usize)?;
            let timeout = match args.get(3) {
                Some(value) => timeout_argument(value, "web_serve_once timeout")?,
                None => default_timeout(),
            };
            let value = net_listen(vec![Value::String(host), Value::Number(port as f64)])?;
            let Value::Listener(listener) = value else {
                return Err(runtime_error("failed to create web listener"));
            };
            (listener, &args[2], timeout)
        }
        _ => {
            return Err(runtime_error(
                "web_serve_once requires listener, response, optional timeout; or host, port, response, optional timeout",
            ));
        }
    };

    let encoded_response = encode_http_response(response).map_err(runtime_error)?;
    let connection = accept_connection(&listener, timeout).map_err(runtime_error)?;
    let result: Result<Value, String> = (|| {
        let request = read_http_request(&connection, timeout)?;
        write_connection_until(
            &connection,
            &encoded_response,
            deadline(timeout),
            "web response",
        )?;
        Ok(request)
    })();
    let close_result = close_connection(&connection);
    match (result, close_result) {
        (Ok(request), Ok(())) => Ok(request),
        (Err(error), _) => Err(runtime_error(error)),
        (Ok(_), Err(error)) => Err(runtime_error(error)),
    }
}

fn accept_connection(
    listener: &Rc<ListenerHandle>,
    timeout: Duration,
) -> Result<Rc<ConnectionHandle>, String> {
    let deadline = deadline(timeout);
    loop {
        if let Some(connection) = try_accept_connection(listener)? {
            return Ok(connection);
        }
        if !wait_for_io(deadline) {
            return Err("net_accept timed out".to_string());
        }
    }
}

fn try_accept_connection(listener: &Rc<ListenerHandle>) -> Result<Option<Rc<ConnectionHandle>>, String> {
    loop {
        let accepted = {
            let listener = listener
                .inner
                .try_borrow()
                .map_err(|_| "network listener is already mutably borrowed".to_string())?;
            let listener = listener
                .as_ref()
                .ok_or_else(|| "network listener is closed".to_string())?;
            listener.accept()
        };
        match accepted {
            Ok((stream, peer_addr)) => {
                stream
                    .set_nonblocking(true)
                    .map_err(|error| format!("cannot configure accepted connection: {error}"))?;
                let _ = stream.set_nodelay(true);
                let local_addr = stream
                    .local_addr()
                    .map_err(|error| format!("cannot read connection address: {error}"))?;
                return Ok(Some(Rc::new(ConnectionHandle {
                    inner: RefCell::new(Some(stream)),
                    pending: RefCell::new(Vec::new()),
                    local_addr,
                    peer_addr,
                })));
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(None),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("cannot accept network connection: {error}")),
        }
    }
}

fn close_connection(connection: &Rc<ConnectionHandle>) -> Result<(), String> {
    let stream = connection
        .inner
        .try_borrow_mut()
        .map_err(|_| "network connection is already borrowed".to_string())?
        .take();
    if let Some(stream) = stream {
        match stream.shutdown(Shutdown::Both) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotConnected | io::ErrorKind::ConnectionReset
                ) => {}
            Err(error) => {
                return Err(format!("cannot close network connection: {error}"));
            }
        }
    }
    connection
        .pending
        .try_borrow_mut()
        .map_err(|_| "network read buffer is already borrowed".to_string())?
        .clear();
    Ok(())
}

fn take_pending(connection: &Rc<ConnectionHandle>, limit: usize) -> Result<Vec<u8>, String> {
    let mut pending = connection
        .pending
        .try_borrow_mut()
        .map_err(|_| "network read buffer is already borrowed".to_string())?;
    let count = pending.len().min(limit);
    Ok(pending.drain(..count).collect())
}

fn take_all_pending(connection: &Rc<ConnectionHandle>) -> Result<Vec<u8>, String> {
    let mut pending = connection
        .pending
        .try_borrow_mut()
        .map_err(|_| "network read buffer is already borrowed".to_string())?;
    Ok(std::mem::take(&mut *pending))
}

fn store_pending(connection: &Rc<ConnectionHandle>, bytes: Vec<u8>) -> Result<(), String> {
    let mut pending = connection
        .pending
        .try_borrow_mut()
        .map_err(|_| "network read buffer is already borrowed".to_string())?;
    if pending.len().saturating_add(bytes.len()) > MAX_HTTP_HEADER_BYTES + MAX_HTTP_BODY_BYTES {
        return Err("network read buffer limit exceeded".to_string());
    }
    pending.extend(bytes);
    Ok(())
}

fn read_connection_chunk_until(
    connection: &Rc<ConnectionHandle>,
    max_bytes: usize,
    deadline: Instant,
    operation: &str,
) -> Result<Option<Vec<u8>>, String> {
    loop {
        let read_result = {
            let mut stream = connection
                .inner
                .try_borrow_mut()
                .map_err(|_| "network connection is already borrowed".to_string())?;
            let stream = stream
                .as_mut()
                .ok_or_else(|| "network connection is closed".to_string())?;
            let mut bytes = vec![0; max_bytes];
            match stream.read(&mut bytes) {
                Ok(0) => Ok(None),
                Ok(count) => {
                    bytes.truncate(count);
                    Ok(Some(bytes))
                }
                Err(error) => Err(error),
            }
        };

        match read_result {
            Ok(result) => return Ok(result),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if !wait_for_io(deadline) {
                    return Err(format!("{operation} timed out"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(format!("{operation} failed: {error}")),
        }
    }
}

fn write_connection_until(
    connection: &Rc<ConnectionHandle>,
    bytes: &[u8],
    deadline: Instant,
    operation: &str,
) -> Result<(), String> {
    let mut written = 0;
    while written < bytes.len() {
        let write_result = {
            let mut stream = connection
                .inner
                .try_borrow_mut()
                .map_err(|_| "network connection is already borrowed".to_string())?;
            let stream = stream
                .as_mut()
                .ok_or_else(|| "network connection is closed".to_string())?;
            stream.write(&bytes[written..])
        };

        match write_result {
            Ok(0) => {
                return Err(format!(
                    "{operation} failed because the peer stopped reading"
                ))
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if !wait_for_io(deadline) {
                    return Err(format!("{operation} timed out"));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(format!("{operation} failed: {error}")),
        }
    }
    Ok(())
}

fn deadline(timeout: Duration) -> Instant {
    Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now)
}

fn wait_for_io(deadline: Instant) -> bool {
    let now = Instant::now();
    if now >= deadline {
        return false;
    }
    thread::sleep((deadline - now).min(IO_POLL_INTERVAL));
    true
}

#[derive(Debug)]
struct HttpHead {
    method: String,
    target: String,
    path: String,
    query: String,
    version: String,
    headers: BTreeMap<String, String>,
    content_length: usize,
    keep_alive: bool,
}

fn read_http_request(
    connection: &Rc<ConnectionHandle>,
    timeout: Duration,
) -> Result<Value, String> {
    let deadline = deadline(timeout);
    let mut buffer = take_all_pending(connection)?;
    let header_end = loop {
        if let Some(position) = find_bytes(&buffer, b"\r\n\r\n") {
            let end = position + 4;
            if end > MAX_HTTP_HEADER_BYTES {
                return Err(format!(
                    "HTTP headers exceed the {MAX_HTTP_HEADER_BYTES}-byte limit"
                ));
            }
            break end;
        }
        if buffer.len() >= MAX_HTTP_HEADER_BYTES {
            return Err(format!(
                "HTTP headers exceed the {MAX_HTTP_HEADER_BYTES}-byte limit"
            ));
        }
        let remaining = MAX_HTTP_HEADER_BYTES - buffer.len();
        let count = remaining.min(8192);
        let Some(bytes) =
            read_connection_chunk_until(connection, count, deadline, "HTTP request read")?
        else {
            return Err("connection closed before the HTTP headers were complete".to_string());
        };
        buffer.extend(bytes);
    };

    let head = parse_http_head(&buffer[..header_end - 4])?;
    if head.content_length > MAX_HTTP_BODY_BYTES {
        return Err(format!(
            "HTTP body exceeds the {MAX_HTTP_BODY_BYTES}-byte limit"
        ));
    }
    let request_end = header_end
        .checked_add(head.content_length)
        .ok_or_else(|| "HTTP request length overflow".to_string())?;
    while buffer.len() < request_end {
        let remaining = request_end - buffer.len();
        let Some(bytes) = read_connection_chunk_until(
            connection,
            remaining.min(8192),
            deadline,
            "HTTP body read",
        )?
        else {
            return Err("connection closed before the HTTP body was complete".to_string());
        };
        buffer.extend(bytes);
    }

    let body = String::from_utf8(buffer[header_end..request_end].to_vec())
        .map_err(|_| "HTTP request body is not valid UTF-8".to_string())?;
    if buffer.len() > request_end {
        store_pending(connection, buffer.split_off(request_end))?;
    }

    let headers = head
        .headers
        .into_iter()
        .map(|(name, value)| (name, Value::String(value)));
    Ok(record_value([
        ("method".to_string(), Value::String(head.method)),
        ("target".to_string(), Value::String(head.target)),
        ("path".to_string(), Value::String(head.path)),
        ("query".to_string(), Value::String(head.query)),
        ("version".to_string(), Value::String(head.version)),
        ("headers".to_string(), record_value(headers)),
        ("body".to_string(), Value::String(body)),
        (
            "content_length".to_string(),
            Value::Number(head.content_length as f64),
        ),
        ("keep_alive".to_string(), Value::Bool(head.keep_alive)),
        (
            "local_addr".to_string(),
            Value::String(connection.local_addr.to_string()),
        ),
        (
            "remote_addr".to_string(),
            Value::String(connection.peer_addr.to_string()),
        ),
    ]))
}

fn parse_http_head(bytes: &[u8]) -> Result<HttpHead, String> {
    validate_crlf(bytes, "HTTP request headers")?;
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "HTTP request headers must be valid UTF-8".to_string())?;
    let mut lines = text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| "HTTP request line is missing".to_string())?;
    if request_line.is_empty() || request_line.len() > MAX_HTTP_LINE_BYTES {
        return Err("HTTP request line length is invalid".to_string());
    }
    let parts = request_line.split(' ').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return Err("HTTP request line must contain method, target, and version".to_string());
    }
    let method = parts[0];
    let target = parts[1];
    let version = parts[2];
    if method.len() > 32 || !method.bytes().all(is_http_token_byte) {
        return Err("HTTP method is invalid".to_string());
    }
    if target.len() > MAX_HTTP_LINE_BYTES
        || target.is_empty()
        || !target.bytes().all(|byte| (0x21..=0x7e).contains(&byte))
    {
        return Err("HTTP request target is invalid".to_string());
    }
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err("only HTTP/1.0 and HTTP/1.1 requests are supported".to_string());
    }

    let mut headers = BTreeMap::<String, String>::new();
    for line in lines {
        if line.is_empty() || line.len() > MAX_HTTP_LINE_BYTES {
            return Err("HTTP header line length is invalid".to_string());
        }
        if line.starts_with([' ', '\t']) {
            return Err("folded HTTP headers are not supported".to_string());
        }
        let (name, raw_value) = line
            .split_once(':')
            .ok_or_else(|| "HTTP header is missing `:`".to_string())?;
        if name.is_empty() || !name.bytes().all(is_http_token_byte) {
            return Err("HTTP header name is invalid".to_string());
        }
        let value = raw_value.trim_matches([' ', '\t']);
        if !valid_header_value(value) {
            return Err("HTTP header value contains invalid characters".to_string());
        }
        let name = name.to_ascii_lowercase();
        if let Some(existing) = headers.get_mut(&name) {
            if matches!(
                name.as_str(),
                "content-length" | "host" | "transfer-encoding"
            ) {
                return Err(format!("duplicate `{name}` HTTP header"));
            }
            existing.push_str(", ");
            existing.push_str(value);
        } else {
            headers.insert(name, value.to_string());
        }
    }

    if version == "HTTP/1.1"
        && headers
            .get("host")
            .map(|value| value.is_empty())
            .unwrap_or(true)
    {
        return Err("HTTP/1.1 request requires a Host header".to_string());
    }
    if headers.contains_key("transfer-encoding") {
        return Err("HTTP Transfer-Encoding is not supported".to_string());
    }
    let content_length = match headers.get("content-length") {
        Some(value) => parse_content_length(value)?,
        None => 0,
    };
    let keep_alive = match headers
        .get("connection")
        .map(|value| value.to_ascii_lowercase())
    {
        Some(value) if value == "close" => false,
        Some(value) if value == "keep-alive" => true,
        Some(_) => return Err("unsupported HTTP Connection header".to_string()),
        None => version == "HTTP/1.1",
    };
    let (path, query) = target
        .split_once('?')
        .map(|(path, query)| (path.to_string(), query.to_string()))
        .unwrap_or_else(|| (target.to_string(), String::new()));

    Ok(HttpHead {
        method: method.to_string(),
        target: target.to_string(),
        path,
        query,
        version: version.to_string(),
        headers,
        content_length,
        keep_alive,
    })
}

fn parse_content_length(value: &str) -> Result<usize, String> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("HTTP Content-Length must be a decimal integer".to_string());
    }
    value
        .parse::<usize>()
        .map_err(|_| "HTTP Content-Length is too large".to_string())
}

fn encode_http_response(response: &Value) -> Result<Vec<u8>, String> {
    match response {
        Value::String(response) => validate_raw_http_response(response),
        Value::Record(response) => encode_structured_http_response(response),
        _ => Err("web response must be a raw HTTP string or response record".to_string()),
    }
}

fn encode_structured_http_response(
    response: &Rc<RefCell<BTreeMap<String, Value>>>,
) -> Result<Vec<u8>, String> {
    let response = response
        .try_borrow()
        .map_err(|_| "web response record is already mutably borrowed".to_string())?;
    let status = match response.get("status") {
        Some(Value::Number(value))
            if value.is_finite() && value.fract() == 0.0 && (100.0..=599.0).contains(value) =>
        {
            *value as u16
        }
        Some(_) => return Err("web response status must be an integer from 100 to 599".to_string()),
        None => 200,
    };
    let body = match response.get("body") {
        Some(Value::String(value)) => value.clone(),
        Some(_) => return Err("web response body must be a string".to_string()),
        None => String::new(),
    };
    if body.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(format!(
            "HTTP response exceeds the {MAX_HTTP_RESPONSE_BYTES}-byte limit"
        ));
    }

    let mut headers = BTreeMap::<String, String>::new();
    if let Some(value) = response.get("headers") {
        let Value::Record(values) = value else {
            return Err("web response headers must be a record".to_string());
        };
        let values = values
            .try_borrow()
            .map_err(|_| "web response headers are already mutably borrowed".to_string())?;
        let mut normalized_names = HashSet::new();
        for (name, value) in values.iter() {
            let Value::String(value) = value else {
                return Err("web response header values must be strings".to_string());
            };
            if name.is_empty() || !name.bytes().all(is_http_token_byte) {
                return Err("web response header name is invalid".to_string());
            }
            if !valid_header_value(value) {
                return Err("web response header value contains CRLF or control data".to_string());
            }
            let normalized = name.to_ascii_lowercase();
            if matches!(
                normalized.as_str(),
                "content-length" | "connection" | "transfer-encoding"
            ) {
                return Err(format!(
                    "web response header `{name}` is managed by the runtime"
                ));
            }
            if !normalized_names.insert(normalized) {
                return Err("duplicate web response header name".to_string());
            }
            headers.insert(name.clone(), value.clone());
        }
    }

    let mut output = format!("HTTP/1.1 {status} {}\r\n", reason_phrase(status));
    for (name, value) in headers {
        output.push_str(&name);
        output.push_str(": ");
        output.push_str(&value);
        output.push_str("\r\n");
    }
    output.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    output.push_str(&body);
    if output.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(format!(
            "HTTP response exceeds the {MAX_HTTP_RESPONSE_BYTES}-byte limit"
        ));
    }
    Ok(output.into_bytes())
}

fn validate_raw_http_response(response: &str) -> Result<Vec<u8>, String> {
    if response.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(format!(
            "HTTP response exceeds the {MAX_HTTP_RESPONSE_BYTES}-byte limit"
        ));
    }
    let bytes = response.as_bytes();
    let header_position = find_bytes(bytes, b"\r\n\r\n")
        .ok_or_else(|| "raw HTTP response is missing the header terminator".to_string())?;
    if header_position + 4 > MAX_HTTP_HEADER_BYTES {
        return Err("raw HTTP response headers are too large".to_string());
    }
    let head = &bytes[..header_position];
    validate_crlf(head, "raw HTTP response headers")?;
    let head = std::str::from_utf8(head)
        .map_err(|_| "raw HTTP response headers must be valid UTF-8".to_string())?;
    let mut lines = head.split("\r\n");
    let status_line = lines
        .next()
        .ok_or_else(|| "raw HTTP response status line is missing".to_string())?;
    if status_line.len() > MAX_HTTP_LINE_BYTES {
        return Err("raw HTTP response status line is too long".to_string());
    }
    let mut status_parts = status_line.splitn(3, ' ');
    let version = status_parts.next().unwrap_or_default();
    let status = status_parts.next().unwrap_or_default();
    let reason = status_parts.next().unwrap_or_default();
    if !matches!(version, "HTTP/1.0" | "HTTP/1.1")
        || status.len() != 3
        || !status.bytes().all(|byte| byte.is_ascii_digit())
        || reason.is_empty()
        || !valid_header_value(reason)
    {
        return Err("raw HTTP response status line is invalid".to_string());
    }
    let status = status
        .parse::<u16>()
        .map_err(|_| "raw HTTP response status is invalid".to_string())?;
    if !(100..=599).contains(&status) {
        return Err("raw HTTP response status must be from 100 to 599".to_string());
    }

    let mut content_length = None;
    let mut names = HashSet::new();
    for line in lines {
        if line.is_empty() || line.len() > MAX_HTTP_LINE_BYTES || line.starts_with([' ', '\t']) {
            return Err("raw HTTP response header line is invalid".to_string());
        }
        let (name, raw_value) = line
            .split_once(':')
            .ok_or_else(|| "raw HTTP response header is missing `:`".to_string())?;
        if name.is_empty() || !name.bytes().all(is_http_token_byte) {
            return Err("raw HTTP response header name is invalid".to_string());
        }
        let value = raw_value.trim_matches([' ', '\t']);
        if !valid_header_value(value) {
            return Err("raw HTTP response header value is invalid".to_string());
        }
        let name = name.to_ascii_lowercase();
        if !names.insert(name.clone())
            && matches!(name.as_str(), "content-length" | "transfer-encoding")
        {
            return Err(format!("duplicate `{name}` raw HTTP response header"));
        }
        match name.as_str() {
            "content-length" => content_length = Some(parse_content_length(value)?),
            "transfer-encoding" => {
                return Err("raw HTTP Transfer-Encoding is not supported".to_string());
            }
            _ => {}
        }
    }
    let body_length = bytes.len() - header_position - 4;
    if let Some(expected) = content_length {
        if expected != body_length {
            return Err(format!(
                "raw HTTP Content-Length is {expected}, but the body is {body_length} bytes"
            ));
        }
    }
    Ok(bytes.to_vec())
}

fn validate_crlf(bytes: &[u8], label: &str) -> Result<(), String> {
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte == b'\r' && bytes.get(index + 1) != Some(&b'\n') {
            return Err(format!("{label} contains a bare carriage return"));
        }
        if byte == b'\n' && (index == 0 || bytes.get(index - 1) != Some(&b'\r')) {
            return Err(format!("{label} contains a bare line feed"));
        }
    }
    Ok(())
}

fn is_http_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

fn valid_header_value(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte == b'\t' || (0x20..=0x7e).contains(&byte))
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Response",
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loopback_listener() -> (Value, u16) {
        let listener = net_listen(vec![
            Value::String("127.0.0.1".to_string()),
            Value::Number(0.0),
        ])
        .expect("listen on loopback");
        let Value::Listener(handle) = &listener else {
            panic!("net_listen must return a listener");
        };
        (listener.clone(), handle.local_addr.port())
    }

    fn connect_and_send(port: u16, request: &'static [u8]) -> thread::JoinHandle<Vec<u8>> {
        thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect client");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("client read timeout");
            stream.write_all(request).expect("write request");
            let mut response = Vec::new();
            stream.read_to_end(&mut response).expect("read response");
            response
        })
    }

    #[test]
    fn nonblocking_adapter_round_trips_over_loopback() {
        let (listener, port) = loopback_listener();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect client");
            stream.write_all(b"ping").expect("write ping");
            let mut response = [0; 4];
            stream.read_exact(&mut response).expect("read pong");
            response
        });
        let connection = net_accept(vec![listener, Value::Number(2_000.0)]).expect("accept");
        assert!(matches!(
            net_read(vec![
                connection.clone(),
                Value::Number(4.0),
                Value::Number(2_000.0),
            ]),
            Ok(Value::String(value)) if value == "ping"
        ));
        assert!(matches!(
            net_write(vec![
                connection.clone(),
                Value::String("pong".to_string()),
                Value::Number(2_000.0),
            ]),
            Ok(Value::Number(4.0))
        ));
        net_close(vec![connection]).expect("close connection");
        assert_eq!(client.join().expect("client thread"), *b"pong");
    }

    #[test]
    fn try_accept_is_nonblocking_and_reports_ready_connections() {
        let (listener, port) = loopback_listener();
        assert!(matches!(net_try_accept(vec![listener.clone()]), Ok(Value::Null)));
        let client = thread::spawn(move || TcpStream::connect(("127.0.0.1", port)).expect("connect client"));
        let deadline = Instant::now() + Duration::from_secs(2);
        let connection = loop {
            match net_try_accept(vec![listener.clone()]).expect("try accept") {
                Value::Connection(connection) => break Value::Connection(connection),
                Value::Null if Instant::now() < deadline => thread::yield_now(),
                Value::Null => panic!("connection was not accepted before the deadline"),
                _ => panic!("net_try_accept returned an unexpected value"),
            }
        };
        net_close(vec![connection]).expect("close connection");
        drop(client.join().expect("client thread"));
        net_close(vec![listener.clone()]).expect("close listener");
        assert!(net_try_accept(vec![listener]).is_err());
    }

    #[test]
    fn http_parser_returns_a_structured_request() {
        let (listener, port) = loopback_listener();
        let client = connect_and_send(
            port,
            b"POST /items?id=7 HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\nhello",
        );
        let connection = net_accept(vec![listener, Value::Number(2_000.0)]).expect("accept");
        let request = http_read_request(vec![connection.clone(), Value::Number(2_000.0)])
            .expect("parse HTTP request");
        let Value::Record(request) = request else {
            panic!("HTTP request must be a record");
        };
        let request = request.borrow();
        assert!(matches!(request.get("method"), Some(Value::String(value)) if value == "POST"));
        assert!(matches!(request.get("path"), Some(Value::String(value)) if value == "/items"));
        assert!(matches!(request.get("query"), Some(Value::String(value)) if value == "id=7"));
        assert!(matches!(request.get("body"), Some(Value::String(value)) if value == "hello"));
        drop(request);
        net_close(vec![connection]).expect("close connection");
        assert!(client.join().expect("client thread").is_empty());
    }

    #[test]
    fn reusable_web_listener_serves_more_than_one_request() {
        let (listener, port) = loopback_listener();
        let response = record_value([
            ("status".to_string(), Value::Number(200.0)),
            ("body".to_string(), Value::String("ok".to_string())),
        ]);

        for _ in 0..2 {
            let client = connect_and_send(port, b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n");
            let request = web_serve_once(vec![
                listener.clone(),
                response.clone(),
                Value::Number(2_000.0),
            ])
            .expect("serve request");
            assert!(matches!(request, Value::Record(_)));
            let bytes = client.join().expect("client thread");
            let text = String::from_utf8(bytes).expect("UTF-8 response");
            assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
            assert!(text.ends_with("\r\n\r\nok"));
        }
        net_close(vec![listener]).expect("close listener");
    }

    #[test]
    fn structured_responses_can_be_written_on_accepted_connections() {
        let (listener, port) = loopback_listener();
        let client = connect_and_send(port, b"");
        let connection = net_accept(vec![listener, Value::Number(2_000.0)]).expect("accept");
        let response = record_value([
            ("status".to_string(), Value::Number(201.0)),
            ("body".to_string(), Value::String("created".to_string())),
        ]);
        assert!(matches!(
            http_write_response(vec![
                connection.clone(),
                response,
                Value::Number(2_000.0),
            ]),
            Ok(Value::Number(bytes)) if bytes > 7.0
        ));
        net_close(vec![connection]).expect("close connection");
        let bytes = client.join().expect("client thread");
        let text = String::from_utf8(bytes).expect("UTF-8 response");
        assert!(text.starts_with("HTTP/1.1 201 Created\r\n"));
        assert!(text.ends_with("\r\n\r\ncreated"));
    }

    #[test]
    fn rejects_smuggling_shapes_and_response_header_injection() {
        assert!(parse_http_head(
            b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\nContent-Length: 2"
        )
        .is_err());
        assert!(parse_http_head(b"GET / HTTP/1.1\nHost: localhost").is_err());

        let headers = record_value([(
            "X-Test".to_string(),
            Value::String("ok\r\nInjected: yes".to_string()),
        )]);
        let response = record_value([
            ("status".to_string(), Value::Number(200.0)),
            ("headers".to_string(), headers),
            ("body".to_string(), Value::String("ok".to_string())),
        ]);
        assert!(encode_http_response(&response).is_err());
    }

    #[test]
    fn accept_timeout_and_http_header_limit_are_enforced() {
        let (listener, port) = loopback_listener();
        assert!(net_accept(vec![listener.clone(), Value::Number(0.0)]).is_err());

        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect client");
            let mut request = b"GET / HTTP/1.1\r\nHost: ".to_vec();
            request.resize(request.len() + MAX_HTTP_HEADER_BYTES, b'a');
            stream.write_all(&request).expect("write oversized request");
        });
        let connection = net_accept(vec![listener, Value::Number(2_000.0)]).expect("accept");
        assert!(http_read_request(vec![connection.clone(), Value::Number(2_000.0)]).is_err());
        net_close(vec![connection]).expect("close connection");
        client.join().expect("client thread");
    }
}
