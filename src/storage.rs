use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::error::AppError;

pub fn read_text(path: &Path, limit: u64) -> Result<String, AppError> {
    let file = File::open(path)
        .map_err(|e| AppError::local(format!("could not read {}: {e}", path.display())))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| AppError::local(format!("could not read {}: {e}", path.display())))?;
    if bytes.len() as u64 > limit {
        return Err(AppError::local(format!(
            "{} exceeds the {limit}-byte input limit",
            path.display()
        )));
    }
    String::from_utf8(bytes)
        .map_err(|_| AppError::local(format!("{} must be UTF-8 text", path.display())))
}

pub fn atomic_write(path: &Path, text: &str) -> Result<(), AppError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|e| AppError::local(format!("could not create output directory: {e}")))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| AppError::local(format!("could not stage output: {e}")))?;
    temporary
        .write_all(text.as_bytes())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|e| AppError::local(format!("could not write output: {e}")))?;
    temporary
        .persist(path)
        .map_err(|e| AppError::local(format!("could not replace {}: {e}", path.display())))?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| AppError::local(format!("output was saved but directory sync failed: {e}")))?;
    Ok(())
}
