//! Bounded GET requests with system TLS verification and explicit credentials.
use std::{
    env,
    io::{Read, Write},
    process::{Command, Stdio},
};

pub(crate) enum Error {
    TooLarge,
    Request(String),
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self::Request(message)
    }
}
impl From<&str> for Error {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

pub(crate) fn check_url(url: &str) -> Result<(), String> {
    let Some(rest) = url.strip_prefix("https://") else {
        return Err("source downloads require an HTTPS URL".into());
    };
    let authority = rest.split('/').next().unwrap_or_default();
    if url.len() > 2048
        || !url.bytes().all(|byte| byte.is_ascii_graphic())
        || url.contains(['\\', '#', '?'])
        || authority.is_empty()
        || authority.contains(['@', '%'])
    {
        return Err(
            "invalid HTTPS source URL; use an HTTPS URL without credentials, query, or fragment"
                .into(),
        );
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
        return Err("HTTPS token environment variable must contain a valid bearer token".into());
    }
    Ok(())
}

fn read_token(variable: &str) -> Result<String, String> {
    if variable.is_empty()
        || variable.len() > 64
        || !variable.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || index != 0 && byte.is_ascii_digit()
        })
    {
        return Err("--token-env requires an environment variable name".into());
    }
    let token = env::var(variable).map_err(|error| match error {
        env::VarError::NotPresent => "HTTPS token environment variable is unset",
        env::VarError::NotUnicode(_) => "HTTPS token environment variable is not valid text",
    })?;
    check_token(&token)?;
    Ok(token)
}

pub(crate) fn download(
    url: &str,
    variable: Option<&str>,
    limit: u64,
    timeout: u32,
    output: &mut impl Write,
) -> Result<(), Error> {
    check_url(url)?;
    let token = variable.map(read_token).transpose()?;
    let max_size = limit.to_string();
    let timeout = timeout.to_string();
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
            &timeout,
            "--max-filesize",
            &max_size,
            "--write-out",
            "%{stderr}%{http_code}",
            "--url",
            url,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(variable) = variable {
        command
            .env_remove(variable)
            .args(["--header", "@-"])
            .stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start curl for the HTTPS request: {error}"))?;
    let result = (|| {
        if let Some(token) = token {
            let mut headers = child
                .stdin
                .take()
                .ok_or("cannot send HTTPS authorization")?;
            headers
                .write_all(b"Authorization: Bearer ")
                .and_then(|()| headers.write_all(token.as_bytes()))
                .and_then(|()| headers.write_all(b"\n"))
                .map_err(|_| "cannot send HTTPS authorization")?;
        }
        let mut stream = child.stdout.take().ok_or("cannot read HTTPS response")?;
        let mut remaining = limit;
        let mut bytes = [0u8; 16 * 1024];
        loop {
            let read = stream
                .read(&mut bytes)
                .map_err(|_| "cannot read HTTPS response")?;
            if read == 0 {
                break;
            }
            remaining = remaining.checked_sub(read as u64).ok_or(Error::TooLarge)?;
            output
                .write_all(&bytes[..read])
                .map_err(|error| format!("cannot save HTTPS response: {error}"))?;
        }
        let mut status = [0; 3];
        let received_status = child
            .stderr
            .take()
            .ok_or("cannot read HTTPS status")?
            .read_exact(&mut status)
            .is_ok();
        let exit = child.wait().map_err(|_| "cannot finish HTTPS request")?;
        if exit.code() == Some(63) {
            return Err(Error::TooLarge);
        }
        if !exit.success() || !received_status || !status.iter().all(u8::is_ascii_digit) {
            return Err("HTTPS request failed; check the address and certificate".into());
        }
        match &status {
            b"200" => Ok(()),
            b"401" | b"403" => {
                Err("HTTPS authorization failed; check your token and --token-env".into())
            }
            _ => Err(format!(
                "HTTPS source returned HTTP {}",
                std::str::from_utf8(&status).unwrap()
            )
            .into()),
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
