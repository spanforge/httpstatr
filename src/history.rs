use std::fs::{self, OpenOptions};
use std::io::Write;
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
            let mut records = read(&command.path)?;
            records.retain(|record| {
                name.as_ref()
                    .is_none_or(|value| record.name.as_ref() == Some(value))
                    && url.as_ref().is_none_or(|value| &record.result.url == value)
                    && tag.as_ref().is_none_or(|value| record.tags.contains(value))
                    && commit
                        .as_ref()
                        .is_none_or(|value| record.commit.as_ref() == Some(value))
                    && since.is_none_or(|value| record.recorded_at_unix_seconds >= value)
                    && until.is_none_or(|value| record.recorded_at_unix_seconds <= value)
            });
            let start = records.len().saturating_sub(last);
            render(&records[start..], chart);
            Ok(())
        }
        HistoryAction::Prune { keep } => {
            let records = read(&command.path)?;
            let start = records.len().saturating_sub(keep);
            rewrite(&command.path, &records[start..])
        }
        HistoryAction::Import { source } => {
            for record in read(&source)? {
                append_record(&command.path, &record)?;
            }
            Ok(())
        }
        HistoryAction::Export { output } => {
            let records = read(&command.path)?;
            let text = encode(&records)?;
            if let Some(path) = output {
                fs::write(&path, text).map_err(|error| {
                    AppError::local(format!("could not export {}: {error}", path.display()))
                })
            } else {
                print!("{text}");
                Ok(())
            }
        }
    }
}

fn read(path: &Path) -> Result<Vec<HistoryRecord>, AppError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path).map_err(|error| {
        AppError::local(format!(
            "could not read history {}: {error}",
            path.display()
        ))
    })?;
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let record: HistoryRecord = serde_json::from_str(line).map_err(|error| {
                AppError::local(format!(
                    "invalid history record at line {}: {error}",
                    index + 1
                ))
            })?;
            if record.history_schema_version != HISTORY_SCHEMA_VERSION {
                return Err(AppError::local(format!(
                    "unsupported history schema {} at line {}",
                    record.history_schema_version,
                    index + 1
                )));
            }
            Ok(record)
        })
        .collect()
}

fn append_record(path: &Path, record: &HistoryRecord) -> Result<(), AppError> {
    ensure_parent(path)?;
    let mut line = serde_json::to_vec(record)
        .map_err(|error| AppError::local(format!("could not serialize history record: {error}")))?;
    line.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            AppError::local(format!(
                "could not open history {}: {error}",
                path.display()
            ))
        })?;
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
    ensure_parent(path)?;
    let temporary = temporary_path(path);
    fs::write(&temporary, encode(records)?).map_err(|error| {
        AppError::local(format!("could not write {}: {error}", temporary.display()))
    })?;
    let backup = backup_path(path);
    let had_original = path.exists();
    if had_original {
        fs::rename(path, &backup).map_err(|error| {
            AppError::local(format!(
                "could not stage {} for replacement: {error}",
                path.display()
            ))
        })?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if had_original {
            let _ = fs::rename(&backup, path);
        }
        return Err(AppError::local(format!(
            "could not replace {}: {error}",
            path.display()
        )));
    }
    if had_original {
        fs::remove_file(&backup).map_err(|error| {
            AppError::local(format!(
                "history was replaced but backup {} could not be removed: {error}",
                backup.display()
            ))
        })?;
    }
    Ok(())
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

fn temporary_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".tmp-{}", std::process::id()));
    PathBuf::from(value)
}

fn backup_path(path: &Path) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(format!(".bak-{}", std::process::id()));
    PathBuf::from(value)
}
