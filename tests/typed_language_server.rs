//! A real stdio client exercises editor buffers through the public executable.
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Project(PathBuf);
impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "oxid-lsp-雪 {}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn uri(&self, name: &str) -> String {
        uri(&self.0.join(name))
    }
    fn package(&self, prefix: &str, name: &str, extra: &str, source: &str) {
        self.write(
            &format!("{prefix}oxid.toml"),
            &format!("[project]\nname=\"{name}\"\nedition=\"typed-preview\"\n{extra}"),
        );
        self.write(&format!("{prefix}src/main.ox"), source);
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_oxid"));
        command
            .current_dir(&self.0)
            .env_remove("OXID_PATH")
            .env("OXID_LLVM_BIN", self.0.join("no-llvm-tools"))
            .env("OXID_CACHE_DIR", self.0.join("download-cache"));
        command
    }
    fn unchanged(&self) {
        for path in [".oxid", "oxid.lock", "download-cache"] {
            assert!(
                !self.0.join(path).exists(),
                "unexpected editor output {path}"
            );
        }
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained LSP fixture: {}", self.0.display());
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn uri(path: &Path) -> String {
    let path = path.to_str().unwrap().replace('\\', "/");
    let path = path.strip_prefix("//?/").unwrap_or(&path);
    let mut result = if path.starts_with('/') {
        "file://"
    } else {
        "file:///"
    }
    .to_owned();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            result.push(char::from(byte));
        } else {
            result.push_str(&format!("%{byte:02X}"));
        }
    }
    result
}

struct Client {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<Result<Value, String>>,
    diagnostics: BTreeMap<String, Value>,
    messages: Vec<Value>,
    next: u64,
}
impl Client {
    fn spawn(project: &Project, explicit: bool) -> Self {
        let mut command = project.command();
        command.args(["lsp", "--stdio"]);
        if explicit {
            command.args(["--edition", "typed-preview"]);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let message = (|| -> Result<Option<Value>, String> {
                    let mut line = String::new();
                    if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                        return Ok(None);
                    }
                    let count: usize = line
                        .strip_prefix("Content-Length: ")
                        .and_then(|s| s.trim().parse().ok())
                        .filter(|n| *n <= 32 * 1024 * 1024)
                        .ok_or_else(|| format!("invalid server header {line:?}"))?;
                    line.clear();
                    reader.read_line(&mut line).map_err(|e| e.to_string())?;
                    if line != "\r\n" {
                        return Err(format!("invalid server separator {line:?}"));
                    }
                    let mut body = vec![0; count];
                    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
                    serde_json::from_slice(&body)
                        .map(Some)
                        .map_err(|e| e.to_string())
                })();
                match message {
                    Ok(Some(value)) => {
                        if sender.send(Ok(value)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            }
        });
        Self {
            child,
            input,
            output,
            diagnostics: BTreeMap::new(),
            messages: Vec::new(),
            next: 0,
        }
    }
    fn raw(&mut self, body: &[u8]) {
        write!(self.input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        // Splitting the body checks that the server consumes a stream, not lines.
        for chunk in body.chunks(7) {
            self.input.write_all(chunk).unwrap();
        }
        self.input.flush().unwrap();
    }
    fn send(&mut self, value: Value) {
        self.raw(&serde_json::to_vec(&value).unwrap());
    }
    fn receive(&mut self) -> Value {
        self.output
            .recv_timeout(Duration::from_secs(30))
            .expect("language server response timeout")
            .expect("invalid language server response")
    }
    fn request_id(&mut self, id: Value, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let response = self.receive();
            if response.get("id") == Some(&id) {
                return response;
            }
            self.record(response);
        }
    }
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        self.request_id(self.next.into(), method, params)
    }
    fn record(&mut self, message: Value) {
        if message["method"] == "textDocument/publishDiagnostics" {
            self.diagnostics.insert(
                message["params"]["uri"].as_str().unwrap().into(),
                message["params"].clone(),
            );
        }
        self.messages.push(message);
    }
    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc":"2.0","method":method,"params":params}));
    }
    fn barrier(&mut self) {
        let response = self.request("oxid/testBarrier", Value::Null);
        assert_eq!(response["error"]["code"], -32601);
    }
    fn initialize(&mut self, project: &Project) {
        let response = self.request(
            "initialize",
            json!({"rootUri":project.uri(""),"capabilities":{}}),
        );
        assert_eq!(
            response["result"]["capabilities"]["positionEncoding"], "utf-16",
            "{response}"
        );
        assert_eq!(
            response["result"]["capabilities"]["textDocumentSync"]["change"],
            2
        );
    }
    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument":{"uri":uri,"languageId":"oxid","version":1,"text":text}}),
        );
        self.barrier();
    }
    fn change(&mut self, uri: &str, version: i32, changes: Value) {
        self.notify(
            "textDocument/didChange",
            json!({"textDocument":{"uri":uri,"version":version},"contentChanges":changes}),
        );
        self.barrier();
    }
    fn clean(&self, uri: &str) {
        let report = self
            .diagnostics
            .get(uri)
            .expect("missing diagnostic publication");
        assert_eq!(report["diagnostics"], json!([]), "{report}");
    }
    fn finish(mut self) {
        assert_eq!(self.request("shutdown", Value::Null)["result"], Value::Null);
        self.notify("exit", Value::Null);
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "server did not exit"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(stderr.is_empty(), "{stderr}");
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn stdio_lifecycle_exact_request_ids_and_malformed_json_recovery() {
    let p = Project::new();
    let mut c = Client::spawn(&p, true);
    assert_eq!(
        c.request("textDocument/formatting", json!({}))["error"]["code"],
        -32002
    );
    c.raw(br#"{"jsonrpc":"2.0","id":1,"id":2,"method":"initialize"}"#);
    assert_eq!(c.receive()["error"]["code"], -32700);
    c.initialize(&p);
    for id in [json!(9_223_372_036_854_775_807i64), json!("雪🦀")] {
        let response = c.request_id(id.clone(), "unknown/request", json!({}));
        assert_eq!(response["id"], id);
        assert_eq!(response["error"]["code"], -32601);
    }
    assert_eq!(c.request("initialize", json!({}))["error"]["code"], -32600);
    c.finish();
    p.unchanged();
}

#[test]
fn utf16_incremental_edits_versioning_resync_and_formatting_preserve_disk() {
    let p = Project::new();
    let saved = "fn main()->i32{return 1/0;}\n";
    p.write("main.ox", saved);
    let uri = p.uri("main.ox");
    let text = "fn main()->i32{let c='🦀';return missing;}\r\n";
    let mut c = Client::spawn(&p, true);
    c.initialize(&p);
    c.open(&uri, text);
    let start = text[..text.find("missing").unwrap()].encode_utf16().count();
    let report = &c.diagnostics[&uri];
    assert_eq!(report["version"], 1);
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["range"]["start"] == json!({"line":0,"character":start})),
        "{report}"
    );
    c.change(&uri, 2, json!([{"range":{"start":{"line":0,"character":start},"end":{"line":0,"character":start+7}},"rangeLength":7,"text":"42"}]));
    c.clean(&uri);
    assert_eq!(c.diagnostics[&uri]["version"], 2);
    c.change(&uri, 1, json!([{"text":"bad stale source"}]));
    c.clean(&uri);
    assert_eq!(c.diagnostics[&uri]["version"], 2);
    let response = c.request(
        "textDocument/formatting",
        json!({"textDocument":{"uri":uri},"options":{"tabSize":4,"insertSpaces":true}}),
    );
    let edits = response["result"].as_array().unwrap();
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["range"]["end"], json!({"line":1,"character":0}));
    let formatted = edits[0]["newText"].as_str().unwrap();
    assert!(formatted.contains("return 42;"));
    assert!(formatted.contains("🦀"));
    c.change(&uri, 3, json!([{"text":formatted}]));
    c.clean(&uri);
    assert_eq!(
        c.request(
            "textDocument/formatting",
            json!({"textDocument":{"uri":uri}})
        )["result"],
        json!([])
    );
    c.change(&uri, 4, json!([{"range":{"start":{"line":0,"character":1},"end":{"line":0,"character":0}},"text":""}]));
    assert_eq!(
        c.request(
            "textDocument/formatting",
            json!({"textDocument":{"uri":uri}})
        )["error"]["code"],
        -32803
    );
    c.change(&uri, 5, json!([{"text":saved}]));
    c.clean(&uri);
    c.notify("textDocument/didClose", json!({"textDocument":{"uri":uri}}));
    c.barrier();
    c.clean(&uri);
    c.finish();
    assert_eq!(fs::read_to_string(p.0.join("main.ox")).unwrap(), saved);
    p.unchanged();
}

#[cfg(target_os = "linux")]
#[test]
fn project_modules_and_declared_tests_use_unsaved_buffers_without_execution() {
    let p = Project::new();
    p.package(
        "",
        "app",
        "[tests]\nsmoke=\"tests/smoke.ox\"\n",
        "mod helper;fn main()->i32{return crate::helper::answer()+1/0;}",
    );
    let bad = "pub fn answer()->i32{return missing;}";
    p.write("src/helper.ox", bad);
    p.write("tests/smoke.ox", "fn main()->i32{return missing;}");
    let helper = p.uri("src/helper.ox");
    let test = p.uri("tests/smoke.ox");
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    c.open(&helper, "pub fn answer()->i32{return 42;}");
    c.clean(&helper);
    assert!(!c.diagnostics[&test]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.open(&test, "fn main()->i32{return 1/0;}");
    c.clean(&test);
    c.clean(&helper);
    c.change(&helper, 2, json!([{"text":bad}]));
    assert!(!c.diagnostics[&helper]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.change(
        &helper,
        3,
        json!([{"text":"pub fn answer()->i32{return 42;}"}]),
    );
    c.clean(&helper);
    c.finish();
    assert_eq!(fs::read_to_string(p.0.join("src/helper.ox")).unwrap(), bad);
    p.unchanged();
}

#[cfg(target_os = "linux")]
#[test]
fn dependency_buffer_changes_update_importing_project_and_preserve_lock() {
    let p = Project::new();
    p.package(
        "",
        "application",
        "[dependencies]\nmath=\"math\"\n",
        "use crate::math::answer;fn main()->i32{return answer();}",
    );
    p.write(
        "math/oxid.toml",
        "[project]\nname=\"math\"\nedition=\"typed-preview\"\nkind=\"lib\"\n",
    );
    let bad = "pub fn answer()->bool{return true;}";
    p.write("math/src/lib.ox", bad);
    let lock = p.command().arg("lock").output().unwrap();
    assert!(lock.status.success(), "{lock:?}");
    let lock = fs::read(p.0.join("oxid.lock")).unwrap();
    let app = p.uri("src/main.ox");
    let library = p.uri("math/src/lib.ox");
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    c.open(
        &app,
        "use crate::math::answer;fn main()->i32{return answer();}",
    );
    assert!(!c.diagnostics[&app]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.open(&library, "pub fn answer()->i32{return 42;}");
    c.clean(&app);
    c.clean(&library);
    c.notify(
        "textDocument/didClose",
        json!({"textDocument":{"uri":library}}),
    );
    c.barrier();
    assert!(!c.diagnostics[&app]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.finish();
    assert_eq!(fs::read(p.0.join("oxid.lock")).unwrap(), lock);
    assert_eq!(
        fs::read_to_string(p.0.join("math/src/lib.ox")).unwrap(),
        bad
    );
    assert!(!p.0.join(".oxid").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn workspace_checks_every_member_including_unselected_members() {
    let p = Project::new();
    p.write("oxid.toml", "[workspace]\nedition=\"typed-preview\"\nmembers=[\"one\",\"two\"]\ndefault-members=[\"one\"]\n");
    p.package("one/", "one", "", "fn main()->i32{return 1/0;}");
    p.package("two/", "two", "", "fn main()->i32{return missing;}");
    let one = p.uri("one/src/main.ox");
    let two = p.uri("two/src/main.ox");
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    c.open(&one, "fn main()->i32{return 1/0;}");
    assert!(!c.diagnostics[&two]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.open(&two, "fn main()->i32{return 2;}");
    c.clean(&one);
    c.clean(&two);
    c.finish();
    p.unchanged();
}

#[test]
fn explicit_edition_boundary_is_visible() {
    let p = Project::new();
    p.write("main.ox", "fn main()->i32{return 42;}");
    let path = p.uri("main.ox");
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    c.open(&path, "fn main()->i32{return 42;}");
    assert!(c.diagnostics[&path]["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("typed"));
    assert_eq!(
        c.request(
            "textDocument/formatting",
            json!({"textDocument":{"uri":path}})
        )["error"]["code"],
        -32803
    );
    c.finish();
    p.unchanged();
}

#[cfg(target_os = "linux")]
#[test]
fn watched_disk_changes_and_unlinked_files_do_not_report_stale_success() {
    let p = Project::new();
    p.package(
        "",
        "application",
        "entry=\"./src/main.ox\"\n",
        "mod helper;fn main()->i32{return crate::helper::answer();}",
    );
    p.write("src/helper.ox", "pub fn answer()->i32{return 42;}");
    p.write("unused.ox", "fn main()->i32{return 0;}");
    let main = p.uri("src/main.ox");
    let helper = p.uri("src/helper.ox");
    let unused = p.uri("unused.ox");
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    c.open(
        &main,
        "mod helper;fn main()->i32{return crate::helper::answer();}",
    );
    c.clean(&main);
    p.write("src/helper.ox", "pub fn answer()->i32{return missing;}");
    c.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes":[{"uri":helper,"type":2}]}),
    );
    c.barrier();
    assert!(!c.diagnostics[&helper]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    p.write("src/helper.ox", "pub fn answer()->i32{return 42;}");
    c.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes":[{"uri":helper,"type":2}]}),
    );
    c.barrier();
    c.clean(&helper);
    c.open(&unused, "fn main()->i32{return 0;}");
    assert!(c.diagnostics[&unused]["diagnostics"][0]["message"]
        .as_str()
        .unwrap()
        .contains("not part of a checked project entry"));
    c.finish();
    p.unchanged();
}

#[test]
fn invalid_versions_and_oversized_updates_clear_results_until_full_resync() {
    let p = Project::new();
    p.write("main.ox", "fn main()->i32{return 42;}");
    let path = p.uri("main.ox");
    let mut c = Client::spawn(&p, true);
    c.initialize(&p);
    c.open(&path, "fn main()->i32{return missing;}");
    assert!(!c.diagnostics[&path]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.notify("textDocument/didChange", json!({"textDocument":{"uri":path,"version":"invalid"},"contentChanges":[{"text":"fn main()->i32{return 42;}"}]}));
    c.barrier();
    c.clean(&path);
    assert_eq!(
        c.request(
            "textDocument/formatting",
            json!({"textDocument":{"uri":path}})
        )["error"]["code"],
        -32803
    );
    c.change(&path, 2, json!([{"text":"fn main()->i32{return 42;}"}]));
    c.clean(&path);
    c.change(&path, 3, json!([{"text":"x".repeat(1_048_577)}]));
    c.clean(&path);
    assert_eq!(
        c.request(
            "textDocument/formatting",
            json!({"textDocument":{"uri":path}})
        )["error"]["code"],
        -32803
    );
    c.notify(
        "textDocument/didSave",
        json!({"textDocument":{"uri":path},"text":"fn main()->i32{return 42;}"}),
    );
    c.barrier();
    c.clean(&path);
    assert!(c
        .request(
            "textDocument/formatting",
            json!({"textDocument":{"uri":path}})
        )
        .get("result")
        .is_some());
    c.finish();
    p.unchanged();
}

#[cfg(target_os = "linux")]
#[test]
fn workspace_folder_changes_replace_closed_project_reports() {
    let p = Project::new();
    p.package("one/", "one", "", "fn main()->i32{return missing;}");
    p.package("two/", "two", "", "fn main()->i32{return other_missing;}");
    let mut c = Client::spawn(&p, false);
    let response = c.request(
        "initialize",
        json!({"workspaceFolders":[{"uri":p.uri("one"),"name":"one"}],"capabilities":{}}),
    );
    assert!(response.get("result").is_some(), "{response}");
    c.notify("initialized", json!({}));
    c.barrier();
    let one = p.uri("one/src/main.ox");
    let two = p.uri("two/src/main.ox");
    assert!(!c.diagnostics[&one]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.notify("workspace/didChangeWorkspaceFolders", json!({"event":{"removed":[{"uri":p.uri("one"),"name":"one"}],"added":[{"uri":p.uri("two"),"name":"two"}]}}));
    c.barrier();
    c.clean(&one);
    assert!(!c.diagnostics[&two]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.finish();
    p.unchanged();
}

#[test]
fn malformed_frame_rejects_before_source_access() {
    let p = Project::new();
    let mut child = p
        .command()
        .args(["lsp", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"Content-Length: 8388609\r\n\r\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(74));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("frame exceeds byte limit"));
    p.unchanged();
}

fn position_at(text: &str, needle: &str) -> Value {
    let prefix = &text[..text.find(needle).unwrap()];
    json!({"line":prefix.bytes().filter(|byte|*byte==b'\n').count(),"character":prefix.rsplit('\n').next().unwrap().encode_utf16().count()})
}

#[test]
fn local_reference_navigation_follows_borrows_fields_calls_and_scope() {
    let p = Project::new();
    let text = "struct V{x:i32}\nfn bump(v:&mut V)->(){v.x=v.x+1;return;}\nfn relay(input:&mut V)->i32{\nlet parent=&mut *input;\nif true{\nlet child=&mut *parent;\nbump(&mut *child);\nchild.x=42;\n}\nreturn parent.x;\n}\nfn main()->i32{let mut value=V{x:0};let local=&mut value;return relay(&mut *local);}";
    p.write("main.ox", text);
    let path = p.uri("main.ox");
    let mut c = Client::spawn(&p, true);
    c.initialize(&p);
    c.open(&path, text);
    c.clean(&path);
    for (use_site, declaration, width) in [
        ("input;", "input:", 5),
        ("parent;", "parent=", 6),
        ("child);", "child=", 5),
        ("child.x=", "child=", 5),
        ("parent.x;", "parent=", 6),
        ("value;", "value=", 5),
        ("local);", "local=", 5),
    ] {
        let result = c.request(
            "textDocument/definition",
            json!({"textDocument":{"uri":path},"position":position_at(text,use_site)}),
        );
        let start = position_at(text, declaration);
        let mut end = start.clone();
        end["character"] = json!(start["character"].as_u64().unwrap() + width);
        assert_eq!(
            result["result"],
            json!([{"uri":path,"range":{"start":start,"end":end}}]),
            "{use_site}: {result}"
        );
    }
    let invalid = text.replace("return parent.x;", "return child.x;");
    c.change(&path, 2, json!([{"text":invalid}]));
    let mut position = position_at(&invalid, "return child.x;");
    position["character"] = json!(position["character"].as_u64().unwrap() + 7);
    let result = c.request(
        "textDocument/definition",
        json!({"textDocument":{"uri":path},"position":position}),
    );
    assert!(result.get("error").is_none(), "{result}");
    assert_eq!(result.get("result"), Some(&Value::Null));
    assert!(!c.diagnostics[&path]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.finish();
    p.unchanged();
}

#[test]
fn definition_navigation_distinguishes_parameters_locals_and_function_calls() {
    let p = Project::new();
    let text="fn square(value:i32)->i32 {\nlet c='🦀';\nlet mut total=value;\ntotal=total+1;\nreturn total;\n}\nfn main()->i32{return square(41);}";
    p.write("main.ox", text);
    let path = p.uri("main.ox");
    let mut c = Client::spawn(&p, true);
    c.initialize(&p);
    c.open(&path, text);
    for (needle, line, start, end) in [
        ("square(41)", 0, 3, 9),
        ("value;", 0, 10, 15),
        ("total+1", 2, 8, 13),
    ] {
        let result = c.request(
            "textDocument/definition",
            json!({"textDocument":{"uri":path},"position":position_at(text,needle)}),
        );
        assert_eq!(
            result["result"],
            json!([{"uri":path,"range":{"start":{"line":line,"character":start},"end":{"line":line,"character":end}}}]),
            "{result}"
        );
    }
    let position = position_at(text, "🦀");
    let mut split = position.clone();
    split["character"] = json!(position["character"].as_u64().unwrap() + 1);
    assert_eq!(
        c.request(
            "textDocument/definition",
            json!({"textDocument":{"uri":path},"position":split})
        )["error"]["code"],
        -32803
    );
    c.finish();
    p.unchanged();
}

#[cfg(target_os = "linux")]
#[test]
fn function_definitions_follow_import_aliases_unsaved_lines_and_privacy() {
    let p = Project::new();
    let main="mod math;use crate::math::answer as value;fn main()->i32{return value()+crate::math::answer();}";
    p.package("", "app", "", main);
    p.write(
        "src/math.ox",
        "pub fn answer()->i32{return 42;}fn private()->i32{return 1/0;}",
    );
    let path = p.uri("src/main.ox");
    let library = p.uri("src/math.ox");
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    c.open(&path, main);
    c.open(
        &library,
        "\n\n\npub fn answer()->i32{return 42;}fn private()->i32{return 1/0;}",
    );
    for needle in ["value()+", "answer();"] {
        let result = c.request(
            "textDocument/definition",
            json!({"textDocument":{"uri":path},"position":position_at(main,needle)}),
        );
        assert_eq!(
            result["result"],
            json!([{"uri":library,"range":{"start":{"line":3,"character":7},"end":{"line":3,"character":13}}}]),
            "{result}"
        );
    }
    let bad = "mod math;fn main()->i32{return crate::math::private();}";
    c.change(&path, 2, json!([{"text":bad}]));
    assert!(!c.diagnostics[&path]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    let result = c.request(
        "textDocument/definition",
        json!({"textDocument":{"uri":path},"position":position_at(bad,"private();")}),
    );
    assert_eq!(result["result"], Value::Null, "{result}");
    assert!(result.get("error").is_none());
    c.finish();
    p.unchanged();
}

#[test]
fn document_outline_uses_declarations_and_survives_body_type_errors() {
    let p = Project::new();
    let text="pub struct Pair{pub left:i32,right:i32}\npub enum Choice{Yes(i32),No}\nfn main()->i32{return missing;}";
    p.write("main.ox", text);
    let path = p.uri("main.ox");
    let mut c = Client::spawn(&p, true);
    c.initialize(&p);
    c.open(&path, text);
    let result = c.request(
        "textDocument/documentSymbol",
        json!({"textDocument":{"uri":path}}),
    );
    let entries = result["result"].as_array().expect("missing outline");
    assert_eq!(entries.len(), 7, "{result}");
    for (name, kind, container) in [
        ("Pair", 23, None),
        ("left", 8, Some("Pair")),
        ("right", 8, Some("Pair")),
        ("Choice", 10, None),
        ("Yes", 22, Some("Choice")),
        ("No", 22, Some("Choice")),
        ("main", 12, None),
    ] {
        let entry = entries.iter().find(|entry| entry["name"] == name).unwrap();
        assert_eq!(entry["kind"], kind);
        assert_eq!(entry["location"]["uri"], path);
        assert_eq!(
            entry.get("containerName").and_then(Value::as_str),
            container
        );
    }
    assert!(!c.diagnostics[&path]["diagnostics"]
        .as_array()
        .unwrap()
        .is_empty());
    c.finish();
    p.unchanged();
}

#[cfg(target_os = "linux")]
#[test]
fn workspace_symbol_search_keeps_same_names_distinct_and_reads_buffers() {
    let p = Project::new();
    p.write("oxid.toml","[workspace]\nedition=\"typed-preview\"\nmembers=[\"one\",\"two\"]\ndefault-members=[\"one\"]\n");
    for name in ["one", "two"] {
        p.package(
            &format!("{name}/"),
            name,
            "kind=\"lib\"\nentry=\"src/main.ox\"\n",
            "pub fn shared()->i32{return 42;}",
        );
    }
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    let result = c.request("workspace/symbol", json!({"query":"shared"}));
    let entries = result["result"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "{result}");
    assert_ne!(entries[0]["location"]["uri"], entries[1]["location"]["uri"]);
    let one = p.uri("one/src/main.ox");
    c.open(&one, "pub fn only_buffer()->i32{return 42;}");
    let result = c.request("workspace/symbol", json!({"query":"only_buffer"}));
    assert_eq!(result["result"].as_array().unwrap().len(), 1);
    assert_eq!(result["result"][0]["location"]["uri"], one);
    let result = c.request("workspace/symbol", json!({"query":"shared"}));
    assert_eq!(result["result"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["result"][0]["location"]["uri"],
        p.uri("two/src/main.ox")
    );
    c.finish();
    p.unchanged();
}

#[test]
fn range_index_definition_is_visible_only_inside_its_loop() {
    let p = Project::new();
    let text =
        "fn main()->i32{let mut total=0;for index in 0..2 {total=total+index;}return total;}";
    p.write("main.ox", text);
    let path = p.uri("main.ox");
    let mut c = Client::spawn(&p, true);
    c.initialize(&p);
    c.open(&path, text);
    let result = c.request(
        "textDocument/definition",
        json!({"textDocument":{"uri":path},"position":position_at(text,"index;")}),
    );
    let start = position_at(text, "index in");
    let mut end = start.clone();
    end["character"] = json!(start["character"].as_u64().unwrap() + 5);
    assert_eq!(
        result["result"],
        json!([{"uri":path,"range":{"start":start,"end":end}}]),
        "{result}"
    );
    let bad = text.replace("return total;", "return index;");
    c.change(&path, 2, json!([{"text":bad}]));
    let prefix = bad.rfind("index;").unwrap();
    let position = json!({"line":0,"character":bad[..prefix].encode_utf16().count()});
    let result = c.request(
        "textDocument/definition",
        json!({"textDocument":{"uri":path},"position":position}),
    );
    assert_eq!(result["result"], Value::Null, "{result}");
    assert!(result.get("error").is_none());
    c.finish();
    p.unchanged();
}

#[test]
fn symbol_limit_is_enforced_before_publication_and_narrow_queries_still_work() {
    let p = Project::new();
    let source = |fields: usize| {
        format!(
            "struct Many{{{}}}\nfn main()->i32{{return 0;}}",
            (0..fields)
                .map(|index| format!("field_{index}:i32,"))
                .collect::<String>()
        )
    };
    let text = source(4094);
    p.package("", "app", "", &text);
    let path = p.uri("src/main.ox");
    let mut c = Client::spawn(&p, false);
    c.initialize(&p);
    c.open(&path, &text);
    let result = c.request(
        "textDocument/documentSymbol",
        json!({"textDocument":{"uri":path}}),
    );
    assert_eq!(result["result"].as_array().unwrap().len(), 4096, "{result}");
    c.change(&path, 2, json!([{"text":source(4095)}]));
    let result = c.request(
        "textDocument/documentSymbol",
        json!({"textDocument":{"uri":path}}),
    );
    assert_eq!(result["error"]["code"], -32803, "{result}");
    assert!(result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("too many symbols"));
    let result = c.request("workspace/symbol", json!({"query":"field_4094"}));
    assert_eq!(result["result"].as_array().unwrap().len(), 1, "{result}");
    assert_eq!(result["result"][0]["name"], "field_4094");
    c.finish();
    p.unchanged();
}

#[test]
fn generic_instantiations_navigate_to_the_original_function_declaration() {
    let p = Project::new();
    let text="fn identity<T>(value:T)->T{return value;}\nfn main()->i32{let ignored=identity::<bool>(true);return identity::<i32>(42);}";
    p.write("main.ox", text);
    let path = p.uri("main.ox");
    let mut c = Client::spawn(&p, true);
    c.initialize(&p);
    c.open(&path, text);
    for needle in ["identity::<bool>", "identity::<i32>"] {
        let result = c.request(
            "textDocument/definition",
            json!({"textDocument":{"uri":path},"position":position_at(text,needle)}),
        );
        assert_eq!(
            result["result"],
            json!([{"uri":path,"range":{"start":{"line":0,"character":3},"end":{"line":0,"character":11}}}]),
            "{result}"
        );
    }
    c.finish();
    p.unchanged();
}
