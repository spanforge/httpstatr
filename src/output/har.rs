use serde_json::{Value, json};

use crate::error::AppError;
use crate::model::{BatchResult, BatchSample, Timings};

pub fn render(result: &BatchResult) -> Result<String, AppError> {
    let entries: Vec<Value> = result
        .samples
        .iter()
        .map(|sample| entry(result, sample))
        .collect();
    serde_json::to_string_pretty(&json!({
        "log": {
            "version": "1.2",
            "creator": { "name": "httpstatr", "version": env!("CARGO_PKG_VERSION") },
            "comment": "Synthetic timing export. Wall-clock start times and unavailable protocol fields are omitted.",
            "entries": entries
        }
    })).map_err(|error| AppError::local(format!("could not serialize HAR: {error}")))
}

fn entry(result: &BatchResult, sample: &BatchSample) -> Value {
    let timings = sample.timings_ms.as_ref();
    let response = sample.response.as_ref();
    json!({
        "comment": format!("httpstatr sample {}", sample.index),
        "startedDateTime": sample.started_at_unix_ms.map_or_else(|| "1970-01-01T00:00:00.000Z".to_string(), rfc3339_millis),
        "time": timings.map_or(0, |value| value.total),
        "request": { "method": request_method(&result.configuration.curl_args), "url": result.url, "httpVersion": sample.connection.as_ref().map_or("", |value| value.http_version.as_str()), "headers": [], "queryString": [], "cookies": [], "headersSize": -1, "bodySize": -1 },
        "response": { "status": response.map_or(0, |value| value.status_code), "statusText": response.map_or_else(String::new, |value| status_text(&value.status_line)), "httpVersion": sample.connection.as_ref().map_or("", |value| value.http_version.as_str()), "headers": response.map_or_else(Vec::new, |value| value.headers.iter().map(|(name, value)| json!({"name": name, "value": value})).collect()), "cookies": [], "content": {"size": sample.response_size_bytes.unwrap_or(0), "mimeType": ""}, "redirectURL": "", "headersSize": -1, "bodySize": sample.response_size_bytes.unwrap_or(0) },
        "cache": {},
        "timings": har_timings(timings),
        "serverIPAddress": response.map_or("", |value| value.remote_ip.as_str()),
        "connection": response.map_or("", |value| value.remote_port.as_str())
    })
}

fn har_timings(value: Option<&Timings>) -> Value {
    value.map_or_else(|| json!({"blocked": -1, "dns": -1, "connect": -1, "ssl": -1, "send": 0, "wait": -1, "receive": -1}), |value| json!({
        "blocked": 0, "dns": value.dns, "connect": value.connect + value.tls,
        "ssl": value.tls, "send": 0, "wait": value.server, "receive": value.transfer
    }))
}

fn request_method(args: &[String]) -> &str {
    args.windows(2)
        .find(|pair| pair[0] == "-X" || pair[0] == "--request")
        .map_or("GET", |pair| pair[1].as_str())
}

fn status_text(status_line: &str) -> String {
    status_line
        .split_whitespace()
        .skip(2)
        .collect::<Vec<_>>()
        .join(" ")
}

fn rfc3339_millis(timestamp: u64) -> String {
    let seconds = timestamp / 1_000;
    let milliseconds = timestamp % 1_000;
    let days = (seconds / 86_400) as i64;
    let seconds_of_day = seconds % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_date(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

fn civil_date(days_since_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_har_timestamp_as_rfc3339() {
        assert_eq!(rfc3339_millis(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(
            rfc3339_millis(1_704_067_200_123),
            "2024-01-01T00:00:00.123Z"
        );
    }
}
