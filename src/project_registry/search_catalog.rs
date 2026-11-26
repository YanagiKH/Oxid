//! Bounded registry search over a trusted HTTPS connection.
use super::{name, version, Budget, MAX_SEARCH_ENTRIES};
use std::{
    env,
    io::{Read, Write},
    process::{Command, Stdio},
};

pub(super) const HEADER: &str = "Oxid registry catalog 1\n";
const CATALOG_LIMIT: usize = 1024 * 1024;

fn check_url(url: &str) -> Result<(), String> {
    let Some(rest) = url.strip_prefix("https://") else {
        return Err("registry catalogs require an HTTPS URL".into());
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if url.len() > 2048
        || !url.bytes().all(|byte| byte.is_ascii_graphic())
        || url.contains(['\\', '#', '?'])
        || authority.is_empty()
        || authority.contains(['@', '%'])
    {
        return Err("invalid registry catalog URL; use an HTTPS URL without credentials, query, or fragment".into());
    }
    Ok(())
}

fn check_token(token: &str) -> Result<(), String> {
    let body = token.trim_end_matches('=');
    if token.len() > 4096
        || body.is_empty()
        || !body.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'+' | b'/')
        })
    {
        return Err("OXID_REGISTRY_TOKEN must contain a valid bearer token".into());
    }
    Ok(())
}

fn fetch(url: &str, budget: &mut Budget) -> Result<Vec<u8>, String> {
    check_url(url)?;
    let token = match env::var("OXID_REGISTRY_TOKEN") {
        Ok(token) => {
            check_token(&token)?;
            Some(token)
        }
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            return Err("OXID_REGISTRY_TOKEN must contain a valid bearer token".into())
        }
    };
    let mut body = Vec::new();
    budget.reserve(&mut body, CATALOG_LIMIT + 1)?;
    let mut command = Command::new("curl");
    command
        .args([
            "--disable",
            "--silent",
            "--globoff",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--connect-timeout",
            "5",
            "--max-time",
            "30",
            "--max-filesize",
            "1048576",
            "--write-out",
            "%{stderr}%{http_code}",
            "--url",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("OXID_REGISTRY_TOKEN");
    if token.is_some() {
        command.args(["--header", "@-"]).stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start curl for registry search: {error}"))?;
    let result = (|| {
        if let Some(token) = token {
            let mut headers = child
                .stdin
                .take()
                .ok_or("cannot send registry authorization")?;
            headers
                .write_all(b"Authorization: Bearer ")
                .and_then(|()| headers.write_all(token.as_bytes()))
                .and_then(|()| headers.write_all(b"\n"))
                .map_err(|_| "cannot send registry authorization")?;
        }
        let mut stream = child.stdout.take().ok_or("cannot read registry catalog")?;
        let mut used = 0;
        body.resize(CATALOG_LIMIT + 1, 0);
        while used < body.len() {
            let read = stream
                .read(&mut body[used..])
                .map_err(|_| "cannot read registry catalog")?;
            if read == 0 {
                break;
            }
            used += read;
        }
        if used > CATALOG_LIMIT {
            return Err("registry catalog exceeds 1 MiB".into());
        }
        body.truncate(used);
        let mut status = [0; 3];
        let received_status = child
            .stderr
            .take()
            .ok_or("cannot read registry status")?
            .read_exact(&mut status)
            .is_ok();
        let exit = child.wait().map_err(|_| "cannot finish registry request")?;
        if exit.code() == Some(63) {
            return Err("registry catalog exceeds 1 MiB".into());
        }
        if !exit.success() || !received_status || !status.iter().all(u8::is_ascii_digit) {
            return Err("HTTPS registry request failed; check the address and certificate".into());
        }
        match &status {
            b"200" => Ok(body),
            b"401" | b"403" => {
                Err("registry authorization failed; check OXID_REGISTRY_TOKEN".into())
            }
            _ => Err(format!(
                "registry catalog returned HTTP {}",
                std::str::from_utf8(&status).unwrap()
            )),
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

fn parse(body: &[u8], prefix: &str, budget: &mut Budget) -> Result<Vec<(String, String)>, String> {
    let rows = std::str::from_utf8(body)
        .ok()
        .and_then(|text| text.strip_prefix(HEADER))
        .ok_or("unsupported registry catalog format")?;
    if !rows.is_empty() && !rows.ends_with('\n') {
        return Err("registry catalog has an incomplete final record".into());
    }
    let mut previous = "";
    let mut results = Vec::new();
    for (index, row) in rows.split_terminator('\n').enumerate() {
        if index >= MAX_SEARCH_ENTRIES {
            return Err("registry catalog exceeds the 4096-package limit".into());
        }
        let (package, latest) = row
            .split_once(' ')
            .ok_or("invalid registry catalog record")?;
        name(package)?;
        version(latest)?;
        if package <= previous {
            return Err("registry catalog package names must be unique and sorted".into());
        }
        previous = package;
        if package.starts_with(prefix) {
            budget.reserve(&mut results, 1)?;
            results.push((budget.text(package)?, budget.text(latest)?));
        }
    }
    Ok(results)
}

pub(super) fn search(
    url: &str,
    prefix: &str,
    budget: &mut Budget,
) -> Result<Vec<(String, String)>, String> {
    let body = fetch(url, budget)?;
    parse(&body, prefix, budget)
}
