//! Coverage of preexisting records.ox wrapper routes (source + OXBC).
//! Roadmap contract coverage — not a claim that records guidance is v1.0-complete.
//! Error text is characterized here, not promoted to a stable diagnostic schema.
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        // A per-process name alone collides when tests in this target run in parallel.
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time follows Unix epoch")
            .as_nanos();
        for _ in 0..16 {
            let root = std::env::temp_dir().join(format!(
                "oxid-stdlib-records-{}-{stamp}-{}",
                std::process::id(),
                NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&root) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create isolated records project: {error}"),
            }
            let project = Self(root);
            fs::create_dir(project.0.join("stdlib")).expect("create stdlib directory");
            // The checkout's records module currently has no imported dependencies.
            fs::write(
                project.0.join("stdlib/records.ox"),
                include_str!("../stdlib/records.ox"),
            )
            .expect("write actual records module");
            fs::write(project.0.join("main.ox"), source).expect("write records fixture");
            return project;
        }
        panic!("could not reserve an exclusive records project directory");
    }

    fn command(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_oxid"))
            .args(arguments)
            .current_dir(&self.0)
            .env("OXID_CACHE_DIR", self.0.join("cache"))
            .env_remove("OXID_PATH")
            .output()
            .expect("run Oxid CLI")
    }

    fn compile(&self) {
        let output = self.command(&["compile", "main.ox", "-o", "main.oxb"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        assert!(fs::read(self.0.join("main.oxb"))
            .expect("read compiled artifact")
            .starts_with(b"OXBC"));
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn assert_stdout(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert_eq!(output.stdout, expected.as_bytes());
}

fn assert_source_and_oxbc_stdout(project: &Project, expected: &str) {
    assert_stdout(&project.command(&["run", "main.ox"]), expected);
    project.compile();
    assert_stdout(&project.command(&["run", "main.oxb"]), expected);
}

fn assert_runtime_reject(output: &Output, reason: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let diagnostic = std::str::from_utf8(&output.stderr).expect("UTF-8 diagnostic");
    assert!(diagnostic.starts_with("error: "), "{diagnostic}");
    // Pin the stable reason substring, not environment-dependent absolute paths.
    assert!(diagnostic.contains(reason), "{diagnostic}");
}

fn assert_source_and_oxbc_reject(project: &Project, reason: &str) {
    assert_runtime_reject(&project.command(&["run", "main.ox"]), reason);
    project.compile();
    assert_runtime_reject(&project.command(&["run", "main.oxb"]), reason);
}

#[test]
fn record_select_new_outer_omits_missing_repeats_last_and_shares_nested_on_both_routes() {
    // Outer selected record is a fresh {}; missing keys are omitted; a repeated
    // selected key is written once (last set wins). Nested Record values from
    // get() are Rc-shared aliases with the source; the outer shell is not.
    let project = Project::new(
        r#"use "stdlib/records.ox";
let src = {a: 1, nested: {ready: true}, b: 2};
let sel = record_select(src, ["missing", "a", "a", "nested"]);
print json_stringify(sel);
print json_stringify(record_keys(sel));
record_put(sel, "extra", 99);
print record_has(src, "extra");
print record_has(sel, "extra");
sel.nested.ready = false;
print src.nested.ready;
print sel.nested.ready;
print record_has(src, "b");
print record_has(sel, "b");
"#,
    );
    let expected = concat!(
        "{\"a\":1,\"nested\":{\"ready\":true}}\n",
        "[\"a\",\"nested\"]\n",
        "false\n",
        "true\n",
        "false\n",
        "false\n",
        "true\n",
        "false\n",
    );
    assert_source_and_oxbc_stdout(&project, expected);
}

#[test]
fn record_merge_into_overwrites_returns_target_alias_and_self_merges_on_both_routes() {
    // Conflict overwrites the target field; the return value is the same target
    // alias (mutate-through-return observes on the original); self-merge works.
    let project = Project::new(
        r#"use "stdlib/records.ox";
let target = {a: 1, b: 2};
let returned = record_merge_into(target, {b: 9, c: 3});
print json_stringify(target);
record_put(returned, "d", 4);
print json_stringify(target);
print json_stringify(returned);
let self_merge = {x: 1, y: 2};
record_merge_into(self_merge, self_merge);
print json_stringify(self_merge);
record_put(self_merge, "x", 7);
print json_stringify(self_merge);
"#,
    );
    let expected = concat!(
        "{\"a\":1,\"b\":9,\"c\":3}\n",
        "{\"a\":1,\"b\":9,\"c\":3,\"d\":4}\n",
        "{\"a\":1,\"b\":9,\"c\":3,\"d\":4}\n",
        "{\"x\":1,\"y\":2}\n",
        "{\"x\":7,\"y\":2}\n",
    );
    assert_source_and_oxbc_stdout(&project, expected);
}

#[test]
fn record_get_preserves_stored_null_false_zero_and_delete_returns_on_both_routes() {
    // Stored null / false / 0 are returned (not treated as missing). Missing
    // keys use the fallback. record_delete returns the removed value, or null
    // when the key is absent.
    let project = Project::new(
        r#"use "stdlib/records.ox";
let value = record_from_pairs(["n", null, "f", false, "z", 0, "s", "keep"]);
print record_get(value, "n", "FALL") == null;
print record_get(value, "f", true) == false;
print record_get(value, "z", 9) == 0;
print record_get(value, "missing", "FALL") == "FALL";
print record_get(value, "n", "FALL");
print record_get(value, "f", true);
print record_get(value, "z", 9);
print record_get(value, "missing", "FALL");
print record_delete(value, "s");
print record_has(value, "s");
print record_delete(value, "gone");
print record_delete(value, "n") == null;
"#,
    );
    let expected = concat!(
        "true\n", "true\n", "true\n", "true\n", "null\n", "false\n", "0\n", "FALL\n", "keep\n",
        "false\n", "null\n", "true\n",
    );
    assert_source_and_oxbc_stdout(&project, expected);
}

#[test]
fn record_from_pairs_duplicate_keys_last_wins_on_both_routes() {
    // Existing behavior: builtin record() inserts into a BTreeMap, so duplicate
    // keys overwrite (last wins) and do not raise a runtime error. Positive
    // oracle for this expected last-wins behavior on both routes.
    let project = Project::new(
        r#"use "stdlib/records.ox";
print json_stringify(record_from_pairs(["a", 1, "b", 2, "a", 3]));
print json_stringify(record_keys(record_from_pairs(["a", 1, "b", 2, "a", 3])));
"#,
    );
    let expected = "{\"a\":3,\"b\":2}\n[\"a\",\"b\"]\n";
    assert_source_and_oxbc_stdout(&project, expected);
}

#[test]
fn record_from_pairs_rejects_odd_length_at_runtime_on_both_routes() {
    // Compile still succeeds; runtime rejects with a fixed reason substring.
    let project = Project::new(
        r#"use "stdlib/records.ox";
print record_from_pairs(["a", 1, "b"]);
"#,
    );
    assert_source_and_oxbc_reject(
        &project,
        "record requires alternating string keys and values",
    );
}

#[test]
fn record_from_pairs_rejects_non_string_key_at_runtime_on_both_routes() {
    // Compile still succeeds; runtime rejects via string_argument on the key.
    let project = Project::new(
        r#"use "stdlib/records.ox";
print record_from_pairs([1, "a"]);
"#,
    );
    assert_source_and_oxbc_reject(&project, "record key must be a string");
}
