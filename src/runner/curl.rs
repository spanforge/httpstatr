use std::collections::BTreeMap;
use std::env;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::AppError;
use crate::model::{Metrics, RedirectHop, ResponseHeaders};
use crate::redact::{redact_command, redact_curl_text};

use super::{Request, RunOutput, Runner};

const WRITE_OUT: &str = concat!(
    "HTTPSTATR_METRICS_BEGIN\\n",
    "time_namelookup=%{time_namelookup}\\n",
    "time_connect=%{time_connect}\\n",
    "time_appconnect=%{time_appconnect}\\n",
    "time_pretransfer=%{time_pretransfer}\\n",
    "time_redirect=%{time_redirect}\\n",
    "time_starttransfer=%{time_starttransfer}\\n",
    "time_total=%{time_total}\\n",
    "speed_download=%{speed_download}\\n",
    "speed_upload=%{speed_upload}\\n",
    "remote_ip=%{remote_ip}\\n",
    "remote_port=%{remote_port}\\n",
    "local_ip=%{local_ip}\\n",
    "local_port=%{local_port}\\n",
    "url_effective=%{url_effective}\\n",
    "http_version=%{http_version}\\n",
    "num_redirects=%{num_redirects}\\n",
    "num_connects=%{num_connects}\\n",
    "HTTPSTATR_METRICS_END\\n"
);

pub const MINIMUM_CURL_VERSION: (u32, u32, u32) = (7, 50, 0);

#[derive(Clone, Debug)]
pub struct CurlRuntime {
    pub version: String,
}

pub fn validate_installation(curl_bin: &str) -> Result<CurlRuntime, AppError> {
    let output = Command::new(curl_bin)
        .arg("--version")
        .env("LC_ALL", "C")
        .output()
        .map_err(|error| {
            AppError::local(format!(
                "could not run curl executable {curl_bin:?}: {error}. Install curl 7.50.0 or newer, or select it with --curl-bin/HTTPSTAT_CURL_BIN"
            ))
        })?;
    if !output.status.success() {
        return Err(AppError::local(format!(
            "curl version check failed for {curl_bin:?} with status {}",
            output.status
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first_line = text.lines().next().unwrap_or_default();
    let version = first_line.split_whitespace().nth(1).ok_or_else(|| {
        AppError::local(format!("could not parse curl version from {first_line:?}"))
    })?;
    let parsed = parse_version(version).ok_or_else(|| {
        AppError::local(format!("could not parse curl version number {version:?}"))
    })?;
    if parsed < MINIMUM_CURL_VERSION {
        return Err(AppError::local(format!(
            "curl {version} is unsupported; httpstatr requires curl 7.50.0 or newer"
        )));
    }
    let supports_http = text.lines().any(|line| {
        line.strip_prefix("Protocols:")
            .is_some_and(|protocols| protocols.split_whitespace().any(|value| value == "http"))
    });
    if !supports_http {
        return Err(AppError::local(format!(
            "curl {version} does not report HTTP protocol support"
        )));
    }
    Ok(CurlRuntime {
        version: version.to_string(),
    })
}

fn parse_version(value: &str) -> Option<(u32, u32, u32)> {
    let numeric = value.split(['-', '_']).next()?;
    let mut parts = numeric.split('.');
    Some((
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next().unwrap_or("0").parse().ok()?,
    ))
}

pub struct CurlRunner;

impl Runner for CurlRunner {
    fn execute(&self, request: &Request<'_>) -> Result<RunOutput, AppError> {
        let body_path = unique_temp_path("body")?;
        let header_path = unique_temp_path("headers")?;
        reserve_file(&body_path)?;
        if let Err(error) = reserve_file(&header_path) {
            let _ = fs::remove_file(&body_path);
            return Err(error);
        }
        let result = execute_inner(request, &body_path, &header_path);
        let _ = fs::remove_file(&header_path);
        if result.is_err() {
            let _ = fs::remove_file(&body_path);
        }
        result
    }
}

fn execute_inner(
    request: &Request<'_>,
    body_path: &Path,
    header_path: &Path,
) -> Result<RunOutput, AppError> {
    let mut command = Command::new(request.curl_bin);
    command
        .arg("-w")
        .arg(WRITE_OUT)
        .arg("-D")
        .arg(header_path)
        .arg("-o")
        .arg(body_path)
        .arg("-s")
        .arg("-S");
    if let Some(seconds) = request.connect_timeout {
        command.arg("--connect-timeout").arg(seconds.to_string());
    }
    if let Some(seconds) = request.timeout {
        command.arg("--max-time").arg(seconds.to_string());
    }
    command
        .args(request.curl_args)
        .arg(request.url)
        .env("LC_ALL", "C");

    if request.debug {
        let args = if request.show_secrets {
            request
                .curl_args
                .iter()
                .cloned()
                .chain(std::iter::once(request.url.to_string()))
                .collect()
        } else {
            redact_command(request.url, request.curl_args)
        };
        eprintln!("cmd: {:?} {}", request.curl_bin, args.join(" "));
    }
    let output = command.output().map_err(|error| {
        AppError::local(format!(
            "could not run curl executable {:?}: {error}",
            request.curl_bin
        ))
    })?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = if request.show_secrets {
        stderr.into_owned()
    } else {
        redact_curl_text(&stderr, request.url, request.curl_args)
    };
    if !output.status.success() {
        let code = output.status.code().unwrap_or(1).clamp(1, 255) as u8;
        return Err(AppError::new(
            format!("curl failed with exit code {code}: {}", stderr.trim()),
            code,
        ));
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    let metrics = parse_metrics(&String::from_utf8_lossy(&output.stdout))?;
    let headers_text = fs::read_to_string(header_path)
        .map_err(|error| AppError::local(format!("could not read response headers: {error}")))?;
    let (headers, redirects) = parse_headers(&headers_text, request.url);
    Ok(RunOutput {
        metrics,
        headers,
        body_path: body_path.to_path_buf(),
        redirects,
    })
}

fn reserve_file(path: &Path) -> Result<(), AppError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map(|_| ())
        .map_err(|error| {
            AppError::local(format!(
                "could not create temporary file {}: {error}",
                path.display()
            ))
        })
}

fn unique_temp_path(kind: &str) -> Result<PathBuf, AppError> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::local(format!("system clock error: {error}")))?
        .as_nanos();
    Ok(env::temp_dir().join(format!(
        "httpstatr-{kind}-{}-{nanos}.tmp",
        std::process::id()
    )))
}

fn seconds_to_ms(value: &str, key: &str) -> Result<u64, AppError> {
    let seconds = value
        .parse::<f64>()
        .map_err(|_| AppError::local(format!("curl returned invalid {key} value: {value:?}")))?;
    Ok((seconds * 1000.0).trunc().max(0.0) as u64)
}

fn parse_metrics(text: &str) -> Result<Metrics, AppError> {
    let start = text
        .find("HTTPSTATR_METRICS_BEGIN")
        .ok_or_else(|| AppError::local("curl output did not contain timing metrics"))?;
    let section = &text[start..];
    let mut values = BTreeMap::new();
    for line in section.lines().skip(1) {
        if line == "HTTPSTATR_METRICS_END" {
            break;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key, value);
        }
    }
    let get = |key: &str| {
        values
            .get(key)
            .copied()
            .ok_or_else(|| AppError::local(format!("curl omitted metric {key}")))
    };
    Ok(Metrics {
        time_namelookup: seconds_to_ms(get("time_namelookup")?, "time_namelookup")?,
        time_connect: seconds_to_ms(get("time_connect")?, "time_connect")?,
        time_pretransfer: seconds_to_ms(get("time_pretransfer")?, "time_pretransfer")?,
        time_starttransfer: seconds_to_ms(get("time_starttransfer")?, "time_starttransfer")?,
        time_total: seconds_to_ms(get("time_total")?, "time_total")?,
        speed_download: get("speed_download")?.parse().unwrap_or(0.0),
        speed_upload: get("speed_upload")?.parse().unwrap_or(0.0),
        remote_ip: get("remote_ip")?.to_string(),
        remote_port: get("remote_port")?.to_string(),
        local_ip: get("local_ip")?.to_string(),
        local_port: get("local_port")?.to_string(),
        effective_url: get("url_effective")?.to_string(),
        http_version: get("http_version")?.to_string(),
        redirect_count: get("num_redirects")?.parse().unwrap_or(0),
        redirect_time: seconds_to_ms(get("time_redirect")?, "time_redirect")?,
        connection_count: get("num_connects")?.parse().unwrap_or(0),
    })
}

fn parse_headers(text: &str, request_url: &str) -> (ResponseHeaders, Vec<RedirectHop>) {
    let normalized = text.replace("\r\n", "\n");
    let blocks: Vec<&str> = normalized
        .split("\n\n")
        .filter(|block| block.trim_start().starts_with("HTTP/"))
        .collect();
    let block = blocks.last().copied().unwrap_or(normalized.trim());
    let mut source = request_url.to_string();
    let mut redirects = Vec::new();
    for candidate in blocks.iter().take(blocks.len().saturating_sub(1)) {
        let mut lines = candidate.lines();
        let status = lines
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let destination = lines
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.trim().eq_ignore_ascii_case("location"))
            .map(|(_, value)| value.trim().to_string());
        if (300..400).contains(&status) {
            if let Some(destination) = destination {
                redirects.push(RedirectHop {
                    index: redirects.len() + 1,
                    status_code: status,
                    source_url: source,
                    destination_url: destination.clone(),
                });
                source = destination;
            }
        }
    }
    let mut lines = block.lines();
    let status_line = lines.next().unwrap_or("").trim().to_string();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|part| part.parse().ok())
        .unwrap_or(0);
    let mut fields = BTreeMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    (
        ResponseHeaders {
            status_line,
            status_code,
            fields,
            display_text: block.trim().to_string(),
        },
        redirects,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_final_header_block() {
        let (headers, _) = parse_headers(
            "HTTP/1.1 200 Connection established\r\n\r\nHTTP/2 204\r\nServer: test\r\n\r\n",
            "https://example.test",
        );
        assert_eq!(headers.status_code, 204);
        assert_eq!(headers.fields["Server"], "test");
    }

    #[test]
    fn parses_curl_metrics() {
        let text = "HTTPSTATR_METRICS_BEGIN\ntime_namelookup=0.001200\ntime_connect=0.003900\ntime_pretransfer=0.007000\ntime_redirect=0\ntime_starttransfer=0.020000\ntime_total=0.025900\nspeed_download=2048\nspeed_upload=0\nremote_ip=127.0.0.1\nremote_port=443\nlocal_ip=127.0.0.2\nlocal_port=1234\nurl_effective=https://example.test\nhttp_version=2\nnum_redirects=0\nnum_connects=1\nHTTPSTATR_METRICS_END\n";
        let metrics = parse_metrics(text).unwrap();
        assert_eq!(metrics.time_total, 25);
        assert_eq!(metrics.connect(), 2);
    }

    #[test]
    fn parses_and_orders_curl_versions() {
        assert_eq!(parse_version("8.12.1"), Some((8, 12, 1)));
        assert_eq!(parse_version("7.50.0-DEV"), Some((7, 50, 0)));
        assert!(parse_version("invalid").is_none());
        assert!((7, 49, 1) < MINIMUM_CURL_VERSION);
    }
}
