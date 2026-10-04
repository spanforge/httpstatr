use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub time_namelookup: u64,
    pub time_connect: u64,
    pub time_pretransfer: u64,
    pub time_starttransfer: u64,
    pub time_total: u64,
    pub speed_download: f64,
    pub speed_upload: f64,
    pub remote_ip: String,
    pub remote_port: String,
    pub local_ip: String,
    pub local_port: String,
    pub effective_url: String,
    pub http_version: String,
    pub redirect_count: u32,
    pub redirect_time: u64,
    pub connection_count: u32,
}

impl Metrics {
    pub fn dns(&self) -> u64 {
        self.time_namelookup
    }

    pub fn connect(&self) -> u64 {
        self.time_connect.saturating_sub(self.time_namelookup)
    }

    pub fn tls(&self) -> u64 {
        self.time_pretransfer.saturating_sub(self.time_connect)
    }

    pub fn server(&self) -> u64 {
        self.time_starttransfer
            .saturating_sub(self.time_pretransfer)
    }

    pub fn transfer(&self) -> u64 {
        self.time_total.saturating_sub(self.time_starttransfer)
    }
}

#[derive(Clone, Debug)]
pub struct ResponseHeaders {
    pub status_line: String,
    pub status_code: u16,
    pub fields: BTreeMap<String, String>,
    pub display_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Violation {
    pub key: String,
    pub threshold_ms: u64,
    pub actual_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssertionFailure {
    pub kind: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AssertionSummary {
    pub pass: bool,
    pub failures: Vec<AssertionFailure>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct JsonResult {
    pub schema_version: u8,
    pub url: String,
    pub ok: bool,
    pub exit_code: u8,
    pub response: ResponseResult,
    pub timings_ms: Timings,
    pub speed: Speed,
    pub slo: Option<SloResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertions: Option<AssertionSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResponseResult {
    pub status_line: String,
    pub status_code: u16,
    pub remote_ip: String,
    pub remote_port: String,
    pub headers: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Timings {
    pub dns: u64,
    pub connect: u64,
    pub tls: u64,
    pub server: u64,
    pub transfer: u64,
    pub total: u64,
    pub namelookup: u64,
    pub initial_connect: u64,
    pub pretransfer: u64,
    pub starttransfer: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Speed {
    pub download_kbs: f64,
    pub upload_kbs: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SloResult {
    pub pass: bool,
    pub violations: Vec<Violation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchResult {
    pub schema_version: u8,
    pub url: String,
    pub ok: bool,
    pub exit_code: u8,
    pub warmup_count: usize,
    pub warmup_failures: usize,
    pub configuration: BatchConfiguration,
    pub summary: BatchSummary,
    pub aggregate: AggregateTimings,
    pub samples: Vec<BatchSample>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<ComparisonResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regression: Option<RegressionResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchConfiguration {
    pub repeat: usize,
    pub warmup: usize,
    pub delay_seconds: f64,
    pub connect_timeout_seconds: Option<f64>,
    pub timeout_seconds: Option<f64>,
    pub curl_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curl_version: Option<String>,
    #[serde(default)]
    pub slo: Option<String>,
    #[serde(default)]
    pub expect_status: Vec<String>,
    #[serde(default)]
    pub expect_header: Vec<String>,
    #[serde(default)]
    pub min_body_bytes: Option<u64>,
    #[serde(default)]
    pub max_body_bytes: Option<u64>,
    #[serde(default)]
    pub expect_body_contains: Vec<String>,
    #[serde(default)]
    pub expect_body_regex: Vec<String>,
    #[serde(default)]
    pub comparison_baseline: Option<String>,
    #[serde(default)]
    pub regression_rules: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchSummary {
    pub requested: usize,
    pub transport_successful: usize,
    pub passed: usize,
    pub failed: usize,
    pub success_rate: f64,
    pub duration_ms: u64,
    pub requests_per_second: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BatchSample {
    pub index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_unix_ms: Option<u64>,
    pub ok: bool,
    pub exit_code: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<ResponseResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timings_ms: Option<Timings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<Speed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slo: Option<SloResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assertions: Option<AssertionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redirects: Vec<RedirectHop>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection: Option<ConnectionDetails>,
}

impl BatchSample {
    pub fn success(
        index: usize,
        started_at_unix_ms: u64,
        result: JsonResult,
        response_size_bytes: u64,
        redirects: Vec<RedirectHop>,
        connection: ConnectionDetails,
    ) -> Self {
        Self {
            index,
            started_at_unix_ms: Some(started_at_unix_ms),
            ok: result.ok,
            exit_code: result.exit_code,
            response: Some(result.response),
            timings_ms: Some(result.timings_ms),
            speed: Some(result.speed),
            response_size_bytes: Some(response_size_bytes),
            slo: result.slo,
            assertions: result.assertions,
            error: None,
            redirects,
            connection: Some(connection),
        }
    }

    pub fn failure(index: usize, started_at_unix_ms: u64, exit_code: u8, error: String) -> Self {
        Self {
            index,
            started_at_unix_ms: Some(started_at_unix_ms),
            ok: false,
            exit_code,
            response: None,
            timings_ms: None,
            speed: None,
            response_size_bytes: None,
            slo: None,
            assertions: None,
            error: Some(error),
            redirects: Vec::new(),
            connection: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RedirectHop {
    pub index: usize,
    pub status_code: u16,
    pub source_url: String,
    pub destination_url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectionDetails {
    pub remote_ip: String,
    pub remote_port: String,
    pub local_ip: String,
    pub local_port: String,
    pub http_version: String,
    pub effective_url: String,
    pub connection_count: u32,
    pub reused: bool,
    pub redirect_count: u32,
    pub redirect_time_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub level: String,
    pub observation: String,
    pub evidence: String,
    pub suggestion: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AggregateTimings {
    pub dns: Statistics,
    pub connect: Statistics,
    pub tls: Statistics,
    pub server: Statistics,
    pub transfer: Statistics,
    pub total: Statistics,
    pub namelookup: Statistics,
    pub initial_connect: Statistics,
    pub pretransfer: Statistics,
    pub starttransfer: Statistics,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Statistics {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
    pub stddev: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ComparisonResult {
    pub baseline_url: String,
    pub metrics: BTreeMap<String, MetricDelta>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MetricDelta {
    pub baseline: f64,
    pub current: f64,
    pub absolute_change: f64,
    pub percent_change: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegressionResult {
    pub pass: bool,
    pub rules: Vec<RegressionRuleResult>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegressionRuleResult {
    pub rule: String,
    pub pass: bool,
    pub actual: f64,
    pub operator: String,
    pub threshold: f64,
    pub baseline: Option<f64>,
}

impl JsonResult {
    pub fn build(
        url: &str,
        metrics: &Metrics,
        headers: &ResponseHeaders,
        slo: Option<Vec<Violation>>,
        assertions: Option<AssertionSummary>,
        exit_code: u8,
    ) -> Self {
        Self {
            schema_version: 1,
            url: url.to_string(),
            ok: exit_code == 0,
            exit_code,
            response: ResponseResult {
                status_line: headers.status_line.clone(),
                status_code: headers.status_code,
                remote_ip: metrics.remote_ip.clone(),
                remote_port: metrics.remote_port.clone(),
                headers: headers.fields.clone(),
            },
            timings_ms: Timings {
                dns: metrics.dns(),
                connect: metrics.connect(),
                tls: metrics.tls(),
                server: metrics.server(),
                transfer: metrics.transfer(),
                total: metrics.time_total,
                namelookup: metrics.time_namelookup,
                initial_connect: metrics.time_connect,
                pretransfer: metrics.time_pretransfer,
                starttransfer: metrics.time_starttransfer,
            },
            speed: Speed {
                download_kbs: round_one_decimal(metrics.speed_download / 1024.0),
                upload_kbs: round_one_decimal(metrics.speed_upload / 1024.0),
            },
            slo: slo.map(|violations| SloResult {
                pass: violations.is_empty(),
                violations,
            }),
            assertions,
        }
    }
}

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_json_v1_fixture() {
        let metrics = Metrics {
            time_namelookup: 5,
            time_connect: 15,
            time_pretransfer: 30,
            time_starttransfer: 80,
            time_total: 100,
            speed_download: 2048.0,
            remote_ip: "192.0.2.1".into(),
            remote_port: "443".into(),
            ..Metrics::default()
        };
        let headers = ResponseHeaders {
            status_line: "HTTP/2 200".into(),
            status_code: 200,
            fields: BTreeMap::from([("Content-Type".into(), "application/json".into())]),
            display_text: String::new(),
        };
        let actual = serde_json::to_value(JsonResult::build(
            "https://example.test/",
            &metrics,
            &headers,
            None,
            None,
            0,
        ))
        .unwrap();
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/json_v1.json")).unwrap();
        assert_eq!(actual, expected);
    }
}
