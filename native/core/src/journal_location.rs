//! The sole production selector for every supported per-origin journal layout.
//! Existence, not row count, selects a layout. Never migrate or delete UNKNOWN.
use crate::pending_journal::PendingJournalError as Error;
use std::fs;
use std::path::{Path, PathBuf};

fn hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn directory(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() { return Err(Error::InvalidScope); }
    let metadata = fs::symlink_metadata(path).map_err(|_| Error::StoreUnavailable)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() { return Err(Error::InvalidScope); }
    Ok(())
}
fn parents(root: &Path, file: &Path) -> Result<(), Error> {
    let relative = file.strip_prefix(root).map_err(|_| Error::InvalidScope)?;
    let mut path = root.to_owned();
    for component in relative.components().take(relative.components().count().saturating_sub(1)) {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(_) => directory(&path)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(Error::StoreUnavailable),
        }
    }
    Ok(())
}
fn present(path: &Path) -> Result<bool, Error> {
    let mut any = false;
    for suffix in ["", "-wal", "-shm"] {
        let path = PathBuf::from(format!("{}{suffix}", path.to_string_lossy()));
        match fs::symlink_metadata(path) {
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() { return Err(Error::StoreUnavailable); }
                any = true;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(_) => return Err(Error::StoreUnavailable),
        }
    }
    Ok(any)
}
pub(crate) fn resolve(root: &Path, key: &str) -> Result<PathBuf, Error> {
    directory(root)?;
    if !hash(key) { return Err(Error::InvalidScope); }
    let mut paths = vec![root.join(format!("vcp-pending-v1-{key}.sqlite")), root.join(key).join("pending.sqlite")];
    // Earlier iOS layouts have a service-hash directory. Scan only bounded,
    // known-shaped names. Service replacement must never hide an existing
    // original; all new journals use the same canonical origin file.
    for (index, entry) in fs::read_dir(root).map_err(|_| Error::StoreUnavailable)?.enumerate() {
        if index >= 128 { return Err(Error::InvalidScope); }
        let entry = entry.map_err(|_| Error::StoreUnavailable)?;
        if hash(&entry.file_name().to_string_lossy()) {
            directory(&entry.path())?;
            paths.push(entry.path().join(key).join("pending.sqlite"));
        }
    }
    paths.sort(); paths.dedup();
    let mut existing = Vec::new();
    for path in paths {
        parents(root, &path)?;
        if present(&path)? { existing.push(path); }
    }
    if existing.len() > 1 { return Err(Error::MultipleStores); }
    if let Some(path) = existing.pop() {
        // Sidecars without their main file are unknown state, not an empty DB.
        if fs::symlink_metadata(&path).is_err() { return Err(Error::StoreUnavailable); }
        return Ok(path);
    }
    // All instances and compatibility constructors choose exactly this file
    // when no prior layout exists. No namespace-dependent creation race.
    Ok(root.join(format!("vcp-pending-v1-{key}.sqlite")))
}
