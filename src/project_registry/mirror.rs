use super::{
    name, open_regular, ordinary, private_stage, read_regular, recheck, registry_packages_path,
    valid_hash, validate_manifest, version, write, Budget, Record,
};
use crate::{https, project_package as package};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

const MAX_RECORD: usize = 8192;

pub(super) fn command(args: &[String]) -> Result<String, String> {
    let usage = "use oxid registry mirror FILE_URI PACKAGE VERSION HTTPS_BASE --sha256 HEX [--token-env NAME]";
    let [_, _, destination, library, release, base, flags @ ..] = args else {
        return Err(usage.into());
    };
    name(library)?;
    version(release)?;
    let base = base.trim_end_matches('/');
    https::check_url(base)?;
    let mut expected = None;
    let mut token_env = None;
    let mut options = flags.iter();
    while let Some(flag) = options.next() {
        let value = options
            .next()
            .filter(|word| !word.starts_with('-'))
            .ok_or(usage)?;
        match flag.as_str() {
            "--sha256" if expected.is_none() && value.len() == 64 => {
                expected = Some(value.to_ascii_lowercase());
            }
            "--token-env" if token_env.is_none() => token_env = Some(value.as_str()),
            _ => return Err(usage.into()),
        }
    }
    let expected = expected
        .filter(|hash| valid_hash(hash))
        .ok_or("registry mirror requires a 64-digit SHA-256")?;
    let mut budget = Budget::new()?;
    let source = budget.normalized_uri(destination)?;
    let packages = registry_packages_path(&source, &mut budget)?;
    ordinary(&packages, false)?;
    let parent = budget.join(&packages, library)?;
    let destination = budget.join(&parent, release)?;
    match fs::symlink_metadata(&destination) {
        Ok(_) => {
            ordinary(&destination, false)?;
            let path = budget.join(&destination, "release.toml")?;
            let raw = read_regular(&path, MAX_RECORD, &mut budget)?;
            let record = release_record(&raw, library, release, &expected, &mut budget)?;
            let artifact = budget.join(&destination, "artifact.tar")?;
            verify_artifact(&artifact, &record, &mut budget)?;
            return Ok(format!(
                "already mirrored {library} {release} (sha256:{expected})\n"
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect release destination: {error}")),
    }

    let url = format!("{base}/packages/{library}/{release}");
    let mut raw = Vec::new();
    budget.reserve(&mut raw, MAX_RECORD)?;
    download(
        &format!("{url}/release.toml"),
        token_env,
        MAX_RECORD as u64,
        30,
        &mut raw,
    )?;
    let record = release_record(&raw, library, release, &expected, &mut budget)?;
    let mut stage = private_stage(&packages, &mut budget)?;
    let artifact = budget.join(&stage.path, "artifact.tar")?;
    let mut file = package::new_file(&artifact)?;
    download(
        &format!("{url}/artifact.tar"),
        token_env,
        record.bytes,
        180,
        &mut file,
    )?;
    file.sync_all()
        .map_err(|error| format!("cannot sync downloaded release: {error}"))?;
    drop(file);
    verify_artifact(&artifact, &record, &mut budget)?;
    write(&budget.join(&stage.path, "release.toml")?, &raw)?;
    fs::File::open(&stage.path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot sync staged release: {error}"))?;

    match fs::create_dir(&parent) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => ordinary(&parent, false)?,
        Err(error) => return Err(format!("cannot create registry package directory: {error}")),
    }
    package::publish_directory(&stage.path, &destination)?;
    stage.published = true;
    for directory in [parent, packages] {
        fs::File::open(directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("release was mirrored but registry sync failed: {error}"))?;
    }
    Ok(format!(
        "mirrored {library} {release} (sha256:{expected})\n"
    ))
}

fn release_record(
    raw: &[u8],
    library: &str,
    release: &str,
    expected: &str,
    budget: &mut Budget,
) -> Result<Record, String> {
    let record = Record::decode(raw, budget)?;
    if record.name != library || record.version != release {
        return Err("registry release name/version differs from the requested identity".into());
    }
    if record.artifact != expected {
        return Err("registry release does not match the supplied SHA-256".into());
    }
    Ok(record)
}

fn verify_artifact(path: &Path, record: &Record, budget: &mut Budget) -> Result<(), String> {
    budget.wire(record.bytes)?;
    let (mut file, before) = open_regular(path, record.bytes as usize)?;
    let mut hash = Sha256::new();
    let count = io::copy(&mut (&mut file).take(record.bytes + 1), &mut hash)
        .map_err(|error| format!("cannot hash registry artifact: {error}"))?;
    recheck(path, &file, &before)?;
    if count != record.bytes || format!("{:x}", hash.finalize()) != record.artifact {
        return Err("registry artifact does not match its SHA-256/length".into());
    }
    let (archive, info) = package::read_library(path, budget)?;
    if info.bytes != record.bytes || info.sha256 != record.artifact {
        return Err("registry artifact changed during verification".into());
    }
    validate_manifest(&archive, record, budget)
}

fn download(
    url: &str,
    token_env: Option<&str>,
    limit: u64,
    seconds: u32,
    output: &mut impl Write,
) -> Result<(), String> {
    https::download(url, token_env, limit, seconds, output).map_err(|error| match error {
        https::Error::TooLarge => "HTTPS registry response exceeds the permitted length".into(),
        https::Error::Request(message) => message,
    })
}
