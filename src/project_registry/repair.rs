use super::{
    name, ordinary, prepare_cache, read_regular, read_release_archive, registry_package_path,
    stage_cache, version, Budget, Record,
};
use crate::{project_lock, project_package as package};
use std::{fs, io, path::Path};

pub(super) fn command(args: &[String]) -> Result<String, String> {
    let [_, _, source, library, release] = args else {
        return Err("use oxid registry repair-cache FILE_URI PACKAGE VERSION".into());
    };
    name(library)?;
    version(release)?;
    let owner = crate::project_cli::registry_cache_owner()?;
    let mut budget = Budget::new()?;
    let source = budget.normalized_uri(source)?;
    let lock = project_lock::registry_trust(&owner, &mut budget)?
        .ok_or("repair-cache requires an existing oxid.lock")?;
    let pin = lock
        .registry_pin(&source, library, release)
        .ok_or("the requested registry release is absent from oxid.lock")?;

    let parent = budget.join(&owner, ".oxid/registry")?;
    let identity = budget.identity(&source, library, release)?;
    let destination = budget.join(&parent, &identity)?;
    let previous = match fs::symlink_metadata(&destination) {
        Ok(metadata) => {
            ordinary(&destination, false)?;
            Some(metadata)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot inspect registry cache: {error}")),
    };

    let package = registry_package_path(&source, library, &mut budget)?;
    let release_dir = budget.join(&package, release)?;
    ordinary(&release_dir, false)?;
    let record_path = budget.join(&release_dir, "release.toml")?;
    let raw = read_regular(&record_path, 1024 * 1024, &mut budget)?;
    let record = Record::decode(&raw, &mut budget)?;
    if record.name != pin.name
        || record.version != pin.version
        || record.artifact != pin.artifact
        || record.bytes != pin.bytes
    {
        return Err(
            "the registry release differs from the version and checksum in oxid.lock".into(),
        );
    }
    let (archive, info) = read_release_archive(&release_dir, &record, &mut budget)?;
    prepare_cache(&owner, &parent, &mut budget)?;
    let mut stage = stage_cache(&parent, &archive, &info, &raw, &mut budget)?;
    if let Some(previous) = previous {
        exchange_cache(&stage.path, &destination, &previous)?;
    } else {
        package::publish_directory(&stage.path, &destination)?;
        stage.published = true;
    }
    fs::File::open(&parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!("cache was repaired but its directory could not be synced: {error}")
        })?;
    Ok(format!(
        "restored cached {library} {release} from oxid.lock\n"
    ))
}

#[cfg(target_os = "linux")]
fn exchange_cache(stage: &Path, cache: &Path, previous: &fs::Metadata) -> Result<(), String> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt, os::unix::fs::MetadataExt};
    ordinary(cache, false)?;
    let current = fs::symlink_metadata(cache)
        .map_err(|error| format!("cannot recheck registry cache: {error}"))?;
    if !current.is_dir()
        || previous.dev() != current.dev()
        || previous.ino() != current.ino()
        || previous.ctime() != current.ctime()
        || previous.ctime_nsec() != current.ctime_nsec()
    {
        return Err("registry cache changed during repair; retry the command".into());
    }
    let stage =
        CString::new(stage.as_os_str().as_bytes()).map_err(|_| "invalid cache stage path")?;
    let cache = CString::new(cache.as_os_str().as_bytes()).map_err(|_| "invalid cache path")?;
    // Exchange the complete directories on the same filesystem. The private
    // stage guard removes the replaced cache after the new entry is installed.
    let renamed = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            stage.as_ptr(),
            libc::AT_FDCWD,
            cache.as_ptr(),
            libc::RENAME_EXCHANGE,
        )
    };
    if renamed == 0 {
        Ok(())
    } else {
        Err(format!(
            "cannot replace registry cache: {}",
            io::Error::last_os_error()
        ))
    }
}

#[cfg(not(target_os = "linux"))]
fn exchange_cache(_stage: &Path, _cache: &Path, _previous: &fs::Metadata) -> Result<(), String> {
    Err("registry cache repair requires Linux".into())
}
