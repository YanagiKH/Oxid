//! Publish one checked API page only after all selected package fragments pass.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const MAX_BYTES: usize = 32 * 1024 * 1024;
const PREFIX: &str = "<!doctype html>\n<html lang=\"en\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Oxid API documentation</title>\n<style>body{font:16px/1.6 system-ui,sans-serif;max-width:70rem;margin:2rem auto;padding:0 1rem;color:#17212b;background:#fff}pre{overflow:auto;background:#f4f6f8;padding:1rem}code{font-family:ui-monospace,monospace}.description{white-space:pre-wrap}.package{border-top:1px solid #ccd3db;margin-top:2rem}article{margin:2rem 0}.field,.variant{margin-left:1rem}h4{margin-bottom:.5rem}</style>\n<main><h1>API documentation</h1>\n";
const SUFFIX: &str = "</main></html>\n";
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) fn remaining() -> usize {
    MAX_BYTES - PREFIX.len() - SUFFIX.len()
}

pub(super) fn fragment(
    prepared: &frontend::PreparedPackageProject<'_>,
    manifest: &runtime::ProjectManifest,
    remaining: &mut usize,
) -> Result<String, String> {
    let fragment = prepared.documentation(
        manifest.name.as_deref().unwrap_or("crate"),
        manifest.version.as_deref(),
        *remaining,
    )?;
    *remaining = remaining
        .checked_sub(fragment.len())
        .ok_or("documentation output limit exceeded")?;
    Ok(fragment)
}

pub(super) fn command(
    root: &Path,
    snapshot: &runtime::ManifestSnapshot,
    options: &Options,
) -> Result<i32, String> {
    if options.words.len() != 1 || options.entry_mode.is_some() || options.project_test.is_some() {
        return Err("typed doc accepts edition, message-format and locking options only".into());
    }
    let existing = project_lock::read(root, options.requires_lock())?;
    let graph = package_graph_options(
        root,
        snapshot,
        options.requires_lock(),
        options.offline || options.frozen,
    )?;
    let prepared =
        frontend::prepare_package_project(&graph.specs, None).map_err(|error| error.message())?;
    project_lock::verify(&lock_snapshot(&graph, &prepared)?, existing.as_ref())?;
    let mut capacity = remaining();
    let page = fragment(&prepared, &snapshot.manifest, &mut capacity)?;
    publish(root, &[page], options)
}

fn directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(format!(
            "documentation output requires an ordinary directory: {}",
            path.display()
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(path)
            .map_err(|error| format!("cannot create documentation directory: {error}")),
        Err(error) => Err(format!("cannot inspect documentation directory: {error}")),
    }
}

struct Temporary(Option<PathBuf>);
impl Drop for Temporary {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

pub(super) fn publish(root: &Path, fragments: &[String], options: &Options) -> Result<i32, String> {
    let length = fragments
        .iter()
        .try_fold(PREFIX.len() + SUFFIX.len(), |length, fragment| {
            length.checked_add(fragment.len())
        })
        .ok_or("documentation output length overflow")?;
    if length > MAX_BYTES {
        return Err("documentation output exceeds 32 MiB".into());
    }
    let base = root.join(".oxid");
    directory(&base)?;
    let output = base.join("doc");
    directory(&output)?;
    let index = output.join("index.html");
    match fs::symlink_metadata(&index) {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Err("documentation index must be a regular file".into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect documentation index: {error}")),
    }
    let mut temporary = None;
    for _ in 0..64 {
        let path = output.join(format!(
            ".index-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut open = fs::OpenOptions::new();
        open.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            open.mode(0o600);
        }
        match open.open(&path) {
            Ok(file) => {
                temporary = Some((file, Temporary(Some(path))));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(format!("cannot stage documentation: {error}")),
        }
    }
    let (mut file, mut temporary) = temporary.ok_or("cannot reserve documentation output")?;
    file.write_all(PREFIX.as_bytes())
        .and_then(|_| {
            for fragment in fragments {
                file.write_all(fragment.as_bytes())?;
            }
            file.write_all(SUFFIX.as_bytes())
        })
        .map_err(|error| format!("cannot write documentation: {error}"))?;
    drop(file);
    fs::rename(temporary.0.as_ref().expect("owned staging path"), &index)
        .map_err(|error| format!("cannot publish documentation: {error}"))?;
    temporary.0 = None;
    let path = index.to_str().ok_or("documentation path must be UTF-8")?;
    let message = if options.json() {
        format!("{{\"schema_version\":1,\"edition\":\"typed-preview\",\"kind\":\"doc-summary\",\"success\":true,\"packages\":{},\"path\":{}}}\n", fragments.len(), json_string(path))
    } else {
        format!("Documentation: {path}\n")
    };
    io::stdout()
        .lock()
        .write_all(message.as_bytes())
        .map_err(|error| format!("cannot report documentation: {error}"))?;
    Ok(0)
}
