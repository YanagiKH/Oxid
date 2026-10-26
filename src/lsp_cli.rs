//! Typed editor service over standard input and output.
mod documents;
mod json;
mod transport;
mod uri;

use crate::{frontend, project_cli};
use documents::{string, Documents};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

const HELP: &str = "Oxid language server\n\nUsage: oxid lsp --stdio [--edition typed-preview]\n\nUses typed project settings and open editor buffers for diagnostics and formatting.\n";
const MAX_DIAGNOSTICS: usize = 256;
const MAX_ROOTS: usize = 16;

pub(crate) fn dispatch(args: &[String]) -> Option<i32> {
    let mut command = 0;
    while command < args.len() {
        if args[command] == "--edition" || args[command] == "--message-format" {
            command += 2;
        } else if args[command].starts_with("--edition=")
            || args[command].starts_with("--message-format=")
        {
            command += 1;
        } else {
            break;
        }
    }
    if args.get(command).map(String::as_str) != Some("lsp") {
        return None;
    }
    if args.len() == 1 && Path::new("lsp").is_file() {
        return None;
    }
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        return Some(if io::stdout().lock().write_all(HELP.as_bytes()).is_ok() {
            0
        } else {
            74
        });
    }
    let options = (|| {
        let (mut typed, mut edition, mut stdio) = (false, false, false);
        let mut index = 0;
        while index < args.len() {
            if index == command {
                index += 1;
                continue;
            }
            let word = &args[index];
            if word == "--stdio" && !stdio {
                stdio = true;
            } else if word == "--edition" || word.starts_with("--edition=") {
                if edition {
                    return Err("--edition may only be specified once");
                }
                edition = true;
                let value = if let Some((_, value)) = word.split_once('=') {
                    value
                } else {
                    index += 1;
                    args.get(index)
                        .map(String::as_str)
                        .ok_or("--edition requires typed-preview")?
                };
                if value != "typed-preview" {
                    return Err("the language server supports typed-preview");
                }
                typed = true;
            } else {
                return Err("unsupported language server option");
            }
            index += 1;
        }
        if !stdio {
            return Err("use oxid lsp --stdio to start the language server");
        }
        Ok(typed)
    })();
    let explicit_typed = match options {
        Ok(value) => value,
        Err(message) => {
            let _ = writeln!(io::stderr().lock(), "error: {message}");
            return Some(2);
        }
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut server = Server::new(explicit_typed);
    Some(
        match server.run(&mut io::BufReader::new(stdin.lock()), &mut stdout.lock()) {
            Ok(code) => code,
            Err(error) => {
                let _ = writeln!(io::stderr().lock(), "language server I/O failed: {error}");
                74
            }
        },
    )
}

struct Server {
    explicit_typed: bool,
    initialized: bool,
    shutdown: bool,
    roots: BTreeMap<String, PathBuf>,
    documents: Documents,
    reported: BTreeSet<String>,
}

fn response(writer: &mut impl Write, id: Value, result: Value) -> io::Result<()> {
    transport::write(writer, &json!({"jsonrpc":"2.0","id":id,"result":result}))
}
fn error(writer: &mut impl Write, id: Value, code: i32, message: &str) -> io::Result<()> {
    transport::write(
        writer,
        &json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}),
    )
}
fn notify(writer: &mut impl Write, method: &str, params: Value) -> io::Result<()> {
    transport::write(
        writer,
        &json!({"jsonrpc":"2.0","method":method,"params":params}),
    )
}
fn range(start: (usize, usize), end: (usize, usize)) -> Value {
    json!({"start":{"line":start.0,"character":start.1},"end":{"line":end.0,"character":end.1}})
}
fn bounded(text: &str, maximum: usize) -> &str {
    let mut end = text.len().min(maximum);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
fn configuration(message: &str) -> Value {
    json!({"range":range((0,0),(0,0)),"severity":1,"code":"E0006","source":"Oxid","message":bounded(message,8192)})
}

impl Server {
    fn new(explicit_typed: bool) -> Self {
        Self {
            explicit_typed,
            initialized: false,
            shutdown: false,
            roots: BTreeMap::new(),
            documents: Documents::default(),
            reported: BTreeSet::new(),
        }
    }
    fn run(&mut self, reader: &mut impl BufRead, writer: &mut impl Write) -> io::Result<i32> {
        while let Some(body) = transport::read(reader)? {
            match json::parse(&body) {
                Ok(value) => {
                    if let Some(code) = self.handle(value, writer)? {
                        return Ok(code);
                    }
                }
                Err(_) => error(writer, Value::Null, -32700, "invalid JSON-RPC payload")?,
            }
        }
        Ok(0)
    }
    fn handle(&mut self, value: Value, writer: &mut impl Write) -> io::Result<Option<i32>> {
        let Some(object) = value.as_object() else {
            error(
                writer,
                Value::Null,
                -32600,
                "JSON-RPC message must be an object",
            )?;
            return Ok(None);
        };
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            error(writer, Value::Null, -32600, "JSON-RPC version must be 2.0")?;
            return Ok(None);
        }
        if !object.contains_key("method")
            && (object.contains_key("result") || object.contains_key("error"))
        {
            return Ok(None);
        }
        let Some(method) = object
            .get("method")
            .and_then(Value::as_str)
            .filter(|method| method.len() <= 128)
        else {
            error(writer, Value::Null, -32600, "invalid JSON-RPC method")?;
            return Ok(None);
        };
        let id = object.get("id").cloned();
        if id.as_ref().is_some_and(|id| {
            !(id.as_i64().is_some()
                || id.as_u64().is_some()
                || id.as_str().is_some_and(|id| id.len() <= 4096))
        }) {
            error(writer, Value::Null, -32600, "invalid request id")?;
            return Ok(None);
        }
        let params = object.get("params").unwrap_or(&Value::Null);
        if method == "exit" {
            if let Some(id) = id {
                error(writer, id, -32600, "exit is a notification")?;
                return Ok(None);
            }
            return Ok(Some(if self.shutdown { 0 } else { 1 }));
        }
        if method == "initialize" {
            if let Some(id) = id {
                if self.initialized {
                    error(writer, id, -32600, "server is already initialized")?;
                } else {
                    match self.initialize(params) {
                        Ok(()) => response(
                            writer,
                            id,
                            json!({"capabilities":{"positionEncoding":"utf-16","textDocumentSync":{"openClose":true,"change":2,"save":{"includeText":true}},"documentFormattingProvider":true,"completionProvider":{"resolveProvider":false},"definitionProvider":true,"documentSymbolProvider":true,"workspaceSymbolProvider":true,"workspace":{"workspaceFolders":{"supported":true,"changeNotifications":true}}},"serverInfo":{"name":"Oxid","version":env!("CARGO_PKG_VERSION")}}),
                        )?,
                        Err(message) => error(writer, id, -32602, &message)?,
                    }
                }
            }
            return Ok(None);
        }
        if !self.initialized || self.shutdown {
            if let Some(id) = id {
                error(
                    writer,
                    id,
                    if self.shutdown { -32600 } else { -32002 },
                    if self.shutdown {
                        "server is shutting down"
                    } else {
                        "server is not initialized"
                    },
                )?;
            }
            return Ok(None);
        }
        if method == "shutdown" {
            if let Some(id) = id {
                self.shutdown = true;
                response(writer, id, Value::Null)?;
            }
            return Ok(None);
        }
        if method == "textDocument/formatting" {
            if let Some(id) = id {
                match self.format(params) {
                    Ok(value) => response(writer, id, value)?,
                    Err(message) => error(writer, id, -32803, &message)?,
                }
            }
            return Ok(None);
        }
        if method == "textDocument/completion" {
            if let Some(id) = id {
                match self.complete(params) {
                    Ok(value) => response(writer, id, value)?,
                    Err(message) => error(writer, id, -32803, &message)?,
                }
            }
            return Ok(None);
        }
        if method == "textDocument/definition" {
            if let Some(id) = id {
                match self.definition(params) {
                    Ok(value) => response(writer, id, value)?,
                    Err(message) => error(writer, id, -32803, &message)?,
                }
            }
            return Ok(None);
        }
        if matches!(method, "textDocument/documentSymbol" | "workspace/symbol") {
            if let Some(id) = id {
                match self.symbols(params, method == "workspace/symbol") {
                    Ok(value) => response(writer, id, value)?,
                    Err(message) => error(writer, id, -32803, &message)?,
                }
            }
            return Ok(None);
        }
        if let Some(id) = id {
            error(writer, id, -32601, "unsupported language server request")?;
            return Ok(None);
        }
        let result = match method {
            "initialized"
            | "workspace/didChangeWatchedFiles"
            | "workspace/didChangeConfiguration" => Ok(true),
            "textDocument/didOpen" => self.documents.open(&params["textDocument"]).map(|()| true),
            "textDocument/didChange" => self
                .documents
                .change(&params["textDocument"], &params["contentChanges"]),
            "textDocument/didSave" => string(&params["textDocument"], "uri")
                .and_then(|uri| {
                    self.documents
                        .save(uri, params.get("text").and_then(Value::as_str))
                })
                .map(|()| true),
            "textDocument/didClose" => string(&params["textDocument"], "uri").map(|uri| {
                self.documents.close(uri);
                true
            }),
            "workspace/didChangeWorkspaceFolders" => {
                self.change_roots(&params["event"]).map(|()| true)
            }
            _ => Ok(false),
        };
        match result {
            Ok(true) => self.refresh(writer)?,
            Ok(false) => {}
            Err(message) => {
                notify(
                    writer,
                    "window/logMessage",
                    json!({"type":1,"message":bounded(&message,8192)}),
                )?;
                if self
                    .documents
                    .files
                    .values()
                    .any(|document| !document.synchronized)
                {
                    self.clear(writer)?;
                }
            }
        }
        Ok(None)
    }
    fn initialize(&mut self, params: &Value) -> Result<(), String> {
        if !params.is_object() {
            return Err("initialize params must be an object".into());
        }
        let mut roots = BTreeMap::new();
        if let Some(folders) = params
            .get("workspaceFolders")
            .filter(|folders| !folders.is_null())
        {
            let folders = folders
                .as_array()
                .filter(|folders| folders.len() <= MAX_ROOTS)
                .ok_or("invalid workspace folder list")?;
            for folder in folders {
                add_root(&mut roots, string(folder, "uri")?)?;
            }
        } else if let Some(uri) = params.get("rootUri").and_then(Value::as_str) {
            add_root(&mut roots, uri)?;
        }
        if roots.is_empty() {
            let root = std::env::current_dir()
                .map_err(|error| error.to_string())?
                .canonicalize()
                .map_err(|error| error.to_string())?;
            roots.insert(uri::encode(&root)?, root);
        }
        self.roots = roots;
        self.initialized = true;
        Ok(())
    }
    fn change_roots(&mut self, event: &Value) -> Result<(), String> {
        let added = event["added"]
            .as_array()
            .filter(|roots| roots.len() <= MAX_ROOTS)
            .ok_or("invalid added workspace folders")?;
        let removed = event["removed"]
            .as_array()
            .filter(|roots| roots.len() <= MAX_ROOTS)
            .ok_or("invalid removed workspace folders")?;
        let mut roots = self.roots.clone();
        for root in removed {
            roots.remove(string(root, "uri")?);
        }
        for root in added {
            add_root(&mut roots, string(root, "uri")?)?;
        }
        if roots.len() > MAX_ROOTS {
            return Err("too many workspace folders".into());
        }
        self.roots = roots;
        Ok(())
    }
    fn format(&self, params: &Value) -> Result<Value, String> {
        let uri = string(&params["textDocument"], "uri")?;
        let document = self.documents.find(uri).ok_or("source is not open")?;
        if !document.synchronized {
            return Err("reopen the document or send a full update before formatting".into());
        }
        project_cli::editor_source_edition(&document.path, self.explicit_typed)?;
        let text = frontend::editor::format(&document.path, &document.text).map_err(|errors| {
            errors.first().map_or_else(
                || "source cannot be formatted".to_owned(),
                |error| bounded(&error.message, 8192).to_owned(),
            )
        })?;
        if text == document.text {
            return Ok(json!([]));
        }
        Ok(json!([{"range":range((0,0), end_position(&document.text)),"newText":text}]))
    }
    fn complete(&self, params: &Value) -> Result<Value, String> {
        let document = self
            .documents
            .find(string(&params["textDocument"], "uri")?)
            .ok_or("source is not open")?;
        if self
            .documents
            .files
            .values()
            .any(|document| !document.synchronized)
        {
            return Err(
                "reopen the unsynchronized document or send its full text before completion".into(),
            );
        }
        let offset = documents::position(&document.text, &params["position"])?;
        let entries: Vec<_> = self
            .documents
            .files
            .values()
            .map(|document| (document.path.as_path(), document.text.as_str()))
            .collect();
        let overlay = frontend::editor::SourceOverlay::new(&entries)?;
        let graphs = project_cli::editor_project_graphs(&document.path, self.explicit_typed)?;
        let (mut files, mut bytes) = (0, 0);
        let mut labels = BTreeSet::new();
        let mut items = Vec::new();
        let mut incomplete = false;
        for graph in graphs {
            if !graph
                .packages
                .iter()
                .any(|package| document.path.starts_with(&package.root))
            {
                continue;
            }
            let result = frontend::editor::complete(
                &graph.packages,
                graph.entry.as_deref(),
                &overlay,
                &document.path,
                offset,
            )
            .map_err(|errors| {
                errors.first().map_or_else(
                    || "cannot read completion names".to_owned(),
                    |error| bounded(&error.message, 8192).to_owned(),
                )
            })?;
            files += result.files;
            bytes += result.bytes;
            if files > 1024 || bytes > 8 * 1024 * 1024 {
                return Err("completion exceeds the source limit".into());
            }
            incomplete |= result.incomplete;
            let Some(replacement) = result.replacement else {
                continue;
            };
            for item in result.entries {
                if !labels.insert(item.label.clone()) {
                    continue;
                }
                if items.len() >= 512 {
                    incomplete = true;
                    continue;
                }
                items.push(json!({"label":item.label,"kind":item.kind,"detail":item.detail,"textEdit":{"range":range(replacement.start,replacement.end),"newText":item.label}}));
            }
        }
        Ok(json!({"isIncomplete":incomplete,"items":items}))
    }
    fn definition(&self, params: &Value) -> Result<Value, String> {
        let document = self
            .documents
            .find(string(&params["textDocument"], "uri")?)
            .ok_or("source is not open")?;
        if self
            .documents
            .files
            .values()
            .any(|document| !document.synchronized)
        {
            return Err(
                "reopen the unsynchronized document or send its full text before navigation".into(),
            );
        }
        let offset = documents::position(&document.text, &params["position"])?;
        let entries: Vec<_> = self
            .documents
            .files
            .values()
            .map(|document| (document.path.as_path(), document.text.as_str()))
            .collect();
        let overlay = frontend::editor::SourceOverlay::new(&entries)?;
        let graphs = project_cli::editor_project_graphs(&document.path, self.explicit_typed)?;
        let mut locations = Vec::new();
        let (mut files, mut bytes) = (0, 0);
        for graph in graphs {
            if !graph
                .packages
                .iter()
                .any(|package| document.path.starts_with(&package.root))
            {
                continue;
            }
            let found = frontend::editor::definition(
                &graph.packages,
                graph.entry.as_deref(),
                &overlay,
                &document.path,
                offset,
            )
            .map_err(|errors| {
                errors.first().map_or_else(
                    || "cannot read source declarations".to_owned(),
                    |error| bounded(&error.message, 8192).to_owned(),
                )
            })?;
            files += found.files;
            bytes += found.bytes;
            if files > 1024 || bytes > 8 * 1024 * 1024 {
                return Err("definition lookup exceeds the source limit".into());
            }
            if let Some(found) = found.location {
                let value = json!({"uri":self.uri_for_path(Path::new(&found.path))?,"range":range(found.start,found.end)});
                if !locations.contains(&value) {
                    locations.push(value);
                }
                if locations.len() > 32 {
                    return Err("navigation exceeds the definition result limit".into());
                }
            }
        }
        Ok(if locations.is_empty() {
            Value::Null
        } else {
            Value::Array(locations)
        })
    }
    fn symbols(&self, params: &Value, workspace: bool) -> Result<Value, String> {
        if self
            .documents
            .files
            .values()
            .any(|document| !document.synchronized)
        {
            return Err(
                "reopen the unsynchronized document or send its full text before listing symbols"
                    .into(),
            );
        }
        let query = if workspace {
            string(params, "query")?
        } else {
            ""
        };
        if query.len() > 128 {
            return Err("workspace symbol query exceeds 128 bytes".into());
        }
        let path = if workspace {
            None
        } else {
            Some(
                &self
                    .documents
                    .find(string(&params["textDocument"], "uri")?)
                    .ok_or("source is not open")?
                    .path,
            )
        };
        let contexts = path.map_or_else(
            || self.roots.values().cloned().collect::<Vec<_>>(),
            |path| vec![path.clone()],
        );
        let entries: Vec<_> = self
            .documents
            .files
            .values()
            .map(|document| (document.path.as_path(), document.text.as_str()))
            .collect();
        let overlay = frontend::editor::SourceOverlay::new(&entries)?;
        let mut graphs_seen = BTreeSet::new();
        let mut symbols_seen = BTreeSet::new();
        let mut output = Vec::new();
        let (mut visits, mut edges, mut files, mut bytes, mut symbol_bytes) = (0, 0, 0, 0, 0);
        for context in contexts {
            for graph in project_cli::editor_project_graphs(&context, self.explicit_typed)? {
                let identity = (
                    graph
                        .packages
                        .iter()
                        .map(|package| package.entry.clone())
                        .collect::<Vec<_>>(),
                    graph.entry.clone(),
                );
                if !graphs_seen.insert(identity) {
                    continue;
                }
                visits += graph.packages.len();
                edges += graph
                    .packages
                    .iter()
                    .map(|package| package.dependencies.len())
                    .sum::<usize>();
                if visits > 256 || edges > 1024 {
                    return Err("symbol lookup exceeds the package or dependency limit".into());
                }
                let result = frontend::editor::symbols(
                    &graph.packages,
                    graph.entry.as_deref(),
                    &overlay,
                    path.map(PathBuf::as_path),
                    query,
                )
                .map_err(|errors| {
                    errors.first().map_or_else(
                        || "cannot read source symbols".to_owned(),
                        |error| bounded(&error.message, 8192).to_owned(),
                    )
                })?;
                files += result.files;
                bytes += result.bytes;
                if files > 1024 || bytes > 8 * 1024 * 1024 {
                    return Err("symbol lookup exceeds the source limit".into());
                }
                for symbol in result.entries {
                    let uri = self.uri_for_path(Path::new(&symbol.location.path))?;
                    if !symbols_seen.insert((
                        uri.clone(),
                        symbol.location.start,
                        symbol.location.end,
                    )) {
                        continue;
                    }
                    symbol_bytes += uri.len()
                        + symbol.name.len()
                        + symbol.container.as_ref().map_or(0, String::len);
                    if output.len() >= 4096 || symbol_bytes > 8 * 1024 * 1024 {
                        return Err("too many symbols; narrow the workspace query".into());
                    }
                    let mut item = json!({"name":symbol.name,"kind":symbol.kind,"location":{"uri":uri,"range":range(symbol.location.start,symbol.location.end)}});
                    if let Some(container) = symbol.container {
                        item["containerName"] = container.into();
                    }
                    output.push(item);
                }
            }
        }
        Ok(Value::Array(output))
    }
    fn uri_for_path(&self, path: &Path) -> Result<String, String> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        if let Some(document) = self.documents.files.get(&path) {
            return Ok(document.uri.clone());
        }
        uri::encode(&path)
    }
    fn refresh(&mut self, writer: &mut impl Write) -> io::Result<()> {
        if self
            .documents
            .files
            .values()
            .any(|document| !document.synchronized)
        {
            return self.clear(writer);
        }
        let entries: Vec<_> = self
            .documents
            .files
            .values()
            .map(|document| (document.path.as_path(), document.text.as_str()))
            .collect();
        let overlay = match frontend::editor::SourceOverlay::new(&entries) {
            Ok(overlay) => overlay,
            Err(message) => {
                notify(
                    writer,
                    "window/logMessage",
                    json!({"type":1,"message":message}),
                )?;
                return self.clear(writer);
            }
        };
        let mut contexts: Vec<_> = self.roots.values().cloned().collect();
        contexts.extend(self.documents.files.keys().cloned());
        let mut graph_ids = BTreeSet::new();
        let mut loaded = BTreeSet::new();
        let mut complete = true;
        let (mut visits, mut edges, mut files, mut bytes) = (0, 0, 0, 0);
        let mut output: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        let mut errors = Vec::new();
        for context in contexts {
            let graphs = match project_cli::editor_project_graphs(&context, self.explicit_typed) {
                Ok(graphs) => graphs,
                Err(message) => {
                    if context.is_file() {
                        if let Ok(uri) = self.uri_for_path(&context) {
                            output.entry(uri).or_default().push(configuration(&message));
                        }
                    }
                    continue;
                }
            };
            for graph in graphs {
                let identity = (
                    graph
                        .packages
                        .iter()
                        .map(|package| package.entry.clone())
                        .collect::<Vec<_>>(),
                    graph.entry.clone(),
                );
                if !graph_ids.insert(identity) {
                    continue;
                }
                visits += graph.packages.len();
                edges += graph
                    .packages
                    .iter()
                    .map(|package| package.dependencies.len())
                    .sum::<usize>();
                if visits > 256 || edges > 1024 {
                    complete = false;
                    break;
                }
                let result = frontend::editor::check_entry(
                    &graph.packages,
                    graph.entry.as_deref(),
                    &overlay,
                );
                files += result.files.len();
                bytes += result.source_bytes;
                if files > 1024 || bytes > 8 * 1024 * 1024 {
                    complete = false;
                    break;
                }
                loaded.extend(result.files.into_iter().map(|path| {
                    let path = PathBuf::from(path);
                    path.canonicalize().unwrap_or(path)
                }));
                complete &= result.sources_complete;
                errors.extend(result.diagnostics);
                if errors.len() >= MAX_DIAGNOSTICS {
                    errors.truncate(MAX_DIAGNOSTICS);
                    break;
                }
            }
            if visits > 256
                || edges > 1024
                || files > 1024
                || bytes > 8 * 1024 * 1024
                || errors.len() >= MAX_DIAGNOSTICS
            {
                break;
            }
        }
        let limit = visits > 256
            || edges > 1024
            || files > 1024
            || bytes > 8 * 1024 * 1024
            || errors.len() >= MAX_DIAGNOSTICS;
        for diagnostic in errors {
            let Some(location) = diagnostic.location else {
                for document in self.documents.files.values().take(1) {
                    output
                        .entry(document.uri.clone())
                        .or_default()
                        .push(configuration(&diagnostic.message));
                }
                continue;
            };
            let uri = match self.uri_for_path(Path::new(&location.path)) {
                Ok(uri) => uri,
                Err(_) => continue,
            };
            let related: Vec<_> = diagnostic.related.iter().take(8).filter_map(|(location,message)| Some(json!({"location":{"uri":self.uri_for_path(Path::new(&location.path)).ok()?,"range":range(location.start,location.end)},"message":bounded(message,2048)}))).collect();
            let value = json!({"range":range(location.start,location.end),"severity":1,"code":diagnostic.code,"source":"Oxid","message":bounded(&diagnostic.message,8192),"relatedInformation":related});
            let values = output.entry(uri).or_default();
            if !values.contains(&value) {
                values.push(value);
            }
        }
        for document in self.documents.files.values() {
            let values = output.entry(document.uri.clone()).or_default();
            if limit {
                values.push(configuration(
                    "editor project checks exceed the source or diagnostic limit",
                ));
            } else if complete && !loaded.contains(&document.path) && values.is_empty() {
                values.push(configuration("source is not part of a checked project entry; declare its module or test entry"));
            }
        }
        let current: BTreeSet<_> = output.keys().cloned().collect();
        let cleared: Vec<_> = self.reported.difference(&current).cloned().collect();
        for uri in cleared {
            self.publish(writer, &uri, Vec::new())?;
        }
        for (uri, diagnostics) in output {
            self.publish(writer, &uri, diagnostics)?;
        }
        self.reported = current;
        Ok(())
    }
    fn publish(
        &self,
        writer: &mut impl Write,
        uri: &str,
        diagnostics: Vec<Value>,
    ) -> io::Result<()> {
        let mut params = json!({"uri":uri,"diagnostics":diagnostics});
        if let Some(document) = self.documents.find(uri) {
            params["version"] = document.version.into();
        }
        notify(writer, "textDocument/publishDiagnostics", params)
    }
    fn clear(&mut self, writer: &mut impl Write) -> io::Result<()> {
        for uri in &self.reported {
            self.publish(writer, uri, Vec::new())?;
        }
        self.reported.clear();
        Ok(())
    }
}

fn add_root(roots: &mut BTreeMap<String, PathBuf>, uri: &str) -> Result<(), String> {
    let path = uri::path(uri)?
        .canonicalize()
        .map_err(|error| format!("cannot open workspace folder: {error}"))?;
    if !path.is_dir() {
        return Err("workspace folder must be a directory".into());
    }
    roots.insert(uri.into(), path);
    Ok(())
}
fn end_position(text: &str) -> (usize, usize) {
    let (mut line, mut column, mut carriage) = (0, 0, false);
    for ch in text.chars() {
        match ch {
            '\r' => {
                line += 1;
                column = 0;
                carriage = true;
            }
            '\n' => {
                if !carriage {
                    line += 1;
                }
                column = 0;
                carriage = false;
            }
            _ => {
                column += ch.len_utf16();
                carriage = false;
            }
        }
    }
    (line, column)
}
