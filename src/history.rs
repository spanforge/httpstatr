use fs2::FileExt;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::cli::{HistoryAction, HistoryCommand};
use crate::error::AppError;
use crate::model::BatchResult;

pub const HISTORY_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HistoryRecord {
    pub history_schema_version: u8,
    pub recorded_at_unix_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub result: BatchResult,
}

pub fn append(
    path: &Path,
    result: &BatchResult,
    name: Option<String>,
    tags: Vec<String>,
    commit: Option<String>,
) -> Result<(), AppError> {
    let _lock = lock(path)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::local(format!("system clock error: {error}")))?
        .as_secs();
    let record = HistoryRecord {
        history_schema_version: HISTORY_SCHEMA_VERSION,
        recorded_at_unix_seconds: timestamp,
        name,
        tags,
        commit,
        result: result.clone(),
    };
    append_record(path, &record)
}

pub fn run(command: HistoryCommand) -> Result<(), AppError> {
    let _lock = lock(&command.path)?;
    match command.action {
        HistoryAction::List {
            last,
            name,
            url,
            tag,
            commit,
            since,
            until,
            chart,
        } => {
            let mut records = VecDeque::new();
            scan(&command.path, |record| {
                let matches = name
                    .as_ref()
                    .is_none_or(|value| record.name.as_ref() == Some(value))
                    && url.as_ref().is_none_or(|value| &record.result.url == value)
                    && tag.as_ref().is_none_or(|value| record.tags.contains(value))
                    && commit
                        .as_ref()
                        .is_none_or(|value| record.commit.as_ref() == Some(value))
                    && since.is_none_or(|value| record.recorded_at_unix_seconds >= value)
                    && until.is_none_or(|value| record.recorded_at_unix_seconds <= value);
                if matches && last > 0 {
                    if records.len() == last {
                        records.pop_front();
                    }
                    records.push_back(record);
                }
                Ok(())
            })?;
            render(records.make_contiguous(), chart);
            Ok(())
        }
        HistoryAction::Prune { keep } => {
            let mut records = VecDeque::new();
            scan(&command.path, |record| {
                if keep > 0 {
                    if records.len() == keep {
                        records.pop_front();
                    }
                    records.push_back(record);
                }
                Ok(())
            })?;
            rewrite(&command.path, records.make_contiguous())
        }
        HistoryAction::Import { source } => {
            if source == command.path
                || (source.exists()
                    && command.path.exists()
                    && fs::canonicalize(&source).ok() == fs::canonicalize(&command.path).ok())
            {
                return Err(AppError::local("cannot import history into itself"));
            }
            let _source_lock = try_lock(&source)?;
            // Validate the source before appending anything.
            scan(&source, |_| Ok(()))?;
            scan(&source, |mut record| {
                redact_record(&mut record);
                append_record(&command.path, &record)
            })
        }
        HistoryAction::Export { output } => {
            if let Some(path) = output {
                if path == command.path
                    || (path.exists()
                        && command.path.exists()
                        && fs::canonicalize(&path).ok() == fs::canonicalize(&command.path).ok())
                {
                    return Err(AppError::local("cannot export over the history source"));
                }
                ensure_parent(&path)?;
                let mut temporary = tempfile::NamedTempFile::new_in(
                    path.parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .unwrap_or(Path::new(".")),
                )
                .map_err(|e| AppError::local(e.to_string()))?;
                scan(&command.path, |record| {
                    let text = encode(&[record])?;
                    temporary
                        .write_all(text.as_bytes())
                        .map_err(|e| AppError::local(e.to_string()))
                })?;
                temporary
                    .as_file()
                    .sync_all()
                    .map_err(|e| AppError::local(e.to_string()))?;
                temporary
                    .persist(&path)
                    .map_err(|e| AppError::local(format!("could not export history: {e}")))?;
                Ok(())
            } else {
                scan(&command.path, |record| {
                    let text = encode(&[record])?;
                    std::io::stdout()
                        .write_all(text.as_bytes())
                        .map_err(|e| AppError::local(e.to_string()))
                })
            }
        }
    }
}

fn scan(
    path: &Path,
    mut visit: impl FnMut(HistoryRecord) -> Result<(), AppError>,
) -> Result<(), AppError> {
    if !path.exists() {
        return Ok(());
    }
    let file = fs::File::open(path).map_err(|error| {
        AppError::local(format!(
            "could not read history {}: {error}",
            path.display()
        ))
    })?;
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    let mut index = 0;
    loop {
        line.clear();
        let size = Read::take(&mut reader, 67_108_865)
            .read_until(b'\n', &mut line)
            .map_err(|e| AppError::local(format!("could not read history: {e}")))?;
        if size == 0 {
            break;
        }
        index += 1;
        if size > 67_108_864 {
            return Err(AppError::local("history record exceeds 64 MiB"));
        }
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let record: HistoryRecord = serde_json::from_slice(&line).map_err(|error| {
            AppError::local(format!("invalid history record at line {}: {error}", index))
        })?;
        if record.history_schema_version != HISTORY_SCHEMA_VERSION {
            return Err(AppError::local(format!(
                "unsupported history schema {} at line {}",
                record.history_schema_version, index
            )));
        }
        visit(record)?;
    }
    Ok(())
}

fn append_record(path: &Path, record: &HistoryRecord) -> Result<(), AppError> {
    ensure_parent(path)?;
    let mut line = serde_json::to_vec(record)
        .map_err(|error| AppError::local(format!("could not serialize history record: {error}")))?;
    line.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            AppError::local(format!(
                "could not open history {}: {error}",
                path.display()
            ))
        })?;
    if file
        .metadata()
        .map_err(|e| AppError::local(e.to_string()))?
        .len()
        > 0
    {
        file.seek(SeekFrom::End(-1))
            .map_err(|e| AppError::local(e.to_string()))?;
        let mut last = [0];
        file.read_exact(&mut last)
            .map_err(|e| AppError::local(e.to_string()))?;
        if last[0] != b'\n' {
            return Err(AppError::local(
                "history has an incomplete final record; preserve a backup and repair it before appending",
            ));
        }
    }
    file.write_all(&line)
        .and_then(|_| file.sync_data())
        .map_err(|error| {
            AppError::local(format!(
                "could not append history {}: {error}",
                path.display()
            ))
        })
}

fn rewrite(path: &Path, records: &[HistoryRecord]) -> Result<(), AppError> {
    crate::storage::atomic_write(path, &encode(records)?)
}

fn encode(records: &[HistoryRecord]) -> Result<String, AppError> {
    let mut output = String::new();
    for record in records {
        output.push_str(
            &serde_json::to_string(record).map_err(|error| {
                AppError::local(format!("could not serialize history: {error}"))
            })?,
        );
        output.push('\n');
    }
    Ok(output)
}

fn render(records: &[HistoryRecord], chart: bool) {
    if records.is_empty() {
        println!("No matching history records.");
        return;
    }
    println!("timestamp    name                 median      p95      p99  success  url");
    let max = records
        .iter()
        .map(|record| record.result.aggregate.total.p95)
        .fold(0.0_f64, f64::max);
    for record in records {
        let result = &record.result;
        println!(
            "{:<12} {:<20} {:>7.1}ms {:>7.1}ms {:>7.1}ms {:>7.1}%  {}",
            record.recorded_at_unix_seconds,
            record.name.as_deref().unwrap_or("-"),
            result.aggregate.total.median,
            result.aggregate.total.p95,
            result.aggregate.total.p99,
            result.summary.success_rate,
            result.url
        );
        if chart {
            let width = if max > 0.0 {
                (result.aggregate.total.p95 / max * 40.0).round() as usize
            } else {
                0
            };
            println!(
                "             p95 |{} {:.1}ms",
                "#".repeat(width),
                result.aggregate.total.p95
            );
        }
    }
}

fn ensure_parent(path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::local(format!("could not create {}: {error}", parent.display()))
        })?;
    }
    Ok(())
}

fn backup_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(".bak");
    PathBuf::from(value)
}

fn open_lock(path: &Path) -> Result<fs::File, AppError> {
    ensure_parent(path)?;
    let mut lock_path = path.as_os_str().to_os_string();
    lock_path.push(".lock");
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(PathBuf::from(lock_path))
        .map_err(|error| AppError::local(format!("could not open history lock: {error}")))
}

fn recover(path: &Path) -> Result<(), AppError> {
    let backup = backup_path(path);
    if backup.exists() {
        if path.exists() {
            fs::remove_file(&backup)
        } else {
            fs::rename(&backup, path)
        }
        .map_err(|error| {
            AppError::local(format!(
                "could not recover interrupted history replacement: {error}"
            ))
        })?;
    }
    Ok(())
}

fn lock(path: &Path) -> Result<fs::File, AppError> {
    let file = open_lock(path)?;
    FileExt::lock_exclusive(&file)
        .map_err(|error| AppError::local(format!("could not lock history: {error}")))?;
    recover(path)?;
    Ok(file)
}

fn try_lock(path: &Path) -> Result<fs::File, AppError> {
    let file = open_lock(path)?;
    FileExt::try_lock_exclusive(&file)
        .map_err(|error| AppError::local(format!("source history is in use: {error}")))?;
    recover(path)?;
    Ok(file)
}

fn redact_record(record: &mut HistoryRecord) {
    use crate::redact::*;
    let result = &mut record.result;
    let url = result.url.clone();
    let args = result.configuration.curl_args.clone();
    result.url = redact_url(&url);
    let mut redacted = redact_command("", &args);
    redacted.pop();
    result.configuration.curl_args = redacted;
    result.configuration.expect_header = result
        .configuration
        .expect_header
        .iter()
        .map(|h| redact_header_line(h))
        .collect();
    for sample in &mut result.samples {
        if let Some(response) = &mut sample.response {
            response.headers = redact_headers(&response.headers);
        }
        if let Some(assertions) = &mut sample.assertions {
            *assertions = redact_assertions(assertions);
        }
        if let Some(error) = &mut sample.error {
            *error = redact_curl_text(error, &url, &args);
        }
        for hop in &mut sample.redirects {
            hop.source_url = redact_url(&hop.source_url);
            hop.destination_url = redact_url(&hop.destination_url);
        }
        if let Some(connection) = &mut sample.connection {
            connection.effective_url = redact_url(&connection.effective_url);
        }
    }
}
